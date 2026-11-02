//! What comes next for a session: whether its prompt cache is still warm, what its next request
//! costs at its pace, and what picking up its work in a new session costs.

use std::borrow::Borrow;

use crate::analysis::LiveContext;
use crate::model::{Request, Session};
use crate::source::AgentId;
use crate::pricing::{Price, Pricing};

/// Prompt-cache state of the main conversation.
#[derive(Clone, Debug)]
pub struct CacheState {
    pub long_ttl: bool,
    pub ttl_ms: i64,
    pub ttl_name: &'static str,
    pub idle_ms: i64,
    /// What the next request costs on top of a warm one once the cache is cold.
    pub rebuild_usd: Option<f64>,
    /// Estimated cost of the next request while the cache is warm, and once it is cold, from
    /// the session's recent writes and output per request.
    pub next_warm_usd: Option<f64>,
    pub next_cold_usd: Option<f64>,
}

impl CacheState {
    pub fn cold(&self) -> bool {
        self.idle_ms >= self.ttl_ms
    }
}

/// A request writes about this much summary when the conversation is compacted.
pub const SUMMARY_TOKENS: f64 = 10_000.0;

/// The requests of the conversation the live context belongs to, in time order, across the
/// chains it is kept in (see `Session::main_conversations`).
pub fn conversation<'a>(s: &'a Session, live: &LiveContext) -> Vec<&'a Request> {
    s.main_conversations()
        .into_iter()
        .find(|(name, requests)| *name == live.chain && requests.iter().any(|r| r.ts_ms == live.ts_ms))
        .map(|(_, requests)| requests)
        .unwrap_or_default()
}

/// The main conversation's cache lifetime, from its latest cache write, and how long it idled.
pub fn cache_state(s: &Session, live: &LiveContext, pricing: &Pricing, now_ms: i64) -> CacheState {
    let requests = conversation(s, live);
    let long_ttl = long_ttl(&requests);
    // The rules of the API the conversation's latest request went to; an API without known rules
    // never counts as cold.
    let api = requests.last().map_or_else(Default::default, |r| r.api);
    let (ttl_ms, ttl_name) = api.ttl(long_ttl).unwrap_or((i64::MAX, "unknown"));
    let size = live.usage.prompt() as f64;
    let (added, output) = pace(&requests);
    // The next request is sent now, at today's prices.
    let price = pricing.get(&live.model, size as u64, now_ms);
    let next = price.map(|p| next_request(size, added, output, p, long_ttl));
    CacheState {
        long_ttl,
        ttl_ms,
        ttl_name,
        idle_ms: now_ms - live.ts_ms,
        rebuild_usd: price.map(|p| size * (if long_ttl { p.write_long } else { p.write_short } - p.read) / 1e6),
        next_warm_usd: next.map(|n| n.0),
        next_cold_usd: next.map(|n| n.1),
    }
}

/// Whether the latest cache write of a conversation used the long lifetime.
pub fn long_ttl<R: Borrow<Request>>(requests: &[R]) -> bool {
    requests.iter().map(Borrow::borrow).rev().find(|r| r.usage.cache_write() > 0).is_some_and(|r| r.usage.write_long > r.usage.write_short)
}

/// What a request of a conversation usually adds, over its latest 20 requests: new content
/// written to the prompt, and output.
pub fn pace<R: Borrow<Request>>(requests: &[R]) -> (f64, f64) {
    let recent: Vec<&Request> = requests.iter().map(Borrow::borrow).rev().filter(|r| r.usage.prompt() > 0 && !r.advisor).take(20).collect();
    let mean = |f: &dyn Fn(&Request) -> u64| recent.iter().map(|r| f(r) as f64).sum::<f64>() / recent.len().max(1) as f64;
    (mean(&|r| r.usage.cache_write() + r.usage.uncached), mean(&|r| r.usage.output))
}

/// The next request on top of a prompt of `size` tokens at this pace: its cost while the
/// cache is warm, and once it is cold and the prompt is written again.
pub fn next_request(size: f64, added: f64, output: f64, p: Price, long_ttl: bool) -> (f64, f64) {
    let write = if long_ttl { p.write_long } else { p.write_short };
    ((size * p.read + added * write + output * p.output) / 1e6, ((size + added) * write + output * p.output) / 1e6)
}

/// What picking up the work costs in a new session that refers to this one, as the reference
/// bench measured it (docs/reference-bench.md). The new session starts from `start` tokens,
/// takes in what it needs from the old log and the project, and answers.
pub fn pickup_usd(agent: &AgentId, start: f64, p: Price, write: f64) -> Option<f64> {
    let (taken, requests, output) = pickup_shape(agent)?;
    Some(((start + taken) * write + (requests - 1.0) * (start + taken / 2.0) * p.read + output * p.output) / 1e6)
}

/// Tokens taken in, requests and output of a pickup, per agent.
pub(crate) fn pickup_shape(agent: &AgentId) -> Option<(f64, f64, f64)> {
    let p = agent.info().harness.pickup?;
    Some((p.taken_tokens, p.requests, p.output_tokens))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Chain, ChainKind, Usage};

    /// The estimate against the pickups the reference bench measured: Claude Code at
    /// $0.44–0.51 from 42–44k tokens, Codex with gpt-6-astra at $0.70–1.26 from 18k.
    #[test]
    fn pickup_estimates_match_the_reference_bench() {
        let pricing = crate::pricing::Pricing::defaults();
        let now = crate::TEST_NOW_MS;
        let opus = pricing.get("claude-opus-5-5", 0, now).unwrap();
        let astra = pricing.get("gpt-6-astra", 0, now).unwrap();
        let claude = pickup_usd(&crate::testing::claude(), 42_000.0, opus, opus.write_long).unwrap();
        let codex = pickup_usd(&crate::testing::codex(), 18_000.0, astra, astra.write_short).unwrap();
        assert!((0.44..=0.51).contains(&claude), "{claude}");
        assert!((0.70..=1.26).contains(&codex), "{codex}");
    }

    /// 0f54de16 ran on 10-12 and 10-13 without request-audit lines and was resumed on 10-25
    /// with them. Read from the audited chain alone, its first prompt was the 721k re-write.
    #[test]
    fn reads_both_chains_of_a_conversation() {
        let req = |ts_ms, uncached| Request { ts_ms, usage: Usage { uncached, ..Usage::default() }, ..Request::default() };
        let mut s = Session::new(crate::testing::claude(), "s".into());
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
            requests_per_prompt: 1.0,
        };
        let prompts: Vec<u64> = conversation(&s, &live).iter().map(|r| r.usage.prompt()).collect();
        assert_eq!(prompts, [34_800, 40_000, 721_000, 724_000]);

        // A chain of the same name that overlaps both parts in time is another conversation.
        s.chains.push(Chain { name: "main".into(), kind: ChainKind::Main, requests: vec![req(15, 70_000), req(35, 80_000)] });
        let live = LiveContext { ts_ms: 31, ..live };
        let prompts: Vec<u64> = conversation(&s, &live).iter().map(|r| r.usage.prompt()).collect();
        assert_eq!(prompts, [34_800, 40_000, 721_000, 724_000]);
    }
}
