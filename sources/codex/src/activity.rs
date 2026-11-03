//! What a Codex log tells about where the work stopped, for a handoff note.

use std::path::Path;

use anyhow::Result;
use serde_json::Value;
use tokprof_core::source::Activity;
use tokprof_logio::parse_ts_ms;

pub(crate) fn read(path: &Path) -> Result<Activity> {
    let mut e = Activity::default();
    tokprof_logio::for_each_line(path, |line| {
        let Ok(v) = serde_json::from_str::<Value>(line) else { return };
        codex_line(&mut e, &v);
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

fn codex_line(e: &mut Activity, v: &Value) {
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
            let ts = parse_ts_ms(v.get("timestamp"));
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

fn codex_tool(e: &mut Activity, name: &str, raw: &str) {
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
                .map(|t| {
                    (
                        t.get("status").and_then(Value::as_str).unwrap_or_default().to_string(),
                        t.get("step").and_then(Value::as_str).unwrap_or_default().to_string(),
                    )
                })
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
