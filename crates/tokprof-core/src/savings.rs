//! What to change, over a period: what could have been saved, by cause; what the cold returns
//! cost as resumed against the other ways to come back; and the habits that cost the most. The
//! rows are those `tokprof share` prints, built the same way, from the file it writes and the
//! report over it; beside them, the sessions behind each row that can be split by session.

use std::collections::{BTreeMap, HashMap};

use crate::advice::{LOOKUP_HINT, THINKING_HINT, miss_hint};
use crate::analysis::cache::Cause;
use crate::analysis::{Fact, MissEvent, RequestRecord, Sink};
use crate::bench;
use crate::model::{BlockKind, Session, setup_source};
use crate::pauses::Pauses;
use crate::pool::{self, BenchRow, Row};
use crate::pricing::Pricing;
use crate::report::Collector;
use crate::share::{self, PROMPT_RANGES, SPAN_RANGES, Share};
use crate::source::AgentId;
use crate::trigger::Trigger;

/// The habits whose dollars can be split by session, as the habit rows of `tokprof share` name
/// them.
pub const EFFORT: &str = "effort max or xhigh";
pub const LONG_SESSIONS: &str = "sessions longer than a day";
pub const BIG_PROMPTS: &str = "prompts of 400k+";
pub const LOOKUPS: &str = "single lookups";
pub const THINKING: &str = "thinking";
pub const SETUP: &str = "setup";

/// What one session spent on the habits that are counted per request or per share of spend.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SessionHabits {
    pub effort: f64,
    pub big_prompts: f64,
    pub lookups: f64,
    pub thinking: f64,
    pub setup: f64,
}

impl SessionHabits {
    fn add(&mut self, o: SessionHabits) {
        self.effort += o.effort;
        self.big_prompts += o.big_prompts;
        self.lookups += o.lookups;
        self.thinking += o.thinking;
        self.setup += o.setup;
    }

    fn of(&self, habit: &str) -> Option<f64> {
        Some(match habit {
            EFFORT => self.effort,
            BIG_PROMPTS => self.big_prompts,
            LOOKUPS => self.lookups,
            THINKING => self.thinking,
            SETUP => self.setup,
            _ => return None,
        })
    }
}

/// The habits of each session, by agent and short id, counted as `tokprof share` counts them for
/// the whole period: a request's cost at effort max or xhigh, with a prompt of 400k or more, or
/// sent for a single lookup; the share of spend on thinking and on setup.
#[derive(Clone, Debug, Default)]
pub struct BySession {
    pub sessions: HashMap<(AgentId, String), SessionHabits>,
    current: Option<(AgentId, String)>,
}

impl BySession {
    pub fn merge(&mut self, o: BySession) {
        for (k, v) in o.sessions {
            self.sessions.entry(k).or_default().add(v);
        }
    }

    fn entry(&mut self, key: (AgentId, String)) -> &mut SessionHabits {
        self.sessions.entry(key).or_default()
    }
}

impl Sink for BySession {
    fn session(&mut self, s: &Session) {
        self.current = Some((s.agent.clone(), s.short_id().to_string()));
    }

    fn request(&mut self, r: &RequestRecord) {
        let Some(key) = self.current.clone() else { return };
        let usd = r.price.map_or(0.0, |p| p.cost(&r.usage));
        let big = PROMPT_RANGES.iter().find(|(name, _)| *name == "400–800k").is_some_and(|(_, from)| r.usage.prompt() >= *from);
        let h = SessionHabits {
            effort: if matches!(r.effort, Some("max" | "xhigh")) { usd } else { 0.0 },
            big_prompts: if big { usd } else { 0.0 },
            lookups: if r.trigger == Trigger::Lookup { usd } else { 0.0 },
            ..SessionHabits::default()
        };
        self.entry(key).add(h);
    }

    fn miss(&mut self, _: MissEvent) {}

