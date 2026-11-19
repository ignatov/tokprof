//! Codex rollouts: `~/.codex/sessions/YYYY/MM/DD/rollout-*.jsonl` and
//! `~/.codex/archived_sessions/`, one thread per file.
//!
//! - Usage per request comes from `token_usage_record` lines, deduplicated by `response_id`.
//!   Older rollouts only have `token_count` events, whose `last_token_usage` counts when the
//!   cumulative total changed. `input_tokens` includes the cached and cache-written parts.
//! - A `compacted` line names the request that made the compaction, whose response is the
//!   summary, in `compaction_response_id`.
//! - The prompt is rebuilt from the rollout. `session_meta.base_instructions` is the system
//!   prompt, every `response_item` is one item of the conversation, and `compacted` replaces
//!   the conversation with its `replacement_history`. A request's prompt is the conversation
//!   before the items its own response added: the trailing reasoning, assistant messages and
//!   tool calls. Tool schemas other than `additional_tools` are not logged.
//! - `world_state` repeats what the messages already carry (AGENTS.md, environment), so it is
//!   not counted.
//! - Subagent threads name their parent in `session_meta.parent_thread_id`.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde_json::Value;
use tokprof_core::api::ApiId;
use tokprof_core::labels::{command_head, display_path, message_label};
use tokprof_core::model::{
    Block, BlockKind, CallEnd, CallRecord, Chain, ChainKind, Class, Compaction, CompactionTrigger, HandoffKind, Origin, Params, PromptDelta, ReminderKind,
    Request, Session, Shape, ToolClass, TypedMessage, Usage,
};
use tokprof_core::source::{AgentId, Decoder};
use tokprof_core::trigger::{CallClass, Pending, Trigger, VcsActions, command_class, vcs_actions};
use tokprof_logio::{json_len, parse_ts_ms, str_at, u64_at};

use crate::tools::{self, tool_target};

/// Size an image is counted as, in characters (roughly 1,400 tokens); low detail is 85 tokens.
const IMAGE_CHARS: u64 = 5_000;
const LOW_DETAIL_IMAGE_CHARS: u64 = 300;

/// Per usage source: the model settings and the prompt state its requests continue from.
#[derive(Clone, Default)]
struct Stream {
    model: String,
    effort: Option<String>,
    compacted: bool,
    context_window: Option<u64>,
    /// Conversation items in the previous request's prompt; `None` before the first request
    /// and after a compaction replaced the conversation.
    prompt_len: Option<usize>,
    /// The calls of the latest response, and what was logged after them.
    pending: Pending,
    /// The latest response ended with an assistant message and made no call.
    ended_with_text: bool,
    requests: Vec<Request>,
}

/// The topmost thread above a parent thread, as the source's thread index knows it.
pub(crate) type RootOf = Arc<dyn Fn(&str) -> String + Send + Sync>;

/// The names Codex shows for threads, by thread id, as `session_index.jsonl` holds them now: a
/// thread renamed after its rollout was read shows its new name in the next snapshot.
pub(crate) type Names = Arc<dyn Fn() -> Arc<HashMap<String, String>> + Send + Sync>;

/// Reads a Codex rollout, one line at a time.
#[derive(Clone)]
pub(crate) struct Parser {
    session: Session,
    home: PathBuf,
    root_of: RootOf,
    names: Names,
    /// The rollout's thread counts under the thread at the top of its parents, whose name it
    /// shows.
    rooted: bool,
    /// The conversation so far, as message indices.
    history: Vec<usize>,
    /// Items at the end of `history` that the response in flight produced.
    trailing_outputs: usize,
    tools_by_call: HashMap<String, (String, Option<String>)>,
    /// What the calls of a conversation item did, by message index.
    call_kinds: HashMap<usize, Vec<CallClass>>,
    /// The calls of the item being read, until it gets its message index.
    item_calls: Vec<CallClass>,
    /// The tool and input head of the call item being read, until it gets its message index.
    item_call: Option<(String, Option<String>)>,
    /// The times and flags of each tool call, and its place in that list by call id; a hosted
    /// call, which has no call id, is keyed by its message.
    calls: Vec<CallRecord>,
    call_by_id: HashMap<String, usize>,
    shape: Option<usize>,
    streams: [Stream; 2],
    /// The response ids of the `token_usage_record` lines read, each with the index of its
    /// stream-0 request when the line had usage.
    responses: HashMap<String, Option<usize>>,
    last_total: String,
    subagent: Option<String>,
    /// `session_meta.originator`: the program that ran Codex, for every request of the rollout.
    client: Option<Arc<str>>,
    /// The time of the latest logged item that the model did not produce: the input of the next
    /// response.
    last_input_ms: Option<i64>,
    /// The latest logged item was one the model produced.
    prev_output: bool,
    /// The latest run of items the model produced: the send of its response (the input before
    /// it) and the time of its last item. Codex logs `token_count` after the outputs of the
    /// tools a response called, so the run, not the usage line, says when the response ended.
    run: Option<(i64, i64)>,
    /// Per stream: it already gave `run` to a request.
    run_used: [bool; 2],
    /// The call id of each conversation item that is a call or its output, by message index.
    call_of_message: HashMap<usize, String>,
    /// The call and output blocks of each call id, in the items and in the responses' copies.
    blocks_by_call: HashMap<String, Vec<usize>>,
    /// The calls whose shell commands committed, pushed or opened a pull request, by call id.
    vcs_by_call: HashMap<String, VcsActions>,
    /// The calls that did not work, by call id.
    call_ends: HashMap<String, CallEnd>,
    /// The first `session_meta.cli_version` of the rollout, for every request of it: a later
    /// `session_meta` of a resumed rollout does not change it, so no miss gets the cause
    /// `agent updated` from a resume.
    version: Option<String>,
}

impl Parser {
    pub(crate) fn new(agent: AgentId, id: String, home: PathBuf, root_of: RootOf, names: Names) -> Parser {
        Parser {
            session: Session::new(agent, id),
            home,
            root_of,
            names,
            rooted: false,
            history: Vec::new(),
            trailing_outputs: 0,
            tools_by_call: HashMap::new(),
            call_kinds: HashMap::new(),
            item_calls: Vec::new(),
            item_call: None,
            calls: Vec::new(),
            call_by_id: HashMap::new(),
            shape: None,
            streams: [Stream::default(), Stream::default()],
            responses: HashMap::new(),
            last_total: String::new(),
            subagent: None,
            client: None,
            last_input_ms: None,
            prev_output: false,
            run: None,
            run_used: [false; 2],
            call_of_message: HashMap::new(),
            blocks_by_call: HashMap::new(),
            vcs_by_call: HashMap::new(),
            call_ends: HashMap::new(),
            version: None,
        }
    }

