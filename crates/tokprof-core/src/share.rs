//! `tokprof share`: one JSON file of aggregates over the selected requests that holds no names,
//! so that several users can compare and pool their numbers without showing their work.
//!
//! The file is built from the summary's collector, the pause report's and [`Share`], with only
//! allowed keys: tokprof's own names for parts, components, causes, triggers, activities, setup
//! sources, content kinds, ranges, hints, compaction triggers and effort levels; the names that
//! ship with the agent, as its source lists them; programs from a fixed list; and model, client and
//! version names. Every other origin, tool and program counts under its kind. Nothing else of the
//! collectors is copied, so a new field in one cannot leak into the file without a change here.
//!
//! [`words`] then walks the finished file by position and returns every key or string that is not
//! one of tokprof's own words, so that [`leak`] can search them for this machine's names before the
//! file is written, and the person can review them after.

use std::collections::{BTreeMap, BTreeSet, HashSet};

use serde_json::{Map, Value, json};

use crate::analysis::cache::Cause;
use crate::analysis::{Component, Fact, Filter, MissEvent, RequestRecord, Sink};
use crate::model::{BlockKind, ChainKind, Class, HandoffKind, Session, ToolClass, WakeSource};
use crate::pauses::{AgentPauses, PAUSE_RANGES, Pauses, RETURN_RANGES, SIZE_RANGES, Starter};
use crate::profile::Values;
use crate::report::{Collector, Spend, WhatIf, Work};
use crate::source::{AgentId, PublicNames};
use crate::tools::{BlockEntry, HINTS, ToolRow, ToolShare, ToolTable, hint};
use crate::trigger::{CallClass, Trigger, known_programs, program_of};

/// The format number of the file; a change that renames or removes a field raises it.
pub const FORMAT: u64 = 2;

/// The setup sources the file names, as setup-breakdown names them; any other is `other`.
const SETUP_SOURCES: [&str; 7] = ["tool schemas", "system prompt", "instructions", "hook context", "deferred tool lists", "skills", "other"];

/// The effort levels the file names. A request that logs another level counts as `other`, one
/// that logs none as `unknown`.
pub const EFFORT_LEVELS: [&str; 6] = ["minimal", "low", "medium", "high", "xhigh", "max"];

const HOUR: i64 = 3_600_000;

/// The ranges of a request's prompt, by lower bound in tokens.
pub const PROMPT_RANGES: [(&str, u64); 6] =
    [("under 50k", 0), ("50–100k", 50_000), ("100–200k", 100_000), ("200–400k", 200_000), ("400–800k", 400_000), ("800k+", 800_000)];

/// The ranges of the time a main session ran, from its first to its last selected request, by
/// lower bound in ms.
pub const SPAN_RANGES: [(&str, i64); 5] = [("under 1h", 0), ("1–4h", HOUR), ("4–24h", 4 * HOUR), ("1–3d", 24 * HOUR), ("over 3d", 72 * HOUR)];

/// The compaction triggers the file names; a trigger a log names otherwise is `other`.
pub const COMPACTION_TRIGGERS: [&str; 5] = ["auto", "manual", "idle", "unknown", "other"];

/// The names of who started a cold return, in `Starter::ALL` order.
const STARTERS: [&str; 4] = ["typed messages", "background tasks", "tool results", "other"];

/// The hand-offs of a main conversation the file counts, as session-logs names them.
pub const HANDOFF_KINDS: [&str; 5] = ["turn_ends", "interrupts", "typed_while_working", "questions", "answers"];

/// What a wake reported, as session-logs names it.
pub const WAKE_SOURCES: [&str; 3] = ["subagent", "command", "other"];

const HOUR_MS: i64 = 3_600_000;

/// The kinds of origin, as the origin of a request is written before its `:`.
const ORIGIN_KINDS: [&str; 7] = ["prompt", "skill", "agent", "command", "notification", "mcp", "plugin"];

/// The keys of the file's parts.
const FIELDS: &[&str] = &[
    "format",
    "tokprof",
    "platform",
    "window",
    "first",
    "last",
    "survey",
    "mcp_names",
    "agents",
    "sessions",
    "main_sessions",
    "spend",
    "clients",
    "cache_writes",
    "misses",
    "origins",
    "triggers",
    "activities",
    "setup",
    "effort",
    "pauses",
    "content",
    "prompt_sizes",
    "session_spans",
    "tools",
    "tool_hints",
    "compactions",
    "versions",
    "whatif",
    "requests",
    "priced",
    "components",
    "total",
    "usd",
    "tokens",
    "avoidable_usd",
    "cold_returns",
    "main",
    "subagents",
    "short",
    "long",
    "names",
    "subagents_usd",
    "output_tokens",
    "typed_messages",
    "typed_by_pause",
    "messages",
    "median_context",
    "typed_after_lifetime",
    "cold_returns_by_pause",
    "returns",
    "cold_returns_started_by",
    "after_cold_return",
    "median",
    "quarter_1",
    "quarter_3",
    "ended_within_3",
    "sessions_by_largest_context",
    "calls",
    "single",
    "results",
    "result_tokens",
    "inputs",
    "input_tokens",
    "rent",
    "trips",
    "programs",
    "rows",
    "actual_usd",
    "activity",
    "active_days",
    "active_hours",
    "busy_ms",
    "failures",
    "failed",
    "refused",
    "denied",
    "after",
    "time",
    "kinds",
    "ms",
    "after_turns_ms",
    "slow",
    "slow_main",
    "longest_ms",
    "retried",
    "handoffs",
    "wakes",
    "prompts",
    "held",
    "all",
];

/// What the file adds to the reports for one agent.
#[derive(Clone, Debug, Default)]
pub struct AgentShare {
    /// Tokens written with the short and the long lifetime: main conversations first, then
    /// subagents and side requests.
    pub cache_writes: [[u64; 2]; 2],
    /// Requests, output tokens and dollars per effort level (see [`EFFORT_LEVELS`]): main
    /// conversations first, then the rest.
    pub effort: [BTreeMap<String, (u64, u64, f64)>; 2],
    /// Spend by origin, trigger and activity, of this agent only.
    pub work: Work,
    /// Logs with a selected request, subagent logs included.
    pub logs: u64,
    /// The main sessions among them, by short id: subagent logs carry their main session's.
    pub main_sessions: HashSet<String>,
    /// Spend by kind of content, in `BlockKind::ALL` order.
    pub content: [Values; 10],
    /// Requests and spend by prompt range (see [`PROMPT_RANGES`]): main conversations first,
    /// then the rest.
    pub prompt_sizes: [[Spend; 6]; 2],
    /// Requests and spend by agent version: `unknown` without one, `other` for a value that is
    /// not a version.
    pub versions: BTreeMap<String, Spend>,
    /// Tokens written with the short and the long lifetime, by client.
    pub client_writes: BTreeMap<String, [u64; 2]>,
    /// Compactions by trigger (see [`COMPACTION_TRIGGERS`]).
    pub compactions: BTreeMap<&'static str, u64>,
    /// The clock hours with a selected main-conversation request, as whole hours of UTC since the
    /// epoch: of the agent, and of each client.
    pub hours: HashSet<i64>,
    pub client_hours: BTreeMap<String, HashSet<i64>>,
    /// The hand-offs of the main conversations inside the window, by kind (see
    /// [`HANDOFF_KINDS`]), and the wakes by source (see [`WAKE_SOURCES`]).
    pub handoffs: BTreeMap<&'static str, u64>,
    pub wakes: BTreeMap<&'static str, u64>,
    /// The selected requests whose prompts the log holds, then all of them.
    pub prompts: [Spend; 2],
    /// Per version, the requests whose prompts the log holds.
    pub versions_held: BTreeMap<String, u64>,
    /// Per client, requests and dollars by effort level.
    pub client_effort: BTreeMap<String, BTreeMap<String, (u64, f64)>>,
    /// The replay of this agent's requests under two cache lifetimes, and those lifetimes in ms.
    pub whatif: WhatIf,
    pub whatif_lifetimes: Option<(i64, i64)>,
}

impl AgentShare {
    fn merge(&mut self, o: AgentShare) {
        for (k, w) in o.cache_writes.iter().enumerate() {
            self.cache_writes[k][0] += w[0];
            self.cache_writes[k][1] += w[1];
        }
        for (k, levels) in o.effort.into_iter().enumerate() {
            for (level, (requests, output, usd)) in levels {
                let e = self.effort[k].entry(level).or_default();
                e.0 += requests;
                e.1 += output;
                e.2 += usd;
            }
        }
        self.work.merge(o.work);
        self.logs += o.logs;
        self.main_sessions.extend(o.main_sessions);
        for (mine, theirs) in self.content.iter_mut().zip(o.content) {
            mine.add(theirs);
        }
        for (mine, theirs) in self.prompt_sizes.iter_mut().zip(o.prompt_sizes) {
            for (a, b) in mine.iter_mut().zip(theirs) {
                a.add(b);
            }
        }
        for (v, s) in o.versions {
            self.versions.entry(v).or_default().add(s);
        }
        for (client, w) in o.client_writes {
            let e = self.client_writes.entry(client).or_default();
            e[0] += w[0];
            e[1] += w[1];
        }
        for (t, n) in o.compactions {
            *self.compactions.entry(t).or_default() += n;
        }
        self.hours.extend(o.hours);
        for (client, hours) in o.client_hours {
            self.client_hours.entry(client).or_default().extend(hours);
        }
        for (k, n) in o.handoffs {
            *self.handoffs.entry(k).or_default() += n;
        }
        for (k, n) in o.wakes {
            *self.wakes.entry(k).or_default() += n;
        }
        for (mine, theirs) in self.prompts.iter_mut().zip(o.prompts) {
            mine.add(theirs);
        }
        for (v, n) in o.versions_held {
            *self.versions_held.entry(v).or_default() += n;
        }
        for (client, levels) in o.client_effort {
            let mine = self.client_effort.entry(client).or_default();
            for (level, (requests, usd)) in levels {
                let e = mine.entry(level).or_default();
                e.0 += requests;
                e.1 += usd;
            }
        }
        self.whatif.merge(o.whatif);
        self.whatif_lifetimes = self.whatif_lifetimes.or(o.whatif_lifetimes);
    }
}

/// The log being read: its agent, its short id, whether a request of it counted yet, and what
/// it adds once one does.
#[derive(Clone, Debug)]
struct Current {
    agent: AgentId,
    session: String,
    counted: bool,
    compactions: Vec<&'static str>,
    /// Hand-off kinds and wake sources inside the window, one entry each.
    handoffs: Vec<&'static str>,
    wakes: Vec<&'static str>,
    ids: Vec<String>,
    title: Option<String>,
}

