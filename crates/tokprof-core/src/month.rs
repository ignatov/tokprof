//! A month in review: what one local calendar month of the day store holds, as a card someone
//! would post. It names no project, title or path: only numbers, the days, the hours, a model
//! and a miss cause.

use std::collections::{HashMap, HashSet};

use crate::analysis::cache::Cause;
use crate::calendar::{Amounts, Calendar, Day, Filters, Streak, cuts};
use crate::daystore::Entry;
use crate::rhythm::{self, Offset};

const MINUTE_MS: i64 = 60_000;
const HOUR_MS: i64 = 60 * MINUTE_MS;

/// The local hours of the night for the night-owl rule: from 22:00 to 05:00.
pub const NIGHT_HOURS: [usize; 7] = [22, 23, 0, 1, 2, 3, 4];
/// The local hours of the morning for the early-bird rule: from 05:00 to 09:00.
pub const MORNING_HOURS: [usize; 4] = [5, 6, 7, 8];
/// Sessions working at once for a minute to count as conducted.
pub const CONDUCTED_SESSIONS: u16 = 3;

/// The titles, in the order they are tried, with their thresholds, set on the author's months:
/// August 2026 night 35%, September 0.77 commits per agent hour, October a 31-day streak, so
/// that each gets another title (see the change add-month-review).
pub const NIGHT_OWL_SHARE: f64 = 0.33;
pub const EARLY_BIRD_SHARE: f64 = 0.20;
pub const CONDUCTOR_SHARE: f64 = 0.25;
pub const MARATHON_DAYS: i64 = 28;
pub const CACHE_KEEPER_SHARE: f64 = 0.05;
pub const SHIPPER_PER_HOUR: f64 = 0.75;

/// A month, as year and month number.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Ym {
    pub year: i32,
    pub month: u32,
}

impl Ym {
    /// `2026-10`.
    pub fn parse(s: &str) -> Option<Ym> {
        let (y, m) = s.split_once('-')?;
        let (year, month) = (y.parse().ok()?, m.parse().ok()?);
        (y.len() == 4 && m.len() == 2 && (1..=12).contains(&month)).then_some(Ym { year, month })
    }

    /// The month of a local date.
    pub fn of_date(date: i64) -> Ym {
        use chrono::Datelike;
        let d = naive(date);
        Ym { year: d.year(), month: d.month() }
    }

    pub fn previous(self) -> Ym {
        if self.month == 1 { Ym { year: self.year - 1, month: 12 } } else { Ym { year: self.year, month: self.month - 1 } }
    }

    pub fn next(self) -> Ym {
        if self.month == 12 { Ym { year: self.year + 1, month: 1 } } else { Ym { year: self.year, month: self.month + 1 } }
    }

    /// Its first day, as a local date (days since 1970-01-01).
    pub fn first_date(self) -> i64 {
        let d = chrono::NaiveDate::from_ymd_opt(self.year, self.month, 1).unwrap_or_default();
        (d - chrono::NaiveDate::from_ymd_opt(1970, 1, 1).unwrap_or_default()).num_days()
    }

    /// `October 2026`.
    pub fn name(self) -> String {
        naive(self.first_date()).format("%B %Y").to_string()
    }

    /// `October`.
    pub fn month_name(self) -> String {
        naive(self.first_date()).format("%B").to_string()
    }

    /// `2026-10`.
    pub fn key(self) -> String {
        format!("{:04}-{:02}", self.year, self.month)
    }
}

fn naive(date: i64) -> chrono::NaiveDate {
    chrono::DateTime::from_timestamp(date * 86_400, 0).map(|t| t.date_naive()).unwrap_or_default()
}

/// A title the month earns, and the rule it met, in words.
#[derive(Clone, Debug, PartialEq)]
pub struct Title {
    pub name: &'static str,
    pub rule: String,
}

/// The numbers of the previous month that the deltas compare.
#[derive(Clone, Debug, PartialEq)]
pub struct Brief {
    pub ym: Ym,
    /// For a month in progress, the end of the same days of the month before that it compares
    /// with; `None` for the whole month.
    pub to_ms: Option<i64>,
    pub usd: f64,
    pub agent_minutes: u64,
    pub avoidable_share: Option<f64>,
}

