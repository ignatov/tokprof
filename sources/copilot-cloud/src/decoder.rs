//! The lines of a saved Copilot log, and the session they give.
//!
//! The log is server-sent events: each useful line is `data: <JSON object>`. A model response is
//! a chat chunk; every chunk of a response shares its `id`. The first chunk of an `id` holds the
//! response: its text and all its calls. For each call that returned, a `tool` object holds the
//! result, and then a chunk of the same `id` holds that one call again with the result as its
//! `content`; that chunk's `created` is when the call ended. Setup steps (`run_setup`,
//! `run_custom_setup_step`) come the same way before the first user message, without `tool`
//! objects. No line holds the usage of a request (seen in logs of 2026-11-14 to 2026-11-21).
//!
//! The decoder keeps what each line tells, a few numbers and short heads, and builds the session
//! from them whenever it is asked, so a snapshot is the session of the lines fed so far.

use std::collections::{HashMap, HashSet};
use std::path::Path;

use serde_json::{Map, Value};
use tokprof_core::labels::{command_head, exit_line, first_line, message_label};
use tokprof_core::model::{
    Block, BlockKind, CallEnd, CallRecord, Chain, ChainKind, Class, HandoffKind, Outcome, PromptDelta, ReminderKind, Request, Session, ToolClass, TypedMessage,
    input_head,
};
use tokprof_core::source::{AgentId, Decoder};
use tokprof_core::trigger::{CallClass, Pending, Trigger, VcsActions, command_class};

/// What the session's record gives (session-logs, Copilot session records).
#[derive(Clone, Debug, Default)]
pub struct Record {
    pub model: Option<String>,
    pub title: Option<String>,
    /// The credits GitHub billed, `usage.amount / 1e9`.
    pub credits: Option<f64>,
}

/// Dollars at list price per credit.
pub const USD_PER_CREDIT: f64 = 0.01;

impl Record {
    /// The record at `path`, when its `id` is the session's.
    pub fn read(path: &Path, id: &str) -> Option<Record> {
        let v: Value = serde_json::from_slice(&std::fs::read(path).ok()?).ok()?;
        (v.get("id").and_then(Value::as_str) == Some(id)).then(|| Record::of(&v))
    }

    pub fn of(v: &Value) -> Record {
        let text = |k: &str| v.get(k).and_then(Value::as_str).filter(|s| !s.is_empty()).map(str::to_string);
        let usage = v.get("usage");
        let credits = usage
            .filter(|u| u.get("type").and_then(Value::as_str) == Some("ai_credits"))
            .and_then(|u| u.get("amount"))
            .and_then(Value::as_f64)
            .map(|amount| amount / 1e9);
        Record { model: text("model").map(|m| model_name(&m)), title: text("name"), credits }
    }
}

/// A record's model as tokprof names it: after the last `:`, with the dots of a `claude-` id as
/// dashes (`sweagent-capi:claude-fable-5.2` is `claude-fable-5-2`).
pub fn model_name(id: &str) -> String {
    let name = id.rsplit(':').next().unwrap_or(id);
    if name.starts_with("claude-") { name.replace('.', "-") } else { name.to_string() }
}

/// The repository a clone step names: `Clone repository <owner>/<repo>`.
pub fn cloned_repository(step: &str) -> Option<&str> {
    step.strip_prefix("Clone repository ").map(str::trim).filter(|r| r.contains('/') && !r.contains(' '))
}

/// The server a step started: `Start '<server>' MCP server`.
fn started_server(step: &str) -> Option<&str> {
    step.strip_prefix("Start '")?.strip_suffix("' MCP server")
}

/// A `run_setup` that marks where a subagent begins or ends, not a step.
fn is_marker(step: &str) -> bool {
    ["▶️ Begin subagent: ", "⏹️ End subagent: ", "❌ Failed subagent: "].iter().any(|m| step.starts_with(m))
}

const SETUP_TOOLS: [&str; 2] = ["run_setup", "run_custom_setup_step"];

/// A call as a chunk gives it.
#[derive(Clone, Debug)]
pub struct Call {
    pub id: String,
    pub name: String,
    pub args_chars: u64,
    /// The `name` of a setup step or a marker; the name of a `task` subagent.
    pub step: Option<String>,
    /// The shell command of `bash`, the path of a file tool, the pattern of a search.
    target: Option<String>,
    input: Option<String>,
    class: CallClass,
    vcs: VcsActions,
    /// The first string value of the arguments, for an MCP tool's input head.
    first_string: Option<String>,
}

