//! Turns sessions into cost facts, cache-miss events and per-request records.
//!
//! Cost is attributed by prompt position, the way prefix caching bills: the first
//! `cache_read` tokens of a prompt are charged at the read rate, the next `cache_write` tokens
//! at the write rate and the rest at the full input rate. Each block of the prompt gets the
//! share of that charge that falls on its span, in every request it is part of, so a block
//! pays for as long as it stays in the context. When a request misses, the span of the prefix
//! it had to process again is charged as `rewrite`, with the cause and the avoidable premium
//! (the difference to the read rate). Output is charged to the blocks the response produced.

pub mod cache;

use std::collections::HashMap;

use crate::model::{Agent, BlockKind, Chain, ChainKind, Request, Session, Usage};
use crate::pricing::{Price, Pricing};
use cache::{Cause, Edit, Prev};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Component {
    Read,
    Write,
    Rewrite,
    Uncached,
    Output,
}

impl Component {
    pub const ALL: [Component; 5] =
        [Component::Read, Component::Write, Component::Rewrite, Component::Uncached, Component::Output];

    pub fn name(self) -> &'static str {
        match self {
            Component::Read => "cache read",
            Component::Write => "cache write",
            Component::Rewrite => "rewrite after miss",
            Component::Uncached => "uncached input",
            Component::Output => "output",
        }
    }

    pub fn parse(s: &str) -> Option<Component> {
        match s {
            "read" => Some(Component::Read),
            "write" => Some(Component::Write),
            "rewrite" => Some(Component::Rewrite),
            "uncached" => Some(Component::Uncached),
            "output" => Some(Component::Output),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Filter {
    pub since_ms: Option<i64>,
    pub until_ms: Option<i64>,
}

impl Filter {
    pub fn accepts(&self, ts_ms: i64) -> bool {
        self.since_ms.is_none_or(|s| ts_ms >= s) && self.until_ms.is_none_or(|u| ts_ms < u)
    }
}

/// A share of spend with every dimension a profile can be stacked by.
pub struct Fact<'a> {
    pub agent: &'a str,
    pub project: &'a str,
    pub session: &'a str,
    pub chain: &'a str,
    /// What kind of work the request was part of (see `Request::origin`).
    pub origin: &'a str,
    /// What the work was: a subagent's task or the session title (see `Session::task_label`).
    pub task: &'a str,
    pub model: &'a str,
    pub turn: Option<u32>,
    pub component: Component,
    pub cause: Option<Cause>,
    pub kind: BlockKind,
    pub tool: Option<&'a str>,
    pub target: Option<&'a str>,
    pub tokens: f64,
    pub usd: Option<f64>,
    pub avoidable_usd: f64,
    /// When the content entered the context (the request time for output): spend is charged
    /// back to what caused it.
    pub ts_ms: i64,
}

#[derive(Clone, Debug)]
pub struct MissEvent {
    pub agent: Agent,
    pub project: String,
    pub session: String,
    pub chain: String,
    pub ts_ms: i64,
    pub model: String,
    pub cause: Cause,
    pub detail: String,
    pub context: u64,
    pub rewritten: u64,
    pub premium_usd: Option<f64>,
}

pub struct RequestRecord<'a> {
    pub agent: Agent,
    pub chain_kind: ChainKind,
    pub advisor: bool,
    pub origin: &'a str,
    pub task: &'a str,
    pub model: &'a str,
    pub price: Option<Price>,
    pub usage: Usage,
    pub gap_s: Option<f64>,
    pub prev_prompt: Option<u64>,
    pub composition: bool,
}

pub trait Sink {
    fn session(&mut self, session: &Session);
    fn request(&mut self, request: &RequestRecord);
    fn miss(&mut self, miss: MissEvent);
    fn fact(&mut self, fact: &Fact);
}

pub fn analyze(s: &Session, pricing: &Pricing, filter: &Filter, sink: &mut dyn Sink) {
    sink.session(s);
    let msg_chars = message_chars(s);
    let task = s.task_label();
    for chain in &s.chains {
        let mut w = Walker::new(s, chain, &task, &msg_chars, pricing, filter);
        for req in &chain.requests {
            w.step(req, sink);
        }
        w.flush(sink);
    }
}

