# Design

## Context

The five fixes are independent. Only the Codex one touches more than one place: the thread index, the rollout parser, and the commands that map a log to its session (`--session`, the X-ray's list of subagent logs, `tokprof ui`'s live updates).

The Codex fields involved are `session_meta.payload.id` and `session_meta.payload.parent_thread_id`, seen in Codex 0.146–0.163, and `thread_name` in `session_index.jsonl`. No new field is read.

## Codex threads: resolve the root through the thread index

tokprof already keeps an index of every rollout's thread id and parent, read from first lines in parallel. Each run that touches Codex reads it before parsing: `sessions` and the reports need it to tell subagent rollouts from main ones. The fix walks that index:
- **The root of a thread** follows `parent_thread_id` until a thread has no parent, or its parent's rollout is not found. A cap of 64 steps guards against a cycle in damaged logs.
- **The session of a rollout** is the rollout of its root. When the root itself is not on disk, there is none, as today for a missing parent.
- **The subagent logs of a session** are all threads below it, breadth first, not only its direct children.
- **The parser** gives a subagent rollout the root's id and the root's title.

Alternatives considered:
1. Give a child its direct parent's id in the parser, and leave grouping to the reports. It would still split nested threads between two ids, and the X-ray would still miss them.
2. Rewrite ids after parsing, in each report. Every report would need the same fix-up, and the flame export names threads before any report sees them.

Resolving in the parser keeps one rule for every command. It is also how Claude Code subagents already work, since their lines carry the parent's `sessionId`.

Measured on the author's logs: 315 of 793 rollouts are subagent threads, 25 at depth 2 and 2 at depth 3. Every root is on disk.

## The share on the `next` line

The share is left out when the session's cost would print as `$0.00`, that is under $0.005. This uses the same threshold as the dollar format, so the line can never show a share of a zero amount. A share over 100% stays when the cost shows a value, as in `= 150% of the session's $0.02`. It is true, and it tells that the session is young.

## The pickup's prices

The advice gets a price lookup for the session's model at today's date and any prompt size, instead of one price for the latest prompt. Resuming, `/compact` and the context cost ask it for the latest prompt, as now. The pickup asks it for the new session's largest prompt: the start plus the tokens it takes in (8,000 for Claude Code, 30,000 for Codex). No session in the author's logs is over a tier threshold today, so no X-ray changes there. A unit test covers the tiered case.

## Requests per prompt

The ratio divided all of the conversation's requests by the turns of the latest prompt, which a compaction rebuilds. Codex re-sends earlier user messages in `compacted.replacement_history`, as new items, so the latest prompt's turns are neither the prompts since the compaction nor all of them.

The fix counts prompts over the conversation's whole life, where its prompt is replayed anyway:
- each request adds the turns its prompt holds beyond the previous request's;
- the first request after a compaction adds none, and its turns are the new baseline;
- the requests counted are those whose prompt the log shows, so a transcript part from before Claude Code 2.1.300 counts on neither side.

Alternatives considered:
1. Only the requests since the last compaction, over the latest prompt's turns. Measured on the author's logs, it turned 182 into 1 for 01a0eec6-eed9-7a71: the turns Codex re-sent are counted against a few requests.
2. Telling re-sent prompts from typed ones by their text. Prompts typed twice, such as "continue", would count once.

The rule misses a prompt typed in the same request that follows a compaction, at most one per compaction. Codex compacts mid-turn, while the agent works, so the next request rarely carries a new prompt.

## A session without timed requests

The header's `time` line drops the two times and the span when no request has a time, and keeps the counts. The `models` line is left out when there are no models. Only sessions without requests reach this case in the author's logs: 35 Codex rollouts.

## Parse time

The root walk is a few hash lookups per subagent rollout. The descendant walk visits each thread once. Neither reads a file, so the parse time of the full corpus does not change.

## Risks / Trade-offs

- [A thread's parent is written after the thread, or moves to `archived_sessions/` later] → The index already reads rollouts that appear later, and keeps the latest path of a moved thread. A child read before its parent's rollout is known gets the topmost id known then. `tokprof ui` reads it again when its log changes.
- [Merging subagent threads into their session changes per-session numbers in `misses` and `flame`] → The totals do not change. The check compares `summary`'s priced total before and after.
- [Requests per prompt after a compaction rest on few requests] → The number is shown as a whole number and only when the prompt is big, so a short stretch gives a small, honest ratio rather than an inflated one.
