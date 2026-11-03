//! Setup: what an agent adds to every request besides the work, and what it cost.
//!
//! Setup is the tool schemas, the system prompt and the reminders that set the request up (see
//! `model::setup_source`). Its cost is the spend that cost attribution charges to that content,
//! summed by agent and source, with the part charged to subagent chains apart.

use std::collections::{BTreeMap, HashMap};

use crate::analysis::{Fact, MissEvent, RequestRecord, Sink, billed, continues_earlier_chain, live_chain, prompt_of};
use crate::model::{BlockKind, Chain, ChainKind, Session, setup_source};
use crate::profile::Values;
use crate::source::AgentId;

/// What a chain starts with: the setup tokens by source in its first request whose prompt the
/// log shows, and its brief, the user prompt blocks of that request.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Start {
    pub by_source: BTreeMap<String, f64>,
    pub brief: f64,
}

impl Start {
    pub fn setup(&self) -> f64 {
        self.by_source.values().sum()
    }
}

/// What `chain` starts with, or `None` when the log shows none of its prompts. `msg_chars` is
/// `analysis::message_chars(s)`.
pub fn start_of(s: &Session, chain: &Chain, msg_chars: &[u64]) -> Option<Start> {
    let first = chain.requests.iter().position(|r| r.prompt.is_some() && billed(r))?;
    let mut start = Start::default();
    for (b, tokens, _) in prompt_of(s, chain, first, msg_chars).blocks {
        let Some(b) = b else { continue };
        let blk = &s.blocks[b];
        if let Some(source) = setup_source(blk.kind, blk.class, blk.target.as_deref()) {
            *start.by_source.entry(source.to_string()).or_default() += tokens;
        } else if blk.kind == BlockKind::Prompt {
            start.brief += tokens;
        }
    }
    Some(start)
}

/// What the session's main conversation starts with: `None` when the log does not show its
/// first prompt, as when its live chain continues an earlier chain the log has no prompts for.
pub fn main_start(s: &Session, msg_chars: &[u64]) -> Option<Start> {
    let (chain, _) = live_chain(s)?;
    if continues_earlier_chain(s, chain) {
        return None;
    }
    start_of(s, chain, msg_chars)
}

/// The mean of several starts: each source's tokens and the brief over `count` starts.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MeanStart {
    pub count: usize,
    pub by_source: BTreeMap<String, f64>,
    pub brief: f64,
}

impl MeanStart {
    pub fn of(starts: &[Start]) -> Option<MeanStart> {
        if starts.is_empty() {
            return None;
        }
        let n = starts.len() as f64;
        let mut mean = MeanStart { count: starts.len(), ..MeanStart::default() };
        for st in starts {
            for (source, tokens) in &st.by_source {
                *mean.by_source.entry(source.clone()).or_default() += tokens / n;
            }
            mean.brief += st.brief / n;
        }
        Some(mean)
    }

    pub fn setup(&self) -> f64 {
        self.by_source.values().sum()
    }
}

/// What the main conversation's latest compaction left: the prompt of its first answered
/// request after the compaction marker, by content kind and reminder source.
#[derive(Clone, Debug, PartialEq)]
pub struct Leftover {
    pub at_ms: i64,
    pub size_tokens: u64,
    /// Kind or reminder source, with its tokens; largest first.
    pub parts: Vec<(String, f64)>,
}

