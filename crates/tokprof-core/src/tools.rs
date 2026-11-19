//! Spend by tool: how often each tool and shell command was called, how big its results and
//! inputs were, what keeping them in the context cost (rent), what they made the agent do
//! (trips), and a fixed hint on the habit to change.

use std::collections::{BTreeSet, HashMap};

use crate::analysis::{Fact, MissEvent, RequestRecord, Sink};
use crate::model::{BlockKind, ChainKind, Class, HandoffKind, Outcome, Session, ToolClass};
use crate::profile::Values;
use crate::trigger::{VcsActions, read_only_command};

/// A block the first time a request held it: its tool, size and class.
pub struct BlockEntry<'a> {
    pub kind: BlockKind,
    pub tool: Option<&'a str>,
    pub target: Option<&'a str>,
    pub class: Class,
    pub tokens: f64,
    /// The call record of the block's call in its session, when the source gives one.
    pub call: Option<u32>,
}

/// A share of a request's spend charged to a tool: the input of a request to the first-time
/// tool results it carried, the output of a response to the calls it made. `tool` is `None`
/// for spend no tool caused. `call` is set on the output shares of calls, with whether the call
/// was alone in its response.
pub struct ToolShare<'a> {
    pub tool: Option<&'a str>,
    pub target: Option<&'a str>,
    pub class: Class,
    pub tokens: f64,
    pub usd: f64,
    pub call: Option<bool>,
    /// For the share of a call: how the call ended, and for a failure the line that tells why.
    pub outcome: Outcome,
    pub error: Option<&'a str>,
    /// For the share of a call: what its shell command did to version control.
    pub vcs: VcsActions,
    /// For the share of a call: how long it took, and whether that counts for times (see
    /// tool-breakdown, Calls that count for times).
    pub duration_ms: Option<i64>,
    pub timed: bool,
}

/// Where a tool comes from (see tool-breakdown, Groups).
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Group {
    Files,
    Search,
    Shell,
    Web,
    Subagents,
    /// The tools with which the agent manages itself.
    Agent,
    /// The tools of one MCP server.
    Mcp(String),
    Other,
}

/// The groups a filter can name, as a usage error lists them.
pub const GROUP_NAMES: &str = "files, search, shell, web, subagents, agent, mcp, mcp <server>, other";

impl Group {
    /// The group of a tool: its MCP server when the target is `mcp:<server>`, else its class's.
    pub fn of(class: Class, target: Option<&str>) -> Group {
        Group::of_parts(class, target.and_then(|t| t.strip_prefix("mcp:")))
    }

    /// The group of a tool of this class and, for an MCP tool, this server.
    pub fn of_parts(class: Class, server: Option<&str>) -> Group {
        if let Some(server) = server {
            return Group::Mcp(server.to_string());
        }
        match class {
            Class::Tool(ToolClass::Read | ToolClass::Edit | ToolClass::Write | ToolClass::Delete | ToolClass::Move) => Group::Files,
            Class::Tool(ToolClass::Search) => Group::Search,
            Class::Tool(ToolClass::Execute | ToolClass::ExecuteOutput) => Group::Shell,
            Class::Tool(ToolClass::Fetch) => Group::Web,
            Class::Tool(ToolClass::Subagent) => Group::Subagents,
            Class::Tool(ToolClass::Think | ToolClass::SwitchMode | ToolClass::Manage) => Group::Agent,
            _ => Group::Other,
        }
    }

    pub fn name(&self) -> String {
        match self {
            Group::Files => "files".into(),
            Group::Search => "search".into(),
            Group::Shell => "shell".into(),
            Group::Web => "web".into(),
            Group::Subagents => "subagents".into(),
            Group::Agent => "agent".into(),
            Group::Mcp(server) => format!("mcp {server}"),
            Group::Other => "other".into(),
        }
    }
}

/// A filter by group, as `--group` names it: one group, or every MCP server with `mcp`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GroupFilter {
    One(Group),
    AnyMcp,
}

impl GroupFilter {
    /// The filter a name gives, without case; `None` for a name that is no group.
    pub fn parse(name: &str) -> Option<GroupFilter> {
        let name = name.trim().to_lowercase();
        Some(match name.as_str() {
            "files" => GroupFilter::One(Group::Files),
            "search" => GroupFilter::One(Group::Search),
            "shell" => GroupFilter::One(Group::Shell),
            "web" => GroupFilter::One(Group::Web),
            "subagents" => GroupFilter::One(Group::Subagents),
            "agent" => GroupFilter::One(Group::Agent),
            "other" => GroupFilter::One(Group::Other),
            "mcp" => GroupFilter::AnyMcp,
            _ => GroupFilter::One(Group::Mcp(name.strip_prefix("mcp ").or(name.strip_prefix("mcp:"))?.trim().to_string())),
        })
    }

    pub fn keeps(&self, g: &Group) -> bool {
        match self {
            GroupFilter::One(one) => match (one, g) {
                (Group::Mcp(a), Group::Mcp(b)) => a.eq_ignore_ascii_case(b),
                _ => one == g,
            },
            GroupFilter::AnyMcp => matches!(g, Group::Mcp(_)),
        }
    }
}

