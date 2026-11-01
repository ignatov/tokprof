//! `tokprof handoff`: what a new session needs to carry on from an earlier one.
//!
//! By default that is one line with the earlier session's id and the path of its log. In the
//! reference bench (docs/reference-bench.md), Claude Code and Codex each picked up a cold
//! session from that line. They pulled what they needed out of the log with a short script, for
//! a fraction of what resuming costs. With `--note`, the line comes with a note built from the
//! log without a model, for an agent that cannot read the log itself: the task, the latest
//! prompts, where the work stopped, the task list, the files changed and read, and the latest
//! commands.

use std::collections::HashMap;
use std::fmt::Write as _;
use std::io::Write as _;
use std::process::{Command, Stdio};

use anyhow::Result;
use serde_json::Value;

use crate::adapters::{self, SourceFile};
use crate::model::Agent;
use crate::xray::local_time;

/// The session id: the transcript's name for Claude Code, the end of the rollout's name for
/// Codex.
pub fn session_id(file: &SourceFile) -> String {
    match file.agent {
        Agent::Claude => file.path.file_stem().and_then(|s| s.to_str()).unwrap_or_default().to_string(),
        Agent::Codex => adapters::codex::id_from_name(&file.path),
    }
}

/// The line to start a new session with.
pub fn line(file: &SourceFile) -> String {
    format!("Continue from the earlier session {}. Its log: {}.", session_id(file), adapters::display_path(&file.path.to_string_lossy()))
}

/// The line and a note about the session.
pub fn with_note(file: &SourceFile) -> Result<String> {
    let id = session_id(file);
    let log = adapters::display_path(&file.path.to_string_lossy());
    Ok(format!("Continue from the earlier session {id}. A note about it follows; its log is {log}.\n\n{}", note(file)?))
}

/// What the log tells about where the session stopped.
#[derive(Default)]
struct Earlier {
    cwd: Option<String>,
    branch: Option<String>,
    first_ms: Option<i64>,
    last_ms: Option<i64>,
    prompts: Vec<(i64, String)>,
    last_text: Option<String>,
    todos: Vec<(String, String)>,
    changed: Vec<String>,
    /// Read count and the order a file was first read in, for stable ties.
    read: HashMap<String, (usize, usize)>,
    commands: Vec<String>,
}

impl Earlier {
    fn seen(&mut self, ts_ms: Option<i64>) {
        if let Some(t) = ts_ms {
            self.first_ms.get_or_insert(t);
            self.last_ms = Some(t);
        }
    }

    fn read_file(&mut self, path: &str) {
        let order = self.read.len();
        self.read.entry(path.to_string()).or_insert((0, order)).0 += 1;
    }
}

fn read(file: &SourceFile) -> Result<Earlier> {
    let mut e = Earlier::default();
    adapters::for_each_line(&file.path, |line| {
        let Ok(v) = serde_json::from_str::<Value>(line) else { return };
        match file.agent {
            Agent::Claude => claude_line(&mut e, &v),
            Agent::Codex => codex_line(&mut e, &v),
        }
    })?;
    Ok(e)
}

/// The text of a message: a string, or the text blocks of a list.
fn text_of(content: Option<&Value>) -> String {
    match content {
        Some(Value::String(s)) => s.trim().to_string(),
        Some(Value::Array(blocks)) => blocks
            .iter()
            .filter(|b| matches!(b.get("type").and_then(Value::as_str), Some("text" | "input_text" | "output_text")))
            .filter_map(|b| b.get("text").and_then(Value::as_str))
            .collect::<Vec<_>>()
            .join("\n")
            .trim()
            .to_string(),
        _ => String::new(),
    }
}

/// The text between `<tag>` and `</tag>`.
fn between<'a>(text: &'a str, tag: &str) -> Option<&'a str> {
    let start = text.find(&format!("<{tag}>"))? + tag.len() + 2;
    let end = text[start..].find(&format!("</{tag}>"))? + start;
    Some(text[start..end].trim())
}

