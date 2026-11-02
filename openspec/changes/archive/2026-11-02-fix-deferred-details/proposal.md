# Proposal

## Why

Five bugs were left open after the baseline fixes. Each one makes a report wrong or confusing on the author's logs:
- **Codex subagent threads are reported under their own ids.** A Claude Code subagent counts under its parent session, but a Codex thread counts under its own thread id. In `tokprof misses --agent codex`, 179 of 1,349 misses name one of 113 subagent thread ids, which no `tokprof sessions` list shows. A thread spawned by a subagent (27 of the 315 subagent threads, up to 3 levels deep) is also left out of its session's X-ray and of `--session`.
- **The `next` line reads `= 244% of the session's $0.00`** in 125 X-rays. The session cost a fraction of a cent, which rounds to $0.00.
- **The pickup estimate is priced at the old prompt's long-context tier.** A new session starts from a small prompt, so it pays the standard prices. None of the author's sessions is over a tier threshold today (Codex keeps prompts under 258k, and the tier starts at 272k), but a model with a lower threshold or a bigger window would show a pickup several times too high.
- **Requests per prompt are wrong after a compaction.** The advice divides all of the conversation's requests by the turns in the latest prompt, which a compaction rebuilds. Codex re-sends some earlier prompts in it, so the ratio is off either way: 165 for a session that sent 25,297 requests over 115 typed prompts (220 each).
- **A session without requests shows `time 1970-01-01 01:00 → 01:00 (0s) · 0 requests`** and an empty `models` line, in 35 X-rays.

## What Changes

- A Codex subagent rollout reports its requests under the id of the session it belongs to: the root of its chain of `parent_thread_id`s. A session's subagent logs are all the threads below it, not only those it spawned itself. `--session`, the X-ray, `tokprof ui` and its live updates follow the same tree, and a nested thread takes its session's title.
- The `next` line leaves out the share when the session's cost shows as $0.00, that is when it is under half a cent.
- The pickup estimate is priced for the new session's own largest prompt: its start plus the tokens it takes in.
- Requests per prompt count the prompts the user sent over the whole conversation: the turns each request adds, where the prompt rebuilt by a compaction adds none.
- The X-ray of a session without timed requests shows `time` with the counts only, as in `time      0 requests`, and leaves out the `models` line when there are no models.

Out of scope:
- Codex subagent rollouts whose parent is not on disk stay as they are, except that their requests are reported under the parent's id.
- The other parts of the pickup model, the `next` forecast itself and the context-cost thresholds.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `session-logs`: a Codex subagent thread belongs to the root of its parent chain, and its requests are reported under that session's id.
- `cost-attribution`: a thread spawned by a subagent thread takes its session's title.
- `session-xray`: the header without timed requests, the share on the `next` line, the pickup's prices, and requests per prompt after a compaction.

## How it is checked on the real logs

X-ray every session with the old and the new binary (`tokprof scan <id>` for each of the 759 sessions), and compare:
- no X-ray reads `of the session's $0.00` (125 today) or `time      1970-` (35 today);
- the X-rays of 01a03fe1-681d-78e2 and 019ffb1b-8c5f-7752, the sessions with the most nested threads, count more subagents than today's 39 and 16, and cost more than today's $458 and $347;
- requests per prompt in the biggest Codex sessions agree with a count of the prompts typed in their rollouts;
- pickup estimates are unchanged, since no session is over a tier;
- the other lines are unchanged in every X-ray without one of these cases.

Then `tokprof misses --agent codex --top 100000` names no subagent thread id (179 rows today), and `tokprof summary --agent codex` gives the same totals as today ($10,485 priced).

## Impact

- The Codex adapter and its thread index: root lookup, all descendants, and the session id of a subagent rollout.
- The X-ray: the header, the `next` line and the inputs of the advice.
- The advice: the pickup's prices and requests per prompt.
- No new log fields. Parse time is unchanged: the thread index is already read for every Codex run.
