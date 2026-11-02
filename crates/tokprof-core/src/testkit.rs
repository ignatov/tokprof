//! The checks every log source must pass, over its own fixtures. A source crate runs them from
//! `tests/conformance.rs`:
//!
//! ```ignore
//! tokprof_core::testkit::conform(&MySource::at(fixtures, home)).unwrap();
//! ```
//!
//! They check the contract the rest of tokprof relies on: what the first records tell agrees with
//! the whole log, a decoder fed in two parts gives what it gives fed at once, subagents' logs point
//! at their main session, and the declared capabilities hold.

use anyhow::{Result, bail, ensure};

use crate::model::{CompactionTrigger, short_id};
use crate::source::{LogSource, OwnCost, SessionRef};

/// Runs every check over every log the source finds; fails with the first that does not hold.
pub fn conform(source: &dyn LogSource) -> Result<()> {
    let logs = source.discover();
    ensure!(!logs.is_empty(), "{}: the fixtures hold no logs", source.name());
    for s in &logs {
        check(source, s).map_err(|e| anyhow::anyhow!("{}: {e}", s.path().display()))?;
    }
    for main in logs.iter().filter(|s| !s.subagent) {
        for sub in source.subagents(main) {
            ensure!(sub.subagent, "{} is listed as a subagent of {} but is not one", sub.session_id, main.session_id);
            ensure!(sub.main_id == main.session_id, "subagent {} names main session {}, not {}", sub.session_id, sub.main_id, main.session_id);
        }
    }
    Ok(())
}

fn check(source: &dyn LogSource, s: &SessionRef) -> Result<()> {
    let records = records(source, s)?;
    let whole = {
        let mut d = source.decoder(s);
        records.iter().for_each(|r| d.feed(r));
        d.finish()
    };
    // Fed in two parts, with a snapshot between them, the decoder gives the same session.
    let parts = {
        let mut d = source.decoder(s);
        let half = records.len() / 2;
        records[..half].iter().for_each(|r| d.feed(r));
        let _ = d.snapshot();
        records[half..].iter().for_each(|r| d.feed(r));
        d.finish()
    };
    ensure!(format!("{whole:?}") == format!("{parts:?}"), "feeding the records in two parts gives another session");

    ensure!(whole.agent == s.agent, "the session's agent is {}, its log's {}", whole.agent, s.agent);
    let head = source.head(s);
    ensure!(!head.session_id.is_empty(), "the first records name no session id");
    if !s.subagent {
        ensure!(head.session_id == whole.id, "the first records give id {}, the log {}", head.session_id, whole.id);
        ensure!(head.project == whole.project, "the first records give project {:?}, the log {:?}", head.project, whole.project);
    }
    let short = short_id(&s.agent, &head.session_id);
    ensure!(head.session_id.starts_with(short), "short id {short} is no prefix of {}", head.session_id);

    for chain in &whole.chains {
        if let Some(w) = chain.requests.windows(2).find(|w| w[1].ts_ms < w[0].ts_ms) {
            bail!("chain {} goes back in time: {} after {}", chain.name, w[1].ts_ms, w[0].ts_ms);
        }
    }
    let caps = &s.agent.info().capabilities;
    if !caps.compaction_trigger {
        ensure!(whole.compactions.iter().all(|c| c.kind == CompactionTrigger::Unknown), "compaction triggers without the capability");
    }
    if caps.own_cost == OwnCost::None {
        ensure!(whole.reported_usd.is_none(), "an own cost without the capability");
    }
    Ok(())
}

fn records(source: &dyn LogSource, s: &SessionRef) -> Result<Vec<String>> {
    let mut out = Vec::new();
    source.open(s)?.for_each(&mut |r| out.push(r.to_string()))?;
    Ok(out)
}
