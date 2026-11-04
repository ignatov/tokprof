//! The report of `tokprof pool` over share files, and of `tokprof share` over the one file it
//! wrote: the header, where the money went, what could have been saved, the habits that cost the
//! most, tools and commands, and the agent versions.

use std::collections::BTreeMap;
use std::fmt::Write;

use tokprof_core::fmt::{count, int, table, tokens, usd};
use tokprof_core::pool::{PainKind, PainRow, Pool, PrevalenceRow, RateRow, RhythmRow, RhythmUnit, Row, SpreadRow, SpreadUnit, Table, Unit};
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
                "activity  {} active days · {} active hours{} · busy {} · {} per active hour{}",
                a.active_days,
                a.active_hours,
                keyboard_part(a.keyboard_ms, ""),
                busy_hours(a.busy_ms),
                usd(a.usd_per_hour),
                a.usd_per_keyboard_hour.map_or(String::new(), |u| format!(" · {} per hour at the keyboard", usd(u)))
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
                "activity  {} active hours{} · {} per active hour{} · per file: median {} active days, {} active hours, {} per active hour{}",
                a.active_hours,
                keyboard_part(a.keyboard_ms, &files_note(a.keyboard_files, p.files.len())),
                usd(a.usd_per_hour),
                a.usd_per_keyboard_hour.map_or(String::new(), |u| format!(" · {} per hour at the keyboard", usd(u))),
                a.median_days,
                a.median_hours,
                usd(a.median_usd_per_hour),
                if a.files < p.files.len() { format!(" (in {} of {} files)", a.files, p.files.len()) } else { String::new() }
            );
        }
    }
    out
}

/// The hours at the keyboard of the activity line, as whole hours, with how many files have them.
fn keyboard_part(ms: Option<f64>, files: &str) -> String {
    ms.map_or(String::new(), |ms| format!(" · {}h at the keyboard{files}", int((ms / 3_600_000.0).round() as u64)))
}

