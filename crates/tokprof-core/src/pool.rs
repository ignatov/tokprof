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
    /// The file's activity, every agent together (see share-export), when the file has it.
    pub active_days: Option<f64>,
    pub active_hours: Option<f64>,
    pub busy_ms: Option<f64>,
    /// The time at the keyboard, from the file's rhythm part.
    pub keyboard_ms: Option<f64>,
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
    /// Failed calls, for the tool table.
    pub failed: u64,
    /// The row's dollars per active hour, for the savings.
    pub per_hour: Option<f64>,
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

/// What a rate row counts.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Unit {
    #[default]
    Count,
    Usd,
    Ms,
}

/// A row of a table of counts with a rate: per active hour, or per 100 typed messages.
#[derive(Clone, Debug, Default, Serialize)]
pub struct RateRow {
    pub agent: Option<String>,
    pub name: String,
    pub unit: Unit,
    /// The pooled value.
    pub value: f64,
    /// What the value is a part of, such as all requests for the slow ones.
    pub of: Option<f64>,
    /// The pooled value over the pooled base, and the median of each file's own rate. `None` when no
    /// file has a base for it.
    pub rate: Option<f64>,
    pub median_rate: Option<f64>,
    /// The files where the value is above zero, and those that have the row's parts.
    pub files: usize,
    pub files_with_part: usize,
    /// Each file's own rate; `None` for a file without the parts.
    pub by_file: Vec<Option<f64>>,
    /// For the cold returns by client: the active hours, and the avoidable dollars per hour.
    pub hours: Option<f64>,
    pub second_rate: Option<f64>,
    /// A row under the row before it, such as an error class under the retried requests.
    pub under: bool,
}

/// A row of `Coming back after a cold break`: a way of coming back, from the bench parts.
#[derive(Clone, Debug, Default, Serialize)]
pub struct BenchRow {
    pub name: String,
    pub usd: f64,
    /// The way's cost over the cost as resumed.
    pub of_resume: f64,
    /// The returns where the way cost least.
    pub cheapest: u64,
    pub median_file: Option<f64>,
    pub by_file: Vec<Option<f64>>,
}

/// The cold returns of the files and what each way of coming back would have cost.
#[derive(Clone, Debug, Default, Serialize)]
pub struct BenchTable {
    pub returns: u64,
    /// The files with a cold return.
    pub files: usize,
    pub rows: Vec<BenchRow>,
}

/// What a row of `Your time with the agents` counts.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RhythmUnit {
    #[default]
    Hours,
    Count,
    Duration,
    Share,
    Rate,
}

/// A row of `Your time with the agents`.
#[derive(Clone, Debug, Default, Serialize)]
pub struct RhythmRow {
    pub name: String,
    pub unit: RhythmUnit,
    /// The pooled value: a sum, a share or a rate. A median has none; `range` names the pause
    /// range that holds the median of the pooled ranges instead.
    pub value: Option<f64>,
    pub range: Option<String>,
    pub median_file: Option<f64>,
    pub files: usize,
    pub by_file: Vec<Option<f64>>,
}

/// A row of `Working with the agent by client`.
#[derive(Clone, Debug, Default, Serialize)]
pub struct ClientWork {
    pub agent: Option<String>,
    pub client: String,
    pub keyboard_hours: f64,
    pub messages: f64,
    pub interrupts: f64,
    pub questions: f64,
    pub commits: f64,
    /// The median answer to an end of turn: the file's, or the median of the files'.
    pub answered_ms: Option<f64>,
    pub files: usize,
}

/// The commits, pushes and pull requests of the agents.
#[derive(Clone, Debug, Default, Serialize)]
pub struct Outcomes {
    pub rows: Vec<RateRow>,
    /// The files with the part.
    pub files: usize,
    /// The spend of the files with the part over their commits; `None` without a commit.
    pub usd_per_commit: Option<f64>,
}

/// A row of the busy time by kind.
#[derive(Clone, Debug, Default, Serialize)]
pub struct TimeRow {
    pub name: String,
    pub ms: f64,
    pub after_turns_ms: f64,
    pub share: f64,
    pub median_file: f64,
    pub files: usize,
    pub by_file: Vec<Option<f64>>,
}

/// A row of the highest effort by client.
#[derive(Clone, Debug, Default, Serialize)]
pub struct EffortRow {
    pub agent: String,
    pub client: String,
    /// The client's pooled dollars, as its effort levels count them.
    pub usd: f64,
    /// The share of the client's spend, and of its requests, at `max` or `xhigh`.
    pub spend_share: f64,
    pub request_share: f64,
    pub median_file: f64,
    pub files: usize,
    pub by_file: Vec<Option<f64>>,
}

/// What the activity line says.
#[derive(Clone, Debug, Default, Serialize)]
pub struct Activity {
    pub files: usize,
    pub active_days: f64,
    pub active_hours: f64,
    pub busy_ms: f64,
    pub usd_per_hour: f64,
    pub median_days: f64,
    pub median_hours: f64,
    pub median_usd_per_hour: f64,
    /// Over the files that also have time at the keyboard: that time, and the dollars per hour
    /// of it, pooled and of the median file.
    pub keyboard_ms: Option<f64>,
    pub usd_per_keyboard_hour: Option<f64>,
    pub median_usd_per_keyboard_hour: Option<f64>,
}

/// The share of spend whose prompts the logs hold.
#[derive(Clone, Debug, Default, Serialize)]
pub struct Coverage {
    pub share: f64,
    pub files: usize,
    pub files_below_90: usize,
}