/// The median of durations in ms: the middle one in order, or the mean of the two middle ones;
/// `None` for none.
pub fn median_ms(durations: &[u32]) -> Option<f64> {
    if durations.is_empty() {
        return None;
    }
    let mut v = durations.to_vec();
    let mid = v.len() / 2;
    let (_, &mut upper, _) = v.select_nth_unstable(mid);
    if v.len() % 2 == 1 {
        return Some(upper as f64);
    }
    let lower = *v[..mid].iter().max().unwrap_or(&upper);
    Some((lower as f64 + upper as f64) / 2.0)
}

/// A duration as the tool table keeps it: whole ms, at most what 32 bits hold.
fn kept_ms(ms: i64) -> u32 {
    ms.clamp(0, u32::MAX as i64) as u32
}

/// A row's key: the tool, and the command head for shell tools.
pub type ToolKey = (String, Option<String>);

/// The row key of a tool block, or `None` for a block no tool produced.
pub fn key_of(tool: Option<&str>, target: Option<&str>, class: Class) -> Option<ToolKey> {
    let tool = tool?;
    let shell = matches!(class, Class::Tool(ToolClass::Execute | ToolClass::ExecuteOutput));
    Some((tool.to_string(), if shell { target.map(str::to_string) } else { None }))
}

#[derive(Clone, Debug, Default)]
pub struct ToolRow {
    pub class: Class,
    pub calls: u64,
    pub single: u64,
    pub results: u64,
    pub result_tokens: f64,
    pub inputs: u64,
    pub input_tokens: f64,
    pub rent: Values,
    pub trips: Values,
    /// Calls that failed (see tool-breakdown, Failed, refused and denied calls).
    pub failed: u64,
    /// The spend of the requests after its failures, until a call of the row worked again.
    pub after: Values,
    /// The line of its latest failure, with the time of the request that made the call.
    pub latest_error: Option<(i64, String)>,
    /// The MCP server of its tool, from the first `mcp:<server>` target seen.
    pub mcp: Option<String>,
    /// The durations of its calls that count for times, in ms, in no order, and their sum.
    pub durations: Vec<u32>,
    pub time_ms: u64,
}

impl ToolRow {
    pub fn add(&mut self, o: &ToolRow) {
        if self.class == Class::None {
            self.class = o.class;
        }
        if self.mcp.is_none() {
            self.mcp.clone_from(&o.mcp);
        }
        self.durations.extend_from_slice(&o.durations);
        self.time_ms += o.time_ms;
        self.calls += o.calls;
        self.single += o.single;
        self.results += o.results;
        self.result_tokens += o.result_tokens;
        self.inputs += o.inputs;
        self.input_tokens += o.input_tokens;
        self.rent.add(o.rent);
        self.trips.add(o.trips);
        self.failed += o.failed;
        self.after.add(o.after);
        if let Some(e) = &o.latest_error
            && self.latest_error.as_ref().is_none_or(|l| e.0 >= l.0)
        {
            self.latest_error = Some(e.clone());
        }
    }

    /// The share of its calls that failed.
    pub fn failed_share(&self) -> f64 {
        if self.calls == 0 { 0.0 } else { self.failed as f64 / self.calls as f64 }
    }

    /// Mean tokens of a result, `None` when the log rebuilt none.
    pub fn avg_result(&self) -> Option<f64> {
        (self.results > 0).then(|| self.result_tokens / self.results as f64)
    }

    pub fn avg_input(&self) -> Option<f64> {
        (self.inputs > 0).then(|| self.input_tokens / self.inputs as f64)
    }

    /// Where its tool comes from.
    pub fn group(&self) -> Group {
        Group::of_parts(self.class, self.mcp.as_deref())
    }

    /// The median duration of its calls that count for times, in ms.
    pub fn median_ms(&self) -> Option<f64> {
        median_ms(&self.durations)
    }

    /// The share of calls that were alone in their response.
    pub fn single_share(&self) -> f64 {
        if self.calls == 0 { 0.0 } else { self.single as f64 / self.calls as f64 }
    }
}

/// Hints apply only to rows with this many calls and this much rent or trips.
pub const HINT_MIN_CALLS: u64 = 5;
pub const HINT_MIN_USD: f64 = 1.0;
const BATCH_SINGLE: f64 = 0.6;
const LONG_SCRIPT_TOKENS: f64 = 400.0;
const LARGE_READ_TOKENS: f64 = 5_000.0;
const LONG_OUTPUT_TOKENS: f64 = 2_000.0;
const LARGE_WRITE_TOKENS: f64 = 2_000.0;

/// Every hint a row can get, in the order of the rules.
pub const HINTS: [&str; 5] = [
    "batch independent reads",
    "save long scripts to a file; edit files with the edit tool",
    "read a range or search instead of the whole file",
    "filter the output with head or grep",
    "edit instead of rewriting the file",
];

