//! Spend by tool: how often each tool and shell command was called, how big its results and
//! inputs were, what keeping them in the context cost (rent), what they made the agent do
//! (trips), and a fixed hint on the habit to change.

use std::collections::HashMap;

use crate::analysis::{Fact, MissEvent, RequestRecord, Sink};
use crate::model::{BlockKind, Class, Session, ToolClass};
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
}

impl ToolTable {
    pub fn merge(&mut self, o: ToolTable) {
        for (k, r) in o.rows {
            self.rows.entry(k).or_default().add(&r);
        }
        self.spend.add(o.spend);
        self.other_rent.add(o.other_rent);
        self.other_trips.add(o.other_trips);
    }

    /// Rows, by rent or by trips, most first: dollars when priced, tokens otherwise.
    pub fn sorted(&self, by_trips: bool) -> Vec<(&ToolKey, &ToolRow)> {
        let priced = self.spend.usd > 0.0;
        let key = |r: &ToolRow| {
            let v = if by_trips { r.trips } else { r.rent };
            if priced { v.usd } else { v.tokens }
        };
        let mut rows: Vec<(&ToolKey, &ToolRow)> = self.rows.iter().filter(|(_, r)| r.calls > 0 || r.rent.tokens > 0.0).collect();
        rows.sort_by(|a, b| key(b.1).total_cmp(&key(a.1)).then(a.0.cmp(b.0)));
        rows
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
    fn session(&mut self, _: &Session) {}

    fn request(&mut self, _: &RequestRecord) {}

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
                let r = self.row(k, s.class);
                r.trips.add(v);
                if let Some(alone) = s.call {
                    r.calls += 1;
                    r.single += u64::from(alone);
                }
            }
            None => self.other_trips.add(v),
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
            trips: Values::default(),
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
}
