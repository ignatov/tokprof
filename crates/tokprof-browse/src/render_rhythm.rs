//! The rhythm views: one day, and the rhythm section of the pause report.

use std::fmt::Write;

use tokprof_core::fmt::{self, local_time, table, usd};
use tokprof_core::pauses::ColdReturn;
use tokprof_core::rhythm::{self, Offset, View};

use crate::render_xray::advice_lines;

/// A date, counted as days since 1970-01-01, as `%Y-%m-%d`. The count already names the local
/// date, so the machine's time zone must not shift it.
fn date_name(date: i64) -> String {
    chrono::DateTime::from_timestamp(date * 86_400, 0).map(|t| t.date_naive().format("%Y-%m-%d").to_string()).unwrap_or_default()
}

/// The day view of a date that runs `[from, to)`, with the cold returns of its requests and a
/// strip in `layout` over the 24 hours from `axis`. The current day ends at `i64::MAX`.
#[allow(clippy::too_many_arguments)]
pub fn day(
    view: &View,
    date: i64,
    window: (i64, i64),
    offset: Offset,
    returns: &[ColdReturn],
    priced: bool,
    layout: rhythm::Layout,
    axis: Option<i64>,
    style: &fmt::Style,
) -> String {
    day_parts(view, date, window, offset, returns, priced, layout, axis, style).text()
}

/// A day view in three parts, for a screen that draws the strip itself: the text above the strip,
/// the strip with its lanes, and the text below it.
#[derive(Clone, Debug, PartialEq)]
pub struct DayParts {
    pub above: String,
    /// `None` on a day without a message, whose text is all above.
    pub strip: Option<(rhythm::Strip, rhythm::StripLanes)>,
    pub below: String,
}

impl DayParts {
    /// The day view as `tokprof day` prints it.
    pub fn text(&self) -> String {
        let mut out = self.above.clone();
        for line in self.strip.iter().flat_map(|(s, _)| &s.lines) {
            let _ = writeln!(out, "{line}");
        }
        out.push_str(&self.below);
        out
    }
}

