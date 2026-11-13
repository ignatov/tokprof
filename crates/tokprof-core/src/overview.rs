//! A period's report, as the window shows it: what `tokprof summary` and `tokprof pauses` count,
//! read in one pass, with the spend by day and by project beside them, and what to change, as
//! `tokprof share` counts it in the same pass.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::atomic::{AtomicUsize, Ordering};

use anyhow::Result;

use crate::analysis::{self, Component, Fact, MissEvent, RequestRecord, Sink};
use crate::bench;
use crate::catalog::Catalog;
use crate::model::Session;
use crate::pauses::Pauses;
use crate::pricing::Pricing;
use crate::query::Query;
use crate::report::{Collector, Spend};
use crate::rhythm;
use crate::savings::{self, BySession, Changes};
use crate::scan::{OnUnreadable, report_logs, scan_counted};
use crate::share::Share;
use crate::source::AgentId;

/// What the requests of one local date cost, by component in `Component::ALL` order.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct DaySpend {
    pub usd: [f64; 5],
    pub requests: u64,
}

impl DaySpend {
    pub fn usd(&self) -> f64 {
        self.usd.iter().sum()
    }

    fn add(&mut self, o: DaySpend) {
        for (a, b) in self.usd.iter_mut().zip(o.usd) {
            *a += b;
        }
        self.requests += o.requests;
    }
}

/// The spend of each counted request by its local date and by its session's project, and the
/// sessions that had one.
#[derive(Clone, Debug)]
pub struct Spread {
    /// By local date, as days since 1970-01-01.
    pub days: BTreeMap<i64, DaySpend>,
    pub projects: HashMap<String, Spend>,
    /// The sessions with a counted request, by agent and short id: a subagent's log counts with
    /// the session that spawned it.
    pub sessions: HashSet<(AgentId, String)>,
    /// The session being analyzed: its agent, short id and project.
    current: Option<(AgentId, String, String)>,
    /// The request counted last, which a miss sent right after it rebuilt.
    last: Option<Last>,
    offset: fn(i64) -> i64,
}

/// What a miss needs of the request it follows: its date, its time, its tokens and its prices
/// per token of cache write and uncached input.
#[derive(Clone, Copy, Debug)]
struct Last {
    date: i64,
    ts_ms: i64,
    prompt: f64,
    read: f64,
    write: f64,
    per_write: f64,
    per_input: f64,
}

impl Default for Spread {
    fn default() -> Spread {
        Spread::with_offset(rhythm::local_offset)
    }
}

impl Spread {
    /// Dates in the time zone `offset` gives, rather than the machine's.
    pub fn with_offset(offset: fn(i64) -> i64) -> Spread {
        Spread { days: BTreeMap::new(), projects: HashMap::new(), sessions: HashSet::new(), current: None, last: None, offset }
    }

    pub fn merge(&mut self, o: Spread) {
        for (d, s) in o.days {
            self.days.entry(d).or_default().add(s);
        }
        for (p, s) in o.projects {
            self.projects.entry(p).or_default().add(s);
        }
        self.sessions.extend(o.sessions);
    }

    /// Projects, most expensive first.
    pub fn projects(&self) -> Vec<(&str, Spend)> {
        let mut rows: Vec<(&str, Spend)> = self.projects.iter().map(|(p, s)| (p.as_str(), *s)).collect();
        rows.sort_by(|a, b| b.1.usd.total_cmp(&a.1.usd).then(b.1.tokens.total_cmp(&a.1.tokens)).then(a.0.cmp(b.0)));
        rows
    }
}

impl Sink for Spread {
    fn session(&mut self, s: &Session) {
        self.current = Some((s.agent.clone(), s.short_id().to_string(), s.project.clone()));
        self.last = None;
    }

