//! Terminal reports for one session: `tokprof` (the latest session on this machine),
//! `tokprof scan` (a given session), and the `tokprof sessions` list.

use std::borrow::Borrow;
use std::collections::{BTreeMap, HashMap};
use std::fmt::Write as _;
use std::io::{BufRead, IsTerminal};
use std::path::Path;
use std::time::SystemTime;

use anyhow::{Result, bail};
use rayon::prelude::*;
use serde_json::Value;

use crate::adapters::{self, SourceFile};
use crate::advice::{self, CacheState};
use crate::analysis::cache::{self, Cause, human_duration};
use crate::analysis::{self, Component, Fact, Filter, LiveContext, MissEvent, RequestRecord, Sink};
use crate::model::{Agent, BlockKind, Request, Session, Usage};
use crate::pricing::{Price, Pricing};
use crate::report::{Spend, Values, Work, clip_end, tokens, usd};

const BAR: usize = 32;

pub struct Style {
    color: bool,
}

impl Style {
    pub fn detect() -> Style {
        Style { color: std::io::stdout().is_terminal() && std::env::var_os("NO_COLOR").is_none() }
    }

    pub fn with_color(color: bool) -> Style {
        Style { color }
    }

    pub fn paint(&self, code: &str, s: &str) -> String {
        if self.color { format!("\x1b[{code}m{s}\x1b[0m") } else { s.to_string() }
    }

    pub fn dim(&self, s: &str) -> String {
        self.paint("2", s)
    }

    pub fn bold(&self, s: &str) -> String {
        self.paint("1", s)
    }
}

fn bar(share: f64) -> String {
    let filled = (share.clamp(0.0, 1.0) * BAR as f64).round() as usize;
    format!("{}{}", "█".repeat(filled), "░".repeat(BAR - filled))
}

fn component_color(c: Component) -> &'static str {
    match c {
        Component::Read => "36",
        Component::Write => "33",
        Component::Rewrite => "31",
        Component::Uncached => "35",
        Component::Output => "34",
    }
}

pub fn local_time(ts_ms: i64, format: &str) -> String {
    chrono::DateTime::from_timestamp_millis(ts_ms)
        .map(|t| t.with_timezone(&chrono::Local).format(format).to_string())
        .unwrap_or_default()
}

fn now_ms() -> i64 {
    SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).map_or(0, |d| d.as_millis() as i64)
}

/// Session files without Claude subagent transcripts, newest first.
pub fn main_sessions(agents: &[Agent]) -> Vec<SourceFile> {
    let mut files: Vec<SourceFile> = adapters::discover(agents).into_iter().filter(|f| !f.is_subagent()).collect();
    files.sort_by_key(|f| std::cmp::Reverse(f.modified));
    files
}

/// A session file given by path, or by a prefix of its session id.
pub fn find(spec: &str, agents: &[Agent]) -> Result<SourceFile> {
    let path = Path::new(spec);
    let agent = if adapters::codex::is_rollout(path) { Agent::Codex } else { Agent::Claude };
    if let Some(file) = SourceFile::at(agent, path) {
        return Ok(file);
    }
    // Claude names the file after the session id; Codex ends the rollout name with it.
    let newest = main_sessions(agents).into_iter().find(|f| {
        let stem = f.path.file_stem().and_then(|s| s.to_str()).unwrap_or_default();
        stem.starts_with(spec) || stem.contains(&format!("-{spec}"))
    });
    match newest {
        Some(f) => Ok(f),
        None => bail!("no session file or session id matches {spec:?}; see `tokprof sessions`"),
    }
}

/// Totals for one session, its subagents included.
#[derive(Default)]
struct Totals {
    components: [Values; 5],
    priced: bool,
    content: HashMap<(BlockKind, Option<String>, Option<String>), Values>,
    chains: BTreeMap<String, Values>,
    misses: Vec<MissEvent>,
    models: BTreeMap<String, u64>,
    requests: u64,
    work: Work,
}

impl Sink for Totals {
    fn session(&mut self, _: &Session) {}

    fn request(&mut self, r: &RequestRecord) {
        self.requests += 1;
        *self.models.entry(r.model.to_string()).or_default() += 1;
        self.priced |= r.price.is_some();
        self.work.add(r);
    }

    fn miss(&mut self, miss: MissEvent) {
        self.misses.push(miss);
    }

    fn fact(&mut self, f: &Fact) {
        let v = Values { tokens: f.tokens, usd: f.usd.unwrap_or(0.0), avoidable: f.avoidable_usd };
        let add = |a: &mut Values| {
            a.tokens += v.tokens;
            a.usd += v.usd;
            a.avoidable += v.avoidable;
        };
        add(&mut self.components[Component::ALL.iter().position(|c| *c == f.component).unwrap_or(0)]);
        add(self.content.entry((f.kind, f.tool.map(str::to_string), f.target.map(str::to_string))).or_default());
        let chain = if f.chain.starts_with("subagent") { "subagents" } else { f.chain };
        add(self.chains.entry(chain.to_string()).or_default());
    }
}

