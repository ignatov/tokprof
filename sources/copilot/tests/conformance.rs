//! The source passes the checks every log source must pass, over its fixtures, and reads them as
//! the session-logs spec says.

use std::path::PathBuf;

use tokprof_core::model::{ChainKind, Outcome, Session};
use tokprof_core::source::LogSource;
use tokprof_source_copilot::Copilot;

fn source() -> Copilot {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("fixtures").join("session-state");
    Copilot::at(root, PathBuf::from("/Users/me"))
}

fn sessions() -> Vec<Session> {
    let source = source();
    let mut out: Vec<Session> = source
        .discover()
        .iter()
        .map(|log| {
            let mut d = source.decoder(log);
            source.open(log).unwrap().for_each(&mut |r| d.feed(r)).unwrap();
            d.finish()
        })
        .collect();
    out.sort_by(|a, b| a.id.cmp(&b.id));
    out
}

#[test]
fn the_fixtures_conform() {
    tokprof_core::testkit::conform(&source()).unwrap();
}

/// A session of two models with a subagent, a compaction, every outcome of a call and a command
/// in the background.
#[test]
fn a_session_with_usage() {
    let s = &sessions()[0];
    assert_eq!((s.id.as_str(), s.project.as_str(), s.title.as_deref()), ("c0de0001-0000-4000-8000-000000000001", "~/src/app", Some("Fix the parser")));
    let chains: Vec<(&str, ChainKind, usize)> = s.chains.iter().map(|c| (c.name.as_str(), c.kind, c.requests.len())).collect();
    assert_eq!(chains, [("main", ChainKind::Main, 5), ("subagent: review-parser", ChainKind::Subagent, 2)]);

    let main = &s.chains[0].requests;
    let models: Vec<&str> = main.iter().map(|r| r.model.as_str()).collect();
    assert_eq!(models, ["claude-sonnet-5-5", "claude-fable-5-2", "claude-fable-5-2", "claude-fable-5-2", "claude-fable-5-2"]);
    assert_eq!(main.iter().map(|r| r.compaction_call).collect::<Vec<_>>(), [false, false, true, false, false]);
    assert_eq!(main.iter().map(|r| r.compacted_before).collect::<Vec<_>>(), [false, false, false, true, false]);
    assert!(main.iter().all(|r| !r.no_usage && r.version.as_deref() == Some("1.0.116")));
    assert_eq!((main[4].usage.write_short, main[4].usage.write_long), (0, 1000), "a cache of an hour");

    // The credits of the six records and the compaction call, as the shutdown line sums them;
    // that line is not read.
    assert!((s.reported_usd.unwrap() - 1.370109).abs() < 1e-9);

    let ends: Vec<Outcome> = s.call_ends.values().map(|e| e.outcome).collect();
    let count = |o: Outcome| ends.iter().filter(|&&e| e == o).count();
    // Each outcome is on the call's block and on its result's.
    assert_eq!((count(Outcome::Failed), count(Outcome::Refused), count(Outcome::Denied)), (4, 2, 2), "a failed test run and a refused delay");
    assert_eq!(s.calls.iter().filter(|c| c.background).count(), 1);
    assert_eq!(s.calls.iter().filter(|c| c.asks_user).count(), 1);
    assert_eq!(s.platform_calls.len(), 1, "the read the CLI made itself");
    assert_eq!(s.typed.len(), 2);
    assert_eq!(s.compactions.len(), 1);
}

/// A log of a CLI that wrote no usage records.
#[test]
fn a_session_without_usage() {
    let s = &sessions()[1];
    assert_eq!((s.project.as_str(), s.title.as_deref(), s.reported_usd), ("~/notes", None, None));
    let r = &s.chains[0].requests;
    assert_eq!(r.len(), 2);
    assert!(r.iter().all(|r| r.no_usage && r.answered() && r.model == "claude-fable-5-1" && r.version.as_deref() == Some("1.0.85")));
    assert_eq!(s.calls.len(), 1);
}
