//! The activity calendar: the entries of the day store summed by work-rhythm day and by local
//! weekday and hour, with what the grid, its levels and its numbers need.
//!
//! A day starts in the middle of the night before it (see `rhythm::Days`), found from the user's
//! actions in the entries kept; after the day of the latest action, each date whose noon has
//! passed starts at its midnight, and before the first action days are local dates. An amount
//! goes to the day its minute starts in.

use std::collections::{HashMap, HashSet};

use crate::daystore::Entry;
use crate::query::id_starts_with;
use crate::rhythm::{self, DayStart, Days, Offset};

const MINUTE_MS: i64 = 60_000;
const HOUR_MS: i64 = 60 * MINUTE_MS;
const DAY_MS: i64 = 24 * HOUR_MS;
/// The fewest and the most weeks the grid shows.
pub const MIN_WEEKS: i64 = 13;
pub const MAX_WEEKS: i64 = 53;

/// What a calendar draws.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Metric {
    Spend,
    Agent,
    Keyboard,
    Messages,
    Commits,
    Avoidable,
}

impl Metric {
    pub const ALL: [Metric; 6] = [Metric::Spend, Metric::Agent, Metric::Keyboard, Metric::Messages, Metric::Commits, Metric::Avoidable];
    pub const NAMES: &str = "spend, agent, keyboard, messages, commits, avoidable";

    /// The name `--metric` takes.
    pub fn name(self) -> &'static str {
        match self {
            Metric::Spend => "spend",
            Metric::Agent => "agent",
            Metric::Keyboard => "keyboard",
            Metric::Messages => "messages",
            Metric::Commits => "commits",
            Metric::Avoidable => "avoidable",
        }
    }

    /// The name a screen shows.
    pub fn title(self) -> &'static str {
        match self {
            Metric::Spend => "Spend",
            Metric::Agent => "Agent time",
            Metric::Keyboard => "At the keyboard",
            Metric::Messages => "Messages",
            Metric::Commits => "Commits",
            Metric::Avoidable => "Avoidable",
        }
    }

    pub fn parse(s: &str) -> Option<Metric> {
        Metric::ALL.into_iter().find(|m| m.name() == s)
    }

    /// Whether amounts are dollars, minutes or counts.
    pub fn unit(self) -> Unit {
        match self {
            Metric::Spend | Metric::Avoidable => Unit::Usd,
            Metric::Agent | Metric::Keyboard => Unit::Minutes,
            Metric::Messages | Metric::Commits => Unit::Count,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unit {
    Usd,
    Minutes,
    Count,
}

/// The amounts of one day, or of one weekday and hour.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Amounts {
    pub usd: f64,
    pub avoidable_usd: f64,
    pub requests: u64,
    /// Minutes some agent worked, each once.
    pub agent_minutes: u64,
    pub keyboard_ms: i64,
    pub sittings: usize,
    pub messages: u64,
    pub commits: u64,
    pub pushes: u64,
    pub pull_requests: u64,
}

impl Amounts {
    /// The metric's value: dollars, minutes or a count.
    pub fn value(&self, m: Metric) -> f64 {
        match m {
            Metric::Spend => self.usd,
            Metric::Avoidable => self.avoidable_usd,
            Metric::Agent => self.agent_minutes as f64,
            Metric::Keyboard => self.keyboard_ms as f64 / MINUTE_MS as f64,
            Metric::Messages => self.messages as f64,
            Metric::Commits => self.commits as f64,
        }
    }
}

/// One cell of the grid.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Day {
    /// The local date, as days since 1970-01-01.
    pub date: i64,
    pub amounts: Amounts,
    /// The main sessions with a request that day.
    pub sessions: usize,
    /// Something of the day comes from a log that exists, or from one that is gone.
    pub live: bool,
    pub gone: bool,
}

impl Day {
    /// The day had a request or an action.
    pub fn any(&self) -> bool {
        self.live || self.gone
    }

    /// Every amount and action of the day comes from logs that are gone.
    pub fn only_kept(&self) -> bool {
        self.gone && !self.live
    }
}

