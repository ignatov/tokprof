//! The text of a year in review, for `tokprof year` (see year-review, The text of tokprof year):
//! the headline, the title, the big numbers of the month in review, a bar per month, the
//! records, the rhythm, the cache, the comparison or its note, whether the card is ready and the
//! line for next year. None of it names a project, a title or a path.

use std::fmt::Write;

use tokprof_core::fmt::{self, Style, int, usd};
use tokprof_core::period::{Before, Period};
use tokprof_core::rhythm;
use tokprof_core::week::dates;
use tokprof_core::year::{MonthKind, Year};

use crate::render_calendar::{day_name, naive, weekday_plural};
use crate::render_month::{big_lines, cache, delta_lines, hours, next_month};

/// The longest bar of the months, in characters.
pub const BAR: usize = 30;
/// The width of the labels of the lines under the months.
const LABEL: usize = 15;

/// `2026 with agents`, `2026 with agents, so far`.
pub fn headline(y: &Year) -> String {
    if y.sums.complete { format!("{} with agents", y.year) } else { format!("{} with agents, so far", y.year) }
}

fn pct(x: f64) -> String {
    format!("{:.0}%", 100.0 * x)
}

/// A line of a label and its text; a label as wide as the column or wider keeps two spaces.
fn row(out: &mut String, label: &str, text: &str) {
    let pad = if label.chars().count() >= LABEL { "  ".to_string() } else { " ".repeat(LABEL - label.chars().count()) };
    let _ = writeln!(out, "{label}{pad}{text}");
}

/// A row per month: its short name, a bar of `█` as long as its share of the biggest month, its
/// spend, and `no logs`, `from <day>`, `so far` or `not begun`.
pub fn month_lines(y: &Year) -> Vec<String> {
    let rows = y.months();
    let most = rows.iter().filter_map(|r| r.month).map(|m| m.total.usd).fold(0.0, f64::max);
    let width = rows.iter().filter_map(|r| r.month).map(|m| usd(m.total.usd).chars().count()).max().unwrap_or(0);
    rows.iter()
        .map(|r| {
            let name = naive(r.ym.first_date()).format("%b").to_string();
            let Some(m) = r.month else {
                return format!("{name}  {}", if r.kind == MonthKind::NotBegun { "not begun" } else { "no logs" });
            };
            let bar = if most > 0.0 && m.total.usd > 0.0 { ((BAR as f64 * m.total.usd / most).round() as usize).max(1) } else { 0 };
            let mut tags = Vec::new();
            if let Some(f) = r.from {
                tags.push(format!("from {}", naive(f).format("%-d %b")));
            }
            if r.kind == MonthKind::SoFar {
                tags.push("so far".to_string());
            }
            let tags = if tags.is_empty() { String::new() } else { format!("  {}", tags.join(", ")) };
            format!("{name}  {}{} {:>width$}{tags}", "█".repeat(bar), " ".repeat(BAR - bar), usd(m.total.usd))
        })
        .collect()
}

/// The records, each with its label: the biggest day and week, the longest streak, the most
/// commits in a day and the most sessions at once. No record names an hour of the day but the
/// minute of the most at once.
pub fn records(y: &Year) -> Vec<(&'static str, String)> {
    let r = &y.sums.records;
    let mut out = Vec::new();
    if let Some((date, v)) = r.biggest_day {
        out.push(("biggest day", format!("{}, {}", day_name(date), usd(v))));
    }
    if let Some(w) = y.biggest_week() {
        let (a, b) = (naive(w.first), naive(w.last));
        let span = if a.format("%b").to_string() == b.format("%b").to_string() {
            format!("{} – {}", a.format("%a %-d"), b.format("%a %-d %b"))
        } else {
            format!("{} – {}", a.format("%a %-d %b"), b.format("%a %-d %b"))
        };
        out.push(("biggest week", format!("{span}, {}", usd(w.amounts.usd))));
    }
    if let Some(s) = r.streak_text() {
        out.push(("longest streak", s));
    }
    if let Some((date, n)) = r.most_commits {
        out.push(("most commits", format!("{}, {}", day_name(date), int(n))));
    }
    if let Some((n, ts)) = r.most_at_once {
        out.push(("most at once", format!("{}, {}", fmt::count(u64::from(n), "session", "sessions"), fmt::local_time(ts, "%a %-d %b %H:%M"))));
    }
    out
}

