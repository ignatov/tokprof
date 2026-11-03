//! Claude Code transcripts: `~/.claude/projects/<project>/<session>.jsonl`, with subagents in
//! `<session>/subagents/agent-<id>.jsonl`.
//!
//! Since 2.1.300 Claude Code also writes request-audit records into the transcript:
//! - `api-request-shape`: tools and system prompt, keyed by `shapeHash`;
//! - `api-request-blob`: messages that cannot be derived from transcript lines, keyed by the
//!   SHA-256 of their canonical JSON;
//! - `api-request`: per request, `keep` messages of the previous request of the same
//!   (agentId, querySource) stream plus the `tail` message hashes, the shape and parameters.
//!
//! Assistant lines point at their request with `requestRef`. Messages without a blob are found
//! by hashing transcript `user` messages and `assistant` messages merged by message id. Older
//! transcripts have no request records; their requests keep `prompt: None`.

use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::PathBuf;
use std::sync::Arc;

use serde_json::{Value, json};
use tokprof_core::api::ApiId;
use tokprof_core::labels::{display_path, message_label};
use tokprof_core::model::{Block, BlockKind, Class, CompactionTrigger, Origin, ReminderKind, ToolClass, Chain, ChainKind, Compaction, Params, PromptDelta, Request, Session, Shape, TypedMessage, Usage};
use tokprof_core::source::{AgentId, Decoder};
use tokprof_core::trigger::{CallClass, Pending, Trigger};
use tokprof_logio::{canonical_hash, canonical_string, json_len, parse_ts_ms, str_at, u64_at};

use crate::tools::{call_class, tool_target};

/// Size an image is counted as, in characters (roughly 1,400 tokens).
const IMAGE_CHARS: u64 = 5_000;

/// How Claude Code's notices after an idle compaction start. The terminal compacts during the
/// break and writes "Compacted while idle, before the prompt cache expired · turn off with …"
/// (seen in 2.1.302–2.1.310). A session without the terminal UI swaps in a summary written during
/// the break at the next turn and writes "Compacted using a summary written while you were away ·
/// turn off with …" (found in the 2.1.316 binary, not yet seen in a log).
const IDLE_NOTICES: [&str; 2] = ["Compacted while idle", "Compacted using a summary written while you were away"];

/// What `subagents/agent-<id>.meta.json` tells about a subagent.
#[derive(Clone, Debug, Default)]
pub(crate) struct AgentMeta {
    pub(crate) agent_type: Option<String>,
    /// The few words the parent agent described the task with.
    pub(crate) description: Option<String>,
    /// The parent's tool call that started the subagent.
    pub(crate) tool_use_id: Option<String>,
}

#[derive(Clone)]
struct ApiRequest {
    id: String,
    stream: (String, String),
    ts_ms: i64,
    version: Option<String>,
    keep: usize,
    tail: Vec<String>,
    shape: Option<String>,
    params: Params,
}

#[derive(Clone)]
struct Response {
    ts_ms: i64,
    model: String,
    usage: Value,
    message_id: Option<String>,
    version: Option<String>,
    agent_id: String,
    sidechain: bool,
    request_ref: Option<String>,
    /// The skill, MCP server or plugin Claude Code attributes the response to, or for the main
    /// conversation the origin of the turn it answers.
    origin: Option<String>,
    /// What the chain logged since its previous response.
    trigger: Option<Trigger>,
    activity: Option<CallClass>,
    /// The `entrypoint` of the response's line: the program that ran Claude Code.
    client: Option<Arc<str>>,
}

/// Reads a Claude Code transcript, one line at a time.
#[derive(Clone)]
pub(crate) struct Parser {
    session: Session,
    home: PathBuf,
    subagent_file: bool,
    /// For a subagent transcript: what its meta file says.
    pub(crate) agent: Option<AgentMeta>,
    /// For a main transcript: what its subagents' meta files say, by agent id.
    pub(crate) metas: HashMap<String, AgentMeta>,
    /// Tool calls that started a subagent, with what their response was attributed to.
    launched_by: HashMap<String, String>,
    agent_origins: HashMap<String, Option<String>>,
    /// Origin of the work the latest user message of the main conversation started.
    turn: Option<String>,
    /// The latest title, and whether the user gave it; the user's own wins.
    title: Option<(String, bool)>,
    msg_by_hash: HashMap<String, usize>,
    msg_by_assistant_id: HashMap<String, usize>,
    tools_by_id: HashMap<String, (String, Option<String>)>,
    shapes_by_hash: HashMap<String, usize>,
    requests: Vec<ApiRequest>,
    responses: Vec<Response>,
    response_by_key: HashMap<String, usize>,
    assistant: Option<(String, Vec<Value>)>,
    /// `compact_boundary` lines: time and `compactMetadata.trigger`.
    compactions: Vec<(i64, Option<String>)>,
    /// Times of the notices Claude Code writes after an idle compaction, in either mode.
    idle_notices: Vec<i64>,
    /// Between a `compact_boundary` and the next response, where an idle notice can come.
    after_boundary: bool,
    /// Per chain (agent id, sidechain): what it logged since its latest response.
    pending: HashMap<(String, bool), Pending>,
    /// The trigger and activity each response's message id was first seen with.
    trigger_by_message: HashMap<String, (Option<Trigger>, Option<CallClass>)>,
    /// The highest `totalCostUSD` of each run of Claude Code's counter, keyed by `startTime`. A resumed
    /// process restores the counter with its `startTime`; a new `startTime` is a count from zero.
    counter_runs: Vec<(Option<i64>, f64)>,
    /// The latest `entrypoint` value, shared by the responses that repeat it.
    client: Option<Arc<str>>,
}

impl Parser {
    pub(crate) fn new(agent: AgentId, fallback_id: String, subagent_file: bool, home: PathBuf) -> Parser {
        Parser {
            session: Session::new(agent, fallback_id),
            home,
            subagent_file,
            agent: None,
            metas: HashMap::new(),
            launched_by: HashMap::new(),
            agent_origins: HashMap::new(),
            turn: None,
            title: None,
            msg_by_hash: HashMap::new(),
            msg_by_assistant_id: HashMap::new(),
            tools_by_id: HashMap::new(),
            shapes_by_hash: HashMap::new(),
            requests: Vec::new(),
            responses: Vec::new(),
            response_by_key: HashMap::new(),
            assistant: None,
            compactions: Vec::new(),
            idle_notices: Vec::new(),
            after_boundary: false,
            pending: HashMap::new(),
            trigger_by_message: HashMap::new(),
            counter_runs: Vec::new(),
            client: None,
        }
    }

    fn line(&mut self, line: &str) {
        let interesting = line.contains("\"type\":\"assistant\"")
            || line.contains("\"type\":\"user\"")
            || line.contains("\"type\":\"api-request")
            || line.contains("\"compact_boundary\"")
            // An idle notice only follows a boundary, so other lines need no extra search.
            || (self.after_boundary && IDLE_NOTICES.iter().any(|n| line.contains(n)))
            || line.contains("\"type\":\"cost-state\"")
            || line.contains("\"type\":\"ai-title\"")
            || line.contains("\"type\":\"custom-title\"");
        if !interesting {
            return;
        }
        let Ok(v) = serde_json::from_str::<Value>(line) else { return };
        match str_at(&v, "type") {
            Some("user") => self.user(&v),
            Some("assistant") => {
                self.after_boundary = false;
                self.assistant(&v)
            }
            Some("api-request") => self.api_request(&v),
            Some("api-request-blob") => {
                if let (Some(hash), Some(msg)) = (str_at(&v, "hash"), v.get("message"))
                    && !self.msg_by_hash.contains_key(hash) {
                        let idx = self.add_message(msg, false);
                        self.msg_by_hash.insert(hash.to_string(), idx);
                    }
            }
            Some("api-request-shape") => {
                if let (Some(hash), Some(shape)) = (str_at(&v, "shapeHash"), v.get("shape"))
                    && !self.shapes_by_hash.contains_key(hash) {
                        let idx = self.add_shape(shape);
                        self.shapes_by_hash.insert(hash.to_string(), idx);
                    }
            }
            Some("system") if str_at(&v, "subtype") == Some("compact_boundary") => {
                if let Some(ts) = parse_ts_ms(v.get("timestamp")) {
                    let trigger = v.get("compactMetadata").and_then(|m| str_at(m, "trigger")).map(str::to_string);
                    self.compactions.push((ts, trigger));
                    self.after_boundary = true;
                }
            }
            Some("system") if str_at(&v, "subtype") == Some("informational")
                && str_at(&v, "content").is_some_and(|c| IDLE_NOTICES.iter().any(|n| c.starts_with(n))) => {
                if let Some(ts) = parse_ts_ms(v.get("timestamp")) {
                    self.idle_notices.push(ts);
                }
            }
            Some("cost-state") => {
                if let Some(usd) = v.get("totalCostUSD").and_then(Value::as_f64) {
                    let start = v.get("startTime").and_then(Value::as_i64);
                    match self.counter_runs.iter_mut().find(|(s, _)| *s == start) {
                        Some((_, top)) => *top = top.max(usd),
                        None => self.counter_runs.push((start, usd)),
                    }
                    self.session.reported_usd = Some(self.counter_runs.iter().map(|(_, usd)| usd).sum());
                }
            }
            Some("ai-title") => self.note_title(str_at(&v, "aiTitle"), false),
            Some("custom-title") => self.note_title(str_at(&v, "customTitle"), true),
            _ => {}
        }
    }

