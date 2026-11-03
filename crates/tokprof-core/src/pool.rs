//! `tokprof pool`: the share files of several people in one report. Where their money went, what
//! could have been saved, which habits cost the most, which tools carry the spend, and which
//! agent versions ran, pooled and for the median file, without showing any one person's work.
//!
//! The report reads only the files. `tokprof share` gives it the one file it wrote, so the person
//! who runs it sees what the file holds. A part that a file lacks, such as `content` in a format-1
//! file, is missing rather than zero: the rows built from it count only the files that have it.

use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;
use serde_json::Value;

use crate::share::{FORMAT, PROMPT_RANGES, SPAN_RANGES};

/// A row is a habit or a saving of a file when it is at least this share of the file's spend.
pub const NOTABLE_SHARE: f64 = 0.05;

/// The costliest rows a table shows before it folds the rest into `<n> more`.
pub const TOP_ROWS: usize = 8;

/// The tools and programs the tool table shows.
pub const TOP_TOOLS: usize = 12;

/// The versions of each agent the version table shows.
pub const TOP_VERSIONS: usize = 5;

/// Why a file is left out of a pool.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Skip {
    NotShare,
    Newer(u64),
}

/// The share file a text holds, when this tokprof reads it.
pub fn parse(text: &str) -> Result<Value, Skip> {
    let v: Value = serde_json::from_str(text).map_err(|_| Skip::NotShare)?;
    let format = v.get("format").and_then(Value::as_u64).ok_or(Skip::NotShare)?;
    if !v.get("agents").is_some_and(Value::is_object) {
        return Err(Skip::NotShare);
    }
    if format > FORMAT {
        return Err(Skip::Newer(format));
    }
    Ok(v)
}

/// What the header says of one file.
#[derive(Clone, Debug, Serialize)]
pub struct FileInfo {
    /// The name it was given by, without `.json`.
    pub name: String,
    pub format: u64,
    pub spend_usd: f64,
    pub survey: Option<String>,
    pub first_day: Option<String>,
    pub last_day: Option<String>,
    pub agents: Vec<String>,
}

/// The section a table belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Section {
    /// Where the money went.
    Spend,
    /// What could have been saved.
    Saved,
    /// Habits that cost the most.
    Habits,
    Tools,
    Versions,
}

/// One row of a table, pooled over the files.
#[derive(Clone, Debug, Default, Serialize)]
pub struct Row {
    /// The agent, for the tables that name one.
    pub agent: Option<String>,
    pub name: String,
    /// A name under the row before it, such as an origin's name under its kind.
    pub under: bool,
    pub usd: f64,
    /// Of the pooled spend of the files that have the part.
    pub share: f64,
    /// The median over those files of the row's share of each file's spend.
    pub median_file: f64,
    /// The files that have the row.
    pub files: usize,
    /// The files where the row is at least [`NOTABLE_SHARE`] of their spend.
    pub notable_files: usize,
    /// The files that have the part the row is built from.
    pub files_with_part: usize,
    /// The row's share of each file's spend, in file order; `None` for a file without the part.
    pub by_file: Vec<Option<f64>>,
    /// Tool calls, for the tool table.
    pub calls: u64,
    /// Trips in dollars, for the tool table, where `usd` is the rent.
    pub trips_usd: f64,
    /// Requests, for the version table.
    pub requests: u64,
}

/// A table of the report.
#[derive(Clone, Debug, Serialize)]
pub struct Table {
    pub key: &'static str,
    pub section: Section,
    /// The files that have the part the table is built from.
    pub files_with_part: usize,
    pub rows: Vec<Row>,
    /// The rows past the top ones, folded: how many and their sum.
    pub more: Option<(usize, f64)>,
}

/// The pooled report.
#[derive(Clone, Debug, Serialize)]
pub struct Pool {
    pub files: Vec<FileInfo>,
    pub spend_usd: f64,
    /// The spend per file at the median and at one and three quarters.
    pub median_usd: f64,
    pub quarter_1_usd: f64,
    pub quarter_3_usd: f64,
    pub tables: Vec<Table>,
}

impl Pool {
    pub fn table(&self, key: &str) -> Option<&Table> {
        self.tables.iter().find(|t| t.key == key)
    }
}

/// The value at a quarter position of a sorted list, as the pause report takes them: the
/// position rounded down, so a median takes the upper of two middle values.
fn at_quarter(sorted: &[f64], q: usize) -> f64 {
    sorted.get(sorted.len() * q / 4).copied().unwrap_or(0.0)
}

