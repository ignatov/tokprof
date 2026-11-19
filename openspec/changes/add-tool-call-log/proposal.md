# Proposal

## Why

The author wants to know which tools the agent called, in what order, and how long each call took. Today tokprof has only sums per tool: `tokprof tools`, the X-ray's `tools` block and the window's `Tools` tab (see tool-breakdown). They tell how often a tool ran and what its results cost. They do not list the calls, they do not tell how long a call took, and they do not group tools by where the tools come from.

The logs already hold the times. Claude Code stamps the line that makes a call and the line that brings its result. Codex stamps the call item and its output item. A quick count on 2026-11-19 over the author's logs:
- **Claude Code, last 7 days, subagents included:** 844 logs, 57,089 calls. All but 7 have a result. 563 shell calls ran in the background, and 765 subagents were started in the background. These return at once. Of the other calls, the median took 0.52 s and the mean 15.1 s. Means per tool: `Bash` 12.3 s (median 0.62 s), `Read` 0.6 s, `Edit` 0.3 s, `WebFetch` 11.9 s, `WebSearch` 27.2 s, MCP tools 2.2 s.
- **Claude Code main conversations only, same days:** 350 logs, 19,066 calls. `AskUserQuestion` took a median of 3.1 minutes, because it waits for the user's answer.
- **Codex, last 30 days:** 28 rollouts, 605 calls, every one with an output. The median took 1.16 s and the mean 7.3 s.

No time was negative. Long means come from a few long calls: one `Bash` call took 8.6 hours. So the views show the median of a call's time, which a few long calls do not move, and the total time of the calls, which shows them. No agent logs permission prompts, so a call's time includes any wait for a permission.

## What Changes

- **Time of a call.** The Claude Code and Codex sources give each tool call the time of the call and the time of its result. A call that returned while its work went on (a background shell command, a subagent started in the background) is marked `background`. A call that asks the user (Claude Code's questions and plan approvals, Codex's `request_user_input`) is marked `asked you`. A call without a result is `running` until the main conversation sends another request, and `no result` after that.
- **Groups.** Every tool belongs to one group, by where it comes from: `files`, `search`, `shell`, `web`, `subagents`, `agent`, one group per MCP server (`mcp <server>`), and `other`. `agent` holds the tools with which the agent manages itself and not the work: skills, tool search, the task list or plan, mode switches, questions to the user. The sources classify their own tools, so that the Codex patch tool, web search and subagent tools do not land in `other`.
- **Connected, never called.** For Claude Code, an MCP server that the session's tool lists show but that got no call is listed as a group with 0 calls.
- **The call log.** `tokprof scan [target] --calls` prints, in place of the X-ray, every tool call of the main conversation as a row: time, request, group, tool, input, status, duration and result tokens. A head gives the totals and a groups table. `--failed`, `--group <name>` and `--tool <name>` narrow the rows.
- **`tokprof tools`** gets the `time` and `med time` columns (the total and the median time of the calls) and, before the tool table, a groups table: calls, failed, time, median time and result tokens per group.
- **The X-ray's `tools` block** gets the `time` and `med time` columns and a `groups` line.
- README.md and the sources' READMEs describe the new output and the log fields.

## Capabilities

### New Capabilities

None. The call log fits tool-breakdown: its rows, statuses and groups are about tool calls.

### Modified Capabilities

- `tool-breakdown`: new requirements for a call's duration, the time and median time per row, groups, the groups table, never-called servers and the call log; the tools command gets the groups table and the `time` and `med time` columns.
- `session-xray`: the `tools` block gets the `time` and `med time` columns and the `groups` line.
- `session-logs`: new requirements for call times, calls that return before their work ends, calls that ask the user, the kind of a tool, the input of a call and connected MCP servers, with the log fields of each agent.

## How it is checked on the real logs

The baseline is the count of 2026-11-19 above. Its rule: a call's time runs from the `timestamp` of the line with the call to the `timestamp` of the line with its result; background calls and calls without a result are left out of the means and medians.
- `tokprof --agent claude --since 7d tools`: for `Read`, `Edit`, `WebFetch`, `WebSearch` and the MCP groups, `time` divided by the counted calls is within 10% of the count's means, and `med time` within 10% of the medians of the same count run again, given the calls added since. `Bash` shows a median near 0.62 s. The groups table's `calls` add up to the table's calls.
- `tokprof scan <id> --calls` on three Claude Code sessions and one Codex session: the number of rows equals the count of calls in the main log, by the same rule, and the background rows equal the count of `backgroundTaskId` and `isAsync` results in it.
- The same sessions: the sum of the rows' result tokens per tool equals the main chains' part of the tool table's result tokens.
- `tokprof --since 30d summary` takes at most 2% longer than before, and `tokprof scan` of a 1,600-request session at most 0.1 s longer.

## Out of scope

- The share file. It gets no call log, no input text and no times, now or later.
- The real end of a background call. Its row shows the time until it returned; the task notification is not followed.
- Joining a Codex command with the polls that read its output later. Each poll is its own row.
- Tool calls of pi and OpenCode. tokprof does not read them today, and this change does not start.
- Connected tools of Codex, pi and OpenCode. Their logs do not list them (see design).
- Subagents' own calls as rows of the call log. They count in the groups and in the tool table, as today.
- Permission prompts. No agent logs them.
- The window. The Tools tab gets the cards and the call log in add-desk-tool-calls.

## Impact

- Model: per call, the time of the call and of its result, whether it ran in the background or asked the user, and a short head of its input; per session, the MCP servers its tool lists show; new tool classes for subagent tools and for the agent's own tools. The tool table keeps the durations of the counted calls, so that medians stay exact when tables merge.
- Sources: the Claude Code and Codex decoders, their tool classes, fixtures and READMEs.
- Core: the call log of the main conversation in the X-ray, groups and durations in the tool table.
- tokprof-browse: the text of the call log and the groups table, shared with the window.
- CLI: `tokprof scan --calls`, the tools command and the X-ray's `tools` block. The goldens of `tokprof tools` change.