    fn note_title(&mut self, title: Option<&str>, custom: bool) {
        let Some(title) = title.map(|t| t.trim().trim_start_matches("[archived]").trim()).filter(|t| !t.is_empty()) else { return };
        if custom || !self.title.as_ref().is_some_and(|(_, c)| *c) {
            self.title = Some((title.to_string(), custom));
        }
    }

    fn note_meta(&mut self, v: &Value) {
        if let Some(id) = str_at(v, "sessionId")
            && self.session.id != id && self.responses.is_empty() && self.requests.is_empty() {
                self.session.id = id.to_string();
            }
        if self.session.cwd.is_none()
            && let Some(cwd) = str_at(v, "cwd") {
                self.session.cwd = Some(cwd.to_string());
                self.session.project = display_path(cwd, None, &self.home);
            }
    }

    fn user(&mut self, v: &Value) {
        self.note_meta(v);
        let Some(msg) = v.get("message") else { return };
        let meta = v.get("isMeta").and_then(Value::as_bool).unwrap_or(false);
        let hash = canonical_hash(msg);
        if !self.msg_by_hash.contains_key(&hash) {
            let idx = self.add_message(msg, meta);
            self.msg_by_hash.insert(hash, idx);
        }
        let sidechain = v.get("isSidechain").and_then(Value::as_bool).unwrap_or(false);
        if !meta
            && let Some(event) = prompt_text(msg).and_then(user_event) {
                let chain = self.chain_key(v);
                self.pending.entry(chain).or_default().event(event);
            }
        if !meta && !sidechain && !self.subagent_file
            && let Some(origin) = prompt_text(msg).and_then(|t| self.turn_origin(t)) {
                self.turn = Some(origin);
            }
        let main = !meta && !sidechain && !self.subagent_file && str_at(v, "agentId").is_none_or(str::is_empty);
        if main
            && let Some(text) = prompt_text(msg).filter(|t| user_event(t) == Some(Trigger::Prompt)) {
                let ts_ms = parse_ts_ms(v.get("timestamp")).unwrap_or(0);
                let label = typed_label(text);
                if self.session.typed.last().is_none_or(|m| m.ts_ms != ts_ms || m.label != label) {
                    self.session.typed.push(TypedMessage { ts_ms, label });
                }
            }
    }

    /// The chain a line belongs to: its agent id and whether it is a sidechain. A subagent's
    /// own transcript is one chain.
    fn chain_key(&self, v: &Value) -> (String, bool) {
        if self.subagent_file {
            return (String::new(), false);
        }
        (str_at(v, "agentId").unwrap_or_default().to_string(), v.get("isSidechain").and_then(Value::as_bool).unwrap_or(false))
    }

    /// Takes what the chain logged since its previous response when a response first shows up,
    /// then records the calls of each of its lines.
    fn note_calls(&mut self, v: &Value, message_id: Option<&str>, model: &str, content: &[Value]) {
        if model.is_empty() || model == "<synthetic>" {
            return;
        }
        let chain = self.chain_key(v);
        let pending = self.pending.entry(chain).or_default();
        if let Some(id) = message_id
            && !self.trigger_by_message.contains_key(id) {
                self.trigger_by_message.insert(id.to_string(), (pending.trigger(), pending.activity()));
                *pending = Pending::default();
            }
        for item in content.iter().filter(|i| str_at(i, "type") == Some("tool_use")) {
            pending.call(call_class(str_at(item, "name").unwrap_or_default(), item.get("input").unwrap_or(&Value::Null)));
        }
    }

    /// What a user message starts: a slash command, the follow-up of a background task, or a
    /// prompt. `None` for a message that carries on the work in progress: an interruption, the
    /// summary that continues a compacted conversation, a shell command run from the prompt.
    fn turn_origin(&mut self, text: &str) -> Option<String> {
        let t = text.trim_start();
        if is_reminder(t) || ["[Request interrupted", "This session is being continued", "<bash-"].iter().any(|p| t.starts_with(p)) {
            return None;
        }
        if t.starts_with("<command-") && let Some(name) = tag_text(t, "command-name") {
            return Some(format!("command:/{}", name.trim_start_matches('/')));
        }
        if t.starts_with("<task-notification>") {
            let agent = tag_text(t, "task-id").and_then(|id| self.agent_origin(id));
            return Some(agent.unwrap_or_else(|| "notification".to_string()));
        }
        Some("prompt".to_string())
    }

    /// The origin of a background subagent's work, by its task id: what the response that
    /// started it was attributed to, or else the subagent's type, both from its meta file.
    fn agent_origin(&mut self, id: &str) -> Option<String> {
        if !self.agent_origins.contains_key(id) {
            if !id.chars().all(|c| c.is_ascii_alphanumeric()) {
                return None;
            }
            let meta = self.metas.get(id).cloned();
            let origin = meta.and_then(|m| {
                let launcher = m.tool_use_id.as_ref().and_then(|t| self.launched_by.get(t).cloned());
                launcher.or_else(|| m.agent_type.map(|ty| format!("agent:{ty}")))
            });
            self.agent_origins.insert(id.to_string(), origin);
        }
        self.agent_origins.get(id).cloned().flatten()
    }

    fn assistant(&mut self, v: &Value) {
        self.note_meta(v);
        let Some(msg) = v.get("message") else { return };
        let message_id = str_at(msg, "id").map(str::to_string);
        let content: Vec<Value> = msg.get("content").and_then(Value::as_array).cloned().unwrap_or_default();
        if let Some(origin) = attribution(v) {
            let launches = content.iter().filter(|i| str_at(i, "type") == Some("tool_use") && matches!(str_at(i, "name"), Some("Agent" | "Task")));
            for id in launches.filter_map(|i| str_at(i, "id")) {
                self.launched_by.insert(id.to_string(), origin.clone());
            }
        }

        // A response is written one content block per line; requests send it back merged.
        let same = matches!((&self.assistant, &message_id), (Some((cur, _)), Some(id)) if cur == id);
        if !same {
            self.finish_assistant();
            if let Some(id) = &message_id {
                self.assistant = Some((id.clone(), Vec::new()));
            }
        }
        if let Some((_, acc)) = &mut self.assistant {
            acc.extend(content.iter().cloned());
        }
        let single = json!({"role": "assistant", "content": content});
        let hash = canonical_hash(&single);
        if !self.msg_by_hash.contains_key(&hash) {
            let idx = self.add_message(&single, false);
            self.msg_by_hash.insert(hash, idx);
        }

        let model = str_at(msg, "model").unwrap_or_default();
        self.note_calls(v, message_id.as_deref(), model, &content);
        let Some(usage) = msg.get("usage") else { return };
        if model.is_empty() || model == "<synthetic>" {
            return;
        }
        let request_ref = str_at(v, "requestRef").map(str::to_string);
        let key = request_ref
            .clone()
            .or_else(|| str_at(v, "requestId").map(str::to_string))
            .or_else(|| message_id.clone());
        let Some(key) = key else { return };
        if let Some(&i) = self.response_by_key.get(&key) {
            self.responses[i].usage = usage.clone();
            return;
        }
        self.response_by_key.insert(key, self.responses.len());
        let agent_id = str_at(v, "agentId").unwrap_or_default().to_string();
        let sidechain = v.get("isSidechain").and_then(Value::as_bool).unwrap_or(false);
        let main = !self.subagent_file && agent_id.is_empty() && !sidechain;
        let origin = attribution(v).or_else(|| if main { self.turn.clone() } else { None });
        let (trigger, activity) = message_id.as_ref().and_then(|id| self.trigger_by_message.get(id).copied()).unwrap_or_default();
        let client = self.client_of(v);
        self.responses.push(Response {
            ts_ms: parse_ts_ms(v.get("timestamp")).unwrap_or(0),
            model: model.to_string(),
            usage: usage.clone(),
            message_id,
            version: str_at(v, "version").map(str::to_string),
            agent_id,
            sidechain,
            request_ref,
            origin,
            trigger,
            activity,
            client,
        });
    }

    /// The `entrypoint` of a line, reusing the previous string while the value repeats.
    fn client_of(&mut self, v: &Value) -> Option<Arc<str>> {
        let name = str_at(v, "entrypoint")?;
        if self.client.as_deref() != Some(name) {
            self.client = Some(Arc::from(name));
        }
        self.client.clone()
    }

