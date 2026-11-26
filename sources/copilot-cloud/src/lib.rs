//! The saved logs of the GitHub Copilot coding agent, the cloud agent that works on a repository
//! and opens pull requests. It writes no local logs: the user saves each session's log, the
//! answer of the agent sessions API, as `<session id>.sse` or `<session id>.sse.gz`, and its
//! record of the agent tasks API as `<session id>.json`, anywhere under one folder:
//! `$TOKPROF_COPILOT_DIR`, else `copilot` in tokprof's own folder (`$TOKPROF_DIR`, else
//! `~/tokprof`). The logs give no usage; the record gives the model and the billed credits.

mod decoder;

use std::collections::HashMap;
use std::fs::Metadata;
use std::io::BufRead;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use anyhow::Result;
use serde_json::Value;
use tokprof_core::source::{
    Activity, AgentId, AgentInfo, Capabilities, Claim, Decoder, Detail, Harness, Head, Locator, LogSource, OwnCost, PublicNames, ReadOn, Records, SessionRef,
    Touched,
};
use tokprof_logio::{file_times, walk};

use decoder::{Line, Parser, parse_line};
pub use decoder::{Record, model_name};

/// The tools that ship with the agent (seen in logs of 2026-11-14 to 2026-11-21).
const TOOLS: &[&str] = &[
    "bash",
    "read_bash",
    "stop_bash",
    "view",
    "edit",
    "create",
    "grep",
    "glob",
    "web_fetch",
    "web_search",
    "task",
    "read_agent",
    "skill",
    "report_progress",
    "codeql_checker",
    "run_setup",
    "run_custom_setup_step",
];

pub struct CopilotCloud {
    /// The folder the logs were saved in.
    root: PathBuf,
    agent: AgentId,
    /// The records under the folder by session id, as the latest walk found them.
    records: Mutex<Option<HashMap<String, PathBuf>>>,
}

impl CopilotCloud {
    /// The logs under `$TOKPROF_COPILOT_DIR`, else under `copilot` in `$TOKPROF_DIR`, else in
    /// `~/tokprof`.
    pub fn from_env(home: &Path) -> CopilotCloud {
        let var = |name: &str| std::env::var_os(name).filter(|v| !v.is_empty()).map(PathBuf::from);
        let root = var("TOKPROF_COPILOT_DIR").unwrap_or_else(|| var("TOKPROF_DIR").unwrap_or_else(|| home.join("tokprof")).join("copilot"));
        CopilotCloud::at(root)
    }

    /// The logs under a folder.
    pub fn at(root: PathBuf) -> CopilotCloud {
        CopilotCloud { root, agent: CopilotCloud::agent(), records: Mutex::new(None) }
    }

    pub fn agent() -> AgentId {
        AgentId::new(AgentInfo {
            name: "copilot-cloud".to_string(),
            title: "GitHub Copilot coding agent".to_string(),
            // A session id is a random UUID.
            short_id_len: 8,
            instruction_files: Vec::new(),
            ttl_settings: None,
            setup_hints: Vec::new(),
            cost_counter: Some("GitHub's billed credits".to_string()),
            harness: Harness::default(),
            capabilities: Capabilities {
                own_cost: OwnCost::Session,
                compaction_trigger: false,
                context_window: false,
                prompt_composition: true,
                titles: true,
                subagent_tasks: false,
                origins: false,
            },
            public_names: PublicNames { tools: TOOLS, ..PublicNames::default() },
        })
    }

