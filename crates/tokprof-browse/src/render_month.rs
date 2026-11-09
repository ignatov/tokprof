//! The texts of a month in review, for `tokprof month` and the window's card: the headline, the
//! big numbers, the facts, the cache, the deltas and the line for next month. None of them
//! names a project, a title or a path.

use std::fmt::Write;

use tokprof_core::advice::miss_hint;
use tokprof_core::analysis::cache::Cause;
use tokprof_core::calendar::{Unit, level, weekday};
use tokprof_core::fmt::{self, Style, int, usd};
use tokprof_core::month::Month;

use crate::render_calendar::{MARKS, day_name};

/// `October 2026 with agents`.
pub fn headline(m: &Month) -> String {
    let name = m.ym.map_or_else(String::new, |ym| ym.name());
    if m.complete { format!("{name} with agents") } else { format!("{name} with agents, so far") }
}

/// The big numbers: the value and what it is.
pub fn big_numbers(m: &Month) -> Vec<(String, &'static str)> {
    let t = &m.total;
    vec![
        (usd(t.usd), "spent"),
        (hours(t.agent_minutes as f64), "agents worked"),
        (hours(t.keyboard_ms as f64 / 60_000.0), "at the keyboard"),
        (int(m.sessions as u64), "sessions"),
        (int(t.messages), "messages"),
        (int(t.commits), "commits by agents"),
    ]
}

/// Minutes as hours: `212h`, or `9.5h` under ten.
pub fn hours(minutes: f64) -> String {
    let h = minutes / 60.0;
    if h < 10.0 { format!("{h:.1}h") } else { format!("{}h", int(h.round() as u64)) }
}

fn pct(x: f64) -> String {
    format!("{:.0}%", 100.0 * x)
}

/// The facts of the month, each with its label: the busiest day and hour, the nights, the
/// longest streak and the top model.
pub fn facts(m: &Month) -> Vec<(&'static str, String)> {
    let mut out = Vec::new();
    if let Some((date, v)) = m.busiest_day {
        out.push(("busiest day", format!("{}, {}", day_name(date), usd(v))));
    }
    if let Some((h, share)) = m.busiest_hour {
        out.push(("busiest hour", format!("{h:02}:00–{:02}:00, {} of agent time", (h + 1) % 24, pct(share))));
    }
    if let Some(s) = m.night_share {
        out.push(("late nights", format!("{} of agent time between 22:00 and 05:00", pct(s))));
    }
    if let Some(s) = m.longest {
        let days = fmt::count(s.days() as u64, "day", "days");
        out.push(("longest streak", if s.days() == 1 { days } else { format!("{days} in a row, {} – {}", day_name(s.from), day_name(s.to)) }));
    }
    if let Some((model, v, share)) = &m.top_model {
        out.push(("top model", format!("{model}, {}, {} of spend", usd(*v), pct(*share))));
    }
    out
}