pub fn xray(file: &SourceFile, pricing: &Pricing, style: &Style) -> Result<String> {
    let mut files = vec![file.clone()];
    files.extend(file.subagent_files());
    let sessions: Vec<Session> = files.par_iter().map(adapters::parse).collect::<Result<_>>()?;
    let mut totals = Totals::default();
    for s in &sessions {
        analysis::analyze(s, pricing, &Filter::default(), &mut totals);
    }
    let main = &sessions[0];
    let live = analysis::live_context(main);
    let ts: Vec<i64> = sessions.iter().flat_map(|s| s.chains.iter().flat_map(|c| c.requests.iter().map(|r| r.ts_ms))).filter(|&t| t > 0).collect();
    let (first, last) = (ts.iter().copied().min().unwrap_or(0), ts.iter().copied().max().unwrap_or(0));

    let mut out = String::new();
    let label = |s: &str| style.dim(&format!("{s:<9}"));
    let _ = writeln!(out, "{}", style.bold("tokprof · session X-ray"));
    let _ = writeln!(out, "{} {}  {}  {}", label("session"), main.agent.name(), style.bold(&main.project), main.id);
    if let Some(title) = &main.title {
        let _ = writeln!(out, "{} {}", label("title"), clip_end(title, 82));
    }
    let models = totals.models.iter().map(|(m, n)| format!("{m} ×{n}")).collect::<Vec<_>>().join(", ");
    let _ = writeln!(out, "{} {models}", label("models"));
    let same_day = local_time(first, "%F") == local_time(last, "%F");
    let subagents = match files.len() - 1 {
        0 => String::new(),
        n => format!(" · {n} subagents"),
    };
    let _ = writeln!(
        out,
        "{} {} → {} ({}) · {} requests{subagents}",
        label("time"),
        local_time(first, "%Y-%m-%d %H:%M"),
        local_time(last, if same_day { "%H:%M" } else { "%Y-%m-%d %H:%M" }),
        human_duration((last - first) as f64 / 1000.0),
        totals.requests,
    );

    let trailing = live.as_ref().and_then(|l| trailing_compaction(main, l, pricing));
    if let Some(live) = &live {
        // After a trailing compaction, the advice sees the prompt the next request will send.
        let compacted = trailing.as_ref().map(|t| compacted_live(main, live, t));
        let shown = compacted.as_ref().unwrap_or(live);
        let cache = cache_state(main, shown, pricing);
        out.push('\n');
        let spend = totals.components.iter().map(|v| v.usd).sum();
        context_lines(&mut out, live, &cache, trailing.as_ref(), spend, pricing, style);
        advice_lines(&mut out, main, shown, &cache, &totals, pricing, style);
    }
    out.push('\n');
    spend_lines(&mut out, &totals, style);
    work_lines(&mut out, &totals, style);
    if let Some(live) = &live {
        out.push('\n');
        composition_lines(&mut out, main, live, trailing.as_ref(), style);
    }
    out.push('\n');
    costly_content_lines(&mut out, &totals, style);
    out.push('\n');
    miss_lines(&mut out, &totals, style);
    let _ = writeln!(
        out,
        "\n{}",
        style.dim(&format!(
            "profile: tokprof flame --session {id} → {}/{id}.jfr (IntelliJ: Open snapshot)",
            adapters::display_path(&crate::export::default_dir().to_string_lossy()),
            id = main.short_id()
        ))
    );
    Ok(out)
}

/// The requests of the conversation the live context belongs to, in time order, across the
/// chains it is kept in (see `Session::main_conversations`).
fn conversation<'a>(s: &'a Session, live: &LiveContext) -> Vec<&'a Request> {
    s.main_conversations()
        .into_iter()
        .find(|(name, requests)| *name == live.chain && requests.iter().any(|r| r.ts_ms == live.ts_ms))
        .map(|(_, requests)| requests)
        .unwrap_or_default()
}

/// The main conversation's cache lifetime, from its latest cache write, and how long it idled.
fn cache_state(s: &Session, live: &LiveContext, pricing: &Pricing) -> CacheState {
    let requests = conversation(s, live);
    let long_ttl = long_ttl(&requests);
    let (ttl_ms, ttl_name) = cache::ttl(s.agent, long_ttl);
    let size = live.usage.prompt() as f64;
    let (added, output) = pace(&requests);
    // The next request is sent now, at today's prices.
    let price = pricing.get(&live.model, size as u64, crate::pricing::now_ms());
    let next = price.map(|p| next_request(size, added, output, p, long_ttl));
    CacheState {
        long_ttl,
        ttl_ms,
        ttl_name,
        idle_ms: now_ms() - live.ts_ms,
        rebuild_usd: price.map(|p| size * (if long_ttl { p.write_long } else { p.write_short } - p.read) / 1e6),
        next_warm_usd: next.map(|n| n.0),
        next_cold_usd: next.map(|n| n.1),
    }
}

/// Whether the latest cache write of a conversation used the long lifetime.
pub(crate) fn long_ttl<R: Borrow<Request>>(requests: &[R]) -> bool {
    requests.iter().map(Borrow::borrow).rev().find(|r| r.usage.cache_write() > 0).is_some_and(|r| r.usage.write_long > r.usage.write_short)
}

/// What a request of a conversation usually adds, over its latest 20 requests: new content
/// written to the prompt, and output.
pub(crate) fn pace<R: Borrow<Request>>(requests: &[R]) -> (f64, f64) {
    let recent: Vec<&Request> = requests.iter().map(Borrow::borrow).rev().filter(|r| r.usage.prompt() > 0 && !r.advisor).take(20).collect();
    let mean = |f: &dyn Fn(&Request) -> u64| recent.iter().map(|r| f(r) as f64).sum::<f64>() / recent.len().max(1) as f64;
    (mean(&|r| r.usage.cache_write() + r.usage.uncached), mean(&|r| r.usage.output))
}

/// The next request on top of a prompt of `size` tokens at this pace: its cost while the
/// cache is warm, and once it is cold and the prompt is written again.
pub(crate) fn next_request(size: f64, added: f64, output: f64, p: Price, long_ttl: bool) -> (f64, f64) {
    let write = if long_ttl { p.write_long } else { p.write_short };
    ((size * p.read + added * write + output * p.output) / 1e6, ((size + added) * write + output * p.output) / 1e6)
}

