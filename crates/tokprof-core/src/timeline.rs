//! The request timeline of a session's main conversation: one record per request with its cost
//! in the X-ray's five components, its miss, what entered its context since the previous
//! request, what changed since then, failed tool results and commits; and the session's marks
//! (typed messages, interrupts, compactions) and its subagents' spans. `tokprof scan --requests`
//! and the window's `Requests` tab read the same records.

use std::collections::HashMap;

use crate::analysis::cache::Cause;
use crate::analysis::{Component, Entered, MissEvent, RequestRecord, request_parts};
use crate::fmt::{human_duration, tokens, usd};
use crate::model::{BlockKind, ChainKind, CompactionTrigger, HandoffKind, Outcome, Session, Usage};
use crate::profile::Values;

/// How many entered groups a request keeps; the rest are summed.
pub const KEPT_GROUPS: usize = 12;
/// Failed results in a row that make a streak.
pub const STREAK: u32 = 3;

/// A request of the main conversation.
#[derive(Clone, Debug)]
pub struct RequestPoint {
    pub ts_ms: i64,
    /// What the request sent, cached or not.
    pub prompt_tokens: u64,
    pub usd: f64,
    pub model: String,
    pub effort: Option<String>,
    pub advisor: bool,
    pub usage: Usage,
    /// The cost in the X-ray's components, in `Component::ALL` order; they add up to `usd`.
    pub parts: [Values; 5],
    pub priced: bool,
    /// The first request of its chain: nothing to compare it with.
    pub first: bool,
    /// The previous request of its chain: its prompt, and the time since it.
    pub prev_prompt: Option<u64>,
    pub gap_ms: Option<i64>,
    /// The cache's lifetime after the previous request, and its name (`1h`).
    pub ttl: Option<(i64, &'static str)>,
    pub miss: Option<PointMiss>,
    /// What entered the context since the previous request; `None` when the log does not show
    /// the prompt.
    pub entered: Option<EnteredList>,
    pub changes: Vec<Change>,
    /// Tool results among the entered blocks that failed, and whether a streak reached
    /// `STREAK` at this request.
    pub failed: u32,
    pub streak: bool,
    /// Shell calls of the response that committed and worked.
    pub commits: u32,
}

impl RequestPoint {
    /// The time since the previous request was longer than the cache lives.
    pub fn past_ttl(&self) -> bool {
        matches!((self.gap_ms, self.ttl), (Some(g), Some((t, _))) if g > t)
    }

    /// The sum of the parts, in dollars or in tokens.
    pub fn total(&self, dollars: bool) -> f64 {
        self.parts.iter().map(|v| if dollars { v.usd } else { v.tokens }).sum()
    }

    /// The cache write and the uncached input as the log gives them.
    pub fn written(&self) -> u64 {
        self.usage.cache_write()
    }
}

/// The miss a request had.
#[derive(Clone, Debug, PartialEq)]
pub struct PointMiss {
    pub cause: Cause,
    pub detail: String,
    pub rewritten: u64,
    pub premium_usd: Option<f64>,
}

impl PointMiss {
    /// The cause with its detail: `idle > TTL: 1.1h idle, 1h TTL`.
    pub fn text(&self) -> String {
        if self.detail.is_empty() { self.cause.name().to_string() } else { format!("{}: {}", self.cause.name(), self.detail) }
    }
}

/// Blocks of one kind, tool and target that entered the context.
#[derive(Clone, Debug, PartialEq)]
pub struct EnteredGroup {
    pub kind: BlockKind,
    pub tool: Option<String>,
    pub target: Option<String>,
    pub tokens: f64,
    pub blocks: u32,
}

impl EnteredGroup {
    /// `6.1k tool results Bash cargo test`.
    pub fn text(&self) -> String {
        let mut out = format!("{} {}", tokens(self.tokens), self.kind.name());
        for part in [&self.tool, &self.target].into_iter().flatten() {
            out.push(' ');
            out.push_str(part);
        }
        out
    }
}

/// What entered a request's context: the largest groups, and the rest summed.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct EnteredList {
    pub groups: Vec<EnteredGroup>,
    pub more_groups: u32,
    pub more_tokens: f64,
}