    fn finish_assistant(&mut self) {
        let Some((id, content)) = self.assistant.take() else { return };
        let merged = json!({"role": "assistant", "content": content});
        let hash = canonical_hash(&merged);
        let idx = match self.msg_by_hash.get(&hash) {
            Some(&idx) => idx,
            None => {
                let idx = self.add_message(&merged, false);
                self.msg_by_hash.insert(hash, idx);
                idx
            }
        };
        self.msg_by_assistant_id.insert(id, idx);
    }

    fn api_request(&mut self, v: &Value) {
        let Some(id) = str_at(v, "id") else { return };
        let params = v.get("params").cloned().unwrap_or(Value::Null);
        let betas = params.get("betas").and_then(Value::as_array).map(|b| {
            let mut b: Vec<&str> = b.iter().filter_map(Value::as_str).collect();
            b.sort_unstable();
            b.join(",")
        });
        self.requests.push(ApiRequest {
            id: id.to_string(),
            stream: (
                str_at(v, "agentId").unwrap_or_default().to_string(),
                str_at(v, "querySource").unwrap_or_default().to_string(),
            ),
            ts_ms: parse_ts_ms(v.get("timestamp")).unwrap_or(0),
            version: str_at(v, "version").map(str::to_string),
            keep: u64_at(v, "keep") as usize,
            tail: v
                .get("tail")
                .and_then(Value::as_array)
                .map(|t| t.iter().filter_map(Value::as_str).map(str::to_string).collect())
                .unwrap_or_default(),
            shape: str_at(v, "shapeHash").map(str::to_string),
            params: Params {
                effort: params.pointer("/output_config/effort").and_then(Value::as_str).map(str::to_string),
                settings: vec![
                    ("thinking", params.get("thinking").map(canonical_string)),
                    ("speed", str_at(&params, "speed").map(str::to_string)),
                    ("betas", betas),
                    ("context management", params.get("context_management").map(canonical_string)),
                ],
            },
        });
    }

    fn add_shape(&mut self, shape: &Value) -> usize {
        let cwd = self.session.cwd.clone();
        let start = self.session.blocks.len();
        let mut tool_names = Vec::new();
        for tool in shape.get("tools").and_then(Value::as_array).into_iter().flatten() {
            let name = str_at(tool, "name").unwrap_or("?").to_string();
            let chars = name.len() as u64
                + str_at(tool, "description").map_or(0, |d| d.len() as u64)
                + tool.get("input_schema").map_or(0, json_len);
            self.session.blocks.push(Block {
                class: Class::None,
                kind: BlockKind::Tools,
                chars,
                target: tool_target(&name, &Value::Null, cwd.as_deref(), &self.home),
                tool: Some(name.clone()),
            });
            tool_names.push(name);
        }
        let mut system_chars = 0;
        for block in shape.get("system").and_then(Value::as_array).into_iter().flatten() {
            let chars = str_at(block, "text").map_or(0, |t| t.len() as u64);
            system_chars += chars;
            self.session.blocks.push(Block { class: Class::None, kind: BlockKind::System, chars, tool: None, target: None });
        }
        let mut stripped = shape.clone();
        strip_cache_control(&mut stripped);
        let mut hasher = DefaultHasher::new();
        canonical_string(&stripped).hash(&mut hasher);
        self.session.shapes.push(Shape {
            blocks: start..self.session.blocks.len(),
            fingerprint: hasher.finish(),
            tool_names,
            system_chars,
        });
        self.session.shapes.len() - 1
    }

    fn add_message(&mut self, msg: &Value, meta: bool) -> usize {
        let user = str_at(msg, "role") != Some("assistant");
        // Since 2.1.300 request-audit blobs carry Claude Code's reminders (environment, hook
        // context, file-change notes) as messages of role `system`, without reminder tags. One such
        // message joins the reminders of a turn, so it is split into them.
        let system = str_at(msg, "role") == Some("system");
        let meta = meta || system;
        let cwd = self.session.cwd.clone();
        let mut blocks = Vec::new();
        let (mut prompt, mut tool_result) = (false, false);
        let text = |blocks: &mut Vec<Block>, s: &str, prompt: &mut bool| {
            let (kind, target) = if !user {
                (BlockKind::Text, None)
            } else if system {
                for (chars, label) in reminder_parts(s) {
                    blocks.push(Block { class: Class::None, kind: BlockKind::Reminder, chars, tool: None, target: Some(label.to_string()) });
                }
                return;
            } else if meta || is_reminder(s) {
                (BlockKind::Reminder, Some(reminder_label(s).to_string()))
            } else {
                *prompt = true;
                (BlockKind::Prompt, None)
            };
            blocks.push(Block { class: Class::None, kind, chars: s.len() as u64, tool: None, target });
        };
        match msg.get("content") {
            Some(Value::String(s)) => text(&mut blocks, s, &mut prompt),
            Some(Value::Array(items)) => {
                for item in items {
                    let ty = str_at(item, "type").unwrap_or_default();
                    match ty {
                        "text" => text(&mut blocks, str_at(item, "text").unwrap_or_default(), &mut prompt),
                        "thinking" | "redacted_thinking" => {
                            let chars = str_at(item, "thinking").map_or_else(
                                || str_at(item, "data").map_or(0, |d| d.len() as u64 / 2),
                                |t| t.len() as u64,
                            );
                            blocks.push(Block { class: Class::None, kind: BlockKind::Thinking, chars, tool: None, target: None });
                        }
                        "tool_use" | "server_tool_use" => {
                            let name = str_at(item, "name").unwrap_or("?").to_string();
                            let input = item.get("input").unwrap_or(&Value::Null);
                            let target = tool_target(&name, input, cwd.as_deref(), &self.home);
                            if let Some(id) = str_at(item, "id") {
                                self.tools_by_id.insert(id.to_string(), (name.clone(), target.clone()));
                            }
                            blocks.push(Block {
                                class: Class::None,
                                kind: BlockKind::ToolUse,
                                chars: name.len() as u64 + json_len(input),
                                tool: Some(name),
                                target,
                            });
                        }
                        _ if ty == "tool_result" || ty.ends_with("_tool_result") => {
                            tool_result = true;
                            let (tool, target) = str_at(item, "tool_use_id")
                                .and_then(|id| self.tools_by_id.get(id).cloned())
                                .map_or((None, None), |(t, g)| (Some(t), g));
                            let chars = result_chars(item.get("content").unwrap_or(&Value::Null));
                            blocks.push(Block { class: Class::None, kind: BlockKind::ToolResult, chars, tool, target });
                        }
                        "image" => blocks.push(Block { class: Class::None, kind: BlockKind::Image, chars: IMAGE_CHARS, tool: None, target: None }),
                        _ => blocks.push(Block { class: Class::None, kind: BlockKind::Unknown, chars: json_len(item), tool: None, target: None }),
                    }
                }
            }
            _ => {}
        }
        self.session.push_message(blocks, user && prompt && !tool_result)
    }

