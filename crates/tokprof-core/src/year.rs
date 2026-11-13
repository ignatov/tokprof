//! A year in review (see year-review): one calendar year of the day store as `PeriodSums` of
//! `Period::Year`, with what only a year adds: the days of logs, the rhythm of the weekdays, the
//! readiness of its card and a title by month-review's rules with `Marathon` from 100 days. It
//! reads no log and names no project, title or path.

use crate::calendar::{Filters, weekday};
use crate::daystore::Entry;
use crate::month::{Month, Title, Ym, title_over};
use crate::period::{Period, PeriodSums, WeekSum};
use crate::rhythm::{self, Offset};

const MINUTE_MS: i64 = 60_000;

/// A year's `Marathon`: days in a row with a request (a month's is 28).
pub const MARATHON_DAYS: i64 = 100;
/// The days of logs that make a year's card ready before the year is over.
pub const READY_DAYS: i64 = 90;

/// Whether the year's card can be saved (see year-review, When the card is ready).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Readiness {
    pub ready: bool,
    /// The year's days with logs: from the later of 1 January and the day the logs begin, to the
    /// earlier of 31 December and today, today counting whole.
    pub logs_days: i64,
    /// The day the logs begin, when it lies inside the year after 1 January.
    pub logs_from: Option<i64>,
    /// The day it gets ready, or got ready: its 90th day of logs, or 1 January after it,
    /// whichever comes first; none for a year without logs, which is never ready.
    pub ready_on: Option<i64>,
}

impl Readiness {
    /// The readiness of `year` with logs from `first_data` on the current day `today` (the
    /// calendar's, by its day rule, so a night counts to the day before); `complete` when the
    /// year has ended.
    pub fn of(year: i32, first_data: Option<i64>, today: i64, complete: bool) -> Readiness {
        let period = Period::Year(year);
        let (first, last) = (period.first_date(), period.last_date());
        let today = if complete { last } else { today.min(last) };
        let Some(begin) = first_data.filter(|&f| f <= today) else { return Readiness::default() };
        let from = begin.max(first);
        let logs_days = (today - from + 1).max(0);
        if logs_days == 0 {
            return Readiness::default();
        }
        let ready_on = (from + READY_DAYS - 1).min(period.next_date());
        Readiness { ready: complete || logs_days >= READY_DAYS, logs_days, logs_from: (from > first).then_some(from), ready_on: Some(ready_on) }
    }

    /// `Fri 1 Jan 2027`.
    pub fn ready_on_text(&self) -> Option<String> {
        self.ready_on.map(|d| naive(d).format("%a %-d %b %Y").to_string())
    }

    /// `2026 has 44 days of logs, from 30 Sep`; without `from` when the logs began before the
    /// year.
    pub fn logs_text(&self, year: i32) -> String {
        let days = crate::fmt::count(self.logs_days as u64, "day", "days");
        match self.logs_from {
            Some(f) => format!("{year} has {days} of logs, from {}", naive(f).format("%-d %b")),
            None => format!("{year} has {days} of logs"),
        }
    }

    /// `logs since 20 Dec`, when the logs began inside the year.
    pub fn since_text(&self) -> Option<String> {
        self.logs_from.map(|f| format!("logs since {}", naive(f).format("%-d %b")))
    }
}

/// What a month's row of the year is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MonthKind {
    /// All its dates lie before the logs begin.
    NoLogs,
    /// It has not begun.
    NotBegun,
    /// It holds today.
    SoFar,
    /// It is over.
    Ended,
}

/// A month of the year, as its bar and its row show it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MonthRow<'a> {
    pub ym: Ym,
    pub kind: MonthKind,
    /// The day the logs begin, when it lies inside the month after its first day.
    pub from: Option<i64>,
    /// Its numbers, as `tokprof month` counts them; none for a month without logs or not begun.
    pub month: Option<&'a Month>,
}

/// One calendar year in review.
#[derive(Clone, Debug, PartialEq)]
pub struct Year {
    pub year: i32,
    /// The key numbers, the comparison or the pace, the cells of the year's dates (`sums.span`:
    /// the numbers of month-review over the year, with the year's title), the weeks, the
    /// projects, the months that have begun and the records.
    pub sums: PeriodSums,
    /// The year's days with a request, of `ready.logs_days`.
    pub active_days: usize,
    /// The weekday with the most spend (Monday 0) and its spend, as the calendar's `weekday`
    /// line counts it; none without spend.
    pub busiest_weekday: Option<(usize, f64)>,
    /// The spend of Saturdays and Sundays over the spend of the year's dates.
    pub weekend_share: Option<f64>,
    pub ready: Readiness,
    pub title: Option<Title>,
    /// The day it was counted on: the calendar's current day while the year runs (see
    /// `PeriodSums::today`), else the local date.
    pub today: i64,
}