/// One month in review.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Month {
    pub ym: Option<Ym>,
    /// The window it counts: local midnight of the first day to that of the next month, or now.
    pub from_ms: i64,
    pub to_ms: i64,
    /// The month has ended.
    pub complete: bool,
    /// Everything in the window, summed over the calendar's days.
    pub total: Amounts,
    /// Main sessions with a request in the month.
    pub sessions: usize,
    pub cold_returns: usize,
    /// The days of the month, its first to its last, and the quartile cuts of their spend.
    pub days: Vec<Day>,
    pub cuts: Option<[f64; 3]>,
    pub busiest_day: Option<(i64, f64)>,
    /// The local hour with the most agent time, and its share of it.
    pub busiest_hour: Option<(usize, f64)>,
    /// Shares of agent time from 22:00 to 05:00 and with 3 or more sessions at once, and of time
    /// at the keyboard from 05:00 to 09:00.
    pub night_share: Option<f64>,
    pub parallel_share: Option<f64>,
    pub morning_share: Option<f64>,
    pub longest: Option<Streak>,
    /// The model with the most spend, its spend and share of the priced spend.
    pub top_model: Option<(String, f64, f64)>,
    /// The miss cause with the most avoidable premium that tokprof has advice for (every cause but
    /// the unknown one), and that premium.
    pub top_cause: Option<(Cause, f64)>,
    pub previous: Option<Brief>,
    /// The agents with requests in the month whose first request in the store comes after the
    /// month before starts (see `compare::gaps`): then there is no comparison, and this says why.
    pub before_gaps: Vec<(String, i64)>,
    /// The month before holds a request or a message.
    pub before_any: bool,
    /// The agents whose first request in the store comes after this month starts: the month holds
    /// only part of their work.
    pub own_gaps: Vec<(String, i64)>,
    /// The local date of the first request in the store of the entries kept, of any agent.
    pub first_stored: Option<i64>,
    pub title: Option<Title>,
}

impl Month {
    /// Whether anything happened in the month.
    pub fn any(&self) -> bool {
        self.total.requests > 0 || self.total.messages > 0
    }

    /// The avoidable share of spend.
    pub fn avoidable_share(&self) -> Option<f64> {
        (self.total.usd > 0.0).then(|| self.total.avoidable_usd / self.total.usd)
    }

    /// The share of prompt tokens read from the cache.
    pub fn read_share(&self) -> Option<f64> {
        (self.total.prompt_tokens > 0).then(|| self.total.read_tokens as f64 / self.total.prompt_tokens as f64)
    }

    /// The longest streak starts on the first day the store holds, so it may have begun before.
    pub fn streak_from_first_stored(&self) -> bool {
        matches!((self.longest, self.first_stored), (Some(s), Some(f)) if s.from == f)
    }

    /// Commits per hour some agent worked.
    pub fn commits_per_hour(&self) -> Option<f64> {
        (self.total.agent_minutes > 0).then(|| self.total.commits as f64 / (self.total.agent_minutes as f64 / 60.0))
    }

    /// The month of `ym` in the entries the filters keep (their time window is replaced by the
    /// month's), as of `now_ms`, with the previous month's numbers beside it.
    pub fn of(entries: &[Entry], filters: &Filters, ym: Ym, now_ms: i64, offset: Offset) -> Month {
        let mut m = Month::alone(entries, filters, ym, None, now_ms, offset);
        // A month in progress compares with the same stretch of the month before, never with
        // all of it.
        let prev_from = rhythm::midnight(ym.previous().first_date(), offset);
        let same = (!m.complete).then(|| crate::period::same_point(prev_from, (m.from_ms, m.to_ms), m.from_ms).1);
        let prev = Month::alone(entries, filters, ym.previous(), same, now_ms, offset);
        // As Reports judge a period before: incomplete when an agent's data starts inside it.
        m.before_gaps = crate::compare::gaps(entries, filters, (m.from_ms, m.to_ms), prev.from_ms);
        m.own_gaps = crate::compare::gaps(entries, filters, (m.from_ms, m.to_ms), m.from_ms);
        m.before_any = prev.any();
        if prev.any() && m.before_gaps.is_empty() {
            m.previous = Some(Brief {
                ym: ym.previous(),
                to_ms: same,
                usd: prev.total.usd,
                agent_minutes: prev.total.agent_minutes,
                avoidable_share: prev.avoidable_share(),
            });
        }
        m
    }

