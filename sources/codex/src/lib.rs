//! Codex rollouts: `$CODEX_HOME/sessions/YYYY/MM/DD/rollout-*.jsonl` and
//! `$CODEX_HOME/archived_sessions/`, by default under `~/.codex`, one thread per file and
//! `.jsonl.zst` when Codex compressed it. A subagent thread's rollout names its parent in
//! `session_meta.parent_thread_id`, and belongs to the session of the topmost thread above it.

mod activity;
mod decoder;
mod tools;

use std::collections::{HashMap, HashSet, VecDeque};
use std::fs::Metadata;
use std::io::BufRead;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock, RwLock};

use anyhow::Result;
use serde_json::Value;
use tokprof_core::labels::display_path;
use tokprof_core::source::{
    Activity, AgentId, AgentInfo, Capabilities, Claim, Decoder, Detail, Harness, Head, Locator, LogSource, OwnCost, Pickup, PublicNames, ReadOn, Records,
    SessionRef, Touched,
};
use tokprof_logio::{file_times, str_at, walk};

/// The names that ship with Codex, which a share file keeps (see README.md): the tools that the
/// 0.164.0 and 0.166.0-alpha binaries give the model. A thread a parent spawned without a role is
/// `default` (see the decoder).
const PUBLIC_NAMES: PublicNames = PublicNames { tools: CODEX_TOOLS, commands: &[], skills: &[], subagent_types: &["default"], mcp: Some(("mcp__", "__")) };

/// Top-level tools, the code-mode tool `exec`, the `collaboration` and `clock` namespaces with their
/// tools as `exec` code names them (`clock__curr_time`), the `functions` prefix of the Responses
/// API, the terminal UI's dynamic tools, and the hosted tools.
const CODEX_TOOLS: &[&str] = &[
    "exec_command",
    "write_stdin",
    "apply_patch",
    "update_plan",
    "view_image",
    "request_user_input",
    "request_user_input_async",
    "request_permissions",
    "request_plugin_install",
    "list_available_plugins_to_install",
    "tool_search",
    "list_mcp_resources",
    "list_mcp_resource_templates",
    "read_mcp_resource",
    "spawn_agent",
    "send_input",
    "wait",
    "close_agent",
    "resume_agent",
    "get_context_remaining",
    "new_context",
    "send_message_to_user_async",
    "create_goal",
    "update_goal",
    "get_goal",
    "multi_env_copy_file",
    "wait_for_environment",
    "wait_for_git_enrichment",
    "test_sync_tool",
    "exec",
    "collaboration",
    "send_message",
    "followup_task",
    "interrupt_agent",
    "list_agents",
    "wait_agent",
    "clock",
    "curr_time",
    "sleep",
    "clock__curr_time",
    "clock__sleep",
    "collaboration__spawn_agent",
    "collaboration__send_message",
    "collaboration__followup_task",
    "collaboration__interrupt_agent",
    "collaboration__list_agents",
    "collaboration__wait_agent",
    "functions",
    "create_worktree",
    "get_worktree_creation_status",
    "list_worktrees",
    "web_search",
    "image_generation",
    "local_shell",
];

use decoder::Parser;

pub struct Codex {
    /// `$CODEX_HOME`.
    base: PathBuf,
    /// The home directory, which paths are shown relative to.
    home: PathBuf,
    agent: AgentId,
    /// The thread of every rollout, read once from their first lines on first use.
    threads: OnceLock<Arc<RwLock<Threads>>>,
    /// The names of `session_index.jsonl`, with the file's size when it was read.
    names: Arc<NamesRead>,
}

/// The names of `session_index.jsonl` as last read, with the file's size then.
type NamesRead = RwLock<(u64, Arc<HashMap<String, String>>)>;

impl Codex {
    /// The rollouts under `$CODEX_HOME`, by default `~/.codex`.
    pub fn from_env(home: &Path) -> Codex {
        let base = std::env::var_os("CODEX_HOME").map(PathBuf::from).unwrap_or_else(|| home.join(".codex"));
        Codex::at(base, home)
    }

