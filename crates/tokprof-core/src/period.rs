//! Calendar months and quarters from the day store: a period's window from local midnight on its
//! first day, the same point of the period before, and what Month and Quarter show of it (see
//! desk-window, Month and quarter periods): the key numbers and their comparison, the cells of
//! its dates, the Monday-to-Sunday weeks that touch it, the projects by week, the records, the
//! months of a quarter and its project shifts.
//!
//! The key numbers count the window by local midnights, as `tokprof month` counts a month. The
//! cells of the dates and the weeks are calendar days, so a night across the first of the period
//! lies in a cell of the period before (`$139` of October 2026 fell in 30 September's cell).

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use crate::calendar::{self, Amounts, Calendar, Filters, Streak, weekday};
use crate::compare;
use crate::daystore::Entry;
use crate::month::{Month, Ym};
use crate::rhythm::{self, Offset};
use crate::week::{dates, dollars, monday_of};

const MINUTE_MS: i64 = 60_000;

/// The projects a quarter's weeks stack, the rest being `other`.
pub const PROJECTS_STACKED: usize = 6;
/// Project shifts: the least share of a week's spend that counts a project in that week.
pub const SHIFT_WEEK_SHARE: f64 = 0.05;
/// Project shifts: the whole weeks in a row without a request that make a pause.
pub const SHIFT_PAUSE_WEEKS: usize = 2;
/// Project shifts: the least change of share, in points, between the halves of the ended weeks.
pub const SHIFT_POINTS: f64 = 0.05;
/// Project shifts: the least ended weeks for `grew` and `fell`.
pub const SHIFT_MIN_WEEKS: usize = 4;
/// Project shifts: how long before a project's first week each agent of it must have data in the
/// store for it to have `started`, in days.
pub const SHIFT_AGENT_DAYS: i64 = 28;
/// Project shifts: the rows listed.
pub const SHIFTS_SHOWN: usize = 5;

/// A quarter, as year and number, 1 for January to March.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Yq {
    pub year: i32,
    pub q: u32,
}

impl Yq {
    /// `2026-Q4`; `2026-q4` too. `2026-Q5` and `2026-Q4x` are not quarters.
    pub fn parse(s: &str) -> Option<Yq> {
        let (y, q) = s.split_once('-')?;
        let n = q.strip_prefix('Q').or_else(|| q.strip_prefix('q'))?;
        let (year, q) = (y.parse().ok()?, n.parse().ok()?);
        (y.len() == 4 && y.bytes().all(|b| b.is_ascii_digit()) && n.len() == 1 && (1..=4).contains(&q)).then_some(Yq { year, q })
    }

    /// The quarter of a month.
    pub fn of(ym: Ym) -> Yq {
        Yq { year: ym.year, q: (ym.month - 1) / 3 + 1 }
    }

    /// Its three months.
    pub fn months(self) -> [Ym; 3] {
        let first = 3 * (self.q - 1) + 1;
        [0, 1, 2].map(|i| Ym { year: self.year, month: first + i })
    }

    pub fn previous(self) -> Yq {
        if self.q == 1 { Yq { year: self.year - 1, q: 4 } } else { Yq { year: self.year, q: self.q - 1 } }
    }

    pub fn next(self) -> Yq {
        if self.q == 4 { Yq { year: self.year + 1, q: 1 } } else { Yq { year: self.year, q: self.q + 1 } }
    }

    /// `Q4 2026`.
    pub fn name(self) -> String {
        format!("Q{} {}", self.q, self.year)
    }

    /// `2026-Q4`.
    pub fn key(self) -> String {
        format!("{:04}-Q{}", self.year, self.q)
    }
}

/// A calendar month or quarter.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Period {
    Month(Ym),
    Quarter(Yq),
}

impl Period {
    /// `2026-10` or `2026-Q4`.
    pub fn parse(s: &str) -> Option<Period> {
        Ym::parse(s).map(Period::Month).or_else(|| Yq::parse(s).map(Period::Quarter))
    }

    /// The month of a local date.
    pub fn month_of(date: i64) -> Period {
        Period::Month(Ym::of_date(date))
    }

    /// The quarter of a local date.
    pub fn quarter_of(date: i64) -> Period {
        Period::Quarter(Yq::of(Ym::of_date(date)))
    }

    /// The month or the quarter, as this one is, that holds `now_ms`.
    pub fn current(self, now_ms: i64, offset: Offset) -> Period {
        let today = rhythm::date_of(now_ms, offset);
        match self {
            Period::Month(_) => Period::month_of(today),
            Period::Quarter(_) => Period::quarter_of(today),
        }
    }

    /// The quarter that holds this period.
    pub fn quarter(self) -> Period {
        match self {
            Period::Month(ym) => Period::Quarter(Yq::of(ym)),
            q => q,
        }
    }

    /// Its months.
    pub fn months(self) -> Vec<Ym> {
        match self {
            Period::Month(ym) => vec![ym],
            Period::Quarter(yq) => yq.months().to_vec(),
        }
    }

    /// The last of its months that has begun at `now_ms`, else its first: the month Month shows
    /// when the user comes from Quarter.
    pub fn last_begun_month(self, now_ms: i64, offset: Offset) -> Ym {
        let months = self.months();
        months.iter().rev().copied().find(|ym| rhythm::midnight(ym.first_date(), offset) <= now_ms).unwrap_or(months[0])
    }