    pub(crate) fn into_session(mut self) -> Session {
        self.finish_assistant();
        let placeholder = Session::new(self.session.agent.clone(), String::new());
        let mut session = std::mem::replace(&mut self.session, placeholder);

        let resolve = |hash: &str, session: &mut Session, map: &mut HashMap<String, usize>| match map.get(hash) {
            Some(&idx) => idx,
            None => {
                let idx = session.push_unresolved_message();
                map.insert(hash.to_string(), idx);
                idx
            }
        };

        let mut chains: Vec<Chain> = Vec::new();
        let mut chain_by_key: HashMap<(String, String), usize> = HashMap::new();
        let mut chain_for = |key: (String, String), name: String, kind: ChainKind, chains: &mut Vec<Chain>| {
            *chain_by_key.entry(key).or_insert_with(|| {
                chains.push(Chain { name, kind, requests: Vec::new() });
                chains.len() - 1
            })
        };

        let by_ref: HashMap<&str, usize> = self
            .responses
            .iter()
            .enumerate()
            .filter_map(|(i, r)| r.request_ref.as_deref().map(|id| (id, i)))
            .collect();
        let mut used = vec![false; self.responses.len()];

        for req in &self.requests {
            let (name, kind) = stream_chain(&req.stream.0, &req.stream.1, self.subagent_file);
            let ci = chain_for(req.stream.clone(), name, kind, &mut chains);
            let tail = req.tail.iter().map(|h| resolve(h, &mut session, &mut self.msg_by_hash)).collect();
            let delta = PromptDelta {
                shape: req.shape.as_ref().and_then(|h| self.shapes_by_hash.get(h).copied()),
                keep: req.keep,
                tail,
            };
            let base = Request {
                ts_ms: req.ts_ms,
                params: Some(req.params.clone()),
                version: req.version.clone(),
                ..Request::default()
            };
            match by_ref.get(req.id.as_str()) {
                Some(&ri) => {
                    used[ri] = true;
                    let resp = &self.responses[ri];
                    let produced = resp.message_id.as_ref().and_then(|id| self.msg_by_assistant_id.get(id).copied());
                    let base = Request { origin: origin_of(resp, &chains[ci], self.agent.as_ref()).map(|o| Origin::parse(&o)), trigger: resp.trigger, activity: resp.activity, client: resp.client.clone(), ..base };
                    push_response(&mut chains[ci], base, resp, Some(delta), produced);
                }
                // Keep the request so later deltas still apply; it has no usage to attribute.
                None => chains[ci].requests.push(Request { prompt: Some(delta), ..base }),
            }
        }

        for (ri, resp) in self.responses.iter().enumerate() {
            if used[ri] {
                continue;
            }
            let (name, kind) = if !resp.agent_id.is_empty() || self.subagent_file {
                ("subagent".to_string(), ChainKind::Subagent)
            } else if resp.sidechain {
                ("sidechain".to_string(), ChainKind::Side)
            } else {
                ("main".to_string(), ChainKind::Main)
            };
            let ci = chain_for((resp.agent_id.clone(), format!("legacy:{name}")), name, kind, &mut chains);
            let origin = origin_of(resp, &chains[ci], self.agent.as_ref());
            let base = Request { ts_ms: resp.ts_ms, version: resp.version.clone(), origin: origin.map(|o| Origin::parse(&o)), trigger: resp.trigger, activity: resp.activity, client: resp.client.clone(), ..Request::default() };
            let produced = resp.message_id.as_ref().and_then(|id| self.msg_by_assistant_id.get(id).copied());
            push_response(&mut chains[ci], base, resp, None, produced);
        }

        self.compactions.sort_unstable();
        let marks: Vec<i64> = self.compactions.iter().map(|(t, _)| *t).collect();
        // A main chain that starts after an earlier main chain of its name has ended continues that
        // conversation (see `Session::main_conversations`), so its first request comes after the
        // earlier chain's last answered request, not after every compaction logged before it.
        let span = |c: &Chain| {
            let ts = c.requests.iter().map(|r| r.ts_ms);
            (ts.clone().min().unwrap_or(0), ts.max().unwrap_or(0))
        };
        let mut order: Vec<usize> = (0..chains.len()).collect();
        order.sort_by_key(|&ci| span(&chains[ci]).0);
        let mut ended: Vec<(usize, i64, i64)> = Vec::new(); // (chain, end, its last answered request)
        for ci in order {
            let (first, last) = span(&chains[ci]);
            let continued = ended
                .iter()
                .filter(|(cj, end, _)| {
                    let (a, b) = (&chains[*cj], &chains[ci]);
                    a.kind == ChainKind::Main && b.kind == ChainKind::Main && a.name == b.name && *end < first
                })
                .max_by_key(|(_, end, _)| *end)
                .map(|(_, _, answered)| *answered);
            let mut prev_ts = continued.unwrap_or(i64::MIN);
            for r in &mut chains[ci].requests {
                let start = marks.partition_point(|&t| t <= prev_ts);
                r.compacted_before = marks.get(start).is_some_and(|&t| t <= r.ts_ms);
                // A request that got no response (cancelled, retried) passes the compaction on
                // to the next one.
                if r.usage.prompt() > 0 || r.usage.output > 0 {
                    prev_ts = r.ts_ms;
                }
            }
            ended.push((ci, last, prev_ts));
        }
        session.title = self.title.take().map(|(title, _)| title);
        session.task = self.agent.as_ref().and_then(|a| a.description.clone());
        if let Some(task) = &session.task {
            for chain in chains.iter_mut().filter(|c| c.kind == ChainKind::Subagent) {
                chain.name = format!("subagent: {task}");
            }
        }
        session.compactions = compaction_kinds(&self.compactions, &self.idle_notices, &chains);
        for b in &mut session.blocks {
            b.class = class_of(b);
        }
        // Claude Code sends every request to the Anthropic API.
        for r in chains.iter_mut().flat_map(|c| &mut c.requests) {
            r.api = ApiId::Anthropic;
        }
        session.chains = chains;
        session
    }
}

/// A request's origin: what Claude Code attributes the response to, the subagent's type in a
/// subagent's chain, the chain's name for other side requests, the user's turn otherwise.
/// The transcript's compactions with their kinds. A boundary is `idle` when Claude Code's idle
/// notice comes after it and no later than the next answered request of any chain; otherwise it
/// takes the trigger it logged.
fn compaction_kinds(boundaries: &[(i64, Option<String>)], notices: &[i64], chains: &[Chain]) -> Vec<Compaction> {
    let mut answered: Vec<i64> = chains.iter().flat_map(|c| &c.requests).filter(|r| r.usage.prompt() > 0 || r.usage.output > 0).map(|r| r.ts_ms).collect();
    answered.sort_unstable();
    boundaries
        .iter()
        .map(|(ts, trigger)| {
            let next = answered.get(answered.partition_point(|&t| t <= *ts)).copied().unwrap_or(i64::MAX);
            let idle = notices.iter().any(|&n| n >= *ts && n <= next);
            Compaction { ts_ms: *ts, kind: if idle { CompactionTrigger::Idle } else { CompactionTrigger::logged(trigger.as_deref()) } }
        })
        .collect()
}

fn origin_of(resp: &Response, chain: &Chain, agent: Option<&AgentMeta>) -> Option<String> {
    resp.origin.clone().or_else(|| match chain.kind {
        ChainKind::Subagent => {
            let ty = agent.and_then(|a| a.agent_type.clone()).or_else(|| chain.name.strip_prefix("subagent:").map(str::to_string));
            Some(ty.map_or_else(|| "agent".to_string(), |ty| format!("agent:{ty}")))
        }
        ChainKind::Main => None,
        _ => Some(chain.name.clone()),
    })
}

/// The skill, MCP server or plugin Claude Code attributes a response to.
fn attribution(v: &Value) -> Option<String> {
    [("attributionSkill", "skill"), ("attributionMcpServer", "mcp"), ("attributionPlugin", "plugin")]
        .into_iter()
        .find_map(|(key, kind)| str_at(v, key).filter(|s| !s.is_empty()).map(|s| format!("{kind}:{s}")))
}

/// The text of a user message without tool results: a prompt, a command, a notification.
fn prompt_text(msg: &Value) -> Option<&str> {
    match msg.get("content")? {
        Value::String(s) => Some(s.as_str()),
        Value::Array(items) => {
            let result = |i: &Value| str_at(i, "type").is_some_and(|t| t == "tool_result" || t.ends_with("_tool_result"));
            if items.iter().any(result) {
                return None;
            }
            items.iter().filter(|i| str_at(i, "type") == Some("text")).filter_map(|i| str_at(i, "text")).find(|t| !is_reminder(t))
        }
        _ => None,
    }
}

/// The label of a typed message: a slash command as its name and arguments, anything else by
/// its last paragraph.
fn typed_label(text: &str) -> String {
    let t = text.trim_start();
    if t.starts_with("<command-") && let Some(name) = tag_text(t, "command-name") {
        let args = tag_text(t, "command-args").unwrap_or_default();
        return message_label(&format!("/{} {args}", name.trim_start_matches('/')));
    }
    message_label(text)
}

/// What a user message's text tells about the next request: a typed prompt or command, a
/// background task's result, an interruption. `None` for what carries on the work: reminders,
/// the summary after a compaction, a shell command run from the prompt.
fn user_event(text: &str) -> Option<Trigger> {
    let t = text.trim_start();
    if t.starts_with("[Request interrupted") {
        Some(Trigger::Continuation)
    } else if t.starts_with("<task-notification>") {
        Some(Trigger::Notification)
    } else if is_reminder(t) || ["This session is being continued", "<bash-", "<local-command"].iter().any(|p| t.starts_with(p)) {
        None
    } else {
        Some(Trigger::Prompt)
    }
}

/// The text between `<tag>` and `</tag>`.
fn tag_text<'a>(text: &'a str, tag: &str) -> Option<&'a str> {
    let start = text.find(&format!("<{tag}>"))? + tag.len() + 2;
    let end = start + text[start..].find(&format!("</{tag}>"))?;
    Some(text[start..end].trim()).filter(|s| !s.is_empty())
}

