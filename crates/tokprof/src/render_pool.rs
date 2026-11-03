//! The report of `tokprof pool` over share files, and of `tokprof share` over the one file it
//! wrote: the header, where the money went, what could have been saved, the habits that cost the
//! most, tools and commands, and the agent versions.

use std::collections::BTreeMap;
use std::fmt::Write;

use tokprof_core::fmt::{int, table, usd};
use tokprof_core::pool::{Pool, RateRow, Row, Table, Unit};
use tokprof_core::rhythm::duration;

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
        if let Some(a) = &p.activity {
            let _ = writeln!(
                out,
                "activity  {} active days · {} active hours · busy {} · {} per active hour",
                a.active_days,
                a.active_hours,
                busy_hours(a.busy_ms),
                usd(a.usd_per_hour)
            );
        }
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
        if let Some(a) = &p.activity {
            let _ = writeln!(
                out,
                "activity  {} active hours · {} per active hour · per file: median {} active days, {} active hours, {} per active hour{}",
                a.active_hours,
                usd(a.usd_per_hour),
                a.median_days,
                a.median_hours,
                usd(a.median_usd_per_hour),
                if a.files < p.files.len() { format!(" (in {} of {} files)", a.files, p.files.len()) } else { String::new() }
            );
        }
    }
    out
}

/// Busy hours as the activity line gives them: whole hours from 10 on.
fn busy_hours(ms: f64) -> String {
    let h = ms / 3_600_000.0;
    if h >= 10.0 { format!("{h:.0}h") } else { format!("{h:.1}h") }
}

/// A value of a rate table, in its unit.
fn value_cell(r: &RateRow) -> String {
    let v = match r.unit {
        Unit::Count => int(r.value.round() as u64),
        Unit::Usd => usd(r.value),
        Unit::Ms => span(r.value),
    };
    match r.of {
        Some(of) if of > 0.0 => format!("{v} ({:.1}%)", 100.0 * r.value / of),
        _ => v,
    }
}

/// A rate in its row's unit: a count with two decimals, dollars, or a time.
fn rate_cell(r: &RateRow, rate: Option<f64>) -> String {
    match (rate, r.unit) {
        (None, _) => "–".into(),
        (Some(x), Unit::Count) => format!("{x:.2}"),
        (Some(x), Unit::Usd) => usd(x),
        (Some(x), Unit::Ms) => span(x),
    }
}

/// A time: hours from one hour on, as the busy table gives them.
fn span(ms: f64) -> String {
    if ms >= 3_600_000.0 { format!("{:.1}h", ms / 3_600_000.0) } else { duration(ms.round() as i64) }
}