    pub fn previous(self) -> Period {
        match self {
            Period::Month(ym) => Period::Month(ym.previous()),
            Period::Quarter(yq) => Period::Quarter(yq.previous()),
        }
    }

    pub fn next(self) -> Period {
        match self {
            Period::Month(ym) => Period::Month(ym.next()),
            Period::Quarter(yq) => Period::Quarter(yq.next()),
        }
    }

    /// Its first date, as days since 1970-01-01.
    pub fn first_date(self) -> i64 {
        self.months()[0].first_date()
    }

    /// The first date of the next period.
    pub fn next_date(self) -> i64 {
        self.next().first_date()
    }

    /// Its last date.
    pub fn last_date(self) -> i64 {
        self.next_date() - 1
    }

    /// How many dates it has: 28 to 31, or 90 to 92.
    pub fn days(self) -> i64 {
        self.next_date() - self.first_date()
    }

    /// `November 2026`, `Q4 2026`.
    pub fn name(self) -> String {
        match self {
            Period::Month(ym) => ym.name(),
            Period::Quarter(yq) => yq.name(),
        }
    }

    /// The name month-review's notes give the period before: `September`, `Q3 2026`.
    pub fn short_name(self) -> String {
        match self {
            Period::Month(ym) => ym.month_name(),
            Period::Quarter(yq) => yq.name(),
        }
    }

    /// `2026-10`, `2026-Q4`.
    pub fn key(self) -> String {
        match self {
            Period::Month(ym) => ym.key(),
            Period::Quarter(yq) => yq.key(),
        }
    }

    /// Local midnight on its first date, and on the next period's first date.
    pub fn bounds(self, offset: Offset) -> (i64, i64) {
        (rhythm::midnight(self.first_date(), offset), rhythm::midnight(self.next_date(), offset))
    }

    /// Its window as of `now_ms`: from its start to its end, or to now while it runs.
    pub fn window(self, now_ms: i64, offset: Offset) -> (i64, i64) {
        let (from, end) = self.bounds(offset);
        (from, end.min(now_ms).max(from))
    }

    /// It has begun at `now_ms`.
    pub fn begun(self, now_ms: i64, offset: Offset) -> bool {
        self.bounds(offset).0 <= now_ms
    }

    /// It has ended at `now_ms`.
    pub fn complete(self, now_ms: i64, offset: Offset) -> bool {
        self.bounds(offset).1 <= now_ms
    }

    /// The window it compares with as of `now_ms`: the whole period before when it has ended,
    /// else the period before for as long as this one has run (see `same_point`).
    pub fn before(self, now_ms: i64, offset: Offset) -> (i64, i64) {
        let (prev_from, prev_end) = self.previous().bounds(offset);
        if self.complete(now_ms, offset) {
            return (prev_from, prev_end);
        }
        same_point(prev_from, self.window(now_ms, offset), prev_end)
    }

    /// The Mondays of the weeks that touch it, the first on or before its first date.
    pub fn mondays(self) -> Vec<i64> {
        (monday_of(self.first_date())..=self.last_date()).step_by(7).collect()
    }
}

/// The same point of the period before: from `prev_from` for as long as `[from, to)` runs, never
/// past `prev_end`. 12 Nov 15:00 gives 1 Oct 00:00 to 12 Oct 15:00; 31 Mar 10:00 all of February.
pub fn same_point(prev_from: i64, (from, to): (i64, i64), prev_end: i64) -> (i64, i64) {
    (prev_from, (prev_from + (to - from).max(0)).min(prev_end))
}

/// How many days a date of `ym` lies after the date of the month before it faces on Spend by
/// day's line: from the month's first Monday to the month before's, 28 or 35.
pub fn weekday_shift(ym: Ym) -> i64 {
    let first_monday = |d: i64| d + (7 - weekday(d) as i64) % 7;
    first_monday(ym.first_date()) - first_monday(ym.previous().first_date())
}

/// A moment in local time, as `format` writes it.
fn local(ts: i64, offset: Offset, format: &str) -> String {
    chrono::DateTime::from_timestamp_millis(ts + offset(ts)).map(|t| t.naive_utc().format(format).to_string()).unwrap_or_default()
}

/// The seven key numbers of a window, with the miss causes and what else tells whether anything
/// happened in it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct KeyNumbers {
    pub usd: f64,
    /// The minutes some agent worked, each once (the calendar's `agent` metric).
    pub agent_minutes: u64,
    pub keyboard_ms: i64,
    /// The main sessions with a request.
    pub sessions: usize,
    pub avoidable_usd: f64,
    pub cold: usize,
    pub saved_usd: f64,
    pub commits: u64,
    pub requests: u64,
    pub messages: u64,
    /// The avoidable dollars of each unexpected miss cause, by name.
    pub causes: BTreeMap<String, f64>,
}

impl KeyNumbers {
    fn of(m: &Month, causes: BTreeMap<String, f64>) -> KeyNumbers {
        let t = &m.total;
        KeyNumbers {
            usd: t.usd,
            agent_minutes: t.agent_minutes,
            keyboard_ms: t.keyboard_ms,
            sessions: m.sessions,
            avoidable_usd: t.avoidable_usd,
            cold: m.cold_returns,
            saved_usd: t.saved_usd,
            commits: t.commits,
            requests: t.requests,
            messages: t.messages,
            causes,
        }
    }