/// Collects what [`to_json`] needs beyond the summary and the pause report.
#[derive(Clone, Debug, Default)]
pub struct Share {
    pub agents: BTreeMap<AgentId, AgentShare>,
    /// The first and the last selected request.
    pub span_ms: Option<(i64, i64)>,
    /// The ids and short ids of the logs with a selected request, for the leak check.
    pub ids: BTreeSet<String>,
    /// Their titles, for the leak check.
    pub titles: BTreeSet<String>,
    /// The tool table of each agent's requests, which gets every call of the sink so that its
    /// failure windows open and close as in `tokprof tools`. An agent without a selected request
    /// has a table but no object in the file.
    pub tables: BTreeMap<AgentId, ToolTable>,
    filter: Filter,
    current: Option<Current>,
}

impl Share {
    /// A collector for the requests this filter keeps, which the analysis is given too.
    pub fn new(filter: Filter) -> Share {
        Share { filter, ..Share::default() }
    }

    pub fn merge(&mut self, o: Share) {
        for (agent, a) in o.agents {
            self.agents.entry(agent).or_default().merge(a);
        }
        self.span_ms = match (self.span_ms, o.span_ms) {
            (Some(a), Some(b)) => Some((a.0.min(b.0), a.1.max(b.1))),
            (a, b) => a.or(b),
        };
        self.ids.extend(o.ids);
        self.titles.extend(o.titles);
        for (agent, t) in o.tables {
            self.tables.entry(agent).or_default().merge(t);
        }
    }

    /// The agent of the log being read, and its share.
    fn current_share(&mut self) -> Option<&mut AgentShare> {
        let agent = &self.current.as_ref()?.agent;
        if !self.agents.contains_key(agent) {
            self.agents.insert(agent.clone(), AgentShare::default());
        }
        self.agents.get_mut(agent)
    }

    /// The tool table of the log being read.
    fn current_table(&mut self) -> Option<&mut ToolTable> {
        let agent = &self.current.as_ref()?.agent;
        if !self.tables.contains_key(agent) {
            self.tables.insert(agent.clone(), ToolTable::default());
        }
        self.tables.get_mut(agent)
    }
}

/// The name the file gives a hand-off kind, or `None` for one it does not count.
fn handoff_name(kind: HandoffKind) -> Option<(&'static str, bool)> {
    Some(match kind {
        HandoffKind::TurnEnd => (HANDOFF_KINDS[0], false),
        HandoffKind::Interrupt => (HANDOFF_KINDS[1], false),
        HandoffKind::TypedWhileWorking => (HANDOFF_KINDS[2], false),
        HandoffKind::Question => (HANDOFF_KINDS[3], false),
        HandoffKind::Answer => (HANDOFF_KINDS[4], false),
        HandoffKind::Wake(WakeSource::Subagent) => (WAKE_SOURCES[0], true),
        HandoffKind::Wake(WakeSource::Command) => (WAKE_SOURCES[1], true),
        HandoffKind::Wake(_) => (WAKE_SOURCES[2], true),
        #[allow(unreachable_patterns)]
        _ => return None,
    })
}

/// The index of the range a value falls in: the last whose lower bound it reaches.
fn range_of<T: PartialOrd + Copy>(ranges: &[(&'static str, T)], value: T) -> usize {
    ranges.iter().rposition(|(_, lower)| value >= *lower).unwrap_or(0)
}

/// The key a version counts under: the version, when it is digits and dots, optionally followed
/// by `-` and a suffix of letters, digits and dots; `other` otherwise; `unknown` without one.
pub fn version_key(v: Option<&str>) -> &str {
    let Some(v) = v else { return "unknown" };
    let (number, suffix) = v.split_once('-').unwrap_or((v, ""));
    let parts_ok = !number.is_empty() && number.split('.').all(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()));
    let suffix_ok = !v.contains('-') || (!suffix.is_empty() && suffix.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'.'));
    if parts_ok && suffix_ok { v } else { "other" }
}

/// The key an effort level counts under (see [`EFFORT_LEVELS`]).
fn effort_key(e: Option<&str>) -> &str {
    match e {
        None => "unknown",
        Some(level) if EFFORT_LEVELS.contains(&level) => level,
        Some(_) => "other",
    }
}

impl Sink for Share {
    fn session(&mut self, s: &Session) {
        // Compactions count for a log of a main conversation, inside the window, once the log
        // has a selected request.
        let main = s.chains.iter().any(|c| c.kind == ChainKind::Main);
        let compactions = if main {
            s.compactions
                .iter()
                .filter(|c| self.filter.accepts(c.ts_ms))
                .map(|c| match c.kind.name() {
                    Some(name) => COMPACTION_TRIGGERS.iter().copied().find(|t| *t == name).unwrap_or("other"),
                    None => "unknown",
                })
                .collect()
        } else {
            Vec::new()
        };
        let (mut handoffs, mut wakes) = (Vec::new(), Vec::new());
        if main {
            for h in s.handoffs.iter().filter(|h| self.filter.accepts(h.ts_ms)) {
                match handoff_name(h.kind) {
                    Some((name, true)) => wakes.push(name),
                    Some((name, false)) => handoffs.push(name),
                    None => {}
                }
            }
        }
        self.current = Some(Current {
            agent: s.agent.clone(),
            session: s.short_id().to_string(),
            counted: false,
            compactions,
            handoffs,
            wakes,
            ids: vec![s.id.clone(), s.short_id().to_string()],
            title: s.title.clone(),
        });
        if let Some(t) = self.current_table() {
            t.session(s);
        }
    }

    fn request(&mut self, r: &RequestRecord) {
        if let Some(t) = self.current_table() {
            t.request(r);
        }
        let Share { agents, span_ms, current, ids, titles, .. } = self;
        let Some(cur) = current else { return };
        let a = agents.entry(cur.agent.clone()).or_default();
        if !cur.counted {
            cur.counted = true;
            a.logs += 1;
            a.main_sessions.insert(cur.session.clone());
            for t in cur.compactions.drain(..) {
                *a.compactions.entry(t).or_default() += 1;
            }
            for h in cur.handoffs.drain(..) {
                *a.handoffs.entry(h).or_default() += 1;
            }
            for w in cur.wakes.drain(..) {
                *a.wakes.entry(w).or_default() += 1;
            }
            ids.extend(cur.ids.drain(..));
            titles.extend(cur.title.take());
        }
        let usd = r.price.map_or(0.0, |p| p.cost(&r.usage));
        let tokens = (r.usage.prompt() + r.usage.output) as f64;
        let spend = Spend { requests: 1, tokens, usd };
        let k = (r.chain_kind != ChainKind::Main) as usize;
        a.cache_writes[k][0] += r.usage.write_short;
        a.cache_writes[k][1] += r.usage.write_long;
        let e = a.effort[k].entry(effort_key(r.effort).to_string()).or_default();
        e.0 += 1;
        e.1 += r.usage.output;
        e.2 += usd;
        a.prompt_sizes[k][range_of(&PROMPT_RANGES, r.usage.prompt())].add(spend);
        a.versions.entry(version_key(r.version).to_string()).or_default().add(spend);
        a.prompts[1].add(spend);
        if r.composition {
            a.prompts[0].add(spend);
            *a.versions_held.entry(version_key(r.version).to_string()).or_default() += 1;
        }
        if r.chain_kind == ChainKind::Main {
            let hour = r.ts_ms.div_euclid(HOUR_MS);
            a.hours.insert(hour);
            if let Some(hours) = a.client_hours.get_mut(r.client) {
                hours.insert(hour);
            } else {
                a.client_hours.insert(r.client.to_string(), HashSet::from([hour]));
            }
        }
        let ce = a.client_effort.entry(r.client.to_string()).or_default().entry(effort_key(r.effort).to_string()).or_default();
        ce.0 += 1;
        ce.1 += usd;
        let w = a.client_writes.entry(r.client.to_string()).or_default();
        w[0] += r.usage.write_short;
        w[1] += r.usage.write_long;
        a.work.add(r);
        if let Some((short, Some(long))) = r.api.rules().map(|c| (c.short.0, c.long.map(|l| l.0))) {
            a.whatif_lifetimes = Some((short, long));
        }
        a.whatif.request(r);
        *span_ms = Some(span_ms.map_or((r.ts_ms, r.ts_ms), |(first, last)| (first.min(r.ts_ms), last.max(r.ts_ms))));
    }

    fn miss(&mut self, _: MissEvent) {}

    fn fact(&mut self, f: &Fact) {
        if let Some(a) = self.current_share() {
            a.content[f.kind as usize].add(Values { tokens: f.tokens, usd: f.usd.unwrap_or(0.0), avoidable: f.avoidable_usd });
        }
        if let Some(t) = self.current_table() {
            t.fact(f);
        }
    }

    fn block_entered(&mut self, b: &BlockEntry) {
        if let Some(t) = self.current_table() {
            t.block_entered(b);
        }
    }

    fn tool_share(&mut self, t: &ToolShare) {
        if let Some(table) = self.current_table() {
            table.tool_share(t);
        }
    }
}

fn dollars(usd: f64) -> Value {
    json!((usd * 1e4).round() / 1e4)
}

fn whole(tokens: f64) -> Value {
    json!(tokens.round() as u64)
}

fn amount(v: &Values) -> Value {
    json!({"usd": dollars(v.usd), "tokens": whole(v.tokens)})
}

fn spend(s: &Spend) -> Value {
    json!({"requests": s.requests, "usd": dollars(s.usd), "tokens": whole(s.tokens)})
}

/// A cache lifetime as the file names it: `5m`, `1h`, `30m`.
fn lifetime_key(ms: i64) -> String {
    if ms % 3_600_000 == 0 { format!("{}h", ms / 3_600_000) } else { format!("{}m", ms / 60_000) }
}

fn pause_parts(p: &AgentPauses) -> Value {
    let typed: Map<String, Value> =
        p.pause_rows().into_iter().map(|r| (r.name.to_string(), json!({"messages": r.count, "median_context": r.median_context}))).collect();
    let after: Map<String, Value> = p.lifetimes.iter().map(|&ms| (lifetime_key(ms), json!(p.typed.iter().filter(|(pause, _)| *pause > ms).count()))).collect();
    let returns: Map<String, Value> = p
        .return_rows()
        .into_iter()
        .map(|r| {
            (
                r.name.to_string(),
                json!({"returns": r.count, "usd": dollars(r.usd), "avoidable_usd": dollars(r.premium_usd), "median_context": r.median_context}),
            )
        })
        .collect();
    let starters = p.starters();
    let started: Map<String, Value> = Starter::ALL.iter().zip(STARTERS).map(|(s, name)| (name.to_string(), json!(starters[*s as usize]))).collect();
    let follow = p.follow_stats().map(|(median, q1, q3, short)| json!({"median": median, "quarter_1": q1, "quarter_3": q3, "ended_within_3": short}));
    let sizes: Map<String, Value> = p
        .size_rows()
        .into_iter()
        .map(|r| (r.name.to_string(), json!({"sessions": r.count, "usd": dollars(r.usd), "tokens": whole(r.tokens), "cold_returns": r.cold_returns})))
        .collect();
    json!({
        "typed_messages": p.typed.len(),
        "typed_by_pause": typed,
        "typed_after_lifetime": after,
        "cold_returns_by_pause": returns,
        "cold_returns_started_by": started,
        "after_cold_return": follow,
        "sessions_by_largest_context": sizes,
    })
}

