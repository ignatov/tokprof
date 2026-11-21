//! The rows of the tool table and the failures line, which `tokprof tools` and the X-ray share.

use std::fmt::Write as _;

use tokprof_core::fmt::{self, clip_end, table, tokens, usd};
use tokprof_core::profile::Values;
use tokprof_core::tools::{GroupFilter, RowOrder, ToolTable, hint};

/// The line on the calls that did not work: how many failed, with the spend of the requests after
/// them until the tool worked again, how many the user refused and how many rules denied. `None`
/// when every call worked.
pub fn failures_line(t: &ToolTable) -> Option<String> {
    let priced = t.spend.usd > 0.0;
    let mut parts = Vec::new();
    if t.failed > 0 && !metered(t) {
        // No request with usage: there is no spend after the failures to tell.
        parts.push(format!("failed {}", count_int(t.failed, "call", "calls")));
    } else if t.failed > 0 {
        let after = if priced { usd(t.after.usd) } else { format!("{} tok", tokens(t.after.tokens)) };
        parts.push(format!("failed {}, after them {after} until the tool worked again", count_int(t.failed, "call", "calls")));
    }
    if t.refused > 0 {
        parts.push(format!("you refused {}", fmt::int(t.refused)));
    }
    if t.denied > 0 {
        parts.push(format!("rules denied {}", fmt::int(t.denied)));
    }
    (!parts.is_empty()).then(|| parts.join(" · "))
}

/// Whether some spend of the table comes from requests with usage (see tool-breakdown, Spend of
/// calls without usage).
fn metered(t: &ToolTable) -> bool {
    t.spend.tokens > 0.0 || t.rows.values().any(|r| r.calls > 0 && !r.no_usage())
}

/// The `time` cell: the sum of the counted calls' durations, or `–` when no call counts for
/// times (see tool-breakdown, Time per row).
pub fn total_time(durations: &[u32], time_ms: u64) -> String {
    if durations.is_empty() { "–".to_string() } else { fmt::call_time(time_ms as f64) }
}

/// The `med time` cell.
pub fn median_time(median_ms: Option<f64>) -> String {
    median_ms.map_or("–".to_string(), fmt::call_time)
}

/// `1 call`, `1,021 calls`.
fn count_int(n: u64, one: &str, many: &str) -> String {
    format!("{} {}", fmt::int(n), if n == 1 { one } else { many })
}

/// The tool table as cells: the header and a row per tool, each with its hint (the header's is
/// `hint`), how many rows did not fit, and the spend line under the table.
pub struct ToolCells {
    pub rows: Vec<Vec<String>>,
    pub hints: Vec<String>,
    pub more: usize,
    pub spend: String,
}

/// The tool table's rows, `<n> more` and the total line; with `failures`, the `failed` and
/// `after` columns too.
pub fn tool_rows(t: &ToolTable, order: RowOrder, limit: usize, failures: bool) -> String {
    let cells = tool_cells(t, order, limit, failures);
    // The hint is text, so it goes after the aligned columns, which the empty failures cells of a
    // row would otherwise cut short.
    let mut out = String::new();
    let lines = table(&cells.rows, 1);
    let width = lines.lines().map(|l| l.chars().count()).max().unwrap_or(0);
    for (line, h) in lines.lines().zip(&cells.hints) {
        let pad = width - line.chars().count();
        let _ = writeln!(out, "{}", format!("{line}{}  {h}", " ".repeat(pad)).trim_end());
    }
    if cells.more > 0 {
        let _ = writeln!(out, "{} more", cells.more);
    }
    let _ = writeln!(out, "{}", cells.spend);
    out
}

/// The cells of the tool table, as `tool_rows` lays them out.
pub fn tool_cells(t: &ToolTable, order: RowOrder, limit: usize, failures: bool) -> ToolCells {
    tool_cells_of(t, order, limit, failures, None)
}

/// The cells of the tool table with only the rows of the group `group` keeps, when it is given:
/// `more` and the spend line count those rows only.
pub fn tool_cells_of(t: &ToolTable, order: RowOrder, limit: usize, failures: bool, group: Option<&GroupFilter>) -> ToolCells {
    let priced = t.spend.usd > 0.0;
    let amount = |v: &Values| if priced { usd(v.usd) } else { format!("{} tok", tokens(v.tokens)) };
    let size = |v: Option<f64>| v.map_or("–".to_string(), tokens);
    let mut rows = t.sorted_by(order);
    if let Some(g) = group {
        rows.retain(|(_, r)| g.keeps(&r.group()));
    }
    let mut header = vec!["tool", "calls", "time", "med time", "avg result", "avg input", "rent", "trips", "single"];
    if failures {
        header.extend(["failed", "after"]);
    }
    let mut table_rows = vec![header.into_iter().map(str::to_string).collect::<Vec<_>>()];
    let mut hints = vec!["hint".to_string()];
    for (key, r) in rows.iter().take(limit) {
        let name = match &key.1 {
            Some(head) => format!("{} {head}", key.0),
            None => key.0.clone(),
        };
        let mut row = vec![
            clip_end(&name, 32),
            r.calls.to_string(),
            total_time(&r.durations, r.time_ms),
            median_time(r.median_ms()),
            size(r.avg_result()),
            size(r.avg_input()),
            if r.no_usage() { "–".to_string() } else { amount(&r.rent) },
            if r.no_usage() { "–".to_string() } else { amount(&r.trips) },
            format!("{:.0}%", 100.0 * r.single_share()),
        ];
        if failures {
            // A row whose calls all worked leaves both cells empty.
            row.push(if r.failed > 0 { r.failed.to_string() } else { String::new() });
            row.push(match (r.failed > 0, r.no_usage()) {
                (false, _) => String::new(),
                (true, false) => amount(&r.after),
                (true, true) => "–".to_string(),
            });
        }
        table_rows.push(row);
        hints.push(hint(key, r).unwrap_or("").to_string());
    }
    let mut rent = Values::default();
    let mut trips = Values::default();
    for (_, r) in &rows {
        rent.add(r.rent);
        trips.add(r.trips);
    }
    let spend = if metered(t) {
        format!(
            "spend {} · rent of these tools {} · trips of these tools {} (two views of the same money; do not add them)",
            amount(&t.spend),
            amount(&rent),
            amount(&trips)
        )
    } else {
        "spend – · the logs give no usage, so these calls have no rent or trips".to_string()
    };
    ToolCells { rows: table_rows, hints, more: rows.len().saturating_sub(limit), spend }
}
