//! The call log of a session (see tool-breakdown, Call log): every tool call of the main
//! conversation as a row, with its start, request, group, input head, status, duration and result
//! tokens. It is built in the X-ray's pass, next to the records of the main requests.

use std::collections::HashMap;

use crate::analysis::Entered;
use crate::model::{BlockKind, CallRecord, Outcome, Session};
use crate::tools::{BlockEntry, Group};

/// How a call ended, as its row shows it (see tool-breakdown, Status of a call).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CallStatus {
    Done,
    Failed,
    Refused,
    Denied,
    /// It returned while its work went on.
    Background,
    /// It asked the user.
    AskedYou,
    /// No result yet, and no later request of the main conversation.
    Running,
    /// No result, and the main conversation went on without it.
    NoResult,
}

impl CallStatus {
    pub fn name(self) -> &'static str {
        match self {
            CallStatus::Done => "done",
            CallStatus::Failed => "failed",
            CallStatus::Refused => "refused",
            CallStatus::Denied => "denied",
            CallStatus::Background => "background",
            CallStatus::AskedYou => "asked you",
            CallStatus::Running => "running",
            CallStatus::NoResult => "no result",
        }
    }
}

/// A row of the call log.
#[derive(Clone, Debug, PartialEq)]
pub struct CallRow {
    /// When the log gives the call, in epoch ms.
    pub start_ms: Option<i64>,
    /// The number of the request whose response made the call, as `tokprof scan --requests`
    /// numbers them; `None` for a request without a time.
    pub request: Option<u32>,
    pub tool: String,
    pub group: Group,
    /// The short head of its input, as its source gives it.
    pub input: Option<String>,
    pub status: CallStatus,
    pub duration_ms: Option<i64>,
    /// Whether its duration counts for medians and sums (see tool-breakdown, Calls that count for
    /// times).
    pub timed: bool,
    /// The tokens of its result once the result entered a prompt.
    pub result_tokens: Option<f64>,
}

/// Collects the rows from the main requests of the session's main log.
#[derive(Default)]
pub struct Collector {
    rows: Vec<CallRow>,
    /// The request index of each row, until the numbers are known.
    request_of: Vec<usize>,
    /// The row of each call record of the main log.
    row_of_call: HashMap<u32, usize>,
    /// The index of the open main request, while the pass is on one.
    open: Option<usize>,
    /// The pass is in the main log.
    main_log: bool,
}

impl Collector {
    /// A log of the session starts; the first is the main log.
    pub fn session(&mut self, first: bool) {
        self.main_log = first;
        self.open = None;
    }

    /// A request of the pass, with the index its main record gets, or `None` when it is not
    /// one of the main conversation's.
    pub fn request(&mut self, index: Option<usize>) {
        self.open = index.filter(|_| self.main_log);
    }

    /// The request's response made its calls; their rows start here.
    pub fn entered(&mut self, e: &Entered) {
        let Some(at) = self.open.take() else { return };
        let s = e.session;
        let Some(m) = e.request.produced else { return };
        for b in s.messages[m].blocks.clone().filter(|&b| s.blocks[b].kind == BlockKind::ToolUse) {
            let blk = &s.blocks[b];
            let record = s.call_of_block.get(&b).copied();
            let call = record.and_then(|i| s.calls.get(i as usize));
            let status = status(s, b, call);
            if let Some(i) = record {
                self.row_of_call.entry(i).or_insert(self.rows.len());
            }
            self.rows.push(CallRow {
                start_ms: call.and_then(|c| c.start_ms),
                request: None,
                tool: blk.tool.clone().unwrap_or_else(|| "?".to_string()),
                group: Group::of(blk.class, blk.target.as_deref()),
                input: call.and_then(|c| c.input.clone()),
                status,
                duration_ms: call.and_then(|c| c.duration_ms()),
                timed: call.is_some_and(|c| c.timed()),
                result_tokens: None,
            });
            self.request_of.push(at);
        }
    }