/// The busy time by kind, with the part after ends of turn, and the requests: slow, longest and
/// retried. The retries' errors and the longest waits, which carry times, stay out.
fn time_part(b: &crate::busy::Busy) -> Value {
    let kinds: Map<String, Value> =
        b.kinds.iter().filter(|(_, v)| v.0 > 0).map(|(k, (ms, after))| (k.to_string(), json!({"ms": ms, "after_turns_ms": after}))).collect();
    json!({
        "kinds": kinds,
        "requests": b.requests,
        "slow": b.slow,
        "slow_main": b.slow_main,
        "longest_ms": b.longest.map(|(ms, _)| ms),
        "retried": b.retried,
    })
}

/// The main sessions by how long they ran, with their spend and cold returns.
fn span_parts(p: &AgentPauses) -> Value {
    let mut rows = [(0u64, Values::default(), 0u64); 5];
    for s in p.sessions.values() {
        let row = &mut rows[range_of(&SPAN_RANGES, s.span_ms())];
        row.0 += 1;
        row.1.add(Values { tokens: s.tokens, usd: s.usd, avoidable: 0.0 });
        row.2 += s.cold_returns;
    }
    let spans: Map<String, Value> = SPAN_RANGES
        .iter()
        .zip(rows)
        .filter(|(_, (n, _, _))| *n > 0)
        .map(|((name, _), (n, v, cold))| (name.to_string(), json!({"sessions": n, "usd": dollars(v.usd), "tokens": whole(v.tokens), "cold_returns": cold})))
        .collect();
    Value::Object(spans)
}

/// The numbers of a tool row.
fn tool_row(r: &ToolRow) -> Map<String, Value> {
    let mut m = Map::new();
    m.insert("calls".into(), json!(r.calls));
    m.insert("single".into(), json!(r.single));
    m.insert("results".into(), json!(r.results));
    m.insert("result_tokens".into(), whole(r.result_tokens));
    m.insert("inputs".into(), json!(r.inputs));
    m.insert("input_tokens".into(), whole(r.input_tokens));
    m.insert("rent".into(), amount(&r.rent));
    m.insert("trips".into(), amount(&r.trips));
    // The count and the window's spend; the error text of a failure stays out.
    m.insert("failed".into(), json!(r.failed));
    m.insert("after".into(), amount(&r.after));
    m
}

/// The tools of an agent by the names the file keeps, shell tools by program, and the hints of
/// the rows before they are joined.
fn tool_parts(table: &ToolTable, names: &PublicNames, keep_mcp: &[String]) -> (Value, Value) {
    let mut tools: BTreeMap<String, (ToolRow, BTreeMap<String, ToolRow>)> = BTreeMap::new();
    let mut hints: BTreeMap<&'static str, (u64, u64, Values, Values)> = BTreeMap::new();
    for (key, row) in &table.rows {
        if let Some(h) = hint(key, row) {
            let e = hints.entry(h).or_default();
            e.0 += 1;
            e.1 += row.calls;
            e.2.add(row.rent);
            e.3.add(row.trips);
        }
        let (tool, head) = key;
        let name = match names.mcp_server(tool) {
            Some(server) if keep_mcp.iter().any(|k| k == server) => format!("mcp:{server}"),
            Some(_) => "mcp".to_string(),
            None if names.tools.contains(&tool.as_str()) => tool.clone(),
            None => "other".to_string(),
        };
        let shell = matches!(row.class, Class::Tool(ToolClass::Execute | ToolClass::ExecuteOutput)) && names.tools.contains(&tool.as_str());
        let e = tools.entry(name).or_default();
        e.0.add(row);
        if let (true, Some(head)) = (shell, head) {
            e.1.entry(program_of(head).to_string()).or_default().add(row);
        }
    }
    let tools: Map<String, Value> = tools
        .into_iter()
        .map(|(name, (row, programs))| {
            let mut m = tool_row(&row);
            if !programs.is_empty() {
                m.insert("programs".into(), Value::Object(programs.iter().map(|(p, r)| (p.clone(), Value::Object(tool_row(r)))).collect()));
            }
            (name, Value::Object(m))
        })
        .collect();
    let hints: Map<String, Value> = hints
        .into_iter()
        .map(|(h, (rows, calls, rent, trips))| (h.to_string(), json!({"rows": rows, "calls": calls, "rent": amount(&rent), "trips": amount(&trips)})))
        .collect();
    (Value::Object(tools), Value::Object(hints))
}

/// The origins of an agent by kind, with the names that ship with it, and the kept MCP servers,
/// under their kind.
fn origin_parts(work: &Work, names: &PublicNames, keep_mcp: &[String]) -> Value {
    let mut kinds: BTreeMap<String, (Spend, BTreeMap<String, Spend>)> = BTreeMap::new();
    for (origin, sp) in &work.by_origin {
        let (kind, name) = match origin.split_once(':') {
            Some((kind, name)) => (kind, Some(name)),
            None => (origin.as_str(), None),
        };
        let e = kinds.entry(kind.to_string()).or_default();
        e.0.add(*sp);
        if let Some(name) = name.filter(|n| names.origin_name(kind, n) || (kind == "mcp" && keep_mcp.iter().any(|k| k == n))) {
            e.1.entry(name.to_string()).or_default().add(*sp);
        }
    }
    let kinds: Map<String, Value> = kinds
        .into_iter()
        .map(|(kind, (total, named))| {
            let mut v = spend(&total);
            if !named.is_empty() {
                v["names"] = Value::Object(named.iter().map(|(n, s)| (n.clone(), spend(s))).collect());
            }
            (kind, v)
        })
        .collect();
    Value::Object(kinds)
}

/// What the header names besides the window: the tokprof that wrote the file, the platform it
/// ran on, the survey label and the MCP servers kept by name.
pub struct Header<'a> {
    pub tokprof: &'a str,
    pub platform: &'a str,
    pub survey: Option<&'a str>,
    pub mcp_names: &'a [String],
}

/// The file's text: the value as indented JSON, ending with a newline.
pub fn pretty(v: &Value) -> String {
    let mut text = serde_json::to_string_pretty(v).unwrap_or_default();
    text.push('\n');
    text
}