impl EnteredList {
    /// The first `n` groups, and the number and tokens of the rest.
    pub fn top(&self, n: usize) -> (&[EnteredGroup], u32, f64) {
        let n = n.min(self.groups.len());
        let rest = &self.groups[n..];
        (&self.groups[..n], self.more_groups + rest.len() as u32, self.more_tokens + rest.iter().map(|g| g.tokens).sum::<f64>())
    }

    pub fn is_empty(&self) -> bool {
        self.groups.is_empty() && self.more_groups == 0
    }

    /// One line of the largest `n` groups: `6.1k tool results Bash cargo test · 2 more 300`.
    pub fn line(&self, n: usize) -> String {
        let (top, more, more_tokens) = self.top(n);
        let mut parts: Vec<String> = top.iter().map(|g| crate::fmt::clip_end(&g.text(), 72)).collect();
        if more > 0 {
            parts.push(format!("{more} more {}", tokens(more_tokens)));
        }
        parts.join(" · ")
    }
}

/// Something that changed since the previous request of the chain.
#[derive(Clone, Debug, PartialEq)]
pub enum Change {
    Model(String, String),
    Effort(String, String),
    /// Another setting the log names.
    Setting(&'static str),
    Version(String, String),
    Tools {
        added: Vec<String>,
        removed: Vec<String>,
    },
    /// The system prompt's size changed by this many characters.
    System(i64),
    /// A compaction came between, with its kind when known.
    Compacted(Option<String>),
}

impl Change {
    /// `effort high → max`, `tools +2 −0`.
    pub fn text(&self) -> String {
        match self {
            Change::Model(a, b) => format!("model {a} → {b}"),
            Change::Effort(a, b) => format!("effort {a} → {b}"),
            Change::Setting(name) => format!("{name} changed"),
            Change::Version(a, b) => format!("agent {a} → {b}"),
            Change::Tools { added, removed } => format!("tools +{} −{}", added.len(), removed.len()),
            Change::System(d) => format!("system prompt {d:+} chars"),
            Change::Compacted(Some(k)) => format!("compacted ({k})"),
            Change::Compacted(None) => "compacted".to_string(),
        }
    }
}

/// A moment of the main log the timeline marks.
#[derive(Clone, Debug, PartialEq)]
pub struct Mark {
    pub ts_ms: i64,
    pub kind: MarkKind,
}

#[derive(Clone, Debug, PartialEq)]
pub enum MarkKind {
    Typed,
    Interrupt,
    /// A compaction, with its kind when the log names it.
    Compaction(Option<String>),
}

/// When a subagent ran: its first and last request, and its task.
#[derive(Clone, Debug, PartialEq)]
pub struct SubagentSpan {
    pub first_ms: i64,
    pub last_ms: i64,
    pub task: String,
}

/// The marks of the main log: typed messages, interrupts and compactions, in time order.
pub fn marks_of(s: &Session) -> Vec<Mark> {
    let mut out: Vec<Mark> = s.typed.iter().map(|m| Mark { ts_ms: m.ts_ms, kind: MarkKind::Typed }).collect();
    out.extend(s.handoffs.iter().filter(|h| h.kind == HandoffKind::Interrupt).map(|h| Mark { ts_ms: h.ts_ms, kind: MarkKind::Interrupt }));
    out.extend(s.compactions.iter().map(|c| Mark { ts_ms: c.ts_ms, kind: MarkKind::Compaction(c.kind.name().map(str::to_string)) }));
    out.retain(|m| m.ts_ms > 0);
    out.sort_by_key(|m| m.ts_ms);
    out
}

/// The spans of the subagents: each subagent chain of every log, from its first request with a
/// time to its last; a subagent's own log carries its task.
pub fn subagent_spans(sessions: &[Session]) -> Vec<SubagentSpan> {
    let mut out = Vec::new();
    for (i, s) in sessions.iter().enumerate() {
        for c in s.chains.iter().filter(|c| c.kind == ChainKind::Subagent) {
            let ts = c.requests.iter().map(|r| r.ts_ms).filter(|&t| t > 0);
            if let (Some(first), Some(last)) = (ts.clone().min(), ts.max()) {
                let task = if i == 0 { c.name.clone() } else { s.task_label() };
                out.push(SubagentSpan { first_ms: first, last_ms: last, task });
            }
        }
    }
    out.sort_by_key(|s| s.first_ms);
    out
}

/// Builds the records of the main requests from the cost pass, in the X-ray's sink.
#[derive(Default)]
pub struct Collector {
    points: Vec<RequestPoint>,
    /// The record of the request the pass is on, while it is a main one.
    open: Option<usize>,
    /// The chain of the open record, to match its miss.
    chain: String,
    /// Failed tool results in a row so far.
    run: u32,
}

impl Collector {
    /// A request of the pass; `main` when it belongs to the main conversation.
    pub fn request(&mut self, r: &RequestRecord, main: bool) {
        self.open = None;
        if !main {
            return;
        }
        let usd = r.price.map_or(0.0, |p| p.cost(&r.usage));
        self.points.push(RequestPoint {
            ts_ms: r.ts_ms,
            prompt_tokens: r.usage.prompt(),
            usd,
            model: r.model.to_string(),
            effort: r.effort.map(str::to_string),
            advisor: r.advisor,
            usage: r.usage,
            parts: request_parts(&r.usage, r.price, 0),
            priced: r.price.is_some(),
            first: r.prev_prompt.is_none(),
            prev_prompt: r.prev_prompt,
            gap_ms: r.gap_s.map(|g| (g * 1000.0).round() as i64),
            ttl: None,
            miss: None,
            entered: None,
            changes: Vec::new(),
            failed: 0,
            streak: false,
            commits: 0,
        });
        self.open = Some(self.points.len() - 1);
        self.chain = r.chain.to_string();
    }

