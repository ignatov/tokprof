//! `tokprof bench`: what it costs to come back to a big session after its prompt cache
//! expired, and what the same work costs after `/compact`, in a new session that refers to
//! the old one, or with heartbeats that keep the cache warm.
//!
//! Every cold return in the logs is a measured case. It is a request of a main conversation
//! with a context of at least `min_context` tokens that came after an idle gap longer than the
//! cache lifetime and had to write that context again. The bench takes it and the requests
//! that followed, up to `requests` of them and until the next such return or a compaction, and
//! prices the same requests four ways:
//! - **resume as is**: as logged.
//! - **/compact first**: a compaction call, then the requests on top of what a compaction
//!   leaves. Claude Code does not log the call. It runs as a fork that shares the
//!   conversation's cache, so on a cold cache it writes the whole context again, and then it
//!   outputs a summary.
//! - **new session + handoff note**: the old session writes a note while its cache is still
//!   warm, and a new session does the requests on top of the same base.
//! - **heartbeats, then resume**: while the session is idle, a ping at 5/6 of the cache
//!   lifetime reads the context, which resets the cache timer, for up to `keep_warm` after the
//!   last request. A return within the lifetime of the last ping reads its context from the
//!   cache and then goes on as logged. A later return is cold anyway, after the pings.
//!
//! The replay keeps each request's own cache behavior and changes only the old conversation
//! in its prompt. That span shrinks to the base, and its read, written and uncached parts
//! shrink with it. The base is the median prompt right after the agent's compactions in the
//! logs. It is what the agent reloads to carry on: the summary, the files it was working on and
//! the task list, on top of the tools and system prompt.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::advice::SUMMARY_TOKENS;
use crate::analysis::Filter;
use crate::analysis::cache::{self, human_duration};
use crate::model::{Agent, Request, Session, Usage};
use crate::pricing::Pricing;
use crate::report::{table, tokens, usd};
use crate::xray::{self, Style};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Strategy {
    Resume,
    Compact,
    Handoff,
    Heartbeats,
}

impl Strategy {
    pub const ALL: [Strategy; 4] = [Strategy::Resume, Strategy::Compact, Strategy::Handoff, Strategy::Heartbeats];

    pub fn name(self) -> &'static str {
        match self {
            Strategy::Resume => "resume as is",
            Strategy::Compact => "/compact first",
            Strategy::Handoff => "new session + handoff note",
            Strategy::Heartbeats => "heartbeats, then resume",
        }
    }
}

/// Output tokens of one heartbeat: a short reply to the ping, which the conversation drops.
const PING_OUTPUT: f64 = 100.0;

/// The pings that keep a cache of lifetime `ttl_ms` warm through a break of `idle_ms`: one at
/// 5/6 of the lifetime after the last request or ping, while the break lasts and for at most
/// `window_ms`. Returns their times since the last request, and whether the return finds the
/// cache still warm.
fn heartbeats(idle_ms: i64, ttl_ms: i64, window_ms: i64) -> (Vec<i64>, bool) {
    let every = ping_every(ttl_ms);
    let times: Vec<i64> = (1..).map(|k| k * every).take_while(|&t| t < idle_ms && t <= window_ms).collect();
    let warm = idle_ms - times.last().copied().unwrap_or(0) <= ttl_ms;
    (times, warm)
}

/// How often heartbeats go out for a cache of lifetime `ttl_ms`: every 50 minutes on a 1-hour
/// cache, as the cache-tax mod does.
fn ping_every(ttl_ms: i64) -> i64 {
    (ttl_ms * 5 / 6).max(1)
}

/// A heartbeat window as the report names it: 6h, 90m.
fn window_name(ms: i64) -> String {
    let minutes = ms / 60_000;
    if minutes % 60 == 0 { format!("{}h", minutes / 60) } else { format!("{minutes}m") }
}

/// A request as logged.
#[derive(Clone, Debug)]
struct Step {
    ts_ms: i64,
    model: String,
    usage: Usage,
}

/// A cold return found in a session, before it is priced.
#[derive(Debug)]
struct Case {
    agent: Agent,
    /// The last request before the break.
    before_ms: i64,
    idle_ms: i64,
    context: u64,
    long_ttl: bool,
    /// Tokens added and output per request before the break, as the X-ray's forecast sees them.
    pace: (f64, f64),
    /// The return and the requests after it.
    steps: Vec<Step>,
}

/// What the sessions of a bench run hold: cold returns, and the context sizes the
/// alternatives start from.
#[derive(Default)]
pub struct Found {
    cases: Vec<Case>,
    /// Prompt size right after a compaction of a big context, per agent.
    compacted: Vec<(Agent, u64)>,
    /// First prompt of each main conversation, per agent.
    starts: Vec<(Agent, u64)>,
}

impl Found {
    pub fn merge(mut self, other: Found) -> Found {
        self.cases.extend(other.cases);
        self.compacted.extend(other.compacted);
        self.starts.extend(other.starts);
        self
    }
}

/// The request came after the cache expired and wrote the previous prompt again.
fn cold_return(agent: Agent, long_ttl: bool, prev: &Request, r: &Request) -> bool {
    r.ts_ms - prev.ts_ms > cache::ttl(agent, long_ttl).0 && cache::rewritten(prev.usage.prompt(), &r.usage).is_some()
}