/// The first hint whose rule applies to a row, or `None`.
pub fn hint(key: &ToolKey, row: &ToolRow) -> Option<&'static str> {
    if row.calls < HINT_MIN_CALLS || row.rent.usd.max(row.trips.usd) < HINT_MIN_USD {
        return None;
    }
    let class = match row.class {
        Class::Tool(c) => c,
        _ => ToolClass::Other,
    };
    let shell = matches!(class, ToolClass::Execute | ToolClass::ExecuteOutput);
    let read_only = matches!(class, ToolClass::Read | ToolClass::Search | ToolClass::Fetch)
        || (class == ToolClass::Execute && key.1.as_deref().is_some_and(read_only_command));
    let (result, input) = (row.avg_result().unwrap_or(0.0), row.avg_input().unwrap_or(0.0));
    if read_only && row.single_share() >= BATCH_SINGLE {
        Some("batch independent reads")
    } else if shell && !read_only && input >= LONG_SCRIPT_TOKENS {
        Some("save long scripts to a file; edit files with the edit tool")
    } else if class == ToolClass::Read && result >= LARGE_READ_TOKENS {
        Some("read a range or search instead of the whole file")
    } else if shell && result >= LONG_OUTPUT_TOKENS {
        Some("filter the output with head or grep")
    } else if class == ToolClass::Write && input >= LARGE_WRITE_TOKENS {
        Some("edit instead of rewriting the file")
    } else {
        None
    }
}

/// A row of the groups table: the rows of one group added up.
#[derive(Clone, Debug, Default)]
pub struct GroupRow {
    /// `None` for the `all` row.
    pub group: Option<Group>,
    pub calls: u64,
    pub failed: u64,
    /// The durations of its calls that count for times, and their sum (see `ToolRow`).
    pub durations: Vec<u32>,
    pub time_ms: u64,
    pub result_tokens: f64,
    /// A server the logs list that got no call.
    pub never_called: bool,
}

impl GroupRow {
    pub fn median_ms(&self) -> Option<f64> {
        median_ms(&self.durations)
    }

    /// The group's name, `all` for the totals.
    pub fn name(&self) -> String {
        self.group.as_ref().map_or_else(|| "all".to_string(), Group::name)
    }

    fn add(&mut self, r: &ToolRow) {
        self.calls += r.calls;
        self.failed += r.failed;
        self.durations.extend_from_slice(&r.durations);
        self.time_ms += r.time_ms;
        self.result_tokens += r.result_tokens;
    }
}

/// Spend by tool, with the spend no tool caused, as rent and as trips.
#[derive(Clone, Debug, Default)]
pub struct ToolTable {
    pub rows: HashMap<ToolKey, ToolRow>,
    /// All the spend.
    pub spend: Values,
    /// Rent of content no tool produced.
    pub other_rent: Values,
    /// Input of requests no tool result triggered, and output of responses without calls.
    pub other_trips: Values,
    /// Calls that failed, that the user refused, and that a rule denied.
    pub failed: u64,
    pub refused: u64,
    pub denied: u64,
    /// The spend of the failure windows of all rows, each request once.
    pub after: Values,
    /// The failure windows that opened: each starts at the request that carried a failure.
    pub opened: u64,
    /// The MCP servers that the logs of sessions with a selected request list.
    pub servers: BTreeSet<String>,
    /// The servers of the session being walked, until one of its requests is selected.
    pending_servers: Vec<String>,
    /// The windows of the chain being walked.
    windows: Windows,
}

/// What to sort the tool rows by.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RowOrder {
    Rent,
    Trips,
    /// Failed calls, then the spend after them.
    Failed,
}

/// The failure windows of one chain while the analysis walks it (see tool-breakdown, What came
/// after a failure).
#[derive(Clone, Debug, Default)]
struct Windows {
    /// The session's ends of turn and interrupts, in time order.
    turn_ends: Vec<i64>,
    /// The chain of the latest request, its number in that chain, and its time.
    chain: Option<usize>,
    count: usize,
    latest: Option<(usize, i64, Values)>,
    /// Rows whose call failed in the latest response: their window starts at the next request.
    opening: Vec<ToolKey>,
    /// Open windows by row: the requests after the first, held back until a call of the row works.
    open: HashMap<ToolKey, Vec<(usize, Values)>>,
    /// Requests of the chain that the table's total counts already.
    counted: std::collections::HashSet<usize>,
}

impl ToolTable {
    pub fn merge(&mut self, o: ToolTable) {
        for (k, r) in o.rows {
            self.rows.entry(k).or_default().add(&r);
        }
        self.spend.add(o.spend);
        self.other_rent.add(o.other_rent);
        self.other_trips.add(o.other_trips);
        self.failed += o.failed;
        self.refused += o.refused;
        self.denied += o.denied;
        self.after.add(o.after);
        self.opened += o.opened;
        self.servers.extend(o.servers);
    }

    /// The groups table: one row per group with a call, most calls first and `other` last, then
    /// the servers never called; and the `all` row.
    pub fn groups(&self) -> (Vec<GroupRow>, GroupRow) {
        let mut by: HashMap<Group, GroupRow> = HashMap::new();
        let mut all = GroupRow::default();
        for r in self.rows.values().filter(|r| r.calls > 0) {
            let g = r.group();
            by.entry(g.clone()).or_insert_with(|| GroupRow { group: Some(g), ..GroupRow::default() }).add(r);
            all.add(r);
        }
        let mut rows: Vec<GroupRow> = by.into_values().collect();
        rows.sort_by(|a, b| {
            let other = |r: &GroupRow| r.group == Some(Group::Other);
            other(a).cmp(&other(b)).then(b.calls.cmp(&a.calls)).then(a.group.cmp(&b.group))
        });
        for server in &self.servers {
            let g = Group::Mcp(server.clone());
            if !rows.iter().any(|r| r.group.as_ref() == Some(&g)) {
                rows.push(GroupRow { group: Some(g), never_called: true, ..GroupRow::default() });
            }
        }
        (rows, all)
    }

