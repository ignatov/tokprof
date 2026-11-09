//! A period against the one of the same length just before it, from the day store: the spend,
//! the avoidable dollars, the cold returns, the sessions and the requests, and the avoidable
//! dollars of each miss cause, with the change from one to the other.

use std::collections::{BTreeMap, HashSet};

use crate::calendar::Filters;
use crate::daystore::Entry;
use crate::fmt::{int, usd};

const MINUTE_MS: i64 = 60_000;
const HOUR_MS: i64 = 3_600_000;

/// What the store holds of a window.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Totals {
    pub from_ms: i64,
    pub to_ms: i64,
    pub spend: f64,
    pub avoidable: f64,
    pub cold: usize,
    pub sessions: usize,
    pub requests: u64,
    /// The avoidable dollars of each unexpected miss cause, by its name. A cause counts by the
    /// hours whose start is in the window, as the store keeps them.
    pub causes: BTreeMap<String, f64>,
    /// Anything of the kept entries falls in the window: a request, a miss or a cold return.
    pub any: bool,
    /// The agents with requests in the period shown whose first request in the store comes after
    /// this window starts, with that first request: the window holds only part of their data,
    /// as when an agent deleted its older logs before the store kept them.
    pub gaps: Vec<(String, i64)>,
}

impl Totals {
    /// The totals of `[from, to)` of the entries `filters` keeps; its own window is not used.
    pub fn of(entries: &[Entry], filters: &Filters, (from, to): (i64, i64)) -> Totals {
        let mut t = Totals { from_ms: from, to_ms: to, ..Totals::default() };
        let inside = |ts: i64| ts >= from && ts < to;
        let mut sessions: HashSet<(&str, &str)> = HashSet::new();
        for e in entries.iter().filter(|e| filters.keeps(e)) {
            for m in e.minutes.iter().filter(|m| inside(m.minute * MINUTE_MS)) {
                t.spend += m.usd;
                t.avoidable += m.avoidable_usd;
                t.requests += m.requests as u64;
                if m.requests > 0 {
                    sessions.insert((e.agent.as_str(), e.main.as_str()));
                    t.any = true;
                }
            }
            for (hour, cause, v) in e.misses.iter().filter(|(h, _, _)| inside(h * HOUR_MS)) {
                let _ = hour;
                *t.causes.entry(cause.clone()).or_default() += v;
                t.any = true;
            }
            t.cold += e.cold.iter().filter(|&&ts| inside(ts)).count();
        }
        t.sessions = sessions.len();
        t
    }

    /// The totals of the window of the same length before `shown`, with the agents whose data
    /// in the store starts after that window does.
    pub fn before(entries: &[Entry], filters: &Filters, shown: (i64, i64)) -> Totals {
        let window = before(shown);
        let mut t = Totals::of(entries, filters, window);
        t.gaps = gaps(entries, filters, shown, window.0);
        t
    }

    /// The comparison holds only part of some agent's data.
    pub fn incomplete(&self) -> bool {
        !self.gaps.is_empty()
    }

    /// The note of an incomplete comparison: `claude only from 29 Sep`.
    pub fn gaps_note(&self) -> String {
        self.gaps.iter().map(|(a, ts)| format!("{a} only from {}", crate::fmt::local_time(*ts, "%-d %b"))).collect::<Vec<_>>().join(", ")
    }

    /// The share of the spend that was avoidable, in percent.
    pub fn share(&self) -> f64 {
        100.0 * self.avoidable / self.spend.max(1e-9)
    }
}

/// The agents with a request in `shown` whose first request in the store is after `start`, with
/// that request's time, by name.
pub fn gaps(entries: &[Entry], filters: &Filters, (from, to): (i64, i64), start: i64) -> Vec<(String, i64)> {
    let mut first: BTreeMap<&str, i64> = BTreeMap::new();
    let mut shown: HashSet<&str> = HashSet::new();
    for e in entries.iter().filter(|e| filters.keeps(e)) {
        for m in e.minutes.iter().filter(|m| m.requests > 0) {
            let ts = m.minute * MINUTE_MS;
            let f = first.entry(e.agent.as_str()).or_insert(ts);
            *f = (*f).min(ts);
            if ts >= from && ts < to {
                shown.insert(e.agent.as_str());
            }
        }
    }
    first.into_iter().filter(|(a, ts)| shown.contains(a) && *ts > start).map(|(a, ts)| (a.to_string(), ts)).collect()
}