    /// Anything happened: a request, a miss, a cold return or a message.
    pub fn any(&self) -> bool {
        self.requests > 0 || self.messages > 0 || self.cold > 0 || !self.causes.is_empty()
    }

    /// The avoidable share of the spend.
    pub fn avoidable_share(&self) -> Option<f64> {
        (self.usd > 0.0).then(|| self.avoidable_usd / self.usd)
    }
}

/// What the key numbers compare with.
#[derive(Clone, Debug, PartialEq)]
pub enum Before {
    /// The period before, or its same point, with the agents too small to judge by.
    Against { window: (i64, i64), partial: bool, numbers: Box<KeyNumbers>, minor: Vec<(String, i64, f64)> },
    /// The period before holds no request, miss, cold return or message.
    NoLogs((i64, i64)),
    /// An agent with 10% or more of the period's spend has its first request in the store after
    /// the period before starts.
    Incomplete { window: (i64, i64), gaps: Vec<(String, i64)> },
    /// `--since` or `--until` cuts the period or the period before; the note names it.
    Within(String),
}

impl Before {
    /// The numbers compared with, when there is a comparison.
    pub fn numbers(&self) -> Option<&KeyNumbers> {
        match self {
            Before::Against { numbers, .. } => Some(numbers),
            _ => None,
        }
    }

    /// The band's note about the period before `before` (the period shown's `previous()`):
    /// `against 1–12 Oct, to 15:00`, `…; opencode only from 5 Nov, 3% of spend, not judged`, `no
    /// logs in September 2026`, `no comparison with September: claude's logs start 29 Sep`,
    /// `within --since 2026-10-09`.
    pub fn note(&self, before: Period, offset: Offset) -> String {
        let span = |(from, to): (i64, i64)| dates(rhythm::date_of(from, offset), rhythm::date_of((to - 1).max(from), offset), false);
        let day = |ts: i64| local(ts, offset, "%-d %b");
        match self {
            Before::Against { window, partial, minor, .. } => {
                let mut note = format!("against {}", span(*window));
                if *partial {
                    note += &format!(", to {}", local(window.1, offset, "%H:%M"));
                }
                for (agent, ts, share) in minor {
                    note += &format!("; {agent} only from {}, {:.0}% of spend, not judged", day(*ts), 100.0 * share);
                }
                note
            }
            Before::NoLogs(_) => format!("no logs in {}", before.name()),
            Before::Incomplete { gaps, .. } => {
                let starts = gaps.iter().map(|(a, ts)| format!("{a}'s logs start {}", day(*ts))).collect::<Vec<_>>().join(", ");
                format!("no comparison with {}: {starts}", before.short_name())
            }
            Before::Within(cut) => format!("within {cut}"),
        }
    }
}

/// What a week of a quarter is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WeekKind {
    /// Its dates inside the period are all before the store's first request: `no logs`.
    NoLogs,
    /// It has not begun: a bare baseline that opens nothing.
    Future,
    /// It holds today: `so far`.
    SoFar,
    /// It is over.
    Ended,
}

/// A Monday-to-Sunday week that touches the period, cut at its bounds: the calendar cells of its
/// dates inside the period, summed.
#[derive(Clone, Debug, PartialEq)]
pub struct WeekSum {
    pub monday: i64,
    /// Its first and last dates inside the period: `Thu–Sun` for the week of 28 Sep in Q4 2026.
    pub first: i64,
    pub last: i64,
    pub kind: WeekKind,
    pub amounts: Amounts,
    pub cold: usize,
}

/// The three weekly trends of a quarter, at a week with requests.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Trend {
    /// Prompt tokens read from the cache over all prompt tokens.
    pub read_share: Option<f64>,
    /// Avoidable spend over spend.
    pub avoidable_share: Option<f64>,
    /// All prompt tokens over all requests.
    pub average_context: f64,
}

impl WeekSum {
    /// Its dates inside the period are not the whole week.
    pub fn cut(&self) -> bool {
        self.first > self.monday || self.last < self.monday + 6
    }

    /// Whether a click on its column opens Week.
    pub fn opens(&self) -> bool {
        matches!(self.kind, WeekKind::SoFar | WeekKind::Ended)
    }

    /// Its point on the trend lines; a week without requests has none.
    pub fn trend(&self) -> Option<Trend> {
        let a = &self.amounts;
        (a.requests > 0 && self.opens()).then(|| Trend {
            read_share: (a.prompt_tokens > 0).then(|| a.read_tokens as f64 / a.prompt_tokens as f64),
            avoidable_share: (a.usd > 0.0).then(|| a.avoidable_usd / a.usd),
            average_context: a.prompt_tokens as f64 / a.requests as f64,
        })
    }
}

/// A project of the period: its spend, and its spend and requests in each week of `weeks`.
#[derive(Clone, Debug, PartialEq)]
pub struct ProjectWeeks {
    pub name: String,
    pub usd: f64,
    pub weeks: Vec<f64>,
    pub requests: Vec<u64>,
}

