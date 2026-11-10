//! The source passes the checks every log source must pass, over its fixtures, and reads them as
//! the session-logs spec says.

use std::path::{Path, PathBuf};

use tokprof_core::Pricing;
use tokprof_core::model::Session;
use tokprof_core::source::LogSource;
use tokprof_source_pi::Pi;

fn source() -> Pi {
    Pi::at(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/sessions"), Path::new("/h"))
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

/// The main log has two branches, an aborted reply and a compaction; the fork copied the path up
/// to its second reply and added one; the local model's error reply has no usage.
#[test]
fn the_fixtures_count_each_call_once() {
    let s = sessions();
    let counts: Vec<(&str, &str, usize)> = s.iter().map(|s| (&s.id[s.id.len() - 1..], s.project.as_str(), s.chains[0].requests.len())).collect();
    assert_eq!(counts, [("a", "~/src/app", 5), ("b", "~/src/app", 1), ("c", "~/src/qwen", 1)]);
    let main = &s[0].chains[0].requests;
    let marks: Vec<(bool, bool)> = main.iter().map(|r| (r.compaction_call, r.compacted_before)).collect();
    assert_eq!(marks, [(false, false), (false, false), (false, false), (true, false), (false, true)]);
    assert_eq!(s[2].chains[0].requests[0].usage.reasoning, 40);
}

/// pi's own cost of the Anthropic logs is within 1.5% of tokprof's; the local model has no price
/// and pi's $0 stays a reported cost only.
#[test]
fn tokprof_s_dollars_agree_with_pi_s_own() {
    let pricing = Pricing::defaults();
    let s = sessions();
    let ours =
        |s: &Session| -> Option<f64> { s.chains[0].requests.iter().map(|r| pricing.get(&r.model, r.usage.prompt(), r.ts_ms).map(|p| p.cost(&r.usage))).sum() };
    for s in &s[..2] {
        let (ours, theirs) = (ours(s).unwrap(), s.reported_usd.unwrap());
        assert!((ours - theirs).abs() <= 0.015 * theirs, "{}: tokprof ${ours:.6}, pi ${theirs:.6}", s.id);
    }
    assert_eq!((ours(&s[2]), s[2].reported_usd), (None, Some(0.0)));
}