/// The window of the same length just before `[from, to)`. A window of whole local days, from
/// one local midnight to another, gets as many local days before it, whatever the clocks did.
pub fn before((from, to): (i64, i64)) -> (i64, i64) {
    use chrono::{Local, TimeZone};
    let midnight = |ts: i64| Local.timestamp_millis_opt(ts).single().map(|t| t.time() == chrono::NaiveTime::MIN).unwrap_or(false);
    if midnight(from) && midnight(to) {
        let days = ((to - from) as f64 / 86_400_000.0).round() as i64;
        let start = Local.timestamp_millis_opt(from).single().map(|t| t.date_naive() - chrono::Days::new(days as u64));
        if let Some(ms) = start.and_then(|d| d.and_hms_opt(0, 0, 0)).and_then(|t| t.and_local_timezone(Local).earliest()).map(|t| t.timestamp_millis()) {
            return (ms, from);
        }
    }
    (from - (to - from), from)
}

/// Whether a number is better when it falls, worse when it rises, or neither.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sense {
    LowerIsBetter,
    Neutral,
}

/// How a change reads: better, worse, or neither.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    Better,
    Worse,
    Neutral,
}

/// The change from `was` to `now`: `↑ $1,203 (+11%)`, `↓ 3 (−20%)`, `= $0.00`; with `money` the
/// amount is dollars, else a count. A change from nothing has no percent.
pub fn delta(now: f64, was: f64, money: bool) -> String {
    let d = now - was;
    let amount = |v: f64| if money { usd(v) } else { int(v.round() as u64) };
    let same = if money { d.abs() < 0.005 } else { d.round() == 0.0 };
    if same {
        return format!("= {}", amount(0.0));
    }
    let arrow = if d > 0.0 { "↑" } else { "↓" };
    let percent = if was.abs() > 1e-9 { format!(" ({}{:.0}%)", if d > 0.0 { "+" } else { "−" }, (100.0 * d / was).abs()) } else { String::new() };
    format!("{arrow} {}{percent}", amount(d.abs()))
}

/// Whether the change from `was` to `now` is better or worse for a number of this sense.
pub fn verdict(now: f64, was: f64, sense: Sense, money: bool) -> Verdict {
    let d = now - was;
    let same = if money { d.abs() < 0.005 } else { d.round() == 0.0 };
    match sense {
        _ if same => Verdict::Neutral,
        Sense::Neutral => Verdict::Neutral,
        Sense::LowerIsBetter if d < 0.0 => Verdict::Better,
        Sense::LowerIsBetter => Verdict::Worse,
    }
}

/// The change of a share in percentage points: `↓ 1.2 points`.
pub fn points(now: f64, was: f64) -> String {
    let d = now - was;
    if d.abs() < 0.05 {
        return "= 0.0 points".to_string();
    }
    format!("{} {:.1} points", if d > 0.0 { "↑" } else { "↓" }, d.abs())
}

/// A cause row's change: `was $1,012 · ↓ 21%`, `new` when it had none before.
pub fn cause_change(now: f64, was: Option<f64>) -> String {
    match was.filter(|w| *w >= 0.005) {
        None => "new".to_string(),
        Some(w) => {
            let d = now - w;
            let pct = if d.abs() < 0.005 { "= 0%".to_string() } else { format!("{} {:.0}%", if d > 0.0 { "↑" } else { "↓" }, (100.0 * d / w).abs()) };
            format!("was {} · {pct}", usd(w))
        }
    }
}

/// The dates of a window as its comparison names them: `10 Sep – 9 Oct`, with the year after
/// the last day when `year`.
pub fn dates((from, to): (i64, i64), year: bool) -> String {
    let last = crate::fmt::local_time(to - 1, if year { "%-d %b %Y" } else { "%-d %b" });
    format!("{} – {last}", crate::fmt::local_time(from, "%-d %b"))
}

