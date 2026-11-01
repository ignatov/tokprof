//! Advice for one session, from what its X-ray shows: whether resuming pays, what fills the
//! context, where the money went and what the cache misses came from. The rules are
//! deterministic and every line carries the numbers behind it.

use std::collections::BTreeMap;

use crate::analysis::LiveContext;
use crate::analysis::cache::{Cause, human_duration};
use crate::model::{Agent, BlockKind, Session};
use crate::pricing::Price;
use crate::report::{tokens, usd};

/// Prompt-cache state of the main conversation.
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

pub struct Inputs<'a> {
    pub session: &'a Session,
    pub live: &'a LiveContext,
    pub price: Option<Price>,
    pub cache: &'a CacheState,
    pub window: Option<u64>,
    pub requests_per_prompt: f64,
    pub spend_usd: f64,
    pub thinking_usd: f64,
    pub effort: Option<String>,
    /// Unexpected misses by cause: count and avoidable cost.
    pub misses: Vec<(Cause, usize, f64)>,
    /// Prompt size right after the session's last compaction.
    pub after_compaction: Option<u64>,
    /// Prompt of the session's first request. With the prompt after a compaction, it bounds
    /// what a new session in the project starts with.
    pub first_prompt: Option<u64>,
}

/// Characters of one advice line: with 12 columns of label and marker it fits the X-ray pane
/// of `tokprof ui`.
pub const WIDTH: usize = 82;

/// A request writes about this much summary when the conversation is compacted.
pub(crate) const SUMMARY_TOKENS: f64 = 10_000.0;

/// What picking up the work costs in a new session that refers to this one, as the reference
/// bench measured it (docs/reference-bench.md). The new session starts from `start` tokens,
/// takes in what it needs from the old log and the project, and answers.
pub fn pickup_usd(agent: Agent, start: f64, p: Price, write: f64) -> f64 {
    // Tokens taken in, requests and output of a pickup, per agent.
    let (taken, requests, output) = match agent {
        Agent::Claude => (8_000.0, 4.0, 2_000.0),
        Agent::Codex => (30_000.0, 6.0, 4_000.0),
    };
    ((start + taken) * write + (requests - 1.0) * (start + taken / 2.0) * p.read + output * p.output) / 1e6
}

