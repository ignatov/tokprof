# Proposal

## Why

Every request re-reads what the agent puts in front of the work: tool schemas, the system prompt, instruction files, skill listings, deferred tool lists and other reminders. Over the 30 days to 2026-11-02 this setup cost $915 of Claude Code's $9,174 (10%), and subagents pay most of it. In one code review (session 03aee587), each of 96 subagents started with 31.2k tokens of setup and re-read it about 59 times. The X-ray shows only the latest prompt of the main conversation, so this money is not visible, and no item says which setting would shrink it. Now that reminders joined in one message are split by source, skill listings show up as $197 of the 30 days. They counted as $5 before.

## What Changes

- A new `setup` block in the X-ray, after `tools`, when the setup is at least $1 and 5% of the session's spend:
  - what the setup cost over the session, subagents included, by source;
  - what the main conversation starts with, by source;
  - what the session's subagents start with on average, by source, and their mean brief;
  - what the prompt held after the main conversation's latest compaction, by content kind and reminder source;
  - one hint for the costliest source, in the agent's own terms. For Claude Code skills: lower `skillListingBudgetFraction` (default 0.01 of the context window), or turn unused skills off with `skillOverrides` in settings.json.
- `tokprof summary` prints `Setup: what every request carries before the work`, after the spend by activity: one row per agent and source with the cost, the part subagents paid, the share of the agent's spend, and the hint.
- Each log source gives the hints for its agent. The Claude Code source names its settings, and the Codex source names `AGENTS.md`.

How it will be checked on the real logs:
- `tokprof summary --since 30d` (to 2026-11-02, Claude Code): instructions about $249, tool schemas $198, skills $197, other reminders $106, deferred tool lists $76, hook context $53, system prompt $36; $915 in total, 10% of the agent's spend.
- `tokprof scan 03aee587`: setup about $74 of $526 (14%), of which subagents about $70; 96 subagents start with about 31.2k tokens: instructions 9.7k, skills 9.6k, tool schemas 6.4k, deferred tool lists 3.0k; brief 1.8k. The hint is the one for skills or for instructions, whichever cost more.
- `tokprof scan 0268e98f`: 75 subagents start with about 26.6k tokens: instructions 10.4k, tool schemas 6.3k, skills 3.9k, deferred tool lists 3.3k.
- `tokprof scan 3edfd2e2`: after the compaction at 2026-10-13 15:31, the next prompt held 134.9k tokens. The largest parts are background agents' results, restored files, the skills invoked before the compaction, and the summary.
- The spend totals, the cost-state reconciliation and the parse time stay as they are.

Out of scope:
- Naming each instruction file (CLAUDE.md, AGENTS.md, MEMORY.md) apart. They stay one `instructions` source.
- Changing the existing advice items on what fills the latest prompt.
- Tool schemas of Codex, which its rollouts do not log.
- Setting anything: tokprof only reads logs and suggests.
- Effort advice that names the effort setting, before/after comparison and the cost of each skill run. These are separate changes.

## Capabilities

### New Capabilities
- `setup-breakdown`: what counts as setup, what it cost by source, what a main conversation and its subagents start with, what a compaction leaves, and the hints that name each agent's settings.

### Modified Capabilities
- `session-xray`: the layout gets the `setup` block after `tools`, and a new requirement defines the block.
- `cost-attribution`: the summary report gets the setup table between the spend by activity and the cache misses.

## Impact

- Core library: a setup aggregate fed by the existing content shares, first prompts of subagent chains and of the request after a compaction, built the way the live context is, and the X-ray data for the block.
- Log sources: the hints per source in the agent facts (Claude Code, Codex).
- CLI: rendering of the X-ray block and of the summary table; `tokprof ui` shows the X-ray text, so it gets the block too.
- README: the X-ray and summary sections.
- No new dependencies. Parse time: one more aggregate over shares that are already computed, and one prompt layout per subagent.
