//! The text of `tokprof sessions`, and the next-request cell it shares with the browser.

use std::fmt::Write as _;

use tokprof_core::fmt::{Style, count, local_time, usd};
use tokprof_core::xray::{Glance, SessionRow};

/// `tokprof sessions`: the rows, with their glances when costs were asked for.
pub fn sessions(rows: &[SessionRow], glances: &[Option<Glance>], cost: bool, style: &Style, now: i64) -> String {
    let mut out = String::new();
    let (cost_head, next_head) = if cost { ("cost", "next request") } else { ("", "") };
    let _ = writeln!(out, "{}", style.dim(&format!("{:<16}  {:<6}  {:<18}  {:>8}  {:>9}  {:<12}  project", "active", "agent", "session", "size", cost_head, next_head)));
    for (r, g) in rows.iter().zip(glances) {
        let g = *g;
        let extra = if r.subagents.is_empty() { String::new() } else { style.dim(&format!("  +{}", count(r.subagents.len() as u64, "subagent", "subagents"))) };
        let next = g.as_ref().and_then(|g| next_cell(g, now)).map_or_else(
            || " ".repeat(12),
            |(cell, cold)| style.paint(if cold { "31" } else { "32" }, &format!("{cell:<12}")),
        );
        let _ = writeln!(
            out,
            "{}  {:<6}  {:<18}  {:>8}  {:>9}  {next}  {}{extra}",
            local_time(r.active_ms, "%Y-%m-%d %H:%M"),
            r.file.agent.name(),
            r.id,
            format!("{:.1} MB", r.bytes as f64 / 1e6),
            g.map_or(String::new(), |g| usd(g.cost)),
            r.project
        );
    }
    out
}

/// The next request's cost as `$3.17 14%`: its share of what the session cost so far.
pub fn next_cell(g: &Glance, now: i64) -> Option<(String, bool)> {
    let (usd_next, cold) = g.next(now)?;
    let share = if g.cost > 0.0 { format!(" {:>3.0}%", (100.0 * usd_next / g.cost).min(999.0)) } else { "     ".to_string() };
    Some((format!("{}{share}", usd(usd_next)), cold))
}