/// The records of a period, from the day store; a tie goes to the earliest.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Records {
    /// The date with the most spend.
    pub biggest_day: Option<(i64, f64)>,
    /// The Monday-to-Friday date with the least spend above zero.
    pub quietest_weekday: Option<(i64, f64)>,
    /// The date with the most cold returns.
    pub most_cold: Option<(i64, usize)>,
    /// The longest run of days with a request inside the period, and whether it ends today.
    pub streak: Option<(Streak, bool)>,
    /// The local hour with the most agent time, and its share of it; the share of agent time
    /// from 22:00 to 05:00 (as month-review counts them).
    pub busiest_hour: Option<(usize, f64)>,
    pub night_share: Option<f64>,
    /// The week with the most spend, by its Monday.
    pub biggest_week: Option<(i64, f64)>,
    /// The most main sessions whose agents worked in one minute, and the first such minute.
    pub most_at_once: Option<(u32, i64)>,
    /// The week with the highest share of prompt tokens read from the cache.
    pub best_cache_week: Option<(i64, f64)>,
}

impl Records {
    /// The streak as the records read it: `31 days, Thu 1 Oct – Sat 31 Oct`, `12 days, since Sun
    /// 1 Nov, still going`, `1 day, Tue 3 Nov`.
    pub fn streak_text(&self) -> Option<String> {
        let (s, going) = self.streak?;
        let day = |d: i64| naive(d).format("%a %-d %b").to_string();
        let days = crate::fmt::count(s.days() as u64, "day", "days");
        Some(if going {
            format!("{days}, since {}, still going", day(s.from))
        } else if s.days() == 1 {
            format!("{days}, {}", day(s.from))
        } else {
            format!("{days}, {} – {}", day(s.from), day(s.to))
        })
    }
}

/// How a project moved over a quarter's ended weeks.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ShiftKind {
    Started,
    Paused,
    Grew,
    Fell,
}

impl ShiftKind {
    pub fn word(self) -> &'static str {
        match self {
            ShiftKind::Started => "started",
            ShiftKind::Paused => "paused",
            ShiftKind::Grew => "grew",
            ShiftKind::Fell => "fell",
        }
    }
}

/// A row of Project shifts.
#[derive(Clone, Debug, PartialEq)]
pub struct Shift {
    pub project: String,
    pub kind: ShiftKind,
    /// Its spend in the quarter.
    pub usd: f64,
    /// For `started`, the Monday of its first week; for `paused`, the first and the last date of
    /// the weeks without a request; for `grew` and `fell`, its share of spend in the first half
    /// of the ended weeks and in the second.
    pub week: Option<i64>,
    pub dates: Option<(i64, i64)>,
    pub shares: Option<(f64, f64)>,
}

impl Shift {
    /// The row's two texts: `swing-rune started`, `week of 2 Nov`; `airpods-motion paused`,
    /// `26 Oct – 8 Nov`; `air fell`, `12% → 6%`. The project's name is the caller's.
    pub fn detail(&self) -> String {
        let day = |d: i64| naive(d).format("%-d %b").to_string();
        match (self.week, self.dates, self.shares) {
            (Some(m), _, _) => format!("week of {}", day(m)),
            (_, Some((a, b)), _) => dates(a, b, false),
            (_, _, Some((a, b))) => format!("{:.0}% → {:.0}%", 100.0 * a, 100.0 * b),
            _ => String::new(),
        }
    }
}

/// The text of Project shifts when none holds.
pub const NO_SHIFTS: &str = "No project started, paused or moved 5 points this quarter.";

fn naive(date: i64) -> chrono::NaiveDate {
    chrono::DateTime::from_timestamp(date * 86_400, 0).map(|t| t.date_naive()).unwrap_or_default()
}

/// What Month and Quarter show of a period from the day store.
#[derive(Clone, Debug, PartialEq)]
pub struct PeriodSums {
    pub period: Period,
    /// What the numbers count: the period's window, to now while it runs, cut by `--since` and
    /// `--until`.
    pub from_ms: i64,
    pub to_ms: i64,
    pub complete: bool,
    pub key: KeyNumbers,
    /// The numbers of the window as `tokprof month` counts a month: the cells of its dates
    /// (`days`) with the cuts of the Month in review card, the busiest day and hour, the night
    /// share, the longest streak.
    pub span: Month,
    /// The cold returns of each date of `span.days`, by the same days.
    pub cold_by_date: Vec<usize>,
    /// The current day by the calendar's rule while the period runs.
    pub today: Option<i64>,
    /// The local date of the store's first request of the agents the filters keep, of any
    /// project: the dates before it read `no logs`.
    pub first_data: Option<i64>,
    /// The weeks that touch the period, Monday first.
    pub weeks: Vec<WeekSum>,
    /// Every project with a request in the weeks, the most spend first.
    pub projects: Vec<ProjectWeeks>,
    /// The months of a quarter that have begun, each as `tokprof month` counts it; empty for a
    /// month.
    pub months: Vec<Month>,
    pub before: Before,
    /// The days the pace per day counts over, when there is no comparison.
    pub pace_days: Option<i64>,
    pub records: Records,
    /// A quarter's project shifts, at most `SHIFTS_SHOWN`; empty for a month.
    pub shifts: Vec<Shift>,
    /// A month's line through the month before (Spend by day): the spend of each calendar cell
    /// of the whole month before, by date, first to last, from the date of the store's first
    /// request on; empty for a quarter and when the key numbers have no comparison.
    pub before_cells: Vec<(i64, f64)>,
}