    pub fn miss(&mut self, m: &MissEvent) {
        let Some(i) = self.open else { return };
        let p = &mut self.points[i];
        if m.ts_ms == p.ts_ms && m.chain == self.chain {
            p.miss = Some(PointMiss { cause: m.cause, detail: m.detail.clone(), rewritten: m.rewritten, premium_usd: m.premium_usd });
        }
    }

    pub fn entered(&mut self, e: &Entered) {
        let Some(i) = self.open.take() else { return };
        let s = e.session;
        let req = e.request;
        let p = &mut self.points[i];
        p.parts = request_parts(&req.usage, e.price, e.rewritten);
        p.entered = e.blocks.map(|blocks| group(s, blocks));
        if let Some(prev) = e.prev {
            p.ttl = req.api.ttl(prev.long_ttl);
            p.changes = changes(s, prev, req, e.shape);
        }
        for &(b, _) in e.blocks.unwrap_or_default() {
            if s.blocks[b].kind != BlockKind::ToolResult {
                continue;
            }
            match s.outcome(b) {
                Outcome::Worked => self.run = 0,
                Outcome::Failed => {
                    p.failed += 1;
                    self.run += 1;
                    if self.run == STREAK {
                        p.streak = true;
                    }
                }
                _ => {}
            }
        }
        if let Some(m) = req.produced {
            p.commits = s.messages[m]
                .blocks
                .clone()
                .filter(|&b| s.blocks[b].kind == BlockKind::ToolUse && s.vcs.get(&b).is_some_and(|v| v.commit) && s.outcome(b) == Outcome::Worked)
                .count() as u32;
        }
    }

