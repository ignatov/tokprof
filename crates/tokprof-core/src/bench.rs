//! `tokprof bench`: what it costs to come back to a big session after its prompt cache
//! expired, and what the same work costs after `/compact`, in a new session that refers to
//! the old one, or with heartbeats that keep the cache warm.
//!
//! Every cold return in the logs is a measured case. It is a request of a main conversation
//! with a context of at least `min_context` tokens that came after an idle gap longer than the
//! cache lifetime and had to write that context again. The bench takes it and the requests
//! that followed, up to `requests` of them and until the next such return or a compaction, and
//! prices the same requests five ways:
//! - **resume as is**: as logged.
//! - **/compact first**: a compaction call, then the requests on top of what a compaction
//!   leaves. Claude Code does not log the call. It runs as a fork that shares the
//!   conversation's cache, so on a cold cache it writes the whole context again, and then it
//!   outputs a summary. Codex sends its call without the conversation's tools, so it reads
//!   nothing from the cache and writes the whole context, warm or cold.
//! - **new session + handoff note**: the old session writes a note while its cache is still
//!   warm, and a new session does the requests on top of the same base.
//! - **heartbeats, then resume**: while the session is idle, a ping at 5/6 of the cache
//!   lifetime reads the context, which resets the cache timer, for up to `keep_warm` after the
//!   last request. A return within the lifetime of the last ping reads its context from the
//!   cache and then goes on as logged. A later return is cold anyway, after the pings.
//! - **heartbeats, then compaction**: the same pings, and at the next ping time past `keep_warm`
//!   a compaction instead, while the cache is still warm. It reads the context where the
//!   agent's compaction call shares the conversation's cache, and writes it again elsewhere. A
//!   return before it is warm; a later one goes on from the base, as after /compact.
//!
//! An IDE that pinged would also ping through the breaks that were no cold return: those after
//! which the cache was still warm, and the end of each conversation. The bench prices those
//! breaks for the two heartbeat ways apart from the returns.
//!
//! The replay keeps each request's own cache behavior and changes only the old conversation
//! in its prompt. That span shrinks to the base, and its read, written and uncached parts
//! shrink with it. The base is the median prompt right after the agent's compactions in the
//! logs. It is what the agent reloads to carry on: the summary, the files it was working on and
//! the task list, on top of the tools and system prompt.

use std::collections::BTreeMap;

use crate::analysis::Filter;
use crate::analysis::cache;
use crate::forecast::{self, SUMMARY_TOKENS};
use crate::model::{Request, Session, Usage};
use crate::pricing::Pricing;
use crate::source::AgentId;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Strategy {
    Resume,
    Compact,
    Handoff,
    Heartbeats,
    WarmThenCompact,
}

impl Strategy {
    pub const ALL: [Strategy; 5] = [Strategy::Resume, Strategy::Compact, Strategy::Handoff, Strategy::Heartbeats, Strategy::WarmThenCompact];

    pub fn name(self) -> &'static str {
        match self {
            Strategy::Resume => "resume as is",
            Strategy::Compact => "/compact first",
            Strategy::Handoff => "new session + handoff note",
            Strategy::Heartbeats => "heartbeats, then resume",
            Strategy::WarmThenCompact => "heartbeats, then compaction",
        }
    }
}

/// The bench's defaults, which `tokprof bench` and the share file use: the requests followed
/// after a return, the smallest context of a return, and how long heartbeats keep the cache warm.
pub const DEFAULT_REQUESTS: usize = 60;
pub const DEFAULT_MIN_CONTEXT: u64 = 100_000;
pub const DEFAULT_KEEP_WARM_MS: i64 = 6 * 3_600_000;

/// Output tokens of one heartbeat: a short reply to the ping, which the conversation drops.
pub const PING_OUTPUT: f64 = 100.0;

/// The pings that keep a cache of lifetime `ttl_ms` warm through a break of `idle_ms`: one at
/// 5/6 of the lifetime after the last request or ping, while the break lasts and for at most
/// `window_ms`. Returns their times since the last request, and whether the return finds the
/// cache still warm.
fn heartbeats(idle_ms: i64, ttl_ms: i64, window_ms: i64) -> (Vec<i64>, bool) {
    let every = ping_every(ttl_ms);
    let times: Vec<i64> = (1..).map(|k| k * every).take_while(|&t| t < idle_ms && t <= window_ms).collect();
    let warm = idle_ms - times.last().copied().unwrap_or(0) <= ttl_ms;
    (times, warm)
}

/// When heartbeats then compaction compacts, after the last request: at the first ping time past
/// the window, which falls within the last ping's lifetime.
pub fn compaction_at(ttl_ms: i64, window_ms: i64) -> i64 {
    let every = ping_every(ttl_ms);
    (window_ms.max(0) / every + 1) * every
}

/// How often heartbeats go out for a cache of lifetime `ttl_ms`: every 50 minutes on a 1-hour
/// cache, as the cache-tax mod does.
pub fn ping_every(ttl_ms: i64) -> i64 {
    (ttl_ms * 5 / 6).max(1)
}

