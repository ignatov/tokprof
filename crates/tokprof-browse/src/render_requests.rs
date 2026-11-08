//! `tokprof scan --requests`: the requests of a session's main conversation, one row each with
//! its tokens, cost and why, and lines of what entered its context and what changed.

use std::fmt::Write as _;

use tokprof_core::analysis::Component;
use tokprof_core::fmt::{Style, count, local_time, tokens, usd};
use tokprof_core::timeline::{Change, RequestPoint, why};
use tokprof_core::xray::XRay;

/// Entered groups a row's line shows.
const ENTERED_SHOWN: usize = 3;

/// The list as text.
pub fn text(x: &XRay, style: &Style) -> String {
    let mut out = String::new();
    let label = |s: &str| style.dim(&format!("{s:<9}"));
    let _ = writeln!(out, "{}", style.bold("tokprof · requests of the main conversation"));
    let _ = writeln!(out, "{} {}  {}  {}", label("session"), x.agent.name(), style.bold(&x.project), x.id);
    let points = &x.points;
    let _ = writeln!(out, "{} {}", label("requests"), head(points));
    if points.is_empty() {
        return out;
    }
    let unknown = points.iter().filter(|p| p.entered.is_none()).count();
    if unknown > 0 {
        let _ = writeln!(out, "          {}", style.dim(&format!("the log does not show what {} held", count(unknown as u64, "prompt", "prompts"))));
    }
    out.push('\n');
    let _ = writeln!(
        out,
        "{}",
        style.dim(&format!("{:>6}  {:<14}{:>8}{:>8}{:>10}{:>8}{:>8}{:>8}  why", "#", "sent", "read", "write", "uncached", "output", "context", "cost"))
    );
    for (i, p) in points.iter().enumerate() {
        out += &row(i + 1, p, style);
    }
    out
}

/// The `requests` line: the count, the cost and its parts that are not zero.
fn head(points: &[RequestPoint]) -> String {
    let n = count(points.len() as u64, "request", "requests");
    if points.is_empty() {
        return n;
    }
    let priced = points.iter().any(|p| p.priced);
    let show = |v: f64| if priced { usd(v) } else { format!("{} tokens", tokens(v)) };
    let part = |i: usize| points.iter().map(|p| if priced { p.parts[i].usd } else { p.parts[i].tokens }).sum::<f64>();
    let total: f64 = (0..5).map(part).sum();
    let parts: Vec<String> = Component::ALL
        .iter()
        .enumerate()
        .filter(|(i, _)| if priced { part(*i) >= 0.005 } else { part(*i) >= 0.5 })
        .map(|(i, c)| format!("{} {}", c.name(), show(part(i))))
        .collect();
    format!("{n} · {}: {}", show(total), parts.join(" · "))
}

