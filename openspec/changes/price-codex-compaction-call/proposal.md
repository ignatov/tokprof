# Proposal

## Why

Codex compacts a full context with a request of its own, the compaction call. Its answer is the summary that becomes the new history. Codex sends this request without the conversation's tools, so its prompt differs from the cached one from the first token. On 30 days of the author's logs (2026-10-08 to 2026-11-07):

- 171 compaction calls read 0.4% of their prompts from the cache. The request before each, usually a minute or two earlier, had cached the same history.
- Each call wrote a median of 233k tokens again. That is about $2.70 on gpt-6-astra and $0.56 on gpt-6.1-sol, $264 in all: 14% of the Codex spend and 9 times its cold returns ($29).
- tokprof put them under `after assistant message`. After the decoder fix 5824258 they are `unknown (server side?)`, although Codex names each call in its log.

tokprof also prices a warm compaction as a cache read of the context: in the X-ray's "/compact before a break (≈ $X now)" and in the bench's heartbeats then compaction. That holds for Claude Code, whose compaction runs as a fork that shares the conversation's cache. A Codex compaction writes the whole context again, warm or cold, so tokprof prices its prompt 12.5 times too low on gpt-6-astra and 25 times too low on gpt-6.1-sol.

## What Changes

- **A new miss cause, `compaction call`.** It names a miss on the request that made a compaction, as the agent's log names that request. In Codex, that is the request whose response id the `compacted` line gives as `compaction_response_id`. It comes right after `compaction` in the order of causes. It is not an expected rebuild: a call that kept the conversation's prefix would have read it from the cache. So it shows in `tokprof misses` and `summary`, in share files and the pool, and in What to change.
- **The advice hint for it.** When it is the costliest cause, the X-ray's cache miss advice ends with "each call wrote the whole context again; hand off to a new session before the context fills".
- **`after assistant message` is removed.** With the decoder fix, it names no miss in the 30 days. The 172 misses it had named were 171 compaction calls and one other miss. A Codex miss that no cause fits is `unknown (server side?)`.
- **A warm Codex compaction is priced as a write of the whole context.**
  - X-ray: on a warm Codex cache, "/compact before a break (≈ $X now)" is the prompt at the short-lifetime write price plus the 10k-token summary, not the prompt at the read price.
  - `tokprof bench`: in /compact first, the Codex compaction reads nothing from the cache. In heartbeats then compaction, it writes the warm context instead of reading it. The report's note and the chart's note say so.
- **A fresh reference run.** `docs/bench.txt`, `docs/bench.svg` and the README's bench numbers are made again on the logs of the day of the change. The return-bench scenarios that quote that run are updated with them.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `session-logs`: the compaction call of a Codex rollout, read from `compacted.compaction_response_id`.
- `cache-misses`: the cause `compaction call` and its place in the order, and the removal of `after assistant message`.
- `session-xray`: the warm `/compact` price for an agent whose compaction call does not read the cache, and the hint for `compaction call`.
- `return-bench`: the Codex compaction in /compact first and in heartbeats then compaction, the notes of the report and the chart, and the scenarios that quote the reference run.

## How it is checked on the real logs

- **Misses.** `tokprof --agent codex --since 30d misses`, run on 2026-11-07, shows today 174 misses as `unknown (server side?)`, 38.9M tokens, $265. After the change, `compaction call` has 171 misses and $264. `unknown (server side?)` keeps 3 misses, about $1. The other Codex causes and all Claude Code causes keep their numbers, and so does the total avoidable in `tokprof summary`.
- **X-ray.** For a warm 230k-token prompt on gpt-6-astra, "/compact before a break" goes from ≈ $0.73 to ≈ $3.38. The logged compaction calls on gpt-6-astra cost a median of $3.20 each, input and output together (233k in, 5.9k out).
- **Bench, 30 days.** `tokprof --agent codex --since 30d bench`:
  - Heartbeats then compaction saves $13.85 of $65.31 (21%) today. Its 8 compactions then write about 180k tokens each instead of reading them. The saving should fall by $3–17, depending on the model, and the tasks record the new figure.
  - /compact first stays at about $45.85. The call no longer reads the part the return still read, but in these 30 days each Codex request that missed after a break of 30+ minutes, at a 100k+ context, read nothing from the cache anyway.
  - The Claude Code numbers do not change.
- **Speed.** The parse of the full corpus takes no longer than before: one more field is read on about 1,000 `compacted` lines.

## Out of scope

- Remote compaction (`/responses/compact`), which Codex uses with the OpenAI and Azure providers. None of the 927 compactions in these logs is remote.
- Rollouts of Codex before 0.153. They log usage only as `token_count` events, with no response id, so their compaction calls keep the cause they get today.
- The size of the summary. The bench and the advice keep 10k tokens, though Codex's compaction calls output a median of 5.7k.
- Changing Codex itself so that the compaction call keeps the conversation's tools. That is an upstream matter.
- Claude Code's compaction call, which its logs do not hold.