/// A table of rates: the row name, the pooled value, its rate and, with several files, the median
/// file's rate and the files with the row; with `--by-file`, each file's rate.
fn rate_table(p: &Pool, rows: &[RateRow], name: &str, rate: &str, by_file: bool) -> String {
    let one = p.files.len() == 1;
    let mut head = vec![name.to_string(), if one { "value".into() } else { "pooled".into() }, rate.to_string()];
    if !one {
        head.push("median file".into());
        head.push("files".into());
    }
    if by_file {
        head.extend(p.files.iter().map(|f| f.name.clone()));
    }
    let mut out = vec![head];
    for r in rows {
        let mut name = r.name.clone();
        if !one && r.files_with_part < p.files.len() {
            name += &format!(" (in {} of {} files)", r.files_with_part, p.files.len());
        }
        let mut cells = vec![name, value_cell(r), rate_cell(r, r.rate)];
        if !one {
            cells.push(rate_cell(r, r.median_rate));
            cells.push(r.files.to_string());
        }
        if by_file {
            cells.extend(r.by_file.iter().map(|x| x.map_or("–".to_string(), |x| rate_cell(r, Some(x)))));
        }
        out.push(cells);
    }
    table(&out, 1)
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
    let per_hour = t.rows.iter().any(|r| r.per_hour.is_some());
    let mut head: Vec<String> = first.iter().map(|s| s.to_string()).collect();
    head.push(money.to_string());
    head.push("share".into());
    if per_hour {
        head.push("per active hour".into());
    }
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
        if per_hour {
            cells.push(r.per_hour.map_or("–".to_string(), usd));
        }
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
    let mut head: Vec<String> = ["tool", "calls", "failed", "rent", "trips"].map(str::to_string).to_vec();
    if !one {
        head.push("files".into());
    }
    if by_file {
        head.extend(p.files.iter().map(|f| f.name.clone()));
    }
    let mut rows = vec![head];
    for r in &t.rows {
        let mut cells = vec![r.name.clone(), r.calls.to_string(), r.failed.to_string(), usd(r.usd), usd(r.trips_usd)];
        if !one {
            cells.push(r.files.to_string());
        }
        cells.extend(file_cells(r, by_file));
        rows.push(cells);
    }
    if let Some((n, sum)) = t.more {
        rows.push(vec![format!("{n} more"), String::new(), String::new(), usd(sum)]);
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
        if let (true, Some(c)) = (key == "content", &p.coverage)
            && c.share < 0.9995
        {
            let below = if one { String::new() } else { format!(", in {} files below 90%", c.files_below_90) };
            let _ = writeln!(out, "prompts the logs hold: {:.1}% of the spend; the rest is unknown{below}", 100.0 * c.share);
        }
    }

    if let Some(t) = p.table("saved").filter(present) {
        let _ = write!(out, "\nWhat could have been saved{}\n\n", if one { "" } else { " (files 5%+: the files where it is 5% of their spend or more)" });
        out.push_str(&money_table(p, t, &["item"], "avoidable", "files 5%+", true, by_file));
        if !p.cold_by_client.is_empty() {
            out.push_str("\nCold returns by client\n\n");
            let mut rows =
                vec![["agent", "client", "active hours", "cold returns", "per active hour", "avoidable per active hour"].map(str::to_string).to_vec()];
            for r in &p.cold_by_client {
                rows.push(vec![
                    r.agent.clone().unwrap_or_default(),
                    r.name.clone(),
                    int(r.hours.unwrap_or(0.0).round() as u64),
                    int(r.value.round() as u64),
                    format!("{:.2}", r.rate.unwrap_or(0.0)),
                    usd(r.second_rate.unwrap_or(0.0)),
                ]);
            }
            out.push_str(&table(&rows, 2));
        }
    }
    if !p.problems.is_empty() {
        out.push_str("\nProblems: failed calls, retries and slow requests\n\n");
        out.push_str(&rate_table(p, &p.problems, "measure", "per active hour", by_file));
    }
    if let Some(t) = p.table("habits").filter(present) {
        out.push_str("\nHabits that cost the most: where to look, not what could be saved; the rows overlap\n\n");
        out.push_str(&money_table(p, t, &["habit"], "cost", "files 5%+", true, by_file));
    }
    if !p.time.is_empty() {
        let _ = write!(out, "\nWhere the agent's time went: what it waited on while it was busy{}\n\n", coverage(p.time_files, n));
        let mut head = ["kind", "time", "share"].map(str::to_string).to_vec();
        if !one {
            head.extend(["median file", "files"].map(str::to_string));
        }
        head.push("after turns".into());
        if by_file {
            head.extend(p.files.iter().map(|f| f.name.clone()));
        }
        let mut rows = vec![head];
        for r in &p.time {
            let mut cells = vec![r.name.clone(), span(r.ms), pct(r.share)];
            if !one {
                cells.push(pct(r.median_file));
                cells.push(r.files.to_string());
            }
            cells.push(if r.after_turns_ms > 0.0 { span(r.after_turns_ms) } else { "–".into() });
            if by_file {
                cells.extend(r.by_file.iter().map(|x| x.map_or("–".to_string(), pct)));
            }
            rows.push(cells);
        }
        out.push_str(&table(&rows, 1));
    }
    if !p.working.is_empty() {
        out.push_str("\nWorking with the agent: hand-offs per 100 typed messages\n\n");
        out.push_str(&rate_table(p, &p.working, "hand-off", "per 100 typed messages", by_file));
    }
    if let Some(t) = p.table("tools").filter(present) {
        let _ = write!(out, "\nTools and commands, by rent{}\n\n", coverage(t.files_with_part, n));
        out.push_str(&tools_table(p, t, by_file));
    }
    if !p.effort_by_client.is_empty() {
        out.push_str("\nHighest effort by client: the share at max or xhigh\n\n");
        let mut head = ["agent", "client", "spend", "requests"].map(str::to_string).to_vec();
        if !one {
            head.extend(["median file", "files"].map(str::to_string));
        }
        if by_file {
            head.extend(p.files.iter().map(|f| f.name.clone()));
        }
        let mut rows = vec![head];
        for r in &p.effort_by_client {
            let mut cells = vec![r.agent.clone(), r.client.clone(), pct(r.spend_share), pct(r.request_share)];
            if !one {
                cells.push(pct(r.median_file));
                cells.push(r.files.to_string());
            }
            if by_file {
                cells.extend(r.by_file.iter().map(|x| x.map_or("–".to_string(), pct)));
            }
            rows.push(cells);
        }
        out.push_str(&table(&rows, 2));
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
        assert_eq!(
            lines,
            [
                "pool    5 files · 2026-10-04 – 2026-11-03 · survey team-2026-11",
                "agents  claude 5",
                "spend   $41,220 in all · per file: median $6,310, quarters $3,900 and $9,870",
            ]
        );
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

    /// A format-2 file with the parts of add-share-friction: `spend` dollars over `hours` active
    /// hours on `days` days, and the rest from `extra`.
    fn friction_file(spend: f64, hours: f64, days: f64, extra: Value) -> Value {
        let mut f = file(spend, None);
        f["activity"] = json!({"active_days": days, "active_hours": hours, "busy_ms": 365.0 * 3_600_000.0});
        let a = &mut f["agents"]["claude"];
        a["failures"] = json!({"failed": 0, "refused": 0, "denied": 0, "after": {"usd": 0.0, "tokens": 0}});
        a["time"] =
            json!({"kinds": {"model": {"ms": 3_600_000, "after_turns_ms": 0}}, "requests": 10, "slow": 0, "slow_main": 0, "longest_ms": 30_000, "retried": 0});
        a["handoffs"] = json!({"turn_ends": 0, "interrupts": 0, "typed_while_working": 0, "questions": 0, "answers": 0, "wakes": {"subagent": 0, "command": 0, "other": 0}});
        a["prompts"] = json!({"held": {"requests": 10, "usd": spend, "tokens": 1}, "all": {"requests": 10, "usd": spend, "tokens": 1}});
        a["pauses"] = json!({"typed_messages": 0});
        if let (Value::Object(agent), Value::Object(e)) = (a, extra) {
            for (k, v) in e {
                agent.insert(k, v);
            }
        }
        f
    }

    #[test]
    fn the_activity_line_of_one_file_and_of_a_pool() {
        let one = pool_of(vec![friction_file(11_200.0, 141.0, 18.0, json!({}))]);
        assert!(text(&one, false).contains("activity  18 active days · 141 active hours · busy 365h · $79.43 per active hour\n"), "{}", text(&one, false));
        let five = [(3_200.0, 40.0, 10.0), (1_836.0, 100.0, 14.0), (8_184.0, 120.0, 16.0), (10_000.0, 200.0, 20.0), (18_000.0, 240.0, 22.0)];
        let p = pool_of(five.iter().map(|(s, h, d)| friction_file(*s, *h, *d, json!({}))).collect());
        let line = text(&p, false).lines().find(|l| l.starts_with("activity")).unwrap().to_string();
        assert_eq!(line, "activity  700 active hours · $58.89 per active hour · per file: median 16 active days, 120 active hours, $68.20 per active hour");
    }

    #[test]
    fn savings_per_active_hour_and_cold_returns_by_client() {
        let misses = json!({
            "misses": {"idle > TTL": {"misses": 206, "tokens": 1, "avoidable_usd": 740.0}},
            "clients": {"sdk-ts": {"requests": 10, "usd": 1_000.0, "avoidable_usd": 740.0, "main_sessions": 9, "cold_returns": 206, "cache_writes": {"short": 0, "long": 0}, "active_hours": 312, "effort": {}}},
        });
        let p = pool_of(vec![friction_file(1_000.0, 312.0, 20.0, misses)]);
        let t = text(&p, false);
        let saved = t.lines().find(|l| l.starts_with("idle > TTL")).unwrap();
        assert!(saved.ends_with("$2.37"), "{saved}");
        let client = t.lines().find(|l| l.starts_with("claude  sdk-ts") && l.contains("206")).unwrap_or_else(|| panic!("{t}"));
        let cells: Vec<&str> = client.split_whitespace().collect();
        assert_eq!(&cells[2..], ["312", "206", "0.66", "$2.37"]);
    }

    #[test]
    fn problems_of_one_person() {
        let extra = json!({
            "failures": {"failed": 1_390, "refused": 55, "denied": 91, "after": {"usd": 226.0, "tokens": 1}},
            "time": {"kinds": {"API retries": {"ms": 3_600_000, "after_turns_ms": 0}}, "requests": 83_890, "slow": 3_001, "slow_main": 2_000, "longest_ms": 1_380_000, "retried": 92},
        });
        let t = text(&pool_of(vec![friction_file(11_200.0, 141.0, 18.0, extra)]), false);
        let row =
            |name: &str| t.lines().find(|l| l.starts_with(name)).unwrap_or_else(|| panic!("{name}: {t}")).split_whitespace().rev().take(2).collect::<Vec<_>>();
        assert_eq!(row("failed tool calls"), ["9.86", "1,390"]);
        assert_eq!(row("spend after failures"), ["$1.60", "$226"]);
        assert!(t.lines().any(|l| l.starts_with("requests of a minute or more") && l.contains("3,001 (3.6%)")), "{t}");
        assert!(t.lines().any(|l| l.starts_with("longest request") && l.contains("23m") && l.trim_end().ends_with('–')), "{t}");
    }

    #[test]
    fn the_agents_time_and_the_hand_offs() {
        let extra = json!({
            "time": {"kinds": {"subagents": {"ms": 67.5 * 3_600_000.0, "after_turns_ms": 54.0 * 3_600_000.0}, "model": {"ms": 297.5 * 3_600_000.0, "after_turns_ms": 0}}, "requests": 10, "slow": 0, "slow_main": 0, "longest_ms": 1_000, "retried": 0},
            "handoffs": {"turn_ends": 900, "interrupts": 41, "typed_while_working": 3, "questions": 5, "answers": 5, "wakes": {"subagent": 1, "command": 2, "other": 0}},
            "pauses": {"typed_messages": 1_030},
        });
        let t = text(&pool_of(vec![friction_file(1_000.0, 141.0, 18.0, extra)]), false);
        let sub: Vec<&str> = t.lines().find(|l| l.starts_with("subagents")).unwrap().split_whitespace().collect();
        assert_eq!(sub[1..], ["67.5h", "18.5%", "54.0h"]);
        let interrupts: Vec<&str> = t.lines().find(|l| l.starts_with("interrupts")).unwrap().split_whitespace().collect();
        assert_eq!(interrupts[1..], ["41", "3.98"]);
    }

    #[test]
    fn effort_by_client_coverage_and_failed_tools() {
        let extra = json!({
            "clients": {"sdk-ts": {"requests": 1_000, "usd": 850.0, "avoidable_usd": 0.0, "main_sessions": 1, "cold_returns": 0, "cache_writes": {"short": 0, "long": 0}, "active_hours": 10,
                "effort": {"max": {"requests": 900, "usd": 800.0}, "medium": {"requests": 100, "usd": 50.0}}}},
            "prompts": {"held": {"requests": 8, "usd": 882.0, "tokens": 1}, "all": {"requests": 10, "usd": 1_000.0, "tokens": 1}},
            "tools": {"Bash": {"calls": 300, "failed": 120, "rent": {"usd": 30.0, "tokens": 1}, "trips": {"usd": 3.0, "tokens": 1}, "programs": {"git": {"calls": 300, "failed": 120, "rent": {"usd": 30.0, "tokens": 1}, "trips": {"usd": 3.0, "tokens": 1}}}}},
        });
        let t = text(&pool_of(vec![friction_file(1_000.0, 10.0, 2.0, extra)]), false);
        let section = &t[t.find("Highest effort by client").unwrap()..];
        let effort: Vec<&str> = section.lines().find(|l| l.starts_with("claude  sdk-ts")).unwrap().split_whitespace().collect();
        assert_eq!(effort[2..], ["94.1%", "90.0%"]);
        assert!(t.contains("prompts the logs hold: 88.2% of the spend; the rest is unknown\n"), "{t}");
        let git: Vec<&str> = t.lines().find(|l| l.starts_with("Bash git")).unwrap().split_whitespace().collect();
        assert_eq!(git[2..4], ["300", "120"]);
    }

    #[test]
    fn the_sections_come_in_their_order_and_format_1_files_have_none_of_the_new_ones() {
        let t = text(
            &pool_of(vec![friction_file(
                1_000.0,
                10.0,
                2.0,
                json!({"handoffs": {"turn_ends": 3, "interrupts": 1, "typed_while_working": 0, "questions": 0, "answers": 0, "wakes": {"subagent": 0, "command": 0, "other": 0}}, "pauses": {"typed_messages": 4}}),
            )]),
            false,
        );
        let at = |title: &str| t.find(title).unwrap_or_else(|| panic!("{title}: {t}"));
        let order = [
            "Where the money went",
            "What could have been saved",
            "Problems",
            "Habits that cost the most",
            "Where the agent's time went",
            "Working with the agent",
            "Tools and commands",
            "Versions",
        ];
        let positions: Vec<usize> = order.iter().map(|o| at(o)).collect();
        assert!(positions.windows(2).all(|w| w[0] < w[1]), "{positions:?}");
        let mut old = file(1_000.0, None);
        old["format"] = json!(1);
        let t1 = text(&pool_of(vec![old]), false);
        for title in ["Problems", "Where the agent's time went", "Working with the agent", "Highest effort by client", "activity  "] {
            assert!(!t1.contains(title), "{title} in {t1}");
        }
    }

    #[test]
    fn a_file_without_the_new_parts_counts_in_none_of_them() {
        let p = pool_of(vec![friction_file(1_000.0, 10.0, 2.0, json!({})), file(1_000.0, None)]);
        assert!(p.problems.iter().all(|r| r.files_with_part == 1 || r.name == "longest request"), "{:?}", p.problems);
        assert_eq!(p.time_files, 1);
        let t = text(&p, false);
        assert!(
            t.contains("failed tool calls (in 1 of 2 files)")
                && t.contains("Where the agent's time went: what it waited on while it was busy, in 1 of 2 files"),
            "{t}"
        );
        assert!(t.lines().any(|l| l.starts_with("activity") && l.ends_with("(in 1 of 2 files)")), "{t}");
    }

    #[test]
    fn by_file_adds_a_column_per_file() {
        let p = pool_of(vec![file(100.0, None), file(200.0, None), file(300.0, None)]);
        let text = text(&p, true);
        let head = text.lines().find(|l| l.starts_with("agent   model") || l.starts_with("agent  model")).unwrap_or_else(|| panic!("{text}"));
        assert!(head.trim_end().ends_with("f0     f1     f2") || head.split_whitespace().rev().take(3).collect::<Vec<_>>() == ["f2", "f1", "f0"], "{head}");
    }
}