    /// The request's cost split as the analysis splits it: cache read, cache write and uncached
    /// input at their prices, output at the output price. A miss right after moves what it rebuilt.
    fn request(&mut self, r: &RequestRecord) {
        let u = r.usage;
        let date = rhythm::date_of(r.ts_ms, &self.offset);
        let (mut day, mut last) = (DaySpend { requests: 1, ..DaySpend::default() }, None);
        if let Some(p) = r.price {
            let write = (u.write_short as f64 * p.write_short + u.write_long as f64 * p.write_long) / 1e6;
            day.usd[slot(Component::Read)] = u.cache_read as f64 * p.read / 1e6;
            day.usd[slot(Component::Write)] = write;
            day.usd[slot(Component::Uncached)] = u.uncached as f64 * p.input / 1e6;
            day.usd[slot(Component::Output)] = u.output as f64 * p.output / 1e6;
            let w = u.cache_write() as f64;
            let per_write = if u.cache_write() == 0 { p.write_short / 1e6 } else { write / w };
            last = Some(Last { date, ts_ms: r.ts_ms, prompt: u.prompt() as f64, read: u.cache_read as f64, write: w, per_write, per_input: p.input / 1e6 });
        }
        self.last = last;
        self.days.entry(date).or_default().add(day);
        if let Some((agent, id, project)) = &self.current {
            if !self.sessions.contains(&(agent.clone(), id.clone())) {
                self.sessions.insert((agent.clone(), id.clone()));
            }
            let s = Spend { requests: 1, tokens: (u.prompt() + u.output) as f64, usd: day.usd() };
            match self.projects.get_mut(project) {
                Some(x) => x.add(s),
                None => {
                    self.projects.insert(project.clone(), s);
                }
            }
        }
    }

    /// The tokens a miss rebuilt were written to the cache, then sent uncached; they move from
    /// those components to the rewrite at the same prices.
    fn miss(&mut self, m: MissEvent) {
        let Some(l) = self.last.take().filter(|l| l.ts_ms == m.ts_ms) else { return };
        let rebuilt = (m.rewritten as f64).min(l.prompt - l.read).max(0.0);
        let written = rebuilt.min(l.write);
        let uncached = (rebuilt - l.write).max(0.0);
        let day = self.days.entry(l.date).or_default();
        day.usd[slot(Component::Write)] -= written * l.per_write;
        day.usd[slot(Component::Uncached)] -= uncached * l.per_input;
        day.usd[slot(Component::Rewrite)] += written * l.per_write + uncached * l.per_input;
    }

    fn fact(&mut self, _: &Fact) {}
}

fn slot(c: Component) -> usize {
    Component::ALL.iter().position(|x| *x == c).unwrap_or(0)
}

/// A period's report: the summary's collector, the pauses and the spread of one pass over the
/// logs, and what the pass read.
pub struct Overview {
    pub collector: Collector,
    pub pauses: Pauses,
    pub spread: Spread,
    /// What to change, as `tokprof share` counts it.
    pub changes: Changes,
    /// The logs read, and those that could not be.
    pub logs: usize,
    pub unreadable: Vec<String>,
}

impl Overview {
    /// The summary's `Total priced`.
    pub fn spend_usd(&self) -> f64 {
        self.collector.spend.priced_usd()
    }

    /// The summary's `avoidable on cache misses`, and its share of the spend in percent as the
    /// summary prints it.
    pub fn avoidable(&self) -> (f64, f64) {
        let a = self.collector.avoidable_usd();
        (a, 100.0 * a / self.spend_usd().max(1e-9))
    }

    /// The cold returns of every agent, their count and what of their first requests was
    /// avoidable, from the totals `tokprof pauses` prints for each.
    pub fn cold_returns(&self) -> (usize, f64) {
        self.pauses.agents.values().map(|a| a.return_totals()).fold((0, 0.0), |(n, p), (count, _, premium)| (n + count, p + premium))
    }

    /// The requests the summary's table counts.
    pub fn requests(&self) -> u64 {
        self.collector.spend.models.values().map(|m| m.requests).sum()
    }

    /// The sessions with a counted request.
    pub fn sessions(&self) -> usize {
        self.spread.sessions.len()
    }

    /// Whether any model has a price, so that amounts are dollars.
    pub fn priced(&self) -> bool {
        self.collector.spend.models.values().any(|m| m.priced)
    }
}

/// Reads the logs a report reads for `select` into an overview, in one pass, calling
/// `progress(done, total)` once before the first log and after each one.
pub fn collect(catalog: &Catalog, select: &Query, pricing: &Pricing, progress: impl Fn(usize, usize) + Sync + Send) -> Result<Overview> {
    let files = report_logs(catalog, select);
    collect_logs(catalog, &files, select, pricing, |_, _| true, None, progress).map(|(o, _)| o)
}

