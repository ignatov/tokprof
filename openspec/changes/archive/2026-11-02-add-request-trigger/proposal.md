# Proposal

## Why

Every request re-reads the whole context. So the cost of a tool call is mostly the next request, not its own output. tokprof charges spend to what fills the context, and it cannot tell why the agent went round again.

Measured on the author's Claude Code logs over 30 days ($9.1k):
- A response that makes exactly one read-only call, such as a `Read`, a `git grep` or a `sed -n`, was followed by 31.8k requests. They cost $3.4k, 37% of the spend.
- In a main conversation over 300k tokens, the input of the request after such a lookup costs about $0.15.
- 2,655 runs of three or more such lookups came back to back.

On the author's Codex logs, requests that follow tool calls hold $1.78k of the $2.0k input spend.

The X-ray tells the user to make tool output shorter. On these logs, the number of trips matters as much as output size: the requests after single lookups cost about as much as all tool results together ($3.4k), and Bash results under 1k tokens hold 36% of the Bash result cost.

## What Changes

- Every request gets a **trigger**: what made the agent send it. The values:
  - `prompt`;
  - `lookup` and `lookups`;
  - `edit`, `run` and `subagent`;
  - `notification`, `compaction` and `continuation`;
  - `advisor` and `unknown`.
- Like origin and task, the trigger covers whole requests, so every share of a request carries it. It is one more dimension for `--stack`, as in `tokprof flame --stack trigger,origin,tool,component`.
- A call is read-only when its tool only reads, searches or fetches. A shell command is read-only when each of its parts starts with a listed read-only command and nothing writes to a file. The list is part of the spec.
- `tokprof summary` prints a `Spend by trigger` table after the spend by origin and task: requests, average prompt, cost, cost per request and share.
- The X-ray advice gets a **lookups** item after the tool-call hints. It fires when the session's single lookups, subagents included, cost at least $5 or 10% of the session, over at least 20 requests:
  > Single lookups made 412 requests, each re-reading ≈ 380.2k tokens: $58.10, 21% of the session. Batch independent reads, or explore in a subagent.
- README gets the trigger in "How cost is attributed", the new table in "Reports over all sessions" and the dimension in "Flame graphs and profiles".

This change builds on `split-core-and-agent-adapters`. It uses that change's neutral tool classes, so a new agent adapter gets triggers by mapping its tools.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `cost-attribution`: a new trigger dimension of spend, the read-only rule, and a `Spend by trigger` table that changes the order of the summary report.
- `session-xray`: a new lookups advice item, which changes the order of the advice block.

## How it is checked on the real logs

- Claude Code, last 30 days: `lookup` holds about 31.8k requests and about $3.4k, within 15% of the measurement above. The spec's read-only rule is stricter than the measurement script, which looked only at the first command.
- The `Spend by trigger` table sums to `Total priced`, to the cent.
- The reconciliation with Claude Code's own cost counter stays within 1.5% for the same sessions.
- Codex, last 30 days: requests after tool calls hold about 90% of the spend.
- The parse time of `tokprof summary` stays within 2% of the baseline, as a median of 3 runs.
- The X-ray of a session with many lookups in a big context shows the item with numbers that match `tokprof --session <id> summary`.

## Out of scope

- Batching or rewriting tool calls for the agent.
- Advice by command, such as "use `rg -C` instead of `sed -n` chunks".
- Telling dependent lookups from independent ones. The advice says "independent", and nothing is promised as a saving.
- Thinking by kind of step, a separate change that reuses the same classes.
- A trigger column in the X-ray's work block.

## Impact

- `tokprof-core`: the request model gets a trigger, the read-only command rule, a `trigger` dimension, the summary table and the advice rule.
- `sources/claude-code` and `sources/codex`: each decoder sets the trigger from what precedes a request in its log.
- `tokprof` CLI: the dimension list in `--help`.
- README.md.