    /// The rollouts under `base`, laid out as `$CODEX_HOME` is.
    pub fn at(base: PathBuf, home: &Path) -> Codex {
        Codex { base, home: home.to_path_buf(), agent: Codex::agent(), threads: OnceLock::new(), names: Arc::new(RwLock::new((u64::MAX, Arc::default()))) }
    }

    pub fn agent() -> AgentId {
        AgentId::new(AgentInfo {
            name: "codex".to_string(),
            title: "Codex".to_string(),
            // A Codex id is a UUIDv7: its first 13 characters are the millisecond it was made,
            // which threads launched together share, and the third group adds 12 random bits.
            short_id_len: 18,
            instruction_files: vec!["AGENTS.md".to_string()],
            ttl_settings: None,
            setup_hints: vec![("instructions".to_string(), "trim AGENTS.md".to_string())],
            cost_counter: None,
            // Measured by the reference bench (docs/reference-bench.md).
            harness: Harness { pickup: Some(Pickup { taken_tokens: 30_000.0, requests: 6.0, output_tokens: 4_000.0 }) },
            capabilities: Capabilities {
                own_cost: OwnCost::None,
                compaction_trigger: false,
                context_window: true,
                prompt_composition: true,
                titles: true,
                subagent_tasks: true,
                origins: true,
            },
            public_names: PUBLIC_NAMES,
        })
    }

    fn roots(&self) -> Vec<PathBuf> {
        vec![self.base.join("sessions"), self.base.join("archived_sessions")]
    }

    fn threads(&self) -> &Arc<RwLock<Threads>> {
        self.threads.get_or_init(|| {
            let mut paths = Vec::new();
            for root in self.roots() {
                walk(&root, &mut |path, _| {
                    if is_rollout(path) {
                        paths.push(path.to_path_buf());
                    }
                });
            }
            let mut t = Threads::default();
            for (path, (thread, settled)) in read_threads(paths) {
                if settled {
                    t.add(path, thread);
                }
            }
            Arc::new(RwLock::new(t))
        })
    }

    fn thread(&self, path: &Path) -> Option<Thread> {
        let known = self.threads().read().expect("thread index").by_path.get(path).cloned();
        if let Some(thread) = known {
            return thread;
        }
        let (thread, settled) = read_thread(path);
        if settled {
            self.threads().write().expect("thread index").add(path.to_path_buf(), thread.clone());
        }
        thread
    }

    /// A subagent thread: its rollout names a parent.
    fn is_child(&self, path: &Path) -> bool {
        self.thread(path).is_some_and(|(_, parent)| parent.is_some())
    }

    /// Rollouts of every subagent thread below a thread: those it spawned, and theirs.
    fn descendants(&self, path: &Path) -> Vec<PathBuf> {
        let Some((id, _)) = self.thread(path) else { return Vec::new() };
        self.threads().read().expect("thread index").descendants(&id)
    }

    /// The rollout of the session a rollout belongs to: its own, or for a subagent thread the
    /// rollout of the topmost thread above it; `None` when that rollout is not known.
    fn session_of(&self, path: &Path) -> Option<PathBuf> {
        match self.thread(path) {
            Some((_, Some(parent))) => {
                let threads = self.threads().read().expect("thread index");
                threads.by_id.get(&threads.root(&parent)).cloned()
            }
            _ => Some(path.to_path_buf()),
        }
    }

    /// The names Codex shows for threads, from `session_index.jsonl`, as a decoder asks for them.
    fn thread_names(&self) -> decoder::Names {
        let (path, names) = (self.base.join("session_index.jsonl"), self.names.clone());
        Arc::new(move || read_names(&path, &names))
    }

