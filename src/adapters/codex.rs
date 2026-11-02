//! Codex rollouts: `~/.codex/sessions/YYYY/MM/DD/rollout-*.jsonl` and
//! `~/.codex/archived_sessions/`, one thread per file.
//!
//! - Usage per request comes from `token_usage_record` lines, deduplicated by `response_id`.
//!   Older rollouts only have `token_count` events, whose `last_token_usage` counts when the
//!   cumulative total changed. `input_tokens` includes the cached and cache-written parts.
//! - The prompt is rebuilt from the rollout. `session_meta.base_instructions` is the system
//!   prompt, every `response_item` is one item of the conversation, and `compacted` replaces
//!   the conversation with its `replacement_history`. A request's prompt is the conversation
//!   before the items its own response added: the trailing reasoning, assistant messages and
//!   tool calls. Tool schemas other than `additional_tools` are not logged.
//! - `world_state` repeats what the messages already carry (AGENTS.md, environment), so it is
//!   not counted.
//! - Subagent threads name their parent in `session_meta.parent_thread_id`.

use std::collections::{HashMap, HashSet, VecDeque};
use std::io::BufRead;
use std::path::{Path, PathBuf};
use std::sync::{OnceLock, RwLock};

use anyhow::Result;
use rayon::prelude::*;
use serde_json::Value;

use super::util::{command_head, display_path, for_each_line, json_len, parse_ts_ms, str_at, tool_target, u64_at};
use super::{home, walk};
use crate::model::{Agent, Block, BlockKind, Chain, ChainKind, Compaction, Params, PromptDelta, Request, Session, Shape, Usage};

/// Size an image is counted as, in characters (roughly 1,400 tokens); low detail is 85 tokens.
const IMAGE_CHARS: u64 = 5_000;
const LOW_DETAIL_IMAGE_CHARS: u64 = 300;

fn codex_home() -> PathBuf {
    std::env::var_os("CODEX_HOME").map(PathBuf::from).unwrap_or_else(|| home().join(".codex"))
}

pub fn roots() -> Vec<PathBuf> {
    let base = codex_home();
    vec![base.join("sessions"), base.join("archived_sessions")]
}

/// The name Codex shows for a thread, from `session_index.jsonl`; the file is read again when
/// it changes size.
fn thread_name(id: &str) -> Option<String> {
    static NAMES: OnceLock<RwLock<(u64, HashMap<String, String>)>> = OnceLock::new();
    let path = codex_home().join("session_index.jsonl");
    let len = std::fs::metadata(&path).map_or(0, |m| m.len());
    let names = NAMES.get_or_init(|| RwLock::new((u64::MAX, HashMap::new())));
    if names.read().expect("thread names").0 != len {
        let mut map = HashMap::new();
        for line in std::fs::read_to_string(&path).unwrap_or_default().lines() {
            let Ok(v) = serde_json::from_str::<Value>(line) else { continue };
            if let (Some(id), Some(name)) = (str_at(&v, "id"), str_at(&v, "thread_name").map(str::trim).filter(|n| !n.is_empty())) {
                map.insert(id.to_string(), name.to_string());
            }
        }
        *names.write().expect("thread names") = (len, map);
    }
    names.read().expect("thread names").1.get(id).cloned()
}

/// A rollout's thread id and the id of the thread that spawned it.
type Thread = (String, Option<String>);

/// The thread of every rollout, read once from their first lines. Rollouts that appear later
/// are read when first asked about.
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

fn threads() -> &'static RwLock<Threads> {
    static THREADS: OnceLock<RwLock<Threads>> = OnceLock::new();
    THREADS.get_or_init(|| {
        let mut paths = Vec::new();
        for root in roots() {
            walk(&root, &mut |path, _, _| {
                if is_rollout(path) {
                    paths.push(path.to_path_buf());
                }
            });
        }
        let read: Vec<(PathBuf, (Option<Thread>, bool))> = paths
            .into_par_iter()
            .map(|path| {
                let thread = read_thread(&path);
                (path, thread)
            })
            .collect();
        let mut t = Threads::default();
        for (path, (thread, settled)) in read {
            if settled {
                t.add(path, thread);
            }
        }
        RwLock::new(t)
    })
}