/// Which entries and which time the calendar counts.
#[derive(Clone, Debug, Default)]
pub struct Filters {
    /// Agent names; empty keeps all.
    pub agents: Vec<String>,
    /// Text the project contains.
    pub project: Option<String>,
    /// The start of the session id or of its main session's id, in any case.
    pub session: Option<String>,
    pub since_ms: Option<i64>,
    pub until_ms: Option<i64>,
}

impl Filters {
    pub fn keeps(&self, e: &Entry) -> bool {
        (self.agents.is_empty() || self.agents.contains(&e.agent))
            && self.project.as_ref().is_none_or(|p| e.project.contains(p.as_str()))
            && self.session.as_ref().is_none_or(|s| id_starts_with(&e.session, s) || id_starts_with(&e.main, s))
    }

    fn inside(&self, ts: i64) -> bool {
        self.since_ms.is_none_or(|s| ts >= s) && self.until_ms.is_none_or(|u| ts < u)
    }
}

/// A run of days with a request.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Streak {
    pub from: i64,
    pub to: i64,
}

impl Streak {
    pub fn days(&self) -> i64 {
        self.to - self.from + 1
    }
}

/// The calendar of every metric over the weeks shown.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Calendar {
    /// The Monday of the first column, and the last day, as local dates.
    pub first_monday: i64,
    pub last: i64,
    /// The first day the numbers count: the first with data, or the first Monday when later.
    pub shown_from: i64,
    /// A day per date from `first_monday` to `last`.
    pub days: Vec<Day>,
    /// By weekday (Monday first) and local hour.
    pub punch: Vec<[Amounts; 24]>,
    /// Nothing was kept: no request and no action.
    pub empty: bool,
}

/// Monday 0 to Sunday 6 of a local date; 1970-01-01 was a Thursday.
pub fn weekday(date: i64) -> usize {
    (date + 3).rem_euclid(7) as usize
}

/// Where each moment's day starts, as the calendar has it.
struct DayRule {
    starts: Vec<DayStart>,
}

impl DayRule {
    /// The rhythm's days of these actions as of `now`, then a day at each midnight after the day
    /// of the latest action whose date's noon has passed.
    fn of(actions: &[i64], now: i64, offset: Offset) -> DayRule {
        let days = Days::of(actions.iter().copied(), now, offset);
        let mut starts = days.starts;
        if let (Some(last), Some(&latest)) = (starts.last().copied(), actions.iter().max()) {
            let mut date = last.date + 1;
            loop {
                let midnight = rhythm::midnight(date, offset);
                if midnight + 12 * HOUR_MS > now {
                    break;
                }
                if midnight > latest && midnight > last.at {
                    starts.push(DayStart { date, at: midnight, night: None });
                }
                date += 1;
            }
        }
        DayRule { starts }
    }

    fn date_of(&self, ts: i64, offset: Offset) -> i64 {
        let i = self.starts.partition_point(|s| s.at <= ts);
        if i == 0 { rhythm::date_of(ts, offset) } else { self.starts[i - 1].date }
    }
}

/// Local weekday and hour of a moment.
fn clock(ts: i64, offset: Offset) -> (usize, usize) {
    let local = ts + offset(ts);
    (weekday(local.div_euclid(DAY_MS)), (local.rem_euclid(DAY_MS) / HOUR_MS) as usize)
}