    /// A call that no request made, such as a setup step: its row has no request.
    pub fn platform_call(&mut self, s: &Session, b: usize) {
        if !self.main_log {
            return;
        }
        let blk = &s.blocks[b];
        let record = s.call_of_block.get(&b).copied();
        let call = record.and_then(|i| s.calls.get(i as usize));
        if let Some(i) = record {
            self.row_of_call.entry(i).or_insert(self.rows.len());
        }
        self.rows.push(CallRow {
            start_ms: call.and_then(|c| c.start_ms),
            request: None,
            tool: blk.tool.clone().unwrap_or_else(|| "?".to_string()),
            group: Group::of(blk.class, blk.target.as_deref()),
            input: call.and_then(|c| c.input.clone()),
            status: status(s, b, call),
            duration_ms: call.and_then(|c| c.duration_ms()),
            timed: call.is_some_and(|c| c.timed()),
            result_tokens: None,
        });
        // No request index: its `req` stays empty.
        self.request_of.push(usize::MAX);
    }

    /// A tool block entered a prompt; a result gives its call's row its tokens the first time.
    pub fn block_entered(&mut self, b: &BlockEntry) {
        if !self.main_log || b.kind != BlockKind::ToolResult {
            return;
        }
        let Some(&row) = b.call.and_then(|c| self.row_of_call.get(&c)) else { return };
        if b.sized {
            self.rows[row].result_tokens.get_or_insert(b.tokens);
        }
    }

    /// The rows in the order of their start, with their request numbers from `numbers` (see
    /// `crate::timeline::Collector::numbers`). A call without a result is `running` only in the
    /// main conversation's last request.
    pub fn rows(self, numbers: &[Option<u32>]) -> Vec<CallRow> {
        let last = numbers.iter().flatten().max().copied();
        let mut rows: Vec<(i64, CallRow)> = self
            .rows
            .into_iter()
            .zip(self.request_of)
            .map(|(mut r, at)| {
                r.request = numbers.get(at).copied().flatten();
                if r.status == CallStatus::Running && r.request.is_some() && r.request != last {
                    r.status = CallStatus::NoResult;
                }
                (r.start_ms.unwrap_or(i64::MAX), r)
            })
            .collect();
        rows.sort_by_key(|(start, _)| *start);
        rows.into_iter().map(|(_, r)| r).collect()
    }
}

