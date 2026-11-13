//! The sources tokprof reads, in registry order, and what is asked of all of them at once.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use anyhow::{Result, bail};

use crate::kept::Kept;
use crate::model::Session;
use crate::place::{Placed, Placement, Placer, Remembered, Rules};
use crate::source::{Activity, AgentId, Claim, Head, LogSource, SessionRef, Touched};

pub struct Catalog {
    sources: Vec<Box<dyn LogSource>>,
    /// The home directory, which paths are shown relative to.
    home: PathBuf,
    /// The logs read so far, once `keep_logs` was called.
    kept: OnceLock<Kept>,
    /// The projects of the folders placed so far (see `place`).
    placer: Placer,
}

impl Catalog {
    /// A catalog that places the folders of its logs from disk only (see `with_places`).
    pub fn new(sources: Vec<Box<dyn LogSource>>, home: PathBuf) -> Catalog {
        let placer = Placer::new(Rules::from_disk(home.clone()));
        Catalog { sources, home, kept: OnceLock::new(), placer }
    }

    /// The catalog, placing folders also by the temporary roots, and by the repositories
    /// remembered in the file at `repositories`, such as `<cache folder>/repositories-1.json`.
    pub fn with_places(mut self, temp_roots: Vec<String>, repositories: PathBuf) -> Catalog {
        self.placer = Placer::new(Rules { home: self.home.clone(), temp_roots, remembered: Some(Remembered::at(repositories)) });
        self
    }

    /// The project of a folder of a log of `source`, written with `~` for the home directory
    /// as the sources write it, and how it was placed (see `place::Rules::place`). Each pair of
    /// source and folder is placed once.
    pub fn place(&self, source: &str, folder: &str) -> Placement {
        self.placer.place(self.source_named(source), folder)
    }