/// The summary's `compared with` line: the spend and the avoidable dollars against the window
/// before, or `no earlier data`.
pub fn summary_line(spend: f64, avoidable: f64, before: &Totals) -> String {
    let span = dates((before.from_ms, before.to_ms), true);
    if !before.any {
        return format!("compared with {span}: no earlier data");
    }
    let share = 100.0 * avoidable / spend.max(1e-9);
    // An incomplete period before gives the differences only, with no arrows to judge them by.
    if before.incomplete() {
        return format!(
            "compared with {span}, incomplete ({}): spend {} · avoidable {}, {} of spend",
            before.gaps_note(),
            signed(spend, before.spend, true),
            signed(avoidable, before.avoidable, true),
            signed_points(share, before.share())
        );
    }
    format!(
        "compared with {span}: spend {} · avoidable {}, {} of spend",
        delta(spend, before.spend, true),
        delta(avoidable, before.avoidable, true),
        points(share, before.share())
    )
}

/// The change from `was` to `now` as a signed difference, without an arrow: `+$1,203 (+11%)`.
pub fn signed(now: f64, was: f64, money: bool) -> String {
    let d = now - was;
    let amount = |v: f64| if money { usd(v) } else { int(v.round() as u64) };
    let same = if money { d.abs() < 0.005 } else { d.round() == 0.0 };
    if same {
        return format!("= {}", amount(0.0));
    }
    let sign = if d > 0.0 { "+" } else { "−" };
    let percent = if was.abs() > 1e-9 { format!(" ({sign}{:.0}%)", (100.0 * d / was).abs()) } else { String::new() };
    format!("{sign}{}{percent}", amount(d.abs()))
}