    /// The month alone, up to `until` when given, else to its end or now.
    fn alone(entries: &[Entry], filters: &Filters, ym: Ym, until: Option<i64>, now_ms: i64, offset: Offset) -> Month {
        let (first, next) = (ym.first_date(), ym.next().first_date());
        let from_ms = rhythm::midnight(first, offset);
        let end = rhythm::midnight(next, offset);
        let to_ms = until.unwrap_or(end).min(end).min(now_ms).max(from_ms);
        Month { ym: Some(ym), complete: end <= now_ms, ..Month::span(entries, filters, (first, next), (from_ms, to_ms), now_ms, offset) }
    }

    /// The numbers of the window `[from_ms, to_ms)` of the entries the filters keep (their own
    /// time window is replaced by it), with the calendar's cells of the dates `first..next`, as
    /// a month counts them; `ym` is not set and the span is not complete. A quarter is counted
    /// so too (see `period`).
    pub fn span(entries: &[Entry], filters: &Filters, (first, next): (i64, i64), (from_ms, to_ms): (i64, i64), now_ms: i64, offset: Offset) -> Month {
        let window = Filters { since_ms: Some(from_ms), until_ms: Some(to_ms), ..filters.clone() };
        let cal = Calendar::of(entries, &window, now_ms.max(from_ms + 1), offset);
        let mut m = Month { from_ms, to_ms, ..Month::default() };
        for d in &cal.days {
            add(&mut m.total, &d.amounts);
        }
        m.days = (first..next).map(|date| cal.day(date).cloned().unwrap_or(Day { date, ..Day::default() })).collect();
        m.cuts = cuts(m.days.iter().map(|d| d.amounts.usd));
        m.busiest_day = m.days.iter().filter(|d| d.amounts.usd > 0.0).fold(None, |b: Option<(i64, f64)>, d| match b {
            Some((_, v)) if v >= d.amounts.usd => b,
            _ => Some((d.date, d.amounts.usd)),
        });
        // The longest run of days with a request in the month.
        let mut run: Option<Streak> = None;
        for d in &m.days {
            if d.amounts.requests > 0 {
                let r = run.get_or_insert(Streak { from: d.date, to: d.date });
                r.to = d.date;
                if m.longest.is_none_or(|l| r.days() > l.days()) {
                    m.longest = Some(*r);
                }
            } else {
                run = None;
            }
        }
        // The hours of the clock, from the punchcard.
        let by_hour = |f: &dyn Fn(&Amounts) -> f64| -> [f64; 24] {
            let mut out = [0.0; 24];
            for w in &cal.punch {
                for (h, a) in w.iter().enumerate() {
                    out[h] += f(a);
                }
            }
            out
        };
        let agent = by_hour(&|a| a.agent_minutes as f64);
        let keyboard = by_hour(&|a| a.keyboard_ms as f64);
        let (agent_all, keyboard_all): (f64, f64) = (agent.iter().sum(), keyboard.iter().sum());
        if agent_all > 0.0 {
            let (h, v) = agent.iter().enumerate().fold((0, 0.0), |b, (h, &v)| if v > b.1 { (h, v) } else { b });
            m.busiest_hour = Some((h, v / agent_all));
            m.night_share = Some(NIGHT_HOURS.iter().map(|&h| agent[h]).sum::<f64>() / agent_all);
        }
        if keyboard_all > 0.0 {
            m.morning_share = Some(MORNING_HOURS.iter().map(|&h| keyboard[h]).sum::<f64>() / keyboard_all);
        }
        // The entries counted, for what the calendar does not sum.
        let inside = |ts: i64| ts >= from_ms && ts < to_ms;
        let kept: Vec<&Entry> = entries.iter().filter(|e| filters.keeps(e)).collect();
        let mut sessions: HashSet<(&str, &str)> = HashSet::new();
        let mut models: HashMap<&str, f64> = HashMap::new();
        let mut causes: HashMap<&str, f64> = HashMap::new();
        let mut at_once: HashMap<i64, u16> = HashMap::new();
        for e in &kept {
            if e.minutes.iter().any(|x| x.requests > 0 && inside(x.minute * MINUTE_MS)) {
                sessions.insert((e.agent.as_str(), e.main.as_str()));
            }
            m.cold_returns += e.cold.iter().filter(|&&t| inside(t)).count();
            for (_, model, usd) in e.models.iter().filter(|x| inside(x.0 * HOUR_MS)) {
                *models.entry(model.as_str()).or_default() += usd;
            }
            for (_, cause, usd) in e.misses.iter().filter(|x| inside(x.0 * HOUR_MS)) {
                *causes.entry(cause.as_str()).or_default() += usd;
            }
            if !e.subagent && !e.left_out {
                for &(hour, bits) in &e.work {
                    for b in (0..60).filter(|b| bits & (1 << b) != 0) {
                        let minute = hour * 60 + b;
                        if inside(minute * MINUTE_MS) {
                            *at_once.entry(minute).or_default() += 1;
                        }
                    }
                }
            }
        }
        m.sessions = sessions.len();
        if !at_once.is_empty() {
            m.parallel_share = Some(at_once.values().filter(|&&n| n >= CONDUCTED_SESSIONS).count() as f64 / at_once.len() as f64);
        }
        let priced: f64 = models.values().sum();
        m.top_model = models
            .into_iter()
            .max_by(|a, b| a.1.total_cmp(&b.1).then(b.0.cmp(a.0)))
            .filter(|(_, v)| *v > 0.0)
            .map(|(name, v)| (name.to_string(), v, v / priced.max(1e-9)));
        m.top_cause = causes
            .into_iter()
            .filter_map(|(name, v)| Cause::ALL.iter().find(|c| c.name() == name).map(|&c| (c, v)))
            // A cause tokprof has no advice for gives no line for next month.
            .filter(|(c, _)| *c != Cause::Unknown)
            .max_by(|a, b| a.1.total_cmp(&b.1).then(b.0.name().cmp(a.0.name())))
            .filter(|(_, v)| *v > 0.0);
        m.first_stored = entries
            .iter()
            .filter(|e| filters.keeps(e))
            .flat_map(|e| e.minutes.iter().filter(|x| x.requests > 0 && x.minute > 0).map(|x| x.minute * MINUTE_MS))
            .min()
            .map(|ts| rhythm::date_of(ts, offset));
        m.title = title(&m);
        m
    }
}

