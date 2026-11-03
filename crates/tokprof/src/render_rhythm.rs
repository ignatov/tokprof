//! The rhythm views: one day, and the rhythm section of the pause report.

use std::fmt::Write;

use tokprof_core::fmt::{self, local_time, table, usd};
use tokprof_core::pauses::ColdReturn;
use tokprof_core::rhythm::{self, Offset, View};

/// The day view of `[from, to)`, with the cold returns of its requests.
pub fn day(view: &View, (from, to): (i64, i64), offset: Offset, returns: &[ColdReturn], priced: bool) -> String {
    let mut out = String::from("tokprof · day\n");
    let _ = writeln!(out, "day       {} → {}", local_time(from, "%a %Y-%m-%d %H:%M"), local_time(to, "%a %Y-%m-%d %H:%M"));
    let _ = writeln!(out, "{}", view.start.line());
    if let Some(line) = rhythm::left_out_line(view.left_out((from, to))) {
        let _ = writeln!(out, "{line}");
    }
    let summary = view.summary((from, to), offset);
    let Some(strip) = view.strip(&summary, (from, to), offset) else {
        let _ = writeln!(out, "\nNo messages on {}.", local_time(from, "%Y-%m-%d"));
        return out;
    };
    out.push('\n');
    for line in &strip.lines {
        let _ = writeln!(out, "{line}");
    }
    out.push('\n');
    for line in summary.lines() {
        let _ = writeln!(out, "{line}");
    }
    let _ = writeln!(out, "{}", cache_line(returns, priced));
    out
}

/// The `cache` line of a day.
pub fn cache_line(returns: &[ColdReturn], priced: bool) -> String {
    if returns.is_empty() {
        return "cache     no cold returns".to_string();
    }
    let n = fmt::count(returns.len() as u64, "cold return", "cold returns");
    if !priced {
        return format!("cache     {n}");
    }
    let first: f64 = returns.iter().map(|r| r.usd).sum();
    let premium: f64 = returns.iter().map(|r| r.premium_usd).sum();
    let requests = if returns.len() == 1 { "its first request" } else { "their first requests" };
    format!("cache     {n}, {} in {requests}, {} of it avoidable", usd(first), usd(premium))
}

/// The rhythm section of the pause report, over `window`.
pub fn section(view: &View, window: (i64, i64), offset: Offset) -> String {
    let mut out = String::from("\nYour rhythm: when you sat at the agents and what they did meanwhile\n\n");
    let summary = view.summary(window, offset);
    if summary.sittings.is_empty() {
        out.push_str("No messages in this time.\n");
        return out;
    }
    let _ = writeln!(out, "{}", view.start.line());
    if let Some(line) = rhythm::left_out_line(view.left_out(window)) {
        let _ = writeln!(out, "{line}");
    }
    out.push('\n');
    for line in summary.lines() {
        let _ = writeln!(out, "{line}");
    }
    out.push('\n');
    out.push_str(&table(&summary.hours(view.start, offset), 1));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokprof_core::pauses::Starter;

    fn ret(usd: f64, premium_usd: f64) -> ColdReturn {
        ColdReturn { ts_ms: 0, pause_ms: 0, usd, premium_usd, context: 0, starter: Starter::Typed }
    }

    #[test]
    fn the_cache_line_of_a_day() {
        let four = [ret(3.0, 2.9), ret(4.0, 3.8), ret(5.0, 4.7), ret(2.2, 2.1)];
        assert_eq!(cache_line(&four, true), "cache     4 cold returns, $14.20 in their first requests, $13.50 of it avoidable");
        assert_eq!(cache_line(&four, false), "cache     4 cold returns");
        assert_eq!(cache_line(&[], true), "cache     no cold returns");
        assert_eq!(cache_line(&four[..1], true), "cache     1 cold return, $3.00 in its first request, $2.90 of it avoidable");
    }

    #[test]
    fn a_day_with_no_message() {
        let view = View::new(Vec::new(), 0, &|_| 0);
        let text = day(&view, (1_793_574_000_000, 1_793_660_400_000), &|_| 0, &[], true);
        assert!(text.starts_with("tokprof · day\nday       "), "{text}");
        assert!(text.contains("\nnight     messages on fewer than 7 of the last 30 days; days start at midnight\n"), "{text}");
        assert!(text.ends_with(&format!("\nNo messages on {}.\n", local_time(1_793_574_000_000, "%Y-%m-%d"))), "{text}");
    }

    #[test]
    fn a_section_with_no_message() {
        let view = View::new(Vec::new(), 0, &|_| 0);
        assert_eq!(section(&view, (i64::MIN, i64::MAX), &|_| 0), "\nYour rhythm: when you sat at the agents and what they did meanwhile\n\nNo messages in this time.\n");
    }
}
