//! The handoff note and line of Claude Code transcripts, and the X-ray's file handling, read
//! through the catalog.

use std::path::{Path, PathBuf};

use serde_json::Value;
use tokprof_core::catalog::Catalog;
use tokprof_core::handoff::{line, note};
use tokprof_core::pricing::Pricing;
use tokprof_core::source::SessionRef;
use tokprof_core::query::Query;
use tokprof_core::xray::{self, SessionRow};
use tokprof_source_claude_code::ClaudeCode;

fn catalog(dir: &Path) -> Catalog {
    Catalog::new(vec![Box::new(ClaudeCode::at(dir.to_path_buf(), Path::new("/h")))], PathBuf::from("/h"))
}

fn dir(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("tokprof-handoff-{name}-{}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn log(c: &Catalog, dir: &Path, name: &str, lines: &[Value]) -> SessionRef {
    let path = dir.join(name);
    let text: String = lines.iter().map(|l| format!("{l}\n")).collect();
    std::fs::write(&path, &text).unwrap();
    c.resolve(&path).unwrap()
}

#[test]
fn a_claude_note_keeps_the_task_the_stop_and_the_files() {
    let d = dir("claude");
    let c = catalog(&d);
    let at = |m: u32| format!("2026-10-27T10:{m:02}:00Z");
    let user = |m, text: &str| serde_json::json!({"type": "user", "timestamp": at(m), "cwd": "/w/p", "gitBranch": "main", "message": {"content": text}});
    let tool = |m, name: &str, input: Value| serde_json::json!({"type": "assistant", "timestamp": at(m), "message": {"content": [{"type": "tool_use", "name": name, "input": input}]}});
    let file = log(&c, &d, "5e5510f0-0000-0000-0000-000000000000.jsonl", &[
        user(0, "fix the flaky test in parser"),
        serde_json::json!({"type": "user", "timestamp": at(1), "message": {"content": "<command-name>/model</command-name><command-args>opus</command-args>"}}),
        serde_json::json!({"type": "user", "timestamp": at(2), "message": {"content": "<system-reminder>not typed</system-reminder>"}}),
        serde_json::json!({"type": "user", "timestamp": at(2), "message": {"content": "[Request interrupted by user for tool use]"}}),
        tool(3, "Read", serde_json::json!({"file_path": "/w/p/src/parser.rs"})),
        tool(4, "Edit", serde_json::json!({"file_path": "/w/p/src/parser.rs"})),
        tool(5, "Bash", serde_json::json!({"command": "cargo test parser\necho done"})),
        tool(6, "TodoWrite", serde_json::json!({"todos": [{"status": "completed", "content": "find the race"}, {"status": "pending", "content": "add a regression test"}]})),
        serde_json::json!({"type": "assistant", "timestamp": at(7), "isSidechain": true, "message": {"content": [{"type": "text", "text": "a subagent's words"}]}}),
        serde_json::json!({"type": "assistant", "timestamp": at(8), "message": {"content": [{"type": "text", "text": "Fixed the race; the regression test is left."}]}}),
    ]);
    let note = note(&c, &file).unwrap();
    assert!(note.starts_with("# Handoff from Claude Code session 5e5510f0"), "{note}");
    assert!(note.contains("branch main"), "{note}");
    assert!(note.contains("## Task\nfix the flaky test in parser"), "{note}");
    assert!(note.contains(" /model opus\n"), "{note}");
    assert!(!note.contains("not typed") && !note.contains("a subagent's words") && !note.contains("interrupted"), "{note}");
    assert!(note.contains("## Where it stopped\nFixed the race; the regression test is left."), "{note}");
    assert!(note.contains("- [x] find the race\n- [ ] add a regression test"), "{note}");
    assert!(note.contains("## Files changed, latest first\n- src/parser.rs"), "{note}");
    assert!(note.contains("## Files read most\n- src/parser.rs ×1"), "{note}");
    assert!(note.contains("- `cargo test parser`"), "{note}");
    assert!(line(&c, &file).starts_with("Continue from the earlier session 5e5510f0-0000-0000-0000-000000000000. Its log: "));
    std::fs::remove_dir_all(d).unwrap();
}

#[test]
fn the_ui_export_leaves_out_unreadable_subagent_logs() {
    let d = dir("export");
    let c = catalog(&d);
    let main = d.join("5e5510f0-0000-0000-0000-000000000000.jsonl");
    std::fs::write(&main, "").unwrap();
    let mut gone = c.resolve(&main).unwrap();
    gone.locator = tokprof_core::source::Locator::File(d.join("subagents").join("gone.jsonl"));
    let row = SessionRow {
        file: c.resolve(&main).unwrap(),
        subagents: vec![gone],
        project: "~/p".into(),
        id: "5e5510f0".into(),
        session_id: "5e5510f0-0000-0000-0000-000000000000".into(),
        bytes: 0,
    };
    let written = xray::export(&c, &row, &Pricing::defaults(), &d.join("x.html")).unwrap();
    assert!(written.ends_with(", 1 unreadable log left out"), "{written}");
    std::fs::remove_dir_all(d).unwrap();
}

#[test]
fn a_relative_target_is_made_absolute() {
    let c = catalog(Path::new("/nonexistent"));
    let file = xray::find("Cargo.toml", &c, &Query::default()).unwrap();
    assert!(file.path().is_absolute() && file.path().ends_with("Cargo.toml"), "{}", file.path().display());
}
