# Design

## Context

- cost-attribution already charges every share of a request's spend to a block of its prompt. A share carries the kind, the target (a reminder's source) and the chain. The X-ray keeps these shares as `content` by kind, tool and target. So the setup cost needs no new attribution, only a new sum.
- The X-ray rebuilds one prompt, the latest of the main conversation. It replays the chain's prompt deltas (`keep` and `tail`) up to that request and sizes each block by its share of the characters, scaled to the request's prompt tokens. A subagent log is its own session in the X-ray, with one subagent chain.
- Since the split of joined reminders, reminder sources are reliable for Claude Code 2.1.300–2.1.316: `instructions`, `skills`, `deferred tool lists`, `hook context`, `background notifications`, `attached files`, `task list` and `other`. The core maps labels to reminder kinds, and `attached files` and `task list` fall into the `other` kind today.
- Only a log source may name its agent's settings (`crates/tokprof/tests/architecture.rs`). The agent facts already carry agent-specific names this way: `instruction_files` and `ttl_settings`.

## Goals / Non-Goals

**Goals:**
- One definition of setup, used by the X-ray block and the summary table.
- What a chain starts with, and what a compaction leaves, from the same prompt replay as the live context. A second way to size prompts would drift.
- Hints as data from the sources, picked by the core.

**Non-Goals:**
- Per-file instruction sizes. The instructions reminder stays one source.
- New advice items in the `advice` block. The hint lives in the `setup` block.
- Any change to cost attribution or to the reconciliation with cost-state.

## Decisions

1. **Setup is decided by kind and reminder kind.** Two reminder kinds are added: attached files and task list. Setup is then tool schemas, the system prompt, and reminders whose kind is not background notifications, attached files or task list. Alternative: compare label strings in the core. Rejected: the labels are the sources' vocabulary, and the kinds are where the core reads them already. The advice keeps its fallback hint for the new kinds, so its output does not change.

2. **The setup cost is a new sink over the existing facts.** A share is setup when its kind and class say so. It adds the share to `(agent, source)`, and to its subagent part when the chain is a subagent chain. Facts gain the chain's kind, so the core does not parse chain names for `subagent`. The X-ray feeds the same sink for its sessions, and `tokprof summary` feeds it in the collector. Alternative: derive the X-ray numbers from `content`. Rejected: `content` has no chain kind, and two code paths would round differently.

3. **One prompt replay for three uses.** The replay in the live context becomes a function that lays out the prompt of a given request of a chain. The live context calls it for the latest request, the setup view for the first request with a known prompt of each chain, and for the first answered request after the latest compaction marker of the main conversation. Sizing is shared: a block's characters over the prompt's characters, times the prompt's tokens. The first request of a chain needs only its own delta, so the per-subagent cost is small.

4. **Means for subagents.** Means add up across sources, so the `each subagent` column sums to its total. The row shows how many subagents had a known first prompt. A median per source would not add up. The proposal's check numbers (mean 31.2k, median 31.7k for 03aee587) show the two are close for these logs.

5. **Hints are a list in the agent facts.** `setup_hints` maps a source label to a sentence. The Claude Code source gives skills, instructions, deferred tool lists and hook context; the Codex source gives instructions. The core takes the hint of the costliest setup source, if there is one. Alternative: hints in the core keyed by agent name. Rejected by the architecture rule.

6. **Rendering follows the tools block.** The X-ray block is a fixed-width table with rows by cost, then the compaction line and the hint. The hint is wrapped like an advice item. The summary table follows the trigger and activity tables. Rows with no cost are left out, and so is the section when nothing remains. `tokprof ui` renders the same X-ray text, so it gets the block with no extra work.

Log fields and versions this depends on:
- Claude Code request-audit lines `api-request` (`keep`, `tail`, `shapeHash`), `api-request-blob` (`message`, role `system` for joined reminders) and `api-request-shape` (`tools`, `system`), seen in 2.1.300–2.1.316.
- The reminder openings that session-logs lists, seen in 2.1.300–2.1.316.
- `compact_boundary` system lines for the compaction marker, as session-logs reads them (seen in 2.1.284–2.1.312).
- Subagent logs in the session's `subagents` directory with `agent-<id>.meta.json`, as session-logs reads them.
- The Claude Code settings the hints name, `skillListingBudgetFraction` (default 0.01) and `skillOverrides`, are in the settings schema of 2.1.316. The sources' README files record this.
- Codex: the reminders its rollouts carry. Codex logs no tool schemas other than `additional_tools`, so its tool schema rows stay small.

Parse time: the summary adds one hash-map update per setup share, next to the profile aggregation that already visits every share. Expected under 1% of the 25–27 s full run. The X-ray adds one prompt replay per subagent session (first request only) and one for the request after the latest compaction. That is under 5% of an X-ray of a session with 98 subagents (about 3 s today). Both are measured in the tasks.

## Risks / Trade-offs

- [Claude Code changes the reminder openings] → Parts fall back to the label their first words give, so sources can drift into `other`. Mitigation: the openings are tested in the source, and session-logs lists them with versions. A drift shows up as `other` growing in `tokprof summary`.
- [A hint names a setting an older Claude Code does not have] → The hint text says "in settings.json". The Claude Code source README records the version the settings were seen in.
- [Setup the work needs looks like waste] → Instructions and skills help the agent. The block reports cost and names the knob. It claims no savings, and the hint is one line.
- [Means hide outliers] → One subagent with a huge setup lifts the mean. The count is shown, and the summary gives the totals, which are exact.
- [Compaction parts mix summary and restored content] → The summary is `user prompt`, which session-logs keeps apart from typed messages. The line names kinds and sources as they are, without guessing which part the compaction wrote.
