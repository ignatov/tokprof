//! The pause report: how long the user pauses before typing again, what those pauses cost
//! through the prompt cache, and how large the sessions grow.
//!
//! It is a sink over the requests and misses the analysis reports. The analysis tells no chain
//! boundary, so the collector works out, before each session, which chain each reported request
//! comes from, by the analysis's own rule: a request with usage that the filter keeps.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use crate::analysis::cache::Cause;
use crate::analysis::{Fact, Filter, MissEvent, RequestRecord, Sink};
use crate::model::{ChainKind, Session};
use crate::source::AgentId;
use crate::trigger::Trigger;

const MINUTE: i64 = 60_000;
const HOUR: i64 = 60 * MINUTE;

/// The ranges of the pause before a typed message: name and lower bound in ms. A range takes
/// its lower bound and ends where the next one starts.
pub const PAUSE_RANGES: [(&str, i64); 10] = [
    ("under 1m", 0),
    ("1–5m", MINUTE),
    ("5–15m", 5 * MINUTE),
    ("15–30m", 15 * MINUTE),
    ("30–60m", 30 * MINUTE),
    ("1–2h", HOUR),
    ("2–4h", 2 * HOUR),
    ("4–8h", 4 * HOUR),
    ("8–24h", 8 * HOUR),
    ("over 1d", 24 * HOUR),
];

/// The ranges of the pause before a cold return.
pub const RETURN_RANGES: [(&str, i64); 7] =
    [("under 1h", 0), ("1–2h", HOUR), ("2–4h", 2 * HOUR), ("4–8h", 4 * HOUR), ("8–16h", 8 * HOUR), ("16–48h", 16 * HOUR), ("over 2d", 48 * HOUR)];

/// The ranges of a session's largest main prompt, by lower bound in tokens.
pub const SIZE_RANGES: [(&str, u64); 5] = [("under 100k", 0), ("100–200k", 100_000), ("200–400k", 200_000), ("400–800k", 400_000), ("800k+", 800_000)];

/// A follow of at most this many requests counts as a return that ended soon.
pub const SHORT_FOLLOW: u64 = 3;

/// Who started a cold return, in the order the report lists them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Starter {
    Typed,
    Background,
    Tool,
    Other,
}

impl Starter {
    pub const ALL: [Starter; 4] = [Starter::Typed, Starter::Background, Starter::Tool, Starter::Other];

    /// The starter of a request with this trigger.
    pub fn of(t: Trigger) -> Starter {
        match t {
            Trigger::Prompt => Starter::Typed,
            Trigger::Notification => Starter::Background,
            Trigger::Lookup | Trigger::Lookups | Trigger::Edit | Trigger::Run | Trigger::Subagent => Starter::Tool,
            _ => Starter::Other,
        }
    }
}

/// A cold return: the pause before it, what its first request cost and could have saved, the
/// context it came back to, and who started it.
#[derive(Clone, Copy, Debug)]
pub struct ColdReturn {
    /// When the return's request was sent.
    pub ts_ms: i64,
    pub pause_ms: i64,
    pub usd: f64,
    pub premium_usd: f64,
    pub context: u64,
    pub starter: Starter,
}

/// One session: its largest main prompt, what all its requests spent, its cold returns, the
/// times of its first and last request, subagents included, and the requests of its main
/// conversations that a typed message started.
#[derive(Clone, Copy, Debug, Default)]
pub struct SessionSize {
    pub largest: u64,
    pub usd: f64,
    pub tokens: f64,
    pub cold_returns: u64,
    pub span: Option<(i64, i64)>,
    pub typed: u64,
}

impl SessionSize {
    /// The time from the first to the last request, in ms.
    pub fn span_ms(&self) -> i64 {
        self.span.map_or(0, |(first, last)| last - first)
    }
}

fn join_span(a: Option<(i64, i64)>, b: Option<(i64, i64)>) -> Option<(i64, i64)> {
    match (a, b) {
        (Some(x), Some(y)) => Some((x.0.min(y.0), x.1.max(y.1))),
        (x, y) => x.or(y),
    }
}

/// A row of a table of ranges: its name, how many there were, and their sums.
#[derive(Clone, Debug, PartialEq)]
pub struct RangeRow {
    pub name: &'static str,
    pub count: usize,
    pub usd: f64,
    pub premium_usd: f64,
    pub tokens: f64,
    pub median_context: Option<u64>,
    pub cold_returns: u64,
}

