//! The groups of tools and the call log (see tool-breakdown): the groups table that `tokprof
//! tools` and `tokprof scan --calls` print, the X-ray's groups line, and the call log of
//! `tokprof scan --calls`, and the window's group cards. Cells come apart from text, so that the
//! window can lay them out.

use std::fmt::Write as _;

use tokprof_core::calllog::{CallRow, CallStatus};
use tokprof_core::fmt::{self, Style, clip_end, local_time, table, tokens};
use tokprof_core::tools::{Group, GroupFilter, GroupRow, ToolTable, median_ms};
use tokprof_core::xray::XRay;

use crate::render_tools::{median_time, total_time};

/// The groups shown on the X-ray's groups line.
const LINE_GROUPS: usize = 5;
/// The most characters of the `input` cell.
pub const INPUT_CHARS: usize = 48;

/// The groups table as cells: the header, a row per group with a call, the `all` row, then the
/// servers never called; and the note after each row (`never called`, or empty).
pub struct GroupCells {
    pub rows: Vec<Vec<String>>,
    pub notes: Vec<String>,
}

/// The cells of the groups table of `t`.
pub fn group_cells(t: &ToolTable) -> GroupCells {
    let (groups, all) = t.groups();
    let mut rows = vec![["group", "calls", "failed", "time", "med time", "tokens"].map(str::to_string).to_vec()];
    let mut notes = vec![String::new()];
    let cells = |r: &GroupRow| {
        vec![
            r.name(),
            fmt::int(r.calls),
            if r.failed > 0 { fmt::int(r.failed) } else { String::new() },
            total_time(&r.durations, r.time_ms),
            median_time(r.median_ms()),
            tokens(r.result_tokens),
        ]
    };
    for r in groups.iter().filter(|r| !r.never_called) {
        rows.push(cells(r));
        notes.push(String::new());
    }
    rows.push(cells(&all));
    notes.push(String::new());
    for r in groups.iter().filter(|r| r.never_called) {
        rows.push(vec![r.name(), "0".to_string(), String::new(), String::new(), String::new(), String::new()]);
        notes.push("never called".to_string());
    }
    GroupCells { rows, notes }
}

/// The groups table as text; empty when no tool was called and no server was listed.
pub fn groups_table(t: &ToolTable) -> String {
    let cells = group_cells(t);
    if cells.rows.len() <= 2 && t.servers.is_empty() && !t.rows.values().any(|r| r.calls > 0) {
        return String::new();
    }
    with_notes(&table(&cells.rows, 1), &cells.notes)
}

/// Aligned lines with a note after each, past the widest line.
fn with_notes(lines: &str, notes: &[String]) -> String {
    let width = lines.lines().map(|l| l.chars().count()).max().unwrap_or(0);
    let mut out = String::new();
    for (line, note) in lines.lines().zip(notes) {
        let pad = width - line.chars().count();
        let _ = writeln!(out, "{}", format!("{line}{}  {note}", " ".repeat(pad)).trim_end());
    }
    out
}

/// A group card of the window (see desk-window, Group cards): the cells of the group's row of
/// the groups table, and its shares of the session's calls and of their counted time.
#[derive(Clone, Debug, PartialEq)]
pub struct CardCells {
    pub group: Group,
    pub name: String,
    /// `340 calls`.
    pub calls: String,
    /// `12 failed`, or `None` when none failed.
    pub failed: Option<String>,
    /// `<time> · med <med time>`, `– · med –` when no call counts for times.
    pub time: String,
    /// `<n> tok`.
    pub tokens: String,
    /// The group's share of the calls of all groups.
    pub calls_share: f64,
    /// Its share of their counted time; `None` when none of its calls counts for times.
    pub time_share: Option<f64>,
    /// A server the logs list that got no call: no numbers and no shares.
    pub never_called: bool,
}