fn usd_at(v: &Value, path: &[&str]) -> f64 {
    path.iter().try_fold(v, |v, k| v.get(*k)).and_then(Value::as_f64).unwrap_or(0.0)
}

fn agents(file: &Value) -> impl Iterator<Item = (&String, &Value)> {
    file.get("agents").and_then(Value::as_object).into_iter().flatten()
}

/// A file's spend: the totals of its models, every agent.
fn spend_of(file: &Value) -> f64 {
    agents(file).flat_map(|(_, a)| a.get("spend").and_then(Value::as_object).into_iter().flatten()).map(|(_, m)| usd_at(m, &["total", "usd"])).sum()
}

/// One item of a file's part: its row key (agent and name), whether it sits under the row
/// before, and its numbers.
#[derive(Clone, Debug, Default)]
struct Item {
    agent: Option<String>,
    name: String,
    under: bool,
    usd: f64,
    calls: u64,
    trips_usd: f64,
    requests: u64,
}

impl Item {
    fn named(name: impl Into<String>, usd: f64) -> Item {
        Item { name: name.into(), usd, ..Item::default() }
    }
}

/// The rows of a part, summed over agents unless the item names its agent. `None` when the file
/// lacks the part: every agent object lacks it.
fn part(file: &Value, key: &str, each: impl Fn(&str, &Value, &mut Vec<Item>)) -> Option<Vec<Item>> {
    let mut items = Vec::new();
    let mut found = false;
    for (agent, a) in agents(file) {
        if let Some(p) = a.get(key) {
            found = true;
            each(agent, p, &mut items);
        }
    }
    found.then_some(items)
}

/// The entries of an object, as (key, value).
fn entries(v: &Value) -> impl Iterator<Item = (&String, &Value)> {
    v.as_object().into_iter().flatten()
}

/// How a table's rows are put in order.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Order {
    /// The costliest first, then `<n> more`.
    Cost(usize),
    /// In the order of these names, all of them.
    Fixed(&'static [&'static str]),
    /// The kinds the costliest first, each with its names under it, the costliest first.
    Kinds,
    /// Each agent's rows with the most requests, then `<n> more` per agent.
    PerAgent(usize),
}

/// The pooled rows of one part over all files.
fn pooled(items: &[Option<Vec<Item>>], spends: &[f64]) -> (Vec<Row>, usize) {
    let with_part = items.iter().filter(|i| i.is_some()).count();
    let part_spend: f64 = items.iter().zip(spends).filter(|(i, _)| i.is_some()).map(|(_, s)| s).sum();
    // Rows by key, in the order they are first met.
    let mut keys: Vec<(Option<String>, String, bool)> = Vec::new();
    let mut index: BTreeMap<(Option<String>, String), usize> = BTreeMap::new();
    let mut values: Vec<Vec<Option<Item>>> = Vec::new();
    for (f, file_items) in items.iter().enumerate() {
        let Some(file_items) = file_items else { continue };
        for it in file_items {
            let key = (it.agent.clone(), it.name.clone());
            let i = *index.entry(key).or_insert_with(|| {
                keys.push((it.agent.clone(), it.name.clone(), it.under));
                values.push(vec![None; items.len()]);
                keys.len() - 1
            });
            let slot = values[i][f].get_or_insert_with(Item::default);
            slot.usd += it.usd;
            slot.calls += it.calls;
            slot.trips_usd += it.trips_usd;
            slot.requests += it.requests;
        }
    }
    let rows = keys
        .into_iter()
        .zip(values)
        .map(|((agent, name, under), per_file)| {
            let by_file: Vec<Option<f64>> = per_file
                .iter()
                .zip(items)
                .zip(spends)
                .map(|((v, part), spend)| part.as_ref().map(|_| v.as_ref().map_or(0.0, |v| v.usd) / spend.max(1e-9)))
                .collect();
            let mut shares: Vec<f64> = by_file.iter().flatten().copied().collect();
            shares.sort_by(f64::total_cmp);
            let usd: f64 = per_file.iter().flatten().map(|v| v.usd).sum();
            Row {
                agent,
                name,
                under,
                usd,
                share: usd / part_spend.max(1e-9),
                median_file: at_quarter(&shares, 2),
                files: per_file.iter().flatten().filter(|v| v.usd > 0.0 || v.calls > 0 || v.requests > 0).count(),
                notable_files: by_file.iter().flatten().filter(|s| **s >= NOTABLE_SHARE).count(),
                files_with_part: with_part,
                by_file,
                calls: per_file.iter().flatten().map(|v| v.calls).sum(),
                trips_usd: per_file.iter().flatten().map(|v| v.trips_usd).sum(),
                requests: per_file.iter().flatten().map(|v| v.requests).sum(),
            }
        })
        .collect();
    (rows, with_part)
}