/// Finds the cold returns of a session's main conversations.
pub fn scan(s: &Session, filter: &Filter, requests: usize, min_context: u64) -> Found {
    let mut found = Found::default();
    for (_, conversation) in s.main_conversations() {
        let billed: Vec<usize> = (0..conversation.len()).filter(|&i| conversation[i].usage.prompt() > 0 && !conversation[i].advisor).collect();
        if let Some(&first) = billed.first() {
            found.starts.push((s.agent, conversation[first].usage.prompt()));
        }
        for (k, pair) in billed.windows(2).enumerate() {
            let (prev, r) = (&conversation[pair[0]], &conversation[pair[1]]);
            if r.compacted_before {
                if prev.usage.prompt() >= min_context {
                    found.compacted.push((s.agent, r.usage.prompt()));
                }
                continue;
            }
            let history = &conversation[..=pair[0]];
            let long_ttl = xray::long_ttl(history);
            if prev.usage.prompt() < min_context || r.model != prev.model || !filter.accepts(r.ts_ms) || !cold_return(s.agent, long_ttl, prev, r) {
                continue;
            }
            let mut steps = vec![r];
            let mut lifetime = long_ttl;
            for &i in &billed[k + 2..] {
                let (last, x) = (steps[steps.len() - 1], &conversation[i]);
                if last.usage.cache_write() > 0 {
                    lifetime = last.usage.write_long > last.usage.write_short;
                }
                if steps.len() >= requests || x.compacted_before || cold_return(s.agent, lifetime, last, x) {
                    break;
                }
                steps.push(x);
            }
            found.cases.push(Case {
                agent: s.agent,
                before_ms: prev.ts_ms,
                idle_ms: r.ts_ms - prev.ts_ms,
                context: prev.usage.prompt(),
                long_ttl,
                pace: xray::pace(history),
                steps: steps.into_iter().map(|x| Step { ts_ms: x.ts_ms, model: x.model.clone(), usage: x.usage }).collect(),
            });
        }
    }
    found
}

/// One measured return, priced four ways.
pub struct Return {
    pub agent: Agent,
    pub context: u64,
    pub idle_ms: i64,
    pub long_ttl: bool,
    /// Cost after each request since the return, per strategy. `[0]` is what the strategy
    /// spends before the first request: the compaction call, the note, or the pings.
    pub cumulative: [Vec<f64>; 4],
    /// The X-ray's forecast of the first request once the cache is cold.
    pub forecast: f64,
    /// How many heartbeats went out during the break, and whether they kept the cache warm
    /// until the return.
    pub pings: usize,
    pub kept_warm: bool,
}

impl Return {
    pub fn requests(&self) -> usize {
        self.cumulative[0].len() - 1
    }

    /// Cost of the first `k` requests, or of all of them if there were fewer.
    pub fn after(&self, s: Strategy, k: usize) -> f64 {
        let c = &self.cumulative[s as usize];
        c[k.min(c.len() - 1)]
    }
}

/// The context the alternatives start from.
#[derive(Clone, Copy, Debug)]
pub struct Base {
    pub tokens: u64,
    /// How many compactions the median comes from; 0 when there were none, and the base is a
    /// first prompt plus a note.
    pub compactions: usize,
}

pub struct Bench {
    pub requests: usize,
    pub min_context: u64,
    /// How long heartbeats keep pinging after the last request.
    pub keep_warm_ms: i64,
    pub returns: Vec<Return>,
    pub base: BTreeMap<Agent, Base>,
}

impl Bench {
    pub fn of(&self, agent: Agent) -> Vec<&Return> {
        self.returns.iter().filter(|r| r.agent == agent).collect()
    }
}

fn median(mut v: Vec<u64>) -> Option<u64> {
    v.sort_unstable();
    v.get(v.len() / 2).copied()
}

/// Prices every return four ways.
pub fn replay(found: Found, pricing: &Pricing, requests: usize, min_context: u64, keep_warm_ms: i64) -> Bench {
    let mut base = BTreeMap::new();
    for agent in Agent::ALL {
        let compacted: Vec<u64> = found.compacted.iter().filter(|(a, _)| *a == agent).map(|(_, t)| *t).collect();
        let n = compacted.len();
        let tokens = match median(compacted) {
            Some(t) => Some(t),
            None => median(found.starts.iter().filter(|(a, _)| *a == agent).map(|(_, t)| *t).collect()).map(|t| t + SUMMARY_TOKENS as u64),
        };
        if let Some(tokens) = tokens {
            base.insert(agent, Base { tokens, compactions: n });
        }
    }
    let returns = found.cases.iter().filter_map(|c| price(c, base.get(&c.agent)?.tokens, pricing, keep_warm_ms)).collect();
    Bench { requests, min_context, keep_warm_ms, returns, base }
}

