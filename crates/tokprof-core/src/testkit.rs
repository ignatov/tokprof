//! The checks every log source must pass, over its own fixtures. A source crate runs them from
//! `tests/conformance.rs`:
//!
//! ```ignore
//! tokprof_core::testkit::conform(&MySource::at(fixtures, home)).unwrap();
//! ```
//!
//! They check the contract the rest of tokprof relies on: what the first records tell agrees with
//! the whole log; a decoder cut at any record, with snapshots there, gives what it gives fed at
//! once, as the browser feeds a live log; a record the decoder cannot read changes nothing; every
//! index a session holds points into it; a log is found again at its path; subagents' logs point
//! at their main session; and the capabilities an agent declares hold both ways: no session shows
//! a fact its agent does not declare, and each declared fact shows in at least one session.

use std::collections::BTreeMap;
use std::ops::Range;
use std::panic::{AssertUnwindSafe, catch_unwind};

use anyhow::{Result, anyhow, bail, ensure};

use crate::model::{BlockKind, CompactionTrigger, Session, short_id};
use crate::source::{AgentId, Capabilities, Claim, Locator, LogSource, OwnCost, SessionRef};

/// Runs every check over every log the source finds; fails with the first that does not hold.
pub fn conform(source: &dyn LogSource) -> Result<()> {
    conform_all(&[source])
}

/// Runs every check over every log the sources find, such as one source over several fixture
/// folders. Each fact an agent's capabilities declare shows in at least one of their sessions.
pub fn conform_all(sources: &[&dyn LogSource]) -> Result<()> {
    let mut shown: BTreeMap<AgentId, Shown> = BTreeMap::new();
    for &source in sources {
        let logs = source.discover();
        ensure!(!logs.is_empty(), "{}: the fixtures hold no logs", source.name());
        for s in &logs {
            let whole = check(source, s).map_err(|e| anyhow!("{}: {}: {e}", source.name(), s.path().display()))?;
            shown.entry(s.agent.clone()).or_default().add(&Shown::of(&whole));
        }
        for main in logs.iter().filter(|s| !s.subagent) {
            for sub in source.subagents(main) {
                ensure!(sub.subagent, "{} is listed as a subagent of {} but is not one", sub.session_id, main.session_id);
                ensure!(sub.main_id == main.session_id, "subagent {} names main session {}, not {}", sub.session_id, sub.main_id, main.session_id);
            }
        }
    }
    for (agent, shown) in &shown {
        for (fact, declared, seen) in shown.against(&agent.info().capabilities) {
            ensure!(!declared || seen, "{agent} declares {fact}, but no fixture session shows it");
        }
    }
    Ok(())
}

