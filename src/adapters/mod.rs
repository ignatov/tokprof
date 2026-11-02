//! Agent log adapters: find session files and turn them into the normalized model.

pub mod claude;
pub mod codex;
mod util;

use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use anyhow::Result;

use crate::model::{Agent, Session};

#[derive(Clone, Debug, PartialEq)]
pub struct SourceFile {
    pub agent: Agent,
    pub path: PathBuf,
    pub size: u64,
    pub modified: SystemTime,
}

impl SourceFile {
    /// The file at `path`, if there is one.
    pub fn at(agent: Agent, path: &Path) -> Option<SourceFile> {
        let meta = fs::metadata(path).ok().filter(|m| m.is_file())?;
        Some(SourceFile { agent, path: path.to_path_buf(), size: meta.len(), modified: meta.modified().unwrap_or(SystemTime::UNIX_EPOCH) })
    }

    /// A subagent's log, which belongs to its parent session: a Claude Code transcript under
    /// `subagents/`, or a Codex rollout that names a parent thread.
    pub fn is_subagent(&self) -> bool {
        match self.agent {
            Agent::Claude => self.path.components().any(|c| c.as_os_str() == "subagents"),
            Agent::Codex => codex::is_child(&self.path),
        }
    }

    /// The logs of the subagents this session spawned, and for Codex of the threads they
    /// spawned in turn.
    pub fn subagent_files(&self) -> Vec<SourceFile> {
        let mut out = Vec::new();
        match self.agent {
            Agent::Claude if !self.is_subagent() => {
                let dir = self.path.with_extension("").join("subagents");
                walk(&dir, &mut |path, size, modified| {
                    if path.extension().is_some_and(|e| e == "jsonl") {
                        out.push(SourceFile { agent: Agent::Claude, path: path.to_path_buf(), size, modified });
                    }
                });
            }
            Agent::Codex => out.extend(codex::descendants(&self.path).iter().filter_map(|path| SourceFile::at(Agent::Codex, path))),
            Agent::Claude => {}
        }
        out
    }
}

/// The directories the agents keep their session logs in; some may not exist.
pub fn roots(agents: &[Agent]) -> Vec<(Agent, PathBuf)> {
    let mut out = Vec::new();
    for &agent in agents {
        let roots = match agent {
            Agent::Claude => vec![claude::root()],
            Agent::Codex => codex::roots(),
        };
        out.extend(roots.into_iter().map(|root| (agent, root)));
    }
    out
}

/// Whether a file in an agent's log directory is a session log.
pub fn is_log(agent: Agent, path: &Path) -> bool {
    match agent {
        Agent::Claude => path.extension().is_some_and(|e| e == "jsonl"),
        Agent::Codex => codex::is_rollout(path),
    }
}

/// The main log of the session a log belongs to: the log itself, or the log of the session that
/// spawned the subagent it records; `None` when that session's log is not known.
pub fn session_of(agent: Agent, path: &Path) -> Option<PathBuf> {
    match agent {
        Agent::Claude => Some(claude::parent_of(path).unwrap_or_else(|| path.to_path_buf())),
        Agent::Codex => codex::session_of(path),
    }
}

pub fn discover(agents: &[Agent]) -> Vec<SourceFile> {
    let mut out: Vec<SourceFile> = roots(agents).iter().flat_map(|(agent, root)| logs_in(*agent, root)).collect();
    // Largest first, so the parallel parse does not end on one huge file.
    out.sort_by_key(|f| std::cmp::Reverse(f.size));
    out
}

/// The session logs in a directory and its subdirectories.
pub fn logs_in(agent: Agent, dir: &Path) -> Vec<SourceFile> {
    let mut out = Vec::new();
    walk(dir, &mut |path, size, modified| {
        if is_log(agent, path) {
            out.push(SourceFile { agent, path: path.to_path_buf(), size, modified });
        }
    });
    out
}

pub fn parse(file: &SourceFile) -> Result<Session> {
    match file.agent {
        Agent::Claude => claude::parse_file(&file.path),
        Agent::Codex => codex::parse_file(&file.path),
    }
}

pub(crate) fn walk(dir: &Path, f: &mut dyn FnMut(&Path, u64, SystemTime)) {
    let Ok(entries) = fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let Ok(meta) = entry.metadata() else { continue };
        let path = entry.path();
        if meta.is_dir() {
            walk(&path, f);
        } else if meta.is_file() {
            f(&path, meta.len(), meta.modified().unwrap_or(SystemTime::UNIX_EPOCH));
        }
    }
}

pub use util::open;
pub(crate) use util::{for_each_line, parse_ts_ms};

/// A path shown relative to the home directory.
pub fn display_path(path: &str) -> String {
    util::display_path(path, None)
}

/// A path shown relative to the project, or to the home directory when it is outside.
pub fn project_path(path: &str, cwd: Option<&str>) -> String {
    util::display_path(path, cwd)
}

pub(crate) fn home() -> PathBuf {
    std::env::home_dir().unwrap_or_default()
}