    fn reference(&self, path: &Path, meta: &Metadata) -> SessionRef {
        let session_id = id_from_name(path);
        let main_id = self.session_of(path).map_or_else(|| session_id.clone(), |main| id_from_name(&main));
        let (modified_ns, created_ms) = file_times(meta);
        SessionRef {
            source: "codex",
            agent: self.agent.clone(),
            locator: Locator::File(path.to_path_buf()),
            subagent: self.is_child(path),
            session_id,
            main_id,
            size: meta.len(),
            modified_ns,
            created_ms,
        }
    }

    fn rollouts_in(&self, dir: &Path) -> Vec<PathBuf> {
        let mut out = Vec::new();
        walk(dir, &mut |path, _| {
            if is_rollout(path) {
                out.push(path.to_path_buf());
            }
        });
        out
    }
}

impl LogSource for Codex {
    fn name(&self) -> &'static str {
        "codex"
    }

    fn agents(&self) -> Vec<AgentId> {
        vec![self.agent.clone()]
    }

    fn detail(&self) -> Detail {
        Detail::Requests
    }

    fn discover(&self) -> Vec<SessionRef> {
        let mut out = Vec::new();
        for root in self.roots() {
            walk(&root, &mut |path, meta| {
                if is_rollout(path) {
                    out.push(self.reference(path, meta));
                }
            });
        }
        out
    }

    fn session_at(&self, path: &Path) -> Option<SessionRef> {
        let meta = std::fs::metadata(path).ok().filter(|m| m.is_file())?;
        Some(self.reference(path, &meta))
    }

    fn claim(&self, path: &Path) -> Claim {
        if is_rollout(path) { Claim::Yes } else { Claim::No }
    }

    fn watched_paths(&self) -> Vec<PathBuf> {
        self.roots()
    }

    fn touched(&self, changed: &Path, arrived: bool) -> Touched {
        if is_rollout(changed) {
            Touched::Log(self.session_of(changed))
        } else if arrived && changed.is_dir() {
            Touched::Logs(self.rollouts_in(changed).iter().filter_map(|log| self.session_of(log)).collect())
        } else {
            Touched::Other
        }
    }

    fn head(&self, s: &SessionRef) -> Head {
        let (mut project, mut id, mut client) = (String::new(), String::new(), None);
        if let Ok(reader) = tokprof_logio::open(s.path()) {
            for line in reader.lines().take(200).map_while(Result::ok) {
                if !line.contains("\"cwd\"") && !line.contains("essionId") && !line.contains("session_meta") {
                    continue;
                }
                let Ok(v) = serde_json::from_str::<Value>(&line) else { continue };
                let payload = v.get("payload").unwrap_or(&v);
                if project.is_empty()
                    && let Some(cwd) = payload.get("cwd").and_then(Value::as_str)
                {
                    project = display_path(cwd, None, &self.home);
                }
                if id.is_empty()
                    && let Some(i) = v.get("sessionId").or_else(|| payload.get("id")).and_then(Value::as_str)
                {
                    id = i.to_string();
                }
                if client.is_none() && v.get("type").and_then(Value::as_str) == Some("session_meta") {
                    client = payload.get("originator").and_then(Value::as_str).map(str::to_string);
                }
                if !project.is_empty() && !id.is_empty() {
                    break;
                }
            }
        }
        if id.is_empty() {
            id = s.session_id.clone();
        }
        Head { project, session_id: id, client }
    }

    fn subagents(&self, s: &SessionRef) -> Vec<SessionRef> {
        self.descendants(s.path()).iter().filter_map(|path| self.session_at(path)).collect()
    }

    fn open(&self, s: &SessionRef) -> Result<Box<dyn Records>> {
        Ok(Box::new(Lines(s.path().to_path_buf())))
    }

    fn decoder(&self, s: &SessionRef) -> Box<dyn Decoder> {
        let threads = self.threads().clone();
        let root_of = Arc::new(move |parent: &str| threads.read().expect("thread index").root(parent));
        Box::new(Parser::new(self.agent.clone(), id_from_name(s.path()), self.home.clone(), root_of, self.thread_names()))
    }

    fn activity(&self, s: &SessionRef) -> Result<Activity> {
        activity::read(s.path())
    }
}