/// A model that some files have no price for: its tokens there count as $0.
#[derive(Clone, Debug, Default, Serialize)]
pub struct Unpriced {
    pub agent: String,
    pub model: String,
    pub tokens: f64,
    pub files: usize,
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
    pub activity: Option<Activity>,
    pub cold_by_client: Vec<RateRow>,
    /// The files with the client parts of the cold returns and of the effort.
    pub cold_files: usize,
    pub effort_files: usize,
    pub problems: Vec<RateRow>,
    pub time: Vec<TimeRow>,
    pub time_files: usize,
    pub working: Vec<RateRow>,
    pub effort_by_client: Vec<EffortRow>,
    /// The clients past the shown ones, folded: how many and their cost.
    pub effort_more: Option<(usize, f64)>,
    pub coverage: Option<Coverage>,
    /// The models without a price, the most tokens first.
    pub unpriced: Vec<Unpriced>,
    pub bench: Option<BenchTable>,
    pub rhythm: Vec<RhythmRow>,
    /// The files with the rhythm part.
    pub rhythm_files: usize,
    pub client_work: Vec<ClientWork>,
    pub outcomes: Option<Outcomes>,
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
    failed: u64,
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
            slot.failed += it.failed;
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
                failed: per_file.iter().flatten().map(|v| v.failed).sum(),
                per_hour: None,
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
            let o = other.get_or_insert_with(|| Row {
                name: "other".into(),
                files_with_part: r.files_with_part,
                by_file: vec![Some(0.0); r.by_file.len()],
                ..Row::default()
            });
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

/// A replay's lifetime in ms, from its key: `simulated_5m_usd` is 5 minutes.
fn lifetime_ms(key: &str) -> Option<i64> {
    let l = key.strip_prefix("simulated_")?.strip_suffix("_usd")?;
    let (n, unit) = l.split_at(l.len().checked_sub(1)?);
    let n: i64 = n.parse().ok()?;
    match unit {
        "m" => Some(n * 60_000),
        "h" => Some(n * 3_600_000),
        _ => None,
    }
}

/// What the what-if replay says the other cache lifetime would have saved in one bucket: the
/// replay under the lifetime the bucket wrote most with, minus the cheaper replay. The logged cost
/// is not used: it holds the other misses too, which their own rows count.
fn lifetime_saving(file: &Value, bucket: &str) -> Option<f64> {
    let mut found = false;
    let mut saved = 0.0;
    for (_, a) in agents(file) {
        let Some(w) = a.get("whatif").map(|w| &w[bucket]) else { continue };
        found = true;
        let mut replays: Vec<(i64, f64)> = entries(w).filter_map(|(k, v)| Some((lifetime_ms(k)?, v.as_f64()?))).collect();
        replays.sort_by_key(|r| r.0);
        let (Some(short), Some(long)) = (replays.first().copied(), replays.last().copied()) else { continue };
        let writes = &a["cache_writes"][bucket];
        let used = if writes["long"].as_f64().unwrap_or(0.0) > writes["short"].as_f64().unwrap_or(0.0) { long.1 } else { short.1 };
        saved += (used - short.1.min(long.1)).max(0.0);
    }
    found.then_some(saved)
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
            active_days: v.pointer("/activity/active_days").and_then(Value::as_f64),
            active_hours: v.pointer("/activity/active_hours").and_then(Value::as_f64),
            busy_ms: v.pointer("/activity/busy_ms").and_then(Value::as_f64),
            keyboard_ms: v.pointer("/rhythm/keyboard_ms").and_then(Value::as_f64),
        })
        .collect::<Vec<FileInfo>>();
    let mut sorted = spends.clone();
    sorted.sort_by(f64::total_cmp);
    let each = |key: &str, f: &dyn Fn(&str, &Value, &mut Vec<Item>)| -> Vec<Option<Vec<Item>>> { files.iter().map(|file| part(file, key, f)).collect() };
    let mut tables = Vec::new();

    // Where the money went.
    tables.push(table(
        "model",
        Section::Spend,
        each("spend", &|agent, p, out| {
            out.extend(entries(p).map(|(m, v)| Item { agent: Some(agent.to_string()), ..Item::named(m.clone(), usd_at(v, &["total", "usd"])) }))
        }),
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
        each("clients", &|agent, p, out| {
            out.extend(entries(p).map(|(c, v)| Item { agent: Some(agent.to_string()), ..Item::named(c.clone(), usd_at(v, &["usd"])) }))
        }),
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
    let hint_rows: BTreeSet<String> = files
        .iter()
        .flat_map(|f| part(f, "tool_hints", |_, p, out| out.extend(entries(p).map(|(h, _)| Item::named(h.clone(), 0.0)))).unwrap_or_default())
        .map(|i| i.name)
        .collect();
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
    let thinking =
        |f: &Value| part(f, "content", |_, p, out| out.push(Item::named("", usd_at(p, &["thinking", "usd"])))).map(|i| i.iter().map(|i| i.usd).sum());
    let setup = |f: &Value| {
        part(f, "setup", |_, p, out| out.extend(entries(p).map(|(_, v)| Item::named("", usd_at(v, &["usd"]))))).map(|i| i.iter().map(|i| i.usd).sum())
    };
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
                    failed: v.get("failed").and_then(Value::as_u64).unwrap_or(0),
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

    let hours: Vec<Option<f64>> = infos.iter().map(|f| f.active_hours.filter(|h| *h > 0.0)).collect();
    let keyboard: Vec<Option<f64>> = infos.iter().map(|f| f.keyboard_ms.map(|ms| ms / 3_600_000.0).filter(|h| *h > 0.0)).collect();
    // Savings per active hour: the dollars of the files with both parts over their hours.
    if let Some(saved) = tables.iter_mut().find(|t| t.key == "saved") {
        for r in &mut saved.rows {
            let (mut usd, mut h) = (0.0, 0.0);
            for ((share, spend), hours) in r.by_file.iter().zip(&spends).zip(&hours) {
                if let (Some(share), Some(hours)) = (share, hours) {
                    usd += share * spend;
                    h += hours;
                }
            }
            r.per_hour = (h > 0.0).then(|| usd / h);
        }
    }
    let activity = activity_of(&infos, &spends);
    let (effort_rows, effort_more) = effort_by_client(&files, min_files, spends.iter().sum());
    Pool {
        cold_by_client: cold_by_client(&files, min_files),
        cold_files: with_client_part(&files, "active_hours"),
        effort_files: with_client_part(&files, "effort"),
        problems: problems(&files, &hours),
        time: time_rows(&files),
        time_files: files.iter().filter(|f| part(f, "time", |_, _, _| {}).is_some()).count(),
        working: working(&files, &keyboard),
        bench: bench_table(&files),
        rhythm: rhythm_rows(&files),
        rhythm_files: files.iter().filter(|f| f.get("rhythm").is_some_and(Value::is_object)).count(),
        client_work: client_work(&files, min_files),
        outcomes: outcomes(&files, &keyboard, &spends),
        effort_by_client: effort_rows,
        effort_more,
        coverage: coverage(&files),
        unpriced: unpriced(&files, min_files),
        activity,
        files: infos,
        spend_usd: spends.iter().sum(),
        median_usd: at_quarter(&sorted, 2),
        quarter_1_usd: at_quarter(&sorted, 1),
        quarter_3_usd: at_quarter(&sorted, 3),
        tables,
    }
}

/// The median of the values that are there.
fn median_of(values: impl Iterator<Item = f64>) -> f64 {
    let mut v: Vec<f64> = values.collect();
    v.sort_by(f64::total_cmp);
    at_quarter(&v, 2)
}

/// The activity line: the files with the part, pooled and at the median.
fn activity_of(infos: &[FileInfo], spends: &[f64]) -> Option<Activity> {
    let with: Vec<(&FileInfo, f64)> = infos.iter().zip(spends).filter(|(f, _)| f.active_hours.is_some()).map(|(f, s)| (f, *s)).collect();
    if with.is_empty() {
        return None;
    }
    let hours: f64 = with.iter().map(|(f, _)| f.active_hours.unwrap_or(0.0)).sum();
    let usd: f64 = with.iter().map(|(_, s)| s).sum();
    let keyboard: Vec<(f64, f64)> = with.iter().filter_map(|(f, s)| f.keyboard_ms.filter(|k| *k > 0.0).map(|k| (k, *s))).collect();
    Some(Activity {
        files: with.len(),
        active_days: with.iter().map(|(f, _)| f.active_days.unwrap_or(0.0)).sum(),
        active_hours: hours,
        busy_ms: with.iter().map(|(f, _)| f.busy_ms.unwrap_or(0.0)).sum(),
        usd_per_hour: usd / hours.max(1e-9),
        median_days: median_of(with.iter().map(|(f, _)| f.active_days.unwrap_or(0.0))),
        median_hours: median_of(with.iter().map(|(f, _)| f.active_hours.unwrap_or(0.0))),
        median_usd_per_hour: median_of(with.iter().map(|(f, s)| s / f.active_hours.unwrap_or(0.0).max(1e-9))),
        keyboard_ms: (!keyboard.is_empty()).then(|| keyboard.iter().map(|x| x.0).sum()),
        usd_per_keyboard_hour: (!keyboard.is_empty())
            .then(|| keyboard.iter().map(|x| x.1).sum::<f64>() / (keyboard.iter().map(|x| x.0).sum::<f64>() / 3_600_000.0).max(1e-9)),
        median_usd_per_keyboard_hour: (!keyboard.is_empty()).then(|| median_of(keyboard.iter().map(|(k, s)| s / (k / 3_600_000.0).max(1e-9)))),
    })
}

/// A rate row over the files: each file's value (`None` without the parts) and its base, such as
/// its active hours. Only the files with both, and a base above zero, count: in the pooled value,
/// the rate, the median and the file counts.
fn rate_row(name: &str, unit: Unit, values: &[Option<f64>], bases: &[Option<f64>], per: f64) -> RateRow {
    let (mut value, mut base, mut with_part, mut above) = (0.0, 0.0, 0, 0);
    let mut by_file = Vec::new();
    for (v, b) in values.iter().zip(bases) {
        match (v, b) {
            (Some(v), Some(b)) if *b > 0.0 => {
                value += v;
                base += b;
                with_part += 1;
                above += usize::from(*v > 0.0);
                by_file.push(Some(v / b * per));
            }
            _ => by_file.push(None),
        }
    }
    RateRow {
        name: name.to_string(),
        unit,
        value,
        rate: (base > 0.0).then(|| value / base * per),
        median_rate: (base > 0.0).then(|| median_of(by_file.iter().flatten().copied())),
        files: above,
        files_with_part: with_part,
        by_file,
        ..RateRow::default()
    }
}

/// A number of the agent objects of a file, summed, or `None` when no agent has it.
fn summed(file: &Value, pointer: &str) -> Option<f64> {
    let mut found = false;
    let mut sum = 0.0;
    for (_, a) in agents(file) {
        if let Some(x) = a.pointer(pointer).and_then(Value::as_f64) {
            found = true;
            sum += x;
        }
    }
    found.then_some(sum)
}

/// The failures, retries and slow requests of the files, per active hour.
fn problems(files: &[Value], hours: &[Option<f64>]) -> Vec<RateRow> {
    let each = |pointer: &str| -> Vec<Option<f64>> { files.iter().map(|f| summed(f, pointer)).collect() };
    let rows: [(&str, Unit, &str); 6] = [
        ("failed tool calls", Unit::Count, "/failures/failed"),
        ("spend after failures", Unit::Usd, "/failures/after/usd"),
        ("calls the user refused", Unit::Count, "/failures/refused"),
        ("calls a rule denied", Unit::Count, "/failures/denied"),
        ("requests that waited for retries", Unit::Count, "/time/retried"),
        ("time waiting for retries", Unit::Ms, "/time/kinds/API retries/ms"),
    ];
    let mut out: Vec<RateRow> = rows
        .iter()
        .filter_map(|(name, unit, pointer)| {
            // A file with the time part and no retries has none: zero, not missing.
            let values: Vec<Option<f64>> = if pointer.starts_with("/time/") {
                files.iter().map(|f| part(f, "time", |_, _, _| {}).map(|_| summed(f, pointer).unwrap_or(0.0))).collect()
            } else {
                each(pointer)
            };
            values.iter().any(Option::is_some).then(|| rate_row(name, *unit, &values, hours, 1.0))
        })
        .collect();
    // The requests that waited for retries, by the class of their last error, under their row.
    if let Some(at) = out.iter().position(|r| r.name == "requests that waited for retries") {
        let mut classes = Vec::new();
        for (key, name) in RETRY_CLASS_NAMES {
            let pointer = format!("/time/retried_by_error/{key}");
            let values: Vec<Option<f64>> = files.iter().map(|f| summed(f, &pointer)).collect();
            if values.iter().flatten().sum::<f64>() > 0.0 {
                classes.push(RateRow { under: true, ..rate_row(name, Unit::Count, &values, hours, 1.0) });
            }
        }
        out.splice(at + 1..at + 1, classes);
    }
    let slow = each("/time/slow");
    if slow.iter().any(Option::is_some) {
        let mut row = rate_row("requests of a minute or more", Unit::Count, &slow, hours, 1.0);
        row.of = Some(each("/time/requests").iter().flatten().sum());
        out.push(row);
    }
    out
}

/// The rows of the error classes under the retried requests: the key in the file, the name.
const RETRY_CLASS_NAMES: [(&str, &str); 7] = [
    ("server", "server errors"),
    ("rate_limit", "rate limits"),
    ("auth", "authentication"),
    ("timeout", "timeouts"),
    ("connection", "connection"),
    ("sleep", "computer asleep"),
    ("other", "other errors"),
];

/// A file's cold returns, and per way of coming back its dollars and the returns it was cheapest in.
type BenchSums = (u64, [(f64, u64); 5]);

/// The ways of coming back over the files' cold returns. `None` when no file has one.
fn bench_table(files: &[Value]) -> Option<BenchTable> {
    let ways = crate::share::BENCH_WAYS;
    let per_file: Vec<Option<BenchSums>> = files
        .iter()
        .map(|f| {
            let mut found: BenchSums = (0, [(0.0, 0); 5]);
            for (_, a) in agents(f) {
                let Some(b) = a.get("bench") else { continue };
                found.0 += b["returns"].as_u64().unwrap_or(0);
                for (i, key) in ways.iter().enumerate() {
                    found.1[i].0 += b.pointer(&format!("/ways/{key}/usd")).and_then(Value::as_f64).unwrap_or(0.0);
                    found.1[i].1 += b.pointer(&format!("/ways/{key}/cheapest")).and_then(Value::as_u64).unwrap_or(0);
                }
            }
            (found.0 > 0).then_some(found)
        })
        .collect();
    let with: Vec<&BenchSums> = per_file.iter().flatten().collect();
    if with.is_empty() {
        return None;
    }
    let resume: f64 = with.iter().map(|w| w.1[0].0).sum();
    let rows = crate::bench::Strategy::ALL
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let usd: f64 = with.iter().map(|w| w.1[i].0).sum();
            let by_file: Vec<Option<f64>> = per_file.iter().map(|w| w.as_ref().map(|w| w.1[i].0 / w.1[0].0.max(1e-9))).collect();
            BenchRow {
                name: s.name().to_string(),
                usd,
                of_resume: usd / resume.max(1e-9),
                cheapest: with.iter().map(|w| w.1[i].1).sum(),
                median_file: Some(median_of(by_file.iter().flatten().copied())),
                by_file,
            }
        })
        .collect();
    Some(BenchTable { returns: with.iter().map(|w| w.0).sum(), files: with.len(), rows })
}

