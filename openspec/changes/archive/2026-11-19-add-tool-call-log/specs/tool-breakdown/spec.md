# Spec Delta

## MODIFIED Requirements

### Requirement: The tools command
`tokprof tools` SHALL print `Spend by tool: calls, sizes, what keeping them cost (rent) and what they made the agent do (trips)`, the groups table, and the tool table over the requests the filters select (see session-logs): `tool`, `calls`, `time`, `med time`, `avg result`, `avg input`, `rent`, `trips`, `single`, `failed`, `after`, `hint`. Rows go by rent unless `--by` names another order. The top 25 rows are shown, then `<n> more`, the total line and the failures line.

#### Scenario: Sorted by trips
- **WHEN** the user runs `tokprof --since 7d tools --by trips`
- **THEN** the row whose calls triggered the most spend comes first

#### Scenario: Sorted by failures
- **WHEN** the user runs `tokprof --since 30d tools --by failed`
- **THEN** the row with the most failed calls comes first, and rows with as many failed calls are sorted by `after`

#### Scenario: Unknown sort
- **WHEN** the user passes `--by size`
- **THEN** the command fails with a usage error that lists `rent, trips, failed`

#### Scenario: No prices
- **WHEN** nothing in the window is priced
- **THEN** `rent`, `trips` and `after` show tokens and the rows are sorted by them

#### Scenario: No failures
- **WHEN** no call of a row failed
- **THEN** its `failed` and `after` cells are empty

#### Scenario: Groups first
- **WHEN** the user runs `tokprof --since 7d tools`
- **THEN** the title comes first, then an empty line, the groups table, an empty line and the tool table

## ADDED Requirements

### Requirement: Duration of a call
A call's duration SHALL run from the time its log gives the call to the time it gives the call's result (see session-logs, Call times). It includes any wait for a permission, since no agent logs permission prompts. It is not busy time (see agent-time): calls run side by side, so their durations can add up to more than the time the agent was busy.

#### Scenario: A read
- **WHEN** a `Read` call is logged at 14:00:12.100 and its result at 14:00:12.180
- **THEN** the call took 0.08 s and shows `0.1s`

#### Scenario: A wait for a permission
- **WHEN** a `Bash` call is logged at 10:00:00, the user allows it at 10:02:30, and its result is logged at 10:02:31
- **THEN** the call took 151 s and shows `2.5m`

#### Scenario: Calls side by side
- **WHEN** two calls of one response are logged at 10:00:00 and both results at 10:00:30
- **THEN** each call took 30 s, the two add up to 60 s, and busy time counts those 30 seconds once

#### Scenario: A result in the same response
- **WHEN** a call's result comes in the same response as the call, as for a tool the API runs itself
- **THEN** the call has no duration and shows `–`

#### Scenario: A source without times
- **WHEN** a call's source gives no times for it
- **THEN** the call has no duration and shows `–`

### Requirement: Format of a duration
A duration under 10 seconds SHALL show one decimal and `s`. Longer ones are written as the X-ray writes the rhythm's durations: whole seconds under a minute, minutes with one decimal from 1 to 10 minutes, whole minutes up to an hour, then hours with one decimal.

#### Scenario: Short and long
- **WHEN** calls took 0.4 s, 23 s, 210 s, 1,680 s and 13,320 s
- **THEN** they show `0.4s`, `23s`, `3.5m`, `28m` and `3.7h`

### Requirement: Calls that count for times
Medians and sums of durations SHALL count only the calls that have a duration and that ran neither in the background nor asked the user (see session-logs). Calls without a result yet are not counted either. The rows of such calls still show their own duration, or `–`.

#### Scenario: A background command
- **WHEN** a `Bash npm run dev` call ran in the background and returned after 0.2 s
- **THEN** its row in the call log shows `0.2s`, and the medians and sums leave it out

#### Scenario: A question to the user
- **WHEN** an `AskUserQuestion` call waited 3 minutes for the answer
- **THEN** its row shows `3.0m`, and the medians and sums leave it out

#### Scenario: Nothing to count
- **WHEN** no call of a row or a group counts for times
- **THEN** its `time` and `med time` cells show `–`

### Requirement: Time per row
A tool row's `time` SHALL be the sum of the durations of its calls that count for times, its subagents' calls included, and `med time` their median: the middle duration in order, or the mean of the two middle ones for an even count. A median, not a mean, since a few long calls pull the mean far from a usual call.

#### Scenario: Three reads
- **WHEN** a session's `Read` calls took 0.1 s, 0.2 s and 0.6 s
- **THEN** the `Read` row shows `0.9s` under `time` and `0.2s` under `med time`