impl Calendar {
    /// The calendar of the entries the filters keep, as of `now_ms` (or `--until` when earlier).
    pub fn of(entries: &[Entry], filters: &Filters, now_ms: i64, offset: Offset) -> Calendar {
        let end = filters.until_ms.map_or(now_ms, |u| u.min(now_ms));
        let kept: Vec<&Entry> = entries.iter().filter(|e| filters.keeps(e)).collect();
        let rhythm: Vec<&Entry> = kept.iter().copied().filter(|e| !e.subagent && !e.left_out).collect();
        let mut all_actions: Vec<i64> = rhythm.iter().flat_map(|e| e.actions.iter().map(|a| a.0)).collect();
        all_actions.sort_unstable();
        all_actions.dedup();
        let rule = DayRule::of(&all_actions, end, offset);
        let day = |ts: i64| rule.date_of(ts, offset);
        let last = day(end.max(1) - 1);

        // The first day with data inside the window.
        let mut first_data: Option<i64> = None;
        let mut seen = |d: i64| first_data = Some(first_data.map_or(d, |f: i64| f.min(d)));
        for e in &kept {
            for m in e.minutes.iter().filter(|m| filters.inside(m.minute * MINUTE_MS)) {
                seen(day(m.minute * MINUTE_MS));
            }
        }
        for e in &rhythm {
            for &(ts, _) in e.actions.iter().filter(|a| filters.inside(a.0)) {
                seen(day(ts));
            }
        }
        let last_monday = last - weekday(last) as i64;
        let first = first_data.unwrap_or(last).min(last);
        let first_monday = (first - weekday(first) as i64).clamp(last_monday - 7 * (MAX_WEEKS - 1), last_monday - 7 * (MIN_WEEKS - 1));
        let mut cal = Calendar {
            first_monday,
            last,
            shown_from: first.max(first_monday),
            days: (first_monday..=last).map(|date| Day { date, ..Day::default() }).collect(),
            punch: vec![Default::default(); 7],
            empty: first_data.is_none(),
        };
        let index = |date: i64| (date >= first_monday && date <= last).then(|| (date - first_monday) as usize);

        // Requests, spend, commits and the sessions of each day.
        let mut sessions: HashMap<usize, HashSet<(&str, &str)>> = HashMap::new();
        for e in &kept {
            for m in &e.minutes {
                let ts = m.minute * MINUTE_MS;
                if !filters.inside(ts) {
                    continue;
                }
                let Some(i) = index(day(ts)) else { continue };
                let d = &mut cal.days[i];
                add_minute(&mut d.amounts, m);
                if e.gone {
                    d.gone = true
                } else {
                    d.live = true
                }
                if m.requests > 0 {
                    sessions.entry(i).or_default().insert((e.agent.as_str(), e.main.as_str()));
                }
                let (w, h) = clock(ts, offset);
                add_minute(&mut cal.punch[w][h], m);
            }
        }
        for (i, s) in sessions {
            cal.days[i].sessions = s.len();
        }

        // Agent time: each minute once, whichever sessions worked in it.
        let mut work: HashMap<i64, u64> = HashMap::new();
        for e in &rhythm {
            for &(hour, bits) in &e.work {
                *work.entry(hour).or_default() |= bits;
            }
        }
        for (hour, bits) in work {
            for m in 0..60 {
                if bits & (1 << m) == 0 {
                    continue;
                }
                let ts = (hour * 60 + m) * MINUTE_MS;
                if !filters.inside(ts) {
                    continue;
                }
                let Some(i) = index(day(ts)) else { continue };
                cal.days[i].amounts.agent_minutes += 1;
                let (w, h) = clock(ts, offset);
                cal.punch[w][h].agent_minutes += 1;
            }
        }

        // Messages and sittings, from the actions of each day.
        let mut by_day: HashMap<usize, Vec<i64>> = HashMap::new();
        for e in &rhythm {
            for &(ts, message) in e.actions.iter().filter(|a| filters.inside(a.0)) {
                let Some(i) = index(day(ts)) else { continue };
                by_day.entry(i).or_default().push(ts);
                let d = &mut cal.days[i];
                if e.gone {
                    d.gone = true
                } else {
                    d.live = true
                }
                if message {
                    d.amounts.messages += 1;
                    let (w, h) = clock(ts, offset);
                    cal.punch[w][h].messages += 1;
                }
            }
        }
        for (i, mut actions) in by_day {
            actions.sort_unstable();
            let sittings = rhythm::sittings(&actions);
            let d = &mut cal.days[i];
            d.amounts.sittings = sittings.len();
            d.amounts.keyboard_ms = sittings.iter().map(|&s| rhythm::sitting_ms(s)).sum();
            for s in sittings {
                spread_sitting(&mut cal.punch, s, offset);
            }
        }
        cal
    }

    /// The days the numbers count: from `shown_from` to `last`.
    pub fn shown(&self) -> &[Day] {
        &self.days[(self.shown_from - self.first_monday).max(0) as usize..]
    }