/// A heartbeat window as the report names it: 6h, 90m.
/// Whole hours as `6h`, anything else in minutes with up to one decimal: `90m`, `90.5m`, `0.5m`.
pub fn window_name(ms: i64) -> String {
    if ms % 3_600_000 == 0 {
        return format!("{}h", ms / 3_600_000);
    }
    let minutes = format!("{:.1}", ms as f64 / 60_000.0);
    format!("{}m", minutes.trim_end_matches(".0"))
}

/// A request as logged.
#[derive(Clone, Debug)]
struct Step {
    ts_ms: i64,
    model: String,
    usage: Usage,
}

/// A cold return found in a session, before it is priced.
#[derive(Debug)]
struct Case {
    agent: AgentId,
    /// The API of the return, whose cache lifetime the heartbeats keep up with.
    api: crate::api::ApiId,
    /// The last request before the break.
    before_ms: i64,
    idle_ms: i64,
    context: u64,
    long_ttl: bool,
    /// Tokens added and output per request before the break, as the X-ray's forecast sees them.
    pace: (f64, f64),
    /// The return and the requests after it.
    steps: Vec<Step>,
}

/// A break of a big main conversation that is no counted return: the heartbeat ways ping
/// through it all the same.
#[derive(Debug)]
struct Break {
    agent: AgentId,
    api: crate::api::ApiId,
    model: String,
    long_ttl: bool,
    /// The prompt before the break.
    context: u64,
    /// The last request before the break.
    start_ms: i64,
    /// The request after it; `None` for a conversation's last request, whose break lasts to the
    /// end of the logs.
    end_ms: Option<i64>,
}

/// What the sessions of a bench run hold: cold returns, the breaks that were none, and the
/// context sizes the alternatives start from.
#[derive(Default)]
pub struct Found {
    cases: Vec<Case>,
    breaks: Vec<Break>,
    /// Prompt size right after a compaction of a big context, per agent.
    compacted: Vec<(AgentId, u64)>,
    /// First prompt of each main conversation, per agent.
    starts: Vec<(AgentId, u64)>,
    /// The latest request read, where open breaks end, and `--until`, when earlier.
    latest_ms: Option<i64>,
    until_ms: Option<i64>,
}

impl Found {
    pub fn merge(mut self, other: Found) -> Found {
        self.cases.extend(other.cases);
        self.breaks.extend(other.breaks);
        self.compacted.extend(other.compacted);
        self.starts.extend(other.starts);
        self.latest_ms = self.latest_ms.max(other.latest_ms);
        self.until_ms = self.until_ms.or(other.until_ms);
        self
    }

    /// Where an open break ends: the latest request read, or `--until` when it is earlier.
    fn end_of_logs(&self) -> Option<i64> {
        match (self.latest_ms, self.until_ms) {
            (Some(latest), Some(until)) => Some(latest.min(until)),
            (latest, _) => latest,
        }
    }
}

/// The request came after the cache expired and wrote the previous prompt again.
fn cold_return(long_ttl: bool, prev: &Request, r: &Request) -> bool {
    r.api.ttl(long_ttl).is_some_and(|(ttl_ms, _)| r.ts_ms - prev.ts_ms > ttl_ms) && cache::rewritten(prev.usage.prompt(), &r.usage).is_some()
}

/// Finds the cold returns of a session's main conversations, and the breaks that were none.
pub fn scan(s: &Session, filter: &Filter, requests: usize, min_context: u64) -> Found {
    let mut found = Found { until_ms: filter.until_ms, ..Found::default() };
    for (_, conversation) in s.main_conversations() {
        let billed: Vec<usize> = (0..conversation.len()).filter(|&i| conversation[i].usage.prompt() > 0 && !conversation[i].advisor).collect();
        if let Some(&first) = billed.first() {
            found.starts.push((s.agent.clone(), conversation[first].usage.prompt()));
        }
        found.latest_ms = found.latest_ms.max(billed.last().map(|&i| conversation[i].ts_ms));
        // A break that is no counted return, after a big prompt the filters keep.
        let mut breaks: Vec<Break> = Vec::new();
        let mut no_return = |prev: &Request, end_ms: Option<i64>, long_ttl: bool| {
            let lasts_a_ping = end_ms.is_none_or(|end| prev.api.ttl(long_ttl).is_some_and(|(ttl_ms, _)| end - prev.ts_ms >= ping_every(ttl_ms)));
            if prev.usage.prompt() >= min_context && filter.keeps(prev) && lasts_a_ping {
                breaks.push(Break {
                    agent: s.agent.clone(),
                    api: prev.api,
                    model: prev.model.clone(),
                    long_ttl,
                    context: prev.usage.prompt(),
                    start_ms: prev.ts_ms,
                    end_ms,
                });
            }
        };
        if let Some(&last) = billed.last() {
            no_return(conversation[last], None, forecast::long_ttl(&conversation[..=last]));
        }
        for (k, pair) in billed.windows(2).enumerate() {
            let (prev, r) = (&conversation[pair[0]], &conversation[pair[1]]);
            let history = &conversation[..=pair[0]];
            let long_ttl = forecast::long_ttl(history);
            if r.compacted_before {
                if prev.usage.prompt() >= min_context {
                    found.compacted.push((s.agent.clone(), r.usage.prompt()));
                }
                no_return(prev, Some(r.ts_ms), long_ttl);
                continue;
            }
            if prev.usage.prompt() < min_context || r.model != prev.model || !filter.keeps(r) || !cold_return(long_ttl, prev, r) {
                no_return(prev, Some(r.ts_ms), long_ttl);
                continue;
            }
            let mut steps = vec![r];
            let mut lifetime = long_ttl;
            for &i in &billed[k + 2..] {
                let (last, x) = (steps[steps.len() - 1], &conversation[i]);
                if last.usage.cache_write() > 0 {
                    lifetime = last.usage.write_long > last.usage.write_short;
                }
                if steps.len() >= requests || x.compacted_before || cold_return(lifetime, last, x) {
                    break;
                }
                steps.push(x);
            }
            found.cases.push(Case {
                agent: s.agent.clone(),
                api: r.api,
                before_ms: prev.ts_ms,
                idle_ms: r.ts_ms - prev.ts_ms,
                context: prev.usage.prompt(),
                long_ttl,
                pace: forecast::pace(history),
                steps: steps.into_iter().map(|x| Step { ts_ms: x.ts_ms, model: x.model.clone(), usage: x.usage }).collect(),
            });
        }
        found.breaks.extend(breaks);
    }
    found
}

