//! What a Claude Code log tells about where the work stopped, for a handoff note.

use std::path::Path;

use anyhow::Result;
use serde_json::Value;
use tokprof_core::source::Activity;
use tokprof_logio::parse_ts_ms;

pub(crate) fn read(path: &Path) -> Result<Activity> {
    let mut e = Activity::default();
    tokprof_logio::for_each_line(path, |line| {
        let Ok(v) = serde_json::from_str::<Value>(line) else { return };
        claude_line(&mut e, &v);
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

fn claude_line(e: &mut Activity, v: &Value) {
    let kind = v.get("type").and_then(Value::as_str);
    if matches!(kind, Some("user" | "assistant")) {
        e.seen(parse_ts_ms(v.get("timestamp")));
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
                // An interruption is logged as a user line too: `[Request interrupted by user]`.
                None if text.starts_with('<') || text.starts_with("[Request interrupted") => return,
                None => text,
            };
            if !text.is_empty() {
                e.prompts.push((parse_ts_ms(v.get("timestamp")).unwrap_or(0), text));
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

fn claude_tool(e: &mut Activity, name: &str, input: &Value) {
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
                .map(|t| {
                    (
                        t.get("status").and_then(Value::as_str).unwrap_or_default().to_string(),
                        t.get("content").and_then(Value::as_str).unwrap_or_default().to_string(),
                    )
                })
                .collect();
        }
        _ => {}
    }
}
