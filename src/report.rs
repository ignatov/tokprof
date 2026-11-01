//! Collects analysis output and renders the text reports.

use std::collections::{BTreeMap, HashMap};
use std::fmt::Write as _;

use crate::analysis::cache::Cause;
use crate::analysis::{Component, Fact, MissEvent, RequestRecord, Sink};
use crate::export::{Frame, Stacks, Tint};
use crate::jfr::TimedStacks;
use crate::model::{Agent, ChainKind, Session, Usage};
use crate::pricing::{Price, Pricing};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dim {
    Agent,
    Project,
    Session,
    Chain,
    Origin,
    Task,
    Model,
    Turn,
    Component,
    Cause,
    Kind,
    Tool,
    Target,
}

/// Stack and JFR thread dimensions of `tokprof flame` by default.
pub const DEFAULT_STACK: &str = "agent,project,session,chain,turn,kind,tool,target,component";
pub const DEFAULT_THREADS: &str = "agent,project,session";

/// Dimensions from a comma-separated list; unknown names are skipped.
pub fn dims(list: &str) -> Vec<Dim> {
    list.split(',').filter_map(|d| Dim::parse(d.trim())).collect()
}

impl Dim {
    pub const NAMES: &str = "agent, project, session, chain, origin, task, model, turn, component, cause, kind, tool, target";

