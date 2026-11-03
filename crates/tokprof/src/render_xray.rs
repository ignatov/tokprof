//! The text of the X-ray: `tokprof`, `tokprof scan` and the right pane of `tokprof ui`.

use std::fmt::Write as _;
use std::path::Path;

use tokprof_core::advice;
use tokprof_core::analysis::{Component, MissEvent};
use tokprof_core::fmt::{Style, clip_end, count, human_duration, local_time, tokens, usd, wrap};
use tokprof_core::labels::display_path;
use tokprof_core::model::{BlockKind, CompactionTrigger};
use tokprof_core::profile::Values;
use tokprof_core::report::Spend;
use tokprof_core::setup::SetupCost;
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

/// The `client` line: one client alone, or each with its request count, most requests first;
/// `None` for a session without requests.
fn client_line(clients: &[(String, u64)]) -> Option<String> {
    match clients {
        [] => None,
        [(one, _)] => Some(one.clone()),
        many => Some(many.iter().map(|(c, n)| format!("{c} ×{n}")).collect::<Vec<_>>().join(" · ")),
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
    if let Some(line) = client_line(&x.clients) {
        let _ = writeln!(out, "{} {line}", label("client"));
    }
    if !x.models.is_empty() {
        let models = x.models.iter().map(|(m, n)| format!("{m} ×{n}")).collect::<Vec<_>>().join(", ");
        let _ = writeln!(out, "{} {models}", label("models"));
    }
    let _ = writeln!(out, "{} {}", label("time"), time_line(x.span_ms, x.requests, x.subagents));
    if let Some(r) = &x.rhythm {
        let _ = writeln!(out, "{}", r.line());
    }
    if let Some(context) = &x.context {
        out.push('\n');
        context_lines(&mut out, context, style);
        advice_lines(&mut out, x.advice.iter().map(|a| a.text.as_str()), style);
    }
    out.push('\n');
    spend_lines(&mut out, x, style);
    work_lines(&mut out, x, style);
    prompt_lines(&mut out, x, style);
    tool_lines(&mut out, x, style);
    setup_lines(&mut out, x, style);
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

/// The advice block, each item wrapped to fit the X-ray pane of `tokprof ui`. The day view and
/// the pause report lay out theirs the same way.
pub(crate) fn advice_lines<'a>(out: &mut String, items: impl IntoIterator<Item = &'a str>, style: &Style) {
    for (i, item) in items.into_iter().enumerate() {
        let label = if i == 0 { style.dim(&format!("{:<9}", "advice")) } else { " ".repeat(9) };
        for (j, part) in wrap(item, advice::WIDTH).iter().enumerate() {
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

/// Where the money went: by the kind of work, by what the work was, and by what the agent was
/// doing. Left out when the session is a single task of typed prompts with no tool call.
fn work_lines(out: &mut String, x: &XRay, style: &Style) {
    let (origins, tasks, activities) = (x.work.origins(), x.work.tasks(), x.work.activities());
    if origins.len() < 2 && tasks.len() < 2 && activities.len() < 2 {
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
    let _ = writeln!(out, "  {}", style.dim("by activity: what the agent was doing before each request"));
    let total: f64 = activities.iter().map(|(_, t)| key(&t.spend)).sum();
    let mut rest = (0, Spend::default());
    for (i, (activity, t)) in activities.iter().enumerate() {
        if i >= 6 {
            rest.0 += 1;
            rest.1.add(t.spend);
            continue;
        }
        let share = key(&t.spend) / total.max(1e-9);
        let _ = writeln!(out, "  {:<24} {}  {:5.1}%  {:>9}", clip_end(activity, 24), style.paint("33", &bar(share)), 100.0 * share, show(&t.spend));
    }
    if rest.0 > 0 {
        let share = key(&rest.1) / total.max(1e-9);
        let _ = writeln!(out, "  {:<24} {}  {:5.1}%  {:>9}", format!("{} more", rest.0), style.paint("33", &bar(share)), 100.0 * share, show(&rest.1));
    }
}

/// Spend by tool for the session: the top 8 rows by rent, then the rest and the total line.
fn tool_lines(out: &mut String, x: &XRay, style: &Style) {
    if !x.tools.rows.values().any(|r| r.calls > 0) {
        return;
    }
    out.push('\n');
    let _ = writeln!(out, "{} {}", style.dim(&format!("{:<9}", "tools")), style.dim("calls, sizes, rent and trips by tool (see tokprof tools)"));
    for line in crate::render_report::tool_rows(&x.tools, false, 8).lines() {
        let _ = writeln!(out, "  {line}");
    }
}

/// What every request carries before the work: the setup by source with what the main
/// conversation and the subagents start with, what the latest compaction left, and the hint for
/// the costliest source.
fn setup_lines(out: &mut String, x: &XRay, style: &Style) {
    let Some(v) = &x.setup else { return };
    out.push('\n');
    let show = |c: &Values| if v.priced { usd(c.usd) } else { format!("{} tok", tokens(c.tokens)) };
    let _ = writeln!(
        out,
        "{} what every request carries before the work, subagents included: {}, {:.0}% of the session",
        style.dim(&format!("{:<9}", "setup")),
        show(&v.total.all),
        100.0 * v.share
    );
    let starts = |start: Option<f64>| start.filter(|t| *t > 0.0).map_or_else(|| "–".to_string(), tokens);
    let main_of = |source: &str| v.main.as_ref().and_then(|m| m.by_source.get(source).copied());
    let sub_of = |source: &str| v.subagents.as_ref().and_then(|m| m.by_source.get(source).copied());
    let row = |name: &str, cost: &SetupCost, main: Option<f64>, sub: Option<f64>| {
        if v.has_subagents {
            format!("  {:<24} {:>9} {:>10}  {:>11}  {:>13}", clip_end(name, 24), show(&cost.all), show(&cost.subagents), starts(main), starts(sub))
        } else {
            format!("  {:<24} {:>9}  {:>11}", clip_end(name, 24), show(&cost.all), starts(main))
        }
    };
    let head = if v.has_subagents {
        format!("  {:<24} {:>9} {:>10}  {:>11}  {:>13}", "source", "cost", "subagents", "main starts", "each subagent")
    } else {
        format!("  {:<24} {:>9}  {:>11}", "source", "cost", "main starts")
    };
    let _ = writeln!(out, "{}", style.dim(&head));
    let sum = |names: &[&str], of: &dyn Fn(&str) -> Option<f64>| {
        let parts: Vec<f64> = names.iter().filter_map(|n| of(n)).collect();
        (!parts.is_empty()).then(|| parts.iter().sum::<f64>())
    };
    for (name, cost) in v.rows.iter().take(8) {
        let _ = writeln!(out, "{}", row(name, cost, main_of(name), sub_of(name)));
    }
    if v.rows.len() > 8 {
        let rest: Vec<&str> = v.rows[8..].iter().map(|(n, _)| n.as_str()).collect();
        let mut cost = SetupCost::default();
        for (_, c) in &v.rows[8..] {
            cost.all.add(c.all);
            cost.subagents.add(c.subagents);
        }
        let _ = writeln!(out, "{}", row(&format!("{} more", rest.len()), &cost, sum(&rest, &main_of), sum(&rest, &sub_of)));
    }
    let mut total = row("total", &v.total, v.main.as_ref().map(|m| m.setup()), v.subagents.as_ref().map(|m| m.setup()));
    if let (true, Some(m)) = (v.has_subagents, &v.subagents) {
        let _ = write!(total, "   {} on average, and a {} brief", count(m.count as u64, "subagent", "subagents"), tokens(m.brief));
    }
    let _ = writeln!(out, "{}", style.bold(&total));
    if let Some(left) = &v.leftover {
        let _ = writeln!(out, "  after the compaction at {}, the next prompt held {}:", local_time(left.at_ms, "%m-%d %H:%M"), tokens(left.size_tokens as f64));
        let mut parts: Vec<String> = left.parts.iter().take(4).map(|(name, t)| format!("{name} {}", tokens(*t))).collect();
        if left.parts.len() > 4 {
            let rest: f64 = left.parts[4..].iter().map(|(_, t)| t).sum();
            parts.push(format!("{} more {}", left.parts.len() - 4, tokens(rest)));
        }
        let _ = writeln!(out, "    {}", style.dim(&parts.join(" · ")));
    }
    if let Some((source, hint)) = &v.hint {
        let mut name = source.clone();
        if let Some(first) = name.get(..1) {
            name = first.to_uppercase() + &name[1..];
        }
        let text = format!("{name} cost the most of the setup: {hint}.");
        for (j, part) in wrap(&text, advice::WIDTH).iter().enumerate() {
            let lead = if j == 0 { format!("  {} ", style.paint("33", "▸")) } else { " ".repeat(4) };
            let _ = writeln!(out, "{lead}{part}");
        }
    }
}

/// What each typed message cost: the 8 most expensive in session order, then the rest together.
fn prompt_lines(out: &mut String, x: &XRay, style: &Style) {
    if x.prompts.is_empty() {
        return;
    }
    let key = |s: &Spend| if x.priced { s.usd } else { s.tokens };
    let show = |s: &Spend| if x.priced { usd(s.usd) } else { format!("{} tok", tokens(s.tokens)) };
    let mut ranked: Vec<usize> = (0..x.prompts.len()).collect();
    ranked.sort_by(|&a, &b| key(&x.prompts[b].spend).total_cmp(&key(&x.prompts[a].spend)).then(a.cmp(&b)));
    let mut shown: Vec<usize> = ranked.iter().copied().take(8).collect();
    shown.sort_unstable();
    out.push('\n');
    let _ = writeln!(out, "{} {}", style.dim(&format!("{:<9}", "prompts")), style.dim("what each message you typed cost, subagents included"));
    for &i in &shown {
        let p = &x.prompts[i];
        let acts = p.activities.iter().take(2).map(|(a, s)| format!("{a} {}", show(s))).collect::<Vec<_>>().join(" · ");
        let _ = writeln!(out, "  {:>3}  {:>8}  {:>4} req  {:<40}  {}", p.number, show(&p.spend), p.spend.requests, clip_end(&p.label, 40), style.dim(&acts));
    }
    if x.prompts.len() > shown.len() {
        let mut rest = Spend::default();
        for (i, p) in x.prompts.iter().enumerate() {
            if !shown.contains(&i) {
                rest.add(p.spend);
            }
        }
        let n = x.prompts.len() - shown.len();
        let _ = writeln!(out, "  {}", style.dim(&format!("{n} more messages  {}  {} req", show(&rest), rest.requests)));
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
    let _ = writeln!(out, "{}", style.dim("what cost the most to keep in the context (each source with every request that re-read it)"));
    let mut rows: Vec<_> = x.content.iter().collect();
    let key = |v: &Values| if x.priced { v.usd } else { v.tokens };
    rows.sort_by(|a, b| key(&b.1).total_cmp(&key(&a.1)));
    for ((kind, tool, target), v) in rows.into_iter().take(8) {
        let what = [tool.as_deref(), target.as_deref()].into_iter().flatten().collect::<Vec<_>>().join("  ");
        let amount = if x.priced { usd(v.usd) } else { tokens(v.tokens) };
        let calls = match kind {
            BlockKind::ToolUse | BlockKind::ToolResult => x.calls.get(&(tool.clone(), target.clone())).map_or(String::new(), |&n| count(n, "call", "calls")),
            _ => String::new(),
        };
        let _ = writeln!(out, "  {amount:>9}  {:<14} {:<60} {}", kind.name(), clip(&what, 60), style.dim(&calls));
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

    fn xray(prompts: Vec<tokprof_core::xray::PromptSpend>) -> XRay {
        XRay {
            agent: tokprof_core::source::AgentId::new(tokprof_core::source::AgentInfo::named("claude")),
            project: "~/p".into(),
            id: "s".into(),
            short_id: "s".into(),
            title: None,
            models: Vec::new(),
            clients: Vec::new(),
            span_ms: None,
            requests: 0,
            subagents: 0,
            context: None,
            advice: Vec::new(),
            priced: true,
            components: Default::default(),
            chains: Vec::new(),
            work: Default::default(),
            composition: None,
            content: Vec::new(),
            calls: Default::default(),
            prompts,
            tools: Default::default(),
            setup: None,
            misses: Vec::new(),
            rhythm: None,
        }
    }

    fn spend(requests: u64, usd: f64) -> Spend {
        Spend { requests, tokens: 1.0, usd }
    }

    fn prompt(number: usize, label: &str, usd: f64, activities: Vec<(&'static str, Spend)>) -> tokprof_core::xray::PromptSpend {
        tokprof_core::xray::PromptSpend { number, label: label.into(), spend: spend(10, usd), activities }
    }

    fn render(x: &XRay, f: fn(&mut String, &XRay, &Style)) -> String {
        let mut out = String::new();
        f(&mut out, x, &Style::with_color(false));
        out
    }

    #[test]
    fn a_prompt_line_names_its_two_costliest_activities() {
        let acts = vec![("reading", spend(30, 3.10)), ("editing", spend(20, 2.90)), ("prompt", spend(1, 0.5))];
        let x = xray(vec![prompt(1, "first", 1.0, Vec::new()), prompt(3, "одобряю add-request-trigger, применяй после сплита", 8.53, acts)]);
        let text = render(&x, prompt_lines);
        assert!(text.contains("prompts   what each message you typed cost, subagents included"), "{text}");
        let line = text.lines().find(|l| l.contains("$8.53")).unwrap();
        assert!(line.contains("  3     $8.53    10 req  одобряю add-request-trigger, применяй п…  reading $3.10 · editing $2.90"), "{line}");
        assert!(render(&xray(Vec::new()), prompt_lines).is_empty(), "no block for fewer than two messages");
    }

    #[test]
    fn many_prompts_show_the_eight_costliest_in_order_and_the_rest_together() {
        let prompts: Vec<_> = (1..=30).map(|n| prompt(n, &format!("message {n}"), n as f64, Vec::new())).collect();
        let total: f64 = prompts.iter().map(|p| p.spend.usd).sum();
        let text = render(&xray(prompts), prompt_lines);
        let numbers: Vec<&str> = text.lines().filter(|l| l.contains(" req  message ")).map(|l| l.split_whitespace().next().unwrap()).collect();
        assert_eq!(numbers, ["23", "24", "25", "26", "27", "28", "29", "30"]);
        let shown: f64 = (23..=30).map(f64::from).sum();
        assert!(text.contains(&format!("22 more messages  {}  220 req", usd(total - shown))), "{text}");
    }

    #[test]
    fn the_tools_block_shows_the_top_rows_and_their_hints() {
        use tokprof_core::model::{Class, ToolClass};
        use tokprof_core::tools::ToolRow;
        let mut x = xray(Vec::new());
        assert!(render(&x, tool_lines).is_empty(), "no tool calls, no block");
        let v = |usd| Values { tokens: 1.0, usd, avoidable: 0.0 };
        x.tools.rows.insert(("Bash".into(), Some("sed -n".into())), ToolRow { class: Class::Tool(ToolClass::Execute), calls: 56, single: 48, results: 56, result_tokens: 56.0 * 1_300.0, inputs: 56, input_tokens: 56.0 * 53.0, rent: v(5.21), trips: v(3.75) });
        x.tools.spend = v(27.37);
        let text = render(&x, tool_lines);
        assert!(text.starts_with("\ntools     calls, sizes, rent and trips by tool (see tokprof tools)"), "{text}");
        let row = text.lines().find(|l| l.contains("sed -n")).unwrap();
        assert_eq!(row.split_whitespace().collect::<Vec<_>>(), ["Bash", "sed", "-n", "56", "1.3k", "53", "$5.21", "$3.75", "86%", "batch", "independent", "reads"]);
        assert!(text.contains("spend $27.37 · rent of these tools $5.21 · trips of these tools $3.75 (two views of the same money; do not add them)"), "{text}");
    }

    #[test]
    fn the_work_block_lists_activities() {
        use tokprof_core::report::TriggerSpend;
        let mut x = xray(Vec::new());
        for (a, u) in [("reading", 5.42), ("editing", 4.71), ("prompt", 2.14)] {
            x.work.by_activity.insert(a, TriggerSpend { spend: spend(1, u), prompt_tokens: 1.0 });
        }
        let text = render(&x, work_lines);
        let names: Vec<&str> = text.lines().skip_while(|l| !l.contains("by activity")).skip(1).map(|l| l.split_whitespace().next().unwrap()).collect();
        assert_eq!(names, ["reading", "editing", "prompt"]);
        let mut one = xray(Vec::new());
        one.work.by_activity.insert("prompt", TriggerSpend { spend: spend(1, 1.0), prompt_tokens: 1.0 });
        assert!(render(&one, work_lines).is_empty(), "typed prompts only, with no tool call");
    }

    #[test]
    fn tool_rows_of_what_cost_the_most_count_their_calls() {
        let mut x = xray(Vec::new());
        let key = |t: &str| (Some("Bash".to_string()), Some(t.to_string()));
        x.content = vec![((BlockKind::ToolResult, key("sed -n").0, key("sed -n").1), Values { tokens: 1.0, usd: 2.75, avoidable: 0.0 }), ((BlockKind::ToolUse, key("cat >").0, key("cat >").1), Values { tokens: 1.0, usd: 1.0, avoidable: 0.0 })];
        x.calls.insert(key("sed -n"), 412);
        x.calls.insert(key("cat >"), 1);
        let text = render(&x, costly_content_lines);
        assert!(text.starts_with("what cost the most to keep in the context (each source with every request that re-read it)"), "{text}");
        assert!(text.lines().any(|l| l.contains("sed -n") && l.trim_end().ends_with("412 calls")), "{text}");
        assert!(text.lines().any(|l| l.contains("cat >") && l.trim_end().ends_with("1 call")), "{text}");
    }

    fn setup_view(has_subagents: bool) -> tokprof_core::setup::SetupView {
        use tokprof_core::setup::{MeanStart, SetupCost, SetupView, Start};
        let cost = |all: f64, sub: f64| SetupCost {
            all: Values { tokens: all * 5_000.0, usd: all, avoidable: 0.0 },
            subagents: Values { tokens: sub * 5_000.0, usd: sub, avoidable: 0.0 },
        };
        let by = |pairs: &[(&str, f64)]| pairs.iter().map(|(k, v)| (k.to_string(), *v)).collect();
        SetupView {
            total: cost(73.90, if has_subagents { 70.23 } else { 0.0 }),
            share: 73.90 / 526.0,
            priced: true,
            rows: vec![("instructions".into(), cost(23.56, 22.10)), ("skills".into(), cost(21.92, 21.03)), ("tool schemas".into(), cost(28.42, 27.10))],
            has_subagents,
            main: has_subagents.then(|| Start { by_source: by(&[("instructions", 11_100.0), ("skills", 7_100.0)]), brief: 600.0 }),
            subagents: has_subagents.then(|| MeanStart { count: 96, by_source: by(&[("instructions", 9_700.0), ("skills", 9_600.0), ("tool schemas", 11_900.0)]), brief: 1_800.0 }),
            leftover: None,
            hint: Some(("skills".into(), "lower skillListingBudgetFraction (default 0.01 of the context window), or turn unused skills off with skillOverrides in settings.json".into())),
        }
    }

    #[test]
    fn the_setup_block_shows_sources_starts_and_the_hint() {
        let mut x = xray(Vec::new());
        x.setup = Some(setup_view(true));
        let text = render(&x, setup_lines);
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines[1], "setup     what every request carries before the work, subagents included: $73.90, 14% of the session");
        assert!(lines[2].contains("source") && lines[2].contains("subagents") && lines[2].contains("main starts") && lines[2].contains("each subagent"), "{text}");
        assert!(lines[3].contains("instructions") && lines[3].contains("$23.56") && lines[3].contains("$22.10") && lines[3].contains("11.1k") && lines[3].contains("9.7k"), "{text}");
        assert!(lines[5].contains("tool schemas") && lines[5].contains("–"), "no tool schemas in the main start: {text}");
        let total = lines.iter().find(|l| l.trim_start().starts_with("total")).unwrap();
        assert!(total.contains("$73.90") && total.contains("$70.23") && total.contains("31.2k"), "{text}");
        assert!(total.ends_with("96 subagents on average, and a 1.8k brief"), "{text}");
        assert!(text.contains("  ▸ Skills cost the most of the setup: lower skillListingBudgetFraction"), "{text}");
        assert!(text.lines().last().unwrap().starts_with("    ") && text.trim_end().ends_with("settings.json."), "the hint wraps: {text}");
    }

    #[test]
    fn a_setup_without_subagents_has_no_subagent_columns() {
        let mut x = xray(Vec::new());
        x.setup = Some(setup_view(false));
        let text = render(&x, setup_lines);
        assert!(!text.contains("each subagent") && !text.contains("subagents on average"), "{text}");
        assert!(text.lines().nth(3).unwrap().trim_end().ends_with('–'), "an unknown main start: {text}");
    }

    #[test]
    fn the_setup_block_says_what_a_compaction_left() {
        let mut v = setup_view(true);
        let at = 1_791_905_499_953;
        let parts = [("background notifications", 37_100.0), ("attached files", 20_300.0), ("user prompt", 13_200.0), ("skills", 11_400.0), ("tool schemas", 8_300.0), ("other", 1_200.0)];
        v.leftover = Some(tokprof_core::setup::Leftover { at_ms: at, size_tokens: 134_857, parts: parts.iter().map(|(k, t)| (k.to_string(), *t)).collect() });
        let mut x = xray(Vec::new());
        x.setup = Some(v);
        let text = render(&x, setup_lines);
        assert!(text.contains(&format!("  after the compaction at {}, the next prompt held 134.9k:", local_time(at, "%m-%d %H:%M"))), "{text}");
        assert!(text.contains("    background notifications 37.1k · attached files 20.3k · user prompt 13.2k · skills 11.4k · 2 more 9.5k"), "{text}");
    }

    #[test]
    fn no_setup_block_below_its_thresholds() {
        assert_eq!(render(&xray(Vec::new()), setup_lines), "");
    }

    #[test]
    fn the_header_names_the_clients() {
        let header = |clients: &[(&str, u64)], title: Option<&str>| {
            let mut x = xray(Vec::new());
            x.clients = clients.iter().map(|(c, n)| (c.to_string(), *n)).collect();
            x.title = title.map(str::to_string);
            text(&x, &Style::with_color(false), Path::new("/tmp"), Path::new("/h"))
        };
        // One client, on the line after the title.
        let one = header(&[("sdk-ts", 657)], Some("Fix the parser"));
        let lines: Vec<&str> = one.lines().collect();
        assert_eq!((lines[2], lines[3]), ("title     Fix the parser", "client    sdk-ts"), "{one}");
        // Several, most requests first.
        assert!(header(&[("cli", 412), ("sdk-ts", 37)], None).contains("\nclient    cli ×412 · sdk-ts ×37\n"));
        // None of the requests had one.
        assert!(header(&[("unknown", 5)], None).contains("\nclient    unknown\n"));
        // A session without requests has no line.
        assert!(!header(&[], None).contains("client"));
    }

    #[test]
    fn the_rhythm_line_follows_time() {
        let mut x = xray(Vec::new());
        let plain = text(&x, &Style::with_color(false), Path::new("/tmp"), Path::new("/h"));
        assert!(!plain.contains("rhythm"), "no turns, no line: {plain}");
        x.rhythm = Some(tokprof_core::rhythm::SessionRhythm {
            turns: 34,
            work_ms: 126 * 60_000,
            turn_median: Some(210_000),
            answer_median: Some(240_000),
            self_started: 12,
            interrupts: 2,
            questions: 0,
        });
        let shown = text(&x, &Style::with_color(false), Path::new("/tmp"), Path::new("/h"));
        let lines: Vec<&str> = shown.lines().collect();
        let at = lines.iter().position(|l| l.starts_with("time")).unwrap();
        assert_eq!(lines[at + 1], "rhythm    34 turns, 2.1h of work · the agent's turn median 3.5m · your answer median 4.0m · 12 it started itself · 2 interrupts");
    }
}
