//! The text of the X-ray: `tokprof`, `tokprof scan` and the right pane of `tokprof ui`.

use std::fmt::Write as _;
use std::path::Path;

use tokprof_core::advice::{self, Advice};
use tokprof_core::analysis::{Component, MissEvent};
use tokprof_core::fmt::{Style, clip_end, count, human_duration, local_time, tokens, usd, wrap};
use tokprof_core::labels::display_path;
use tokprof_core::model::{BlockKind, CompactionTrigger};
use tokprof_core::profile::Values;
use tokprof_core::report::Spend;
use tokprof_core::xray::{Composition, ContextNow, XRay};

const BAR: usize = 32;

fn bar(share: f64) -> String {
    let filled = (share.clamp(0.0, 1.0) * BAR as f64).round() as usize;
    format!("{}{}", "█".repeat(filled), "░".repeat(BAR - filled))
}

fn component_color(c: Component) -> &'static str {
    match c {
        Component::Read => "36",
        Component::Write => "33",
        Component::Rewrite => "31",
        Component::Uncached => "35",
        Component::Output => "34",
        _ => "37",
    }
}

/// The X-ray as text. Its last line names the profile `tokprof flame` writes into `profile_dir`.
pub fn text(x: &XRay, style: &Style, profile_dir: &Path, home: &Path) -> String {
    let mut out = String::new();
    let label = |s: &str| style.dim(&format!("{s:<9}"));
    let _ = writeln!(out, "{}", style.bold("tokprof · session X-ray"));
    let _ = writeln!(out, "{} {}  {}  {}", label("session"), x.agent.name(), style.bold(&x.project), x.id);
    if let Some(title) = &x.title {
        let _ = writeln!(out, "{} {}", label("title"), clip_end(title, 82));
    }
    if !x.models.is_empty() {
        let models = x.models.iter().map(|(m, n)| format!("{m} ×{n}")).collect::<Vec<_>>().join(", ");
        let _ = writeln!(out, "{} {models}", label("models"));
    }
    let _ = writeln!(out, "{} {}", label("time"), time_line(x.span_ms, x.requests, x.subagents));
    if let Some(context) = &x.context {
        out.push('\n');
        context_lines(&mut out, context, style);
        advice_lines(&mut out, &x.advice, style);
    }
    out.push('\n');
    spend_lines(&mut out, x, style);
    work_lines(&mut out, x, style);
    if let Some(c) = &x.composition {
        out.push('\n');
        composition_lines(&mut out, x, c, style);
    }
    out.push('\n');
    costly_content_lines(&mut out, x, style);
    out.push('\n');
    miss_lines(&mut out, x, style);
    let _ = writeln!(
        out,
        "\n{}",
        style.dim(&format!(
            "profile: tokprof flame --session {id} → {}/{id}.jfr (IntelliJ: Open snapshot)",
            display_path(&profile_dir.to_string_lossy(), None, home),
            id = x.short_id
        ))
    );
    out
}

/// The `time` line after its label: the first and last request, the span, and the counts; only
/// the counts when no request has a time.
fn time_line(span: Option<(i64, i64)>, requests: u64, subagents: usize) -> String {
    let mut line = String::new();
    if let Some((first, last)) = span {
        let same_day = local_time(first, "%F") == local_time(last, "%F");
        line = format!(
            "{} → {} ({}) · ",
            local_time(first, "%Y-%m-%d %H:%M"),
            local_time(last, if same_day { "%H:%M" } else { "%Y-%m-%d %H:%M" }),
            human_duration((last - first) as f64 / 1000.0)
        );
    }
    line += &count(requests, "request", "requests");
    if subagents > 0 {
        line += &format!(" · {}", count(subagents as u64, "subagent", "subagents"));
    }
    line
}

/// A forecast as a share of the session's cost, as ` = 6% of the session's $107`; empty when
/// the cost would read `$0.00`, since a share of that says nothing.
fn of_session(usd_next: f64, spend: f64) -> String {
    if spend < 0.005 {
        return String::new();
    }
    format!(" = {:.0}% of the session's {}", 100.0 * usd_next / spend, usd(spend))
}

