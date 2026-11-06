//! Terminal reports for one session: `tokprof` (the latest session on this machine),
//! `tokprof scan` (a given session), and the `tokprof sessions` list.

use std::collections::{BTreeMap, HashMap};
use std::fmt::Write as _;
use std::path::Path;

use anyhow::{Result, bail};
use rayon::prelude::*;

use crate::advice::{self, Advice};
use crate::analysis::cache::Cause;
use crate::analysis::{self, Component, Fact, Filter, LiveContext, MissEvent, RequestRecord, Sink};
use crate::catalog::Catalog;
use crate::fmt::count;
use crate::fmt::local_time;
use crate::forecast::{CacheState, cache_state, conversation, long_ttl, pace};
use crate::labels::display_path;
use crate::model::{BlockKind, ChainKind, CompactionTrigger, Session, TypedMessage, Usage};
use crate::pricing::{Price, Pricing};
use crate::profile::Values;
use crate::query::Query;
pub use crate::query::id_starts_with;
use crate::report::{Spend, Work};
use crate::setup::{self, MeanStart, SetupSpend, SetupView, Start};
use crate::source::{AgentId, SessionRef};
use crate::tools::{BlockEntry, ToolShare, ToolTable};
use crate::trigger::Trigger;

/// The main logs of sessions, without subagents' logs, newest first.
pub fn main_sessions(catalog: &Catalog, agents: &[AgentId]) -> Vec<SessionRef> {
    let mut files: Vec<SessionRef> = catalog.discover(agents).into_iter().filter(|f| !f.subagent).collect();
    files.sort_by_key(|f| std::cmp::Reverse(f.modified_ns));
    files
}

/// The main session active last among the selected sessions. Logs are read newest written first,
/// and only while one could still be newer: a session's last activity is never after its log's
/// last write.
pub fn latest(catalog: &Catalog, select: &Query) -> Option<SessionRef> {
    let mut best: Option<(i64, SessionRef)> = None;
    for f in main_sessions(catalog, &select.agents).into_iter().filter(|f| select.times(f)) {
        // A file system that keeps no modification time gives 0, which bounds nothing.
        if let Some((t, _)) = &best
            && f.modified_ns > 0
            && f.modified_ms() <= *t
        {
            break;
        }
        if select.reads_meta() {
            let head = catalog.head(&f);
            if !select.meta(&head.project, &head.session_id, head.client.as_deref()) {
                continue;
            }
        }
        let t = catalog.last_active(&f);
        if best.as_ref().is_none_or(|(b, _)| t > *b) {
            best = Some((t, f));
        }
    }
    best.map(|(_, f)| f)
}

/// A session file given by path, or among the selected sessions by the start of its id.
pub fn find(spec: &str, catalog: &Catalog, select: &Query) -> Result<SessionRef> {
    let path = Path::new(spec);
    // An absolute path, so that what is printed about the file can be opened from anywhere. The
    // source that claims the file reads it, whatever the agents asked for.
    let absolute = std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf());
    if let Some(file) = catalog.resolve(&absolute) {
        return Ok(file);
    }
    let matches: Vec<SessionRow> = main_sessions(catalog, &select.agents)
        .into_iter()
        .filter(|f| select.times(f) && id_starts_with(&f.session_id, spec))
        .map(|f| SessionRow::new(catalog, f))
        .filter(|r| select.row(r))
        .collect();
    match matches.as_slice() {
        [] => bail!("no session file or session id matches {spec:?}; see `tokprof sessions`"),
        [one] => Ok(one.file.clone()),
        many => bail!("{}", ambiguous(spec, many)),
    }
}

/// The error for a prefix that several sessions share: up to 10 of them, newest first by last
/// activity.
fn ambiguous(spec: &str, rows: &[SessionRow]) -> String {
    let mut out = format!("{spec:?} matches {} sessions; give more of the id:", rows.len());
    let mut rows = rows.to_vec();
    rows.sort_by(newest_first);
    for r in rows.iter().take(10) {
        let _ = write!(out, "\n  {:<18}  {}  {:<6}  {}", r.id, local_time(r.active_ms, "%Y-%m-%d %H:%M"), r.file.agent.name(), r.project);
    }
    if rows.len() > 10 {
        let _ = write!(out, "\n  … and {} more", rows.len() - 10);
    }
    out
}

/// Totals for one session, its subagents included.
#[derive(Default)]
struct Totals {
    components: [Values; 5],
    priced: bool,
    content: HashMap<(BlockKind, Option<String>, Option<String>), Values>,
    chains: BTreeMap<String, Values>,
    misses: Vec<MissEvent>,
    models: BTreeMap<String, u64>,
    clients: BTreeMap<String, u64>,
    requests: u64,
    work: Work,
    /// One row per request, for the prompts block; the file's index among the session's logs.
    rows: Vec<RequestRow>,
    file: usize,
    tools: ToolTable,
    setup: SetupSpend,
}

/// The setup block of a session: what it cost, what the main conversation and the subagents
/// start with, and what the latest compaction left.
fn setup_of(main: &Session, sessions: &[Session], totals: &Totals, spend_usd: f64) -> Option<SetupView> {
    let msg_chars = analysis::message_chars(main);
    let starts: Vec<Start> = sessions
        .par_iter()
        .flat_map_iter(|s| {
            let chars = analysis::message_chars(s);
            s.chains.iter().filter(|c| c.kind == ChainKind::Subagent).filter_map(|c| setup::start_of(s, c, &chars)).collect::<Vec<_>>()
        })
        .collect();
    let spend = Values { tokens: totals.components.iter().map(|v| v.tokens).sum(), usd: spend_usd, avoidable: 0.0 };
    setup::setup_view(&totals.setup, &main.agent, spend, setup::main_start(main, &msg_chars), MeanStart::of(&starts), setup::leftover(main, &msg_chars))
}

/// A request as the prompts block needs it.
#[derive(Clone, Debug)]
struct RequestRow {
    file: usize,
    chain: String,
    main: bool,
    ts_ms: i64,
    usd: f64,
    tokens: f64,
    /// The prompt's size: what the request sent, cached or not.
    prompt: u64,
    activity: &'static str,
}