/// The change of a share in points, signed: `−1.2 points`.
pub fn signed_points(now: f64, was: f64) -> String {
    let d = now - was;
    if d.abs() < 0.05 {
        return "= 0.0 points".to_string();
    }
    format!("{}{:.1} points", if d > 0.0 { "+" } else { "−" }, d.abs())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::daystore::Minute;

    const H: i64 = HOUR_MS;

    /// Local midnight of a date, in ms.
    fn midnight(y: i32, m: u32, d: u32) -> i64 {
        chrono::NaiveDate::from_ymd_opt(y, m, d).unwrap().and_hms_opt(0, 0, 0).unwrap().and_local_timezone(chrono::Local).earliest().unwrap().timestamp_millis()
    }

    fn entry(agent: &str, main: &str, minutes: Vec<(i64, f64, f64, u32)>, misses: Vec<(i64, &str, f64)>, cold: Vec<i64>) -> Entry {
        Entry {
            agent: agent.into(),
            main: main.into(),
            session: main.into(),
            project: "~/src/app".into(),
            minutes: minutes
                .into_iter()
                .map(|(m, usd, avoidable_usd, requests)| Minute { minute: m, usd, avoidable_usd, requests, ..Minute::default() })
                .collect(),
            misses: misses.into_iter().map(|(h, c, v)| (h, c.to_string(), v)).collect(),
            cold,
            ..Entry::default()
        }
    }

    #[test]
    fn a_window_totals_its_minutes_hours_and_cold_returns() {
        let entries = vec![
            entry("claude", "a", vec![(60, 1.0, 0.5, 2), (120, 2.0, 0.0, 1)], vec![(1, "idle > TTL", 0.5)], vec![60 * 60_000]),
            entry("claude", "a", vec![(61, 0.25, 0.0, 1)], vec![], vec![]),
            entry("codex", "b", vec![(200, 4.0, 1.0, 3)], vec![(3, "tools changed", 1.0)], vec![]),
        ];
        let t = Totals::of(&entries, &Filters::default(), (H, 3 * H));
        assert_eq!((t.spend, t.avoidable, t.requests, t.sessions, t.cold), (3.25, 0.5, 4, 1, 1));
        assert_eq!(t.causes.into_iter().collect::<Vec<_>>(), [("idle > TTL".to_string(), 0.5)]);
        assert!(t.any);
        let none = Totals::of(&entries, &Filters::default(), (10 * H, 20 * H));
        assert!(!none.any && none.spend == 0.0);
        let codex = Totals::of(&entries, &Filters { agents: vec!["codex".into()], ..Filters::default() }, (0, 10 * H));
        assert_eq!((codex.spend, codex.sessions), (4.0, 1));
        assert_eq!(before((10 * H + 7, 40 * H + 7)), (-20 * H + 7, 10 * H + 7));
        // Whole local days keep whole days across a change of the clocks.
        assert_eq!(before((midnight(2026, 10, 10), midnight(2026, 11, 9))), (midnight(2026, 9, 10), midnight(2026, 10, 10)));
    }

    #[test]
    fn an_agent_whose_data_starts_after_the_period_before_starts_makes_it_incomplete() {
        // claude's first request is at hour 50, codex's at hour 1; both worked in the period shown.
        let entries = vec![
            entry("claude", "a", vec![(50 * 60, 1.0, 0.0, 1), (100 * 60, 1.0, 0.0, 1)], vec![], vec![]),
            entry("codex", "b", vec![(60, 1.0, 0.0, 1), (100 * 60, 1.0, 0.0, 1)], vec![], vec![]),
        ];
        let shown = (90 * H, 130 * H);
        let t = Totals::before(&entries, &Filters::default(), shown);
        assert_eq!((t.from_ms, t.to_ms), (50 * H, 90 * H));
        assert!(!t.incomplete(), "the period before starts at claude's first request");
        let t = Totals::before(&entries, &Filters::default(), (80 * H, 130 * H));
        assert_eq!(t.gaps, [("claude".to_string(), 50 * H)]);
        // An agent that did not work in the period shown does not count.
        let t = Totals::before(&entries, &Filters { agents: vec!["codex".into()], ..Filters::default() }, (80 * H, 130 * H));
        assert!(!t.incomplete());
        assert_eq!(signed(12_203.0, 11_000.0, true), "+$1,203 (+11%)");
        assert_eq!(signed_points(10.1, 11.3), "−1.2 points");
    }

    #[test]
    fn changes_read_with_an_arrow_an_amount_and_a_percent() {
        assert_eq!(delta(12_203.0, 11_000.0, true), "↑ $1,203 (+11%)");
        assert_eq!(delta(8.0, 10.0, false), "↓ 2 (−20%)");
        assert_eq!(delta(5.0, 0.0, true), "↑ $5.00");
        assert_eq!(delta(3.0, 3.0, false), "= 0");
        assert_eq!(verdict(90.0, 100.0, Sense::LowerIsBetter, true), Verdict::Better);
        assert_eq!(verdict(110.0, 100.0, Sense::LowerIsBetter, true), Verdict::Worse);
        assert_eq!(verdict(110.0, 100.0, Sense::Neutral, true), Verdict::Neutral);
        assert_eq!(points(10.1, 11.3), "↓ 1.2 points");
        assert_eq!(cause_change(796.0, Some(1012.0)), "was $1,012 · ↓ 21%");
        assert_eq!(cause_change(5.0, None), "new");
    }

    #[test]
    fn the_summary_line_compares_spend_and_avoidable_or_says_there_is_nothing_before() {
        let from = midnight(2026, 9, 10);
        let to = midnight(2026, 10, 10);
        let before = Totals { from_ms: from, to_ms: to, spend: 5_284.0, avoidable: 597.0, any: true, ..Totals::default() };
        assert_eq!(
            summary_line(5_500.0, 540.0, &before),
            "compared with 10 Sep – 9 Oct 2026: spend ↑ $216 (+4%) · avoidable ↓ $57.00 (−10%), ↓ 1.5 points of spend"
        );
        let partial = Totals { gaps: vec![("claude".into(), midnight(2026, 9, 29) + 10 * H)], ..before.clone() };
        assert_eq!(
            summary_line(5_500.0, 540.0, &partial),
            "compared with 10 Sep – 9 Oct 2026, incomplete (claude only from 29 Sep): spend +$216 (+4%) · avoidable −$57.00 (−10%), −1.5 points of spend"
        );
        let empty = Totals { any: false, ..before };
        assert_eq!(summary_line(5_500.0, 540.0, &empty), "compared with 10 Sep – 9 Oct 2026: no earlier data");
    }
}