fn message_chars(s: &Session) -> Vec<u64> {
    s.messages.iter().map(|m| s.blocks[m.blocks.clone()].iter().map(|b| b.chars).sum()).collect()
}

/// Prompt layout: tools and system prompt, then the messages in order, sized in characters.
/// Unresolved messages count as an average resolved one. `on_block` sees every message block
/// with its turn number. Returns the number of turns.
fn layout(
    s: &Session,
    msg_chars: &[u64],
    shape: Option<usize>,
    list: &[usize],
    spans: &mut Vec<(Slot, f64)>,
    mut on_block: impl FnMut(usize, u32),
) -> u32 {
    if let Some(shape) = shape {
        spans.extend(s.shapes[shape].blocks.clone().map(|b| (Slot::Block(b), s.blocks[b].chars as f64)));
    }
    let (known, count) =
        list.iter().filter(|&&m| s.messages[m].resolved).fold((0u64, 0u64), |(c, n), &m| (c + msg_chars[m], n + 1));
    let unresolved_chars = if count > 0 { known as f64 / count as f64 } else { 1.0 };
    let mut turn = 0;
    for &m in list {
        let msg = &s.messages[m];
        if msg.starts_turn {
            turn += 1;
        }
        for b in msg.blocks.clone() {
            on_block(b, turn);
            spans.push((Slot::Block(b), if msg.resolved { s.blocks[b].chars as f64 } else { unresolved_chars }));
        }
    }
    turn
}

/// The prompt of the latest request of a session's main conversation.
pub struct LiveContext {
    pub chain: String,
    pub ts_ms: i64,
    pub model: String,
    pub usage: Usage,
    pub context_window: Option<u64>,
    /// Block index, or `None` for content the log does not show; tokens; turn.
    pub blocks: Vec<(Option<usize>, f64, u32)>,
    pub turns: u32,
}

pub fn live_context(s: &Session) -> Option<LiveContext> {
    let billed = |r: &Request| r.usage.prompt() > 0 && !r.advisor;
    let chain = s
        .chains
        .iter()
        .filter(|c| c.kind == ChainKind::Main)
        .max_by_key(|c| c.requests.iter().filter(|r| billed(r)).map(|r| r.ts_ms).max().unwrap_or(i64::MIN))?;
    let last = chain.requests.iter().rposition(billed)?;
    let (mut list, mut shape) = (Vec::new(), None);
    for r in &chain.requests[..=last] {
        if let Some(d) = &r.prompt {
            list.truncate(d.keep.min(list.len()));
            list.extend_from_slice(&d.tail);
            if d.shape.is_some() {
                shape = d.shape;
            }
        }
    }
    let req = &chain.requests[last];
    let (mut spans, mut turns) = (Vec::new(), HashMap::new());
    let mut count = 0;
    if req.prompt.is_some() {
        count = layout(s, &message_chars(s), shape, &list, &mut spans, |b, t| {
            turns.insert(b, t);
        });
    }
    let total: f64 = spans.iter().map(|(_, c)| c).sum();
    let prompt = req.usage.prompt() as f64;
    let blocks = if total <= 0.0 {
        vec![(None, prompt, 0)]
    } else {
        spans
            .iter()
            .map(|&(slot, c)| match slot {
                Slot::Block(b) => (Some(b), c * prompt / total, turns.get(&b).copied().unwrap_or(0)),
                _ => (None, c * prompt / total, 0),
            })
            .collect()
    };
    Some(LiveContext {
        chain: chain.name.clone(),
        ts_ms: req.ts_ms,
        model: req.model.clone(),
        usage: req.usage,
        context_window: req.context_window,
        blocks,
        turns: count,
    })
}

/// Pseudo blocks after the session's own blocks.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum Slot {
    Block(usize),
    UnknownContext,
    Response,
    HiddenThinking,
}

const ADVISOR: usize = 1;