/// The checks of one log; gives the session the whole log gives.
fn check(source: &dyn LogSource, s: &SessionRef) -> Result<Session> {
    let records = records(source, s)?;
    let n = records.len();
    let decode = |records: &[String]| {
        let mut d = source.decoder(s);
        records.iter().for_each(|r| d.feed(r));
        d.finish()
    };
    let whole = decode(&records);
    indices(&whole).map_err(|e| anyhow!("the whole log: {e}"))?;

    // A reader that follows a live log keeps its decoder, feeds it what was appended and takes a
    // snapshot each time. Cut at every record, the decoder gives at the cut what a decoder fed
    // that much gives, a snapshot leaves it as it was, and the rest gives the whole log's session.
    for k in 0..=n {
        let mut d = source.decoder(s);
        records[..k].iter().for_each(|r| d.feed(r));
        let snapshot = d.snapshot();
        indices(&snapshot).map_err(|e| anyhow!("the snapshot after {k} of {n} records: {e}"))?;
        same(&decode(&records[..k]), &snapshot)
            .map_err(|hint| anyhow!("after {k} of {n} records, the snapshot differs from a decoder fed them and finished: {hint}"))?;
        same(&snapshot, &d.snapshot()).map_err(|hint| anyhow!("after {k} of {n} records, a second snapshot differs from the first: {hint}"))?;
        records[k..].iter().for_each(|r| d.feed(r));
        same(&whole, &d.finish())
            .map_err(|hint| anyhow!("fed {k} of {n} records, a snapshot and the rest, the decoder gives another session than fed them at once: {hint}"))?;
    }

    // A record the decoder cannot read is skipped, wherever it comes.
    for junk in Junk::ALL {
        let mut fed = Vec::with_capacity(2 * n + 1);
        for i in 0..=n {
            fed.push(junk.record(&records, i));
            fed.extend(records.get(i).cloned());
        }
        let got = catch_unwind(AssertUnwindSafe(|| decode(&fed))).map_err(|_| anyhow!("the decoder panics on {}", junk.name()))?;
        same(&whole, &got).map_err(|hint| anyhow!("{} between the records changes the session: {hint}", junk.name()))?;
    }

    ensure!(whole.agent == s.agent, "the session's agent is {}, its log's {}", whole.agent, s.agent);
    let head = source.head(s);
    ensure!(!head.session_id.is_empty(), "the first records name no session id");
    if !s.subagent {
        ensure!(head.session_id == whole.id, "the first records give id {}, the log {}", head.session_id, whole.id);
        ensure!(head.project == whole.project, "the first records give project {:?}, the log {:?}", head.project, whole.project);
    }
    let short = short_id(&s.agent, &head.session_id);
    ensure!(head.session_id.starts_with(short), "short id {short} is no prefix of {}", head.session_id);
    // A record is written after what it records, so the log's last write is never earlier.
    if let Some(t) = source.last_active(s) {
        ensure!(t <= s.modified_ms(), "the last activity {t} is later than the log's last write {}", s.modified_ms());
    }

    // A log in a file is found again at its path, and given as a target, it is read.
    match &s.locator {
        Locator::File(path) => {
            let again = source.session_at(path);
            ensure!(again.as_ref() == Some(s), "the log at its own path is {again:?}");
            ensure!(source.claim(path) != Claim::No, "the source does not read its own log given as a target");
        }
        Locator::Row { db, .. } => ensure!(source.claim(db) != Claim::No, "the source does not read the store of its log given as a target"),
        Locator::Dir(_) => {}
    }

    for chain in &whole.chains {
        if let Some(w) = chain.requests.windows(2).find(|w| w[1].ts_ms < w[0].ts_ms) {
            bail!("chain {} goes back in time: {} after {}", chain.name, w[1].ts_ms, w[0].ts_ms);
        }
    }
    for (fact, declared, seen) in Shown::of(&whole).against(&s.agent.info().capabilities) {
        ensure!(declared || !seen, "the session shows {fact} without the capability");
    }
    Ok(whole)
}

fn records(source: &dyn LogSource, s: &SessionRef) -> Result<Vec<String>> {
    let mut out = Vec::new();
    source.open(s)?.for_each(&mut |r| out.push(r.to_string()))?;
    Ok(out)
}

/// Whether two sessions are the same, by their `Debug` text; else where they first differ.
fn same(expected: &Session, got: &Session) -> std::result::Result<(), String> {
    if format!("{expected:?}") == format!("{got:?}") {
        return Ok(());
    }
    let (a, b) = (format!("{expected:#?}"), format!("{got:#?}"));
    let (a, b): (Vec<&str>, Vec<&str>) = (a.lines().collect(), b.lines().collect());
    let at = a.iter().zip(&b).position(|(x, y)| x != y).unwrap_or(a.len().min(b.len()));
    // The fields that hold the first difference, from the indentation of the lines before it.
    let mut path: Vec<(usize, &str)> = Vec::new();
    for line in &a[..at] {
        let indent = line.len() - line.trim_start().len();
        path.retain(|(i, _)| *i < indent);
        if line.ends_with(['{', '[', '(']) {
            path.push((indent, line.trim().trim_end_matches(['{', '[', '(']).trim().trim_end_matches(':')));
        }
    }
    let path: Vec<&str> = path.iter().map(|(_, name)| *name).filter(|n| !n.is_empty()).collect();
    let line = |lines: &[&str]| lines.get(at).map_or_else(|| "(nothing)".to_string(), |l| l.trim().to_string());
    Err(format!("in {}: expected `{}`, got `{}`", path.join(" > "), line(&a), line(&b)))
}

