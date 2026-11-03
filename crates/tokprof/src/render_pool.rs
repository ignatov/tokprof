//! The report of `tokprof pool` over share files, and of `tokprof share` over the one file it
//! wrote: the header, where the money went, what could have been saved, the habits that cost the
//! most, tools and commands, and the agent versions.

use std::collections::BTreeMap;
use std::fmt::Write;

use tokprof_core::fmt::{table, usd};
use tokprof_core::pool::{Pool, Row, Table};

fn pct(share: f64) -> String {
    format!("{:.1}%", 100.0 * share)
}

/// `in <k> of <n> files` when some files lack a part.
fn coverage(with_part: usize, files: usize) -> String {
    if with_part < files { format!(", in {with_part} of {files} files") } else { String::new() }
}

/// The days the files cover: the first first day and the last last day.
fn window(p: &Pool) -> Option<String> {
    let first = p.files.iter().filter_map(|f| f.first_day.as_deref()).min()?;
    let last = p.files.iter().filter_map(|f| f.last_day.as_deref()).max()?;
    Some(format!("{first} – {last}"))
}

fn header(p: &Pool) -> String {
    let mut out = String::new();
    let one = p.files.len() == 1;
    let mut labels: Vec<&str> = Vec::new();
    for f in &p.files {
        let label = f.survey.as_deref().unwrap_or("none");
        if !labels.contains(&label) {
            labels.push(label);
        }
    }
    let survey = if labels == ["none"] { String::new() } else { format!(" · survey {}", labels.join(", ")) };
    let days = window(p).map(|w| format!(" · {w}")).unwrap_or_default();
    if one {
        let _ = writeln!(out, "share   {}{survey}", window(p).unwrap_or_default());
        let _ = writeln!(out, "spend   {}", usd(p.spend_usd));
    } else {
        let _ = writeln!(out, "pool    {} files{days}{survey}", p.files.len());
        let mut agents: BTreeMap<&str, usize> = BTreeMap::new();
        for f in &p.files {
            for a in &f.agents {
                *agents.entry(a.as_str()).or_default() += 1;
            }
        }
        let agents: Vec<String> = agents.iter().map(|(a, n)| format!("{a} {n}")).collect();
        let _ = writeln!(out, "agents  {}", agents.join(" · "));
        let _ = writeln!(
            out,
            "spend   {} in all · per file: median {}, quarters {} and {}",
            usd(p.spend_usd),
            usd(p.median_usd),
            usd(p.quarter_1_usd),
            usd(p.quarter_3_usd)
        );
    }
    out
}

/// The file columns of `--by-file`: each file's share, or `–` for a file without the part.
fn file_cells(r: &Row, by_file: bool) -> Vec<String> {
    if !by_file {
        return Vec::new();
    }
    r.by_file.iter().map(|s| s.map_or("–".to_string(), pct)).collect()
}

/// The name cell of a row: indented under the row before it when it is a name of a kind.
fn name_cell(r: &Row) -> String {
    if r.under { format!("  {}", r.name) } else { r.name.clone() }
}

/// A table whose rows have a cost and a share, with the median file and the files when there
/// are several, and the per-file columns of `--by-file`.
fn money_table(p: &Pool, t: &Table, first: &[&str], money: &str, files_col: &str, notable: bool, by_file: bool) -> String {
    let one = p.files.len() == 1;
    let mut head: Vec<String> = first.iter().map(|s| s.to_string()).collect();
    head.push(money.to_string());
    head.push("share".into());
    if !one {
        head.push("median file".into());
        head.push(files_col.into());
    }
    if by_file {
        head.extend(p.files.iter().map(|f| f.name.clone()));
    }
    let mut rows = vec![head];
    for r in &t.rows {
        let mut cells = Vec::new();
        if first.len() == 2 {
            cells.push(r.agent.clone().unwrap_or_default());
        }
        let mut name = name_cell(r);
        // A saving or habit comes from a part of its own, which some files can lack.
        if notable && r.files_with_part < p.files.len() {
            name += &format!(" (in {} of {} files)", r.files_with_part, p.files.len());
        }
        cells.push(name);
        cells.push(usd(r.usd));
        cells.push(pct(r.share));
        if !one {
            cells.push(pct(r.median_file));
            cells.push(if notable { r.notable_files } else { r.files }.to_string());
        }
        cells.extend(file_cells(r, by_file));
        rows.push(cells);
    }
    if let Some((n, sum)) = t.more {
        let mut cells = vec![String::new(); first.len() - 1];
        cells.push(format!("{n} more"));
        cells.push(usd(sum));
        rows.push(cells);
    }
    table(&rows, first.len())
}