/// The file's content, from one scan into the three collectors with the same filters. `day`
/// turns epoch ms into `YYYY-MM-DD`.
pub fn to_json(c: &Collector, p: &Pauses, s: &Share, h: &Header, day: impl Fn(i64) -> String) -> Value {
    let mut agents = Map::new();
    for (agent, a) in &s.agents {
        let names = &agent.info().public_names;
        // Spend by model and component, as the summary's model table counts it.
        let mut models = Map::new();
        for ((who, model), m) in &c.spend.models {
            if who != agent {
                continue;
            }
            let components: Map<String, Value> = Component::ALL.iter().zip(&m.by_component).map(|(comp, v)| (comp.name().to_string(), amount(v))).collect();
            let mut total = Values::default();
            m.by_component.iter().for_each(|v| total.add(*v));
            models.insert(model.clone(), json!({"requests": m.requests, "priced": m.priced, "components": components, "total": amount(&total)}));
        }
        // The misses the cause table lists, and their premium per client.
        let mut causes: BTreeMap<&str, (u64, u64, f64)> = BTreeMap::new();
        let mut avoidable: BTreeMap<&str, f64> = BTreeMap::new();
        for m in c.misses.iter().filter(|m| &m.agent == agent && !m.cause.expected()) {
            let e = causes.entry(m.cause.name()).or_default();
            e.0 += 1;
            e.1 += m.rewritten;
            e.2 += m.premium_usd.unwrap_or(0.0);
            *avoidable.entry(m.client.as_str()).or_default() += m.premium_usd.unwrap_or(0.0);
        }
        let misses: Map<String, Value> = causes
            .into_iter()
            .map(|(cause, (n, tokens, usd))| (cause.to_string(), json!({"misses": n, "tokens": tokens, "avoidable_usd": dollars(usd)})))
            .collect();
        let pauses = p.agents.get(agent);
        let writes = |w: [u64; 2]| json!({"short": w[0], "long": w[1]});
        let clients: Map<String, Value> = c
            .clients
            .rows
            .iter()
            .filter(|((who, _), _)| who == agent)
            .map(|((_, client), r)| {
                let cold = pauses.and_then(|p| p.cold_returns_by_client.get(client)).copied().unwrap_or(0);
                let w = a.client_writes.get(client).copied().unwrap_or_default();
                let hours = a.client_hours.get(client).map_or(0, HashSet::len);
                let effort: Map<String, Value> = a
                    .client_effort
                    .get(client)
                    .into_iter()
                    .flatten()
                    .map(|(level, (requests, usd))| (level.clone(), json!({"requests": requests, "usd": dollars(*usd)})))
                    .collect();
                (
                    client.clone(),
                    json!({
                        "requests": r.spend.requests,
                        "usd": dollars(r.spend.usd),
                        "avoidable_usd": dollars(avoidable.get(client.as_str()).copied().unwrap_or(0.0)),
                        "main_sessions": r.sessions.len(),
                        "cold_returns": cold,
                        "cache_writes": writes(w),
                        "active_hours": hours,
                        "effort": effort,
                    }),
                )
            })
            .collect();
        let triggers: Map<String, Value> = a.work.triggers().iter().map(|(t, sp)| (t.name().to_string(), spend(&sp.spend))).collect();
        let activities: Map<String, Value> = a.work.activities().iter().map(|(name, sp)| (name.to_string(), spend(&sp.spend))).collect();
        // Setup by source, from the fixed list.
        let mut setup: BTreeMap<&str, (Values, Values)> = BTreeMap::new();
        for ((who, source), cost) in &c.setup.by_source {
            if who != agent {
                continue;
            }
            let name = SETUP_SOURCES.iter().copied().find(|s| s == source).unwrap_or("other");
            let e = setup.entry(name).or_default();
            e.0.add(cost.all);
            e.1.add(cost.subagents);
        }
        let setup: Map<String, Value> = setup
            .into_iter()
            .map(|(name, (all, subagents))| {
                (name.to_string(), json!({"usd": dollars(all.usd), "tokens": whole(all.tokens), "subagents_usd": dollars(subagents.usd)}))
            })
            .collect();
        let effort = |levels: &BTreeMap<String, (u64, u64, f64)>| -> Map<String, Value> {
            levels
                .iter()
                .map(|(level, (requests, output, usd))| (level.clone(), json!({"requests": requests, "output_tokens": output, "usd": dollars(*usd)})))
                .collect()
        };
        let content: Map<String, Value> = BlockKind::ALL
            .iter()
            .zip(&a.content)
            .filter(|(_, v)| v.tokens > 0.0 || v.usd > 0.0)
            .map(|(kind, v)| (kind.name().to_string(), amount(v)))
            .collect();
        let sizes = |rows: &[Spend; 6]| -> Map<String, Value> {
            PROMPT_RANGES.iter().zip(rows).filter(|(_, s)| s.requests > 0).map(|((name, _), s)| (name.to_string(), spend(s))).collect()
        };
        let compactions: Map<String, Value> = a.compactions.iter().map(|(t, n)| (t.to_string(), json!(n))).collect();
        let versions: Map<String, Value> = a
            .versions
            .iter()
            .map(|(v, s)| (v.clone(), json!({"requests": s.requests, "usd": dollars(s.usd), "held": a.versions_held.get(v).copied().unwrap_or(0)})))
            .collect();
        let empty = ToolTable::default();
        let table = s.tables.get(agent).unwrap_or(&empty);
        let (tools, tool_hints) = tool_parts(table, names, h.mcp_names);
        let failures = json!({"failed": table.failed, "refused": table.refused, "denied": table.denied, "after": amount(&table.after)});
        let handoffs: Map<String, Value> = HANDOFF_KINDS.iter().map(|k| (k.to_string(), json!(a.handoffs.get(k).copied().unwrap_or(0)))).collect();
        let wakes: Map<String, Value> = WAKE_SOURCES.iter().map(|k| (k.to_string(), json!(a.wakes.get(k).copied().unwrap_or(0)))).collect();
        let days: BTreeSet<String> = a.hours.iter().map(|h| day(h * HOUR_MS)).collect();
        let mut entry = json!({
            "sessions": a.logs,
            "main_sessions": a.main_sessions.len(),
            "spend": models,
            "clients": clients,
            "cache_writes": {"main": writes(a.cache_writes[0]), "subagents": writes(a.cache_writes[1])},
            "misses": misses,
            "origins": origin_parts(&a.work, names, h.mcp_names),
            "triggers": triggers,
            "activities": activities,
            "setup": setup,
            "effort": {"main": effort(&a.effort[0]), "subagents": effort(&a.effort[1])},
            "content": content,
            "prompt_sizes": {"main": sizes(&a.prompt_sizes[0]), "subagents": sizes(&a.prompt_sizes[1])},
            "tools": tools,
            "tool_hints": tool_hints,
            "compactions": compactions,
            "versions": versions,
            "failures": failures,
            "handoffs": {"turn_ends": handoffs["turn_ends"], "interrupts": handoffs["interrupts"], "typed_while_working": handoffs["typed_while_working"],
                "questions": handoffs["questions"], "answers": handoffs["answers"], "wakes": wakes},
            "prompts": {"held": spend(&a.prompts[0]), "all": spend(&a.prompts[1])},
        });
        let Value::Object(e) = &mut entry else { unreachable!("built as an object") };
        let busy_ms = pauses.map_or(0, |p| p.busy.total_ms());
        e.insert("activity".into(), json!({"active_days": days.len(), "active_hours": a.hours.len(), "busy_ms": busy_ms}));
        if let Some(pauses) = pauses {
            e.insert("pauses".into(), pause_parts(pauses));
            e.insert("session_spans".into(), span_parts(pauses));
            e.insert("time".into(), time_part(&pauses.busy));
        }
        if let (Some((short, long)), true) = (a.whatif_lifetimes, a.whatif.requests.iter().any(|n| *n > 0)) {
            let w = &a.whatif;
            let bucket = |i: usize| {
                let mut m = Map::new();
                m.insert("requests".into(), json!(w.requests[i]));
                m.insert("actual_usd".into(), dollars(w.actual[i]));
                m.insert(format!("simulated_{}_usd", lifetime_key(short)), dollars(w.simulated[i][0]));
                m.insert(format!("simulated_{}_usd", lifetime_key(long)), dollars(w.simulated[i][1]));
                Value::Object(m)
            };
            e.insert("whatif".into(), json!({"main": bucket(0), "subagents": bucket(1)}));
        }
        agents.insert(agent.name().to_string(), entry);
    }
    let mut file = Map::new();
    file.insert("format".into(), json!(FORMAT));
    file.insert("tokprof".into(), json!(h.tokprof));
    file.insert("platform".into(), json!(h.platform));
    file.insert("window".into(), s.span_ms.map_or(Value::Null, |(first, last)| json!({"first": day(first), "last": day(last)})));
    // Every agent's hours together, an hour or a day counted once.
    let hours: HashSet<i64> = s.agents.values().flat_map(|a| a.hours.iter().copied()).collect();
    let days: BTreeSet<String> = hours.iter().map(|h| day(h * HOUR_MS)).collect();
    let busy_ms: i64 = s.agents.keys().filter_map(|agent| p.agents.get(agent)).map(|a| a.busy.total_ms()).sum();
    file.insert("activity".into(), json!({"active_days": days.len(), "active_hours": hours.len(), "busy_ms": busy_ms}));
    if let Some(label) = h.survey {
        file.insert("survey".into(), json!(label));
    }
    if !h.mcp_names.is_empty() {
        file.insert("mcp_names".into(), json!(h.mcp_names));
    }
    file.insert("agents".into(), Value::Object(agents));
    Value::Object(file)
}

/// Whether a survey label is 1 to 40 letters, digits, `.`, `_` or `-`.
pub fn valid_label(label: &str) -> bool {
    (1..=40).contains(&label.chars().count()) && label.chars().all(|c| c.is_alphanumeric() || matches!(c, '.' | '_' | '-'))
}

/// What a word of the file that tokprof did not make is, in the order the review lists them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum WordKind {
    Model,
    Client,
    Version,
    Label,
    Server,
    Other,
}

/// A key or string of the file that is not one of tokprof's own words, and the field it is in.
#[derive(Clone, Debug, PartialEq)]
pub struct Word {
    pub kind: WordKind,
    pub text: String,
    pub field: String,
}

/// tokprof's own words: the fixed names it writes, and the names that ship with the agents.
pub fn own_words(agents: &[AgentId]) -> HashSet<String> {
    let mut own: HashSet<String> = FIELDS.iter().map(|s| s.to_string()).collect();
    let fixed = Component::ALL
        .iter()
        .map(|c| c.name())
        .chain(Cause::ALL.iter().map(|c| c.name()))
        .chain(Trigger::ALL.iter().map(|t| t.name()))
        .chain(CallClass::PRECEDENCE.iter().map(|c| c.name()))
        .chain(SETUP_SOURCES)
        .chain(BlockKind::ALL.iter().map(|k| k.name()))
        .chain(PAUSE_RANGES.iter().map(|r| r.0))
        .chain(RETURN_RANGES.iter().map(|r| r.0))
        .chain(SIZE_RANGES.iter().map(|r| r.0))
        .chain(PROMPT_RANGES.iter().map(|r| r.0))
        .chain(SPAN_RANGES.iter().map(|r| r.0))
        .chain(HINTS)
        .chain(COMPACTION_TRIGGERS)
        .chain(EFFORT_LEVELS)
        .chain(STARTERS)
        .chain(ORIGIN_KINDS)
        .chain(HANDOFF_KINDS)
        .chain(WAKE_SOURCES)
        .chain([crate::busy::MODEL, crate::busy::API_RETRIES, crate::busy::SUBAGENTS, crate::busy::COMPACTION, crate::busy::BACKGROUND, crate::busy::NOTHING])
        .chain(["other", "unknown"])
        .chain(known_programs());
    own.extend(fixed.map(str::to_string));
    for agent in agents {
        own.insert(agent.name().to_string());
        let n = &agent.info().public_names;
        own.extend(n.tools.iter().chain(n.skills).chain(n.subagent_types).map(|s| s.to_string()));
        own.extend(n.commands.iter().map(|c| format!("/{c}")));
    }
    own
}

/// Whether a key is a cache lifetime or a replay under one, such as `5m` or `simulated_1h_usd`.
fn lifetime_word(s: &str) -> bool {
    let s = s.strip_prefix("simulated_").and_then(|r| r.strip_suffix("_usd")).unwrap_or(s);
    s.len() >= 2 && s[..s.len() - 1].bytes().all(|b| b.is_ascii_digit()) && matches!(s.as_bytes()[s.len() - 1], b'm' | b'h')
}

/// Every key or string of the file that is not one of tokprof's own words, by position: models
/// under `spend`, clients under `clients`, versions under `versions`, the survey label, the kept
/// MCP servers, and any other.
pub fn words(v: &Value, own: &HashSet<String>) -> Vec<Word> {
    let mut out = Vec::new();
    let Value::Object(file) = v else { return out };
    for (key, value) in file {
        match (key.as_str(), value) {
            ("survey", Value::String(s)) => out.push(Word { kind: WordKind::Label, text: s.clone(), field: "survey".into() }),
            ("mcp_names", Value::Array(names)) => {
                for n in names {
                    out.push(Word { kind: WordKind::Server, text: n.as_str().map_or_else(|| n.to_string(), str::to_string), field: "mcp_names".into() });
                }
            }
            ("agents", Value::Object(agents)) => {
                for (agent, a) in agents {
                    let path = format!("agents.{agent}");
                    if !own.contains(agent) {
                        out.push(Word { kind: WordKind::Other, text: agent.clone(), field: "agents".into() });
                    }
                    walk_agent(a, &path, own, &mut out);
                }
            }
            // The tokprof version, the platform, the window's days and the activity counts are
            // tokprof's own.
            ("format" | "tokprof" | "platform" | "window" | "activity", _) => {}
            (other, value) => {
                out.push(Word { kind: WordKind::Other, text: other.to_string(), field: String::new() });
                walk(value, other, own, &mut out);
            }
        }
    }
    out
}

