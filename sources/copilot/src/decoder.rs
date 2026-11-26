//! The lines of a Copilot CLI session log, and the session they give.
//!
//! The log is `events.jsonl`: one JSON object per line, with `type`, `data`, `timestamp`, and
//! `agentId` on the lines of a subagent. A `session.usage_record` line is one API request with
//! its usage and credits; the `assistant.message` line after it, whose `apiCallId` is the
//! record's `accounting.usageId`, is its response with its text and tool requests. Each call has
//! a `tool.execution_start` and a `tool.execution_complete` line with its `toolCallId` (seen in
//! Copilot CLI 1.0.85–1.0.116; 1.0.85 writes no usage records).
//!
//! The decoder keeps what each line tells, a few numbers and short heads, and builds the session
//! from them whenever it is asked, so a snapshot is the session of the lines fed so far.

use std::collections::HashMap;
use std::path::PathBuf;

use serde_json::Value;
use tokprof_core::api::ApiId;
use tokprof_core::labels::{command_head, display_path, exit_line, first_line, message_label};
use tokprof_core::model::{
    Block, BlockKind, CallEnd, CallRecord, Chain, ChainKind, Class, Compaction, CompactionTrigger, HandoffKind, Outcome, PromptDelta, ReminderKind, Request,
    Session, ToolClass, TypedMessage, Usage, input_head,
};
use tokprof_core::source::{AgentId, Decoder};
use tokprof_core::trigger::{CallClass, Pending, Trigger, VcsActions, command_class, vcs_actions};
use tokprof_logio::{json_len, parse_ts_ms, str_at, u64_at};

/// Dollars at list price per credit.
pub const USD_PER_CREDIT: f64 = 0.01;

/// A cache lifetime over this many seconds is the long one.
const LONG_TTL_S: f64 = 1800.0;

/// The line types the decoder reads; any other line is skipped before its body is parsed.
const READ: &[&str] = &[
    "session.start",
    "session.usage_record",
    "session.compaction_complete",
    "assistant.turn_start",
    "assistant.message",
    "tool.execution_start",
    "tool.execution_complete",
    "user.message",
    "subagent.started",
];

/// A model as tokprof names it: the dots of a `claude-` id as dashes (`claude-fable-5.2` is
/// `claude-fable-5-2`).
pub fn model_name(id: &str) -> String {
    if id.starts_with("claude-") { id.replace('.', "-") } else { id.to_string() }
}

/// The `type` of a line that starts with it, read without parsing the line.
fn leading_type(line: &str) -> Option<&str> {
    let rest = line.trim_start().strip_prefix("{\"type\":\"")?;
    rest.split('"').next()
}

/// What a usage record or a compaction's `compactionTokensUsed` gives.
#[derive(Clone, Debug)]
pub(crate) struct UsageRec {
    ms: i64,
    /// `accounting.usageId`, the `apiCallId` of the record's response.
    id: Option<String>,
    model: Option<String>,
    usage: Usage,
    duration_ms: i64,
    credits: Option<f64>,
    api: ApiId,
}

fn usage_rec(u: &Value, ms: i64) -> UsageRec {
    let (input, read, written, output) = (u64_at(u, "inputTokens"), u64_at(u, "cacheReadTokens"), u64_at(u, "cacheWriteTokens"), u64_at(u, "outputTokens"));
    let ttl = u.get("cacheTtlSeconds").and_then(Value::as_f64).or_else(|| parse_ts_ms(u.get("cacheExpiresAt")).map(|expires| (expires - ms) as f64 / 1000.0));
    let long = ttl.is_some_and(|s| s > LONG_TTL_S);
    let model = str_at(u, "model").filter(|m| !m.is_empty()).map(model_name);
    let api = match str_at(u, "apiEndpoint") {
        Some("/v1/messages") => ApiId::Anthropic,
        Some(e) if e.ends_with("/responses") || e.ends_with("/chat/completions") => ApiId::OpenAi,
        _ => match model.as_deref() {
            Some(m) if m.starts_with("claude-") => ApiId::Anthropic,
            Some(m) if m.starts_with("gpt-") => ApiId::OpenAi,
            _ => ApiId::Unknown,
        },
    };
    UsageRec {
        ms,
        id: u.get("accounting").and_then(|a| str_at(a, "usageId")).map(str::to_string),
        model,
        usage: Usage {
            // `inputTokens` is the whole prompt, cache reads and writes included.
            uncached: input.saturating_sub(read + written),
            cache_read: read,
            write_short: if long { 0 } else { written },
            write_long: if long { written } else { 0 },
            // `reasoningTokens` is part of `outputTokens`.
            output,
            reasoning: u64_at(u, "reasoningTokens").min(output),
        },
        duration_ms: u.get("duration").and_then(Value::as_f64).map_or(0, |d| d.max(0.0) as i64),
        credits: u.get("copilotUsage").and_then(|c| c.get("totalNanoAiu")).and_then(Value::as_f64).map(|n| n / 1e9),
        api,
    }
}

/// A tool request of a response, with what the session needs of its arguments.
#[derive(Clone, Debug)]
pub(crate) struct Call {
    id: String,
    name: String,
    args_chars: u64,
    /// The shell command of `bash`, the path of a file tool, the pattern of a search.
    target: Option<String>,
    input: Option<String>,
    class: CallClass,
    vcs: VcsActions,
    /// The first string value of the arguments, for an MCP tool's input head.
    first_string: Option<String>,
}