    fn reference(&self, path: &Path, meta: &Metadata) -> SessionRef {
        let session_id = id_from_name(path);
        let (modified_ns, created_ms) = file_times(meta);
        SessionRef {
            source: "copilot-cloud",
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

    /// Every log under the folder, and the records found on the way.
    fn walk(&self) -> (Vec<SessionRef>, HashMap<String, PathBuf>) {
        let (mut logs, mut records) = (Vec::new(), HashMap::new());
        walk(&self.root, &mut |path, meta| {
            if is_log(path) {
                logs.push(self.reference(path, meta));
            } else if let Some(id) = record_id(path) {
                records.insert(id.to_string(), path.to_path_buf());
            }
        });
        (logs, records)
    }

    /// The record of a session: beside its log, else anywhere under the folder.
    fn record(&self, s: &SessionRef) -> Record {
        let beside = s.path().with_file_name(format!("{}.json", s.session_id));
        let path = if beside.is_file() {
            Some(beside)
        } else {
            let mut held = self.records.lock().unwrap_or_else(|e| e.into_inner());
            held.get_or_insert_with(|| self.walk().1).get(&s.session_id).cloned()
        };
        path.and_then(|p| Record::read(&p, &s.session_id)).unwrap_or_default()
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
}

impl LogSource for CopilotCloud {
    fn name(&self) -> &'static str {
        "copilot-cloud"
    }

    fn agents(&self) -> Vec<AgentId> {
        vec![self.agent.clone()]
    }

    fn on_request(&self) -> bool {
        true
    }

    fn detail(&self) -> Detail {
        Detail::Requests
    }

    fn discover(&self) -> Vec<SessionRef> {
        let (logs, records) = self.walk();
        *self.records.lock().unwrap_or_else(|e| e.into_inner()) = Some(records);
        logs
    }

    fn session_at(&self, path: &Path) -> Option<SessionRef> {
        let meta = std::fs::metadata(path).ok().filter(|m| m.is_file())?;
        Some(self.reference(path, &meta))
    }

    /// A log under the folder, or one elsewhere whose first data line is a chat chunk.
    fn claim(&self, path: &Path) -> Claim {
        if is_log(path) && (path.starts_with(&self.root) || first_chunk(path)) { Claim::Yes } else { Claim::No }
    }

    fn watched_paths(&self) -> Vec<PathBuf> {
        vec![self.root.clone()]
    }

    fn touched(&self, changed: &Path, arrived: bool) -> Touched {
        if is_log(changed) {
            Touched::Log(Some(changed.to_path_buf()))
        } else if let Some(id) = record_id(changed) {
            // A record saved after its log changes the session of the log beside it.
            let log = ["sse", "sse.gz"].iter().map(|ext| changed.with_file_name(format!("{id}.{ext}"))).find(|p| p.is_file());
            *self.records.lock().unwrap_or_else(|e| e.into_inner()) = None;
            if log.is_some() { Touched::Log(log) } else { Touched::Other }
        } else if arrived && changed.is_dir() {
            Touched::Logs(self.logs_in(changed))
        } else {
            Touched::Other
        }
    }

    fn head(&self, s: &SessionRef) -> Head {
        let project = head_lines(s.path()).project.unwrap_or_default();
        Head { project, session_id: s.session_id.clone(), client: None, ..Head::default() }
    }

    fn first_active(&self, s: &SessionRef) -> Option<i64> {
        head_lines(s.path()).first_ms
    }

    /// The time of the log's latest chunk: a saved log was written when it was saved.
    fn last_active(&self, s: &SessionRef) -> Option<i64> {
        let mut last = None;
        tokprof_logio::for_each_line(s.path(), |line| {
            if let Some(Line::Chunk(c)) = parse_line(line) {
                last = last.max(Some(c.ms));
            }
        })
        .ok()?;
        last
    }

    fn subagents(&self, _: &SessionRef) -> Vec<SessionRef> {
        Vec::new()
    }

    fn open(&self, s: &SessionRef) -> Result<Box<dyn Records>> {
        Ok(Box::new(Lines(s.path().to_path_buf())))
    }

    fn decoder(&self, s: &SessionRef) -> Box<dyn Decoder> {
        Box::new(Parser::new(self.agent.clone(), s.session_id.clone(), self.record(s)))
    }

    /// When the session worked; its prompts and tools are not read for a handoff note yet.
    fn activity(&self, s: &SessionRef) -> Result<Activity> {
        let mut a = Activity::default();
        tokprof_logio::for_each_line(s.path(), |line| {
            if let Some(Line::Chunk(c)) = parse_line(line) {
                a.seen(Some(c.ms));
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

fn file_name(path: &Path) -> &str {
    path.file_name().and_then(|n| n.to_str()).unwrap_or_default()
}

fn is_log(path: &Path) -> bool {
    let name = file_name(path);
    (name.ends_with(".sse") || name.ends_with(".sse.gz")) && !id_from_name(path).is_empty()
}

/// The session id of a log: its name without `.sse` or `.sse.gz`.
fn id_from_name(path: &Path) -> String {
    let name = file_name(path);
    name.strip_suffix(".sse.gz").or_else(|| name.strip_suffix(".sse")).unwrap_or(name).to_string()
}

/// The session id a record's name gives: `<uuid>.json`.
fn record_id(path: &Path) -> Option<&str> {
    let id = file_name(path).strip_suffix(".json")?;
    let uuid = id.len() == 36 && id.chars().enumerate().all(|(i, c)| if [8, 13, 18, 23].contains(&i) { c == '-' } else { c.is_ascii_hexdigit() });
    uuid.then_some(id)
}

/// What the first lines of a log tell, up to its first user message.
#[derive(Default)]
struct HeadLines {
    project: Option<String>,
    first_ms: Option<i64>,
}

fn head_lines(path: &Path) -> HeadLines {
    let mut h = HeadLines::default();
    let Ok(mut reader) = tokprof_logio::open(path) else { return h };
    let mut line = String::new();
    while reader.read_line(&mut line).is_ok_and(|n| n > 0) {
        match parse_line(&line) {
            Some(Line::Chunk(c)) => {
                h.first_ms.get_or_insert(c.ms);
                if h.project.is_none() {
                    h.project = c.calls.iter().find_map(|call| call.step.as_deref().and_then(decoder::cloned_repository)).map(|r| format!("github.com/{r}"));
                }
            }
            Some(Line::User { .. }) => break,
            _ => {}
        }
        line.clear();
    }
    h
}

/// Whether the first data line of a file is a chat chunk.
fn first_chunk(path: &Path) -> bool {
    let Ok(mut reader) = tokprof_logio::open(path) else { return false };
    let mut line = String::new();
    while reader.read_line(&mut line).is_ok_and(|n| n > 0) {
        if let Some(body) = line.trim_end().strip_prefix("data:") {
            return serde_json::from_str::<Value>(body.trim()).is_ok_and(|v| v.get("object").and_then(Value::as_str) == Some("chat.completion.chunk"));
        }
        line.clear();
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_id_comes_from_the_name() {
        assert_eq!(id_from_name(Path::new("/c/2026-11-15/5f0c2a9e-1b7d-4e3a-9c6f-2d8b7a4e1c03.sse.gz")), "5f0c2a9e-1b7d-4e3a-9c6f-2d8b7a4e1c03");
        assert_eq!(id_from_name(Path::new("/c/a.sse")), "a");
        assert!(!is_log(Path::new("/c/.sse")) && !is_log(Path::new("/c/a.jsonl")) && is_log(Path::new("/c/a.sse")));
        assert_eq!(record_id(Path::new("/c/5f0c2a9e-1b7d-4e3a-9c6f-2d8b7a4e1c03.json")), Some("5f0c2a9e-1b7d-4e3a-9c6f-2d8b7a4e1c03"));
        assert_eq!(record_id(Path::new("/c/tasks-list.json")), None, "no record");
    }

    #[test]
    fn the_folder_is_the_one_given() {
        let c = CopilotCloud::at(PathBuf::from("/data/copilot"));
        assert_eq!(c.watched_paths(), [PathBuf::from("/data/copilot")]);
        assert_eq!(c.claim(Path::new("/data/copilot/2026-11-15/a.sse.gz")), Claim::Yes);
        assert_eq!(c.claim(Path::new("/data/copilot/a.json")), Claim::No);
        assert!(c.on_request(), "read only when asked for");
    }
}