/// The rhythm of the year: the busiest hour, the nights, the busiest weekday and the weekend,
/// and the top model.
pub fn rhythm_lines(y: &Year) -> Vec<(&'static str, String)> {
    let m = &y.sums.span;
    let mut out = Vec::new();
    if let Some((h, share)) = m.busiest_hour {
        out.push(("busiest hour", format!("{h:02}:00–{:02}:00, {} of agent time", (h + 1) % 24, pct(share))));
    }
    if let Some(s) = m.night_share {
        out.push(("late nights", format!("{} of agent time between 22:00 and 05:00", pct(s))));
    }
    if let Some((w, v)) = y.busiest_weekday {
        out.push(("weekday", format!("{}, {}", weekday_plural(w), usd(v))));
    }
    if let Some(s) = y.weekend_share {
        out.push(("weekend", format!("{} of spend on Saturdays and Sundays", pct(s))));
    }
    if let Some((model, v, share)) = &m.top_model {
        out.push(("top model", format!("{model}, {}, {} of spend", usd(*v), pct(*share))));
    }
    out
}

/// The comparison with the year before, as its label and its changes; none without one.
pub fn comparison(y: &Year) -> Option<(String, Vec<String>)> {
    let Before::Against { window, partial, numbers, .. } = &y.sums.before else { return None };
    let local = |ts: i64| rhythm::date_of(ts, &rhythm::local_offset);
    let label = if *partial { format!("vs {}", dates(local(window.0), local((window.1 - 1).max(window.0)), true)) } else { format!("vs {}", y.year - 1) };
    let k = &y.sums.key;
    Some((label, delta_lines((k.usd, k.agent_minutes, k.avoidable_share()), (numbers.usd, numbers.agent_minutes, numbers.avoidable_share()))))
}

/// What the year cannot compare, and the agents too small to judge by.
pub fn notes(y: &Year) -> Vec<String> {
    let before = Period::Year(y.year - 1);
    match &y.sums.before {
        Before::NoLogs(_) => vec![format!("no comparison with {}: no logs", y.year - 1)],
        Before::Against { minor, .. } => minor
            .iter()
            .map(|(agent, ts, share)| format!("{agent} only from {}, {:.0}% of spend, not judged", fmt::local_time(*ts, "%-d %b"), 100.0 * share))
            .collect(),
        other => vec![other.note(before, &rhythm::local_offset)],
    }
}

/// The pace per day when there is no comparison: `$248 a day, 6.0h of agents, …, over 99 days of
/// logs`.
pub fn pace(y: &Year) -> Option<String> {
    let days = y.sums.pace_days?;
    let k = &y.sums.key;
    let per = |v: f64| v / days as f64;
    Some(format!(
        "{} a day, {} of agents, {} at the keyboard, {:.1} sessions and {:.1} commits; over {} of logs",
        tokprof_core::week::dollars(per(k.usd)),
        hours(per(k.agent_minutes as f64)),
        hours(per(k.keyboard_ms as f64 / 60_000.0)),
        per(k.sessions as f64),
        per(k.commits as f64),
        fmt::count(days as u64, "day", "days"),
    ))
}

/// Whether the card is ready: `yes: 2026 has 99 days of logs, from 6 Aug`, or `on Fri 1 Jan
/// 2027: 2026 has 44 days of logs, from 30 Sep`.
pub fn ready(y: &Year) -> String {
    let r = &y.ready;
    match (r.ready, r.ready_on_text()) {
        (true, _) => format!("yes: {}", r.logs_text(y.year)),
        (false, Some(on)) => format!("on {on}: {}", r.logs_text(y.year)),
        (false, None) => format!("never: {} has no logs", y.year),
    }
}