pub fn advise(i: &Inputs) -> Vec<String> {
    let mut out = Vec::new();
    let prompt = i.live.usage.prompt() as f64;
    let per_request = i.price.map(|p| prompt * p.read / 1e6);

    let mut by_kind: BTreeMap<BlockKind, f64> = BTreeMap::new();
    let mut by_source: BTreeMap<(BlockKind, String), f64> = BTreeMap::new();
    let mut pasted: Vec<(u32, f64)> = Vec::new();
    for &(block, t, turn) in &i.live.blocks {
        let Some(b) = block else { continue };
        let blk = &i.session.blocks[b];
        *by_kind.entry(blk.kind).or_default() += t;
        let source = match blk.kind {
            BlockKind::ToolResult | BlockKind::ToolUse => blk.tool.clone(),
            BlockKind::Reminder => blk.target.clone(),
            _ => None,
        };
        if let Some(source) = source {
            *by_source.entry((blk.kind, source)).or_default() += t;
        }
        if blk.kind == BlockKind::Prompt && t >= 10_000.0 {
            pasted.push((turn, t));
        }
    }
    let held = |k: BlockKind| by_kind.get(&k).copied().unwrap_or(0.0);
    let share = |k: BlockKind| held(k) / prompt.max(1.0);
    let sources = |k: BlockKind| {
        let mut v: Vec<(&str, f64)> = by_source.iter().filter(|((kind, _), _)| *kind == k).map(|((_, s), t)| (s.as_str(), *t)).collect();
        v.sort_by(|a, b| b.1.total_cmp(&a.1));
        v
    };
    // The numbers with up to two leading sources, then what to do.
    let item = |head: String, sources: &[(&str, f64)], hint: &str| {
        let list = sources.iter().take(2).map(|(s, t)| format!("{s} {}", tokens(*t))).collect::<Vec<_>>().join(", ");
        if list.is_empty() { format!("{head}: {hint}.") } else { format!("{head} ({list}): {hint}.") }
    };

    if prompt >= 100_000.0 {
        let setup = held(BlockKind::Tools) + held(BlockKind::System);
        // A session forked from another starts big, so the smaller of its first prompt and the
        // prompt after a compaction (the setup and a summary) stands for a fresh start.
        let start = [i.first_prompt, i.after_compaction].into_iter().flatten().min().map_or(setup, |t| t as f64);
        let write = i.price.map_or(0.0, |p| if i.cache.long_ttl { p.write_long } else { p.write_short });
        let pickup = i.price.filter(|_| start > 0.0).map(|p| pickup_usd(i.session.agent, start, p, write));
        let handoff = format!("tokprof handoff {}", i.session.short_id());
        if i.cache.cold() {
            let mut line = format!("Resuming writes the whole {} context again", tokens(prompt));
            if let Some(r) = i.cache.rebuild_usd {
                line += &format!(" (+{})", usd(r));
            }
            if let Some(per) = per_request {
                line += &format!(", then {} per request", usd(per));
            }
            line.push('.');
            out.push(line);
            if let (Some(p), Some(pickup)) = (i.price, pickup) {
                if i.cache.rebuild_usd.is_some_and(|r| pickup < r) {
                    out.push(format!(
                        "A new session that refers to this one is cheaper: ≈ {} to pick up the work. `{handoff}` prints the line to start it with.",
                        usd(pickup)
                    ));
                } else {
                    out.push(format!("A new session that refers to this one costs about as much (≈ {}).", usd(pickup)));
                }
                let after = i.after_compaction.map_or(setup + 2.0 * SUMMARY_TOKENS, |t| t as f64);
                // The compaction call shares the conversation's cache, so on a cold cache it
                // writes the whole context again before it summarizes it.
                let compact = prompt * write / 1e6 + SUMMARY_TOKENS * p.output / 1e6;
                out.push(format!("/compact costs about as much as resuming (≈ {} once), then ~{} per request.", usd(compact), tokens(after)));
            }
        } else {
            let left = human_duration((i.cache.ttl_ms - i.cache.idle_ms) as f64 / 1000.0);
            let mut line = format!("Cache warm for {left}: keep going now");
            // While warm, the compaction call reads the context at the cache rate.
            if let Some(p) = i.price {
                line += &format!(", or /compact before a break (≈ {} now)", usd(prompt * p.read / 1e6 + SUMMARY_TOKENS * p.output / 1e6));
            }
            line.push('.');
            out.push(line);
            if let Some(r) = i.cache.rebuild_usd {
                out.push(format!("Once it is cold, resuming writes the whole {} context again (+{}).", tokens(prompt), usd(r)));
                if let Some(pickup) = pickup.filter(|u| *u < r) {
                    out.push(format!("Then a new session that refers to this one is cheaper: ≈ {} to pick up the work (`{handoff}`).", usd(pickup)));
                }
            }
        }
    }

    let full = i.window.map(|w| prompt / w as f64);
    if let Some(per) = per_request.filter(|_| prompt >= 200_000.0 || full.is_some_and(|f| f >= 0.7)) {
        let mut parts: Vec<String> = full.map(|f| format!("The context fills {:.0}% of the window.", 100.0 * f)).into_iter().collect();
        if i.live.turns > 0 {
            parts.push(format!(
                "At {:.0} requests per prompt, each prompt costs ≈ {} in reads alone.",
                i.requests_per_prompt,
                usd(per * i.requests_per_prompt)
            ));
        } else {
            parts.push(format!("Each request re-reads {} tokens ({}).", tokens(prompt), usd(per)));
        }
        if full.is_some_and(|f| f >= 0.85) {
            parts.push("Auto-compaction is close.".into());
        }
        out.push(parts.join(" "));
    }

    if share(BlockKind::ToolResult) >= 0.3 {
        let v = sources(BlockKind::ToolResult);
        let hint = match v.first().map_or("", |s| s.0) {
            "Bash" | "BashOutput" | "exec_command" | "shell" => "pipe long command output through tail or grep",
            "Read" => "read only the ranges you need",
            "WebFetch" | "WebSearch" => "ask for a summary instead of the page",
            "Grep" | "Glob" => "narrow the pattern or the path",
            _ => "use narrower calls, or a subagent for exploration",
        };
        out.push(item(format!("Tool results take {:.0}% of the context", 100.0 * share(BlockKind::ToolResult)), &v, hint));
    }
    if share(BlockKind::ToolUse) >= 0.2 {
        let v = sources(BlockKind::ToolUse);
        let hint = match v.first().map_or("", |s| s.0) {
            "Bash" | "exec_command" | "shell" => "write long inline scripts to a file and run the file",
            "Write" => "prefer small edits to whole-file writes",
            "Edit" | "MultiEdit" => "smaller edits are cheaper",
            _ => "keep tool inputs short",
        };
        out.push(item(format!("Tool calls take {:.0}% of the context", 100.0 * share(BlockKind::ToolUse)), &v, hint));
    }
    if held(BlockKind::Reminder) >= 30_000.0 || share(BlockKind::Reminder) >= 0.05 {
        let v = sources(BlockKind::Reminder);
        let hint = match v.first().map_or("", |s| s.0) {
            "instructions" => "trim CLAUDE.md and AGENTS.md, or move rarely needed parts to docs read on demand",
            "background notifications" => "run fewer or quieter background jobs",
            "hook context" => "make the hooks print less",
            "deferred tool lists" => "use fewer MCP servers or tools",
            "skills" => "load fewer skills per session",
            _ => "trim what the task does not need",
        };
        out.push(item(format!("Reminders hold {} tokens", tokens(held(BlockKind::Reminder))), &v, hint));
    }
    if !pasted.is_empty() {
        // The largest blocks cost the most to re-read; they are listed in turn order.
        let mut largest = pasted.clone();
        largest.sort_by(|a, b| b.1.total_cmp(&a.1));
        largest.truncate(3);
        largest.sort_by_key(|p| p.0);
        let mut list = largest.iter().map(|(turn, t)| format!("turn {turn} {}", tokens(*t))).collect::<Vec<_>>().join(", ");
        if pasted.len() > largest.len() {
            list += &format!(" and {} more", pasted.len() - largest.len());
        }
        out.push(format!("Pasted prompts ({list}) are re-read on every request: pass a file path instead."));
    }

    if i.spend_usd > 0.0 && i.thinking_usd / i.spend_usd >= 0.1 {
        let effort = i.effort.as_ref().map_or(String::new(), |e| format!(", at effort {e}"));
        out.push(format!(
            "Thinking cost {}, {:.0}% of the session{effort}: use a lower effort for routine steps.",
            usd(i.thinking_usd),
            100.0 * i.thinking_usd / i.spend_usd
        ));
    }

    let missed: f64 = i.misses.iter().map(|(_, _, u)| u).sum();
    if let Some((cause, n, _)) = i.misses.iter().max_by(|a, b| a.2.total_cmp(&b.2))
        && (missed >= 5.0 || (i.spend_usd > 0.0 && missed / i.spend_usd >= 0.05)) {
            out.push(format!("Cache misses could have saved {}, mostly {} ({n}×): {}.", usd(missed), cause.name(), miss_hint(*cause)));
        }
    out
}