fn context_lines(out: &mut String, c: &ContextNow, style: &Style) {
    let (size, approx) = (c.size_tokens, if c.approx { "~" } else { "" });
    let label = style.dim(&format!("{:<9}", "context"));
    match c.window_tokens {
        Some(w) if w > 0 => {
            let share = size as f64 / w as f64;
            let _ = writeln!(out, "{label} {}  {:5.1}%  {approx}{} of {}", bar(share), 100.0 * share, tokens(size as f64), tokens(w as f64));
        }
        _ => {
            let _ = writeln!(out, "{label} {approx}{} tokens", tokens(size as f64));
        }
    }
    let of_session = |x: f64| of_session(x, c.spend_usd);
    if let Some(t) = &c.trailing {
        let when = local_time(t.at_ms, "%H:%M");
        let how = match (&t.kind, t.kind.name()) {
            (CompactionTrigger::Idle, _) => format!("compacted while idle at {when}"),
            (_, Some(kind)) => format!("compacted at {when} ({kind})"),
            (_, None) => format!("compacted at {when}"),
        };
        let estimate = tokens(t.estimate_tokens);
        let _ = writeln!(out, "{} {how} · the next request re-builds ~{estimate}", style.dim(&format!("{:<9}", "cache")));
        if let Some(next) = t.next_usd {
            let _ = writeln!(out, "{} ≈ {} for one request, re-building ~{estimate}{}", style.dim(&format!("{:<9}", "next")), usd(next), of_session(next));
        }
        return;
    }
    let cache = &c.cache;
    let label = style.dim(&format!("{:<9}", "cache"));
    let ttl = cache.ttl_name;
    if !cache.cold() {
        let left = human_duration((cache.ttl_ms - cache.idle_ms) as f64 / 1000.0);
        let _ = writeln!(out, "{label} {} · {ttl} TTL · cold in {left}", style.paint("32", "warm"));
    } else {
        let idle = human_duration((cache.idle_ms - cache.ttl_ms) as f64 / 1000.0);
        let _ = writeln!(out, "{label} {} for {idle} ({ttl} TTL)", style.paint("31", "cold"));
    }

    // What the next request will cost, against what the whole session cost so far.
    let (Some(warm), Some(cold)) = (cache.next_warm_usd, cache.next_cold_usd) else { return };
    let label = style.dim(&format!("{:<9}", "next"));
    if cache.cold() {
        let line = format!("≈ {} for one request, re-writing {}{}", usd(cold), tokens(size as f64), of_session(cold));
        let _ = writeln!(out, "{label} {}", style.paint("31", &line));
    } else {
        let _ = writeln!(
            out,
            "{label} ≈ {} per request while warm · ≈ {} once cold{}",
            style.paint("32", &usd(warm)),
            style.paint("31", &usd(cold)),
            of_session(cold)
        );
    }
}

/// The advice, each item wrapped to fit the X-ray pane of `tokprof ui`.
fn advice_lines(out: &mut String, items: &[Advice], style: &Style) {
    for (i, item) in items.iter().enumerate() {
        let label = if i == 0 { style.dim(&format!("{:<9}", "advice")) } else { " ".repeat(9) };
        for (j, part) in wrap(&item.text, advice::WIDTH).iter().enumerate() {
            let lead = if j == 0 { format!("{label} {} ", style.paint("33", "▸")) } else { " ".repeat(12) };
            let _ = writeln!(out, "{lead}{part}");
        }
    }
}

fn spend_lines(out: &mut String, x: &XRay, style: &Style) {
    let total: f64 = x.components.iter().map(|v| if x.priced { v.usd } else { v.tokens }).sum();
    let show = |v: &Values| if x.priced { usd(v.usd) } else { format!("{} tok", tokens(v.tokens)) };
    let headline = if x.priced { format!("{} (API-equivalent)", usd(total)) } else { format!("{} tokens (no price)", tokens(total)) };
    let _ = writeln!(out, "{} {}", style.dim(&format!("{:<9}", "spend")), style.bold(&headline));
    for (c, v) in Component::ALL.iter().zip(&x.components) {
        let value = if x.priced { v.usd } else { v.tokens };
        let share = value / total.max(1e-9);
        if share < 0.0005 {
            continue;
        }
        let _ = writeln!(out, "  {:<19} {}  {:5.1}%  {:>9}", c.name(), style.paint(component_color(*c), &bar(share)), 100.0 * share, show(v));
    }
    let chains: Vec<String> = x.chains.iter().map(|(name, v)| format!("{name} {}", show(v))).collect();
    if chains.len() > 1 {
        let _ = writeln!(out, "  {}", style.dim(&chains.join(" · ")));
    }
}

