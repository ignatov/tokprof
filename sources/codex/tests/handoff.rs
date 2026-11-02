//! The handoff note and line of Codex rollouts, read
//! through the catalog.

use std::path::{Path, PathBuf};

use serde_json::Value;
use tokprof_core::catalog::Catalog;
use tokprof_core::handoff::note;
use tokprof_core::source::SessionRef;
use tokprof_source_codex::Codex;

fn catalog(dir: &Path) -> Catalog {
    Catalog::new(vec![Box::new(Codex::at(dir.to_path_buf(), Path::new("/h")))], PathBuf::from("/h"))
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
fn a_codex_note_reads_patches_commands_and_the_plan() {
    let d = dir("codex");
    let c = catalog(&d);
    let item = |payload: Value| serde_json::json!({"type": "response_item", "timestamp": "2026-08-12T11:44:00Z", "payload": payload});
    let file = log(&c, &d, "rollout-2026-08-12T13-44-47-019ff5ca-0db2-7180-8f61-e241e484f21a.jsonl", &[
        serde_json::json!({"type": "session_meta", "payload": {"id": "019ff5ca", "cwd": "/w/u", "git": {"branch": "master"}}}),
        item(serde_json::json!({"type": "message", "role": "user", "content": [{"type": "input_text", "text": "# AGENTS.md instructions"}]})),
        item(serde_json::json!({"type": "message", "role": "user", "content": [{"type": "input_text", "text": "sync the specs with the code"}]})),
        item(serde_json::json!({"type": "function_call", "name": "exec_command", "arguments": "{\"cmd\":\"git status --short\"}"})),
        item(serde_json::json!({"type": "custom_tool_call", "name": "exec", "input": "await tools.exec_command({cmd: \"rg -n Done spec\"}); await tools.apply_patch(\"*** Begin Patch\\n*** Update File: spec/a.md\\n\");"})),
        item(serde_json::json!({"type": "function_call", "name": "update_plan", "arguments": "{\"plan\":[{\"status\":\"in_progress\",\"step\":\"group the conflicts\"}]}"})),
        item(serde_json::json!({"type": "message", "role": "assistant", "content": [{"type": "output_text", "text": "Specs updated, code unchanged."}]})),
    ]);
    let note = note(&c, &file).unwrap();
    assert!(note.starts_with("# Handoff from Codex session 019ff5ca"), "{note}");
    assert!(note.contains("branch master at the start"), "{note}");
    assert!(note.contains("## Task\nsync the specs with the code"), "{note}");
    assert!(note.contains("## Where it stopped\nSpecs updated, code unchanged."), "{note}");
    assert!(note.contains("- [~] group the conflicts"), "{note}");
    assert!(note.contains("- spec/a.md"), "{note}");
    assert!(note.contains("- `rg -n Done spec`\n- `git status --short`"), "{note}");
    assert_eq!(file.session_id, "019ff5ca-0db2-7180-8f61-e241e484f21a");
    std::fs::remove_dir_all(d).unwrap();
}