    fn fact(&mut self, f: &Fact) {
        let usd = f.usd.unwrap_or(0.0);
        let h = SessionHabits {
            thinking: if f.kind == BlockKind::Thinking { usd } else { 0.0 },
            setup: if setup_source(f.kind, f.class, f.target).is_some() { usd } else { 0.0 },
            ..SessionHabits::default()
        };
        if h != SessionHabits::default() {
            self.entry((f.agent.clone(), f.session.to_string())).add(h);
        }
    }
}

/// A session behind a row, and what of the row's dollars it holds.
#[derive(Clone, Debug, PartialEq)]
pub struct SessionCost {
    pub agent: AgentId,
    /// The short id, as the sessions name themselves in the reports.
    pub session: String,
    pub usd: f64,
}

/// What to change over a period, as `tokprof share` prints it.
#[derive(Clone, Debug, Default)]
pub struct Changes {
    /// `What could have been saved`: a row per cause, with its share of spend and its dollars
    /// per active hour, most first.
    pub saved: Vec<Row>,
    /// `Coming back after a cold break`: the cold returns, and per way of coming back its dollars
    /// and the returns it was cheapest in; none without a cold return.
    pub returns: u64,
    pub ways: Vec<BenchRow>,
    /// `Habits that cost the most`, most first; the rows overlap.
    pub habits: Vec<Row>,
    /// The sessions behind each row that can be split by session, most first.
    pub sessions: BTreeMap<String, Vec<SessionCost>>,
    /// The active hours the rates are per.
    pub active_hours: f64,
    /// What to do about a row, in the words of tokprof's advice, by row; a row without advice of
    /// its own has none.
    pub hints: BTreeMap<String, String>,
}

impl Changes {
    /// The sessions behind a row, most first; `None` for a row that is not split by session
    /// (the cache lifetimes, the tool hints).
    pub fn sessions_of(&self, row: &str) -> Option<&[SessionCost]> {
        self.sessions.get(row).map(Vec::as_slice)
    }

    pub fn hint(&self, row: &str) -> Option<&str> {
        self.hints.get(row).map(String::as_str)
    }

    /// The way of coming back that cost least.
    pub fn cheapest(&self) -> Option<&BenchRow> {
        self.ways.iter().min_by(|a, b| a.usd.total_cmp(&b.usd))
    }
}

/// The share file of the period, without what the window does not read (the rhythm), and the
/// report over it, kept to the three tables; with the sessions behind each row.
pub fn changes(c: &Collector, p: &Pauses, s: &Share, found: bench::Found, pricing: &Pricing, by_session: &BySession) -> Changes {
    if s.span_ms.is_none() {
        return Changes::default();
    }
    let header = share::Header { tokprof: env!("CARGO_PKG_VERSION"), platform: "", survey: None, mcp_names: &[], about: &[] };
    let extra = share::Extra {
        bench: Some(bench::replay(found, pricing, bench::DEFAULT_REQUESTS, bench::DEFAULT_MIN_CONTEXT, bench::DEFAULT_KEEP_WARM_MS)),
        ..share::Extra::default()
    };
    let value = share::to_json_with(c, p, s, &header, &extra, |ms| crate::fmt::local_time(ms, "%Y-%m-%d"));
    let report = pool::build(&[("period".to_string(), value)], 1);
    let rows = |key: &str| report.table(key).map(|t| t.rows.clone()).unwrap_or_default();
    let (returns, ways) = report.bench.as_ref().map_or((0, Vec::new()), |b| (b.returns, b.rows.clone()));
    let mut sessions: BTreeMap<String, HashMap<(AgentId, String), f64>> = BTreeMap::new();
    // The misses the cause table lists, as the file counts them.
    for m in c.misses.iter().filter(|m| !m.cause.expected()) {
        *sessions.entry(m.cause.name().to_string()).or_default().entry((m.agent.clone(), m.session.clone())).or_default() += m.premium_usd.unwrap_or(0.0);
    }
    for ((agent, id), h) in &by_session.sessions {
        for habit in [EFFORT, BIG_PROMPTS, LOOKUPS, THINKING, SETUP] {
            let usd = h.of(habit).unwrap_or(0.0);
            if usd > 0.0 {
                *sessions.entry(habit.to_string()).or_default().entry((agent.clone(), id.clone())).or_default() += usd;
            }
        }
    }
    // A session longer than a day counts whole, as the file's session spans count it.
    let day = SPAN_RANGES.iter().find(|(name, _)| *name == "1–3d").map_or(i64::MAX, |(_, from)| *from);
    for (agent, a) in &p.agents {
        for (id, size) in a.sessions.iter().filter(|(_, size)| size.span_ms() >= day) {
            *sessions.entry(LONG_SESSIONS.to_string()).or_default().entry((agent.clone(), id.clone())).or_default() += size.usd;
        }
    }
    let sessions = sessions
        .into_iter()
        .map(|(row, by)| {
            let mut list: Vec<SessionCost> =
                by.into_iter().filter(|(_, usd)| *usd > 0.0).map(|((agent, session), usd)| SessionCost { agent, session, usd }).collect();
            list.sort_by(|a, b| b.usd.total_cmp(&a.usd).then_with(|| a.session.cmp(&b.session)));
            (row, list)
        })
        .collect();
    let (saved, habits) = (rows("saved"), rows("habits"));
    let hints = saved.iter().chain(&habits).filter_map(|r| Some((r.name.clone(), hint(&r.name, c, s)?))).collect();
    Changes { saved, returns, ways, habits, sessions, active_hours: report.activity.as_ref().map_or(0.0, |a| a.active_hours), hints }
}