/// Folds the rows that fewer than `min_files` files have: flat rows into one `other` row, names
/// under a kind and versions out.
fn fold_rare(rows: Vec<Row>, min_files: usize, section: Section) -> Vec<Row> {
    if min_files <= 1 {
        return rows;
    }
    let mut kept = Vec::new();
    let mut other: Option<Row> = None;
    for r in rows {
        if r.files >= min_files {
            kept.push(r);
        } else if r.under || section == Section::Versions {
            // Left out; a name's spend stays in its kind's row.
        } else {
            let o = other.get_or_insert_with(|| Row { name: "other".into(), files_with_part: r.files_with_part, by_file: vec![Some(0.0); r.by_file.len()], ..Row::default() });
            o.usd += r.usd;
            o.share += r.share;
            o.calls += r.calls;
            o.trips_usd += r.trips_usd;
            o.files = o.files.max(r.files);
            for (a, b) in o.by_file.iter_mut().zip(&r.by_file) {
                *a = match (*a, *b) {
                    (Some(x), Some(y)) => Some(x + y),
                    (_, b) => b,
                };
            }
        }
    }
    if let Some(mut o) = other {
        let mut shares: Vec<f64> = o.by_file.iter().flatten().copied().collect();
        shares.sort_by(f64::total_cmp);
        o.median_file = at_quarter(&shares, 2);
        o.notable_files = o.by_file.iter().flatten().filter(|s| **s >= NOTABLE_SHARE).count();
        // An `other` row already there takes the folded rows in.
        match kept.iter_mut().find(|r| r.name == "other" && !r.under && r.agent.is_none()) {
            Some(existing) => {
                existing.usd += o.usd;
                existing.share += o.share;
            }
            None => kept.push(o),
        }
    }
    kept
}

/// Puts a table's rows in order and folds the ones past the top.
fn order(mut rows: Vec<Row>, how: Order) -> (Vec<Row>, Option<(usize, f64)>) {
    let by_cost = |a: &Row, b: &Row| b.usd.total_cmp(&a.usd).then_with(|| a.name.cmp(&b.name));
    match how {
        Order::Cost(top) => {
            rows.sort_by(by_cost);
            if rows.len() > top {
                let rest = rows.split_off(top);
                let sum = rest.iter().map(|r| r.usd).sum();
                (rows, Some((rest.len(), sum)))
            } else {
                (rows, None)
            }
        }
        Order::Fixed(names) => {
            rows.sort_by_key(|r| names.iter().position(|n| *n == r.name).unwrap_or(usize::MAX));
            (rows, None)
        }
        Order::Kinds => {
            let (mut kinds, names): (Vec<Row>, Vec<Row>) = rows.into_iter().partition(|r| !r.under);
            kinds.sort_by(by_cost);
            let mut out = Vec::new();
            for k in kinds {
                let prefix = format!("{}:", k.name);
                let mut under: Vec<Row> = names.iter().filter(|n| n.name.starts_with(&prefix)).cloned().collect();
                under.sort_by(by_cost);
                out.push(k);
                out.extend(under);
            }
            (out, None)
        }
        Order::PerAgent(top) => {
            let mut by_agent: BTreeMap<Option<String>, Vec<Row>> = BTreeMap::new();
            for r in rows {
                by_agent.entry(r.agent.clone()).or_default().push(r);
            }
            let mut out = Vec::new();
            for (agent, mut rs) in by_agent {
                rs.sort_by(|a, b| b.requests.cmp(&a.requests).then_with(|| a.name.cmp(&b.name)));
                if rs.len() > top {
                    let rest = rs.split_off(top);
                    let requests = rest.iter().map(|r| r.requests).sum();
                    out.extend(rs);
                    out.push(Row { agent: agent.clone(), name: format!("{} more", rest.len()), requests, ..Row::default() });
                } else {
                    out.extend(rs);
                }
            }
            (out, None)
        }
    }
}

fn table(key: &'static str, section: Section, items: Vec<Option<Vec<Item>>>, spends: &[f64], how: Order, min_files: usize) -> Table {
    let (rows, with_part) = pooled(&items, spends);
    let rows = fold_rare(rows, min_files, section);
    let (rows, more) = order(rows, how);
    Table { key, section, files_with_part: with_part, rows, more }
}