/// One measured return, priced five ways.
pub struct Return {
    pub agent: AgentId,
    /// The API of the return.
    pub api: crate::api::ApiId,
    pub context: u64,
    pub idle_ms: i64,
    pub long_ttl: bool,
    /// Cost after each request since the return, per strategy. `[0]` is what the strategy
    /// spends before the first request: the compaction call, the note, or the pings.
    pub cumulative: [Vec<f64>; 5],
    /// The X-ray's forecast of the first request once the cache is cold.
    pub forecast: f64,
    /// How many heartbeats went out during the break, and whether they kept the cache warm
    /// until the return.
    pub pings: usize,
    pub kept_warm: bool,
    /// Whether heartbeats then compaction compacted before the return.
    pub compacted: bool,
}

impl Return {
    pub fn requests(&self) -> usize {
        self.cumulative[0].len() - 1
    }

    /// Cost of the first `k` requests, or of all of them if there were fewer.
    pub fn after(&self, s: Strategy, k: usize) -> f64 {
        let c = &self.cumulative[s as usize];
        c[k.min(c.len() - 1)]
    }
}

/// The context the alternatives start from.
#[derive(Clone, Copy, Debug)]
pub struct Base {
    pub tokens: u64,
    /// How many compactions the median comes from; 0 when there were none, and the base is a
    /// first prompt plus a note.
    pub compactions: usize,
}

/// What the two heartbeat ways send during an agent's breaks that were no counted return: the
/// pings both send, and the compactions heartbeats then compaction adds.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Elsewhere {
    pub pings: usize,
    pub ping_usd: f64,
    pub compactions: usize,
    pub compaction_usd: f64,
}

pub struct Bench {
    pub requests: usize,
    pub min_context: u64,
    /// How long heartbeats keep pinging after the last request.
    pub keep_warm_ms: i64,
    pub returns: Vec<Return>,
    pub base: BTreeMap<AgentId, Base>,
    /// Per agent, what the heartbeat ways send during breaks that were no counted return.
    pub elsewhere: BTreeMap<AgentId, Elsewhere>,
}

impl Bench {
    pub fn of(&self, agent: &AgentId) -> Vec<&Return> {
        self.returns.iter().filter(|r| &r.agent == agent).collect()
    }

    /// The agents with returns, by name.
    pub fn agents(&self) -> Vec<AgentId> {
        let mut agents: Vec<AgentId> = self.returns.iter().map(|r| r.agent.clone()).collect();
        agents.sort();
        agents.dedup();
        agents
    }
}

pub fn median(mut v: Vec<u64>) -> Option<u64> {
    v.sort_unstable();
    v.get(v.len() / 2).copied()
}

/// What a warm read of a `context`-token prompt and `output` tokens of reply cost on `model` at
/// a moment; `None` without a price.
fn warm_call(pricing: &Pricing, model: &str, context: u64, output: f64, at_ms: i64) -> Option<f64> {
    pricing.get(model, context, at_ms).map(|p| (context as f64 * p.read + output * p.output) / 1e6)
}

/// What a compaction of a `context`-token prompt on `model` costs at a moment while the cache is
/// warm: it reads the context where the agent's compaction call shares the conversation's cache,
/// and writes it again at the write price of the cache lifetime in use elsewhere; then it outputs
/// the summary. `None` without a price.
fn warm_compaction(pricing: &Pricing, agent: &AgentId, model: &str, context: u64, long_ttl: bool, at_ms: i64) -> Option<f64> {
    if agent.info().harness.compaction_reads_cache {
        return warm_call(pricing, model, context, SUMMARY_TOKENS, at_ms);
    }
    let p = pricing.get(model, context, at_ms)?;
    let write = if long_ttl { p.write_long } else { p.write_short };
    Some((context as f64 * write + SUMMARY_TOKENS * p.output) / 1e6)
}