fn tools_table(p: &Pool, t: &Table, by_file: bool) -> String {
    let one = p.files.len() == 1;
    let mut head: Vec<String> = ["tool", "calls", "rent", "trips"].map(str::to_string).to_vec();
    if !one {
        head.push("files".into());
    }
    if by_file {
        head.extend(p.files.iter().map(|f| f.name.clone()));
    }
    let mut rows = vec![head];
    for r in &t.rows {
        let mut cells = vec![r.name.clone(), r.calls.to_string(), usd(r.usd), usd(r.trips_usd)];
        if !one {
            cells.push(r.files.to_string());
        }
        cells.extend(file_cells(r, by_file));
        rows.push(cells);
    }
    if let Some((n, sum)) = t.more {
        rows.push(vec![format!("{n} more"), String::new(), usd(sum)]);
    }
    table(&rows, 1)
}

fn versions_table(p: &Pool, t: &Table) -> String {
    let one = p.files.len() == 1;
    let mut head: Vec<String> = ["agent", "version", "requests"].map(str::to_string).to_vec();
    if !one {
        head.push("files".into());
    }
    let mut rows = vec![head];
    for r in &t.rows {
        let mut cells = vec![r.agent.clone().unwrap_or_default(), r.name.clone(), r.requests.to_string()];
        if !one && !r.name.ends_with(" more") {
            cells.push(r.files.to_string());
        }
        rows.push(cells);
    }
    table(&rows, 2)
}

/// The tables of the section "Where the money went": key, column titles.
const SPEND_TABLES: [(&str, &[&str]); 8] = [
    ("model", &["agent", "model"]),
    ("component", &["cost component"]),
    ("content", &["content of the context"]),
    ("origin", &["origin"]),
    ("activity", &["what the agent was doing"]),
    ("prompt_size", &["prompt size"]),
    ("session_span", &["session ran for"]),
    ("client", &["agent", "client"]),
];

/// The report over the pool. With one file, the columns about files are left out.
pub fn text(p: &Pool, by_file: bool) -> String {
    let mut out = header(p);
    let n = p.files.len();
    let one = n == 1;
    let present = |t: &&Table| t.files_with_part > 0 && !t.rows.is_empty();

    let _ = write!(out, "\nWhere the money went{}\n", if one { "" } else { ": pooled, and the median file's share of its own spend" });
    for (key, cols) in SPEND_TABLES {
        let Some(t) = p.table(key).filter(present) else { continue };
        let _ = write!(out, "\n{}", coverage_line(t, n));
        out.push_str(&money_table(p, t, cols, "cost", "files", false, by_file));
    }

    if let Some(t) = p.table("saved").filter(present) {
        let _ = write!(out, "\nWhat could have been saved{}\n\n", if one { "" } else { " (files 5%+: the files where it is 5% of their spend or more)" });
        out.push_str(&money_table(p, t, &["item"], "avoidable", "files 5%+", true, by_file));
    }
    if let Some(t) = p.table("habits").filter(present) {
        out.push_str("\nHabits that cost the most: where to look, not what could be saved; the rows overlap\n\n");
        out.push_str(&money_table(p, t, &["habit"], "cost", "files 5%+", true, by_file));
    }
    if let Some(t) = p.table("tools").filter(present) {
        let _ = write!(out, "\nTools and commands, by rent{}\n\n", coverage(t.files_with_part, n));
        out.push_str(&tools_table(p, t, by_file));
    }
    if let Some(t) = p.table("versions").filter(present) {
        let _ = write!(out, "\nVersions{}\n\n", coverage(t.files_with_part, n));
        out.push_str(&versions_table(p, t));
    }
    out
}