fn price(c: &Case, base: u64, pricing: &Pricing, keep_warm_ms: i64) -> Option<Return> {
    let first = &c.steps[0];
    let p = pricing.get(&first.model, c.context, first.ts_ms)?;
    let note = pricing.get(&first.model, c.context, c.before_ms).unwrap_or(p);
    let ctx = c.context as f64;
    // The tools and system prompt other sessions keep warm: the return still reads them.
    let shared = (first.usage.cache_read as f64).min(base as f64).min(ctx);
    let keep = if ctx > shared { ((base as f64).min(ctx) - shared).max(0.0) / (ctx - shared) } else { 1.0 };
    let write = if c.long_ttl { p.write_long } else { p.write_short };
    // Each ping reads the context at the price of its own time.
    let (pings, kept_warm) = heartbeats(c.idle_ms, cache::ttl(c.agent, c.long_ttl).0, keep_warm_ms);
    let pinged: f64 = pings
        .iter()
        .filter_map(|t| pricing.get(&first.model, c.context, c.before_ms + t))
        .map(|at| (ctx * at.read + PING_OUTPUT * at.output) / 1e6)
        .sum();
    let up_front = [
        0.0,
        (shared * p.read + (ctx - shared) * write + SUMMARY_TOKENS * p.output) / 1e6,
        (ctx * note.read + SUMMARY_TOKENS * note.output) / 1e6,
        pinged,
    ];
    let cumulative = Strategy::ALL.map(|s| {
        let keep = if matches!(s, Strategy::Resume | Strategy::Heartbeats) { 1.0 } else { keep };
        let mut total = up_front[s as usize];
        let mut out = vec![total];
        for (i, step) in c.steps.iter().enumerate() {
            let u = if s == Strategy::Heartbeats && i == 0 && kept_warm { warmed(step.usage, c.context) } else { shrink(step.usage, shared, ctx, keep) };
            total += pricing.get(&step.model, u.prompt(), step.ts_ms).map_or(0.0, |p| p.cost(&u));
            out.push(total);
        }
        out
    });
    let (added, output) = c.pace;
    Some(Return {
        agent: c.agent,
        context: c.context,
        idle_ms: c.idle_ms,
        long_ttl: c.long_ttl,
        cumulative,
        forecast: xray::next_request(ctx, added, output, p, c.long_ttl).1,
        pings: pings.len(),
        kept_warm,
    })
}

/// The usage of the first request back had the cache stayed warm: it reads the previous
/// prompt, `context` tokens, from the cache, and writes or sends uncached only what is new.
fn warmed(u: Usage, context: u64) -> Usage {
    let moved = context.min(u.prompt()).saturating_sub(u.cache_read);
    let written = moved.min(u.cache_write());
    let long = if u.cache_write() > 0 { (written as f64 * u.write_long as f64 / u.cache_write() as f64).round() as u64 } else { 0 };
    let long = long.min(u.write_long);
    let short = (written - long).min(u.write_short);
    let uncached = (moved - written).min(u.uncached);
    Usage {
        cache_read: u.cache_read + long + short + uncached,
        write_short: u.write_short - short,
        write_long: u.write_long - long,
        uncached: u.uncached - uncached,
        ..u
    }
}

/// The usage of a request whose old conversation, the span from `shared` to `context` of its
/// prompt, is scaled by `keep`. The span's read, written and uncached parts shrink with it.
fn shrink(u: Usage, shared: f64, context: f64, keep: f64) -> Usage {
    if keep >= 1.0 {
        return u;
    }
    let t = u.prompt() as f64;
    let end = context.min(t);
    let (r, w) = (u.cache_read as f64, u.cache_write() as f64);
    let old = |a: f64, z: f64| (z.min(end) - a.max(shared)).max(0.0) * (1.0 - keep);
    let (read, written, uncached) = (old(0.0, r), old(r, r + w), old(r + w, t));
    let long = if w > 0.0 { u.write_long as f64 / w } else { 0.0 };
    let less = |x: u64, by: f64| (x as f64 - by).round().max(0.0) as u64;
    Usage {
        cache_read: less(u.cache_read, read),
        write_short: less(u.write_short, written * (1.0 - long)),
        write_long: less(u.write_long, written * long),
        uncached: less(u.uncached, uncached),
        ..u
    }
}

/// Mean cost per return after each of the first `n` requests.
pub fn mean_curve(returns: &[&Return], s: Strategy, n: usize) -> Vec<f64> {
    (0..=n).map(|k| returns.iter().map(|r| r.after(s, k)).sum::<f64>() / returns.len().max(1) as f64).collect()
}

/// The first request count after which `a` costs less than `b`, on average.
fn break_even(returns: &[&Return], a: Strategy, b: Strategy, n: usize) -> Option<usize> {
    let (ca, cb) = (mean_curve(returns, a, n), mean_curve(returns, b, n));
    (1..=n).find(|&k| ca[k] < cb[k])
}