fn thread(path: &Path) -> Option<Thread> {
    let known = threads().read().expect("thread index").by_path.get(path).cloned();
    if let Some(thread) = known {
        return thread;
    }
    let (thread, settled) = read_thread(path);
    if settled {
        threads().write().expect("thread index").add(path.to_path_buf(), thread.clone());
    }
    thread
}

/// The thread a rollout's `session_meta` line names, and whether the answer holds for good: it
/// does not while the first line is still being written.
fn read_thread(path: &Path) -> (Option<Thread>, bool) {
    let mut line = String::new();
    if super::util::open(path).and_then(|mut r| Ok(r.read_line(&mut line)?)).is_err() {
        return (None, false);
    }
    let meta: Value = serde_json::from_str(&line).unwrap_or_default();
    let payload = meta.get("payload").unwrap_or(&Value::Null);
    let thread = str_at(payload, "id").map(|id| (id.to_string(), str_at(payload, "parent_thread_id").map(str::to_string)));
    let settled = thread.is_some() || line.ends_with('\n');
    (thread, settled)
}

/// `rollout-*.jsonl`, or `rollout-*.jsonl.zst` for archived threads.
pub fn is_rollout(path: &Path) -> bool {
    path.file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|n| n.starts_with("rollout-") && (n.ends_with(".jsonl") || n.ends_with(".jsonl.zst")))
}

/// A subagent thread: its rollout names a parent.
pub fn is_child(path: &Path) -> bool {
    thread(path).is_some_and(|(_, parent)| parent.is_some())
}

/// Rollouts of every subagent thread below a thread: those it spawned, and theirs.
pub fn descendants(path: &Path) -> Vec<PathBuf> {
    let Some((id, _)) = thread(path) else { return Vec::new() };
    threads().read().expect("thread index").descendants(&id)
}

/// The rollout of the session a rollout belongs to: its own, or for a subagent thread the
/// rollout of the topmost thread above it; `None` when that rollout is not known.
pub fn session_of(path: &Path) -> Option<PathBuf> {
    match thread(path) {
        Some((_, Some(parent))) => {
            let threads = threads().read().expect("thread index");
            threads.by_id.get(&threads.root(&parent)).cloned()
        }
        _ => Some(path.to_path_buf()),
    }
}

/// The id of the session a subagent thread with this parent belongs to.
fn session_id(parent: &str) -> String {
    threads().read().expect("thread index").root(parent)
}

/// Per usage source: the model settings and the prompt state its requests continue from.
#[derive(Default)]
struct Stream {
    model: String,
    effort: Option<String>,
    compacted: bool,
    after_assistant: bool,
    context_window: Option<u64>,
    /// Conversation items in the previous request's prompt; `None` before the first request
    /// and after a compaction replaced the conversation.
    prompt_len: Option<usize>,
    requests: Vec<Request>,
}

struct Parser {
    session: Session,
    /// The conversation so far, as message indices.
    history: Vec<usize>,
    /// Items at the end of `history` that the response in flight produced.
    trailing_outputs: usize,
    tools_by_call: HashMap<String, (String, Option<String>)>,
    shape: Option<usize>,
    streams: [Stream; 2],
    seen: HashSet<String>,
    last_total: String,
    subagent: Option<String>,
}

/// The thread id at the end of a rollout file name, for rollouts without `session_meta`.
pub fn id_from_name(path: &Path) -> String {
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or_default();
    let stem = name.trim_end_matches(".zst").trim_end_matches(".jsonl");
    let tail = stem.len().saturating_sub(36);
    if stem.len() >= 36 && stem.as_bytes().get(tail.wrapping_sub(1)) == Some(&b'-') { stem[tail..].to_string() } else { stem.to_string() }
}

pub fn parse_file(path: &Path) -> Result<Session> {
    let mut p = Parser::new(id_from_name(path));
    for_each_line(path, |line| p.line(line))?;
    Ok(p.finish())
}

impl Parser {
    fn new(id: String) -> Parser {
        Parser {
            session: Session::new(Agent::Codex, id),
            history: Vec::new(),
            trailing_outputs: 0,
            tools_by_call: HashMap::new(),
            shape: None,
            streams: [Stream::default(), Stream::default()],
            seen: HashSet::new(),
            last_total: String::new(),
            subagent: None,
        }
    }