#### Scenario: A command run once in the background
- **WHEN** `Bash cargo test` ran 4 times for 20 s, 30 s, 40 s and 50 s, and once in the background
- **THEN** the row shows `5` calls, `2.3m` under `time` and `35s` under `med time`

#### Scenario: One long call
- **WHEN** `Bash sleep` ran 9 times for 0.5 s and once for 8.6 hours
- **THEN** the row shows `8.6h` under `time` and `0.5s` under `med time`

#### Scenario: Rows merged
- **WHEN** the same tool's calls come from several sessions
- **THEN** the median is that of all their counted calls, not of the sessions' medians

### Requirement: Groups
Every tool call SHALL belong to one group, by where its tool comes from. An MCP tool belongs to `mcp <server>`, one group per server. Any other tool belongs to the group of its kind (see session-logs, Kind of a tool): `files` for read, edit, write, delete and move; `search`; `shell` for execute and execute output; `web` for fetch; `subagents` for subagent; `agent` for think, switch mode and manage; `other` for the rest.

#### Scenario: An MCP tool
- **WHEN** a Claude Code session called `mcp__idea__search_symbol` 12 times
- **THEN** the 12 calls belong to the group `mcp idea`

#### Scenario: Built-in tools
- **WHEN** a session called `Read`, `Grep`, `Bash`, `WebSearch`, `Agent` and `Monitor`
- **THEN** they belong to `files`, `search`, `shell`, `web`, `subagents` and `other`

#### Scenario: The agent's own tools
- **WHEN** a Claude Code session called `Skill`, `ToolSearch`, `TodoWrite` and `ExitPlanMode`
- **THEN** they belong to `agent`, not `other`

#### Scenario: Codex tools
- **WHEN** a Codex session called `apply_patch`, `exec_command`, `spawn_agent`, the hosted web search and `update_plan`
- **THEN** they belong to `files`, `shell`, `subagents`, `web` and `agent`

### Requirement: Groups table
The groups table SHALL have one row per group with a call: `group`, `calls`, `failed`, `time`, `med time` and `tokens`. `time` and `med time` are as for a tool row (see Time per row), over the group's calls; `tokens` are its results' tokens as `avg result` counts them. Rows are sorted by calls, most first, `other` last. A last row `all` gives the totals.

#### Scenario: Three groups
- **WHEN** `shell` had 340 calls, 12 of them failed, and 300 that count for times took 42 minutes with a median of 0.6 s; `files` had 120 calls that took 24 s, median 0.1 s; `mcp idea` had 12 calls, 1 failed, that took 18 s, median 1.2 s
- **THEN** the rows read `shell 340 12 42m 0.6s`, `files 120 24s 0.1s` with an empty `failed` cell, and `mcp idea 12 1 18s 1.2s`, each with its tokens
- **AND** the row `all` reads `472 13 43m` and the median of all 432 counted calls

#### Scenario: Subagents' calls
- **WHEN** the main conversation made 412 calls and its subagents 608
- **THEN** the groups count all 1,020 calls, as the tool table does

### Requirement: Servers connected but never called
When a session's logs list the MCP servers it had (see session-logs, Connected MCP servers), a server with no call in the selected requests SHALL get a row in the groups table after the others: its group, `0` calls, empty cells, and `never called`. When the logs do not list the servers, no such row is shown and nothing says so.

#### Scenario: A Claude Code server never called
- **WHEN** a Claude Code session's tool list held the tools of the MCP server `linear`, and none of them was called
- **THEN** the groups table ends with the row `mcp linear 0` and `never called`

#### Scenario: Codex
- **WHEN** the selected requests come only from Codex sessions
- **THEN** the groups table has no `never called` row and no line about connected servers

### Requirement: Call log
`tokprof scan [target] --calls` SHALL print, in place of the X-ray, the tool calls of the main conversation of the session `tokprof scan` would pick: the calls of the main log's main chains, in the order of their start. The head is `tokprof · tool calls of the main conversation`, the X-ray's `session` line, a `calls` line, and the groups table of the session with its subagents.

#### Scenario: The calls line
- **WHEN** the main conversation made 412 calls and its subagents 608; 9 of the 412 failed, 2 were refused, 1 was denied, 1 is running, 2 got no result and 40 ran in the background; those that count for times took 28 minutes, with a median of 0.6 s
- **THEN** the line reads `calls     412 in the main conversation, 1,020 with the subagents' · 9 failed · 2 refused · 1 denied · 1 running · 2 no result · 40 background · 28m in calls, median 0.6s`
- **AND** a count that is zero is left out of the line