pub fn report(b: &Bench, style: &Style) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "tokprof bench · coming back to a big session after its prompt cache expired\n");
    if b.returns.is_empty() {
        let _ = writeln!(out, "No cold returns to contexts of {}+ tokens in these logs.", round_tokens(b.min_context));
        return out;
    }
    let n = b.requests;
    for agent in Agent::ALL {
        let returns = b.of(agent);
        if returns.is_empty() {
            continue;
        }
        let ctx = median(returns.iter().map(|r| r.context).collect()).unwrap_or(0);
        let idle = median(returns.iter().map(|r| r.idle_ms as u64).collect()).unwrap_or(0);
        let len = median(returns.iter().map(|r| r.requests() as u64).collect()).unwrap_or(0);
        let _ = writeln!(
            out,
            "{} {} returns to contexts of {}+ tokens (median {}) after a break of {} (median);",
            style.bold(&format!("{:<6}", agent.name())),
            returns.len(),
            round_tokens(b.min_context),
            tokens(ctx as f64),
            human_duration(idle as f64 / 1000.0),
        );
        let _ = writeln!(out, "       each followed for up to {n} requests, until the next break or compaction (median {len})\n");
        let marks = [1, 10, n];
        let mut rows = vec![
            ["mean per return".to_string(), "1st request".to_string(), "10 requests".to_string(), format!("{n} requests"), "all returns".to_string()].to_vec(),
        ];
        for s in Strategy::ALL {
            let mut row = vec![s.name().to_string()];
            row.extend(marks.iter().map(|&k| usd(returns.iter().map(|r| r.after(s, k)).sum::<f64>() / returns.len() as f64)));
            row.push(usd(returns.iter().map(|r| r.after(s, n)).sum()));
            rows.push(row);
        }
        for line in table(&rows, 1).lines() {
            let _ = writeln!(out, "  {line}");
        }
        let total = |s| returns.iter().map(|r| r.after(s, n)).sum::<f64>();
        let (resume, compact, handoff) = (total(Strategy::Resume), total(Strategy::Compact), total(Strategy::Handoff));
        let _ = writeln!(
            out,
            "\n  A handoff note before the break would have saved {} of {} ({:.0}%).",
            usd(resume - handoff),
            usd(resume),
            100.0 * (resume - handoff) / resume.max(1e-9)
        );
        let mut saved: Vec<(f64, f64)> = returns
            .iter()
            .map(|r| (r.after(Strategy::Resume, n) - r.after(Strategy::Handoff, n), 1.0 - r.after(Strategy::Handoff, n) / r.after(Strategy::Resume, n).max(1e-9)))
            .collect();
        let cheaper = saved.iter().filter(|(usd, _)| *usd > 0.0).count();
        saved.sort_by(|a, b| a.0.total_cmp(&b.0));
        let (median_usd, _) = saved[saved.len() / 2];
        saved.sort_by(|a, b| a.1.total_cmp(&b.1));
        let (_, median_share) = saved[saved.len() / 2];
        let _ = writeln!(
            out,
            "  Per return it saves a median of {} ({:.0}%), and it is cheaper in {cheaper} of {} returns.",
            usd(median_usd),
            100.0 * median_share,
            returns.len()
        );
        match break_even(&returns, Strategy::Compact, Strategy::Resume, n) {
            Some(k) => {
                let _ = writeln!(out, "  /compact first costs more up front and is cheaper than resuming from request {k} on ({} over all).", usd(resume - compact));
            }
            None => {
                let _ = writeln!(out, "  /compact first does not pay off within {n} requests.");
            }
        }
        let heartbeats = total(Strategy::Heartbeats);
        let warm = returns.iter().filter(|r| r.kept_warm).count();
        let pinged: f64 = returns.iter().map(|r| r.after(Strategy::Heartbeats, 0)).sum();
        let change = if heartbeats <= resume {
            format!("saved {} of {} ({:.0}%)", usd(resume - heartbeats), usd(resume), 100.0 * (resume - heartbeats) / resume.max(1e-9))
        } else {
            format!("cost {} more than resuming", usd(heartbeats - resume))
        };
        let _ = writeln!(
            out,
            "  Heartbeats for up to {} would have kept {warm} of {} returns warm, for {} of pings, and {change}.",
            window_name(b.keep_warm_ms),
            returns.len(),
            usd(pinged)
        );
        let below_note = returns.iter().filter(|r| r.after(Strategy::Heartbeats, n) < r.after(Strategy::Handoff, n)).count();
        let _ = writeln!(out, "  They cost less than a handoff note in {below_note} of {} returns.", returns.len());
        let errors: Vec<f64> = returns.iter().map(|r| (r.forecast - r.after(Strategy::Resume, 1)).abs() / r.after(Strategy::Resume, 1).max(1e-9)).collect();
        let within = errors.iter().filter(|e| **e <= 0.1).count();
        let _ = writeln!(
            out,
            "  The X-ray's forecast of the first request was within 10% of its logged cost for {within} of {} returns ({:.0}%).\n",
            errors.len(),
            100.0 * within as f64 / errors.len() as f64
        );
    }
    let bases: Vec<String> = b
        .base
        .iter()
        .filter(|(a, _)| !b.of(**a).is_empty())
        .map(|(a, base)| match base.compactions {
            0 => format!("{} {} (first prompt + note)", a.name(), tokens(base.tokens as f64)),
            c => format!("{} {} (median of {c} compactions)", a.name(), tokens(base.tokens as f64)),
        })
        .collect();
    let _ = writeln!(out, "{}", style.dim(&format!("The alternatives start from what a compaction leaves: {}.", bases.join(", "))));
    let _ = writeln!(
        out,
        "{}",
        style.dim(
            &format!(
                "A compaction re-writes the cold context and outputs {} tokens; a handoff note is {} tokens written while the cache was warm.",
                tokens(SUMMARY_TOKENS),
                tokens(SUMMARY_TOKENS)
            )
        )
    );
    let _ = writeln!(
        out,
        "{}",
        style.dim(&format!(
            "A heartbeat goes out at 5/6 of the cache lifetime, every {} on a 1h cache, reads the context and outputs {} tokens; for Codex this assumes it resets the cache timer as it does in Claude Code.",
            human_duration(ping_every(3_600_000) as f64 / 1000.0),
            PING_OUTPUT
        ))
    );
    out
}