/// What the two heartbeat ways send during the breaks that were no counted return, per agent.
fn price_elsewhere(found: &Found, pricing: &Pricing, keep_warm_ms: i64) -> BTreeMap<AgentId, Elsewhere> {
    let mut out: BTreeMap<AgentId, Elsewhere> = BTreeMap::new();
    let end_of_logs = found.end_of_logs();
    for b in &found.breaks {
        let Some(end) = b.end_ms.or(end_of_logs) else { continue };
        let Some((ttl_ms, _)) = b.api.ttl(b.long_ttl) else { continue };
        let idle = end - b.start_ms;
        let (pings, _) = heartbeats(idle, ttl_ms, keep_warm_ms);
        let e = out.entry(b.agent.clone()).or_default();
        for t in &pings {
            if let Some(usd) = warm_call(pricing, &b.model, b.context, PING_OUTPUT, b.start_ms + t) {
                e.pings += 1;
                e.ping_usd += usd;
            }
        }
        let at = compaction_at(ttl_ms, keep_warm_ms);
        if at < idle
            && let Some(usd) = warm_compaction(pricing, &b.agent, &b.model, b.context, b.long_ttl, b.start_ms + at)
        {
            e.compactions += 1;
            e.compaction_usd += usd;
        }
    }
    out
}

/// Prices every return five ways, and what the heartbeat ways send elsewhere.
pub fn replay(found: Found, pricing: &Pricing, requests: usize, min_context: u64, keep_warm_ms: i64) -> Bench {
    let mut base = BTreeMap::new();
    let mut agents: Vec<AgentId> = found.compacted.iter().chain(&found.starts).map(|(a, _)| a.clone()).collect();
    agents.sort();
    agents.dedup();
    for agent in agents {
        let compacted: Vec<u64> = found.compacted.iter().filter(|(a, _)| *a == agent).map(|(_, t)| *t).collect();
        let n = compacted.len();
        let tokens = match median(compacted) {
            Some(t) => Some(t),
            None => median(found.starts.iter().filter(|(a, _)| *a == agent).map(|(_, t)| *t).collect()).map(|t| t + SUMMARY_TOKENS as u64),
        };
        if let Some(tokens) = tokens {
            base.insert(agent, Base { tokens, compactions: n });
        }
    }
    let returns = found.cases.iter().filter_map(|c| price(c, base.get(&c.agent)?.tokens, pricing, keep_warm_ms)).collect();
    let elsewhere = price_elsewhere(&found, pricing, keep_warm_ms);
    Bench { requests, min_context, keep_warm_ms, returns, base, elsewhere }
}

fn price(c: &Case, base: u64, pricing: &Pricing, keep_warm_ms: i64) -> Option<Return> {
    let first = &c.steps[0];
    let p = pricing.get(&first.model, c.context, first.ts_ms)?;
    let note = pricing.get(&first.model, c.context, c.before_ms).unwrap_or(p);
    let ctx = c.context as f64;
    // The tools and system prompt other sessions keep warm: the return still reads them.
    let shared = (first.usage.cache_read as f64).min(base as f64).min(ctx);
    let keep = if ctx > shared { ((base as f64).min(ctx) - shared).max(0.0) / (ctx - shared) } else { 1.0 };
    let write = if c.long_ttl { p.write_long } else { p.write_short };
    // The compaction call of /compact first reads what the return still read from the cache when
    // it shares the conversation's cache; otherwise it writes the whole context.
    let call_read = if c.agent.info().harness.compaction_reads_cache { shared } else { 0.0 };
    // Each ping reads the context at the price of its own time.
    let ttl_ms = c.api.ttl(c.long_ttl).map_or(i64::MAX, |(ttl_ms, _)| ttl_ms);
    let (pings, kept_warm) = heartbeats(c.idle_ms, ttl_ms, keep_warm_ms);
    let pinged: f64 = pings.iter().filter_map(|t| warm_call(pricing, &first.model, c.context, PING_OUTPUT, c.before_ms + t)).sum();
    // Heartbeats then compaction: the same pings, then a compaction while the cache is warm.
    let compact_at = compaction_at(ttl_ms, keep_warm_ms);
    let compacted = compact_at < c.idle_ms;
    let compaction =
        if compacted { warm_compaction(pricing, &c.agent, &first.model, c.context, c.long_ttl, c.before_ms + compact_at).unwrap_or(0.0) } else { 0.0 };
    let up_front = [
        0.0,
        (call_read * p.read + (ctx - call_read) * write + SUMMARY_TOKENS * p.output) / 1e6,
        (ctx * note.read + SUMMARY_TOKENS * note.output) / 1e6,
        pinged,
        pinged + compaction,
    ];
    let cumulative = Strategy::ALL.map(|s| {
        // The ways that keep the whole context: resuming, heartbeats, and heartbeats then
        // compaction while it has not compacted.
        let whole = match s {
            Strategy::Resume | Strategy::Heartbeats => true,
            Strategy::WarmThenCompact => !compacted,
            Strategy::Compact | Strategy::Handoff => false,
        };
        let warm_first = match s {
            Strategy::Heartbeats => kept_warm,
            Strategy::WarmThenCompact => !compacted,
            _ => false,
        };
        let keep = if whole { 1.0 } else { keep };
        let mut total = up_front[s as usize];
        let mut out = vec![total];
        for (i, step) in c.steps.iter().enumerate() {
            let u = if warm_first && i == 0 { warmed(step.usage, c.context) } else { shrink(step.usage, shared, ctx, keep) };
            total += pricing.get(&step.model, u.prompt(), step.ts_ms).map_or(0.0, |p| p.cost(&u));
            out.push(total);
        }
        out
    });
    let (added, output) = c.pace;
    Some(Return {
        agent: c.agent.clone(),
        api: c.api,
        context: c.context,
        idle_ms: c.idle_ms,
        long_ttl: c.long_ttl,
        cumulative,
        forecast: forecast::next_request(ctx, added, output, p, c.long_ttl).1,
        pings: pings.len(),
        kept_warm,
        compacted,
    })
}