/// Reads the logs a report reads for `select` into an overview as `collect` does, keeping only the
/// sessions whose agent, project, session id or main session id contains `text`, ignoring case,
/// as the day store's entries are kept by the window's filter text (see `calendar::Filters`).
pub fn collect_text(
    catalog: &Catalog,
    select: &Query,
    text: Option<&str>,
    pricing: &Pricing,
    progress: impl Fn(usize, usize) + Sync + Send,
) -> Result<Overview> {
    let files = report_logs(catalog, select);
    let keep = |s: &Session, f: &crate::source::SessionRef| {
        text.is_none_or(|t| crate::calendar::text_matches(t, f.agent.name(), &s.project, &f.session_id, &f.main_id))
    };
    collect_logs(catalog, &files, select, pricing, keep, None, progress).map(|(o, _)| o)
}

/// Reads `files` into an overview as `collect` does, keeping only the sessions `keep` keeps, and,
/// with `temp` (the temporary folders, see `rhythm::Timeline::of`), the rhythm's timeline of each
/// main session kept that has a request of the client `select` asks for, as `tokprof pauses` reads
/// them.
pub fn collect_logs(
    catalog: &Catalog,
    files: &[crate::source::SessionRef],
    select: &Query,
    pricing: &Pricing,
    keep: impl Fn(&Session, &crate::source::SessionRef) -> bool + Sync + Send,
    temp: Option<&[String]>,
    progress: impl Fn(usize, usize) + Sync + Send,
) -> Result<(Overview, Vec<rhythm::Timeline>)> {
    let total = files.len();
    progress(0, total);
    let done = AtomicUsize::new(0);
    let filter = select.window();
    // The sinks of `tokprof summary`, `pauses` and `share` with the spread and the habits by
    // session, the bench's scan of the main sessions, as `tokprof share` reads them, and the
    // timelines.
    type Parts = ((Collector, (Pauses, (Spread, (Share, BySession)))), bench::Found, Vec<rhythm::Timeline>);
    let (((collector, (mut pauses, (spread, (share, by_session)))), found, timelines), stats) = scan_counted(
        catalog,
        files,
        select,
        OnUnreadable::Skip,
        || {
            (
                (Collector::new(None, None), (Pauses::new(filter.clone()), (Spread::default(), (Share::new(filter.clone()), BySession::default())))),
                bench::Found::default(),
                Vec::new(),
            )
        },
        |(t, found, timelines): &mut Parts, s, f| {
            if !keep(s, f) {
                return;
            }
            analysis::analyze(s, pricing, &filter, t);
            if !f.subagent {
                *found = std::mem::take(found).merge(bench::scan(s, &filter, bench::DEFAULT_REQUESTS, bench::DEFAULT_MIN_CONTEXT));
                let client = filter.client.is_none() || s.chains.iter().flat_map(|c| &c.requests).any(|r| filter.keeps_client(r.client_name()));
                if let (Some(temp), true) = (temp, client) {
                    timelines.push(rhythm::Timeline::of(s, temp));
                }
            }
        },
        |(mut a, fa, mut ta): Parts, (b, fb, tb): Parts| {
            a.0.merge(b.0);
            a.1.0.merge(b.1.0);
            a.1.1.0.merge(b.1.1.0);
            a.1.1.1.0.merge(b.1.1.1.0);
            a.1.1.1.1.merge(b.1.1.1.1);
            ta.extend(tb);
            (a, fa.merge(fb), ta)
        },
        || progress(done.fetch_add(1, Ordering::Relaxed) + 1, total),
    )?;
    pauses.finish();
    let changes = savings::changes(&collector, &pauses, &share, found, pricing, &by_session);
    Ok((Overview { collector, pauses, spread, changes, logs: total, unreadable: stats.unreadable }, timelines))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::Filter;
    use crate::model::{Chain, ChainKind, Request, Usage};
    use crate::trigger::Trigger;

    const MINUTE: i64 = 60_000;
    const DAY: i64 = 24 * 60 * MINUTE;
    /// Midnight UTC two days before the tests' now, when the prices hold.
    const BASE: i64 = crate::TEST_NOW_MS - 12 * 60 * MINUTE - 2 * DAY;
    const D0: i64 = BASE / DAY;

    fn utc(_: i64) -> i64 {
        0
    }

    /// A request `min` minutes after `BASE` that reads `read` tokens from the cache and writes
    /// `write` with the long lifetime.
    fn req(min: i64, read: u64, write: u64, uncached: u64) -> Request {
        Request {
            ts_ms: BASE + min * MINUTE,
            api: crate::api::ApiId::Anthropic,
            model: "claude-opus-5-5".into(),
            usage: Usage { cache_read: read, write_long: write, uncached, output: 300, ..Usage::default() },
            trigger: Some(Trigger::Prompt),
            ..Request::default()
        }
    }

    fn session(id: &str, project: &str, requests: Vec<Request>) -> Session {
        let mut s = Session::new(crate::testing::claude(), id.into());
        s.project = project.into();
        s.chains = vec![Chain { name: "main".into(), kind: ChainKind::Main, requests }];
        s
    }

    /// Two sessions over two days: the second comes back after three hours to a cold cache and
    /// writes its whole context again.
    fn sessions() -> Vec<Session> {
        vec![
            session("a", "~/src/app", vec![req(600, 0, 20_000, 10), req(602, 20_000, 1_000, 5), req(604, 21_000, 800, 3)]),
            session(
                "b",
                "~/src/web",
                vec![req(DAY / MINUTE + 60, 0, 50_000, 10), req(DAY / MINUTE + 240, 0, 52_000, 10), req(DAY / MINUTE + 241, 52_000, 400, 2)],
            ),
        ]
    }

    fn collected(sessions: &[Session], filter: &Filter) -> (Collector, Spread) {
        let mut t = (Collector::new(None, None), Spread::with_offset(utc));
        for s in sessions {
            analysis::analyze(s, &Pricing::defaults(), filter, &mut t);
        }
        t
    }

    #[test]
    fn the_days_and_the_projects_sum_to_the_summary_s_total_and_columns() {
        let (c, spread) = collected(&sessions(), &Filter::default());
        let total = c.spend.priced_usd();
        assert!(total > 0.0);
        let days: f64 = spread.days.values().map(DaySpend::usd).sum();
        assert!((days - total).abs() < 1e-9, "{days} against {total}");
        let projects: f64 = spread.projects.values().map(|s| s.usd).sum();
        assert!((projects - total).abs() < 1e-9, "{projects} against {total}");
        // Each component, over the days, is the summary's column.
        let m = c.spend.models.values().next().unwrap();
        for (i, v) in m.by_component.iter().enumerate() {
            let column: f64 = spread.days.values().map(|d| d.usd[i]).sum();
            assert!((column - v.usd).abs() < 1e-9, "{:?}: {column} against {}", Component::ALL[i], v.usd);
        }
        assert_eq!(spread.days.keys().copied().collect::<Vec<_>>(), vec![D0, D0 + 1], "the requests fall on their dates");
        assert_eq!(spread.days[&D0].requests, 3);
        assert_eq!(spread.projects["~/src/web"].requests, 3);
    }

    #[test]
    fn a_miss_moves_what_it_rebuilt_into_the_rewrite() {
        let (c, spread) = collected(&sessions(), &Filter::default());
        let rewrite = slot(Component::Rewrite);
        assert_eq!(spread.days[&D0].usd[rewrite], 0.0, "the first day has no miss");
        assert!(spread.days[&(D0 + 1)].usd[rewrite] > 0.0, "the cold return rebuilt its context");
        let m = c.spend.models.values().next().unwrap();
        assert!((spread.days[&(D0 + 1)].usd[rewrite] - m.by_component[rewrite].usd).abs() < 1e-9);
    }

    #[test]
    fn sessions_count_once_with_their_subagents_and_only_with_a_counted_request() {
        let mut all = sessions();
        // A subagent's log has the id of the session that spawned it.
        all.push(session("a", "~/src/app", vec![req(610, 0, 5_000, 10)]));
        let (_, spread) = collected(&all, &Filter::default());
        assert_eq!(spread.sessions.len(), 2);
        let (_, spread) = collected(&all, &Filter { since_ms: Some(BASE + DAY), ..Filter::default() });
        assert_eq!(spread.sessions.len(), 1, "a session without a request in the window is not counted");
        assert_eq!(spread.days.keys().copied().collect::<Vec<_>>(), vec![D0 + 1]);
    }

    #[test]
    fn merged_spreads_add_up() {
        let all = sessions();
        let (_, mut a) = collected(&all[..1], &Filter::default());
        let (_, b) = collected(&all[1..], &Filter::default());
        let (_, whole) = collected(&all, &Filter::default());
        a.merge(b);
        assert_eq!(a.days, whole.days);
        assert_eq!(a.sessions, whole.sessions);
        assert_eq!(a.projects().iter().map(|(p, s)| (p.to_string(), s.requests)).collect::<Vec<_>>(), vec![("~/src/web".into(), 3), ("~/src/app".into(), 3)]);
    }
}