/// A row of the rhythm table, with its median file and files from each file's value.
fn rhythm_row(name: &str, unit: RhythmUnit, value: Option<f64>, range: Option<String>, by_file: Vec<Option<f64>>) -> RhythmRow {
    let values: Vec<f64> = by_file.iter().flatten().copied().collect();
    RhythmRow {
        name: name.to_string(),
        unit,
        value,
        range,
        median_file: (!values.is_empty()).then(|| median_of(values.iter().copied())),
        files: values.len(),
        by_file,
    }
}

/// The pause range that holds the median of these counts by range, the upper of two middle
/// values, as the rhythm takes a median.
fn median_range(counts: &BTreeMap<String, f64>) -> Option<String> {
    let total: f64 = counts.values().sum();
    if total <= 0.0 {
        return None;
    }
    let at = (total / 2.0).floor();
    let mut seen = 0.0;
    for (name, _) in crate::pauses::PAUSE_RANGES {
        seen += counts.get(name).copied().unwrap_or(0.0);
        if seen > at {
            return Some(name.to_string());
        }
    }
    None
}

/// The rows of a wait: its median, and the share of it within 5 minutes when `within` names it.
fn wait_rows(parts: &[Option<&Value>], key: &str, name: &str, within: Option<&str>) -> Vec<RhythmRow> {
    let mut merged: BTreeMap<String, f64> = BTreeMap::new();
    let (mut medians, mut shares) = (Vec::new(), Vec::new());
    let (mut fast_all, mut count_all) = (0.0, 0.0);
    for p in parts {
        let w = p.and_then(|r| r.get(key));
        let n = w.and_then(|w| w["count"].as_f64()).unwrap_or(0.0);
        medians.push(w.and_then(|w| w["median_ms"].as_f64()));
        let by = w.and_then(|w| w["by_pause"].as_object());
        let fast: f64 = by.map_or(0.0, |b| ["under 1m", "1–5m"].iter().filter_map(|k| b.get(*k)).filter_map(Value::as_f64).sum());
        shares.push((n > 0.0).then(|| fast / n));
        fast_all += fast;
        count_all += n;
        for (k, v) in by.into_iter().flatten() {
            *merged.entry(k.clone()).or_default() += v.as_f64().unwrap_or(0.0);
        }
    }
    let mut rows = vec![rhythm_row(name, RhythmUnit::Duration, None, median_range(&merged), medians)];
    if let Some(w) = within {
        rows.push(rhythm_row(w, RhythmUnit::Share, (count_all > 0.0).then(|| fast_all / count_all), None, shares));
    }
    rows
}