    fn line(&mut self, line: &str) {
        // Completed-item events repeat the response items for the UI.
        if line.contains("\"item_completed\"") {
            return;
        }
        let Ok(v) = serde_json::from_str::<Value>(line) else { return };
        let payload = v.get("payload").unwrap_or(&Value::Null);
        let ts = parse_ts_ms(v.get("timestamp")).unwrap_or(0);
        let ptype = str_at(payload, "type").unwrap_or_default();
        match str_at(&v, "type").unwrap_or_default() {
            "session_meta" => self.session_meta(payload),
            "turn_context" => {
                for s in &mut self.streams {
                    if let Some(m) = str_at(payload, "model") {
                        s.model = m.to_string();
                    }
                    s.effort = str_at(payload, "effort").map(str::to_string);
                }
                self.note_cwd(payload);
            }
            "compacted" => {
                let items: Vec<Value> = payload.get("replacement_history").and_then(Value::as_array).cloned().unwrap_or_default();
                self.history.clear();
                self.trailing_outputs = 0;
                for item in &items {
                    self.push_item(item);
                }
                self.trailing_outputs = 0;
                for s in &mut self.streams {
                    s.compacted = true;
                    s.prompt_len = None;
                }
                self.session.compactions.push(Compaction { ts_ms: ts, kind: None });
            }
            "response_item" => {
                if ptype == "message" && str_at(payload, "role") == Some("assistant") {
                    self.streams.iter_mut().for_each(|s| s.after_assistant = true);
                }
                self.push_item(payload);
            }
            "token_usage_record" => {
                let fresh = str_at(payload, "response_id").is_none_or(|id| self.seen.insert(id.to_string()));
                if let (true, Some(usage)) = (fresh, payload.get("usage")) {
                    self.request(0, usage, ts);
                }
            }
            "event_msg" if ptype == "token_count" => {
                let Some(info) = payload.get("info").filter(|i| !i.is_null()) else { return };
                if let Some(window) = info.get("model_context_window").and_then(Value::as_u64) {
                    self.streams.iter_mut().for_each(|s| s.context_window = Some(window));
                }
                let total = info.get("total_token_usage").map(Value::to_string).unwrap_or_default();
                if total != self.last_total {
                    self.last_total = total;
                    if let Some(last) = info.get("last_token_usage") {
                        self.request(1, last, ts);
                    }
                }
            }
            "event_msg" if ptype.contains("compact") => {
                self.streams.iter_mut().for_each(|s| s.compacted = true);
                self.session.compactions.push(Compaction { ts_ms: ts, kind: None });
            }
            _ => {}
        }
    }

    fn session_meta(&mut self, payload: &Value) {
        if let Some(id) = str_at(payload, "id") {
            self.session.id = id.to_string();
        }
        self.note_cwd(payload);
        if payload.get("parent_thread_id").is_some_and(|p| !p.is_null()) {
            let source = payload.pointer("/source/subagent");
            let spawn = source.and_then(|s| s.get("thread_spawn"));
            let role = match (spawn, source) {
                // A thread the parent spawned with `spawn_agent`; it has no role by default.
                (Some(spawn), _) => Some(str_at(spawn, "agent_role").unwrap_or("default").to_string()),
                (None, Some(Value::String(s))) => Some(s.clone()),
                (None, Some(Value::Object(m))) => m.values().find_map(|v| v.as_str().map(str::to_string)),
                _ => None,
            };
            self.subagent = Some(role.map_or_else(|| "subagent".to_string(), |r| format!("subagent:{r}")));
            // The task name the parent gave the thread ends its agent path: `/root/fix_tests`.
            self.session.task = spawn
                .and_then(|s| str_at(s, "agent_path"))
                .and_then(|p| p.rsplit('/').next())
                .filter(|t| !t.is_empty())
                .map(|t| t.replace('_', " "));
            // The thread's requests count under its session, however deep the thread is.
            if let Some(parent) = str_at(payload, "parent_thread_id") {
                self.session.id = session_id(parent);
                self.session.title = thread_name(&self.session.id);
            }
        }
        if self.shape.is_none() {
            let chars = payload.pointer("/base_instructions/text").and_then(Value::as_str).map_or(0, |t| t.len() as u64);
            let start = self.session.blocks.len();
            self.session.blocks.push(Block { kind: BlockKind::System, chars, tool: None, target: None });
            self.session.shapes.push(Shape { blocks: start..start + 1, fingerprint: chars, tool_names: Vec::new(), system_chars: chars });
            self.shape = Some(self.session.shapes.len() - 1);
        }
    }

