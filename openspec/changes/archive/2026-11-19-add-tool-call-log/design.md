# Design

## Context

See proposal.md for why. What the code has today:
- The neutral model has no time on a tool call or its result. A block carries its kind, class, size, tool and target, and nothing else. A request has its send and end times. The precedent for per-call facts is the session's map of call ends, keyed by the index of every block of the call, the copies in later prompts included.
- Both decoders already map a call id to all the blocks of that call (Claude Code decoder.rs:760 and 776, Codex decoder.rs:105). The ids are log fields and stay in the sources.
- The tool table is a sink of the analysis pass. It counts a call when it sees the call's share of its response's output (analysis/mod.rs:696-726), and a result when the result first enters a prompt. The share has the call's block index, but the table keeps no times.
- `tokprof scan --requests` is the precedent for per-session records. The X-ray's sink holds a collector of the main requests (timeline.rs:226-312), rendered by tokprof-browse (render_requests.rs) and by the window (requests.rs).
- Tool classes follow ACP's tool kinds. Claude Code sets 7 classes (decoder.rs:1166-1177): `Agent` and `Task` are `Other`, and so are `LS`, `NotebookRead` and `NotebookEdit`. Codex sets only execute and execute output (decoder.rs:533-540), so `apply_patch`, `spawn_agent` and `web_search_call` are `Other`.
- Both sources already write `mcp:<server>` as the target of an MCP tool's blocks (claude-code tools.rs:67, codex tools.rs:12). Claude Code's tool lists come from `api-request-shape` lines, with the same targets (decoder.rs:683-709).
- `crates/tokprof/tests/architecture.rs` forbids agent names, tool names and log field names outside the sources.
- Logs are parsed, analysed and dropped one session at a time (scan.rs:38-90), so per-session data costs memory only while its session is open.

## Goals / Non-Goals

**Goals:**
- Times and flags per call from the same parse, with no second read of the logs.
- One set of groups for the CLI, the X-ray and the window. They come from neutral data only: the class and the MCP target.
- The call log and the tool table agree: the main chains' rows of the call log are the calls the tool table counts for those chains.

**Non-Goals:**
- Following a background call to the task notification that tells its real end.
- Joining a Codex command with the polls that read its output later.
- Times for pi and OpenCode, whose tool calls tokprof does not read.
- Any change of the share file.

## Decisions

1. **A call record on the session, keyed like call ends.**
   - The session gets a list of call records. A record holds the start, the end when the result came, whether the call ran in the background or asked the user, and the input head.
   - A map from block index to record index covers every block of the call: the call, the result, and their copies in later prompts, as for call ends.
   - The analysis reads the record at the call block of the response that made the call. That is where the tool table counts calls, so both views count the same calls.
   - Rejected: a time on every block. Most blocks are not calls, and the copies would need the same times.
   - Rejected: keying by the call id. The ids are log fields, which belong to the sources.
2. **Claude Code, seen in 2.1.284–2.1.326.**
   - Start: the `timestamp` of the `assistant` line with the `tool_use` item. Calls on one line share it.
   - End: the `timestamp` of the `user` line with the `tool_result` item whose `tool_use_id` matches. The decoder parses these timestamps already.
   - Background: `toolUseResult.backgroundTaskId` on a `Bash` result, and `toolUseResult.isAsync` `true` with `status` `async_launched` on an `Agent` result. Both were checked on 2.1.326 only, so the README names that version, and task 1.2 checks the older logs.
   - Asks the user: the source's existing rule (`AskUserQuestion`, `ExitPlanMode`, tools.rs:83).
   - A call without a result: see decision 11.
   - A `server_tool_use` item has its result in the same response, so it has no end.
   - Connected servers: the targets `mcp:<server>` of the shapes' tool blocks, and the server of each name in `addedNames` of `attachment` lines of `type` `deferred_tools_delta`. Names go through the source's `mcp__` pattern. The line filter (decoder.rs:219-231) lets `"deferred_tools_delta"` through. `removedNames` is not read: a server that was there once was connected.
3. **Codex, seen in 0.147–0.166.**
   - Start: the `timestamp` of the `function_call` or `custom_tool_call` line. End: the `timestamp` of the `*_output` line with the same `call_id`.
   - A hosted call, such as `web_search_call`, has no `call_id` and no output. It is done, with a start and no end.
   - Items replayed from a compaction have no time, so their calls have no times.
   - No call is marked as background. A command that still runs returns `Process running with session ID`, and its polls are rows of their own.
   - Asks the user: `request_user_input` and `request_user_input_async`, by name. The logs of 0.147–0.169 (every rollout on this machine, archived ones too) hold one such call, of `request_user_input_async` in 0.159.2. Its output `{"accepted":true}` came 26 ms after the call, and the answer came later as a message. So the call is `asked you`, and it stays out of times like every call that asks. `request_user_input` shows only in tool lists, never as a call; it is marked by its name, which says it waits for the user.