/// A chat chunk.
#[derive(Clone, Debug)]
pub struct Chunk {
    pub id: String,
    pub ms: i64,
    agent: Option<String>,
    /// The characters of `content`, when it has some.
    content: Option<u64>,
    pub calls: Vec<Call>,
}

/// One line of a log.
#[derive(Clone, Debug)]
pub enum Line {
    Chunk(Chunk),
    User { agent: Option<String>, source: Option<String>, chars: u64, label: String },
    Tool(ToolResult),
}

#[derive(Clone, Debug)]
pub struct ToolResult {
    call: String,
    chars: u64,
    tokens: Option<u64>,
    outcome: Outcome,
    error: Option<String>,
    background: bool,
}

/// A chunk's `created`: epoch seconds, as a number or a string, or epoch milliseconds when it has
/// 13 digits or more.
fn created_ms(v: Option<&Value>) -> Option<i64> {
    let n = match v? {
        Value::Number(n) => n.as_f64()?,
        Value::String(s) => s.trim().parse::<f64>().ok()?,
        _ => return None,
    };
    Some(if n >= 1e12 { n as i64 } else { (n * 1000.0) as i64 })
}

fn text<'a>(o: &'a Map<String, Value>, k: &str) -> Option<&'a str> {
    o.get(k).and_then(Value::as_str)
}

fn chars(s: &str) -> u64 {
    s.chars().count() as u64
}

/// The line, when it is a data line of an object the decoder reads.
pub fn parse_line(line: &str) -> Option<Line> {
    let body = line.trim_end_matches(['\n', '\r']).strip_prefix("data:")?.trim();
    let Ok(Value::Object(o)) = serde_json::from_str::<Value>(body) else { return None };
    let agent = text(&o, "agentId").filter(|a| !a.is_empty()).map(str::to_string);
    if let Some(choices) = o.get("choices").and_then(Value::as_array) {
        let delta = choices.first()?.get("delta")?;
        let calls = delta.get("tool_calls").and_then(Value::as_array).map(|c| c.iter().filter_map(call).collect()).unwrap_or_default();
        return Some(Line::Chunk(Chunk {
            id: text(&o, "id")?.to_string(),
            ms: created_ms(o.get("created"))?,
            agent,
            content: delta.get("content").and_then(Value::as_str).map(chars),
            calls,
        }));
    }
    let content = text(&o, "content").unwrap_or_default();
    match text(&o, "role")? {
        "user" => Some(Line::User { agent, source: text(&o, "source").map(str::to_string), chars: chars(content), label: message_label(content) }),
        "tool" => {
            let tele = o.get("toolTelemetry");
            let metrics = tele.and_then(|t| t.get("metrics"));
            let props = tele.and_then(|t| t.get("properties"));
            let prop = |k: &str| props.and_then(|p| p.get(k)).and_then(Value::as_str);
            let exit = metrics.and_then(|m| m.get("exit_code")).and_then(Value::as_i64);
            let outcome = match prop("shell_error_category") {
                Some("command_nonzero_exit" | "invalid_input" | "shell_unknown") => Outcome::Failed,
                Some("kill_command_blocked") => Outcome::Denied,
                _ => Outcome::Worked,
            };
            let error = (outcome == Outcome::Failed).then(|| exit.filter(|&c| c != 0).map(|c| exit_line(c, content)).or_else(|| first_line(content))).flatten();
            Some(Line::Tool(ToolResult {
                call: text(&o, "tool_call_id")?.to_string(),
                chars: chars(content),
                tokens: metrics.and_then(|m| m.get("result_token_count")).and_then(Value::as_u64),
                outcome,
                error,
                background: prop("executionMode") == Some("async") || prop("detached") == Some("true"),
            }))
        }
        _ => None,
    }
}