struct Walker<'a> {
    s: &'a Session,
    chain: &'a Chain,
    task: &'a str,
    msg_chars: &'a [u64],
    pricing: &'a Pricing,
    filter: &'a Filter,
    list: Vec<usize>,
    shape: Option<usize>,
    prev: Option<Prev>,
    block_turn: Vec<Option<u32>>,
    block_ts: Vec<Option<i64>>,
    first_ts: Option<i64>,
    models: Vec<String>,
    origins: Vec<&'a str>,
    /// (label, model, origin) -> per block [read, read $, write, write $, uncached, uncached $].
    input: HashMap<(usize, usize, usize), Vec<[f64; 6]>>,
    unknown_input: HashMap<(usize, usize, usize), [f64; 6]>,
    rewrite: HashMap<(usize, usize, usize, Slot, Cause), [f64; 3]>,
    output: HashMap<(usize, usize, usize, Slot), [f64; 2]>,
    priced: HashMap<usize, bool>,
    spans: Vec<(Slot, f64)>,
}

impl<'a> Walker<'a> {
    fn new(s: &'a Session, chain: &'a Chain, task: &'a str, msg_chars: &'a [u64], pricing: &'a Pricing, filter: &'a Filter) -> Self {
        Walker {
            s,
            chain,
            task,
            msg_chars,
            pricing,
            filter,
            list: Vec::new(),
            shape: None,
            prev: None,
            block_turn: vec![None; s.blocks.len()],
            block_ts: vec![None; s.blocks.len()],
            first_ts: None,
            models: Vec::new(),
            origins: Vec::new(),
            input: HashMap::new(),
            unknown_input: HashMap::new(),
            rewrite: HashMap::new(),
            output: HashMap::new(),
            priced: HashMap::new(),
            spans: Vec::new(),
        }
    }

    fn model_idx(&mut self, model: &str) -> usize {
        match self.models.iter().position(|m| m == model) {
            Some(i) => i,
            None => {
                self.models.push(model.to_string());
                self.models.len() - 1
            }
        }
    }

    fn origin_idx(&mut self, origin: &'a str) -> usize {
        match self.origins.iter().position(|o| *o == origin) {
            Some(i) => i,
            None => {
                self.origins.push(origin);
                self.origins.len() - 1
            }
        }
    }

    fn step(&mut self, req: &'a Request, sink: &mut dyn Sink) {
        let s = self.s;
        let mut removed = Vec::new();
        let mut edit_at = None;
        let mut tail_blocks = 0;
        let len_before = self.list.len();
        if let Some(d) = &req.prompt {
            let keep = d.keep.min(self.list.len());
            if keep < self.list.len() {
                edit_at = Some(keep);
                removed = self.list[keep..].to_vec();
            }
            self.list.truncate(keep);
            self.list.extend_from_slice(&d.tail);
            if d.shape.is_some() {
                self.shape = d.shape;
            }
            tail_blocks = d.tail.iter().map(|&m| s.messages[m].blocks.len()).sum();
        }
        let usage = req.usage;
        let prompt = usage.prompt();
        if prompt == 0 && usage.output == 0 {
            return; // a request record without a response
        }

        let verdict = match (&self.prev, req.advisor) {
            (Some(prev), false) => {
                let edit = edit_at.map(|at| Edit {
                    at,
                    len_before,
                    removed: &removed,
                    added: &self.list[at..self.list.len().min(at + removed.len())],
                });
                cache::judge(s, prev, req, self.shape, edit.as_ref(), tail_blocks)
            }
            _ => None,
        };

        if self.filter.accepts(req.ts_ms) {
            let price = self.pricing.get(&req.model, prompt, req.ts_ms);
            sink.request(&RequestRecord {
                agent: s.agent,
                chain_kind: if req.advisor { ChainKind::Advisor } else { self.chain.kind },
                advisor: req.advisor,
                origin: req.origin.as_deref().unwrap_or("prompt"),
                task: self.task,
                model: &req.model,
                price,
                usage,
                gap_s: self.prev.as_ref().map(|p| (req.ts_ms - p.ts_ms) as f64 / 1000.0),
                prev_prompt: self.prev.as_ref().map(|p| p.prompt),
                composition: req.prompt.is_some(),
            });
            let premium = self.attribute(req, price, verdict.as_ref());
            if let Some(v) = &verdict {
                sink.miss(MissEvent {
                    agent: s.agent,
                    project: s.project.clone(),
                    session: s.short_id().to_string(),
                    chain: self.chain.name.clone(),
                    ts_ms: req.ts_ms,
                    model: req.model.clone(),
                    cause: v.cause,
                    detail: v.detail.clone(),
                    context: prompt,
                    rewritten: v.rewritten,
                    premium_usd: price.map(|_| premium),
                });
            }
        }

        if !req.advisor {
            self.prev = Some(Prev {
                prompt,
                ts_ms: req.ts_ms,
                model: req.model.clone(),
                long_ttl: usage.write_long > usage.write_short,
                params: req.params.clone(),
                version: req.version.clone(),
                shape: self.shape,
            });
        }
    }