/// `day` in its parts.
#[allow(clippy::too_many_arguments)]
pub fn day_parts(
    view: &View,
    date: i64,
    (from, to): (i64, i64),
    offset: Offset,
    returns: &[ColdReturn],
    priced: bool,
    layout: rhythm::Layout,
    axis: Option<i64>,
    style: &fmt::Style,
) -> DayParts {
    let mut above = String::from("tokprof · day\n");
    let end = if to == i64::MAX { "now".to_string() } else { local_time(to, "%a %Y-%m-%d %H:%M") };
    let _ = writeln!(above, "day       {} → {end}", local_time(from, "%a %Y-%m-%d %H:%M"));
    let _ = writeln!(above, "{}", view.days.line(date, offset));
    if let Some(line) = rhythm::left_out_line(view.left_out((from, to))) {
        let _ = writeln!(above, "{line}");
    }
    let summary = view.summary((from, to));
    let strip = view.strip(&summary, (from, to), layout, axis, style, offset).zip(view.strip_lanes(&summary, (from, to), axis, offset));
    if strip.is_none() {
        let _ = writeln!(above, "\nNo messages on {}.", date_name(date));
        return DayParts { above, strip, below: String::new() };
    }
    above.push('\n');
    let mut below = String::from("\n");
    for line in summary.lines() {
        let _ = writeln!(below, "{line}");
    }
    let _ = writeln!(below, "{}", cache_line(returns, priced));
    let advice = summary.advice(&rhythm::Returns::of(returns, priced), &view.instruction_files((from, to)));
    advice_lines(&mut below, advice.iter().map(String::as_str), style);
    DayParts { above, strip, below }
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

/// The rhythm section of the pause report, over `window`, with the cold returns of all its agents.
pub fn section(view: &View, window: (i64, i64), offset: Offset, returns: &rhythm::Returns, style: &fmt::Style) -> String {
    let mut out = String::from("\nYour rhythm: when you sat at the agents and what they did meanwhile\n\n");
    let summary = view.summary(window);
    if summary.sittings.is_empty() {
        out.push_str("No messages in this time.\n");
        return out;
    }
    let _ = writeln!(out, "{}", view.days.nights_line(window, offset));
    if let Some(line) = rhythm::left_out_line(view.left_out(window)) {
        let _ = writeln!(out, "{line}");
    }
    out.push('\n');
    for line in summary.lines() {
        let _ = writeln!(out, "{line}");
    }
    let advice = summary.advice(returns, &view.instruction_files(window));
    advice_lines(&mut out, advice.iter().map(String::as_str), style);
    out.push('\n');
    let first = view.days.median_start(window, offset).map_or(0, |m| m.div_euclid(60).rem_euclid(24) as usize);
    out.push_str(&table(&summary.hours(first, offset), 1));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokprof_core::pauses::Starter;
    use tokprof_core::rhythm::{Event, Timeline};
    use tokprof_core::{AgentId, AgentInfo};

    /// 2026-11-02 00:00 UTC, the zone of these tests.
    const NOV2: i64 = 1_793_577_600_000;
    const MINUTE: i64 = 60_000;

    fn ret(usd: f64, premium_usd: f64) -> ColdReturn {
        ColdReturn { ts_ms: 0, pause_ms: 0, usd, premium_usd, context: 0, starter: Starter::Typed }
    }

    /// Three cold returns that a warm cache would have made $12 cheaper, after breaks of 1.4h.
    fn returns() -> Vec<ColdReturn> {
        (0..3).map(|k| ColdReturn { ts_ms: NOV2 + (11 + k) * 60 * MINUTE, pause_ms: 84 * MINUTE, ..ret(4.2, 4.0) }).collect()
    }

    /// A session on 2026-11-02 from 10:00 with four turns of `turn` minutes, one started every
    /// `every` minutes, after a message the evening before.
    fn session(turn: i64, every: i64) -> View {
        let agent = AgentId::new(AgentInfo { instruction_files: vec!["CLAUDE.md".into()], ..AgentInfo::named("claude") });
        let at = |m: i64| NOV2 + 10 * 60 * MINUTE + m * MINUTE;
        let evening = [(NOV2 - 4 * 60 * MINUTE, Event::Typed), (NOV2 - 4 * 60 * MINUTE + MINUTE, Event::TurnEnd)];
        let turns = (0..4).flat_map(|k| [(at(every * k), Event::Typed), (at(every * k + 1), Event::Request), (at(every * k + turn), Event::TurnEnd)]);
        let events = evening.into_iter().chain(turns).collect();
        View::new(vec![Timeline { agent, id: "a".into(), title: None, left_out: false, events }], NOV2 + 86_400_000, &|_| 0)
    }

    fn day_of(view: &View, returns: &[ColdReturn]) -> String {
        let date = NOV2 / 86_400_000;
        day(view, date, view.days.bounds(date, &|_| 0), &|_| 0, returns, true, rhythm::Layout::TERMINAL, None, &fmt::Style::with_color(false))
    }

    #[test]
    fn the_advice_follows_the_cache_line_one_item_after_the_other() {
        // Turns of 3 minutes, one ending every 4 minutes while the user was there.
        let text = day_of(&session(3, 4), &returns());
        let lines: Vec<&str> = text.lines().collect();
        let cache = lines.iter().position(|l| l.starts_with("cache     ")).unwrap();
        assert_eq!(lines[cache], "cache     3 cold returns, $12.60 in their first requests, $12.00 of it avoidable");
        assert!(lines[cache + 1].starts_with("advice    ▸ 3 returns after the cache expired cost $12.00 "), "{text}");
        let step = lines.iter().position(|l| l.starts_with("          ▸ While you were there, an agent ended a turn every 4.0m, ")).expect(&text);
        assert!(step > cache + 2, "{text}");
        for line in lines[cache + 2..step].iter().chain(&lines[step + 1..]) {
            assert!(line.starts_with(&" ".repeat(12)) && !line.starts_with(&" ".repeat(13)), "a wrapped line: {line:?}");
        }
        assert!(lines[cache + 1..].iter().all(|l| l.chars().count() <= 12 + 82), "{text}");
        let joined = lines[step..].iter().map(|l| l.trim_start()).collect::<Vec<_>>().join(" ");
        assert!(joined.contains("after turns of a median 3.0m: you led it step by step. "), "{joined}");
    }

    #[test]
    fn the_parts_of_a_day_join_into_its_text() {
        let view = session(3, 4);
        let date = NOV2 / 86_400_000;
        let parts =
            day_parts(&view, date, view.days.bounds(date, &|_| 0), &|_| 0, &returns(), true, rhythm::Layout::TERMINAL, None, &fmt::Style::with_color(true));
        let (strip, lanes) = parts.strip.as_ref().expect("the day has a strip");
        assert!(parts.above.starts_with("tokprof · day\nday       ") && parts.above.ends_with("\n\n"), "the lines above, then a blank line: {:?}", parts.above);
        assert!(parts.below.starts_with("\nsittings  "), "a blank line, then the rhythm lines: {:?}", parts.below);
        let text = format!("{}{}\n{}", parts.above, strip.lines.join("\n"), parts.below);
        assert_eq!(text, day_of_colored(&view, &returns()), "above, the strip and below make the day's text");
        assert_eq!(parts.text(), text);
        assert_eq!(lanes.lanes.len(), 1, "one lane, as in the strip");
        assert!(!parts.above.contains('█') && !parts.below.contains('█'), "the strip is in neither part");

        let empty = View::new(Vec::new(), 0, &|_| 0);
        let parts = day_parts(&empty, date, empty.days.bounds(date, &|_| 0), &|_| 0, &[], true, rhythm::Layout::TERMINAL, None, &fmt::Style::with_color(false));
        assert_eq!((parts.strip.is_none(), parts.below.as_str()), (true, ""), "a day without a message is all above");
        assert!(parts.above.ends_with("\nNo messages on 2026-11-02.\n"));
    }

    fn day_of_colored(view: &View, returns: &[ColdReturn]) -> String {
        let date = NOV2 / 86_400_000;
        day(view, date, view.days.bounds(date, &|_| 0), &|_| 0, returns, true, rhythm::Layout::TERMINAL, None, &fmt::Style::with_color(true))
    }

    #[test]
    fn a_day_without_advice_ends_with_the_cache_line() {
        let text = day_of(&session(6, 8), &[]);
        assert!(text.ends_with("\ncache     no cold returns\n"), "{text}");
    }

    #[test]
    fn the_advice_of_the_pause_report_follows_the_rhythm_lines() {
        let r = rhythm::Returns::of(&returns(), true);
        let text = section(&session(3, 4), (i64::MIN, i64::MAX), &|_| 0, &r, &fmt::Style::with_color(false));
        let lines: Vec<&str> = text.lines().collect();
        let stops = lines.iter().position(|l| l.starts_with("stops     ")).expect(&text);
        assert!(lines[stops + 1].starts_with("advice    ▸ 3 returns after the cache expired "), "{text}");
        let blank = stops + lines[stops..].iter().position(|l| l.is_empty()).expect(&text);
        assert!(lines[stops + 2..blank].iter().any(|l| l.starts_with("          ▸ While you were there")), "{text}");
        assert!(lines[blank + 1].starts_with("hour"), "the hour table follows the advice: {text}");
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
        // 2026-11-02 in UTC.
        let date = 1_793_577_600_000 / 86_400_000;
        let text = day(&view, date, view.days.bounds(date, &|_| 0), &|_| 0, &[], true, rhythm::Layout::TERMINAL, None, &fmt::Style::with_color(false));
        assert!(text.starts_with("tokprof · day\nday       "), "{text}");
        assert!(text.contains("\nnight     no pause before the day; it starts at midnight\n"), "{text}");
        assert!(text.ends_with("\nNo messages on 2026-11-02.\n"), "{text}");
    }

    #[test]
    fn a_day_with_no_message_keeps_its_date_in_any_zone() {
        // A day at UTC+14 starts at 10:00 UTC the day before; read in another zone, its noon
        // fell on the wrong date.
        let plus14 = |_: i64| 14 * 3_600_000;
        let view = View::new(Vec::new(), 0, &plus14);
        let date = 1_793_577_600_000 / 86_400_000;
        let text = day(&view, date, view.days.bounds(date, &plus14), &plus14, &[], true, rhythm::Layout::TERMINAL, None, &fmt::Style::with_color(false));
        assert!(text.ends_with("\nNo messages on 2026-11-02.\n"), "{text}");
    }

    #[test]
    fn a_section_with_no_message() {
        let view = View::new(Vec::new(), 0, &|_| 0);
        assert_eq!(
            section(&view, (i64::MIN, i64::MAX), &|_| 0, &rhythm::Returns::default(), &fmt::Style::with_color(false)),
            "\nYour rhythm: when you sat at the agents and what they did meanwhile\n\nNo messages in this time.\n"
        );
    }
}
