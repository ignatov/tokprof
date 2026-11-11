//! The source passes the checks every log source must pass, over its fixtures.

use std::path::{Path, PathBuf};

/// The fixtures and the audited session below; each capability shows in one of them.
#[test]
fn the_fixtures_conform() {
    let fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let source = tokprof_source_claude_code::ClaudeCode::at(fixtures.join("projects"), Path::new("/h"));
    tokprof_core::testkit::conform_all(&[&source, &audited()]).unwrap();
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
    assert_eq!(
        handoffs,
        [
            (at("09:01:10"), Question),
            (at("09:03:00"), Answer),
            (at("09:03:41"), TypedWhileWorking),
            // The two lines of one response end the turn once, at the later.
            (at("09:04:00"), TurnEnd),
            (at("10:05:00"), Wake(tokprof_core::model::WakeSource::Command)),
            (at("10:06:00"), Interrupt),
            // A `<synthetic>` API error ends the turn too.
            (at("10:07:30"), TurnEnd),
            (at("10:08:00"), Wake(tokprof_core::model::WakeSource::Subagent)),
        ]
    );
    assert!(!main.scripted, "an IDE session");
    assert!(decode(true).handoffs.is_empty(), "a subagent has no hand-offs");
}

/// The fixture holds a failed, a refused and a denied call, and a request retried after an API
/// error, so the checks above run over them.
#[test]
fn the_fixtures_hold_call_outcomes_and_a_retry() {
    use tokprof_core::model::Outcome;
    use tokprof_core::source::LogSource;
    let fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let source = tokprof_source_claude_code::ClaudeCode::at(fixtures.join("projects"), Path::new("/h"));
    let log = source.discover().into_iter().find(|s| !s.subagent).unwrap();
    let mut d = source.decoder(&log);
    source.open(&log).unwrap().for_each(&mut |r| d.feed(r)).unwrap();
    let main = d.finish();
    let mut outcomes: Vec<Outcome> = main.call_ends.values().map(|e| e.outcome).collect();
    outcomes.sort_by_key(|o| format!("{o:?}"));
    outcomes.dedup();
    assert_eq!(outcomes, [Outcome::Denied, Outcome::Failed, Outcome::Refused]);
    let failed = main.call_ends.values().find(|e| e.outcome == Outcome::Failed).unwrap();
    assert_eq!(failed.error.as_deref(), Some("exit code 101: error: test failed, to rerun pass --lib"));
    let at = |ts: &str| tokprof_logio::parse_ts_ms(Some(&serde_json::Value::String(format!("2026-10-30T{ts}Z")))).unwrap();
    let retried = main.chains.iter().flat_map(|c| &c.requests).find(|r| r.retries > 0).unwrap();
    assert_eq!((retried.first_sent_ms, retried.sent_ms, retried.retries), (Some(at("10:05:00")), Some(at("10:05:05")), 1));
    assert_eq!(retried.retry_error.as_deref(), Some("502 local proxy: upstream request failed"));
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

/// A session with request-audit lines, kept apart from `tests/fixtures`, which other tests read.
fn audited() -> tokprof_source_claude_code::ClaudeCode {
    tokprof_source_claude_code::ClaudeCode::at(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/conform/projects"), Path::new("/h"))
}

/// The audited session holds each path of the audit lines, so the checks above run over them: a
/// retried request, an advisor call, a message no line holds, a compaction, a second shape, a
/// request nobody answered, the origins of a skill, a command and a subagent's notification, a
/// custom title over the generated one, and a subagent with its meta file and its own retry.
#[test]
fn the_audited_fixtures_hold_each_path() {
    use tokprof_core::model::{CompactionTrigger, Origin};
    use tokprof_core::source::LogSource;
    let source = audited();
    let decode = |subagent: bool| {
        let log = source.discover().into_iter().find(|s| s.subagent == subagent).unwrap();
        let mut d = source.decoder(&log);
        source.open(&log).unwrap().for_each(&mut |r| d.feed(r)).unwrap();
        d.finish()
    };
    let at = |ts: &str| tokprof_logio::parse_ts_ms(Some(&serde_json::Value::String(format!("2026-11-02T{ts}Z")))).unwrap();
    let main = decode(false);
    let requests = &main.chains[0].requests;
    assert_eq!((main.chains.len(), requests.len(), main.shapes.len()), (1, 9, 2));
    assert!(requests.iter().all(|r| r.prompt.is_some()), "every request has its prompt");
    assert_eq!(main.messages.iter().filter(|m| !m.resolved).count(), 1, "one message is in no line");
    let (failed, retried) = (&requests[0], &requests[1]);
    assert_eq!((failed.first_sent_ms, failed.usage.output), (None, 0), "the failed attempt keeps no time of its own");
    assert_eq!((retried.first_sent_ms, retried.sent_ms, retried.retries), (Some(at("09:00:01")), Some(at("09:00:05")), 1));
    assert_eq!(retried.retry_error.as_deref(), Some("529 overloaded"));
    let advisor: Vec<&str> = requests.iter().filter(|r| r.advisor).map(|r| r.model.as_str()).collect();
    assert_eq!(advisor, ["claude-fable-5-2"]);
    assert_eq!(main.compactions.iter().map(|c| c.kind.clone()).collect::<Vec<_>>(), [CompactionTrigger::Manual]);
    let compacted: Vec<Option<&Origin>> = requests.iter().filter(|r| r.compacted_before).map(|r| r.origin.as_ref()).collect();
    assert_eq!(compacted, [Some(&Origin::Command("goal".into()))]);
    let skill = Origin::Skill("code-review".into());
    assert_eq!(
        requests.iter().filter(|r| r.origin.as_ref() == Some(&skill)).count(),
        4,
        "the skill's response, its advisor call and the subagent's notification"
    );
    assert_eq!(requests.last().map(|r| (r.ended_ms, r.usage.prompt())), Some((None, 0)), "nobody answered the last request");
    assert_eq!(main.title.as_deref(), Some("Rename the loader"));
    assert!((main.reported_usd.unwrap() - 0.67).abs() < 1e-9, "the highest count of each run of the counter");
    assert_eq!(main.vcs.len(), 2, "the commit and push call, in its line and in its merged response");

    let sub = decode(true);
    assert_eq!((sub.task.as_deref(), sub.chains[0].name.as_str()), (Some("Map the loader"), "subagent: Map the loader"));
    let answered = sub.chains[0].requests.last().unwrap();
    assert_eq!((answered.first_sent_ms, answered.retries, answered.retry_error.as_deref()), (Some(at("09:00:32")), 1, Some("500 internal error")));
    assert_eq!(answered.origin, Some(Origin::Subagent("general-purpose".into())));
}
