# tool-breakdown Specification

## Purpose
Shows, per tool and shell command, how often an agent called it, how big its results and inputs were, what they cost and which habit to change, so the user can tell whether a session could have been cheaper.

## Requirements

### Requirement: Rows by tool and command
The tool breakdown SHALL have one row per tool, the session's subagents included. A shell tool, such as Claude Code `Bash` or Codex `exec_command`, gets one row per command head (see cost-attribution): `Bash sed -n`, `exec_command cargo test`. A Codex `exec` call whose code calls one inner tool shows as that tool, as cost-attribution names it; one that calls several shows as `exec`.

#### Scenario: Shell commands
- **WHEN** a session ran `sed -n 1,80p a.rs` 56 times and `grep -n foo src` 40 times through `Bash`
- **THEN** the rows are `Bash sed -n` with 56 calls and `Bash grep -n` with 40 calls

#### Scenario: Other tools
- **WHEN** a session called `Read` on 3 different files
- **THEN** they make one `Read` row with 3 calls, whatever the files

### Requirement: Sizes per call
`avg result` and `avg input` SHALL be the mean tokens of a row's tool results and of its call inputs, each block counted once, at the size it had in the first request that held it. A row whose sizes the log cannot rebuild shows `–`.

#### Scenario: A large read
- **WHEN** one `Read` returned a 16,900-token file
- **THEN** its row shows `1` call and `16.9k` as `avg result`

### Requirement: Rent
A row's `rent` SHALL be the spend charged to its tool results and tool calls by cost-attribution: every component, over every request that held them, rewrites after misses included.

#### Scenario: Results kept for a long time
- **WHEN** 56 `sed -n` results of 1,300 tokens each stayed in a growing context for the rest of the session
- **THEN** the `Bash sed -n` row's rent is the sum of what each result cost in every request after it

### Requirement: Trips
A row's `trips` SHALL be the input spend of the requests that its calls' results triggered, everything but their output, plus the output of the responses that made the calls. A request is triggered by the tool results that its prompt carries for the first time. When a response made several calls, its spend and the next request's are split equally between them.

#### Scenario: Calls in one response
- **WHEN** a response made a `Read` and a `Bash grep -n` call, its output cost $0.10, and the next request's input cost $0.30
- **THEN** each row gets $0.20 of trips from that response

#### Scenario: A prompt without composition
- **WHEN** a request's prompt cannot be rebuilt from the log
- **THEN** no row gets its input spend as trips

#### Scenario: Both views add up
- **WHEN** the rows' rent is added to the spend of content no tool produced, and the rows' trips to the input of requests no tool result triggered and the output of responses without calls
- **THEN** each sum equals the spend

### Requirement: Two views of the same money
The table SHALL never add rent and trips. Its last line SHALL give the session's or window's spend, how much of it the rows' rent covers, and how much the rows' trips cover, so that each view can be compared with the whole.

#### Scenario: The total line
- **WHEN** a session cost $27.37, its rows' rent sums to $21.40, and their trips to $24.90
- **THEN** the line reads `spend $27.37 · rent of these tools $21.40 · trips of these tools $24.90 (two views of the same money; do not add them)`

### Requirement: Single calls
A row's `single` SHALL be the share of its calls that were the only call of their response.

#### Scenario: Reads one at a time
- **WHEN** 48 of 56 `sed -n` calls were alone in their response
- **THEN** the row shows `86%`

### Requirement: Hints
Each row SHALL get the first hint whose rule applies, in the order of the rule scenarios below, or none. The rules apply only to rows with at least 5 calls and at least $1 of rent or trips.

#### Scenario: Rule 1, reads made one per response
- **WHEN** a read-only row, a read, search or fetch tool or a shell row whose command head is read-only (see cost-attribution), has `single` at 60% or more
- **THEN** its hint is `batch independent reads`

#### Scenario: Rule 2, long inline scripts
- **WHEN** a shell row that is not read-only has an `avg input` of 400 tokens or more
- **THEN** its hint is `save long scripts to a file; edit files with the edit tool`

#### Scenario: Rule 3, large reads
- **WHEN** a read row, such as `Read`, has an `avg result` of 5,000 tokens or more
- **THEN** its hint is `read a range or search instead of the whole file`

#### Scenario: Rule 4, long command output
- **WHEN** a shell row has an `avg result` of 2,000 tokens or more
- **THEN** its hint is `filter the output with head or grep`

#### Scenario: Rule 5, whole-file rewrites
- **WHEN** a whole-file write row, such as `Write`, has an `avg input` of 2,000 tokens or more
- **THEN** its hint is `edit instead of rewriting the file`

