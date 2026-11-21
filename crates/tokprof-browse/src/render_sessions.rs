//! The text of `tokprof sessions`, and the next-request cell it shares with the browser.

use std::fmt::Write as _;

use tokprof_core::fmt::{Style, count, local_time, usd};
use tokprof_core::xray::{Glance, SessionRow};

/// `tokprof sessions`: the rows, with their glances when costs were asked for.
pub fn sessions(rows: &[SessionRow], glances: &[Option<Glance>], cost: bool, style: &Style, now: i64) -> String {
    let mut out = String::new();
    let (cost_head, next_head) = if cost { ("cost", "next request") } else { ("", "") };
    let _ = writeln!(
        out,
        "{}",
        style.dim(&format!("{:<16}  {:<6}  {:<18}  {:>8}  {:>9}  {:<12}  project", "active", "agent", "session", "size", cost_head, next_head))
    );
    let mut own = false;
    for (r, g) in rows.iter().zip(glances) {
        let extra = if r.subagents.is_empty() { String::new() } else { style.dim(&format!("  +{}", count(r.subagents.len() as u64, "subagent", "subagents"))) };
        let next = g
            .as_ref()
            .filter(|g| !g.without_usage)
            .and_then(|g| next_cell(g, now))
            .map_or_else(|| " ".repeat(12), |(cell, cold)| style.paint(if cold { "31" } else { "32" }, &format!("{cell:<12}")));
        let _ = writeln!(
            out,
            "{}  {:<6}  {:<18}  {:>8}  {:>9}  {next}  {}{extra}",
            local_time(r.active_ms, "%Y-%m-%d %H:%M"),
            r.file.agent.name(),
            r.id,
            format!("{:.1} MB", r.bytes as f64 / 1e6),
            g.as_ref().map_or(String::new(), |g| cost_cell(g, &mut own)),
            r.project
        );
    }
    if own {
        let _ = writeln!(out, "{}", style.dim("* the agent's own cost; its log gives no usage"));
    }
    out
}

/// The `cost` cell: tokprof's cost, or for a session whose log gives no usage its own cost with
/// `*`, or `–` without one; `own` is set when a cell has `*`.
fn cost_cell(g: &Glance, own: &mut bool) -> String {
    if !g.without_usage {
        return usd(g.cost);
    }
    match g.own_usd {
        Some(u) => {
            *own = true;
            format!("{}*", usd(u))
        }
        None => "–".to_string(),
    }
}

/// The next request's cost as `$3.17 14%`: its share of what the session cost so far.
pub fn next_cell(g: &Glance, now: i64) -> Option<(String, bool)> {
    let (usd_next, cold) = g.next(now)?;
    let share = if g.cost > 0.0 { format!(" {:>3.0}%", (100.0 * usd_next / g.cost).min(999.0)) } else { "     ".to_string() };
    Some((format!("{}{share}", usd(usd_next)), cold))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_session_without_usage_shows_its_own_cost() {
        let mut own = false;
        assert_eq!(cost_cell(&Glance { cost: 2.0, ..Glance::default() }, &mut own), "$2.00");
        assert!(!own);
        assert_eq!(cost_cell(&Glance { without_usage: true, own_usd: None, ..Glance::default() }, &mut own), "–");
        assert!(!own, "no cost, no mark");
        assert_eq!(cost_cell(&Glance { without_usage: true, own_usd: Some(1.5), ..Glance::default() }, &mut own), "$1.50*");
        assert!(own);
    }
}