fn claude_line(e: &mut Earlier, v: &Value) {
    let kind = v.get("type").and_then(Value::as_str);
    if matches!(kind, Some("user" | "assistant")) {
        e.seen(adapters::parse_ts_ms(v.get("timestamp")));
    }
    if e.cwd.is_none() {
        e.cwd = v.get("cwd").and_then(Value::as_str).map(str::to_string);
    }
    if let Some(b) = v.get("gitBranch").and_then(Value::as_str).filter(|b| !b.is_empty()) {
        e.branch = Some(b.to_string());
    }
    if v.get("isSidechain").and_then(Value::as_bool) == Some(true) {
        return;
    }
    let flag = |k: &str| v.get(k).and_then(Value::as_bool) == Some(true);
    let content = v.get("message").and_then(|m| m.get("content"));
    match kind {
        Some("user") if !flag("isMeta") && !flag("isCompactSummary") => {
            let text = text_of(content);
            // Slash commands are logged as tags; anything else in tags is not typed by a person.
            let text = match between(&text, "command-name") {
                Some(name) => [name, between(&text, "command-args").unwrap_or_default()].join(" ").trim().to_string(),
                None if text.starts_with('<') => return,
                None => text,
            };
            if !text.is_empty() {
                e.prompts.push((adapters::parse_ts_ms(v.get("timestamp")).unwrap_or(0), text));
            }
        }
        Some("assistant") => {
            for b in content.and_then(Value::as_array).into_iter().flatten() {
                match b.get("type").and_then(Value::as_str) {
                    Some("text") => {
                        if let Some(t) = b.get("text").and_then(Value::as_str).map(str::trim).filter(|t| !t.is_empty()) {
                            e.last_text = Some(t.to_string());
                        }
                    }
                    Some("tool_use") => claude_tool(e, b.get("name").and_then(Value::as_str).unwrap_or_default(), b.get("input").unwrap_or(&Value::Null)),
                    _ => {}
                }
            }
        }
        _ => {}
    }
}

fn claude_tool(e: &mut Earlier, name: &str, input: &Value) {
    let s = |k: &str| input.get(k).and_then(Value::as_str);
    let file = s("file_path").or(s("notebook_path"));
    match (name, file) {
        ("Edit" | "Write" | "MultiEdit" | "NotebookEdit", Some(f)) => e.changed.push(f.to_string()),
        ("Read", Some(f)) => e.read_file(f),
        ("Bash", _) => e.commands.extend(s("command").map(str::to_string)),
        ("TodoWrite", _) => {
            e.todos = input
                .get("todos")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .map(|t| (t.get("status").and_then(Value::as_str).unwrap_or_default().to_string(), t.get("content").and_then(Value::as_str).unwrap_or_default().to_string()))
                .collect();
        }
        _ => {}
    }
}

fn codex_line(e: &mut Earlier, v: &Value) {
    let p = v.get("payload").unwrap_or(&Value::Null);
    match v.get("type").and_then(Value::as_str) {
        Some("session_meta") => {
            if e.cwd.is_none() {
                e.cwd = p.get("cwd").and_then(Value::as_str).map(str::to_string);
            }
            // Codex records the branch when the session starts.
            if e.branch.is_none() {
                e.branch = p.get("git").and_then(|g| g.get("branch")).and_then(Value::as_str).map(|b| format!("{b} at the start"));
            }
        }
        Some("response_item") => {
            let ts = adapters::parse_ts_ms(v.get("timestamp"));
            e.seen(ts);
            match p.get("type").and_then(Value::as_str) {
                Some("message") => {
                    let text = text_of(p.get("content"));
                    match p.get("role").and_then(Value::as_str) {
                        Some("user") if !text.is_empty() && !text.starts_with('<') && !text.starts_with("# AGENTS.md") => {
                            e.prompts.push((ts.unwrap_or(0), text));
                        }
                        Some("assistant") if !text.is_empty() => e.last_text = Some(text),
                        _ => {}
                    }
                }
                Some("function_call" | "custom_tool_call") => {
                    let raw = p.get("arguments").or(p.get("input")).and_then(Value::as_str).unwrap_or_default();
                    codex_tool(e, p.get("name").and_then(Value::as_str).unwrap_or_default(), raw);
                }
                _ => {}
            }
        }
        _ => {}
    }
}