/// The cards of the groups table of `t`, in its order, the servers never called last.
pub fn card_cells(t: &ToolTable) -> Vec<CardCells> {
    let (groups, all) = t.groups();
    groups
        .into_iter()
        .filter_map(|r| {
            let group = r.group.clone()?;
            let name = r.name();
            if r.never_called {
                return Some(CardCells {
                    group,
                    name,
                    calls: String::new(),
                    failed: None,
                    time: String::new(),
                    tokens: String::new(),
                    calls_share: 0.0,
                    time_share: None,
                    never_called: true,
                });
            }
            let timed = !r.durations.is_empty();
            Some(CardCells {
                group,
                name,
                calls: format!("{} {}", fmt::int(r.calls), if r.calls == 1 { "call" } else { "calls" }),
                failed: (r.failed > 0).then(|| format!("{} failed", fmt::int(r.failed))),
                time: format!("{} · med {}", total_time(&r.durations, r.time_ms), median_time(r.median_ms())),
                tokens: format!("{} tok", tokens(r.result_tokens)),
                calls_share: r.calls as f64 / all.calls.max(1) as f64,
                time_share: timed.then(|| r.time_ms as f64 / all.time_ms.max(1) as f64),
                never_called: false,
            })
        })
        .collect()
}

/// The X-ray's groups line: the first groups of the table, each with its calls and, when some of
/// them count for times, its time and median; `None` without calls.
pub fn groups_line(t: &ToolTable) -> Option<String> {
    let (groups, _) = t.groups();
    let called: Vec<&GroupRow> = groups.iter().filter(|r| !r.never_called).collect();
    if called.is_empty() {
        return None;
    }
    let mut parts: Vec<String> = called
        .iter()
        .take(LINE_GROUPS)
        .map(|r| match r.median_ms() {
            Some(med) => format!("{} {} ({}, med {})", r.name(), fmt::int(r.calls), fmt::call_time(r.time_ms as f64), fmt::call_time(med)),
            None => format!("{} {}", r.name(), fmt::int(r.calls)),
        })
        .collect();
    if called.len() > LINE_GROUPS {
        parts.push(format!("{} more", called.len() - LINE_GROUPS));
    }
    Some(parts.join(" · "))
}

/// What narrows the rows of the call log (see tool-breakdown, Narrowing the call log).
#[derive(Clone, Debug, Default)]
pub struct CallFilter {
    pub failed: bool,
    pub group: Option<GroupFilter>,
    pub tool: Option<String>,
}

impl CallFilter {
    pub fn any(&self) -> bool {
        self.failed || self.group.is_some() || self.tool.is_some()
    }

    pub fn keeps(&self, r: &CallRow) -> bool {
        (!self.failed || r.status == CallStatus::Failed)
            && self.group.as_ref().is_none_or(|g| g.keeps(&r.group))
            && self.tool.as_ref().is_none_or(|t| t.eq_ignore_ascii_case(&r.tool))
    }
}

/// The `calls` line of the call log.
pub fn calls_line(x: &XRay) -> String {
    let rows = &x.call_log;
    if rows.is_empty() {
        return "0 calls".to_string();
    }
    let (_, all) = x.tools.groups();
    let mut head = format!("{} in the main conversation", fmt::int(rows.len() as u64));
    if all.calls > rows.len() as u64 {
        let _ = write!(head, ", {} with the subagents'", fmt::int(all.calls));
    }
    let mut parts = vec![head];
    for status in [CallStatus::Failed, CallStatus::Refused, CallStatus::Denied, CallStatus::Running, CallStatus::NoResult, CallStatus::Background] {
        let n = rows.iter().filter(|r| r.status == status).count();
        if n > 0 {
            parts.push(format!("{} {}", fmt::int(n as u64), status.name()));
        }
    }
    let timed: Vec<u32> = rows.iter().filter(|r| r.timed).filter_map(|r| r.duration_ms).map(|ms| ms.clamp(0, u32::MAX as i64) as u32).collect();
    if let Some(med) = median_ms(&timed) {
        let sum: u64 = timed.iter().map(|&ms| ms as u64).sum();
        parts.push(format!("{} in calls, median {}", fmt::call_time(sum as f64), fmt::call_time(med)));
    }
    parts.join(" · ")
}

/// The header of the call log.
pub const CALL_HEADER: [&str; 9] = ["#", "started", "req", "group", "tool", "input", "status", "took", "tokens"];

/// The indices of the calls that `filter` keeps, in their order.
pub fn kept_calls(rows: &[CallRow], filter: &CallFilter) -> Vec<usize> {
    rows.iter().enumerate().filter(|(_, r)| filter.keeps(r)).map(|(i, _)| i).collect()
}