impl PeriodSums {
    /// The period of the entries `filters` keeps, its window cut by their `--since` and
    /// `--until`, as of `now_ms`.
    pub fn of(entries: &[Entry], filters: &Filters, period: Period, now_ms: i64, offset: Offset) -> PeriodSums {
        let (start, end) = period.bounds(offset);
        let cut = |(a, b): (i64, i64)| (filters.since_ms.map_or(a, |s| s.max(a)), filters.until_ms.map_or(b, |u| u.min(b)).max(a));
        let window = period.window(now_ms, offset);
        let (from_ms, to_ms) = cut(window);
        let to_ms = to_ms.max(from_ms);
        let (first, next) = (period.first_date(), period.next_date());
        let complete = end <= now_ms;
        let span = Month::span(entries, filters, (first, next), (from_ms, to_ms), now_ms, offset);
        let causes = compare::Totals::of(entries, filters, (from_ms, to_ms)).causes;
        let key = KeyNumbers::of(&span, causes);
        let kept: Vec<&Entry> = entries.iter().filter(|e| filters.keeps(e)).collect();

        // The cold returns by date, by the days of the cells.
        let rule = calendar::rule_of(kept.iter().copied(), to_ms.min(now_ms.max(from_ms + 1)), offset);
        let mut cold_by_date = vec![0; span.days.len()];
        for e in &kept {
            for &ts in e.cold.iter().filter(|&&t| t > 0 && t >= from_ms && t < to_ms) {
                let d = rule.date_of(ts, offset) - first;
                if (0..cold_by_date.len() as i64).contains(&d) {
                    cold_by_date[d as usize] += 1;
                }
            }
        }

        // The store's history per agent: the project, session and text filters do not move it.
        let agent_kept = |e: &Entry| filters.agents.is_empty() || filters.agents.contains(&e.agent);
        let first_request = |e: &Entry| e.minutes.iter().filter(|m| m.requests > 0 && m.minute > 0).map(|m| m.minute * MINUTE_MS).min();
        let first_data = entries.iter().filter(|e| agent_kept(e)).filter_map(first_request).min().map(|ts| rhythm::date_of(ts, offset));

        // The weeks: the calendar's cells as Week has them, as of now, cut by the window's filters.
        let cal = Calendar::span(entries, filters, (first, period.last_date()), now_ms, offset);
        // The current day by the calendar's rule, while the period runs.
        let today = (!complete && start <= now_ms).then_some(cal.last);
        let week_rule = calendar::rule_of(kept.iter().copied(), filters.until_ms.map_or(now_ms, |u| u.min(now_ms)), offset);
        let mondays = period.mondays();
        let mut weeks: Vec<WeekSum> = mondays
            .iter()
            .map(|&monday| {
                let (a, b) = (monday.max(first), (monday + 6).min(next - 1));
                let mut amounts = Amounts::default();
                for d in a..=b {
                    if let Some(day) = cal.day(d) {
                        add(&mut amounts, &day.amounts);
                    }
                }
                let kind = if first_data.is_none_or(|f| b < f) {
                    WeekKind::NoLogs
                } else if cal.day(a).is_none() {
                    WeekKind::Future
                } else if today.is_some_and(|t| t >= a && t <= b) {
                    WeekKind::SoFar
                } else {
                    WeekKind::Ended
                };
                WeekSum { monday, first: a, last: b, kind, amounts, cold: 0 }
            })
            .collect();
        let inside = |ts: i64| ts > 0 && filters.since_ms.is_none_or(|s| ts >= s) && filters.until_ms.is_none_or(|u| ts < u);
        let week_of = |date: i64| (date >= first && date < next).then(|| ((date - monday_of(first)) / 7) as usize);
        let mut by_project: HashMap<&str, (Vec<f64>, Vec<u64>)> = HashMap::new();
        for e in &kept {
            for &ts in e.cold.iter().filter(|&&t| inside(t)) {
                if let Some(w) = week_of(week_rule.date_of(ts, offset)) {
                    weeks[w].cold += 1;
                }
            }
            for m in e.minutes.iter().filter(|m| inside(m.minute * MINUTE_MS)) {
                let Some(w) = week_of(week_rule.date_of(m.minute * MINUTE_MS, offset)) else { continue };
                let p = by_project.entry(e.project.as_str()).or_insert_with(|| (vec![0.0; mondays.len()], vec![0; mondays.len()]));
                p.0[w] += m.usd;
                p.1[w] += m.requests as u64;
            }
        }
        let mut projects: Vec<ProjectWeeks> = by_project
            .into_iter()
            .filter(|(_, (_, r))| r.iter().any(|&n| n > 0))
            .map(|(name, (usd, requests))| ProjectWeeks { name: name.to_string(), usd: usd.iter().sum(), weeks: usd, requests })
            .collect();
        projects.sort_by(|a, b| b.usd.total_cmp(&a.usd).then_with(|| a.name.cmp(&b.name)));

        let months = match period {
            Period::Month(_) => Vec::new(),
            Period::Quarter(_) => period
                .months()
                .into_iter()
                .filter(|ym| rhythm::midnight(ym.first_date(), offset) <= now_ms)
                .map(|ym| Month::of(entries, filters, ym, now_ms, offset))
                .collect(),
        };

        let before = before(entries, filters, period, (from_ms, to_ms), now_ms, offset);
        let first_kept = kept.iter().filter_map(|e| first_request(e)).min().map(|ts| rhythm::date_of(ts, offset));
        let pace_days = match before {
            Before::Against { .. } => None,
            _ => {
                let since = filters.since_ms.map(|s| rhythm::date_of(s, offset));
                let from = [Some(first), first_kept, since].into_iter().flatten().max().unwrap_or(first);
                let last = (next - 1).min(rhythm::date_of((to_ms - 1).max(from_ms), offset));
                (last >= from && to_ms > from_ms).then_some(last - from + 1)
            }
        };

        let peak = most_at_once(&kept, (from_ms, to_ms));
        let records = records(&span, &cold_by_date, &weeks, today, peak);
        let shifts = match period {
            Period::Month(_) => Vec::new(),
            Period::Quarter(_) => shifts(entries, filters, &kept, &weeks, &projects, offset),
        };
        // Only against a period before that is compared, and only over its dates with logs: a
        // date before the store's first request is no zero.
        let before_cells = match period {
            Period::Month(_) if before.numbers().is_some() => {
                let prev = period.previous();
                let cells = Calendar::span(entries, filters, (prev.first_date(), prev.last_date()), now_ms, offset);
                let from = first_data.map_or(prev.first_date(), |f| f.max(prev.first_date()));
                (from..=prev.last_date()).map(|d| (d, cells.day(d).map_or(0.0, |x| x.amounts.usd))).collect()
            }
            _ => Vec::new(),
        };
        PeriodSums {
            period,
            from_ms,
            to_ms,
            complete,
            key,
            span,
            cold_by_date,
            today,
            first_data,
            weeks,
            projects,
            months,
            before,
            pace_days,
            records,
            shifts,
            before_cells,
        }
    }

