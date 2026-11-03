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
        active_ms: 0,
        client: None,
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

#[test]
fn the_latest_session_is_the_one_active_last() {
    let d = dir("latest");
    let c = catalog(&d);
    let (idle, busy) = ("5e5510f0-aaaa-4000-8000-000000000000", "5e5510f0-bbbb-4000-8000-000000000000");
    let prompt = |id: &str, ts: &str| serde_json::json!({"type": "user", "sessionId": id, "cwd": "/w/app", "timestamp": ts, "message": {"role": "user", "content": "go"}});
    log(&c, &d, &format!("{busy}.jsonl"), &[prompt(busy, "2026-11-02T15:00:00Z")]);
    // Idle since 08:27, then written last when the IDE resumed it: lines without a time.
    let resumed = [
        prompt(idle, "2026-11-02T08:27:39Z"),
        serde_json::json!({"type": "cost-state", "sessionId": idle, "totalCostUSD": 1.98, "totalDuration": 24366969, "startTime": 1793607245463u64}),
        serde_json::json!({"type": "last-prompt", "sessionId": idle, "lastPrompt": "go"}),
    ];
    log(&c, &d, &format!("{idle}.jsonl"), &resumed);
    let ms = |ts: &str| tokprof_logio::parse_ts_ms(Some(&Value::String(ts.to_string()))).unwrap();
    let at = |name: &str, ts: &str| {
        let f = std::fs::File::options().write(true).open(d.join(name)).unwrap();
        f.set_modified(std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_millis(ms(ts) as u64)).unwrap();
    };
    at(&format!("{busy}.jsonl"), "2026-11-02T15:00:05Z");
    at(&format!("{idle}.jsonl"), "2026-11-02T15:01:50Z");

    let select = Query { agents: c.agents(), ..Query::default() };
    assert_eq!(xray::latest(&c, &select).unwrap().session_id, busy, "the log written last only got lines without a time");

    let error = xray::find("5e5510f0", &c, &select).unwrap_err().to_string();
    let lines: Vec<&str> = error.lines().collect();
    assert_eq!(lines[0], "\"5e5510f0\" matches 2 sessions; give more of the id:");
    let local = |ts: &str| tokprof_core::fmt::local_time(ms(ts), "%Y-%m-%d %H:%M");
    assert!(lines[1].starts_with("  5e5510f0  ") && lines[1].contains(&local("2026-11-02T15:00:00Z")), "the busy session first, at its last activity: {}", lines[1]);
    assert!(lines[2].contains(&local("2026-11-02T08:27:39Z")), "then the idle one, at its last activity: {}", lines[2]);
    std::fs::remove_dir_all(d).unwrap();
}

#[test]
fn a_client_filter_picks_the_latest_session_of_that_client() {
    let d = dir("client");
    let c = catalog(&d);
    let user = |ts: &str, id: &str, entrypoint: &str| serde_json::json!({"type": "user", "timestamp": ts, "sessionId": id, "cwd": "/w/p", "entrypoint": entrypoint, "message": {"content": "go"}});
    let ide = "1f000000-0000-0000-0000-000000000000";
    let terminal = "2f000000-0000-0000-0000-000000000000";
    log(&c, &d, &format!("{ide}.jsonl"), &[user("2026-11-02T08:00:00Z", ide, "sdk-ts")]);
    log(&c, &d, &format!("{terminal}.jsonl"), &[user("2026-11-02T09:00:00Z", terminal, "cli")]);
    let pick = |client: Option<&str>| {
        let q = Query { agents: c.agents(), client: client.map(str::to_string), ..Query::default() };
        xray::latest(&c, &q).map(|f| f.session_id)
    };
    assert_eq!(pick(None).as_deref(), Some(terminal), "the latest session of all");
    assert_eq!(pick(Some("sdk-ts")).as_deref(), Some(ide), "the latest session of the client");
    assert_eq!(pick(Some("desktop")), None);
    let listed: Vec<String> = xray::rows(&c, &Query { agents: c.agents(), client: Some("sdk-ts".into()), ..Query::default() }, 20).into_iter().map(|r| r.session_id).collect();
    assert_eq!(listed, [ide]);
    std::fs::remove_dir_all(d).unwrap();
}