    fn note_cwd(&mut self, payload: &Value) {
        if self.session.cwd.is_none()
            && let Some(cwd) = str_at(payload, "cwd")
        {
            self.session.cwd = Some(cwd.to_string());
            self.session.project = display_path(cwd, None);
        }
    }

    fn push_item(&mut self, item: &Value) {
        let (blocks, output) = self.blocks(item);
        if blocks.is_empty() {
            return;
        }
        let starts_turn = blocks.iter().any(|b| b.kind == BlockKind::Prompt);
        let idx = self.session.push_message(blocks, starts_turn);
        self.history.push(idx);
        self.trailing_outputs = if output { self.trailing_outputs + 1 } else { 0 };
    }

    /// The blocks of one conversation item, and whether the model produced it.
    fn blocks(&mut self, item: &Value) -> (Vec<Block>, bool) {
        let cwd = self.session.cwd.clone();
        let cwd = cwd.as_deref();
        let block = |kind, chars, tool: Option<String>, target: Option<String>| Block { kind, chars, tool, target };
        let ty = str_at(item, "type").unwrap_or_default();
        match ty {
            "message" => {
                let role = str_at(item, "role").unwrap_or("user");
                let mut blocks = Vec::new();
                for c in item.get("content").and_then(Value::as_array).into_iter().flatten() {
                    if str_at(c, "type") == Some("input_image") {
                        blocks.push(block(BlockKind::Image, image_chars(c), None, None));
                        continue;
                    }
                    let text = str_at(c, "text").unwrap_or_default();
                    let (kind, target) = match role {
                        "assistant" => (BlockKind::Text, None),
                        "user" => match context_label(text) {
                            Some(label) => (BlockKind::Reminder, Some(label)),
                            None => (BlockKind::Prompt, None),
                        },
                        _ => (BlockKind::Reminder, Some(context_label(text).unwrap_or_else(|| "developer instructions".to_string()))),
                    };
                    blocks.push(block(kind, text.len() as u64, None, target));
                }
                (blocks, role == "assistant")
            }
            "reasoning" => {
                let encrypted = str_at(item, "encrypted_content").map_or(0, |e| e.len() as u64 * 3 / 4);
                let summary: u64 = item
                    .get("summary")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .map(|s| str_at(s, "text").map_or(0, |t| t.len() as u64))
                    .sum();
                (vec![block(BlockKind::Thinking, encrypted + summary, None, None)], true)
            }
            "function_call" | "custom_tool_call" => {
                let name = str_at(item, "name").unwrap_or("?");
                let (tool, target, chars) = if ty == "custom_tool_call" {
                    let input = str_at(item, "input").unwrap_or_default();
                    let (tool, target) = code_target(name, input, cwd);
                    (tool, target, input.len() as u64)
                } else {
                    let args = str_at(item, "arguments").unwrap_or_default();
                    let parsed: Value = serde_json::from_str(args).unwrap_or(Value::Null);
                    (name.to_string(), call_target(name, &parsed, cwd), args.len() as u64)
                };
                if let Some(id) = str_at(item, "call_id") {
                    self.tools_by_call.insert(id.to_string(), (tool.clone(), target.clone()));
                }
                (vec![block(BlockKind::ToolUse, name.len() as u64 + chars, Some(tool), target)], true)
            }
            "function_call_output" | "custom_tool_call_output" => {
                let (tool, target) = str_at(item, "call_id")
                    .and_then(|id| self.tools_by_call.get(id).cloned())
                    .map_or((None, None), |(t, g)| (Some(t), g));
                let chars = match item.get("output") {
                    Some(Value::String(s)) => s.len() as u64,
                    Some(Value::Array(items)) => items
                        .iter()
                        .map(|c| if str_at(c, "type") == Some("input_image") { image_chars(c) } else { str_at(c, "text").map_or(0, |t| t.len() as u64) })
                        .sum(),
                    Some(other) => json_len(other),
                    None => 0,
                };
                (vec![block(BlockKind::ToolResult, chars, tool, target)], false)
            }
            "additional_tools" => {
                let tools = item.get("tools").and_then(Value::as_array).cloned().unwrap_or_default();
                let blocks = tools
                    .iter()
                    .map(|t| {
                        let name = str_at(t, "name").unwrap_or("?").to_string();
                        block(BlockKind::Tools, json_len(t), Some(name), None)
                    })
                    .collect();
                (blocks, false)
            }
            t if t.ends_with("_call") => (vec![block(BlockKind::ToolUse, json_len(item), Some(t.to_string()), None)], true),
            _ => (vec![block(BlockKind::Unknown, json_len(item), None, None)], false),
        }
    }