4. **Neutral classes for subagent tools and the agent's own tools, and sources that classify their tools.**
   - The class list gets `Subagent` and `Manage`, both refinements of ACP's `other`. `Manage` is a tool with which the agent manages itself and not the work. ACP's `think` and `switch_mode`, which the list has as `Think` and `SwitchMode`, are of that sort too; no source set them so far.
   - Claude Code: `LS` is search, `NotebookRead` read, `NotebookEdit` edit; `Agent`, `Task`, `SendMessage`, `ListAgents` and `SubagentHandback` subagent; `TodoWrite`, `TaskCreate`, `TaskGet`, `TaskUpdate` and `TaskList` think; `EnterPlanMode`, `ExitPlanMode`, `EnterWorktree` and `ExitWorktree` switch mode; `Skill`, `ToolSearch`, `ScheduleWakeup`, `AskUserQuestion` and the API's `advisor` manage. `Monitor`, `TaskStop` and `Artifact` stay other.
   - Codex: `apply_patch` is edit, `view_image` read, `wait` execute output; `spawn_agent`, `send_input`, `close_agent` and `resume_agent` subagent; `update_plan` think; `request_user_input`, `request_user_input_async` and `sleep` manage; `web_search_call` fetch. An `exec` call with one inner tool already takes that tool's name, so it gets that tool's class. `js` stays other.
   - The evidence for Codex, counted on 2026-11-19 over every rollout on this machine (0.147–0.169, archived ones too), inner tools of `exec` included: `exec_command` 333 calls, `write_stdin` 171, `js` 60, `apply_patch` 53, `wait` 40, `view_image` 5, `sleep` 4, `update_plan` 4, `request_user_input_async` 1. All 40 `wait` calls pass a `cell_id` and wait for the output of a running code cell, so `wait` is execute output, not a subagent tool. `spawn_agent`, `send_input`, `close_agent`, `resume_agent`, the `collaboration` tools and `web_search_call` were never called. Their kinds follow what their names say, and the README says they were not seen.
   - The evidence for Claude Code, in 7 days to 2026-11-19 (lines with calls, subagents included): `SendMessage` 669, `ToolSearch` 398, `Monitor` 260, `advisor` 246, `TaskStop` 177, `Skill` 155, `AskUserQuestion` 80, `ListAgents` 35, `SubagentHandback` 25, `ScheduleWakeup` 17, `EnterWorktree` and `ExitWorktree` 3 each. Before this change all of them were other.
   - The README of each source lists the kind of every tool it knows.
   - Effect on existing output: a Codex `web_search_call` or `view_image` row is now a fetch or read row, and a Claude Code `LS` or `NotebookRead` row a search or read row, so the hint rules for reads (tool-breakdown, Hints) can apply to them. The X-ray's content advice can name them, and Codex patches. No hint rule reads edit, think, switch mode or the new classes. Task 2.4 measures how many rows change their hint.
5. **Groups from the class and the MCP target.**
   - A row's group is set when the row is first seen. `mcp <server>` comes from a target that starts with `mcp:`, the convention both sources write. Otherwise the group comes from the class, as tool-breakdown's Groups lists: `Think`, `SwitchMode` and `Manage` give `agent`.
   - The groups table adds up the rows of each group. A group is a property of the tool, so all rows of one tool share it.
   - Rejected: `tokprof tools --by group`. `--by` picks the order of the same rows (`rent`, `trips`, `failed`); groups are other rows, and the usage error would list a sort that is not one. The groups table is short (7 to 10 lines), so it is printed before the tool table every time.