fn codex_tool(e: &mut Earlier, name: &str, raw: &str) {
    let args: Value = if raw.trim_start().starts_with('{') { serde_json::from_str(raw).unwrap_or(Value::Null) } else { Value::Null };
    match name {
        "exec_command" | "shell" => match args.get("cmd").or(args.get("command")) {
            Some(Value::String(c)) => e.commands.push(c.clone()),
            Some(Value::Array(parts)) => {
                let parts: Vec<&str> = parts.iter().filter_map(Value::as_str).collect();
                let cmd = match parts.as_slice() {
                    [_, flag, script] if flag.starts_with('-') => script.to_string(),
                    _ => parts.join(" "),
                };
                e.commands.push(cmd);
            }
            _ => {}
        },
        "update_plan" => {
            e.todos = args
                .get("plan")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .map(|t| (t.get("status").and_then(Value::as_str).unwrap_or_default().to_string(), t.get("step").and_then(Value::as_str).unwrap_or_default().to_string()))
                .collect();
        }
        _ => {
            // apply_patch, or the JavaScript of the `exec` tool that calls the real tools.
            let text = args.get("input").and_then(Value::as_str).unwrap_or(raw);
            for marker in ["*** Update File: ", "*** Add File: ", "*** Delete File: "] {
                for (at, _) in text.match_indices(marker) {
                    let rest = &text[at + marker.len()..];
                    let end = rest.find(['\n', '\\', '"', '\'', '`']).unwrap_or(rest.len());
                    e.changed.push(rest[..end].trim().to_string());
                }
            }
            for (at, _) in text.match_indices("cmd:") {
                if let Some(cmd) = js_string(&text[at + 4..]) {
                    e.commands.push(cmd);
                }
            }
        }
    }
}

/// The JavaScript string literal at the start of `s`, unescaped.
fn js_string(s: &str) -> Option<String> {
    let s = s.trim_start();
    let quote = s.chars().next().filter(|c| matches!(c, '"' | '\'' | '`'))?;
    let mut out = String::new();
    let mut chars = s[1..].chars();
    while let Some(c) = chars.next() {
        match c {
            '\\' => match chars.next()? {
                'n' => out.push('\n'),
                't' => out.push('\t'),
                other => out.push(other),
            },
            c if c == quote => return Some(out),
            c => out.push(c),
        }
    }
    None
}

/// Text on one line, cut to `n` characters.
fn clip(text: &str, n: usize) -> String {
    let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if text.chars().count() <= n {
        return text;
    }
    let cut: String = text.chars().take(n - 1).collect();
    format!("{cut}…")
}

/// Text cut to `n` characters with its line breaks kept.
fn cut(text: &str, n: usize) -> String {
    if text.chars().count() <= n {
        return text.to_string();
    }
    let cut: String = text.chars().take(n - 1).collect();
    format!("{}…", cut.trim_end())
}