/// Every index a session holds into its own tables is in range: the analysis indexes them
/// without checks. A tool call's outcome and actions sit on its call and result blocks.
fn indices(s: &Session) -> Result<()> {
    let (blocks, messages, shapes) = (s.blocks.len(), s.messages.len(), s.shapes.len());
    let in_blocks = |r: &Range<usize>| r.start <= r.end && r.end <= blocks;
    for (i, m) in s.messages.iter().enumerate() {
        ensure!(in_blocks(&m.blocks), "message {i} holds blocks {:?} of {blocks}", m.blocks);
    }
    for (i, shape) in s.shapes.iter().enumerate() {
        ensure!(in_blocks(&shape.blocks), "shape {i} holds blocks {:?} of {blocks}", shape.blocks);
    }
    for chain in &s.chains {
        for (i, r) in chain.requests.iter().enumerate() {
            if let Some(p) = &r.prompt {
                if let Some(shape) = p.shape {
                    ensure!(shape < shapes, "request {i} of chain {} has shape {shape} of {shapes}", chain.name);
                }
                if let Some(m) = p.tail.iter().find(|&&m| m >= messages) {
                    bail!("request {i} of chain {} adds message {m} of {messages}", chain.name);
                }
            }
            if let Some(m) = r.produced {
                ensure!(m < messages, "request {i} of chain {} produced message {m} of {messages}", chain.name);
            }
        }
    }
    let kind = |b: usize| s.blocks.get(b).map(|b| b.kind);
    for &b in s.call_ends.keys() {
        ensure!(matches!(kind(b), Some(BlockKind::ToolUse | BlockKind::ToolResult)), "a call's outcome is on block {b}, a {:?}, of {blocks}", kind(b));
    }
    for &b in s.vcs.keys() {
        ensure!(kind(b) == Some(BlockKind::ToolUse), "a call's version-control actions are on block {b}, a {:?}, of {blocks}", kind(b));
    }
    Ok(())
}

/// What a session shows of each fact a capability promises.
#[derive(Clone, Copy, Debug, Default)]
struct Shown {
    own_cost: bool,
    compaction_trigger: bool,
    context_window: bool,
    prompt_composition: bool,
    titles: bool,
    subagent_tasks: bool,
    origins: bool,
}

impl Shown {
    fn of(s: &Session) -> Shown {
        let requests = || s.chains.iter().flat_map(|c| &c.requests);
        Shown {
            own_cost: s.reported_usd.is_some(),
            compaction_trigger: s.compactions.iter().any(|c| c.kind != CompactionTrigger::Unknown),
            context_window: requests().any(|r| r.context_window.is_some()),
            prompt_composition: requests().any(|r| r.prompt.is_some()),
            titles: s.title.is_some(),
            subagent_tasks: s.task.is_some(),
            origins: requests().any(|r| r.origin.is_some()),
        }
    }

    fn add(&mut self, other: &Shown) {
        self.own_cost |= other.own_cost;
        self.compaction_trigger |= other.compaction_trigger;
        self.context_window |= other.context_window;
        self.prompt_composition |= other.prompt_composition;
        self.titles |= other.titles;
        self.subagent_tasks |= other.subagent_tasks;
        self.origins |= other.origins;
    }

    /// Each fact: what it is, whether the capabilities declare it, and whether it shows. An own
    /// cost per request and one per session both show as the session's reported cost.
    fn against(&self, caps: &Capabilities) -> [(&'static str, bool, bool); 7] {
        [
            ("an own cost", caps.own_cost != OwnCost::None, self.own_cost),
            ("compaction triggers", caps.compaction_trigger, self.compaction_trigger),
            ("a context window", caps.context_window, self.context_window),
            ("prompts", caps.prompt_composition, self.prompt_composition),
            ("a title", caps.titles, self.titles),
            ("a subagent's task", caps.subagent_tasks, self.subagent_tasks),
            ("origins", caps.origins, self.origins),
        ]
    }
}

/// Records a decoder cannot read, fed between the real ones.
#[derive(Clone, Copy)]
enum Junk {
    Empty,
    NotJson,
    Truncated,
    UnknownType,
    NotAnObject,
}

impl Junk {
    const ALL: [Junk; 5] = [Junk::Empty, Junk::NotJson, Junk::Truncated, Junk::UnknownType, Junk::NotAnObject];

