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

/// Rollouts of the usage streams and items `tests/fixtures` lacks, kept apart from it, since other
/// tests read it.
fn streams() -> tokprof_source_codex::Codex {
    tokprof_source_codex::Codex::at(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/conform"), Path::new("/h"))
}

#[test]
fn the_stream_fixtures_conform() {
    tokprof_core::testkit::conform(&streams()).unwrap();
}

/// A rollout whose usage comes only from `token_count` events, with reasoning, a repeated total
/// and a compaction event; one with both usage streams, a repeated and an empty usage record, a
/// compaction call, tools and items of every kind; and the subagent it spawned, which spawned
/// another, so the checks above run over all of them.
#[test]
fn the_stream_fixtures_hold_each_path() {
    use tokprof_core::model::{Origin, Outcome, Session};
    use tokprof_core::source::LogSource;
    let source = streams();
    let logs = source.discover();
    let decode = |id: char| -> Session {
        let log = logs.iter().find(|l| l.session_id.ends_with(id)).unwrap();
        let mut d = source.decoder(log);
        source.open(log).unwrap().for_each(&mut |r| d.feed(r)).unwrap();
        d.finish()
    };
    let counts = decode('a');
    let r = &counts.chains[0].requests;
    let got: Vec<_> = r.iter().map(|r| (r.usage.uncached, r.usage.reasoning, r.context_window, r.compacted_before)).collect();
    assert_eq!(
        got,
        [(5000, 20, Some(272_000), false), (200, 0, Some(272_000), false), (900, 0, Some(272_000), true)],
        "a repeated total and no info count nothing"
    );
    assert_eq!((counts.title.as_deref(), counts.compactions.len()), (Some("List the crates"), 1));

    let both = decode('b');
    let r = &both.chains[0].requests;
    assert_eq!(r.len(), 6, "the usage records, a repeated one once and an empty one not");
    assert_eq!((r[0].usage.uncached, r[0].usage.write_short), (1000, 6000));
    let marks: Vec<(bool, bool)> = r.iter().map(|r| (r.compaction_call, r.compacted_before)).collect();
    assert_eq!(marks[3..5], [(true, false), (false, true)]);
    assert!(both.scripted && both.title.as_deref() == Some("Rename the loader"));
    assert!(both.vcs.values().all(|v| v.commit && v.pull_request) && !both.vcs.is_empty());
    assert!(both.call_ends.values().all(|e| e.outcome == Outcome::Failed) && !both.call_ends.is_empty());

    let (child, grandchild) = (decode('c'), decode('d'));
    assert_eq!((child.id.as_str(), child.task.as_deref(), child.chains[0].name.as_str()), (both.id.as_str(), Some("map callers"), "subagent: map callers"));
    assert_eq!(child.chains[0].requests[0].origin, Some(Origin::Subagent("explorer".into())));
    assert_eq!((grandchild.id.as_str(), grandchild.chains[0].name.as_str()), (both.id.as_str(), "subagent:review"));
    let main = logs.iter().find(|l| l.session_id.ends_with('b')).unwrap();
    assert_eq!(source.subagents(main).len(), 2, "the thread it spawned, and the one that spawned");
}