/// The rows of `Your time with the agents`, from the files' rhythm parts.
fn rhythm_rows(files: &[Value]) -> Vec<RhythmRow> {
    let parts: Vec<Option<&Value>> = files.iter().map(|f| f.get("rhythm").filter(|r| r.is_object())).collect();
    if parts.iter().all(Option::is_none) {
        return Vec::new();
    }
    let each = |f: &dyn Fn(&Value) -> Option<f64>| -> Vec<Option<f64>> { parts.iter().map(|p| p.and_then(f)).collect() };
    let num = |r: &Value, pointer: &str| r.pointer(pointer).and_then(Value::as_f64);
    let sum = |v: &[Option<f64>]| v.iter().flatten().sum::<f64>();
    let hours = each(&|r| num(r, "/keyboard_ms").map(|ms| ms / 3_600_000.0));
    let sittings = each(&|r| num(r, "/sittings"));
    let mut out = vec![
        rhythm_row("time at the keyboard", RhythmUnit::Hours, Some(sum(&hours)), None, hours.clone()),
        rhythm_row("sittings", RhythmUnit::Count, Some(sum(&sittings)), None, sittings),
    ];
    out.extend(wait_rows(&parts, "answers", "you answered an end of turn in", Some("… within 5 minutes")));
    out.extend(wait_rows(&parts, "question_answers", "you answered a question in", Some("… within 5 minutes")));
    out.extend(wait_rows(&parts, "turns", "the agent's turn took", None));
    // Ends while the person was there, per hour at the keyboard, over the files with both.
    let present = each(&|r| num(r, "/ends/present"));
    let both: Vec<(f64, f64)> = present.iter().zip(&hours).filter_map(|(p, h)| Some((p.as_ref().copied()?, h.filter(|h| *h > 0.0)?))).collect();
    let rate = (!both.is_empty()).then(|| both.iter().map(|x| x.0).sum::<f64>() / both.iter().map(|x| x.1).sum::<f64>());
    let per_hour: Vec<Option<f64>> = present.iter().zip(&hours).map(|(p, h)| Some(p.as_ref().copied()? / h.filter(|h| *h > 0.0)?)).collect();
    out.push(rhythm_row("ends of turn while you were there", RhythmUnit::Rate, rate, None, per_hour));
    let own = each(&|r| num(r, "/ends/started_by_agent"));
    let ends = each(&|r| num(r, "/ends/count"));
    let shares: Vec<Option<f64>> = own.iter().zip(&ends).map(|(o, e)| Some(o.as_ref().copied()? / e.filter(|e| *e > 0.0)?)).collect();
    let all_ends: f64 = own.iter().zip(&ends).filter_map(|(o, e)| e.filter(|e| *e > 0.0).and(*o)).sum();
    let share = (sum(&ends) > 0.0).then(|| all_ends / sum(&ends));
    out.push(rhythm_row("turns the agent started itself", RhythmUnit::Share, share, None, shares));
    out.extend(wait_rows(&parts, "stood_still", "the agents stood still after you left", None));
    let worked = each(&|r| num(r, "/worked_share_median"));
    out.push(rhythm_row("the agents worked through a break", RhythmUnit::Share, None, None, worked));
    out
}