/// A request of the main conversation, as the window's request track draws it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RequestPoint {
    pub ts_ms: i64,
    pub prompt_tokens: u64,
    pub usd: f64,
}

impl Sink for Totals {
    fn session(&mut self, s: &Session) {
        self.file += 1;
        self.tools.session(s);
    }

    fn request(&mut self, r: &RequestRecord) {
        self.tools.request(r);
        self.rows.push(RequestRow {
            file: self.file,
            chain: r.chain.to_string(),
            // An advisor call counts when it was made, like the main request it was part of.
            main: self.file == 1 && (r.chain_kind == ChainKind::Main || r.advisor),
            ts_ms: r.ts_ms,
            usd: r.price.map_or(0.0, |p| p.cost(&r.usage)),
            tokens: (r.usage.prompt() + r.usage.output) as f64,
            prompt: r.usage.prompt(),
            activity: r.activity,
        });
        self.requests += 1;
        *self.models.entry(r.model.to_string()).or_default() += 1;
        match self.clients.get_mut(r.client) {
            Some(n) => *n += 1,
            None => {
                self.clients.insert(r.client.to_string(), 1);
            }
        }
        self.priced |= r.price.is_some();
        self.work.request(r);
    }

    fn miss(&mut self, miss: MissEvent) {
        self.misses.miss(miss);
    }

    fn block_entered(&mut self, b: &BlockEntry) {
        self.tools.block_entered(b);
    }

    fn tool_share(&mut self, t: &ToolShare) {
        self.tools.tool_share(t);
    }

    fn fact(&mut self, f: &Fact) {
        self.tools.fact(f);
        self.setup.add(f);
        let v = Values { tokens: f.tokens, usd: f.usd.unwrap_or(0.0), avoidable: f.avoidable_usd };
        let add = |a: &mut Values| {
            a.tokens += v.tokens;
            a.usd += v.usd;
            a.avoidable += v.avoidable;
        };
        add(&mut self.components[Component::ALL.iter().position(|c| *c == f.component).unwrap_or(0)]);
        add(self.content.entry((f.kind, f.tool.map(str::to_string), f.target.map(str::to_string))).or_default());
        let chain = if f.chain.starts_with("subagent") { "subagents" } else { f.chain };
        add(self.chains.entry(chain.to_string()).or_default());
    }
}

/// What the X-ray of a session shows, as of a moment: its header, the state of its context and
/// cache, the advice, where the money went, what the context holds and the cache misses.
#[derive(Clone, Debug)]
pub struct XRay {
    pub agent: AgentId,
    pub project: String,
    pub id: String,
    pub short_id: String,
    pub title: Option<String>,
    /// Models by name, with their request counts.
    pub models: Vec<(String, u64)>,
    /// The clients of the session's requests, subagents included, with their request counts,
    /// most requests first.
    pub clients: Vec<(String, u64)>,
    /// The first and the last request, in epoch ms, when requests have times.
    pub span_ms: Option<(i64, i64)>,
    pub requests: u64,
    pub subagents: usize,
    /// The latest prompt of the main conversation and its cache; `None` when no request has a
    /// prompt size.
    pub context: Option<ContextNow>,
    pub advice: Vec<Advice>,
    /// Whether any request had a price; amounts are tokens otherwise.
    pub priced: bool,
    /// Spend by component, in `Component::ALL` order.
    pub components: [Values; 5],
    /// Spend by chain, subagents together, by name.
    pub chains: Vec<(String, Values)>,
    pub work: Work,
    pub composition: Option<Composition>,
    /// What each piece of content cost over every request it was part of: kind, tool, target.
    pub content: Vec<(ContentKey, Values)>,
    /// Tool calls by tool and target, the session's subagents included.
    pub calls: HashMap<(Option<String>, Option<String>), u64>,
    /// What each message the user typed cost; empty for fewer than two messages.
    pub prompts: Vec<PromptSpend>,
    /// Spend by tool, the session's subagents included.
    pub tools: ToolTable,
    /// What every request carries before the work; `None` below the block's thresholds.
    pub setup: Option<SetupView>,
    pub misses: Vec<MissEvent>,
    /// The session's turns, work and answers; `None` without turns or when nobody typed in it.
    pub rhythm: Option<crate::rhythm::SessionRhythm>,
    /// Where the agent's time went while it was busy, and its slow requests (see agent-time).
    pub busy: crate::busy::Busy,
    /// The main conversation's requests in time order, its advisor calls included; requests
    /// without a time are left out.
    pub points: Vec<RequestPoint>,
}