/// The cache numbers: the share of input read from the cache, the dollars those reads saved,
/// the avoidable dollars and the cold returns; and a note on requests without a price.
pub fn cache(m: &Month) -> (Vec<(String, &'static str)>, Option<String>) {
    let t = &m.total;
    let mut out = Vec::new();
    if let Some(s) = m.read_share() {
        out.push((pct(s), "of input read from cache"));
    }
    out.push((usd(t.saved_usd), "saved by the cache"));
    let share = m.avoidable_share().map_or(String::new(), |s| format!(" ({:.1}%)", 100.0 * s));
    out.push((format!("{}{share}", usd(t.avoidable_usd)), "avoidable on misses"));
    out.push((int(m.cold_returns as u64), if m.cold_returns == 1 { "cold return" } else { "cold returns" }));
    let note = (t.unpriced > 0)
        .then(|| format!("{} {} without a price are not counted in dollars", int(t.unpriced), if t.unpriced == 1 { "request" } else { "requests" }));
    (out, note)
}

/// Against the previous month: spend, agent time and the avoidable share, or `None` when it has
/// no data.
pub fn deltas(m: &Month) -> Option<(String, Vec<String>)> {
    let p = m.previous.as_ref()?;
    let arrow = |d: f64| {
        if d > 0.0 {
            "↑"
        } else if d < 0.0 {
            "↓"
        } else {
            "="
        }
    };
    let rel = |d: f64, base: f64| if base > 0.0 { format!(" ({:+.0}%)", 100.0 * d / base) } else { String::new() };
    let mut out = Vec::new();
    let d = m.total.usd - p.usd;
    out.push(format!("spend {} {}{}", arrow(d), usd(d.abs()), rel(d, p.usd)));
    let d = m.total.agent_minutes as f64 - p.agent_minutes as f64;
    out.push(format!("agent time {} {}{}", arrow(d), hours(d.abs()), rel(d, p.agent_minutes as f64)));
    if let (Some(now), Some(was)) = (m.avoidable_share(), p.avoidable_share) {
        let d = 100.0 * (now - was);
        out.push(format!("avoidable share {} {:.1} points ({:.1}%, was {:.1}%)", arrow(d), d.abs(), 100.0 * now, 100.0 * was));
    }
    Some((p.ym.month_name(), out))
}

/// The deltas as a card shows them: the month before's name, and per number its change and
/// what it is.
pub fn delta_cells(m: &Month) -> Option<(String, Vec<(String, String)>)> {
    let p = m.previous.as_ref()?;
    let arrow = |d: f64| {
        if d > 0.0 {
            "↑"
        } else if d < 0.0 {
            "↓"
        } else {
            "="
        }
    };
    let rel = |d: f64, base: f64| if base > 0.0 { format!(", {:+.0}%", 100.0 * d / base) } else { String::new() };
    let mut out = Vec::new();
    let d = m.total.usd - p.usd;
    out.push((format!("{} {}", arrow(d), usd(d.abs())), format!("spend{}", rel(d, p.usd))));
    let d = m.total.agent_minutes as f64 - p.agent_minutes as f64;
    out.push((format!("{} {}", arrow(d), hours(d.abs())), format!("agent time{}", rel(d, p.agent_minutes as f64))));
    if let (Some(now), Some(was)) = (m.avoidable_share(), p.avoidable_share) {
        let d = 100.0 * (now - was);
        out.push((format!("{} {:.1} pt", arrow(d), d.abs()), format!("avoidable, {:.1}% from {:.1}%", 100.0 * now, 100.0 * was)));
    }
    Some((p.ym.month_name(), out))
}

/// The line for next month: the costliest miss cause, its dollars and tokprof's advice for it.
pub fn next_month(m: &Month) -> Option<String> {
    let (cause, v) = m.top_cause?;
    let hint = miss_hint(cause);
    let mut chars = hint.chars();
    let hint = chars.next().map_or(String::new(), |c| c.to_uppercase().collect::<String>() + chars.as_str());
    Some(format!("{} went to {}. {hint}.", usd(v), cause.name()))
}

/// A miss cause in plain words, for the card someone posts: what happened, not tokprof's name.
pub fn plain_cause(cause: Cause) -> &'static str {
    match cause {
        Cause::IdleTtl => "coming back after the cache expired",
        Cause::CompactionCall => "compaction rewriting the context",
        Cause::ModelSwitch => "switching models in the middle of a session",
        Cause::ParamsChanged => "changing effort or settings in the middle of a session",
        Cause::AgentUpdated => "updating the agent in the middle of a session",
        Cause::ToolsChanged => "tools changing in the middle of a session",
        Cause::SystemChanged => "the system prompt changing in the middle of a session",
        Cause::ImagesRemoved => "images dropped from the history",
        Cause::ThinkingDropped => "thinking dropped from the history",
        Cause::HistoryEdited => "earlier messages being rewritten",
        Cause::Lookback => "steps too long for the cache to look back over",
        Cause::Compaction => "compaction",
        Cause::ToolResultsCleared => "old tool results being cleared",
        _ => "misses of no known cause",
    }
}