/// What the latest compaction left, when a request after it in the main conversation's live
/// chain has a prompt the log shows.
pub fn leftover(s: &Session, msg_chars: &[u64]) -> Option<Leftover> {
    let c = s.compactions.iter().max_by_key(|c| c.ts_ms)?;
    let (chain, _) = live_chain(s)?;
    let i = chain.requests.iter().position(|r| r.ts_ms >= c.ts_ms && billed(r))?;
    let req = &chain.requests[i];
    req.prompt.as_ref()?;
    let mut parts: BTreeMap<String, f64> = BTreeMap::new();
    for (b, tokens, _) in prompt_of(s, chain, i, msg_chars).blocks {
        let name = match b.map(|b| &s.blocks[b]) {
            Some(blk) if blk.kind == BlockKind::Reminder => blk.target.clone().unwrap_or_else(|| "other".to_string()),
            Some(blk) => blk.kind.name().to_string(),
            None => BlockKind::Unknown.name().to_string(),
        };
        *parts.entry(name).or_default() += tokens;
    }
    let mut parts: Vec<(String, f64)> = parts.into_iter().collect();
    parts.sort_by(|a, b| b.1.total_cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    Some(Leftover { at_ms: c.ts_ms, size_tokens: req.usage.prompt(), parts })
}

/// What one setup source cost: all of it, and the part charged to subagent chains.
#[derive(Clone, Copy, Debug, Default)]
pub struct SetupCost {
    pub all: Values,
    pub subagents: Values,
}

impl SetupCost {
    fn add(&mut self, o: SetupCost) {
        self.all.add(o.all);
        self.subagents.add(o.subagents);
    }
}

/// Setup cost by agent and source, and each agent's whole spend, which shares are taken of.
#[derive(Clone, Debug, Default)]
pub struct SetupSpend {
    pub by_source: HashMap<(AgentId, String), SetupCost>,
    pub spend: HashMap<AgentId, Values>,
    /// Whether any share had a price; amounts are tokens otherwise.
    pub priced: bool,
}

impl SetupSpend {
    pub fn add(&mut self, f: &Fact) {
        let v = Values { tokens: f.tokens, usd: f.usd.unwrap_or(0.0), avoidable: f.avoidable_usd };
        self.priced |= f.usd.is_some();
        self.spend.entry(f.agent.clone()).or_default().add(v);
        let Some(source) = setup_source(f.kind, f.class, f.target) else { return };
        let subagents = if f.chain_kind == ChainKind::Subagent { v } else { Values::default() };
        self.by_source.entry((f.agent.clone(), source.to_string())).or_default().add(SetupCost { all: v, subagents });
    }

    pub fn merge(&mut self, o: SetupSpend) {
        for (key, cost) in o.by_source {
            self.by_source.entry(key).or_default().add(cost);
        }
        for (agent, v) in o.spend {
            self.spend.entry(agent).or_default().add(v);
        }
        self.priced |= o.priced;
    }

    /// The amount that ranks and sums: dollars when any share had a price, tokens otherwise.
    pub fn amount(&self, v: &Values) -> f64 {
        if self.priced { v.usd } else { v.tokens }
    }

    /// Every agent's rows, most expensive first, leaving out sources that cost nothing.
    pub fn rows(&self) -> Vec<(&AgentId, &str, SetupCost)> {
        let mut rows: Vec<(&AgentId, &str, SetupCost)> =
            self.by_source.iter().filter(|(_, c)| self.amount(&c.all) > 0.0).map(|((agent, source), c)| (agent, source.as_str(), *c)).collect();
        rows.sort_by(|a, b| self.amount(&b.2.all).total_cmp(&self.amount(&a.2.all)).then_with(|| (a.0, a.1).cmp(&(b.0, b.1))));
        rows
    }

    /// One agent's setup, all sources together.
    pub fn total(&self, agent: &AgentId) -> SetupCost {
        let mut total = SetupCost::default();
        for ((a, _), c) in &self.by_source {
            if a == agent {
                total.add(*c);
            }
        }
        total
    }
}

/// The setup of one session for the X-ray's setup block.
#[derive(Clone, Debug)]
pub struct SetupView {
    /// The setup, subagents included.
    pub total: SetupCost,
    /// Its share of the session's spend, or of its tokens without a price.
    pub share: f64,
    /// Whether amounts are dollars; tokens otherwise.
    pub priced: bool,
    /// Every source that cost something, most expensive first.
    pub rows: Vec<(String, SetupCost)>,
    /// Whether the table has the subagent columns: subagents had setup cost or a known start.
    pub has_subagents: bool,
    /// What the main conversation starts with, when the log shows it.
    pub main: Option<Start>,
    /// What the subagents start with on average.
    pub subagents: Option<MeanStart>,
    pub leftover: Option<Leftover>,
    /// The costliest source and its hint, when the agent's source gives one.
    pub hint: Option<(String, String)>,
}

/// The smallest setup the X-ray shows: $1 and 5% of the session, or 5% of its tokens without a
/// price.
pub const MIN_SETUP_USD: f64 = 1.0;
pub const MIN_SETUP_SHARE: f64 = 0.05;

/// The setup block of a session whose shares fed `setup`, which spent `spend` in all; `None`
/// below the thresholds.
pub fn setup_view(
    setup: &SetupSpend,
    agent: &AgentId,
    spend: Values,
    main: Option<Start>,
    subagents: Option<MeanStart>,
    leftover: Option<Leftover>,
) -> Option<SetupView> {
    let total = setup.total(agent);
    let (amount, whole) = if setup.priced { (total.all.usd, spend.usd) } else { (total.all.tokens, spend.tokens) };
    let share = if whole > 0.0 { amount / whole } else { 0.0 };
    if share < MIN_SETUP_SHARE || (setup.priced && amount < MIN_SETUP_USD) || amount <= 0.0 {
        return None;
    }
    let rows: Vec<(String, SetupCost)> = setup.rows().into_iter().filter(|(a, _, _)| *a == agent).map(|(_, source, cost)| (source.to_string(), cost)).collect();
    let has_subagents = subagents.is_some() || rows.iter().any(|(_, c)| setup.amount(&c.subagents) > 0.0);
    let hint = rows.first().and_then(|(source, _)| agent.info().setup_hints.iter().find(|(s, _)| s == source).map(|(s, h)| (s.clone(), h.clone())));
    Some(SetupView { total, share, priced: setup.priced, rows, has_subagents, main, subagents, leftover, hint })
}

impl Sink for SetupSpend {
    fn session(&mut self, _: &Session) {}
    fn request(&mut self, _: &RequestRecord) {}
    fn miss(&mut self, _: MissEvent) {}
    fn fact(&mut self, f: &Fact) {
        self.add(f);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::Component;
    use crate::model::{BlockKind, Class, ReminderKind};
    use crate::trigger::Trigger;

    fn fact<'a>(agent: &'a AgentId, chain_kind: ChainKind, kind: BlockKind, target: Option<&'a str>, tokens: f64, usd: Option<f64>) -> Fact<'a> {
        let class = match kind {
            BlockKind::Reminder => Class::Reminder(ReminderKind::of_label(target.unwrap_or_default())),
            _ => Class::None,
        };
        Fact {
            agent,
            project: "~/p",
            session: "s",
            chain: if chain_kind == ChainKind::Subagent { "subagent: Angle A" } else { "main" },
            chain_kind,
            origin: "prompt",
            trigger: Trigger::Prompt,
            activity: "prompt",
            task: "t",
            model: "claude-opus-5-5",
            turn: None,
            component: Component::Read,
            cause: None,
            kind,
            tool: None,
            target,
            class,
            tokens,
            usd,
            avoidable_usd: 0.0,
            ts_ms: 1,
        }
    }

    #[test]
    fn sums_setup_by_source_with_the_subagent_part_apart() {
        let claude = crate::testing::claude();
        let mut s = SetupSpend::default();
        s.add(&fact(&claude, ChainKind::Main, BlockKind::Reminder, Some("skills"), 1000.0, Some(0.20)));
        s.add(&fact(&claude, ChainKind::Subagent, BlockKind::Reminder, Some("skills"), 3000.0, Some(0.60)));
        s.add(&fact(&claude, ChainKind::Subagent, BlockKind::Tools, Some("Read"), 500.0, Some(0.10)));
        s.add(&fact(&claude, ChainKind::Main, BlockKind::Reminder, Some("background notifications"), 9000.0, Some(1.80)));
        s.add(&fact(&claude, ChainKind::Main, BlockKind::ToolResult, Some("src/main.rs"), 2000.0, Some(0.40)));

        let rows = s.rows();
        let names: Vec<&str> = rows.iter().map(|(_, source, _)| *source).collect();
        assert_eq!(names, ["skills", "tool schemas"]);
        let skills = rows[0].2;
        assert!((skills.all.usd - 0.80).abs() < 1e-12 && (skills.subagents.usd - 0.60).abs() < 1e-12);
        assert_eq!(skills.all.tokens, 4000.0);
        assert!((s.total(&claude).all.usd - 0.90).abs() < 1e-12);
        assert!((s.spend[&claude].usd - 3.10).abs() < 1e-12, "the agent's spend counts every share");
    }

    #[test]
    fn without_a_price_it_counts_tokens() {
        let codex = crate::testing::codex();
        let mut s = SetupSpend::default();
        s.add(&fact(&codex, ChainKind::Main, BlockKind::Reminder, Some("instructions"), 700.0, None));
        s.add(&fact(&codex, ChainKind::Main, BlockKind::System, None, 300.0, None));
        assert!(!s.priced);
        let rows = s.rows();
        assert_eq!(rows.iter().map(|(_, source, c)| (*source, s.amount(&c.all))).collect::<Vec<_>>(), [("instructions", 700.0), ("system prompt", 300.0)]);
    }

    use crate::analysis::message_chars;
    use crate::model::{Block, Compaction, CompactionTrigger, PromptDelta, Request, Usage};

    fn reminder(label: &str, chars: u64) -> Block {
        Block { class: Class::Reminder(ReminderKind::of_label(label)), kind: BlockKind::Reminder, chars, tool: None, target: Some(label.into()) }
    }

    fn text(kind: BlockKind, chars: u64) -> Block {
        Block { class: Class::None, kind, chars, tool: None, target: None }
    }

    /// A request whose prompt is `tokens` long, so that one character is one token when the
    /// prompt's blocks hold as many characters.
    fn req(ts_ms: i64, tokens: u64, prompt: Option<PromptDelta>) -> Request {
        Request { ts_ms, model: "claude-opus-5-5".into(), usage: Usage { cache_read: tokens, output: 10, ..Usage::default() }, prompt, ..Request::default() }
    }

    fn delta(keep: usize, tail: Vec<usize>) -> Option<PromptDelta> {
        Some(PromptDelta { shape: None, keep, tail })
    }

    #[test]
    fn a_subagent_starts_with_its_setup_and_brief() {
        let mut s = Session::new(crate::testing::claude(), "a1".into());
        let first = s.push_message(
            vec![reminder("instructions", 2_000), reminder("skills", 3_000), reminder("background notifications", 1_000), text(BlockKind::Prompt, 500)],
            true,
        );
        let result = s.push_message(vec![text(BlockKind::ToolResult, 4_000)], false);
        let requests = vec![req(1, 6_500, delta(0, vec![first])), req(2, 10_500, delta(1, vec![result]))];
        s.chains.push(Chain { name: "subagent: Angle A".into(), kind: ChainKind::Subagent, requests });

        let start = start_of(&s, &s.chains[0], &message_chars(&s)).expect("a known first prompt");
        assert_eq!(start.by_source.iter().map(|(k, v)| (k.as_str(), *v)).collect::<Vec<_>>(), [("instructions", 2_000.0), ("skills", 3_000.0)]);
        assert_eq!((start.setup(), start.brief), (5_000.0, 500.0));
    }

    #[test]
    fn a_subagent_without_a_known_prompt_is_not_counted() {
        let mut s = Session::new(crate::testing::claude(), "a2".into());
        s.chains.push(Chain { name: "subagent".into(), kind: ChainKind::Subagent, requests: vec![req(1, 30_000, None)] });
        assert!(start_of(&s, &s.chains[0], &message_chars(&s)).is_none());

        let a = Start { by_source: [("skills".to_string(), 3_000.0)].into(), brief: 400.0 };
        let b = Start { by_source: [("skills".to_string(), 1_000.0), ("instructions".to_string(), 2_000.0)].into(), brief: 800.0 };
        let mean = MeanStart::of(&[a, b]).unwrap();
        assert_eq!((mean.count, mean.by_source["skills"], mean.by_source["instructions"], mean.brief), (2, 2_000.0, 1_000.0, 600.0));
        assert_eq!(mean.setup(), 3_000.0);
        assert!(MeanStart::of(&[]).is_none());
    }

    #[test]
    fn a_continued_main_chain_has_no_known_start() {
        let mut s = Session::new(crate::testing::claude(), "m".into());
        let first = s.push_message(vec![reminder("instructions", 2_000), text(BlockKind::Prompt, 100)], true);
        s.chains.push(Chain { name: "main".into(), kind: ChainKind::Main, requests: vec![req(1, 9_000, None)] });
        s.chains.push(Chain { name: "main".into(), kind: ChainKind::Main, requests: vec![req(10, 2_100, delta(0, vec![first]))] });
        assert!(main_start(&s, &message_chars(&s)).is_none());
        s.chains.remove(0);
        assert_eq!(main_start(&s, &message_chars(&s)).unwrap().setup(), 2_000.0);
    }

    #[test]
    fn a_compaction_leaves_what_the_next_prompt_holds() {
        let mut s = Session::new(crate::testing::claude(), "c".into());
        let before = s.push_message(vec![reminder("skills", 5_000), text(BlockKind::Prompt, 100)], true);
        let summary = s.push_message(vec![text(BlockKind::Prompt, 1_000)], false);
        let restored = s.push_message(vec![reminder("attached files", 3_000), reminder("skills", 2_000)], false);
        let requests = vec![req(5, 5_100, delta(0, vec![before])), req(12, 6_000, delta(0, vec![summary, restored]))];
        s.chains.push(Chain { name: "main".into(), kind: ChainKind::Main, requests });
        s.compactions.push(Compaction { ts_ms: 10, kind: CompactionTrigger::Auto });

        let left = leftover(&s, &message_chars(&s)).expect("a known prompt after the compaction");
        assert_eq!((left.at_ms, left.size_tokens), (10, 6_000));
        assert_eq!(left.parts, [("attached files".to_string(), 3_000.0), ("skills".to_string(), 2_000.0), ("user prompt".to_string(), 1_000.0)]);

        // A compaction after the latest request leaves nothing to show yet.
        s.compactions.push(Compaction { ts_ms: 20, kind: CompactionTrigger::Idle });
        assert!(leftover(&s, &message_chars(&s)).is_none());
        // Nor does a session without compactions.
        s.compactions.clear();
        assert!(leftover(&s, &message_chars(&s)).is_none());
    }

    /// A session's setup: `(source, usd, subagent usd)` shares, priced at $0.20 per 1,000 tokens.
    fn session_setup(agent: &AgentId, shares: &[(&str, f64, f64)]) -> SetupSpend {
        let mut s = SetupSpend::default();
        for &(source, usd, sub) in shares {
            let (kind, target) = match source {
                "tool schemas" => (BlockKind::Tools, None),
                "system prompt" => (BlockKind::System, None),
                label => (BlockKind::Reminder, Some(label)),
            };
            if usd - sub > 0.0 {
                s.add(&fact(agent, ChainKind::Main, kind, target, (usd - sub) * 5_000.0, Some(usd - sub)));
            }
            if sub > 0.0 {
                s.add(&fact(agent, ChainKind::Subagent, kind, target, sub * 5_000.0, Some(sub)));
            }
        }
        s
    }

    fn spend(usd: f64) -> Values {
        Values { tokens: usd * 5_000.0, usd, avoidable: 0.0 }
    }

    #[test]
    fn a_code_review_shows_its_setup_with_the_subagent_part() {
        let claude = crate::testing::claude();
        let setup = session_setup(&claude, &[("instructions", 23.56, 22.10), ("skills", 21.92, 21.03), ("tool schemas", 9.29, 8.67)]);
        let mean = MeanStart { count: 96, by_source: [("instructions".to_string(), 9_700.0), ("skills".to_string(), 9_600.0)].into(), brief: 1_800.0 };
        let view = setup_view(&setup, &claude, spend(526.0), None, Some(mean), None).expect("the block");
        assert!((view.total.all.usd - 54.77).abs() < 1e-9 && (view.total.subagents.usd - 51.80).abs() < 1e-9);
        assert!((view.share - 54.77 / 526.0).abs() < 1e-12);
        assert_eq!(view.rows.iter().map(|(s, _)| s.as_str()).collect::<Vec<_>>(), ["instructions", "skills", "tool schemas"]);
        assert!(view.has_subagents && view.main.is_none());
        // Instructions cost the most, and the test agent has a hint only for skills.
        assert!(view.hint.is_none());
        let setup = session_setup(&claude, &[("skills", 21.92, 21.03), ("instructions", 10.0, 9.0)]);
        let view = setup_view(&setup, &claude, spend(526.0), None, None, None).unwrap();
        assert_eq!(view.hint.as_ref().map(|(s, _)| s.as_str()), Some("skills"));
        assert!(view.hint.unwrap().1.contains("skillListingBudgetFraction"));
    }

    #[test]
    fn a_small_setup_has_no_block() {
        let claude = crate::testing::claude();
        let setup = session_setup(&claude, &[("instructions", 0.40, 0.0)]);
        assert!(setup_view(&setup, &claude, spend(2.0), None, None, None).is_none(), "under $1");
        let setup = session_setup(&claude, &[("instructions", 3.0, 0.0)]);
        assert!(setup_view(&setup, &claude, spend(100.0), None, None, None).is_none(), "3% of the session");
        assert!(setup_view(&setup, &claude, spend(60.0), None, None, None).is_some(), "5% and over $1");
    }

    #[test]
    fn without_subagents_there_are_no_subagent_columns() {
        let claude = crate::testing::claude();
        let setup = session_setup(&claude, &[("instructions", 4.0, 0.0), ("system prompt", 1.0, 0.0)]);
        let view = setup_view(&setup, &claude, spend(40.0), None, None, None).unwrap();
        assert!(!view.has_subagents);
    }

    #[test]
    fn an_unpriced_session_compares_tokens() {
        let codex = crate::testing::codex();
        let mut setup = SetupSpend::default();
        setup.add(&fact(&codex, ChainKind::Main, BlockKind::Reminder, Some("instructions"), 6_000.0, None));
        let tokens = |t: f64| Values { tokens: t, usd: 0.0, avoidable: 0.0 };
        assert!(setup_view(&setup, &codex, tokens(100_000.0), None, None, None).is_some(), "6% of the tokens");
        assert!(setup_view(&setup, &codex, tokens(200_000.0), None, None, None).is_none(), "3% of the tokens");
    }

    #[test]
    fn merging_adds_up_sources_and_spend() {
        let claude = crate::testing::claude();
        let (mut a, mut b) = (SetupSpend::default(), SetupSpend::default());
        a.add(&fact(&claude, ChainKind::Main, BlockKind::Reminder, Some("instructions"), 100.0, Some(0.02)));
        b.add(&fact(&claude, ChainKind::Subagent, BlockKind::Reminder, Some("instructions"), 300.0, Some(0.06)));
        a.merge(b);
        let c = a.by_source[&(claude.clone(), "instructions".to_string())];
        assert_eq!((c.all.tokens, c.subagents.tokens), (400.0, 300.0));
        assert!((a.spend[&claude].usd - 0.08).abs() < 1e-12 && a.priced);
    }
}