/// A compaction logged after the conversation's latest request, and the prompt the next request
/// will send instead of the compacted one.
struct Trailing {
    at_ms: i64,
    /// `idle`, the logged trigger, or `None` (see `Compaction`).
    kind: Option<String>,
    /// The prompt of the conversation's first request after its previous compaction, or the
    /// setup plus 20,000 tokens.
    estimate: f64,
    /// Tool schemas and system prompt of the latest prompt, which the next request reads.
    setup: f64,
    /// The next request at today's price, when the model has one.
    next_usd: Option<f64>,
}

fn is_setup(s: &Session, block: Option<usize>) -> bool {
    block.is_some_and(|i| matches!(s.blocks[i].kind, BlockKind::Tools | BlockKind::System))
}

/// The compaction after the live prompt, if the session logged one.
fn trailing_compaction(s: &Session, live: &LiveContext, pricing: &Pricing) -> Option<Trailing> {
    let c = s.compactions.last().filter(|c| c.ts_ms > live.ts_ms)?;
    let setup: f64 = live.blocks.iter().filter(|(b, _, _)| is_setup(s, *b)).map(|(_, t, _)| t).sum();
    let requests = conversation(s, live);
    let after = requests.iter().rev().find(|r| r.compacted_before && r.usage.prompt() > 0 && !r.advisor).map(|r| r.usage.prompt() as f64);
    let estimate = after.unwrap_or(setup + 2.0 * crate::advice::SUMMARY_TOKENS);
    let (added, output) = pace(&requests);
    let next_usd = pricing
        .get(&live.model, estimate as u64, crate::pricing::now_ms())
        .map(|p| next_after_compaction(estimate, setup, added, output, p, long_ttl(&requests)));
    Some(Trailing { at_ms: c.ts_ms, kind: c.kind.clone(), estimate, setup, next_usd })
}

/// The next request after a compaction: it reads the setup, writes the rest of the prompt and
/// what a request adds at the conversation's pace, and outputs as usual.
fn next_after_compaction(estimate: f64, setup: f64, added: f64, output: f64, p: Price, long_ttl: bool) -> f64 {
    let write = if long_ttl { p.write_long } else { p.write_short };
    (setup * p.read + ((estimate - setup).max(0.0) + added) * write + output * p.output) / 1e6
}

/// The live prompt as the advice should see it after a trailing compaction: the estimate's size,
/// with only the setup's blocks left, since the compaction replaced the rest.
fn compacted_live(s: &Session, live: &LiveContext, t: &Trailing) -> LiveContext {
    let setup = t.setup.min(t.estimate);
    LiveContext {
        chain: live.chain.clone(),
        ts_ms: live.ts_ms,
        model: live.model.clone(),
        usage: Usage { cache_read: setup as u64, write_long: (t.estimate - setup) as u64, ..Usage::default() },
        context_window: live.context_window,
        blocks: live.blocks.iter().filter(|(b, _, _)| is_setup(s, *b)).copied().collect(),
        turns: 0,
    }
}

fn context_lines(out: &mut String, live: &LiveContext, cache: &CacheState, trailing: Option<&Trailing>, spend: f64, pricing: &Pricing, style: &Style) {
    // After a trailing compaction, the context is the estimate of the next prompt.
    let (size, approx) = trailing.map_or((live.usage.prompt(), ""), |t| (t.estimate as u64, "~"));
    let window = live.context_window.or_else(|| pricing.context_window(&live.model));
    let label = style.dim(&format!("{:<9}", "context"));
    match window {
        Some(w) if w > 0 => {
            let share = size as f64 / w as f64;
            let _ = writeln!(out, "{label} {}  {:5.1}%  {approx}{} of {}", bar(share), 100.0 * share, tokens(size as f64), tokens(w as f64));
        }
        _ => {
            let _ = writeln!(out, "{label} {approx}{} tokens", tokens(size as f64));
        }
    }
    let of_session = |x: f64| if spend > 0.0 { format!(" = {:.0}% of the session's {}", 100.0 * x / spend, usd(spend)) } else { String::new() };
    if let Some(t) = trailing {
        let when = local_time(t.at_ms, "%H:%M");
        let how = match t.kind.as_deref() {
            Some("idle") => format!("compacted while idle at {when}"),
            Some(kind) => format!("compacted at {when} ({kind})"),
            None => format!("compacted at {when}"),
        };
        let estimate = tokens(t.estimate);
        let _ = writeln!(out, "{} {how} · the next request re-builds ~{estimate}", style.dim(&format!("{:<9}", "cache")));
        if let Some(next) = t.next_usd {
            let _ = writeln!(out, "{} ≈ {} for one request, re-building ~{estimate}{}", style.dim(&format!("{:<9}", "next")), usd(next), of_session(next));
        }
        return;
    }
    let label = style.dim(&format!("{:<9}", "cache"));
    let ttl = cache.ttl_name;
    if !cache.cold() {
        let left = human_duration((cache.ttl_ms - cache.idle_ms) as f64 / 1000.0);
        let _ = writeln!(out, "{label} {} · {ttl} TTL · cold in {left}", style.paint("32", "warm"));
    } else {
        let idle = human_duration((cache.idle_ms - cache.ttl_ms) as f64 / 1000.0);
        let _ = writeln!(out, "{label} {} for {idle} ({ttl} TTL)", style.paint("31", "cold"));
    }

    // What the next request will cost, against what the whole session cost so far.
    let (Some(warm), Some(cold)) = (cache.next_warm_usd, cache.next_cold_usd) else { return };
    let label = style.dim(&format!("{:<9}", "next"));
    if cache.cold() {
        let line = format!("≈ {} for one request, re-writing {}{}", usd(cold), tokens(size as f64), of_session(cold));
        let _ = writeln!(out, "{label} {}", style.paint("31", &line));
    } else {
        let _ = writeln!(
            out,
            "{label} ≈ {} per request while warm · ≈ {} once cold{}",
            style.paint("32", &usd(warm)),
            style.paint("31", &usd(cold)),
            of_session(cold)
        );
    }
}