    /// The day of a date in the grid.
    pub fn day(&self, date: i64) -> Option<&Day> {
        (date >= self.first_monday && date <= self.last).then(|| &self.days[(date - self.first_monday) as usize])
    }

    pub fn weeks(&self) -> usize {
        ((self.last - self.first_monday) / 7 + 1) as usize
    }

    /// The quartile cuts of the non-zero days shown.
    pub fn cuts(&self, m: Metric) -> Option<[f64; 3]> {
        cuts(self.shown().iter().map(|d| d.amounts.value(m)))
    }

    /// The quartile cuts of the non-zero cells of the punchcard.
    pub fn punch_cuts(&self, m: Metric) -> Option<[f64; 3]> {
        cuts(self.punch.iter().flatten().map(|a| a.value(m)))
    }

    pub fn total(&self, m: Metric) -> f64 {
        self.shown().iter().map(|d| d.amounts.value(m)).sum()
    }

    /// Days with a request, of the days shown.
    pub fn active(&self) -> (usize, usize) {
        (self.shown().iter().filter(|d| d.amounts.requests > 0).count(), self.shown().len())
    }

    /// The current streak, ending at the last day or the day before it, and the longest one.
    pub fn streaks(&self) -> (Option<Streak>, Option<Streak>) {
        let days = self.shown();
        let active = |d: &Day| d.amounts.requests > 0;
        let mut longest: Option<Streak> = None;
        let mut run: Option<Streak> = None;
        for d in days {
            if active(d) {
                let r = run.get_or_insert(Streak { from: d.date, to: d.date });
                r.to = d.date;
                if longest.is_none_or(|l| r.days() > l.days()) {
                    longest = Some(*r);
                }
            } else {
                run = None;
            }
        }
        let current = match days.last() {
            Some(d) if active(d) => run,
            Some(d) if days.len() >= 2 && active(&days[days.len() - 2]) => {
                let end = d.date - 1;
                let from = days[..days.len() - 1].iter().rev().take_while(|x| active(x)).last().map_or(end, |x| x.date);
                Some(Streak { from, to: end })
            }
            _ => None,
        };
        (current, longest)
    }

    /// The day with the highest value; the earliest of equal ones.
    pub fn busiest(&self, m: Metric) -> Option<&Day> {
        let mut best: Option<&Day> = None;
        for d in self.shown().iter().filter(|d| d.amounts.value(m) > 0.0) {
            if best.is_none_or(|b| d.amounts.value(m) > b.amounts.value(m)) {
                best = Some(d);
            }
        }
        best
    }

    /// The weekday with the highest sum (Monday 0), and its sum.
    pub fn busiest_weekday(&self, m: Metric) -> Option<(usize, f64)> {
        let mut sums = [0.0; 7];
        for d in self.shown() {
            sums[weekday(d.date)] += d.amounts.value(m);
        }
        let (w, v) = sums.iter().enumerate().fold((0, 0.0), |b, (w, &v)| if v > b.1 { (w, v) } else { b });
        (v > 0.0).then_some((w, v))
    }

    /// The days shown whose every amount and action comes from logs that are gone.
    pub fn only_kept(&self) -> usize {
        self.shown().iter().filter(|d| d.only_kept()).count()
    }
}

fn add_minute(a: &mut Amounts, m: &crate::daystore::Minute) {
    a.usd += m.usd;
    a.avoidable_usd += m.avoidable_usd;
    a.requests += m.requests as u64;
    a.commits += m.commits as u64;
    a.pushes += m.pushes as u64;
    a.pull_requests += m.pull_requests as u64;
}

/// A sitting's time over the local hours it spans; one shorter than a minute counts a minute in
/// the hour it starts.
fn spread_sitting(punch: &mut [[Amounts; 24]], (a, b): (i64, i64), offset: Offset) {
    if b - a < MINUTE_MS {
        let (w, h) = clock(a, offset);
        punch[w][h].keyboard_ms += MINUTE_MS;
        return;
    }
    let mut t = a;
    while t < b {
        let next = ((t.div_euclid(MINUTE_MS) + 1) * MINUTE_MS).min(b);
        let (w, h) = clock(t, offset);
        punch[w][h].keyboard_ms += next - t;
        t = next;
    }
}