fn call(v: &Value) -> Option<Call> {
    let name = str_at(v, "name")?.to_string();
    let args = v.get("arguments").unwrap_or(&Value::Null);
    let arg = |k: &str| args.get(k).and_then(Value::as_str).filter(|s| !s.trim().is_empty());
    let command = arg("command");
    let (target, input, class) = match name.as_str() {
        "bash" => (command.map(command_head), command.and_then(input_head), command.map_or(CallClass::System, command_class)),
        "read_bash" | "stop_bash" => (None, arg("shellId").and_then(input_head), CallClass::System),
        "view" => (arg("path").map(str::to_string), arg("path").and_then(input_head), CallClass::Reading),
        "edit" | "create" => (arg("path").map(str::to_string), arg("path").and_then(input_head), CallClass::Editing),
        "grep" | "glob" => (arg("pattern").map(str::to_string), arg("pattern").and_then(input_head), CallClass::Reading),
        "web_fetch" => (None, arg("url").and_then(input_head), CallClass::Web),
        "task" | "read_agent" | "list_agents" => (None, arg("name").or(arg("description")).or(arg("agent_id")).and_then(input_head), CallClass::Subagents),
        "skill" => (None, arg("skill").and_then(input_head), CallClass::OtherTools),
        _ => (None, None, CallClass::OtherTools),
    };
    let first_string = args.as_object().and_then(|o| o.values().find_map(|v| v.as_str().filter(|s| !s.trim().is_empty()).and_then(input_head)));
    let vcs = if name == "bash" { command.map(vcs_actions).unwrap_or_default() } else { VcsActions::default() };
    Some(Call { id: str_at(v, "toolCallId")?.to_string(), args_chars: json_len(args), name, target, input, class, vcs, first_string })
}

/// The kind of a tool (session-logs, Copilot CLI tool kinds and input heads).
fn tool_class(name: &str) -> ToolClass {
    match name {
        "bash" | "stop_bash" => ToolClass::Execute,
        "read_bash" => ToolClass::ExecuteOutput,
        "view" => ToolClass::Read,
        "edit" => ToolClass::Edit,
        "create" => ToolClass::Write,
        "grep" | "glob" => ToolClass::Search,
        "web_fetch" => ToolClass::Fetch,
        "task" | "read_agent" | "list_agents" => ToolClass::Subagent,
        "skill" | "ask_user" | "vote_memory" => ToolClass::Manage,
        _ => ToolClass::Other,
    }
}

/// One line of a log, as far as the decoder reads it.
#[derive(Clone, Debug)]
pub(crate) enum Line {
    Start {
        id: Option<String>,
        cwd: Option<String>,
        version: Option<String>,
    },
    Usage {
        agent: Option<String>,
        rec: UsageRec,
    },
    TurnStart {
        agent: Option<String>,
        ms: i64,
    },
    Message {
        agent: Option<String>,
        ms: i64,
        api_call: Option<String>,
        model: Option<String>,
        text: u64,
        thinking: u64,
        calls: Vec<Call>,
    },
    ToolStart {
        ms: i64,
        call: String,
        /// The MCP server and its tool, when the call is one of an MCP server.
        mcp: Option<(String, String)>,
    },
    ToolEnd {
        ms: i64,
        call: String,
        chars: u64,
        outcome: Outcome,
        error: Option<String>,
        background: bool,
    },
    User {
        agent: Option<String>,
        ms: i64,
        source: Option<String>,
        chars: u64,
        label: String,
        steering: bool,
    },
    Subagent {
        agent: String,
        name: Option<String>,
    },
    Compacted {
        ms: i64,
        success: bool,
        call: Option<UsageRec>,
        summary: u64,
    },
}

fn chars(s: &str) -> u64 {
    s.chars().count() as u64
}

/// The line, when it is one the decoder reads.
pub(crate) fn parse_line(line: &str) -> Option<Line> {
    if leading_type(line).is_some_and(|t| !READ.contains(&t)) {
        return None;
    }
    let Ok(v @ Value::Object(_)) = serde_json::from_str::<Value>(line) else { return None };
    let kind = str_at(&v, "type").filter(|t| READ.contains(t))?;
    let d = v.get("data")?;
    let owned = |v: &Value, k: &str| str_at(v, k).filter(|s| !s.is_empty()).map(str::to_string);
    let agent = owned(&v, "agentId");
    let ts = parse_ts_ms(v.get("timestamp"));
    Some(match kind {
        "session.start" => Line::Start { id: owned(d, "sessionId"), cwd: d.get("context").and_then(|c| owned(c, "cwd")), version: owned(d, "copilotVersion") },
        "session.usage_record" => Line::Usage { agent, rec: usage_rec(d.get("usage")?, ts?) },
        "session.compaction_complete" => {
            let ms = ts?;
            Line::Compacted {
                ms,
                success: d.get("success").and_then(Value::as_bool) == Some(true),
                call: d.get("compactionTokensUsed").filter(|u| u.is_object()).map(|u| usage_rec(u, ms)),
                summary: str_at(d, "summaryContent").map_or(0, chars),
            }
        }
        "assistant.turn_start" => Line::TurnStart { agent, ms: ts? },
        "assistant.message" => Line::Message {
            agent,
            ms: ts?,
            api_call: owned(d, "apiCallId"),
            model: owned(d, "model").map(|m| model_name(&m)),
            text: str_at(d, "content").map_or(0, chars),
            thinking: str_at(d, "reasoningText").map_or(0, chars),
            calls: d.get("toolRequests").and_then(Value::as_array).map(|c| c.iter().filter_map(call).collect()).unwrap_or_default(),
        },
        "tool.execution_start" => Line::ToolStart {
            ms: ts?,
            call: owned(d, "toolCallId")?,
            mcp: owned(d, "mcpServerName").zip(owned(d, "mcpToolName").or_else(|| owned(d, "toolName"))),
        },
        "tool.execution_complete" => {
            let content = d.get("result").and_then(|r| r.get("content"));
            let text = content.and_then(Value::as_str).unwrap_or_default();
            let failed = d.get("success").and_then(Value::as_bool) == Some(false);
            let err = d.get("error");
            let exit = d.get("shellExecution").and_then(|s| s.get("exitCode")).and_then(Value::as_i64).filter(|&c| c != 0);
            let (outcome, error) = match (failed, err.and_then(|e| str_at(e, "code"))) {
                (true, Some("rejected")) => (Outcome::Refused, None),
                (true, Some("denied")) => (Outcome::Denied, None),
                (true, _) => (Outcome::Failed, err.and_then(|e| str_at(e, "message")).and_then(first_line).or_else(|| first_line(text))),
                (false, _) => match exit {
                    Some(code) => (Outcome::Failed, Some(exit_line(code, text))),
                    None => (Outcome::Worked, None),
                },
            };
            let prop = |k: &str| d.get("toolTelemetry").and_then(|t| t.get("properties")).and_then(|p| str_at(p, k));
            Line::ToolEnd {
                ms: ts?,
                call: owned(d, "toolCallId")?,
                chars: content.map_or(0, |c| c.as_str().map_or_else(|| json_len(c), chars)),
                outcome,
                error,
                background: prop("executionMode") == Some("async") || prop("detached") == Some("true"),
            }
        }
        "user.message" => {
            let content = str_at(d, "content").unwrap_or_default();
            Line::User {
                agent,
                ms: ts?,
                source: owned(d, "source"),
                // What the model read as the message, when the log tells.
                chars: chars(str_at(d, "transformedContent").unwrap_or(content)),
                label: message_label(content),
                steering: str_at(d, "delivery") == Some("steering"),
            }
        }
        "subagent.started" => Line::Subagent { agent: agent?, name: owned(d, "agentDisplayName").or_else(|| owned(d, "agentName")) },
        _ => return None,
    })
}

