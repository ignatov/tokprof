//! The source passes the checks every log source must pass, over its fixtures.

use std::path::{Path, PathBuf};

#[test]
fn the_fixtures_conform() {
    let fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    tokprof_core::testkit::conform(&tokprof_source_codex::Codex::at(fixtures, Path::new("/h"))).unwrap();
}

/// The main rollout holds an end of turn and an interrupt, so the checks above run over both.
#[test]
fn the_fixtures_hand_off_at_the_end_and_the_abort_of_a_turn() {
    use tokprof_core::model::HandoffKind::*;
    use tokprof_core::source::LogSource;
    let fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let source = tokprof_source_codex::Codex::at(fixtures, Path::new("/h"));
    let decode = |subagent: bool| {
        let log = source.discover().into_iter().find(|s| s.subagent == subagent).unwrap();
        let mut d = source.decoder(&log);
        source.open(&log).unwrap().for_each(&mut |r| d.feed(r)).unwrap();
        d.finish()
    };
    let main = decode(false);
    let at = |ts: &str| tokprof_logio::parse_ts_ms(Some(&serde_json::Value::String(format!("2026-10-31T{ts}Z")))).unwrap();
    let handoffs: Vec<_> = main.handoffs.iter().map(|h| (h.ts_ms, h.kind)).collect();
    assert_eq!(handoffs, [(at("08:31:02"), TurnEnd), (at("08:41:00"), Interrupt)]);
    assert!(!main.scripted, "codex-tui is someone typing");
    assert!(decode(true).handoffs.is_empty(), "a subagent thread has no hand-offs");
}

/// The main rollout holds a failed direct command, a script that printed a non-zero exit code
/// and a call the user aborted; its requests are timed by their items.
#[test]
fn the_fixtures_hold_call_outcomes_and_request_times() {
    use tokprof_core::model::Outcome;
    use tokprof_core::source::LogSource;
    let fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let source = tokprof_source_codex::Codex::at(fixtures, Path::new("/h"));
    let log = source.discover().into_iter().find(|s| !s.subagent).unwrap();
    let mut d = source.decoder(&log);
    source.open(&log).unwrap().for_each(&mut |r| d.feed(r)).unwrap();
    let main = d.finish();
    let mut errors: Vec<(Outcome, Option<String>)> = main.call_ends.values().map(|e| (e.outcome, e.error.clone())).collect();
    errors.sort_by_key(|e| format!("{e:?}"));
    errors.dedup();
    assert_eq!(
        errors,
        [
            (Outcome::Failed, Some("exit code 101: error: test failed, to rerun pass --lib".to_string())),
            (Outcome::Failed, Some("exit code 1: ModuleNotFoundError: No module named 'bs4'".to_string())),
            (Outcome::Refused, None),
        ]
    );
    let at = |ts: &str| tokprof_logio::parse_ts_ms(Some(&serde_json::Value::String(format!("2026-10-31T{ts}Z")))).unwrap();
    let first = &main.chains[0].requests[0];
    assert_eq!((first.sent_ms, first.ended_ms), (Some(at("08:00:02")), Some(at("08:00:05"))));
}