fn advice_lines(out: &mut String, s: &Session, live: &LiveContext, cache: &CacheState, t: &Totals, pricing: &Pricing, style: &Style) {
    let requests = conversation(s, live);
    let billed: Vec<_> = requests.iter().filter(|r| r.usage.prompt() > 0 && !r.advisor).collect();
    let mut efforts: BTreeMap<&str, usize> = BTreeMap::new();
    for r in &billed {
        if let Some(e) = r.params.as_ref().and_then(|p| p.effort.as_deref()) {
            *efforts.entry(e).or_default() += 1;
        }
    }
    let mut misses: BTreeMap<Cause, (usize, f64)> = BTreeMap::new();
    for m in t.misses.iter().filter(|m| !m.cause.expected()) {
        let e = misses.entry(m.cause).or_default();
        e.0 += 1;
        e.1 += m.premium_usd.unwrap_or(0.0);
    }
    let inputs = advice::Inputs {
        session: s,
        live,
        price: pricing.get(&live.model, live.usage.prompt(), crate::pricing::now_ms()),
        cache,
        window: live.context_window.or_else(|| pricing.context_window(&live.model)),
        requests_per_prompt: billed.len() as f64 / live.turns.max(1) as f64,
        spend_usd: t.components.iter().map(|v| v.usd).sum(),
        thinking_usd: t.content.iter().filter(|((k, _, _), _)| *k == BlockKind::Thinking).map(|(_, v)| v.usd).sum(),
        effort: efforts.iter().max_by_key(|(_, n)| **n).map(|(e, _)| e.to_string()),
        misses: misses.into_iter().map(|(c, (n, u))| (c, n, u)).collect(),
        after_compaction: billed.iter().rev().find(|r| r.compacted_before).map(|r| r.usage.prompt()),
        first_prompt: billed.first().map(|r| r.usage.prompt()),
    };
    let lines = advice::advise(&inputs);
    if lines.is_empty() {
        return;
    }
    for (i, line) in lines.iter().enumerate() {
        let label = if i == 0 { style.dim(&format!("{:<9}", "advice")) } else { " ".repeat(9) };
        for (j, part) in wrap(line, advice::WIDTH).iter().enumerate() {
            let lead = if j == 0 { format!("{label} {} ", style.paint("33", "▸")) } else { " ".repeat(12) };
            let _ = writeln!(out, "{lead}{part}");
        }
    }
}

/// Splits text into lines of at most `width` characters at spaces. `≈` stays with the amount
/// after it.
pub(crate) fn wrap(text: &str, width: usize) -> Vec<String> {
    let mut words: Vec<String> = Vec::new();
    for word in text.split(' ') {
        match words.last_mut() {
            Some(last) if last.ends_with('≈') => {
                last.push(' ');
                last.push_str(word);
            }
            _ => words.push(word.to_string()),
        }
    }
    let mut lines = vec![String::new()];
    for word in &words {
        let line = lines.last_mut().expect("never empty");
        if !line.is_empty() && line.chars().count() + 1 + word.chars().count() > width {
            lines.push(word.to_string());
        } else {
            if !line.is_empty() {
                line.push(' ');
            }
            line.push_str(word);
        }
    }
    lines
}

fn spend_lines(out: &mut String, t: &Totals, style: &Style) {
    let total: f64 = t.components.iter().map(|v| if t.priced { v.usd } else { v.tokens }).sum();
    let show = |v: &Values| if t.priced { usd(v.usd) } else { format!("{} tok", tokens(v.tokens)) };
    let headline = if t.priced { format!("{} (API-equivalent)", usd(total)) } else { format!("{} tokens (no price)", tokens(total)) };
    let _ = writeln!(out, "{} {}", style.dim(&format!("{:<9}", "spend")), style.bold(&headline));
    for (c, v) in Component::ALL.iter().zip(&t.components) {
        let x = if t.priced { v.usd } else { v.tokens };
        let share = x / total.max(1e-9);
        if share < 0.0005 {
            continue;
        }
        let _ = writeln!(out, "  {:<19} {}  {:5.1}%  {:>9}", c.name(), style.paint(component_color(*c), &bar(share)), 100.0 * share, show(v));
    }
    let chains: Vec<String> = t.chains.iter().map(|(name, v)| format!("{name} {}", show(v))).collect();
    if chains.len() > 1 {
        let _ = writeln!(out, "  {}", style.dim(&chains.join(" · ")));
    }
}

/// Where the money went: by the kind of work, then by what the work was. Left out when the
/// session is a single task of typed prompts.
fn work_lines(out: &mut String, t: &Totals, style: &Style) {
    let (origins, tasks) = (t.work.origins(), t.work.tasks());
    if origins.len() < 2 && tasks.len() < 2 {
        return;
    }
    out.push('\n');
    let key = |s: &Spend| if t.priced { s.usd } else { s.tokens };
    let show = |s: &Spend| if t.priced { usd(s.usd) } else { format!("{} tok", tokens(s.tokens)) };
    let total: f64 = origins.iter().map(|(_, s)| key(s)).sum();
    let _ = writeln!(out, "{} {}", style.dim(&format!("{:<9}", "work")), style.dim("by origin: the kind of work each request was part of"));
    let mut rest = (0, Spend::default());
    for (i, (origin, s)) in origins.iter().enumerate() {
        if i >= 5 {
            rest.0 += 1;
            rest.1.add(*s);
            continue;
        }
        let share = key(s) / total.max(1e-9);
        let _ = writeln!(out, "  {:<24} {}  {:5.1}%  {:>9}", clip_end(origin, 24), style.paint("32", &bar(share)), 100.0 * share, show(s));
    }
    if rest.0 > 0 {
        let share = key(&rest.1) / total.max(1e-9);
        let _ = writeln!(out, "  {:<24} {}  {:5.1}%  {:>9}", format!("{} more", rest.0), style.paint("32", &bar(share)), 100.0 * share, show(&rest.1));
    }
    if tasks.len() > 1 {
        let _ = writeln!(out, "  {}", style.dim("by task: the session title, and what each subagent was asked to do"));
        for (task, origin, s) in tasks.iter().take(6) {
            let _ = writeln!(out, "  {:>9}  {:<50} {}", show(s), clip_end(task, 50), style.dim(&clip_end(origin, 26)));
        }
        if tasks.len() > 6 {
            let _ = writeln!(out, "  {}", style.dim(&format!("{} more tasks", tasks.len() - 6)));
        }
    }
}