/// A line before a table only when some files lack its part.
fn coverage_line(t: &Table, files: usize) -> String {
    if t.files_with_part < files { format!("in {} of {} files\n", t.files_with_part, files) } else { String::new() }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};
    use tokprof_core::pool;

    fn file(spend: f64, survey: Option<&str>) -> Value {
        let mut f = json!({
            "format": 2, "tokprof": "0.2.0", "platform": "macos-aarch64", "window": {"first": "2026-10-04", "last": "2026-11-03"},
            "agents": {"claude": {
                "spend": {"claude-opus-5-5": {"requests": 10, "priced": true, "components": {"cache read": {"usd": spend, "tokens": 1}}, "total": {"usd": spend, "tokens": 2}}},
                "content": {"tool results": {"usd": spend * 0.4, "tokens": 1}, "thinking": {"usd": spend * 0.2, "tokens": 1}},
                "origins": {"agent": {"requests": 1, "usd": spend * 0.3, "tokens": 1, "names": {"general-purpose": {"requests": 1, "usd": spend * 0.25, "tokens": 1}}}},
                "misses": {"idle > TTL": {"misses": 2, "tokens": 1, "avoidable_usd": spend * 0.07}},
                "setup": {"skills": {"usd": spend * 0.1, "tokens": 1, "subagents_usd": 0.0}},
                "tools": {"Read": {"calls": 3, "rent": {"usd": 4.0, "tokens": 1}, "trips": {"usd": 2.0, "tokens": 1}}},
                "versions": {"2.1.316": {"requests": 10, "usd": spend}},
            }}
        });
        if let Some(s) = survey {
            f["survey"] = json!(s);
        }
        f
    }

    fn pool_of(files: Vec<Value>) -> Pool {
        let named: Vec<(String, Value)> = files.into_iter().enumerate().map(|(i, f)| (format!("f{i}"), f)).collect();
        pool::build(&named, 1)
    }

    #[test]
    fn five_files_start_with_the_pool_lines() {
        let p = pool_of([2_140.0, 3_900.0, 6_310.0, 9_870.0, 19_000.0].map(|s| file(s, Some("team-2026-11"))).to_vec());
        let text = text(&p, false);
        let lines: Vec<&str> = text.lines().take(3).collect();
        assert_eq!(lines, [
            "pool    5 files · 2026-10-04 – 2026-11-03 · survey team-2026-11",
            "agents  claude 5",
            "spend   $41,220 in all · per file: median $6,310, quarters $3,900 and $9,870",
        ]);
        assert!(text.contains("median file") && text.contains("files 5%+"), "{text}");
        assert!(text.contains("  agent:general-purpose"), "a name sits under its kind: {text}");
    }

    #[test]
    fn files_of_two_surveys_list_their_labels() {
        let p = pool_of(vec![file(10.0, Some("team-2026-11")), file(10.0, Some("all-2026-12")), file(10.0, None)]);
        assert!(text(&p, false).lines().next().unwrap().ends_with("survey team-2026-11, all-2026-12, none"));
    }

    #[test]
    fn the_report_of_one_file_has_no_columns_about_files() {
        let p = pool_of(vec![file(100.0, Some("team-2026-11"))]);
        let text = text(&p, false);
        assert!(text.starts_with("share   2026-10-04 – 2026-11-03 · survey team-2026-11\nspend   $100\n"), "{text}");
        assert!(!text.contains("median file") && !text.contains("files 5%+") && !text.contains("agents  "), "{text}");
        assert!(text.contains("What could have been saved") && text.contains("Habits that cost the most") && text.contains("Tools and commands"), "{text}");
    }

    #[test]
    fn by_file_adds_a_column_per_file() {
        let p = pool_of(vec![file(100.0, None), file(200.0, None), file(300.0, None)]);
        let text = text(&p, true);
        let head = text.lines().find(|l| l.starts_with("agent   model") || l.starts_with("agent  model")).unwrap_or_else(|| panic!("{text}"));
        assert!(head.trim_end().ends_with("f0     f1     f2") || head.split_whitespace().rev().take(3).collect::<Vec<_>>() == ["f2", "f1", "f0"], "{head}");
    }
}