/// The decoder of one log: it keeps the lines it read, and builds the session from them.
pub struct Parser {
    agent: AgentId,
    id: String,
    home: PathBuf,
    /// The `name` of the `workspace.yaml` beside the log.
    title: Option<String>,
    lines: Vec<Line>,
}

impl Parser {
    pub fn new(agent: AgentId, id: String, home: PathBuf, title: Option<String>) -> Parser {
        Parser { agent, id, home, title, lines: Vec::new() }
    }
}

impl Decoder for Parser {
    fn feed(&mut self, record: &str) {
        if let Some(line) = parse_line(record) {
            self.lines.push(line);
        }
    }

    fn snapshot(&self) -> Session {
        Build::new(self).run(&self.lines)
    }

    fn finish(self: Box<Self>) -> Session {
        Build::new(&self).run(&self.lines)
    }
}

/// A chain while it is built.
struct ChainState {
    name: String,
    kind: ChainKind,
    requests: Vec<Request>,
    /// Messages in the prompt so far, and those logged since its latest request.
    held: usize,
    tail: Vec<usize>,
    pending: Pending,
    /// The latest `assistant.turn_start` of the chain.
    turn_start: Option<i64>,
    /// A usage record whose response has not come yet.
    usage: Option<UsageRec>,
    /// A compaction came after the latest request.
    compacted: bool,
}

impl ChainState {
    fn new(name: String, kind: ChainKind) -> ChainState {
        ChainState { name, kind, requests: Vec::new(), held: 0, tail: Vec::new(), pending: Pending::default(), turn_start: None, usage: None, compacted: false }
    }
}

/// A call while the session is built.
struct CallState {
    record: usize,
    blocks: Vec<usize>,
    chain: usize,
    /// No request made the call: its result is not added to a prompt.
    platform: bool,
    done: bool,
    asks: bool,
    tool: String,
    class: Class,
    target: Option<String>,
    vcs: VcsActions,
    outcome: Option<(Outcome, Option<String>)>,
}

/// A response of the model.
struct Response<'a> {
    ms: i64,
    model: Option<&'a str>,
    text: u64,
    thinking: u64,
    calls: &'a [Call],
}

struct Build<'a> {
    p: &'a Parser,
    s: Session,
    chains: Vec<ChainState>,
    by_agent: HashMap<&'a str, usize>,
    /// The names of the subagents, and the MCP tools of the calls, as the whole log gives them.
    names: HashMap<&'a str, &'a str>,
    mcp: HashMap<&'a str, (&'a str, &'a str)>,
    calls: HashMap<&'a str, CallState>,
    records: Vec<CallRecord>,
    version: Option<&'a str>,
    credits: Option<f64>,
}