    fn request(&mut self, stream: usize, usage: &Value, ts_ms: i64) {
        let prompt_len = self.history.len() - self.trailing_outputs.min(self.history.len());
        let produced = (self.trailing_outputs > 0).then(|| {
            let blocks: Vec<Block> = self.history[prompt_len..]
                .iter()
                .flat_map(|&m| self.session.blocks[self.session.messages[m].blocks.clone()].to_vec())
                .collect();
            self.session.push_message(blocks, false)
        });
        let s = &mut self.streams[stream];
        let keep = s.prompt_len.filter(|&prev| prev <= prompt_len).unwrap_or(0);
        let delta = PromptDelta {
            shape: if s.requests.is_empty() { self.shape } else { None },
            keep,
            tail: self.history[keep..prompt_len].to_vec(),
        };
        let input = u64_at(usage, "input_tokens");
        let cached = u64_at(usage, "cached_input_tokens");
        let written = u64_at(usage, "cache_write_input_tokens");
        s.requests.push(Request {
            ts_ms,
            model: if s.model.is_empty() { "unknown".to_string() } else { s.model.clone() },
            usage: Usage {
                uncached: input.saturating_sub(cached + written),
                cache_read: cached,
                write_short: written,
                write_long: 0,
                output: u64_at(usage, "output_tokens"),
                reasoning: u64_at(usage, "reasoning_output_tokens"),
            },
            params: Some(Params { effort: s.effort.clone(), ..Params::default() }),
            prompt: Some(delta),
            produced,
            compacted_before: s.compacted,
            after_assistant_message: s.after_assistant,
            context_window: s.context_window,
            ..Request::default()
        });
        s.compacted = false;
        s.after_assistant = false;
        s.prompt_len = Some(prompt_len);
    }

    fn finish(mut self) -> Session {
        let [records, counts] = std::mem::take(&mut self.streams);
        let mut requests = if records.requests.is_empty() { counts.requests } else { records.requests };
        let (name, kind) = match self.subagent.take() {
            Some(name) => {
                let origin = name.replacen("subagent", "agent", 1);
                requests.iter_mut().for_each(|r| r.origin = Some(origin.clone()));
                let name = self.session.task.as_ref().map_or(name, |task| format!("subagent: {task}"));
                (name, ChainKind::Subagent)
            }
            None => {
                self.session.title = thread_name(&self.session.id);
                ("main".to_string(), ChainKind::Main)
            }
        };
        self.session.chains.push(Chain { name, kind, requests });
        self.session.compactions.sort_by_key(|c| c.ts_ms);
        self.session
    }
}

fn image_chars(item: &Value) -> u64 {
    if str_at(item, "detail") == Some("low") { LOW_DETAIL_IMAGE_CHARS } else { IMAGE_CHARS }
}