/// The whole text of `tokprof year`.
pub fn text(y: &Year, style: &Style) -> String {
    let mut out = format!("tokprof · year · {}\n", headline(y));
    if !y.any() {
        out.push_str("No requests and no messages in this year.\n");
        return out;
    }
    out.push('\n');
    match &y.title {
        Some(t) => {
            let _ = writeln!(out, "{}", style.bold(t.name));
            let _ = writeln!(out, "{}", style.dim(&t.rule));
        }
        None => {
            let _ = writeln!(out, "{}", style.dim("no title this year: no rule of the six held"));
        }
    }
    out.push('\n');
    for line in big_lines(&y.sums.span, style) {
        let _ = writeln!(out, "{line}");
    }
    out.push('\n');
    for line in month_lines(y) {
        let _ = writeln!(out, "{}", line.trim_end());
    }
    out.push('\n');
    for (label, value) in records(y).into_iter().chain(rhythm_lines(y)) {
        row(&mut out, label, &value);
    }
    let (cache, note) = cache(&y.sums.span);
    let cache: Vec<String> = cache.into_iter().map(|(v, what)| format!("{v} {what}")).collect();
    row(&mut out, "cache", &cache.join(", "));
    if let Some(n) = note {
        row(&mut out, "", &n);
    }
    if let Some((label, lines)) = comparison(y) {
        row(&mut out, &label, &lines.join(", "));
    }
    if let Some(p) = pace(y) {
        row(&mut out, "pace", &p);
    }
    for n in notes(y) {
        row(&mut out, "note", &n);
    }
    row(&mut out, "ready", &ready(y));
    if let Some(line) = next_month(&y.sums.span) {
        row(&mut out, "next year", &line);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokprof_core::calendar::Filters;
    use tokprof_core::daystore::{Entry, Minute};
    use tokprof_core::month::Ym;

    const HOUR_MS: i64 = 3_600_000;

    fn date(year: i32, month: u32, day: i64) -> i64 {
        Ym { year, month }.first_date() + day - 1
    }

    /// Noon on a date in the machine's zone.
    fn noon(date: i64) -> i64 {
        rhythm::midnight(date, &rhythm::local_offset) + 12 * HOUR_MS
    }

    /// One request a day of `project` from `from` to `to`, a tenth of it avoidable.
    fn daily(id: &str, project: &str, from: i64, to: i64, usd: f64) -> Entry {
        let minutes = (from..=to).map(|d| Minute { minute: noon(d) / 60_000, usd, avoidable_usd: usd / 10.0, requests: 1, ..Minute::default() }).collect();
        Entry { key: format!("/a/{id}"), agent: "agent".into(), session: id.into(), main: id.into(), project: project.into(), minutes, ..Entry::default() }
    }

    fn year(entries: &[Entry], filters: &Filters, y: i32, now: i64) -> Year {
        Year::of(entries, filters, y, now, &rhythm::local_offset)
    }

    #[test]
    fn the_year_as_text() {
        let mut big = daily("a", "~/src/heron", date(2026, 8, 6), date(2026, 11, 12), 10.0);
        big.minutes.push(Minute {
            minute: noon(date(2026, 10, 25)) / 60_000 + 1,
            usd: 1_211.0,
            avoidable_usd: 121.1,
            commits: 175,
            requests: 1,
            ..Minute::default()
        });
        let now = noon(date(2026, 11, 12)) + HOUR_MS;
        let y = year(&[big], &Filters::default(), 2026, now);
        let text = text(&y, &Style::with_color(false));
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines[0], "tokprof · year · 2026 with agents, so far");
        assert_eq!(lines[2], "no title this year: no rule of the six held");
        assert_eq!(lines[4], "$2,201 spent            0.0h agents worked      0.0h at the keyboard");
        let before: Vec<String> = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul"].iter().map(|m| format!("{m}  no logs")).collect();
        assert_eq!(lines[7..14], before);
        assert_eq!(lines[14], format!("Aug  {}{} {:>6}  from 6 Aug", "█".repeat(5), " ".repeat(25), "$260"));
        assert_eq!(lines[16], format!("Oct  {} $1,521", "█".repeat(30)), "the biggest month has the longest bar");
        assert_eq!(lines[17], format!("Nov  {}{} {:>6}  so far", "█".repeat(2), " ".repeat(28), "$120"));
        assert_eq!(lines[18], "Dec  not begun");
        assert!(text.contains("\nbiggest day    Sun 25 Oct, $1,221\n"), "{text}");
        assert!(text.contains("\nbiggest week   Mon 19 – Sun 25 Oct, $1,281\n"), "{text}");
        assert!(text.contains("\nlongest streak 99 days, since Thu 6 Aug, still going\n"), "{text}");
        assert!(text.contains("\nmost commits   Sun 25 Oct, 175\n"), "{text}");
        assert!(text.contains("\nweekday        Sundays, $1,351\n"), "{text}");
        assert!(
            text.contains("\npace           $22 a day, 0.0h of agents, 0.0h at the keyboard, 0.0 sessions and 1.8 commits; over 99 days of logs\n"),
            "{text}"
        );
        assert!(text.contains("\nnote           no comparison with 2025: no logs\n"), "{text}");
        assert!(text.contains("\nready          yes: 2026 has 99 days of logs, from 6 Aug\n"), "{text}");
        assert!(!text.contains("busiest hour"), "no agent time, no busiest hour");
        assert!(!text.contains('\x1b'), "no escape code without color");
    }

    #[test]
    fn a_year_not_ready_and_a_comparison() {
        let e = daily("a", "~/src/heron", date(2026, 10, 10), date(2026, 11, 12), 10.0);
        let now = noon(date(2026, 11, 12)) + HOUR_MS;
        let text = text(&year(&[e], &Filters::default(), 2026, now), &Style::with_color(false));
        assert!(text.contains("\nready          on Fri 1 Jan 2027: 2026 has 34 days of logs, from 10 Oct\n"), "{text}");
        // A year in progress against the same stretch of the year before.
        let e = daily("a", "~/src/heron", date(2024, 12, 20), date(2026, 3, 1), 1.0);
        let now = noon(date(2026, 3, 1)) + HOUR_MS;
        let y = year(&[e], &Filters::default(), 2026, now);
        let (label, lines) = comparison(&y).unwrap();
        assert_eq!(label, "vs 1 Jan – 1 Mar 2025");
        assert!(lines[0].starts_with("spend = "), "{lines:?}");
        assert!(text_of(&y).contains(&format!("\nvs 1 Jan – 1 Mar 2025  {}, ", lines[0])));
        assert!(pace(&y).is_none() && notes(&y).is_empty());
    }

    fn text_of(y: &Year) -> String {
        text(y, &Style::with_color(false))
    }

    #[test]
    fn a_project_run_names_no_project() {
        let billing = daily("b", "~/src/billing", date(2026, 9, 1), date(2026, 11, 12), 4.0);
        let heron = daily("a", "~/src/heron", date(2026, 8, 6), date(2026, 11, 12), 10.0);
        let only = Filters { project: Some("billing".into()), ..Filters::default() };
        let now = noon(date(2026, 11, 12)) + HOUR_MS;
        let y = year(&[billing, heron], &only, 2026, now);
        let text = text_of(&y);
        assert!(text.contains("$292 spent"), "{text}");
        assert!(text.contains("\nAug  ") && text.contains("$0.00  from 6 Aug"), "August has logs and nothing of the project: {text}");
        for private in ["billing", "heron", "~/", "/Users"] {
            assert!(!text.contains(private), "{private}: {text}");
        }
    }

    #[test]
    fn a_year_with_nothing() {
        let now = noon(date(2026, 11, 12));
        let y = year(&[], &Filters::default(), 2024, now);
        assert_eq!(text_of(&y), "tokprof · year · 2024 with agents\nNo requests and no messages in this year.\n");
    }
}