impl Year {
    /// The year `year` of the entries `filters` keeps, as of `now_ms`: about 16 passes over the
    /// store's entries (the year, the year before, the cells and the twelve months).
    pub fn of(entries: &[Entry], filters: &Filters, year: i32, now_ms: i64, offset: Offset) -> Year {
        Year::from_sums(PeriodSums::of(entries, filters, Period::Year(year), now_ms, offset), now_ms, offset)
    }

    /// The year from the sums of `Period::Year`, as the window's period worker gives them; with
    /// no other pass over the store. Sums of another period are taken as the year of their
    /// first date.
    pub fn from_sums(mut sums: PeriodSums, now_ms: i64, offset: Offset) -> Year {
        let year = Ym::of_date(sums.period.first_date()).year;
        // The calendar's current day while the year runs: after midnight, a night still counts to
        // the day before.
        let today = sums.today.unwrap_or_else(|| rhythm::date_of(now_ms, offset));
        let ready = Readiness::of(year, sums.first_data, today, sums.complete);
        let active_days = sums.span.days.iter().filter(|d| d.amounts.requests > 0).count();
        let (busiest_weekday, weekend_share) = weekdays(&sums.span.days);
        let title = title_over(&sums.span, MARATHON_DAYS);
        sums.span.title = title.clone();
        Year { year, sums, active_days, busiest_weekday, weekend_share, ready, title, today }
    }

    pub fn period(&self) -> Period {
        Period::Year(self.year)
    }

    /// Whether anything happened in the year: a request or a message.
    pub fn any(&self) -> bool {
        self.sums.span.any()
    }

    /// Whether a date has logs: it is on or after the day the logs begin.
    pub fn has_logs(&self, date: i64) -> bool {
        self.sums.first_data.is_some_and(|f| date >= f)
    }

    /// The year's dates before the logs begin.
    pub fn days_without_logs(&self) -> i64 {
        let p = self.period();
        match self.sums.first_data {
            Some(f) => (f.min(p.next_date()) - p.first_date()).max(0),
            None => p.days(),
        }
    }

    /// The twelve months, January first, as of the day it was counted on.
    pub fn months(&self) -> Vec<MonthRow<'_>> {
        months_of(&self.sums, self.today)
    }

    /// The week of the biggest-week record, with its dates inside the year.
    pub fn biggest_week(&self) -> Option<&WeekSum> {
        let (monday, _) = self.sums.records.biggest_week?;
        self.sums.weeks.iter().find(|w| w.monday == monday)
    }
}

/// The twelve months of a year's sums, January first, as of the calendar's day `today`.
pub fn months_of(sums: &PeriodSums, today: i64) -> Vec<MonthRow<'_>> {
    sums.period
        .months()
        .into_iter()
        .map(|ym| {
            let (first, last) = (ym.first_date(), ym.next().first_date() - 1);
            let kind = if first > today {
                MonthKind::NotBegun
            } else if sums.first_data.is_none_or(|f| f > last) {
                MonthKind::NoLogs
            } else if today <= last {
                MonthKind::SoFar
            } else {
                MonthKind::Ended
            };
            let from = sums.first_data.filter(|&f| f > first && f <= last);
            let month = matches!(kind, MonthKind::SoFar | MonthKind::Ended).then(|| sums.months.iter().find(|m| m.ym == Some(ym))).flatten();
            MonthRow { ym, kind, from, month }
        })
        .collect()
}

/// The weekday with the most spend (Monday 0) and its spend, as the calendar's `weekday` line
/// counts it, and the spend of Saturdays and Sundays over all of it; none without spend.
pub fn weekdays(days: &[crate::calendar::Day]) -> (Option<(usize, f64)>, Option<f64>) {
    let mut by_weekday = [0.0; 7];
    for d in days {
        by_weekday[weekday(d.date)] += d.amounts.usd;
    }
    let (w, v) = by_weekday.iter().enumerate().fold((0, 0.0), |b, (w, &v)| if v > b.1 { (w, v) } else { b });
    let all: f64 = by_weekday.iter().sum();
    ((v > 0.0).then_some((w, v)), (all > 0.0).then(|| (by_weekday[5] + by_weekday[6]) / all))
}

/// What Year's cards show from the store alone: the spend of each agent and model, and the
/// avoidable dollars of each miss cause with its months and the main sessions behind it. A model
/// and a cause count by the hours whose start is in the window, as the store keeps them.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct StoreCards {
    /// Spend by agent, most first.
    pub agents: Vec<(String, f64)>,
    /// Spend by agent and model, most first.
    pub models: Vec<(String, String, f64)>,
    /// The unexpected miss causes with avoidable dollars, most first.
    pub causes: Vec<CauseYear>,
}