/// Where the money went: by the kind of work, then by what the work was. Left out when the
/// session is a single task of typed prompts.
fn work_lines(out: &mut String, x: &XRay, style: &Style) {
    let (origins, tasks) = (x.work.origins(), x.work.tasks());
    if origins.len() < 2 && tasks.len() < 2 {
        return;
    }
    out.push('\n');
    let key = |s: &Spend| if x.priced { s.usd } else { s.tokens };
    let show = |s: &Spend| if x.priced { usd(s.usd) } else { format!("{} tok", tokens(s.tokens)) };
    let total: f64 = origins.iter().map(|(_, s)| key(s)).sum();
    let _ = writeln!(out, "{} {}", style.dim(&format!("{:<9}", "work")), style.dim("by origin: the kind of work each request was part of"));
    let mut rest = (0, Spend::default());
    for (i, (origin, s)) in origins.iter().enumerate() {
        if i >= 5 {
            rest.0 += 1;
            rest.1.add(*s);
            continue;
        }
        let share = key(s) / total.max(1e-9);
        let _ = writeln!(out, "  {:<24} {}  {:5.1}%  {:>9}", clip_end(origin, 24), style.paint("32", &bar(share)), 100.0 * share, show(s));
    }
    if rest.0 > 0 {
        let share = key(&rest.1) / total.max(1e-9);
        let _ = writeln!(out, "  {:<24} {}  {:5.1}%  {:>9}", format!("{} more", rest.0), style.paint("32", &bar(share)), 100.0 * share, show(&rest.1));
    }
    if tasks.len() > 1 {
        let _ = writeln!(out, "  {}", style.dim("by task: the session title, and what each subagent was asked to do"));
        for (task, origin, s) in tasks.iter().take(6) {
            let _ = writeln!(out, "  {:>9}  {:<50} {}", show(s), clip_end(task, 50), style.dim(&clip_end(origin, 26)));
        }
        if tasks.len() > 6 {
            let _ = writeln!(out, "  {}", style.dim(&format!("{} more tasks", tasks.len() - 6)));
        }
    }
}

fn composition_lines(out: &mut String, x: &XRay, c: &Composition, style: &Style) {
    let turns = match c.turns {
        0 => String::new(),
        1 => ", 1 turn".to_string(),
        n => format!(", {n} turns"),
    };
    let before = c.before_compaction_ms.map_or(String::new(), |t| format!(", before the compaction at {}", local_time(t, "%H:%M")));
    let _ = writeln!(
        out,
        "{} {} tokens{turns}, as of {}{before}",
        style.dim(&format!("{:<9}", "in context")),
        tokens(c.total_tokens),
        local_time(c.as_of_ms, "%H:%M")
    );
    if !c.known {
        let _ = writeln!(out, "  {}", style.dim(&format!("{} logs do not show what the prompt holds yet", x.agent.name())));
        return;
    }
    for (kind, t) in &c.by_kind {
        let share = t / c.total_tokens.max(1e-9);
        let _ = writeln!(out, "  {:<19} {}  {:5.1}%  {:>9}", kind.name(), style.paint("36", &bar(share)), 100.0 * share, tokens(*t));
    }
    if !c.largest.is_empty() {
        let _ = writeln!(out, "  {}", style.dim("largest blocks"));
    }
    for b in c.largest.iter().take(6) {
        let what = [b.tool.as_deref(), b.target.as_deref()].into_iter().flatten().collect::<Vec<_>>().join("  ");
        let when = if matches!(b.kind, BlockKind::Tools | BlockKind::System) { "setup".to_string() } else { format!("turn {}", b.turn) };
        let _ = writeln!(out, "  {:>9}  {:<14} {:<50} {}", tokens(b.tokens), b.kind.name(), clip(&what, 50), style.dim(&when));
    }
}

fn costly_content_lines(out: &mut String, x: &XRay, style: &Style) {
    let _ = writeln!(out, "{}", style.dim("what cost the most (every request it was part of, re-reads included)"));
    let mut rows: Vec<_> = x.content.iter().collect();
    let key = |v: &Values| if x.priced { v.usd } else { v.tokens };
    rows.sort_by(|a, b| key(&b.1).total_cmp(&key(&a.1)));
    for ((kind, tool, target), v) in rows.into_iter().take(8) {
        let what = [tool.as_deref(), target.as_deref()].into_iter().flatten().collect::<Vec<_>>().join("  ");
        let amount = if x.priced { usd(v.usd) } else { tokens(v.tokens) };
        let _ = writeln!(out, "  {amount:>9}  {:<14} {}", kind.name(), clip(&what, 60));
    }
}