fn composition_lines(out: &mut String, s: &Session, live: &LiveContext, trailing: Option<&Trailing>, style: &Style) {
    let total: f64 = live.blocks.iter().map(|(_, t, _)| t).sum();
    let turns = match live.turns {
        0 => String::new(),
        1 => ", 1 turn".to_string(),
        n => format!(", {n} turns"),
    };
    let before = trailing.map_or(String::new(), |t| format!(", before the compaction at {}", local_time(t.at_ms, "%H:%M")));
    let _ = writeln!(
        out,
        "{} {} tokens{turns}, as of {}{before}",
        style.dim(&format!("{:<9}", "in context")),
        tokens(total),
        local_time(live.ts_ms, "%H:%M")
    );
    if live.blocks.iter().all(|(b, _, _)| b.is_none()) {
        let _ = writeln!(out, "  {}", style.dim(&format!("{} logs do not show what the prompt holds yet", s.agent.name())));
        return;
    }
    let mut by_kind: BTreeMap<BlockKind, f64> = BTreeMap::new();
    for (b, t, _) in &live.blocks {
        *by_kind.entry(b.map_or(BlockKind::Unknown, |b| s.blocks[b].kind)).or_default() += t;
    }
    for (kind, t) in &by_kind {
        let share = t / total.max(1e-9);
        let _ = writeln!(out, "  {:<19} {}  {:5.1}%  {:>9}", kind.name(), style.paint("36", &bar(share)), 100.0 * share, tokens(*t));
    }
    let mut largest: Vec<&(Option<usize>, f64, u32)> = live.blocks.iter().filter(|(b, _, _)| b.is_some()).collect();
    largest.sort_by(|a, b| b.1.total_cmp(&a.1));
    if !largest.is_empty() {
        let _ = writeln!(out, "  {}", style.dim("largest blocks"));
    }
    for (b, t, turn) in largest.into_iter().take(6) {
        let blk = &s.blocks[b.expect("filtered")];
        let what = [blk.tool.as_deref(), blk.target.as_deref()].into_iter().flatten().collect::<Vec<_>>().join("  ");
        let when = if matches!(blk.kind, BlockKind::Tools | BlockKind::System) { "setup".to_string() } else { format!("turn {turn}") };
        let _ = writeln!(out, "  {:>9}  {:<14} {:<50} {}", tokens(*t), blk.kind.name(), clip(&what, 50), style.dim(&when));
    }
}

fn costly_content_lines(out: &mut String, t: &Totals, style: &Style) {
    let _ = writeln!(out, "{}", style.dim("what cost the most (every request it was part of, re-reads included)"));
    let mut rows: Vec<_> = t.content.iter().collect();
    let key = |v: &Values| if t.priced { v.usd } else { v.tokens };
    rows.sort_by(|a, b| key(b.1).total_cmp(&key(a.1)));
    for ((kind, tool, target), v) in rows.into_iter().take(8) {
        let what = [tool.as_deref(), target.as_deref()].into_iter().flatten().collect::<Vec<_>>().join("  ");
        let amount = if t.priced { usd(v.usd) } else { tokens(v.tokens) };
        let _ = writeln!(out, "  {amount:>9}  {:<14} {}", kind.name(), clip(&what, 60));
    }
}

fn miss_lines(out: &mut String, t: &Totals, style: &Style) {
    let misses: Vec<&MissEvent> = t.misses.iter().filter(|m| !m.cause.expected()).collect();
    let expected = t.misses.len() - misses.len();
    if t.misses.is_empty() {
        let _ = writeln!(out, "{} {}", style.dim(&format!("{:<9}", "cache")), style.paint("32", "no misses"));
        return;
    }
    let rewritten: u64 = misses.iter().map(|m| m.rewritten).sum();
    let premium: f64 = misses.iter().filter_map(|m| m.premium_usd).sum();
    let head = format!(
        "{} misses · re-processed {} tokens{}{}",
        misses.len(),
        tokens(rewritten as f64),
        if t.priced { format!(" · {} avoidable", usd(premium)) } else { String::new() },
        if expected > 0 { format!(" · {expected} expected rebuilds") } else { String::new() }
    );
    let _ = writeln!(out, "{} {}", style.dim(&format!("{:<9}", "cache")), if misses.is_empty() { head } else { style.paint("31", &head) });
    let mut by_cause: BTreeMap<Cause, (usize, f64)> = BTreeMap::new();
    for m in &misses {
        let e = by_cause.entry(m.cause).or_default();
        e.0 += 1;
        e.1 += m.premium_usd.unwrap_or(0.0);
    }
    let mut top: Vec<&&MissEvent> = misses.iter().collect();
    top.sort_by(|a, b| b.premium_usd.unwrap_or(0.0).total_cmp(&a.premium_usd.unwrap_or(0.0)).then(b.rewritten.cmp(&a.rewritten)));
    for m in top.into_iter().take(5) {
        let cause = if m.detail.is_empty() { m.cause.name().to_string() } else { format!("{}: {}", m.cause.name(), m.detail) };
        let _ = writeln!(
            out,
            "  {}  {:<44} {:>8}  {:>8}  {}",
            local_time(m.ts_ms, "%m-%d %H:%M"),
            clip(&cause, 44),
            tokens(m.rewritten as f64),
            m.premium_usd.map_or("-".to_string(), usd),
            style.dim(&m.chain)
        );
    }
}