/// What one typed message cost: the requests from it to the next typed message, and the
/// subagents started in that time.
#[derive(Clone, Debug)]
pub struct PromptSpend {
    /// Its number among the session's typed messages, from 1.
    pub number: usize,
    pub label: String,
    pub spend: Spend,
    /// Activities, most expensive first, with their spend.
    pub activities: Vec<(&'static str, Spend)>,
}

/// A piece of content: its kind, tool and target.
pub type ContentKey = (BlockKind, Option<String>, Option<String>);

/// The latest prompt of the main conversation, and its cache.
#[derive(Clone, Debug)]
pub struct ContextNow {
    /// The prompt's size, or the estimate of the next prompt after a trailing compaction.
    pub size_tokens: u64,
    /// The size is an estimate.
    pub approx: bool,
    pub window_tokens: Option<u64>,
    pub cache: CacheState,
    pub trailing: Option<TrailingCompaction>,
    /// What the whole session cost so far.
    pub spend_usd: f64,
}

/// A compaction after the main conversation's latest request.
#[derive(Clone, Debug)]
pub struct TrailingCompaction {
    pub at_ms: i64,
    pub kind: CompactionTrigger,
    /// The estimate of the next prompt.
    pub estimate_tokens: f64,
    /// The next request at today's price, when the model has one.
    pub next_usd: Option<f64>,
}

/// What the latest prompt holds.
#[derive(Clone, Debug)]
pub struct Composition {
    pub total_tokens: f64,
    pub turns: u32,
    pub as_of_ms: i64,
    /// The time of a compaction after it.
    pub before_compaction_ms: Option<i64>,
    /// Whether the log shows what the prompt holds.
    pub known: bool,
    pub by_kind: Vec<(BlockKind, f64)>,
    /// The largest blocks, largest first.
    pub largest: Vec<LargeBlock>,
}

#[derive(Clone, Debug)]
pub struct LargeBlock {
    pub tokens: f64,
    pub kind: BlockKind,
    pub tool: Option<String>,
    pub target: Option<String>,
    pub turn: u32,
}

/// The X-ray of a session as of `now_ms`, with its subagents: idle time, cache state and
/// next-request costs at that time. It fails when a log of the session cannot be read.
pub fn xray(catalog: &Catalog, file: &SessionRef, pricing: &Pricing, now_ms: i64) -> Result<XRay> {
    let mut files = vec![file.clone()];
    files.extend(catalog.subagents(file));
    let sessions: Vec<Session> = files.par_iter().map(|f| catalog.parse(f)).collect::<Result<_>>()?;
    let mut totals = Totals::default();
    for s in &sessions {
        analysis::analyze(s, pricing, &Filter::default(), &mut totals);
    }
    let main = &sessions[0];
    let live = analysis::live_context(main);
    let ts: Vec<i64> = sessions.iter().flat_map(|s| s.chains.iter().flat_map(|c| c.requests.iter().map(|r| r.ts_ms))).filter(|&t| t > 0).collect();
    let span_ms = ts.iter().copied().min().zip(ts.iter().copied().max());
    let trailing = live.as_ref().and_then(|l| trailing_compaction(main, l, pricing, now_ms));
    let spend_usd = totals.components.iter().map(|v| v.usd).sum();
    let mut context = None;
    let mut advice = Vec::new();
    if let Some(live) = &live {
        // After a trailing compaction, the advice sees the prompt the next request will send.
        let compacted = trailing.as_ref().map(|t| compacted_live(main, live, t));
        let shown = compacted.as_ref().unwrap_or(live);
        let cache = cache_state(main, shown, pricing, now_ms);
        // The instruction files of every agent tokprof knows, in registry order.
        let instruction_files: Vec<String> = catalog.agents().iter().flat_map(|a| a.info().instruction_files.clone()).collect();
        advice = advice_of(main, shown, &cache, &totals, pricing, now_ms, &instruction_files);
        context = Some(context_of(live, cache, trailing.as_ref(), spend_usd, pricing, now_ms));
    }
    let setup = setup_of(main, &sessions, &totals, spend_usd);
    Ok(XRay {
        agent: main.agent.clone(),
        project: main.project.clone(),
        id: main.id.clone(),
        short_id: main.short_id().to_string(),
        title: main.title.clone(),
        models: totals.models.iter().map(|(m, n)| (m.clone(), *n)).collect(),
        clients: {
            let mut clients: Vec<(String, u64)> = totals.clients.iter().map(|(c, n)| (c.clone(), *n)).collect();
            clients.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
            clients
        },
        span_ms,
        requests: totals.requests,
        subagents: files.len() - 1,
        context,
        advice,
        priced: totals.priced,
        components: totals.components,
        chains: totals.chains.iter().map(|(n, v)| (n.clone(), *v)).collect(),
        composition: live.as_ref().map(|l| composition_of(main, l, trailing.as_ref())),
        content: totals.content.into_iter().collect(),
        calls: call_counts(&sessions),
        prompts: prompts_of(&main.typed, &totals.rows),
        tools: totals.tools,
        setup,
        misses: totals.misses,
        work: totals.work,
        points: points_of(&totals.rows),
        rhythm: crate::rhythm::SessionRhythm::of(main, now_ms, &crate::rhythm::local_offset),
        busy: crate::busy::Busy::of(main, &sessions[1..]),
    })
}

/// The main conversation's requests that have a time, in time order.
fn points_of(rows: &[RequestRow]) -> Vec<RequestPoint> {
    let mut points: Vec<RequestPoint> =
        rows.iter().filter(|r| r.main && r.ts_ms > 0).map(|r| RequestPoint { ts_ms: r.ts_ms, prompt_tokens: r.prompt, usd: r.usd }).collect();
    points.sort_by_key(|p| p.ts_ms);
    points
}

/// Tool calls by tool and target: the calls in the messages the requests produced, so each call
/// counts once however many prompts held it.
fn call_counts(sessions: &[Session]) -> HashMap<(Option<String>, Option<String>), u64> {
    let mut out = HashMap::new();
    for s in sessions {
        for m in s.chains.iter().flat_map(|c| &c.requests).filter_map(|r| r.produced) {
            for b in s.blocks[s.messages[m].blocks.clone()].iter().filter(|b| b.kind == BlockKind::ToolUse) {
                *out.entry((b.tool.clone(), b.target.clone())).or_default() += 1;
            }
        }
    }
    out
}

/// What each typed message cost. A main request counts with the latest message at or before it,
/// a request before the first message with the first; every other chain counts, whole, with the
/// message before its first request.
fn prompts_of(typed: &[TypedMessage], rows: &[RequestRow]) -> Vec<PromptSpend> {
    if typed.len() < 2 {
        return Vec::new();
    }
    let at = |ts: i64| typed.partition_point(|m| m.ts_ms <= ts).saturating_sub(1);
    let mut first: HashMap<(usize, &str), i64> = HashMap::new();
    for r in rows.iter().filter(|r| !r.main) {
        let t = first.entry((r.file, r.chain.as_str())).or_insert(r.ts_ms);
        *t = (*t).min(r.ts_ms);
    }
    let mut out: Vec<PromptSpend> =
        typed.iter().enumerate().map(|(i, m)| PromptSpend { number: i + 1, label: m.label.clone(), spend: Spend::default(), activities: Vec::new() }).collect();
    let mut by: Vec<HashMap<&'static str, Spend>> = vec![HashMap::new(); typed.len()];
    for r in rows {
        let i = if r.main { at(r.ts_ms) } else { at(first[&(r.file, r.chain.as_str())]) };
        let s = Spend { requests: 1, tokens: r.tokens, usd: r.usd };
        out[i].spend.add(s);
        by[i].entry(r.activity).or_default().add(s);
    }
    for (p, acts) in out.iter_mut().zip(by) {
        let mut acts: Vec<(&'static str, Spend)> = acts.into_iter().collect();
        acts.sort_by(|a, b| b.1.usd.total_cmp(&a.1.usd).then(b.1.tokens.total_cmp(&a.1.tokens)).then(a.0.cmp(b.0)));
        p.activities = acts;
    }
    out
}

fn context_of(live: &LiveContext, cache: CacheState, trailing: Option<&Trailing>, spend_usd: f64, pricing: &Pricing, now_ms: i64) -> ContextNow {
    // After a trailing compaction, the context is the estimate of the next prompt.
    let (size_tokens, approx) = trailing.map_or((live.usage.prompt(), false), |t| (t.estimate as u64, true));
    ContextNow {
        size_tokens,
        approx,
        window_tokens: live.context_window.or_else(|| pricing.context_window(&live.model, now_ms)),
        cache,
        trailing: trailing.map(|t| TrailingCompaction { at_ms: t.at_ms, kind: t.kind.clone(), estimate_tokens: t.estimate, next_usd: t.next_usd }),
        spend_usd,
    }
}

fn composition_of(s: &Session, live: &LiveContext, trailing: Option<&Trailing>) -> Composition {
    let mut by_kind: BTreeMap<BlockKind, f64> = BTreeMap::new();
    for (b, t, _) in &live.blocks {
        *by_kind.entry(b.map_or(BlockKind::Unknown, |b| s.blocks[b].kind)).or_default() += t;
    }
    let mut largest: Vec<&(Option<usize>, f64, u32)> = live.blocks.iter().filter(|(b, _, _)| b.is_some()).collect();
    largest.sort_by(|a, b| b.1.total_cmp(&a.1));
    Composition {
        total_tokens: live.blocks.iter().map(|(_, t, _)| t).sum(),
        turns: live.turns,
        as_of_ms: live.ts_ms,
        before_compaction_ms: trailing.map(|t| t.at_ms),
        known: live.blocks.iter().any(|(b, _, _)| b.is_some()),
        by_kind: by_kind.into_iter().collect(),
        largest: largest
            .into_iter()
            .map(|(b, t, turn)| {
                let blk = &s.blocks[b.expect("filtered")];
                LargeBlock { tokens: *t, kind: blk.kind, tool: blk.tool.clone(), target: blk.target.clone(), turn: *turn }
            })
            .collect(),
    }
}

#[allow(clippy::too_many_arguments)]
fn advice_of(s: &Session, live: &LiveContext, cache: &CacheState, t: &Totals, pricing: &Pricing, now_ms: i64, instruction_files: &[String]) -> Vec<Advice> {
    let requests = conversation(s, live);
    let billed: Vec<_> = requests.iter().filter(|r| r.usage.prompt() > 0 && !r.advisor).collect();
    let mut efforts: BTreeMap<&str, usize> = BTreeMap::new();
    for r in &billed {
        if let Some(e) = r.params.as_ref().and_then(|p| p.effort.as_deref()) {
            *efforts.entry(e).or_default() += 1;
        }
    }
    let mut misses: BTreeMap<Cause, (usize, f64)> = BTreeMap::new();
    for m in t.misses.iter().filter(|m| !m.cause.expected()) {
        let e = misses.entry(m.cause).or_default();
        e.0 += 1;
        e.1 += m.premium_usd.unwrap_or(0.0);
    }
    let price_at = |prompt| pricing.get(&live.model, prompt, now_ms);
    let inputs = advice::Inputs {
        instruction_files,
        session: s,
        live,
        price_at: &price_at,
        cache,
        window: live.context_window.or_else(|| pricing.context_window(&live.model, now_ms)),
        requests_per_prompt: live.requests_per_prompt,
        spend_usd: t.components.iter().map(|v| v.usd).sum(),
        thinking_usd: t.content.iter().filter(|((k, _, _), _)| *k == BlockKind::Thinking).map(|(_, v)| v.usd).sum(),
        lookups: t.work.by_trigger.get(&Trigger::Lookup).copied().unwrap_or_default(),
        effort: efforts.iter().max_by_key(|(_, n)| **n).map(|(e, _)| e.to_string()),
        misses: misses.into_iter().map(|(c, (n, u))| (c, n, u)).collect(),
        after_compaction: billed.iter().rev().find(|r| r.compacted_before).map(|r| r.usage.prompt()),
        first_prompt: billed.first().map(|r| r.usage.prompt()),
    };
    advice::advise(&inputs)
}

/// A compaction logged after the conversation's latest request, and the prompt the next request
/// will send instead of the compacted one.
struct Trailing {
    at_ms: i64,
    /// `idle`, the logged trigger, or `None` (see `Compaction`).
    kind: CompactionTrigger,
    /// The prompt of the conversation's first request after its previous compaction, or the
    /// setup plus 20,000 tokens.
    estimate: f64,
    /// Tool schemas and system prompt of the latest prompt, which the next request reads.
    setup: f64,
    /// The next request at today's price, when the model has one.
    next_usd: Option<f64>,
}

fn is_setup(s: &Session, block: Option<usize>) -> bool {
    block.is_some_and(|i| matches!(s.blocks[i].kind, BlockKind::Tools | BlockKind::System))
}

/// The compaction after the live prompt, if the session logged one.
fn trailing_compaction(s: &Session, live: &LiveContext, pricing: &Pricing, now_ms: i64) -> Option<Trailing> {
    let c = s.compactions.last().filter(|c| c.ts_ms > live.ts_ms)?;
    let setup: f64 = live.blocks.iter().filter(|(b, _, _)| is_setup(s, *b)).map(|(_, t, _)| t).sum();
    let requests = conversation(s, live);
    let after = requests.iter().rev().find(|r| r.compacted_before && r.usage.prompt() > 0 && !r.advisor).map(|r| r.usage.prompt() as f64);
    let estimate = after.unwrap_or(setup + 2.0 * crate::forecast::SUMMARY_TOKENS);
    let (added, output) = pace(&requests);
    let next_usd = pricing.get(&live.model, estimate as u64, now_ms).map(|p| next_after_compaction(estimate, setup, added, output, p, long_ttl(&requests)));
    Some(Trailing { at_ms: c.ts_ms, kind: c.kind.clone(), estimate, setup, next_usd })
}

/// The next request after a compaction: it reads the setup, writes the rest of the prompt and
/// what a request adds at the conversation's pace, and outputs as usual.
fn next_after_compaction(estimate: f64, setup: f64, added: f64, output: f64, p: Price, long_ttl: bool) -> f64 {
    let write = if long_ttl { p.write_long } else { p.write_short };
    (setup * p.read + ((estimate - setup).max(0.0) + added) * write + output * p.output) / 1e6
}

/// The live prompt as the advice should see it after a trailing compaction: the estimate's size,
/// with only the setup's blocks left, since the compaction replaced the rest.
fn compacted_live(s: &Session, live: &LiveContext, t: &Trailing) -> LiveContext {
    let setup = t.setup.min(t.estimate);
    LiveContext {
        chain: live.chain.clone(),
        ts_ms: live.ts_ms,
        model: live.model.clone(),
        usage: Usage { cache_read: setup as u64, write_long: (t.estimate - setup) as u64, ..Usage::default() },
        context_window: live.context_window,
        blocks: live.blocks.iter().filter(|(b, _, _)| is_setup(s, *b)).copied().collect(),
        turns: 0,
        requests_per_prompt: live.requests_per_prompt,
    }
}

/// A session as listed: its main file, subagent files and what the first lines tell.
#[derive(Clone)]
pub struct SessionRow {
    pub file: SessionRef,
    pub subagents: Vec<SessionRef>,
    pub project: String,
    /// The short id the session is shown by.
    pub id: String,
    /// The whole id.
    pub session_id: String,
    pub bytes: u64,
    /// When the session last did something, in epoch ms: the time of the last record of its
    /// main log that carries one, never later than the log's last write.
    pub active_ms: i64,
    /// The first client the main log names.
    pub client: Option<String>,
}

impl SessionRow {
    /// The row of the session whose main log this is.
    pub fn new(catalog: &Catalog, file: SessionRef) -> SessionRow {
        let head = catalog.head(&file);
        let mut subagents = catalog.subagents(&file);
        subagents.sort_by(|a, b| a.path().cmp(b.path()));
        let bytes = file.size + subagents.iter().map(|s| s.size).sum::<u64>();
        let id = crate::model::short_id(&file.agent, &head.session_id).to_string();
        let active_ms = catalog.last_active(&file);
        SessionRow { file, subagents, project: head.project, id, session_id: head.session_id, bytes, active_ms, client: head.client }
    }

    pub fn modified_ms(&self) -> i64 {
        self.file.modified_ms()
    }

    fn files(&self) -> impl Iterator<Item = &SessionRef> {
        std::iter::once(&self.file).chain(&self.subagents)
    }
}

/// Rows of the selected sessions, newest first by last activity.
pub fn rows(catalog: &Catalog, select: &Query, limit: usize) -> Vec<SessionRow> {
    let files: Vec<SessionRef> = main_sessions(catalog, &select.agents).into_iter().filter(|f| select.times(f)).collect();
    newest_rows(&files, limit, |batch| batch.par_iter().map(|f| SessionRow::new(catalog, f.clone())).filter(|r| select.row(r)).collect())
}

/// Newest first by last activity; rows active at the same moment, by their last write.
pub fn newest_first(a: &SessionRow, b: &SessionRow) -> std::cmp::Ordering {
    b.active_ms.cmp(&a.active_ms).then(b.file.modified_ns.cmp(&a.file.modified_ns)).then_with(|| a.file.path().cmp(b.file.path()))
}

/// The rows of the `limit` sessions active last, newest first. `files` come newest written
/// first, and their rows are built in batches until no log left can be among them: a session's
/// last activity is never after its log's last write.
fn newest_rows(files: &[SessionRef], limit: usize, build: impl Fn(&[SessionRef]) -> Vec<SessionRow>) -> Vec<SessionRow> {
    let mut rows = Vec::new();
    if limit == 0 {
        return rows;
    }
    let batch = limit.clamp(16, 256);
    let mut at = 0;
    while at < files.len() {
        let end = (at + batch).min(files.len());
        rows.extend(build(&files[at..end]));
        at = end;
        if rows.len() >= limit && at < files.len() {
            rows.sort_by(newest_first);
            rows.truncate(limit);
            let next = &files[at];
            // A file system that keeps no modification time gives 0, which bounds nothing.
            if next.modified_ns > 0 && rows[limit - 1].active_ms > next.modified_ms() {
                break;
            }
        }
    }
    rows.sort_by(newest_first);
    rows.truncate(limit);
    rows
}

/// A session's cost with its subagents, and what its next request would cost.
#[derive(Clone, Debug, Default)]
pub struct Glance {
    pub cost: f64,
    pub next_warm: Option<f64>,
    pub next_cold: Option<f64>,
    /// When the main conversation's cache goes cold, in epoch ms.
    pub cold_at_ms: i64,
    /// When the main conversation sent its latest request, in epoch ms; 0 without one.
    pub last_ms: i64,
    /// The session's title, when its log names one.
    pub title: Option<String>,
}

impl Glance {
    /// The next request's cost at `now_ms`, and whether the cache is cold by then.
    pub fn next(&self, now_ms: i64) -> Option<(f64, bool)> {
        let cold = now_ms >= self.cold_at_ms;
        if cold { self.next_cold } else { self.next_warm }.map(|usd| (usd, cold))
    }
}

pub fn glance(catalog: &Catalog, row: &SessionRow, pricing: &Pricing, now_ms: i64) -> Glance {
    let mut t = Totals::default();
    let mut main = None;
    for (k, file) in row.files().enumerate() {
        if let Ok(s) = catalog.parse(file) {
            analysis::analyze(&s, pricing, &Filter::default(), &mut t);
            if k == 0 {
                main = Some(s);
            }
        }
    }
    let mut g = Glance { cost: t.components.iter().map(|v| v.usd).sum(), cold_at_ms: i64::MAX, ..Glance::default() };
    if let Some((s, live)) = main.as_ref().and_then(|s| analysis::live_context(s).map(|l| (s, l))) {
        let cache = cache_state(s, &live, pricing, now_ms);
        g.next_warm = cache.next_warm_usd;
        g.next_cold = cache.next_cold_usd;
        g.cold_at_ms = live.ts_ms + cache.ttl_ms;
        g.last_ms = live.ts_ms;
    }
    g.title = main.and_then(|s| s.title);
    g
}

/// The glances of these rows, in parallel.
pub fn glances(catalog: &Catalog, rows: &[SessionRow], pricing: &Pricing, now_ms: i64) -> Vec<Glance> {
    rows.par_iter().map(|r| glance(catalog, r, pricing, now_ms)).collect()
}

/// Writes a session's profile to `path` (.jfr or .html), the same one `tokprof flame --session`
/// writes; returns what was written.
pub fn export(catalog: &Catalog, row: &SessionRow, pricing: &Pricing, path: &Path) -> Result<String> {
    use crate::export::{self, HtmlOptions};
    use crate::profile::Measure;
    use crate::report::{Collector, DEFAULT_STACK, DEFAULT_THREADS, dims};
    let jfr = export::has_ext(path, "jfr");
    let mut c = Collector::new(Some(dims(DEFAULT_STACK)), None);
    if jfr {
        c.profile.threads = Some(dims(DEFAULT_THREADS));
    }
    analysis::analyze(&catalog.parse(&row.file)?, pricing, &Filter::default(), &mut c);
    // A subagent log that cannot be read is left out, as `tokprof flame` leaves it out.
    let mut unreadable = 0u64;
    for file in &row.subagents {
        match catalog.parse(file) {
            Ok(s) => analysis::analyze(&s, pricing, &Filter::default(), &mut c),
            Err(_) => unreadable += 1,
        }
    }
    let left_out = if unreadable == 0 { String::new() } else { format!(", {} left out", count(unreadable, "unreadable log", "unreadable logs")) };
    let shown = display_path(&path.to_string_lossy(), None, catalog.home());
    if jfr {
        let w = crate::jfr::write(path, &c.profile.timed, Measure::Cost)?;
        Ok(format!("{} (1 = ${}) to {shown}{left_out}", count(w.samples, "sample", "samples"), w.quantum as f64 / 1e6))
    } else {
        let title = format!("tokprof: {} {} {}", row.file.agent.name(), row.project, row.id);
        let n = export::write(path, &c.profile.stacks, Measure::Cost, &HtmlOptions { title: &title, highlight: None })?;
        Ok(format!("{} to {shown}{left_out}", count(n as u64, "frame", "frames")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(file: usize, chain: &str, main: bool, ts_ms: i64, usd: f64, activity: &'static str) -> RequestRow {
        RequestRow { file, chain: chain.into(), main, ts_ms, usd, tokens: 1.0, prompt: ts_ms as u64 * 10, activity }
    }

    #[test]
    fn the_points_are_the_main_requests_with_a_time_in_time_order() {
        let rows = [
            row(1, "main", true, 20, 2.0, "reading"),
            row(2, "subagent: survey", false, 15, 4.0, "web"),
            row(1, "main", true, 5, 1.0, "prompt"),
            row(1, "main", true, 0, 9.0, "prompt"),
        ];
        let points = points_of(&rows);
        assert_eq!(points, [RequestPoint { ts_ms: 5, prompt_tokens: 50, usd: 1.0 }, RequestPoint { ts_ms: 20, prompt_tokens: 200, usd: 2.0 }]);
    }

    #[test]
    fn each_typed_message_gets_its_requests_and_subagents() {
        let typed: Vec<TypedMessage> =
            [(10, "first"), (100, "second"), (200, "third")].iter().map(|&(ts_ms, l)| TypedMessage { ts_ms, label: l.into() }).collect();
        let rows = [
            row(1, "main", true, 5, 1.0, "prompt"),
            row(1, "main", true, 20, 2.0, "reading"),
            row(1, "main", true, 120, 3.0, "editing"),
            // A subagent started after the second message runs on past the third.
            row(2, "subagent: survey", false, 150, 4.0, "web"),
            row(2, "subagent: survey", false, 250, 5.0, "web"),
            row(1, "main", true, 210, 0.5, "prompt"),
        ];
        let p = prompts_of(&typed, &rows);
        let costs: Vec<f64> = p.iter().map(|p| p.spend.usd).collect();
        assert_eq!(costs, [3.0, 12.0, 0.5], "a request before the first message counts with the first");
        assert_eq!(p[1].activities[0], ("web", Spend { requests: 2, tokens: 2.0, usd: 9.0 }));
        assert_eq!((p[1].number, p[1].spend.requests), (2, 3));
        assert!(prompts_of(&typed[..1], &rows).is_empty(), "one message gives no block");
    }

    fn log(agent: AgentId, path: &str) -> SessionRef {
        SessionRef {
            source: "test",
            agent,
            locator: crate::source::Locator::File(path.into()),
            session_id: String::new(),
            main_id: String::new(),
            subagent: false,
            size: 0,
            modified_ns: 0,
            created_ms: None,
        }
    }
    /// The row of session `n`, its log last written at `written` ms, the session last active at
    /// `active` ms.
    fn listed(n: usize, written: i64, active: i64) -> SessionRow {
        let mut file = log(crate::testing::codex(), &format!("/r/{n:03}.jsonl"));
        file.modified_ns = written as i128 * 1_000_000;
        SessionRow {
            file,
            subagents: Vec::new(),
            project: String::new(),
            id: n.to_string(),
            session_id: n.to_string(),
            bytes: 0,
            active_ms: active,
            client: None,
        }
    }

    /// `newest_rows` over these rows, with their logs in last-write order as discovery gives them,
    /// and how many rows it built.
    fn newest(rows: &[SessionRow], limit: usize) -> (Vec<usize>, usize) {
        let mut files: Vec<SessionRef> = rows.iter().map(|r| r.file.clone()).collect();
        files.sort_by_key(|f| std::cmp::Reverse(f.modified_ns));
        let built = std::cell::Cell::new(0);
        let out = newest_rows(&files, limit, |batch| {
            built.set(built.get() + batch.len());
            batch.iter().map(|f| rows.iter().find(|r| r.file.path() == f.path()).unwrap().clone()).collect()
        });
        (out.iter().map(|r| r.id.parse().unwrap()).collect(), built.get())
    }

    #[test]
    fn the_list_is_newest_first_by_last_activity() {
        let hour = 60 * MIN;
        // Written last by bookkeeping on a resume, idle since 09:27; another session worked at 16:00.
        let rows = [listed(0, 16 * hour + MIN, 9 * hour + 27 * MIN), listed(1, 16 * hour, 16 * hour), listed(2, 12 * hour, 12 * hour)];
        assert_eq!(newest(&rows, 3).0, [1, 2, 0], "an idle session written last ranks below the ones active after it");

        // Stopping early gives the rows that sorting every row gives.
        let mut seed = 7u64;
        let mut next = move |n: i64| {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            (seed >> 33) as i64 % n
        };
        let rows: Vec<SessionRow> = (0..300)
            .map(|n| {
                let written = 1_000 * hour + next(500 * hour);
                // Half are idle sessions that a resume wrote to long after their last activity.
                let active = if next(2) == 0 { written - next(100 * hour) } else { written - next(MIN) };
                listed(n, written, active)
            })
            .collect();
        let mut all = rows.clone();
        all.sort_by(newest_first);
        for limit in [1, 2, 5, 20, 50, 299, 300, 1_000, usize::MAX] {
            let (got, built) = newest(&rows, limit);
            let want: Vec<usize> = all.iter().take(limit).map(|r| r.id.parse().unwrap()).collect();
            assert_eq!(got, want, "limit {limit}");
            if limit <= 20 {
                assert!(built < 300, "limit {limit} built every row");
            }
        }
        assert!(newest(&rows, 0).0.is_empty());
    }

    use crate::model::Request;
    use crate::model::{Block, Chain, ChainKind, Compaction, Usage};

    const MIN: i64 = 60_000;

    #[test]
    fn session_ids_match_from_their_start_in_any_case() {
        let id = "01a197e1-4d9f-76b1-b25d-cdd77b01e10f";
        assert!(id_starts_with(id, "01a197e1") && id_starts_with(id, "01A197E1-4D9F") && id_starts_with(id, id));
        assert!(!id_starts_with(id, "4d9f") && !id_starts_with(id, "2026") && !id_starts_with(id, &format!("{id}0")));
        assert_eq!(crate::model::short_id(&crate::testing::codex(), id), "01a197e1-4d9f-76b1");
        // Launched in the same millisecond: the random third group keeps them apart.
        assert_eq!(crate::model::short_id(&crate::testing::codex(), "01a1978f-f718-7312-87f0-032deca644fd"), "01a1978f-f718-7312");
        assert_eq!(crate::model::short_id(&crate::testing::codex(), "01a1978f-f718-7691-b255-4f2e7d2af8e2"), "01a1978f-f718-7691");
        assert_eq!(crate::model::short_id(&crate::testing::claude(), "3f951de0-14bc-4966-880b-81f649966ccf"), "3f951de0");
    }

    #[test]
    fn a_shared_prefix_lists_the_sessions() {
        let row = |id: &str| SessionRow {
            file: log(crate::testing::codex(), &format!("/r/rollout-{id}.jsonl")),
            subagents: Vec::new(),
            project: "~/src/p".into(),
            id: crate::model::short_id(&crate::testing::codex(), id).to_string(),
            session_id: id.into(),
            bytes: 0,
            active_ms: 0,
            client: None,
        };
        let rows: Vec<SessionRow> = (0..12).map(|i| row(&format!("01a197e1-4d{i:02}-76b1-b25d-cdd77b01e10f"))).collect();
        let text = ambiguous("01a197e1", &rows);
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines[0], "\"01a197e1\" matches 12 sessions; give more of the id:");
        assert!(lines[1].starts_with("  01a197e1-4d00-76b1  ") && lines[1].ends_with("codex   ~/src/p"), "{}", lines[1]);
        assert_eq!((lines.len(), lines[11]), (12, "  … and 2 more"));

        let select = Query { project: Some("src/p".into()), session: Some("01A197E1-4D05".into()), ..Query::default() };
        assert_eq!(rows.iter().filter(|r| select.row(r)).count(), 1);
    }

    /// da18a2d8 on 2026-10-29: a 440.1k-token prompt at 18:18, 40% of it tool results, then a
    /// compaction at 19:13 with no request after it. With `earlier`, its previous compaction was
    /// followed by a 55.8k-token prompt.
    fn compacted_session(kind: &str, earlier: bool) -> (Session, LiveContext) {
        let mut s = Session::new(crate::testing::claude(), "s".into());
        let kinds = [(BlockKind::Tools, 10_000.0), (BlockKind::System, 4_400.0), (BlockKind::ToolResult, 176_040.0), (BlockKind::Text, 249_660.0)];
        s.blocks = kinds.iter().map(|&(kind, _)| Block { class: crate::model::Class::None, kind, chars: 1, tool: None, target: None }).collect();
        let req = |ts_ms, write_long, compacted_before| Request {
            ts_ms,
            api: crate::api::ApiId::Anthropic,
            model: "claude-opus-5-5".into(),
            usage: Usage { cache_read: 1_000, write_long, output: 1_000, ..Usage::default() },
            compacted_before,
            ..Request::default()
        };
        let mut requests = vec![req(0, 300_000, false)];
        if earlier {
            requests.push(req(10 * MIN, 54_800, true));
        }
        requests.push(req(60 * MIN, 439_100, false));
        s.chains.push(Chain { name: "main".into(), kind: ChainKind::Main, requests });
        if earlier {
            s.compactions.push(Compaction { ts_ms: 5 * MIN, kind: CompactionTrigger::Auto });
        }
        s.compactions.push(Compaction { ts_ms: 115 * MIN, kind: if kind == "idle" { CompactionTrigger::Idle } else { CompactionTrigger::logged(Some(kind)) } });
        let live = LiveContext {
            chain: "main".into(),
            ts_ms: 60 * MIN,
            model: "claude-opus-5-5".into(),
            usage: Usage { cache_read: 1_000, write_long: 439_100, ..Usage::default() },
            context_window: Some(1_000_000),
            blocks: kinds.iter().enumerate().map(|(n, &(_, t))| (Some(n), t, 1)).collect(),
            turns: 30,
            requests_per_prompt: 3.0,
        };
        (s, live)
    }

    #[test]
    fn the_setup_total_is_the_setup_content() {
        use crate::model::{Class, PromptDelta, ReminderKind, setup_source};
        let class = |kind, target: Option<&str>| match kind {
            BlockKind::Reminder => Class::Reminder(ReminderKind::of_label(target.unwrap_or_default())),
            _ => Class::None,
        };
        let block = |kind, target: Option<&str>, chars| Block { class: class(kind, target), kind, chars, tool: None, target: target.map(str::to_string) };
        let mut s = Session::new(crate::testing::claude(), "s".into());
        let first = s.push_message(
            vec![
                block(BlockKind::Reminder, Some("instructions"), 4_000),
                block(BlockKind::Reminder, Some("skills"), 3_000),
                block(BlockKind::Prompt, None, 500),
            ],
            true,
        );
        let notice = s.push_message(vec![block(BlockKind::Reminder, Some("background notifications"), 2_000)], false);
        let req = |ts_ms, keep, tail| Request {
            ts_ms,
            model: "claude-opus-5-5".into(),
            usage: Usage { cache_read: 2_000, write_long: 7_500, output: 100, ..Usage::default() },
            prompt: Some(PromptDelta { shape: None, keep, tail }),
            ..Request::default()
        };
        s.chains.push(Chain { name: "main".into(), kind: ChainKind::Main, requests: vec![req(1, 0, vec![first]), req(2, 1, vec![notice])] });

        let mut totals = Totals::default();
        analysis::analyze(&s, &Pricing::defaults(), &Filter::default(), &mut totals);
        let in_content: f64 = totals
            .content
            .iter()
            .filter(|((kind, _, target), _)| setup_source(*kind, class(*kind, target.as_deref()), target.as_deref()).is_some())
            .map(|(_, v)| v.usd)
            .sum();
        assert!(in_content > 0.0);
        assert!((totals.setup.total(&crate::testing::claude()).all.usd - in_content).abs() < 1e-9);
    }

    #[test]
    fn a_trailing_compaction_sets_the_next_prompt() {
        let pricing = Pricing::defaults();
        let (s, live) = compacted_session("idle", true);
        let t = trailing_compaction(&s, &live, &pricing, crate::TEST_NOW_MS).expect("a trailing compaction");
        assert_eq!((t.at_ms, t.kind.name(), t.estimate, t.setup), (115 * MIN, Some("idle"), 55_800.0, 14_400.0));
        // Without an earlier compaction: the setup plus 20,000 tokens.
        let (s, live) = compacted_session("idle", false);
        assert_eq!(trailing_compaction(&s, &live, &pricing, crate::TEST_NOW_MS).unwrap().estimate, 34_400.0);
        // A compaction before the latest request is not trailing.
        let (mut s, live) = compacted_session("idle", true);
        s.compactions.pop();
        assert!(trailing_compaction(&s, &live, &pricing, crate::TEST_NOW_MS).is_none());
    }

    #[test]
    fn the_forecast_after_a_compaction_reads_the_setup_and_writes_the_rest() {
        let p = Pricing::defaults().get("claude-opus-5-5", 55_800, crate::TEST_NOW_MS).unwrap();
        // (14,400 × $0.20 + 43,400 × $8 + 1,000 × $20) per million.
        assert!((next_after_compaction(55_800.0, 14_400.0, 2_000.0, 1_000.0, p, true) - 0.370_08).abs() < 1e-9);
    }

    #[test]
    fn the_x_ray_names_a_trailing_compaction() {
        let pricing = Pricing::defaults();
        for kind in ["idle", "auto"] {
            let (s, live) = compacted_session(kind, true);
            let t = trailing_compaction(&s, &live, &pricing, crate::TEST_NOW_MS).unwrap();
            let shown = compacted_live(&s, &live, &t);
            let cache = cache_state(&s, &shown, &pricing, crate::TEST_NOW_MS);
            let c = context_of(&live, cache, Some(&t), 414.0, &pricing, crate::TEST_NOW_MS);
            assert_eq!((c.size_tokens, c.approx, c.window_tokens), (55_800, true, Some(1_000_000)));
            let trailing = c.trailing.expect("the compaction");
            assert_eq!((trailing.at_ms, trailing.kind.name(), trailing.estimate_tokens), (115 * MIN, Some(kind), 55_800.0));
            assert!(trailing.next_usd.is_some());
            assert_eq!(composition_of(&s, &live, Some(&t)).before_compaction_ms, Some(115 * MIN));
        }
    }

    #[test]
    fn advice_after_a_compaction_sees_the_next_prompt() {
        let pricing = Pricing::defaults();
        let (s, live) = compacted_session("idle", true);
        let t = trailing_compaction(&s, &live, &pricing, crate::TEST_NOW_MS).unwrap();
        let advise = |live: &LiveContext| {
            let cache = cache_state(&s, live, &pricing, crate::TEST_NOW_MS);
            let price_at = |prompt| pricing.get(&live.model, prompt, crate::TEST_NOW_MS);
            advice::advise(&advice::Inputs {
                instruction_files: crate::testing::instruction_files(),
                session: &s,
                live,
                price_at: &price_at,
                cache: &cache,
                window: live.context_window,
                requests_per_prompt: 3.0,
                spend_usd: 414.0,
                thinking_usd: 100.0,
                lookups: crate::report::TriggerSpend::default(),
                effort: None,
                misses: vec![(Cause::HistoryEdited, 44, 154.0)],
                after_compaction: Some(55_800),
                first_prompt: Some(301_000),
            })
        };
        let fires = |lines: &[crate::advice::Advice], start: &str| lines.iter().any(|l| l.text.starts_with(start));
        // The compacted prompt: carry-on, context cost and the tool-results hint fire.
        let before = advise(&live);
        assert!(fires(&before, "Resuming writes") && fires(&before, "Tool results"), "{before:?}");
        let after = advise(&compacted_live(&s, &live, &t));
        assert!(!fires(&after, "Resuming writes") && !fires(&after, "Cache warm") && !fires(&after, "Tool results"), "{after:?}");
        assert!(!after.iter().any(|l| l.text.contains("of the window") || l.text.contains("re-reads")), "{after:?}");
        assert!(fires(&after, "Thinking cost") && fires(&after, "Cache misses"), "{after:?}");
    }
}