/// A table with one row per habit or saving, each from its own part: a file without the part of
/// a row counts only in the other rows.
fn rows_table(key: &'static str, section: Section, files: &[Value], spends: &[f64], rows: &[(&str, RowValue)]) -> Table {
    let mut out = Vec::new();
    for (name, value) in rows {
        let items: Vec<Option<Vec<Item>>> = files.iter().map(|f| value(f).map(|usd| vec![Item::named(*name, usd)])).collect();
        let (mut r, _) = pooled(&items, spends);
        if let Some(r) = r.pop() {
            out.push(r);
        }
    }
    out.sort_by(|a, b| b.usd.total_cmp(&a.usd).then_with(|| a.name.cmp(&b.name)));
    let with_part = files.len();
    Table { key, section, files_with_part: with_part, rows: out, more: None }
}

/// What one row of a habit or saving table takes from a file: its dollars, or `None` when the
/// file lacks the part.
type RowValue<'a> = &'a dyn Fn(&Value) -> Option<f64>;

/// The ranges of an agent part with `main` and `subagents` halves, summed.
fn halves(p: &Value, out: &mut Vec<Item>) {
    for half in ["main", "subagents"] {
        for (range, r) in entries(&p[half]) {
            out.push(Item::named(range.clone(), usd_at(r, &["usd"])));
        }
    }
}

/// The dollars of a part's ranges, summed over agents and halves, or `None` without the part.
fn ranges_usd(file: &Value, key: &str, names: &[&str], halved: bool) -> Option<f64> {
    part(file, key, |_, p, out| {
        if halved {
            halves(p, out);
        } else {
            out.extend(entries(p).map(|(n, r)| Item::named(n.clone(), usd_at(r, &["usd"]))));
        }
    })
    .map(|items| items.iter().filter(|i| names.contains(&i.name.as_str())).map(|i| i.usd).sum())
}

/// What the what-if replay says could have been saved in one bucket: the logged input cost minus
/// the cheaper of the replays, when that is positive.
fn lifetime_saving(file: &Value, bucket: &str) -> Option<f64> {
    part(file, "whatif", |_, w, out| {
        let b = &w[bucket];
        let actual = usd_at(b, &["actual_usd"]);
        let cheapest = entries(b).filter(|(k, _)| k.starts_with("simulated_")).filter_map(|(_, v)| v.as_f64()).fold(f64::INFINITY, f64::min);
        if cheapest.is_finite() {
            out.push(Item::named("", (actual - cheapest).max(0.0)));
        }
    })
    .map(|items| items.iter().map(|i| i.usd).sum())
}

