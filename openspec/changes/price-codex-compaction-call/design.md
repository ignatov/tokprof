# Design

## Context

See proposal.md, Why, for the numbers.

How tokprof works today:
- **Codex source.** Requests come from `token_usage_record` lines, deduplicated by `response_id`, which the decoder already keeps in a set. A `compacted` line replaces the history with its `replacement_history` and marks the next request `compacted_before`. Since 5824258, the request after a response whose last item was an assistant message carries `after_assistant_message`.
- **Core.** `analysis/cache.rs` checks the causes in order. `Compaction` and `ToolResultsCleared` are the expected rebuilds. `advice.rs` prices a warm `/compact` as the prompt at the read price plus `SUMMARY_TOKENS` (10k) of output.
- **Bench.** `bench.rs` prices /compact first as `shared × read + (context − shared) × write + 10k × output`, where `shared` is what the return still read from the cache, at most the base. It prices the warm compaction of the fifth way, and the compactions on breaks that were not returns, with `warm_call`: the context at the read price plus 10k of output. `crates/tokprof/src/render_bench.rs` prints the report's notes and draws the chart's notes.

What the Codex logs show:
- **The `compacted` payload** has `message` (the summary, which starts "Another language model started to solve this problem"), `replacement_history`, `compaction_response_id`, `latest_token_usage_record`, `window_number` and other fields. It is seen in Codex 0.145–0.159. `compaction_response_id` is missing in 0.152.0 (7 compactions).
- **`token_usage_record` lines** appear from 0.153 on; earlier rollouts have only `token_count` events. In 0.153.2–0.159.2, each of the 477 compactions came right after the usage line of its call, and the id matched every time.
- **Remote compaction.** All 927 compactions in the logs are local: their `message` is the summary.

Why the call misses, from Codex's source (checkout of 2026-08-26):
- **No tools.** `codex-rs/core/src/compact.rs` builds the call as `Prompt { input, base_instructions, ..Default::default() }`, so it has no tools and `parallel_tool_calls: false`. A turn passes the tool router's specs and `parallel_tool_calls: true` (`session/turn.rs`).
- **Same cache key.** `prompt_cache_key` is the session id for both kinds of request.
- **Local, not remote.** Codex compacts on the server only for the OpenAI and Azure providers (`model-provider/src/provider.rs`). The author's provider is a local proxy.

## Goals / Non-Goals

**Goals:**
- Name the compaction call only where the log names it. No guess from the call's position.
- Keep the core free of agent names: the source says what its compaction call does, and the core reads that through the agent's capabilities.

**Non-Goals:**
- No change to how the request after a compaction is judged or priced.
- No per-agent summary size.

## Decisions

### 1. Mark the call by its response id, in the Codex source

On a `compacted` line with `compaction_response_id`, the decoder marks the stream-0 request whose `response_id` matches. In every case seen, that is the latest request. A new neutral request fact, `compaction_call`, carries the mark to the core, next to `compacted_before`. The Claude Code source never sets it.

*Alternative:* mark the last request before the `compacted` line. It gives the same result on 0.153–0.159. But a rollout with only `token_count` events logs the call's usage after the line, and the rule would mark the wrong request there. The id leaves those rollouts as they are, as the proposal says.

### 2. The cause goes right after `compaction`, and is not expected

`CompactionCall` (name `compaction call`, no detail) is checked after `Compaction` and before `IdleTtl`. The call misses on a warm cache too, so it is the more certain cause of the two. It stays out of the expected set. The money is avoidable in principle, and the advice has a fix for the user: start a new session before the context fills.

### 3. Remove `after assistant message`

This removes the `AfterAssistantMessage` variant, the `after_assistant_message` request field, and the decoder's `ended_with_message` from 5824258. `ended_with_text` stays, because it feeds the `continuation` trigger.

*Alternative:* keep the cause with its fixed meaning. It names no miss on the corpus, and the spec's claim that it is a Codex signature does not hold.

### 4. A capability says whether the agent's compaction call reads the cache

A new `Capabilities` flag, `compaction_reads_cache`:
- **Claude Code: true.** Its binary runs the compaction as a fork that shares the conversation's cache. Its logs hold no call.
- **Codex: false.** Its logged calls read 0.4% from the cache.

The flag is used in three places:
- **advice.rs:** a warm `/compact` is the prompt at the read price when the flag is true, and at the write price of the conversation's cache lifetime when it is false.
- **bench.rs:** with the flag false, /compact first takes `shared = 0`. The warm compaction of the fifth way, and the compactions it prices on breaks that were not returns, write the context instead of reading it.
- **render_bench.rs:** the notes name, by the agent's title, the agents whose compaction writes the warm context.

*Alternative:* measure it: take the median cache-read share of each agent's logged compaction calls. An X-ray of one session rarely has a compaction call of its own, so a default per agent is needed anyway. If Codex changes its call, the `compaction call` misses stop on the real logs. Then the source can turn the flag by version.

### 5. Which write price a warm Codex compaction pays

It pays the write price of the conversation's cache lifetime, as the cold /compact first does. For Codex that is the short lifetime, since the source logs every Codex write as short.

### 6. The hint lives in `miss_hint`

The new text, "each call wrote the whole context again; hand off to a new session before the context fills", goes in `miss_hint`. The X-ray's cache miss advice and What to change (`savings.rs`) both show it. It names the call, not the compaction, so that the advice item keeps within the two lines of 82 characters that the X-ray's fixed texts keep to, even for $2,500 of 999 calls.

### 7. The reference run is refreshed in this change

`docs/bench.txt` was made on 2026-11-03. It cannot be made again for the same logs: with `--until 2026-11-04` the Claude Code part already has 222 returns, not 218. So the run is made on the logs of the day this change is applied. return-bench quotes it in eight requirements:
- The base the alternatives start from
- Per-agent table
- What the alternatives would have saved
- Forecast check
- The report states its assumptions
- Chart labels
- What heartbeats then compaction would have saved
- What the breaks that were not cold returns add

The two already in this change's delta get the new numbers. The tasks add MODIFIED blocks for the others when their numbers change.

*Alternative:* refresh in a follow-up change. That keeps this change small, but the README would quote, until then, Codex savings for heartbeats then compaction that this change shows to be wrong.

## Risks / Trade-offs

- [Codex starts sending its compaction call with the tools] → The flag then overprices a warm Codex compaction. → `tokprof misses` would show `compaction call` misses vanish after that version. Turn the flag per `cli_version` then. The Codex README records the versions seen.
- [Users on remote compaction] → Their calls may not be logged as usage lines, or may read the cache, and the flag may then misprice their warm compaction. → Not seen in these logs. The Codex README says the flag rests on local compaction.
- [Share files written before the change] → They hold `after assistant message`. The pool shows the row and its dollars but no hint. → Acceptable: the name only appears in old files.
- [The removed request field] → Code that builds a `Request` literal with `after_assistant_message` stops compiling. → Only the Codex decoder and core set it. `tokprof-browse` and `tokprof-desk` do not name it. The desk is still built once as a check.

## Parse time

The decoder reads one more field on each `compacted` line, about 1,000 in the corpus. It also maps each response id it already keeps to its request. The effect on the ~8 s parse is not measurable.

## Migration Plan

Nothing is stored, so no data migrates. Reverting the commits restores the old causes and prices.

## Open Questions

- Should the summary size be per agent? Codex's calls output a median of 5.7k tokens, against the 10k used for every agent. It would move a Codex compaction by about $0.20 on gpt-6-astra. It is left for a later change.