/// The quartile cuts of the non-zero values: with them in ascending order v1…vn, the values at
/// ranks ⌈n/4⌉, ⌈n/2⌉ and ⌈3n/4⌉.
pub fn cuts(values: impl Iterator<Item = f64>) -> Option<[f64; 3]> {
    let mut v: Vec<f64> = values.filter(|x| *x > 0.0).collect();
    if v.is_empty() {
        return None;
    }
    v.sort_by(f64::total_cmp);
    let n = v.len();
    let at = |k: usize| v[(k * n).div_ceil(4).max(1) - 1];
    Some([at(1), at(2), at(3)])
}

/// The level of a value: 0 for nothing, else 1 to 4 by the cuts.
pub fn level(value: f64, cuts: Option<[f64; 3]>) -> usize {
    match cuts {
        _ if value <= 0.0 => 0,
        None => 0,
        Some(c) => 1 + c.iter().filter(|&&cut| value > cut).count(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::daystore::Minute;

    const CET: i64 = HOUR_MS;

    fn off(_: i64) -> i64 {
        CET
    }

    /// 2026-11-02 00:00 in a zone one hour east of UTC, a Monday.
    const MIDNIGHT: i64 = 1_793_574_000_000;
    const NOV2: i64 = 20_759;

    fn at(day: i64, h: i64, m: i64) -> i64 {
        MIDNIGHT + day * DAY_MS + h * HOUR_MS + m * MINUTE_MS
    }

    fn minute(ts: i64, usd: f64) -> Minute {
        Minute { minute: ts / MINUTE_MS, usd, requests: 1, ..Minute::default() }
    }

    fn main_log(actions: &[i64], minutes: Vec<Minute>) -> Entry {
        Entry {
            key: "/a".into(),
            agent: "claude".into(),
            session: "s1".into(),
            main: "s1".into(),
            project: "~/app".into(),
            actions: actions.iter().map(|&t| (t, true)).collect(),
            minutes,
            ..Entry::default()
        }
    }

    #[test]
    fn dates_are_weekdays() {
        assert_eq!(weekday(NOV2), 0, "2026-11-02 is a Monday");
        assert_eq!(rhythm::date_of(MIDNIGHT + HOUR_MS, &off), NOV2);
    }

    #[test]
    fn the_cuts_are_the_quartiles_of_the_days_with_any() {
        let c = cuts([1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 0.0].into_iter());
        assert_eq!(c, Some([2.0, 4.0, 6.0]));
        assert_eq!((level(5.0, c), level(8.0, c), level(2.0, c), level(0.0, c)), (3, 4, 1, 0));
        let one = cuts([0.0, 3.0].into_iter());
        assert_eq!(level(3.0, one), 1, "a single day is level 1");
        assert_eq!(cuts([0.0].into_iter()), None);
    }

    #[test]
    fn a_night_in_the_middle_of_an_hour_splits_it() {
        // Actions at 00:42 and 09:09 on Nov 3: the day of Nov 3 starts at 04:55:30.
        let night = (at(1, 0, 42), at(1, 9, 9));
        let start = night.0 + (night.1 - night.0) / 2;
        let actions = [at(0, 10, 0), night.0, night.1, at(1, 12, 0)];
        let e = main_log(&actions, vec![minute(at(1, 3, 10), 1.0), minute(start - MINUTE_MS, 2.0), minute(start + MINUTE_MS, 4.0)]);
        let cal = Calendar::of(&[e], &Filters::default(), at(1, 20, 0), &off);
        let nov2 = cal.day(NOV2).unwrap();
        let nov3 = cal.day(NOV2 + 1).unwrap();
        assert_eq!(nov2.amounts.usd, 3.0, "03:10 and the minute before the start count on the day before");
        assert_eq!(nov3.amounts.usd, 4.0);
        assert_eq!((nov2.amounts.messages, nov3.amounts.messages), (2, 2));
        assert_eq!(cal.last, NOV2 + 1);
    }

    #[test]
    fn keyboard_time_is_the_days_sittings() {
        // Sittings of 10m, 30s and 1h: 1h 11m.
        let actions = [
            at(0, 9, 0),
            at(0, 9, 10),
            at(0, 11, 0),
            at(0, 11, 0) + 30_000,
            at(0, 13, 0),
            at(0, 13, 14),
            at(0, 13, 28),
            at(0, 13, 42),
            at(0, 13, 56),
            at(0, 14, 0),
        ];
        let cal = Calendar::of(&[main_log(&actions, vec![])], &Filters::default(), at(0, 20, 0), &off);
        let d = cal.day(NOV2).unwrap();
        assert_eq!((d.amounts.sittings, d.amounts.keyboard_ms), (3, 71 * MINUTE_MS));
        assert_eq!(d.amounts.value(Metric::Keyboard), 71.0);
        // The punchcard: 10 min at 09, a minute at 11, 60 at 13 and 14 together.
        let mon = &cal.punch[0];
        assert_eq!((mon[9].keyboard_ms, mon[11].keyboard_ms, mon[13].keyboard_ms + mon[14].keyboard_ms), (10 * MINUTE_MS, MINUTE_MS, 60 * MINUTE_MS));
    }

    #[test]
    fn a_sitting_across_hours_spreads_over_them() {
        let mut punch = vec![<[Amounts; 24]>::default(); 7];
        spread_sitting(&mut punch, (at(0, 9, 50), at(0, 10, 20)), &off);
        spread_sitting(&mut punch, (at(0, 11, 0), at(0, 11, 0) + 20_000), &off);
        assert_eq!((punch[0][9].keyboard_ms, punch[0][10].keyboard_ms, punch[0][11].keyboard_ms), (10 * MINUTE_MS, 20 * MINUTE_MS, MINUTE_MS));
    }

    #[test]
    fn agent_minutes_count_once_across_sessions() {
        let hour = (at(0, 10, 0) / HOUR_MS, (1u64 << 30) - 1);
        let a = Entry { work: vec![hour], ..main_log(&[at(0, 9, 0)], vec![]) };
        let b = Entry { key: "/b".into(), session: "s2".into(), main: "s2".into(), ..a.clone() };
        let cal = Calendar::of(&[a, b], &Filters::default(), at(0, 20, 0), &off);
        assert_eq!(cal.day(NOV2).unwrap().amounts.agent_minutes, 30);
    }

    #[test]
    fn the_window_cuts_a_day() {
        // The day of Nov 2 runs until the middle of the night before Nov 3; --since is Nov 3's
        // midnight, so only what came after midnight counts on Nov 2.
        let actions = [at(0, 22, 0), at(1, 9, 0)];
        let e = main_log(&actions, vec![minute(at(0, 23, 0), 1.0), minute(at(1, 1, 0), 2.0), minute(at(1, 10, 0), 4.0)]);
        let f = Filters { since_ms: Some(at(1, 0, 0)), ..Filters::default() };
        let cal = Calendar::of(&[e], &f, at(1, 20, 0), &off);
        assert_eq!(cal.day(NOV2).unwrap().amounts.usd, 2.0);
        assert_eq!(cal.total(Metric::Spend), 6.0);
        assert_eq!(cal.shown_from, NOV2);
    }

    #[test]
    fn days_after_the_latest_action_start_at_midnight() {
        // The last action on Thursday Nov 5 at 22:00; agents work on Friday and Saturday; it is
        // Saturday 15:00.
        let e = main_log(&[at(3, 9, 0), at(3, 22, 0)], vec![minute(at(4, 10, 0), 1.0), minute(at(5, 10, 0), 2.0)]);
        let cal = Calendar::of(&[e], &Filters::default(), at(5, 15, 0), &off);
        assert_eq!((cal.day(NOV2 + 4).unwrap().amounts.usd, cal.day(NOV2 + 5).unwrap().amounts.usd), (1.0, 2.0));
        assert_eq!(cal.last, NOV2 + 5);
        // Before Saturday's noon, Saturday has not started: its morning is Friday's.
        let e = main_log(&[at(3, 9, 0), at(3, 22, 0)], vec![minute(at(5, 10, 0), 2.0)]);
        let cal = Calendar::of(&[e], &Filters::default(), at(5, 11, 0), &off);
        assert_eq!((cal.last, cal.day(NOV2 + 4).unwrap().amounts.usd), (NOV2 + 4, 2.0));
    }

    #[test]
    fn the_grid_shows_13_to_53_weeks() {
        let now = at(6, 20, 0);
        // Three weeks of data: still 13 columns.
        let e = main_log(&[at(-20, 10, 0)], vec![minute(at(-20, 10, 0), 1.0)]);
        let cal = Calendar::of(&[e], &Filters::default(), now, &off);
        assert_eq!((cal.weeks(), cal.last, cal.first_monday), (13, NOV2 + 6, NOV2 - 84));
        assert_eq!(cal.shown_from, NOV2 - 20);
        // 70 weeks: 53 columns, and the earlier weeks count in nothing.
        let e = main_log(&[at(-490, 10, 0), at(-2, 10, 0)], vec![minute(at(-490, 10, 0), 5.0), minute(at(-2, 10, 0), 1.0)]);
        let cal = Calendar::of(&[e], &Filters::default(), now, &off);
        assert_eq!(cal.weeks(), 53);
        assert_eq!(cal.total(Metric::Spend), 1.0);
        assert_eq!(cal.shown_from, cal.first_monday);
    }

    #[test]
    fn streaks_end_at_the_last_day_or_the_day_before() {
        let mut cal = Calendar { first_monday: 0, last: 9, shown_from: 0, ..Calendar::default() };
        let active = [true, true, true, false, true, true, true, true, true, false];
        cal.days = active
            .iter()
            .enumerate()
            .map(|(i, &a)| Day { date: i as i64, amounts: Amounts { requests: a as u64, ..Amounts::default() }, ..Day::default() })
            .collect();
        let (current, longest) = cal.streaks();
        assert_eq!(current, Some(Streak { from: 4, to: 8 }), "the last day has no request yet");
        assert_eq!(longest.map(|s| s.days()), Some(5));
        cal.days[9].amounts.requests = 1;
        assert_eq!(cal.streaks().0.map(|s| s.days()), Some(6));
        cal.days[8].amounts.requests = 0;
        cal.days[9].amounts.requests = 0;
        assert_eq!(cal.streaks().0, None);
        assert_eq!(cal.active(), (7, 10));
    }

    #[test]
    fn filters_pick_entries_as_the_summary_does() {
        let e = main_log(&[], vec![]);
        let sub = Entry { session: "agent-1".into(), main: "3F951de0".into(), subagent: true, ..e.clone() };
        assert!(Filters { session: Some("3f95".into()), ..Filters::default() }.keeps(&sub));
        assert!(!Filters { project: Some("~/other".into()), ..Filters::default() }.keeps(&e));
        assert!(!Filters { agents: vec!["codex".into()], ..Filters::default() }.keeps(&e));
    }

    #[test]
    fn days_of_gone_logs_are_kept_and_counted() {
        let live = main_log(&[at(0, 10, 0)], vec![minute(at(0, 10, 0), 1.0)]);
        let gone = Entry { key: "/old".into(), gone: true, ..main_log(&[at(-3, 10, 0)], vec![minute(at(-3, 10, 0), 2.0)]) };
        let cal = Calendar::of(&[live, gone], &Filters::default(), at(0, 20, 0), &off);
        assert_eq!(cal.total(Metric::Spend), 3.0);
        assert_eq!(cal.only_kept(), 1);
        assert!(cal.day(NOV2 - 3).unwrap().only_kept());
    }

    #[test]
    fn busiest_day_and_weekday() {
        let e = main_log(&[at(0, 9, 0), at(1, 9, 0), at(7, 9, 0)], vec![minute(at(0, 10, 0), 3.0), minute(at(1, 10, 0), 5.0), minute(at(7, 10, 0), 4.0)]);
        let cal = Calendar::of(&[e], &Filters::default(), at(7, 20, 0), &off);
        assert_eq!(cal.busiest(Metric::Spend).map(|d| d.date), Some(NOV2 + 1));
        assert_eq!(cal.busiest_weekday(Metric::Spend), Some((0, 7.0)), "Mondays: $3 and $4");
        assert_eq!(cal.busiest(Metric::Commits), None);
    }
}
