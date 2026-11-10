//! pi session logs: `$PI_CODING_AGENT_SESSION_DIR`, else `$PI_CODING_AGENT_DIR/sessions`, by
//! default `~/.pi/agent/sessions`, as `--<path>--/<time>_<uuid>.jsonl` in pi 0.84.2. One session
//! per file; its entries form a tree, and every branch stays in the file. A fork (`/fork`,
//! `/clone`, `--fork`) is a new file that names the one it came from in `parentSession` and
//! copies its entries, with their ids and times.

mod decoder;

use std::collections::HashSet;
use std::fs::Metadata;
use std::io::BufRead;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::Result;
use serde_json::Value;
use tokprof_core::labels::display_path;
use tokprof_core::source::{
    Activity, AgentId, AgentInfo, Capabilities, Claim, Decoder, Detail, Harness, Head, Locator, LogSource, OwnCost, PublicNames, ReadOn, Records, SessionRef,
    Touched,
};
use tokprof_logio::{file_times, parse_ts_ms, str_at, walk};

use decoder::{Parser, entry_key};

pub struct Pi {
    /// The session directory.
    root: PathBuf,
    /// The home directory, which paths are shown relative to.
    home: PathBuf,
    agent: AgentId,
}

impl Pi {
    /// The logs under `$PI_CODING_AGENT_SESSION_DIR`, else `$PI_CODING_AGENT_DIR/sessions`, by
    /// default `~/.pi/agent/sessions`.
    pub fn from_env(home: &Path) -> Pi {
        let var = |name: &str| std::env::var_os(name).filter(|v| !v.is_empty()).map(PathBuf::from);
        let root = var("PI_CODING_AGENT_SESSION_DIR")
            .or_else(|| var("PI_CODING_AGENT_DIR").map(|d| d.join("sessions")))
            .unwrap_or_else(|| home.join(".pi/agent/sessions"));
        Pi::at(root, home)
    }

    /// The logs under a session directory.
    pub fn at(root: PathBuf, home: &Path) -> Pi {
        Pi { root, home: home.to_path_buf(), agent: Pi::agent() }
    }

    pub fn agent() -> AgentId {
        AgentId::new(AgentInfo {
            name: "pi".to_string(),
            title: "pi".to_string(),
            // A pi id is a UUIDv7, as Codex's: the first 13 characters are its millisecond.
            short_id_len: 18,
            instruction_files: vec!["AGENTS.md".to_string(), "CLAUDE.md".to_string()],
            ttl_settings: None,
            setup_hints: Vec::new(),
            cost_counter: Some("pi's own cost count".to_string()),
            // No pickup measured; the compaction call is not known to read the cache, and the
            // ones seen read nothing from it.
            harness: Harness { pickup: None, compaction_reads_cache: false },
            capabilities: Capabilities {
                own_cost: OwnCost::Session,
                compaction_trigger: false,
                context_window: false,
                prompt_composition: false,
                titles: false,
                subagent_tasks: false,
                origins: false,
            },
            public_names: PublicNames::default(),
        })
    }

    fn reference(&self, path: &Path, meta: &Metadata) -> SessionRef {
        let session_id = id_from_name(path);
        let (modified_ns, created_ms) = file_times(meta);
        SessionRef {
            source: "pi",
            agent: self.agent.clone(),
            locator: Locator::File(path.to_path_buf()),
            main_id: session_id.clone(),
            session_id,
            subagent: false,
            size: meta.len(),
            modified_ns,
            created_ms,
        }
    }

    fn logs_in(&self, dir: &Path) -> Vec<PathBuf> {
        let mut out = Vec::new();
        walk(dir, &mut |path, _| {
            if is_log(path) {
                out.push(path.to_path_buf());
            }
        });
        out
    }

    /// The entries a fork copied: those of the log its header names, when that log is there,
    /// at its path or next to the fork.
    fn copied(&self, path: &Path) -> HashSet<String> {
        let Some(parent) = header(path).as_ref().and_then(|h| str_at(h, "parentSession")).map(PathBuf::from) else { return HashSet::new() };
        let beside = parent.file_name().and_then(|name| Some(path.parent()?.join(name)));
        let Some(parent) = [Some(parent), beside].into_iter().flatten().find(|p| p != path && p.is_file()) else { return HashSet::new() };
        let mut keys = HashSet::new();
        let _ = tokprof_logio::for_each_line(&parent, |line| {
            if let Ok(v) = serde_json::from_str::<Value>(line)
                && str_at(&v, "type") != Some("session")
                && let Some(k) = entry_key(&v)
            {
                keys.insert(k);
            }
        });
        keys
    }
}

