//! The source passes the checks every log source must pass, over its fixtures.

use std::path::{Path, PathBuf};

#[test]
fn the_fixtures_conform() {
    let fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    tokprof_core::testkit::conform(&tokprof_source_claude_code::ClaudeCode::at(fixtures.join("projects"), Path::new("/h"))).unwrap();
}

/// The fixture holds one hand-off of each kind, so the checks above run over all of them.
#[test]
fn the_fixtures_hand_off_once_of_each_kind() {
    use tokprof_core::model::HandoffKind::*;
    use tokprof_core::source::LogSource;
    let fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let source = tokprof_source_claude_code::ClaudeCode::at(fixtures.join("projects"), Path::new("/h"));
    let decode = |subagent: bool| {
        let log = source.discover().into_iter().find(|s| s.subagent == subagent).unwrap();
        let mut d = source.decoder(&log);
        source.open(&log).unwrap().for_each(&mut |r| d.feed(r)).unwrap();
        d.finish()
    };
    let main = decode(false);
    let at = |ts: &str| tokprof_logio::parse_ts_ms(Some(&serde_json::Value::String(format!("2026-10-30T{ts}Z")))).unwrap();
    let handoffs: Vec<_> = main.handoffs.iter().map(|h| (h.ts_ms, h.kind)).collect();
    assert_eq!(handoffs, [
        (at("09:01:10"), Question),
        (at("09:03:00"), Answer),
        (at("09:03:41"), TypedWhileWorking),
        // The two lines of one response end the turn once, at the later.
        (at("09:04:00"), TurnEnd),
        (at("10:05:00"), Wake),
        (at("10:06:00"), Interrupt),
        // A `<synthetic>` API error ends the turn too.
        (at("10:07:30"), TurnEnd),
    ]);
    assert!(!main.scripted, "an IDE session");
    assert!(decode(true).handoffs.is_empty(), "a subagent has no hand-offs");
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
