//! Spend by tool: how often each tool and shell command was called, how big its results and
//! inputs were, what keeping them in the context cost (rent), what they made the agent do
//! (trips), and a fixed hint on the habit to change.

use std::collections::HashMap;

use crate::analysis::{Fact, MissEvent, RequestRecord, Sink};
use crate::model::{BlockKind, ChainKind, Class, HandoffKind, Outcome, Session, ToolClass};
use crate::profile::Values;
use crate::trigger::read_only_command;

/// A block the first time a request held it: its tool, size and class.
pub struct BlockEntry<'a> {
    pub kind: BlockKind,
    pub tool: Option<&'a str>,
    pub target: Option<&'a str>,
    pub class: Class,
    pub tokens: f64,
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
}

impl ToolRow {
    fn add(&mut self, o: &ToolRow) {
        if self.class == Class::None {
            self.class = o.class;
        }
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
            && self.latest_error.as_ref().is_none_or(|l| e.0 >= l.0) {
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

    fn row(&mut self, key: ToolKey, class: Class) -> &mut ToolRow {
        let r = self.rows.entry(key).or_default();
        if r.class == Class::None {
            r.class = class;
        }
        r
    }
}

impl Sink for ToolTable {
    fn session(&mut self, s: &Session) {
        let mut turn_ends: Vec<i64> = s.handoffs.iter().filter(|h| matches!(h.kind, HandoffKind::TurnEnd | HandoffKind::Interrupt)).map(|h| h.ts_ms).collect();
        turn_ends.sort_unstable();
        self.windows = Windows { turn_ends, ..Windows::default() };
    }

    fn request(&mut self, r: &RequestRecord) {
        let w = &mut self.windows;
        let main = matches!(r.chain_kind, ChainKind::Main | ChainKind::Advisor);
        // A new chain, or a new turn of a main conversation, closes the open windows: a failure
        // never worked past it, so its window keeps its first request only.
        let ended = w.latest.is_some_and(|(_, prev, _)| main && w.turn_ends[w.turn_ends.partition_point(|&t| t < prev)..].first().is_some_and(|&t| t < r.ts_ms));
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
            Some(k) => self.row(k, f.class).rent.add(v),
            None => self.other_rent.add(v),
        }
    }

    fn block_entered(&mut self, b: &BlockEntry) {
        let Some(k) = key_of(b.tool, b.target, b.class) else { return };
        let r = self.row(k, b.class);
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
                let r = self.row(k.clone(), s.class);
                r.trips.add(v);
                if let Some(alone) = s.call {
                    r.calls += 1;
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
            };
            self.table.request(&r);
            self
        }

        /// A call of the latest response, by tool and command head.
        fn call(&mut self, tool: &str, head: Option<&str>, outcome: Outcome) -> &mut Walk {
            let class = Class::Tool(if head.is_some() { ToolClass::Execute } else { ToolClass::Edit });
            self.table.tool_share(&ToolShare { tool: Some(tool), target: head, class, tokens: 0.0, usd: 0.0, call: Some(true), outcome, error: Some("why") });
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
}