/// The line for next month on the card: the cause in plain words, its dollars and tokprof's
/// advice, as `Coming back after the cache expired cost $873. Compact or wrap up…`.
pub fn next_month_plain(m: &Month) -> Option<String> {
    let (cause, v) = m.top_cause?;
    let upper = |t: &str| {
        let mut c = t.chars();
        c.next().map_or(String::new(), |f| f.to_uppercase().collect::<String>() + c.as_str())
    };
    Some(format!("{} cost {}. {}.", upper(plain_cause(cause)), usd(v), upper(miss_hint(cause))))
}

/// The weeks of the month for its mini calendar: rows of seven local dates, Monday first, with
/// `None` before the first day and after the last.
pub fn weeks(m: &Month) -> Vec<[Option<usize>; 7]> {
    let mut rows: Vec<[Option<usize>; 7]> = Vec::new();
    let Some(first) = m.days.first() else { return rows };
    let mut row = [None; 7];
    let mut col = weekday(first.date);
    for (i, _) in m.days.iter().enumerate() {
        row[col] = Some(i);
        col += 1;
        if col == 7 {
            rows.push(row);
            row = [None; 7];
            col = 0;
        }
    }
    if row.iter().any(Option::is_some) {
        rows.push(row);
    }
    rows
}

/// The whole text of `tokprof month`.
pub fn text(m: &Month, style: &Style) -> String {
    let mut out = format!("tokprof · month · {}\n", headline(m));
    if !m.any() {
        out.push_str("No requests and no messages in this month.\n");
        return out;
    }
    out.push('\n');
    match &m.title {
        Some(t) => {
            let _ = writeln!(out, "{}", style.bold(t.name));
            let _ = writeln!(out, "{}", style.dim(&t.rule));
        }
        None => {
            let _ = writeln!(out, "{}", style.dim("no title this month: no rule of the six held"));
        }
    }
    out.push('\n');
    let big = big_numbers(m);
    for row in big.chunks(3) {
        let cells: Vec<String> = row
            .iter()
            .map(|(v, what)| {
                let width = v.chars().count() + 1 + what.chars().count();
                format!("{} {what}{}", style.bold(v), " ".repeat(24usize.saturating_sub(width)))
            })
            .collect();
        let _ = writeln!(out, "{}", cells.join("").trim_end());
    }
    out.push('\n');
    let _ = writeln!(out, "     Mo Tu We Th Fr Sa Su");
    for row in weeks(m) {
        let mut line = "    ".to_string();
        for cell in row {
            match cell {
                Some(i) => {
                    let lvl = level(m.days[i].amounts.usd, m.cuts);
                    let mark = MARKS[lvl].to_string();
                    let mark = if lvl == 0 { style.dim(&mark) } else { style.paint(["38;5;22", "38;5;28", "38;5;34", "38;5;40"][lvl - 1], &mark) };
                    let _ = write!(line, "  {mark}");
                }
                None => line.push_str("   "),
            }
        }
        let _ = writeln!(out, "{}", line.trim_end());
    }
    let _ = writeln!(out, "     spend by day: {}", crate::render_calendar::legend(m.cuts, Unit::Usd, style));
    out.push('\n');
    for (label, value) in facts(m) {
        let _ = writeln!(out, "{label:<15}{value}");
    }
    let (cache, note) = cache(m);
    let cache: Vec<String> = cache.into_iter().map(|(v, what)| format!("{v} {what}")).collect();
    let _ = writeln!(out, "{:<15}{}", "cache", cache.join(", "));
    if let Some(n) = note {
        let _ = writeln!(out, "{:<15}{n}", "");
    }
    if let Some((name, lines)) = deltas(m) {
        let _ = writeln!(out, "{:<15}{}", format!("vs {name}"), lines.join(", "));
    }
    if let Some(line) = next_month(m) {
        let _ = writeln!(out, "{:<15}{line}", "next month");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokprof_core::calendar::{Amounts, Day, Streak};
    use tokprof_core::month::{Brief, Title, Ym};

    fn october() -> Month {
        let first = Ym { year: 2026, month: 10 }.first_date();
        let days = (0..31)
            .map(|i| Day { date: first + i, amounts: Amounts { usd: (i % 5) as f64, requests: u64::from(i % 5 > 0), ..Amounts::default() }, ..Day::default() })
            .collect::<Vec<_>>();
        Month {
            ym: Some(Ym { year: 2026, month: 10 }),
            complete: true,
            total: Amounts {
                usd: 11_959.2,
                avoidable_usd: 1_328.4,
                requests: 95_000,
                agent_minutes: 12_740,
                keyboard_ms: 98 * 3_600_000,
                messages: 1_796,
                commits: 412,
                read_tokens: 93,
                prompt_tokens: 100,
                saved_usd: 41_200.0,
                ..Amounts::default()
            },
            sessions: 301,
            cold_returns: 218,
            cuts: tokprof_core::calendar::cuts(days.iter().map(|d| d.amounts.usd)),
            days,
            busiest_day: Some((first + 24, 1_221.0)),
            busiest_hour: Some((15, 0.08)),
            night_share: Some(0.18),
            longest: Some(Streak { from: first, to: first + 3 }),
            top_model: Some(("claude-opus-5-5".into(), 8_882.0, 0.74)),
            top_cause: Some((Cause::IdleTtl, 749.0)),
            previous: Some(Brief { ym: Ym { year: 2026, month: 9 }, usd: 9_859.2, agent_minutes: 10_940, avoidable_share: Some(0.123) }),
            title: Some(Title { name: "Night owl", rule: "34% of agent time 22:00–05:00 (title from 33%)".into() }),
            ..Month::default()
        }
    }

    #[test]
    fn the_card_as_text() {
        let text = text(&october(), &Style::with_color(false));
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines[0], "tokprof · month · October 2026 with agents");
        assert_eq!((lines[2], lines[3]), ("Night owl", "34% of agent time 22:00–05:00 (title from 33%)"));
        assert_eq!(lines[5], "$11,959 spent           212h agents worked      98h at the keyboard");
        assert_eq!(lines[6], "301 sessions            1,796 messages          412 commits by agents");
        assert_eq!(lines[8], "     Mo Tu We Th Fr Sa Su");
        assert_eq!(lines[9], "               ·  ░  ▒  ▓", "October 2026 starts on a Thursday");
        assert!(text.contains("\nbusiest day    Sun 25 Oct, $1,221\n"));
        assert!(text.contains("\nbusiest hour   15:00–16:00, 8% of agent time\n"));
        assert!(text.contains("\nlongest streak 4 days in a row, Thu 1 Oct – Sun 4 Oct\n"));
        assert!(
            text.contains("\ncache          93% of input read from cache, $41,200 saved by the cache, $1,328 (11.1%) avoidable on misses, 218 cold returns\n")
        );
        assert!(text.contains("\nvs September   spend ↑ $2,100 (+21%), agent time ↑ 30h (+16%), avoidable share ↓ 1.2 points (11.1%, was 12.3%)\n"), "{text}");
        assert!(text.contains("\nnext month     $749 went to idle > TTL. Compact or wrap up a big session before a long break.\n"));
        for private in ["~/", "/Users", "json-profiler"] {
            assert!(!text.contains(private));
        }
    }

    #[test]
    fn the_card_says_the_cause_in_plain_words() {
        assert_eq!(
            next_month_plain(&october()).unwrap(),
            "Coming back after the cache expired cost $749. Compact or wrap up a big session before a long break."
        );
        let m = Month { top_cause: Some((Cause::CompactionCall, 347.0)), ..october() };
        assert!(next_month_plain(&m).unwrap().starts_with("Compaction rewriting the context cost $347. "));
        // The command line keeps tokprof's names.
        assert!(next_month(&october()).unwrap().starts_with("$749 went to idle > TTL."));
    }

    #[test]
    fn no_previous_month_no_deltas_and_no_title() {
        let m = Month { previous: None, title: None, top_cause: None, ..october() };
        let text = text(&m, &Style::with_color(false));
        assert!(!text.contains("vs ") && !text.contains("next month"));
        assert!(text.contains("no title this month: no rule of the six held"));
        let empty = Month { ym: Some(Ym { year: 2026, month: 3 }), complete: true, ..Month::default() };
        assert_eq!(super::text(&empty, &Style::with_color(false)), "tokprof · month · March 2026 with agents\nNo requests and no messages in this month.\n");
    }
}