    fn line(&mut self, line: &str) {
        // Completed-item events repeat the response items for the UI.
        if line.contains("\"item_completed\"") {
            return;
        }
        let Ok(v) = serde_json::from_str::<Value>(line) else { return };
        let payload = v.get("payload").unwrap_or(&Value::Null);
        let at = parse_ts_ms(v.get("timestamp"));
        let ts = at.unwrap_or(0);
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
                // The replacement history repeats old messages; none of them is typed anew.
                for item in &items {
                    self.push_item(item, None);
                }
                self.trailing_outputs = 0;
                for s in &mut self.streams {
                    s.compacted = true;
                    s.prompt_len = None;
                }
                self.session.compactions.push(Compaction { ts_ms: ts, kind: CompactionTrigger::Unknown });
                // The request that made the compaction, by its response id (0.153–0.159). The
                // usage of a rollout with only `token_count` events has no id, so its call is not
                // marked.
                if let Some(&Some(at)) = str_at(payload, "compaction_response_id").and_then(|id| self.responses.get(id)) {
                    self.streams[0].requests[at].compaction_call = true;
                }
            }
            "response_item" => self.push_item(payload, Some(ts)),
            "token_usage_record" => {
                let id = str_at(payload, "response_id");
                if id.is_some_and(|id| self.responses.contains_key(id)) {
                    return;
                }
                let usage = payload.get("usage");
                if let Some(usage) = usage {
                    self.request(0, usage, ts);
                }
                if let Some(id) = id {
                    let at = usage.map(|_| self.streams[0].requests.len() - 1);
                    self.responses.insert(id.to_string(), at);
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
                self.session.compactions.push(Compaction { ts_ms: ts, kind: CompactionTrigger::Unknown });
            }
            // Seen in 0.147–0.166.
            "event_msg" if ptype == "task_complete" => self.handoff(at, HandoffKind::TurnEnd),
            "event_msg" if ptype == "turn_aborted" => self.handoff(at, HandoffKind::Interrupt),
            _ => {}
        }
    }

    /// A hand-off between the user and the agent, at the time of its line. A subagent thread's
    /// turns are the agent's own work, so it has none.
    fn handoff(&mut self, ts_ms: Option<i64>, kind: HandoffKind) {
        if let (None, Some(ts_ms)) = (&self.subagent, ts_ms) {
            self.session.push_handoff(ts_ms, kind);
        }
    }

    fn session_meta(&mut self, payload: &Value) {
        if let Some(id) = str_at(payload, "id") {
            self.session.id = id.to_string();
        }
        if self.version.is_none() {
            self.version = str_at(payload, "cli_version").map(str::to_string);
        }
        if let Some(originator) = str_at(payload, "originator") {
            self.client = Some(Arc::from(originator));
            // `codex exec` ran the thread, with nobody typing in it.
            self.session.scripted = originator == "codex_exec";
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
            self.session.task =
                spawn.and_then(|s| str_at(s, "agent_path")).and_then(|p| p.rsplit('/').next()).filter(|t| !t.is_empty()).map(|t| t.replace('_', " "));
            // The thread's requests count under its session, however deep the thread is.
            if let Some(parent) = str_at(payload, "parent_thread_id") {
                self.session.id = (self.root_of)(parent);
                self.rooted = true;
            }
        }
        if self.shape.is_none() {
            let chars = payload.pointer("/base_instructions/text").and_then(Value::as_str).map_or(0, |t| t.len() as u64);
            let start = self.session.blocks.len();
            self.session.blocks.push(Block { class: Class::None, kind: BlockKind::System, chars, tool: None, target: None });
            self.session.shapes.push(Shape { blocks: start..start + 1, fingerprint: chars, tool_names: Vec::new(), system_chars: chars });
            self.shape = Some(self.session.shapes.len() - 1);
        }
    }

    fn note_cwd(&mut self, payload: &Value) {
        if self.session.cwd.is_none()
            && let Some(cwd) = str_at(payload, "cwd")
        {
            self.session.cwd = Some(cwd.to_string());
            self.session.project = display_path(cwd, None, &self.home);
        }
    }

    /// Adds a conversation item; `ts_ms` is `None` for items replayed from a compaction.
    fn push_item(&mut self, item: &Value, ts_ms: Option<i64>) {
        let (blocks, output) = self.blocks(item);
        if blocks.is_empty() {
            return;
        }
        let starts_turn = blocks.iter().any(|b| b.kind == BlockKind::Prompt);
        if starts_turn {
            self.streams.iter_mut().for_each(|s| s.pending.event(Trigger::Prompt));
            if let (Some(ts_ms), None) = (ts_ms, &self.subagent) {
                let text: Vec<&str> = item
                    .get("content")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter_map(|c| str_at(c, "text"))
                    .filter(|t| context_label(t).is_none())
                    .collect();
                self.session.typed.push(TypedMessage { ts_ms, label: message_label(&text.join("\n\n")) });
            }
        }
        let idx = self.session.push_message(blocks, starts_turn);
        let calls = std::mem::take(&mut self.item_calls);
        if !calls.is_empty() {
            self.call_kinds.insert(idx, calls);
        }
        let call = self.item_call.take();
        // A hosted call has no call id and no output; its message keys it.
        let key = str_at(item, "call_id").map(str::to_string).or_else(|| call.as_ref().map(|_| format!("\0hosted {idx}")));
        if let Some(id) = key.as_deref() {
            self.blocks_by_call.entry(id.to_string()).or_default().extend(self.session.messages[idx].blocks.clone());
            self.call_of_message.insert(idx, id.to_string());
            if matches!(str_at(item, "type"), Some("function_call_output" | "custom_tool_call_output"))
                && let Some(end) = tools::call_end(item.get("output").unwrap_or(&Value::Null))
            {
                self.call_ends.insert(id.to_string(), end);
            }
            self.note_call(id, call, output, ts_ms.filter(|&t| t > 0));
        }
        self.history.push(idx);
        self.trailing_outputs = if output { self.trailing_outputs + 1 } else { 0 };
        // Items replayed from a compaction tell no time.
        if let Some(ts) = ts_ms.filter(|&t| t > 0) {
            match (output, self.prev_output, &mut self.run) {
                (true, true, Some(run)) => run.1 = run.1.max(ts),
                (true, _, _) => {
                    self.run = Some((self.last_input_ms.unwrap_or(ts), ts));
                    self.run_used = [false; 2];
                }
                (false, _, _) => self.last_input_ms = Some(ts),
            }
            self.prev_output = output;
        }
    }

    /// The times of a call (see session-logs, Call times): a call item starts it at its line's
    /// `timestamp`, and the output item with the same `call_id` ends it at its own. A hosted call
    /// is done without an end. Items replayed from a compaction tell no time; a call seen before
    /// keeps its record. Seen in 0.147–0.169.
    fn note_call(&mut self, id: &str, call: Option<(String, Option<String>)>, output: bool, ts_ms: Option<i64>) {
        match (call, self.call_by_id.get(id)) {
            (Some((tool, input)), None) if output => {
                self.call_by_id.insert(id.to_string(), self.calls.len());
                self.calls.push(CallRecord {
                    start_ms: ts_ms,
                    end_ms: None,
                    done: id.starts_with('\0'),
                    background: false,
                    asks_user: tools::asks_the_user(&tool),
                    input,
                });
            }
            (None, Some(&at)) if !output => {
                let call = &mut self.calls[at];
                if !call.done {
                    call.done = true;
                    call.end_ms = ts_ms.filter(|_| call.start_ms.is_some());
                }
            }
            _ => {}
        }
    }

    /// The blocks of one conversation item, and whether the model produced it.
    fn blocks(&mut self, item: &Value) -> (Vec<Block>, bool) {
        let cwd = self.session.cwd.clone();
        let cwd = cwd.as_deref();
        let block = |kind, chars, tool: Option<String>, target: Option<String>| Block { class: Class::None, kind, chars, tool, target };
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
                let summary: u64 =
                    item.get("summary").and_then(Value::as_array).into_iter().flatten().map(|s| str_at(s, "text").map_or(0, |t| t.len() as u64)).sum();
                (vec![block(BlockKind::Thinking, encrypted + summary, None, None)], true)
            }
            "function_call" | "custom_tool_call" => {
                // The log's name sizes the block; an MCP server's tool is named with its server.
                let logged = str_at(item, "name").unwrap_or("?");
                let name = mcp_name(item);
                let name = name.as_deref().unwrap_or(logged);
                let (tool, target, chars, vcs) = if ty == "custom_tool_call" {
                    let input = str_at(item, "input").unwrap_or_default();
                    self.item_calls = code_calls(name, input);
                    let (tool, target) = code_target(name, input, cwd, &self.home);
                    (tool, target, input.len() as u64, code_vcs(name, input))
                } else {
                    let args = str_at(item, "arguments").unwrap_or_default();
                    let parsed: Value = serde_json::from_str(args).unwrap_or(Value::Null);
                    self.item_calls = vec![call_class(name, &parsed)];
                    let vcs = shell_command(name, &parsed).map(|c| vcs_actions(&c)).unwrap_or_default();
                    (name.to_string(), call_target(name, &parsed, cwd, &self.home), args.len() as u64, vcs)
                };
                if let (Some(id), true) = (str_at(item, "call_id"), vcs.any()) {
                    self.vcs_by_call.insert(id.to_string(), vcs);
                }
                if let Some(id) = str_at(item, "call_id") {
                    self.tools_by_call.insert(id.to_string(), (tool.clone(), target.clone()));
                }
                let input = if ty == "custom_tool_call" {
                    code_input(&tool, str_at(item, "input").unwrap_or_default(), target.as_deref(), cwd, &self.home)
                } else {
                    call_input(name, &serde_json::from_str(str_at(item, "arguments").unwrap_or_default()).unwrap_or(Value::Null), target.as_deref())
                };
                self.item_call = Some((tool.clone(), input));
                (vec![block(BlockKind::ToolUse, logged.len() as u64 + chars, Some(tool), target)], true)
            }
            "function_call_output" | "custom_tool_call_output" => {
                let (tool, target) = str_at(item, "call_id").and_then(|id| self.tools_by_call.get(id).cloned()).map_or((None, None), |(t, g)| (Some(t), g));
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
            t if t.ends_with("_call") => {
                // A hosted call, such as `web_search_call`, which the API runs.
                let query = item.get("action").and_then(|a| str_at(a, "query")).and_then(tokprof_core::model::input_head);
                self.item_call = Some((t.to_string(), query));
                (vec![block(BlockKind::ToolUse, json_len(item), Some(t.to_string()), None)], true)
            }
            _ => (vec![block(BlockKind::Unknown, json_len(item), None, None)], false),
        }
    }

    fn request(&mut self, stream: usize, usage: &Value, ts_ms: i64) {
        let prompt_len = self.history.len() - self.trailing_outputs.min(self.history.len());
        let produced = (self.trailing_outputs > 0).then(|| {
            let blocks: Vec<Block> =
                self.history[prompt_len..].iter().flat_map(|&m| self.session.blocks[self.session.messages[m].blocks.clone()].to_vec()).collect();
            // The copy holds the calls too, so their outcome reaches it.
            let mut at = self.session.blocks.len();
            for &m in &self.history[prompt_len..] {
                let n = self.session.messages[m].blocks.len();
                if let Some(id) = self.call_of_message.get(&m) {
                    self.blocks_by_call.entry(id.clone()).or_default().extend(at..at + n);
                }
                at += n;
            }
            self.session.push_message(blocks, false)
        });
        let outputs = &self.history[prompt_len..];
        let calls: Vec<CallClass> = outputs.iter().filter_map(|m| self.call_kinds.get(m)).flatten().copied().collect();
        let text = outputs.iter().any(|&m| self.session.blocks[self.session.messages[m].blocks.clone()].iter().any(|b| b.kind == BlockKind::Text));
        let s = &mut self.streams[stream];
        let trigger = s.pending.trigger().or((s.ended_with_text && !s.requests.is_empty()).then_some(Trigger::Continuation));
        let activity = s.pending.activity();
        s.pending = Pending::default();
        calls.iter().for_each(|&k| s.pending.call(k));
        s.ended_with_text = text && calls.is_empty();
        let keep = s.prompt_len.filter(|&prev| prev <= prompt_len).unwrap_or(0);
        let delta = PromptDelta { shape: if s.requests.is_empty() { self.shape } else { None }, keep, tail: self.history[keep..prompt_len].to_vec() };
        let input = u64_at(usage, "input_tokens");
        let cached = u64_at(usage, "cached_input_tokens");
        let written = u64_at(usage, "cache_write_input_tokens");
        // The response's run of items, once per stream; a response that added none is timed by its
        // own line.
        let run = self.run.filter(|_| !self.run_used[stream]);
        self.run_used[stream] = true;
        let (sent, ended) = run.unwrap_or((ts_ms, ts_ms));
        let s = &mut self.streams[stream];
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
            context_window: s.context_window,
            trigger,
            activity,
            client: self.client.clone(),
            first_sent_ms: Some(sent),
            sent_ms: Some(sent),
            ended_ms: Some(ended.max(sent)),
            ..Request::default()
        });
        s.compacted = false;
        s.prompt_len = Some(prompt_len);
    }

    pub(crate) fn into_session(mut self) -> Session {
        let [records, counts] = std::mem::take(&mut self.streams);
        let mut requests = if records.requests.is_empty() { counts.requests } else { records.requests };
        let (name, kind) = match self.subagent.take() {
            Some(name) => {
                let origin = name.replacen("subagent", "agent", 1);
                requests.iter_mut().for_each(|r| r.origin = Some(Origin::parse(&origin)));
                let name = self.session.task.as_ref().map_or(name, |task| format!("subagent: {task}"));
                if self.rooted {
                    self.session.title = (self.names)().get(&self.session.id).cloned();
                }
                (name, ChainKind::Subagent)
            }
            None => {
                self.session.title = (self.names)().get(&self.session.id).cloned();
                ("main".to_string(), ChainKind::Main)
            }
        };
        for b in &mut self.session.blocks {
            b.class = class_of(b);
        }
        // Codex sends its requests to the OpenAI API; its configured provider is not read yet.
        requests.iter_mut().for_each(|r| {
            r.api = ApiId::OpenAi;
            r.version = self.version.clone();
        });
        self.session.chains.push(Chain { name, kind, requests });
        self.session.compactions.sort_by_key(|c| c.ts_ms);
        // A call's times go to all its blocks, the copies in responses too.
        for (id, &at) in &self.call_by_id {
            for &b in self.blocks_by_call.get(id).into_iter().flatten() {
                self.session.call_of_block.insert(b, at as u32);
            }
        }
        self.session.calls = std::mem::take(&mut self.calls);
        for (id, end) in &self.call_ends {
            for &b in self.blocks_by_call.get(id).into_iter().flatten() {
                self.session.call_ends.insert(b, end.clone());
            }
        }
        // A call's actions go to its call blocks, the copies in responses too.
        for (id, vcs) in &self.vcs_by_call {
            for &b in self.blocks_by_call.get(id).into_iter().flatten() {
                if self.session.blocks.get(b).is_some_and(|x| x.kind == BlockKind::ToolUse) {
                    self.session.vcs.insert(b, *vcs);
                }
            }
        }
        self.session
    }
}

