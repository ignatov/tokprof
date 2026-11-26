//! The session logs of the GitHub Copilot CLI: `<session id>/events.jsonl` in
//! `$COPILOT_HOME/session-state`, by default `~/.copilot/session-state`. Every API request has a
//! usage record with its tokens and the credits GitHub billed for it; the `workspace.yaml` beside
//! a log names the session. The saved logs of the cloud coding agent are another source,
//! `sources/copilot-cloud`.

mod decoder;

use std::fs::Metadata;
use std::io::BufRead;
use std::ops::ControlFlow;
use std::path::{Path, PathBuf};

use anyhow::Result;
use serde_json::Value;
use tokprof_core::labels::display_path;
use tokprof_core::source::{
    Activity, AgentId, AgentInfo, Capabilities, Claim, Decoder, Detail, Harness, Head, Locator, LogSource, OwnCost, PublicNames, ReadOn, Records, SessionRef,
    Touched,
};
use tokprof_logio::{file_times, parse_ts_ms, str_at};

use decoder::Parser;
pub use decoder::{USD_PER_CREDIT, model_name};

/// The tools that ship with the CLI (seen in Copilot CLI 1.0.85–1.0.116).
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
    "task",
    "read_agent",
    "list_agents",
    "skill",
    "ask_user",
    "vote_memory",
    "task_complete",
];

const LOG: &str = "events.jsonl";
const WORKSPACE: &str = "workspace.yaml";

pub struct Copilot {
    /// The folder of the session folders.
    root: PathBuf,
    home: PathBuf,
    agent: AgentId,
}

impl Copilot {
    /// The logs under `$COPILOT_HOME/session-state`, by default `~/.copilot/session-state`.
    pub fn from_env(home: &Path) -> Copilot {
        let dir = std::env::var_os("COPILOT_HOME").filter(|v| !v.is_empty()).map_or_else(|| home.join(".copilot"), PathBuf::from);
        Copilot::at(dir.join("session-state"), home.to_path_buf())
    }

    /// The logs of the session folders in `root`, with `home` as the home directory.
    pub fn at(root: PathBuf, home: PathBuf) -> Copilot {
        Copilot { root, home, agent: Copilot::agent() }
    }