/// The clients with time at the keyboard, the most first: those with [`MIN_CLIENT_SHARE`] of the
/// pooled time or more, at most [`TOP_ROWS`], then the rest folded into how many they are.
fn client_work(files: &[Value], min_files: usize) -> Vec<ClientWork> {
    let mut by: BTreeMap<(String, String), (ClientWork, Vec<f64>)> = BTreeMap::new();
    for f in files {
        for (agent, a) in agents(f) {
            for (client, c) in entries(&a["clients"]) {
                let Some(r) = c.get("rhythm") else { continue };
                let hours = r["keyboard_ms"].as_f64().unwrap_or(0.0) / 3_600_000.0;
                if hours <= 0.0 {
                    continue;
                }
                let e = by
                    .entry((agent.clone(), client.clone()))
                    .or_insert_with(|| (ClientWork { agent: Some(agent.clone()), client: client.clone(), ..ClientWork::default() }, Vec::new()));
                e.0.keyboard_hours += hours;
                e.0.messages += r["messages"].as_f64().unwrap_or(0.0);
                e.0.interrupts += r["interrupts"].as_f64().unwrap_or(0.0);
                e.0.questions += r["questions"].as_f64().unwrap_or(0.0);
                e.0.commits += c.pointer("/outcomes/commits").and_then(Value::as_f64).unwrap_or(0.0);
                e.0.files += 1;
                e.1.extend(r.pointer("/answers/median_ms").and_then(Value::as_f64));
            }
        }
    }
    let mut rows: Vec<ClientWork> = by
        .into_values()
        .map(|(mut w, medians)| {
            w.answered_ms = (!medians.is_empty()).then(|| median_of(medians.into_iter()));
            w
        })
        .collect();
    rows.sort_by(|a, b| b.keyboard_hours.total_cmp(&a.keyboard_hours).then_with(|| a.client.cmp(&b.client)));
    let total: f64 = rows.iter().map(|r| r.keyboard_hours).sum();
    let (mut shown, mut rest, mut folded) = (Vec::new(), ClientWork::default(), 0);
    for r in rows {
        if shown.len() < TOP_ROWS && r.files >= min_files.max(1) && r.keyboard_hours >= MIN_CLIENT_SHARE * total {
            shown.push(r);
        } else {
            folded += 1;
            rest.keyboard_hours += r.keyboard_hours;
            rest.messages += r.messages;
            rest.interrupts += r.interrupts;
            rest.questions += r.questions;
            rest.commits += r.commits;
            rest.files = rest.files.max(r.files);
        }
    }
    if folded > 0 {
        rest.client = format!("{folded} more");
        shown.push(rest);
    }
    shown
}