fn clip(s: &str, width: usize) -> String {
    if s.chars().count() <= width {
        return s.to_string();
    }
    let tail: String = s.chars().rev().take(width - 1).collect::<Vec<_>>().into_iter().rev().collect();
    format!("…{tail}")
}

/// Project and session id from the first lines of a session file.
fn quick_meta(file: &SourceFile) -> (String, String) {
    let (mut project, mut id) = (String::new(), String::new());
    let Ok(reader) = adapters::open(&file.path) else { return (project, id) };
    for line in reader.lines().take(200).map_while(Result::ok) {
        if !line.contains("\"cwd\"") && !line.contains("essionId") && !line.contains("session_meta") {
            continue;
        }
        let Ok(v) = serde_json::from_str::<Value>(&line) else { continue };
        let payload = v.get("payload").unwrap_or(&v);
        if project.is_empty()
            && let Some(cwd) = payload.get("cwd").and_then(Value::as_str) {
                project = adapters::display_path(cwd);
            }
        if id.is_empty()
            && let Some(s) = v.get("sessionId").or_else(|| payload.get("id")).and_then(Value::as_str) {
                id = s.chars().take(8).collect();
            }
        if !project.is_empty() && !id.is_empty() {
            break;
        }
    }
    if id.is_empty() {
        let name = match file.agent {
            Agent::Codex => adapters::codex::id_from_name(&file.path),
            Agent::Claude => file.path.file_stem().and_then(|s| s.to_str()).unwrap_or_default().to_string(),
        };
        id = name.chars().take(8).collect();
    }
    (project, id)
}

/// A session as listed: its main file, subagent files and what the first lines tell.
#[derive(Clone)]
pub struct SessionRow {
    pub file: SourceFile,
    pub subagents: Vec<SourceFile>,
    pub project: String,
    pub id: String,
    pub bytes: u64,
}

impl SessionRow {
    /// The row of the session whose main log this is.
    pub fn new(file: SourceFile) -> SessionRow {
        let (project, id) = quick_meta(&file);
        let mut subagents = file.subagent_files();
        subagents.sort_by(|a, b| a.path.cmp(&b.path));
        let bytes = file.size + subagents.iter().map(|s| s.size).sum::<u64>();
        SessionRow { file, subagents, project, id, bytes }
    }

    pub fn modified_ms(&self) -> i64 {
        self.file.modified.duration_since(SystemTime::UNIX_EPOCH).map_or(0, |d| d.as_millis() as i64)
    }

    fn files(&self) -> impl Iterator<Item = &SourceFile> {
        std::iter::once(&self.file).chain(&self.subagents)
    }
}

/// Session rows, newest first.
pub fn rows(agents: &[Agent], limit: usize) -> Vec<SessionRow> {
    let files: Vec<SourceFile> = main_sessions(agents).into_iter().take(limit).collect();
    files.into_par_iter().map(SessionRow::new).collect()
}

/// A session's cost with its subagents, and what its next request would cost.
#[derive(Clone, Copy, Debug, Default)]
pub struct Glance {
    pub cost: f64,
    pub next_warm: Option<f64>,
    pub next_cold: Option<f64>,
    /// When the main conversation's cache goes cold, in epoch ms.
    pub cold_at_ms: i64,
}

impl Glance {
    /// The next request's cost at `now_ms`, and whether the cache is cold by then.
    pub fn next(&self, now_ms: i64) -> Option<(f64, bool)> {
        let cold = now_ms >= self.cold_at_ms;
        if cold { self.next_cold } else { self.next_warm }.map(|usd| (usd, cold))
    }
}

pub fn glance(row: &SessionRow, pricing: &Pricing) -> Glance {
    let mut t = Totals::default();
    let mut main = None;
    for (k, file) in row.files().enumerate() {
        if let Ok(s) = adapters::parse(file) {
            analysis::analyze(&s, pricing, &Filter::default(), &mut t);
            if k == 0 {
                main = Some(s);
            }
        }
    }
    let mut g = Glance { cost: t.components.iter().map(|v| v.usd).sum(), cold_at_ms: i64::MAX, ..Glance::default() };
    if let Some((s, live)) = main.as_ref().and_then(|s| analysis::live_context(s).map(|l| (s, l))) {
        let cache = cache_state(s, &live, pricing);
        g.next_warm = cache.next_warm_usd;
        g.next_cold = cache.next_cold_usd;
        g.cold_at_ms = live.ts_ms + cache.ttl_ms;
    }
    g
}

/// The next request's cost as `$3.17 14%`: its share of what the session cost so far.
pub fn next_cell(g: &Glance, now: i64) -> Option<(String, bool)> {
    let (usd_next, cold) = g.next(now)?;
    let share = if g.cost > 0.0 { format!(" {:>3.0}%", (100.0 * usd_next / g.cost).min(999.0)) } else { "     ".to_string() };
    Some((format!("{}{share}", usd(usd_next)), cold))
}