#### Scenario: No calls
- **WHEN** the main conversation made no tool call
- **THEN** the head ends with `calls     0 calls`, and there is no table

#### Scenario: Both lists
- **WHEN** the user passes `--calls` and `--requests` together
- **THEN** the command fails with a usage error that says they cannot be used together

#### Scenario: The plain X-ray
- **WHEN** the user runs `tokprof scan 5e5510f0` without `--calls`
- **THEN** the output is the X-ray, with the `tools` block as session-xray describes it

### Requirement: Call log rows
Under a header, the call log SHALL print one row per call: `#`, the local start time `MM-DD HH:MM:SS`, `req`, `group`, `tool`, `input`, `status`, `took` and `tokens`. `req` is the number of the request whose response made the call, as `tokprof scan --requests` numbers it. `took` is the call's duration. `tokens` is the size of its result as `avg result` counts it, or `–` when the log cannot rebuild it.

#### Scenario: A failed test run
- **WHEN** call 17 started at 11-19 14:00:12, was made by request 5, ran `cargo test -p tokprof --test golden` through `Bash`, failed after 41 s and returned 3,400 tokens
- **THEN** its row reads like `    17  11-19 14:00:12    5  shell  Bash  cargo test -p tokprof --test golden  failed  41s  3.4k`

#### Scenario: Calls of one response
- **WHEN** the response of request 5 made a `Read` and a `Grep` call
- **THEN** both rows show `5` under `req`, in the order of the log

#### Scenario: A subagent
- **WHEN** the main conversation's `Agent` call started a subagent that worked for 4 minutes and 10 seconds and made 30 calls
- **THEN** the call log has one row for it in the group `subagents`, with `4.2m` under `took`, and no row for the subagent's 30 calls

#### Scenario: A result not yet in a prompt
- **WHEN** the session's last call returned, but no request has carried its result yet
- **THEN** its `tokens` cell shows `–`

### Requirement: Status of a call
A row's status SHALL be `failed`, `refused` or `denied` as session-logs' Call outcomes tells. Otherwise it is `background` for a call that returned while its work went on, `asked you` for a call that asked the user, and `done` for the rest. A call without a result is `running` while no later request of the main conversation is logged, and `no result` after one.

#### Scenario: Background
- **WHEN** a `Bash` call returned at once with the id of a background task
- **THEN** its status is `background`

#### Scenario: Running
- **WHEN** a call of the main conversation's last response has no result in the log yet
- **THEN** its status is `running` and its `took` cell shows `–`

#### Scenario: No result
- **WHEN** a call has no result, and the main conversation sent requests after the one that made it
- **THEN** its status is `no result` and its `took` cell shows `–`, as the agent went on without it

#### Scenario: Refused
- **WHEN** the user refused an `Edit` call
- **THEN** its status is `refused`, whatever else the log says about it

### Requirement: Input of a call
A row's `input` SHALL show the short head of the call's input that its source gives (see session-logs, Input of a call), cut to 48 characters with `…`. It is empty when the source gives none. The input text is shown on this machine only and never goes into a share file (see share-export).

#### Scenario: A long command
- **WHEN** a `Bash` call ran a command whose first line is 70 characters long
- **THEN** the cell shows its first 47 characters and `…`

#### Scenario: No input head
- **WHEN** the source gives no input head for a call
- **THEN** the cell is empty

### Requirement: Narrowing the call log
With `--calls`, `--failed` SHALL keep only the rows whose status is `failed`, `--group <name>` only the rows of that group, and `--tool <name>` only the calls of that tool, its name compared without case. They can be combined. The `calls` line and the groups table still count every call, and a line under the header reads `showing <n> of <m> calls`.

#### Scenario: Failed shell calls
- **WHEN** the user runs `tokprof scan --calls --failed --group shell` on a session whose 412 calls hold 9 failures, 7 of them in `shell`
- **THEN** the log shows 7 rows and the line `showing 7 of 412 calls`

#### Scenario: All MCP servers
- **WHEN** the user passes `--group mcp`
- **THEN** the rows of every `mcp <server>` group are kept

#### Scenario: An unknown group
- **WHEN** the user passes `--group disk`
- **THEN** the command fails with a usage error that lists `files, search, shell, web, subagents, agent, mcp, mcp <server>, other`

#### Scenario: Without the call log
- **WHEN** the user passes `--failed`, `--group` or `--tool` without `--calls`
- **THEN** the command fails with a usage error that says they need `--calls`