/// The usage of the first request back had the cache stayed warm: it reads the previous
/// prompt, `context` tokens, from the cache, and writes or sends uncached only what is new.
fn warmed(u: Usage, context: u64) -> Usage {
    let moved = context.min(u.prompt()).saturating_sub(u.cache_read);
    let written = moved.min(u.cache_write());
    let long = if u.cache_write() > 0 { (written as f64 * u.write_long as f64 / u.cache_write() as f64).round() as u64 } else { 0 };
    let long = long.min(u.write_long);
    let short = (written - long).min(u.write_short);
    let uncached = (moved - written).min(u.uncached);
    Usage {
        cache_read: u.cache_read + long + short + uncached,
        write_short: u.write_short - short,
        write_long: u.write_long - long,
        uncached: u.uncached - uncached,
        ..u
    }
}

/// The usage of a request whose old conversation, the span from `shared` to `context` of its
/// prompt, is scaled by `keep`. The span's read, written and uncached parts shrink with it.
fn shrink(u: Usage, shared: f64, context: f64, keep: f64) -> Usage {
    if keep >= 1.0 {
        return u;
    }
    let t = u.prompt() as f64;
    let end = context.min(t);
    let (r, w) = (u.cache_read as f64, u.cache_write() as f64);
    let old = |a: f64, z: f64| (z.min(end) - a.max(shared)).max(0.0) * (1.0 - keep);
    let (read, written, uncached) = (old(0.0, r), old(r, r + w), old(r + w, t));
    let long = if w > 0.0 { u.write_long as f64 / w } else { 0.0 };
    let less = |x: u64, by: f64| (x as f64 - by).round().max(0.0) as u64;
    Usage {
        cache_read: less(u.cache_read, read),
        write_short: less(u.write_short, written * (1.0 - long)),
        write_long: less(u.write_long, written * long),
        uncached: less(u.uncached, uncached),
        ..u
    }
}

/// Mean cost per return after each of the first `n` requests.
pub fn mean_curve(returns: &[&Return], s: Strategy, n: usize) -> Vec<f64> {
    (0..=n).map(|k| returns.iter().map(|r| r.after(s, k)).sum::<f64>() / returns.len().max(1) as f64).collect()
}

/// The first request count from which `a` costs less than `b` on average, up to `n`.
pub fn break_even(returns: &[&Return], a: Strategy, b: Strategy, n: usize) -> Option<usize> {
    lasting_lead(&mean_curve(returns, a, n), &mean_curve(returns, b, n))
}