/// The names of `session_index.jsonl`, kept in `names` with the file's size; the file is read
/// again when it changes size.
fn read_names(path: &Path, names: &NamesRead) -> Arc<HashMap<String, String>> {
    let len = std::fs::metadata(path).map_or(0, |m| m.len());
    if names.read().expect("thread names").0 != len {
        let mut map = HashMap::new();
        for line in std::fs::read_to_string(path).unwrap_or_default().lines() {
            let Ok(v) = serde_json::from_str::<Value>(line) else { continue };
            if let (Some(id), Some(name)) = (str_at(&v, "id"), str_at(&v, "thread_name").map(str::trim).filter(|n| !n.is_empty())) {
                map.insert(id.to_string(), name.to_string());
            }
        }
        *names.write().expect("thread names") = (len, Arc::new(map));
    }
    names.read().expect("thread names").1.clone()
}

/// The lines of a rollout.
struct Lines(PathBuf);

impl Records for Lines {
    fn for_each(&mut self, f: &mut dyn FnMut(&str)) -> Result<()> {
        tokprof_logio::for_each_line(&self.0, f)
    }

    /// A compressed rollout is read whole.
    fn for_each_from(&mut self, from: u64, check: &[u8], f: &mut dyn FnMut(&str)) -> Result<Option<ReadOn>> {
        let read = tokprof_logio::for_each_line_from(&self.0, from, check, f)?;
        Ok(read.map(|r| ReadOn { end: r.end, check: r.check, open: r.open }))
    }
}

/// `rollout-*.jsonl`, or `rollout-*.jsonl.zst` for archived threads.
fn is_rollout(path: &Path) -> bool {
    path.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.starts_with("rollout-") && (n.ends_with(".jsonl") || n.ends_with(".jsonl.zst")))
}

/// The thread id at the end of a rollout file name, for rollouts without `session_meta`.
fn id_from_name(path: &Path) -> String {
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or_default();
    let stem = name.trim_end_matches(".zst").trim_end_matches(".jsonl");
    let tail = stem.len().saturating_sub(36);
    if stem.len() >= 36 && stem.as_bytes().get(tail.wrapping_sub(1)) == Some(&b'-') { stem[tail..].to_string() } else { stem.to_string() }
}

/// A rollout's thread id and the id of the thread that spawned it.
type Thread = (String, Option<String>);

/// The thread of every rollout. Rollouts that appear later are read when first asked about.
#[derive(Default)]
struct Threads {
    /// `None` for rollouts without a `session_meta` line.
    by_path: HashMap<PathBuf, Option<Thread>>,
    /// The rollout of each thread, the latest one found when a thread was moved.
    by_id: HashMap<String, PathBuf>,
    children: HashMap<String, Vec<PathBuf>>,
}

impl Threads {
    fn add(&mut self, path: PathBuf, thread: Option<Thread>) {
        if self.by_path.contains_key(&path) {
            return;
        }
        if let Some((id, parent)) = &thread {
            if let Some(p) = parent {
                self.children.entry(p.clone()).or_default().push(path.clone());
            }
            self.by_id.insert(id.clone(), path.clone());
        }
        self.by_path.insert(path, thread);
    }

    /// The topmost thread above `id`: follows parents while their rollouts are known, so a
    /// parent whose rollout is missing ends the walk. 64 steps stop a cycle in damaged logs.
    fn root(&self, id: &str) -> String {
        let mut id = id.to_string();
        for _ in 0..64 {
            let parent = self.by_id.get(&id).and_then(|path| self.by_path.get(path)).and_then(|t| t.as_ref()?.1.clone());
            match parent {
                Some(parent) => id = parent,
                None => break,
            }
        }
        id
    }