/// A note about the session, built from its log without a model.
pub fn note(file: &SourceFile) -> Result<String> {
    let e = read(file)?;
    let cwd = e.cwd.as_deref();
    let shown = |path: &str| adapters::project_path(path, cwd);
    let when = |ts: Option<i64>| ts.map_or("?".to_string(), |t| local_time(t, "%Y-%m-%d %H:%M"));
    let agent = match file.agent {
        Agent::Claude => "Claude Code",
        Agent::Codex => "Codex",
    };
    let id = session_id(file);
    let mut out = String::new();
    let _ = writeln!(out, "# Handoff from {agent} session {}\n", id.chars().take(8).collect::<String>());
    let branch = e.branch.as_deref().map_or(String::new(), |b| format!(", branch {b}"));
    let _ = writeln!(out, "Project {}{branch}; {} → {}.", cwd.map_or("?".to_string(), adapters::display_path), when(e.first_ms), when(e.last_ms));
    let _ = writeln!(out, "The full log is {:.1} MB: search it only for details this note lacks.", file.size as f64 / 1e6);
    if let Some((_, task)) = e.prompts.first() {
        let _ = writeln!(out, "\n## Task\n{}", clip(task, 900));
    }
    let latest: Vec<&(i64, String)> = e.prompts.iter().skip(1).rev().take(6).collect();
    if !latest.is_empty() {
        let _ = writeln!(out, "\n## Latest prompts");
        for (ts, p) in latest.into_iter().rev() {
            let _ = writeln!(out, "- {} {}", local_time(*ts, "%m-%d %H:%M"), clip(p, 300));
        }
    }
    if let Some(text) = &e.last_text {
        let _ = writeln!(out, "\n## Where it stopped\n{}", cut(text, 2500));
    }
    if !e.todos.is_empty() {
        let _ = writeln!(out, "\n## Task list");
        for (status, t) in &e.todos {
            let mark = match status.as_str() {
                "completed" => "x",
                "in_progress" => "~",
                _ => " ",
            };
            let _ = writeln!(out, "- [{mark}] {}", clip(t, 160));
        }
    }
    let mut changed: Vec<&String> = Vec::new();
    for f in e.changed.iter().rev() {
        if !changed.contains(&f) && changed.len() < 15 {
            changed.push(f);
        }
    }
    if !changed.is_empty() {
        let _ = writeln!(out, "\n## Files changed, latest first");
        for f in changed {
            let _ = writeln!(out, "- {}", shown(f));
        }
    }
    if !e.read.is_empty() {
        let mut read: Vec<(&String, &(usize, usize))> = e.read.iter().collect();
        read.sort_by_key(|(_, (n, order))| (std::cmp::Reverse(*n), *order));
        let _ = writeln!(out, "\n## Files read most");
        for (f, (n, _)) in read.into_iter().take(8) {
            let _ = writeln!(out, "- {} ×{n}", shown(f));
        }
    }
    let mut commands: Vec<String> = Vec::new();
    for c in e.commands.iter().rev() {
        let c = clip(c.lines().find(|l| !l.trim().is_empty()).unwrap_or_default(), 140);
        if !c.is_empty() && !commands.contains(&c) && commands.len() < 8 {
            commands.push(c);
        }
    }
    if !commands.is_empty() {
        let _ = writeln!(out, "\n## Latest commands");
        for c in commands {
            let _ = writeln!(out, "- `{c}`");
        }
    }
    Ok(out)
}