fn miss_lines(out: &mut String, x: &XRay, style: &Style) {
    let misses: Vec<&MissEvent> = x.misses.iter().filter(|m| !m.cause.expected()).collect();
    let expected = x.misses.len() - misses.len();
    if x.misses.is_empty() {
        let _ = writeln!(out, "{} {}", style.dim(&format!("{:<9}", "cache")), style.paint("32", "no misses"));
        return;
    }
    let rewritten: u64 = misses.iter().map(|m| m.rewritten).sum();
    let premium: f64 = misses.iter().filter_map(|m| m.premium_usd).sum();
    let head = format!(
        "{} · re-processed {} tokens{}{}",
        count(misses.len() as u64, "miss", "misses"),
        tokens(rewritten as f64),
        if x.priced { format!(" · {} avoidable", usd(premium)) } else { String::new() },
        if expected > 0 { format!(" · {}", count(expected as u64, "expected rebuild", "expected rebuilds")) } else { String::new() }
    );
    let _ = writeln!(out, "{} {}", style.dim(&format!("{:<9}", "cache")), if misses.is_empty() { head } else { style.paint("31", &head) });
    let mut top: Vec<&&MissEvent> = misses.iter().collect();
    top.sort_by(|a, b| b.premium_usd.unwrap_or(0.0).total_cmp(&a.premium_usd.unwrap_or(0.0)).then(b.rewritten.cmp(&a.rewritten)));
    for m in top.into_iter().take(5) {
        let cause = if m.detail.is_empty() { m.cause.name().to_string() } else { format!("{}: {}", m.cause.name(), m.detail) };
        let _ = writeln!(
            out,
            "  {}  {:<44} {:>8}  {:>8}  {}",
            local_time(m.ts_ms, "%m-%d %H:%M"),
            clip(&cause, 44),
            tokens(m.rewritten as f64),
            m.premium_usd.map_or("-".to_string(), usd),
            style.dim(&m.chain)
        );
    }
}

fn clip(s: &str, width: usize) -> String {
    if s.chars().count() <= width {
        return s.to_string();
    }
    let tail: String = s.chars().rev().take(width - 1).collect::<Vec<_>>().into_iter().rev().collect();
    format!("…{tail}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokprof_core::forecast::CacheState;
    use tokprof_core::xray::TrailingCompaction;

    #[test]
    fn the_share_of_a_cost_that_reads_zero_is_left_out() {
        assert_eq!(of_session(0.01, 0.004), "");
        assert_eq!(of_session(0.01, 0.0), "");
        assert_eq!(of_session(0.03, 0.02), " = 150% of the session's $0.02");
        assert_eq!(of_session(6.57, 107.0), " = 6% of the session's $107");
    }

    #[test]
    fn a_session_without_times_shows_only_its_counts() {
        assert_eq!(time_line(None, 0, 0), "0 requests");
        assert_eq!(time_line(None, 12, 1), "12 requests · 1 subagent");
        use chrono::TimeZone;
        let at = |h: u32, m: u32| chrono::Local.with_ymd_and_hms(2026, 11, 1, h, m, 0).unwrap().timestamp_millis();
        assert_eq!(time_line(Some((at(8, 49), at(18, 15))), 657, 0), "2026-11-01 08:49 → 18:15 (9.4h) · 657 requests");
    }

    #[test]
    fn a_trailing_compaction_replaces_the_cache_line() {
        let at = 115 * 60_000;
        let context = |kind| ContextNow {
            size_tokens: 55_800,
            approx: true,
            window_tokens: Some(1_000_000),
            cache: CacheState { long_ttl: true, ttl_ms: 3_600_000, ttl_name: "1h", idle_ms: 0, rebuild_usd: None, next_warm_usd: None, next_cold_usd: None },
            trailing: Some(TrailingCompaction { at_ms: at, kind, estimate_tokens: 55_800.0, next_usd: Some(0.37) }),
            spend_usd: 414.0,
        };
        let render = |kind| {
            let mut out = String::new();
            context_lines(&mut out, &context(kind), &Style::with_color(false));
            out
        };
        let when = local_time(at, "%H:%M");
        let idle = render(CompactionTrigger::Idle);
        assert!(idle.contains("~55.8k of 1.0M"), "{idle}");
        assert!(idle.contains(&format!("cache     compacted while idle at {when} · the next request re-builds ~55.8k")), "{idle}");
        assert!(idle.contains("for one request, re-building ~55.8k = ") && idle.contains("of the session's $414"), "{idle}");
        assert!(!idle.contains("warm") && !idle.contains("cold"), "{idle}");
        let auto = render(CompactionTrigger::Auto);
        assert!(auto.contains(&format!("cache     compacted at {when} (auto) · the next request re-builds ~55.8k")), "{auto}");
    }
}