    /// The name of the first source that declares the agent, which places the folders of the
    /// agent's logs.
    pub fn source_of(&self, agent: &AgentId) -> Option<&'static str> {
        self.sources.iter().find(|s| s.agents().contains(agent)).map(|s| s.name())
    }

    /// From now on, keeps the recent logs it reads, so that reading one again reads only what
    /// was appended since, for a reader that runs for long and reads growing logs again and
    /// again. What `parse` gives stays the same (see `kept`).
    pub fn keep_logs(&self) {
        self.kept.get_or_init(Kept::default);
    }

    pub fn home(&self) -> &Path {
        &self.home
    }

    /// The agents the sources declare, in registry order.
    pub fn agents(&self) -> Vec<AgentId> {
        let mut out: Vec<AgentId> = Vec::new();
        for agent in self.sources.iter().flat_map(|s| s.agents()) {
            if !out.contains(&agent) {
                out.push(agent);
            }
        }
        out
    }

    /// The declared agent of this name.
    pub fn agent(&self, name: &str) -> Result<AgentId> {
        match self.agents().into_iter().find(|a| a.name() == name) {
            Some(agent) => Ok(agent),
            None => bail!("unknown agent {name:?}"),
        }
    }

    /// Where an agent sorts: the declared agents in registry order, then the others by name.
    pub fn order(&self, agent: &AgentId) -> (usize, String) {
        let agents = self.agents();
        (agents.iter().position(|a| a == agent).unwrap_or(agents.len()), agent.name().to_string())
    }

    fn source(&self, s: &SessionRef) -> &dyn LogSource {
        self.source_named(s.source).unwrap_or_else(|| panic!("no source {:?} in the catalog", s.source))
    }

    fn source_named(&self, name: &str) -> Option<&dyn LogSource> {
        self.sources.iter().find(|s| s.name() == name).map(|s| s.as_ref())
    }

    /// The sources that declare one of these agents.
    fn sources_of<'a>(&'a self, agents: &'a [AgentId]) -> impl Iterator<Item = &'a dyn LogSource> + 'a {
        self.sources.iter().map(|s| s.as_ref()).filter(|s| s.agents().iter().any(|a| agents.contains(a)))
    }

    /// The logs of these agents, largest first so a parallel parse does not end on one huge file.
    ///
    /// A session that two sources report is kept from the source that shows more of it; the
    /// other source's log of it is left out. A session only one source reports is kept from it.
    pub fn discover(&self, agents: &[AgentId]) -> Vec<SessionRef> {
        let found: Vec<(usize, SessionRef)> = self
            .sources
            .iter()
            .enumerate()
            .filter(|(_, s)| s.agents().iter().any(|a| agents.contains(a)))
            .flat_map(|(i, s)| s.discover().into_iter().map(move |r| (i, r)))
            .filter(|(_, r)| agents.contains(&r.agent))
            .collect();
        let key = |r: &SessionRef| (r.agent.name().to_string(), r.session_id.clone());
        let rank = |i: usize| (self.sources[i].detail(), std::cmp::Reverse(i));
        let mut best: HashMap<(String, String), (crate::source::Detail, std::cmp::Reverse<usize>)> = HashMap::new();
        for (i, r) in &found {
            let e = best.entry(key(r)).or_insert(rank(*i));
            *e = (*e).max(rank(*i));
        }
        let mut out: Vec<SessionRef> = found.into_iter().filter(|(i, r)| best[&key(r)] == rank(*i)).map(|(_, r)| r).collect();
        out.sort_by_key(|r| std::cmp::Reverse(r.size));
        out
    }

    /// The log at a path given as a target, read by the source that claims it, wherever it lies.
    pub fn resolve(&self, path: &Path) -> Option<SessionRef> {
        let claimed = |claim| self.sources.iter().find(|s| s.claim(path) == claim);
        claimed(Claim::Yes).or_else(|| claimed(Claim::Fallback))?.session_at(path)
    }

    /// The log at a path, as a source finds it there now.
    pub fn session_at(&self, source: &str, path: &Path) -> Option<SessionRef> {
        self.source_named(source)?.session_at(path)
    }

    /// The session a log holds, with its project placed from the folder the log names, which
    /// it keeps as its `folder`.
    pub fn parse(&self, s: &SessionRef) -> Result<Session> {
        let source = self.source(s);
        let mut session = match self.kept.get() {
            Some(kept) => kept.parse(source, s),
            None => crate::kept::read_whole(source, s),
        }?;
        (session.folder, session.placed) = self.placed(s.source, &mut session.project);
        Ok(session)
    }

    /// What the first records of a log tell, with the project placed as `parse` places it.
    pub fn head(&self, s: &SessionRef) -> Head {
        let mut head = self.source(s).head(s);
        (head.folder, head.placed) = self.placed(s.source, &mut head.project);
        head
    }

    /// Puts the project of the folder in its place, and gives the folder and how it was placed.
    fn placed(&self, source: &str, project: &mut String) -> (String, Option<Placed>) {
        let placement = self.place(source, project);
        (std::mem::replace(project, placement.project), Some(placement.placed))
    }

    /// When a session last did something, in epoch ms: what its source reads from the end of the
    /// log, or else the log's last write, and never later than the last write.
    pub fn last_active(&self, s: &SessionRef) -> i64 {
        let written = s.modified_ms();
        match self.source(s).last_active(s) {
            // A file system that keeps no modification time gives 0, which bounds nothing.
            Some(t) if written > 0 => t.min(written),
            Some(t) => t,
            None => written,
        }
    }

    /// When a session first did something, in epoch ms, as its source reads it from the start of
    /// the log; `None` when the source cannot tell.
    pub fn first_active(&self, s: &SessionRef) -> Option<i64> {
        self.source(s).first_active(s)
    }

    pub fn subagents(&self, s: &SessionRef) -> Vec<SessionRef> {
        self.source(s).subagents(s)
    }

    pub fn activity(&self, s: &SessionRef) -> Result<Activity> {
        self.source(s).activity(s)
    }

    /// The directories to watch for these agents, with the source each belongs to.
    pub fn watched(&self, agents: &[AgentId]) -> Vec<(&'static str, PathBuf)> {
        self.sources_of(agents).flat_map(|s| s.watched_paths().into_iter().map(move |p| (s.name(), p))).collect()
    }

    /// The files whose writes change a session (see `LogSource::polled`).
    pub fn polled(&self, s: &SessionRef) -> Vec<PathBuf> {
        self.source(s).polled(s)
    }

    pub fn touched(&self, source: &str, changed: &Path, arrived: bool) -> Touched {
        self.source_named(source).map_or(Touched::Other, |s| s.touched(changed, arrived))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source::{AgentInfo, Decoder, Detail, Locator, Records};

    struct Fake {
        name: &'static str,
        group: Option<&'static str>,
        agents: Vec<AgentId>,
        detail: Detail,
        refs: Vec<SessionRef>,
        claim: Claim,
        active: Option<i64>,
    }

    impl LogSource for Fake {
        fn name(&self) -> &'static str {
            self.name
        }
        fn last_active(&self, _: &SessionRef) -> Option<i64> {
            self.active
        }
        fn folder_group(&self, _: &Path) -> Option<&'static str> {
            self.group
        }
        fn agents(&self) -> Vec<AgentId> {
            self.agents.clone()
        }
        fn detail(&self) -> Detail {
            self.detail
        }
        fn discover(&self) -> Vec<SessionRef> {
            self.refs.clone()
        }
        fn session_at(&self, path: &Path) -> Option<SessionRef> {
            Some(log(self.name, &self.agents[0], path.to_str()?, 1))
        }
        fn claim(&self, _: &Path) -> Claim {
            self.claim
        }
        fn watched_paths(&self) -> Vec<PathBuf> {
            Vec::new()
        }
        fn touched(&self, _: &Path, _: bool) -> Touched {
            Touched::Other
        }
        fn head(&self, _: &SessionRef) -> Head {
            Head::default()
        }
        fn subagents(&self, _: &SessionRef) -> Vec<SessionRef> {
            Vec::new()
        }
        fn open(&self, _: &SessionRef) -> Result<Box<dyn Records>> {
            bail!("no records")
        }
        fn decoder(&self, _: &SessionRef) -> Box<dyn Decoder> {
            unimplemented!()
        }
        fn activity(&self, _: &SessionRef) -> Result<Activity> {
            Ok(Activity::default())
        }
    }

    fn agent(name: &str) -> AgentId {
        AgentId::new(AgentInfo::named(name))
    }

    fn log(source: &'static str, agent: &AgentId, id: &str, size: u64) -> SessionRef {
        SessionRef {
            source,
            agent: agent.clone(),
            locator: Locator::File(PathBuf::from(format!("/{source}/{id}"))),
            session_id: id.to_string(),
            main_id: id.to_string(),
            subagent: false,
            size,
            modified_ns: 0,
            created_ms: None,
        }
    }

    fn fake(name: &'static str, agents: &[&AgentId], detail: Detail, refs: Vec<SessionRef>, claim: Claim) -> Box<dyn LogSource> {
        Box::new(Fake { name, group: None, agents: agents.iter().map(|a| (*a).clone()).collect(), detail, refs, claim, active: None })
    }

    #[test]
    fn the_last_activity_is_never_after_the_last_write() {
        let a = agent("a");
        let source = Fake { name: "one", group: None, agents: vec![a.clone()], detail: Detail::Requests, refs: vec![], claim: Claim::No, active: Some(5_000) };
        let c = Catalog::new(vec![Box::new(source)], PathBuf::new());
        let mut s = log("one", &a, "s1", 1);
        s.modified_ns = 9_000 * 1_000_000;
        assert_eq!(c.last_active(&s), 5_000, "the time the source reads");
        s.modified_ns = 3_000 * 1_000_000;
        assert_eq!(c.last_active(&s), 3_000, "never after the last write");
        s.modified_ns = 0;
        assert_eq!(c.last_active(&s), 5_000, "a file system without modification times bounds nothing");
        let c = Catalog::new(vec![fake("one", &[&a], Detail::Requests, vec![], Claim::No)], PathBuf::new());
        s.modified_ns = 9_000 * 1_000_000;
        assert_eq!(c.last_active(&s), 9_000, "a source that cannot tell gives the last write");
    }

    #[test]
    fn a_claim_beats_a_fallback() {
        let (a, b) = (agent("a"), agent("b"));
        let c = Catalog::new(
            vec![fake("first", &[&a], Detail::Requests, vec![], Claim::Fallback), fake("second", &[&b], Detail::Requests, vec![], Claim::Yes)],
            PathBuf::new(),
        );
        assert_eq!(c.resolve(Path::new("x")).unwrap().source, "second");
        let c = Catalog::new(vec![fake("only", &[&a], Detail::Requests, vec![], Claim::Fallback)], PathBuf::new());
        assert_eq!(c.resolve(Path::new("x")).unwrap().source, "only");
        let c = Catalog::new(vec![fake("none", &[&a], Detail::Requests, vec![], Claim::No)], PathBuf::new());
        assert!(c.resolve(Path::new("x")).is_none());
    }

    #[test]
    fn agents_are_found_by_name_in_registry_order() {
        let (b, a) = (agent("b"), agent("a"));
        let c = Catalog::new(
            vec![fake("one", &[&b], Detail::Requests, vec![], Claim::No), fake("two", &[&a, &b], Detail::Turns, vec![], Claim::No)],
            PathBuf::new(),
        );
        assert_eq!(c.agents(), [b.clone(), a.clone()]);
        assert_eq!(c.agent("a").unwrap(), a);
        assert_eq!(c.agent("x").unwrap_err().to_string(), "unknown agent \"x\"");
        // An agent found only at run time sorts after the declared ones, by name.
        let mut agents = [agent("zed"), a.clone(), agent("found"), b.clone()];
        agents.sort_by_key(|x| c.order(x));
        assert_eq!(agents.iter().map(AgentId::name).collect::<Vec<_>>(), ["b", "a", "found", "zed"]);
    }

    #[test]
    fn a_session_two_sources_report_counts_once() {
        let a = agent("a");
        let native = vec![log("native", &a, "s1", 10), log("native", &a, "s2", 20)];
        let turns = vec![log("turns", &a, "s1", 5), log("turns", &a, "s3", 7)];
        let c = Catalog::new(
            vec![fake("turns", &[&a], Detail::Turns, turns, Claim::No), fake("native", &[&a], Detail::Requests, native, Claim::No)],
            PathBuf::new(),
        );
        let found = c.discover(std::slice::from_ref(&a));
        let found: Vec<(&str, &str)> = found.iter().map(|r| (r.source, r.session_id.as_str())).collect();
        // The source with requests wins s1; s3 is only in the other source, which fills it in.
        assert_eq!(found, [("native", "s2"), ("native", "s1"), ("turns", "s3")]);
        // Only the agents asked for.
        assert!(c.discover(&[agent("b")]).is_empty());
    }

    #[test]
    fn a_catalog_places_folders_by_the_rules_it_was_given() {
        use crate::place::{Placed, TEMPORARY};
        let a = agent("a");
        let chats =
            Fake { name: "chats", group: Some("chats"), agents: vec![a.clone()], detail: Detail::Requests, refs: vec![], claim: Claim::No, active: None };
        let plain = fake("plain", &[&a], Detail::Requests, vec![], Claim::No);
        let home = std::env::temp_dir().join(format!("tokprof-catalog-place-{}", std::process::id()));
        let temp = home.join("t").to_string_lossy().to_string();
        let run = format!("{temp}{}run", std::path::MAIN_SEPARATOR);
        let c = Catalog::new(vec![plain, Box::new(chats)], home.clone());
        assert_eq!(c.place("chats", &run).placed, Placed::Group, "the source's group first");
        // From disk only: no temporary roots.
        assert_eq!(c.place("plain", &run).placed, Placed::NotFound);
        assert_eq!(c.source_of(&a), Some("plain"));
        let c = Catalog::new(vec![fake("plain", &[&a], Detail::Requests, vec![], Claim::No)], home.clone())
            .with_places(vec![temp], home.join("cache").join(crate::place::REPOSITORIES_FILE));
        assert_eq!(c.place("plain", &run).project, TEMPORARY);
        assert!(!home.exists(), "nothing learned, nothing written");
    }
}