    /// Rollouts of every thread below `id`, breadth first: the threads it spawned, then theirs.
    fn descendants(&self, id: &str) -> Vec<PathBuf> {
        let (mut out, mut seen) = (Vec::new(), HashSet::from([id.to_string()]));
        let mut queue = VecDeque::from([id.to_string()]);
        while let Some(parent) = queue.pop_front() {
            for path in self.children.get(&parent).into_iter().flatten() {
                let Some(Some((child, _))) = self.by_path.get(path) else { continue };
                if seen.insert(child.clone()) {
                    queue.push_back(child.clone());
                }
                if !out.contains(path) {
                    out.push(path.clone());
                }
            }
        }
        out
    }
}

/// The first lines of the rollouts, read on plain threads and in order. Not on rayon: this runs
/// inside the index's `OnceLock`, which rayon workers parsing rollouts wait on, and a rayon worker
/// that waits for its own jobs could pick up such a parse and wait on itself.
fn read_threads(paths: Vec<PathBuf>) -> Vec<(PathBuf, (Option<Thread>, bool))> {
    let workers = std::thread::available_parallelism().map_or(4, |n| n.get());
    let chunk = paths.len().div_ceil(workers).max(1);
    std::thread::scope(|s| {
        let handles: Vec<_> =
            paths.chunks(chunk).map(|part| s.spawn(move || part.iter().map(|path| (path.clone(), read_thread(path))).collect::<Vec<_>>())).collect();
        handles.into_iter().flat_map(|h| h.join().expect("thread index reader")).collect()
    })
}

