//! The one interface through which tokprof reads agent logs.
//!
//! A [`LogSource`] finds, opens and watches the logs of one place, and knows their layout. Its
//! [`Decoder`] turns the records of one log into a [`Session`] and touches no file, no clock and no
//! environment. An agent is a property of each session a source reports, not of the source, so
//! one source can report sessions of several agents, some known only at run time. Nothing outside
//! the sources knows a log format, a log location or the names of an agent's tools.

use std::collections::HashMap;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::Result;

use crate::model::Session;

/// An agent: a shared handle to what is known about it. Agents compare, hash and order by name.
#[derive(Clone)]
pub struct AgentId(Arc<AgentInfo>);

impl AgentId {
    pub fn new(info: AgentInfo) -> AgentId {
        AgentId(Arc::new(info))
    }

    /// An agent that no source declared: only its name is known.
    pub fn unknown(name: &str) -> AgentId {
        AgentId::new(AgentInfo::named(name))
    }

    /// The name the command line and the profiles use, such as `claude`.
    pub fn name(&self) -> &str {
        &self.0.name
    }

    pub fn info(&self) -> &AgentInfo {
        &self.0
    }
}

impl PartialEq for AgentId {
    fn eq(&self, other: &AgentId) -> bool {
        self.name() == other.name()
    }
}

impl Eq for AgentId {}

impl Hash for AgentId {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.name().hash(state);
    }
}

impl PartialOrd for AgentId {
    fn partial_cmp(&self, other: &AgentId) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for AgentId {
    fn cmp(&self, other: &AgentId) -> std::cmp::Ordering {
        self.name().cmp(other.name())
    }
}

impl fmt::Debug for AgentId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

impl fmt::Display for AgentId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// What tokprof knows about an agent. A fact it lacks is absent, never borrowed from another
/// agent: an agent without a pickup calibration gets no pickup estimate.
#[derive(Clone, Debug)]
pub struct AgentInfo {
    /// The name the command line and the profiles use, such as `claude`.
    pub name: String,
    /// The name reports print, such as `Claude Code`.
    pub title: String,
    /// Characters of a session id that the lists and profiles show.
    pub short_id_len: usize,
    /// The files the agent loads as instructions, such as `CLAUDE.md`.
    pub instruction_files: Vec<String>,
    /// The settings that choose the cache lifetime of the main conversation and of subagents.
    pub ttl_settings: Option<(String, String)>,
    /// What shrinks each setup source the agent can name a remedy for: the source, as the
    /// reminders are labeled, and the hint, in the agent's own terms.
    pub setup_hints: Vec<(String, String)>,
    /// What the agent's own cost counter is called, for the reconciliation line.
    pub cost_counter: Option<String>,
    pub harness: Harness,
    pub capabilities: Capabilities,
}

impl AgentInfo {
    /// The facts of an agent of which only the name is known.
    pub fn named(name: &str) -> AgentInfo {
        AgentInfo {
            name: name.to_string(),
            title: name.to_string(),
            short_id_len: 8,
            instruction_files: Vec::new(),
            ttl_settings: None,
            setup_hints: Vec::new(),
            cost_counter: None,
            harness: Harness::default(),
            capabilities: Capabilities::default(),
        }
    }
}

/// How the agent's harness works, where tokprof needs it for an estimate.
#[derive(Clone, Debug, Default)]
pub struct Harness {
    /// What picking up a session's work in a new session takes, as measured for this agent.
    pub pickup: Option<Pickup>,
}

/// A pickup as the reference bench measured it: the tokens a new session takes in, over how many
/// requests, with how much output.
#[derive(Clone, Copy, Debug)]
pub struct Pickup {
    pub taken_tokens: f64,
    pub requests: f64,
    pub output_tokens: f64,
}

/// What the agent's logs can tell. When a capability is missing, a view shows nothing or "not
/// known" rather than zero.
#[derive(Clone, Debug, Default)]
pub struct Capabilities {
    pub own_cost: OwnCost,
    pub compaction_trigger: bool,
    pub context_window: bool,
    pub prompt_composition: bool,
    pub titles: bool,
    pub subagent_tasks: bool,
    pub origins: bool,
}

/// Where the agent reports what a session cost it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum OwnCost {
    #[default]
    None,
    Session,
    Request,
}

/// Where the records of a session are.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Locator {
    File(PathBuf),
    Dir(PathBuf),
    Row { db: PathBuf, id: String },
}

impl Locator {
    /// The file or directory to show and to open: the log, its directory, or the database.
    pub fn path(&self) -> &Path {
        match self {
            Locator::File(p) | Locator::Dir(p) => p,
            Locator::Row { db, .. } => db,
        }
    }
}

/// One log a source found: a main session's or a subagent's.
#[derive(Clone, Debug, PartialEq)]
pub struct SessionRef {
    /// The name of the source that found it.
    pub source: &'static str,
    pub agent: AgentId,
    pub locator: Locator,
    /// The agent's own id of this log's session, as the log's name or first records give it.
    pub session_id: String,
    /// The id of the main session the log belongs to: its own, or for a subagent's log the id of
    /// the session that started it, when that session's log is known.
    pub main_id: String,
    /// A subagent's log, which belongs to its main session.
    pub subagent: bool,
    pub size: u64,
    /// When the log was last written, in epoch nanoseconds, as precise as the file system keeps it.
    pub modified_ns: i128,
    /// When the log was created, when the file system tells.
    pub created_ms: Option<i64>,
}