    /// Rows, by rent or by trips, most first: dollars when priced, tokens otherwise.
    pub fn sorted(&self, by_trips: bool) -> Vec<(&ToolKey, &ToolRow)> {
        self.sorted_by(if by_trips { RowOrder::Trips } else { RowOrder::Rent })
    }

    /// Rows in the given order, most first: dollars when priced, tokens otherwise.
    pub fn sorted_by(&self, order: RowOrder) -> Vec<(&ToolKey, &ToolRow)> {
        let priced = self.spend.usd > 0.0;
        let amount = |v: Values| if priced { v.usd } else { v.tokens };
        let key = |r: &ToolRow| match order {
            RowOrder::Rent => (0, amount(r.rent)),
            RowOrder::Trips => (0, amount(r.trips)),
            RowOrder::Failed => (r.failed, amount(r.after)),
        };
        let mut rows: Vec<(&ToolKey, &ToolRow)> = self.rows.iter().filter(|(_, r)| r.calls > 0 || r.rent.tokens > 0.0).collect();
        rows.sort_by(|a, b| {
            let (ka, kb) = (key(a.1), key(b.1));
            kb.0.cmp(&ka.0).then(kb.1.total_cmp(&ka.1)).then(a.0.cmp(b.0))
        });
        rows
    }

    /// The rows whose calls failed, most spend after their failures first, then most failures.
    pub fn failing(&self) -> Vec<(&ToolKey, &ToolRow)> {
        let priced = self.spend.usd > 0.0;
        let amount = |v: Values| if priced { v.usd } else { v.tokens };
        let mut rows: Vec<(&ToolKey, &ToolRow)> = self.rows.iter().filter(|(_, r)| r.failed > 0).collect();
        rows.sort_by(|a, b| amount(b.1.after).total_cmp(&amount(a.1.after)).then(b.1.failed.cmp(&a.1.failed)).then(a.0.cmp(b.0)));
        rows
    }

    /// Adds a request of a failure window to its row, and to the total once.
    fn commit(&mut self, key: &ToolKey, (index, cost): (usize, Values)) {
        if let Some(r) = self.rows.get_mut(key) {
            r.after.add(cost);
        }
        if self.windows.counted.insert(index) {
            self.after.add(cost);
        }
    }

    fn row(&mut self, key: ToolKey, class: Class, target: Option<&str>) -> &mut ToolRow {
        let r = self.rows.entry(key).or_default();
        if r.class == Class::None {
            r.class = class;
        }
        if r.mcp.is_none()
            && let Some(server) = target.and_then(|t| t.strip_prefix("mcp:"))
        {
            r.mcp = Some(server.to_string());
        }
        r
    }
}

impl Sink for ToolTable {
    fn session(&mut self, s: &Session) {
        let mut turn_ends: Vec<i64> = s.handoffs.iter().filter(|h| matches!(h.kind, HandoffKind::TurnEnd | HandoffKind::Interrupt)).map(|h| h.ts_ms).collect();
        turn_ends.sort_unstable();
        self.windows = Windows { turn_ends, ..Windows::default() };
        self.pending_servers = s.mcp_servers.clone();
    }

    fn request(&mut self, r: &RequestRecord) {
        if !self.pending_servers.is_empty() {
            self.servers.extend(std::mem::take(&mut self.pending_servers));
        }
        let w = &mut self.windows;
        let main = matches!(r.chain_kind, ChainKind::Main | ChainKind::Advisor);
        // A new chain, or a new turn of a main conversation, closes the open windows: a failure
        // never worked past it, so its window keeps its first request only.
        let ended =
            w.latest.is_some_and(|(_, prev, _)| main && w.turn_ends[w.turn_ends.partition_point(|&t| t < prev)..].first().is_some_and(|&t| t < r.ts_ms));
        if w.chain != Some(r.chain_index) || ended {
            if w.chain != Some(r.chain_index) {
                w.counted.clear();
                w.count = 0;
            }
            w.chain = Some(r.chain_index);
            w.open.clear();
            w.opening.clear();
        }
        let usd = r.price.map_or(0.0, |p| p.cost(&r.usage));
        let cost = Values { tokens: (r.usage.prompt() + r.usage.output) as f64, usd, avoidable: 0.0 };
        let index = w.count;
        w.count += 1;
        w.latest = Some((index, r.ts_ms, cost));
        for held in w.open.values_mut() {
            held.push((index, cost));
        }
        // The request that carried a failure opens the row's window, unless one is open already.
        let opening = std::mem::take(&mut w.opening);
        for key in opening {
            if !self.windows.open.contains_key(&key) {
                self.windows.open.insert(key.clone(), Vec::new());
                self.opened += 1;
                self.commit(&key, (index, cost));
            }
        }
    }

    fn miss(&mut self, _: MissEvent) {}

    fn fact(&mut self, f: &Fact) {
        let v = Values { tokens: f.tokens, usd: f.usd.unwrap_or(0.0), avoidable: f.avoidable_usd };
        self.spend.add(v);
        let tool_block = matches!(f.kind, BlockKind::ToolUse | BlockKind::ToolResult);
        match key_of(f.tool, f.target, f.class).filter(|_| tool_block) {
            Some(k) => self.row(k, f.class, f.target).rent.add(v),
            None => self.other_rent.add(v),
        }
    }