/// The agents' commits, pushes and pull requests per hour at the keyboard, and the dollars per
/// commit. `None` when no file has the part.
fn outcomes(files: &[Value], keyboard: &[Option<f64>], spends: &[f64]) -> Option<Outcomes> {
    let each = |key: &str| -> Vec<Option<f64>> {
        files
            .iter()
            .map(|f| {
                let found: Vec<f64> = agents(f).filter_map(|(_, a)| a.get("outcomes")).map(|o| o[key].as_f64().unwrap_or(0.0)).collect();
                (!found.is_empty()).then(|| found.iter().sum())
            })
            .collect()
    };
    let commits = each("commits");
    let with = commits.iter().flatten().count();
    if with == 0 {
        return None;
    }
    let rows = vec![
        rate_row("commits", Unit::Count, &commits, keyboard, 1.0),
        rate_row("pushes", Unit::Count, &each("pushes"), keyboard, 1.0),
        rate_row("pull requests", Unit::Count, &each("pull_requests"), keyboard, 1.0),
    ];
    let (usd, n) = commits.iter().zip(spends).filter_map(|(c, s)| c.map(|c| (*s, c))).fold((0.0, 0.0), |a, x| (a.0 + x.0, a.1 + x.1));
    Some(Outcomes { rows, files: with, usd_per_commit: (n > 0.0).then(|| usd / n) })
}

/// The busy time by kind: pooled, and each file's share of its own busy time.
fn time_rows(files: &[Value]) -> Vec<TimeRow> {
    let per_file: Vec<Option<BTreeMap<String, (f64, f64)>>> = files
        .iter()
        .map(|f| {
            part(f, "time", |_, t, out| {
                for (kind, k) in entries(&t["kinds"]) {
                    out.push(Item { trips_usd: k["after_turns_ms"].as_f64().unwrap_or(0.0), ..Item::named(kind.clone(), k["ms"].as_f64().unwrap_or(0.0)) });
                }
            })
            .map(|items| {
                let mut m: BTreeMap<String, (f64, f64)> = BTreeMap::new();
                for i in items {
                    let e = m.entry(i.name).or_default();
                    e.0 += i.usd;
                    e.1 += i.trips_usd;
                }
                m
            })
        })
        .collect();
    let total: f64 = per_file.iter().flatten().flat_map(|m| m.values()).map(|v| v.0).sum();
    let kinds: BTreeSet<String> = per_file.iter().flatten().flat_map(|m| m.keys().cloned()).collect();
    let mut rows: Vec<TimeRow> = kinds
        .into_iter()
        .map(|kind| {
            let by_file: Vec<Option<f64>> =
                per_file.iter().map(|m| m.as_ref().map(|m| m.get(&kind).map_or(0.0, |v| v.0) / m.values().map(|v| v.0).sum::<f64>().max(1e-9))).collect();
            let ms: f64 = per_file.iter().flatten().filter_map(|m| m.get(&kind)).map(|v| v.0).sum();
            TimeRow {
                after_turns_ms: per_file.iter().flatten().filter_map(|m| m.get(&kind)).map(|v| v.1).sum(),
                share: ms / total.max(1e-9),
                median_file: median_of(by_file.iter().flatten().copied()),
                files: per_file.iter().flatten().filter(|m| m.get(&kind).is_some_and(|v| v.0 > 0.0)).count(),
                name: kind,
                ms,
                by_file,
            }
        })
        .collect();
    rows.sort_by(|a, b| b.ms.total_cmp(&a.ms).then_with(|| a.name.cmp(&b.name)));
    rows
}