/// Adds the requests of one response. A response that consulted the advisor reports several
/// iterations: executor requests that read the growing prompt, and advisor requests that read
/// the conversation with another model. The top-level usage covers only the executor.
fn push_response(chain: &mut Chain, base: Request, resp: &Response, prompt: Option<PromptDelta>, produced: Option<usize>) {
    let iterations = resp.usage.get("iterations").and_then(Value::as_array).filter(|i| i.len() > 1);
    let Some(iterations) = iterations else {
        chain.requests.push(Request {
            model: resp.model.clone(),
            usage: usage_of(&resp.usage),
            prompt,
            produced,
            ..base
        });
        return;
    };
    // Iterations after the first read the same prompt plus what the response added so far,
    // and the advisor reads the conversation as is: none of them change the chain's prompt.
    let keep_all = || PromptDelta { shape: None, keep: usize::MAX, tail: Vec::new() };
    let has_prompt = prompt.is_some();
    let mut prompt = prompt;
    let last_executor = iterations.iter().rposition(|it| str_at(it, "type") != Some("advisor_message"));
    for (i, it) in iterations.iter().enumerate() {
        let advisor = str_at(it, "type") == Some("advisor_message");
        let mut usage = usage_of(it);
        if Some(i) == last_executor {
            usage.reasoning = usage_of(&resp.usage).reasoning;
        }
        let delta = match (has_prompt, advisor) {
            (false, _) => None,
            (true, true) => Some(keep_all()),
            (true, false) => Some(prompt.take().unwrap_or_else(keep_all)),
        };
        chain.requests.push(Request {
            model: str_at(it, "model").unwrap_or(&resp.model).to_string(),
            usage,
            prompt: delta,
            produced: if Some(i) == last_executor { produced } else { None },
            advisor,
            ..base.clone()
        });
    }
}

fn usage_of(u: &Value) -> Usage {
    let write = u64_at(u, "cache_creation_input_tokens");
    let (short, long) = match u.get("cache_creation") {
        Some(cc) => {
            let (s, l) = (u64_at(cc, "ephemeral_5m_input_tokens"), u64_at(cc, "ephemeral_1h_input_tokens"));
            (s + write.saturating_sub(s + l), l)
        }
        None => (write, 0),
    };
    Usage {
        uncached: u64_at(u, "input_tokens"),
        cache_read: u64_at(u, "cache_read_input_tokens"),
        write_short: short,
        write_long: long,
        output: u64_at(u, "output_tokens"),
        reasoning: u.get("output_tokens_details").map_or(0, |d| u64_at(d, "thinking_tokens")),
    }
}

fn stream_chain(agent_id: &str, source: &str, subagent_file: bool) -> (String, ChainKind) {
    if let Some(rest) = source.strip_prefix("agent:") {
        let ty = rest.rsplit(':').next().unwrap_or(rest);
        return (format!("subagent:{ty}"), ChainKind::Subagent);
    }
    match source {
        "" | "repl_main_thread" | "sdk" | "main" => {
            if agent_id.is_empty() && !subagent_file {
                ("main".to_string(), ChainKind::Main)
            } else {
                ("subagent".to_string(), ChainKind::Subagent)
            }
        }
        s if s.contains("compact") => ("compact".to_string(), ChainKind::Side),
        s => (s.to_string(), ChainKind::Side),
    }
}

/// What injected a reminder, from the opening words Claude Code uses for it.
/// The class of a block's Claude Code tool or reminder.
fn class_of(b: &Block) -> Class {
    match b.kind {
        BlockKind::ToolUse | BlockKind::ToolResult => Class::Tool(match b.tool.as_deref().unwrap_or_default() {
            "Bash" => ToolClass::Execute,
            "BashOutput" => ToolClass::ExecuteOutput,
            "Read" => ToolClass::Read,
            "Write" => ToolClass::Write,
            "Edit" | "MultiEdit" => ToolClass::Edit,
            "WebFetch" | "WebSearch" => ToolClass::Fetch,
            "Grep" | "Glob" => ToolClass::Search,
            _ => ToolClass::Other,
        }),
        BlockKind::Reminder => Class::Reminder(ReminderKind::of_label(b.target.as_deref().unwrap_or_default())),
        _ => Class::None,
    }
}

/// The openings Claude Code gives the reminders it joins into one `system` message, seen in
/// 2.1.300–2.1.316, with the source each opening names.
const OPENINGS: &[(&str, &str)] = &[
    ("[SYSTEM NOTIFICATION", "background notifications"),
    ("<task-notification>", "background notifications"),
    ("Background agent \"", "background notifications"),
    ("The following deferred tools", "deferred tool lists"),
    ("The following tools just became available", "deferred tool lists"),
    ("The following skills are available", "skills"),
    ("The following skills were invoked", "skills"),
    ("Codebase and user instructions are shown below", "instructions"),
    ("Instruction files were re-read", "instructions"),
    ("# MCP Server Instructions\n\nThe following MCP servers", "other"),
    ("# Environment\nYou have been invoked", "other"),
    ("# Advisor Tool\n\nYou have access to an `advisor`", "other"),
    ("You are powered by the model", "other"),
    ("Available agent types", "other"),
    ("While bypass permissions mode is active", "other"),
    ("Today's date is", "other"),
];

/// The source of a reminder whose opening is known.
fn opening_label(text: &str) -> Option<&'static str> {
    if let Some((_, label)) = OPENINGS.iter().find(|(opening, _)| text.starts_with(opening)) {
        return Some(label);
    }
    let line = text.lines().next().unwrap_or_default();
    if (line.starts_with("Called the ") && line.contains(" tool with the following input"))
        || (line.starts_with("Note: ") && line.contains("was read before the last conversation was summarized"))
    {
        Some("attached files")
    } else if line.contains(" hook additional context:") {
        Some("hook context")
    } else {
        None
    }
}

/// A `system` message split into its reminders: a new one starts at each blank line followed by
/// a known opening of another source. Each part has its characters, separators included, and its
/// source.
fn reminder_parts(text: &str) -> Vec<(u64, &'static str)> {
    let mut starts = vec![(0, reminder_label(text))];
    let mut from = 0;
    while let Some(i) = text[from..].find("\n\n") {
        let mut at = from + i + 2;
        while text[at..].starts_with('\n') {
            at += 1;
        }
        if let Some(label) = opening_label(&text[at..])
            && starts.last().is_some_and(|(_, last)| *last != label)
        {
            starts.push((at, label));
        }
        from = at;
    }
    let ends = starts.iter().skip(1).map(|(at, _)| *at).chain([text.len()]);
    starts.iter().zip(ends).map(|((at, label), end)| ((end - at) as u64, *label)).collect()
}

fn reminder_label(text: &str) -> &'static str {
    let body = text.trim_start().trim_start_matches("<system-reminder>").trim_start();
    if let Some(label) = opening_label(body) {
        return label;
    }
    let head: String = body.chars().take(200).collect::<String>().to_ascii_lowercase();
    if head.contains("instruction") || head.contains("claude.md") || head.contains("claudemd") || head.contains("following context") {
        "instructions"
    } else if head.contains("system notification") || head.contains("task-notification") || head.contains("background") {
        "background notifications"
    } else if head.contains("deferred tool") || head.contains("tools are now available") {
        "deferred tool lists"
    } else if head.contains("hook") {
        "hook context"
    } else if head.contains("skill") {
        "skills"
    } else if head.contains("todo") || head.contains("task list") {
        "task list"
    } else if head.contains("called the") || head.contains("file") {
        "attached files"
    } else {
        "other"
    }
}

fn is_reminder(text: &str) -> bool {
    let t = text.trim_start();
    t.starts_with("<system-reminder>") || t.starts_with("<local-command-") || t.starts_with("Caveat:")
}

fn result_chars(content: &Value) -> u64 {
    match content {
        Value::String(s) => s.len() as u64,
        Value::Array(items) => items
            .iter()
            .map(|it| match str_at(it, "type") {
                Some("image") => IMAGE_CHARS,
                Some("text") => str_at(it, "text").map_or(0, |t| t.len() as u64),
                _ => json_len(it),
            })
            .sum(),
        other => json_len(other),
    }
}