/// The class of a block's Codex tool or injected context.
fn class_of(b: &Block) -> Class {
    match b.kind {
        BlockKind::ToolUse | BlockKind::ToolResult => Class::Tool(match b.tool.as_deref().unwrap_or_default() {
            // `exec` here runs code that calls no tool or several, mostly `exec_command` and
            // `write_stdin`; with one inner tool it shows as that tool.
            "exec_command" | "shell" | "exec" => ToolClass::Execute,
            // Reads what a command running in its session printed since, and how it exited; `wait`
            // waits for the output of a running code cell.
            "write_stdin" | "wait" => ToolClass::ExecuteOutput,
            "apply_patch" => ToolClass::Edit,
            "view_image" => ToolClass::Read,
            "web_search_call" => ToolClass::Fetch,
            "spawn_agent" | "send_input" | "close_agent" | "resume_agent" | "wait_agent" | "list_agents" | "followup_task" | "send_message"
            | "interrupt_agent" => ToolClass::Subagent,
            "update_plan" | "create_goal" | "update_goal" | "get_goal" => ToolClass::Think,
            "request_user_input" | "request_user_input_async" | "sleep" => ToolClass::Manage,
            _ => ToolClass::Other,
        }),
        BlockKind::Reminder => Class::Reminder(ReminderKind::of_label(b.target.as_deref().unwrap_or_default())),
        _ => Class::None,
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
fn call_target(name: &str, args: &Value, cwd: Option<&str>, home: &Path) -> Option<String> {
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
        "apply_patch" => s("input").and_then(|p| patched_file(p, cwd, home)),
        "spawn_agent" => s("agent_type").or(s("role")).or(s("name")).map(str::to_string),
        _ => tool_target(name, args, cwd, home),
    }
}

/// The input head of a function call (see session-logs, Input of a call): the first line of a
/// shell command, the patched file or the agent type as the target shows them.
fn call_input(name: &str, args: &Value, target: Option<&str>) -> Option<String> {
    match name {
        "apply_patch" | "spawn_agent" => target.map(str::to_string),
        _ => shell_command(name, args).as_deref().and_then(tokprof_core::model::input_head),
    }
}

/// The input head of a custom tool call: for an `exec` call with one inner tool, that tool's, as
/// `code_target` names it, or the file a direct `apply_patch` call patched.
fn code_input(tool: &str, code: &str, target: Option<&str>, cwd: Option<&str>, home: &Path) -> Option<String> {
    match tool {
        "apply_patch" => target.map(str::to_string).or_else(|| patched_file(code, cwd, home)),
        "spawn_agent" => target.map(str::to_string),
        "exec_command" => js_string(code, "cmd").as_deref().and_then(tokprof_core::model::input_head),
        _ => None,
    }
}

/// The name of a call of an MCP server's tool that the item names by its server's `namespace`
/// (`js` in `mcp__cua_repl`, seen in 0.154–0.169), as `exec` code names such a tool:
/// `mcp__cua_repl__js`. `None` for a tool of Codex's own namespaces, such as `clock`.
fn mcp_name(item: &Value) -> Option<String> {
    let namespace = str_at(item, "namespace").filter(|n| n.starts_with("mcp__") && n.len() > 5)?;
    Some(format!("{namespace}__{}", str_at(item, "name")?))
}

/// Codex's `exec` tool takes JavaScript that calls the real tools (`tools.exec_command(...)`);
/// a single inner tool becomes the tool, with its command or patched file as the target.
fn code_target(name: &str, code: &str, cwd: Option<&str>, home: &Path) -> (String, Option<String>) {
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
                "apply_patch" => patched_file(code, cwd, home),
                t => crate::tools::tool_target(t, &Value::Null, cwd, home),
            };
            (one.clone(), target)
        }
        many => (name.to_string(), Some(many.join(", "))),
    }
}