/// Codex wraps injected context in tags or headings; the label says what it is.
fn context_label(text: &str) -> Option<String> {
    let t = text.trim_start();
    if t.starts_with("# AGENTS.md") || t.starts_with("<INSTRUCTIONS>") || t.starts_with("<user_instructions>") {
        return Some("instructions".to_string());
    }
    if t.starts_with("# Files mentioned by the user") {
        return Some("attached files".to_string());
    }
    let tag: String = t.strip_prefix('<')?.chars().take_while(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-').collect();
    (!tag.is_empty()).then(|| tag.replace(['_', '-'], " "))
}

/// Target of a function call, from its JSON arguments.
fn call_target(name: &str, args: &Value, cwd: Option<&str>) -> Option<String> {
    let s = |k: &str| args.get(k).and_then(Value::as_str);
    match name {
        "exec_command" | "shell" | "container.exec" => match args.get("cmd").or(args.get("command")) {
            Some(Value::String(c)) => Some(command_head(c)),
            Some(Value::Array(parts)) => {
                let parts: Vec<&str> = parts.iter().filter_map(Value::as_str).collect();
                let cmd = match parts.as_slice() {
                    [_, flag, script] if flag.starts_with('-') => script.to_string(),
                    _ => parts.join(" "),
                };
                Some(command_head(&cmd))
            }
            _ => None,
        },
        "js" => s("title").map(str::to_string),
        "apply_patch" => s("input").and_then(|p| patched_file(p, cwd)),
        "spawn_agent" => s("agent_type").or(s("role")).or(s("name")).map(str::to_string),
        _ => tool_target(name, args, cwd),
    }
}

/// Codex's `exec` tool takes JavaScript that calls the real tools (`tools.exec_command(...)`);
/// a single inner tool becomes the tool, with its command or patched file as the target.
fn code_target(name: &str, code: &str, cwd: Option<&str>) -> (String, Option<String>) {
    let mut inner: Vec<String> = Vec::new();
    for (at, _) in code.match_indices("tools.") {
        let ident: String = code[at + 6..].chars().take_while(|c| c.is_ascii_alphanumeric() || *c == '_').collect();
        if !ident.is_empty() && !inner.contains(&ident) {
            inner.push(ident);
        }
    }
    match inner.as_slice() {
        [] => (name.to_string(), None),
        [one] => {
            let target = match one.as_str() {
                "exec_command" => js_string(code, "cmd").map(|c| command_head(&c)),
                "apply_patch" => patched_file(code, cwd),
                _ => None,
            };
            (one.clone(), target)
        }
        many => (name.to_string(), Some(many.join(", "))),
    }
}

/// The string value of `field:` in JavaScript source, with any quote style.
fn js_string(code: &str, field: &str) -> Option<String> {
    let at = code.find(&format!("{field}:")).or_else(|| code.find(&format!("\"{field}\":")))?;
    let rest = code[at..].split_once(':')?.1.trim_start();
    let quote = rest.chars().next().filter(|c| matches!(c, '"' | '\'' | '`'))?;
    let mut out = String::new();
    let mut escaped = false;
    for c in rest[1..].chars() {
        match (escaped, c) {
            (true, c) => {
                out.push(if c == 'n' { ' ' } else { c });
                escaped = false;
            }
            (false, '\\') => escaped = true,
            (false, c) if c == quote => return Some(out),
            (false, c) => out.push(c),
        }
    }
    None
}

fn patched_file(patch: &str, cwd: Option<&str>) -> Option<String> {
    ["*** Update File: ", "*** Add File: ", "*** Delete File: "].iter().find_map(|marker| {
        let rest = &patch[patch.find(marker)? + marker.len()..];
        let end = rest.find(['\n', '\\', '"', '\'', '`']).unwrap_or(rest.len());
        Some(display_path(rest[..end].trim(), cwd))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exec_code_names_the_inner_tool() {
        let cwd = Some("/w/p");
        assert_eq!(
            code_target("exec", "text(await tools.exec_command({cmd: \"cargo test --all\", yield_time_ms: 1000}))", cwd),
            ("exec_command".to_string(), Some("cargo test".to_string()))
        );
        assert_eq!(
            code_target("exec", "text(await tools.apply_patch(`*** Begin Patch\n*** Update File: /w/p/src/a.rs\n@@`))", cwd),
            ("apply_patch".to_string(), Some("src/a.rs".to_string()))
        );
        assert_eq!(code_target("exec", "await Promise.all([tools.exec_command({}), tools.write_stdin({})])", cwd).1.as_deref(), Some("exec_command, write_stdin"));
    }

    #[test]
    fn thread_ids_come_from_file_names() {
        let p = Path::new("/x/rollout-2026-08-27T22-32-52-01a044ec-eb9e-7923-829a-0123456789ab.jsonl.zst");
        assert_eq!(id_from_name(p), "01a044ec-eb9e-7923-829a-0123456789ab");
    }

    #[test]
    fn injected_context_is_labelled() {
        assert_eq!(context_label("# AGENTS.md instructions for /w/p").as_deref(), Some("instructions"));
        assert_eq!(context_label("<environment_context>\n  <cwd>").as_deref(), Some("environment context"));
        assert_eq!(context_label("как успехи?"), None);
    }

    #[test]
    fn subagent_threads_get_their_task_and_origin() {
        let child = |source: Value| {
            let mut p = Parser::new("c".into());
            let meta = serde_json::json!({"type": "session_meta", "payload": {"id": "c", "parent_thread_id": "p", "cwd": "/w", "source": {"subagent": source}}});
            p.line(&meta.to_string());
            p.line(&serde_json::json!({"type": "token_usage_record", "payload": {"response_id": "r1", "usage": {"input_tokens": 10, "output_tokens": 1}}}).to_string());
            p.finish()
        };
        let s = child(serde_json::json!({"thread_spawn": {"parent_thread_id": "p", "depth": 1, "agent_path": "/root/fix_flaky_tests", "agent_nickname": "Carver", "agent_role": null}}));
        assert_eq!(s.task.as_deref(), Some("fix flaky tests"));
        // The parent's rollout is not on disk, so the thread counts under the parent's id.
        assert_eq!(s.id, "p");
        assert_eq!(s.chains[0].name, "subagent: fix flaky tests");
        assert_eq!(s.chains[0].requests[0].origin.as_deref(), Some("agent:default"));
        let s = child(serde_json::json!({"other": "guardian"}));
        assert_eq!((s.task.as_deref(), s.chains[0].name.as_str()), (None, "subagent:guardian"));
        assert_eq!(s.chains[0].requests[0].origin.as_deref(), Some("agent:guardian"));
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
    fn a_compaction_marks_the_next_request_and_is_kept_without_a_kind() {
        let mut p = Parser::new("t".into());
        let mut line = |v: Value| p.line(&v.to_string());
        line(serde_json::json!({"type": "session_meta", "payload": {"id": "t", "cwd": "/w/p"}}));
        line(serde_json::json!({"timestamp": "2026-10-29T10:00:00Z", "type": "token_usage_record", "payload": {"response_id": "r1", "usage": {"input_tokens": 100, "output_tokens": 1}}}));
        line(serde_json::json!({"timestamp": "2026-10-29T10:05:00Z", "type": "compacted", "payload": {"replacement_history": []}}));
        line(serde_json::json!({"timestamp": "2026-10-29T10:10:00Z", "type": "token_usage_record", "payload": {"response_id": "r2", "usage": {"input_tokens": 20, "output_tokens": 1}}}));
        let s = p.finish();
        let marked: Vec<bool> = s.chains[0].requests.iter().map(|r| r.compacted_before).collect();
        assert_eq!(marked, [false, true]);
        assert_eq!(s.compactions, [Compaction { ts_ms: parse_ts_ms(Some(&Value::from("2026-10-29T10:05:00Z"))).unwrap(), kind: None }]);
    }

    #[test]
    fn a_request_prompt_excludes_its_own_response() {
        let mut p = Parser::new("t".into());
        let line = |v: Value| p.line(&v.to_string());
        let mut line = line;
        line(serde_json::json!({"type": "session_meta", "payload": {"id": "t", "cwd": "/w/p", "base_instructions": {"text": "be good"}}}));
        line(serde_json::json!({"type": "response_item", "payload": {"type": "message", "role": "user", "content": [{"type": "input_text", "text": "fix it"}]}}));
        line(serde_json::json!({"type": "response_item", "payload": {"type": "reasoning", "encrypted_content": "abcd"}}));
        line(serde_json::json!({"type": "response_item", "payload": {"type": "function_call", "name": "exec_command", "arguments": "{\"cmd\":\"ls -la\"}", "call_id": "c1"}}));
        line(serde_json::json!({"type": "token_usage_record", "payload": {"response_id": "r1", "usage": {"input_tokens": 100, "cached_input_tokens": 0, "output_tokens": 10}}}));
        line(serde_json::json!({"type": "response_item", "payload": {"type": "function_call_output", "call_id": "c1", "output": "a b c"}}));
        line(serde_json::json!({"type": "token_usage_record", "payload": {"response_id": "r2", "usage": {"input_tokens": 120, "cached_input_tokens": 100, "output_tokens": 5}}}));
        let s = p.finish();
        let reqs = &s.chains[0].requests;
        assert_eq!(reqs.len(), 2);
        let (d1, d2) = (reqs[0].prompt.as_ref().unwrap(), reqs[1].prompt.as_ref().unwrap());
        assert_eq!((d1.keep, d1.tail.len()), (0, 1), "first prompt: the user message only");
        assert_eq!((d2.keep, d2.tail.len()), (1, 3), "then reasoning, the call and its output");
        let result = &s.blocks[s.messages[d2.tail[2]].blocks.clone()][0];
        assert_eq!((result.kind, result.tool.as_deref(), result.target.as_deref()), (BlockKind::ToolResult, Some("exec_command"), Some("ls -la")));
        assert!(reqs[0].produced.is_some());
    }
}