impl<'a> Build<'a> {
    fn new(p: &'a Parser) -> Build<'a> {
        let mut s = Session::new(p.agent.clone(), p.id.clone());
        s.title = p.title.clone();
        Build {
            p,
            s,
            chains: vec![ChainState::new("main".to_string(), ChainKind::Main)],
            by_agent: HashMap::new(),
            names: HashMap::new(),
            mcp: HashMap::new(),
            calls: HashMap::new(),
            records: Vec::new(),
            version: None,
            credits: None,
        }
    }

    /// The chain of an `agentId`: the main one for none, else a subagent's, started when it is
    /// first seen.
    fn chain(&mut self, agent: Option<&'a str>) -> usize {
        let Some(agent) = agent else { return 0 };
        if let Some(&c) = self.by_agent.get(agent) {
            return c;
        }
        let name = self.names.get(agent).map_or_else(|| "subagent".to_string(), |n| format!("subagent: {n}"));
        self.chains.push(ChainState::new(name, ChainKind::Subagent));
        self.by_agent.insert(agent, self.chains.len() - 1);
        self.chains.len() - 1
    }

    fn push(&mut self, chain: usize, blocks: Vec<Block>, starts_turn: bool) -> usize {
        let m = self.s.push_message(blocks, starts_turn);
        self.chains[chain].tail.push(m);
        m
    }

    fn run(mut self, lines: &'a [Line]) -> Session {
        for line in lines {
            match line {
                Line::Subagent { agent, name: Some(name) } => {
                    self.names.entry(agent).or_insert(name);
                }
                Line::ToolStart { call, mcp: Some((server, tool)), .. } => {
                    self.mcp.entry(call).or_insert((server, tool));
                }
                _ => {}
            }
        }
        for line in lines {
            match line {
                Line::Start { id, cwd, version } => self.start(id.as_deref(), cwd.as_deref(), version.as_deref()),
                Line::Usage { agent, rec } => {
                    let chain = self.chain(agent.as_deref());
                    if let Some(old) = self.chains[chain].usage.replace(rec.clone()) {
                        self.request(chain, Some(old), None, false);
                    }
                }
                Line::TurnStart { agent, ms } => {
                    let chain = self.chain(agent.as_deref());
                    self.chains[chain].turn_start = Some(*ms);
                }
                Line::Message { agent, ms, api_call, model, text, thinking, calls } => {
                    let chain = self.chain(agent.as_deref());
                    let response = Response { ms: *ms, model: model.as_deref(), text: *text, thinking: *thinking, calls };
                    self.message(chain, api_call.as_deref(), response);
                }
                Line::ToolStart { ms, call, .. } => self.tool_start(call, *ms),
                Line::ToolEnd { ms, call, chars, outcome, error, background } => self.tool_end(call, *ms, *chars, *outcome, error.as_deref(), *background),
                Line::User { agent, ms, source, chars, label, steering } => {
                    let chain = self.chain(agent.as_deref());
                    self.user(chain, *ms, source.as_deref(), *chars, label, *steering);
                }
                Line::Subagent { .. } => {}
                Line::Compacted { ms, success, call, summary } => self.compacted(*ms, *success, call.as_ref(), *summary),
            }
        }
        self.finish()
    }

    fn start(&mut self, id: Option<&str>, cwd: Option<&'a str>, version: Option<&'a str>) {
        if self.version.is_some() || self.s.cwd.is_some() {
            return;
        }
        if let Some(id) = id {
            self.s.id = id.to_string();
        }
        if let Some(cwd) = cwd {
            self.s.project = display_path(cwd, None, &self.p.home);
            self.s.cwd = Some(cwd.to_string());
        }
        self.version = version;
    }

    fn message(&mut self, chain: usize, api_call: Option<&str>, response: Response<'a>) {
        let Some(api_call) = api_call else {
            // The CLI wrote the message itself: its calls are calls no request made.
            for call in response.calls {
                self.platform_call(chain, call, response.ms);
            }
            return;
        };
        let rec = match self.chains[chain].usage.take() {
            Some(rec) if rec.id.as_deref().is_none_or(|id| id == api_call) => Some(rec),
            // The record of another response, which the log does not hold.
            Some(other) => {
                self.request(chain, Some(other), None, false);
                None
            }
            None => None,
        };
        self.request(chain, rec, Some(response), false);
    }

    /// One request: a usage record with its response, a record whose response is not in the
    /// log, or a response without a record, which is a request without usage.
    fn request(&mut self, chain: usize, rec: Option<UsageRec>, response: Option<Response<'a>>, compaction_call: bool) {
        let mut blocks = Vec::new();
        let mut made = Vec::new();
        if let Some(r) = &response {
            if r.thinking > 0 {
                blocks.push(Block { kind: BlockKind::Thinking, class: Class::None, chars: r.thinking, tool: None, target: None });
            }
            if r.text > 0 {
                blocks.push(Block { kind: BlockKind::Text, class: Class::None, chars: r.text, tool: None, target: None });
            }
            let first_block = self.s.blocks.len() + blocks.len();
            for (i, call) in r.calls.iter().enumerate() {
                let (tool, class, target) = self.tool_of(call);
                blocks.push(Block { kind: BlockKind::ToolUse, class, chars: call.args_chars, tool: Some(tool.clone()), target: target.clone() });
                made.push((first_block + i, call, tool, class, target));
            }
        }
        let ended = rec.as_ref().map(|r| r.ms).or(response.as_ref().map(|r| r.ms)).unwrap_or(0);
        let st = &mut self.chains[chain];
        // A chain never goes back in time, though the lines may.
        let ts_ms = st.requests.last().map_or(ended, |r| r.ts_ms.max(ended));
        let sent = match &rec {
            Some(r) => ts_ms - r.duration_ms,
            None => st.turn_start.filter(|&t| t <= ts_ms).unwrap_or(ts_ms),
        };
        let tail = std::mem::take(&mut st.tail);
        let keep = st.held;
        st.held += tail.len();
        let trigger = match compaction_call {
            true => Some(Trigger::Compaction),
            false => st.pending.trigger().or((!st.requests.is_empty()).then_some(Trigger::Continuation)),
        };
        let activity = if compaction_call { None } else { st.pending.activity() };
        if !compaction_call {
            st.pending = Pending::default();
        }
        let compacted_before = !compaction_call && std::mem::take(&mut st.compacted);
        let produced = response.as_ref().map(|_| self.s.push_message(blocks, false));
        if let Some(credits) = rec.as_ref().and_then(|r| r.credits) {
            *self.credits.get_or_insert(0.0) += credits;
        }
        let model = rec.as_ref().and_then(|r| r.model.as_deref()).or(response.as_ref().and_then(|r| r.model)).unwrap_or("unknown").to_string();
        let st = &mut self.chains[chain];
        st.tail.extend(produced);
        st.requests.push(Request {
            ts_ms,
            api: rec.as_ref().map_or_else(|| if model.starts_with("claude-") { ApiId::Anthropic } else { ApiId::Unknown }, |r| r.api),
            model,
            usage: rec.as_ref().map(|r| r.usage).unwrap_or_default(),
            version: self.version.map(str::to_string),
            prompt: Some(PromptDelta { shape: None, keep, tail }),
            produced,
            compacted_before,
            compaction_call,
            trigger,
            activity,
            first_sent_ms: Some(sent),
            sent_ms: Some(sent),
            ended_ms: Some(ts_ms),
            no_usage: rec.is_none(),
            ..Request::default()
        });
        // A response without calls ends the agent's turn.
        if chain == 0 && !compaction_call && response.as_ref().is_some_and(|r| r.calls.is_empty()) {
            self.s.push_handoff(ts_ms, HandoffKind::TurnEnd);
        }
        let start = response.as_ref().map(|r| r.ms);
        for (b, call, tool, class, target) in made {
            self.chains[chain].pending.call(call.class);
            let input = if target.as_deref().is_some_and(|t| t.starts_with("mcp:")) { call.first_string.clone() } else { call.input.clone() };
            let asks = call.name == "ask_user";
            self.records.push(CallRecord { start_ms: start, end_ms: None, done: false, background: false, asks_user: asks, input });
            self.calls.entry(&call.id).or_insert(CallState {
                record: self.records.len() - 1,
                blocks: vec![b],
                chain,
                platform: false,
                done: false,
                asks,
                tool,
                class,
                target,
                vcs: call.vcs,
                outcome: None,
            });
        }
    }

    /// A call of a message the CLI wrote itself, such as reading a finished background task.
    fn platform_call(&mut self, chain: usize, call: &'a Call, ms: i64) {
        if self.calls.contains_key(call.id.as_str()) {
            return;
        }
        let (tool, class, target) = self.tool_of(call);
        let block = Block { kind: BlockKind::ToolUse, class, chars: call.args_chars, tool: Some(tool.clone()), target: target.clone() };
        let m = self.s.push_message(vec![block], false);
        let b = self.s.messages[m].blocks.start;
        self.s.platform_calls.push(b);
        self.records.push(CallRecord { start_ms: Some(ms), end_ms: None, done: false, background: false, asks_user: false, input: call.input.clone() });
        let record = self.records.len() - 1;
        self.calls.insert(
            &call.id,
            CallState { record, blocks: vec![b], chain, platform: true, done: false, asks: false, tool, class, target, vcs: call.vcs, outcome: None },
        );
    }

    fn tool_start(&mut self, call: &str, ms: i64) {
        let Some(st) = self.calls.get(call).filter(|st| !st.done) else { return };
        self.records[st.record].start_ms = Some(ms);
        if st.asks && st.chain == 0 {
            self.s.push_handoff(ms, HandoffKind::Question);
        }
    }

    fn tool_end(&mut self, call: &str, ms: i64, chars: u64, outcome: Outcome, error: Option<&str>, background: bool) {
        let b = self.s.blocks.len();
        let Some(st) = self.calls.get_mut(call).filter(|st| !st.done) else { return };
        st.done = true;
        st.outcome = Some((outcome, error.map(str::to_string)));
        let r = &mut self.records[st.record];
        (r.end_ms, r.done) = (Some(ms.max(r.start_ms.unwrap_or(ms))), true);
        r.background |= background;
        let (chain, answered) = (st.chain, st.asks && st.chain == 0 && outcome == Outcome::Worked);
        if !st.platform {
            st.blocks.push(b);
            let block = Block { kind: BlockKind::ToolResult, class: st.class, chars, tool: Some(st.tool.clone()), target: st.target.clone() };
            self.push(chain, vec![block], false);
        }
        if answered {
            self.s.push_handoff(ms, HandoffKind::Answer);
        }
    }

    fn user(&mut self, chain: usize, ms: i64, source: Option<&str>, chars: u64, label: &str, steering: bool) {
        let prompt = Block { kind: BlockKind::Prompt, class: Class::None, chars, tool: None, target: None };
        if chain != 0 {
            // A subagent's brief.
            self.push(chain, vec![prompt], true);
            self.chains[chain].pending.event(Trigger::Prompt);
        } else if let Some(source) = source {
            // A message the CLI sent.
            let skill = source.starts_with("skill-");
            let kind = if skill { ReminderKind::Skills } else { ReminderKind::Other };
            let target = Some(if skill { "skills" } else { "other" }.to_string());
            self.push(0, vec![Block { kind: BlockKind::Reminder, class: Class::Reminder(kind), chars, tool: None, target }], false);
        } else {
            self.s.typed.push(TypedMessage { ts_ms: ms, label: label.to_string() });
            if steering {
                self.s.push_handoff(ms, HandoffKind::TypedWhileWorking);
            }
            self.push(0, vec![prompt], true);
            self.chains[0].pending.event(Trigger::Prompt);
        }
    }

    /// A compaction of the main conversation: its call, when the line carries its usage, and
    /// the summary that replaces the conversation when it worked.
    fn compacted(&mut self, ms: i64, success: bool, call: Option<&UsageRec>, summary: u64) {
        if let Some(rec) = call {
            self.request(0, Some(rec.clone()), None, true);
        }
        if !success {
            return;
        }
        let m = self.s.push_message(vec![Block { kind: BlockKind::Text, class: Class::None, chars: summary, tool: None, target: None }], false);
        let st = &mut self.chains[0];
        if let Some(r) = call.and(st.requests.last_mut()) {
            r.produced = Some(m);
        }
        (st.held, st.tail, st.compacted) = (0, vec![m], true);
        self.s.compactions.push(Compaction { ts_ms: ms, kind: CompactionTrigger::Unknown });
    }

    /// A call's tool name, class and target; an MCP tool names its server.
    fn tool_of(&self, call: &Call) -> (String, Class, Option<String>) {
        match self.mcp.get(call.id.as_str()) {
            Some((server, tool)) => (format!("{server}-{tool}"), Class::Tool(ToolClass::Other), Some(format!("mcp:{server}"))),
            None => (call.name.clone(), Class::Tool(tool_class(&call.name)), call.target.clone()),
        }
    }

    fn finish(mut self) -> Session {
        // A record whose response is not written yet is a request all the same.
        for chain in 0..self.chains.len() {
            if let Some(rec) = self.chains[chain].usage.take() {
                self.request(chain, Some(rec), None, false);
            }
        }
        let mut calls: Vec<&CallState> = self.calls.values().collect();
        calls.sort_by_key(|c| c.record);
        for c in calls {
            for &b in &c.blocks {
                self.s.call_of_block.insert(b, c.record as u32);
                if let Some((outcome, error)) = c.outcome.as_ref().filter(|(o, _)| *o != Outcome::Worked) {
                    self.s.call_ends.insert(b, CallEnd { outcome: *outcome, error: error.clone() });
                }
            }
            if c.vcs.any() && c.outcome.as_ref().is_some_and(|(o, _)| *o == Outcome::Worked) {
                self.s.vcs.insert(c.blocks[0], c.vcs);
            }
        }
        self.s.reported_usd = self.credits.map(|c| c * USD_PER_CREDIT);
        self.s.calls = std::mem::take(&mut self.records);
        self.s.chains = self.chains.into_iter().map(|c| Chain { name: c.name, kind: c.kind, requests: c.requests }).collect();
        self.s
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const AT: &str = "2026-11-25T22:05:49.917Z";

    fn decode(lines: &[Value]) -> Session {
        let mut p = Box::new(Parser::new(AgentId::unknown("copilot"), "from-folder".into(), PathBuf::from("/Users/me"), None));
        lines.iter().for_each(|l| p.feed(&l.to_string()));
        p.finish()
    }

    fn line(kind: &str, at: &str, data: Value) -> Value {
        json!({"type": kind, "data": data, "id": "e", "timestamp": at, "parentId": null})
    }

    fn of_agent(mut line: Value, agent: &str) -> Value {
        line["agentId"] = json!(agent);
        line
    }

    fn usage(id: &str, model: &str, tokens: [u64; 4], nano: u64) -> Value {
        json!({"accounting": {"usageId": id}, "model": model, "inputTokens": tokens[0], "cacheReadTokens": tokens[1], "cacheWriteTokens": tokens[2],
            "outputTokens": tokens[3], "duration": 59272, "apiEndpoint": "/v1/messages", "copilotUsage": {"totalNanoAiu": nano}})
    }

    fn record(at: &str, usage: Value) -> Value {
        line("session.usage_record", at, json!({"usage": usage}))
    }

    fn message(at: &str, api_call: Option<&str>, calls: Value) -> Value {
        let mut data = json!({"content": "ok", "toolRequests": calls});
        if let Some(id) = api_call {
            data["apiCallId"] = json!(id);
            data["model"] = json!("claude-fable-5.1");
        }
        line("assistant.message", at, data)
    }

    fn requested(id: &str, name: &str, arguments: Value) -> Value {
        json!({"toolCallId": id, "name": name, "arguments": arguments})
    }

    fn started(at: &str, id: &str) -> Value {
        line("tool.execution_start", at, json!({"toolCallId": id}))
    }

    fn completed(at: &str, id: &str, rest: Value) -> Value {
        let mut data = json!({"toolCallId": id, "success": true, "result": {"content": "out"}});
        data.as_object_mut().unwrap().extend(rest.as_object().cloned().unwrap_or_default());
        line("tool.execution_complete", at, data)
    }

    /// The outcome of the one call of a session, and its error line.
    fn outcome(rest: Value) -> (Outcome, Option<String>) {
        let s = decode(&[
            message("2026-11-25T10:00:00.000Z", Some("m"), json!([requested("t", "bash", json!({"command": "make"}))])),
            completed("2026-11-25T10:00:01.000Z", "t", rest),
        ]);
        s.call_ends.values().next().map_or((Outcome::Worked, None), |e| (e.outcome, e.error.clone()))
    }

    #[test]
    fn the_start_line_names_the_session_the_project_and_the_version() {
        let start = json!({"sessionId": "8d362bc6-27c5-4206-97e9-2807b9610b79", "copilotVersion": "1.0.116", "context": {"cwd": "/Users/me/src/app"}});
        let s = decode(&[line("session.start", AT, start), record(AT, usage("m", "claude-fable-5.2", [10, 0, 0, 1], 1)), message(AT, Some("m"), json!([]))]);
        assert_eq!((s.id.as_str(), s.project.as_str()), ("8d362bc6-27c5-4206-97e9-2807b9610b79", "~/src/app"));
        assert_eq!(s.chains[0].requests[0].version.as_deref(), Some("1.0.116"));
        assert_eq!(decode(&[]).id, "from-folder", "a log without a start line keeps the id of its folder");
    }

    #[test]
    fn a_usage_record_is_a_request() {
        let s = decode(&[record(AT, usage("m", "claude-fable-5.2", [176733, 174203, 2528, 4773], 25105660000)), message(AT, Some("m"), json!([]))]);
        let r = &s.chains[0].requests;
        assert_eq!(r.len(), 1, "the record and its response are one request");
        let u = r[0].usage;
        assert_eq!((u.uncached, u.cache_read, u.write_short, u.write_long, u.output), (2, 174203, 2528, 0, 4773));
        assert_eq!((r[0].model.as_str(), r[0].api, r[0].no_usage), ("claude-fable-5-2", ApiId::Anthropic, false));
        let ended = parse_ts_ms(Some(&json!(AT))).unwrap();
        assert_eq!((r[0].ended_ms, r[0].sent_ms), (Some(ended), Some(ended - 59272)));
        assert!((s.reported_usd.unwrap() - 0.2510566).abs() < 1e-9, "25.10566 credits at $0.01");
    }

    #[test]
    fn the_uncached_input_is_never_below_zero() {
        let s = decode(&[record(AT, usage("m", "gpt-6.2-sol", [100, 90, 20, 5], 1))]);
        let r = &s.chains[0].requests[0];
        assert_eq!((r.usage.uncached, r.model.as_str()), (0, "gpt-6.2-sol"), "a record whose response is not written yet is a request");
    }

    #[test]
    fn a_cache_lifetime_over_half_an_hour_is_the_long_one() {
        let written = |extra: Value| {
            let mut u = usage("m", "claude-opus-5.5", [1000, 0, 900, 5], 1);
            u.as_object_mut().unwrap().extend(extra.as_object().cloned().unwrap());
            let s = decode(&[record(AT, u)]);
            let u = s.chains[0].requests[0].usage;
            (u.write_short, u.write_long)
        };
        assert_eq!(written(json!({"cacheTtlSeconds": 300})), (900, 0));
        assert_eq!(written(json!({"cacheExpiresAt": "2026-11-25T22:10:45.000Z"})), (900, 0), "295 s after the line");
        assert_eq!(written(json!({"cacheTtlSeconds": 3600})), (0, 900));
        assert_eq!(written(json!({"cacheExpiresAt": "2026-11-25T23:05:45.000Z"})), (0, 900));
    }

    #[test]
    fn each_request_has_the_model_of_its_own_record() {
        let s = decode(&[
            record("2026-11-25T10:00:00.000Z", usage("a", "claude-sonnet-5.5", [10, 0, 0, 1], 1)),
            message("2026-11-25T10:00:00.001Z", Some("a"), json!([])),
            line("session.model_change", "2026-11-25T10:00:01.000Z", json!({"newModel": "claude-fable-5.2"})),
            record("2026-11-25T10:00:02.000Z", usage("b", "claude-fable-5.2", [10, 0, 0, 1], 1)),
            message("2026-11-25T10:00:02.001Z", Some("b"), json!([])),
        ]);
        let models: Vec<&str> = s.chains[0].requests.iter().map(|r| r.model.as_str()).collect();
        assert_eq!(models, ["claude-sonnet-5-5", "claude-fable-5-2"]);
    }

    #[test]
    fn a_response_without_a_record_is_a_request_without_usage() {
        let s = decode(&[
            line("assistant.turn_start", "2026-10-09T20:30:03.200Z", json!({"turnId": "0"})),
            message("2026-10-09T20:30:06.000Z", Some("m1"), json!([])),
        ]);
        let r = &s.chains[0].requests[0];
        assert!(r.no_usage && r.answered() && r.usage.prompt() == 0);
        assert_eq!((r.model.as_str(), r.run_ms()), ("claude-fable-5-1", Some(2800)), "sent at the turn's start");
        assert_eq!(s.reported_usd, None, "no credits, no cost");
    }

    #[test]
    fn a_message_the_cli_wrote_is_no_request() {
        let s = decode(&[
            message("2026-11-25T10:00:00.000Z", None, json!([requested("t", "read_bash", json!({"shellId": "7"}))])),
            started("2026-11-25T10:00:00.100Z", "t"),
            completed("2026-11-25T10:00:05.100Z", "t", json!({})),
        ]);
        assert!(s.chains[0].requests.is_empty());
        assert_eq!(s.platform_calls.len(), 1, "its call is a call no request made");
        assert_eq!((s.calls[0].duration_ms(), s.calls[0].input.as_deref()), (Some(5000), Some("7")));
    }

    #[test]
    fn a_compaction_marks_the_next_request_and_its_usage_is_a_call() {
        let used = json!({"model": "claude-sonnet-5.5", "inputTokens": 161982, "cacheReadTokens": 160307, "cacheWriteTokens": 1671, "outputTokens": 5181,
            "duration": 35074, "copilotUsage": {"totalNanoAiu": 7202620000u64}});
        let s = decode(&[
            record("2026-11-25T10:00:00.000Z", usage("a", "claude-sonnet-5.5", [10, 0, 0, 1], 0)),
            message("2026-11-25T10:00:00.001Z", Some("a"), json!([])),
            line("session.compaction_complete", "2026-11-25T10:01:00.000Z", json!({"success": false})),
            line(
                "session.compaction_complete",
                "2026-11-25T10:02:00.000Z",
                json!({"success": true, "summaryContent": "summary", "compactionTokensUsed": used}),
            ),
            record("2026-11-25T10:03:00.000Z", usage("b", "claude-sonnet-5.5", [10, 0, 0, 1], 0)),
            message("2026-11-25T10:03:00.001Z", Some("b"), json!([])),
        ]);
        let r = &s.chains[0].requests;
        let marks: Vec<(bool, bool)> = r.iter().map(|r| (r.compaction_call, r.compacted_before)).collect();
        assert_eq!(marks, [(false, false), (true, false), (false, true)], "the failed compaction marks nothing");
        let u = r[1].usage;
        assert_eq!((u.uncached, u.cache_read, u.cache_write(), u.output, r[1].model.as_str()), (4, 160307, 1671, 5181, "claude-sonnet-5-5"));
        assert_eq!(r[1].run_ms(), Some(35074));
        assert!((s.reported_usd.unwrap() - 0.0720262).abs() < 1e-9);
        assert_eq!(s.compactions.len(), 1);
        assert_eq!(r[2].prompt.as_ref().map(|p| (p.keep, p.tail.len())), Some((0, 1)), "the summary replaces the conversation");
    }

    #[test]
    fn calls_have_the_times_of_their_own_lines() {
        let s = decode(&[
            message(
                "2026-11-25T21:12:22.696Z",
                Some("m"),
                json!([requested("a", "bash", json!({"command": "ls\npwd"})), requested("b", "bash", json!({"command": "id"}))]),
            ),
            started("2026-11-25T21:12:22.697Z", "a"),
            started("2026-11-25T21:12:22.698Z", "b"),
            completed("2026-11-25T21:12:22.778Z", "a", json!({})),
        ]);
        assert_eq!((s.calls[0].duration_ms(), s.calls[0].done, s.calls[0].input.as_deref()), (Some(81), true, Some("ls")));
        assert_eq!((s.calls[1].start_ms.is_some(), s.calls[1].end_ms, s.calls[1].done), (true, None, false), "a call still running has no end");
        assert_eq!(s.blocks.iter().filter(|b| b.kind == BlockKind::ToolResult).count(), 1);
    }

    #[test]
    fn a_result_tells_how_its_call_ended() {
        assert_eq!(outcome(json!({})), (Outcome::Worked, None));
        assert_eq!(outcome(json!({"success": false, "error": {"message": "The user rejected this tool call.", "code": "rejected"}})).0, Outcome::Refused);
        assert_eq!(outcome(json!({"success": false, "error": {"message": "Permission denied.", "code": "denied"}})).0, Outcome::Denied);
        let failed = outcome(json!({"success": false, "error": {"message": "Invalid delay: 900.", "code": "failure"}}));
        assert_eq!(failed, (Outcome::Failed, Some("Invalid delay: 900.".to_string())));
        let exited = outcome(json!({"result": {"content": "a\nerror: boom\n"}, "shellExecution": {"exitCode": 1}}));
        assert_eq!(exited, (Outcome::Failed, Some("exit code 1: error: boom".to_string())), "though the log says it succeeded");
        assert_eq!(outcome(json!({"shellExecution": {"exitCode": 0}})).0, Outcome::Worked);
    }

    #[test]
    fn calls_in_the_background_and_questions_are_marked() {
        let at = "2026-11-25T10:00:00.000Z";
        let calls = json!([
            requested("a", "bash", json!({"command": "sleep 99"})),
            requested("b", "bash", json!({"command": "x"})),
            requested("c", "ask_user", json!({}))
        ]);
        let s = decode(&[
            message(at, Some("m"), calls),
            completed(at, "a", json!({"toolTelemetry": {"properties": {"executionMode": "async", "detached": "false"}}})),
            completed(at, "b", json!({"toolTelemetry": {"properties": {"executionMode": "sync", "detached": "true"}}})),
            started(at, "c"),
            completed(at, "c", json!({})),
        ]);
        let flags: Vec<(bool, bool)> = s.calls.iter().map(|c| (c.background, c.asks_user)).collect();
        assert_eq!(flags, [(true, false), (true, false), (false, true)]);
        let kinds: Vec<HandoffKind> = s.handoffs.iter().map(|h| h.kind).collect();
        assert_eq!(kinds, [HandoffKind::Question, HandoffKind::Answer]);
    }

    #[test]
    fn tools_have_their_kinds_and_input_heads() {
        let names = [
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
            "sql",
        ];
        use ToolClass::*;
        let kinds =
            [Execute, ExecuteOutput, Execute, Read, Edit, Write, Search, Search, Fetch, Subagent, Subagent, Subagent, Manage, Manage, Manage, Other, Other];
        assert_eq!(names.map(tool_class), kinds);
        let head = |name: &str, arguments: Value| call(&requested("t", name, arguments)).unwrap().input;
        assert_eq!(head("bash", json!({"command": "cargo test\necho done"})).as_deref(), Some("cargo test"));
        assert_eq!(head("view", json!({"path": "/a/b.rs"})).as_deref(), Some("/a/b.rs"));
        assert_eq!(head("grep", json!({"pattern": "fn parse", "path": "src"})).as_deref(), Some("fn parse"));
        assert_eq!(head("web_fetch", json!({"url": "https://example.com"})).as_deref(), Some("https://example.com"));
        assert_eq!(head("task", json!({"name": "review", "prompt": "long"})).as_deref(), Some("review"));
        assert_eq!(head("skill", json!({"skill": "release"})).as_deref(), Some("release"));
        assert_eq!(head("sql", json!({"query": "select 1"})), None, "a tool the source does not list");
    }

    #[test]
    fn an_mcp_tool_is_a_tool_of_its_server() {
        let at = "2026-11-25T10:00:00.000Z";
        let s = decode(&[
            message(at, Some("m"), json!([requested("t", "web_search", json!({"query": "rust parsers"}))])),
            line(
                "tool.execution_start",
                at,
                json!({"toolCallId": "t", "toolName": "web_search", "mcpServerName": "github-mcp-server", "mcpToolName": "web_search"}),
            ),
        ]);
        let b = s.blocks.iter().find(|b| b.kind == BlockKind::ToolUse).unwrap();
        assert_eq!(
            (b.tool.as_deref(), b.target.as_deref(), b.class),
            (Some("github-mcp-server-web_search"), Some("mcp:github-mcp-server"), Class::Tool(ToolClass::Other))
        );
        assert_eq!(s.calls[0].input.as_deref(), Some("rust parsers"), "the first string value");
        assert!(s.mcp_servers.is_empty(), "the log does not list the servers a session had");
    }

    #[test]
    fn the_lines_of_an_agent_id_are_a_subagent_chain() {
        let at = "2026-11-25T10:00:00.000Z";
        let sub = |l: Value| of_agent(l, "61182870");
        let s = decode(&[
            line("user.message", at, json!({"content": "review it"})),
            record(at, usage("a", "claude-fable-5.2", [10, 0, 0, 1], 1000000000)),
            message(at, Some("a"), json!([requested("t", "task", json!({"name": "review-factory-template"}))])),
            sub(line("subagent.started", at, json!({"toolCallId": "t", "agentName": "code-review", "agentDisplayName": "review-factory-template"}))),
            sub(line("user.message", at, json!({"content": "the brief", "source": "agent-x"}))),
            sub(record(at, usage("b", "claude-fable-5.2", [10, 0, 0, 1], 2000000000))),
            sub(message(at, Some("b"), json!([]))),
        ]);
        let chains: Vec<(&str, ChainKind, usize)> = s.chains.iter().map(|c| (c.name.as_str(), c.kind, c.requests.len())).collect();
        assert_eq!(chains, [("main", ChainKind::Main, 1), ("subagent: review-factory-template", ChainKind::Subagent, 1)]);
        assert_eq!(s.typed.len(), 1, "the brief is no typed message");
        assert_eq!(s.chains[1].requests[0].trigger, Some(Trigger::Prompt));
        assert!((s.reported_usd.unwrap() - 0.03).abs() < 1e-9, "the subagent's credits count in the session");
        assert!(s.handoffs.is_empty(), "a subagent's last response ends no turn of the session");
    }

    #[test]
    fn a_message_without_a_source_is_typed() {
        let s = decode(&[
            line("user.message", "2026-11-25T10:00:00.000Z", json!({"content": "fix the failing test", "transformedContent": "<now/>\nfix the failing test"})),
            line("user.message", "2026-11-25T10:00:05.000Z", json!({"content": "the nested one", "delivery": "steering"})),
            line("user.message", "2026-11-25T10:00:09.000Z", json!({"content": "Continue.", "source": "autopilot", "isAutopilotContinuation": true})),
            line("user.message", "2026-11-25T10:00:10.000Z", json!({"content": "a skill", "source": "skill-release"})),
        ]);
        let typed: Vec<&str> = s.typed.iter().map(|t| t.label.as_str()).collect();
        assert_eq!(typed, ["fix the failing test", "the nested one"]);
        let kinds: Vec<(BlockKind, Class, u64)> = s.blocks.iter().map(|b| (b.kind, b.class, b.chars)).collect();
        use ReminderKind::*;
        let expected = [
            (BlockKind::Prompt, Class::None, 27),
            (BlockKind::Prompt, Class::None, 14),
            (BlockKind::Reminder, Class::Reminder(Other), 9),
            (BlockKind::Reminder, Class::Reminder(Skills), 7),
        ];
        assert_eq!(kinds, expected, "the size is what the model read");
        assert_eq!(s.messages.iter().map(|m| m.starts_turn).collect::<Vec<_>>(), [true, true, false, false]);
        assert_eq!(s.handoffs.iter().map(|h| h.kind).collect::<Vec<_>>(), [HandoffKind::TypedWhileWorking]);
    }

    #[test]
    fn lines_the_decoder_does_not_read_are_skipped_unparsed() {
        assert_eq!(leading_type(r#"{"type":"session.usage_checkpoint","data":{"#), Some("session.usage_checkpoint"));
        assert!(parse_line(r#"{"type":"session.usage_checkpoint","data":{"#).is_none(), "not even valid JSON");
        assert!(parse_line(r#"{"data":{},"type":"session.shutdown","timestamp":"2026-11-25T10:00:00.000Z"}"#).is_none());
        assert!(parse_line(r#"{"data":{"turnId":"0"},"type":"assistant.turn_start","timestamp":"2026-11-25T10:00:00.000Z"}"#).is_some(), "keys in any order");
    }
}