    fn block_entered(&mut self, b: &BlockEntry) {
        let Some(k) = key_of(b.tool, b.target, b.class) else { return };
        let r = self.row(k, b.class, b.target);
        match b.kind {
            BlockKind::ToolResult => {
                r.results += 1;
                r.result_tokens += b.tokens;
            }
            BlockKind::ToolUse => {
                r.inputs += 1;
                r.input_tokens += b.tokens;
            }
            _ => {}
        }
    }

    fn tool_share(&mut self, s: &ToolShare) {
        let v = Values { tokens: s.tokens, usd: s.usd, avoidable: 0.0 };
        match key_of(s.tool, s.target, s.class) {
            Some(k) => {
                let ts = self.windows.latest.map_or(0, |l| l.1);
                let r = self.row(k.clone(), s.class, s.target);
                r.trips.add(v);
                if let Some(alone) = s.call {
                    r.calls += 1;
                    if let Some(ms) = s.duration_ms.filter(|_| s.timed) {
                        r.durations.push(kept_ms(ms));
                        r.time_ms += kept_ms(ms) as u64;
                    }
                    r.single += u64::from(alone);
                    if s.outcome == Outcome::Failed {
                        r.failed += 1;
                        if let Some(e) = s.error.filter(|_| r.latest_error.as_ref().is_none_or(|l| ts >= l.0)) {
                            r.latest_error = Some((ts, e.to_string()));
                        }
                    }
                }
                if s.call.is_some() {
                    self.call_ended(k, s.class, s.outcome);
                }
            }
            None => self.other_trips.add(v),
        }
    }
}