/// Writes a session's profile to `path` (.jfr or .html), the same one `tokprof flame --session`
/// writes; returns what was written.
pub fn export(row: &SessionRow, pricing: &Pricing, path: &Path) -> Result<String> {
    use crate::export::{self as profile, HtmlOptions, Measure};
    use crate::report::{Collector, DEFAULT_STACK, DEFAULT_THREADS, dims};
    let jfr = path.extension().is_some_and(|e| e == "jfr");
    let mut c = Collector::new(Some(dims(DEFAULT_STACK)), None);
    if jfr {
        c.threads = Some(dims(DEFAULT_THREADS));
    }
    for file in row.files() {
        analysis::analyze(&adapters::parse(file)?, pricing, &Filter::default(), &mut c);
    }
    if jfr {
        let w = crate::jfr::write(path, &c.timed, Measure::Cost)?;
        Ok(format!("{} samples (1 = ${}) to {}", w.samples, w.quantum as f64 / 1e6, adapters::display_path(&path.to_string_lossy())))
    } else {
        let title = format!("tokprof: {} {} {}", row.file.agent.name(), row.project, row.id);
        let n = profile::write(path, &c.stacks, Measure::Cost, &HtmlOptions { title: &title, highlight: None })?;
        Ok(format!("{n} frames to {}", adapters::display_path(&path.to_string_lossy())))
    }
}

