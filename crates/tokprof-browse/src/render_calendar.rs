//! The text of `tokprof calendar`: the grid of days, its legend, its numbers and the punchcard.

use std::fmt::Write;

use tokprof_core::calendar::{Calendar, Metric, Unit, level, weekday};
use tokprof_core::fmt::{self, Style};

/// The marks of the five levels, none first.
pub const MARKS: [char; 5] = ['·', '░', '▒', '▓', '█'];
/// The 256-color greens of levels 1 to 4, dark to bright.
const GREENS: [&str; 4] = ["38;5;22", "38;5;28", "38;5;34", "38;5;40"];
/// A grid of at most this many weeks gives each week two characters.
pub const WIDE_WEEKS: usize = 38;
const WEEKDAYS: [&str; 7] = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
const LABEL: usize = 5;

/// A local date, counted as days since 1970-01-01.
pub fn naive(date: i64) -> chrono::NaiveDate {
    chrono::DateTime::from_timestamp(date * 86_400, 0).map(|t| t.date_naive()).unwrap_or_default()
}

/// A day as the numbers name it: `Tue 14 Oct`.
pub fn day_name(date: i64) -> String {
    naive(date).format("%a %-d %b").to_string()
}

/// An amount in the metric's unit: dollars, hours with one decimal, or a count.
pub fn amount(v: f64, unit: Unit) -> String {
    match unit {
        Unit::Usd => fmt::usd(v),
        Unit::Minutes => format!("{:.1}h", v / 60.0),
        Unit::Count => fmt::int(v.round() as u64),
    }
}

/// A level's mark, in its color when the style takes color.
fn mark(lvl: usize, style: &Style) -> String {
    let m = MARKS[lvl].to_string();
    if lvl == 0 { style.dim(&m) } else { style.paint(GREENS[lvl - 1], &m) }
}

/// The title of the calendar of a metric.
pub fn title(metric: Metric) -> String {
    format!("tokprof · calendar · {}\n", metric.name())
}

/// The month labels over the columns, `width` characters per week.
pub fn months(cal: &Calendar, width: usize) -> Vec<(usize, String)> {
    let mut out: Vec<(usize, String)> = Vec::new();
    for w in 0..cal.weeks() {
        let monday = naive(cal.first_monday + 7 * w as i64);
        if chrono::Datelike::day(&monday) > 7 {
            continue;
        }
        let at = w * width;
        let name = monday.format("%b").to_string();
        if out.last().is_none_or(|(p, n)| at > p + n.chars().count()) {
            out.push((at, name));
        }
    }
    out
}

/// The whole text of `tokprof calendar`.
pub fn text(cal: &Calendar, metric: Metric, style: &Style) -> String {
    let mut out = title(metric);
    if cal.empty {
        out.push_str("No requests and no messages in this time.\n");
        return out;
    }
    let unit = metric.unit();
    let width = if cal.weeks() <= WIDE_WEEKS { 2 } else { 1 };
    let cuts = cal.cuts(metric);
    // Month labels.
    let mut line = String::new();
    for (at, name) in months(cal, width) {
        while line.chars().count() < at {
            line.push(' ');
        }
        line.push_str(&name);
    }
    let _ = writeln!(out, "{}{}", " ".repeat(LABEL), line.trim_end());
    for (wd, name) in WEEKDAYS.iter().enumerate() {
        let mut row = format!("{name:<LABEL$}");
        for w in 0..cal.weeks() {
            let date = cal.first_monday + 7 * w as i64 + wd as i64;
            if date > cal.last {
                break;
            }
            let lvl = if date < cal.shown_from { 0 } else { cal.day(date).map_or(0, |d| level(d.amounts.value(metric), cuts)) };
            row.push_str(&mark(lvl, style));
            if width == 2 && w + 1 < cal.weeks() {
                row.push(' ');
            }
        }
        let _ = writeln!(out, "{}", row.trim_end());
    }
    let _ = writeln!(out, "{}{}", " ".repeat(LABEL), legend(cuts, unit, style));
    out.push('\n');
    for l in numbers(cal, metric) {
        let _ = writeln!(out, "{l}");
    }
    out.push('\n');
    out.push_str(&punchcard(cal, metric, style));
    out
}