/// The hand-offs per active hour, and per 100 typed messages for the ones a typed message is the
/// base of: interrupts, messages typed while the agent worked, questions and answers.
fn working(files: &[Value], hours: &[Option<f64>]) -> Vec<RateRow> {
    let typed: Vec<Option<f64>> = files.iter().map(|f| summed(f, "/pauses/typed_messages")).collect();
    if !files.iter().any(|f| part(f, "handoffs", |_, _, _| {}).is_some()) {
        return Vec::new();
    }
    let handoff =
        |pointer: &str| -> Vec<Option<f64>> { files.iter().map(|f| part(f, "handoffs", |_, _, _| {}).map(|_| summed(f, pointer).unwrap_or(0.0))).collect() };
    let mut rows = vec![rate_row("typed messages", Unit::Count, &typed, hours, 1.0)];
    for (name, pointer, per_typed) in [
        ("ends of turn", "/handoffs/turn_ends", false),
        ("interrupts", "/handoffs/interrupts", true),
        ("typed while the agent worked", "/handoffs/typed_while_working", true),
        ("questions", "/handoffs/questions", true),
        ("answers", "/handoffs/answers", true),
        ("wakes: subagent", "/handoffs/wakes/subagent", false),
        ("wakes: command", "/handoffs/wakes/command", false),
        ("wakes: other", "/handoffs/wakes/other", false),
    ] {
        let values = handoff(pointer);
        let mut row = rate_row(name, Unit::Count, &values, hours, 1.0);
        if per_typed {
            row.second_rate = rate_row(name, Unit::Count, &values, &typed, 100.0).rate;
        }
        rows.push(row);
    }
    rows
}

/// The cold returns of each client per active hour, from the client parts.
fn cold_by_client(files: &[Value], min_files: usize) -> Vec<RateRow> {
    let mut by: BTreeMap<(String, String), ColdSums> = BTreeMap::new();
    for f in files {
        for (agent, a) in agents(f) {
            for (client, c) in entries(&a["clients"]) {
                let Some(hours) = c.get("active_hours").and_then(Value::as_f64) else { continue };
                let e = by.entry((agent.clone(), client.clone())).or_default();
                e.0 += hours;
                e.1 += c["cold_returns"].as_f64().unwrap_or(0.0);
                e.2 += c["cold_returns_avoidable_usd"].as_f64().unwrap_or(0.0);
                e.3 += 1;
            }
        }
    }
    let row = |agent: Option<String>, name: String, (hours, cold, avoidable, n): ColdSums| RateRow {
        agent,
        name,
        value: cold,
        hours: Some(hours),
        rate: Some(cold / hours.max(1e-9)),
        second_rate: Some(avoidable / hours.max(1e-9)),
        files: n,
        files_with_part: n,
        ..RateRow::default()
    };
    let mut clients: Vec<((String, String), ColdSums)> = by.into_iter().filter(|(_, v)| v.0 > 0.0).collect();
    clients.sort_by(|a, b| b.1.0.total_cmp(&a.1.0).then_with(|| a.0.1.cmp(&b.0.1)));
    let (mut rows, mut rest) = (Vec::new(), (0, (0.0, 0.0, 0.0, 0)));
    for ((agent, client), v) in clients {
        if rows.len() < TOP_ROWS && v.3 >= min_files.max(1) {
            rows.push(row(Some(agent), client, v));
        } else {
            let r = &mut rest.1;
            (r.0, r.1, r.2, r.3) = (r.0 + v.0, r.1 + v.1, r.2 + v.2, r.3.max(v.3));
            rest.0 += 1;
        }
    }
    if rest.0 > 0 {
        rows.push(row(None, format!("{} more", rest.0), rest.1));
    }
    rows
}

/// The files where some client has this part.
fn with_client_part(files: &[Value], key: &str) -> usize {
    files.iter().filter(|f| agents(f).any(|(_, a)| entries(&a["clients"]).any(|(_, c)| c.get(key).is_some()))).count()
}

/// A client's active hours, cold returns, their avoidable dollars, and the files with it.
type ColdSums = (f64, f64, f64, usize);

/// A client's dollars at the highest effort and in all, then its requests the same way.
type EffortSums = (f64, f64, f64, f64);

/// The share of the pooled spend a client needs for a row of its own in the effort table.
const MIN_CLIENT_SHARE: f64 = 0.01;