/// ` (in <k> of <n> files)` when some files lack a part.
fn files_note(with_part: usize, files: usize) -> String {
    if with_part < files { format!(" (in {with_part} of {files} files)") } else { String::new() }
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
        let mut name = if r.under { format!("  {}", r.name) } else { r.name.clone() };
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
        let mut cells = vec![r.name.clone(), int(r.calls), int(r.failed), usd(r.usd), usd(r.trips_usd)];
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
        let mut cells = vec![r.agent.clone().unwrap_or_default(), r.name.clone(), int(r.requests)];
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

/// The note under each section's title (see share-pool, Section notes).
const NOTES: [(&str, &str); 17] = [
    (
        "Pains side by side",
        "each pain by the people it touches: in a quarter of their days or sessions or more, or half of their spend for effort; the rows overlap, so money and time are not added up",
    ),
    ("Where the money went", "what the spend paid for: model, content, origin, activity, prompt size, session length and client"),
    ("What could have been saved", "dollars the prompt cache lost to misses, by cause, and what the cheaper cache lifetime would have kept"),
    ("Cold returns by client", "returns to a session after its cache expired, per active hour"),
    ("Coming back after a cold break", "what the cold returns cost as resumed, against the other ways to come back (see tokprof bench)"),
    ("Problems", "calls that failed, requests that waited for retries, and slow requests, per active hour"),
    ("How often it happens", "the active days and main sessions in which each happened at least once"),
    ("How the sessions spread", "per session, cold return, turn or answer: the quarters and the 90th percentile, given from 5 values on"),
    ("Habits that cost the most", "where to look, not what could be saved; the rows overlap"),
    ("Where the agent's time went", "what the agents waited on while busy; after turns: while their own background work ran on"),
    ("Your time with the agents", "your sittings and answers, and what the agents did after you left"),
    ("Working with the agent", "hand-offs per hour at the keyboard, and per 100 typed messages for those a message starts"),
    ("Working with the agent by client", "the same by the program that ran the agent, such as an IDE or a terminal"),
    ("Commits and pull requests", "what the agents committed, pushed and opened, from their shell calls that did not fail"),
    ("Tools and commands", "rent: what keeping a tool's results in the context cost; trips: the requests its calls made the agent send"),
    ("Highest effort by client", "the share of each client's spend and requests at max or xhigh"),
    ("Versions", "the agent versions in use, by requests"),
];

/// A section's title, with `in <k> of <n> files` when some files lack its parts, and its note.
fn section_head(out: &mut String, title: &str, coverage: &str, one: bool) {
    let note = NOTES.iter().find(|(t, _)| *t == title).map_or("", |(_, n)| n);
    let extra = match (title, one) {
        ("Where the money went", false) => "; median file: the median of each file's own share",
        ("What could have been saved", false) => "; files 5%+: the files where a row is 5% of their spend or more",
        ("How often it happens", false) => "; files: where it happened at all; files 25%+: in a quarter of their days or sessions or more",
        ("How the sessions spread", false) => "; p50 and q1–q3: the median and quarters of the files' own p50; p90: the median of their p90",
        _ => "",
    };
    let _ = write!(out, "\n{title}{coverage}\n{note}{extra}\n");
}

/// The median of some values, the upper of two middle ones.
fn median(values: &[f64]) -> Option<f64> {
    let mut v = values.to_vec();
    v.sort_by(f64::total_cmp);
    v.get(v.len() / 2).copied()
}

/// How `At a glance` names an error class of the retried requests.
fn class_phrase(row: &str) -> &str {
    match row {
        "authentication" => "authentication errors",
        "connection" => "connection errors",
        "computer asleep" => "the computer asleep",
        other => other,
    }
}

/// The lines of `At a glance`, each from the table of the section it sums up, and each left out
/// when no file has its parts (see share-pool, At a glance).
fn glance(p: &Pool) -> Vec<String> {
    let one = p.files.len() == 1;
    let mut lines = Vec::new();
    let in_median = |x: String| if one { String::new() } else { format!(" (median file {x})") };
    let mut spend = usd(p.spend_usd);
    if let Some(a) = &p.activity {
        spend += &format!(" · {} per active hour{}", usd(a.usd_per_hour), in_median(usd(a.median_usd_per_hour)));
        if let (Some(u), Some(m)) = (a.usd_per_keyboard_hour, a.median_usd_per_keyboard_hour) {
            spend += &format!(" · {} per hour at the keyboard{}{}", usd(u), in_median(usd(m)), files_note(a.keyboard_files, p.files.len()));
        }
    }
    lines.push(format!("spend     {spend}"));
    if let Some(t) = p.table("saved").filter(|t| t.files_with_part > 0 && !t.rows.is_empty()) {
        let total: f64 = t.rows.iter().map(|r| r.usd).sum();
        let share: f64 = t.rows.iter().map(|r| r.share).sum();
        let mut top: Vec<&Row> = t.rows.iter().filter(|r| r.usd > 0.0).collect();
        top.sort_by(|a, b| b.usd.total_cmp(&a.usd));
        let largest: Vec<String> = top.iter().take(2).map(|r| format!("{} {}", r.name, usd(r.usd))).collect();
        let per_file: Vec<f64> = (0..p.files.len())
            .filter_map(|i| {
                let shares: Vec<f64> = t.rows.iter().filter_map(|r| r.by_file.get(i).copied().flatten()).collect();
                (!shares.is_empty()).then(|| shares.iter().sum())
            })
            .collect();
        let median_share = if one { String::new() } else { median(&per_file).map_or(String::new(), |m| format!(", median file {:.1}%", 100.0 * m)) };
        lines.push(format!("cache     {} could have been saved ({:.1}%{median_share}): {}", usd(total), 100.0 * share, largest.join(", ")));
    }
    if let Some(b) = &p.bench {
        let (resume, note, beats) = (&b.rows[0], &b.rows[2], &b.rows[3]);
        let of_resume = if one { String::new() } else { note.median_file.map_or(String::new(), |m| format!(" ({} of resuming in the median file)", pct(m))) };
        lines.push(format!(
            "returns   {} cold returns cost {} as resumed; a handoff note would have cost {}{of_resume} and was cheapest in {}; heartbeats {}{}",
            int(b.returns),
            usd(resume.usd),
            usd(note.usd),
            int(note.cheapest),
            usd(beats.usd),
            files_note(b.files, p.files.len())
        ));
    }
    if !p.problems.is_empty() {
        let find = |name: &str| p.problems.iter().find(|r| r.name == name);
        let mut parts = Vec::new();
        if let Some(f) = find("failed tool calls") {
            let after = find("spend after failures").map_or(String::new(), |a| format!(", {} after them", usd(a.value)));
            parts.push(format!("{} failed calls{after}", int(f.value.round() as u64)));
        }
        if let Some(r) = find("requests that waited for retries") {
            if r.value > 0.0 {
                let time = find("time waiting for retries").map_or(String::new(), |t| format!(" {}", span(t.value)));
                let classes: Vec<&RateRow> = p.problems.iter().filter(|c| c.under).collect();
                let classed: f64 = classes.iter().map(|c| c.value).sum();
                let top = classes.iter().max_by(|a, b| a.value.total_cmp(&b.value));
                // Older files hold no classes: the share is then of the requests that have one.
                let of = if classed < r.value { format!("of the {} with a class", int(classed.round() as u64)) } else { "of them".to_string() };
                let class = top.map_or(String::new(), |c| format!(", {} {of} for {}", int(c.value.round() as u64), class_phrase(&c.name)));
                parts.push(format!("{} requests waited{time} for retries{class}", int(r.value.round() as u64)));
            } else {
                parts.push("no request waited for retries".to_string());
            }
        }
        if !parts.is_empty() {
            lines.push(format!("problems  {}", parts.join("; ")));
        }
    }
    if !p.rhythm.is_empty() {
        let row = |name: &str| p.rhythm.iter().find(|r| r.name == name);
        let total = |name: &str| row(name).and_then(|r| r.value).unwrap_or(0.0);
        let wait = |name: &str| row(name).and_then(|r| r.median_file).map(|ms| duration(ms.round() as i64));
        let mut text = format!("{}h at the keyboard in {} sittings", int(total("time at the keyboard").round() as u64), int(total("sittings").round() as u64));
        match (one, wait("you answered an end of turn in"), wait("the agents stood still after you left")) {
            (true, answered, still) => {
                if let Some(a) = answered {
                    text += &format!("; you answered in a median {a}");
                }
                if let Some(s) = still {
                    text += &format!("; the agents stood still a median {s} after you left");
                }
            }
            (false, answered, still) => {
                if let Some(a) = answered {
                    text += &format!("; in the median file, answers came in {a}");
                }
                if let Some(s) = still {
                    text += &format!(", and the agents stood still {s} after the person left");
                }
            }
        }
        lines.push(format!("you       {text}{}", files_note(p.rhythm_files, p.files.len())));
    }
    if let Some(o) = &p.outcomes {
        let n = |i: usize| int(o.rows[i].value.round() as u64);
        lines.push(match o.usd_per_commit {
            Some(u) => format!(
                "made      the agents made {} commits, {} pushes and {} pull requests: {} of spend per commit{}",
                n(0),
                n(1),
                n(2),
                usd(u),
                files_note(o.files, p.files.len())
            ),
            None => format!("made      no commits by the agents{}", files_note(o.files, p.files.len())),
        });
    }
    lines
}

/// A value of `Your time with the agents`, in its row's unit.
fn rhythm_cell(unit: RhythmUnit, x: f64) -> String {
    match unit {
        RhythmUnit::Hours => format!("{x:.1}h"),
        RhythmUnit::Count => int(x.round() as u64),
        RhythmUnit::Duration => duration(x.round() as i64),
        RhythmUnit::Share => format!("{:.0}%", 100.0 * x),
        RhythmUnit::Rate => format!("{x:.2}"),
    }
}

/// The table of `Your time with the agents`. One file shows its own values; a pool, the pooled
/// value (for a median, the pause range of the pooled median), the median file and the files.
fn rhythm_table(p: &Pool, rows: &[RhythmRow], by_file: bool) -> String {
    let one = p.files.len() == 1;
    let mut head = vec!["item".to_string()];
    if one {
        head.push("value".into());
    } else {
        head.extend(["pooled", "median file", "files"].map(str::to_string));
    }
    if by_file {
        head.extend(p.files.iter().map(|f| f.name.clone()));
    }
    let mut out = vec![head];
    let cell = |unit, x: Option<f64>| x.map_or("–".to_string(), |x| rhythm_cell(unit, x));
    for r in rows {
        let mut cells = vec![r.name.clone()];
        if one {
            cells.push(cell(r.unit, r.median_file));
        } else {
            cells.push(r.value.map(|x| rhythm_cell(r.unit, x)).or_else(|| r.range.clone()).unwrap_or_else(|| "–".to_string()));
            cells.push(cell(r.unit, r.median_file));
            cells.push(r.files.to_string());
        }
        if by_file {
            cells.extend(r.by_file.iter().map(|x| cell(r.unit, *x)));
        }
        out.push(cells);
    }
    table(&out, 1)
}

/// A count of files, with its share of the files that have the row.
fn files_cell(k: usize, of: usize) -> String {
    format!("{k} ({:.0}%)", 100.0 * k as f64 / of.max(1) as f64)
}

/// The table of `How often it happens`. One file shows its counts; a pool, the pooled counts, the
/// median file's share and the files where it happened at all and often.
fn prevalence_table(p: &Pool, rows: &[PrevalenceRow], by_file: bool) -> String {
    let one = p.files.len() == 1;
    let mut head = ["item", "count", "of", "share"].map(str::to_string).to_vec();
    if !one {
        head.extend(["median file", "files", "files 25%+"].map(str::to_string));
    }
    if by_file {
        head.extend(p.files.iter().map(|f| f.name.clone()));
    }
    let mut out = vec![head];
    for r in rows {
        let mut name = r.name.to_string();
        // The title says in how many files the part is; a row says so only when it differs.
        if !one && r.files_with_part != p.prevalence_files {
            name += &format!(" (in {} of {} files)", r.files_with_part, p.files.len());
        }
        let base = int(r.base.round() as u64) + if r.days { " days" } else { " sessions" };
        let mut cells = vec![name, int(r.count.round() as u64), base, pct(r.count / r.base.max(1e-9))];
        if !one {
            cells.push(r.median_file.map_or("–".to_string(), pct));
            cells.push(files_cell(r.files, r.files_with_part));
            cells.push(files_cell(r.common_files, r.files_with_part));
        }
        if by_file {
            cells.extend(r.by_file.iter().map(|x| x.map_or("–".to_string(), pct)));
        }
        out.push(cells);
    }
    table(&out, 1)
}

/// The line under the tables of sessions: the main sessions their rows leave out, as the rhythm
/// leaves them out. Empty when there are none.
fn left_out_line(p: &Pool) -> String {
    let n = p.left_out_sessions.round() as u64;
    if n == 0 {
        return String::new();
    }
    format!("the session rows leave out {} that nobody typed in: a program ran them, or they ran in a temporary folder\n", count(n, "session", "sessions"))
}

/// A value of `How the sessions spread`, in its row's unit.
fn spread_cell(unit: SpreadUnit, x: f64) -> String {
    match unit {
        SpreadUnit::Usd => usd(x),
        SpreadUnit::Count => int(x.round() as u64),
        SpreadUnit::Ms => span(x),
        SpreadUnit::Tokens => tokens(x),
    }
}

/// The table of `How the sessions spread`. One file shows its values and quantiles; a pool, the
/// values of all files, the median and quarters of the files' `p50`, and the median `p90`.
fn spread_table(p: &Pool, rows: &[SpreadRow], by_file: bool) -> String {
    let one = p.files.len() == 1;
    let mut head = ["agent", "measure", "values"].map(str::to_string).to_vec();
    if one {
        head.extend(["p25", "p50", "p75", "p90"].map(str::to_string));
    } else {
        head.extend(["p50", "q1–q3", "p90", "files"].map(str::to_string));
    }
    if by_file {
        head.extend(p.files.iter().map(|f| format!("{} p50", f.name)));
    }
    let mut out = vec![head];
    for r in rows {
        let cell = |x: f64| spread_cell(r.unit, x);
        let mut cells = vec![r.agent.clone(), r.name.to_string(), int(r.n.round() as u64)];
        if one {
            let q = r.by_file.iter().flatten().next();
            cells.extend((0..4).map(|i| q.map_or("–".to_string(), |q| cell(q[i]))));
        } else {
            cells.push(cell(r.median_p50));
            cells.push(format!("{}–{}", cell(r.quarter_1_p50), cell(r.quarter_3_p50)));
            cells.push(cell(r.median_p90));
            cells.push(if r.files_with_part > r.files { format!("{} of {}", r.files, r.files_with_part) } else { r.files.to_string() });
        }
        if by_file {
            cells.extend(r.by_file.iter().map(|q| q.map_or("–".to_string(), |q| cell(q[1]))));
        }
        out.push(cells);
    }
    table(&out, 2)
}

/// A value of `Pains side by side` in its row's kind: dollars or a time.
fn pain_cell(r: &PainRow, x: Option<f64>) -> String {
    match (r.kind, x) {
        (PainKind::None, _) | (_, None) => "–".into(),
        (PainKind::Time, Some(ms)) => span(ms),
        (_, Some(usd_)) => usd(usd_),
    }
}

/// The table of `Pains side by side`: per pain, the people it touches of the files with its
/// fields, the kind of its money, the median file's money or time per active hour, and the pooled
/// money or time; with `--by-file`, each file's share that the people rule reads.
fn pains_table(p: &Pool, by_file: bool) -> String {
    let mut head = ["pain", "people", "kind", "median file per active hour", "pooled"].map(str::to_string).to_vec();
    if by_file {
        head.extend(p.files.iter().map(|f| f.name.clone()));
    }
    let mut out = vec![head];
    for r in &p.pains {
        let mut name = r.name.to_string();
        if r.of < p.files.len() {
            name += &format!(" (in {} of {} files)", r.of, p.files.len());
        }
        let kind = match r.kind {
            PainKind::None => "–",
            PainKind::Avoidable => "avoidable",
            PainKind::Spent => "spent",
            PainKind::Time => "time",
        };
        let mut cells = vec![name, format!("{} of {}", r.people, r.of), kind.to_string(), pain_cell(r, r.median_per_hour), pain_cell(r, r.pooled)];
        if by_file {
            cells.extend(r.by_file.iter().map(|x| x.map_or("–".to_string(), pct)));
        }
        out.push(cells);
    }
    table(&out, 1)
}

/// The report over the pool. With one file, the columns about files are left out.
pub fn text(p: &Pool, by_file: bool) -> String {
    let mut out = header(p);
    let n = p.files.len();
    let one = n == 1;
    let present = |t: &&Table| t.files_with_part > 0 && !t.rows.is_empty();

    let lines = glance(p);
    if !lines.is_empty() {
        out.push_str("\nAt a glance\n\n");
        for l in lines {
            out.push_str(&l);
            out.push('\n');
        }
    }
    // A pain touches people, so one file has no such section.
    if !one && !p.pains.is_empty() {
        section_head(&mut out, "Pains side by side", "", one);
        out.push('\n');
        out.push_str(&pains_table(p, by_file));
    }

    section_head(&mut out, "Where the money went", "", one);
    for (key, cols) in SPEND_TABLES {
        let Some(t) = p.table(key).filter(present) else { continue };
        let _ = write!(out, "\n{}", coverage_line(t, n));
        out.push_str(&money_table(p, t, cols, "cost", "files", false, by_file));
        if key == "model" && !p.unpriced.is_empty() {
            let models: Vec<String> = p
                .unpriced
                .iter()
                .map(|u| {
                    let files = if one { String::new() } else { format!(", {}", count(u.files as u64, "file", "files")) };
                    format!("{} ({} tokens{files})", u.model, tokens(u.tokens))
                })
                .collect();
            let _ = writeln!(out, "models without a price: {}; their tokens count as $0", models.join(", "));
        }
        if let (true, Some(c)) = (key == "content", &p.coverage)
            && c.share < 0.9995
        {
            let below = if one { String::new() } else { format!(", in {} files below 90%", c.files_below_90) };
            let _ = writeln!(out, "prompts the logs hold: {:.1}% of the spend; the rest is unknown{below}", 100.0 * c.share);
        }
    }

    if let Some(t) = p.table("saved").filter(present) {
        section_head(&mut out, "What could have been saved", "", one);
        out.push('\n');
        out.push_str(&money_table(p, t, &["item"], "avoidable", "files 5%+", true, by_file));
        if !p.cold_by_client.is_empty() {
            section_head(&mut out, "Cold returns by client", &coverage(p.cold_files, n), one);
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
            out.push('\n');
            out.push_str(&table(&rows, 2));
        }
    }
    if let Some(b) = &p.bench {
        section_head(&mut out, "Coming back after a cold break", &coverage(b.files, n), one);
        let mut head = ["way", "cost", "of resume", "cheapest in"].map(str::to_string).to_vec();
        if !one {
            head.extend(["median file", "files"].map(str::to_string));
        }
        if by_file {
            head.extend(p.files.iter().map(|f| f.name.clone()));
        }
        let mut rows = vec![head];
        for r in &b.rows {
            let mut cells = vec![r.name.clone(), usd(r.usd), pct(r.of_resume), format!("{} of {}", int(r.cheapest), int(b.returns))];
            if !one {
                cells.push(r.median_file.map_or("–".to_string(), pct));
                cells.push(b.files.to_string());
            }
            if by_file {
                cells.extend(r.by_file.iter().map(|x| x.map_or("–".to_string(), pct)));
            }
            rows.push(cells);
        }
        out.push('\n');
        out.push_str(&table(&rows, 1));
    }
    if !p.problems.is_empty() {
        section_head(&mut out, "Problems", "", one);
        out.push('\n');
        out.push_str(&rate_table(p, &p.problems, "measure", "per active hour", by_file));
    }
    if !p.prevalence.is_empty() {
        section_head(&mut out, "How often it happens", &coverage(p.prevalence_files, n), one);
        out.push('\n');
        out.push_str(&prevalence_table(p, &p.prevalence, by_file));
        out.push_str(&left_out_line(p));
    }
    if !p.spread.is_empty() {
        section_head(&mut out, "How the sessions spread", &coverage(p.spread_files, n), one);
        out.push('\n');
        out.push_str(&spread_table(p, &p.spread, by_file));
        out.push_str(&left_out_line(p));
    }
    if let Some(t) = p.table("habits").filter(present) {
        section_head(&mut out, "Habits that cost the most", "", one);
        out.push('\n');
        out.push_str(&money_table(p, t, &["habit"], "cost", "files 5%+", true, by_file));
    }
    if !p.time.is_empty() {
        section_head(&mut out, "Where the agent's time went", &coverage(p.time_files, n), one);
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
        out.push('\n');
        out.push_str(&table(&rows, 1));
    }
    if !p.rhythm.is_empty() {
        section_head(&mut out, "Your time with the agents", &coverage(p.rhythm_files, n), one);
        out.push('\n');
        out.push_str(&rhythm_table(p, &p.rhythm, by_file));
    }
    // Without the hours at the keyboard, the hand-offs have no base.
    if p.working.iter().any(|r| r.files_with_part > 0) {
        section_head(&mut out, "Working with the agent", "", one);
        let mut head = vec![
            "hand-off".to_string(),
            if one { "value".into() } else { "pooled".into() },
            "per hour at the keyboard".into(),
            "per 100 typed messages".into(),
        ];
        if !one {
            head.push("median file".into());
            head.push("files".into());
        }
        if by_file {
            head.extend(p.files.iter().map(|f| f.name.clone()));
        }
        let mut rows = vec![head];
        for r in &p.working {
            let mut name = r.name.clone();
            if !one && r.files_with_part < p.files.len() {
                name += &format!(" (in {} of {} files)", r.files_with_part, p.files.len());
            }
            let mut cells = vec![name, value_cell(r), rate_cell(r, r.rate), r.second_rate.map_or("–".to_string(), |x| format!("{x:.2}"))];
            if !one {
                cells.push(rate_cell(r, r.median_rate));
                cells.push(r.files.to_string());
            }
            if by_file {
                cells.extend(r.by_file.iter().map(|x| x.map_or("–".to_string(), |x| rate_cell(r, Some(x)))));
            }
            rows.push(cells);
        }
        out.push('\n');
        out.push_str(&table(&rows, 1));
    }
    if !p.client_work.is_empty() {
        section_head(&mut out, "Working with the agent by client", &coverage(p.client_work_files, n), one);
        let mut rows =
            vec![["agent", "client", "at the keyboard", "messages", "interrupts", "questions", "commits", "answered in"].map(str::to_string).to_vec()];
        for w in &p.client_work {
            let per = |x: f64| format!("{:.2}", x / w.keyboard_hours.max(1e-9));
            rows.push(vec![
                w.agent.clone().unwrap_or_default(),
                w.client.clone(),
                format!("{:.1}h", w.keyboard_hours),
                per(w.messages),
                per(w.interrupts),
                per(w.questions),
                per(w.commits),
                w.answered_ms.map_or("–".to_string(), |ms| duration(ms.round() as i64)),
            ]);
        }
        out.push('\n');
        out.push_str(&table(&rows, 2));
    }
    if let Some(o) = &p.outcomes {
        section_head(&mut out, "Commits and pull requests", &coverage(o.files, n), one);
        out.push('\n');
        out.push_str(&rate_table(p, &o.rows, "action", "per hour at the keyboard", by_file));
        let _ = writeln!(out, "{}", o.usd_per_commit.map_or("no commits by the agents".to_string(), |u| format!("{} of spend per commit", usd(u))));
    }
    if let Some(t) = p.table("tools").filter(present) {
        section_head(&mut out, "Tools and commands", &coverage(t.files_with_part, n), one);
        out.push('\n');
        out.push_str(&tools_table(p, t, by_file));
    }
    if !p.effort_by_client.is_empty() {
        section_head(&mut out, "Highest effort by client", &coverage(p.effort_files, n), one);
        let mut head = ["agent", "client", "cost", "of spend", "of requests"].map(str::to_string).to_vec();
        if !one {
            head.extend(["median file", "files"].map(str::to_string));
        }
        if by_file {
            head.extend(p.files.iter().map(|f| f.name.clone()));
        }
        let mut rows = vec![head];
        for r in &p.effort_by_client {
            let mut cells = vec![r.agent.clone(), r.client.clone(), usd(r.usd), pct(r.spend_share), pct(r.request_share)];
            if !one {
                cells.push(pct(r.median_file));
                cells.push(r.files.to_string());
            }
            if by_file {
                cells.extend(r.by_file.iter().map(|x| x.map_or("–".to_string(), pct)));
            }
            rows.push(cells);
        }
        if let Some((n, sum)) = p.effort_more {
            rows.push(vec![String::new(), format!("{n} more"), usd(sum)]);
        }
        out.push('\n');
        out.push_str(&table(&rows, 2));
    }
    if let Some(t) = p.table("versions").filter(present) {
        section_head(&mut out, "Versions", &coverage(t.files_with_part, n), one);
        out.push('\n');
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

    /// The file with a rhythm part whose time at the keyboard is `hours`.
    fn with_keyboard(mut f: Value, hours: f64) -> Value {
        f["rhythm"] = json!({"keyboard_ms": hours * 3_600_000.0, "sittings": 10, "breaks": 9, "messages": 100, "sessions": 5,
            "ends": {"count": 100, "present": 60, "started_by_agent": 20}, "interrupts": 1, "typed_while_working": 0, "questions": 2,
            "answers": {"count": 10, "median_ms": 120_000, "by_pause": {"1–5m": 10}}, "question_answers": {"count": 2, "median_ms": 60_000, "by_pause": {"1–5m": 2}},
            "turns": {"count": 10, "median_ms": 180_000, "by_pause": {"1–5m": 10}}, "stood_still": {"count": 9, "median_ms": 300_000, "by_pause": {"5–15m": 9}},
            "worked_share_median": 0.4});
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
    fn the_activity_line_with_the_hours_at_the_keyboard() {
        let one = pool_of(vec![with_keyboard(friction_file(11_200.0, 141.0, 18.0, json!({})), 52.0)]);
        assert!(
            text(&one, false).contains(
                "activity  18 active days · 141 active hours · 52h at the keyboard · busy 365h · $79.43 per active hour · $215 per hour at the keyboard\n"
            ),
            "{}",
            text(&one, false)
        );
        let five = [
            (3_200.0, 40.0, 10.0, 20.0),
            (1_836.0, 100.0, 14.0, 30.0),
            (8_184.0, 120.0, 16.0, 50.0),
            (10_000.0, 200.0, 20.0, 70.0),
            (18_000.0, 240.0, 22.0, 80.0),
        ];
        let p = pool_of(five.iter().map(|(s, h, d, k)| with_keyboard(friction_file(*s, *h, *d, json!({})), *k)).collect());
        let line = text(&p, false).lines().find(|l| l.starts_with("activity")).unwrap().to_string();
        assert_eq!(
            line,
            "activity  700 active hours · 250h at the keyboard · $58.89 per active hour · $165 per hour at the keyboard · per file: median 16 active days, 120 active hours, $68.20 per active hour"
        );
    }

    #[test]
    fn savings_per_active_hour_and_cold_returns_by_client() {
        let misses = json!({
            "misses": {"idle > TTL": {"misses": 206, "tokens": 1, "avoidable_usd": 740.0}},
            "clients": {"sdk-ts": {"requests": 10, "usd": 1_000.0, "avoidable_usd": 890.0, "main_sessions": 9, "cold_returns": 206, "cold_returns_avoidable_usd": 740.0, "cache_writes": {"short": 0, "long": 0}, "active_hours": 312, "effort": {}}},
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
        // One sleep of the computer during a request would decide the longest one.
        assert!(!t.contains("longest request"), "{t}");
    }

    #[test]
    fn the_agents_time_and_the_hand_offs() {
        let extra = json!({
            "time": {"kinds": {"subagents": {"ms": 67.5 * 3_600_000.0, "after_turns_ms": 54.0 * 3_600_000.0}, "model": {"ms": 297.5 * 3_600_000.0, "after_turns_ms": 0}}, "requests": 10, "slow": 0, "slow_main": 0, "longest_ms": 1_000, "retried": 0},
            "handoffs": {"turn_ends": 900, "interrupts": 41, "typed_while_working": 3, "questions": 5, "answers": 5, "wakes": {"subagent": 1, "command": 2, "other": 0}},
            "pauses": {"typed_messages": 1_030},
        });
        let t = text(&pool_of(vec![with_keyboard(friction_file(1_000.0, 141.0, 18.0, extra), 50.0)]), false);
        let sub: Vec<&str> = t.lines().find(|l| l.starts_with("subagents")).unwrap().split_whitespace().collect();
        assert_eq!(sub[1..], ["67.5h", "18.5%", "54.0h"]);
        // Per hour at the keyboard, and per 100 typed messages.
        let interrupts: Vec<&str> = t.lines().find(|l| l.starts_with("interrupts")).unwrap().split_whitespace().collect();
        assert_eq!(interrupts[1..], ["41", "0.82", "3.98"]);
        // Ends of turn have no value per 100 typed messages: many come after wakes.
        let turns = json!({"handoffs": {"turn_ends": 2_671, "interrupts": 0, "typed_while_working": 0, "questions": 0, "answers": 0, "wakes": {"subagent": 0, "command": 0, "other": 0}}, "pauses": {"typed_messages": 1_163}});
        let t = text(&pool_of(vec![with_keyboard(friction_file(1_000.0, 395.0, 30.0, turns), 151.0)]), false);
        let section = &t[t.find("Working with the agent").unwrap()..];
        let ends: Vec<&str> = section.lines().find(|l| l.starts_with("ends of turn")).unwrap().split_whitespace().collect();
        assert_eq!(ends[3..], ["2,671", "17.69", "–"]);
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
        assert_eq!(effort[2..], ["$850", "94.1%", "90.0%"]);
        assert!(t.contains("prompts the logs hold: 88.2% of the spend; the rest is unknown\n"), "{t}");
        let git: Vec<&str> = t.lines().find(|l| l.starts_with("Bash git")).unwrap().split_whitespace().collect();
        assert_eq!(git[2..4], ["300", "120"]);
    }

    #[test]
    fn a_file_without_the_activity_part_counts_in_no_rate() {
        let client = json!({"requests": 10, "usd": 1_000.0, "avoidable_usd": 0.0, "main_sessions": 1, "cold_returns": 2, "cold_returns_avoidable_usd": 9.0, "cache_writes": {"short": 0, "long": 0}, "active_hours": 395,
            "effort": {"max": {"requests": 10, "usd": 1_000.0}}});
        let new = with_keyboard(friction_file(1_000.0, 395.0, 30.0, json!({"pauses": {"typed_messages": 1_168}, "clients": {"sdk-ts": client}})), 151.0);
        let mut old = file(1_000.0, None);
        old["agents"]["claude"]["pauses"] = json!({"typed_messages": 1_158});
        let p = pool_of(vec![new, old]);
        let typed = &p.working[0];
        assert_eq!((typed.value, typed.files_with_part), (1_168.0, 1));
        assert!((typed.rate.unwrap() - 1_168.0 / 151.0).abs() < 1e-9, "{typed:?}");
        let t = text(&p, false);
        let row: Vec<&str> = t.lines().find(|l| l.starts_with("typed messages")).unwrap().split_whitespace().collect();
        assert_eq!(row[..9], ["typed", "messages", "(in", "1", "of", "2", "files)", "1,168", "7.74"]);
        assert!(t.contains("\nCold returns by client, in 1 of 2 files\n"), "{t}");
        assert!(t.contains("\nHighest effort by client, in 1 of 2 files\n"), "{t}");
    }

    #[test]
    fn ten_clients_show_the_eight_with_the_most_active_hours() {
        let client = |hours: u64| json!({"requests": 10, "usd": 10.0, "avoidable_usd": 0.0, "main_sessions": 1, "cold_returns": 1, "cold_returns_avoidable_usd": 1.0, "cache_writes": {"short": 0, "long": 0}, "active_hours": hours, "effort": {}});
        let clients: serde_json::Map<String, Value> = (1..=10).map(|i| (format!("c{i:02}"), client(i * 10))).collect();
        let p = pool_of(vec![friction_file(1_000.0, 400.0, 30.0, json!({"clients": clients}))]);
        let names: Vec<&str> = p.cold_by_client.iter().map(|r| r.name.as_str()).collect();
        assert_eq!(names, ["c10", "c09", "c08", "c07", "c06", "c05", "c04", "c03", "2 more"]);
        let more = p.cold_by_client.last().unwrap();
        assert_eq!((more.agent.as_deref(), more.hours, more.value), (None, Some(30.0), 2.0));
    }

    #[test]
    fn small_clients_fold_into_more_costliest_first() {
        let client = |usd: f64, max: f64| {
            json!({"requests": 10, "usd": usd, "avoidable_usd": 0.0, "main_sessions": 1, "cold_returns": 0, "cache_writes": {"short": 0, "long": 0}, "active_hours": 1,
            "effort": {"max": {"requests": 9, "usd": usd * max}, "medium": {"requests": 1, "usd": usd * (1.0 - max)}}})
        };
        let extra = json!({"clients": {"sdk-ts": client(8_999.0, 0.8), "cli": client(421.0, 0.0), "JetBrains.IntelliJ IDEA": client(0.91, 0.982), "codex_exec": client(27.46, 0.27)}});
        let p = pool_of(vec![friction_file(11_356.0, 395.0, 30.0, extra)]);
        let names: Vec<&str> = p.effort_by_client.iter().map(|r| r.client.as_str()).collect();
        assert_eq!(names, ["sdk-ts", "cli"], "the costliest first, and only those with 1% of the spend or more");
        let (n, sum) = p.effort_more.unwrap();
        assert_eq!(n, 2);
        assert!((sum - 28.37).abs() < 1e-9, "{sum}");
        let t = text(&p, false);
        let section = &t[t.find("Highest effort by client").unwrap()..];
        assert!(section.lines().any(|l| l.split_whitespace().collect::<Vec<_>>() == ["2", "more", "$28.37"]), "{section}");
        assert!(!section.contains("JetBrains"), "{section}");
    }

    #[test]
    fn models_without_a_price_are_named_under_the_model_table() {
        let mut f = file(11_356.0, None);
        f["agents"]["codex"] =
            json!({"spend": {"codex-auto-review": {"requests": 1_947, "priced": false, "components": {}, "total": {"usd": 0.0, "tokens": 101_273_848}}}});
        let t = text(&pool_of(vec![f.clone()]), false);
        let models = &t[t.find("agent   model").unwrap()..t.find("cost component").unwrap()];
        assert!(models.ends_with("models without a price: codex-auto-review (101.3M tokens); their tokens count as $0\n\n"), "{models}");
        // In a pool, the model names its files, and --min-files leaves out one that fewer have.
        let pool = text(&pool_of(vec![f.clone(), file(1_000.0, None)]), false);
        assert!(pool.contains("models without a price: codex-auto-review (101.3M tokens, 1 file); their tokens count as $0\n"), "{pool}");
        let named = vec![("a".to_string(), f), ("b".to_string(), file(1_000.0, None))];
        assert!(!text(&pool::build(&named, 2), false).contains("models without a price"));
    }

    #[test]
    fn the_sections_come_in_their_order_and_format_1_files_have_none_of_the_new_ones() {
        let t = text(&pool_of(vec![rich_file()]), false);
        let at = |title: &str| t.find(&format!("\n{title}\n")).unwrap_or_else(|| panic!("{title}: {t}"));
        let order = [
            "At a glance",
            "Where the money went",
            "What could have been saved",
            "Cold returns by client",
            "Coming back after a cold break",
            "Problems",
            "How often it happens",
            "How the sessions spread",
            "Habits that cost the most",
            "Where the agent's time went",
            "Your time with the agents",
            "Working with the agent",
            "Working with the agent by client",
            "Commits and pull requests",
            "Tools and commands",
            "Highest effort by client",
            "Versions",
        ];
        let positions: Vec<usize> = order.iter().map(|o| at(o)).collect();
        assert!(positions.windows(2).all(|w| w[0] < w[1]), "{positions:?}");
        // In a pool, the pains come right after At a glance.
        let pool = text(&pool_of(vec![rich_file(), rich_file()]), false);
        let at = |title: &str| pool.find(&format!("\n{title}\n")).unwrap_or_else(|| panic!("{title}: {pool}"));
        assert!(at("At a glance") < at("Pains side by side") && at("Pains side by side") < at("Where the money went"), "{pool}");
        assert!(!t.contains("Pains side by side"), "one file has no pains: {t}");
        let mut old = file(1_000.0, None);
        old["format"] = json!(1);
        let t1 = text(&pool_of(vec![old]), false);
        for title in [
            "Problems",
            "Where the agent's time went",
            "Your time with the agents",
            "Working with the agent",
            "Coming back after a cold break",
            "Commits and pull requests",
            "Highest effort by client",
            "activity  ",
        ] {
            assert!(!t1.contains(title), "{title} in {t1}");
        }
        assert!(t1.contains("At a glance\n\nspend     $1,000\n"), "{t1}");
        // Format 2 without the parts of this change: no section built from them, and no hand-offs.
        let before = friction_file(
            1_000.0,
            10.0,
            2.0,
            json!({"handoffs": {"turn_ends": 3, "interrupts": 1, "typed_while_working": 0, "questions": 0, "answers": 0, "wakes": {"subagent": 0, "command": 0, "other": 0}}}),
        );
        let t2 = text(&pool_of(vec![before]), false);
        for title in [
            "Your time with the agents",
            "Coming back after a cold break",
            "Commits and pull requests",
            "Working with the agent",
            "How often it happens",
            "How the sessions spread",
        ] {
            assert!(!t2.contains(title), "{title} in {t2}");
        }
    }

    #[test]
    fn a_file_without_the_new_parts_counts_in_none_of_them() {
        let p = pool_of(vec![friction_file(1_000.0, 10.0, 2.0, json!({})), file(1_000.0, None)]);
        assert!(p.problems.iter().all(|r| r.files_with_part == 1), "{:?}", p.problems);
        assert_eq!(p.time_files, 1);
        let t = text(&p, false);
        assert!(t.contains("failed tool calls (in 1 of 2 files)") && t.contains("Where the agent's time went, in 1 of 2 files"), "{t}");
        assert!(t.lines().any(|l| l.starts_with("activity") && l.ends_with("(in 1 of 2 files)")), "{t}");
    }

    /// A file with every part this change adds, in two agents.
    fn rich_file() -> Value {
        let extra = json!({
            "misses": {"idle > TTL": {"misses": 207, "tokens": 1, "avoidable_usd": 812.0}, "after assistant message": {"misses": 90, "tokens": 1, "avoidable_usd": 405.0}},
            "failures": {"failed": 1_398, "refused": 0, "denied": 0, "after": {"usd": 227.0, "tokens": 1}},
            "time": {"kinds": {"API retries": {"ms": 23.3 * 3_600_000.0, "after_turns_ms": 0}}, "requests": 100, "slow": 0, "slow_main": 0, "longest_ms": 1, "retried": 92,
                "retried_by_error": {"server": 87, "rate_limit": 0, "auth": 0, "timeout": 5, "connection": 0, "sleep": 0, "other": 0}},
            "handoffs": {"turn_ends": 10, "interrupts": 1, "typed_while_working": 0, "questions": 0, "answers": 0, "wakes": {"subagent": 0, "command": 0, "other": 0}},
            "pauses": {"typed_messages": 50},
            "outcomes": {"commits": 410, "pushes": 95, "pull_requests": 12},
            "bench": bench(203, [1_644.0, 1_273.0, 518.0, 1_268.0, 809.0], 2),
            "clients": {
                "sdk-ts": {"requests": 10, "usd": 11_000.0, "avoidable_usd": 0.0, "main_sessions": 1, "cold_returns": 0, "cache_writes": {"short": 0, "long": 0}, "active_hours": 300, "effort": {"max": {"requests": 9, "usd": 10_000.0}, "medium": {"requests": 1, "usd": 1_000.0}},
                    "outcomes": {"commits": 300, "pushes": 90, "pull_requests": 12},
                    "rhythm": {"keyboard_ms": 120.0 * 3_600_000.0, "sittings": 150, "messages": 900, "interrupts": 30, "typed_while_working": 2, "questions": 60, "ends": {"count": 1_000, "present": 900}, "answers": {"count": 800, "median_ms": 210_000}, "question_answers": {"count": 60, "median_ms": 120_000}}},
                "cli": {"requests": 10, "usd": 356.0, "avoidable_usd": 0.0, "main_sessions": 1, "cold_returns": 0, "cache_writes": {"short": 0, "long": 0}, "active_hours": 40, "effort": {"medium": {"requests": 10, "usd": 356.0}},
                    "outcomes": {"commits": 40, "pushes": 5, "pull_requests": 0},
                    "rhythm": {"keyboard_ms": 20.0 * 3_600_000.0, "sittings": 30, "messages": 150, "interrupts": 2, "typed_while_working": 0, "questions": 4, "ends": {"count": 100, "present": 90}, "answers": {"count": 120, "median_ms": 72_000}, "question_answers": {"count": 4, "median_ms": 30_000}}},
            },
        });
        let mut f = friction_file(11_356.0, 395.0, 30.0, extra);
        f["rhythm"] = json!({"keyboard_ms": 151.0 * 3_600_000.0, "sittings": 190, "breaks": 180, "messages": 1_460, "sessions": 228,
            "ends": {"count": 2_420, "present": 2_013, "started_by_agent": 691}, "interrupts": 45, "typed_while_working": 35, "questions": 73,
            "answers": {"count": 100, "median_ms": 222_000, "by_pause": {"under 1m": 30, "1–5m": 32, "5–15m": 38}},
            "question_answers": {"count": 73, "median_ms": 138_000, "by_pause": {"1–5m": 73}},
            "turns": {"count": 1_425, "median_ms": 175_000, "by_pause": {"1–5m": 1_425}},
            "stood_still": {"count": 180, "median_ms": 480_000, "by_pause": {"5–15m": 180}},
            "worked_share_median": 0.42});
        f["agents"]["codex"] = json!({"bench": bench(18, [77.89, 55.82, 32.22, 90.29, 62.98], 2)});
        spread_parts(&mut f, 12.0, 0.5);
        f
    }

    /// Adds the prevalence and distribution parts: cold returns on `days` of the file's active
    /// days, and sessions whose median cost is `p50`.
    fn spread_parts(f: &mut Value, days: f64, p50: f64) {
        f["prevalence"] = json!({"days_with_cold_return": days, "days_with_retry_wait": 0, "days_with_interrupt": 3});
        let a = &mut f["agents"]["claude"];
        a["main_sessions"] = json!(100);
        a["prevalence"] = json!({"days_with_cold_return": days, "days_with_retry_wait": 0, "days_with_interrupt": 3, "sessions_with_cold_return": 20,
            "sessions_with_failure_window": 30, "sessions_over_400k": 10, "sessions_over_1d": 5, "sessions_with_question": 0});
        a["distributions"] = json!({"session_usd": {"n": 100, "p25": 0.01, "p50": p50, "p75": 15.0, "p90": 67.0},
            "cold_return_usd": {"n": 4}, "largest_context": {"n": 100, "p25": 24_000, "p50": 54_000, "p75": 362_000, "p90": 691_000}});
    }

    /// A bench part: the returns, each way's dollars, and the way cheapest in all of them.
    fn bench(returns: u64, usd: [f64; 5], cheapest: usize) -> Value {
        let keys = ["resume", "compact", "handoff", "heartbeats", "heartbeats_then_compaction"];
        let ways: serde_json::Map<String, Value> =
            keys.iter().enumerate().map(|(i, k)| (k.to_string(), json!({"usd": usd[i], "cheapest": if i == cheapest { returns } else { 0 }}))).collect();
        json!({"returns": returns, "requests": 60, "min_context": 100_000, "keep_warm_ms": 21_600_000, "ways": ways, "kept_warm": 1, "ping_usd": 1.0,
            "elsewhere": {"pings": 1, "ping_usd": 1.0, "compactions": 0, "compaction_usd": 0.0}})
    }

    #[test]
    fn at_a_glance_of_one_file() {
        let t = text(&pool_of(vec![rich_file()]), false);
        let glance = &t[t.find("At a glance").unwrap()..t.find("\nWhere the money went").unwrap()];
        let lines: Vec<&str> = glance.lines().skip(2).collect();
        assert_eq!(
            lines,
            [
                "spend     $11,356 · $28.75 per active hour · $75.21 per hour at the keyboard",
                "cache     $1,217 could have been saved (10.7%): idle > TTL $812, after assistant message $405",
                "returns   221 cold returns cost $1,722 as resumed; a handoff note would have cost $550 and was cheapest in 221; heartbeats $1,358",
                "problems  1,398 failed calls, $227 after them; 92 requests waited 23.3h for retries, 87 of them for server errors",
                "you       151h at the keyboard in 190 sittings; you answered in a median 3.7m; the agents stood still a median 8.0m after you left",
                "made      the agents made 410 commits, 95 pushes and 12 pull requests: $27.70 of spend per commit",
            ]
        );
        // Without the bench parts, there is no returns line.
        let mut plain = rich_file();
        plain["agents"]["claude"].as_object_mut().unwrap().remove("bench");
        plain["agents"].as_object_mut().unwrap().remove("codex");
        assert!(!text(&pool_of(vec![plain]), false).contains("\nreturns   "));
    }

    #[test]
    fn a_pool_with_an_older_file_says_in_how_many_files_each_line_is() {
        let mut older =
            friction_file(11_356.0, 395.0, 30.0, json!({"time": {"kinds": {}, "requests": 100, "slow": 0, "slow_main": 0, "longest_ms": 1, "retried": 90}}));
        older["agents"]["claude"].as_object_mut().unwrap().remove("outcomes");
        let t = text(&pool_of(vec![rich_file(), older]), false);
        let line = |label: &str| t.lines().find(|l| l.starts_with(label)).unwrap_or_else(|| panic!("{label}: {t}")).to_string();
        assert!(line("returns   ").ends_with(" (in 1 of 2 files)"), "{}", line("returns   "));
        assert!(line("you       ").ends_with(" (in 1 of 2 files)"), "{}", line("you       "));
        assert!(line("made      ").ends_with(" (in 1 of 2 files)"), "{}", line("made      "));
        assert!(
            line("problems  ").contains("182 requests waited") && line("problems  ").ends_with("87 of the 92 with a class for server errors"),
            "{}",
            line("problems  ")
        );
        assert!(line("activity  ").contains("151h at the keyboard (in 1 of 2 files)"), "{}", line("activity  "));
        assert!(t.contains("\nWorking with the agent by client, in 1 of 2 files\n"), "{t}");
    }

    #[test]
    fn at_a_glance_of_a_pool() {
        let five = [(3_200.0, 40.0, 10.0), (1_836.0, 100.0, 14.0), (8_184.0, 120.0, 16.0), (10_000.0, 200.0, 20.0), (18_000.0, 240.0, 22.0)];
        let p = pool_of(five.iter().map(|(s, h, d)| friction_file(*s, *h, *d, json!({}))).collect());
        let t = text(&p, false);
        assert!(t.contains("\nspend     $41,220 · $58.89 per active hour (median file $68.20)\n"), "{t}");
    }

    #[test]
    fn the_rows_of_your_time_with_the_agents() {
        let p = pool_of(vec![rich_file()]);
        let names: Vec<&str> = p.rhythm.iter().map(|r| r.name.as_str()).collect();
        assert_eq!(
            names,
            [
                "time at the keyboard",
                "sittings",
                "you answered an end of turn in",
                "… within 5 minutes",
                "you answered a question in",
                "… within 5 minutes",
                "the agent's turn took",
                "ends of turn while you were there",
                "turns the agent started itself",
                "the agents stood still after you left",
                "the agents worked through a break",
            ]
        );
        let t = text(&p, false);
        let value =
            |name: &str| t.lines().find(|l| l.starts_with(name)).unwrap_or_else(|| panic!("{name}: {t}")).split_whitespace().last().unwrap().to_string();
        assert_eq!(
            [value("time at the keyboard"), value("sittings"), value("you answered an end of turn in"), value("… within 5 minutes")],
            ["151.0h", "190", "3.7m", "62%"]
        );
        assert_eq!(value("ends of turn while you were there"), "13.33", "2,013 ends in 151 hours at the keyboard");
    }

    #[test]
    fn a_pooled_median_is_a_pause_range() {
        let mut a = rich_file();
        let mut b = rich_file();
        a["rhythm"]["answers"] = json!({"count": 3, "median_ms": 30_000, "by_pause": {"under 1m": 3}});
        b["rhythm"]["answers"] = json!({"count": 5, "median_ms": 240_000, "by_pause": {"1–5m": 5}});
        let p = pool_of(vec![a, b]);
        let row = p.rhythm.iter().find(|r| r.name == "you answered an end of turn in").unwrap();
        assert_eq!(row.range.as_deref(), Some("1–5m"), "the 5th of 8 answers falls in 1–5m");
        assert_eq!(row.median_file, Some(240_000.0), "the upper of the two files' medians");
    }

    #[test]
    fn coming_back_after_a_cold_break() {
        let t = text(&pool_of(vec![rich_file()]), false);
        let row: Vec<&str> = t.lines().find(|l| l.starts_with("new session + handoff note")).unwrap().split_whitespace().collect();
        assert_eq!(row[5..], ["$550", "32.0%", "221", "of", "221"]);
        assert!(!text(&pool_of(vec![file(100.0, None)]), false).contains("Coming back after a cold break"), "no returns, no section");
    }

    #[test]
    fn retries_by_error_class() {
        let t = text(&pool_of(vec![rich_file()]), false);
        let rows: Vec<&str> = t.lines().filter(|l| l.starts_with("  ")).collect();
        assert!(rows.iter().any(|l| l.split_whitespace().collect::<Vec<_>>()[..3] == ["server", "errors", "87"]), "{t}");
        assert!(rows.iter().any(|l| l.split_whitespace().collect::<Vec<_>>()[..2] == ["timeouts", "5"]), "{t}");
        assert!(!t.contains("  rate limits") && !t.contains("  computer asleep"), "classes without requests have no row");
    }

    #[test]
    fn working_with_the_agent_by_client() {
        let t = text(&pool_of(vec![rich_file()]), false);
        let section = &t[t.find("Working with the agent by client").unwrap()..];
        let ide: Vec<&str> = section.lines().find(|l| l.starts_with("claude  sdk-ts")).unwrap().split_whitespace().collect();
        assert_eq!(ide[2..], ["120.0h", "7.50", "0.25", "0.50", "2.50", "3.5m"]);
        let term: Vec<&str> = section.lines().find(|l| l.starts_with("claude  cli")).unwrap().split_whitespace().collect();
        assert_eq!(term[2..], ["20.0h", "7.50", "0.10", "0.20", "2.00", "1.2m"]);
        // Ten clients: the eight with the most time at the keyboard, then the rest.
        let client = |h: f64| {
            json!({"requests": 1, "usd": 1.0, "avoidable_usd": 0.0, "main_sessions": 1, "cold_returns": 0, "cache_writes": {"short": 0, "long": 0}, "active_hours": 1, "effort": {},
            "rhythm": {"keyboard_ms": h * 3_600_000.0, "sittings": 1, "messages": 1, "interrupts": 0, "typed_while_working": 0, "questions": 0, "ends": {"count": 1, "present": 1}, "answers": {"count": 0, "median_ms": null}, "question_answers": {"count": 0, "median_ms": null}}})
        };
        let clients: serde_json::Map<String, Value> = (1..=10).map(|i| (format!("c{i:02}"), client(i as f64))).collect();
        let p = pool_of(vec![friction_file(1_000.0, 100.0, 10.0, json!({"clients": clients}))]);
        assert_eq!(p.client_work.last().map(|w| w.client.as_str()), Some("2 more"));
        assert_eq!(p.client_work.len(), 9);
        // A client of minutes, under 1% of the time at the keyboard, has no row of its own.
        let mut few = serde_json::Map::new();
        few.insert("sdk-ts".into(), client(100.0));
        few.insert("codex-tui".into(), client(0.5));
        let p = pool_of(vec![friction_file(1_000.0, 100.0, 10.0, json!({"clients": few}))]);
        let names: Vec<&str> = p.client_work.iter().map(|w| w.client.as_str()).collect();
        assert_eq!(names, ["sdk-ts", "1 more"]);
    }

    #[test]
    fn commits_and_pull_requests() {
        let t = text(&pool_of(vec![rich_file()]), false);
        let section = &t[t.find("Commits and pull requests").unwrap()..];
        let commits: Vec<&str> = section.lines().find(|l| l.starts_with("commits")).unwrap().split_whitespace().collect();
        assert_eq!(commits[1..], ["410", "2.72"]);
        assert!(section.contains("\n$27.70 of spend per commit\n"), "{section}");
        let none = with_keyboard(friction_file(1_000.0, 10.0, 2.0, json!({"outcomes": {"commits": 0, "pushes": 0, "pull_requests": 0}})), 5.0);
        let t = text(&pool_of(vec![none]), false);
        assert!(t.contains("\nno commits by the agents\n") && t.contains("made      no commits by the agents"), "{t}");
    }

    #[test]
    fn every_section_has_its_note() {
        let t = text(&pool_of(vec![rich_file()]), false);
        // A pain touches people, so its section is in a pool only.
        for (title, note) in NOTES.iter().filter(|(t, _)| *t != "Pains side by side") {
            let at = t.find(&format!("\n{title}\n")).unwrap_or_else(|| panic!("{title}: {t}"));
            let next = t[at + title.len() + 2..].lines().next().unwrap();
            assert_eq!(next, *note, "{title}");
        }
        let pool = text(&pool_of(vec![rich_file(), rich_file()]), false);
        assert!(pool.contains(&format!("{}; median file: the median of each file's own share\n", NOTES[1].1)), "{pool}");
        assert!(pool.contains(&format!("{}; files 5%+: the files where a row is 5% of their spend or more\n", NOTES[2].1)), "{pool}");
        assert!(pool.contains(&format!("\nPains side by side\n{}\n", NOTES[0].1)), "{pool}");
    }

    #[test]
    fn counts_have_thousands_separators() {
        let mut f = file(1_000.0, None);
        f["agents"]["claude"]["tools"] =
            json!({"Read": {"calls": 15_080, "failed": 1_026, "rent": {"usd": 30.0, "tokens": 1}, "trips": {"usd": 3.0, "tokens": 1}}});
        f["agents"]["claude"]["versions"] = json!({"2.1.316": {"requests": 16_678, "usd": 1.0, "held": 1}});
        let t = text(&pool_of(vec![f]), false);
        let read: Vec<&str> = t.lines().find(|l| l.starts_with("Read")).unwrap().split_whitespace().collect();
        assert_eq!(read[1..3], ["15,080", "1,026"]);
        assert!(t.lines().any(|l| l.contains("2.1.316") && l.trim_end().ends_with("16,678")), "{t}");
    }

    #[test]
    fn by_file_adds_a_column_per_file() {
        let p = pool_of(vec![file(100.0, None), file(200.0, None), file(300.0, None)]);
        let text = text(&p, true);
        let head = text.lines().find(|l| l.starts_with("agent   model") || l.starts_with("agent  model")).unwrap_or_else(|| panic!("{text}"));
        assert!(head.trim_end().ends_with("f0     f1     f2") || head.split_whitespace().rev().take(3).collect::<Vec<_>>() == ["f2", "f1", "f0"], "{head}");
    }

    #[test]
    fn pains_side_by_side_in_a_pool() {
        // Cold returns on 12, 12 and 3 of 30 days, and a file from before the prevalence parts.
        let files: Vec<Value> = [12.0, 12.0, 3.0]
            .iter()
            .map(|d| {
                let mut f = rich_file();
                spread_parts(&mut f, *d, 1.0);
                f
            })
            .chain([friction_file(1_000.0, 100.0, 30.0, json!({}))])
            .collect();
        let p = pool_of(files);
        let t = text(&p, false);
        let section = &t[t.find("\nPains side by side\n").unwrap()..t.find("\nWhere the money went").unwrap()];
        assert!(section.contains("\npain  "), "{section}");
        let row =
            |name: &str| section.lines().find(|l| l.starts_with(name)).unwrap_or_else(|| panic!("{name}: {section}")).split_whitespace().collect::<Vec<_>>();
        // 40% of the days in two files, 10% in the third; the fourth file has no prevalence part.
        let idle = p.table("saved").unwrap().rows.iter().find(|r| r.name == "idle > TTL").unwrap();
        assert!(row("cold returns (in 3 of 4 files)").ends_with(&["2", "of", "3", "avoidable", "$2.06", &usd(idle.usd)]), "{section}");
        let retries = p.problems.iter().find(|r| r.name == "time waiting for retries").unwrap();
        assert!(row("waits for retries").ends_with(&["time", &span(retries.median_rate.unwrap()), &span(retries.value)]), "{section}");
        assert!(row("interrupts").ends_with(&["–", "–", "–"]), "{section}");
        assert!(row("failures").ends_with(&["3", "of", "3", "spent", "$0.57", "$681"]), "$227 after failures in each of 3 files: {section}");
        // No file has the effort dollars, so the pain has no row.
        assert!(!section.contains("effort max or xhigh"), "{section}");
    }

    #[test]
    fn how_often_it_happens_in_one_file() {
        let t = text(&pool_of(vec![rich_file()]), false);
        let section = &t[t.find("\nHow often it happens\n").unwrap()..t.find("\nHow the sessions spread").unwrap()];
        assert!(section.contains("\nthe active days and main sessions in which each happened at least once\n"), "{section}");
        let row = |name: &str| {
            section.lines().find(|l| l.starts_with(name)).unwrap_or_else(|| panic!("{name}: {section}")).split_whitespace().rev().take(4).collect::<Vec<_>>()
        };
        assert_eq!(row("days with a cold return"), ["40.0%", "days", "30", "12"]);
        assert_eq!(row("sessions with a cold return"), ["20.0%", "sessions", "100", "20"]);
        assert!(!section.contains("files 25%+"), "{section}");
    }

    #[test]
    fn how_often_it_happens_in_a_pool() {
        // Cold returns on 15, 0, 6 and 30 of 30 days, and a file from before this change.
        let mut files: Vec<Value> = [15.0, 0.0, 6.0, 30.0]
            .iter()
            .map(|d| {
                let mut f = friction_file(1_000.0, 100.0, 30.0, json!({}));
                spread_parts(&mut f, *d, 1.0);
                f
            })
            .collect();
        files.push(friction_file(1_000.0, 100.0, 30.0, json!({})));
        let p = pool_of(files);
        let r = p.prevalence.iter().find(|r| r.key == "days_with_cold_return").unwrap();
        assert_eq!((r.count, r.base, r.files, r.common_files, r.files_with_part), (51.0, 120.0, 3, 2, 4));
        assert_eq!(r.median_file, Some(0.5), "the upper of 20% and 50%");
        let t = text(&p, false);
        assert!(t.contains("\nHow often it happens, in 4 of 5 files\n"), "{t}");
        assert!(t.contains("; files: where it happened at all; files 25%+: in a quarter of their days or sessions or more\n"), "{t}");
        let line = t.lines().find(|l| l.starts_with("days with a cold return")).unwrap();
        assert!(line.split_whitespace().collect::<Vec<_>>().ends_with(&["51", "120", "days", "42.5%", "50.0%", "3", "(75%)", "2", "(50%)"]), "{line}");
    }

    #[test]
    fn the_session_rows_leave_out_the_sessions_nobody_typed_in() {
        let mut f = rich_file();
        f["agents"]["claude"]["prevalence"]["left_out_sessions"] = json!(60);
        f["agents"]["claude"]["main_sessions"] = json!(160);
        let p = pool_of(vec![f]);
        let r = p.prevalence.iter().find(|r| r.key == "sessions_with_cold_return").unwrap();
        assert_eq!((r.count, r.base), (20.0, 100.0), "the base is the sessions someone worked in");
        let t = text(&p, false);
        let line = "the session rows leave out 60 sessions that nobody typed in: a program ran them, or they ran in a temporary folder\n";
        assert_eq!(t.matches(line).count(), 2, "{t}");
        assert!(!text(&pool_of(vec![rich_file()]), false).contains("leave out"), "none left out, no line");
    }

    #[test]
    fn how_the_sessions_spread_in_one_file_and_in_a_pool() {
        let t = text(&pool_of(vec![rich_file()]), false);
        let section = &t[t.find("\nHow the sessions spread\n").unwrap()..t.find("\nHabits that cost the most").unwrap()];
        assert!(section.contains("agent   measure                       values    p25    p50     p75     p90\n"), "{section}");
        assert!(section.contains("claude  cost of a session                100  $0.01  $0.50  $15.00  $67.00\n"), "{section}");
        assert!(section.contains("claude  largest context of a session     100  24.0k  54.0k  362.0k  691.0k\n"), "{section}");
        assert!(!section.contains("first request after a cold return"), "4 returns give no quantiles: {section}");
        // Five files whose median session cost $1 to $5, and one with too few sessions.
        let mut files: Vec<Value> = (1..=5)
            .map(|k| {
                let mut f = friction_file(1_000.0, 100.0, 30.0, json!({}));
                spread_parts(&mut f, 3.0, k as f64);
                f
            })
            .collect();
        let mut few = friction_file(1_000.0, 100.0, 30.0, json!({}));
        spread_parts(&mut few, 3.0, 1.0);
        few["agents"]["claude"]["distributions"]["session_usd"] = json!({"n": 3});
        files.push(few);
        let p = pool_of(files.clone());
        let r = p.spread.iter().find(|r| r.key == "session_usd").unwrap();
        assert_eq!((r.median_p50, r.quarter_1_p50, r.quarter_3_p50, r.median_p90, r.files, r.files_with_part, r.n), (3.0, 2.0, 4.0, 67.0, 5, 6, 503.0));
        let t = text(&p, false);
        assert!(t.contains("; p50 and q1–q3: the median and quarters of the files' own p50; p90: the median of their p90\n"), "{t}");
        let line = t.lines().find(|l| l.starts_with("claude  cost of a session")).unwrap();
        assert!(line.split_whitespace().collect::<Vec<_>>().ends_with(&["503", "$3.00", "$2.00–$4.00", "$67.00", "5", "of", "6"]), "{line}");
        // With --min-files 6, no row has quantiles from 6 files, and the section is left out.
        let named: Vec<(String, Value)> = files.into_iter().enumerate().map(|(i, f)| (format!("f{i}"), f)).collect();
        let p = pool::build(&named, 6);
        assert!(p.spread.iter().all(|r| r.key != "session_usd") && p.spread.iter().any(|r| r.key == "largest_context"), "{:?}", p.spread);
    }
}