    /// Charges the request to the blocks of its prompt and response; returns the avoidable
    /// premium of its re-written prefix.
    fn attribute(&mut self, req: &'a Request, price: Option<Price>, verdict: Option<&cache::Verdict>) -> f64 {
        let s = self.s;
        self.first_ts.get_or_insert(req.ts_ms);
        let label = if req.advisor { ADVISOR } else { 0 };
        let model = self.model_idx(&req.model);
        let origin = self.origin_idx(req.origin.as_deref().unwrap_or("prompt"));
        self.priced.insert(model, price.is_some());
        let usage = req.usage;
        let t = usage.prompt() as f64;

        let mut spans = std::mem::take(&mut self.spans);
        spans.clear();
        let mut turn = 0u32;
        if req.prompt.is_some() {
            let (block_turn, block_ts) = (&mut self.block_turn, &mut self.block_ts);
            turn = layout(s, self.msg_chars, self.shape, &self.list, &mut spans, |b, t| {
                block_turn[b].get_or_insert(t);
                block_ts[b].get_or_insert(req.ts_ms);
            });
        }
        let total_chars: f64 = spans.iter().map(|(_, c)| c).sum();
        if total_chars <= 0.0 {
            spans.clear();
            spans.push((Slot::UnknownContext, 1.0));
        }
        let scale = t / spans.iter().map(|(_, c)| c).sum::<f64>();

        let (pr, pw, pu, po) = match price {
            Some(p) => {
                let w = usage.cache_write().max(1) as f64;
                let pw = (usage.write_short as f64 * p.write_short + usage.write_long as f64 * p.write_long) / w;
                let pw = if usage.cache_write() == 0 { p.write_short } else { pw };
                (p.read / 1e6, pw / 1e6, p.input / 1e6, p.output / 1e6)
            }
            None => (0.0, 0.0, 0.0, 0.0),
        };
        let r = usage.cache_read as f64;
        let w = usage.cache_write() as f64;
        let rw = verdict.map_or(0.0, |v| (v.rewritten as f64).min(t - r));
        let cause = verdict.map(|v| v.cause);
        let ov = |a: f64, z: f64, lo: f64, hi: f64| (z.min(hi) - a.max(lo)).max(0.0);

        let mut premium = 0.0;
        let mut pos = 0.0;
        let blocks_len = s.blocks.len();
        for &(slot, chars) in &spans {
            let (a, z) = (pos, pos + chars * scale);
            pos = z;
            let read = ov(a, z, 0.0, r);
            let w_re = ov(a, z, r, (r + w).min(r + rw));
            let u_re = ov(a, z, r + w, (r + rw).max(r + w));
            let write = ov(a, z, r, r + w) - w_re;
            let uncached = ov(a, z, r + w, t) - u_re;
            let vals = [read, read * pr, write, write * pw, uncached, uncached * pu];
            match slot {
                Slot::Block(b) => {
                    let row = &mut self.input.entry((label, model, origin)).or_insert_with(|| vec![[0.0; 6]; blocks_len])[b];
                    row.iter_mut().zip(vals).for_each(|(x, v)| *x += v);
                }
                _ => {
                    let row = self.unknown_input.entry((label, model, origin)).or_insert([0.0; 6]);
                    row.iter_mut().zip(vals).for_each(|(x, v)| *x += v);
                }
            }
            if let (Some(cause), true) = (cause, w_re + u_re > 0.0) {
                let avoidable = w_re * (pw - pr) + u_re * (pu - pr);
                premium += avoidable;
                let row = self.rewrite.entry((label, model, origin, slot, cause)).or_insert([0.0; 3]);
                row[0] += w_re + u_re;
                row[1] += w_re * pw + u_re * pu;
                row[2] += avoidable;
            }
        }
        self.spans = spans;

        // Output goes to what the response produced: thinking gets the reported reasoning
        // tokens, text and tool calls share the rest by size.
        if usage.output > 0 {
            let produced: Vec<usize> = req.produced.map(|m| s.messages[m].blocks.clone().collect()).unwrap_or_default();
            for &b in &produced {
                self.block_turn[b].get_or_insert(turn);
                self.block_ts[b].get_or_insert(req.ts_ms);
            }
            let thinking: Vec<usize> = produced.iter().copied().filter(|&b| s.blocks[b].kind == BlockKind::Thinking).collect();
            let visible: Vec<usize> = produced.iter().copied().filter(|&b| s.blocks[b].kind != BlockKind::Thinking).collect();
            let reasoning = usage.reasoning.min(usage.output) as f64;
            let mut charge = |slots: Vec<(Slot, f64)>, tokens: f64, fallback: Slot| {
                let total: f64 = slots.iter().map(|(_, c)| c).sum();
                if tokens <= 0.0 {
                    return;
                }
                if total <= 0.0 {
                    let row = self.output.entry((label, model, origin, fallback)).or_insert([0.0; 2]);
                    row[0] += tokens;
                    row[1] += tokens * po;
                    return;
                }
                for (slot, c) in slots {
                    let row = self.output.entry((label, model, origin, slot)).or_insert([0.0; 2]);
                    row[0] += tokens * c / total;
                    row[1] += tokens * c / total * po;
                }
            };
            let sized = |bs: &[usize]| bs.iter().map(|&b| (Slot::Block(b), s.blocks[b].chars as f64)).collect::<Vec<_>>();
            if reasoning > 0.0 {
                charge(sized(&thinking), reasoning, Slot::HiddenThinking);
                charge(sized(&visible), usage.output as f64 - reasoning, Slot::Response);
            } else {
                charge(sized(&produced), usage.output as f64, Slot::Response);
            }
        }
        premium
    }