impl ToolTable {
    /// A call of the latest response ended: a failure opens its row's window at the next request,
    /// and a call that worked closes the open windows of its tool's rows with all their requests;
    /// any shell call that worked closes those of every shell row.
    fn call_ended(&mut self, key: ToolKey, class: Class, outcome: Outcome) {
        match outcome {
            Outcome::Failed => {
                self.failed += 1;
                if !self.windows.opening.contains(&key) {
                    self.windows.opening.push(key);
                }
            }
            Outcome::Refused => self.refused += 1,
            Outcome::Denied => self.denied += 1,
            _ => {
                let shell = |c: Class| matches!(c, Class::Tool(ToolClass::Execute | ToolClass::ExecuteOutput));
                let closes = |k: &ToolKey| k.0 == key.0 || (shell(class) && self.rows.get(k).is_some_and(|r| shell(r.class)));
                let rows: Vec<ToolKey> = self.windows.open.keys().filter(|k| closes(k)).cloned().collect();
                for row in rows {
                    for request in self.windows.open.remove(&row).unwrap_or_default() {
                        self.commit(&row, request);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(class: ToolClass, calls: u64, single: u64, result: f64, input: f64, rent: f64) -> ToolRow {
        ToolRow {
            class: Class::Tool(class),
            calls,
            single,
            results: calls,
            result_tokens: result * calls as f64,
            inputs: calls,
            input_tokens: input * calls as f64,
            rent: Values { tokens: 1.0, usd: rent, avoidable: 0.0 },
            ..ToolRow::default()
        }
    }

    fn key(tool: &str, head: Option<&str>) -> ToolKey {
        (tool.to_string(), head.map(str::to_string))
    }

    #[test]
    fn each_rule_gives_its_hint() {
        let sed = key("Bash", Some("sed -n"));
        assert_eq!(hint(&sed, &row(ToolClass::Execute, 56, 48, 1_300.0, 53.0, 5.21)), Some("batch independent reads"));
        let py = key("Bash", Some("python3 -I"));
        assert_eq!(hint(&py, &row(ToolClass::Execute, 60, 10, 120.0, 702.0, 4.32)), Some("save long scripts to a file; edit files with the edit tool"));
        let read = key("Read", None);
        assert_eq!(hint(&read, &row(ToolClass::Read, 8, 2, 9_000.0, 20.0, 3.0)), Some("read a range or search instead of the whole file"));
        let test = key("Bash", Some("cargo test"));
        assert_eq!(hint(&test, &row(ToolClass::Execute, 6, 6, 2_500.0, 30.0, 1.5)), Some("filter the output with head or grep"));
        let write = key("Write", None);
        assert_eq!(hint(&write, &row(ToolClass::Write, 16, 16, 40.0, 2_147.0, 3.75)), Some("edit instead of rewriting the file"));
    }

    #[test]
    fn small_rows_get_no_hint() {
        let read = key("Read", None);
        assert_eq!(hint(&read, &row(ToolClass::Read, 1, 1, 16_900.0, 20.0, 1.16)), None, "a large read, once");
        let sed = key("Bash", Some("sed -n"));
        assert_eq!(hint(&sed, &row(ToolClass::Execute, 4, 4, 1_300.0, 50.0, 3.0)), None, "four calls");
        assert_eq!(hint(&sed, &row(ToolClass::Execute, 50, 50, 1_300.0, 50.0, 0.5)), None, "under $1");
    }

    #[test]
    fn shell_rows_split_by_command_head_and_others_do_not() {
        let shell = Class::Tool(ToolClass::Execute);
        assert_eq!(key_of(Some("Bash"), Some("sed -n"), shell), Some(key("Bash", Some("sed -n"))));
        assert_eq!(key_of(Some("Read"), Some("src/a.rs"), Class::Tool(ToolClass::Read)), Some(key("Read", None)));
        assert_eq!(key_of(None, None, Class::None), None);
    }

    #[test]
    fn tables_merge() {
        let mut a = ToolTable::default();
        a.rows.insert(key("Read", None), row(ToolClass::Read, 2, 1, 100.0, 10.0, 1.0));
        let mut b = ToolTable::default();
        b.rows.insert(key("Read", None), row(ToolClass::Read, 3, 3, 100.0, 10.0, 2.0));
        a.merge(b);
        let r = &a.rows[&key("Read", None)];
        assert_eq!((r.calls, r.single, r.rent.usd), (5, 4, 3.0));
    }

    /// Feeds the table as the analysis does: a session, then per request its record and the
    /// shares of the calls its response made. Requests have no price, so spend is in tokens.
    struct Walk {
        table: ToolTable,
        agent: crate::source::AgentId,
    }

    impl Walk {
        fn new(turn_ends: &[i64]) -> Walk {
            let mut s = Session::new(crate::testing::claude(), "s".into());
            for &t in turn_ends {
                s.push_handoff(t, HandoffKind::TurnEnd);
            }
            let mut table = ToolTable::default();
            table.session(&s);
            Walk { table, agent: crate::testing::claude() }
        }

        /// A request of chain `chain` at `ts` whose prompt holds `tokens`.
        fn request(&mut self, chain: usize, ts: i64, tokens: u64) -> &mut Walk {
            let r = RequestRecord {
                agent: &self.agent,
                ts_ms: ts,
                chain: "main",
                chain_index: chain,
                api: crate::api::ApiId::Anthropic,
                chain_kind: ChainKind::Main,
                advisor: false,
                origin: "prompt",
                trigger: crate::trigger::Trigger::Prompt,
                activity: "prompt",
                task: "",
                model: "m",
                price: None,
                usage: crate::model::Usage { cache_read: tokens, ..crate::model::Usage::default() },
                gap_s: None,
                prev_prompt: None,
                composition: true,
                client: "cli",
                effort: None,
                version: None,
            };
            self.table.request(&r);
            self
        }

        /// A call of the latest response, by tool and command head.
        fn call(&mut self, tool: &str, head: Option<&str>, outcome: Outcome) -> &mut Walk {
            let class = Class::Tool(if head.is_some() { ToolClass::Execute } else { ToolClass::Edit });
            self.table.tool_share(&ToolShare {
                tool: Some(tool),
                target: head,
                class,
                tokens: 0.0,
                usd: 0.0,
                call: Some(true),
                outcome,
                error: Some("why"),
                vcs: VcsActions::default(),
                duration_ms: None,
                timed: false,
            });
            self
        }

        fn after(&self, tool: &str, head: Option<&str>) -> (u64, f64) {
            let r = &self.table.rows[&key(tool, head)];
            (r.failed, r.after.tokens)
        }
    }

    #[test]
    fn failed_refused_and_denied_calls_are_counted_apart() {
        let mut w = Walk::new(&[]);
        w.request(0, 1, 100).call("Read", None, Outcome::Failed).call("Edit", None, Outcome::Refused).call("Bash", Some("rm -rf"), Outcome::Denied);
        assert_eq!(w.after("Read", None).0, 1);
        assert_eq!(w.after("Edit", None).0, 0, "a refused call is not a failure");
        assert_eq!((w.table.failed, w.table.refused, w.table.denied), (1, 1, 1));
        assert_eq!(w.table.rows[&key("Read", None)].latest_error.as_ref().map(|e| e.1.as_str()), Some("why"));
    }

    #[test]
    fn read_again_then_edited() {
        let mut w = Walk::new(&[]);
        w.request(0, 1, 1).call("Edit", None, Outcome::Failed);
        w.request(0, 2, 10).call("Read", None, Outcome::Worked);
        w.request(0, 3, 100).call("Edit", None, Outcome::Worked);
        w.request(0, 4, 1_000);
        assert_eq!(w.after("Edit", None), (1, 110.0), "the request that read the file and the one that edited");
        assert_eq!(w.table.after.tokens, 110.0);
    }

    #[test]
    fn tried_again_at_once_and_two_failures_in_a_row() {
        let mut w = Walk::new(&[]);
        w.request(0, 1, 1).call("Bash", Some("cargo test"), Outcome::Failed);
        w.request(0, 2, 10).call("Bash", Some("cargo test"), Outcome::Worked);
        assert_eq!(w.after("Bash", Some("cargo test")), (1, 10.0), "the one request that carried the failure");
        let mut w = Walk::new(&[]);
        w.request(0, 1, 1).call("Bash", Some("git push"), Outcome::Failed);
        w.request(0, 2, 10).call("Bash", Some("git push"), Outcome::Failed);
        w.request(0, 3, 100).call("Bash", Some("git push"), Outcome::Worked);
        assert_eq!(w.after("Bash", Some("git push")), (2, 110.0), "one window for both failures");
    }

    #[test]
    fn a_failure_that_never_worked_again_keeps_its_first_request() {
        // The turn ends at 5; the call of the row works only in the next turn.
        let mut w = Walk::new(&[5]);
        w.request(0, 1, 1).call("Edit", None, Outcome::Failed);
        w.request(0, 2, 10);
        w.request(0, 3, 100);
        w.request(0, 6, 1_000).call("Edit", None, Outcome::Worked);
        assert_eq!(w.after("Edit", None), (1, 10.0));
        // Another chain ends the window as well.
        let mut w = Walk::new(&[]);
        w.request(0, 1, 1).call("Edit", None, Outcome::Failed);
        w.request(0, 2, 10);
        w.request(1, 3, 100).call("Edit", None, Outcome::Worked);
        assert_eq!(w.after("Edit", None), (1, 10.0));
    }

    #[test]
    fn a_shell_call_of_another_tool_ends_the_window() {
        let mut w = Walk::new(&[]);
        w.request(0, 1, 1).call("write_stdin", Some("poll"), Outcome::Failed);
        w.request(0, 2, 10).call("exec_command", Some("cargo test"), Outcome::Worked);
        w.request(0, 3, 100).call("write_stdin", Some("poll"), Outcome::Worked);
        assert_eq!(w.after("write_stdin", Some("poll")), (1, 10.0), "Codex polls a command with write_stdin and runs one with exec_command");
    }

    #[test]
    fn another_command_of_the_shell_ends_the_window() {
        let mut w = Walk::new(&[]);
        w.request(0, 1, 1).call("Bash", Some("cargo test"), Outcome::Failed);
        w.request(0, 2, 10).call("Bash", Some("git diff"), Outcome::Worked);
        w.request(0, 3, 100).call("Bash", Some("cargo test"), Outcome::Worked);
        assert_eq!(w.after("Bash", Some("cargo test")), (1, 10.0), "git diff worked, so the shell worked again");
    }

    #[test]
    fn the_windows_of_two_rows_count_a_request_once_in_the_total() {
        let mut w = Walk::new(&[]);
        w.request(0, 1, 1).call("Edit", None, Outcome::Failed).call("Bash", Some("cargo test"), Outcome::Failed);
        w.request(0, 2, 10).call("Edit", None, Outcome::Worked).call("Bash", Some("cargo test"), Outcome::Worked);
        assert_eq!((w.after("Edit", None).1, w.after("Bash", Some("cargo test")).1, w.table.after.tokens), (10.0, 10.0, 10.0));
    }

    /// A share of one call: its tool, class, target, duration and whether it counts for times.
    fn timed_call<'a>(tool: &'a str, class: ToolClass, target: Option<&'a str>, ms: Option<i64>, timed: bool, outcome: Outcome) -> ToolShare<'a> {
        ToolShare {
            tool: Some(tool),
            target,
            class: Class::Tool(class),
            tokens: 0.0,
            usd: 0.0,
            call: Some(true),
            outcome,
            error: None,
            vcs: VcsActions::default(),
            duration_ms: ms,
            timed,
        }
    }

    #[test]
    fn a_row_sums_and_takes_the_median_of_its_counted_calls() {
        let mut t = ToolTable::default();
        for ms in [100, 200, 600] {
            t.tool_share(&timed_call("Read", ToolClass::Read, Some("src/a.rs"), Some(ms), true, Outcome::Worked));
        }
        let read = &t.rows[&key("Read", None)];
        assert_eq!((read.time_ms, read.median_ms()), (900, Some(200.0)));
        // Four timed runs and one in the background, which only counts as a call.
        let head = Some("cargo test");
        for s in [20, 30, 40, 50] {
            t.tool_share(&timed_call("Bash", ToolClass::Execute, head, Some(s * 1000), true, Outcome::Worked));
        }
        t.tool_share(&timed_call("Bash", ToolClass::Execute, head, Some(200), false, Outcome::Worked));
        let bash = &t.rows[&key("Bash", head)];
        assert_eq!((bash.calls, bash.time_ms, bash.median_ms()), (5, 140_000, Some(35_000.0)));
        // A call without a duration, or one that asked, leaves the times as they were.
        t.tool_share(&timed_call("AskUserQuestion", ToolClass::Manage, None, Some(180_000), false, Outcome::Worked));
        t.tool_share(&timed_call("WebSearch", ToolClass::Fetch, None, None, false, Outcome::Worked));
        for tool in ["AskUserQuestion", "WebSearch"] {
            let r = &t.rows[&key(tool, None)];
            assert_eq!((r.calls, r.time_ms, r.median_ms()), (1, 0, None), "{tool}");
        }
    }

    #[test]
    fn one_long_call_moves_the_time_but_not_the_median() {
        let mut t = ToolTable::default();
        for _ in 0..9 {
            t.tool_share(&timed_call("Bash", ToolClass::Execute, Some("sleep"), Some(500), true, Outcome::Worked));
        }
        t.tool_share(&timed_call("Bash", ToolClass::Execute, Some("sleep"), Some(30_960_000), true, Outcome::Worked));
        let r = &t.rows[&key("Bash", Some("sleep"))];
        assert_eq!((crate::fmt::call_time(r.time_ms as f64), r.median_ms()), ("8.6h".to_string(), Some(500.0)));
    }

    #[test]
    fn merged_rows_take_the_median_of_all_their_calls() {
        let mut a = ToolTable::default();
        for ms in [100, 100, 100] {
            a.tool_share(&timed_call("Read", ToolClass::Read, None, Some(ms), true, Outcome::Worked));
        }
        let mut b = ToolTable::default();
        for ms in [900, 1000] {
            b.tool_share(&timed_call("Read", ToolClass::Read, None, Some(ms), true, Outcome::Worked));
        }
        a.merge(b);
        let r = &a.rows[&key("Read", None)];
        // The sessions' medians are 100 and 950; all five calls' median is 100.
        assert_eq!((r.calls, r.time_ms, r.median_ms()), (5, 2_200, Some(100.0)));
        assert_eq!(median_ms(&[4, 1, 3, 2]), Some(2.5));
        assert_eq!(median_ms(&[]), None);
    }

    #[test]
    fn tools_belong_to_the_group_of_their_kind_or_server() {
        let g = |class: ToolClass, target: Option<&str>| Group::of(Class::Tool(class), target).name();
        assert_eq!(g(ToolClass::Other, Some("mcp:idea")), "mcp idea");
        let built_in = [ToolClass::Read, ToolClass::Search, ToolClass::Execute, ToolClass::Fetch, ToolClass::Subagent, ToolClass::Other];
        assert_eq!(built_in.map(|c| g(c, None)), ["files", "search", "shell", "web", "subagents", "other"]);
        let own = [ToolClass::Manage, ToolClass::Manage, ToolClass::Think, ToolClass::SwitchMode];
        assert_eq!(own.map(|c| g(c, None)), ["agent"; 4]);
        let files = [ToolClass::Edit, ToolClass::Write, ToolClass::Delete, ToolClass::Move, ToolClass::ExecuteOutput];
        assert_eq!(files.map(|c| g(c, None)), ["files", "files", "files", "files", "shell"]);
        assert_eq!(GroupFilter::parse("MCP"), Some(GroupFilter::AnyMcp));
        assert_eq!(GroupFilter::parse("mcp idea"), Some(GroupFilter::One(Group::Mcp("idea".into()))));
        assert_eq!(GroupFilter::parse("disk"), None);
        assert!(GroupFilter::AnyMcp.keeps(&Group::Mcp("idea".into())) && !GroupFilter::AnyMcp.keeps(&Group::Shell));
    }

    #[test]
    fn the_groups_table_adds_up_its_rows_and_lists_servers_never_called() {
        let mut s = Session::new(crate::testing::claude(), "s".into());
        s.mcp_servers = vec!["idea".into(), "linear".into()];
        let mut w = Walk::new(&[]);
        w.table.session(&s);
        w.request(0, 1, 1);
        let t = &mut w.table;
        for i in 0..340 {
            let outcome = if i < 12 { Outcome::Failed } else { Outcome::Worked };
            let timed = i < 300;
            let ms = if i < 150 {
                600
            } else if i < 299 {
                610
            } else {
                2_520_000 - 150 * 600 - 149 * 610
            };
            t.tool_share(&timed_call("Bash", ToolClass::Execute, Some(if i % 2 == 0 { "ls" } else { "cat" }), Some(ms), timed, outcome));
        }
        for i in 0..120 {
            t.tool_share(&timed_call("Read", ToolClass::Read, Some("a.rs"), Some(if i < 60 { 100 } else { 300 }), true, Outcome::Worked));
        }
        for i in 0..12 {
            let outcome = if i == 0 { Outcome::Failed } else { Outcome::Worked };
            t.tool_share(&timed_call("mcp__idea__find", ToolClass::Other, Some("mcp:idea"), Some(1_500), true, outcome));
        }
        t.tool_share(&timed_call("Monitor", ToolClass::Other, None, Some(1_000), true, Outcome::Worked));
        t.block_entered(&BlockEntry {
            kind: BlockKind::ToolResult,
            tool: Some("Read"),
            target: Some("a.rs"),
            class: Class::Tool(ToolClass::Read),
            tokens: 2_000.0,
            call: None,
        });
        let (rows, all) = t.groups();
        let cells: Vec<(String, u64, u64, u64, bool)> = rows.iter().map(|r| (r.name(), r.calls, r.failed, r.time_ms, r.never_called)).collect();
        assert_eq!(
            cells,
            [
                ("shell".to_string(), 340, 12, 2_520_000, false),
                ("files".to_string(), 120, 0, 24_000, false),
                ("mcp idea".to_string(), 12, 1, 18_000, false),
                ("other".to_string(), 1, 0, 1_000, false),
                ("mcp linear".to_string(), 0, 0, 0, true),
            ]
        );
        assert_eq!(rows[0].median_ms(), Some(605.0));
        assert_eq!((rows[1].median_ms(), rows[1].result_tokens), (Some(200.0), 2_000.0));
        assert_eq!((all.name(), all.calls, all.failed, all.durations.len()), ("all".to_string(), 473, 13, 433));
        // A session none of whose requests is selected lists no servers.
        let mut idle = Session::new(crate::testing::claude(), "t".into());
        idle.mcp_servers = vec!["sentry".into()];
        let mut t = ToolTable::default();
        t.session(&idle);
        assert!(t.servers.is_empty());
    }
}