/// A request's row, and its `entered` and `changed` lines.
fn row(n: usize, p: &RequestPoint, style: &Style) -> String {
    let u = &p.usage;
    let cost = if p.priced { usd(p.usd) } else { format!("{} tok", tokens(p.total(false))) };
    let why = why(p, true);
    let why = if p.miss.as_ref().is_some_and(|m| !m.cause.expected()) { style.paint("31", &why) } else { why };
    let mut out = format!(
        "{n:>6}  {:<14}{:>8}{:>8}{:>10}{:>8}{:>8}{:>8}  {why}",
        local_time(p.ts_ms, "%m-%d %H:%M:%S"),
        tokens(u.cache_read as f64),
        tokens(u.cache_write() as f64),
        tokens(u.uncached as f64),
        tokens(u.output as f64),
        tokens(p.prompt_tokens as f64),
        cost
    );
    out = out.trim_end().to_string();
    out.push('\n');
    if let Some(e) = p.entered.as_ref().filter(|e| !e.is_empty()) {
        let _ = writeln!(out, "        {}  {}", style.dim("entered"), e.line(ENTERED_SHOWN));
    }
    if !p.changes.is_empty() {
        let _ = writeln!(out, "        {}  {}", style.dim("changed"), p.changes.iter().map(Change::text).collect::<Vec<_>>().join(" · "));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokprof_core::analysis::cache::Cause;
    use tokprof_core::model::{BlockKind, Usage};
    use tokprof_core::profile::Values;
    use tokprof_core::timeline::{EnteredGroup, EnteredList, PointMiss};

    fn point(ts_ms: i64) -> RequestPoint {
        RequestPoint {
            ts_ms,
            prompt_tokens: 0,
            usd: 0.0,
            model: "claude-opus-5-5".into(),
            effort: None,
            advisor: false,
            usage: Usage::default(),
            parts: Default::default(),
            priced: true,
            first: false,
            prev_prompt: None,
            gap_ms: None,
            ttl: None,
            miss: None,
            entered: Some(EnteredList::default()),
            changes: Vec::new(),
            failed: 0,
            streak: false,
            commits: 0,
        }
    }

    /// A local time `YYYY-MM-DD HH:MM:SS` in ms.
    fn local_ts(t: &str) -> i64 {
        let (d, hms) = t.split_once(' ').unwrap();
        let secs: Vec<i64> = hms.split(':').map(|x| x.parse().unwrap()).collect();
        crate::filters::parse_date(d).unwrap() + 1000 * (secs[0] * 3600 + secs[1] * 60 + secs[2])
    }

    fn v(tokens: f64, usd: f64) -> Values {
        Values { tokens, usd, avoidable: 0.0 }
    }

    fn group(tokens: f64, kind: BlockKind, tool: Option<&str>, target: Option<&str>) -> EnteredGroup {
        EnteredGroup { kind, tool: tool.map(str::to_string), target: target.map(str::to_string), tokens, blocks: 1 }
    }

    /// Request 57: an idle miss that re-wrote 490.3k.
    fn missed() -> RequestPoint {
        let ts = local_ts("2026-11-03 14:00:12");
        let mut p = point(ts);
        p.usage = Usage { cache_read: 12_300, write_long: 490_300, uncached: 3, output: 1_200, ..Usage::default() };
        p.prompt_tokens = p.usage.prompt();
        p.parts = [v(12_300.0, 0.01), v(0.0, 0.0), v(490_303.0, 3.90), v(0.0, 0.0), v(1_200.0, 0.04)];
        p.usd = 3.95;
        p.prev_prompt = Some(500_000);
        p.miss = Some(PointMiss { cause: Cause::IdleTtl, detail: "1.0h idle, 1h TTL".into(), rewritten: 490_303, premium_usd: Some(3.82) });
        p
    }

    #[test]
    fn a_missed_request_reads_its_cause_and_avoidable_dollars() {
        let r = row(57, &missed(), &Style::with_color(false));
        assert_eq!(r, "    57  11-03 14:00:12   12.3k  490.3k         3    1.2k  502.6k   $3.95  missed: idle > TTL: 1.0h idle, 1h TTL · $3.82 avoidable\n");
    }

    #[test]
    fn entered_and_changed_lines_follow_a_row() {
        let mut p = point(local_ts("2026-11-03 14:10:00"));
        p.entered = Some(EnteredList {
            groups: vec![
                group(6_100.0, BlockKind::ToolResult, Some("Bash"), Some("cargo test")),
                group(400.0, BlockKind::Text, None, None),
                group(200.0, BlockKind::ToolUse, Some("Bash"), Some("cargo test")),
                group(200.0, BlockKind::Reminder, None, None),
                group(100.0, BlockKind::Prompt, None, None),
            ],
            more_groups: 0,
            more_tokens: 0.0,
        });
        p.changes = vec![Change::Effort("high".into(), "max".into()), Change::Tools { added: vec!["A".into(), "B".into()], removed: Vec::new() }];
        let r = row(58, &p, &Style::with_color(false));
        let lines: Vec<&str> = r.lines().skip(1).collect();
        assert_eq!(
            lines,
            [
                "        entered  6.1k tool results Bash cargo test · 400 assistant text · 200 tool calls Bash cargo test · 2 more 300",
                "        changed  effort high → max · tools +2 −0"
            ]
        );
    }

    #[test]
    fn the_head_totals_the_parts_and_counts_unknown_prompts() {
        let mut x = crate::render_xray::tests::xray(Vec::new());
        let mut old = point(1);
        old.entered = None;
        old.first = true;
        x.points = vec![old, missed()];
        x.points[0].parts = [v(0.0, 0.0), v(1_000.0, 0.05), v(0.0, 0.0), v(0.0, 0.0), v(100.0, 0.0)];
        let t = text(&x, &Style::with_color(false));
        let lines: Vec<&str> = t.lines().collect();
        assert_eq!(lines[2], "requests  2 requests · $4.00: cache read $0.01 · cache write $0.05 · rewrite after miss $3.90 · output $0.04");
        assert_eq!(lines[3], "          the log does not show what 1 prompt held");
        assert!(lines[6].ends_with("first request"), "{}", lines[6]);
    }

    #[test]
    fn a_session_without_requests_prints_the_head() {
        let x = crate::render_xray::tests::xray(Vec::new());
        assert!(text(&x, &Style::with_color(false)).ends_with("requests  0 requests\n"));
    }
}