6. **Durations in the tool table, kept for exact medians.**
   - The call's share gets its duration, and whether it counts for times.
   - A row keeps the durations of its counted calls, in whole milliseconds as 32-bit numbers, unsorted, and their sum. Merging rows appends the lists. `med time` sorts a copy, or selects its middle, when the row is shown. A group's median comes from its rows' lists together, so it is the median of all its calls, as tool-breakdown asks.
   - Memory: 4 bytes per counted call. 7 days had 57,089 calls, so 30 days hold about 250,000, or about 1 MB in the merged table, plus 24 bytes per row for the list. Each parallel worker of the scan keeps such a table while it folds, so the peak is a few MB. The parse keeps no more than before. Task 6.3 measures it.
   - Rejected: a mean. In 7 days `Bash` had a mean of 12.3 s and a median of 0.62 s; one call of 8.6 hours sets the mean. The author chose the median and the total.
   - Rejected: a histogram of time buckets. It is smaller, but its median is a bucket's bound, so the call log and the tables would not show the same numbers, and tokprof does not estimate.
   - The share file's `tool_row` writes its fields one by one (share.rs:962-976), and its agents' failures object names its fields too (share.rs:1342). A new field of the row or the table cannot reach the file. A share test checks that the file of a fixture is byte for byte what it was.
   - The only way this change can move a share file is the class change of decision 4: through the hint counts (share.rs:983-997), and through the failure windows, which end when any shell call works (tool-breakdown, Failed, refused and denied calls). Codex `wait` becomes execute output, a shell call, so its calls now end shell windows. Task 2.4 measured both: no hint count moved, and Codex's spend after failures went from $47.96 to $49.07 over 30 days.
7. **The call log in the X-ray pass.**
   - A collector next to the request collector takes the main requests' produced calls. Each row gets its start, tool, group, input head, status and duration, and a link to its request.
   - Result tokens come in when the result block first enters a prompt. The block is matched to its row through the call record.
   - The request numbers are given after the pass, from the sorted request points, so they are the `#` of `tokprof scan --requests`.
   - Rows are sorted by start, and calls with the same start keep the order of the log. Main conversation only, as for `--requests`.
   - Memory: one row per main call, about 100 bytes. The largest sessions have a few thousand calls.
8. **Text in tokprof-browse.**
   - A new module renders the call log, and the groups table that the call log and `tokprof tools` share. Cells come apart from text, as with the tool cells, so that the window can use them in add-desk-tool-calls.
   - The tool cells get the `time` and `med time` columns after `calls`. The window's Tools tab lays out these cells with a fixed list of names and widths (inspector.rs:1020-1046). So this change adds the columns there too, and the desk checks run.
9. **CLI.** `tokprof scan` gets `--calls`, which conflicts with `--requests`. `--failed`, `--group` and `--tool` require `--calls`. The usage errors come from the argument parser, as for `--by`.
10. **Format.** A duration function in the core writes one decimal under 10 s and uses the rhythm's duration from 10 s on.
11. **A call without a result: `running` or `no result`.** A call has no result while the agent waits for it, or when it never got one: in 7 days, 7 of 57,089 calls. The call log says `running` only for calls of the main conversation's last request, and `no result` when later requests follow, since the agent went on without the result. The rule needs no clock and no idle threshold, so a snapshot and a later read of the same log agree. Rejected: an hour of silence, which would turn a long run into `no result` while it still runs.

## Parse time

- Claude Code: the timestamps are parsed already. New: one record and two map entries per call, one input head of at most 60 characters, a look at `toolUseResult` on `user` lines that are parsed already, and one more substring check per line for `"deferred_tools_delta"`. The last lets about 2 more lines per session through: 601 lines in 300 recent logs.
- Codex: the item times are parsed already. New: one record per call and the input head.
- Core: one lookup per call share and per entered result, and one push of 4 bytes per counted call.
- Expected: under 1% of the parse of the corpus. The check (task 6.3) allows `tokprof --since 30d summary` 2% and the X-ray of a 1,600-request session 0.1 s.

## Risks / Trade-offs

- [A permission wait or a user's answer inflates a call's time] → The spec says so. Calls that ask the user are left out of medians and sums. A permission wait cannot be told apart, since no agent logs it.
- [A median hides the long calls] → The `time` column next to it sums every counted call, so a few long calls still show.
- [Claude Code changes `toolUseResult` or the deferred-tools attachment] → The README names the fields with versions, the fixtures hold one line of each, and the conformance test runs over them.
- [A call without a result in an old session] → In 7 days, 7 of 57,089 calls had no result. Decision 11 shows them as `no result`.
- [Rows that change their kind get a hint] → Measured in task 2.4 and stated in the README.
- [Which tools are the agent's own is a judgment call] → The README of each source lists the kind of every tool, so a tool can move with one line. The share file reads no group.

## Settled Questions

- Median or mean: the median, with the total time next to it (decision 6).
- A group for the agent's own tools: `agent` (decisions 4 and 5).
- An old call without a result: `no result` (decision 11).
- Codex's agent tools and `request_user_input`: decided on the counts of decisions 3 and 4.