#### Scenario: Reads one per response
- **WHEN** `Bash sed -n` has 56 calls, 86% single, and $5.21 of rent
- **THEN** its hint is `batch independent reads`

#### Scenario: Inline scripts
- **WHEN** `Bash python3 -I` has 60 calls with an `avg input` of 702 tokens and $4.32 of rent
- **THEN** its hint is `save long scripts to a file; edit files with the edit tool`

#### Scenario: A large read, once
- **WHEN** `Read` has 1 call of 16,900 tokens
- **THEN** it has no hint, since rules need at least 5 calls

#### Scenario: Small rows
- **WHEN** a row has 4 calls, or less than $1 of both rent and trips
- **THEN** it has no hint

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

### Requirement: Failed, refused and denied calls
A row's `failed` SHALL count its calls that failed (see session-logs, Call outcomes). Calls that the user refused or a permission rule denied are not failures, and are counted apart for the whole table.

#### Scenario: A missing file
- **WHEN** 3 of a session's 120 `Read` calls returned `File does not exist`
- **THEN** the `Read` row shows `3` under `failed`

#### Scenario: A refused edit
- **WHEN** the user refused one `Edit` call
- **THEN** the `Edit` row does not count it as failed, and the table counts one refused call

### Requirement: What came after a failure
After a failed call, the window SHALL hold the requests of its chain from the first that carried the failure up to the first whose response made a call that worked of the same tool, or of any shell tool after a shell call, within one turn of a main conversation (see work-rhythm, Turns) or within a subagent's chain. A row's `after` is the spend of its windows. Failures before the working call share one window.

#### Scenario: Read again, then edited
- **WHEN** an `Edit` fails because its text was not found, the next request reads the file, and the request after it makes an `Edit` that works
- **THEN** the window holds those two requests, and their spend counts in the `Edit` row's `after`

#### Scenario: Tried again at once
- **WHEN** a `Bash cargo test` call fails, and the request that carried the failure runs `cargo test` again, which passes
- **THEN** the window holds that one request

#### Scenario: Another command of the shell
- **WHEN** a `Bash cargo test` call fails, and the request that carried the failure runs `git diff`, which works
- **THEN** the window holds that one request, since the shell worked again
- **AND** a command that appears much later in the turn does not stretch the window over the work between
- **AND** a Codex `write_stdin` that reads a failed command's exit ends its window at the next `exec_command` that works, since both are shell calls

#### Scenario: Two failures in a row
- **WHEN** a `Bash git push` call fails twice and the third works
- **THEN** the two failures share one window, from the first request that carried a failure to the request that made the call that worked

#### Scenario: Never worked again
- **WHEN** a call fails and no call of its tool works before the turn ends
- **THEN** the window is the one request that carried the failure

#### Scenario: Windows of two rows
- **WHEN** the windows of two rows hold the same request
- **THEN** each row's `after` counts it, and the failures line counts it once

### Requirement: Failures line
After the total line, `tokprof tools` SHALL print how many calls failed, how many the user refused and how many permission rules denied, and the spend of the failure windows of all rows, each request counted once. Counts that are zero are left out, and with no such call at all there is no line.

#### Scenario: The line
- **WHEN** 1,021 calls failed, the user refused 52, rules denied 101, and the windows hold $141.80 of spend
- **THEN** the line reads `failed 1,021 calls, after them $142 until the tool worked again · you refused 52 · rules denied 101`, with dollars written as everywhere in tokprof

#### Scenario: Only failures
- **WHEN** 3 calls failed and none was refused or denied
- **THEN** the line reads `failed 3 calls, after them $0.42 until the tool worked again`

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
- **WHEN** a session called `Read`, `Grep`, `Bash`, `Monitor`, `WebSearch`, `Agent` and `Artifact`
- **THEN** they belong to `files`, `search`, `shell`, `shell`, `web`, `subagents` and `other`

#### Scenario: The agent's own tools
- **WHEN** a Claude Code session called `Skill`, `ToolSearch`, `TodoWrite`, `ExitPlanMode` and `CronCreate`
- **THEN** they belong to `agent`, not `other`

#### Scenario: Codex tools
- **WHEN** a Codex session called `apply_patch`, `exec_command`, `exec` with code that calls several tools, `spawn_agent`, the hosted web search and `update_plan`
- **THEN** they belong to `files`, `shell`, `shell`, `subagents`, `web` and `agent`

#### Scenario: Codex MCP tools
- **WHEN** a Codex session called `js` in the `namespace` `mcp__cua_repl`, and `exec` with code that calls only `tools.mcp__docs__search`
- **THEN** they belong to `mcp cua_repl` and `mcp docs`

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
