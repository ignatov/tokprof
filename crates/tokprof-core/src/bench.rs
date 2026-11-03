//! `tokprof bench`: what it costs to come back to a big session after its prompt cache
//! expired, and what the same work costs after `/compact`, in a new session that refers to
//! the old one, or with heartbeats that keep the cache warm.
//!
//! Every cold return in the logs is a measured case. It is a request of a main conversation
//! with a context of at least `min_context` tokens that came after an idle gap longer than the
//! cache lifetime and had to write that context again. The bench takes it and the requests
//! that followed, up to `requests` of them and until the next such return or a compaction, and
//! prices the same requests four ways:
//! - **resume as is**: as logged.
//! - **/compact first**: a compaction call, then the requests on top of what a compaction
//!   leaves. Claude Code does not log the call. It runs as a fork that shares the
//!   conversation's cache, so on a cold cache it writes the whole context again, and then it
//!   outputs a summary.
//! - **new session + handoff note**: the old session writes a note while its cache is still
//!   warm, and a new session does the requests on top of the same base.
//! - **heartbeats, then resume**: while the session is idle, a ping at 5/6 of the cache
//!   lifetime reads the context, which resets the cache timer, for up to `keep_warm` after the
//!   last request. A return within the lifetime of the last ping reads its context from the
//!   cache and then goes on as logged. A later return is cold anyway, after the pings.
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
}

impl Strategy {
    pub const ALL: [Strategy; 4] = [Strategy::Resume, Strategy::Compact, Strategy::Handoff, Strategy::Heartbeats];

    pub fn name(self) -> &'static str {
        match self {
            Strategy::Resume => "resume as is",
            Strategy::Compact => "/compact first",
            Strategy::Handoff => "new session + handoff note",
            Strategy::Heartbeats => "heartbeats, then resume",
        }
    }
}

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

/// What the sessions of a bench run hold: cold returns, and the context sizes the
/// alternatives start from.
#[derive(Default)]
pub struct Found {
    cases: Vec<Case>,
    /// Prompt size right after a compaction of a big context, per agent.
    compacted: Vec<(AgentId, u64)>,
    /// First prompt of each main conversation, per agent.
    starts: Vec<(AgentId, u64)>,
}

impl Found {
    pub fn merge(mut self, other: Found) -> Found {
        self.cases.extend(other.cases);
        self.compacted.extend(other.compacted);
        self.starts.extend(other.starts);
        self
    }
}

/// The request came after the cache expired and wrote the previous prompt again.
fn cold_return(long_ttl: bool, prev: &Request, r: &Request) -> bool {
    r.api.ttl(long_ttl).is_some_and(|(ttl_ms, _)| r.ts_ms - prev.ts_ms > ttl_ms) && cache::rewritten(prev.usage.prompt(), &r.usage).is_some()
}

/// Finds the cold returns of a session's main conversations.
pub fn scan(s: &Session, filter: &Filter, requests: usize, min_context: u64) -> Found {
    let mut found = Found::default();
    for (_, conversation) in s.main_conversations() {
        let billed: Vec<usize> = (0..conversation.len()).filter(|&i| conversation[i].usage.prompt() > 0 && !conversation[i].advisor).collect();
        if let Some(&first) = billed.first() {
            found.starts.push((s.agent.clone(), conversation[first].usage.prompt()));
        }
        for (k, pair) in billed.windows(2).enumerate() {
            let (prev, r) = (&conversation[pair[0]], &conversation[pair[1]]);
            if r.compacted_before {
                if prev.usage.prompt() >= min_context {
                    found.compacted.push((s.agent.clone(), r.usage.prompt()));
                }
                continue;
            }
            let history = &conversation[..=pair[0]];
            let long_ttl = forecast::long_ttl(history);
            if prev.usage.prompt() < min_context || r.model != prev.model || !filter.keeps(r) || !cold_return(long_ttl, prev, r) {
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
    }
    found
}

/// One measured return, priced four ways.
pub struct Return {
    pub agent: AgentId,
    /// The API of the return.
    pub api: crate::api::ApiId,
    pub context: u64,
    pub idle_ms: i64,
    pub long_ttl: bool,
    /// Cost after each request since the return, per strategy. `[0]` is what the strategy
    /// spends before the first request: the compaction call, the note, or the pings.
    pub cumulative: [Vec<f64>; 4],
    /// The X-ray's forecast of the first request once the cache is cold.
    pub forecast: f64,
    /// How many heartbeats went out during the break, and whether they kept the cache warm
    /// until the return.
    pub pings: usize,
    pub kept_warm: bool,
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

pub struct Bench {
    pub requests: usize,
    pub min_context: u64,
    /// How long heartbeats keep pinging after the last request.
    pub keep_warm_ms: i64,
    pub returns: Vec<Return>,
    pub base: BTreeMap<AgentId, Base>,
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

/// Prices every return four ways.
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
    Bench { requests, min_context, keep_warm_ms, returns, base }
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
    // Each ping reads the context at the price of its own time.
    let (pings, kept_warm) = heartbeats(c.idle_ms, c.api.ttl(c.long_ttl).map_or(i64::MAX, |(ttl_ms, _)| ttl_ms), keep_warm_ms);
    let pinged: f64 = pings
        .iter()
        .filter_map(|t| pricing.get(&first.model, c.context, c.before_ms + t))
        .map(|at| (ctx * at.read + PING_OUTPUT * at.output) / 1e6)
        .sum();
    let up_front = [
        0.0,
        (shared * p.read + (ctx - shared) * write + SUMMARY_TOKENS * p.output) / 1e6,
        (ctx * note.read + SUMMARY_TOKENS * note.output) / 1e6,
        pinged,
    ];
    let cumulative = Strategy::ALL.map(|s| {
        let keep = if matches!(s, Strategy::Resume | Strategy::Heartbeats) { 1.0 } else { keep };
        let mut total = up_front[s as usize];
        let mut out = vec![total];
        for (i, step) in c.steps.iter().enumerate() {
            let u = if s == Strategy::Heartbeats && i == 0 && kept_warm { warmed(step.usage, c.context) } else { shrink(step.usage, shared, ctx, keep) };
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
        Request { ts_ms, api: crate::api::ApiId::Anthropic, model: "claude-opus-5-5".into(), usage: Usage { cache_read: read, write_long, output, ..Usage::default() }, ..Request::default() }
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
    fn a_cold_return_is_replayed_four_ways() {
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
        // requests, keeping the full context warm costs less than the note.
        let costs = Strategy::ALL.map(|s| (cost(s) * 100.0).round() / 100.0);
        assert_eq!(costs, [4.37, 4.62, 0.8, 0.75], "{costs:?}");
        // The X-ray forecast at the break: the 500k context and the pace before it, all written.
        assert!((r.forecast - ((500_000.0 + 250_000.0) * 8.0 + 1_000.0 * 20.0) / 1e6).abs() < 1e-9);
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