    /// The date of the month before that `date` faces on Spend by day's line, aligned by weekday
    /// (see `weekday_shift`); none where it falls outside the month before.
    pub fn facing(&self, date: i64) -> Option<i64> {
        let Period::Month(ym) = self.period else { return None };
        let to = date - weekday_shift(ym);
        let prev = self.period.previous();
        (prev.first_date()..=prev.last_date()).contains(&to).then_some(to)
    }

    /// The spend of the date `date` faces on the line through the month before: none where it
    /// faces no date, where that date lies before the store's first request, or when the key
    /// numbers have no comparison and there is no line.
    pub fn before_at(&self, date: i64) -> Option<f64> {
        let f = self.facing(date)?;
        self.before_cells.iter().find(|c| c.0 == f).map(|c| c.1)
    }

    /// A number over the days the period has run, when there is no comparison.
    pub fn pace(&self, v: f64) -> Option<f64> {
        self.pace_days.map(|d| v / d as f64)
    }

    /// The pace of dollars: `$375 a day`.
    pub fn pace_usd(&self, usd: f64) -> Option<String> {
        self.pace(usd).map(|v| format!("{} a day", dollars(v)))
    }

    /// What saved by the cache shows with no comparison: `7.5× the spend`.
    pub fn saved_multiple(&self) -> Option<String> {
        (self.pace_days.is_some() && self.key.usd > 0.0).then(|| format!("{:.1}× the spend", self.key.saved_usd / self.key.usd))
    }