fn add(a: &mut Amounts, b: &Amounts) {
    a.usd += b.usd;
    a.avoidable_usd += b.avoidable_usd;
    a.requests += b.requests;
    a.agent_minutes += b.agent_minutes;
    a.keyboard_ms += b.keyboard_ms;
    a.sittings += b.sittings;
    a.messages += b.messages;
    a.commits += b.commits;
    a.pushes += b.pushes;
    a.pull_requests += b.pull_requests;
    a.read_tokens += b.read_tokens;
    a.prompt_tokens += b.prompt_tokens;
    a.saved_usd += b.saved_usd;
    a.unpriced += b.unpriced;
}

fn pct(x: f64) -> String {
    format!("{:.0}%", 100.0 * x)
}

/// A share as the card prints it, in whole percent.
fn shown_pct(x: f64) -> f64 {
    (100.0 * x).round()
}

/// The first title whose rule the month meets, in a fixed order. Each rule compares the value as
/// the card prints it: a share in whole percent (in tenths for the cache keeper), commits per
/// hour in hundredths, so that a printed `33%` meets `from 33%` and a printed `5.0%` is not
/// `under 5%`.
pub fn title(m: &Month) -> Option<Title> {
    if let Some(s) = m.night_share.filter(|&s| shown_pct(s) >= shown_pct(NIGHT_OWL_SHARE)) {
        return Some(Title { name: "Night owl", rule: format!("{} of agent time 22:00–05:00 (title from {})", pct(s), pct(NIGHT_OWL_SHARE)) });
    }
    if let Some(s) = m.morning_share.filter(|&s| shown_pct(s) >= shown_pct(EARLY_BIRD_SHARE)) {
        return Some(Title { name: "Early bird", rule: format!("{} of your keyboard time 05:00–09:00 (title from {})", pct(s), pct(EARLY_BIRD_SHARE)) });
    }
    if let Some(s) = m.parallel_share.filter(|&s| shown_pct(s) >= shown_pct(CONDUCTOR_SHARE)) {
        return Some(Title {
            name: "Conductor",
            rule: format!("{} of agent time with {CONDUCTED_SESSIONS} or more sessions at once (title from {})", pct(s), pct(CONDUCTOR_SHARE)),
        });
    }
    if let Some(s) = m.longest.filter(|s| s.days() >= MARATHON_DAYS) {
        return Some(Title { name: "Marathon", rule: format!("{} days in a row with the agents (title from {MARATHON_DAYS})", s.days()) });
    }
    if let Some(s) = m.avoidable_share().filter(|&s| (1000.0 * s).round() / 10.0 < 100.0 * CACHE_KEEPER_SHARE) {
        return Some(Title {
            name: "Cache keeper",
            rule: format!("{:.1}% of spend lost to cache misses (title under {})", 100.0 * s, pct(CACHE_KEEPER_SHARE)),
        });
    }
    if let Some(r) = m.commits_per_hour().filter(|&r| (100.0 * r).round() >= (100.0 * SHIPPER_PER_HOUR).round()) {
        return Some(Title { name: "Shipper", rule: format!("{r:.2} commits per agent hour (title from {SHIPPER_PER_HOUR:.2})") });
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::daystore::Minute;

    const CET: i64 = HOUR_MS;

    fn off(_: i64) -> i64 {
        CET
    }

    /// 2026-10-01 00:00 in a zone one hour east of UTC.
    fn oct1() -> i64 {
        rhythm::midnight(Ym { year: 2026, month: 10 }.first_date(), &off)
    }

    fn entry(minutes: Vec<Minute>) -> Entry {
        Entry { key: "/a".into(), agent: "claude".into(), session: "s".into(), main: "s".into(), minutes, ..Entry::default() }
    }

    fn minute(ts: i64, usd: f64) -> Minute {
        Minute { minute: ts / MINUTE_MS, usd, requests: 1, ..Minute::default() }
    }

    #[test]
    fn months_parse_and_step() {
        let oct = Ym::parse("2026-10").unwrap();
        assert_eq!(
            (oct.previous(), oct.next().next(), oct.name(), oct.key()),
            (Ym { year: 2026, month: 9 }, Ym { year: 2026, month: 12 }, "October 2026".into(), "2026-10".into())
        );
        assert_eq!(Ym { year: 2026, month: 1 }.previous(), Ym { year: 2025, month: 12 });
        assert!(Ym::parse("2026-13").is_none() && Ym::parse("26-10").is_none() && Ym::parse("october").is_none());
        assert_eq!(Ym::of_date(oct.first_date() + 30), oct);
    }

    #[test]
    fn the_month_counts_its_window_and_compares_the_one_before() {
        let day = 86_400_000;
        let e = entry(vec![
            minute(oct1() - 35 * day, 0.5),             // August: the store starts before September
            minute(oct1() - HOUR_MS, 7.0),              // September 30, 23:00
            minute(oct1() + 10 * HOUR_MS, 2.0),         // October 1
            minute(oct1() + 20 * day, 3.0),             // October 21
            minute(oct1() + 31 * day - MINUTE_MS, 1.0), // October 31, 23:59
            minute(oct1() + 31 * day, 9.0),             // November 1
        ]);
        let m = Month::of(&[e], &Filters::default(), Ym { year: 2026, month: 10 }, oct1() + 40 * day, &off);
        assert_eq!((m.total.usd, m.total.requests, m.days.len(), m.complete), (6.0, 3, 31, true));
        assert_eq!(m.busiest_day.map(|b| b.1), Some(3.0));
        assert_eq!(m.sessions, 1);
        let prev = m.previous.expect("September has spend");
        assert_eq!((prev.ym, prev.usd), (Ym { year: 2026, month: 9 }, 7.0));
        // This month so far: up to now.
        let so_far = Month::of(&[entry(vec![minute(oct1() + 10 * HOUR_MS, 2.0)])], &Filters::default(), Ym { year: 2026, month: 10 }, oct1() + 5 * day, &off);
        assert!(!so_far.complete && so_far.previous.is_none());
        assert_eq!(so_far.to_ms, oct1() + 5 * day);
    }

    #[test]
    fn no_comparison_when_an_agent_starts_inside_the_month_before() {
        let day = 86_400_000;
        // Codex all along; claude only from 29 September, as after Claude Code deleted its logs.
        let codex =
            Entry { agent: "codex".into(), ..entry(vec![minute(oct1() - 40 * day, 1.0), minute(oct1() - 20 * day, 1.0), minute(oct1() + 3 * day, 1.0)]) };
        let claude =
            Entry { key: "/c".into(), session: "c".into(), main: "c".into(), ..entry(vec![minute(oct1() - 2 * day, 9.0), minute(oct1() + 4 * day, 30.0)]) };
        let m = Month::of(&[codex.clone(), claude.clone()], &Filters::default(), Ym { year: 2026, month: 10 }, oct1() + 40 * day, &off);
        assert!(m.previous.is_none(), "September is incomplete, so no deltas");
        assert_eq!(m.before_gaps, vec![("claude".to_string(), oct1() - 2 * day)]);
        assert!(m.own_gaps.is_empty(), "October holds all of both");
        // September itself holds claude only from the 29th.
        let sep = Month::of(&[codex.clone(), claude], &Filters::default(), Ym { year: 2026, month: 9 }, oct1() + 40 * day, &off);
        assert_eq!(sep.own_gaps.len(), 1);
        // With codex alone, September is whole and October compares with it.
        let whole = Month::of(&[codex], &Filters::default(), Ym { year: 2026, month: 10 }, oct1() + 40 * day, &off);
        assert!(whole.previous.is_some() && whole.before_gaps.is_empty());
    }

    #[test]
    fn titles_compare_the_value_as_printed() {
        // 32.6% prints as 33%, so it meets "from 33%".
        let m = Month { night_share: Some(0.326), ..Month::default() };
        assert_eq!(title(&m).unwrap().name, "Night owl");
        assert!(title(&Month { night_share: Some(0.324), ..Month::default() }).is_none(), "32% is under");
        // 4.96% prints as 5.0%, which is not under 5%.
        let m = Month { total: Amounts { usd: 100.0, avoidable_usd: 4.96, ..Amounts::default() }, ..Month::default() };
        assert!(title(&m).is_none());
        let m = Month { total: Amounts { usd: 100.0, avoidable_usd: 4.94, ..Amounts::default() }, ..Month::default() };
        assert_eq!(title(&m).unwrap().name, "Cache keeper");
        // 0.746 commits per agent hour print as 0.75.
        let m = Month { total: Amounts { usd: 100.0, avoidable_usd: 10.0, agent_minutes: 1000, commits: 12, ..Amounts::default() }, ..Month::default() };
        assert!((m.commits_per_hour().unwrap() - 0.72).abs() < 1e-9);
        assert!(title(&m).is_none());
        let m = Month { total: Amounts { commits: 50, agent_minutes: 4020, ..m.total }, ..m };
        assert_eq!(format!("{:.2}", m.commits_per_hour().unwrap()), "0.75");
        assert_eq!(title(&m).unwrap().name, "Shipper");
    }

    #[test]
    fn a_month_in_progress_compares_with_the_same_days_before() {
        let day = 86_400_000;
        let e = entry(vec![
            minute(oct1() - 35 * day, 0.5),   // August, so the store holds all of September
            minute(oct1() - 28 * day, 4.0),   // 3 September
            minute(oct1() - 20 * day, 100.0), // 11 September: after the same days
            minute(oct1() + 2 * day, 3.0),    // 3 October
        ]);
        let now = oct1() + 8 * day + 5 * HOUR_MS;
        let m = Month::of(&[e], &Filters::default(), Ym { year: 2026, month: 10 }, now, &off);
        let p = m.previous.expect("September is whole in the store");
        assert_eq!(p.usd, 4.0, "1 to 9 September, not all of it");
        assert_eq!(p.to_ms, Some(rhythm::midnight(Ym { year: 2026, month: 9 }.first_date(), &off) + 8 * day + 5 * HOUR_MS));
    }

    #[test]
    fn titles_go_in_their_order() {
        let mut m = Month { night_share: Some(0.34), morning_share: Some(0.5), ..Month::default() };
        assert_eq!(title(&m).unwrap().name, "Night owl");
        assert_eq!(title(&m).unwrap().rule, "34% of agent time 22:00–05:00 (title from 33%)");
        m.night_share = Some(0.1);
        assert_eq!(title(&m).unwrap().name, "Early bird");
        m.morning_share = Some(0.1);
        m.parallel_share = Some(0.5);
        assert_eq!(title(&m).unwrap().name, "Conductor");
        m.parallel_share = Some(0.2);
        m.longest = Some(Streak { from: 0, to: 29 });
        assert_eq!(title(&m).unwrap().name, "Marathon");
        m.longest = Some(Streak { from: 0, to: 3 });
        m.total = Amounts { usd: 100.0, avoidable_usd: 4.0, agent_minutes: 60, commits: 3, ..Amounts::default() };
        assert_eq!(title(&m).unwrap().name, "Cache keeper");
        m.total.avoidable_usd = 10.0;
        assert_eq!(title(&m).unwrap().name, "Shipper");
        m.total.commits = 0;
        assert_eq!(title(&m), None);
    }

    #[test]
    fn models_causes_cold_returns_and_sessions_at_once() {
        let h = |d: i64| (oct1() + d * HOUR_MS) / HOUR_MS;
        let mut a = entry(vec![minute(oct1() + HOUR_MS, 5.0)]);
        a.models = vec![(h(1), "opus".into(), 4.0), (h(2), "haiku".into(), 1.0), (h(-5), "sonnet".into(), 99.0)];
        a.misses = vec![(h(1), "idle > TTL".into(), 2.0), (h(1), "model switch".into(), 0.5), (h(1), Cause::Unknown.name().into(), 9.0)];
        a.cold = vec![oct1() + HOUR_MS, oct1() - HOUR_MS];
        // Three sessions work in the same 10 minutes, then one alone for 10 more.
        let bits = (1u64 << 10) - 1;
        a.work = vec![(h(1), bits | (bits << 10))];
        let b = Entry { key: "/b".into(), session: "b".into(), main: "b".into(), work: vec![(h(1), bits)], ..entry(vec![]) };
        let c = Entry { key: "/c".into(), session: "c".into(), main: "c".into(), work: vec![(h(1), bits)], ..entry(vec![]) };
        let m = Month::of(&[a, b, c], &Filters::default(), Ym { year: 2026, month: 10 }, oct1() + 40 * 86_400_000, &off);
        assert_eq!(m.top_model, Some(("opus".into(), 4.0, 0.8)));
        assert_eq!(m.top_cause, Some((Cause::IdleTtl, 2.0)));
        assert_eq!((m.cold_returns, m.sessions), (1, 1));
        assert_eq!(m.parallel_share, Some(0.5));
        assert_eq!(m.total.agent_minutes, 20);
    }
}