impl SessionRef {
    pub fn path(&self) -> &Path {
        self.locator.path()
    }

    /// When the log was last written, in epoch ms.
    pub fn modified_ms(&self) -> i64 {
        (self.modified_ns / 1_000_000) as i64
    }
}

/// Whether a source reads the file at a path, given as a target.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Claim {
    No,
    Yes,
    /// It reads it when no source says yes.
    Fallback,
}

/// What a changed path means to a source.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Touched {
    /// Not a log of this source.
    Other,
    /// A log: the path of its main session's log, when that is known.
    Log(Option<PathBuf>),
    /// A directory that arrived with logs: the paths of their main sessions' logs.
    Logs(Vec<PathBuf>),
}

/// What the first records of a log tell, without reading the rest.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Head {
    /// The project, written with `~` for the home directory.
    pub project: String,
    /// The agent's own session id, or the id the log's name gives.
    pub session_id: String,
    /// The first client the log names: the program that ran the agent.
    pub client: Option<String>,
}

/// How much of the work a source's logs show, which decides between two sources of one session.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Detail {
    /// Usage per turn.
    Turns,
    /// Every request with its usage and prompt.
    Requests,
}

/// The records of one log, in order.
pub trait Records {
    /// Calls `f` with each complete record. A record still being written is left out; invalid
    /// UTF-8 is replaced.
    fn for_each(&mut self, f: &mut dyn FnMut(&str)) -> Result<()>;
}

/// Turns the records of one log into a session.
pub trait Decoder {
    /// Takes one record. A record the decoder cannot read is skipped.
    fn feed(&mut self, record: &str);
    /// The session the records so far give, leaving the decoder as it is.
    fn snapshot(&self) -> Session;
    /// The session the records give.
    fn finish(self: Box<Self>) -> Session;
}

/// What a log tells about where the work stopped, for a handoff note.
#[derive(Clone, Debug, Default)]
pub struct Activity {
    pub cwd: Option<String>,
    pub branch: Option<String>,
    pub first_ms: Option<i64>,
    pub last_ms: Option<i64>,
    /// Prompts the user typed, with their times.
    pub prompts: Vec<(i64, String)>,
    /// The agent's latest text.
    pub last_text: Option<String>,
    /// The latest task list: status and text of each item.
    pub todos: Vec<(String, String)>,
    pub changed: Vec<String>,
    /// Read count and the order a file was first read in, for stable ties.
    pub read: HashMap<String, (usize, usize)>,
    pub commands: Vec<String>,
}

impl Activity {
    /// Notes the time of a record of the conversation.
    pub fn seen(&mut self, ts_ms: Option<i64>) {
        if let Some(t) = ts_ms {
            self.first_ms.get_or_insert(t);
            self.last_ms = Some(t);
        }
    }

    pub fn read_file(&mut self, path: &str) {
        let order = self.read.len();
        self.read.entry(path.to_string()).or_insert((0, order)).0 += 1;
    }
}

/// A place that holds agent logs, and how to read them.
pub trait LogSource: Send + Sync {
    /// The source's name, such as `claude` or `codex`.
    fn name(&self) -> &'static str;
    /// The agents the source knows. It may report other agents that its logs name.
    fn agents(&self) -> Vec<AgentId>;
    fn detail(&self) -> Detail;
    /// Every log the source holds: main sessions and subagents.
    fn discover(&self) -> Vec<SessionRef>;
    /// The log at a path, when it is there and is a log of this source.
    fn session_at(&self, path: &Path) -> Option<SessionRef>;
    /// Whether the source reads the file at a path given as a target, wherever it lies.
    fn claim(&self, path: &Path) -> Claim;
    /// The directories to watch for changes; some may not exist.
    fn watched_paths(&self) -> Vec<PathBuf>;
    /// What a change at a path in a watched directory means. `arrived` says the path was
    /// created or moved in.
    fn touched(&self, changed: &Path, arrived: bool) -> Touched;
    fn head(&self, s: &SessionRef) -> Head;
    /// When the session last did something, in epoch ms: the time of the last record in its log
    /// that carries one. Agents also write records that are not work, such as bookkeeping when a
    /// session is resumed, and those carry no time. Read from the end of the log; `None` means
    /// the log's last write tells it as well, or the source cannot tell.
    fn last_active(&self, _s: &SessionRef) -> Option<i64> {
        None
    }
    /// The logs of the subagents a main session started, and of the ones they started in turn.
    fn subagents(&self, s: &SessionRef) -> Vec<SessionRef>;
    fn open(&self, s: &SessionRef) -> Result<Box<dyn Records>>;
    /// A decoder for the log, with what it needs from outside the log already read.
    fn decoder(&self, s: &SessionRef) -> Box<dyn Decoder>;
    fn activity(&self, s: &SessionRef) -> Result<Activity>;
}