    fn flush(self, sink: &mut dyn Sink) {
        let s = self.s;
        let chain_name = |label: usize| if label == ADVISOR { "advisor" } else { self.chain.name.as_str() };
        let task = self.task;
        let fact = |(label, model, origin): (usize, usize, usize), slot: Slot, component: Component, tokens: f64, usd: f64, cause: Option<Cause>, avoidable: f64| {
            let first = self.first_ts.unwrap_or(0);
            let (kind, tool, target, turn, ts_ms) = match slot {
                Slot::Block(b) => {
                    let blk = &s.blocks[b];
                    (blk.kind, blk.tool.as_deref(), blk.target.as_deref(), self.block_turn[b], self.block_ts[b].unwrap_or(first))
                }
                Slot::UnknownContext => (BlockKind::Unknown, None, None, None, first),
                Slot::Response => (BlockKind::Text, None, None, None, first),
                Slot::HiddenThinking => (BlockKind::Thinking, None, None, None, first),
            };
            Fact {
                agent: s.agent.name(),
                project: &s.project,
                session: s.short_id(),
                chain: chain_name(label),
                origin: self.origins[origin],
                task,
                model: &self.models[model],
                turn,
                component,
                cause,
                kind,
                tool,
                target,
                tokens,
                usd: self.priced.get(&model).copied().unwrap_or(false).then_some(usd),
                avoidable_usd: avoidable,
                ts_ms,
            }
        };
        let emit_input = |sink: &mut dyn Sink, key: (usize, usize, usize), slot: Slot, row: &[f64; 6]| {
            for (i, component) in [Component::Read, Component::Write, Component::Uncached].into_iter().enumerate() {
                if row[2 * i] > 0.0 {
                    sink.fact(&fact(key, slot, component, row[2 * i], row[2 * i + 1], None, 0.0));
                }
            }
        };
        for (&key, rows) in &self.input {
            for (b, row) in rows.iter().enumerate() {
                emit_input(sink, key, Slot::Block(b), row);
            }
        }
        for (&key, row) in &self.unknown_input {
            emit_input(sink, key, Slot::UnknownContext, row);
        }
        for (&(label, model, origin, slot, cause), row) in &self.rewrite {
            sink.fact(&fact((label, model, origin), slot, Component::Rewrite, row[0], row[1], Some(cause), row[2]));
        }
        for (&(label, model, origin, slot), row) in &self.output {
            sink.fact(&fact((label, model, origin), slot, Component::Output, row[0], row[1], None, 0.0));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Block, PromptDelta};

    #[test]
    fn live_context_scales_the_last_prompt_to_its_usage() {
        let mut s = Session::new(Agent::Claude, "s".into());
        let block = |kind, chars, tool: Option<&str>| Block { kind, chars, tool: tool.map(str::to_string), target: None };
        let prompt = s.push_message(vec![block(BlockKind::Prompt, 300, None)], true);
        let result = s.push_message(vec![block(BlockKind::ToolResult, 700, Some("Read"))], false);
        let request = |ts_ms, keep, tail, read| Request {
            ts_ms,
            model: "claude-opus-5-5".into(),
            usage: Usage { cache_read: read, ..Usage::default() },
            prompt: Some(PromptDelta { shape: None, keep, tail }),
            ..Request::default()
        };
        let requests = vec![request(1, 0, vec![prompt], 100), request(2, 1, vec![result], 2000)];
        s.chains.push(Chain { name: "main".into(), kind: ChainKind::Main, requests });

        let live = live_context(&s).unwrap();
        assert_eq!((live.ts_ms, live.turns), (2, 1));
        let sizes: Vec<(Option<usize>, f64)> = live.blocks.iter().map(|(b, t, _)| (*b, *t)).collect();
        assert_eq!(sizes, vec![(Some(0), 600.0), (Some(1), 1400.0)]);
    }

    #[test]
    fn spend_goes_to_the_origin_of_each_request() {
        let mut s = Session::new(Agent::Claude, "s".into());
        s.title = Some("Fix the parser".into());
        let prompt = s.push_message(vec![Block { kind: BlockKind::Prompt, chars: 400, tool: None, target: None }], true);
        let usage = Usage { cache_read: 1000, output: 100, ..Usage::default() };
        let request = |ts_ms, keep, origin: Option<&str>| Request {
            ts_ms,
            model: "claude-opus-5-5".into(),
            usage,
            prompt: Some(PromptDelta { shape: None, keep, tail: if keep == 0 { vec![prompt] } else { Vec::new() } }),
            origin: origin.map(str::to_string),
            ..Request::default()
        };
        let requests = vec![request(1, 0, None), request(2, 1, Some("skill:code-review"))];
        s.chains.push(Chain { name: "main".into(), kind: ChainKind::Main, requests });

        let pricing = Pricing::defaults();
        let one = pricing.get("claude-opus-5-5", 1000, 1).unwrap().cost(&usage);
        let mut c = crate::report::Collector::new(Some(crate::report::dims("origin,task")), None);
        analyze(&s, &pricing, &Filter::default(), &mut c);
        let spend = |origin: &str| c.stacks.iter().filter(|(k, _)| k[0].name == origin).map(|(_, v)| v.usd).sum::<f64>();
        assert!((spend("prompt") - one).abs() < 1e-12 && (spend("skill:code-review") - one).abs() < 1e-12);
        assert!(c.stacks.keys().all(|k| k[1].name == "Fix the parser"));
        assert!((c.work.by_origin["skill:code-review"].usd - one).abs() < 1e-12, "whole requests agree with the facts");
    }
}