/// The thread a rollout's `session_meta` line names, and whether the answer holds for good: it
/// does not while the first line is still being written.
fn read_thread(path: &Path) -> (Option<Thread>, bool) {
    let mut line = String::new();
    if tokprof_logio::open(path).and_then(|mut r| Ok(r.read_line(&mut line)?)).is_err() {
        return (None, false);
    }
    let meta: Value = serde_json::from_str(&line).unwrap_or_default();
    let payload = meta.get("payload").unwrap_or(&Value::Null);
    let thread = str_at(payload, "id").map(|id| (id.to_string(), str_at(payload, "parent_thread_id").map(str::to_string)));
    let settled = thread.is_some() || line.ends_with('\n');
    (thread, settled)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_names_that_ship_with_codex_are_listed() {
        let n = &PUBLIC_NAMES;
        for tool in ["exec_command", "apply_patch", "view_image", "exec", "clock__curr_time", "write_stdin"] {
            assert!(n.tools.contains(&tool), "{tool}");
        }
        for tool in ["js", "search_file", "read_file", "git_status", "shell"] {
            assert!(!n.tools.contains(&tool), "{tool} is not a Codex tool");
        }
        assert_eq!(n.mcp_server("mcp__docs__read_file"), Some("docs"));
        assert!(n.origin_name("agent", "default") && !n.origin_name("agent", "walrus"));
    }

    #[test]
    fn the_head_names_the_originator() {
        let path = std::env::temp_dir().join(format!("tokprof-codex-head-{}.jsonl", std::process::id()));
        let meta = r#"{"type":"session_meta","payload":{"id":"t1","cwd":"/h/src/app","originator":"codex_exec"}}"#;
        std::fs::write(&path, meta).unwrap();
        let source = Codex::at(PathBuf::from("/h/.codex"), Path::new("/h"));
        let file = SessionRef {
            source: "codex",
            agent: Codex::agent(),
            locator: Locator::File(path.clone()),
            session_id: "t1".into(),
            main_id: "t1".into(),
            subagent: false,
            size: 0,
            modified_ns: 0,
            created_ms: None,
        };
        let head = source.head(&file);
        std::fs::remove_file(&path).ok();
        assert_eq!((head.project.as_str(), head.session_id.as_str(), head.client.as_deref()), ("~/src/app", "t1", Some("codex_exec")));
    }

    #[test]
    fn thread_ids_come_from_file_names() {
        let p = Path::new("/x/rollout-2026-08-27T22-32-52-01a044ec-eb9e-7923-829a-0123456789ab.jsonl.zst");
        assert_eq!(id_from_name(p), "01a044ec-eb9e-7923-829a-0123456789ab");
    }

    #[test]
    fn nested_threads_belong_to_the_topmost_thread() {
        let mut t = Threads::default();
        let mut add = |id: &str, parent: Option<&str>| {
            t.add(PathBuf::from(format!("/r/rollout-{id}.jsonl")), Some((id.to_string(), parent.map(str::to_string))));
        };
        add("m", None);
        add("a", Some("m"));
        add("b", Some("a"));
        add("c", Some("b"));
        add("x", Some("gone"));
        add("y", Some("z"));
        add("z", Some("y"));
        assert_eq!((t.root("c"), t.root("a"), t.root("m")), ("m".to_string(), "m".to_string(), "m".to_string()));
        // A parent whose rollout is missing is as far as the walk gets.
        assert_eq!((t.root("x"), t.root("gone")), ("gone".to_string(), "gone".to_string()));
        // A cycle ends after the cap on one of its threads.
        assert!(["y", "z"].contains(&t.root("y").as_str()));
        let names = |paths: Vec<PathBuf>| paths.iter().map(|p| p.file_name().unwrap().to_string_lossy().into_owned()).collect::<Vec<_>>();
        assert_eq!(names(t.descendants("m")), ["rollout-a.jsonl", "rollout-b.jsonl", "rollout-c.jsonl"]);
        assert_eq!(names(t.descendants("b")), ["rollout-c.jsonl"]);
        assert_eq!(names(t.descendants("y")), ["rollout-z.jsonl", "rollout-y.jsonl"]);
    }

    #[test]
    fn two_sources_over_two_directories_keep_their_own_threads() {
        let base = std::env::temp_dir().join(format!("tokprof-codex-test-{}", std::process::id()));
        let rollout = |dir: &str, id: &str, parent: Option<&str>| {
            let day = base.join(dir).join("sessions/2026/11/02");
            std::fs::create_dir_all(&day).unwrap();
            let meta = serde_json::json!({"type": "session_meta", "payload": {"id": id, "parent_thread_id": parent, "cwd": "/w"}});
            let path = day.join(format!("rollout-2026-11-02T10-00-00-{id}.jsonl"));
            std::fs::write(&path, format!("{meta}\n")).unwrap();
            path
        };
        let main_a = rollout("a", "01a19a00-0000-7000-8000-00000000000a", None);
        rollout("a", "01a19a00-0000-7000-8000-0000000000a1", Some("01a19a00-0000-7000-8000-00000000000a"));
        let main_b = rollout("b", "01a19a00-0000-7000-8000-00000000000b", None);
        let (a, b) = (Codex::at(base.join("a"), &base), Codex::at(base.join("b"), &base));
        let names = |source: &Codex, main: &Path| {
            let main = source.session_at(main).unwrap();
            source.subagents(&main).iter().map(|s| s.session_id.clone()).collect::<Vec<_>>()
        };
        let (from_a, from_b) = (names(&a, &main_a), names(&b, &main_b));
        let found_a: Vec<String> = a.discover().iter().map(|s| s.session_id.clone()).collect();
        std::fs::remove_dir_all(&base).ok();
        assert_eq!(from_a, ["01a19a00-0000-7000-8000-0000000000a1"]);
        assert!(from_b.is_empty(), "{from_b:?}");
        assert_eq!(found_a.len(), 2);
    }

    #[test]
    fn the_setup_hint_names_agents_md() {
        let agent = Codex::agent();
        assert_eq!(agent.info().setup_hints, [("instructions".to_string(), "trim AGENTS.md".to_string())]);
    }
}