/// The shell command of a function call that runs one: `exec_command`'s `cmd`, or the `command`
/// of `shell` and `container.exec`, a string or an array whose script follows a flag.
fn shell_command(name: &str, args: &Value) -> Option<String> {
    if !matches!(name, "exec_command" | "shell" | "container.exec") {
        return None;
    }
    match args.get("cmd").or(args.get("command")) {
        Some(Value::String(c)) => Some(c.clone()),
        Some(Value::Array(parts)) => {
            let parts: Vec<&str> = parts.iter().filter_map(Value::as_str).collect();
            match parts.as_slice() {
                [_, flag, script] if flag.starts_with('-') => Some(script.to_string()),
                _ => Some(parts.join(" ")),
            }
        }
        _ => None,
    }
}

/// What the commands of a custom tool did to version control: the `exec` tool's inner
/// `tools.exec_command(...)` calls, each with its `cmd`.
fn code_vcs(name: &str, code: &str) -> VcsActions {
    if name == "apply_patch" {
        return VcsActions::default();
    }
    let starts: Vec<usize> = code.match_indices("tools.exec_command").map(|(at, _)| at).collect();
    let mut all = VcsActions::default();
    for (i, &at) in starts.iter().enumerate() {
        let segment = &code[at..starts.get(i + 1).copied().unwrap_or(code.len())];
        if let Some(cmd) = js_string(segment, "cmd") {
            let a = vcs_actions(&cmd);
            all.commit |= a.commit;
            all.push |= a.push;
            all.pull_request |= a.pull_request;
        }
    }
    all
}

/// What a Codex function call did, for the trigger and activity of the request after it.
fn call_class(name: &str, args: &Value) -> CallClass {
    match name {
        "exec_command" | "shell" | "container.exec" => shell_command(name, args).map_or(CallClass::System, |c| command_class(&c)),
        "apply_patch" => CallClass::Editing,
        "spawn_agent" => CallClass::Subagents,
        _ => CallClass::OtherTools,
    }
}

