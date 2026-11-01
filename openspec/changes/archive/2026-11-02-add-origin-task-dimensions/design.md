# Design

## Context

See proposal.md for the motivation and specs/ for the rules.

Today the attribution walk adds up each chain's cost per prompt block over all of its requests, and then emits one share of spend per block, component and model. A share therefore has no request-level fields. Origin and task belong to a request: a skill can start in the middle of a main conversation.

Each Claude Code subagent transcript and each Codex rollout is parsed as a session of its own. Session-level facts reach every request of a subagent with no lookup, such as its task.

The log fields this design reads, and the agent versions they were seen in:
- Claude Code 2.1.284–2.1.312:
  - `attributionSkill`, `attributionMcpServer` and `attributionPlugin` on assistant lines;
  - `<command-name>` and `<task-notification>`/`<task-id>` in user messages;
  - `ai-title` (`aiTitle`) and `custom-title` (`customTitle`) lines;
  - `subagents/agent-<id>.meta.json` with `agentType`, `description` and `toolUseId`.
- Codex 0.146–0.163:
  - `session_meta.source.subagent` with `thread_spawn.agent_path` and `agent_role`, `{"other": "guardian"}` or `"review"`;
  - `session_index.jsonl` with `id` and `thread_name`.

## Goals / Non-Goals

**Goals:**
- Origin and task come from the logs alone and are deterministic, with no rules, models or prompt text.
- Cost attribution and its totals do not change; only new splits are added.
- The full corpus parses about as fast as before, in about the same memory.

**Non-Goals:**
- Labels derived from prompt text, such as regex categories, clustering or model tagging.
- Redacting titles in shared profiles. Titles can hold personal names; that is a separate change.

## Decisions

### Origin is per request, not per block

A request's whole cost goes to its origin. The rejected alternative was to label each block by the turn that added it, the way `turn` works. That is cheaper, but it would charge a review's re-reads of the existing context to the turns that added the context. "What did code review cost" would then miss most of the review's cost.

### The walk splits its sums by origin, and the task stays per session

Before, the per-block sums were keyed by chain label and model; now they are keyed by chain label, model and origin. A chain has few origins: a subagent has one, and a main conversation has typed prompts plus the few skills and commands it used. So the extra memory is a few more rows per chain.

The task is constant per session: a subagent's description, or else the title. It needs no split, and every share takes it from its session.

Two alternatives were rejected:
- Flushing the sums whenever the origin changes. It emits one share per block per run of requests, which is far too many.
- A sparse map per block. It is slower on the hot path.

### Summary and X-ray tables sum whole requests

The `Spend by origin` and `Most expensive tasks` tables add up each request's cost at its own price. They do not add up the per-block shares, of which there are about a thousand times more. Attribution keeps each request's total, so the two sums agree; a unit test checks this. Flame graphs keep using the per-block shares, so `--stack origin,task,kind,…` works.

### The subagent task lives on the session, not on the chain

Each subagent log is its own session, so its description is a session field, along with the title. The chain structure stays as it is: other code builds chains directly. The subagent's chains are renamed to `subagent: <task>` when the session is built.

### A background task's follow-up goes to whoever started the task

While the parent transcript is read, every `Agent`/`Task` tool call in a response with an attribution is remembered with that attribution. A `<task-notification>`'s `<task-id>` names the subagent's meta file, whose `toolUseId` finds the starting call.
- If that response had an attribution, the follow-up turn takes it.
- Otherwise the follow-up turn takes the meta file's `agentType`.
- Shell-task ids have no meta file and give `notification`.

The meta file is read at most once per task id, and the id must be alphanumeric before it becomes part of a path.

### Codex titles come from one index, read again when it changes size

`session_index.jsonl` is about 35 KB. It is read on first use and cached. A size check on every parse picks up new threads for the live session browser. The index has no entry for threads from IDE plugins and `codex exec`, so those stay untitled. Taking a title from the prompt instead would be guessing.

### Untitled sessions keep their id

The task of a session without a title is `untitled <8-char id>`, not one shared `untitled` bucket. On the author's logs such a bucket was half of all spend, and the id points to the session to open with `tokprof scan`.

### Parse time

Measured on the author's 8.4 GB corpus (1,033 Claude and 792 Codex files), with the machine at a load average of about 11:

| `tokprof summary` | wall time | peak memory |
|---|---|---|
| previous binary | 23.5 s and 23.9 s | 250 MB |
| this change | 23.6 s and 25.1 s | 244 MB |

The difference is within noise. The added I/O is one small meta file per Claude subagent transcript (754 on disk) and one index read.

## Risks / Trade-offs

- [The log fields are undocumented and change between agent releases] → Each field is named with its versions in session-logs. A missing field degrades to `prompt`, the stream's type or `untitled <id>`; nothing fails.
- [Renaming subagent chains changes folded stacks, so `jfrconv --diff` against an older profile shows the subagents as removed and added] → It is noted as breaking in the proposal. Origin and task give the stable grouping.
- [A long main conversation with many distinct skills and commands multiplies the per-block rows] → No session on the author's logs came close; peak memory did not grow. Origins can be capped per chain if one ever does.
- [Titles carry personal data into shared profiles] → It is out of scope here, and a later redaction option covers it.
- [40% of Codex spend has no title, because IDE and `codex exec` threads are not indexed] → Those sessions show as `untitled <id>`. A later change can find another source for their names.

## Migration Plan

No data or configuration migrates. Reinstalling the binary is enough. To undo the change, go back to the previous binary.