/// `less · ░ ▒ ▓ █ more` and the cuts.
pub fn legend(cuts: Option<[f64; 3]>, unit: Unit, style: &Style) -> String {
    let marks: Vec<String> = (0..5).map(|l| mark(l, style)).collect();
    let mut s = format!("less {} more", marks.join(" "));
    if let Some(c) = cuts {
        let _ = write!(
            s,
            "   {} up to {}  {} up to {}  {} up to {}  {} more",
            marks[1],
            amount(c[0], unit),
            marks[2],
            amount(c[1], unit),
            marks[3],
            amount(c[2], unit),
            marks[4]
        );
    }
    s
}

/// The lines under the grid: total, active, streaks, busiest, weekday, kept.
pub fn numbers(cal: &Calendar, metric: Metric) -> Vec<String> {
    let unit = metric.unit();
    let shown = cal.shown();
    let mut out = Vec::new();
    let (Some(first), Some(last)) = (shown.first(), shown.last()) else { return out };
    out.push(format!(
        "total     {} from {} to {} {}, {}",
        amount(cal.total(metric), unit),
        day_name(first.date),
        day_name(last.date),
        naive(last.date).format("%Y"),
        fmt::count(shown.len() as u64, "day", "days")
    ));
    let (active, all) = cal.active();
    out.push(format!("active    {active} of {} had a request", fmt::count(all as u64, "day", "days")));
    let (current, longest) = cal.streaks();
    let days = |n: i64| fmt::count(n as u64, "day", "days");
    let current = current.map_or("no current streak".to_string(), |s| format!("current {}", days(s.days())));
    let longest = longest.map_or("no day with a request".to_string(), |s| format!("longest {} ({} – {})", days(s.days()), day_name(s.from), day_name(s.to)));
    out.push(format!("streaks   {current}, {longest}"));
    out.push(match cal.busiest(metric) {
        Some(d) => format!("busiest   {}, {}", day_name(d.date), amount(d.amounts.value(metric), unit)),
        None => "busiest   no day with any".to_string(),
    });
    if let Some((w, v)) = cal.busiest_weekday(metric) {
        out.push(format!("weekday   {}, {}", weekday_plural(w), amount(v, unit)));
    }
    match cal.only_kept() {
        0 => out.push("kept      no day comes only from the store yet; it keeps the days of logs the agents delete".to_string()),
        n => out.push(format!("kept      {} only from the store: their logs are gone", fmt::count(n as u64, "day comes", "days come"))),
    }
    out
}

/// `Mondays` … `Sundays`.
pub fn weekday_plural(w: usize) -> &'static str {
    ["Mondays", "Tuesdays", "Wednesdays", "Thursdays", "Fridays", "Saturdays", "Sundays"][w]
}

/// The weekday × hour punchcard.
pub fn punchcard(cal: &Calendar, metric: Metric, style: &Style) -> String {
    let mut out = format!("punchcard {} by weekday and hour, local time\n", metric.title().to_lowercase());
    let cuts = cal.punch_cuts(metric);
    let mut hours = " ".repeat(LABEL);
    for h in (0..24).step_by(3) {
        let _ = write!(hours, "{h:02}    ");
    }
    let _ = writeln!(out, "{}", hours.trim_end());
    for (wd, name) in WEEKDAYS.iter().enumerate() {
        let mut row = format!("{name:<LABEL$}");
        for h in 0..24 {
            row.push_str(&mark(level(cal.punch[wd][h].value(metric), cuts), style));
            row.push(' ');
        }
        let _ = writeln!(out, "{}", row.trim_end());
    }
    let _ = writeln!(out, "{}{}", " ".repeat(LABEL), legend(cuts, metric.unit(), style));
    out
}

