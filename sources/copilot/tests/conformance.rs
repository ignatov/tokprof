//! The source passes the checks every log source must pass, over its fixtures, and reads them as
//! the session-logs spec says.

use std::path::{Path, PathBuf};

use tokprof_core::model::{CallRecord, ChainKind, Outcome, Session, ToolClass};
use tokprof_core::source::{Claim, LogSource};
use tokprof_source_copilot::Copilot;

fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("fixtures")
}

fn source() -> Copilot {
    Copilot::at(fixtures())
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

/// The first session of a task: setup steps, a custom agent, a failed and a passed test run, and
/// a commit; its record carries the task's credits.
#[test]
fn a_first_session_with_its_record() {
    let s = &sessions()[0];
    assert_eq!(s.project, "github.com/octo/app");
    assert_eq!(s.title.as_deref(), Some("Read empty lists in the parser"));
    assert_eq!(s.reported_usd, Some(1.5), "150 credits at $0.01");
    assert_eq!(s.mcp_servers, ["docs", "web"]);
    let chains: Vec<_> = s.chains.iter().map(|c| (c.name.as_str(), c.kind, c.requests.len())).collect();
    assert_eq!(chains, [("main", ChainKind::Main, 7)], "a custom agent is the main conversation");
    let main = &s.chains[0].requests;
    assert!(main.iter().all(|r| r.no_usage && r.model == "claude-opus-5-5" && r.answered()));
    assert_eq!(s.platform_calls.len(), 5, "two custom steps, the clone and two MCP servers; no marker");
    let steps: Vec<_> = s.platform_calls.iter().map(|&b| s.call(b).and_then(CallRecord::duration_ms)).collect();
    assert_eq!(steps, [Some(2_000), Some(60_000), Some(1_000), Some(250), Some(1_000)]);
    assert_eq!(s.typed.len(), 1, "the platform's later messages are no typed messages");
    assert_eq!(s.typed[0].ts_ms, 1_793_000_000_000, "at the log's first chunk");
    let failed: Vec<_> = s.call_ends.values().filter(|e| e.outcome == Outcome::Failed).map(|e| e.error.clone()).collect();
    assert!(failed.contains(&Some("exit code 1: BUILD FAILED".to_string())), "{failed:?}");
    assert_eq!(s.vcs.len(), 1, "the commit and push of report_progress");
    let docs = s.blocks.iter().find(|b| b.tool.as_deref() == Some("docs-search")).unwrap();
    assert_eq!(docs.target.as_deref(), Some("mcp:docs"));
    let tests = s.blocks.iter().find(|b| b.tool.as_deref() == Some("bash")).unwrap();
    assert_eq!(tests.target.as_deref(), Some("gradlew test"));
    assert_eq!(s.logged_tokens.values().sum::<u64>(), 160 + 3 + 2 + 9 + 4 + 5);
}

/// A later session of the task: its record has no usage, so it has no cost.
#[test]
fn a_later_session_without_usage_has_no_cost() {
    let s = &sessions()[1];
    assert_eq!((s.reported_usd, s.title.is_some()), (None, true));
    assert_eq!(s.chains[0].requests.len(), 2);
}

/// A log without a record: a subagent, a command in the background, a blocked one.
#[test]
fn a_log_without_a_record() {
    let s = &sessions()[2];
    assert_eq!((s.title.as_deref(), s.reported_usd), (None, None));
    assert!(s.chains.iter().flat_map(|c| &c.requests).all(|r| r.model == "unknown"));
    let chains: Vec<_> = s.chains.iter().map(|c| (c.name.as_str(), c.requests.len())).collect();
    assert_eq!(chains, [("main", 4), ("subagent: check-tests", 2)]);
    let task = s.blocks.iter().position(|b| b.tool.as_deref() == Some("task")).unwrap();
    assert_eq!(s.call(task).and_then(CallRecord::duration_ms), Some(29_000));
    let dev = s.blocks.iter().position(|b| b.tool.as_deref() == Some("bash")).unwrap();
    assert!(s.call(dev).unwrap().background);
    assert!(s.call_ends.values().any(|e| e.outcome == Outcome::Denied));
    let fetch = s.blocks.iter().position(|b| b.tool.as_deref() == Some("web_fetch")).unwrap();
    assert_eq!(s.blocks[fetch].class, tokprof_core::model::Class::Tool(ToolClass::Fetch));
    assert_eq!(s.call(fetch).and_then(CallRecord::duration_ms), Some(2_000), "its result from the repeat");
}

/// A log outside the folder is read when its first data line is a chat chunk.
#[test]
fn a_log_given_as_the_target() {
    let elsewhere = Copilot::at(fixtures().join("2026-10-27"));
    let loose = fixtures().join("loose").join("1b2c3d4e-0000-4000-8000-000000000003.sse");
    assert_eq!(elsewhere.claim(&loose), Claim::Yes);
    assert_eq!(elsewhere.claim(Path::new("no-such.sse")), Claim::No);
    let r = elsewhere.session_at(&loose).unwrap();
    assert_eq!(r.session_id, "1b2c3d4e-0000-4000-8000-000000000003");
}
