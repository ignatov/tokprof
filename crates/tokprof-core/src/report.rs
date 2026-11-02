//! Collects analysis output: spend by model, component and work, misses, the cache-lifetime
//! replay and the profile stacks. The CLI renders the reports from it.

use std::collections::{BTreeMap, HashMap};

use crate::analysis::{Component, Fact, Filter, MissEvent, RequestRecord, Sink};
use crate::model::{ChainKind, Session, Usage};
use crate::profile::{Frame, Stacks, Tint, TimedStacks, Values};
use crate::source::AgentId;
use crate::tools::{BlockEntry, ToolShare, ToolTable};
use crate::trigger::Trigger;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dim {
    Agent,
    Project,
    Session,
    Chain,
    Origin,
    Task,
    Trigger,
    Activity,
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
    pub const NAMES: &str = "agent, project, session, chain, origin, task, trigger, activity, model, turn, component, cause, kind, tool, target";

    /// Short name, used as the class name of JFR frames.
    pub fn name(self) -> &'static str {
        match self {
            Dim::Agent => "agent",
            Dim::Project => "project",
            Dim::Session => "session",
            Dim::Chain => "chain",
            Dim::Origin => "origin",
            Dim::Task => "task",
            Dim::Trigger => "trigger",
            Dim::Activity => "activity",
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
            "trigger" => Dim::Trigger,
            "activity" => Dim::Activity,
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
            Dim::Trigger => (f.trigger.name().to_string(), Tint::Structure),
            Dim::Activity => (f.activity.to_string(), Tint::Structure),
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

/// Spend of whole requests: how many, their tokens and their cost.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
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

/// Spend of the requests with one trigger, and the prompt tokens they read.
#[derive(Clone, Copy, Debug, Default)]
pub struct TriggerSpend {
    pub spend: Spend,
    pub prompt_tokens: f64,
}

impl TriggerSpend {
    pub fn add(&mut self, o: TriggerSpend) {
        self.spend.add(o.spend);
        self.prompt_tokens += o.prompt_tokens;
    }

    /// The mean prompt of the requests.
    pub fn avg_prompt(&self) -> f64 {
        self.prompt_tokens / self.spend.requests.max(1) as f64
    }
}

/// Spend by the kind of work the requests were part of, by what the work was, and by what
/// made the agent send them.
#[derive(Clone, Debug, Default)]
pub struct Work {
    pub by_origin: HashMap<String, Spend>,
    /// Task, then origin.
    pub by_task: HashMap<String, HashMap<String, Spend>>,
    pub by_trigger: HashMap<Trigger, TriggerSpend>,
    pub by_activity: HashMap<&'static str, TriggerSpend>,
}

impl Work {
    pub fn add(&mut self, r: &RequestRecord) {
        let s = Spend { requests: 1, tokens: (r.usage.prompt() + r.usage.output) as f64, usd: r.price.map_or(0.0, |p| p.cost(&r.usage)) };
        add_spend(&mut self.by_origin, r.origin, s);
        if !self.by_task.contains_key(r.task) {
            self.by_task.insert(r.task.to_string(), HashMap::new());
        }
        add_spend(self.by_task.get_mut(r.task).expect("inserted above"), r.origin, s);
        let t = TriggerSpend { spend: s, prompt_tokens: r.usage.prompt() as f64 };
        self.by_trigger.entry(r.trigger).or_default().add(t);
        self.by_activity.entry(r.activity).or_default().add(t);
    }

    pub fn merge(&mut self, o: Work) {
        for (origin, s) in o.by_origin {
            self.by_origin.entry(origin).or_default().add(s);
        }
        for (task, origins) in o.by_task {
            let t = self.by_task.entry(task).or_default();
            for (origin, s) in origins {
                t.entry(origin).or_default().add(s);
            }
        }
        for (trigger, s) in o.by_trigger {
            self.by_trigger.entry(trigger).or_default().add(s);
        }
        for (activity, s) in o.by_activity {
            self.by_activity.entry(activity).or_default().add(s);
        }
    }

    /// Activities, most expensive first.
    pub fn activities(&self) -> Vec<(&'static str, TriggerSpend)> {
        let mut rows: Vec<(&'static str, TriggerSpend)> = self.by_activity.iter().map(|(a, s)| (*a, *s)).collect();
        rows.sort_by(|a, b| b.1.spend.usd.total_cmp(&a.1.spend.usd).then(b.1.spend.tokens.total_cmp(&a.1.spend.tokens)).then(a.0.cmp(b.0)));
        rows
    }

    /// Triggers, most expensive first.
    pub fn triggers(&self) -> Vec<(Trigger, TriggerSpend)> {
        let mut rows: Vec<(Trigger, TriggerSpend)> = self.by_trigger.iter().map(|(t, s)| (*t, *s)).collect();
        rows.sort_by(|a, b| b.1.spend.usd.total_cmp(&a.1.spend.usd).then(b.1.spend.tokens.total_cmp(&a.1.spend.tokens)).then(a.0.cmp(&b.0)));
        rows
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
    /// The first and the last request of the session, in every chain and file.
    span: Option<(i64, i64)>,
}

fn union(a: Option<(i64, i64)>, b: Option<(i64, i64)>) -> Option<(i64, i64)> {
    match (a, b) {
        (Some(a), Some(b)) => Some((a.0.min(b.0), a.1.max(b.1))),
        (a, b) => a.or(b),
    }
}

/// Simulated input cost of the Claude requests under each TTL policy.
#[derive(Clone, Debug, Default)]
pub struct WhatIf {
    pub actual: [f64; 2],
    /// By chain bucket, then policy: bucket 0 = main conversation, 1 = everything else; policy 0 = 5m, 1 = 1h.
    pub simulated: [[f64; 2]; 2],
    pub requests: [u64; 2],
}

/// Spend by model and cost component, and per session for the reconciliation with the agents'
/// own cost counters.
#[derive(Default)]
pub struct ModelSpend {
    pub models: BTreeMap<(AgentId, String), ModelTotals>,
    sessions: HashMap<(AgentId, String), SessionTotals>,
}

impl ModelSpend {
    pub fn merge(&mut self, o: ModelSpend) {
        for (k, v) in o.models {
            let m = self.models.entry(k).or_default();
            m.requests += v.requests;
            m.with_prompt += v.with_prompt;
            m.priced |= v.priced;
            for (a, b) in m.by_component.iter_mut().zip(v.by_component) {
                a.add(b);
            }
        }
        for (k, v) in o.sessions {
            let s = self.sessions.entry(k).or_default();
            s.ours += v.ours;
            s.reported = max_opt(s.reported, v.reported);
            s.span = union(s.span, v.span);
        }
    }
}

impl Sink for ModelSpend {
    fn session(&mut self, s: &Session) {
        // Keyed by the short id that facts carry; subagent files share their parent's id.
        let e = self.sessions.entry((s.agent.clone(), s.short_id().to_string())).or_default();
        e.reported = max_opt(e.reported, s.reported_usd);
        let times = s.chains.iter().flat_map(|c| &c.requests).map(|r| r.ts_ms);
        let span = times.clone().min().zip(times.max());
        e.span = union(e.span, span);
    }

    fn request(&mut self, r: &RequestRecord) {
        let m = self.models.entry((r.agent.clone(), r.model.to_string())).or_default();
        m.requests += 1;
        m.with_prompt += r.composition as u64;
        m.priced |= r.price.is_some();
    }

    fn miss(&mut self, _: MissEvent) {}

    fn fact(&mut self, f: &Fact) {
        let values = Values { tokens: f.tokens, usd: f.usd.unwrap_or(0.0), avoidable: f.avoidable_usd };
        let m = self.models.entry((f.agent.clone(), f.model.to_string())).or_default();
        m.by_component[Component::ALL.iter().position(|c| *c == f.component).unwrap_or(0)].add(values);
        if let Some(usd) = f.usd {
            self.sessions.entry((f.agent.clone(), f.session.to_string())).or_default().ours += usd;
        }
    }
}

impl Sink for Work {
    fn session(&mut self, _: &Session) {}

    fn request(&mut self, r: &RequestRecord) {
        self.add(r);
    }

    fn miss(&mut self, _: MissEvent) {}

    fn fact(&mut self, _: &Fact) {}
}

impl WhatIf {
    pub fn merge(&mut self, o: WhatIf) {
        for i in 0..2 {
            self.actual[i] += o.actual[i];
            self.requests[i] += o.requests[i];
            for p in 0..2 {
                self.simulated[i][p] += o.simulated[i][p];
            }
        }
    }
}

impl Sink for WhatIf {
    fn session(&mut self, _: &Session) {}

    fn request(&mut self, r: &RequestRecord) {
        // The replay needs an API with a short and a long cache lifetime to choose between.
        let Some((short, Some(long))) = r.api.rules().map(|c| (c.short, c.long)) else { return };
        let (Some(p), false) = (r.price, r.advisor) else { return };
        let u = &r.usage;
        let bucket = (r.chain_kind != ChainKind::Main) as usize;
        let t = u.prompt() as f64;
        self.requests[bucket] += 1;
        self.actual[bucket] += p.cost(&Usage { output: 0, ..*u });
        for (policy, (ttl_s, write)) in [(short.0 as f64 / 1000.0, p.write_short), (long.0 as f64 / 1000.0, p.write_long)].into_iter().enumerate() {
            let cost = match (r.prev_prompt, r.gap_s) {
                (Some(prev), Some(gap)) if gap <= ttl_s => {
                    let kept = (prev as f64).min(t);
                    kept * p.read + (t - kept) * write
                }
                _ => t * write,
            };
            self.simulated[bucket][policy] += cost / 1e6;
        }
    }

    fn miss(&mut self, _: MissEvent) {}

    fn fact(&mut self, _: &Fact) {}
}

/// The stacks of a profile: what each stack of the chosen dimensions spent, and for JFR the same
/// per thread and minute.
#[derive(Default)]
pub struct Profile {
    pub stack: Option<Vec<Dim>>,
    pub components: Option<Vec<Component>>,
    pub stacks: Stacks,
    /// For JFR output: the dimensions that name a thread. Facts then go to `timed`, by thread
    /// and by the minute their content entered the context.
    pub threads: Option<Vec<Dim>>,
    pub timed: TimedStacks,
}

impl Profile {
    pub fn merge(&mut self, o: Profile) {
        for (k, v) in o.stacks {
            self.stacks.entry(k).or_default().add(v);
        }
        for (k, v) in o.timed {
            self.timed.entry(k).or_default().add(v);
        }
    }
}

impl Sink for Profile {
    fn session(&mut self, _: &Session) {}

    fn request(&mut self, _: &RequestRecord) {}

    fn miss(&mut self, _: MissEvent) {}

    fn fact(&mut self, f: &Fact) {
        let Some(stack) = &self.stack else { return };
        if self.components.as_ref().is_some_and(|cs| !cs.contains(&f.component)) {
            return;
        }
        let values = Values { tokens: f.tokens, usd: f.usd.unwrap_or(0.0), avoidable: f.avoidable_usd };
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

/// Everything the reports and the profile writers show, gathered in one pass.
#[derive(Default)]
pub struct Collector {
    pub profile: Profile,
    pub spend: ModelSpend,
    pub misses: Vec<MissEvent>,
    pub whatif: WhatIf,
    pub work: Work,
    pub tools: ToolTable,
}

impl Collector {
    pub fn new(stack: Option<Vec<Dim>>, components: Option<Vec<Component>>) -> Collector {
        Collector { profile: Profile { stack, components, ..Profile::default() }, ..Collector::default() }
    }

    pub fn merge(&mut self, o: Collector) {
        self.profile.merge(o.profile);
        self.spend.merge(o.spend);
        self.misses.extend(o.misses);
        self.whatif.merge(o.whatif);
        self.work.merge(o.work);
        self.tools.merge(o.tools);
    }
}

impl Sink for Collector {
    fn session(&mut self, s: &Session) {
        self.spend.session(s);
    }

    fn request(&mut self, r: &RequestRecord) {
        self.spend.request(r);
        self.work.request(r);
        self.whatif.request(r);
    }

    fn miss(&mut self, miss: MissEvent) {
        self.misses.miss(miss);
    }

    fn fact(&mut self, f: &Fact) {
        self.spend.fact(f);
        self.profile.fact(f);
        self.tools.fact(f);
    }

    fn block_entered(&mut self, b: &BlockEntry) {
        self.tools.block_entered(b);
    }

    fn tool_share(&mut self, t: &ToolShare) {
        self.tools.tool_share(t);
    }
}

fn max_opt(a: Option<f64>, b: Option<f64>) -> Option<f64> {
    match (a, b) {
        (Some(x), Some(y)) => Some(x.max(y)),
        (x, None) => x,
        (None, y) => y,
    }
}

/// What an agent's own cost counter says against tokprof, for the sessions it counted.
#[derive(Clone, Debug, PartialEq)]
pub struct Reconciliation {
    pub agent: AgentId,
    /// How many sessions the counter covers.
    pub sessions: usize,
    pub reported_usd: f64,
    pub ours_usd: f64,
}

impl ModelSpend {
    /// One reconciliation per agent that reports its own cost, by agent. With a time window,
    /// only the sessions it holds whole count, since an agent's counter covers a session's whole
    /// life.
    pub fn reconciliation(&self, window: &Filter) -> Vec<Reconciliation> {
        let windowed = window.since_ms.is_some() || window.until_ms.is_some();
        let inside = |s: &SessionTotals| !windowed || s.span.is_some_and(|(first, last)| window.accepts(first) && window.accepts(last));
        let mut by_agent: BTreeMap<&AgentId, (f64, f64, usize)> = BTreeMap::new();
        for ((agent, _), s) in self.sessions.iter().filter(|(_, s)| inside(s)) {
            if let Some(r) = s.reported {
                let e = by_agent.entry(agent).or_default();
                *e = (e.0 + s.ours, e.1 + r, e.2 + 1);
            }
        }
        by_agent.into_iter().map(|(agent, (ours_usd, reported_usd, sessions))| Reconciliation { agent: agent.clone(), sessions, reported_usd, ours_usd }).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_pair_of_sinks_gets_everything_both_would() {
        use crate::model::{Chain, Request};
        let mut s = Session::new(crate::testing::claude(), "s".into());
        let request = Request { ts_ms: 1, model: "claude-opus-5-5".into(), usage: Usage { cache_read: 1_000, output: 10, ..Usage::default() }, ..Request::default() };
        s.chains.push(Chain { name: "main".into(), kind: ChainKind::Main, requests: vec![request] });
        let pricing = crate::pricing::Pricing::defaults();
        let mut pair = (ModelSpend::default(), Work::default());
        crate::analysis::analyze(&s, &pricing, &Filter::default(), &mut pair);
        let mut whole = Collector::new(None, None);
        crate::analysis::analyze(&s, &pricing, &Filter::default(), &mut whole);
        assert_eq!(pair.0.models.len(), whole.spend.models.len());
        assert_eq!(pair.1.origins().len(), whole.work.origins().len());
    }

    #[test]
    fn a_task_shows_the_origin_most_of_its_spend_came_from() {
        let price = crate::pricing::Pricing::defaults().get("claude-opus-5-5", 0, 1);
        let mut w = Work::default();
        for (origin, task, read) in [("prompt", "Fix the parser", 1_000), ("skill:code-review", "Fix the parser", 5_000), ("agent:Explore", "Scan docs", 2_000)] {
            w.add(&RequestRecord {
                agent: &crate::testing::claude(),
                ts_ms: 0,
                chain: "main",
                api: crate::api::ApiId::Anthropic,
                chain_kind: ChainKind::Main,
                advisor: false,
                origin,
                trigger: crate::trigger::Trigger::Prompt,
                activity: "prompt",
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
}