/// How the call in block `b` ended, as its row shows it before the later requests are known.
fn status(s: &Session, b: usize, call: Option<&CallRecord>) -> CallStatus {
    match (s.outcome(b), call) {
        (Outcome::Failed, _) => CallStatus::Failed,
        (Outcome::Refused, _) => CallStatus::Refused,
        (Outcome::Denied, _) => CallStatus::Denied,
        (_, Some(c)) if c.background => CallStatus::Background,
        (_, Some(c)) if c.asks_user => CallStatus::AskedYou,
        (_, Some(c)) if !c.done => CallStatus::Running,
        _ => CallStatus::Done,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::{Fact, Filter, MissEvent, RequestRecord, Sink, analyze};
    use crate::model::{Block, CallEnd, CallRecord, Chain, ChainKind, Class, PromptDelta, Request, Session, ToolClass, Usage};
    use crate::pricing::Pricing;

    /// The X-ray's two collectors of the main log, fed by the pass.
    #[derive(Default)]
    struct Pass {
        timeline: crate::timeline::Collector,
        calls: Collector,
    }

    impl Sink for Pass {
        fn session(&mut self, _: &Session) {
            self.calls.session(true);
        }
        fn request(&mut self, r: &RequestRecord) {
            let main = r.chain_kind == ChainKind::Main;
            self.calls.request(main.then(|| self.timeline.next_index()));
            self.timeline.request(r, main);
        }
        fn miss(&mut self, _: MissEvent) {}
        fn fact(&mut self, _: &Fact) {}
        fn entered(&mut self, e: &Entered) {
            self.calls.entered(e);
            self.timeline.entered(e);
        }
        fn block_entered(&mut self, b: &BlockEntry) {
            self.calls.block_entered(b);
        }
        fn platform_call(&mut self, s: &Session, b: usize) {
            self.calls.platform_call(s, b);
        }
    }

    fn block(kind: BlockKind, tool: &str, class: ToolClass, target: Option<&str>) -> Block {
        Block { class: Class::Tool(class), kind, chars: 400, tool: Some(tool.into()), target: target.map(str::to_string) }
    }

    /// Gives the call and result blocks of a call their record.
    fn record(s: &mut Session, blocks: &[usize], start: i64, end: Option<i64>, background: bool, input: &str) {
        let at = s.calls.len() as u32;
        s.calls.push(CallRecord { start_ms: Some(start), end_ms: end, done: end.is_some(), background, asks_user: false, input: Some(input.into()) });
        for &b in blocks {
            s.call_of_block.insert(b, at);
        }
    }

    /// A row as the test reads it: tool, request, group, status, duration and whether it counts.
    type Shown<'a> = (&'a str, Option<u32>, String, CallStatus, Option<i64>, bool);

    #[test]
    fn a_setup_step_has_a_row_without_a_request() {
        let mut s = Session::new(crate::testing::claude(), "s".into());
        let prompt = s.push_message(vec![Block { class: Class::None, kind: BlockKind::Prompt, chars: 400, tool: None, target: None }], true);
        let step = s.push_message(vec![block(BlockKind::ToolUse, "run_custom_setup_step", ToolClass::EnvironmentSetup, None)], false);
        let call = s.push_message(vec![block(BlockKind::ToolUse, "view", ToolClass::Read, Some("a.rs"))], false);
        let result = s.push_message(vec![block(BlockKind::ToolResult, "view", ToolClass::Read, Some("a.rs"))], false);
        let (sb, cb, rb) = (s.messages[step].blocks.start, s.messages[call].blocks.start, s.messages[result].blocks.start);
        record(&mut s, &[sb], 1_000, Some(9_000), false, "Install the JDK");
        record(&mut s, &[cb, rb], 20_000, Some(21_000), false, "a.rs");
        s.platform_calls.push(sb);
        s.logged_tokens.insert(rb, 120);
        let req = |ts_ms, keep, tail: Vec<usize>, produced| Request {
            ts_ms,
            model: "m".into(),
            prompt: Some(PromptDelta { shape: None, keep, tail }),
            produced,
            sent_ms: Some(ts_ms - 1),
            ended_ms: Some(ts_ms),
            no_usage: true,
            ..Request::default()
        };
        let requests = vec![req(20_000, 0, vec![prompt], Some(call)), req(22_000, 1, vec![call, result], None)];
        s.chains.push(Chain { name: "main".into(), kind: ChainKind::Main, requests });
        let mut pass = Pass::default();
        analyze(&s, &Pricing::defaults(), &Filter::default(), &mut pass);
        let numbers = pass.timeline.numbers();
        let rows = pass.calls.rows(&numbers);
        let shown: Vec<_> = rows.iter().map(|r| (r.tool.as_str(), r.request, r.group.name(), r.duration_ms, r.result_tokens)).collect();
        assert_eq!(
            shown,
            [("run_custom_setup_step", None, "environment".to_string(), Some(8_000), None), ("view", Some(1), "files".to_string(), Some(1_000), Some(120.0))]
        );
    }

    #[test]
    fn rows_of_the_main_calls_in_the_order_of_their_start() {
        let mut s = Session::new(crate::testing::claude(), "s".into());
        let prompt = s.push_message(vec![Block { class: Class::None, kind: BlockKind::Prompt, chars: 400, tool: None, target: None }], true);
        // Request 1 reads and searches; request 2 starts a subagent and a command in the
        // background; request 3 runs a command that never returns; request 4 calls a tool that is
        // still running when the log ends.
        let r1 = s.push_message(
            vec![block(BlockKind::ToolUse, "Read", ToolClass::Read, Some("a.rs")), block(BlockKind::ToolUse, "Grep", ToolClass::Search, Some("(project)"))],
            false,
        );
        let res1 = s.push_message(
            vec![
                block(BlockKind::ToolResult, "Read", ToolClass::Read, Some("a.rs")),
                block(BlockKind::ToolResult, "Grep", ToolClass::Search, Some("(project)")),
            ],
            false,
        );
        let r2 = s.push_message(
            vec![block(BlockKind::ToolUse, "Agent", ToolClass::Subagent, None), block(BlockKind::ToolUse, "Bash", ToolClass::Execute, Some("npm run"))],
            false,
        );
        let res2 = s.push_message(
            vec![block(BlockKind::ToolResult, "Agent", ToolClass::Subagent, None), block(BlockKind::ToolResult, "Bash", ToolClass::Execute, Some("npm run"))],
            false,
        );
        let r3 = s.push_message(vec![block(BlockKind::ToolUse, "Bash", ToolClass::Execute, Some("cargo test"))], false);
        let r4 = s.push_message(vec![block(BlockKind::ToolUse, "mcp__idea__find", ToolClass::Other, Some("mcp:idea"))], false);
        let b = |m: usize| s.messages[m].blocks.clone().collect::<Vec<usize>>();
        let (c1, x1, c2, x2, c3, c4) = (b(r1), b(res1), b(r2), b(res2), b(r3), b(r4));
        record(&mut s, &[c1[0], x1[0]], 1_000, Some(1_080), false, "a.rs");
        record(&mut s, &[c1[1], x1[1]], 1_000, Some(1_300), false, "fn main");
        record(&mut s, &[c2[0], x2[0]], 5_000, Some(255_000), false, "Find the parser");
        record(&mut s, &[c2[1], x2[1]], 5_000, Some(5_200), true, "npm run dev");
        record(&mut s, &c3, 300_000, None, false, "cargo test");
        record(&mut s, &c4, 400_000, None, false, "Parser");
        s.call_ends.insert(c1[1], CallEnd { outcome: crate::model::Outcome::Failed, error: None });
        let req = |ts_ms, keep, tail: Vec<usize>, produced| Request {
            ts_ms,
            model: "claude-opus-5-5".into(),
            usage: Usage { cache_read: 4_000, output: 10, ..Usage::default() },
            prompt: Some(PromptDelta { shape: None, keep, tail }),
            produced: Some(produced),
            ..Request::default()
        };
        let requests =
            vec![req(900, 0, vec![prompt], r1), req(4_000, 1, vec![r1, res1], r2), req(260_000, 3, vec![r2, res2], r3), req(350_000, 5, vec![r3], r4)];
        s.chains.push(Chain { name: "main".into(), kind: ChainKind::Main, requests });
        let mut pass = Pass::default();
        analyze(&s, &Pricing::defaults(), &Filter::default(), &mut pass);
        let numbers = pass.timeline.numbers();
        let rows = pass.calls.rows(&numbers);
        let shown: Vec<Shown> = rows.iter().map(|r| (r.tool.as_str(), r.request, r.group.name(), r.status, r.duration_ms, r.timed)).collect();
        assert_eq!(
            shown,
            [
                ("Read", Some(1), "files".to_string(), CallStatus::Done, Some(80), true),
                ("Grep", Some(1), "search".to_string(), CallStatus::Failed, Some(300), true),
                ("Agent", Some(2), "subagents".to_string(), CallStatus::Done, Some(250_000), true),
                ("Bash", Some(2), "shell".to_string(), CallStatus::Background, Some(200), false),
                ("Bash", Some(3), "shell".to_string(), CallStatus::NoResult, None, false),
                ("mcp__idea__find", Some(4), "mcp idea".to_string(), CallStatus::Running, None, false),
            ]
        );
        // The results that entered a prompt give their tokens: 4,000 tokens over 5 blocks of one size.
        let tokens: Vec<Option<f64>> = rows.iter().map(|r| r.result_tokens.map(|t| t.round())).collect();
        assert_eq!(tokens[..2], [Some(800.0), Some(800.0)]);
        assert_eq!(tokens[4..], [None, None]);
        assert_eq!(rows[3].input.as_deref(), Some("npm run dev"));
    }
}