impl LogSource for Pi {
    fn name(&self) -> &'static str {
        "pi"
    }

    fn agents(&self) -> Vec<AgentId> {
        vec![self.agent.clone()]
    }

    fn detail(&self) -> Detail {
        Detail::Requests
    }

    fn discover(&self) -> Vec<SessionRef> {
        let mut out = Vec::new();
        walk(&self.root, &mut |path, meta| {
            if is_log(path) {
                out.push(self.reference(path, meta));
            }
        });
        out
    }

    fn session_at(&self, path: &Path) -> Option<SessionRef> {
        let meta = std::fs::metadata(path).ok().filter(|m| m.is_file())?;
        Some(self.reference(path, &meta))
    }

    /// A log in the session directory, or one elsewhere that starts with pi's header.
    fn claim(&self, path: &Path) -> Claim {
        let pi_header = || header(path).is_some_and(|h| h.get("version").is_some() && h.get("cwd").is_some());
        if is_log(path) && (path.starts_with(&self.root) || pi_header()) { Claim::Yes } else { Claim::No }
    }

    fn watched_paths(&self) -> Vec<PathBuf> {
        vec![self.root.clone()]
    }

    fn touched(&self, changed: &Path, arrived: bool) -> Touched {
        if is_log(changed) {
            Touched::Log(Some(changed.to_path_buf()))
        } else if arrived && changed.is_dir() {
            Touched::Logs(self.logs_in(changed))
        } else {
            Touched::Other
        }
    }

    fn head(&self, s: &SessionRef) -> Head {
        let h = header(s.path()).unwrap_or_default();
        let project = str_at(&h, "cwd").map(|cwd| display_path(cwd, None, &self.home)).unwrap_or_default();
        let session_id = str_at(&h, "id").filter(|id| !id.is_empty()).map_or_else(|| s.session_id.clone(), str::to_string);
        Head { project, session_id, client: None }
    }

    fn first_active(&self, s: &SessionRef) -> Option<i64> {
        parse_ts_ms(header(s.path())?.get("timestamp"))
    }

    fn subagents(&self, _: &SessionRef) -> Vec<SessionRef> {
        Vec::new()
    }

    fn open(&self, s: &SessionRef) -> Result<Box<dyn Records>> {
        Ok(Box::new(Lines(s.path().to_path_buf())))
    }

    fn decoder(&self, s: &SessionRef) -> Box<dyn Decoder> {
        Box::new(Parser::new(self.agent.clone(), id_from_name(s.path()), self.home.clone(), Arc::new(self.copied(s.path()))))
    }

    /// Where the session worked and when; pi's messages and tools are not read yet.
    fn activity(&self, s: &SessionRef) -> Result<Activity> {
        let mut a = Activity::default();
        tokprof_logio::for_each_line(s.path(), |line| {
            let Ok(v) = serde_json::from_str::<Value>(line) else { return };
            if str_at(&v, "type") == Some("session") {
                a.cwd = str_at(&v, "cwd").map(str::to_string);
            } else {
                a.seen(parse_ts_ms(v.get("timestamp")));
            }
        })?;
        Ok(a)
    }
}

/// The lines of a log.
struct Lines(PathBuf);

impl Records for Lines {
    fn for_each(&mut self, f: &mut dyn FnMut(&str)) -> Result<()> {
        tokprof_logio::for_each_line(&self.0, f)
    }

    fn for_each_from(&mut self, from: u64, check: &[u8], f: &mut dyn FnMut(&str)) -> Result<Option<ReadOn>> {
        let read = tokprof_logio::for_each_line_from(&self.0, from, check, f)?;
        Ok(read.map(|r| ReadOn { end: r.end, check: r.check, open: r.open }))
    }
}

fn is_log(path: &Path) -> bool {
    path.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.ends_with(".jsonl"))
}

/// The first line of a log, when it is pi's `session` header.
fn header(path: &Path) -> Option<Value> {
    let mut line = String::new();
    tokprof_logio::open(path).ok()?.read_line(&mut line).ok()?;
    let v: Value = serde_json::from_str(&line).ok()?;
    (str_at(&v, "type") == Some("session")).then_some(v)
}

/// The UUID at the end of a log's name, after the `_`.
fn id_from_name(path: &Path) -> String {
    let stem = path.file_name().and_then(|n| n.to_str()).unwrap_or_default().trim_end_matches(".jsonl");
    stem.rsplit_once('_').map_or(stem, |(_, id)| id).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_id_comes_from_the_name() {
        let p = Path::new("/s/--h--/2026-08-16T20-20-40-708Z_01a00c3b-cc83-7a8d-a89b-262bd2f8ecac.jsonl");
        assert_eq!(id_from_name(p), "01a00c3b-cc83-7a8d-a89b-262bd2f8ecac");
    }

    #[test]
    fn the_session_directory_comes_from_the_environment() {
        // The variables are read by `from_env`; `at` takes the directory they name.
        let pi = Pi::at(PathBuf::from("/data/pi/sessions"), Path::new("/h"));
        assert_eq!(pi.watched_paths(), [PathBuf::from("/data/pi/sessions")]);
        assert_eq!(pi.claim(Path::new("/data/pi/sessions/--w--/a_b.jsonl")), Claim::Yes);
        assert_eq!(pi.claim(Path::new("/data/pi/sessions/store.sqlite")), Claim::No);
    }
}