    /// The records with a time, in time order.
    pub fn points(self) -> Vec<RequestPoint> {
        let mut points: Vec<RequestPoint> = self.points.into_iter().filter(|p| p.ts_ms > 0).collect();
        points.sort_by_key(|p| p.ts_ms);
        points
    }
}

/// The entered blocks grouped by kind, tool and target, largest first, `KEPT_GROUPS` kept.
fn group(s: &Session, blocks: &[(usize, f64)]) -> EnteredList {
    // By kind, tool and target: tokens and blocks.
    type Key<'a> = (BlockKind, Option<&'a str>, Option<&'a str>);
    let mut by: HashMap<Key, (f64, u32)> = HashMap::new();
    // Blocks without text, such as thinking the log keeps only signed, add nothing.
    for &(b, t) in blocks.iter().filter(|(_, t)| *t >= 0.5) {
        let blk = &s.blocks[b];
        let e = by.entry((blk.kind, blk.tool.as_deref(), blk.target.as_deref())).or_default();
        e.0 += t;
        e.1 += 1;
    }
    let mut groups: Vec<EnteredGroup> = by
        .into_iter()
        .map(|((kind, tool, target), (tokens, blocks))| EnteredGroup {
            kind,
            tool: tool.map(str::to_string),
            target: target.map(str::to_string),
            tokens,
            blocks,
        })
        .collect();
    groups.sort_by(|a, b| {
        b.tokens.total_cmp(&a.tokens).then_with(|| a.kind.cmp(&b.kind)).then_with(|| a.tool.cmp(&b.tool)).then_with(|| a.target.cmp(&b.target))
    });
    let rest = groups.split_off(groups.len().min(KEPT_GROUPS));
    EnteredList { groups, more_groups: rest.len() as u32, more_tokens: rest.iter().map(|g| g.tokens).sum() }
}

/// What changed from the previous request of the chain to this one.
fn changes(s: &Session, prev: &crate::analysis::cache::Prev, req: &crate::model::Request, shape: Option<usize>) -> Vec<Change> {
    let mut out = Vec::new();
    if prev.model != req.model {
        out.push(Change::Model(prev.model.clone(), req.model.clone()));
    }
    if let (Some(a), Some(b)) = (&prev.params, &req.params) {
        match (&a.effort, &b.effort) {
            (Some(x), Some(y)) if x != y => out.push(Change::Effort(x.clone(), y.clone())),
            _ => {}
        }
        let mut a = a.clone();
        a.effort = b.effort.clone();
        if let Some(name) = b.changed_from(&a) {
            out.push(Change::Setting(name));
        }
    }
    if let (Some(a), Some(b)) = (&prev.version, &req.version)
        && a != b
    {
        out.push(Change::Version(a.clone(), b.clone()));
    }
    if let (Some(a), Some(b)) = (prev.shape, shape)
        && a != b
    {
        let (a, b) = (&s.shapes[a], &s.shapes[b]);
        if a.tool_names != b.tool_names {
            let added = b.tool_names.iter().filter(|t| !a.tool_names.contains(t)).cloned().collect();
            let removed = a.tool_names.iter().filter(|t| !b.tool_names.contains(t)).cloned().collect();
            out.push(Change::Tools { added, removed });
        }
        if a.system_chars != b.system_chars {
            out.push(Change::System(b.system_chars as i64 - a.system_chars as i64));
        }
    }
    if req.compacted_before {
        let kind = s.compactions.iter().rev().find(|c| c.ts_ms > prev.ts_ms && c.ts_ms <= req.ts_ms).map(|c| &c.kind);
        out.push(Change::Compacted(kind.and_then(CompactionTrigger::name).map(str::to_string)));
    }
    out
}

/// The short why of a request, as the table and `tokprof scan --requests` give it.
pub fn why(p: &RequestPoint, long: bool) -> String {
    if let Some(m) = &p.miss {
        let head = if m.cause.expected() { "rebuilt" } else { "missed" };
        let mut out = if long { format!("{head}: {}", m.text()) } else { format!("{head}: {}", m.cause.name()) };
        if long
            && !m.cause.expected()
            && let Some(u) = m.premium_usd
        {
            out += &format!(" · {} avoidable", usd(u));
        }
        return out;
    }
    if p.first {
        return "first request".to_string();
    }
    match p.prev_prompt {
        Some(prev) if prev != p.prompt_tokens => {
            let d = p.prompt_tokens as f64 - prev as f64;
            format!("context {}{}", if d > 0.0 { "+" } else { "−" }, tokens(d.abs()))
        }
        _ => String::new(),
    }
}

/// A span of time in plain words: minutes up to two hours, then hours or days.
fn span(ms: i64) -> String {
    let m = ms as f64 / 60_000.0;
    if m < 120.0 { format!("{:.0} min", m.max(1.0)) } else { human_duration(ms as f64 / 1000.0) }
}

/// The cache's lifetime in plain words: `60 min`, `5 min`.
fn life(ttl_ms: i64) -> String {
    format!("{:.0} min", ttl_ms as f64 / 60_000.0)
}

/// Why a miss made the request write again, in plain words.
fn because(p: &RequestPoint, m: &PointMiss) -> String {
    let with = |s: &str| if m.detail.is_empty() { s.to_string() } else { format!("{s}: {}", m.detail) };
    match m.cause {
        Cause::IdleTtl => match (p.gap_ms, p.ttl) {
            (Some(g), Some((t, _))) => format!("the cache had expired: {} since the previous request, the cache lives {}", span(g), life(t)),
            _ => with("the cache had expired"),
        },
        Cause::ModelSwitch => with("the model changed"),
        Cause::Compaction => {
            if m.detail.is_empty() {
                "the conversation was compacted".to_string()
            } else {
                format!("the conversation was compacted ({})", m.detail)
            }
        }
        Cause::CompactionCall => "it was the call that writes the compaction summary".to_string(),
        Cause::ToolResultsCleared => with("the agent cleared old tool results"),
        Cause::ParamsChanged => with("a setting changed"),
        Cause::AgentUpdated => with("the agent was updated"),
        Cause::ToolsChanged => with("the tool set changed"),
        Cause::SystemChanged => with("the system prompt changed"),
        Cause::ImagesRemoved => with("images were removed from the history"),
        Cause::ThinkingDropped => with("thinking was dropped from the history"),
        Cause::HistoryEdited => with("the history was edited"),
        Cause::Lookback => with("too many blocks were appended at once"),
        _ => "the cache did not hold it, and the log shows no cause (server side?)".to_string(),
    }
}

/// An amount of a part: dollars when priced, tokens with `tok` when not.
fn money(p: &RequestPoint, v: &Values) -> String {
    if p.priced { usd(v.usd) } else { format!("{} tok", tokens(v.tokens)) }
}

/// The first line of the selected request's card: why it cost what it did, in plain words with
/// dollars. A miss comes first: `This request wrote 310.0k again ($1.94) because the cache had
/// expired: 72 min since the previous request, the cache lives 60 min.`
pub fn sentence(p: &RequestPoint) -> String {
    let part = |c: Component| &p.parts[Component::ALL.iter().position(|x| *x == c).unwrap_or(0)];
    let (read, write, rewrite, uncached, output) =
        (part(Component::Read), part(Component::Write), part(Component::Rewrite), part(Component::Uncached), part(Component::Output));
    let mut out = String::new();
    if let Some(m) = &p.miss {
        out += &format!("This request wrote {} again ({}) because {}", tokens(rewrite.tokens), money(p, rewrite), because(p, m));
        match m.premium_usd {
            Some(a) if !m.cause.expected() && a >= 0.005 => out += &format!("; {} of it could have been read from the cache.", usd(a)),
            _ => out.push('.'),
        }
        let mut also = Vec::new();
        if read.tokens >= 1.0 {
            also.push(format!("read {} from the cache ({})", tokens(read.tokens), money(p, read)));
        }
        if write.tokens + uncached.tokens >= 1.0 {
            also.push(format!("wrote {} new ({})", tokens(write.tokens + uncached.tokens), money(p, &sum(write, uncached))));
        }
        also.push(format!("output {} ({})", tokens(output.tokens), money(p, output)));
        let last = also.pop().unwrap_or_default();
        out += &if also.is_empty() {
            format!(" Its {last}.").replacen("Its output", "Its output was", 1)
        } else {
            format!(" It also {} and {last}.", also.join(", "))
        };
        return out;
    }
    let new = sum(write, uncached);
    if p.first {
        out += &format!("The first request of its chain: it wrote its {} prompt ({})", tokens(new.tokens), money(p, &new));
        if read.tokens >= 1.0 {
            out += &format!(" and read {} from the cache ({})", tokens(read.tokens), money(p, read));
        }
    } else {
        out += &format!("This request read {} from the cache ({}) and wrote {} ({})", tokens(read.tokens), money(p, read), tokens(new.tokens), money(p, &new));
        if let Some(prev) = p.prev_prompt {
            let d = p.prompt_tokens as f64 - prev as f64;
            if d > 0.0 {
                out += &format!(" as the context grew by {}", tokens(d));
            } else if d < 0.0 {
                out += &format!(" while the context shrank by {}", tokens(-d));
            }
        }
    }
    out += &format!("; its output was {} ({}).", tokens(output.tokens), money(p, output));
    out
}

fn sum(a: &Values, b: &Values) -> Values {
    Values { tokens: a.tokens + b.tokens, usd: a.usd + b.usd, avoidable: a.avoidable + b.avoidable }
}

/// The time since the previous request against the cache's lifetime: `1.0h after the previous
/// request, past the 1h cache`.
pub fn gap_text(p: &RequestPoint) -> Option<String> {
    let g = p.gap_ms?;
    let after = format!("{} after the previous request", human_duration(g as f64 / 1000.0));
    Some(match p.ttl {
        Some((t, name)) if g > t => format!("{after}, past the {name} cache"),
        Some((_, name)) => format!("{after}, within the {name} cache"),
        None => after,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::{self, Filter, Sink};
    use crate::model::{Block, Class, Compaction, Params, PromptDelta, Request, Shape};
    use crate::pricing::Pricing;

    fn price() -> crate::pricing::Price {
        crate::pricing::Price { input: 5.0, output: 25.0, read: 0.5, write_short: 6.25, write_long: 10.0 }
    }

    #[test]
    fn the_parts_add_up_to_the_price_of_the_usage() {
        let u = Usage { uncached: 3, cache_read: 12_300, write_short: 0, write_long: 490_300, output: 1_200, reasoning: 0 };
        let parts = request_parts(&u, Some(price()), 495_000);
        let total: f64 = parts.iter().map(|v| v.usd).sum();
        assert!((total - price().cost(&u)).abs() < 1e-9);
        // The re-processed prefix comes out of the write first, then the uncached input.
        assert_eq!(parts[2].tokens, 490_303.0);
        assert_eq!((parts[1].tokens, parts[3].tokens), (0.0, 0.0));
        assert!((parts[2].avoidable - (490_300.0 * 9.5 + 3.0 * 4.5) / 1e6).abs() < 1e-9);
        let tokens: f64 = parts.iter().map(|v| v.tokens).sum();
        assert_eq!(tokens, (u.prompt() + u.output) as f64);
    }

    fn block(kind: BlockKind, chars: u64, tool: Option<&str>, target: Option<&str>) -> Block {
        Block { kind, class: Class::None, chars, tool: tool.map(str::to_string), target: target.map(str::to_string) }
    }

    /// A session of three requests: the second adds a tool result, a call and text; the third
    /// comes after two hours and a changed effort, and misses.
    fn session() -> Session {
        let mut s = Session::new(crate::testing::claude(), "s".into());
        let sys = s.push_message(vec![block(BlockKind::Tools, 4_000, None, None), block(BlockKind::Prompt, 1_000, None, None)], true);
        s.shapes.push(Shape { blocks: 0..0, fingerprint: 1, tool_names: vec!["Read".into()], system_chars: 100 });
        s.shapes.push(Shape { blocks: 0..0, fingerprint: 2, tool_names: vec!["Read".into(), "Edit".into(), "Grep".into()], system_chars: 100 });
        let answer = s.push_message(vec![block(BlockKind::Text, 400, None, None), block(BlockKind::ToolUse, 200, Some("Bash"), Some("cargo test"))], false);
        let result = s.push_message(vec![block(BlockKind::ToolResult, 6_100, Some("Bash"), Some("cargo test"))], false);
        let params = |e: &str| Some(Params { effort: Some(e.into()), settings: Vec::new() });
        let r = |ts: i64, usage: Usage, prompt: PromptDelta, produced: Option<usize>, p: Option<Params>| Request {
            ts_ms: ts,
            api: crate::api::ApiId::Anthropic,
            model: "claude-opus-5-5".into(),
            usage,
            prompt: Some(prompt),
            produced,
            params: p,
            ..Request::default()
        };
        let chain = crate::model::Chain {
            name: "main".into(),
            kind: ChainKind::Main,
            requests: vec![
                r(
                    1_000,
                    Usage { write_long: 5_000, output: 600, ..Usage::default() },
                    PromptDelta { shape: Some(0), keep: 0, tail: vec![sys] },
                    Some(answer),
                    params("high"),
                ),
                r(
                    60_000,
                    Usage { cache_read: 5_000, write_long: 6_700, output: 100, ..Usage::default() },
                    PromptDelta { shape: None, keep: 1, tail: vec![answer, result] },
                    None,
                    params("high"),
                ),
                r(
                    60_000 + 7_200_000,
                    Usage { write_long: 11_700, output: 100, ..Usage::default() },
                    PromptDelta { shape: Some(1), keep: 3, tail: vec![] },
                    None,
                    params("max"),
                ),
            ],
        };
        s.chains.push(chain);
        s
    }

    #[derive(Default)]
    struct Probe {
        c: Collector,
        components: [f64; 5],
    }

    impl Sink for Probe {
        fn session(&mut self, _: &Session) {}
        fn request(&mut self, r: &RequestRecord) {
            self.c.request(r, true);
        }
        fn miss(&mut self, m: MissEvent) {
            self.c.miss(&m);
        }
        fn fact(&mut self, f: &analysis::Fact) {
            self.components[Component::ALL.iter().position(|c| *c == f.component).unwrap()] += f.usd.unwrap_or(0.0);
        }
        fn entered(&mut self, e: &Entered) {
            self.c.entered(e);
        }
    }

    fn points() -> Vec<RequestPoint> {
        let s = session();
        let mut probe = Probe::default();
        analysis::analyze(&s, &Pricing::defaults(), &Filter::default(), &mut probe);
        probe.c.points()
    }

    #[test]
    fn a_request_lists_what_entered_since_the_previous_one_largest_first() {
        let p = points();
        let e = p[1].entered.as_ref().expect("the log shows the prompt");
        let texts: Vec<String> = e.groups.iter().map(EnteredGroup::text).collect();
        assert_eq!(texts, ["6.1k tool results Bash cargo test", "400 assistant text", "200 tool calls Bash cargo test"]);
        // The first request holds its whole prompt.
        assert_eq!(p[0].entered.as_ref().unwrap().line(3), "4.0k tool schemas · 1.0k user prompt");
        assert!(p[0].first && !p[1].first);
        assert_eq!(why(&p[1], true), "context +6.7k");
        assert_eq!(why(&p[0], true), "first request");
    }

    #[test]
    fn a_request_after_a_long_break_names_what_changed_and_why_it_missed() {
        let p = points();
        let third = &p[2];
        assert_eq!(third.changes.iter().map(Change::text).collect::<Vec<_>>(), ["effort high → max", "tools +2 −0"]);
        assert!(third.past_ttl());
        assert_eq!(gap_text(third).unwrap(), "2.0h after the previous request, past the 1h cache");
        // An idle miss comes before the effort change.
        let m = third.miss.as_ref().expect("a miss");
        assert_eq!(m.cause, Cause::IdleTtl);
        assert!(why(third, true).starts_with("missed: idle > TTL: 2.0h idle, 1h TTL · $"), "{}", why(third, true));
        assert_eq!(why(third, false), "missed: idle > TTL");
        let s = sentence(third);
        assert_eq!(
            s,
            "This request wrote 11.7k again ($0.09) because the cache had expired: 2.0h since the previous request, the cache lives 60 min; $0.09 of it could have been read from the cache. Its output was 100 ($0.00)."
        );
        // The parts add up to the cost.
        assert!((third.total(true) - third.usd).abs() < 1e-9);
    }

    #[test]
    fn the_parts_of_the_requests_add_up_to_the_spend_by_component() {
        let s = session();
        let mut probe = Probe::default();
        analysis::analyze(&s, &Pricing::defaults(), &Filter::default(), &mut probe);
        let components = probe.components;
        let points = probe.c.points();
        for (i, c) in components.iter().enumerate() {
            let parts: f64 = points.iter().map(|p| p.parts[i].usd).sum();
            assert!((parts - c).abs() < 1e-9, "{:?}: {parts} against {c}", Component::ALL[i]);
        }
        assert!(components[2] > 0.0, "the miss re-wrote its prefix");
    }

    #[test]
    fn the_sentence_of_a_growing_context_says_how_much_it_grew() {
        let p = points();
        assert_eq!(
            sentence(&p[1]),
            "This request read 5.0k from the cache ($0.00) and wrote 6.7k ($0.05) as the context grew by 6.7k; its output was 100 ($0.00)."
        );
        assert!(sentence(&p[0]).starts_with("The first request of its chain: it wrote its 5.0k prompt"));
    }

    #[test]
    fn three_failed_results_in_a_row_make_a_streak_and_a_refused_one_does_not_break_it() {
        let mut s = Session::new(crate::testing::claude(), "s".into());
        let results: Vec<usize> =
            (0..5).map(|i| s.push_message(vec![block(BlockKind::ToolResult, 1_000, Some("Read"), Some(&format!("f{i}")))], false)).collect();
        let first = s.messages[results[0]].blocks.start;
        for (i, outcome) in [Outcome::Failed, Outcome::Refused, Outcome::Failed, Outcome::Failed, Outcome::Worked].into_iter().enumerate() {
            if outcome != Outcome::Worked {
                s.call_ends.insert(first + i, crate::model::CallEnd { outcome, error: None });
            }
        }
        let requests = results
            .iter()
            .enumerate()
            .map(|(i, &m)| Request {
                ts_ms: 1_000 * (i as i64 + 1),
                model: "claude-opus-5-5".into(),
                usage: Usage { write_long: 1_000 * (i as u64 + 1), output: 10, ..Usage::default() },
                prompt: Some(PromptDelta { shape: None, keep: i, tail: vec![m] }),
                ..Request::default()
            })
            .collect();
        s.chains.push(crate::model::Chain { name: "main".into(), kind: ChainKind::Main, requests });
        let mut probe = Probe::default();
        analysis::analyze(&s, &Pricing::defaults(), &Filter::default(), &mut probe);
        let p = probe.c.points();
        assert_eq!(p.iter().map(|p| (p.failed, p.streak)).collect::<Vec<_>>(), [(1, false), (0, false), (1, false), (1, true), (0, false)]);
    }

    #[test]
    fn a_worked_commit_and_a_compaction_mark_their_requests() {
        let mut s = session();
        // The first response's call commits and works; a compaction comes before the third.
        let call = s.messages[1].blocks.start + 1;
        s.vcs.insert(call, crate::trigger::VcsActions { commit: true, push: false, pull_request: false });
        s.compactions.push(Compaction { ts_ms: 61_000, kind: CompactionTrigger::Auto });
        s.chains[0].requests[2].compacted_before = true;
        let mut probe = Probe::default();
        analysis::analyze(&s, &Pricing::defaults(), &Filter::default(), &mut probe);
        let p = probe.c.points();
        assert_eq!(p.iter().map(|p| p.commits).collect::<Vec<_>>(), [1, 0, 0]);
        assert!(p[2].changes.contains(&Change::Compacted(Some("auto".into()))));
        assert_eq!(why(&p[2], true), "rebuilt: compaction: auto");
        // A failed commit is no commit.
        s.call_ends.insert(call, crate::model::CallEnd { outcome: Outcome::Failed, error: None });
        let mut probe = Probe::default();
        analysis::analyze(&s, &Pricing::defaults(), &Filter::default(), &mut probe);
        assert_eq!(probe.c.points()[0].commits, 0);
    }

    #[test]
    fn marks_hold_typed_messages_interrupts_and_compactions_in_time_order() {
        let mut s = Session::new(crate::testing::claude(), "s".into());
        s.typed.push(crate::model::TypedMessage { ts_ms: 3_000, label: "go".into() });
        s.compactions.push(Compaction { ts_ms: 2_000, kind: CompactionTrigger::Auto });
        s.handoffs.push(crate::model::Handoff { ts_ms: 1_000, kind: HandoffKind::Interrupt });
        let kinds: Vec<MarkKind> = marks_of(&s).into_iter().map(|m| m.kind).collect();
        assert_eq!(kinds, [MarkKind::Interrupt, MarkKind::Compaction(Some("auto".into())), MarkKind::Typed]);
    }
}