/// The cells of call `i` of the log.
pub fn call_row_cells(i: usize, r: &CallRow) -> Vec<String> {
    vec![
        (i + 1).to_string(),
        r.start_ms.map_or("–".to_string(), |t| local_time(t, "%m-%d %H:%M:%S")),
        r.request.map_or("–".to_string(), |n| n.to_string()),
        r.group.name(),
        r.tool.clone(),
        r.input.as_deref().map_or(String::new(), |t| clip_end(t, INPUT_CHARS)),
        r.status.name().to_string(),
        r.duration_ms.map_or("–".to_string(), |ms| fmt::call_time(ms as f64)),
        r.result_tokens.map_or("–".to_string(), tokens),
    ]
}

/// The cells of the call log: the header and a row per call that `filter` keeps.
pub fn call_cells(rows: &[CallRow], filter: &CallFilter) -> Vec<Vec<String>> {
    let mut out = vec![CALL_HEADER.map(str::to_string).to_vec()];
    out.extend(kept_calls(rows, filter).into_iter().map(|i| call_row_cells(i, &rows[i])));
    out
}

/// Which columns of the call log are right-aligned: `#`, `req`, `took` and `tokens`.
const RIGHT: [bool; 9] = [true, false, true, false, false, false, false, true, true];

/// `tokprof scan --calls`: the head with the groups table, then the rows `filter` keeps.
pub fn text(x: &XRay, filter: &CallFilter, style: &Style) -> String {
    let mut out = String::new();
    let label = |s: &str| style.dim(&format!("{s:<9}"));
    let _ = writeln!(out, "{}", style.bold("tokprof · tool calls of the main conversation"));
    let _ = writeln!(out, "{} {}  {}  {}", label("session"), x.agent.name(), style.bold(&x.project), x.id);
    let _ = writeln!(out, "{} {}", label("calls"), calls_line(x));
    if x.call_log.is_empty() {
        return out;
    }
    out.push('\n');
    out.push_str(&groups_table(&x.tools));
    out.push('\n');
    let cells = call_cells(&x.call_log, filter);
    let widths: Vec<usize> = (0..RIGHT.len()).map(|c| cells.iter().map(|r| r[c].chars().count()).max().unwrap_or(0)).collect();
    for (n, row) in cells.iter().enumerate() {
        let mut line = String::new();
        for (c, cell) in row.iter().enumerate() {
            let pad = " ".repeat(widths[c] - cell.chars().count());
            let cell = if n > 0 && c == 6 && row[6] == CallStatus::Failed.name() { style.paint("31", cell) } else { cell.clone() };
            if RIGHT[c] {
                let _ = write!(line, "{pad}{cell}  ");
            } else {
                let _ = write!(line, "{cell}{pad}  ");
            }
        }
        let line = line.trim_end();
        if n == 0 {
            let _ = writeln!(out, "{}", style.dim(line));
            if filter.any() {
                let _ = writeln!(out, "{}", style.dim(&format!("showing {} of {} calls", fmt::int(cells.len() as u64 - 1), fmt::int(x.call_log.len() as u64))));
            }
        } else {
            let _ = writeln!(out, "{line}");
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokprof_core::model::{Class, ToolClass};
    use tokprof_core::tools::{Group, ToolRow};

    /// A row of `calls` calls, `failed` of them failed, whose counted calls took `durations` ms.
    fn row(class: ToolClass, mcp: Option<&str>, calls: u64, failed: u64, durations: Vec<u32>) -> ToolRow {
        ToolRow {
            class: Class::Tool(class),
            calls,
            failed,
            time_ms: durations.iter().map(|&d| d as u64).sum(),
            durations,
            result_tokens: 1_000.0,
            mcp: mcp.map(str::to_string),
            ..ToolRow::default()
        }
    }

    /// `n` durations of `ms`, then one that brings their sum to `sum`.
    fn spread(n: usize, ms: u32, sum: u32) -> Vec<u32> {
        let mut v = vec![ms; n];
        v.push(sum - ms * n as u32);
        v
    }

    fn table_of(rows: Vec<(&str, ToolRow)>) -> ToolTable {
        let mut t = ToolTable::default();
        for (tool, r) in rows {
            t.rows.insert((tool.to_string(), None), r);
        }
        t
    }

    fn three_groups() -> ToolTable {
        table_of(vec![
            ("Bash", row(ToolClass::Execute, None, 340, 12, spread(299, 600, 2_520_000))),
            ("Read", row(ToolClass::Read, None, 120, 0, spread(119, 100, 24_000))),
            ("mcp__idea__find", row(ToolClass::Other, Some("idea"), 12, 1, spread(11, 1_200, 18_000))),
        ])
    }

    fn words(line: &str) -> Vec<&str> {
        line.split_whitespace().collect()
    }

    #[test]
    fn the_groups_table_has_a_row_per_group_and_the_totals() {
        let text = groups_table(&three_groups());
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(words(lines[0]), ["group", "calls", "failed", "time", "med", "time", "tokens"]);
        assert_eq!(words(lines[1]), ["shell", "340", "12", "42m", "0.6s", "1.0k"]);
        assert_eq!(words(lines[2]), ["files", "120", "24s", "0.1s", "1.0k"]);
        let files_failed = lines[0].find("failed").unwrap();
        assert_eq!(lines[2][files_failed..files_failed + 6].trim(), "", "an empty failed cell");
        assert_eq!(words(lines[3]), ["mcp", "idea", "12", "1", "18s", "1.2s", "1.0k"]);
        assert_eq!(words(lines[4]), ["all", "472", "13", "43m", "0.6s", "3.0k"]);
        assert_eq!(lines.len(), 5);
    }

    #[test]
    fn a_server_never_called_ends_the_groups_table() {
        let mut t = three_groups();
        t.servers.insert("linear".into());
        t.servers.insert("idea".into());
        let text = groups_table(&t);
        let last = text.lines().last().unwrap();
        assert_eq!(words(last), ["mcp", "linear", "0", "never", "called"]);
        assert_eq!(text.lines().filter(|l| l.contains("never called")).count(), 1, "a server with calls is not listed again");
        assert_eq!(groups_line(&t).unwrap().matches("linear").count(), 0, "the groups line leaves it out");
    }

    #[test]
    fn the_groups_line_shows_the_first_five_groups() {
        let mut t = three_groups();
        t.rows.insert(("Monitor".into(), None), row(ToolClass::Other, None, 30, 0, Vec::new()));
        t.rows.insert(("WebSearch".into(), None), row(ToolClass::Fetch, None, 5, 0, vec![12_000, 12_000, 12_000, 19_500, 19_500]));
        t.rows.insert(("Agent".into(), None), row(ToolClass::Subagent, None, 3, 0, vec![252_000, 252_000, 276_000]));
        assert_eq!(
            groups_line(&t).unwrap(),
            "shell 340 (42m, med 0.6s) · files 120 (24s, med 0.1s) · mcp idea 12 (18s, med 1.2s) · web 5 (1.3m, med 12s) · subagents 3 (13m, med 4.2m) · 1 more"
        );
        let only_background = table_of(vec![("Agent", row(ToolClass::Subagent, None, 4, 0, Vec::new()))]);
        assert_eq!(groups_line(&only_background).as_deref(), Some("subagents 4"));
        assert_eq!(groups_line(&ToolTable::default()), None);
    }

    #[test]
    fn tool_rows_show_their_time_and_median() {
        let mut t = table_of(vec![("Read", row(ToolClass::Read, None, 3, 0, vec![100, 200, 600]))]);
        t.rows.insert(("Bash".into(), Some("cargo test".into())), row(ToolClass::Execute, None, 5, 0, vec![20_000, 30_000, 40_000, 50_000]));
        t.rows.insert(("Agent".into(), None), row(ToolClass::Subagent, None, 2, 0, Vec::new()));
        let cells = crate::render_tools::tool_cells(&t, tokprof_core::tools::RowOrder::Rent, 25, true);
        assert_eq!(cells.rows[0][..4], ["tool", "calls", "time", "med time"]);
        let find = |name: &str| cells.rows.iter().find(|r| r[0] == name).unwrap()[..4].to_vec();
        assert_eq!(find("Read"), ["Read", "3", "0.9s", "0.2s"]);
        assert_eq!(find("Bash cargo test"), ["Bash cargo test", "5", "2.3m", "35s"]);
        assert_eq!(find("Agent"), ["Agent", "2", "–", "–"]);
    }

    fn call(tool: &str, group: Group, status: CallStatus, ms: Option<i64>, input: Option<&str>) -> CallRow {
        CallRow {
            start_ms: Some(1_763_560_812_000),
            request: Some(5),
            tool: tool.into(),
            group,
            input: input.map(str::to_string),
            status,
            duration_ms: ms,
            timed: ms.is_some() && matches!(status, CallStatus::Done | CallStatus::Failed),
            result_tokens: Some(3_400.0),
        }
    }

    fn session(rows: Vec<CallRow>) -> XRay {
        let mut x = crate::render_xray::tests::xray(Vec::new());
        x.tools = table_of(vec![
            ("Bash", row(ToolClass::Execute, None, 3, 1, vec![41_000])),
            ("Read", row(ToolClass::Read, None, 2, 0, vec![100])),
            ("Agent", row(ToolClass::Subagent, None, 1, 0, Vec::new())),
        ]);
        x.call_log = rows;
        x
    }

    #[test]
    fn the_call_log_has_a_head_and_a_row_per_call() {
        let test = "cargo test -p tokprof --test golden";
        let x = session(vec![
            call("Bash", Group::Shell, CallStatus::Failed, Some(41_000), Some(test)),
            call("Read", Group::Files, CallStatus::Done, Some(100), Some("src/a.rs")),
            call("Bash", Group::Shell, CallStatus::Background, Some(200), Some("npm run dev")),
            call("Agent", Group::Subagents, CallStatus::Done, Some(250_000), Some("Find the parser")),
            call("mcp__idea__find", Group::Mcp("idea".into()), CallStatus::Running, None, None),
        ]);
        let text = text(&x, &CallFilter::default(), &Style::with_color(false));
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines[0], "tokprof · tool calls of the main conversation");
        assert!(lines[1].starts_with("session   claude  ~/p"), "{}", lines[1]);
        assert_eq!(lines[2], "calls     5 in the main conversation, 6 with the subagents' · 1 failed · 1 running · 1 background · 4.9m in calls, median 41s");
        let header = lines.iter().position(|l| l.trim_start().starts_with("#  started")).unwrap();
        assert!(lines[4].starts_with("group") && lines[header - 1].is_empty(), "the groups table, then the log: {text}");
        let first = words(lines[header + 1]);
        assert_eq!(first[0], "1");
        assert_eq!(first[3..], ["5", "shell", "Bash", "cargo", "test", "-p", "tokprof", "--test", "golden", "failed", "41s", "3.4k"]);
        assert!(words(lines[header + 4]).ends_with(&["subagents", "Agent", "Find", "the", "parser", "done", "4.2m", "3.4k"]));
        assert!(words(lines[header + 5]).ends_with(&["mcp", "idea", "mcp__idea__find", "running", "–", "3.4k"]));
        let empty = session(Vec::new());
        assert_eq!(super::text(&empty, &CallFilter::default(), &Style::with_color(false)).lines().last(), Some("calls     0 calls"));
    }

    #[test]
    fn a_long_input_is_cut_and_a_missing_one_is_empty() {
        let long = "x".repeat(70);
        let cells = call_cells(
            &[call("Bash", Group::Shell, CallStatus::Done, Some(1), Some(&long)), call("Monitor", Group::Other, CallStatus::Done, None, None)],
            &CallFilter::default(),
        );
        assert_eq!(cells[1][5], format!("{}…", "x".repeat(47)));
        assert_eq!((cells[2][5].as_str(), cells[2][7].as_str()), ("", "–"));
    }

    #[test]
    fn filters_narrow_the_rows_and_say_how_many_are_shown() {
        let mut rows = Vec::new();
        for i in 0..412 {
            let (tool, group) = if i % 2 == 0 { ("Bash", Group::Shell) } else { ("Read", Group::Files) };
            let failed = (i < 14 && i % 2 == 0) || i == 1 || i == 3;
            rows.push(call(tool, group, if failed { CallStatus::Failed } else { CallStatus::Done }, Some(100), None));
        }
        rows.push(call("mcp__idea__find", Group::Mcp("idea".into()), CallStatus::Done, Some(100), None));
        rows.push(call("mcp__linear__get", Group::Mcp("linear".into()), CallStatus::Done, Some(100), None));
        let x = session(rows);
        let shown = |f: CallFilter| {
            let text = text(&x, &f, &Style::with_color(false));
            let line = text.lines().find(|l| l.starts_with("showing")).map(str::to_string);
            (line, call_cells(&x.call_log, &f).len() - 1)
        };
        let failed_shell = CallFilter { failed: true, group: GroupFilter::parse("shell"), tool: None };
        assert_eq!(shown(failed_shell), (Some("showing 7 of 414 calls".to_string()), 7));
        assert_eq!(shown(CallFilter { group: GroupFilter::parse("mcp"), ..CallFilter::default() }).1, 2);
        assert_eq!(shown(CallFilter { tool: Some("read".into()), ..CallFilter::default() }).1, 206);
        assert_eq!(shown(CallFilter::default()).0, None);
    }

    #[test]
    fn the_cards_hold_the_numbers_of_the_groups_table() {
        let mut t = three_groups();
        t.servers.insert("linear".into());
        let cards = card_cells(&t);
        let table = group_cells(&t);
        // The rows with calls, then `all`, then the server never called.
        let called: Vec<&Vec<String>> = table.rows.iter().skip(1).filter(|r| r[0] != "all" && r[1] != "0").collect();
        assert_eq!(cards.len(), called.len() + 1);
        for (card, row) in cards.iter().zip(&called) {
            assert_eq!(card.name, row[0]);
            assert_eq!(card.calls, format!("{} calls", row[1]));
            assert_eq!(card.failed, (!row[2].is_empty()).then(|| format!("{} failed", row[2])));
            assert_eq!(card.time, format!("{} · med {}", row[3], row[4]));
            assert_eq!(card.tokens, format!("{} tok", row[5]));
        }
        let shell = &cards[0];
        assert_eq!((shell.name.as_str(), shell.calls.as_str(), shell.failed.as_deref()), ("shell", "340 calls", Some("12 failed")));
        assert_eq!(format!("{:.0}%", 100.0 * shell.calls_share), "72%", "340 of 472 calls");
        assert_eq!(format!("{:.0}%", 100.0 * shell.time_share.unwrap()), "98%", "42 of 43 minutes");
        assert_eq!(cards[1].failed, None, "files has no failed call");
        let never = cards.last().unwrap();
        assert!(never.never_called && never.name == "mcp linear" && never.calls.is_empty() && never.time_share.is_none());
    }

    #[test]
    fn a_card_without_counted_time_has_no_time_share() {
        let t = table_of(vec![("Agent", row(ToolClass::Subagent, None, 4, 0, Vec::new())), ("Read", row(ToolClass::Read, None, 2, 0, vec![100, 300]))]);
        let cards = card_cells(&t);
        let agent = cards.iter().find(|c| c.name == "subagents").unwrap();
        assert_eq!((agent.time.as_str(), agent.time_share), ("– · med –", None));
        assert_eq!(cards.iter().find(|c| c.name == "files").unwrap().time_share, Some(1.0));
        assert_eq!(cards[0].calls, "4 calls");
    }

    #[test]
    fn the_group_filter_keeps_the_tool_rows_of_its_group() {
        let mut t = three_groups();
        t.rows.insert(("Bash".into(), Some("cargo test".into())), row(ToolClass::Execute, None, 5, 0, vec![1_000]));
        let all = crate::render_tools::tool_cells(&t, tokprof_core::tools::RowOrder::Rent, 25, false);
        assert_eq!(all.rows.len(), 5);
        let shell = GroupFilter::parse("shell");
        let cells = crate::render_tools::tool_cells_of(&t, tokprof_core::tools::RowOrder::Rent, 25, false, shell.as_ref());
        let names: Vec<&str> = cells.rows.iter().skip(1).map(|r| r[0].as_str()).collect();
        assert_eq!(names.len(), 2);
        assert!(names.iter().all(|n| n.starts_with("Bash")), "{names:?}");
        let one = crate::render_tools::tool_cells_of(&t, tokprof_core::tools::RowOrder::Rent, 1, false, shell.as_ref());
        assert_eq!(one.more, 1, "more counts the rows of the group");
        let mcp = crate::render_tools::tool_cells_of(&t, tokprof_core::tools::RowOrder::Rent, 25, false, GroupFilter::parse("mcp idea").as_ref());
        assert_eq!(mcp.rows.len(), 2);
        // The same filter narrows the call log.
        let rows = vec![call("Bash", Group::Shell, CallStatus::Done, Some(1), None), call("Read", Group::Files, CallStatus::Done, Some(1), None)];
        let f = CallFilter { group: shell, ..CallFilter::default() };
        assert_eq!(kept_calls(&rows, &f), [0]);
        assert_eq!(call_cells(&rows, &f)[1], call_row_cells(0, &rows[0]));
    }
}