fn walk_agent(a: &Value, path: &str, own: &HashSet<String>, out: &mut Vec<Word>) {
    let Value::Object(parts) = a else { return walk(a, path, own, out) };
    for (part, value) in parts {
        let here = format!("{path}.{part}");
        if !own.contains(part) {
            out.push(Word { kind: WordKind::Other, text: part.clone(), field: path.to_string() });
        }
        let open = match part.as_str() {
            "spend" => Some(WordKind::Model),
            "clients" => Some(WordKind::Client),
            "versions" => Some(WordKind::Version),
            _ => None,
        };
        match (open, value) {
            (Some(kind), Value::Object(rows)) => {
                for (name, row) in rows {
                    if !(kind == WordKind::Version && own.contains(name)) {
                        out.push(Word { kind, text: name.clone(), field: here.clone() });
                    }
                    walk(row, &format!("{here}.{name}"), own, out);
                }
            }
            (None, Value::Object(rows)) if part == "tools" || part == "origins" => {
                for (name, row) in rows {
                    match name.strip_prefix("mcp:") {
                        Some(server) => out.push(Word { kind: WordKind::Server, text: server.to_string(), field: here.clone() }),
                        None if !own.contains(name) => out.push(Word { kind: WordKind::Other, text: name.clone(), field: here.clone() }),
                        None => {}
                    }
                    let inner = format!("{here}.{name}");
                    match (name.as_str(), row) {
                        // The names kept under the kind `mcp` are servers.
                        ("mcp", Value::Object(m)) if part == "origins" => {
                            for (k, v) in m {
                                match (k.as_str(), v) {
                                    ("names", Value::Object(servers)) => {
                                        for (server, s) in servers {
                                            out.push(Word { kind: WordKind::Server, text: server.clone(), field: format!("{inner}.names") });
                                            walk(s, &format!("{inner}.names.{server}"), own, out);
                                        }
                                    }
                                    _ => {
                                        if !own.contains(k) {
                                            out.push(Word { kind: WordKind::Other, text: k.clone(), field: inner.clone() });
                                        }
                                        walk(v, &format!("{inner}.{k}"), own, out);
                                    }
                                }
                            }
                        }
                        _ => walk(row, &inner, own, out),
                    }
                }
            }
            _ => walk(value, &here, own, out),
        }
    }
}

/// A part of tokprof's own: every key and string must be one of its own words.
fn walk(v: &Value, path: &str, own: &HashSet<String>, out: &mut Vec<Word>) {
    match v {
        Value::Object(m) => {
            for (k, v) in m {
                if !own.contains(k) && !lifetime_word(k) {
                    out.push(Word { kind: WordKind::Other, text: k.clone(), field: path.to_string() });
                }
                walk(v, &format!("{path}.{k}"), own, out);
            }
        }
        Value::Array(items) => items.iter().for_each(|i| walk(i, path, own, out)),
        Value::String(s) if !own.contains(s) => out.push(Word { kind: WordKind::Other, text: s.clone(), field: path.to_string() }),
        _ => {}
    }
}

/// The words to review, each once, in the order of their kinds.
pub fn review_list(words: &[Word]) -> Vec<String> {
    let mut sorted: Vec<&Word> = words.iter().collect();
    sorted.sort_by_key(|w| w.kind);
    let mut seen = HashSet::new();
    sorted.into_iter().filter(|w| seen.insert(w.text.as_str())).map(|w| w.text.clone()).collect()
}

/// This machine's names, which a share file must not hold.
pub struct Secrets<'a> {
    /// The home directory, as the logs write it.
    pub home: &'a str,
    pub user: Option<&'a str>,
    pub host: Option<&'a str>,
    /// The ids and short ids of the selected sessions.
    pub ids: &'a BTreeSet<String>,
    /// Their titles.
    pub titles: &'a BTreeSet<String>,
}

/// The shortest name the leak check searches for.
const MIN_SECRET: usize = 4;

/// Whether `word` holds `name` as a whole word, ignoring case: with no letter or digit right
/// before or after it.
fn holds_word(word: &str, name: &str) -> bool {
    let (word, name) = (word.to_lowercase(), name.to_lowercase());
    if name.is_empty() {
        return false;
    }
    let edge = |c: Option<char>| c.is_none_or(|c| !c.is_alphanumeric());
    word.match_indices(&name).any(|(i, m)| edge(word[..i].chars().next_back()) && edge(word[i + m.len()..].chars().next()))
}