/// Copies text to the clipboard; returns the tool that did it, or `None` if none worked.
pub fn copy(text: &str) -> Option<&'static str> {
    let tools: [(&str, &[&str]); 3] = [("pbcopy", &[]), ("wl-copy", &[]), ("xclip", &["-selection", "clipboard"])];
    for (tool, args) in tools {
        let Ok(mut child) = Command::new(tool).args(args).stdin(Stdio::piped()).stdout(Stdio::null()).stderr(Stdio::null()).spawn() else {
            continue;
        };
        if let Some(mut stdin) = child.stdin.take() {
            let _ = stdin.write_all(text.as_bytes());
        }
        if child.wait().is_ok_and(|s| s.success()) {
            return Some(tool);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn log(dir: &Path, name: &str, agent: Agent, lines: &[Value]) -> SourceFile {
        let path = dir.join(name);
        let text: String = lines.iter().map(|l| format!("{l}\n")).collect();
        std::fs::write(&path, &text).unwrap();
        SourceFile { agent, path, size: text.len() as u64, modified: std::time::SystemTime::now() }
    }

    fn dir(name: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("tokprof-handoff-{name}-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn a_claude_note_keeps_the_task_the_stop_and_the_files() {
        let d = dir("claude");
        let at = |m: u32| format!("2026-10-27T10:{m:02}:00Z");
        let user = |m, text: &str| serde_json::json!({"type": "user", "timestamp": at(m), "cwd": "/w/p", "gitBranch": "main", "message": {"content": text}});
        let tool = |m, name: &str, input: Value| serde_json::json!({"type": "assistant", "timestamp": at(m), "message": {"content": [{"type": "tool_use", "name": name, "input": input}]}});
        let file = log(&d, "5e5510f0-0000-0000-0000-000000000000.jsonl", Agent::Claude, &[
            user(0, "fix the flaky test in parser"),
            serde_json::json!({"type": "user", "timestamp": at(1), "message": {"content": "<command-name>/model</command-name><command-args>opus</command-args>"}}),
            serde_json::json!({"type": "user", "timestamp": at(2), "message": {"content": "<system-reminder>not typed</system-reminder>"}}),
            tool(3, "Read", serde_json::json!({"file_path": "/w/p/src/parser.rs"})),
            tool(4, "Edit", serde_json::json!({"file_path": "/w/p/src/parser.rs"})),
            tool(5, "Bash", serde_json::json!({"command": "cargo test parser\necho done"})),
            tool(6, "TodoWrite", serde_json::json!({"todos": [{"status": "completed", "content": "find the race"}, {"status": "pending", "content": "add a regression test"}]})),
            serde_json::json!({"type": "assistant", "timestamp": at(7), "isSidechain": true, "message": {"content": [{"type": "text", "text": "a subagent's words"}]}}),
            serde_json::json!({"type": "assistant", "timestamp": at(8), "message": {"content": [{"type": "text", "text": "Fixed the race; the regression test is left."}]}}),
        ]);
        let note = note(&file).unwrap();
        assert!(note.starts_with("# Handoff from Claude Code session 5e5510f0"), "{note}");
        assert!(note.contains("branch main"), "{note}");
        assert!(note.contains("## Task\nfix the flaky test in parser"), "{note}");
        assert!(note.contains(" /model opus\n"), "{note}");
        assert!(!note.contains("not typed") && !note.contains("a subagent's words"), "{note}");
        assert!(note.contains("## Where it stopped\nFixed the race; the regression test is left."), "{note}");
        assert!(note.contains("- [x] find the race\n- [ ] add a regression test"), "{note}");
        assert!(note.contains("## Files changed, latest first\n- src/parser.rs"), "{note}");
        assert!(note.contains("## Files read most\n- src/parser.rs ×1"), "{note}");
        assert!(note.contains("- `cargo test parser`"), "{note}");
        assert!(line(&file).starts_with("Continue from the earlier session 5e5510f0-0000-0000-0000-000000000000. Its log: "));
        std::fs::remove_dir_all(d).unwrap();
    }

    #[test]
    fn a_codex_note_reads_patches_commands_and_the_plan() {
        let d = dir("codex");
        let item = |payload: Value| serde_json::json!({"type": "response_item", "timestamp": "2026-08-12T11:44:00Z", "payload": payload});
        let file = log(&d, "rollout-2026-08-12T13-44-47-019ff5ca-0db2-7180-8f61-e241e484f21a.jsonl", Agent::Codex, &[
            serde_json::json!({"type": "session_meta", "payload": {"id": "019ff5ca", "cwd": "/w/u", "git": {"branch": "master"}}}),
            item(serde_json::json!({"type": "message", "role": "user", "content": [{"type": "input_text", "text": "# AGENTS.md instructions"}]})),
            item(serde_json::json!({"type": "message", "role": "user", "content": [{"type": "input_text", "text": "sync the specs with the code"}]})),
            item(serde_json::json!({"type": "function_call", "name": "exec_command", "arguments": "{\"cmd\":\"git status --short\"}"})),
            item(serde_json::json!({"type": "custom_tool_call", "name": "exec", "input": "await tools.exec_command({cmd: \"rg -n Done spec\"}); await tools.apply_patch(\"*** Begin Patch\\n*** Update File: spec/a.md\\n\");"})),
            item(serde_json::json!({"type": "function_call", "name": "update_plan", "arguments": "{\"plan\":[{\"status\":\"in_progress\",\"step\":\"group the conflicts\"}]}"})),
            item(serde_json::json!({"type": "message", "role": "assistant", "content": [{"type": "output_text", "text": "Specs updated, code unchanged."}]})),
        ]);
        let note = note(&file).unwrap();
        assert!(note.starts_with("# Handoff from Codex session 019ff5ca"), "{note}");
        assert!(note.contains("branch master at the start"), "{note}");
        assert!(note.contains("## Task\nsync the specs with the code"), "{note}");
        assert!(note.contains("## Where it stopped\nSpecs updated, code unchanged."), "{note}");
        assert!(note.contains("- [~] group the conflicts"), "{note}");
        assert!(note.contains("- spec/a.md"), "{note}");
        assert!(note.contains("- `rg -n Done spec`\n- `git status --short`"), "{note}");
        assert_eq!(session_id(&file), "019ff5ca-0db2-7180-8f61-e241e484f21a");
        std::fs::remove_dir_all(d).unwrap();
    }
}