/// What tokprof's advice says to do about a row of savings or habits: the cache-miss advice for a
/// cause, the cache lifetime settings of the agents for a lifetime, the thinking and lookup
/// advice, the setup hint of the costliest source, and a tool hint's own words.
fn hint(row: &str, c: &Collector, s: &Share) -> Option<String> {
    let sentence = |t: &str| {
        let mut out: String = t.chars().take(1).flat_map(char::to_uppercase).chain(t.chars().skip(1)).collect();
        if !out.ends_with('.') {
            out.push('.');
        }
        out
    };
    if let Some(cause) = Cause::ALL.into_iter().find(|c| c.name() == row) {
        return Some(sentence(miss_hint(cause)));
    }
    if row.starts_with("cache lifetime") {
        let lines: Vec<String> = s.agents.keys().filter_map(|a| a.info().ttl_line()).collect();
        return (!lines.is_empty()).then(|| lines.join(" "));
    }
    if let Some(tool) = row.strip_prefix("tool hint: ") {
        return Some(sentence(tool));
    }
    match row {
        EFFORT | THINKING => Some(sentence(THINKING_HINT)),
        LOOKUPS => Some(LOOKUP_HINT.to_string()),
        SETUP => c.setup.rows().into_iter().find_map(|(agent, source, _)| {
            let (_, h) = agent.info().setup_hints.iter().find(|(s, _)| s == source)?;
            Some(sentence(&format!("{} cost the most of the setup: {h}", sentence(source).trim_end_matches('.'))))
        }),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::{self, Filter};
    use crate::model::{Chain, ChainKind, Request, Usage};

    const MINUTE: i64 = 60_000;
    const BASE: i64 = crate::TEST_NOW_MS - 3 * 24 * 60 * MINUTE;

    fn req(min: i64, read: u64, write: u64, trigger: Trigger, effort: Option<&str>) -> Request {
        Request {
            ts_ms: BASE + min * MINUTE,
            api: crate::api::ApiId::Anthropic,
            model: "claude-opus-5-5".into(),
            usage: Usage { cache_read: read, write_long: write, uncached: 5, output: 300, ..Usage::default() },
            trigger: Some(trigger),
            params: effort.map(|e| crate::model::Params { effort: Some(e.into()), ..crate::model::Params::default() }),
            ..Request::default()
        }
    }

    fn session(id: &str, requests: Vec<Request>) -> Session {
        let mut s = Session::new(crate::testing::claude(), id.into());
        s.project = "~/src/app".into();
        s.chains = vec![Chain { name: "main".into(), kind: ChainKind::Main, requests }];
        s
    }

    /// Two sessions: one at effort max that comes back cold after two hours with a big prompt,
    /// one that runs for two days with lookups.
    fn sessions() -> Vec<Session> {
        use Trigger::{Lookup, Prompt};
        vec![
            session("a", vec![req(0, 0, 420_000, Prompt, Some("max")), req(2, 420_000, 1_000, Prompt, Some("max")), req(122, 0, 421_000, Prompt, Some("max"))]),
            session("b", vec![req(10, 0, 30_000, Prompt, None), req(11, 30_000, 500, Lookup, None), req(10 + 2 * 24 * 60, 30_500, 200, Lookup, None)]),
        ]
    }

    fn changes_of(sessions: &[Session]) -> Changes {
        let filter = Filter::default();
        let pricing = Pricing::defaults();
        let mut t = (Collector::new(None, None), (Pauses::new(filter.clone()), (Share::new(filter.clone()), BySession::default())));
        let mut found = bench::Found::default();
        for s in sessions {
            analysis::analyze(s, &pricing, &filter, &mut t);
            found = found.merge(bench::scan(s, &filter, bench::DEFAULT_REQUESTS, bench::DEFAULT_MIN_CONTEXT));
        }
        let (c, (mut p, (s, by))) = t;
        p.finish();
        changes(&c, &p, &s, found, &pricing, &by)
    }

    #[test]
    fn the_sessions_of_a_row_sum_to_the_row() {
        let ch = changes_of(&sessions());
        assert!(!ch.saved.is_empty() && !ch.habits.is_empty());
        let rows = ch.saved.iter().chain(&ch.habits);
        let mut split = 0;
        for row in rows {
            let Some(list) = ch.sessions_of(&row.name) else { continue };
            split += 1;
            let sum: f64 = list.iter().map(|s| s.usd).sum();
            // The file keeps dollars to four places.
            assert!((sum - row.usd).abs() < 1e-3, "{}: {sum} against {}", row.name, row.usd);
            assert!(list.windows(2).all(|w| w[0].usd >= w[1].usd), "most first");
        }
        assert!(split >= 4, "the causes and most habits are split by session");
        assert_eq!(ch.sessions_of(EFFORT).unwrap().iter().map(|s| s.session.as_str()).collect::<Vec<_>>(), ["a"]);
        assert_eq!(ch.sessions_of(LONG_SESSIONS).unwrap().iter().map(|s| s.session.as_str()).collect::<Vec<_>>(), ["b"]);
        assert_eq!(ch.sessions_of(LOOKUPS).unwrap()[0].session, "b");
        assert!(ch.sessions_of("idle > TTL").is_some_and(|l| l[0].session == "a"), "the cold return's miss");
    }

    #[test]
    fn rows_carry_the_advice_tokprof_gives_for_them() {
        let ch = changes_of(&sessions());
        assert_eq!(ch.hint("idle > TTL"), Some("Compact or wrap up a big session before a long break."));
        assert_eq!(ch.hint(EFFORT), Some("Use a lower effort for routine steps."));
        assert_eq!(ch.hint(LOOKUPS), Some(LOOKUP_HINT));
        assert_eq!(ch.hint(BIG_PROMPTS), None, "no advice of its own");
        assert_eq!(
            hint("tool hint: batch independent reads", &Collector::default(), &Share::new(Default::default())).as_deref(),
            Some("Batch independent reads.")
        );
    }

    #[test]
    fn the_ways_back_name_the_cold_return_and_the_cheapest() {
        let ch = changes_of(&sessions());
        assert_eq!(ch.returns, 1);
        assert_eq!(ch.ways.len(), 5);
        assert_eq!(ch.ways[0].of_resume, 1.0, "resume as is first");
        let cheapest = ch.cheapest().unwrap();
        assert_eq!(cheapest.cheapest, 1, "{cheapest:?}");
        assert!(ch.active_hours >= 2.0);
    }

    #[test]
    fn a_period_without_requests_has_nothing_to_change() {
        let ch = changes_of(&[]);
        assert!(ch.saved.is_empty() && ch.habits.is_empty() && ch.ways.is_empty() && ch.returns == 0);
    }
}