/// The share of each client's spend and requests at `max` or `xhigh`, costliest first: the clients
/// with [`MIN_CLIENT_SHARE`] of the pooled spend or more, at most [`TOP_ROWS`], and the rest folded
/// into how many they are and their cost.
fn effort_by_client(files: &[Value], min_files: usize, spend: f64) -> (Vec<EffortRow>, Option<(usize, f64)>) {
    let mut by: BTreeMap<(String, String), Vec<Option<EffortSums>>> = BTreeMap::new();
    for (i, f) in files.iter().enumerate() {
        for (agent, a) in agents(f) {
            for (client, c) in entries(&a["clients"]) {
                let Some(levels) = c.get("effort").and_then(Value::as_object) else { continue };
                let (mut high_usd, mut usd, mut high_req, mut req) = (0.0, 0.0, 0.0, 0.0);
                for (level, l) in levels {
                    let (u, r) = (l["usd"].as_f64().unwrap_or(0.0), l["requests"].as_f64().unwrap_or(0.0));
                    usd += u;
                    req += r;
                    if level == "max" || level == "xhigh" {
                        high_usd += u;
                        high_req += r;
                    }
                }
                let e = by.entry((agent.clone(), client.clone())).or_insert_with(|| vec![None; files.len()]);
                e[i] = Some((high_usd, usd, high_req, req));
            }
        }
    }
    let mut rows: Vec<EffortRow> = by
        .into_iter()
        .map(|((agent, client), per)| {
            let sum = |k: usize| per.iter().flatten().map(|t| [t.0, t.1, t.2, t.3][k]).sum::<f64>();
            let by_file: Vec<Option<f64>> = per.iter().map(|t| t.map(|t| t.0 / t.1.max(1e-9))).collect();
            EffortRow {
                agent,
                client,
                usd: sum(1),
                spend_share: sum(0) / sum(1).max(1e-9),
                request_share: sum(2) / sum(3).max(1e-9),
                median_file: median_of(by_file.iter().flatten().copied()),
                files: per.iter().flatten().count(),
                by_file,
            }
        })
        .collect();
    rows.sort_by(|a, b| b.usd.total_cmp(&a.usd).then_with(|| a.client.cmp(&b.client)));
    let (mut shown, mut rest) = (Vec::new(), (0, 0.0));
    for r in rows {
        if shown.len() < TOP_ROWS && r.files >= min_files.max(1) && r.usd >= MIN_CLIENT_SHARE * spend {
            shown.push(r);
        } else {
            rest.0 += 1;
            rest.1 += r.usd;
        }
    }
    (shown, (rest.0 > 0).then_some(rest))
}

/// The models some file has no price for, the most tokens first. `--min-files` leaves out a model
/// that fewer files have, as it does a version.
fn unpriced(files: &[Value], min_files: usize) -> Vec<Unpriced> {
    let mut by: BTreeMap<(String, String), (f64, usize)> = BTreeMap::new();
    for f in files {
        for (agent, a) in agents(f) {
            for (model, m) in entries(&a["spend"]) {
                let tokens = m.pointer("/total/tokens").and_then(Value::as_f64).unwrap_or(0.0);
                if m.get("priced").and_then(Value::as_bool) == Some(false) && tokens > 0.0 {
                    let e = by.entry((agent.clone(), model.clone())).or_default();
                    e.0 += tokens;
                    e.1 += 1;
                }
            }
        }
    }
    let mut out: Vec<Unpriced> =
        by.into_iter().filter(|(_, (_, n))| *n >= min_files.max(1)).map(|((agent, model), (tokens, files))| Unpriced { agent, model, tokens, files }).collect();
    out.sort_by(|a, b| b.tokens.total_cmp(&a.tokens).then_with(|| a.model.cmp(&b.model)));
    out
}

/// The share of spend whose prompts the logs hold, over the files with the part.
fn coverage(files: &[Value]) -> Option<Coverage> {
    let per: Vec<(f64, f64)> = files.iter().filter_map(|f| Some((summed(f, "/prompts/held/usd")?, summed(f, "/prompts/all/usd")?))).collect();
    if per.is_empty() {
        return None;
    }
    let (held, all) = per.iter().fold((0.0, 0.0), |a, p| (a.0 + p.0, a.1 + p.1));
    Some(Coverage { share: held / all.max(1e-9), files: per.len(), files_below_90: per.iter().filter(|p| p.0 / p.1.max(1e-9) < 0.9).count() })
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
    fn a_cache_lifetime_saves_the_difference_of_the_replays_from_the_one_in_use() {
        // Both buckets wrote mostly with the 1-hour lifetime, as the author's logs do.
        let parts = json!({
            "cache_writes": {"main": {"short": 17_167, "long": 229_060_448}, "subagents": {"short": 303_857, "long": 157_574_065}},
            "whatif": {
                "main": {"requests": 33_432, "actual_usd": 4_476.0, "simulated_5m_usd": 5_365.0, "simulated_1h_usd": 4_259.0},
                "subagents": {"requests": 39_519, "actual_usd": 2_820.0, "simulated_5m_usd": 2_479.0, "simulated_1h_usd": 2_884.0},
            },
        });
        let p = build(&named(vec![file(10_000.0, parts.clone())]), 1);
        assert_eq!(row(&p, "saved", "cache lifetime: main").usd, 0.0, "already on the cheaper lifetime; the rest is other misses");
        assert_eq!(row(&p, "saved", "cache lifetime: subagents").usd, 405.0);
        // On the 5-minute lifetime, the main conversation would have saved the difference.
        let mut short = parts;
        short["cache_writes"]["main"] = json!({"short": 229_060_448, "long": 0});
        let p = build(&named(vec![file(10_000.0, short)]), 1);
        assert_eq!(row(&p, "saved", "cache lifetime: main").usd, 1_106.0);
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