/// A call of a chunk, with what the session needs of its arguments.
fn call(v: &Value) -> Option<Call> {
    let f = v.get("function")?;
    let name = f.get("name").and_then(Value::as_str)?.to_string();
    let raw = f.get("arguments").and_then(Value::as_str).unwrap_or_default();
    let args: Value = serde_json::from_str(raw).unwrap_or(Value::Null);
    let arg = |k: &str| args.get(k).and_then(Value::as_str).filter(|s| !s.trim().is_empty());
    let command = arg("command");
    let (target, input, class) = match name.as_str() {
        "bash" => (command.map(command_head), command.and_then(input_head), command.map_or(CallClass::System, command_class)),
        "read_bash" | "stop_bash" => (None, arg("shellId").and_then(input_head), CallClass::System),
        "view" => (arg("path").map(str::to_string), arg("path").and_then(input_head), CallClass::Reading),
        "edit" | "create" => (arg("path").map(str::to_string), arg("path").and_then(input_head), CallClass::Editing),
        "grep" | "glob" => (arg("pattern").map(str::to_string), arg("pattern").and_then(input_head), CallClass::Reading),
        "web_fetch" => (None, arg("url").and_then(input_head), CallClass::Web),
        "web_search" => (None, arg("query").and_then(input_head), CallClass::Web),
        "task" | "read_agent" => (None, arg("name").or(arg("description")).or(arg("agent_id")).and_then(input_head), CallClass::Subagents),
        "skill" => (None, arg("skill").and_then(input_head), CallClass::OtherTools),
        "report_progress" => (None, arg("commitMessage").and_then(input_head), CallClass::VersionControl),
        "run_setup" | "run_custom_setup_step" => (None, arg("name").and_then(input_head), CallClass::OtherTools),
        _ => (None, None, CallClass::OtherTools),
    };
    let step = match name.as_str() {
        "run_setup" | "run_custom_setup_step" | "task" => arg("name").map(str::to_string),
        _ => None,
    };
    let first_string = args.as_object().and_then(|o| o.values().find_map(|v| v.as_str().filter(|s| !s.trim().is_empty()).and_then(input_head)));
    // `report_progress` commits the work and pushes it.
    let vcs = VcsActions { commit: name == "report_progress", push: name == "report_progress", pull_request: false };
    Some(Call { id: v.get("id").and_then(Value::as_str)?.to_string(), args_chars: chars(raw), name, step, target, input, class, vcs, first_string })
}

/// The kind of a tool (session-logs, Kind of a tool); an MCP tool is `other`.
fn tool_class(name: &str) -> ToolClass {
    match name {
        "bash" | "stop_bash" => ToolClass::Execute,
        "read_bash" => ToolClass::ExecuteOutput,
        "view" => ToolClass::Read,
        "edit" => ToolClass::Edit,
        "create" => ToolClass::Write,
        "grep" | "glob" => ToolClass::Search,
        "web_fetch" | "web_search" => ToolClass::Fetch,
        "task" | "read_agent" => ToolClass::Subagent,
        "skill" => ToolClass::Manage,
        "run_setup" | "run_custom_setup_step" => ToolClass::EnvironmentSetup,
        _ => ToolClass::Other,
    }
}

/// The decoder of one log: it keeps the lines it read, and builds the session from them.
pub struct Parser {
    agent: AgentId,
    id: String,
    record: Record,
    lines: Vec<Line>,
}

impl Parser {
    pub fn new(agent: AgentId, id: String, record: Record) -> Parser {
        Parser { agent, id, record, lines: Vec::new() }
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
    /// The latest time logged in the chain.
    last_ms: Option<i64>,
}

impl ChainState {
    fn new(name: String, kind: ChainKind) -> ChainState {
        ChainState { name, kind, requests: Vec::new(), held: 0, tail: Vec::new(), pending: Pending::default(), last_ms: None }
    }

    fn saw(&mut self, ms: i64) {
        self.last_ms = self.last_ms.max(Some(ms));
    }
}

/// A call while the session is built.
struct CallState {
    record: usize,
    blocks: Vec<usize>,
    chain: usize,
    /// The call is a setup step, made by no request.
    setup: bool,
    has_result: bool,
    tool: String,
    class: Class,
    target: Option<String>,
    vcs: VcsActions,
    outcome: Option<(Outcome, Option<String>)>,
}

struct Build<'a> {
    p: &'a Parser,
    s: Session,
    chains: Vec<ChainState>,
    by_agent: HashMap<String, usize>,
    seen: HashSet<String>,
    calls: HashMap<String, CallState>,
    records: Vec<CallRecord>,
    /// The names of the `task` calls whose subagents have not started yet, oldest first.
    tasks: Vec<String>,
    first_ms: Option<i64>,
    typed: Option<String>,
}