    /// The miss causes of the period before that have no dollars in the period shown, most
    /// first; none without a comparison.
    pub fn gone_causes(&self) -> Vec<(String, f64)> {
        let Some(before) = self.before.numbers() else { return Vec::new() };
        let mut gone: Vec<(String, f64)> = before
            .causes
            .iter()
            .filter(|(c, v)| **v >= 0.005 && self.key.causes.get(*c).is_none_or(|now| *now < 0.005))
            .map(|(c, v)| (c.clone(), *v))
            .collect();
        gone.sort_by(|a, b| b.1.total_cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        gone
    }

    /// The projects the weeks stack, and the spend of the rest in each week.
    pub fn stacked(&self) -> (&[ProjectWeeks], Vec<f64>) {
        let n = self.projects.len().min(PROJECTS_STACKED);
        let mut other = vec![0.0; self.weeks.len()];
        for p in &self.projects[n..] {
            for (o, v) in other.iter_mut().zip(&p.weeks) {
                *o += v;
            }
        }
        (&self.projects[..n], other)
    }

    /// The days a month of this period has run as of `now_ms`, from the later of its first date
    /// and the store's first request, today whole; `None` before it begins.
    pub fn month_days(&self, m: &Month, now_ms: i64, offset: Offset) -> Option<i64> {
        let ym = m.ym?;
        let from = self.first_data.map_or(ym.first_date(), |f| f.max(ym.first_date()));
        let last = (ym.next().first_date() - 1).min(rhythm::date_of(now_ms, offset));
        (last >= from).then_some(last - from + 1)
    }

    /// The line under a quarter's Months card: `November runs $330 a day against October's $375
    /// (−12%)`, comparing the last month that has begun with the one before it in the quarter.
    pub fn months_line(&self, now_ms: i64, offset: Offset) -> Option<String> {
        let [.., a, b] = self.months.as_slice() else { return None };
        let pace = |m: &Month| self.month_days(m, now_ms, offset).map(|d| m.total.usd / d as f64);
        let (now, was) = (pace(b)?, pace(a)?);
        let change = if was > 0.0 { format!(" ({})", signed_pct((now - was) / was)) } else { String::new() };
        let (ya, yb) = (a.ym?, b.ym?);
        Some(format!("{} runs {} a day against {}'s {}{change}", yb.month_name(), dollars(now), ya.month_name(), dollars(was)))
    }
}

/// `+12%`, `−12%`, `0%`.
fn signed_pct(x: f64) -> String {
    let p = (100.0 * x).round();
    if p > 0.0 {
        format!("+{p:.0}%")
    } else if p < 0.0 {
        format!("−{:.0}%", -p)
    } else {
        "0%".to_string()
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

/// The comparison of the period, by the rules of An incomplete period before and A small agent
/// in the period before.
fn before(entries: &[Entry], filters: &Filters, period: Period, shown: (i64, i64), now_ms: i64, offset: Offset) -> Before {
    let window = period.before(now_ms, offset);
    let day = |ts: i64| local(ts, offset, "%Y-%m-%d");
    let since = filters.since_ms.filter(|&s| s > window.0).map(|s| format!("--since {}", day(s)));
    let until = filters.until_ms.filter(|&u| u < period.window(now_ms, offset).1).map(|u| format!("--until {}", day(u)));
    match (since, until) {
        (Some(s), Some(u)) => return Before::Within(format!("{s} {u}")),
        (Some(c), None) | (None, Some(c)) => return Before::Within(c),
        (None, None) => {}
    }
    let prev = period.previous();
    let span = Month::span(entries, filters, (prev.first_date(), prev.next_date()), window, now_ms, offset);
    let numbers = KeyNumbers::of(&span, compare::Totals::of(entries, filters, window).causes);
    // No logs: the store's history of the agents kept holds nothing then. The project, session
    // and text filters do not make logs missing: a project new this month compares with nothing.
    if !stored_any(entries, filters, window) {
        return Before::NoLogs(window);
    }
    let gaps = compare::gaps(entries, filters, shown, window.0);
    if !gaps.is_empty() {
        return Before::Incomplete { window, gaps };
    }
    let minor = compare::minor_gaps(entries, filters, shown, window.0);
    Before::Against { window, partial: !period.complete(now_ms, offset), numbers: Box::new(numbers), minor }
}

/// Whether the store holds a request, a miss, a cold return or a message in `[from, to)` of the
/// agents the filters keep, whatever their other filters.
pub fn stored_any(entries: &[Entry], filters: &Filters, (from, to): (i64, i64)) -> bool {
    let inside = |ts: i64| ts > 0 && ts >= from && ts < to;
    entries.iter().filter(|e| filters.agents.is_empty() || filters.agents.contains(&e.agent)).any(|e| {
        e.minutes.iter().any(|m| m.requests > 0 && inside(m.minute * MINUTE_MS))
            || e.misses.iter().any(|x| inside(x.0 * 3_600_000))
            || e.cold.iter().any(|&t| inside(t))
            || (!e.subagent && !e.left_out && e.actions.iter().any(|a| a.1 && inside(a.0)))
    })
}

/// The most main sessions whose agents worked in one minute of the window, and the first such
/// minute; each session once, though a resumed session has two logs.
fn most_at_once(kept: &[&Entry], (from, to): (i64, i64)) -> Option<(u32, i64)> {
    let mut minutes: HashMap<i64, HashSet<(&str, &str)>> = HashMap::new();
    for e in kept.iter().filter(|e| !e.subagent && !e.left_out) {
        for &(hour, bits) in &e.work {
            for b in (0..60).filter(|b| bits & (1 << b) != 0) {
                let minute = hour * 60 + b;
                let ts = minute * MINUTE_MS;
                if ts > 0 && ts >= from && ts < to {
                    minutes.entry(minute).or_default().insert((e.agent.as_str(), e.main.as_str()));
                }
            }
        }
    }
    minutes.into_iter().map(|(m, k)| (k.len() as u32, m * MINUTE_MS)).fold(None, |best: Option<(u32, i64)>, (n, ts)| match best {
        Some((bn, bts)) if bn > n || (bn == n && bts < ts) => best,
        _ => Some((n, ts)),
    })
}

fn records(span: &Month, cold: &[usize], weeks: &[WeekSum], today: Option<i64>, peak: Option<(u32, i64)>) -> Records {
    // The first of equal values: a later one replaces it only when strictly better.
    fn best<T: Copy, V: Copy>(items: impl Iterator<Item = (T, V)>, better: impl Fn(V, V) -> bool) -> Option<(T, V)> {
        items.fold(None, |b, (t, v)| match b {
            Some((_, bv)) if !better(v, bv) => b,
            _ => Some((t, v)),
        })
    }
    let days = &span.days;
    let quietest_weekday = best(days.iter().filter(|d| weekday(d.date) < 5 && d.amounts.usd > 0.0).map(|d| (d.date, d.amounts.usd)), |v, b| v < b);
    let most_cold = best(days.iter().zip(cold).filter(|(_, n)| **n > 0).map(|(d, n)| (d.date, *n)), |v, b| v > b);
    let streak = span.longest.map(|s| {
        let going = today.is_some_and(|t| s.to == t);
        (s, going)
    });
    let shown = || weeks.iter().filter(|w| w.opens());
    let biggest_week = best(shown().filter(|w| w.amounts.usd > 0.0).map(|w| (w.monday, w.amounts.usd)), |v, b| v > b);
    let best_cache_week =
        best(shown().filter(|w| w.amounts.prompt_tokens > 0).map(|w| (w.monday, w.amounts.read_tokens as f64 / w.amounts.prompt_tokens as f64)), |v, b| v > b);
    Records {
        biggest_day: span.busiest_day,
        quietest_weekday,
        most_cold,
        streak,
        busiest_hour: span.busiest_hour,
        night_share: span.night_share,
        biggest_week,
        most_at_once: peak,
        best_cache_week,
    }
}

/// The project shifts of a quarter by the rule of Project shifts, over its ended weeks.
fn shifts(entries: &[Entry], filters: &Filters, kept: &[&Entry], weeks: &[WeekSum], projects: &[ProjectWeeks], offset: Offset) -> Vec<Shift> {
    let ended: Vec<usize> = (0..weeks.len()).filter(|&i| weeks[i].kind == WeekKind::Ended).collect();
    let totals: Vec<f64> = weeks.iter().map(|w| w.amounts.usd).collect();
    let share = |p: &ProjectWeeks, i: usize| if totals[i] > 0.0 { p.weeks[i] / totals[i] } else { 0.0 };
    // The store's first request of each agent, and of each project with the agents it used.
    let timed = |e: &&Entry| e.minutes.iter().filter(|m| m.requests > 0 && m.minute > 0).map(|m| m.minute * MINUTE_MS).min();
    let agent_kept = |e: &Entry| filters.agents.is_empty() || filters.agents.contains(&e.agent);
    let mut agent_first: HashMap<&str, i64> = HashMap::new();
    for e in entries.iter().filter(|e| agent_kept(e)) {
        if let Some(ts) = timed(&e) {
            let f = agent_first.entry(e.agent.as_str()).or_insert(ts);
            *f = (*f).min(ts);
        }
    }
    let mut project_first: HashMap<&str, (i64, BTreeSet<&str>)> = HashMap::new();
    for e in kept {
        if let Some(ts) = timed(e) {
            let p = project_first.entry(e.project.as_str()).or_insert((ts, BTreeSet::new()));
            p.0 = p.0.min(ts);
            p.1.insert(e.agent.as_str());
        }
    }
    let first_week_end = weeks.first().map_or(i64::MIN, |w| w.last);
    let mut out = Vec::new();
    for p in projects {
        let shift = |kind, week, dates, shares| Shift { project: p.name.clone(), kind, usd: p.usd, week, dates, shares };
        // Started: its first request after the quarter's first week, 5% of an ended week, and
        // every agent of it with data from 28 days before its first week.
        if let Some((ts, agents)) = project_first.get(p.name.as_str()) {
            let date = rhythm::date_of(*ts, offset);
            let monday = monday_of(date);
            let known = agents.iter().all(|a| agent_first.get(a).is_some_and(|&f| rhythm::date_of(f, offset) <= monday - SHIFT_AGENT_DAYS));
            if date > first_week_end && known && ended.iter().any(|&i| share(p, i) >= SHIFT_WEEK_SHARE) {
                out.push(shift(ShiftKind::Started, Some(monday), None, None));
                continue;
            }
        }
        // Paused: after an ended week at 5% or more, two or more ended weeks in a row without a
        // request; the latest such run.
        let mut paused = None;
        let mut seen_big = false;
        let mut run: Vec<usize> = Vec::new();
        let close = |run: &Vec<usize>, paused: &mut Option<(i64, i64)>| {
            if run.len() >= SHIFT_PAUSE_WEEKS {
                *paused = Some((weeks[run[0]].first, weeks[*run.last().unwrap_or(&run[0])].last));
            }
        };
        for &i in &ended {
            if p.requests[i] == 0 {
                if seen_big {
                    run.push(i);
                }
                continue;
            }
            close(&run, &mut paused);
            run.clear();
            seen_big |= share(p, i) >= SHIFT_WEEK_SHARE;
        }
        close(&run, &mut paused);
        if let Some(d) = paused {
            out.push(shift(ShiftKind::Paused, None, Some(d), None));
            continue;
        }
        // Grew or fell: its share of the second half of the ended weeks against the first half;
        // with an odd count the middle week is left out.
        let n = ended.len();
        if n >= SHIFT_MIN_WEEKS {
            let half = n / 2;
            let part = |idx: &[usize]| {
                let all: f64 = idx.iter().map(|&i| totals[i]).sum();
                (all > 0.0).then(|| idx.iter().map(|&i| p.weeks[i]).sum::<f64>() / all)
            };
            if let (Some(a), Some(b)) = (part(&ended[..half]), part(&ended[n - half..])) {
                if b - a >= SHIFT_POINTS - 1e-9 {
                    out.push(shift(ShiftKind::Grew, None, None, Some((a, b))));
                } else if a - b >= SHIFT_POINTS - 1e-9 {
                    out.push(shift(ShiftKind::Fell, None, None, Some((a, b))));
                }
            }
        }
    }
    out.sort_by(|a, b| a.kind.cmp(&b.kind).then_with(|| b.usd.total_cmp(&a.usd)).then_with(|| a.project.cmp(&b.project)));
    out.truncate(SHIFTS_SHOWN);
    out
}

#[cfg(test)]
#[path = "period_tests.rs"]
mod tests;