    pub fn agent() -> AgentId {
        AgentId::new(AgentInfo {
            name: "copilot".to_string(),
            title: "GitHub Copilot CLI".to_string(),
            // A session id is a random UUID.
            short_id_len: 8,
            instruction_files: Vec::new(),
            ttl_settings: None,
            setup_hints: Vec::new(),
            cost_counter: Some("GitHub's billed credits".to_string()),
            harness: Harness::default(),
            capabilities: Capabilities {
                // The credits are per request; a session's own cost is their sum.
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
        let session_id = first_line(path).id.unwrap_or_else(|| folder_name(path));
        let (modified_ns, created_ms) = file_times(meta);
        SessionRef {
            source: "copilot",
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

    /// The logs of the session folders in `dir`.
    fn logs_in(&self, dir: &Path) -> Vec<PathBuf> {
        let Ok(entries) = std::fs::read_dir(dir) else { return Vec::new() };
        let mut out: Vec<PathBuf> = entries.flatten().map(|e| e.path().join(LOG)).filter(|p| p.is_file()).collect();
        out.sort();
        out
    }
}

impl LogSource for Copilot {
    fn name(&self) -> &'static str {
        "copilot"
    }

    fn agents(&self) -> Vec<AgentId> {
        vec![self.agent.clone()]
    }

    fn detail(&self) -> Detail {
        Detail::Requests
    }

    fn discover(&self) -> Vec<SessionRef> {
        self.logs_in(&self.root).iter().filter_map(|p| self.session_at(p)).collect()
    }

    fn session_at(&self, path: &Path) -> Option<SessionRef> {
        let meta = std::fs::metadata(path).ok().filter(|m| m.is_file() && is_log(path))?;
        Some(self.reference(path, &meta))
    }

    /// A log of a session folder, or one elsewhere whose first line starts a CLI session.
    fn claim(&self, path: &Path) -> Claim {
        let ours = path.parent().and_then(Path::parent) == Some(self.root.as_path());
        if is_log(path) && (ours || first_line(path).starts) { Claim::Yes } else { Claim::No }
    }

    fn watched_paths(&self) -> Vec<PathBuf> {
        vec![self.root.clone()]
    }

    fn touched(&self, changed: &Path, arrived: bool) -> Touched {
        if is_log(changed) {
            Touched::Log(Some(changed.to_path_buf()))
        } else if file_name(changed) == WORKSPACE {
            // The file that names the session.
            let log = changed.with_file_name(LOG);
            if log.is_file() { Touched::Log(Some(log)) } else { Touched::Other }
        } else if arrived && changed.is_dir() {
            let log = changed.join(LOG);
            Touched::Logs(if log.is_file() { vec![log] } else { self.logs_in(changed) })
        } else {
            Touched::Other
        }
    }

    fn head(&self, s: &SessionRef) -> Head {
        let project = first_line(s.path()).cwd.map(|cwd| display_path(&cwd, None, &self.home)).unwrap_or_default();
        Head { project, session_id: s.session_id.clone(), client: None, ..Head::default() }
    }

    fn first_active(&self, s: &SessionRef) -> Option<i64> {
        first_line(s.path()).ms
    }

    /// The time of the log's last line that carries one.
    fn last_active(&self, s: &SessionRef) -> Option<i64> {
        let mut last = None;
        tokprof_logio::for_each_line_back(s.path(), |line| {
            last = timestamp(line);
            if last.is_some() { ControlFlow::Break(()) } else { ControlFlow::Continue(()) }
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
        Box::new(Parser::new(self.agent.clone(), s.session_id.clone(), self.home.clone(), title(s.path())))
    }

    fn activity(&self, s: &SessionRef) -> Result<Activity> {
        let mut a = Activity::default();
        tokprof_logio::for_each_line(s.path(), |line| {
            if let Ok(v) = serde_json::from_str::<Value>(line) {
                activity_line(&mut a, &v);
            }
        })?;
        Ok(a)
    }
}

/// What a line of the main conversation tells about where the work stopped.
fn activity_line(a: &mut Activity, v: &Value) {
    let (Some(kind), Some(d)) = (str_at(v, "type"), v.get("data")) else { return };
    if v.get("agentId").is_some() {
        return;
    }
    let ts = parse_ts_ms(v.get("timestamp"));
    let text = |k: &str| str_at(d, k).map(str::trim).filter(|s| !s.is_empty());
    match kind {
        "session.start" | "session.context_changed" => {
            let context = if kind == "session.start" { d.get("context") } else { Some(d) };
            if let Some(cwd) = context.and_then(|c| str_at(c, "cwd")) {
                a.cwd = Some(cwd.to_string());
            }
            if let Some(branch) = context.and_then(|c| str_at(c, "branch")) {
                a.branch = Some(branch.to_string());
            }
        }
        "user.message" => {
            a.seen(ts);
            if let (Some(content), None) = (text("content"), str_at(d, "source")) {
                a.prompts.push((ts.unwrap_or(0), content.to_string()));
            }
        }
        "assistant.message" => {
            a.seen(ts);
            if let Some(content) = text("content") {
                a.last_text = Some(content.to_string());
            }
        }
        "tool.execution_start" => {
            a.seen(ts);
            let arg = |k: &str| d.get("arguments").and_then(|args| str_at(args, k)).filter(|s| !s.trim().is_empty());
            match str_at(d, "toolName") {
                Some("bash") => a.commands.extend(arg("command").map(str::to_string)),
                Some("view") => {
                    if let Some(path) = arg("path") {
                        a.read_file(path);
                    }
                }
                Some("edit" | "create") => {
                    if let Some(path) = arg("path").filter(|p| !a.changed.iter().any(|c| c == p)) {
                        a.changed.push(path.to_string());
                    }
                }
                _ => {}
            }
        }
        _ => {}
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
    file_name(path) == LOG
}

/// The name of the session folder a log lies in.
fn folder_name(path: &Path) -> String {
    path.parent().map(file_name).unwrap_or_default().to_string()
}

/// What the first line of a log tells.
#[derive(Default)]
struct FirstLine {
    /// The line starts a session of the CLI.
    starts: bool,
    id: Option<String>,
    cwd: Option<String>,
    ms: Option<i64>,
}

fn first_line(path: &Path) -> FirstLine {
    let mut line = String::new();
    let read = tokprof_logio::open(path).is_ok_and(|mut r| r.read_line(&mut line).is_ok());
    let Some(v) = read.then(|| serde_json::from_str::<Value>(&line).ok()).flatten() else { return FirstLine::default() };
    let d = v.get("data");
    let text = |v: Option<&Value>, k: &str| v.and_then(|v| str_at(v, k)).filter(|s| !s.is_empty()).map(str::to_string);
    if str_at(&v, "type") != Some("session.start") {
        return FirstLine { ms: parse_ts_ms(v.get("timestamp")), ..FirstLine::default() };
    }
    FirstLine {
        starts: d.and_then(|d| str_at(d, "producer")) == Some("copilot-agent"),
        id: text(d, "sessionId"),
        cwd: text(d.and_then(|d| d.get("context")), "cwd"),
        ms: parse_ts_ms(v.get("timestamp")),
    }
}

/// The `timestamp` of a line.
fn timestamp(line: &str) -> Option<i64> {
    let v: Value = serde_json::from_str(line).ok()?;
    parse_ts_ms(v.get("timestamp"))
}

/// The session's name: the `name` of the `workspace.yaml` beside its log, a file of plain
/// `key: value` lines.
fn title(log: &Path) -> Option<String> {
    let text = std::fs::read_to_string(log.with_file_name(WORKSPACE)).ok()?;
    let value = text.lines().find_map(|l| l.strip_prefix("name:"))?.trim();
    let value = ['"', '\''].iter().find_map(|&q| value.strip_prefix(q)?.strip_suffix(q)).unwrap_or(value);
    (!value.is_empty()).then(|| value.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixtures() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("fixtures").join("session-state")
    }

    #[test]
    fn a_session_is_a_folder_with_a_log() {
        let c = Copilot::at(fixtures(), PathBuf::from("/Users/me"));
        let ids: Vec<String> = c.discover().into_iter().map(|s| s.session_id).collect();
        assert_eq!(ids, ["c0de0001-0000-4000-8000-000000000001", "c0de0002-0000-4000-8000-000000000002"], "the folder without a log adds none");
        assert!(fixtures().join("c0de0003-0000-4000-8000-000000000003").join(WORKSPACE).is_file());
    }

    #[test]
    fn the_folder_is_the_one_given() {
        let c = Copilot::at(PathBuf::from("/data/copilot/session-state"), PathBuf::from("/h"));
        assert_eq!(c.watched_paths(), [PathBuf::from("/data/copilot/session-state")]);
        assert_eq!(c.claim(Path::new("/data/copilot/session-state/abc/events.jsonl")), Claim::Yes);
        assert_eq!(c.claim(Path::new("/data/copilot/session-state/abc/workspace.yaml")), Claim::No);
        assert_eq!(c.claim(Path::new("/elsewhere/abc/events.jsonl")), Claim::No, "no file, so no first line");
        assert!(!c.on_request(), "read by default");
    }

    #[test]
    fn a_log_elsewhere_is_claimed_by_its_first_line() {
        let c = Copilot::at(PathBuf::from("/nowhere/session-state"), PathBuf::from("/h"));
        let log = fixtures().join("c0de0001-0000-4000-8000-000000000001").join(LOG);
        assert_eq!(c.claim(&log), Claim::Yes);
        let s = c.session_at(&log).unwrap();
        assert_eq!(s.session_id, "c0de0001-0000-4000-8000-000000000001");
        assert_eq!(c.head(&s).project, "/Users/me/src/app", "another home");
    }

    #[test]
    fn the_title_is_the_name_of_the_workspace_file() {
        let log = |id: &str| fixtures().join(id).join(LOG);
        assert_eq!(title(&log("c0de0001-0000-4000-8000-000000000001")).as_deref(), Some("Fix the parser"));
        assert_eq!(title(&log("c0de0002-0000-4000-8000-000000000002")), None, "no workspace file");
    }
}