impl<'a> Build<'a> {
    fn new(p: &'a Parser) -> Build<'a> {
        let mut s = Session::new(p.agent.clone(), p.id.clone());
        s.title = p.record.title.clone();
        s.reported_usd = p.record.credits.map(|c| c * USD_PER_CREDIT);
        Build {
            p,
            s,
            chains: vec![ChainState::new("main".to_string(), ChainKind::Main)],
            by_agent: HashMap::new(),
            seen: HashSet::new(),
            calls: HashMap::new(),
            records: Vec::new(),
            tasks: Vec::new(),
            first_ms: None,
            typed: None,
        }
    }

    /// The chain of an `agentId`: the main one for none or a custom agent's, else a subagent's,
    /// started when it is first seen.
    fn chain(&mut self, agent: Option<&str>) -> usize {
        let Some(agent) = agent.filter(|a| !a.starts_with("selected-custom-agent-")) else { return 0 };
        if let Some(&c) = self.by_agent.get(agent) {
            return c;
        }
        let name = if self.tasks.is_empty() { "subagent".to_string() } else { format!("subagent: {}", self.tasks.remove(0)) };
        self.chains.push(ChainState::new(name, ChainKind::Subagent));
        self.by_agent.insert(agent.to_string(), self.chains.len() - 1);
        self.chains.len() - 1
    }

    fn push(&mut self, chain: usize, blocks: Vec<Block>, starts_turn: bool) -> usize {
        let m = self.s.push_message(blocks, starts_turn);
        self.chains[chain].tail.push(m);
        m
    }

    fn run(mut self, lines: &[Line]) -> Session {
        for line in lines {
            match line {
                Line::Chunk(c) => self.chunk(c),
                Line::User { agent, source, chars, label } => self.user(agent.as_deref(), source.as_deref(), *chars, label),
                Line::Tool(t) => self.tool(t),
            }
        }
        self.finish()
    }

    fn chunk(&mut self, c: &Chunk) {
        self.first_ms.get_or_insert(c.ms);
        if !self.seen.insert(c.id.clone()) {
            return self.repeat(c);
        }
        let setup = !c.calls.is_empty() && c.calls.iter().all(|call| SETUP_TOOLS.contains(&call.name.as_str()));
        if setup {
            for call in &c.calls {
                let step = call.step.as_deref().unwrap_or_default();
                if is_marker(step) {
                    continue;
                }
                if let Some(server) = started_server(step).filter(|s| !self.s.mcp_servers.iter().any(|x| x == s)) {
                    self.s.mcp_servers.push(server.to_string());
                }
                if self.s.project.is_empty()
                    && let Some(repo) = cloned_repository(step)
                {
                    self.s.project = format!("github.com/{repo}");
                }
                self.step(call, c.ms);
            }
            self.chains[0].saw(c.ms);
            return;
        }
        let chain = self.chain(c.agent.as_deref());
        let mut blocks = Vec::new();
        if let Some(n) = c.content.filter(|&n| n > 0) {
            blocks.push(Block { kind: BlockKind::Text, class: Class::None, chars: n, tool: None, target: None });
        }
        let first_block = self.s.blocks.len() + blocks.len();
        let mut made = Vec::new();
        for (i, call) in c.calls.iter().enumerate() {
            let (tool, class, target) = self.tool_of(call);
            blocks.push(Block { kind: BlockKind::ToolUse, class, chars: call.args_chars, tool: Some(tool.clone()), target: target.clone() });
            made.push((first_block + i, call, tool, class, target));
        }
        let st = &mut self.chains[chain];
        let tail = std::mem::take(&mut st.tail);
        let keep = st.held;
        st.held += tail.len();
        // A chain never goes back in time, though the lines may.
        let ts_ms = st.requests.last().map_or(c.ms, |r| r.ts_ms.max(c.ms));
        let sent = st.last_ms.map_or(ts_ms, |t| t.min(ts_ms));
        let trigger = st.pending.trigger().or((!st.requests.is_empty()).then_some(Trigger::Continuation));
        let activity = st.pending.activity();
        st.pending = Pending::default();
        let produced = self.s.push_message(blocks, false);
        let st = &mut self.chains[chain];
        st.tail.push(produced);
        st.saw(c.ms);
        st.requests.push(Request {
            ts_ms,
            model: self.p.record.model.clone().unwrap_or_else(|| "unknown".to_string()),
            prompt: Some(PromptDelta { shape: None, keep, tail }),
            produced: Some(produced),
            trigger,
            activity,
            first_sent_ms: Some(sent),
            sent_ms: Some(sent),
            ended_ms: Some(ts_ms),
            no_usage: true,
            ..Request::default()
        });
        for (b, call, tool, class, target) in made {
            self.chains[chain].pending.call(call.class);
            if call.name == "task" {
                self.tasks.extend(call.step.clone());
            }
            let input = if matches!(class, Class::Tool(ToolClass::Other)) && target.as_deref().is_some_and(|t| t.starts_with("mcp:")) {
                call.first_string.clone()
            } else {
                call.input.clone()
            };
            self.records.push(CallRecord { start_ms: Some(c.ms), end_ms: None, done: false, background: false, asks_user: false, input });
            self.calls.entry(call.id.clone()).or_insert(CallState {
                record: self.records.len() - 1,
                blocks: vec![b],
                chain,
                setup: false,
                has_result: false,
                tool,
                class,
                target,
                vcs: call.vcs,
                outcome: None,
            });
        }
    }

    /// A setup step: a call no request made.
    fn step(&mut self, call: &Call, ms: i64) {
        if let Some(st) = self.calls.get(&call.id) {
            // Seen before with another chunk id: it ran between the two times.
            let r = &mut self.records[st.record];
            let start = r.start_ms.unwrap_or(ms);
            (r.start_ms, r.end_ms, r.done) = (Some(start.min(ms)), Some(start.max(ms)), true);
            return;
        }
        let class = Class::Tool(ToolClass::EnvironmentSetup);
        let block = Block { kind: BlockKind::ToolUse, class, chars: call.args_chars, tool: Some(call.name.clone()), target: None };
        let m = self.s.push_message(vec![block], false);
        let b = self.s.messages[m].blocks.start;
        self.s.platform_calls.push(b);
        self.records.push(CallRecord { start_ms: Some(ms), end_ms: None, done: false, background: false, asks_user: false, input: call.input.clone() });
        let st = CallState {
            record: self.records.len() - 1,
            blocks: vec![b],
            chain: 0,
            setup: true,
            has_result: true,
            tool: call.name.clone(),
            class,
            target: None,
            vcs: VcsActions::default(),
            outcome: None,
        };
        self.calls.insert(call.id.clone(), st);
    }

    /// A chunk of an `id` seen before: the calls it repeats ended at its time.
    fn repeat(&mut self, c: &Chunk) {
        for call in &c.calls {
            // A subagent's markers carry the id of its `task` call, which they neither start nor end.
            if call.step.as_deref().is_some_and(is_marker) {
                continue;
            }
            let b = self.s.blocks.len();
            let Some(st) = self.calls.get_mut(&call.id) else { continue };
            let r = &mut self.records[st.record];
            if st.setup {
                let start = r.start_ms.unwrap_or(c.ms);
                (r.start_ms, r.end_ms, r.done) = (Some(start.min(c.ms)), Some(start.max(c.ms)), true);
                continue;
            }
            if r.end_ms.is_none() {
                (r.end_ms, r.done) = (Some(c.ms.max(r.start_ms.unwrap_or(c.ms))), true);
            }
            let chain = st.chain;
            // A call without a `tool` object has its result here.
            let result = match c.content {
                Some(n) if !st.has_result && c.calls.len() == 1 => {
                    st.has_result = true;
                    st.blocks.push(b);
                    Some(Block { kind: BlockKind::ToolResult, class: st.class, chars: n, tool: Some(st.tool.clone()), target: st.target.clone() })
                }
                _ => None,
            };
            self.chains[chain].saw(c.ms);
            if let Some(block) = result {
                self.push(chain, vec![block], false);
            }
        }
    }

    fn user(&mut self, agent: Option<&str>, source: Option<&str>, chars: u64, label: &str) {
        let known = agent.is_none_or(|a| a.starts_with("selected-custom-agent-") || self.by_agent.contains_key(a));
        let chain = self.chain(agent);
        let prompt = Block { kind: BlockKind::Prompt, class: Class::None, chars, tool: None, target: None };
        if chain != 0 && !known {
            // A subagent's brief.
            self.push(chain, vec![prompt], true);
            self.chains[chain].pending.event(Trigger::Prompt);
        } else if chain == 0 && self.typed.is_none() {
            self.typed = Some(label.to_string());
            self.push(0, vec![prompt], true);
            self.chains[0].pending.event(Trigger::Prompt);
        } else {
            let skill = source.is_some_and(|s| s.starts_with("skill-"));
            let kind = if skill { ReminderKind::Skills } else { ReminderKind::Other };
            let target = Some(if skill { "skills" } else { "other" }.to_string());
            self.push(chain, vec![Block { kind: BlockKind::Reminder, class: Class::Reminder(kind), chars, tool: None, target }], false);
        }
    }

    fn tool(&mut self, t: &ToolResult) {
        let b = self.s.blocks.len();
        let Some(st) = self.calls.get_mut(&t.call).filter(|st| !st.setup && !st.has_result) else { return };
        st.has_result = true;
        st.outcome = Some((t.outcome, t.error.clone()));
        st.blocks.push(b);
        self.records[st.record].background |= t.background;
        let block = Block { kind: BlockKind::ToolResult, class: st.class, chars: t.chars, tool: Some(st.tool.clone()), target: st.target.clone() };
        // The result goes to the chain of its call, whatever `agentId` it carries.
        let chain = st.chain;
        if let Some(tokens) = t.tokens {
            self.s.logged_tokens.insert(b, tokens);
        }
        self.push(chain, vec![block], false);
    }

    /// A call's tool name, class and target; an MCP tool of a started server names it.
    fn tool_of(&self, call: &Call) -> (String, Class, Option<String>) {
        let server = self.s.mcp_servers.iter().filter(|s| call.name.starts_with(&format!("{s}-"))).max_by_key(|s| s.len());
        match server {
            Some(server) => (call.name.clone(), Class::Tool(ToolClass::Other), Some(format!("mcp:{server}"))),
            None => (call.name.clone(), Class::Tool(tool_class(&call.name)), call.target.clone()),
        }
    }

    fn finish(mut self) -> Session {
        if let (Some(label), Some(ts_ms)) = (self.typed.take(), self.first_ms) {
            self.s.typed.push(TypedMessage { ts_ms, label });
        }
        if let Some(end) = self.chains[0].requests.last().and_then(|r| r.ended_ms) {
            self.s.push_handoff(end, HandoffKind::TurnEnd);
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
        self.s.calls = std::mem::take(&mut self.records);
        self.s.chains = self.chains.into_iter().map(|c| Chain { name: c.name, kind: c.kind, requests: c.requests }).collect();
        self.s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn decode(lines: &[&str], record: Record) -> Session {
        let mut p = Box::new(Parser::new(AgentId::unknown("copilot"), "s".into(), record));
        lines.iter().for_each(|l| p.feed(l));
        p.finish()
    }

    fn chunk(id: &str, created: &str, content: Option<&str>, calls: &[(&str, &str, &str)]) -> String {
        let calls: Vec<Value> =
            calls.iter().map(|(id, name, args)| serde_json::json!({"id": id, "type": "function", "function": {"name": name, "arguments": args}})).collect();
        let mut delta = serde_json::json!({"role": "assistant", "tool_calls": calls});
        if let Some(c) = content {
            delta["content"] = c.into();
        }
        let created: Value = serde_json::from_str(created).unwrap();
        format!(
            "data: {}",
            serde_json::json!({"id": id, "created": created, "model": "", "object": "chat.completion.chunk", "choices": [{"index": 0, "finish_reason": "tool_calls", "delta": delta}]})
        )
    }

    fn tool(call: &str, content: &str, metrics: Value, props: Value) -> String {
        format!(
            "data: {}",
            serde_json::json!({"role": "tool", "tool_call_id": call, "content": content, "toolTelemetry": {"metrics": metrics, "properties": props}})
        )
    }

    #[test]
    fn times_in_seconds_strings_and_milliseconds() {
        assert_eq!(created_ms(Some(&serde_json::json!(1800000100))), Some(1_800_000_100_000));
        assert_eq!(created_ms(Some(&serde_json::json!("1800000100"))), Some(1_800_000_100_000));
        assert_eq!(created_ms(Some(&serde_json::json!(1800000099523_i64))), Some(1_800_000_099_523));
        assert_eq!(created_ms(Some(&serde_json::json!(null))), None);
    }

    #[test]
    fn model_ids_as_tokprof_names_them() {
        assert_eq!(model_name("sweagent-capi:claude-fable-5.2"), "claude-fable-5-2");
        assert_eq!(model_name("sweagent-capi:gpt-6.2-sol"), "gpt-6.2-sol");
        assert_eq!(model_name("claude-opus-5-5"), "claude-opus-5-5");
    }

    #[test]
    fn records_give_credits_only_with_usage() {
        let r = Record::of(
            &serde_json::json!({"id": "s", "model": "sweagent-capi:claude-opus-5.5", "name": "Fix it", "usage": {"amount": 150000000000_u64, "type": "ai_credits"}}),
        );
        assert_eq!((r.model.as_deref(), r.title.as_deref(), r.credits), (Some("claude-opus-5-5"), Some("Fix it"), Some(150.0)));
        assert_eq!(Record::of(&serde_json::json!({"id": "s", "model": "m"})).credits, None, "no usage, no cost");
    }

    #[test]
    fn one_request_per_response_with_its_calls_and_their_ends() {
        let lines = [
            chunk("a1", "1800000000", None, &[("c1", "view", r#"{"path":"/w/a.rs"}"#), ("c2", "bash", r#"{"command":"cd /w && cargo test -q"}"#)]),
            tool("c1", "fn main() {}", serde_json::json!({"result_token_count": 5}), serde_json::json!({})),
            chunk("a1", "1800000001", Some("fn main() {}"), &[("c1", "view", r#"{"path":"/w/a.rs"}"#)]),
            tool("c2", "error: 1 failed", serde_json::json!({"exit_code": 101}), serde_json::json!({"shell_error_category": "command_nonzero_exit"})),
            chunk("a1", "1800000009", Some("error: 1 failed"), &[("c2", "bash", r#"{"command":"cd /w && cargo test -q"}"#)]),
            chunk("a2", "1800000012", Some("Done."), &[]),
        ];
        let lines: Vec<&str> = lines.iter().map(String::as_str).collect();
        let s = decode(&lines, Record { model: Some("claude-opus-5-5".into()), ..Record::default() });
        let reqs = &s.chains[0].requests;
        assert_eq!(reqs.len(), 2, "one request per response id");
        assert!(reqs.iter().all(|r| r.no_usage && r.answered()));
        assert_eq!((reqs[1].sent_ms, reqs[1].ended_ms), (Some(1_800_000_009_000), Some(1_800_000_012_000)), "sent after the last call it waited for");
        assert_eq!(reqs[1].activity(), "building and testing");
        let durations: Vec<_> = s.calls.iter().map(CallRecord::duration_ms).collect();
        assert_eq!(durations, [Some(1_000), Some(9_000)]);
        let failed: Vec<_> = s.call_ends.values().map(|e| (e.outcome, e.error.clone())).collect();
        assert!(failed.contains(&(Outcome::Failed, Some("exit code 101: error: 1 failed".into()))));
        assert_eq!(s.logged_tokens.values().copied().collect::<Vec<_>>(), [5]);
        let bash = s.blocks.iter().find(|b| b.tool.as_deref() == Some("bash")).unwrap();
        assert_eq!(bash.target.as_deref(), Some("cargo test"), "the command head");
    }

    #[test]
    fn setup_steps_markers_servers_and_the_project() {
        let lines = [
            chunk("s1", "1800000000", None, &[("k1", "run_custom_setup_step", r#"{"name":"Install the JDK"}"#)]),
            chunk("s1", "1800000255", Some("ok"), &[("k1", "run_custom_setup_step", r#"{"name":"Install the JDK"}"#)]),
            chunk("clone-1", "1800000256", None, &[("clone-1", "run_setup", r#"{"name":"Clone repository octo/app"}"#)]),
            chunk("mcp-1", "1800000257500", Some("started"), &[("mcp-1", "run_setup", r#"{"name":"Start 'docs' MCP server"}"#)]),
            chunk("mcp-1x", "1800000257", None, &[("mcp-1", "run_setup", r#"{"name":"Start 'docs' MCP server"}"#)]),
            chunk("selected-custom-agent-helper-start", "1800000258", None, &[("m1", "run_setup", r#"{"name":"▶️ Begin subagent: helper"}"#)]),
            r#"data: {"role":"user","content":"Fix the bug"}"#.to_string(),
            chunk("a1", "1800000260", None, &[("c1", "docs-search", r#"{"query":"parser"}"#)]),
        ];
        let lines: Vec<&str> = lines.iter().map(String::as_str).collect();
        let s = decode(&lines, Record::default());
        assert_eq!(s.project, "github.com/octo/app");
        assert_eq!(s.mcp_servers, ["docs"]);
        assert_eq!(s.platform_calls.len(), 3, "the marker is no step");
        let step = s.call(s.platform_calls[0]).unwrap();
        assert_eq!((step.duration_ms(), step.input.as_deref()), (Some(255_000), Some("Install the JDK")));
        let mcp = s.call(s.platform_calls[2]).unwrap();
        assert_eq!(mcp.duration_ms(), Some(500), "a repeat logged before its call");
        assert_eq!(s.chains[0].requests.len(), 1, "setup chunks are no requests");
        assert_eq!(s.typed[0].ts_ms, 1_800_000_000_000, "the prompt at the log's first chunk");
        let call = s.blocks.iter().find(|b| b.tool.as_deref() == Some("docs-search")).unwrap();
        assert_eq!(call.target.as_deref(), Some("mcp:docs"));
        assert_eq!(s.chains.len(), 1, "a custom agent is the main conversation");
    }

    #[test]
    fn a_task_subagent_is_a_chain_of_its_own() {
        let lines = [
            r#"data: {"role":"user","content":"Go"}"#.to_string(),
            chunk("a1", "1800000000", None, &[("t1", "task", r#"{"name":"check-tests","prompt":"Check"}"#)]),
            chunk("t1", "1800000001", Some("begin"), &[("t1", "run_setup", r#"{"name":"▶️ Begin subagent: check-tests"}"#)]),
            r#"data: {"role":"user","content":"Check","agentId":"sub-1","source":"agent-s"}"#.to_string(),
            chunk("b1", "1800000002", None, &[("x1", "bash", r#"{"command":"ls"}"#)]).replace(r#""choices""#, r#""agentId":"sub-1","choices""#),
            tool("x1", "a b", serde_json::json!({}), serde_json::json!({})).replace(r#""role""#, r#""agentId":"sub-1","role""#),
            chunk("b1", "1800000003", Some("a b"), &[("x1", "bash", r#"{"command":"ls"}"#)]).replace(r#""choices""#, r#""agentId":"sub-1","choices""#),
            chunk("t1", "1800000010", Some("end"), &[("t1", "run_setup", r#"{"name":"⏹️ End subagent: check-tests"}"#)]),
            chunk("a1", "1800000011", Some("Checked."), &[("t1", "task", r#"{"name":"check-tests","prompt":"Check"}"#)]),
        ];
        let lines: Vec<&str> = lines.iter().map(String::as_str).collect();
        let s = decode(&lines, Record::default());
        let names: Vec<_> = s.chains.iter().map(|c| (c.name.as_str(), c.requests.len())).collect();
        assert_eq!(names, [("main", 1), ("subagent: check-tests", 1)]);
        let task = s.blocks.iter().position(|b| b.tool.as_deref() == Some("task")).unwrap();
        assert_eq!(s.call(task).and_then(CallRecord::duration_ms), Some(11_000));
        assert!(s.platform_calls.is_empty(), "markers are no steps");
    }

    #[test]
    fn a_call_without_a_tool_object_takes_its_result_from_the_repeat() {
        let lines = [
            chunk("a1", "1800000000", None, &[("c1", "bash", r#"{"command":"ls","mode":"async"}"#)]),
            chunk("a1", "1800000002", Some("started"), &[("c1", "bash", r#"{"command":"ls"}"#)]),
        ];
        let lines: Vec<&str> = lines.iter().map(String::as_str).collect();
        let s = decode(&lines, Record::default());
        let result = s.blocks.iter().find(|b| b.kind == BlockKind::ToolResult).unwrap();
        assert_eq!(result.chars, 7);
        assert!(s.calls[0].done);
    }

    #[test]
    fn background_and_blocked_commands() {
        let lines = [
            chunk("a1", "1800000000", None, &[("c1", "bash", r#"{"command":"npm run dev"}"#), ("c2", "bash", r#"{"command":"kill 1"}"#)]),
            tool("c1", "shell 1", serde_json::json!({}), serde_json::json!({"executionMode": "async"})),
            tool("c2", "blocked", serde_json::json!({}), serde_json::json!({"shell_error_category": "kill_command_blocked"})),
        ];
        let lines: Vec<&str> = lines.iter().map(String::as_str).collect();
        let s = decode(&lines, Record::default());
        assert!(s.calls[0].background && !s.calls[1].background);
        assert!(s.call_ends.values().any(|e| e.outcome == Outcome::Denied));
    }

    #[test]
    fn lines_that_are_not_data_are_skipped() {
        for line in ["", "data: [DONE]", ": comment", "data: 42", "event: x", "data: {\"role\":\"system\"}"] {
            assert!(parse_line(line).is_none(), "{line}");
        }
        assert!(parse_line("data: {\"role\":\"user\",\"content\":\"x\"}\r\n").is_some(), "a line ending kept by Windows");
    }
}