/// What the pause report shows of one agent.
#[derive(Clone, Debug, Default)]
pub struct AgentPauses {
    /// Each typed message after an earlier request: its pause in ms and the prompt of its request.
    pub typed: Vec<(i64, u64)>,
    /// The cache lifetimes the agent's APIs can use, in ms.
    pub lifetimes: BTreeSet<i64>,
    pub returns: Vec<ColdReturn>,
    /// For each cold return, the requests from it up to the next cold pause or compaction.
    pub follows: Vec<u64>,
    /// By the session's short id; a subagent's log adds to its main session's.
    pub sessions: HashMap<String, SessionSize>,
    /// The cold returns of each client (see `Request::client_name`), and their avoidable dollars.
    pub cold_returns_by_client: BTreeMap<String, (u64, f64)>,
    /// Whether any request had a price; amounts are tokens otherwise.
    pub priced: bool,
    /// Where the agent's busy time went over the sessions (see agent-time); the caller paints it
    /// once every log of a session is read.
    pub busy: crate::busy::Busy,
}

/// The upper of the two middle values, as the bench takes medians.
fn median(mut v: Vec<u64>) -> Option<u64> {
    v.sort_unstable();
    v.get(v.len() / 2).copied()
}

fn range_of<T: PartialOrd + Copy>(ranges: &[(&'static str, T)], value: T) -> usize {
    ranges.iter().rposition(|(_, lower)| value >= *lower).unwrap_or(0)
}

impl AgentPauses {
    fn merge(&mut self, o: AgentPauses) {
        self.typed.extend(o.typed);
        self.lifetimes.extend(o.lifetimes);
        self.returns.extend(o.returns);
        self.follows.extend(o.follows);
        for (id, s) in o.sessions {
            let e = self.sessions.entry(id).or_default();
            e.largest = e.largest.max(s.largest);
            e.usd += s.usd;
            e.tokens += s.tokens;
            e.cold_returns += s.cold_returns;
            e.span = join_span(e.span, s.span);
            e.typed += s.typed;
        }
        for (client, (n, usd)) in o.cold_returns_by_client {
            let e = self.cold_returns_by_client.entry(client).or_default();
            e.0 += n;
            e.1 += usd;
        }
        self.priced |= o.priced;
        self.busy.merge(o.busy);
    }

    /// The typed messages per pause range, with the median prompt of their requests; every range,
    /// empty ones included.
    pub fn pause_rows(&self) -> Vec<RangeRow> {
        let mut contexts: Vec<Vec<u64>> = vec![Vec::new(); PAUSE_RANGES.len()];
        for &(pause, context) in &self.typed {
            contexts[range_of(&PAUSE_RANGES, pause)].push(context);
        }
        PAUSE_RANGES
            .iter()
            .zip(contexts)
            .map(|((name, _), c)| RangeRow { name, count: c.len(), usd: 0.0, premium_usd: 0.0, tokens: 0.0, median_context: median(c), cold_returns: 0 })
            .collect()
    }

    /// The share of typed messages whose pause is longer than a cache lifetime, in percent.
    pub fn share_after(&self, lifetime_ms: i64) -> f64 {
        let after = self.typed.iter().filter(|(pause, _)| *pause > lifetime_ms).count();
        100.0 * after as f64 / self.typed.len().max(1) as f64
    }

    /// The cold returns per pause range, with what their first requests cost and could have saved
    /// and the median context they came back to; empty ranges left out.
    pub fn return_rows(&self) -> Vec<RangeRow> {
        let mut rows: Vec<RangeRow> = RETURN_RANGES
            .iter()
            .map(|(name, _)| RangeRow { name, count: 0, usd: 0.0, premium_usd: 0.0, tokens: 0.0, median_context: None, cold_returns: 0 })
            .collect();
        let mut contexts: Vec<Vec<u64>> = vec![Vec::new(); RETURN_RANGES.len()];
        for r in &self.returns {
            let i = range_of(&RETURN_RANGES, r.pause_ms);
            rows[i].count += 1;
            rows[i].usd += r.usd;
            rows[i].premium_usd += r.premium_usd;
            contexts[i].push(r.context);
        }
        for (row, c) in rows.iter_mut().zip(contexts) {
            row.median_context = median(c);
        }
        rows.into_iter().filter(|r| r.count > 0).collect()
    }

    /// The cold returns' count, what their first requests cost, and how much of it was avoidable,
    /// summed over `return_rows`.
    pub fn return_totals(&self) -> (usize, f64, f64) {
        self.return_rows().iter().fold((0, 0.0, 0.0), |(n, u, p), r| (n + r.count, u + r.usd, p + r.premium_usd))
    }

    /// How many cold returns each starter started, in `Starter::ALL` order.
    pub fn starters(&self) -> [usize; 4] {
        Starter::ALL.map(|s| self.returns.iter().filter(|r| r.starter == s).count())
    }

    /// The median follow, the follows at one and three quarters of the sorted list, and how many
    /// were at most `SHORT_FOLLOW` requests; `None` without cold returns.
    pub fn follow_stats(&self) -> Option<(u64, u64, u64, usize)> {
        let mut f = self.follows.clone();
        f.sort_unstable();
        let at = |q: usize| f.get(f.len() * q / 4).copied();
        Some((at(2)?, at(1)?, at(3)?, f.iter().filter(|n| **n <= SHORT_FOLLOW).count()))
    }

    /// The sessions per range of their largest main prompt, with what they spent and their cold
    /// returns; empty ranges left out.
    pub fn size_rows(&self) -> Vec<RangeRow> {
        let mut rows: Vec<RangeRow> = SIZE_RANGES
            .iter()
            .map(|(name, _)| RangeRow { name, count: 0, usd: 0.0, premium_usd: 0.0, tokens: 0.0, median_context: None, cold_returns: 0 })
            .collect();
        for s in self.sessions.values() {
            let row = &mut rows[range_of(&SIZE_RANGES, s.largest)];
            row.count += 1;
            row.usd += s.usd;
            row.tokens += s.tokens;
            row.cold_returns += s.cold_returns;
        }
        rows.into_iter().filter(|r| r.count > 0).collect()
    }

    /// What all the agent's sessions spent.
    pub fn spend(&self) -> (f64, f64) {
        self.sessions.values().fold((0.0, 0.0), |(usd, tokens), s| (usd + s.usd, tokens + s.tokens))
    }
}

/// A main conversation as the collector follows it.
#[derive(Clone, Copy, Debug, Default)]
struct Conversation {
    last_ts: Option<i64>,
    /// Whether its latest request that wrote to the cache wrote more with the long lifetime.
    long_ttl: bool,
    /// The requests counted so far since a cold return, while that follow lasts.
    follow: Option<u64>,
}

/// The latest main-conversation request, until its miss comes or the next request does.
#[derive(Clone, Debug)]
struct Last {
    ts_ms: i64,
    conversation: usize,
    pause_ms: Option<i64>,
    usd: f64,
    context: u64,
    trigger: Trigger,
    client: String,
}

/// The session being read.
#[derive(Clone, Debug)]
struct Current {
    agent: AgentId,
    session: String,
    /// The chain of each request the analysis reports, in its order.
    order: Vec<usize>,
    next: usize,
    /// The conversation of each main chain.
    conversation_of: Vec<Option<usize>>,
    conversations: Vec<Conversation>,
    last: Option<Last>,
}

/// Collects the pause report of the requests a filter keeps. Feed it the analysis of each
/// session, with the same filter, then call [`Pauses::finish`].
#[derive(Clone, Debug, Default)]
pub struct Pauses {
    filter: Filter,
    pub agents: BTreeMap<AgentId, AgentPauses>,
    current: Option<Current>,
}

impl Pauses {
    pub fn new(filter: Filter) -> Pauses {
        Pauses { filter, ..Pauses::default() }
    }

    /// Ends the session being read: the follows still open end with it.
    pub fn finish(&mut self) {
        let Some(cur) = self.current.take() else { return };
        let open: Vec<u64> = cur.conversations.iter().filter_map(|c| c.follow).collect();
        if !open.is_empty() {
            self.agents.entry(cur.agent).or_default().follows.extend(open);
        }
    }

    pub fn merge(&mut self, mut o: Pauses) {
        self.finish();
        o.finish();
        for (agent, a) in o.agents {
            self.agents.entry(agent).or_default().merge(a);
        }
    }
}

impl Sink for Pauses {
    fn session(&mut self, s: &Session) {
        self.finish();
        let mut order = Vec::new();
        for (i, c) in s.chains.iter().enumerate() {
            for r in &c.requests {
                if (r.usage.prompt() > 0 || r.usage.output > 0) && self.filter.keeps(r) {
                    order.push(i);
                }
            }
        }
        let conversations = s.main_conversation_chains();
        let mut conversation_of = vec![None; s.chains.len()];
        for (k, chains) in conversations.iter().enumerate() {
            for &i in chains {
                conversation_of[i] = Some(k);
            }
        }
        self.current = Some(Current {
            agent: s.agent.clone(),
            session: s.short_id().to_string(),
            order,
            next: 0,
            conversation_of,
            conversations: vec![Conversation::default(); conversations.len()],
            last: None,
        });
    }

    fn request(&mut self, r: &RequestRecord) {
        let Pauses { agents, current, .. } = self;
        let Some(cur) = current else { return };
        // A miss comes right after its request: the one before this request had none.
        cur.last = None;
        let chain = cur.order.get(cur.next).copied();
        cur.next += 1;
        let usd = r.price.map_or(0.0, |p| p.cost(&r.usage));
        let agent = agents.entry(cur.agent.clone()).or_default();
        agent.priced |= r.price.is_some();
        let size = agent.sessions.entry(cur.session.clone()).or_default();
        size.usd += usd;
        size.tokens += (r.usage.prompt() + r.usage.output) as f64;
        size.span = join_span(size.span, Some((r.ts_ms, r.ts_ms)));
        let Some(k) = chain.and_then(|c| cur.conversation_of.get(c).copied().flatten()) else { return };
        if r.advisor || r.chain_kind != ChainKind::Main {
            return;
        }
        size.largest = size.largest.max(r.usage.prompt());
        if let Some(rules) = r.api.rules() {
            agent.lifetimes.insert(rules.short.0);
            agent.lifetimes.extend(rules.long.map(|l| l.0));
        }
        let conv = &mut cur.conversations[k];
        let pause = r.gap_s.map(|g| (g * 1000.0).round() as i64).or_else(|| conv.last_ts.map(|t| r.ts_ms - t));
        if let Some(n) = conv.follow {
            let lifetime = r.api.ttl(conv.long_ttl).map_or(i64::MAX, |(ms, _)| ms);
            if r.trigger == Trigger::Compaction || pause.is_some_and(|p| p > lifetime) {
                agent.follows.push(n);
                conv.follow = None;
            } else {
                conv.follow = Some(n + 1);
            }
        }
        if r.trigger == Trigger::Prompt {
            // The first message of a conversation counts in its session, but has no pause.
            size.typed += 1;
            if let Some(p) = pause {
                agent.typed.push((p, r.usage.prompt()));
            }
        }
        // The lifetime the next request is judged by, as cache-misses keeps it.
        if r.usage.write_long > r.usage.write_short {
            conv.long_ttl = true;
        } else if r.usage.write_short > r.usage.write_long {
            conv.long_ttl = false;
        }
        conv.last_ts = Some(r.ts_ms);
        cur.last = Some(Last {
            ts_ms: r.ts_ms,
            conversation: k,
            pause_ms: pause,
            usd,
            context: r.prev_prompt.unwrap_or(r.usage.prompt()),
            trigger: r.trigger,
            client: r.client.to_string(),
        });
    }

    fn miss(&mut self, m: MissEvent) {
        let Pauses { agents, current, .. } = self;
        let Some(cur) = current else { return };
        let (Some(last), Cause::IdleTtl) = (cur.last.take(), m.cause) else { return };
        let Some(pause_ms) = last.pause_ms else { return };
        let agent = agents.entry(cur.agent.clone()).or_default();
        agent.returns.push(ColdReturn {
            ts_ms: last.ts_ms,
            pause_ms,
            usd: last.usd,
            premium_usd: m.premium_usd.unwrap_or(0.0),
            context: last.context,
            starter: Starter::of(last.trigger),
        });
        if let Some(size) = agent.sessions.get_mut(&cur.session) {
            size.cold_returns += 1;
        }
        let by_client = agent.cold_returns_by_client.entry(last.client).or_default();
        by_client.0 += 1;
        by_client.1 += m.premium_usd.unwrap_or(0.0);
        cur.conversations[last.conversation].follow = Some(1);
    }

    fn fact(&mut self, _: &Fact) {}
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Chain, Request, Usage};
    use crate::pricing::Pricing;

    /// A request `min` minutes in, of a conversation on the 1-hour cache: it reads `read` tokens
    /// from the cache and writes `write` with the long lifetime.
    fn req(min: f64, trigger: Trigger, read: u64, write: u64) -> Request {
        Request {
            ts_ms: (min * MINUTE as f64) as i64,
            api: crate::api::ApiId::Anthropic,
            model: "claude-opus-5-5".into(),
            usage: Usage { cache_read: read, write_long: write, output: 100, ..Usage::default() },
            trigger: Some(trigger),
            ..Request::default()
        }
    }

    fn session(id: &str, chains: Vec<(&str, ChainKind, Vec<Request>)>) -> Session {
        let mut s = Session::new(crate::testing::claude(), id.into());
        s.chains = chains.into_iter().map(|(name, kind, requests)| Chain { name: name.into(), kind, requests }).collect();
        s
    }

    fn pauses_of(sessions: &[Session]) -> AgentPauses {
        let mut p = Pauses::new(Filter::default());
        for s in sessions {
            crate::analysis::analyze(s, &Pricing::defaults(), &Filter::default(), &mut p);
        }
        p.finish();
        p.agents.remove(&crate::testing::claude()).unwrap_or_default()
    }

    use Trigger::{Compaction, Lookup, Notification, Prompt, Run};

    #[test]
    fn typed_messages_fall_in_their_pause_range() {
        let s = session(
            "s",
            vec![(
                "main",
                ChainKind::Main,
                vec![
                    req(0.0, Prompt, 0, 100_000), // the first message: no pause
                    req(1.0, Lookup, 100_000, 1_000),
                    req(8.0, Prompt, 101_000, 1_000),  // 7 minutes later
                    req(13.0, Prompt, 102_000, 1_000), // exactly 5 minutes later
                ],
            )],
        );
        let p = pauses_of(&[s]);
        assert_eq!(p.typed.len(), 2, "the first message has no pause");
        let rows = p.pause_rows();
        assert_eq!(rows.len(), PAUSE_RANGES.len(), "every range, empty ones included");
        let row = rows.iter().find(|r| r.name == "5–15m").unwrap();
        assert_eq!(row.count, 2, "7 minutes, and 5 minutes on the boundary");
        assert_eq!(row.median_context, Some(103_000), "the upper of the two middle prompts");
    }

    #[test]
    fn the_median_context_is_of_the_messages_requests() {
        let p = AgentPauses { typed: vec![(90 * MINUTE, 300_000), (95 * MINUTE, 420_000), (100 * MINUTE, 600_000)], ..AgentPauses::default() };
        let row = p.pause_rows().into_iter().find(|r| r.name == "1–2h").unwrap();
        assert_eq!((row.count, row.median_context), (3, Some(420_000)));
    }

    #[test]
    fn a_split_conversation_keeps_its_pause_across_the_chains() {
        // Two chains of one name, the second starting 10 minutes after the first ends.
        let s = session(
            "s",
            vec![
                ("main", ChainKind::Main, vec![req(0.0, Prompt, 0, 50_000), req(1.0, Lookup, 50_000, 1_000)]),
                ("main", ChainKind::Main, vec![req(11.0, Prompt, 51_000, 1_000)]),
            ],
        );
        assert_eq!(pauses_of(&[s]).typed, vec![(10 * MINUTE, 52_000)]);
    }

    #[test]
    fn shares_after_each_lifetime() {
        let typed: Vec<(i64, u64)> = (0..961)
            .map(|i| {
                (
                    if i < 485 {
                        MINUTE
                    } else if i < 793 {
                        30 * MINUTE
                    } else {
                        2 * HOUR
                    },
                    1,
                )
            })
            .collect();
        let p = AgentPauses { typed, ..AgentPauses::default() };
        assert_eq!((p.share_after(5 * MINUTE).round(), p.share_after(HOUR).round()), (50.0, 17.0));
        // Codex: 40 of 400 after 30 minutes.
        let codex = AgentPauses { typed: (0..400).map(|i| (if i < 40 { 31 * MINUTE } else { MINUTE }, 1)).collect(), ..AgentPauses::default() };
        assert_eq!(codex.share_after(30 * MINUTE), 10.0);
    }

    /// A conversation with a cold return after `hours`, started by `trigger`, then `after` warm
    /// requests.
    fn with_return(id: &str, hours: f64, trigger: Trigger, after: usize) -> Session {
        let mut requests = vec![req(0.0, Prompt, 0, 878_000)];
        let back = hours * 60.0;
        requests.push(req(back, trigger, 0, 880_000));
        for i in 0..after {
            requests.push(req(back + 1.0 + i as f64, Lookup, 880_000 + i as u64 * 1_000, 1_000));
        }
        session(id, vec![("main", ChainKind::Main, requests)])
    }

    #[test]
    fn a_return_after_the_night() {
        let p = pauses_of(&[with_return("s", 9.5, Prompt, 0)]);
        let rows = p.return_rows();
        assert_eq!(rows.len(), 1, "empty ranges are left out");
        assert_eq!((rows[0].name, rows[0].count, rows[0].median_context), ("8–16h", 1, Some(878_000)));
        // It re-wrote 880k at $8/M; the premium is that less the read price.
        assert!((rows[0].usd - (880_000.0 * 8.0 + 100.0 * 20.0) / 1e6).abs() < 1e-9, "{}", rows[0].usd);
        assert!(rows[0].premium_usd > 6.0 && rows[0].premium_usd < rows[0].usd, "{}", rows[0].premium_usd);
        assert_eq!(p.sessions["s"].cold_returns, 1);
    }

    #[test]
    fn subagents_make_no_cold_returns() {
        let mut s = with_return("s", 2.0, Prompt, 0);
        s.chains[0].kind = ChainKind::Subagent;
        let p = pauses_of(&[s]);
        assert!(p.returns.is_empty() && p.return_rows().is_empty() && p.typed.is_empty());
        assert_eq!(p.sessions["s"].largest, 0, "a subagent's prompt does not size the session");
    }

    #[test]
    fn starters_by_trigger() {
        let sessions: Vec<Session> =
            [Prompt, Prompt, Notification, Run, Lookup, Trigger::Unknown].iter().enumerate().map(|(i, t)| with_return(&format!("s{i}"), 2.0, *t, 0)).collect();
        assert_eq!(pauses_of(&sessions).starters(), [2, 1, 2, 1]);
    }

    #[test]
    fn what_followed_a_return() {
        let mut long = with_return("a", 2.0, Prompt, 6);
        // A pause longer than the lifetime ends the follow: 1 return + 6 requests.
        long.chains[0].requests.push(req(2.0 * 60.0 + 200.0, Prompt, 0, 890_000));
        let mut compacted = with_return("b", 2.0, Prompt, 0);
        // A compaction right after the return: the return counts 1 request.
        compacted.chains[0].requests.push(req(2.0 * 60.0 + 1.0, Compaction, 0, 50_000));
        let open = with_return("c", 2.0, Prompt, 2);
        let p = pauses_of(&[long, compacted, open]);
        let mut follows = p.follows.clone();
        follows.sort_unstable();
        assert_eq!(follows, vec![1, 1, 3, 7], "the return after the 200-minute pause starts its own follow of 1");
        assert_eq!(p.follow_stats(), Some((3, 1, 7, 3)), "1, 1 and 3 are at most 3 requests");
    }

    #[test]
    fn sessions_by_largest_context() {
        let big = with_return("s", 2.0, Prompt, 0);
        let mut sub = session("s", vec![("subagent", ChainKind::Subagent, vec![req(1.0, Prompt, 0, 30_000)])]);
        sub.task = Some("look around".into());
        let small = session("t", vec![("main", ChainKind::Main, vec![req(0.0, Prompt, 0, 20_000)])]);
        let p = pauses_of(&[big.clone(), sub.clone(), small.clone()]);
        let rows = p.size_rows();
        let names: Vec<&str> = rows.iter().map(|r| r.name).collect();
        assert_eq!(names, ["under 100k", "800k+"]);
        let top = &rows[1];
        assert_eq!((top.count, top.cold_returns), (1, 1));
        // The subagent's spend lands on its main session.
        let alone = pauses_of(std::slice::from_ref(&big));
        assert!(top.usd > alone.size_rows()[0].usd, "{} vs {}", top.usd, alone.size_rows()[0].usd);
        // Two partial collectors merge into the same rows as one.
        let mut a = Pauses::new(Filter::default());
        crate::analysis::analyze(&big, &Pricing::defaults(), &Filter::default(), &mut a);
        let mut b = Pauses::new(Filter::default());
        crate::analysis::analyze(&sub, &Pricing::defaults(), &Filter::default(), &mut b);
        crate::analysis::analyze(&small, &Pricing::defaults(), &Filter::default(), &mut b);
        a.merge(b);
        a.finish();
        assert_eq!(a.agents[&crate::testing::claude()].size_rows(), rows);
    }

    #[test]
    fn the_lifetimes_of_the_agents_apis() {
        let p = pauses_of(&[with_return("s", 2.0, Prompt, 0)]);
        assert_eq!(p.lifetimes.iter().copied().collect::<Vec<_>>(), [5 * MINUTE, HOUR]);
    }
}