    /// Short name, used as the class name of JFR frames.
    pub fn name(self) -> &'static str {
        match self {
            Dim::Agent => "agent",
            Dim::Project => "project",
            Dim::Session => "session",
            Dim::Chain => "chain",
            Dim::Origin => "origin",
            Dim::Task => "task",
            Dim::Model => "model",
            Dim::Turn => "turn",
            Dim::Component => "component",
            Dim::Cause => "miss",
            Dim::Kind => "kind",
            Dim::Tool => "tool",
            Dim::Target => "target",
        }
    }

    pub fn parse(s: &str) -> Option<Dim> {
        Some(match s {
            "agent" => Dim::Agent,
            "project" => Dim::Project,
            "session" => Dim::Session,
            "chain" => Dim::Chain,
            "origin" => Dim::Origin,
            "task" => Dim::Task,
            "model" => Dim::Model,
            "turn" => Dim::Turn,
            "component" => Dim::Component,
            "cause" => Dim::Cause,
            "kind" => Dim::Kind,
            "tool" => Dim::Tool,
            "target" => Dim::Target,
            _ => return None,
        })
    }

    /// Frame for this dimension; `None` leaves the frame out of the stack.
    fn frame(self, f: &Fact) -> Option<Frame> {
        let (name, tint) = match self {
            Dim::Agent => (f.agent.to_string(), Tint::Structure),
            Dim::Project => (Some(f.project.to_string()).filter(|p| !p.is_empty())?, Tint::Structure),
            Dim::Session => (f.session.to_string(), Tint::Structure),
            Dim::Chain => (f.chain.to_string(), Tint::Structure),
            Dim::Origin => (f.origin.to_string(), Tint::Structure),
            Dim::Task => (f.task.to_string(), Tint::Structure),
            Dim::Model => (f.model.to_string(), Tint::Structure),
            Dim::Turn => (
                match f.turn {
                    Some(0) | None if matches!(f.kind, crate::model::BlockKind::Tools | crate::model::BlockKind::System) => {
                        "setup".to_string()
                    }
                    Some(t) => format!("turn {t}"),
                    None => return None,
                },
                Tint::Structure,
            ),
            Dim::Component => (
                f.component.name().to_string(),
                match f.component {
                    Component::Read => Tint::Read,
                    Component::Write => Tint::Write,
                    Component::Rewrite => Tint::Miss,
                    Component::Uncached => Tint::Uncached,
                    Component::Output => Tint::Output,
                },
            ),
            Dim::Cause => (format!("miss: {}", f.cause?.name()), Tint::Miss),
            Dim::Kind => (f.kind.name().to_string(), Tint::Content),
            Dim::Tool => (f.tool?.to_string(), Tint::Content),
            Dim::Target => (f.target?.to_string(), Tint::Content),
        };
        Some(Frame { name: name.replace([';', '\n'], " "), tint, dim: self.name() })
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Values {
    pub tokens: f64,
    pub usd: f64,
    pub avoidable: f64,
}

impl Values {
    fn add(&mut self, o: Values) {
        self.tokens += o.tokens;
        self.usd += o.usd;
        self.avoidable += o.avoidable;
    }
}

/// Spend of whole requests: how many, their tokens and their cost.
#[derive(Clone, Copy, Debug, Default)]
pub struct Spend {
    pub requests: u64,
    pub tokens: f64,
    pub usd: f64,
}

impl Spend {
    pub fn add(&mut self, o: Spend) {
        self.requests += o.requests;
        self.tokens += o.tokens;
        self.usd += o.usd;
    }
}

/// Spend by the kind of work the requests were part of, and by what the work was.
#[derive(Clone, Debug, Default)]
pub struct Work {
    pub by_origin: HashMap<String, Spend>,
    /// Task, then origin.
    pub by_task: HashMap<String, HashMap<String, Spend>>,
}

impl Work {
    pub fn add(&mut self, r: &RequestRecord) {
        let s = Spend { requests: 1, tokens: (r.usage.prompt() + r.usage.output) as f64, usd: r.price.map_or(0.0, |p| p.cost(&r.usage)) };
        add_spend(&mut self.by_origin, r.origin, s);
        if !self.by_task.contains_key(r.task) {
            self.by_task.insert(r.task.to_string(), HashMap::new());
        }
        add_spend(self.by_task.get_mut(r.task).expect("inserted above"), r.origin, s);
    }

    fn merge(&mut self, o: Work) {
        for (origin, s) in o.by_origin {
            self.by_origin.entry(origin).or_default().add(s);
        }
        for (task, origins) in o.by_task {
            let t = self.by_task.entry(task).or_default();
            for (origin, s) in origins {
                t.entry(origin).or_default().add(s);
            }
        }
    }

    /// Origins, most expensive first.
    pub fn origins(&self) -> Vec<(&str, Spend)> {
        let mut rows: Vec<(&str, Spend)> = self.by_origin.iter().map(|(o, s)| (o.as_str(), *s)).collect();
        rows.sort_by(|a, b| b.1.usd.total_cmp(&a.1.usd).then(b.1.tokens.total_cmp(&a.1.tokens)).then(a.0.cmp(b.0)));
        rows
    }

    /// Tasks, most expensive first, each with the origin most of its spend came from.
    pub fn tasks(&self) -> Vec<(&str, &str, Spend)> {
        let mut rows: Vec<(&str, &str, Spend)> = self
            .by_task
            .iter()
            .map(|(task, origins)| {
                let mut total = Spend::default();
                origins.values().for_each(|s| total.add(*s));
                let main = origins.iter().max_by(|a, b| a.1.usd.total_cmp(&b.1.usd).then(a.1.tokens.total_cmp(&b.1.tokens)).then(b.0.cmp(a.0)));
                (task.as_str(), main.map_or("", |(o, _)| o.as_str()), total)
            })
            .collect();
        rows.sort_by(|a, b| b.2.usd.total_cmp(&a.2.usd).then(b.2.tokens.total_cmp(&a.2.tokens)).then(a.0.cmp(b.0)));
        rows
    }
}

fn add_spend(map: &mut HashMap<String, Spend>, key: &str, s: Spend) {
    match map.get_mut(key) {
        Some(x) => x.add(s),
        None => {
            map.insert(key.to_string(), s);
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct ModelTotals {
    pub requests: u64,
    pub with_prompt: u64,
    pub priced: bool,
    pub by_component: [Values; 5],
}

#[derive(Clone, Copy, Debug, Default)]
struct SessionTotals {
    ours: f64,
    reported: Option<f64>,
}

/// Simulated input cost of the Claude requests under each TTL policy.
#[derive(Clone, Debug, Default)]
pub struct WhatIf {
    pub actual: [f64; 2],
    /// [chain bucket][policy]: bucket 0 = main conversation, 1 = everything else; policy 0 = 5m, 1 = 1h.
    pub simulated: [[f64; 2]; 2],
    pub requests: [u64; 2],
}

#[derive(Default)]
pub struct Collector {
    pub stack: Option<Vec<Dim>>,
    pub components: Option<Vec<Component>>,
    pub stacks: Stacks,
    /// For JFR output: the dimensions that name a thread. Facts then go to `timed`, by thread
    /// and by the minute their content entered the context.
    pub threads: Option<Vec<Dim>>,
    pub timed: TimedStacks,
    pub models: BTreeMap<(Agent, String), ModelTotals>,
    pub misses: Vec<MissEvent>,
    sessions: HashMap<(Agent, String), SessionTotals>,
    pub whatif: WhatIf,
    pub work: Work,
    pub files: usize,
    pub bytes: u64,
    pub errors: Vec<String>,
}

impl Collector {
    pub fn new(stack: Option<Vec<Dim>>, components: Option<Vec<Component>>) -> Collector {
        Collector { stack, components, ..Collector::default() }
    }

    pub fn merge(&mut self, o: Collector) {
        for (k, v) in o.stacks {
            self.stacks.entry(k).or_default().add(v);
        }
        for (k, v) in o.timed {
            self.timed.entry(k).or_default().add(v);
        }
        for (k, v) in o.models {
            let m = self.models.entry(k).or_default();
            m.requests += v.requests;
            m.with_prompt += v.with_prompt;
            m.priced |= v.priced;
            for (a, b) in m.by_component.iter_mut().zip(v.by_component) {
                a.add(b);
            }
        }
        self.misses.extend(o.misses);
        for (k, v) in o.sessions {
            let s = self.sessions.entry(k).or_default();
            s.ours += v.ours;
            s.reported = max_opt(s.reported, v.reported);
        }
        for i in 0..2 {
            self.whatif.actual[i] += o.whatif.actual[i];
            self.whatif.requests[i] += o.whatif.requests[i];
            for p in 0..2 {
                self.whatif.simulated[i][p] += o.whatif.simulated[i][p];
            }
        }
        self.work.merge(o.work);
        self.files += o.files;
        self.bytes += o.bytes;
        self.errors.extend(o.errors);
    }
}

impl Sink for Collector {
    fn session(&mut self, s: &Session) {
        // Keyed by the short id that facts carry; subagent files share their parent's id.
        let e = self.sessions.entry((s.agent, s.short_id().to_string())).or_default();
        e.reported = max_opt(e.reported, s.reported_usd);
    }

    fn request(&mut self, r: &RequestRecord) {
        let m = self.models.entry((r.agent, r.model.to_string())).or_default();
        m.requests += 1;
        m.with_prompt += r.composition as u64;
        m.priced |= r.price.is_some();
        self.work.add(r);

        let (Agent::Claude, Some(p), false) = (r.agent, r.price, r.advisor) else { return };
        let u = &r.usage;
        let bucket = (r.chain_kind != ChainKind::Main) as usize;
        let t = u.prompt() as f64;
        self.whatif.requests[bucket] += 1;
        self.whatif.actual[bucket] += p.cost(&Usage { output: 0, ..*u });
        for (policy, (ttl_s, write)) in [(300.0, p.write_short), (3600.0, p.write_long)].into_iter().enumerate() {
            let cost = match (r.prev_prompt, r.gap_s) {
                (Some(prev), Some(gap)) if gap <= ttl_s => {
                    let kept = (prev as f64).min(t);
                    kept * p.read + (t - kept) * write
                }
                _ => t * write,
            };
            self.whatif.simulated[bucket][policy] += cost / 1e6;
        }
    }

    fn miss(&mut self, miss: MissEvent) {
        self.misses.push(miss);
    }

    fn fact(&mut self, f: &Fact) {
        let values = Values { tokens: f.tokens, usd: f.usd.unwrap_or(0.0), avoidable: f.avoidable_usd };
        let agent = Agent::parse(f.agent).unwrap_or(Agent::Claude);
        let m = self.models.entry((agent, f.model.to_string())).or_default();
        m.by_component[Component::ALL.iter().position(|c| *c == f.component).unwrap_or(0)].add(values);
        if let Some(usd) = f.usd {
            self.sessions.entry((agent, f.session.to_string())).or_default().ours += usd;
        }
        let Some(stack) = &self.stack else { return };
        if self.components.as_ref().is_some_and(|cs| !cs.contains(&f.component)) {
            return;
        }
        if let Some(threads) = &self.threads {
            let thread: Vec<String> = threads.iter().filter_map(|d| d.frame(f)).map(|fr| fr.name).collect();
            let key: Vec<Frame> = stack.iter().filter(|d| !threads.contains(d)).filter_map(|d| d.frame(f)).collect();
            let minute = f.ts_ms.div_euclid(60_000) * 60_000;
            self.timed.entry((thread.join(" "), key, minute)).or_default().add(values);
            return;
        }
        let key: Vec<Frame> = stack.iter().filter_map(|d| d.frame(f)).collect();
        self.stacks.entry(key).or_default().add(values);
    }
}

fn max_opt(a: Option<f64>, b: Option<f64>) -> Option<f64> {
    match (a, b) {
        (Some(x), Some(y)) => Some(x.max(y)),
        (x, None) => x,
        (None, y) => y,
    }
}

pub fn tokens(n: f64) -> String {
    let a = n.abs();
    if a >= 1e9 {
        format!("{:.2}B", n / 1e9)
    } else if a >= 1e6 {
        format!("{:.1}M", n / 1e6)
    } else if a >= 1e3 {
        format!("{:.1}k", n / 1e3)
    } else {
        format!("{n:.0}")
    }
}

pub fn usd(n: f64) -> String {
    let n = n + 0.0; // no "-0.00"
    if n.abs() >= 100.0 {
        let whole = format!("{:.0}", n.abs());
        let mut out = String::new();
        for (i, ch) in whole.chars().enumerate() {
            if i > 0 && (whole.len() - i) % 3 == 0 {
                out.push(',');
            }
            out.push(ch);
        }
        format!("{}${out}", if n < 0.0 { "-" } else { "" })
    } else {
        format!("${n:.2}")
    }
}

pub(crate) fn table(rows: &[Vec<String>], right_from: usize) -> String {
    let cols = rows.iter().map(Vec::len).max().unwrap_or(0);
    let widths: Vec<usize> =
        (0..cols).map(|c| rows.iter().filter_map(|r| r.get(c)).map(|s| s.chars().count()).max().unwrap_or(0)).collect();
    let mut out = String::new();
    for row in rows {
        let mut line = String::new();
        for (c, cell) in row.iter().enumerate() {
            let pad = widths[c] - cell.chars().count();
            if c >= right_from {
                let _ = write!(line, "{}{cell}  ", " ".repeat(pad));
            } else {
                let _ = write!(line, "{cell}{}  ", " ".repeat(pad));
            }
        }
        out.push_str(line.trim_end());
        out.push('\n');
    }
    out
}

pub fn summary(c: &Collector) -> String {
    let mut out = String::new();
    let mut rows = vec![
        ["agent", "model", "requests", "prompt known"].iter().map(|s| s.to_string())
            .chain(Component::ALL.iter().map(|c| c.name().to_string()))
            .chain(["total".to_string()])
            .collect::<Vec<_>>(),
    ];
    let (mut total, mut avoidable) = (0.0, 0.0);
    let mut unpriced: Vec<(String, f64)> = Vec::new();
    let mut models: Vec<_> = c.models.iter().collect();
    models.sort_by(|a, b| {
        let cost = |m: &ModelTotals| m.by_component.iter().map(|v| v.usd).sum::<f64>();
        cost(b.1).total_cmp(&cost(a.1)).then(a.0.cmp(b.0))
    });
    for ((agent, model), m) in models {
        let sum: f64 = m.by_component.iter().map(|v| v.usd).sum();
        let cell = |v: &Values| if m.priced { usd(v.usd) } else { tokens(v.tokens) };
        let mut row = vec![
            agent.name().to_string(),
            model.clone(),
            m.requests.to_string(),
            format!("{:.0}%", 100.0 * m.with_prompt as f64 / m.requests.max(1) as f64),
        ];
        row.extend(m.by_component.iter().map(cell));
        row.push(if m.priced { usd(sum) } else { format!("{} tok", tokens(m.by_component.iter().map(|v| v.tokens).sum())) });
        rows.push(row);
        if m.priced {
            total += sum;
            avoidable += m.by_component.iter().map(|v| v.avoidable).sum::<f64>();
        } else {
            unpriced.push((model.clone(), m.by_component.iter().map(|v| v.tokens).sum::<f64>()));
        }
    }
    let _ = writeln!(out, "Spend by model (API-equivalent USD; token counts where no price is known)\n");
    out.push_str(&table(&rows, 2));
    let _ = writeln!(out, "\nTotal priced: {}   avoidable on cache misses: {} ({:.1}%)", usd(total), usd(avoidable), 100.0 * avoidable / total.max(1e-9));
    if !unpriced.is_empty() {
        let list = unpriced.iter().map(|(m, t)| format!("{m} ({} tokens)", tokens(*t))).collect::<Vec<_>>().join(", ");
        let _ = writeln!(out, "No price for {list}. Add them to {}", crate::pricing::user_config_path().display());
    }
    let (ours, reported, n) = c.sessions.values().filter_map(|s| s.reported.map(|r| (s.ours, r))).fold((0.0, 0.0, 0), |(a, b, n), (o, r)| (a + o, b + r, n + 1));
    if n > 0 {
        let _ = writeln!(out, "Claude Code's own cost counter for {n} sessions: {}; tokprof for the same sessions: {}", usd(reported), usd(ours));
    }
    out.push('\n');
    out.push_str(&work(&c.work, 15, 15));
    out.push('\n');
    out.push_str(&miss_causes(c, false));
    out
}

/// Spend by origin, and the most expensive tasks.
pub fn work(w: &Work, top_origins: usize, top_tasks: usize) -> String {
    let origins = w.origins();
    let priced = origins.iter().any(|(_, s)| s.usd > 0.0);
    let key = |s: &Spend| if priced { s.usd } else { s.tokens };
    let total: f64 = origins.iter().map(|(_, s)| key(s)).sum();
    let share = |s: &Spend| format!("{:.1}%", 100.0 * key(s) / total.max(1e-9));
    let amount = |s: &Spend| if priced { usd(s.usd) } else { format!("{} tok", tokens(s.tokens)) };

    let mut out = String::from("Spend by origin: the kind of work each request was part of\n\n");
    let mut rows = vec![["origin", "requests", "tokens", "cost", "share"].map(str::to_string).to_vec()];
    let mut rest = (0, Spend::default());
    for (i, (origin, s)) in origins.iter().enumerate() {
        if i < top_origins {
            rows.push(vec![origin.to_string(), s.requests.to_string(), tokens(s.tokens), amount(s), share(s)]);
        } else {
            rest.0 += 1;
            rest.1.add(*s);
        }
    }
    if rest.0 > 0 {
        let s = rest.1;
        rows.push(vec![format!("{} more", rest.0), s.requests.to_string(), tokens(s.tokens), amount(&s), share(&s)]);
    }
    out.push_str(&table(&rows, 1));

    let _ = writeln!(out, "\nMost expensive tasks: session titles, and what subagents were asked to do\n");
    let mut rows = vec![["task", "origin", "requests", "cost", "share"].map(str::to_string).to_vec()];
    for (task, origin, s) in w.tasks().into_iter().take(top_tasks) {
        rows.push(vec![clip_end(task, 60), origin.to_string(), s.requests.to_string(), amount(&s), share(&s)]);
    }
    out.push_str(&table(&rows, 2));
    out
}

/// The text cut to `width` characters, with an ellipsis where it was cut.
pub(crate) fn clip_end(s: &str, width: usize) -> String {
    let s = s.split_whitespace().collect::<Vec<_>>().join(" ");
    if s.chars().count() <= width {
        return s;
    }
    let head: String = s.chars().take(width - 1).collect();
    format!("{}…", head.trim_end())
}

pub fn miss_causes(c: &Collector, include_expected: bool) -> String {
    let mut by: BTreeMap<(Agent, Cause), (u64, f64, f64, bool)> = BTreeMap::new();
    for m in &c.misses {
        let e = by.entry((m.agent, m.cause)).or_default();
        e.0 += 1;
        e.1 += m.rewritten as f64;
        e.2 += m.premium_usd.unwrap_or(0.0);
        e.3 |= m.premium_usd.is_some();
    }
    let mut rows = vec![["agent", "cause", "misses", "re-processed", "avoidable"].map(str::to_string).to_vec()];
    let mut entries: Vec<_> = by.into_iter().filter(|((_, cause), _)| include_expected || !cause.expected()).collect();
    entries.sort_by(|a, b| b.1.2.total_cmp(&a.1.2).then(b.1.1.total_cmp(&a.1.1)));
    for ((agent, cause), (n, tok, prem, priced)) in entries {
        rows.push(vec![
            agent.name().to_string(),
            format!("{}{}", cause.name(), if cause.expected() { " (expected)" } else { "" }),
            n.to_string(),
            tokens(tok),
            if priced { usd(prem) } else { "-".to_string() },
        ]);
    }
    let mut out = String::from("Cache misses by cause\n\n");
    out.push_str(&table(&rows, 2));
    out
}

pub fn misses(c: &Collector, top: usize, include_expected: bool) -> String {
    let mut out = miss_causes(c, include_expected);
    let mut events: Vec<&MissEvent> = c.misses.iter().filter(|m| include_expected || !m.cause.expected()).collect();
    events.sort_by(|a, b| {
        b.premium_usd.unwrap_or(0.0).total_cmp(&a.premium_usd.unwrap_or(0.0)).then(b.rewritten.cmp(&a.rewritten))
    });
    let mut rows = vec![["when", "agent", "project", "session", "chain", "context", "re-processed", "avoidable", "cause"]
        .map(str::to_string)
        .to_vec()];
    for m in events.into_iter().take(top) {
        let when = chrono::DateTime::from_timestamp_millis(m.ts_ms)
            .map(|t| t.with_timezone(&chrono::Local).format("%Y-%m-%d %H:%M").to_string())
            .unwrap_or_default();
        rows.push(vec![
            when,
            m.agent.name().to_string(),
            m.project.clone(),
            m.session.clone(),
            m.chain.clone(),
            tokens(m.context as f64),
            tokens(m.rewritten as f64),
            m.premium_usd.map_or("-".to_string(), usd),
            if m.detail.is_empty() { m.cause.name().to_string() } else { format!("{}: {}", m.cause.name(), m.detail) },
        ]);
    }
    let _ = writeln!(out, "\nMost expensive misses\n");
    out.push_str(&table(&rows, 5));
    out
}

pub fn whatif(c: &Collector) -> String {
    let w = &c.whatif;
    let mut out = String::from(
        "Claude input cost under each cache TTL, replaying the same requests.\n\
         The model counts only TTL expiry, so compare the simulated columns with each other.\n\n",
    );
    let mut rows = vec![["requests", "bucket", "actual", "simulated 5m", "simulated 1h"].map(str::to_string).to_vec()];
    for (i, name) in ["main conversation", "subagents and side requests"].into_iter().enumerate() {
        rows.push(vec![
            w.requests[i].to_string(),
            name.to_string(),
            usd(w.actual[i]),
            usd(w.simulated[i][0]),
            usd(w.simulated[i][1]),
        ]);
    }
    out.push_str(&table(&rows, 0));
    let best = |i: usize| if w.simulated[i][0] < w.simulated[i][1] { "5m" } else { "1h" };
    let saving = (0..2).map(|i| w.simulated[i][1].max(w.simulated[i][0]) - w.simulated[i][1].min(w.simulated[i][0])).collect::<Vec<_>>();
    let _ = writeln!(
        out,
        "\nCheapest: main conversation {} (saves {} over the other), subagents {} (saves {}).",
        best(0),
        usd(saving[0]),
        best(1),
        usd(saving[1])
    );
    let _ = writeln!(out, "Claude Code settings: promptCacheTtl and subagentPromptCacheTtl (\"5m\" or \"1h\").");
    out
}

/// The price table; with `at`, only the prices in effect at that moment.
pub fn prices(p: &Pricing, at: Option<(i64, &str)>) -> String {
    let mut out = String::from("Prices in USD per million tokens. Later layers replace earlier ones:\n");
    for layer in p.layers() {
        let state = match (&layer.path, layer.loaded) {
            (None, _) => "prices.toml in the binary".to_string(),
            (Some(path), true) => path.display().to_string(),
            (Some(path), false) => format!("{} (not found)", path.display()),
        };
        let _ = writeln!(out, "  {:<12} {state}", layer.name);
    }
    if let Some((_, shown)) = at {
        let _ = writeln!(out, "Showing the prices in effect on {shown}.");
    }
    let at = at.map(|(t, _)| t);
    out.push('\n');
    let num = |x: f64| format!("{x:.3}").trim_end_matches('0').trim_end_matches('.').to_string();
    let cells = |p: &Price| [p.input, p.write_short, p.write_long, p.read, p.output].map(num).to_vec();
    let mut rows = vec![["model", "input", "write short", "write long", "read", "output", "in effect", "source"].map(str::to_string).to_vec()];
    let mut entries: Vec<_> = p.entries().iter().collect();
    entries.sort_by(|a, b| a.model.cmp(&b.model));
    for e in entries {
        let periods: Vec<_> = e.periods.iter().filter(|period| at.is_none_or(|t| period.covers(t))).collect();
        for (i, period) in periods.iter().enumerate() {
            let when = match (period.since_ms, period.until_ms) {
                (None, None) => "always".to_string(),
                (Some(s), None) => format!("since {}", day(s)),
                (None, Some(u)) => format!("until {}", day(u)),
                (Some(s), Some(u)) => format!("{} to {}", day(s), day(u)),
            };
            let mut row = vec![if i == 0 { e.model.clone() } else { String::new() }];
            row.extend(cells(&period.price));
            row.push(when);
            row.push(if i == 0 { e.source.to_string() } else { String::new() });
            rows.push(row);
            if let Some((above, long)) = &period.long_context {
                let threshold = if above % 1000 == 0 { format!("{}k", above / 1000) } else { above.to_string() };
                let mut row = vec![format!("  over {threshold} tokens")];
                row.extend(cells(long));
                row.extend([String::new(), String::new()]);
                rows.push(row);
            }
        }
    }
    out.push_str(&table(&rows, 1));
    let _ = writeln!(out, "\nShort and long writes are the 5m and 1h cache lifetimes for Anthropic, 30m and 12h for OpenAI.");
    let _ = writeln!(out, "Each request is priced by the period in effect when it was sent; `until` dates are excluded.");
    out
}

fn day(ts_ms: i64) -> String {
    chrono::DateTime::from_timestamp_millis(ts_ms).map(|t| t.format("%Y-%m-%d").to_string()).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_task_shows_the_origin_most_of_its_spend_came_from() {
        let price = Pricing::defaults().get("claude-opus-5-5", 0, 1);
        let mut w = Work::default();
        for (origin, task, read) in [("prompt", "Fix the parser", 1_000), ("skill:code-review", "Fix the parser", 5_000), ("agent:Explore", "Scan docs", 2_000)] {
            w.add(&RequestRecord {
                agent: Agent::Claude,
                chain_kind: ChainKind::Main,
                advisor: false,
                origin,
                task,
                model: "claude-opus-5-5",
                price,
                usage: Usage { cache_read: read, ..Usage::default() },
                gap_s: None,
                prev_prompt: None,
                composition: true,
            });
        }
        let tasks = w.tasks();
        assert_eq!((tasks[0].0, tasks[0].1, tasks[0].2.requests), ("Fix the parser", "skill:code-review", 2));
        assert_eq!(w.origins().iter().map(|(o, _)| *o).collect::<Vec<_>>(), ["skill:code-review", "agent:Explore", "prompt"]);
        assert_eq!(dims("origin, task"), [Dim::Origin, Dim::Task]);
    }

    #[test]
    fn long_text_is_cut_at_the_end() {
        assert_eq!(clip_end("Квартальные отчёты отдела", 12), "Квартальные…");
        assert_eq!(clip_end("a  b\nc", 10), "a b c");
    }
}
