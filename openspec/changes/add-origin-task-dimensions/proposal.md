# Proposal

## Why

tokprof shows spend by model, component, chain and content, but not by the kind of work. It cannot answer "how much went to code review, to subagents, to typed prompts?", the question AgentSight's agentpprof is built around. Two things stand in the way:
- the `turn` dimension is only a number;
- 45% of the author's Claude Code spend in the last 30 days sits in chains named `subagent:general-purpose`.

The logs already say what each request was part of, with no rules to write. Measured on the author's last 30 days of Claude Code logs ($9.1k):

| Request | Share of spend |
|---|---:|
| attributed to a skill | 27% |
| subagent work with a description of its own | 27% |
| a turn that follows a background agent's result | 8% |
| a slash command turn | 2% |
| a session title and no other label | 30% |
| none of these | 1% |

agentpprof's default `task` frame, the first 120 characters of the prompt, spread $4.8k over 1,724 frames, and its top 20 covered 17%.

## What Changes

- Every request gets an **origin**, the kind of work it was part of, taken from the logs:
  - `skill:<name>`, `mcp:<server>`, `plugin:<name>`;
  - `agent:<type>` for subagents;
  - `command:/<name>` for slash commands;
  - the starter of a background task for its follow-up turn;
  - `notification` for other background tasks;
  - `prompt` for typed prompts.
- Every request gets a **task**, what the work was:
  - what a subagent was asked to do;
  - else the session title;
  - else `untitled <id>`.
- Sessions get a title:
  - Claude Code: `custom-title` over `ai-title`;
  - Codex: `thread_name` in `session_index.jsonl`.
- Subagent chains are named after their task, `subagent: <task>`, instead of `subagent:<type>`. Misses, flame graphs and the X-ray then show what each subagent did. **BREAKING** for anyone who matches chain names in folded stacks.
- `origin` and `task` are new stack dimensions in `tokprof flame --stack`; the default stack does not change.
- `tokprof summary` adds two tables:
  - `Spend by origin`;
  - `Most expensive tasks`.
- The X-ray adds:
  - a `title` line to the header;
  - a `work` block after `spend`, with origins as bars and the top tasks.

Out of scope:
- User-defined categories, such as regex rules that map prompts to `review` or `debug`.
- Model-based tagging.
- Grouping prompts by their text.
- Wall-clock time by origin.
- Linking a subagent to the turn of its parent that started it, beyond what the follow-up turn needs.
- Junie and OpenCode, which tokprof does not read yet.

## How it is checked on the real logs

- `tokprof --agent claude --since <30 days> summary` must match the measurement script within a few percent:
  - skills about 27%;
  - code-review about $1.7k;
  - `/goal` about $120 and `/model` about $65;
  - subagent types, the linked follow-up turns and unlinked notifications together about 37%.
- The spend-by-model table must stay identical to the version before the change: same totals per model.
- The X-ray of a code-review session must show the skill and the reviewers' own task names. For session 03aee587: skill:code-review $508 of $526; top tasks `Angle C cross-file tracer`, `Angle A line-by-line scan`.
- Parse time and peak memory of `tokprof summary` over all logs must stay within noise of the previous binary.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `session-logs`:
  - the log fields read for origin, task and title, per agent;
  - subagent chains named after their task.
- `cost-attribution`:
  - every share of spend carries the origin and task of its request;
  - the rules for both;
  - session titles;
  - the summary report gains the two tables.
- `profile-export`: `--stack` accepts `origin` and `task`.
- `session-xray`: the header gains the title line, and the layout gains the `work` block.
- `cache-misses`: misses of a subagent are listed with its chain named after its task.

## Impact

Code:
- the Claude Code and Codex adapters read new log fields: attribution fields, slash command and notification markers, subagent meta files, title lines, the Codex session index;
- the analysis splits spend by request origin;
- the reports and the X-ray;
- README.

The X-ray pane of `tokprof ui` shows the new block with no change of its own.

There are no new dependencies. The change adds one more small file read for each Claude subagent transcript (its meta file) and one read of the Codex session index.