pub fn sessions(agents: &[Agent], limit: usize, cost: bool, pricing: &Pricing, style: &Style) -> String {
    let rows = rows(agents, limit);
    let glances: Vec<Option<Glance>> = rows.par_iter().map(|r| cost.then(|| glance(r, pricing))).collect();
    let now = now_ms();
    let mut out = String::new();
    let (cost_head, next_head) = if cost { ("cost", "next request") } else { ("", "") };
    let _ = writeln!(out, "{}", style.dim(&format!("{:<16}  {:<6}  {:<8}  {:>8}  {:>9}  {:<12}  project", "modified", "agent", "session", "size", cost_head, next_head)));
    for (r, g) in rows.iter().zip(glances) {
        let extra = if r.subagents.is_empty() { String::new() } else { style.dim(&format!("  +{} subagents", r.subagents.len())) };
        let next = g.as_ref().and_then(|g| next_cell(g, now)).map_or_else(
            || " ".repeat(12),
            |(cell, cold)| style.paint(if cold { "31" } else { "32" }, &format!("{cell:<12}")),
        );
        let _ = writeln!(
            out,
            "{}  {:<6}  {:<8}  {:>8}  {:>9}  {next}  {}{extra}",
            local_time(r.modified_ms(), "%Y-%m-%d %H:%M"),
            r.file.agent.name(),
            r.id,
            format!("{:.1} MB", r.bytes as f64 / 1e6),
            g.map_or(String::new(), |g| usd(g.cost)),
            r.project
        );
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Block, Chain, ChainKind, Compaction, Usage};

    const MIN: i64 = 60_000;

    /// da18a2d8 on 2026-10-29: a 440.1k-token prompt at 18:18, 40% of it tool results, then a
    /// compaction at 19:13 with no request after it. With `earlier`, its previous compaction was
    /// followed by a 55.8k-token prompt.
    fn compacted_session(kind: &str, earlier: bool) -> (Session, LiveContext) {
        let mut s = Session::new(Agent::Claude, "s".into());
        let kinds = [(BlockKind::Tools, 10_000.0), (BlockKind::System, 4_400.0), (BlockKind::ToolResult, 176_040.0), (BlockKind::Text, 249_660.0)];
        s.blocks = kinds.iter().map(|&(kind, _)| Block { kind, chars: 1, tool: None, target: None }).collect();
        let req = |ts_ms, write_long, compacted_before| Request {
            ts_ms,
            model: "claude-opus-5-5".into(),
            usage: Usage { cache_read: 1_000, write_long, output: 1_000, ..Usage::default() },
            compacted_before,
            ..Request::default()
        };
        let mut requests = vec![req(0, 300_000, false)];
        if earlier {
            requests.push(req(10 * MIN, 54_800, true));
        }
        requests.push(req(60 * MIN, 439_100, false));
        s.chains.push(Chain { name: "main".into(), kind: ChainKind::Main, requests });
        if earlier {
            s.compactions.push(Compaction { ts_ms: 5 * MIN, kind: Some("auto".into()) });
        }
        s.compactions.push(Compaction { ts_ms: 115 * MIN, kind: Some(kind.into()) });
        let live = LiveContext {
            chain: "main".into(),
            ts_ms: 60 * MIN,
            model: "claude-opus-5-5".into(),
            usage: Usage { cache_read: 1_000, write_long: 439_100, ..Usage::default() },
            context_window: Some(1_000_000),
            blocks: kinds.iter().enumerate().map(|(n, &(_, t))| (Some(n), t, 1)).collect(),
            turns: 30,
        };
        (s, live)
    }

    #[test]
    fn a_trailing_compaction_sets_the_next_prompt() {
        let pricing = Pricing::defaults();
        let (s, live) = compacted_session("idle", true);
        let t = trailing_compaction(&s, &live, &pricing).expect("a trailing compaction");
        assert_eq!((t.at_ms, t.kind.as_deref(), t.estimate, t.setup), (115 * MIN, Some("idle"), 55_800.0, 14_400.0));
        // Without an earlier compaction: the setup plus 20,000 tokens.
        let (s, live) = compacted_session("idle", false);
        assert_eq!(trailing_compaction(&s, &live, &pricing).unwrap().estimate, 34_400.0);
        // A compaction before the latest request is not trailing.
        let (mut s, live) = compacted_session("idle", true);
        s.compactions.pop();
        assert!(trailing_compaction(&s, &live, &pricing).is_none());
    }

    #[test]
    fn the_forecast_after_a_compaction_reads_the_setup_and_writes_the_rest() {
        let p = Pricing::defaults().get("claude-opus-5-5", 55_800, crate::pricing::now_ms()).unwrap();
        // (14,400 × $0.20 + 43,400 × $8 + 1,000 × $20) per million.
        assert!((next_after_compaction(55_800.0, 14_400.0, 2_000.0, 1_000.0, p, true) - 0.370_08).abs() < 1e-9);
    }

    #[test]
    fn the_x_ray_names_a_trailing_compaction() {
        let pricing = Pricing::defaults();
        let plain = Style { color: false };
        let render = |kind: &str| {
            let (s, live) = compacted_session(kind, true);
            let t = trailing_compaction(&s, &live, &pricing).unwrap();
            let shown = compacted_live(&s, &live, &t);
            let cache = cache_state(&s, &shown, &pricing);
            let mut out = String::new();
            context_lines(&mut out, &live, &cache, Some(&t), 414.0, &pricing, &plain);
            composition_lines(&mut out, &s, &live, Some(&t), &plain);
            out
        };
        let at = local_time(115 * MIN, "%H:%M");
        let idle = render("idle");
        assert!(idle.contains("~55.8k of 1.0M"), "{idle}");
        assert!(idle.contains(&format!("cache     compacted while idle at {at} · the next request re-builds ~55.8k")), "{idle}");
        // The sum itself is checked in `the_forecast_after_a_compaction_reads_the_setup_and_writes_the_rest`.
        assert!(idle.contains("for one request, re-building ~55.8k = ") && idle.contains("of the session's $414"), "{idle}");
        assert!(!idle.contains("warm") && !idle.contains("cold"), "{idle}");
        assert!(idle.contains(&format!(", before the compaction at {at}")), "{idle}");
        let auto = render("auto");
        assert!(auto.contains(&format!("cache     compacted at {at} (auto) · the next request re-builds ~55.8k")), "{auto}");
    }

    #[test]
    fn advice_after_a_compaction_sees_the_next_prompt() {
        let pricing = Pricing::defaults();
        let (s, live) = compacted_session("idle", true);
        let t = trailing_compaction(&s, &live, &pricing).unwrap();
        let advise = |live: &LiveContext| {
            let cache = cache_state(&s, live, &pricing);
            advice::advise(&advice::Inputs {
                session: &s,
                live,
                price: pricing.get(&live.model, live.usage.prompt(), crate::pricing::now_ms()),
                cache: &cache,
                window: live.context_window,
                requests_per_prompt: 3.0,
                spend_usd: 414.0,
                thinking_usd: 100.0,
                effort: None,
                misses: vec![(Cause::HistoryEdited, 44, 154.0)],
                after_compaction: Some(55_800),
                first_prompt: Some(301_000),
            })
        };
        let fires = |lines: &[String], start: &str| lines.iter().any(|l| l.starts_with(start));
        // The compacted prompt: carry-on, context cost and the tool-results hint fire.
        let before = advise(&live);
        assert!(fires(&before, "Resuming writes") && fires(&before, "Tool results"), "{before:?}");
        let after = advise(&compacted_live(&s, &live, &t));
        assert!(!fires(&after, "Resuming writes") && !fires(&after, "Cache warm") && !fires(&after, "Tool results"), "{after:?}");
        assert!(!after.iter().any(|l| l.contains("of the window") || l.contains("re-reads")), "{after:?}");
        assert!(fires(&after, "Thinking cost") && fires(&after, "Cache misses"), "{after:?}");
    }

    /// 0f54de16 ran on 10-12 and 10-13 without request-audit lines and was resumed on 10-25
    /// with them. Read from the audited chain alone, its first prompt was the 721k re-write.
    #[test]
    fn reads_both_chains_of_a_conversation() {
        let req = |ts_ms, uncached| Request { ts_ms, usage: Usage { uncached, ..Usage::default() }, ..Request::default() };
        let mut s = Session::new(Agent::Claude, "s".into());
        s.chains.push(Chain { name: "main".into(), kind: ChainKind::Main, requests: vec![req(30, 721_000), req(31, 724_000)] });
        s.chains.push(Chain { name: "main".into(), kind: ChainKind::Main, requests: vec![req(10, 34_800), req(20, 40_000)] });
        s.chains.push(Chain { name: "subagent".into(), kind: ChainKind::Subagent, requests: vec![req(15, 5_000)] });
        let live = LiveContext {
            chain: "main".into(),
            ts_ms: 31,
            model: String::new(),
            usage: Usage::default(),
            context_window: None,
            blocks: Vec::new(),
            turns: 2,
        };
        let prompts: Vec<u64> = conversation(&s, &live).iter().map(|r| r.usage.prompt()).collect();
        assert_eq!(prompts, [34_800, 40_000, 721_000, 724_000]);

        // A chain of the same name that overlaps both parts in time is another conversation.
        s.chains.push(Chain { name: "main".into(), kind: ChainKind::Main, requests: vec![req(15, 70_000), req(35, 80_000)] });
        let live = LiveContext { ts_ms: 31, ..live };
        let prompts: Vec<u64> = conversation(&s, &live).iter().map(|r| r.usage.prompt()).collect();
        assert_eq!(prompts, [34_800, 40_000, 721_000, 724_000]);
    }

    /// Exports the latest local session; needs session logs, so run with `--ignored`.
    #[test]
    #[ignore]
    fn exports_the_latest_session() {
        let row = rows(&Agent::ALL, 1).pop().expect("a local session");
        let dir = std::env::temp_dir().join("tokprof-test");
        std::fs::create_dir_all(&dir).unwrap();
        for ext in ["jfr", "html"] {
            let path = dir.join(format!("{}.{ext}", row.id));
            let written = export(&row, &Pricing::defaults(), &path).unwrap();
            assert!(path.metadata().unwrap().len() > 1000, "{written}");
            println!("{written}");
        }
    }
}