fn miss_hint(cause: Cause) -> &'static str {
    match cause {
        Cause::IdleTtl => "compact or wrap up a big session before a long break",
        Cause::ModelSwitch | Cause::ParamsChanged => "switch model or effort in a new session, not mid-task",
        Cause::ToolsChanged | Cause::SystemChanged | Cause::AgentUpdated => {
            "avoid toggling MCP servers or plugins, or updating the agent, mid-task"
        }
        Cause::HistoryEdited | Cause::ToolResultsCleared | Cause::ImagesRemoved | Cause::ThinkingDropped => {
            "earlier messages were rewritten, and everything after them was written again"
        }
        Cause::Lookback => "one step added more than 20 content blocks, past the cache lookback window",
        Cause::Compaction => "compaction rebuilds the cache by design",
        Cause::AfterAssistantMessage | Cause::Unknown => "the logs do not show the cause",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Agent, Block, Usage};

    fn cold_cache(rebuild: Option<f64>) -> CacheState {
        CacheState {
            long_ttl: true,
            ttl_ms: 3_600_000,
            ttl_name: "1h",
            idle_ms: 10 * 3_600_000,
            rebuild_usd: rebuild,
            next_warm_usd: Some(0.3),
            next_cold_usd: Some(7.7),
        }
    }

    fn warm_cache(rebuild: Option<f64>) -> CacheState {
        CacheState { idle_ms: 10 * 60_000, ..cold_cache(rebuild) }
    }

    /// A session whose latest prompt holds these blocks: kind, source (tool or reminder
    /// target), tokens and turn.
    fn session(blocks: &[(BlockKind, Option<&str>, f64, u32)], turns: u32) -> (Session, LiveContext) {
        let mut s = Session::new(Agent::Claude, "s".into());
        s.blocks = blocks
            .iter()
            .map(|&(kind, source, _, _)| Block { kind, chars: 1, tool: source.map(str::to_string), target: source.map(str::to_string) })
            .collect();
        let live = LiveContext {
            chain: "main".into(),
            ts_ms: 0,
            model: "claude-opus-5-5".into(),
            usage: Usage { cache_read: blocks.iter().map(|b| b.2).sum::<f64>() as u64, ..Usage::default() },
            context_window: Some(1_000_000),
            blocks: blocks.iter().enumerate().map(|(n, b)| (Some(n), b.2, b.3)).collect(),
            turns,
        };
        (s, live)
    }

    /// The advice for a session, checked to fit two lines of the X-ray per item.
    fn advice(s: &Session, live: &LiveContext, cache: &CacheState, edit: impl FnOnce(&mut Inputs<'_>)) -> Vec<String> {
        let mut i = Inputs {
            session: s,
            live,
            price: crate::pricing::Pricing::defaults().get("claude-opus-5-5", 0, crate::pricing::now_ms()),
            cache,
            window: live.context_window,
            requests_per_prompt: 30.0,
            spend_usd: 100.0,
            thinking_usd: 0.0,
            effort: None,
            misses: Vec::new(),
            after_compaction: None,
            first_prompt: Some(40_000),
        };
        edit(&mut i);
        let lines = advise(&i);
        for line in &lines {
            assert!(crate::xray::wrap(line, WIDTH).len() <= 2, "more than two lines: {line}");
        }
        lines
    }

    /// 950.0k tokens: tool schemas, Bash output and a pasted prompt.
    fn full() -> (Session, LiveContext) {
        session(
            &[(BlockKind::Tools, None, 30_000.0, 0), (BlockKind::ToolResult, Some("Bash"), 800_000.0, 3), (BlockKind::Prompt, None, 120_000.0, 2)],
            10,
        )
    }

    #[test]
    fn a_full_cold_session_gets_one_thought_per_item() {
        let (s, live) = full();
        let lines = advice(&s, &live, &cold_cache(Some(7.4)), |i| {
            i.thinking_usd = 20.0;
            i.effort = Some("max".into());
            i.misses = vec![(Cause::IdleTtl, 3, 12.0)];
        });
        assert_eq!(
            lines,
            [
                "Resuming writes the whole 950.0k context again (+$7.40), then $0.19 per request.",
                "A new session that refers to this one is cheaper: ≈ $0.45 to pick up the work. `tokprof handoff s` prints the line to start it with.",
                "/compact costs about as much as resuming (≈ $7.80 once), then ~50.0k per request.",
                "The context fills 95% of the window. At 30 requests per prompt, each prompt costs ≈ $5.70 in reads alone. Auto-compaction is close.",
                "Tool results take 84% of the context (Bash 800.0k): pipe long command output through tail or grep.",
                "Pasted prompts (turn 2 120.0k) are re-read on every request: pass a file path instead.",
                "Thinking cost $20.00, 20% of the session, at effort max: use a lower effort for routine steps.",
                "Cache misses could have saved $12.00, mostly idle > TTL (3×): compact or wrap up a big session before a long break.",
            ]
        );
    }

    #[test]
    fn how_to_carry_on_on_a_warm_cache() {
        let (s, live) = full();
        let lines = advice(&s, &live, &warm_cache(Some(7.4)), |_| {});
        assert_eq!(
            lines[..3],
            [
                "Cache warm for 50m: keep going now, or /compact before a break (≈ $0.39 now).",
                "Once it is cold, resuming writes the whole 950.0k context again (+$7.40).",
                "Then a new session that refers to this one is cheaper: ≈ $0.45 to pick up the work (`tokprof handoff s`).",
            ]
        );
    }

    #[test]
    fn a_new_session_that_is_not_cheaper_gives_no_handoff() {
        let (s, live) = full();
        let cold = advice(&s, &live, &cold_cache(Some(0.2)), |_| {});
        assert_eq!(cold[1], "A new session that refers to this one costs about as much (≈ $0.45).");
        assert!(cold[2].starts_with("/compact costs about as much as resuming"), "{cold:#?}");
        let warm = advice(&s, &live, &warm_cache(Some(0.2)), |_| {});
        assert_eq!(warm[1], "Once it is cold, resuming writes the whole 950.0k context again (+$0.20).");
        assert!(warm[2].starts_with("The context fills 95% of the window."), "{warm:#?}");
    }

    #[test]
    fn without_a_price_only_the_tokens_remain() {
        let (s, live) = full();
        let results = "Tool results take 84% of the context (Bash 800.0k): pipe long command output through tail or grep.";
        let cold = advice(&s, &live, &cold_cache(None), |i| i.price = None);
        assert_eq!(cold[..2], ["Resuming writes the whole 950.0k context again.", results]);
        let warm = advice(&s, &live, &warm_cache(None), |i| i.price = None);
        assert_eq!(warm[..2], ["Cache warm for 50m: keep going now.", results]);
    }

    #[test]
    fn a_forked_session_starts_from_its_prompt_after_compaction() {
        let (s, live) = full();
        let lines = advice(&s, &live, &cold_cache(Some(7.4)), |i| {
            i.after_compaction = Some(21_800);
            i.first_prompt = Some(171_000);
        });
        assert!(lines[1].starts_with("A new session that refers to this one is cheaper: ≈ $0.29 to pick up the work."), "{lines:#?}");
        assert_eq!(lines[2], "/compact costs about as much as resuming (≈ $7.80 once), then ~21.8k per request.");
    }

    #[test]
    fn the_wrap_keeps_an_estimate_with_its_amount() {
        let line = "The context fills 27% of the window. At 4 requests per prompt, each prompt costs ≈ $0.22 in reads alone.";
        assert_eq!(
            crate::xray::wrap(line, WIDTH),
            ["The context fills 27% of the window. At 4 requests per prompt, each prompt costs", "≈ $0.22 in reads alone."]
        );
    }

    #[test]
    fn context_cost_without_turns_or_window() {
        let (s, mut live) = full();
        live.turns = 0;
        let lines = advice(&s, &live, &cold_cache(Some(7.4)), |_| {});
        assert_eq!(lines[3], "The context fills 95% of the window. Each request re-reads 950.0k tokens ($0.19). Auto-compaction is close.");
        live.turns = 10;
        let lines = advice(&s, &live, &cold_cache(Some(7.4)), |i| i.window = None);
        assert_eq!(lines[3], "At 30 requests per prompt, each prompt costs ≈ $5.70 in reads alone.");
    }

    #[test]
    fn content_items_name_up_to_two_sources() {
        let (s, live) = session(
            &[
                (BlockKind::System, None, 43_000.0, 0),
                (BlockKind::ToolResult, Some("Bash"), 30_000.0, 1),
                (BlockKind::ToolResult, Some("Read"), 6_000.0, 1),
                (BlockKind::ToolResult, Some("ListAgents"), 1_000.0, 1),
            ],
            1,
        );
        assert_eq!(
            advice(&s, &live, &cold_cache(None), |_| {}),
            ["Tool results take 46% of the context (Bash 30.0k, Read 6.0k): pipe long command output through tail or grep."]
        );

        let (s, live) = session(
            &[
                (BlockKind::System, None, 20_000.0, 0),
                (BlockKind::Reminder, Some("background notifications"), 40_000.0, 1),
                (BlockKind::Reminder, Some("hook context"), 20_000.0, 1),
            ],
            1,
        );
        assert_eq!(
            advice(&s, &live, &cold_cache(None), |_| {}),
            ["Reminders hold 60.0k tokens (background notifications 40.0k, hook context 20.0k): run fewer or quieter background jobs."]
        );
    }

    #[test]
    fn each_leading_source_gets_its_hint() {
        use BlockKind::{Reminder, ToolResult, ToolUse};
        let bash_output = "pipe long command output through tail or grep";
        let scripts = "write long inline scripts to a file and run the file";
        let hints = [
            (ToolResult, "Bash", bash_output),
            (ToolResult, "BashOutput", bash_output),
            (ToolResult, "exec_command", bash_output),
            (ToolResult, "shell", bash_output),
            (ToolResult, "Read", "read only the ranges you need"),
            (ToolResult, "WebFetch", "ask for a summary instead of the page"),
            (ToolResult, "WebSearch", "ask for a summary instead of the page"),
            (ToolResult, "Grep", "narrow the pattern or the path"),
            (ToolResult, "Glob", "narrow the pattern or the path"),
            (ToolResult, "ListAgents", "use narrower calls, or a subagent for exploration"),
            (ToolUse, "Bash", scripts),
            (ToolUse, "exec_command", scripts),
            (ToolUse, "shell", scripts),
            (ToolUse, "Write", "prefer small edits to whole-file writes"),
            (ToolUse, "Edit", "smaller edits are cheaper"),
            (ToolUse, "MultiEdit", "smaller edits are cheaper"),
            (ToolUse, "Agent", "keep tool inputs short"),
            (Reminder, "instructions", "trim CLAUDE.md and AGENTS.md, or move rarely needed parts to docs read on demand"),
            (Reminder, "background notifications", "run fewer or quieter background jobs"),
            (Reminder, "hook context", "make the hooks print less"),
            (Reminder, "deferred tool lists", "use fewer MCP servers or tools"),
            (Reminder, "skills", "load fewer skills per session"),
            (Reminder, "todo list", "trim what the task does not need"),
        ];
        for (kind, source, hint) in hints {
            let (s, live) = session(&[(BlockKind::System, None, 10_000.0, 0), (kind, Some(source), 40_000.0, 1)], 1);
            let lines = advice(&s, &live, &cold_cache(None), |_| {});
            assert_eq!(lines.len(), 1, "{lines:#?}");
            assert!(lines[0].ends_with(&format!("({source} 40.0k): {hint}.")), "{kind:?} {source}: {}", lines[0]);
        }
    }

    #[test]
    fn pasted_prompts_list_the_three_largest_in_turn_order() {
        let block = |t, turn| (BlockKind::Prompt, None, t, turn);
        let (s, live) = session(&[block(14_600.0, 1), block(29_600.0, 3), block(10_500.0, 5), block(11_000.0, 8), block(110_200.0, 12)], 12);
        let lines = advice(&s, &live, &cold_cache(None), |i| i.price = None);
        let expected = "Pasted prompts (turn 1 14.6k, turn 3 29.6k, turn 12 110.2k and 2 more) are re-read on every request: pass a file path instead.";
        assert!(lines.iter().any(|l| l == expected), "{lines:#?}");
    }

    #[test]
    fn thinking_without_an_effort_level() {
        let (s, live) = session(&[(BlockKind::System, None, 10_000.0, 0)], 1);
        let lines = advice(&s, &live, &cold_cache(None), |i| i.thinking_usd = 20.0);
        assert_eq!(lines, ["Thinking cost $20.00, 20% of the session: use a lower effort for routine steps."]);
    }

    #[test]
    fn each_miss_cause_gets_its_hint() {
        let switch = "switch model or effort in a new session, not mid-task";
        let toggles = "avoid toggling MCP servers or plugins, or updating the agent, mid-task";
        let rewritten = "earlier messages were rewritten, and everything after them was written again";
        let unknown = "the logs do not show the cause";
        let hints = [
            (Cause::IdleTtl, "compact or wrap up a big session before a long break"),
            (Cause::ModelSwitch, switch),
            (Cause::ParamsChanged, switch),
            (Cause::ToolsChanged, toggles),
            (Cause::SystemChanged, toggles),
            (Cause::AgentUpdated, toggles),
            (Cause::HistoryEdited, rewritten),
            (Cause::ImagesRemoved, rewritten),
            (Cause::ThinkingDropped, rewritten),
            (Cause::Lookback, "one step added more than 20 content blocks, past the cache lookback window"),
            (Cause::AfterAssistantMessage, unknown),
            (Cause::Unknown, unknown),
        ];
        let (s, live) = session(&[(BlockKind::System, None, 10_000.0, 0)], 1);
        for (cause, hint) in hints {
            let lines = advice(&s, &live, &cold_cache(None), |i| i.misses = vec![(cause, 13, 112.0)]);
            assert_eq!(lines, [format!("Cache misses could have saved $112, mostly {} (13×): {hint}.", cause.name())]);
        }
        let small = advice(&s, &live, &cold_cache(None), |i| {
            i.spend_usd = 107.0;
            i.misses = vec![(Cause::IdleTtl, 2, 3.09)];
        });
        assert!(small.is_empty(), "{small:#?}");
    }

    /// The estimate against the pickups the reference bench measured: Claude Code at
    /// $0.44–0.51 from 42–44k tokens, Codex with gpt-6-astra at $0.70–1.26 from 18k.
    #[test]
    fn pickup_estimates_match_the_reference_bench() {
        let pricing = crate::pricing::Pricing::defaults();
        let now = crate::pricing::now_ms();
        let opus = pricing.get("claude-opus-5-5", 0, now).unwrap();
        let astra = pricing.get("gpt-6-astra", 0, now).unwrap();
        let claude = pickup_usd(Agent::Claude, 42_000.0, opus, opus.write_long);
        let codex = pickup_usd(Agent::Codex, 18_000.0, astra, astra.write_short);
        assert!((0.44..=0.51).contains(&claude), "{claude}");
        assert!((0.70..=1.26).contains(&codex), "{codex}");
    }
}