/// The first of this machine's names that a word to review holds: what kind of name it is, and
/// the field the word is in. tokprof's own words are not searched.
pub fn leak(words: &[Word], s: &Secrets) -> Option<(&'static str, String)> {
    let long = |n: &&String| n.chars().count() >= MIN_SECRET;
    for w in words {
        let text = w.text.to_lowercase();
        let found = if s.home.chars().count() >= MIN_SECRET && text.contains(&s.home.to_lowercase()) {
            Some("the home path")
        } else if s.ids.iter().filter(long).any(|id| text.contains(&id.to_lowercase())) {
            Some("a session id")
        } else if s.titles.iter().filter(long).any(|t| text.contains(&t.to_lowercase())) {
            Some("a session title")
        } else if s.user.is_some_and(|u| u.chars().count() >= MIN_SECRET && holds_word(&w.text, u)) {
            Some("the user name")
        } else if s.host.is_some_and(|h| h.chars().count() >= MIN_SECRET && holds_word(&w.text, h)) {
            Some("the host name")
        } else {
            None
        };
        if let Some(what) = found {
            return Some((what, w.field.clone()));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::Sink;
    use crate::model::{Block, Chain, Compaction, CompactionTrigger, Origin, Params, PromptDelta, Request, Usage};
    use crate::pricing::Pricing;
    use crate::tools::ToolShare;

    fn request(min: i64, chain_effort: Option<&str>, usage: Usage) -> Request {
        Request {
            ts_ms: 1_793_620_800_000 + min * 60_000,
            api: crate::api::ApiId::Anthropic,
            model: "claude-opus-5-5".into(),
            usage,
            params: chain_effort.map(|e| Params { effort: Some(e.into()), ..Params::default() }),
            ..Request::default()
        }
    }

    fn session(agent: AgentId, id: &str, chains: Vec<(&str, ChainKind, Vec<Request>)>) -> Session {
        let mut s = Session::new(agent, id.into());
        s.chains = chains.into_iter().map(|(name, kind, requests)| Chain { name: name.into(), kind, requests }).collect();
        s
    }

    /// The three collectors of one scan, over the given sessions.
    fn collect(sessions: &[Session]) -> (Collector, Pauses, Share) {
        let pricing = Pricing::defaults();
        let filter = Filter::default();
        let mut sink = (Collector::new(None, None), (Pauses::new(filter.clone()), Share::new(filter.clone())));
        for s in sessions {
            crate::analysis::analyze(s, &pricing, &filter, &mut sink);
        }
        let (c, (mut p, s)) = sink;
        p.finish();
        (c, p, s)
    }

    const HEADER: Header<'static> = Header { tokprof: "0.1.0", platform: "macos-aarch64", survey: None, mcp_names: &[] };

    fn file_with(sessions: &[Session], h: &Header) -> Value {
        let (c, p, s) = collect(sessions);
        to_json(&c, &p, &s, h, |_| "2026-11-02".into())
    }

    fn file(sessions: &[Session]) -> Value {
        file_with(sessions, &HEADER)
    }

    fn one() -> Vec<Request> {
        vec![request(0, None, Usage { cache_read: 1_000, output: 1, ..Usage::default() })]
    }

    fn agents() -> Vec<AgentId> {
        vec![crate::testing::claude(), crate::testing::codex()]
    }

    #[test]
    fn cache_writes_by_lifetime_and_effort_by_level() {
        let main = vec![
            request(0, Some("max"), Usage { write_long: 190_000, output: 10, ..Usage::default() }),
            request(1, Some("max"), Usage { cache_read: 190_000, write_long: 700, output: 20, ..Usage::default() }),
            request(2, None, Usage { cache_read: 190_700, output: 5, ..Usage::default() }),
        ];
        let sub = vec![request(3, Some("high"), Usage { write_long: 154_000, write_short: 300, output: 7, ..Usage::default() })];
        let s = session(crate::testing::claude(), "s", vec![("main", ChainKind::Main, main), ("subagent", ChainKind::Subagent, sub)]);
        let (_, _, share) = collect(&[s]);
        let a = &share.agents[&crate::testing::claude()];
        assert_eq!(a.cache_writes, [[0, 190_700], [300, 154_000]]);
        assert_eq!((a.effort[0]["max"].0, a.effort[0]["max"].1), (2, 30));
        assert!(a.effort[0]["max"].2 > 0.0, "effort has dollars");
        assert_eq!((a.effort[0]["unknown"].0, a.effort[0]["unknown"].1), (1, 5));
        assert_eq!((a.effort[1]["high"].0, a.effort[1]["high"].1), (1, 7));
    }

    #[test]
    fn effort_levels_outside_the_list_count_as_other() {
        let main = vec![
            request(0, Some("turbo"), Usage { cache_read: 1_000, output: 1, ..Usage::default() }),
            request(1, Some("xhigh"), Usage { cache_read: 1_000, output: 1, ..Usage::default() }),
        ];
        let v = file(&[session(crate::testing::claude(), "s", vec![("main", ChainKind::Main, main)])]);
        let levels: Vec<&String> = v["agents"]["claude"]["effort"]["main"].as_object().unwrap().keys().collect();
        assert_eq!(levels, ["other", "xhigh"]);
    }

    #[test]
    fn a_session_with_subagents_counts_its_logs_and_one_main_session() {
        let mut logs = vec![session(crate::testing::claude(), "s", vec![("main", ChainKind::Main, one())])];
        for _ in 0..9 {
            logs.push(session(crate::testing::claude(), "s", vec![("subagent", ChainKind::Subagent, one())]));
        }
        // A log with no selected request counts in neither.
        logs.push(session(crate::testing::claude(), "t", vec![("main", ChainKind::Main, vec![request(0, None, Usage::default())])]));
        let v = file(&logs);
        assert_eq!((v["agents"]["claude"]["sessions"].as_u64(), v["agents"]["claude"]["main_sessions"].as_u64()), (Some(10), Some(1)));
    }

    #[test]
    fn the_file_has_its_header_and_one_object_per_agent() {
        let v = file(&[
            session(crate::testing::claude(), "s", vec![("main", ChainKind::Main, one())]),
            session(crate::testing::codex(), "c", vec![("main", ChainKind::Main, one())]),
        ]);
        assert_eq!(v["format"], 2);
        assert_eq!(v["tokprof"], "0.1.0");
        assert_eq!(v["platform"], "macos-aarch64");
        assert_eq!(v["window"], json!({"first": "2026-11-02", "last": "2026-11-02"}));
        assert!(v.get("survey").is_none() && v.get("mcp_names").is_none(), "without a label: {v}");
        let keys: Vec<&String> = v["agents"].as_object().unwrap().keys().collect();
        assert_eq!(keys, ["claude", "codex"]);
    }

    #[test]
    fn a_survey_file_names_its_label_and_kept_servers() {
        let h = Header { survey: Some("team-2026-11"), mcp_names: &["docs".to_string()], ..HEADER };
        let v = file_with(&[session(crate::testing::claude(), "s", vec![("main", ChainKind::Main, one())])], &h);
        assert_eq!((v["survey"].as_str(), &v["mcp_names"]), (Some("team-2026-11"), &json!(["docs"])));
    }

    #[test]
    fn labels_are_letters_digits_and_a_few_signs() {
        assert!(valid_label("team-2026-11") && valid_label("a.b_c") && valid_label(&"x".repeat(40)));
        assert!(!valid_label("team 11") && !valid_label("") && !valid_label(&"x".repeat(41)) && !valid_label("a/b"));
    }

    #[test]
    fn origins_keep_their_kind_and_the_names_that_ship_with_the_agent() {
        let with = |origin: &str, client: &str, read: u64| Request {
            origin: Some(Origin::parse(origin)),
            client: Some(client.into()),
            ..request(0, None, Usage { cache_read: read, output: 10, ..Usage::default() })
        };
        let s = session(
            crate::testing::claude(),
            "s",
            vec![(
                "main",
                ChainKind::Main,
                vec![
                    with("skill:walrus-review", "sdk-ts", 1_000_000),
                    with("skill:release-notes", "sdk-ts", 1_000_000),
                    with("agent:general-purpose", "cli", 4_000_000),
                    with("agent:ocelot", "cli", 1_000_000),
                ],
            )],
        );
        let v = file(&[s]);
        let a = &v["agents"]["claude"];
        let origins: Vec<&String> = a["origins"].as_object().unwrap().keys().collect();
        assert_eq!(origins, ["agent", "skill"]);
        assert_eq!(a["origins"]["skill"]["requests"], 2);
        assert!(a["origins"]["skill"].get("names").is_none(), "no skill name is on the list: {a}");
        let names: Vec<&String> = a["origins"]["agent"]["names"].as_object().unwrap().keys().collect();
        assert_eq!(names, ["general-purpose"]);
        assert_eq!(a["origins"]["agent"]["requests"], 2);
        assert_eq!(a["origins"]["agent"]["names"]["general-purpose"]["requests"], 1);
        let clients = a["clients"].as_object().unwrap();
        assert_eq!((clients["sdk-ts"]["requests"].as_u64(), clients["cli"]["requests"].as_u64()), (Some(2), Some(2)));
        let text = v.to_string();
        for word in ["walrus-review", "release-notes", "ocelot"] {
            assert!(!text.contains(word), "{word} in {text}");
        }
    }

    #[test]
    fn content_prompt_sizes_and_versions_add_up_to_the_spend() {
        let main = vec![
            Request { version: Some("2.1.316".into()), ..request(0, None, Usage { write_long: 400_000, output: 100, ..Usage::default() }) },
            Request {
                version: Some("build of zebra".into()),
                ..request(1, None, Usage { cache_read: 400_000, write_long: 1_000, output: 100, ..Usage::default() })
            },
            request(2, None, Usage { cache_read: 10_000, output: 10, ..Usage::default() }),
        ];
        let s = session(crate::testing::claude(), "s", vec![("main", ChainKind::Main, main)]);
        let v = file(&[s]);
        let a = &v["agents"]["claude"];
        let total: f64 = a["spend"].as_object().unwrap().values().map(|m| m["total"]["usd"].as_f64().unwrap()).sum();
        let sum = |part: &Value| -> f64 { part.as_object().unwrap().values().map(|r| r["usd"].as_f64().unwrap()).sum() };
        assert!((sum(&a["content"]) - total).abs() < 1e-3, "content {} vs spend {total}", sum(&a["content"]));
        assert!((sum(&a["prompt_sizes"]["main"]) - total).abs() < 1e-3);
        // A prompt of 400,000 tokens counts in the range that starts there.
        assert_eq!(a["prompt_sizes"]["main"]["400–800k"]["requests"], 2);
        assert_eq!(a["prompt_sizes"]["main"]["under 50k"]["requests"], 1);
        let versions: Vec<&String> = a["versions"].as_object().unwrap().keys().collect();
        assert_eq!(versions, ["2.1.316", "other", "unknown"]);
        assert!((sum(&a["versions"]) - total).abs() < 1e-3);
    }

    #[test]
    fn versions_are_digits_and_dots_with_a_suffix() {
        assert_eq!(version_key(Some("2.1.316")), "2.1.316");
        assert_eq!(version_key(Some("0.166.0-alpha.5.1")), "0.166.0-alpha.5.1");
        assert_eq!(version_key(Some("build of zebra")), "other");
        assert_eq!(version_key(Some("1.2-")), "other");
        assert_eq!(version_key(Some("/Users/me")), "other");
        assert_eq!(version_key(None), "unknown");
    }

    #[test]
    fn content_kinds_are_indexed_in_declaration_order() {
        for (i, k) in BlockKind::ALL.iter().enumerate() {
            assert_eq!(*k as usize, i);
        }
    }

    #[test]
    fn sessions_fall_in_their_span_and_compactions_count_by_trigger() {
        let main = vec![
            request(0, None, Usage { write_long: 500_000, output: 100, ..Usage::default() }),
            request(57 * 60, None, Usage { cache_read: 10_000, write_long: 492_000, output: 100, ..Usage::default() }),
        ];
        let mut s = session(crate::testing::claude(), "s", vec![("main", ChainKind::Main, main)]);
        let at = |min: i64| 1_793_620_800_000 + min * 60_000;
        s.compactions = vec![
            Compaction { ts_ms: at(10), kind: CompactionTrigger::Auto },
            Compaction { ts_ms: at(20), kind: CompactionTrigger::Auto },
            Compaction { ts_ms: at(30), kind: CompactionTrigger::Idle },
            Compaction { ts_ms: at(40), kind: CompactionTrigger::Logged("fancy".into()) },
        ];
        let v = file(&[s]);
        let a = &v["agents"]["claude"];
        assert_eq!(a["session_spans"]["1–3d"]["sessions"], 1, "57 hours: {a}");
        assert_eq!(a["session_spans"]["1–3d"]["cold_returns"], 1);
        assert_eq!(a["compactions"], json!({"auto": 2, "idle": 1, "other": 1}));
    }

    #[test]
    fn clients_carry_their_cache_lifetimes_sessions_and_cold_returns() {
        let with = |r: Request, client: &str| Request { client: Some(client.into()), ..r };
        let main = vec![
            with(request(0, None, Usage { write_short: 2_100_000, output: 100, ..Usage::default() }), "sdk-ts"),
            with(request(120, None, Usage { write_short: 2_000_000, output: 100, ..Usage::default() }), "sdk-ts"),
        ];
        let v = file(&[session(crate::testing::claude(), "s", vec![("main", ChainKind::Main, main)])]);
        let c = &v["agents"]["claude"]["clients"]["sdk-ts"];
        assert_eq!(c["cache_writes"], json!({"short": 4_100_000, "long": 0}));
        assert_eq!((c["main_sessions"].as_u64(), c["cold_returns"].as_u64()), (Some(1), Some(1)));
    }

    #[test]
    fn the_numbers_are_the_summarys_and_the_pause_reports() {
        // A cold return after two hours, so misses and pauses have something to say.
        let main = vec![
            request(0, None, Usage { write_long: 500_000, output: 100, ..Usage::default() }),
            request(120, None, Usage { cache_read: 10_000, write_long: 492_000, output: 100, ..Usage::default() }),
        ];
        let sessions = [session(crate::testing::claude(), "s", vec![("main", ChainKind::Main, main)])];
        let (c, p, s) = collect(&sessions);
        let v = to_json(&c, &p, &s, &HEADER, |_| String::new());
        let a = &v["agents"]["claude"];
        let model = &c.spend.models[&(crate::testing::claude(), "claude-opus-5-5".to_string())];
        let total: f64 = model.by_component.iter().map(|x| x.usd).sum();
        assert_eq!(a["spend"]["claude-opus-5-5"]["total"]["usd"], dollars(total));
        let miss = &c.misses[0];
        assert_eq!(a["misses"]["idle > TTL"], json!({"misses": 1, "tokens": miss.rewritten, "avoidable_usd": dollars(miss.premium_usd.unwrap())}));
        let pauses = &p.agents[&crate::testing::claude()];
        // Two hours is the lower bound of `2–4h`; empty ranges are left out, as in the report.
        assert_eq!(a["pauses"]["cold_returns_by_pause"]["1–2h"], Value::Null, "{a}");
        assert_eq!(a["pauses"]["cold_returns_by_pause"]["2–4h"]["returns"], pauses.return_rows()[0].count);
        assert_eq!(a["pauses"]["typed_after_lifetime"], json!({"5m": 0, "1h": 0}), "no typed message");
        // The what-if replay is the summary collector's, as `tokprof whatif` prints it.
        assert_eq!(a["whatif"]["main"]["actual_usd"], dollars(c.whatif.actual[0]));
        assert_eq!(a["whatif"]["main"]["simulated_5m_usd"], dollars(c.whatif.simulated[0][0]));
        assert_eq!(a["whatif"]["main"]["simulated_1h_usd"], dollars(c.whatif.simulated[0][1]));
        assert_eq!(a["whatif"]["main"]["requests"], c.whatif.requests[0]);
    }

    #[test]
    fn codex_has_no_whatif() {
        let r = Request {
            api: crate::api::ApiId::OpenAi,
            model: "gpt-6-astra".into(),
            ..request(0, None, Usage { cache_read: 1_000, output: 1, ..Usage::default() })
        };
        let v = file(&[session(crate::testing::codex(), "c", vec![("main", ChainKind::Main, vec![r])])]);
        assert!(v["agents"]["codex"].get("whatif").is_none(), "{v}");
    }

    /// A session whose main conversation called these tools, each a response of its own.
    fn tool_session(calls: &[(&str, Option<&str>, ToolClass)]) -> Session {
        let mut s = session(crate::testing::claude(), "s", vec![]);
        let mut requests = Vec::new();
        for (i, (tool, target, class)) in calls.iter().enumerate() {
            let block = Block {
                class: Class::Tool(*class),
                kind: BlockKind::ToolResult,
                chars: 4_000,
                tool: Some(tool.to_string()),
                target: target.map(str::to_string),
            };
            let m = s.push_message(vec![block], false);
            requests.push(Request {
                prompt: Some(PromptDelta { shape: None, keep: i, tail: vec![m] }),
                ..request(i as i64, None, Usage { cache_read: 10_000 + 1_000 * i as u64, write_long: 1_000, output: 10, ..Usage::default() })
            });
        }
        s.chains = vec![Chain { name: "main".into(), kind: ChainKind::Main, requests }];
        s
    }

    #[test]
    fn tools_keep_the_names_that_ship_with_the_agent_and_shell_tools_split_by_program() {
        let calls = [
            ("Bash", Some("sed -n"), ToolClass::Execute),
            ("Bash", Some("sed -i"), ToolClass::Execute),
            ("Bash", Some("./yak-deploy.sh --fast"), ToolClass::Execute),
            ("Read", Some("src/a.rs"), ToolClass::Read),
            ("mcp__narwhal__search", Some("mcp:narwhal"), ToolClass::Other),
            ("mcp__docs__read", Some("mcp:docs"), ToolClass::Other),
            ("WalrusTool", None, ToolClass::Other),
        ];
        let s = tool_session(&calls);
        let h = Header { mcp_names: &["docs".to_string()], ..HEADER };
        let (c, p, share) = collect(std::slice::from_ref(&s));
        let v = to_json(&c, &p, &share, &h, |_| String::new());
        let tools = &v["agents"]["claude"]["tools"];
        let names: Vec<&String> = tools.as_object().unwrap().keys().collect();
        assert_eq!(names, ["Bash", "Read", "mcp", "mcp:docs", "other"], "{tools}");
        let programs: Vec<&String> = tools["Bash"]["programs"].as_object().unwrap().keys().collect();
        assert_eq!(programs, ["other", "sed"]);
        assert_eq!(tools["Bash"]["programs"]["sed"]["results"], 2, "sed -n and sed -i join under sed");
        // Rent adds up to the tool table's, as `tokprof tools` counts it.
        let rent: f64 = tools.as_object().unwrap().values().map(|t| t["rent"]["usd"].as_f64().unwrap()).sum();
        let table: f64 = c.tools.rows.values().map(|r| r.rent.usd).sum();
        assert!((rent - table).abs() < 1e-3, "{rent} vs {table}");
        let text = v.to_string();
        for word in ["narwhal", "yak", "WalrusTool"] {
            assert!(!text.contains(word), "{word} in {text}");
        }
    }

    #[test]
    fn tool_hints_count_the_rows_before_they_join() {
        let mut table = ToolTable::default();
        let row = |calls: u64, single: u64, rent: f64| ToolRow {
            class: Class::Tool(ToolClass::Execute),
            calls,
            single,
            results: calls,
            result_tokens: 1_300.0 * calls as f64,
            inputs: calls,
            input_tokens: 50.0 * calls as f64,
            rent: Values { tokens: 1.0, usd: rent, avoidable: 0.0 },
            trips: Values { tokens: 1.0, usd: rent / 2.0, avoidable: 0.0 },
            ..ToolRow::default()
        };
        table.rows.insert(("Bash".into(), Some("sed -n".into())), row(56, 48, 5.21));
        table.rows.insert(("Bash".into(), Some("git grep".into())), row(40, 36, 3.0));
        table.rows.insert(("Bash".into(), Some("cargo test".into())), row(2, 2, 0.5));
        let names = crate::testing::claude().info().public_names.clone();
        let (_, hints) = tool_parts(&table, &names, &[]);
        assert_eq!(hints["batch independent reads"]["rows"], 2);
        assert_eq!(hints["batch independent reads"]["calls"], 96);
        assert_eq!(hints["batch independent reads"]["rent"]["usd"], dollars(8.21));
        assert_eq!(hints.as_object().unwrap().len(), 1);
    }

    #[test]
    fn the_file_holds_no_names() {
        let mut s = session(crate::testing::claude(), "5e5510f0-zebra", vec![]);
        s.project = "~/src/zebra-billing".into();
        s.cwd = Some("/Users/me/src/zebra-billing".into());
        s.title = Some("Fix quokka export".into());
        s.task = Some("ask the walrus".into());
        let block = |kind, tool: Option<&str>, target: Option<&str>, class| Block {
            class,
            kind,
            chars: 4_000,
            tool: tool.map(str::to_string),
            target: target.map(str::to_string),
        };
        let prompt = s.push_message(vec![block(BlockKind::Prompt, None, Some("ping the yak"), Class::None)], true);
        let read = s.push_message(vec![block(BlockKind::ToolResult, Some("Read"), Some("/tmp/heron.txt"), Class::Tool(ToolClass::Read))], false);
        let reminder = s.push_message(vec![block(BlockKind::Reminder, None, Some("narwhal server notes"), Class::None)], false);
        let script = s.push_message(vec![block(BlockKind::ToolResult, Some("Bash"), Some("./yak-deploy.sh --fast"), Class::Tool(ToolClass::Execute))], false);
        let mcp = s.push_message(vec![block(BlockKind::ToolResult, Some("mcp__narwhal__search"), Some("mcp:narwhal"), Class::Tool(ToolClass::Other))], false);
        let r = |min, keep, tail: Vec<usize>, origin: &str| Request {
            origin: Some(Origin::parse(origin)),
            prompt: Some(PromptDelta { shape: None, keep, tail }),
            ..request(min, Some("max"), Usage { cache_read: 10_000, write_long: 2_000, output: 10, ..Usage::default() })
        };
        s.chains = vec![Chain {
            name: "main".into(),
            kind: ChainKind::Main,
            requests: vec![
                r(0, 0, vec![prompt], "skill:walrus-review"),
                r(1, 1, vec![read], "mcp:narwhal"),
                r(2, 2, vec![reminder], "agent:ocelot"),
                r(3, 3, vec![script], "command:/walrus-sync"),
                r(4, 4, vec![mcp], "prompt"),
            ],
        }];
        let text = serde_json::to_string_pretty(&file(&[s])).unwrap();
        for word in ["zebra", "quokka", "walrus", "narwhal", "ocelot", "heron", "yak", "5e5510f0", "/Users/", "~/"] {
            assert!(!text.contains(word), "{word} leaked into {text}");
        }
    }

    #[test]
    fn the_words_to_review_are_the_models_clients_and_versions() {
        let r = |model: &str, client: &str, version: &str| Request {
            model: model.into(),
            client: Some(client.into()),
            version: Some(version.into()),
            ..request(0, None, Usage { cache_read: 1_000, output: 1, ..Usage::default() })
        };
        let codex = |model: &str, client: &str, version: &str| Request { api: crate::api::ApiId::OpenAi, ..r(model, client, version) };
        let v = file(&[
            session(crate::testing::claude(), "s", vec![("main", ChainKind::Main, vec![r("claude-opus-5-5", "sdk-ts", "2.1.316")])]),
            session(crate::testing::codex(), "c", vec![("main", ChainKind::Main, vec![codex("gpt-6-astra", "codex_exec", "0.166.0")])]),
        ]);
        let own = own_words(&agents());
        let w = words(&v, &own);
        assert_eq!(review_list(&w), ["claude-opus-5-5", "gpt-6-astra", "sdk-ts", "codex_exec", "2.1.316", "0.166.0"], "{w:?}");
        // Every kind of origin is tokprof's own word.
        let mut with_kinds = v.clone();
        for kind in ORIGIN_KINDS {
            with_kinds["agents"]["claude"]["origins"][kind] = json!({"requests": 1, "usd": 0.0, "tokens": 1});
        }
        assert_eq!(review_list(&words(&with_kinds, &own)).len(), 6);
    }

    #[test]
    fn a_string_of_no_known_kind_comes_last() {
        let mut v = file(&[session(crate::testing::claude(), "s", vec![("main", ChainKind::Main, one())])]);
        v["agents"]["claude"]["triggers"]["zebra-billing"] = json!({"requests": 1});
        v["agents"]["claude"]["tools"]["walrus"] = json!({"calls": 1});
        let w = words(&v, &own_words(&agents()));
        let list = review_list(&w);
        assert_eq!(&list[list.len() - 2..], ["walrus", "zebra-billing"], "{list:?}");
        assert!(w.iter().any(|x| x.text == "zebra-billing" && x.field == "agents.claude.triggers"), "{w:?}");
    }

    #[test]
    fn the_leak_check_finds_this_machines_names_in_the_words_to_review() {
        let ids: BTreeSet<String> = ["5e5510f0-1111".to_string(), "5e5510f0".to_string()].into();
        let titles: BTreeSet<String> = ["Fix quokka export".to_string(), "thinking".to_string()].into();
        let secrets = Secrets { home: "/Users/me", user: Some("user"), host: Some("walrus-mbp"), ids: &ids, titles: &titles };
        let model = |text: &str| vec![Word { kind: WordKind::Model, text: text.into(), field: "agents.claude.spend".into() }];
        assert_eq!(leak(&model("/Users/me/models/m1"), &secrets), Some(("the home path", "agents.claude.spend".to_string())));
        assert_eq!(leak(&model("claude-5e5510f0"), &secrets).map(|l| l.0), Some("a session id"));
        assert_eq!(leak(&model("Fix quokka export model"), &secrets).map(|l| l.0), Some("a session title"));
        assert_eq!(leak(&model("walrus-mbp-proxy"), &secrets).map(|l| l.0), Some("the host name"));
        assert_eq!(leak(&model("claude-opus-5-5"), &secrets), None, "nothing found");
        // tokprof's own words are not words to review, so `user prompt` and `thinking` stay.
        let v = file(&[session(crate::testing::claude(), "s", vec![("main", ChainKind::Main, one())])]);
        let w = words(&v, &own_words(&agents()));
        assert_eq!(leak(&w, &secrets), None, "{w:?}");
    }

    /// The three collectors of one scan with this filter, over the given sessions.
    fn collect_with(filter: Filter, sessions: &[Session]) -> (Collector, Pauses, Share) {
        let pricing = Pricing::defaults();
        let mut sink = (Collector::new(None, None), (Pauses::new(filter.clone()), Share::new(filter.clone())));
        for s in sessions {
            crate::analysis::analyze(s, &pricing, &filter, &mut sink);
        }
        let (c, (mut p, s)) = sink;
        p.finish();
        (c, p, s)
    }

    /// A request record of a main chain, as the analysis hands it to the sinks.
    fn record(agent: &AgentId, index: i64, price: Option<crate::pricing::Price>) -> crate::analysis::RequestRecord<'_> {
        crate::analysis::RequestRecord {
            agent,
            ts_ms: 1_793_620_800_000 + index * 60_000,
            chain: "main",
            chain_index: 0,
            api: crate::api::ApiId::Anthropic,
            chain_kind: ChainKind::Main,
            advisor: false,
            origin: "prompt",
            trigger: crate::trigger::Trigger::Run,
            activity: "building and testing",
            task: "",
            model: "claude-opus-5-5",
            price,
            usage: Usage { cache_read: 100_000, output: 100, ..Usage::default() },
            gap_s: Some(60.0),
            prev_prompt: Some(100_000),
            composition: true,
            client: "sdk-ts",
            effort: None,
            version: None,
        }
    }

    #[test]
    fn failures_are_the_tools_commands_and_hold_no_error_text() {
        let agent = crate::testing::claude();
        let s = session(agent.clone(), "s", vec![("main", ChainKind::Main, vec![])]);
        let price = Pricing::defaults().get("claude-opus-5-5", 100_000, 1_793_620_800_000);
        let mut sink = (Collector::new(None, None), (Pauses::new(Filter::default()), Share::new(Filter::default())));
        let call = |outcome, error| ToolShare {
            tool: Some("Bash"),
            target: Some("cargo test"),
            class: Class::Tool(ToolClass::Execute),
            tokens: 10.0,
            usd: 0.01,
            call: Some(true),
            outcome,
            error,
        };
        let edit = |outcome| ToolShare { tool: Some("Edit"), target: None, class: Class::Tool(ToolClass::Edit), ..call(outcome, None) };
        sink.session(&s);
        sink.request(&record(&agent, 0, price));
        sink.tool_share(&call(crate::model::Outcome::Failed, Some("cannot open ~/src/zebra-billing/a.rs")));
        sink.request(&record(&agent, 1, price));
        sink.request(&record(&agent, 2, price));
        sink.tool_share(&call(crate::model::Outcome::Worked, None));
        sink.tool_share(&edit(crate::model::Outcome::Refused));
        sink.tool_share(&edit(crate::model::Outcome::Denied));
        sink.request(&record(&agent, 3, price));
        let (c, (p, sh)) = sink;
        let v = to_json(&c, &p, &sh, &HEADER, |_| "2026-11-02".into());
        let f = &v["agents"]["claude"]["failures"];
        assert_eq!((c.tools.failed, c.tools.refused, c.tools.denied), (1, 1, 1));
        assert!(c.tools.after.usd > 0.0, "the window holds the requests after the failure");
        assert_eq!(f["failed"], c.tools.failed);
        assert_eq!(f["refused"], c.tools.refused);
        assert_eq!(f["denied"], c.tools.denied);
        assert_eq!(f["after"]["usd"], dollars(c.tools.after.usd));
        let bash = &v["agents"]["claude"]["tools"]["Bash"];
        assert_eq!((bash["failed"].as_u64(), &bash["programs"]["cargo"]["failed"]), (Some(1), &json!(1)));
        assert_eq!(bash["after"]["usd"], dollars(c.tools.after.usd));
        assert!(!v.to_string().contains("zebra"), "{v}");
    }

    /// The local day of a moment `offset_h` hours east of UTC.
    fn day_at(offset_h: i64) -> impl Fn(i64) -> String {
        move |ms| chrono::DateTime::from_timestamp_millis(ms + offset_h * 3_600_000).unwrap().format("%Y-%m-%d").to_string()
    }

    #[test]
    fn active_hours_are_hours_of_main_requests_and_days_are_local() {
        // 2026-11-02 at 10:05, 10:50 and 11:10 UTC; and a subagent alone at 15:00.
        let at = |h: i64, m: i64| (h - 12) * 60 + m;
        let one = |min| request(min, None, Usage { cache_read: 1_000, output: 1, ..Usage::default() });
        let main = vec![one(at(10, 5)), one(at(10, 50)), one(at(11, 10))];
        let sub = vec![one(at(15, 0))];
        let s = session(crate::testing::claude(), "s", vec![("main", ChainKind::Main, main), ("subagent", ChainKind::Subagent, sub)]);
        let (c, p, sh) = collect(std::slice::from_ref(&s));
        let v = to_json(&c, &p, &sh, &HEADER, day_at(0));
        assert_eq!(v["agents"]["claude"]["activity"]["active_hours"], 2, "a subagent alone is no active hour");
        assert_eq!(v["agents"]["claude"]["activity"]["active_days"], 1);
        assert_eq!(v["agents"]["claude"]["clients"]["unknown"]["active_hours"], 2);
        // A request at 23:30 UTC falls on the next day two hours east.
        let late = session(crate::testing::claude(), "s", vec![("main", ChainKind::Main, vec![one(at(10, 0)), one(at(23, 30))])]);
        let (c, p, sh) = collect(&[late]);
        assert_eq!(to_json(&c, &p, &sh, &HEADER, day_at(0))["agents"]["claude"]["activity"]["active_days"], 1);
        assert_eq!(to_json(&c, &p, &sh, &HEADER, day_at(2))["agents"]["claude"]["activity"]["active_days"], 2);
    }

    #[test]
    fn the_file_counts_an_hour_of_two_agents_once() {
        let one = |min| request(min, None, Usage { cache_read: 1_000, output: 1, ..Usage::default() });
        let codex = |min| Request { api: crate::api::ApiId::OpenAi, model: "gpt-6-astra".into(), ..one(min) };
        let v = file(&[
            session(crate::testing::claude(), "s", vec![("main", ChainKind::Main, vec![one(0)])]),
            session(crate::testing::codex(), "c", vec![("main", ChainKind::Main, vec![codex(10)])]),
        ]);
        assert_eq!(v["agents"]["claude"]["activity"]["active_hours"], 1);
        assert_eq!(v["agents"]["codex"]["activity"]["active_hours"], 1);
        assert_eq!(v["activity"]["active_hours"], 1);
        assert_eq!(v["activity"]["active_days"], 1);
    }

    #[test]
    fn hand_offs_of_main_logs_count_inside_the_window() {
        let one = |min| request(min, None, Usage { cache_read: 1_000, output: 1, ..Usage::default() });
        let mut s = session(crate::testing::claude(), "s", vec![("main", ChainKind::Main, vec![one(0), one(100)])]);
        let at = |min: i64| 1_793_620_800_000 + min * 60_000;
        let mut push = |kind, n: i64, from: i64| (0..n).for_each(|i| s.push_handoff(at(from + i), kind));
        push(HandoffKind::TurnEnd, 10, 1);
        push(HandoffKind::Interrupt, 2, 20);
        push(HandoffKind::Question, 3, 30);
        push(HandoffKind::Answer, 3, 40);
        push(HandoffKind::Wake(WakeSource::Command), 4, 50);
        // An end of turn the day before the window.
        s.push_handoff(at(-24 * 60), HandoffKind::TurnEnd);
        let filter = Filter { since_ms: Some(at(-60)), ..Filter::default() };
        let (c, p, sh) = collect_with(filter, &[s]);
        let h = &to_json(&c, &p, &sh, &HEADER, day_at(0))["agents"]["claude"]["handoffs"];
        assert_eq!((h["turn_ends"].as_u64(), h["interrupts"].as_u64(), h["questions"].as_u64(), h["answers"].as_u64()), (Some(10), Some(2), Some(3), Some(3)));
        assert_eq!(h["wakes"], json!({"subagent": 0, "command": 4, "other": 0}));
        assert_eq!(h["typed_while_working"], 0);
    }

    #[test]
    fn prompt_coverage_counts_the_requests_whose_prompts_the_log_holds() {
        let held = |min, version: &str| Request {
            version: Some(version.into()),
            prompt: Some(PromptDelta { shape: None, keep: 0, tail: vec![] }),
            ..request(min, None, Usage { cache_read: 1_000, output: 1, ..Usage::default() })
        };
        let unheld =
            |min, version: &str| Request { version: Some(version.into()), ..request(min, None, Usage { cache_read: 1_000, output: 1, ..Usage::default() }) };
        let s = session(crate::testing::claude(), "s", vec![("main", ChainKind::Main, vec![unheld(0, "2.1.296"), unheld(1, "2.1.296"), held(2, "2.1.316")])]);
        let v = file(&[s]);
        let a = &v["agents"]["claude"];
        assert_eq!((a["prompts"]["held"]["requests"].as_u64(), a["prompts"]["all"]["requests"].as_u64()), (Some(1), Some(3)));
        assert_eq!(a["versions"]["2.1.296"]["held"], 0);
        assert_eq!(a["versions"]["2.1.316"]["held"], 1);
    }

    #[test]
    fn each_client_has_its_effort() {
        let at = |min, effort: &str, client: &str| Request {
            client: Some(client.into()),
            ..request(min, Some(effort), Usage { cache_read: 100_000, output: 100, ..Usage::default() })
        };
        let mut requests: Vec<Request> = (0..9).map(|i| at(i, "max", "sdk-ts")).collect();
        requests.push(at(9, "medium", "sdk-ts"));
        let v = file(&[session(crate::testing::claude(), "s", vec![("main", ChainKind::Main, requests)])]);
        let e = &v["agents"]["claude"]["clients"]["sdk-ts"]["effort"];
        assert_eq!((e["max"]["requests"].as_u64(), e["medium"]["requests"].as_u64()), (Some(9), Some(1)));
        assert!(e["max"]["usd"].as_f64().unwrap() > e["medium"]["usd"].as_f64().unwrap());
    }

    #[test]
    fn the_time_part_holds_no_retry_error_and_the_new_parts_nothing_to_review() {
        let agent = crate::testing::claude();
        let retried = Request {
            retries: 2,
            retry_error: Some("502 Bad Gateway from walrus.internal".into()),
            first_sent_ms: Some(1_793_620_800_000),
            sent_ms: Some(1_793_620_830_000),
            ended_ms: Some(1_793_620_900_000),
            trigger: Some(crate::trigger::Trigger::Prompt),
            ..request(0, None, Usage { cache_read: 1_000, output: 10, ..Usage::default() })
        };
        let mut s = session(agent.clone(), "s", vec![("main", ChainKind::Main, vec![retried])]);
        s.push_handoff(1_793_620_900_000, HandoffKind::TurnEnd);
        let (c, mut p, sh) = collect(std::slice::from_ref(&s));
        p.agents.get_mut(&agent).unwrap().busy = crate::busy::Parts::of_main_in(&s, None).finish();
        let v = to_json(&c, &p, &sh, &HEADER, day_at(0));
        let t = &v["agents"]["claude"]["time"];
        assert_eq!((t["retried"].as_u64(), t["requests"].as_u64()), (Some(1), Some(1)));
        assert_eq!(t["kinds"]["API retries"]["ms"], 30_000);
        assert!(!v.to_string().contains("walrus"), "{v}");
        // Every key of the new parts is tokprof's own.
        let w = words(&v, &own_words(&agents()));
        assert!(w.iter().all(|w| w.kind != WordKind::Other), "{w:?}");
    }

    #[test]
    fn a_whole_word_counts_for_user_and_host_names() {
        assert!(holds_word("sdk-ts", "sdk") && !holds_word("sdk-ts", "sd") && holds_word("a_b-C", "c"));
        assert!(holds_word("walrus-mbp-proxy", "walrus-mbp") && !holds_word("walrus-mbpx", "walrus-mbp"));
    }
}