/// A miss cause over a year.
#[derive(Clone, Debug, PartialEq)]
pub struct CauseYear {
    pub name: String,
    pub usd: f64,
    /// Its dollars in each month that has any, January first.
    pub months: Vec<(Ym, f64)>,
    /// The main sessions behind it, by agent and main session id, most first.
    pub sessions: Vec<(String, String, f64)>,
}

impl StoreCards {
    /// The cards of the entries `kept` in `[from, to)`.
    pub fn of(kept: &[&Entry], (from, to): (i64, i64), offset: Offset) -> StoreCards {
        use std::collections::{BTreeMap, HashMap};
        const HOUR_MS: i64 = 3_600_000;
        type Cause<'a> = (f64, BTreeMap<Ym, f64>, HashMap<(&'a str, &'a str), f64>);
        let inside = |ts: i64| ts > 0 && ts >= from && ts < to;
        let mut agents: HashMap<&str, f64> = HashMap::new();
        let mut models: HashMap<(&str, &str), f64> = HashMap::new();
        let mut causes: HashMap<&str, Cause> = HashMap::new();
        for e in kept {
            for m in e.minutes.iter().filter(|m| inside(m.minute * MINUTE_MS)) {
                *agents.entry(e.agent.as_str()).or_default() += m.usd;
            }
            for (_, model, v) in e.models.iter().filter(|x| inside(x.0 * HOUR_MS)) {
                *models.entry((e.agent.as_str(), model.as_str())).or_default() += v;
            }
            for (hour, cause, v) in e.misses.iter().filter(|x| inside(x.0 * HOUR_MS)) {
                let c = causes.entry(cause.as_str()).or_default();
                c.0 += v;
                *c.1.entry(Ym::of_date(rhythm::date_of(hour * HOUR_MS, offset))).or_default() += v;
                *c.2.entry((e.agent.as_str(), e.main.as_str())).or_default() += v;
            }
        }
        let most = |a: f64, b: f64| b.total_cmp(&a);
        let mut agents: Vec<(String, f64)> = agents.into_iter().filter(|x| x.1 > 0.0).map(|(a, v)| (a.to_string(), v)).collect();
        agents.sort_by(|a, b| most(a.1, b.1).then_with(|| a.0.cmp(&b.0)));
        let mut models: Vec<(String, String, f64)> = models.into_iter().filter(|x| x.1 > 0.0).map(|((a, m), v)| (a.to_string(), m.to_string(), v)).collect();
        models.sort_by(|a, b| most(a.2, b.2).then_with(|| (&a.0, &a.1).cmp(&(&b.0, &b.1))));
        let mut causes: Vec<CauseYear> = causes
            .into_iter()
            .filter(|x| x.1.0 >= 0.005)
            .map(|(name, (usd, months, sessions))| {
                let mut sessions: Vec<(String, String, f64)> =
                    sessions.into_iter().filter(|x| x.1 > 0.0).map(|((a, m), v)| (a.to_string(), m.to_string(), v)).collect();
                sessions.sort_by(|a, b| most(a.2, b.2).then_with(|| (&a.0, &a.1).cmp(&(&b.0, &b.1))));
                CauseYear { name: name.to_string(), usd, months: months.into_iter().filter(|x| x.1 >= 0.005).collect(), sessions }
            })
            .collect();
        causes.sort_by(|a, b| most(a.usd, b.usd).then_with(|| a.name.cmp(&b.name)));
        StoreCards { agents, models, causes }
    }
}

/// The year `tokprof year` shows by default: this year when it holds a request or a message of
/// the entries `filters` keeps, else the last year that does, else this year.
pub fn default_year(entries: &[Entry], filters: &Filters, now_ms: i64, offset: Offset) -> i32 {
    let this = Ym::of_date(rhythm::date_of(now_ms, offset)).year;
    let latest = entries
        .iter()
        .filter(|e| filters.keeps(e))
        .flat_map(|e| {
            let requests = e.minutes.iter().filter(|m| m.requests > 0 && m.minute > 0).map(|m| m.minute * MINUTE_MS);
            let messages = e.actions.iter().filter(|a| a.1 && !e.subagent && !e.left_out).map(|a| a.0).filter(|&t| t > 0);
            requests.chain(messages)
        })
        .filter(|&ts| ts < now_ms)
        .max();
    match latest.map(|ts| Ym::of_date(rhythm::date_of(ts, offset)).year) {
        Some(y) if y < this => y,
        _ => this,
    }
}

fn naive(date: i64) -> chrono::NaiveDate {
    chrono::DateTime::from_timestamp(date * 86_400, 0).map(|t| t.date_naive()).unwrap_or_default()
}

#[cfg(test)]
#[path = "year_tests.rs"]
mod tests;