/// Colors of one chart theme.
struct Theme {
    surface: &'static str,
    grid: &'static str,
    axis: &'static str,
    ink: &'static str,
    ink2: &'static str,
    muted: &'static str,
    series: [&'static str; 4],
}

const LIGHT: Theme = Theme {
    surface: "#fcfcfb",
    grid: "#e1e0d9",
    axis: "#c3c2b7",
    ink: "#0b0b0b",
    ink2: "#52514e",
    muted: "#898781",
    series: ["#2a78d6", "#eb6834", "#1baf7a", "#eda100"],
};

const DARK: Theme = Theme {
    surface: "#1a1a19",
    grid: "#2c2c2a",
    axis: "#383835",
    ink: "#f0efec",
    ink2: "#c3c2b7",
    muted: "#898781",
    series: ["#3987e5", "#d95926", "#199e70", "#c98500"],
};

/// Writes the chart of the mean cost per return to `path`, and a dark-theme version next to it
/// with `-dark` before the extension; returns the files written.
pub fn write_svg(b: &Bench, path: &Path) -> Result<Vec<PathBuf>> {
    let stem = path.file_stem().and_then(|s| s.to_str()).context("the chart needs a file name")?;
    let dark = path.with_file_name(format!("{stem}-dark.svg"));
    if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
        std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
    }
    let mut written = Vec::new();
    for (theme, file) in [(&LIGHT, path.to_path_buf()), (&DARK, dark)] {
        std::fs::write(&file, svg(b, theme)).with_context(|| format!("writing {}", file.display()))?;
        written.push(file);
    }
    Ok(written)
}

/// Tick step and plot top for values up to `max`: a round step with at most five intervals.
fn nice_axis(max: f64) -> (f64, f64) {
    let rough = (max / 5.0).max(1e-9);
    let magnitude = 10f64.powf(rough.log10().floor());
    let step = [1.0, 2.0, 5.0, 10.0].into_iter().map(|m| m * magnitude).find(|s| *s >= rough).unwrap_or(10.0 * magnitude);
    (step, (max * 1.08).max(step))
}

fn money(v: f64, step: f64) -> String {
    if step >= 1.0 { format!("${v:.0}") } else { format!("${v:.2}") }
}

/// Token counts as the chart shows them: 100k, 491k, 1.2M.
fn round_tokens(n: u64) -> String {
    match n {
        n if n >= 1_000_000 => format!("{:.1}M", n as f64 / 1e6),
        n if n >= 1_000 => format!("{:.0}k", n as f64 / 1e3),
        n => n.to_string(),
    }
}

/// Approximate width of a label in the chart's sans at `size` px.
fn text_width(text: &str, size: f64) -> f64 {
    0.5 * size * text.chars().count() as f64
}

fn agent_title(agent: Agent) -> &'static str {
    match agent {
        Agent::Claude => "Claude Code",
        Agent::Codex => "Codex",
    }
}

/// One agent's part of the chart: its returns and the mean cost curve of each strategy.
struct Panel<'a> {
    agent: Agent,
    returns: Vec<&'a Return>,
    curves: [Vec<f64>; 4],
}