fn strip_cache_control(v: &mut Value) {
    match v {
        Value::Object(map) => {
            map.remove("cache_control");
            map.values_mut().for_each(strip_cache_control);
        }
        Value::Array(items) => items.iter_mut().for_each(strip_cache_control),
        _ => {}
    }
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

    fn parser(id: &str, subagent_file: bool) -> Parser {
        Parser::new(crate::ClaudeCode::agent(), id.into(), subagent_file, PathBuf::from("/h"))
    }

    fn session_from(lines: &[Value]) -> Session {
        let mut p = parser("s", false);
        for l in lines {
            p.line(&l.to_string());
        }
        p.into_session()
    }

    fn reported_cost(states: &[(i64, f64)]) -> Option<f64> {
        let lines: Vec<Value> = states.iter()
            .map(|(start, usd)| json!({"type": "cost-state", "startTime": start, "totalCostUSD": usd}))
            .collect();
        session_from(&lines).reported_usd.map(|usd| (usd * 100.0).round() / 100.0)
    }

    #[test]
    fn sums_the_highest_value_of_each_counter_run() {
        assert_eq!(reported_cost(&[(1, 10.0), (1, 30.0), (1, 28.0)]), Some(30.0));
        assert_eq!(reported_cost(&[(1, 24.06), (2, 2.66), (2, 32.89)]), Some(56.95));
        assert_eq!(reported_cost(&[(2, 1.34), (1, 1.41), (2, 24.70)]), Some(26.11));
        assert_eq!(reported_cost(&[]), None);
    }

    #[test]
    fn rebuilds_prompts_from_request_records() {
        let user = json!({"role": "user", "content": "hello"});
        let user_hash = canonical_hash(&user);
        let s = session_from(&[
            json!({"type": "user", "sessionId": "s1", "cwd": "/w/p", "message": user}),
            json!({"type": "api-request-shape", "shapeHash": "sh", "shape": {
                "system": [{"type": "text", "text": "You are", "cache_control": {"type": "ephemeral"}}],
                "tools": [{"name": "Read", "description": "reads", "input_schema": {}}]}}),
            json!({"type": "api-request", "id": "r1", "querySource": "repl_main_thread", "timestamp": "2026-10-01T10:00:00Z",
                   "keep": 0, "tail": [user_hash, "missing"], "shapeHash": "sh", "params": {"output_config": {"effort": "high"}}}),
            json!({"type": "assistant", "requestRef": "r1", "requestId": "q1", "timestamp": "2026-10-01T10:00:05Z",
                   "message": {"id": "m1", "model": "claude-opus-5-5", "content": [{"type": "text", "text": "hi"}],
                   "usage": {"input_tokens": 3, "cache_read_input_tokens": 0, "cache_creation_input_tokens": 1000,
                             "cache_creation": {"ephemeral_5m_input_tokens": 0, "ephemeral_1h_input_tokens": 1000}, "output_tokens": 7}}}),
        ]);
        assert_eq!(s.id, "s1");
        assert_eq!(s.project, "/w/p");
        let chain = &s.chains[0];
        assert_eq!((chain.name.as_str(), chain.kind), ("main", ChainKind::Main));
        let r = &chain.requests[0];
        assert_eq!(r.usage.write_long, 1000);
        assert_eq!(r.params.as_ref().unwrap().effort.as_deref(), Some("high"));
        let delta = r.prompt.as_ref().unwrap();
        assert_eq!(delta.tail.len(), 2);
        assert!(s.messages[delta.tail[0]].resolved && s.messages[delta.tail[0]].starts_turn);
        assert!(!s.messages[delta.tail[1]].resolved);
        assert_eq!(s.shapes[delta.shape.unwrap()].tool_names, vec!["Read"]);
        assert!(r.produced.is_some());
    }

    /// The lines Claude Code 2.1.310 writes around an idle compaction, trimmed to the fields read.
    #[test]
    fn compactions_get_their_kind() {
        let reply = |id: &str, ts: &str| assistant_line(id, json!({"timestamp": ts}), json!([{"type": "text", "text": "ok"}]));
        let boundary = |ts: &str, trigger: &str| json!({"type": "system", "subtype": "compact_boundary", "timestamp": ts,
            "compactMetadata": {"trigger": trigger, "preTokens": 440991, "postTokens": 8728}});
        let notice = |ts: &str| json!({"type": "system", "subtype": "informational", "timestamp": ts, "isMeta": false,
            "content": "Compacted while idle, before the prompt cache expired · turn off with \"idleCompaction\": false in settings.json"});
        let s = session_from(&[
            reply("m1", "2026-10-29T18:18:52Z"),
            boundary("2026-10-29T19:13:39Z", "auto"),
            notice("2026-10-29T19:13:40Z"),
            reply("m2", "2026-10-29T19:19:26Z"),
            boundary("2026-10-29T20:00:00Z", "auto"),
            reply("m3", "2026-10-29T20:00:10Z"),
            notice("2026-10-29T20:01:00Z"),
            boundary("2026-10-29T21:00:00Z", "manual"),
            reply("m4", "2026-10-29T21:00:10Z"),
        ]);
        let kinds: Vec<Option<&str>> = s.compactions.iter().map(|c| c.kind.name()).collect();
        // The notice after m3 comes after the next answered request, so the boundary before m3 stays `auto`.
        assert_eq!(kinds, [Some("idle"), Some("auto"), Some("manual")]);
        let marked: Vec<bool> = s.chains[0].requests.iter().map(|r| r.compacted_before).collect();
        assert_eq!(marked, [false, true, true, true]);
    }

    /// A session that ran without request-audit lines, was compacted, and went on after an upgrade
    /// that added them: its main conversation sits in two chains named `main`.
    #[test]
    fn a_main_chain_that_continues_an_earlier_one_follows_its_last_request() {
        let reply = |id: &str, ts: &str| assistant_line(id, json!({"timestamp": ts}), json!([{"type": "text", "text": "ok"}]));
        let s = session_from(&[
            reply("m1", "2026-10-12T18:06:00Z"),
            json!({"type": "system", "subtype": "compact_boundary", "timestamp": "2026-10-13T15:31:39Z", "compactMetadata": {"trigger": "auto"}}),
            reply("m2", "2026-10-13T15:32:31Z"),
            reply("m3", "2026-10-13T16:00:00Z"),
            json!({"type": "api-request", "id": "r1", "querySource": "sdk", "timestamp": "2026-10-19T15:07:22Z", "keep": 0, "tail": []}),
            assistant_line("m4", json!({"requestRef": "r1", "timestamp": "2026-10-19T15:07:30Z"}), json!([{"type": "text", "text": "ok"}])),
        ]);
        let mut chains: Vec<(i64, Vec<bool>)> = s.chains.iter()
            .filter(|c| c.kind == ChainKind::Main)
            .map(|c| (c.requests[0].ts_ms, c.requests.iter().map(|r| r.compacted_before).collect()))
            .collect();
        chains.sort();
        let marked: Vec<Vec<bool>> = chains.into_iter().map(|(_, m)| m).collect();
        assert_eq!(marked, [vec![false, true, false], vec![false]]);
    }

    /// Claude Code 2.1.316's second idle mode, in the order its binary emits the lines: the summary
    /// written during the break is swapped in at the first turn after the return. Synthetic, since
    /// no log has this notice yet.
    #[test]
    fn an_away_summary_notice_makes_the_compaction_idle() {
        let reply = |id: &str, ts: &str| assistant_line(id, json!({"timestamp": ts}), json!([{"type": "text", "text": "ok"}]));
        let mut prompt = user_line("go on");
        prompt["timestamp"] = json!("2026-11-02T10:02:10Z");
        let mut summary = user_line("This session is being continued from a previous conversation that ran out of context.");
        summary["timestamp"] = json!("2026-11-02T10:02:11Z");
        summary["isCompactSummary"] = json!(true);
        let s = session_from(&[
            reply("m1", "2026-11-02T07:01:40Z"),
            prompt,
            json!({"type": "system", "subtype": "compact_boundary", "timestamp": "2026-11-02T10:02:11Z",
                   "compactMetadata": {"trigger": "auto", "preTokens": 512000, "postTokens": 9100}}),
            summary,
            json!({"type": "system", "subtype": "informational", "level": "notice", "isMeta": false, "timestamp": "2026-11-02T10:02:11Z",
                   "content": "Compacted using a summary written while you were away ·\u{a0}turn off with \"idleCompaction\": false in settings.json"}),
            reply("m2", "2026-11-02T10:02:30Z"),
        ]);
        let kinds: Vec<Option<&str>> = s.compactions.iter().map(|c| c.kind.name()).collect();
        assert_eq!(kinds, [Some("idle")]);
        let marked: Vec<bool> = s.chains[0].requests.iter().map(|r| r.compacted_before).collect();
        assert_eq!(marked, [false, true]);
    }

    fn assistant_line(id: &str, extra: Value, content: Value) -> Value {
        let mut v = json!({"type": "assistant", "requestId": id, "timestamp": "2026-10-01T10:00:05Z",
            "message": {"id": id, "model": "claude-opus-5-5", "content": content, "usage": {"input_tokens": 10, "output_tokens": 1}}});
        if let (Some(line), Value::Object(extra)) = (v.as_object_mut(), extra) {
            line.extend(extra);
        }
        v
    }

    fn user_line(text: &str) -> Value {
        json!({"type": "user", "message": {"role": "user", "content": text}})
    }

    fn results_line(ids: &[&str], agent: Option<&str>) -> Value {
        let items: Vec<Value> = ids.iter().map(|id| json!({"type": "tool_result", "tool_use_id": id, "content": "x"})).collect();
        let mut v = json!({"type": "user", "message": {"role": "user", "content": items}});
        if let Some(a) = agent {
            v["agentId"] = json!(a);
        }
        v
    }

    fn triggers(s: &Session, chain: usize) -> Vec<Trigger> {
        s.chains[chain].requests.iter().map(Request::trigger).collect()
    }

    fn clients(s: &Session) -> Vec<&str> {
        s.chains.iter().flat_map(|c| &c.requests).map(Request::client_name).collect()
    }

    #[test]
    fn each_request_carries_the_entrypoint_of_its_response() {
        // A session started in the terminal and resumed in an IDE through the agent SDK.
        let text = json!([{"type": "text", "text": "ok"}]);
        let s = session_from(&[
            user_line("start"),
            assistant_line("m1", json!({"entrypoint": "cli"}), text.clone()),
            assistant_line("m2", json!({"entrypoint": "cli"}), text.clone()),
            user_line("go on"),
            assistant_line("m3", json!({"entrypoint": "sdk-ts"}), text.clone()),
            assistant_line("m4", json!({}), text),
        ]);
        assert_eq!(clients(&s), ["cli", "cli", "sdk-ts", "unknown"]);
        // Repeated values share one string.
        let r = &s.chains[0].requests;
        assert!(std::sync::Arc::ptr_eq(r[0].client.as_ref().unwrap(), r[1].client.as_ref().unwrap()));
    }

    #[test]
    fn a_subagent_transcript_has_the_client_of_its_lines() {
        let mut p = parser("s", true);
        for l in [user_line("look around"), assistant_line("m1", json!({"entrypoint": "sdk-ts", "agentId": "a1"}), json!([{"type": "text", "text": "ok"}]))] {
            p.line(&l.to_string());
        }
        assert_eq!(clients(&p.into_session()), ["sdk-ts"]);
    }

    #[test]
    fn requests_carry_what_triggered_them() {
        let call = |id: &str, name: &str, input: Value| json!({"type": "tool_use", "id": id, "name": name, "input": input});
        let calls = |id: &str, items: Value| assistant_line(id, json!({}), items);
        let s = session_from(&[
            user_line("fix the parser"),
            calls("m1", json!([call("t1", "Read", json!({"file_path": "src/lib.rs"}))])),
            results_line(&["t1"], None),
            // Parallel calls written one per line, with their results in separate lines.
            calls("m2", json!([call("t2", "Grep", json!({"pattern": "fn"}))])),
            calls("m2", json!([call("t3", "Bash", json!({"command": "git show HEAD:src/lib.rs | head -40"}))])),
            results_line(&["t2"], None),
            results_line(&["t3"], None),
            calls("m3", json!([call("t4", "Bash", json!({"command": "cargo test"})), call("t5", "Edit", json!({"file_path": "src/lib.rs"}))])),
            results_line(&["t4", "t5"], None),
            calls("m4", json!([call("t6", "Bash", json!({"command": "cargo test"}))])),
            results_line(&["t6"], None),
            calls("m5", json!([call("t7", "Agent", json!({"description": "x"}))])),
            results_line(&["t7"], None),
            calls("m6", json!([call("t8", "Read", json!({"file_path": "a"}))])),
            user_line("[Request interrupted by user]"),
            calls("m7", json!([{"type": "text", "text": "ok"}])),
            user_line("[Request interrupted by user]"),
            user_line("now the tests"),
            calls("m8", json!([{"type": "text", "text": "ok"}])),
            user_line("<task-notification>\n<task-id>b7x2k9</task-id>"),
            calls("m9", json!([{"type": "text", "text": "ok"}])),
        ]);
        use Trigger::*;
        assert_eq!(triggers(&s, 0), [Prompt, Lookup, Lookups, Edit, Run, Subagent, Continuation, Prompt, Notification]);
    }

    #[test]
    fn requests_carry_the_activity_and_typed_messages_are_kept() {
        let call = |id: &str, name: &str, input: Value| json!({"type": "tool_use", "id": id, "name": name, "input": input});
        let mut typed = user_line("Tool results take 47%\n  advice ▸ trim\n\nwhat else can we analyze?");
        typed["timestamp"] = json!("2026-10-01T10:00:00Z");
        let mut command = user_line("<command-message>goal</command-message>\n<command-name>/goal</command-name>\n<command-args>finish the parser</command-args>");
        command["timestamp"] = json!("2026-10-01T10:10:00Z");
        let s = session_from(&[
            typed,
            assistant_line("m1", json!({}), json!([call("t1", "Bash", json!({"command": "cargo test"})), call("t2", "Read", json!({"file_path": "a"}))])),
            results_line(&["t1", "t2"], None),
            assistant_line("m2", json!({}), json!([call("t3", "Bash", json!({"command": "cat > notes.md <<'EOF'\nx\nEOF"}))])),
            results_line(&["t3"], None),
            assistant_line("m3", json!({}), json!([{"type": "text", "text": "ok"}])),
            command,
            assistant_line("m4", json!({}), json!([{"type": "text", "text": "ok"}])),
        ]);
        let activities: Vec<&str> = s.chains[0].requests.iter().map(Request::activity).collect();
        assert_eq!(activities, ["prompt", "building and testing", "editing", "prompt"]);
        let labels: Vec<&str> = s.typed.iter().map(|m| m.label.as_str()).collect();
        assert_eq!(labels, ["what else can we analyze?", "/goal finish the parser"]);

        let mut p = parser("agent-a1", true);
        p.line(&user_line("Find the handlers").to_string());
        assert!(p.into_session().typed.is_empty(), "a subagent's brief is not typed by the user");
    }

    #[test]
    fn a_retry_keeps_the_trigger_of_the_failed_request() {
        let call = json!([{"type": "tool_use", "id": "t1", "name": "Read", "input": {"file_path": "a"}}]);
        let mut failed = assistant_line("e1", json!({"isApiErrorMessage": true}), json!([{"type": "text", "text": "API Error"}]));
        failed["message"]["model"] = json!("<synthetic>");
        let s = session_from(&[
            user_line("go"),
            assistant_line("m1", json!({}), call),
            results_line(&["t1"], None),
            failed,
            assistant_line("m2", json!({}), json!([{"type": "text", "text": "ok"}])),
        ]);
        assert_eq!(triggers(&s, 0), [Trigger::Prompt, Trigger::Lookup]);
    }

    #[test]
    fn a_subagent_starts_from_its_brief_and_legacy_transcripts_get_triggers() {
        // A subagent's own transcript, without request-audit lines.
        let mut p = parser("agent-a1", true);
        for l in [
            user_line("Find the handlers"),
            assistant_line("m1", json!({}), json!([{"type": "tool_use", "id": "t1", "name": "Glob", "input": {"pattern": "*.rs"}}])),
            results_line(&["t1"], None),
            assistant_line("m2", json!({}), json!([{"type": "text", "text": "done"}])),
        ] {
            p.line(&l.to_string());
        }
        let s = p.into_session();
        assert!(s.chains[0].requests.iter().all(|r| r.prompt.is_none()), "a legacy transcript");
        assert_eq!(triggers(&s, 0), [Trigger::Prompt, Trigger::Lookup]);
    }

    #[test]
    fn system_messages_are_reminders_and_start_no_turn() {
        let mut p = parser("s", false);
        let blob = |hash: &str, role: &str, text: &str| json!({"type": "api-request-blob", "hash": hash, "message": {"role": role, "content": [{"type": "text", "text": text}]}});
        p.line(&blob("h1", "system", "The user hasn't heard from you in a while — say in a few words what you're doing, then continue.").to_string());
        p.line(&blob("h2", "system", "UserPromptSubmit hook additional context: Any multi-step request needs a todo list").to_string());
        p.line(&blob("h3", "user", "fix the parser").to_string());
        let s = p.into_session();
        let kinds: Vec<(BlockKind, bool)> = s.messages.iter().map(|m| (s.blocks[m.blocks.start].kind, m.starts_turn)).collect();
        assert_eq!(kinds, [(BlockKind::Reminder, false), (BlockKind::Reminder, false), (BlockKind::Prompt, true)]);
        assert_eq!(s.blocks[s.messages[1].blocks.start].target.as_deref(), Some("hook context"));
    }

    #[test]
    fn requests_carry_the_origin_of_their_work() {
        let reply = |id: &str, extra: Value| assistant_line(id, extra, json!([{"type": "text", "text": "ok"}]));
        let s = session_from(&[
            json!({"type": "ai-title", "aiTitle": "Generated"}),
            user_line("fix the parser"),
            reply("m1", json!({})),
            user_line("<command-message>goal</command-message>\n<command-name>/goal</command-name>"),
            reply("m2", json!({})),
            reply("m3", json!({"attributionSkill": "code-review"})),
            user_line("[Request interrupted by user]"),
            reply("m4", json!({})),
            user_line("<task-notification>\n<task-id>b7x2k9</task-id>\n<status>completed</status>"),
            reply("m5", json!({})),
            json!({"type": "custom-title", "customTitle": "[archived] Mine"}),
            json!({"type": "ai-title", "aiTitle": "Generated later"}),
        ]);
        let origins: Vec<Option<String>> = s.chains[0].requests.iter().map(|r| r.origin.as_ref().map(ToString::to_string)).collect();
        let origins: Vec<Option<&str>> = origins.iter().map(|o| o.as_deref()).collect();
        assert_eq!(
            origins,
            [Some("prompt"), Some("command:/goal"), Some("skill:code-review"), Some("command:/goal"), Some("notification")]
        );
        assert_eq!(s.title.as_deref(), Some("Mine"), "the user's own title wins, without the archive mark");
    }

    #[test]
    fn subagents_are_named_by_their_task_and_credited_to_what_started_them() {
        let dir = std::env::temp_dir().join(format!("tokprof-claude-test-{}", std::process::id()));
        let subagents = dir.join("s1").join("subagents");
        std::fs::create_dir_all(&subagents).unwrap();
        let meta = |id: &str, ty: &str, task: &str, call: &str| {
            let text = json!({"agentType": ty, "description": task, "toolUseId": call}).to_string();
            std::fs::write(subagents.join(format!("agent-{id}.meta.json")), text).unwrap();
        };
        meta("a1", "general-purpose", "Find handlers", "toolu_1");
        meta("a2", "Explore", "Scan docs", "toolu_2");

        // A skill starts one background subagent and a typed prompt another; their results
        // come back as notifications.
        let launch = |id: &str, call: &str, extra: Value| {
            assistant_line(id, extra, json!([{"type": "tool_use", "id": call, "name": "Agent", "input": {"description": "x"}}]))
        };
        let mut p = parser("s1", false);
        p.metas = crate::read_metas(&subagents);
        for line in [
            user_line("review this"),
            launch("m1", "toolu_1", json!({"attributionSkill": "code-review"})),
            launch("m2", "toolu_2", json!({})),
            user_line("<task-notification>\n<task-id>a1</task-id>"),
            assistant_line("m3", json!({}), json!([])),
            user_line("<task-notification>\n<task-id>a2</task-id>"),
            assistant_line("m4", json!({}), json!([])),
        ] {
            p.line(&line.to_string());
        }
        let s = p.into_session();
        let origins: Vec<Option<String>> = s.chains[0].requests.iter().map(|r| r.origin.as_ref().map(ToString::to_string)).collect();
        let origins: Vec<Option<&str>> = origins.iter().map(|o| o.as_deref()).collect();
        assert_eq!(origins, [Some("skill:code-review"), Some("prompt"), Some("skill:code-review"), Some("agent:Explore")]);

        let mut p = parser("s1", true);
        p.agent = crate::read_meta(&subagents.join("agent-a1.meta.json"));
        p.line(&assistant_line("m9", json!({"agentId": "a1", "isSidechain": true}), json!([])).to_string());
        let s = p.into_session();
        std::fs::remove_dir_all(&dir).ok();
        assert_eq!(s.task.as_deref(), Some("Find handlers"));
        assert_eq!(s.chains[0].name, "subagent: Find handlers");
        assert_eq!(s.chains[0].requests[0].origin.as_ref().map(ToString::to_string).as_deref(), Some("agent:general-purpose"));
    }

    #[test]
    fn reminders_are_labelled_by_their_source() {
        assert_eq!(reminder_label("<system-reminder>\nCodebase and user instructions are shown below."), "instructions");
        assert_eq!(reminder_label("<system-reminder>[SYSTEM NOTIFICATION - NOT USER INPUT] This is an automated background"), "background notifications");
        assert_eq!(reminder_label("<system-reminder>The following deferred tools are now available"), "deferred tool lists");
        assert_eq!(reminder_label("<system-reminder>Attribution for git commits"), "other");
        // A skill description that mentions a background task is still the skill listing.
        assert_eq!(reminder_label("The following skills are available for use with the Skill tool:\n\n- profiler: attribute a background task"), "skills");
        assert_eq!(reminder_label("# MCP Server Instructions\n\nThe following MCP servers have provided instructions"), "other");
    }

    /// The `system` message Claude Code 2.1.300 sent after a compaction, trimmed: background
    /// agents' results, the hook context of each turn, restored files and the session's setup.
    #[test]
    fn a_system_message_is_split_into_its_reminders() {
        let parts = [
            "[SYSTEM NOTIFICATION - NOT USER INPUT]\nThis is an automated background-task event.\n\n<task-notification>\n<result>Angle A found 2 bugs\n\nin two files</result>\n</task-notification>",
            "UserPromptSubmit hook additional context: Any multi-step request needs a todo list.",
            "Called the Read tool with the following input: {\"file_path\":\"/tmp/bt.log\"}\nResult of calling the Read tool:\n1\tPASS\n\n2\tPASS",
            "Note: /w/Big.kt was read before the last conversation was summarized, but the contents are too large to include.",
            "The following skills were invoked EARLIER in this session (before the conversation was compacted).\n\n### Skill: youtrack\n\n# YouTrack Integration",
            "The following deferred tools are now available via ToolSearch:\nmcp__idea__search_symbol",
            "# Environment\nYou have been invoked in the following environment:\n - Platform: darwin",
            "The following skills are available for use with the Skill tool:\n\n- deploy: Any deploy work.",
            "# Advisor Tool\n\nYou have access to an `advisor` tool.",
        ];
        let text = parts.join("\n\n");
        let got = reminder_parts(&text);
        let labels: Vec<&str> = got.iter().map(|(_, l)| *l).collect();
        // Neighbours of one source stay one part: the restored file and the note on another.
        assert_eq!(labels, [
            "background notifications", "hook context", "attached files", "skills",
            "deferred tool lists", "other", "skills", "other",
        ]);
        assert_eq!(got.iter().map(|(c, _)| c).sum::<u64>(), text.len() as u64);
        assert_eq!(got[0].0, parts[0].len() as u64 + 2);
        // A message with one reminder stays whole, even when its text has a heading of its own.
        let one = "Codebase and user instructions are shown below.\n\n# Environment\nuse bazel";
        assert_eq!(reminder_parts(one), [(one.len() as u64, "instructions")]);
    }

    #[test]
    fn splits_advisor_iterations() {
        let s = session_from(&[json!({"type": "assistant", "requestId": "q1", "timestamp": "2026-10-01T10:00:05Z",
            "message": {"id": "m1", "model": "claude-fable-5-1", "content": [],
            "usage": {"input_tokens": 35, "cache_read_input_tokens": 500, "cache_creation_input_tokens": 0, "output_tokens": 9,
                "iterations": [
                    {"type": "message", "input_tokens": 33, "cache_read_input_tokens": 250, "cache_creation_input_tokens": 0, "output_tokens": 4},
                    {"type": "advisor_message", "model": "claude-fable-5-1", "input_tokens": 260, "cache_read_input_tokens": 0, "cache_creation_input_tokens": 0, "output_tokens": 11},
                    {"type": "message", "input_tokens": 2, "cache_read_input_tokens": 250, "cache_creation_input_tokens": 0, "output_tokens": 5}]}}})]);
        let reqs = &s.chains[0].requests;
        assert_eq!(reqs.len(), 3);
        assert!(reqs[1].advisor && !reqs[0].advisor);
        assert_eq!(reqs[1].usage.uncached, 260);
    }

    #[test]
    fn tools_and_reminders_get_their_class() {
        let block = |kind, name: &str| Block { class: Class::None, kind, chars: 1, tool: Some(name.into()), target: Some(name.into()) };
        let tool = |kind, name| class_of(&block(kind, name));
        assert_eq!(tool(BlockKind::ToolResult, "Bash"), Class::Tool(ToolClass::Execute));
        assert_eq!(tool(BlockKind::ToolResult, "BashOutput"), Class::Tool(ToolClass::ExecuteOutput));
        assert_eq!(tool(BlockKind::ToolUse, "Write"), Class::Tool(ToolClass::Write));
        assert_eq!(tool(BlockKind::ToolUse, "MultiEdit"), Class::Tool(ToolClass::Edit));
        assert_eq!(tool(BlockKind::ToolResult, "WebSearch"), Class::Tool(ToolClass::Fetch));
        assert_eq!(tool(BlockKind::ToolResult, "Glob"), Class::Tool(ToolClass::Search));
        assert_eq!(tool(BlockKind::ToolUse, "Agent"), Class::Tool(ToolClass::Other));
        assert_eq!(tool(BlockKind::Reminder, "hook context"), Class::Reminder(ReminderKind::HookContext));
        assert_eq!(tool(BlockKind::Reminder, "task list"), Class::Reminder(ReminderKind::TaskList));
        assert_eq!(tool(BlockKind::Reminder, "attached files"), Class::Reminder(ReminderKind::AttachedFiles));
        assert_eq!(tool(BlockKind::Reminder, "other"), Class::Reminder(ReminderKind::Other));
        assert_eq!(tool(BlockKind::Text, "Bash"), Class::None);
    }
}