/// The report over these files, given as (name, file). Rows that fewer than `min_files` files
/// have are folded or left out (see [`fold_rare`]).
pub fn build(named: &[(String, Value)], min_files: usize) -> Pool {
    let files: Vec<Value> = named.iter().map(|(_, v)| v.clone()).collect();
    let spends: Vec<f64> = files.iter().map(spend_of).collect();
    let infos = named
        .iter()
        .zip(&spends)
        .map(|((name, v), spend)| FileInfo {
            name: name.clone(),
            format: v.get("format").and_then(Value::as_u64).unwrap_or(0),
            spend_usd: *spend,
            survey: v.get("survey").and_then(Value::as_str).map(str::to_string),
            first_day: v.pointer("/window/first").and_then(Value::as_str).map(str::to_string),
            last_day: v.pointer("/window/last").and_then(Value::as_str).map(str::to_string),
            agents: agents(v).map(|(a, _)| a.clone()).collect(),
        })
        .collect();
    let mut sorted = spends.clone();
    sorted.sort_by(f64::total_cmp);
    let each = |key: &str, f: &dyn Fn(&str, &Value, &mut Vec<Item>)| -> Vec<Option<Vec<Item>>> { files.iter().map(|file| part(file, key, f)).collect() };
    let mut tables = Vec::new();

    // Where the money went.
    tables.push(table(
        "model",
        Section::Spend,
        each("spend", &|agent, p, out| out.extend(entries(p).map(|(m, v)| Item { agent: Some(agent.to_string()), ..Item::named(m.clone(), usd_at(v, &["total", "usd"])) }))),
        &spends,
        Order::Cost(TOP_ROWS),
        min_files,
    ));
    tables.push(table(
        "component",
        Section::Spend,
        each("spend", &|_, p, out| {
            for (_, m) in entries(p) {
                out.extend(entries(&m["components"]).map(|(c, v)| Item::named(c.clone(), usd_at(v, &["usd"]))));
            }
        }),
        &spends,
        Order::Cost(TOP_ROWS),
        min_files,
    ));
    tables.push(table(
        "content",
        Section::Spend,
        each("content", &|_, p, out| out.extend(entries(p).map(|(k, v)| Item::named(k.clone(), usd_at(v, &["usd"]))))),
        &spends,
        Order::Cost(TOP_ROWS),
        min_files,
    ));
    tables.push(table(
        "origin",
        Section::Spend,
        each("origins", &|_, p, out| {
            for (kind, v) in entries(p) {
                out.push(Item::named(kind.clone(), usd_at(v, &["usd"])));
                out.extend(entries(&v["names"]).map(|(n, s)| Item { under: true, ..Item::named(format!("{kind}:{n}"), usd_at(s, &["usd"])) }));
            }
        }),
        &spends,
        Order::Kinds,
        min_files,
    ));
    tables.push(table(
        "activity",
        Section::Spend,
        each("activities", &|_, p, out| out.extend(entries(p).map(|(k, v)| Item::named(k.clone(), usd_at(v, &["usd"]))))),
        &spends,
        Order::Cost(TOP_ROWS),
        min_files,
    ));
    const PROMPT_NAMES: [&str; 6] = [PROMPT_RANGES[0].0, PROMPT_RANGES[1].0, PROMPT_RANGES[2].0, PROMPT_RANGES[3].0, PROMPT_RANGES[4].0, PROMPT_RANGES[5].0];
    const SPAN_NAMES: [&str; 5] = [SPAN_RANGES[0].0, SPAN_RANGES[1].0, SPAN_RANGES[2].0, SPAN_RANGES[3].0, SPAN_RANGES[4].0];
    tables.push(table("prompt_size", Section::Spend, each("prompt_sizes", &|_, p, out| halves(p, out)), &spends, Order::Fixed(&PROMPT_NAMES), 1));
    tables.push(table(
        "session_span",
        Section::Spend,
        each("session_spans", &|_, p, out| out.extend(entries(p).map(|(k, v)| Item::named(k.clone(), usd_at(v, &["usd"]))))),
        &spends,
        Order::Fixed(&SPAN_NAMES),
        1,
    ));
    tables.push(table(
        "client",
        Section::Spend,
        each("clients", &|agent, p, out| out.extend(entries(p).map(|(c, v)| Item { agent: Some(agent.to_string()), ..Item::named(c.clone(), usd_at(v, &["usd"])) }))),
        &spends,
        Order::Cost(TOP_ROWS),
        min_files,
    ));

    // What could have been saved.
    let mut saved = table(
        "saved",
        Section::Saved,
        each("misses", &|_, p, out| out.extend(entries(p).map(|(cause, v)| Item::named(cause.clone(), usd_at(v, &["avoidable_usd"]))))),
        &spends,
        Order::Cost(usize::MAX),
        1,
    );
    let lifetimes = rows_table(
        "saved",
        Section::Saved,
        &files,
        &spends,
        &[("cache lifetime: main", &|f: &Value| lifetime_saving(f, "main")), ("cache lifetime: subagents", &|f: &Value| lifetime_saving(f, "subagents"))],
    );
    saved.rows.extend(lifetimes.rows.into_iter().filter(|r| r.files_with_part > 0));
    saved.rows.sort_by(|a, b| b.usd.total_cmp(&a.usd).then_with(|| a.name.cmp(&b.name)));
    tables.push(saved);

    // Habits that cost the most.
    let hint_rows: BTreeSet<String> = files.iter().flat_map(|f| part(f, "tool_hints", |_, p, out| out.extend(entries(p).map(|(h, _)| Item::named(h.clone(), 0.0)))).unwrap_or_default()).map(|i| i.name).collect();
    let hint_value = |hint: String| {
        move |f: &Value| -> Option<f64> {
            part(f, "tool_hints", |_, p, out| {
                if let Some(h) = p.get(&hint) {
                    let view = if hint == "batch independent reads" { "trips" } else { "rent" };
                    out.push(Item::named("", usd_at(h, &[view, "usd"])));
                }
            })
            .map(|items| items.iter().map(|i| i.usd).sum())
        }
    };
    type Boxed = Box<dyn Fn(&Value) -> Option<f64>>;
    let hint_fns: Vec<(String, Boxed)> = hint_rows.into_iter().map(|h| (format!("tool hint: {h}"), Box::new(hint_value(h.clone())) as Boxed)).collect();
    let big_prompts = |f: &Value| ranges_usd(f, "prompt_sizes", &["400–800k", "800k+"], true);
    let long_sessions = |f: &Value| ranges_usd(f, "session_spans", &["1–3d", "over 3d"], false);
    let lookups = |f: &Value| part(f, "triggers", |_, p, out| out.push(Item::named("", usd_at(p, &["lookup", "usd"])))).map(|i| i.iter().map(|i| i.usd).sum());
    let thinking = |f: &Value| part(f, "content", |_, p, out| out.push(Item::named("", usd_at(p, &["thinking", "usd"])))).map(|i| i.iter().map(|i| i.usd).sum());
    let setup = |f: &Value| part(f, "setup", |_, p, out| out.extend(entries(p).map(|(_, v)| Item::named("", usd_at(v, &["usd"]))))).map(|i| i.iter().map(|i| i.usd).sum());
    // Format 1 counts no dollars per effort level, so its files have no part for this row.
    let effort = |f: &Value| -> Option<f64> {
        let mut found = false;
        let mut usd = 0.0;
        for (_, a) in agents(f) {
            for half in ["main", "subagents"] {
                for level in ["max", "xhigh"] {
                    if let Some(u) = a.pointer(&format!("/effort/{half}/{level}/usd")).and_then(Value::as_f64) {
                        found = true;
                        usd += u;
                    }
                }
                found |= entries(&a["effort"][half]).any(|(_, l)| l.get("usd").is_some());
            }
        }
        found.then_some(usd)
    };
    let mut habit_rows: Vec<(&str, RowValue)> = vec![
        ("prompts of 400k+", &big_prompts),
        ("sessions longer than a day", &long_sessions),
        ("single lookups", &lookups),
        ("thinking", &thinking),
        ("setup", &setup),
        ("effort max or xhigh", &effort),
    ];
    for (name, f) in &hint_fns {
        habit_rows.push((name.as_str(), f.as_ref()));
    }
    tables.push(rows_table("habits", Section::Habits, &files, &spends, &habit_rows));

    // Tools and commands: shell tools by program, other tools as they are, by rent.
    tables.push(table(
        "tools",
        Section::Tools,
        each("tools", &|_, p, out| {
            for (tool, t) in entries(p) {
                let row = |name: String, v: &Value| Item {
                    calls: v.get("calls").and_then(Value::as_u64).unwrap_or(0),
                    trips_usd: usd_at(v, &["trips", "usd"]),
                    ..Item::named(name, usd_at(v, &["rent", "usd"]))
                };
                match t.get("programs").and_then(Value::as_object) {
                    Some(programs) if !programs.is_empty() => out.extend(programs.iter().map(|(prog, v)| row(format!("{tool} {prog}"), v))),
                    _ => out.push(row(tool.clone(), t)),
                }
            }
        }),
        &spends,
        Order::Cost(TOP_TOOLS),
        min_files,
    ));

    // Versions in use.
    tables.push(table(
        "versions",
        Section::Versions,
        each("versions", &|agent, p, out| {
            out.extend(entries(p).map(|(v, r)| Item {
                agent: Some(agent.to_string()),
                requests: r.get("requests").and_then(Value::as_u64).unwrap_or(0),
                ..Item::named(v.clone(), usd_at(r, &["usd"]))
            }))
        }),
        &spends,
        Order::PerAgent(TOP_VERSIONS),
        min_files,
    ));

    Pool {
        files: infos,
        spend_usd: spends.iter().sum(),
        median_usd: at_quarter(&sorted, 2),
        quarter_1_usd: at_quarter(&sorted, 1),
        quarter_3_usd: at_quarter(&sorted, 3),
        tables,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// A format-2 file of one agent with this spend on one model and a few parts.
    fn file(spend: f64, extra: Value) -> Value {
        let mut agent = json!({
            "spend": {"claude-opus-5-5": {"requests": 10, "priced": true, "components": {"cache read": {"usd": spend * 0.6, "tokens": 1}, "output": {"usd": spend * 0.4, "tokens": 1}}, "total": {"usd": spend, "tokens": 2}}},
            "clients": {"sdk-ts": {"requests": 10, "usd": spend}},
            "misses": {},
            "triggers": {"lookup": {"requests": 1, "usd": spend * 0.3, "tokens": 1}},
            "activities": {"reading": {"requests": 1, "usd": spend, "tokens": 1}},
            "setup": {"skills": {"usd": spend * 0.1, "tokens": 1, "subagents_usd": 0.0}},
            "effort": {"main": {"max": {"requests": 10, "output_tokens": 5, "usd": spend * 0.9}}, "subagents": {}},
            "origins": {"prompt": {"requests": 10, "usd": spend, "tokens": 1}},
        });
        if let (Value::Object(a), Value::Object(e)) = (&mut agent, extra) {
            a.extend(e);
        }
        json!({"format": 2, "tokprof": "0.2.0", "platform": "macos-aarch64", "window": {"first": "2026-10-04", "last": "2026-11-03"}, "survey": "team-2026-11", "agents": {"claude": agent}})
    }

    fn named(files: Vec<Value>) -> Vec<(String, Value)> {
        files.into_iter().enumerate().map(|(i, f)| (format!("f{i}"), f)).collect()
    }

    fn row<'a>(p: &'a Pool, table: &str, name: &str) -> &'a Row {
        p.table(table).unwrap().rows.iter().find(|r| r.name == name).unwrap_or_else(|| panic!("no {name} in {table}"))
    }

    #[test]
    fn five_files_give_the_spend_per_file() {
        let p = build(&named([2_140.0, 3_900.0, 6_310.0, 9_870.0, 19_000.0].map(|s| file(s, json!({}))).to_vec()), 1);
        assert_eq!(p.files.len(), 5);
        assert!((p.spend_usd - 41_220.0).abs() < 1e-6);
        assert_eq!((p.median_usd, p.quarter_1_usd, p.quarter_3_usd), (6_310.0, 3_900.0, 9_870.0));
    }

    #[test]
    fn a_row_that_two_of_five_files_have() {
        let names = |share: f64| json!({"origins": {"prompt": {"requests": 1, "usd": 100.0 * (1.0 - share), "tokens": 1}, "skill": {"requests": 1, "usd": 100.0 * share, "tokens": 1, "names": {"code-review": {"requests": 1, "usd": 100.0 * share, "tokens": 1}}}}});
        let files = vec![file(100.0, names(0.2)), file(100.0, names(0.3)), file(100.0, json!({})), file(100.0, json!({})), file(100.0, json!({}))];
        let p = build(&named(files), 1);
        let r = row(&p, "origin", "skill:code-review");
        assert_eq!((r.files, r.median_file), (2, 0.0));
        assert!(r.under);
        // The kind comes before its name.
        let rows: Vec<&str> = p.table("origin").unwrap().rows.iter().map(|r| r.name.as_str()).collect();
        assert_eq!(rows, ["prompt", "skill", "skill:code-review"]);
    }

    #[test]
    fn ranges_keep_their_order() {
        let sizes = json!({"prompt_sizes": {"main": {"800k+": {"requests": 1, "usd": 90.0, "tokens": 1}, "under 50k": {"requests": 1, "usd": 10.0, "tokens": 1}}, "subagents": {}}});
        let p = build(&named(vec![file(100.0, sizes)]), 1);
        let rows: Vec<&str> = p.table("prompt_size").unwrap().rows.iter().map(|r| r.name.as_str()).collect();
        assert_eq!(rows, ["under 50k", "800k+"]);
        assert_eq!(row(&p, "habits", "prompts of 400k+").usd, 90.0);
    }

    #[test]
    fn cold_returns_in_most_files_count_as_notable() {
        let miss = |usd: f64| json!({"misses": {"idle > TTL": {"misses": 3, "tokens": 1, "avoidable_usd": usd}}});
        let files = vec![file(100.0, miss(6.0)), file(100.0, miss(5.0)), file(100.0, miss(9.0)), file(100.0, miss(20.0)), file(100.0, miss(1.0))];
        let p = build(&named(files), 1);
        assert_eq!(row(&p, "saved", "idle > TTL").notable_files, 4);
    }

    #[test]
    fn a_cache_lifetime_saves_only_on_the_dearer_one() {
        let whatif = |actual: f64, short: f64, long: f64| json!({"whatif": {"main": {"requests": 1, "actual_usd": actual, "simulated_5m_usd": short, "simulated_1h_usd": long}, "subagents": {"requests": 0, "actual_usd": 0.0, "simulated_5m_usd": 0.0, "simulated_1h_usd": 0.0}}});
        let cheaper = build(&named(vec![file(1_000.0, whatif(118.0, 300.0, 120.0))]), 1);
        assert_eq!(row(&cheaper, "saved", "cache lifetime: main").usd, 0.0);
        let dearer = build(&named(vec![file(1_000.0, whatif(300.0, 300.0, 120.0))]), 1);
        assert_eq!(row(&dearer, "saved", "cache lifetime: main").usd, 180.0);
    }

    #[test]
    fn a_format_1_file_counts_only_in_the_rows_it_has_the_parts_for() {
        let mut old = file(100.0, json!({}));
        old["format"] = json!(1);
        // Format 1 counts effort in requests and output only.
        old["agents"]["claude"]["effort"] = json!({"main": {"max": {"requests": 10, "output_tokens": 5}}, "subagents": {}});
        let new = file(100.0, json!({"content": {"thinking": {"usd": 20.0, "tokens": 1}}}));
        let p = build(&named(vec![old, new]), 1);
        assert_eq!(row(&p, "habits", "setup").files_with_part, 2);
        assert_eq!(row(&p, "habits", "single lookups").files_with_part, 2);
        assert_eq!(row(&p, "habits", "effort max or xhigh").files_with_part, 1);
        assert_eq!(row(&p, "habits", "thinking").files_with_part, 1);
        let content = p.table("content").unwrap();
        assert_eq!(content.files_with_part, 1);
        // The share is of the spend of the files that have the part.
        assert!((content.rows[0].share - 0.2).abs() < 1e-9);
    }

    #[test]
    fn shell_tools_show_by_program_and_without_a_row_of_their_own() {
        let tools = json!({"tools": {"Bash": {"calls": 30, "rent": {"usd": 30.0, "tokens": 1}, "trips": {"usd": 3.0, "tokens": 1}, "programs": {
            "sed": {"calls": 10, "rent": {"usd": 10.0, "tokens": 1}, "trips": {"usd": 1.0, "tokens": 1}},
            "git": {"calls": 15, "rent": {"usd": 15.0, "tokens": 1}, "trips": {"usd": 1.5, "tokens": 1}},
            "other": {"calls": 5, "rent": {"usd": 5.0, "tokens": 1}, "trips": {"usd": 0.5, "tokens": 1}}}},
            "Read": {"calls": 3, "rent": {"usd": 40.0, "tokens": 1}, "trips": {"usd": 4.0, "tokens": 1}}}});
        let p = build(&named(vec![file(100.0, tools)]), 1);
        let rows: Vec<&str> = p.table("tools").unwrap().rows.iter().map(|r| r.name.as_str()).collect();
        assert_eq!(rows, ["Read", "Bash git", "Bash sed", "Bash other"]);
        assert_eq!(row(&p, "tools", "Bash git").calls, 15);
    }

    #[test]
    fn many_versions_fold_past_five() {
        let versions: serde_json::Map<String, Value> = (0..14).map(|i| (format!("2.1.{}", 300 + i), json!({"requests": 100 + i, "usd": 1.0}))).collect();
        let p = build(&named(vec![file(100.0, json!({"versions": versions}))]), 1);
        let rows = &p.table("versions").unwrap().rows;
        assert_eq!(rows.len(), 6);
        assert_eq!(rows[0].name, "2.1.313");
        assert_eq!(rows[5].name, "9 more");
    }

    #[test]
    fn rare_rows_fold_into_other_and_rare_names_drop() {
        let rare = json!({"spend": {"claude-haiku-4-5-20251001": {"requests": 1, "priced": true, "components": {}, "total": {"usd": 5.0, "tokens": 1}}, "claude-opus-5-5": {"requests": 1, "priced": true, "components": {}, "total": {"usd": 95.0, "tokens": 1}}},
            "origins": {"prompt": {"requests": 1, "usd": 90.0, "tokens": 1}, "skill": {"requests": 1, "usd": 10.0, "tokens": 1, "names": {"simplify": {"requests": 1, "usd": 10.0, "tokens": 1}}}}});
        let mut files = vec![file(100.0, rare)];
        // Every file has skills; only one has the name.
        let skills = json!({"origins": {"prompt": {"requests": 1, "usd": 90.0, "tokens": 1}, "skill": {"requests": 1, "usd": 10.0, "tokens": 1}}});
        files.extend((0..7).map(|_| file(100.0, skills.clone())));
        let p = build(&named(files), 3);
        let models: Vec<&str> = p.table("model").unwrap().rows.iter().map(|r| r.name.as_str()).collect();
        assert_eq!(models, ["claude-opus-5-5", "other"]);
        assert!(p.table("origin").unwrap().rows.iter().all(|r| r.name != "skill:simplify"));
        assert!(row(&p, "origin", "skill").usd > 0.0, "the kind keeps the name's spend");
    }

    #[test]
    fn files_are_share_files_of_a_format_this_tokprof_reads() {
        assert_eq!(parse("not json"), Err(Skip::NotShare));
        assert_eq!(parse(r#"{"format": 2}"#), Err(Skip::NotShare));
        assert_eq!(parse(r#"{"format": 3, "agents": {}}"#), Err(Skip::Newer(3)));
        assert!(parse(r#"{"format": 1, "agents": {}}"#).is_ok());
    }
}