/// The first request from which curve `a` stays below `b` to the end, if it ends below.
pub fn lasting_lead(a: &[f64], b: &[f64]) -> Option<usize> {
    let n = a.len().min(b.len()).checked_sub(1)?;
    let behind = (1..=n).rev().find(|&k| a[k] >= b[k]).unwrap_or(0);
    (behind < n).then_some(behind + 1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Chain, ChainKind};

    const HOUR: i64 = 3_600_000;

    fn request(ts_ms: i64, read: u64, write_long: u64, output: u64) -> Request {
        Request {
            ts_ms,
            api: crate::api::ApiId::Anthropic,
            model: "claude-opus-5-5".into(),
            usage: Usage { cache_read: read, write_long, output, ..Usage::default() },
            ..Request::default()
        }
    }

    /// A 500k-token conversation that idles for two hours, comes back cold, does three more
    /// requests, compacts and goes on.
    fn session() -> Session {
        let mut s = Session::new(crate::testing::claude(), "s".into());
        let mut requests = vec![request(0, 0, 400_000, 1_000), request(60_000, 400_000, 100_000, 1_000)];
        requests.push(request(2 * HOUR, 10_000, 492_000, 1_000));
        for k in 1..=3 {
            requests.push(request(2 * HOUR + k * 30_000, 500_000 + 2_000 * k as u64, 2_000, 1_000));
        }
        let mut after = request(2 * HOUR + 200_000, 10_000, 40_000, 1_000);
        after.compacted_before = true;
        requests.push(after);
        s.chains.push(Chain { name: "main".into(), kind: ChainKind::Main, requests });
        s
    }

    #[test]
    fn shrink_scales_the_old_conversation_only() {
        // The cold return: 10k still cached, the old 490k and 2k of new content written.
        let cold = shrink(Usage { cache_read: 10_000, write_long: 492_000, ..Usage::default() }, 10_000.0, 500_000.0, 0.1);
        assert_eq!((cold.cache_read, cold.write_long), (10_000, 51_000));
        // A warm request after it reads the old span and writes only what is new.
        let warm = shrink(Usage { cache_read: 502_000, write_long: 3_000, ..Usage::default() }, 10_000.0, 500_000.0, 0.1);
        assert_eq!((warm.cache_read, warm.write_long), (61_000, 3_000));
    }

    #[test]
    fn a_cold_return_is_replayed_five_ways() {
        let found = scan(&session(), &Filter::default(), 60, 100_000);
        assert_eq!((found.cases.len(), found.compacted.len()), (1, 1));
        let case = &found.cases[0];
        assert_eq!((case.context, case.steps.len(), case.idle_ms), (500_000, 4, 2 * HOUR - 60_000));

        let b = replay(found, &Pricing::defaults(), 60, 100_000, 6 * HOUR);
        assert_eq!(b.base[&crate::testing::claude()].tokens, 50_000);
        let r = &b.returns[0];
        let cost = |s| r.after(s, 60);
        // Resuming is the logged cost: 10k read, 492k written at $8/M, then the warm requests.
        let logged = (10_000.0 * 0.2 + 492_000.0 * 8.0 + 1_000.0 * 20.0) / 1e6;
        assert!((r.after(Strategy::Resume, 1) - logged).abs() < 1e-9);
        // The compaction call writes the cold context again before anything else.
        assert!((r.after(Strategy::Compact, 0) - (10_000.0 * 0.2 + 490_000.0 * 8.0 + 10_000.0 * 20.0) / 1e6).abs() < 1e-9);
        // Two pings, at 50 and 100 minutes of the 119-minute break, each read the 500k context.
        assert_eq!((r.pings, r.kept_warm), (2, true));
        assert!((r.after(Strategy::Heartbeats, 0) - 2.0 * (500_000.0 * 0.2 + 100.0 * 20.0) / 1e6).abs() < 1e-9);
        // The warm return reads the old 500k and writes only the 2k that is new.
        let warm = (500_000.0 * 0.2 + 2_000.0 * 8.0 + 1_000.0 * 20.0) / 1e6;
        assert!((r.after(Strategy::Heartbeats, 1) - r.after(Strategy::Heartbeats, 0) - warm).abs() < 1e-9);
        // Three requests are too few for /compact to pay off: it saves only on reads. Over so few
        // requests, keeping the full context warm costs less than the note. Within the 6h window,
        // heartbeats then compaction never compacts, so it costs what heartbeats do.
        let costs = Strategy::ALL.map(|s| (cost(s) * 100.0).round() / 100.0);
        assert_eq!(costs, [4.37, 4.62, 0.8, 0.75, 0.75], "{costs:?}");
        assert!(!r.compacted);
        // The X-ray forecast at the break: the 500k context and the pace before it, all written.
        assert!((r.forecast - ((500_000.0 + 250_000.0) * 8.0 + 1_000.0 * 20.0) / 1e6).abs() < 1e-9);
    }

    /// The example session with its return `idle_min` minutes after the last request before it.
    fn session_back_after(idle_min: i64) -> Session {
        let mut s = session();
        let shift = idle_min * 60_000 - (2 * HOUR - 60_000);
        for r in s.chains[0].requests.iter_mut().skip(2) {
            r.ts_ms += shift;
        }
        s
    }

    #[test]
    fn heartbeats_then_compaction_keeps_a_return_inside_the_window_warm() {
        let b = replay(scan(&session_back_after(119), &Filter::default(), 60, 100_000), &Pricing::defaults(), 60, 100_000, 2 * HOUR);
        let r = &b.returns[0];
        let ping = (500_000.0 * 0.2 + 100.0 * 20.0) / 1e6;
        assert!((r.after(Strategy::WarmThenCompact, 0) - 2.0 * ping).abs() < 1e-9, "two pings, no compaction");
        assert!(!r.compacted);
        for k in 0..=4 {
            assert!((r.after(Strategy::WarmThenCompact, k) - r.after(Strategy::Heartbeats, k)).abs() < 1e-9, "priced as heartbeats at {k}");
        }
    }

    #[test]
    fn heartbeats_then_compaction_compacts_past_the_window() {
        let b = replay(scan(&session_back_after(180), &Filter::default(), 60, 100_000), &Pricing::defaults(), 60, 100_000, 2 * HOUR);
        let r = &b.returns[0];
        let ping = (500_000.0 * 0.2 + 100.0 * 20.0) / 1e6;
        let compaction: f64 = (500_000.0 * 0.2 + 10_000.0 * 20.0) / 1e6;
        assert!((compaction - 0.30).abs() < 1e-9);
        assert!(r.compacted);
        assert!((r.after(Strategy::WarmThenCompact, 0) - (2.0 * ping + compaction)).abs() < 1e-9, "{}", r.after(Strategy::WarmThenCompact, 0));
        // The requests after it run on the base, as they do for /compact first.
        for k in 1..=4 {
            let mine = r.after(Strategy::WarmThenCompact, k) - r.after(Strategy::WarmThenCompact, 0);
            let compact = r.after(Strategy::Compact, k) - r.after(Strategy::Compact, 0);
            assert!((mine - compact).abs() < 1e-9, "request {k}: {mine} vs {compact}");
        }
    }

    /// A Codex request on `gpt-6-astra`, which writes at the 30-minute price.
    fn codex_request(ts_ms: i64, read: u64, write: u64) -> Request {
        Request {
            ts_ms,
            api: crate::api::ApiId::OpenAi,
            model: "gpt-6-astra".into(),
            usage: Usage { cache_read: read, write_short: write, output: 1_000, ..Usage::default() },
            ..Request::default()
        }
    }

    /// A Codex conversation with a 200k-token context, a cold return `idle_min` minutes later that
    /// reads 15k of it from the cache, a request after the return, then a compaction.
    fn codex_session(idle_min: i64) -> Session {
        let back = 60_000 + idle_min * 60_000;
        let after = Request { compacted_before: true, ..codex_request(back + 200_000, 15_000, 5_000) };
        let requests = vec![
            codex_request(0, 0, 190_000),
            codex_request(60_000, 190_000, 10_000),
            codex_request(back, 15_000, 187_000),
            codex_request(back + 30_000, 202_000, 2_000),
            after,
        ];
        let mut s = Session::new(crate::testing::codex(), "c".into());
        s.chains.push(Chain { name: "main".into(), kind: ChainKind::Main, requests });
        s
    }

    #[test]
    fn a_codex_compaction_writes_the_whole_context_warm_or_cold() {
        let b = replay(scan(&codex_session(120), &Filter::default(), 60, 100_000), &Pricing::defaults(), 60, 100_000, HOUR);
        let r = &b.returns[0];
        assert_eq!((r.context, r.long_ttl), (200_000, false));
        // /compact first: all 200k written at the 30m price, though the return read 15k.
        let compaction: f64 = (200_000.0 * 12.5 + 10_000.0 * 50.0) / 1e6;
        assert!((compaction - 3.00).abs() < 1e-9);
        assert!((r.after(Strategy::Compact, 0) - compaction).abs() < 1e-9, "{}", r.after(Strategy::Compact, 0));
        // Heartbeats then compaction: pings at 25 and 50 minutes, a compaction at 75 that writes
        // the warm context again, then the return two hours after the last request.
        assert_eq!((r.pings, r.compacted), (2, true));
        let warm_then_compact = r.after(Strategy::WarmThenCompact, 0) - r.after(Strategy::Heartbeats, 0);
        assert!((warm_then_compact - compaction).abs() < 1e-9, "{warm_then_compact}");
    }

    #[test]
    fn a_codex_break_that_was_no_return_compacts_by_writing_the_context() {
        let codex = crate::testing::codex();
        let mut s = Session::new(codex.clone(), "c".into());
        s.chains.push(Chain {
            name: "main".into(),
            kind: ChainKind::Main,
            requests: vec![codex_request(0, 0, 190_000), codex_request(60_000, 190_000, 10_000)],
        });
        let mut found = scan(&s, &Filter::default(), 60, 100_000);
        // The logs end two hours after the conversation's last request.
        found.latest_ms = Some(60_000 + 2 * HOUR);
        let e = price_elsewhere(&found, &Pricing::defaults(), HOUR)[&codex];
        assert_eq!((e.pings, e.compactions), (2, 1));
        assert!((e.compaction_usd - 3.00).abs() < 1e-9, "{e:?}");
    }

    #[test]
    fn with_no_window_the_compaction_goes_out_at_the_first_ping_time() {
        let minute = 60_000;
        assert_eq!(compaction_at(HOUR, 0), 50 * minute);
        assert_eq!(compaction_at(5 * minute, 0), 250_000);
        assert_eq!(compaction_at(HOUR, 2 * HOUR), 150 * minute);
        assert_eq!(compaction_at(HOUR, 6 * HOUR), 400 * minute, "6 hours 40 minutes");
        let b = replay(scan(&session(), &Filter::default(), 60, 100_000), &Pricing::defaults(), 60, 100_000, 0);
        let r = &b.returns[0];
        assert_eq!((r.pings, r.compacted), (0, true));
    }

    /// A conversation of `context` tokens on the 1-hour cache whose second request comes
    /// `idle_min` minutes later and reads all of the first prompt, then a third request.
    fn warm_break(context: u64, idle_min: i64) -> Session {
        let mut s = Session::new(crate::testing::claude(), "w".into());
        let requests = vec![
            request(0, 0, context, 1_000),
            request(idle_min * 60_000, context, 1_000, 1_000),
            request(idle_min * 60_000 + 30_000, context + 1_000, 1_000, 1_000),
        ];
        s.chains.push(Chain { name: "main".into(), kind: ChainKind::Main, requests });
        s
    }

    #[test]
    fn breaks_that_were_no_return_are_priced_for_the_heartbeat_ways() {
        let pricing = Pricing::defaults();
        let claude = crate::testing::claude();
        // The cache was still warm after 55 minutes: one ping at 50 minutes. The conversation's
        // last request ends the logs, so its open break is empty.
        let found = scan(&warm_break(300_000, 55), &Filter::default(), 60, 100_000);
        assert!(found.cases.is_empty());
        let e = price_elsewhere(&found, &pricing, 6 * HOUR)[&claude];
        assert_eq!((e.pings, e.compactions), (1, 0));
        assert!((e.ping_usd - (300_000.0 * 0.2 + 100.0 * 20.0) / 1e6).abs() < 1e-9);
        // A small context and a short break price nothing.
        let priced = |context, idle_min| {
            price_elsewhere(&scan(&warm_break(context, idle_min), &Filter::default(), 60, 100_000), &pricing, 6 * HOUR)
                .get(&claude)
                .copied()
                .unwrap_or_default()
        };
        assert_eq!(priced(60_000, 55), Elsewhere::default());
        assert_eq!(priced(300_000, 40), Elsewhere::default());
    }

    #[test]
    fn a_conversation_left_open_pings_until_the_end_of_the_logs() {
        let pricing = Pricing::defaults();
        let claude = crate::testing::claude();
        // The last request of one conversation comes 10 hours before the latest one read.
        let mut found = scan(&warm_break(300_000, 1), &Filter::default(), 60, 100_000);
        let other = scan(
            &{
                let mut s = warm_break(10_000, 1);
                s.chains[0].requests.iter_mut().for_each(|r| r.ts_ms += 10 * HOUR + 90_000);
                s
            },
            &Filter::default(),
            60,
            100_000,
        );
        found = found.merge(other);
        let e = price_elsewhere(&found, &pricing, 6 * HOUR)[&claude];
        assert_eq!((e.pings, e.compactions), (7, 1), "seven pings to 5h50m, then a compaction at 6h40m");
        // An earlier --until ends the open break there.
        found.until_ms = Some(3 * HOUR);
        let e = price_elsewhere(&found, &pricing, 6 * HOUR)[&claude];
        assert_eq!((e.pings, e.compactions), (3, 0));
    }

    #[test]
    fn heartbeats_keep_the_cache_warm_only_within_their_window() {
        let minute = 60_000;
        // A 1h cache gets a ping every 50 minutes; the return 19 minutes after the last is warm.
        assert_eq!(heartbeats(119 * minute, HOUR, 6 * HOUR), (vec![50 * minute, 100 * minute], true));
        // A break of 10h outlasts a 6h window: seven pings, then the cache expires anyway.
        let (pings, warm) = heartbeats(10 * HOUR, HOUR, 6 * HOUR);
        assert_eq!((pings.len(), pings.last().copied(), warm), (7, Some(350 * minute), false));
        // The 5-minute cache needs a ping every 4m10s.
        let (pings, warm) = heartbeats(30 * minute, 5 * minute, 6 * HOUR);
        assert_eq!((pings.len(), warm), (7, true));
        // With no window there are no pings, and the return stays cold.
        assert_eq!(heartbeats(2 * HOUR, HOUR, 0), (vec![], false));
        assert_eq!(window_name(6 * HOUR), "6h");
        assert_eq!(window_name(90 * minute + 30_000), "90.5m");
        assert_eq!(window_name(30_000), "0.5m");
        assert_eq!(window_name(90 * minute), "90m");
    }

    #[test]
    fn a_warm_return_reads_the_previous_prompt() {
        // Cold: 10k read, 490k of old context and 2k of new content written, 1k uncached.
        let cold = Usage { cache_read: 10_000, write_long: 400_000, write_short: 92_000, uncached: 1_000, output: 500, ..Usage::default() };
        let warm = warmed(cold, 500_000);
        assert_eq!(warm.prompt(), cold.prompt());
        assert_eq!(warm.cache_read, 500_000);
        assert_eq!(warm.cache_write(), 2_000);
        assert_eq!((warm.uncached, warm.output), (1_000, 500));
        // A prompt smaller than the old context reads all of it.
        let small = warmed(Usage { cache_read: 0, write_long: 30_000, ..Usage::default() }, 50_000);
        assert_eq!((small.cache_read, small.write_long), (30_000, 0));
    }

    /// A session that spans the release which added request-audit lines: its main thread is in two
    /// chains of one name, and the return is the first request of the second.
    #[test]
    fn a_return_between_the_two_chains_of_a_conversation_is_found() {
        let mut s = session();
        let requests = std::mem::take(&mut s.chains[0].requests);
        let (before, after) = requests.split_at(2);
        s.chains[0].requests = before.to_vec();
        s.chains.push(Chain { name: "main".into(), kind: ChainKind::Main, requests: after.to_vec() });
        let found = scan(&s, &Filter::default(), 60, 100_000);
        assert_eq!(found.cases.len(), 1);
        assert_eq!((found.cases[0].context, found.cases[0].steps.len()), (500_000, 4));
    }

    #[test]
    fn compact_pays_off_only_if_it_stays_cheaper() {
        let resume = [0.0, 5.0, 6.0, 7.0, 8.0];
        assert_eq!(lasting_lead(&[0.0, 6.0, 6.5, 6.9, 7.5], &resume), Some(3));
        // Below at request 2, above again at the end: no payoff.
        assert_eq!(lasting_lead(&[0.0, 6.0, 5.0, 7.5, 8.5], &resume), None);
        assert_eq!(lasting_lead(&[0.0, 4.0, 5.0, 6.0, 7.0], &resume), Some(1));
    }
}
