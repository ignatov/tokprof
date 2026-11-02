//! The source passes the checks every log source must pass, over its fixtures.

use std::path::{Path, PathBuf};

#[test]
fn the_fixtures_conform() {
    let fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    tokprof_core::testkit::conform(&tokprof_source_claude_code::ClaudeCode::at(fixtures.join("projects"), Path::new("/h"))).unwrap();
}

#[test]
fn a_broken_fixture_fails() {
    // A transcript whose second response is older than its first: its chain goes back in time.
    let dir = std::env::temp_dir().join(format!("tokprof-conformance-{}", std::process::id()));
    let project = dir.join("-w-app");
    std::fs::create_dir_all(&project).unwrap();
    let reply = |id: &str, ts: &str| {
        format!(
            r#"{{"type":"assistant","sessionId":"5e5510f0-aaaa-4bbb-8ccc-dddddddddddd","cwd":"/w/app","timestamp":"{ts}","message":{{"id":"{id}","model":"claude-opus-5-5","content":[],"usage":{{"input_tokens":10,"output_tokens":1}}}}}}"#
        )
    };
    let text = [reply("m1", "2026-10-30T10:00:00Z"), reply("m2", "2026-10-30T09:00:00Z")].join("\n") + "\n";
    std::fs::write(project.join("5e5510f0-aaaa-4bbb-8ccc-dddddddddddd.jsonl"), text).unwrap();
    let result = tokprof_core::testkit::conform(&tokprof_source_claude_code::ClaudeCode::at(dir.clone(), Path::new("/h")));
    std::fs::remove_dir_all(&dir).ok();
    let error = result.expect_err("the broken fixture fails").to_string();
    assert!(error.contains("goes back in time"), "{error}");
}