/// The weekday of a date, as the grid's rows name it.
pub fn weekday_name(date: i64) -> &'static str {
    WEEKDAYS[weekday(date)]
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokprof_core::calendar::{Amounts, Day};

    /// Two weeks of a grid from Monday 2026-10-26 (date 20752) to Sunday 2026-11-08.
    fn small() -> Calendar {
        let first = 20_752;
        let mut cal = Calendar { first_monday: first - 77, last: first + 13, shown_from: first, punch: vec![Default::default(); 7], ..Calendar::default() };
        cal.days = (cal.first_monday..=cal.last).map(|date| Day { date, ..Day::default() }).collect();
        for (i, usd) in [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0].into_iter().enumerate() {
            let d = &mut cal.days[77 + i];
            d.amounts = Amounts { usd, requests: 1, ..Amounts::default() };
            d.live = true;
        }
        // Two cells: the cuts are $1, $1 and $36, so the larger one is level 3.
        cal.punch[1][22].usd = 36.0;
        cal.punch[0][9].usd = 1.0;
        cal
    }

    #[test]
    fn the_text_without_color() {
        let text = text(&small(), Metric::Spend, &Style::with_color(false));
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines[0], "tokprof · calendar · spend");
        assert_eq!(lines[1], "             Sep     Oct     Nov", "months over their first Mondays");
        assert_eq!(lines[2], "Mon  · · · · · · · · · · · ░ █");
        assert_eq!(lines[3], "Tue  · · · · · · · · · · · ░ ·");
        assert_eq!(lines[9], "     less · ░ ▒ ▓ █ more   ░ up to $2.00  ▒ up to $4.00  ▓ up to $6.00  █ more");
        assert_eq!(lines[11], "total     $36.00 from Mon 26 Oct to Sun 8 Nov 2026, 14 days");
        assert_eq!(lines[12], "active    8 of 14 days had a request");
        assert_eq!(lines[13], "streaks   no current streak, longest 8 days (Mon 26 Oct – Mon 2 Nov)");
        assert_eq!(lines[14], "busiest   Mon 2 Nov, $8.00");
        assert_eq!(lines[15], "weekday   Mondays, $9.00");
        assert_eq!(lines[16], "kept      no day comes only from the store yet; it keeps the days of logs the agents delete");
        assert_eq!(lines[18], "punchcard spend by weekday and hour, local time");
        assert_eq!(lines[19], "     00    03    06    09    12    15    18    21");
        assert!(lines[21].starts_with("Tue  · ") && lines[21].ends_with("▓ ·"), "{}", lines[21]);
        assert!(!text.contains('\x1b'));
    }

    #[test]
    fn colors_paint_the_marks() {
        let text = text(&small(), Metric::Spend, &Style::with_color(true));
        assert!(text.contains("\x1b[38;5;40m█\x1b[0m") && text.contains("\x1b[38;5;22m░\x1b[0m") && text.contains("\x1b[2m·\x1b[0m"));
    }

    #[test]
    fn days_from_the_store_only_are_named() {
        let mut cal = small();
        cal.days[78].live = false;
        cal.days[78].gone = true;
        let lines = numbers(&cal, Metric::Spend);
        assert_eq!(lines.last().unwrap(), "kept      1 day comes only from the store: their logs are gone");
    }

    #[test]
    fn units() {
        assert_eq!((amount(8550.0, Unit::Minutes), amount(1234.0, Unit::Count), amount(812.4, Unit::Usd)), ("142.5h".into(), "1,234".into(), "$812".into()));
        let empty = Calendar { empty: true, ..Calendar::default() };
        assert_eq!(text(&empty, Metric::Messages, &Style::with_color(false)), "tokprof · calendar · messages\nNo requests and no messages in this time.\n");
    }
}