    fn name(self) -> &'static str {
        match self {
            Junk::Empty => "an empty record",
            Junk::NotJson => "a record that is not JSON",
            Junk::Truncated => "a truncated copy of a record",
            Junk::UnknownType => "a record of an unknown type",
            Junk::NotAnObject => "JSON that is not an object",
        }
    }

    /// The record fed before real record `i` (after the last one when `i` is their count).
    fn record(self, records: &[String], i: usize) -> String {
        match self {
            Junk::Empty => String::new(),
            Junk::NotJson => "not a record {".to_string(),
            Junk::Truncated => {
                let Some(r) = records.get(i).or(records.last()) else { return "{".to_string() };
                let mut cut = r.len() / 2;
                while !r.is_char_boundary(cut) {
                    cut -= 1;
                }
                r[..cut].to_string()
            }
            Junk::UnknownType => {
                r#"{"type":"tokprof-unknown","timestamp":"2026-01-01T00:00:00Z","payload":{"type":"tokprof-unknown"},"message":{"type":"tokprof-unknown"}}"#
                    .to_string()
            }
            Junk::NotAnObject => ["null", "42", "\"text\"", "[]", r#"[{"type":"user"},{"type":"assistant"}]"#][i % 5].to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU32, Ordering};

    use serde_json::Value;

    use super::*;
    use crate::model::{Chain, ChainKind, Request};
    use crate::source::{Activity, AgentId, AgentInfo, Decoder, Detail, Head, Records, Touched};

    /// What is wrong with the toy source.
    #[derive(Clone, Copy, PartialEq)]
    enum Flaw {
        None,
        /// The snapshot leaves out the latest request.
        SnapshotDrops,
        /// Taking a snapshot changes what the decoder gives.
        SnapshotChanges,
        /// A record that is not JSON is a request.
        ReadsJunk,
        /// A request names a message the session does not hold.
        BadIndex,
        /// Given its own log as a target, the source does not read it.
        ClaimsNothing,
        /// The session has a title its agent does not declare.
        UndeclaredTitle,
        /// The agent declares titles, but no session has one.
        MissingTitle,
    }

    /// A source of one log of three records, `{"n": <time>}`, each a request.
    struct Toy(Flaw);

    const LOG: &str = "/toy/log.jsonl";

    impl Toy {
        fn log(&self) -> SessionRef {
            let mut info = AgentInfo::named("toy");
            info.capabilities.titles = self.0 == Flaw::MissingTitle;
            let agent = AgentId::new(info);
            SessionRef {
                source: "toy",
                agent,
                locator: Locator::File(PathBuf::from(LOG)),
                session_id: "t1".into(),
                main_id: "t1".into(),
                subagent: false,
                size: 0,
                modified_ns: 0,
                created_ms: None,
            }
        }
    }

    struct Lines;

    impl Records for Lines {
        fn for_each(&mut self, f: &mut dyn FnMut(&str)) -> Result<()> {
            ["{\"n\":1}", "{\"n\":2}", "{\"n\":3}"].into_iter().for_each(f);
            Ok(())
        }
    }

    struct ToyDecoder {
        flaw: Flaw,
        agent: AgentId,
        times: Vec<i64>,
        snapshots: AtomicU32,
    }

    impl ToyDecoder {
        fn session(&self, snapshot: bool) -> Session {
            let mut s = Session::new(self.agent.clone(), "t1".into());
            let mut times = self.times.clone();
            if snapshot && self.flaw == Flaw::SnapshotDrops {
                times.pop();
            }
            let produced = (self.flaw == Flaw::BadIndex).then_some(7);
            let requests = times.into_iter().map(|ts_ms| Request { ts_ms, produced, ..Request::default() }).collect();
            s.chains.push(Chain { name: "main".into(), kind: ChainKind::Main, requests });
            if self.flaw == Flaw::SnapshotChanges {
                s.title = Some(format!("{} snapshots", self.snapshots.load(Ordering::Relaxed)));
            }
            if self.flaw == Flaw::UndeclaredTitle {
                s.title = Some("a title".into());
            }
            s
        }
    }

    impl Decoder for ToyDecoder {
        fn feed(&mut self, record: &str) {
            match serde_json::from_str::<Value>(record) {
                Ok(v) => self.times.extend(v.get("n").and_then(Value::as_i64)),
                Err(_) if self.flaw == Flaw::ReadsJunk && !record.is_empty() => self.times.push(4),
                Err(_) => {}
            }
        }

        fn snapshot(&self) -> Session {
            let s = self.session(true);
            self.snapshots.fetch_add(1, Ordering::Relaxed);
            s
        }

        fn finish(self: Box<Self>) -> Session {
            self.session(false)
        }
    }

    impl LogSource for Toy {
        fn name(&self) -> &'static str {
            "toy"
        }
        fn agents(&self) -> Vec<AgentId> {
            vec![self.log().agent]
        }
        fn detail(&self) -> Detail {
            Detail::Requests
        }
        fn discover(&self) -> Vec<SessionRef> {
            vec![self.log()]
        }
        fn session_at(&self, path: &Path) -> Option<SessionRef> {
            (path == Path::new(LOG)).then(|| self.log())
        }
        fn claim(&self, _: &Path) -> Claim {
            if self.0 == Flaw::ClaimsNothing { Claim::No } else { Claim::Yes }
        }
        fn watched_paths(&self) -> Vec<PathBuf> {
            Vec::new()
        }
        fn touched(&self, _: &Path, _: bool) -> Touched {
            Touched::Other
        }
        fn head(&self, _: &SessionRef) -> Head {
            Head { session_id: "t1".into(), ..Head::default() }
        }
        fn subagents(&self, _: &SessionRef) -> Vec<SessionRef> {
            Vec::new()
        }
        fn open(&self, _: &SessionRef) -> Result<Box<dyn Records>> {
            Ok(Box::new(Lines))
        }
        fn decoder(&self, s: &SessionRef) -> Box<dyn Decoder> {
            Box::new(ToyDecoder { flaw: self.0, agent: s.agent.clone(), times: Vec::new(), snapshots: AtomicU32::new(0) })
        }
        fn activity(&self, _: &SessionRef) -> Result<Activity> {
            Ok(Activity::default())
        }
    }

    fn error(flaw: Flaw) -> String {
        conform(&Toy(flaw)).expect_err("the flawed source fails").to_string()
    }

    #[test]
    fn a_sound_source_conforms() {
        conform(&Toy(Flaw::None)).unwrap();
    }

    #[test]
    fn each_flaw_fails_where_it_is() {
        let e = error(Flaw::SnapshotDrops);
        assert!(e.starts_with("toy: ") && e.contains("after 1 of 3 records, the snapshot differs from a decoder fed them and finished"), "{e}");
        assert!(e.contains("in Session > chains > Chain: expected `requests: [`, got `requests: [],`"), "a short hint of where: {e}");
        let e = error(Flaw::SnapshotChanges);
        assert!(e.contains("after 0 of 3 records, a second snapshot differs from the first"), "{e}");
        let e = error(Flaw::ReadsJunk);
        assert!(e.contains("a record that is not JSON between the records changes the session"), "{e}");
        let e = error(Flaw::BadIndex);
        assert!(e.contains("the whole log: request 0 of chain main produced message 7 of 0"), "{e}");
        let e = error(Flaw::ClaimsNothing);
        assert!(e.contains("does not read its own log given as a target"), "{e}");
        let e = error(Flaw::UndeclaredTitle);
        assert!(e.contains("the session shows a title without the capability"), "{e}");
        assert_eq!(error(Flaw::MissingTitle), "toy declares a title, but no fixture session shows it");
    }
}
