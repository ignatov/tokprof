//! The sources tokprof reads, in registry order, and what is asked of all of them at once.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{Result, bail};

use crate::model::Session;
use crate::source::{Activity, AgentId, Claim, Head, LogSource, SessionRef, Touched};

pub struct Catalog {
    sources: Vec<Box<dyn LogSource>>,
    /// The home directory, which paths are shown relative to.
    home: PathBuf,
}

impl Catalog {
    pub fn new(sources: Vec<Box<dyn LogSource>>, home: PathBuf) -> Catalog {
        Catalog { sources, home }
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

    pub fn parse(&self, s: &SessionRef) -> Result<Session> {
        let source = self.source(s);
        let mut decoder = source.decoder(s);
        source.open(s)?.for_each(&mut |record| decoder.feed(record))?;
        Ok(decoder.finish())
    }

    pub fn head(&self, s: &SessionRef) -> Head {
        self.source(s).head(s)
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
        agents: Vec<AgentId>,
        detail: Detail,
        refs: Vec<SessionRef>,
        claim: Claim,
    }

    impl LogSource for Fake {
        fn name(&self) -> &'static str {
            self.name
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
        Box::new(Fake { name, agents: agents.iter().map(|a| (*a).clone()).collect(), detail, refs, claim })
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
        let c = Catalog::new(vec![fake("one", &[&b], Detail::Requests, vec![], Claim::No), fake("two", &[&a, &b], Detail::Turns, vec![], Claim::No)], PathBuf::new());
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
}