fn svg(b: &Bench, t: &Theme) -> String {
    const W: f64 = 880.0;
    const PAD: f64 = 24.0;
    const GAP: f64 = 40.0;
    const ROW: f64 = 20.0;
    const FONT: &str = "system-ui, -apple-system, 'Segoe UI', Helvetica, Arial, sans-serif";
    let n = b.requests;
    let panels: Vec<Panel> = Agent::ALL
        .into_iter()
        .map(|agent| (agent, b.of(agent)))
        .filter(|(_, returns)| !returns.is_empty())
        .map(|(agent, returns)| {
            let curves = Strategy::ALL.map(|s| mean_curve(&returns, s, n));
            Panel { agent, returns, curves }
        })
        .collect();
    let top = panels.iter().flat_map(|p| p.curves.iter().flatten()).fold(0.0f64, |m, v| m.max(*v));
    let (step, ymax) = nice_axis(top);
    // Heartbeats sit next to /compact in hue, so their line is dotted, not dashed.
    let dash = |s: Strategy| match s {
        Strategy::Resume => "",
        Strategy::Heartbeats => " stroke-dasharray=\"1 4\"",
        _ => " stroke-dasharray=\"6 4\"",
    };
    // The legend: one key per strategy, wrapped to the chart's width; the rows below it push
    // the panels down.
    let keys: Vec<(Strategy, String, f64, f64)> = {
        let (mut lx, mut ly) = (PAD, 80.0);
        let mut keys = Vec::new();
        for s in Strategy::ALL {
            let label = format!("{} ({})", s.name(), if s == Strategy::Resume { "as logged" } else { "replayed" });
            let width = 30.0 + text_width(&label, 12.5);
            if lx > PAD && lx + width > W - PAD {
                (lx, ly) = (PAD, ly + ROW);
            }
            keys.push((s, label, lx, ly));
            lx += width + 24.0;
        }
        keys
    };
    let below = keys.last().map_or(0.0, |k| k.3 - 80.0);
    let h = 440.0 + below;

    let mut desc = String::new();
    for Panel { agent, returns, curves } in &panels {
        let ends: Vec<String> = Strategy::ALL.iter().map(|s| format!("{} {}", s.name(), usd(curves[*s as usize][n]))).collect();
        let _ = write!(desc, "{}: {} returns, after {n} requests {}. ", agent_title(*agent), returns.len(), ends.join(", "));
    }

    let mut o = String::new();
    let _ = writeln!(
        o,
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{W}" height="{h}" viewBox="0 0 {W} {h}" role="img" aria-labelledby="title desc" font-family="{FONT}">"#
    );
    let _ = writeln!(o, r#"<title id="title">Coming back to a big session after its prompt cache expired</title>"#);
    let _ = writeln!(o, r#"<desc id="desc">Mean cost per return. {}</desc>"#, desc.trim_end());
    let _ = writeln!(
        o,
        "<style>.num{{font-variant-numeric:tabular-nums}} .cross{{opacity:0}} .col:hover .cross{{opacity:1}}</style>"
    );
    let _ = writeln!(o, r#"<rect width="{W}" height="{h}" rx="10" fill="{}"/>"#, t.surface);
    let _ = writeln!(o, r#"<text x="{PAD}" y="34" font-size="17" font-weight="600" fill="{}">Coming back to a big session after its prompt cache expired</text>"#, t.ink);
    let _ = writeln!(
        o,
        r#"<text x="{PAD}" y="56" font-size="12.5" fill="{}">Mean cost per return over the requests that follow, for cold returns to sessions of {}+ tokens in local logs</text>"#,
        t.ink2,
        round_tokens(b.min_context)
    );

    // The legend keys, each stroke drawn like its line.
    for (s, label, lx, ly) in &keys {
        let _ = writeln!(
            o,
            r#"<line x1="{lx}" y1="{ly}" x2="{}" y2="{ly}" stroke="{}" stroke-width="2" stroke-linecap="round"{}/>"#,
            lx + 24.0,
            t.series[*s as usize],
            dash(*s)
        );
        let _ = writeln!(o, r#"<text x="{}" y="{}" font-size="12.5" fill="{}">{label}</text>"#, lx + 30.0, ly + 4.0, t.ink2);
    }

    let panel_w = (W - 2.0 * PAD - GAP * (panels.len().max(1) - 1) as f64) / panels.len().max(1) as f64;
    let (y0, y1) = (136.0 + below, 344.0 + below);
    let xstep = (n as f64 / 6.0).ceil().max(1.0) as usize;
    for (i, Panel { agent, returns, curves }) in panels.iter().enumerate() {
        let left = PAD + i as f64 * (panel_w + GAP);
        let (x0, x1) = (left + 34.0, left + panel_w - 50.0);
        let x = |k: f64| x0 + (x1 - x0) * k / n as f64;
        let y = |v: f64| y1 - (y1 - y0) * v / ymax;
        let ctx = median(returns.iter().map(|r| r.context).collect()).unwrap_or(0);
        let _ = writeln!(
            o,
            r#"<text x="{left}" y="{}" font-size="13" font-weight="600" fill="{}">{}<tspan font-weight="400" fill="{}"> · {} returns, median context {} tokens</tspan></text>"#,
            116.0 + below,
            t.ink,
            agent_title(*agent),
            t.ink2,
            returns.len(),
            round_tokens(ctx)
        );
        let mut v = 0.0;
        while v <= ymax + 1e-9 {
            let yy = y(v);
            let stroke = if v == 0.0 { t.axis } else { t.grid };
            let _ = writeln!(o, r#"<line x1="{x0}" y1="{yy:.1}" x2="{x1}" y2="{yy:.1}" stroke="{stroke}" stroke-width="1"/>"#);
            let _ = writeln!(
                o,
                r#"<text class="num" x="{:.1}" y="{:.1}" font-size="11.5" text-anchor="end" fill="{}">{}</text>"#,
                x0 - 8.0,
                yy + 4.0,
                t.ink2,
                money(v, step)
            );
            v += step;
        }
        for k in (0..=n).step_by(xstep) {
            let _ = writeln!(
                o,
                r#"<text class="num" x="{:.1}" y="{}" font-size="11.5" text-anchor="middle" fill="{}">{k}</text>"#,
                x(k as f64),
                y1 + 18.0,
                t.ink2
            );
        }
        let _ = writeln!(
            o,
            r#"<text x="{:.1}" y="{}" font-size="12" text-anchor="middle" fill="{}">requests after coming back</text>"#,
            (x0 + x1) / 2.0,
            y1 + 38.0,
            t.ink2
        );
        for s in Strategy::ALL {
            let points: Vec<String> = curves[s as usize].iter().enumerate().map(|(k, v)| format!("{:.1},{:.1}", x(k as f64), y(*v))).collect();
            let _ = writeln!(
                o,
                r#"<polyline points="{}" fill="none" stroke="{}" stroke-width="2" stroke-linejoin="round" stroke-linecap="round"{}/>"#,
                points.join(" "),
                t.series[s as usize],
                dash(s)
            );
        }
        // The first request after the break, where resuming writes the whole context again:
        // labeled above every line it would cross, with a leader down to the point.
        if n >= 1 {
            let first = curves[Strategy::Resume as usize][1];
            let label = format!("1st request {}", usd(first));
            let (px, py) = (x(1.0), y(first));
            let reach = ((text_width(&label, 11.5) + 6.0) / (x1 - x0) * n as f64).ceil() as usize + 1;
            let highest = (1..=reach.min(n)).flat_map(|k| curves.iter().map(move |c| c[k])).fold(first, f64::max);
            let ly = (y(highest) - 12.0).max(y0 + 10.0);
            let _ = writeln!(o, r#"<line x1="{px:.1}" y1="{:.1}" x2="{px:.1}" y2="{:.1}" stroke="{}" stroke-width="1"/>"#, py - 6.0, ly + 4.0, t.muted);
            let _ = writeln!(
                o,
                r#"<circle cx="{px:.1}" cy="{py:.1}" r="4" fill="{}" stroke="{}" stroke-width="2"/>"#,
                t.series[Strategy::Resume as usize],
                t.surface
            );
            let _ = writeln!(
                o,
                r#"<text class="num" x="{:.1}" y="{ly:.1}" font-size="11.5" fill="{}" stroke="{}" stroke-width="3" paint-order="stroke">{label}</text>"#,
                px + 5.0,
                t.ink2,
                t.surface
            );
        }
        // End values, kept at least a line apart; a label pushed off its line gets a leader.
        let mut ends: Vec<(Strategy, f64, f64)> = Strategy::ALL.iter().map(|s| (*s, curves[*s as usize][n], y(curves[*s as usize][n]))).collect();
        ends.sort_by(|a, b| a.2.total_cmp(&b.2));
        let mut placed: Vec<f64> = Vec::new();
        for (j, e) in ends.iter().enumerate() {
            let ly = if j == 0 { e.2 } else { e.2.max(placed[j - 1] + 15.0) };
            placed.push(ly);
        }
        // Bottom up, so where end points crowd, the one beside its own label is drawn on top.
        for (e, ly) in ends.iter().zip(&placed).rev() {
            let (s, _, ey) = *e;
            if (ly - ey).abs() > 3.0 {
                let _ = writeln!(o, r#"<line x1="{:.1}" y1="{ey:.1}" x2="{:.1}" y2="{ly:.1}" stroke="{}" stroke-width="1"/>"#, x1 + 5.0, x1 + 9.0, t.muted);
            }
            let _ = writeln!(
                o,
                r#"<circle cx="{x1:.1}" cy="{ey:.1}" r="4" fill="{}" stroke="{}" stroke-width="2"/>"#,
                t.series[s as usize],
                t.surface
            );
        }
        for (e, ly) in ends.iter().zip(&placed) {
            let value = e.1;
            let _ = writeln!(
                o,
                r#"<text class="num" x="{:.1}" y="{:.1}" font-size="12" font-weight="600" fill="{}">{}</text>"#,
                x1 + 10.0,
                ly + 4.0,
                t.ink,
                usd(value)
            );
        }
        // Hovering a column shows a crosshair and every strategy's cost at that request.
        let half = (x1 - x0) / n as f64 / 2.0;
        for (k, at) in (0..=n).map(|k| Strategy::ALL.map(|s| curves[s as usize][k])).enumerate() {
            let cx = x(k as f64);
            let when = if k == 0 { "before the first request".to_string() } else { format!("after {k} requests") };
            let values: Vec<String> = Strategy::ALL.iter().zip(at).map(|(s, v)| format!("{} {}", s.name(), usd(v))).collect();
            let _ = writeln!(
                o,
                r#"<g class="col"><title>{}, {when}: {}</title><rect x="{:.1}" y="{y0}" width="{:.1}" height="{}" fill="{}" fill-opacity="0"/><line class="cross" x1="{cx:.1}" y1="{y0}" x2="{cx:.1}" y2="{y1}" stroke="{}" stroke-width="1"/></g>"#,
                agent_title(*agent),
                values.join(" · "),
                (cx - half).max(x0 - half),
                2.0 * half,
                y1 - y0,
                t.surface,
                t.axis
            );
        }
    }
    let _ = writeln!(
        o,
        r#"<text x="{PAD}" y="{}" font-size="11" fill="{}">tokprof bench · requests priced at their time · the compaction call is not logged: modeled as writing the cold context again plus a {}-token summary</text>"#,
        h - 30.0,
        t.muted,
        round_tokens(SUMMARY_TOKENS as u64)
    );
    let _ = writeln!(
        o,
        r#"<text x="{PAD}" y="{}" font-size="11" fill="{}">heartbeats: a ping at 5/6 of the cache lifetime, every {} on a 1h cache, for up to {} after the last request; a return within the last ping's lifetime reads the cache</text>"#,
        h - 14.0,
        t.muted,
        human_duration(ping_every(3_600_000) as f64 / 1000.0),
        window_name(b.keep_warm_ms)
    );
    o.push_str("</svg>\n");
    o
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Chain, ChainKind};

    const HOUR: i64 = 3_600_000;

    fn request(ts_ms: i64, read: u64, write_long: u64, output: u64) -> Request {
        Request { ts_ms, model: "claude-opus-5-5".into(), usage: Usage { cache_read: read, write_long, output, ..Usage::default() }, ..Request::default() }
    }

    /// A 500k-token conversation that idles for two hours, comes back cold, does three more
    /// requests, compacts and goes on.
    fn session() -> Session {
        let mut s = Session::new(Agent::Claude, "s".into());
        let mut requests = vec![request(0, 0, 400_000, 1_000), request(60_000, 400_000, 100_000, 1_000)];
        requests.push(request(2 * HOUR, 10_000, 492_000, 1_000));
        for k in 1..=3 {
            requests.push(request(2 * HOUR + k * 30_000, 500_000 + 2_000 * k as u64, 2_000, 1_000));
        }
        let mut after = request(2 * HOUR + 200_000, 10_000, 40_000, 1_000);
        after.compacted_before = true;
        requests.push(after);
        s.chains.push(Chain { name: "main".into(), kind: ChainKind::Main, requests });
        s
    }

    #[test]
    fn shrink_scales_the_old_conversation_only() {
        // The cold return: 10k still cached, the old 490k and 2k of new content written.
        let cold = shrink(Usage { cache_read: 10_000, write_long: 492_000, ..Usage::default() }, 10_000.0, 500_000.0, 0.1);
        assert_eq!((cold.cache_read, cold.write_long), (10_000, 51_000));
        // A warm request after it reads the old span and writes only what is new.
        let warm = shrink(Usage { cache_read: 502_000, write_long: 3_000, ..Usage::default() }, 10_000.0, 500_000.0, 0.1);
        assert_eq!((warm.cache_read, warm.write_long), (61_000, 3_000));
    }

    #[test]
    fn a_cold_return_is_replayed_four_ways() {
        let found = scan(&session(), &Filter::default(), 60, 100_000);
        assert_eq!((found.cases.len(), found.compacted.len()), (1, 1));
        let case = &found.cases[0];
        assert_eq!((case.context, case.steps.len(), case.idle_ms), (500_000, 4, 2 * HOUR - 60_000));

        let b = replay(found, &Pricing::defaults(), 60, 100_000, 6 * HOUR);
        assert_eq!(b.base[&Agent::Claude].tokens, 50_000);
        let r = &b.returns[0];
        let cost = |s| r.after(s, 60);
        // Resuming is the logged cost: 10k read, 492k written at $8/M, then the warm requests.
        let logged = (10_000.0 * 0.2 + 492_000.0 * 8.0 + 1_000.0 * 20.0) / 1e6;
        assert!((r.after(Strategy::Resume, 1) - logged).abs() < 1e-9);
        // The compaction call writes the cold context again before anything else.
        assert!((r.after(Strategy::Compact, 0) - (10_000.0 * 0.2 + 490_000.0 * 8.0 + 10_000.0 * 20.0) / 1e6).abs() < 1e-9);
        // Two pings, at 50 and 100 minutes of the 119-minute break, each read the 500k context.
        assert_eq!((r.pings, r.kept_warm), (2, true));
        assert!((r.after(Strategy::Heartbeats, 0) - 2.0 * (500_000.0 * 0.2 + 100.0 * 20.0) / 1e6).abs() < 1e-9);
        // The warm return reads the old 500k and writes only the 2k that is new.
        let warm = (500_000.0 * 0.2 + 2_000.0 * 8.0 + 1_000.0 * 20.0) / 1e6;
        assert!((r.after(Strategy::Heartbeats, 1) - r.after(Strategy::Heartbeats, 0) - warm).abs() < 1e-9);
        // Three requests are too few for /compact to pay off: it saves only on reads. Over so few
        // requests, keeping the full context warm costs less than the note.
        let costs = Strategy::ALL.map(|s| (cost(s) * 100.0).round() / 100.0);
        assert_eq!(costs, [4.37, 4.62, 0.8, 0.75], "{costs:?}");
        // The X-ray forecast at the break: the 500k context and the pace before it, all written.
        assert!((r.forecast - ((500_000.0 + 250_000.0) * 8.0 + 1_000.0 * 20.0) / 1e6).abs() < 1e-9);
    }

    #[test]
    fn heartbeats_keep_the_cache_warm_only_within_their_window() {
        let minute = 60_000;
        // A 1h cache gets a ping every 50 minutes; the return 19 minutes after the last is warm.
        assert_eq!(heartbeats(119 * minute, HOUR, 6 * HOUR), (vec![50 * minute, 100 * minute], true));
        // A break of 10h outlasts a 6h window: seven pings, then the cache expires anyway.
        let (pings, warm) = heartbeats(10 * HOUR, HOUR, 6 * HOUR);
        assert_eq!((pings.len(), pings.last().copied(), warm), (7, Some(350 * minute), false));
        // The 5-minute cache needs a ping every 4m10s.
        let (pings, warm) = heartbeats(30 * minute, 5 * minute, 6 * HOUR);
        assert_eq!((pings.len(), warm), (7, true));
        // With no window there are no pings, and the return stays cold.
        assert_eq!(heartbeats(2 * HOUR, HOUR, 0), (vec![], false));
        assert_eq!(window_name(6 * HOUR), "6h");
        assert_eq!(window_name(90 * minute), "90m");
    }

    #[test]
    fn a_warm_return_reads_the_previous_prompt() {
        // Cold: 10k read, 490k of old context and 2k of new content written, 1k uncached.
        let cold = Usage { cache_read: 10_000, write_long: 400_000, write_short: 92_000, uncached: 1_000, output: 500, ..Usage::default() };
        let warm = warmed(cold, 500_000);
        assert_eq!(warm.prompt(), cold.prompt());
        assert_eq!(warm.cache_read, 500_000);
        assert_eq!(warm.cache_write(), 2_000);
        assert_eq!((warm.uncached, warm.output), (1_000, 500));
        // A prompt smaller than the old context reads all of it.
        let small = warmed(Usage { cache_read: 0, write_long: 30_000, ..Usage::default() }, 50_000);
        assert_eq!((small.cache_read, small.write_long), (30_000, 0));
    }

    /// A session that spans the release which added request-audit lines: its main thread is in two
    /// chains of one name, and the return is the first request of the second.
    #[test]
    fn a_return_between_the_two_chains_of_a_conversation_is_found() {
        let mut s = session();
        let requests = std::mem::take(&mut s.chains[0].requests);
        let (before, after) = requests.split_at(2);
        s.chains[0].requests = before.to_vec();
        s.chains.push(Chain { name: "main".into(), kind: ChainKind::Main, requests: after.to_vec() });
        let found = scan(&s, &Filter::default(), 60, 100_000);
        assert_eq!(found.cases.len(), 1);
        assert_eq!((found.cases[0].context, found.cases[0].steps.len()), (500_000, 4));
    }

    #[test]
    fn axis_steps_are_round() {
        assert_eq!(nice_axis(8.93).0, 2.0);
        assert_eq!(nice_axis(3.98).0, 1.0);
        assert_eq!(nice_axis(0.4).0, 0.1);
        assert_eq!(round_tokens(490_700), "491k");
    }
}