/// What the calls of a custom tool did. The `exec` tool's JavaScript is read by its inner
/// `tools.<name>(...)` calls, one class each; a command that is not a plain string literal is
/// a file and system command.
fn code_calls(name: &str, code: &str) -> Vec<CallClass> {
    if name == "apply_patch" {
        return vec![CallClass::Editing];
    }
    let starts: Vec<usize> = code.match_indices("tools.").map(|(at, _)| at).collect();
    let mut kinds = Vec::new();
    for (i, &at) in starts.iter().enumerate() {
        let segment = &code[at..starts.get(i + 1).copied().unwrap_or(code.len())];
        let ident: String = segment[6..].chars().take_while(|c| c.is_ascii_alphanumeric() || *c == '_').collect();
        kinds.push(match ident.as_str() {
            "" => continue,
            "exec_command" => js_string(segment, "cmd").map_or(CallClass::System, |c| command_class(&c)),
            "apply_patch" => CallClass::Editing,
            "spawn_agent" => CallClass::Subagents,
            _ => CallClass::OtherTools,
        });
    }
    if kinds.is_empty() {
        kinds.push(CallClass::OtherTools);
    }
    kinds
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

fn patched_file(patch: &str, cwd: Option<&str>, home: &Path) -> Option<String> {
    ["*** Update File: ", "*** Add File: ", "*** Delete File: "].iter().find_map(|marker| {
        let rest = &patch[patch.find(marker)? + marker.len()..];
        let end = rest.find(['\n', '\\', '"', '\'', '`']).unwrap_or(rest.len());
        Some(display_path(rest[..end].trim(), cwd, home))
    })
}

impl Decoder for Parser {
    fn feed(&mut self, record: &str) {
        self.line(record);
    }

    fn snapshot(&self) -> Session {
        self.clone().into_session()
    }

    fn finish(self: Box<Self>) -> Session {
        (*self).into_session()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parser(id: &str) -> Parser {
        Parser::new(crate::Codex::agent(), id.into(), PathBuf::from("/h"), Arc::new(|parent: &str| parent.to_string()), Arc::new(Arc::default))
    }

    #[test]
    fn exec_code_names_the_inner_tool() {
        let cwd = Some("/w/p");
        assert_eq!(
            code_target("exec", "text(await tools.exec_command({cmd: \"cargo test --all\", yield_time_ms: 1000}))", cwd, Path::new("/h")),
            ("exec_command".to_string(), Some("cargo test".to_string()))
        );
        assert_eq!(
            code_target("exec", "text(await tools.apply_patch(`*** Begin Patch\n*** Update File: /w/p/src/a.rs\n@@`))", cwd, Path::new("/h")),
            ("apply_patch".to_string(), Some("src/a.rs".to_string()))
        );
        assert_eq!(
            code_target("exec", "await Promise.all([tools.exec_command({}), tools.write_stdin({})])", cwd, Path::new("/h")).1.as_deref(),
            Some("exec_command, write_stdin")
        );
        assert_eq!(
            code_target("exec", "text(await tools.mcp__docs__search({query: \"proxy\"}))", cwd, Path::new("/h")),
            ("mcp__docs__search".to_string(), Some("mcp:docs".to_string())),
            "an MCP tool belongs to its server"
        );
        assert_eq!(code_target("exec", "text(await tools.update_goal({status: \"complete\"}))", cwd, Path::new("/h")), ("update_goal".to_string(), None));
    }

    #[test]
    fn a_call_in_an_mcp_namespace_is_a_tool_of_that_server() {
        let call =
            |namespace: &str, name: &str| serde_json::json!({"type": "function_call", "name": name, "namespace": namespace, "call_id": "c", "arguments": "{}"});
        assert_eq!(mcp_name(&call("mcp__cua_repl", "js")).as_deref(), Some("mcp__cua_repl__js"));
        assert_eq!(mcp_name(&call("clock", "sleep")), None, "a namespace of Codex's own");
        assert_eq!(mcp_name(&serde_json::json!({"type": "function_call", "name": "wait"})), None);
    }

    #[test]
    fn a_request_is_sent_after_its_input_and_ends_with_its_last_item() {
        use serde_json::json;
        let ms = |ts: &str| tokprof_logio::parse_ts_ms(Some(&Value::String(ts.to_string()))).unwrap();
        let item = |ts: &str, payload: Value| json!({"type": "response_item", "timestamp": ts, "payload": payload});
        let usage = json!({"input_tokens": 100, "output_tokens": 1});
        let times = |lines: &[Value]| {
            let mut p = parser("t");
            lines.iter().for_each(|l| p.line(&l.to_string()));
            p.into_session().chains[0].requests.iter().map(|r| (r.sent_ms, r.ended_ms)).collect::<Vec<_>>()
        };
        let run = |usage_line: &dyn Fn(&str, u32) -> Value| {
            vec![
                item("2026-10-01T10:00:00Z", json!({"type": "message", "role": "user", "content": [{"type": "input_text", "text": "fix it"}]})),
                item("2026-10-01T10:00:03Z", json!({"type": "reasoning", "summary": []})),
                item("2026-10-01T10:00:04Z", json!({"type": "function_call", "name": "exec_command", "call_id": "d", "arguments": "{\"cmd\":\"cargo test\"}"})),
                usage_line("2026-10-01T10:00:04.500Z", 1),
                item("2026-10-01T10:00:30Z", json!({"type": "function_call_output", "call_id": "d", "output": "ok"})),
                usage_line("2026-10-01T10:00:31Z", 1),
                item("2026-10-01T10:00:35Z", json!({"type": "message", "role": "assistant", "content": [{"type": "output_text", "text": "done"}]})),
                usage_line("2026-10-01T10:00:36Z", 2),
            ]
        };
        let expected =
            [(Some(ms("2026-10-01T10:00:00Z")), Some(ms("2026-10-01T10:00:04Z"))), (Some(ms("2026-10-01T10:00:30Z")), Some(ms("2026-10-01T10:00:35Z")))];
        // A usage record per response, logged before the tool ran.
        let record = |ts: &str, n: u32| {
            if ts.ends_with("31Z") {
                json!({"type": "event_msg", "timestamp": ts, "payload": {"type": "noop"}})
            } else {
                json!({"type": "token_usage_record", "timestamp": ts, "payload": {"response_id": format!("r{n}"), "usage": usage}})
            }
        };
        assert_eq!(times(&run(&record)), expected);
        // An older rollout's `token_count` comes after the tool's output, and the response still
        // ends at its last item.
        let total = std::cell::Cell::new(0);
        let count = |ts: &str, _: u32| {
            if ts.ends_with("04.500Z") {
                json!({"type": "event_msg", "timestamp": ts, "payload": {"type": "noop"}})
            } else {
                total.set(total.get() + 101);
                json!({"type": "event_msg", "timestamp": ts, "payload": {"type": "token_count", "info": {"total_token_usage": {"total_tokens": total.get()}, "last_token_usage": usage}}})
            }
        };
        assert_eq!(times(&run(&count)), expected);
    }

    #[test]
    fn a_failed_call_marks_the_call_its_response_made_and_its_output() {
        use serde_json::json;
        use tokprof_core::model::Outcome;
        let mut p = parser("t");
        let item = |payload: Value| json!({"type": "response_item", "timestamp": "2026-10-01T10:00:00Z", "payload": payload});
        for v in [
            item(json!({"type": "message", "role": "user", "content": [{"type": "input_text", "text": "fix it"}]})),
            item(json!({"type": "custom_tool_call", "name": "exec", "call_id": "c", "input": "text(await tools.exec_command({cmd: \"python3 x.py\"}))"})),
            json!({"type": "token_usage_record", "payload": {"response_id": "r1", "usage": {"input_tokens": 100, "output_tokens": 1}}}),
            item(
                json!({"type": "custom_tool_call_output", "call_id": "c", "output": [{"type": "input_text", "text": "Script failed\nWall time 0.0 seconds\nOutput:\n"}, {"type": "input_text", "text": "Script error:\nReferenceError: x is not defined"}]}),
            ),
        ] {
            p.line(&v.to_string());
        }
        let s = p.into_session();
        let produced = s.chains[0].requests[0].produced.unwrap();
        let call = s.messages[produced].blocks.clone().find(|&b| s.blocks[b].kind == BlockKind::ToolUse).unwrap();
        assert_eq!((s.outcome(call), s.error_line(call)), (Outcome::Failed, Some("ReferenceError: x is not defined")));
        let output = (0..s.blocks.len()).find(|&b| s.blocks[b].kind == BlockKind::ToolResult).unwrap();
        assert_eq!(s.outcome(output), Outcome::Failed);
    }

    #[test]
    fn requests_carry_what_triggered_them() {
        use serde_json::json;
        let mut p = parser("t");
        let mut n = 0;
        let mut line = |v: Value| p.line(&v.to_string());
        let mut record = |line: &mut dyn FnMut(Value)| {
            n += 1;
            line(json!({"type": "token_usage_record", "payload": {"response_id": format!("r{n}"), "usage": {"input_tokens": 100, "output_tokens": 1}}}));
        };
        let item = |payload: Value| json!({"type": "response_item", "payload": payload});
        let exec = |code: &str| item(json!({"type": "custom_tool_call", "name": "exec", "call_id": "c", "input": code}));
        let out = || item(json!({"type": "custom_tool_call_output", "call_id": "c", "output": "x"}));
        line(item(json!({"type": "message", "role": "user", "content": [{"type": "input_text", "text": "fix it"}]})));
        line(exec("text(await tools.exec_command({cmd: \"rg -n foo\"})); text(await tools.exec_command({cmd: 'sed -n 1,80p src/lib.rs'}))"));
        record(&mut line);
        line(out());
        line(exec("text(await tools.apply_patch(`*** Begin Patch\n*** Update File: a.rs`))"));
        record(&mut line);
        line(out());
        line(exec("const c = 'ls'; text(await tools.exec_command({cmd: c}))"));
        record(&mut line);
        line(out());
        line(item(json!({"type": "function_call", "name": "exec_command", "call_id": "d", "arguments": "{\"cmd\":\"cat Cargo.toml\"}"})));
        record(&mut line);
        line(item(json!({"type": "function_call_output", "call_id": "d", "output": "x"})));
        line(item(json!({"type": "message", "role": "assistant", "content": [{"type": "output_text", "text": "checking"}]})));
        record(&mut line);
        record(&mut line);
        let s = p.into_session();
        use Trigger::*;
        assert_eq!(s.chains[0].requests.iter().map(Request::trigger).collect::<Vec<_>>(), [Prompt, Lookups, Edit, Run, Lookup, Continuation]);
    }

    #[test]
    fn calls_that_commit_or_push_are_marked() {
        use serde_json::json;
        let mut p = parser("t");
        let item = |payload: Value| json!({"type": "response_item", "timestamp": "2026-10-01T10:00:00Z", "payload": payload});
        let record =
            |n: u32| json!({"type": "token_usage_record", "payload": {"response_id": format!("r{n}"), "usage": {"input_tokens": 100, "output_tokens": 1}}});
        for v in [
            item(json!({"type": "message", "role": "user", "content": [{"type": "input_text", "text": "ship it"}]})),
            item(
                json!({"type": "custom_tool_call", "name": "exec", "call_id": "c", "input": "text(await tools.exec_command({cmd: \"git push origin main\"}))"}),
            ),
            record(1),
            item(json!({"type": "custom_tool_call_output", "call_id": "c", "output": "ok"})),
            item(json!({"type": "function_call", "name": "exec_command", "call_id": "d", "arguments": "{\"cmd\":\"git -C ../zebra commit -m x\"}"})),
            record(2),
            item(json!({"type": "function_call_output", "call_id": "d", "output": "ok"})),
            item(json!({"type": "function_call", "name": "exec_command", "call_id": "e", "arguments": "{\"cmd\":\"git status\"}"})),
            record(3),
            item(json!({"type": "function_call_output", "call_id": "e", "output": "ok"})),
            record(4),
        ] {
            p.line(&v.to_string());
        }
        let s = p.into_session();
        let mut acts: Vec<(bool, bool)> = s.vcs.iter().filter(|(b, _)| s.blocks[**b].kind == BlockKind::ToolUse).map(|(_, a)| (a.commit, a.push)).collect();
        acts.dedup();
        assert!(acts.contains(&(false, true)) && acts.contains(&(true, false)) && acts.len() == 2, "{acts:?}");
    }

    #[test]
    fn requests_carry_the_activity_and_compactions_replay_no_typed_message() {
        use serde_json::json;
        let mut p = parser("t");
        let item = |payload: Value| json!({"type": "response_item", "timestamp": "2026-10-01T10:00:00Z", "payload": payload});
        let record =
            |n: u32| json!({"type": "token_usage_record", "payload": {"response_id": format!("r{n}"), "usage": {"input_tokens": 100, "output_tokens": 1}}});
        let user = |t: &str| item(json!({"type": "message", "role": "user", "content": [{"type": "input_text", "text": t}]}));
        for v in [
            user("run the tests"),
            item(json!({"type": "custom_tool_call", "name": "exec", "call_id": "c", "input": "text(await tools.exec_command({cmd: \"cargo test\"}))"})),
            record(1),
            item(json!({"type": "custom_tool_call_output", "call_id": "c", "output": "ok"})),
            item(json!({"type": "custom_tool_call", "name": "apply_patch", "call_id": "d", "input": "*** Begin Patch"})),
            record(2),
            item(json!({"type": "custom_tool_call_output", "call_id": "d", "output": "ok"})),
            record(3),
            json!({"type": "compacted", "payload": {"replacement_history": [{"type": "message", "role": "user", "content": [{"type": "input_text", "text": "run the tests"}]}]}}),
            record(4),
        ] {
            p.line(&v.to_string());
        }
        let s = p.into_session();
        let activities: Vec<&str> = s.chains[0].requests.iter().map(Request::activity).collect();
        assert_eq!(activities, ["prompt", "building and testing", "editing", "compaction"]);
        assert_eq!(s.typed.iter().map(|m| m.label.as_str()).collect::<Vec<_>>(), ["run the tests"]);
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
            let mut p = parser("c");
            let meta =
                serde_json::json!({"type": "session_meta", "payload": {"id": "c", "parent_thread_id": "p", "cwd": "/w", "source": {"subagent": source}}});
            p.line(&meta.to_string());
            p.line(
                &serde_json::json!({"type": "token_usage_record", "payload": {"response_id": "r1", "usage": {"input_tokens": 10, "output_tokens": 1}}})
                    .to_string(),
            );
            p.into_session()
        };
        let s = child(
            serde_json::json!({"thread_spawn": {"parent_thread_id": "p", "depth": 1, "agent_path": "/root/fix_flaky_tests", "agent_nickname": "Carver", "agent_role": null}}),
        );
        assert_eq!(s.task.as_deref(), Some("fix flaky tests"));
        // The parent's rollout is not on disk, so the thread counts under the parent's id.
        assert_eq!(s.id, "p");
        assert_eq!(s.chains[0].name, "subagent: fix flaky tests");
        assert_eq!(s.chains[0].requests[0].origin.as_ref().map(ToString::to_string).as_deref(), Some("agent:default"));
        let s = child(serde_json::json!({"other": "guardian"}));
        assert_eq!((s.task.as_deref(), s.chains[0].name.as_str()), (None, "subagent:guardian"));
        assert_eq!(s.chains[0].requests[0].origin.as_ref().map(ToString::to_string).as_deref(), Some("agent:guardian"));
    }

    #[test]
    fn a_compaction_marks_the_next_request_and_is_kept_without_a_kind() {
        let mut p = parser("t");
        let mut line = |v: Value| p.line(&v.to_string());
        line(serde_json::json!({"type": "session_meta", "payload": {"id": "t", "cwd": "/w/p"}}));
        line(
            serde_json::json!({"timestamp": "2026-10-29T10:00:00Z", "type": "token_usage_record", "payload": {"response_id": "r1", "usage": {"input_tokens": 100, "output_tokens": 1}}}),
        );
        line(serde_json::json!({"timestamp": "2026-10-29T10:05:00Z", "type": "compacted", "payload": {"replacement_history": []}}));
        line(
            serde_json::json!({"timestamp": "2026-10-29T10:10:00Z", "type": "token_usage_record", "payload": {"response_id": "r2", "usage": {"input_tokens": 20, "output_tokens": 1}}}),
        );
        let s = p.into_session();
        let marked: Vec<bool> = s.chains[0].requests.iter().map(|r| r.compacted_before).collect();
        assert_eq!(marked, [false, true]);
        assert_eq!(s.compactions, [Compaction { ts_ms: parse_ts_ms(Some(&Value::from("2026-10-29T10:05:00Z"))).unwrap(), kind: CompactionTrigger::Unknown }]);
    }

    #[test]
    fn a_turns_effort_reaches_its_requests() {
        let mut p = parser("t");
        p.line(&serde_json::json!({"type": "session_meta", "payload": {"id": "t", "cwd": "/w"}}).to_string());
        p.line(&serde_json::json!({"type": "turn_context", "payload": {"model": "gpt-6-astra", "effort": "xhigh"}}).to_string());
        p.line(
            &serde_json::json!({"type": "token_usage_record", "payload": {"response_id": "r1", "usage": {"input_tokens": 100, "output_tokens": 1}}})
                .to_string(),
        );
        let s = p.into_session();
        assert_eq!(s.chains[0].requests[0].params.as_ref().and_then(|x| x.effort.as_deref()), Some("xhigh"));
    }

    #[test]
    fn every_request_of_a_rollout_has_its_originator() {
        let clients = |meta: Value| {
            let mut p = parser("t");
            p.line(&serde_json::json!({"type": "session_meta", "payload": meta}).to_string());
            for (id, input) in [("r1", 100), ("r2", 120)] {
                p.line(
                    &serde_json::json!({"type": "token_usage_record", "payload": {"response_id": id, "usage": {"input_tokens": input, "output_tokens": 1}}})
                        .to_string(),
                );
            }
            p.into_session().chains[0].requests.iter().map(|r| r.client_name().to_string()).collect::<Vec<_>>()
        };
        assert_eq!(clients(serde_json::json!({"id": "t", "cwd": "/w", "originator": "codex_exec"})), ["codex_exec", "codex_exec"]);
        assert_eq!(clients(serde_json::json!({"id": "t", "cwd": "/w"})), ["unknown", "unknown"]);
    }

    /// The versions of the requests of a rollout with these `session_meta` payloads, one before
    /// each of its two requests.
    fn versions(metas: [Value; 2]) -> Vec<Option<String>> {
        let mut p = parser("t");
        for (meta, (id, input)) in metas.into_iter().zip([("r1", 100), ("r2", 120)]) {
            p.line(&serde_json::json!({"type": "session_meta", "payload": meta}).to_string());
            p.line(
                &serde_json::json!({"type": "token_usage_record", "payload": {"response_id": id, "usage": {"input_tokens": input, "output_tokens": 1}}})
                    .to_string(),
            );
        }
        p.into_session().chains[0].requests.iter().map(|r| r.version.clone()).collect()
    }

    #[test]
    fn every_request_of_a_rollout_has_its_first_cli_version() {
        let meta = |v: Option<&str>| serde_json::json!({"id": "t", "cwd": "/w", "cli_version": v});
        let one = Some("0.166.0-alpha.5.1".to_string());
        assert_eq!(versions([meta(Some("0.166.0-alpha.5.1")), serde_json::json!({"id": "t", "cwd": "/w"})]), [one.clone(), one.clone()], "a Codex rollout");
        assert_eq!(versions([meta(Some("0.166.0-alpha.5.1")), meta(Some("0.167.0"))]), [one.clone(), one], "a later session_meta keeps the first version");
        assert_eq!(
            versions([serde_json::json!({"id": "t", "cwd": "/w"}), serde_json::json!({"id": "t", "cwd": "/w"})]),
            [None, None],
            "a log without a version"
        );
    }

    #[test]
    fn a_request_prompt_excludes_its_own_response() {
        let mut p = parser("t");
        let line = |v: Value| p.line(&v.to_string());
        let mut line = line;
        line(serde_json::json!({"type": "session_meta", "payload": {"id": "t", "cwd": "/w/p", "base_instructions": {"text": "be good"}}}));
        line(
            serde_json::json!({"type": "response_item", "payload": {"type": "message", "role": "user", "content": [{"type": "input_text", "text": "fix it"}]}}),
        );
        line(serde_json::json!({"type": "response_item", "payload": {"type": "reasoning", "encrypted_content": "abcd"}}));
        line(
            serde_json::json!({"type": "response_item", "payload": {"type": "function_call", "name": "exec_command", "arguments": "{\"cmd\":\"ls -la\"}", "call_id": "c1"}}),
        );
        line(
            serde_json::json!({"type": "token_usage_record", "payload": {"response_id": "r1", "usage": {"input_tokens": 100, "cached_input_tokens": 0, "output_tokens": 10}}}),
        );
        line(serde_json::json!({"type": "response_item", "payload": {"type": "function_call_output", "call_id": "c1", "output": "a b c"}}));
        line(
            serde_json::json!({"type": "token_usage_record", "payload": {"response_id": "r2", "usage": {"input_tokens": 120, "cached_input_tokens": 100, "output_tokens": 5}}}),
        );
        let s = p.into_session();
        let reqs = &s.chains[0].requests;
        assert_eq!(reqs.len(), 2);
        let (d1, d2) = (reqs[0].prompt.as_ref().unwrap(), reqs[1].prompt.as_ref().unwrap());
        assert_eq!((d1.keep, d1.tail.len()), (0, 1), "first prompt: the user message only");
        assert_eq!((d2.keep, d2.tail.len()), (1, 3), "then reasoning, the call and its output");
        let result = &s.blocks[s.messages[d2.tail[2]].blocks.clone()][0];
        assert_eq!((result.kind, result.tool.as_deref(), result.target.as_deref()), (BlockKind::ToolResult, Some("exec_command"), Some("ls -la")));
        assert!(reqs[0].produced.is_some());
    }

    /// The compaction calls of a rollout: a turn, the call that answers with the summary, the
    /// `compacted` line with these fields, and the request after it.
    fn compaction_calls(compacted: Value) -> Vec<(bool, bool)> {
        use serde_json::json;
        let usage = |id: &str, input: u64, cached: u64| json!({"type": "token_usage_record", "payload": {"response_id": id, "usage": {"input_tokens": input, "cached_input_tokens": cached, "output_tokens": 10}}});
        let mut p = parser("t");
        for line in [
            json!({"type": "response_item", "payload": {"type": "message", "role": "user", "content": [{"type": "input_text", "text": "fix it"}]}}),
            usage("resp_a", 242_713, 240_000),
            json!({"type": "response_item", "payload": {"type": "message", "role": "assistant", "content": [{"type": "output_text", "text": "Another language model started to solve this problem"}]}}),
            usage("resp_b", 232_687, 0),
            json!({"type": "compacted", "payload": compacted}),
            usage("resp_c", 30_311, 17_200),
        ] {
            p.line(&line.to_string());
        }
        p.into_session().chains[0].requests.iter().map(|r| (r.compaction_call, r.compacted_before)).collect()
    }

    #[test]
    fn the_request_a_compacted_line_names_is_its_compaction_call() {
        let calls = compaction_calls(serde_json::json!({"message": "summary", "replacement_history": [], "compaction_response_id": "resp_b"}));
        assert_eq!(calls, [(false, false), (true, false), (false, true)]);
    }

    #[test]
    fn a_compacted_line_without_a_known_id_marks_no_call() {
        let none = [(false, false), (false, false), (false, true)];
        assert_eq!(compaction_calls(serde_json::json!({"message": "summary", "replacement_history": []})), none, "no id, as in 0.152");
        assert_eq!(compaction_calls(serde_json::json!({"replacement_history": [], "compaction_response_id": "resp_z"})), none, "an id no usage line has");
    }

    #[test]
    fn tools_and_context_get_their_class() {
        let block = |kind, name: &str| Block { class: Class::None, kind, chars: 1, tool: Some(name.into()), target: Some(name.into()) };
        let tool = |kind, name| class_of(&block(kind, name));
        assert_eq!(tool(BlockKind::ToolResult, "exec_command"), Class::Tool(ToolClass::Execute));
        assert_eq!(tool(BlockKind::ToolUse, "shell"), Class::Tool(ToolClass::Execute));
        assert_eq!(tool(BlockKind::ToolUse, "apply_patch"), Class::Tool(ToolClass::Edit));
        assert_eq!(tool(BlockKind::ToolResult, "write_stdin"), Class::Tool(ToolClass::ExecuteOutput));
        assert_eq!(tool(BlockKind::ToolUse, "wait"), Class::Tool(ToolClass::ExecuteOutput));
        assert_eq!(tool(BlockKind::ToolUse, "view_image"), Class::Tool(ToolClass::Read));
        assert_eq!(tool(BlockKind::ToolUse, "spawn_agent"), Class::Tool(ToolClass::Subagent));
        assert_eq!(tool(BlockKind::ToolUse, "web_search_call"), Class::Tool(ToolClass::Fetch));
        assert_eq!(tool(BlockKind::ToolUse, "update_plan"), Class::Tool(ToolClass::Think));
        assert_eq!(tool(BlockKind::ToolUse, "exec"), Class::Tool(ToolClass::Execute), "code that calls several tools");
        for subagent in ["wait_agent", "list_agents", "followup_task", "send_message", "interrupt_agent"] {
            assert_eq!(tool(BlockKind::ToolUse, subagent), Class::Tool(ToolClass::Subagent), "{subagent}");
        }
        for goal in ["create_goal", "update_goal", "get_goal"] {
            assert_eq!(tool(BlockKind::ToolUse, goal), Class::Tool(ToolClass::Think), "{goal}");
        }
        for own in ["request_user_input", "request_user_input_async", "sleep"] {
            assert_eq!(tool(BlockKind::ToolUse, own), Class::Tool(ToolClass::Manage), "{own}");
        }
        assert_eq!(tool(BlockKind::ToolUse, "js"), Class::Tool(ToolClass::Other));
        assert_eq!(tool(BlockKind::Reminder, "instructions"), Class::Reminder(ReminderKind::Instructions));
    }

    fn ms(ts: &str) -> i64 {
        parse_ts_ms(Some(&Value::String(ts.to_string()))).unwrap()
    }

    /// The session of a rollout that starts with this `session_meta` payload and has these events.
    fn rollout(meta: Value, events: &[(&str, Value)]) -> Session {
        let mut p = parser("t");
        p.line(&serde_json::json!({"timestamp": "2026-10-31T08:00:00Z", "type": "session_meta", "payload": meta}).to_string());
        for (ts, payload) in events {
            p.line(&serde_json::json!({"timestamp": ts, "type": "event_msg", "payload": payload}).to_string());
        }
        p.into_session()
    }

    fn handoffs(s: &Session) -> Vec<(i64, HandoffKind)> {
        s.handoffs.iter().map(|h| (h.ts_ms, h.kind)).collect()
    }

    /// The events of two turns: one that ended, one the user stopped.
    fn two_turns() -> Vec<(&'static str, Value)> {
        use serde_json::json;
        vec![
            ("2026-10-31T08:00:01Z", json!({"type": "task_started", "turn_id": "u1"})),
            ("2026-10-31T08:01:00Z", json!({"type": "task_complete", "turn_id": "u1", "last_agent_message": "Fixed."})),
            ("2026-10-31T08:02:00Z", json!({"type": "task_started", "turn_id": "u2"})),
            ("2026-10-31T08:03:00Z", json!({"type": "turn_aborted", "turn_id": "u2", "reason": "interrupted"})),
        ]
    }

    #[test]
    fn a_main_thread_hands_off_when_a_turn_completes_or_is_aborted() {
        let s = rollout(serde_json::json!({"id": "t", "cwd": "/w/p", "originator": "codex-tui"}), &two_turns());
        use HandoffKind::*;
        assert_eq!(handoffs(&s), [(ms("2026-10-31T08:01:00Z"), TurnEnd), (ms("2026-10-31T08:03:00Z"), Interrupt)]);
        // An event without a time hands off nothing.
        let mut p = parser("t");
        p.line(&serde_json::json!({"type": "event_msg", "payload": {"type": "task_complete"}}).to_string());
        assert!(p.into_session().handoffs.is_empty());
    }

    #[test]
    fn a_subagent_thread_hands_off_nothing() {
        let meta = serde_json::json!({"id": "c", "parent_thread_id": "p", "cwd": "/w/p", "source": {"subagent": {"thread_spawn": {"parent_thread_id": "p", "agent_role": "explorer"}}}});
        let s = rollout(meta, &two_turns());
        assert!(s.handoffs.is_empty(), "{:?}", s.handoffs);
    }

    #[test]
    fn a_thread_that_codex_exec_ran_is_scripted() {
        let scripted = |meta: Value| rollout(meta, &[]).scripted;
        assert!(scripted(serde_json::json!({"id": "t", "cwd": "/w/p", "originator": "codex_exec"})));
        assert!(!scripted(serde_json::json!({"id": "t", "cwd": "/w/p", "originator": "codex-tui"})));
        assert!(!scripted(serde_json::json!({"id": "t", "cwd": "/w/p", "originator": "Codex Desktop"})));
        assert!(!scripted(serde_json::json!({"id": "t", "cwd": "/w/p"})));
    }

    /// The record of the first call of this tool.
    fn call_record<'a>(s: &'a Session, tool: &str) -> &'a CallRecord {
        let b = s.blocks.iter().position(|b| b.kind == BlockKind::ToolUse && b.tool.as_deref() == Some(tool)).unwrap();
        s.call(b).unwrap()
    }

    #[test]
    fn calls_start_at_their_item_and_end_at_their_output() {
        use serde_json::json;
        let item = |ts: &str, payload: Value| json!({"type": "response_item", "timestamp": ts, "payload": payload});
        let usage = |ts: &str, id: &str| json!({"type": "token_usage_record", "timestamp": ts, "payload": {"response_id": id, "usage": {"input_tokens": 100, "output_tokens": 1}}});
        let mut p = parser("t");
        for l in [
            item("2026-11-19T10:00:00Z", json!({"type": "message", "role": "user", "content": [{"type": "input_text", "text": "go"}]})),
            item(
                "2026-11-19T10:00:04Z",
                json!({"type": "function_call", "name": "exec_command", "call_id": "d", "arguments": "{\"cmd\":\"cargo test\\nls\"}"}),
            ),
            item("2026-11-19T10:00:04Z", json!({"type": "web_search_call", "status": "completed", "action": {"type": "search", "query": "codex rollout"}})),
            item(
                "2026-11-19T10:00:04Z",
                json!({"type": "custom_tool_call", "name": "apply_patch", "call_id": "e", "input": "*** Begin Patch\n*** Update File: /w/p/src/a.rs\n@@"}),
            ),
            item("2026-11-19T10:00:04Z", json!({"type": "function_call", "name": "request_user_input_async", "call_id": "q", "arguments": "{}"})),
            usage("2026-11-19T10:00:05Z", "r1"),
            item("2026-11-19T10:00:30Z", json!({"type": "function_call_output", "call_id": "d", "output": "ok"})),
            item("2026-11-19T10:00:31Z", json!({"type": "custom_tool_call_output", "call_id": "e", "output": "ok"})),
            item("2026-11-19T10:00:04.026Z", json!({"type": "function_call_output", "call_id": "q", "output": "{\"accepted\":true}"})),
        ] {
            p.line(&l.to_string());
        }
        // A compaction replays the call; its copy keeps the record of the call.
        p.line(
            &json!({"type": "compacted", "timestamp": "2026-11-19T10:05:00Z", "payload": {"replacement_history": [
                {"type": "function_call", "name": "exec_command", "call_id": "d", "arguments": "{\"cmd\":\"cargo test\"}"},
                {"type": "function_call", "name": "exec_command", "call_id": "old", "arguments": "{\"cmd\":\"git log\"}"},
                {"type": "function_call_output", "call_id": "old", "output": "x"}]}})
            .to_string(),
        );
        let s = p.into_session();
        let exec = call_record(&s, "exec_command");
        assert_eq!((exec.start_ms, exec.end_ms, exec.duration_ms()), (Some(ms("2026-11-19T10:00:04Z")), Some(ms("2026-11-19T10:00:30Z")), Some(26_000)));
        assert!(exec.done && exec.timed() && !exec.background);
        assert_eq!(exec.input.as_deref(), Some("cargo test"));
        let web = call_record(&s, "web_search_call");
        assert!(web.done && web.start_ms.is_some() && web.end_ms.is_none() && !web.timed());
        assert_eq!(web.input.as_deref(), Some("codex rollout"));
        let patch = call_record(&s, "apply_patch");
        assert_eq!((patch.duration_ms(), patch.input.as_deref()), (Some(27_000), Some("/w/p/src/a.rs")));
        let ask = call_record(&s, "request_user_input_async");
        assert!(ask.asks_user && !ask.timed());
        let replayed: Vec<&CallRecord> = (0..s.blocks.len())
            .filter(|&b| s.blocks[b].kind == BlockKind::ToolUse && s.blocks[b].tool.as_deref() == Some("exec_command"))
            .filter_map(|b| s.call(b))
            .collect();
        assert!(replayed.contains(&exec) && replayed.iter().any(|c| c.start_ms.is_none() && c.end_ms.is_none() && c.done), "{replayed:?}");
    }

    #[test]
    fn input_heads_of_codex_calls() {
        let args = |cmd: &str| serde_json::json!({ "cmd": cmd });
        assert_eq!(call_input("exec_command", &args("cargo test -p a\necho"), Some("cargo test")).as_deref(), Some("cargo test -p a"));
        assert_eq!(call_input("spawn_agent", &serde_json::json!({"agent_type": "explorer"}), Some("explorer")).as_deref(), Some("explorer"));
        assert_eq!(call_input("write_stdin", &serde_json::json!({"chars": "y"}), None), None);
        let home = Path::new("/h");
        assert_eq!(
            code_input("exec_command", "text(await tools.exec_command({cmd: \"git status\"}))", Some("git status"), None, home).as_deref(),
            Some("git status")
        );
        assert_eq!(code_input("apply_patch", "*** Update File: /w/p/a.rs\n", None, Some("/w/p"), home).as_deref(), Some("a.rs"));
        assert_eq!(code_input("js", "1 + 1", None, None, home), None);
    }
}
