## MODIFIED Requirements

### Requirement: Log locations
tokprof SHALL look for session logs only in the agents' log locations, at any depth, as Claude Code and Codex log locations, Pi log locations, OpenCode stores and Copilot log folders name them. A location that does not exist adds no logs and causes no error.

#### Scenario: Default directories
- **WHEN** none of `CLAUDE_CONFIG_DIR`, `CODEX_HOME`, `PI_CODING_AGENT_DIR`, `PI_CODING_AGENT_SESSION_DIR`, `XDG_DATA_HOME` and `OPENCODE_DB` is set
- **THEN** logs are read from `~/.claude/projects`, `~/.codex/sessions`, `~/.codex/archived_sessions` and `~/.pi/agent/sessions`, and sessions from the OpenCode stores in `~/.local/share/opencode`

#### Scenario: Directory set by the environment
- **WHEN** `CODEX_HOME=/data/codex` is set
- **THEN** Codex rollouts are read from `/data/codex/sessions` and `/data/codex/archived_sessions` only

#### Scenario: Compressed archived rollout
- **WHEN** `~/.codex/archived_sessions/` holds `rollout-2026-08-06T17-31-27-<thread id>.jsonl.zst`
- **THEN** it is read like a plain rollout

#### Scenario: Missing directory
- **WHEN** a log directory does not exist, such as `~/.codex/archived_sessions` before Codex archives its first thread
- **THEN** it adds no logs and causes no error

#### Scenario: Other files
- **WHEN** a Claude Code transcript is compressed as `.jsonl.zst`, or a file such as `~/.codex/history.jsonl` lies outside the Codex log directories
- **THEN** it is not picked up when tokprof looks for session logs

#### Scenario: Copilot logs
- **WHEN** `tokprof --agent copilot sessions` runs and none of `TOKPROF_COPILOT_DIR` and `TOKPROF_DIR` is set
- **THEN** Copilot logs are read from `~/tokprof/copilot`

### Requirement: Agent selection
`--agent` SHALL choose whose logs every command reads, as a comma-separated list of `claude`, `codex`, `pi`, `opencode` and `copilot`; the default is `claude,codex,pi,opencode`.

#### Scenario: One agent
- **WHEN** `tokprof --agent codex sessions` runs
- **THEN** only Codex sessions are listed

#### Scenario: Unknown agent
- **WHEN** `--agent foo` is given
- **THEN** the command fails with `unknown agent "foo"` before reading any log

#### Scenario: pi only
- **WHEN** `tokprof --agent pi sessions` runs
- **THEN** only pi sessions are listed

#### Scenario: OpenCode only
- **WHEN** `tokprof --agent opencode sessions` runs
- **THEN** only OpenCode sessions are listed

#### Scenario: Copilot only when asked
- **WHEN** `tokprof sessions` runs without `--agent`, and the Copilot log folder holds logs
- **THEN** no Copilot session is listed, and `tokprof` with no arguments never picks one as the latest session

### Requirement: Agent-reported cost
tokprof SHALL take a session's own cost, as the agent counted it, where its log reports one: Claude Code's from Claude Code's cost counter, pi's from Pi's own cost, OpenCode's from OpenCode's own cost, Copilot's from Copilot's billed credits. cost-attribution compares it with tokprof's total in `tokprof summary`, or shows it alone for sessions without usage; it is never used as a price. Codex logs no such figure.

#### Scenario: Several cost-state lines
- **WHEN** a transcript's `cost-state` lines share one `startTime` and read $10.00, $30.00 and $28.00
- **THEN** the session's reported cost is $30.00

#### Scenario: A run that started from zero
- **WHEN** a transcript has `cost-state` lines with one `startTime` up to $24.06, then lines with another `startTime` from $2.66 up to $32.89
- **THEN** the session's reported cost is $56.95

#### Scenario: Runs that interleave
- **WHEN** two processes wrote to one transcript at once, and its `cost-state` lines read $1.34 under one `startTime`, $1.41 under another, then $24.70 under the first
- **THEN** the session's reported cost is $26.11

#### Scenario: No cost-state line
- **WHEN** a transcript has no `cost-state` line
- **THEN** the session has no reported cost and is left out of the comparison

### Requirement: Call times
The Claude Code, Codex and Copilot sources SHALL give each tool call the time of the line that made it and the time of the line that brought its result, as Copilot calls and results says for Copilot. A call without a result line has no end. pi and OpenCode give no tool calls, so they give no call times.

#### Scenario: Claude Code
- **WHEN** an `assistant` line holds a `tool_use` item, and a later `user` line holds the `tool_result` item whose `tool_use_id` is that item's `id` (seen in 2.1.284–2.1.326)
- **THEN** the call starts at the `timestamp` of the `assistant` line and ends at the `timestamp` of the `user` line

#### Scenario: Calls on one line
- **WHEN** one `assistant` line holds two `tool_use` items
- **THEN** both calls start at that line's `timestamp`

#### Scenario: A tool the API runs
- **WHEN** a Claude Code `server_tool_use` item has its result in the same response
- **THEN** the call has no end of its own, and no duration

#### Scenario: Codex
- **WHEN** a `function_call` or `custom_tool_call` item is followed by the `function_call_output` or `custom_tool_call_output` item with the same `call_id` (seen in 0.147–0.166)
- **THEN** the call starts at the `timestamp` of the first item's line and ends at the `timestamp` of the second's

#### Scenario: A Codex hosted call
- **WHEN** a `web_search_call` item has no output item
- **THEN** the call starts at its line's `timestamp`, has no end and no duration, and counts as done

#### Scenario: Items replayed from a compaction
- **WHEN** a Codex call comes from the history a compaction replayed, without a time
- **THEN** the call has no times

#### Scenario: Copilot
- **WHEN** a Copilot response's first chunk with `created` 1800000011 makes a `bash` call, and the chunk of the same `id` that repeats that call has `created` 1800000015
- **THEN** the call starts at 1800000011 and ends at 1800000015, and took 4 s

### Requirement: Calls that return before their work ends
A source SHALL mark a call that returned while the work it started went on. Claude Code and Copilot tell this.

#### Scenario: A background command
- **WHEN** a `Bash` call's `user` line has `toolUseResult.backgroundTaskId` (seen in 2.1.284–2.1.328)
- **THEN** the call is marked as one that ran in the background

#### Scenario: A subagent in the background
- **WHEN** an `Agent` call's `user` line has `toolUseResult.isAsync` set to `true` and `toolUseResult.status` set to `async_launched` (seen in 2.1.284–2.1.328)
- **THEN** the call is marked as one that ran in the background

#### Scenario: Codex
- **WHEN** a Codex `exec_command` call returns while its command still runs, and later `write_stdin` calls read its output
- **THEN** no call is marked, and each call keeps its own times

#### Scenario: A Copilot command in the background
- **WHEN** a Copilot `bash` call's result has `toolTelemetry.properties.executionMode` `async`, or `detached` `true` (seen in logs of 2026-11-14 to 2026-11-21)
- **THEN** the call is marked as one that ran in the background

### Requirement: Kind of a tool
Each source SHALL give each of its tools one kind: read, edit, write, delete, move, search, execute, execute output, fetch, subagent, think, switch mode, manage, environment setup or other. Manage is a tool with which the agent manages itself and not the work. Environment setup is a step that the agent's platform runs to prepare the environment before the model works. A tool the source does not know is other. The source's README lists the kind of each tool it knows.

#### Scenario: Claude Code
- **WHEN** a Claude Code session calls `Read`, `NotebookRead`, `Edit`, `MultiEdit`, `NotebookEdit`, `Write`, `Grep`, `Glob`, `LS`, `Bash`, `BashOutput`, `WebFetch`, `WebSearch`, `Agent`, `Task` and `SendMessage`
- **THEN** their kinds are read, read, edit, edit, edit, write, search, search, search, execute, execute output, fetch, fetch, subagent, subagent and subagent

#### Scenario: Claude Code's background commands
- **WHEN** a Claude Code session calls `Monitor`, which runs a command in the background and reports its lines, and `TaskStop`, which stops such a command (seen in 2.1.284–2.1.328)
- **THEN** their kinds are execute and execute

#### Scenario: Claude Code's own tools
- **WHEN** a Claude Code session calls `TodoWrite`, `ExitPlanMode`, `EnterWorktree`, `Skill`, `ToolSearch`, `ScheduleWakeup`, `CronCreate`, `AskUserQuestion` and the API's `advisor`
- **THEN** their kinds are think, switch mode, switch mode, manage, manage, manage, manage, manage and manage

#### Scenario: Codex
- **WHEN** a Codex session calls `exec_command`, `write_stdin`, `wait`, `apply_patch`, `view_image`, `spawn_agent` and the hosted `web_search_call`
- **THEN** their kinds are execute, execute output, execute output, edit, read, subagent and fetch, as `wait` waits for the output of a running code cell (seen in 0.147–0.167)

#### Scenario: Codex's own tools
- **WHEN** a Codex session calls `update_plan`, `update_goal`, `request_user_input`, `sleep` and the `collaboration` tool `wait_agent`
- **THEN** their kinds are think, think, manage, manage and subagent

#### Scenario: A Codex `exec` call with one inner tool
- **WHEN** a Codex `exec` call's code calls only `tools.apply_patch`
- **THEN** the call has the kind of `apply_patch`, edit, as it shows as that tool (see tool-breakdown)

#### Scenario: A Codex `exec` call with several inner tools
- **WHEN** a Codex `exec` call's code calls `tools.exec_command` and `tools.write_stdin`
- **THEN** the call shows as `exec` and its kind is execute, as `exec` runs code

#### Scenario: A Codex MCP tool
- **WHEN** a Codex `function_call` item names the tool `js` in the `namespace` `mcp__cua_repl` (seen in 0.154–0.169), or an `exec` call's code calls only `tools.mcp__docs__search`
- **THEN** the tools are `mcp__cua_repl__js` and `mcp__docs__search`, tools of the MCP servers `cua_repl` and `docs`

#### Scenario: An unknown tool
- **WHEN** a session calls a tool its source does not list, such as Claude Code's `bash`, which Claude Code refuses as a tool it does not have
- **THEN** its kind is other

#### Scenario: Copilot
- **WHEN** a Copilot session calls `bash`, `read_bash`, `stop_bash`, `view`, `edit`, `create`, `grep`, `glob`, `web_fetch`, `task`, `read_agent`, `skill`, `run_custom_setup_step`, `run_setup` and `report_progress` (seen in logs of 2026-11-14 to 2026-11-21)
- **THEN** their kinds are execute, execute output, execute, read, edit, write, search, search, fetch, subagent, subagent, manage, environment setup, environment setup and other

#### Scenario: A Copilot MCP tool
- **WHEN** a Copilot session started the MCP server `github-mcp-server` in a setup step, and calls `github-mcp-server-issue_read`
- **THEN** the tool is a tool of the MCP server `github-mcp-server`

### Requirement: Input of a call
The Claude Code, Codex and Copilot sources SHALL give each tool call a short head of its input: at most 60 characters of one line, taken from the field the source's README names for the tool, or none.

#### Scenario: A Claude Code command
- **WHEN** a `Bash` call's `input.command` is `cargo test -p tokprof --test golden` followed by more lines
- **THEN** its input head is `cargo test -p tokprof --test golden`

#### Scenario: A Claude Code file and a search
- **WHEN** a `Read` call has `input.file_path` and a `Grep` call has `input.pattern`
- **THEN** the head of the `Read` is the path as the tool's target shows it, and the head of the `Grep` is the pattern

#### Scenario: An MCP tool
- **WHEN** an MCP tool's input holds string values
- **THEN** its input head is the first of them

#### Scenario: Codex
- **WHEN** an `exec_command` call has `cmd`, and a `web_search_call` has `action.query`
- **THEN** their input heads are the first line of `cmd` and the query

#### Scenario: Copilot
- **WHEN** a Copilot `bash` call's `arguments` have `command`, a `view` call's have `path`, and a `run_custom_setup_step` call's have `name`
- **THEN** their input heads are the first line of the command, the path and the step's name, such as `Install the JDK`

### Requirement: Connected MCP servers
The Claude Code and Copilot sources SHALL tell the MCP servers a session had. For Claude Code these are the servers of the tools in its tool lists, and of the tools named in its deferred-tool lines; a tool named `mcp__<server>__<tool>` names its server. For Copilot they are the servers its setup steps started (see Copilot environment setup). Other sources tell none.

#### Scenario: A tool list
- **WHEN** an `api-request-shape` line's tool list holds `mcp__linear__list_issues`
- **THEN** the session had the server `linear`

#### Scenario: Deferred tools
- **WHEN** an `attachment` line of `type` `deferred_tools_delta` has `mcp__sentry__search_issues` in `addedNames` (seen in 2.1.284–2.1.328)
- **THEN** the session had the server `sentry`

#### Scenario: Codex, pi and OpenCode
- **WHEN** a session comes from Codex, pi or OpenCode
- **THEN** its source tells no connected servers, whatever MCP tools it called

#### Scenario: Copilot
- **WHEN** a Copilot log's setup steps are `Start 'runtime-tools' MCP server`, `Start 'playwright' MCP server` and `Start 'github-mcp-server' MCP server`
- **THEN** the session had the servers `runtime-tools`, `playwright` and `github-mcp-server`

## ADDED Requirements

### Requirement: Copilot log folders
The GitHub Copilot coding agent's logs SHALL be the files named `<session id>.sse` or `<session id>.sse.gz` at any depth under `$TOKPROF_COPILOT_DIR`, else under `copilot` in the profile directory (see profile-export), by default `~/tokprof/copilot`. A `.gz` file is decompressed while it is read. The agent writes no local logs: the user saves each session's log there.

#### Scenario: A folder per day
- **WHEN** `~/tokprof/copilot/2026-11-15/5f0c2a9e-1b7d-4e3a-9c6f-2d8b7a4e1c03.sse.gz` exists
- **THEN** it is the log of session `5f0c2a9e-1b7d-4e3a-9c6f-2d8b7a4e1c03`

#### Scenario: A log given as the target
- **WHEN** `tokprof scan ~/Downloads/5f0c2a9e-1b7d-4e3a-9c6f-2d8b7a4e1c03.sse` runs on a log outside the folder whose first data line is a `chat.completion.chunk`
- **THEN** it X-rays that log as a Copilot session

### Requirement: Copilot session records
A Copilot session's record SHALL be the file `<session id>.json` anywhere under the same folder, whose `id` is the session id. It gives the session's model, title (`name`) and billed credits. A session without a record has the model `unknown`, no title and no cost. A JSON file whose `id` is not its name is no record.

#### Scenario: Records in another folder
- **WHEN** the folder holds `logs/<id>.sse.gz` and `meta/<id>.json`
- **THEN** the record of `meta/` belongs to the log of `logs/`

#### Scenario: No record
- **WHEN** a log has no record in the folder
- **THEN** its session has the model `unknown` and no cost, and its requests and calls are read as usual

### Requirement: Copilot log lines
A Copilot log SHALL be read as server-sent events: each line `data: <JSON object>` is a record, and every other line, `data: [DONE]` and data that is not a JSON object is skipped. A chunk's `created` is in epoch seconds, as a number or a string, or in epoch milliseconds when it has 13 digits or more (seen in logs of 2026-11-14 to 2026-11-21).

#### Scenario: Milliseconds
- **WHEN** a chunk has `created` 1800000099523 and the next one `created` "1800000100"
- **THEN** their times are 1800000099.523 s and 1800000100 s

### Requirement: Copilot requests
Each Copilot request SHALL come from the first chunk of each `id` whose calls are not all setup steps or markers (see Copilot environment setup). Its response ended at that chunk's `created`. It was sent at the latest time logged before it in its chain, else at its own time. The log gives no usage of the request (see cost-attribution, Requests without usage).

#### Scenario: A response with two calls
- **WHEN** a chunk of `id` `a1b2c3d4` makes two calls, and two later chunks of `id` `a1b2c3d4` repeat one call each with its result
- **THEN** the session has one request of `a1b2c3d4`, whose response made both calls

#### Scenario: Request times
- **WHEN** the last call a request waited for ended at 10:00:04, and the request's first chunk has the time 10:00:09
- **THEN** the request was sent at 10:00:04 and its response ended at 10:00:09

#### Scenario: The last answer
- **WHEN** a chunk of `id` `turn-22` has `finish_reason` `stop`, text and no calls
- **THEN** it is the session's last request

### Requirement: Copilot model
A Copilot request's model SHALL be its record's `model` after the last `:`, with each `.` of a `claude-` id written as `-`, so that it is the id tokprof and Claude Code use. The chunks' own `model`, empty or `unknown` in every log seen, is not read.

#### Scenario: A Claude model
- **WHEN** the record's model is `sweagent-capi:claude-fable-5.2`
- **THEN** every request of the session has the model `claude-fable-5-2`

#### Scenario: Another model
- **WHEN** the record's model is `sweagent-capi:gpt-6.2-sol`
- **THEN** the model is `gpt-6.2-sol`

### Requirement: Copilot's billed credits
A Copilot session's own cost SHALL be its record's `usage.amount` divided by 10^9, when `usage.type` is `ai_credits`: the credits GitHub billed, at $0.01 a credit. A record without `usage` gives no cost, shown `–`, not $0. GitHub bills the later sessions of a task on its first session, so a first session's credits can include those of later sessions with the same `task_id`, which then have no `usage`; tokprof takes each record as it is.

#### Scenario: A session's credits
- **WHEN** a record has `usage` `{"amount": 150000000000, "type": "ai_credits"}`
- **THEN** the session's reported cost is $1.50, 150 credits

#### Scenario: A later session of a task
- **WHEN** two records share a `task_id`, the first session's record has `usage` with 200 credits, and the later one's has no `usage`
- **THEN** the first session's reported cost is $2.00, the later one has no reported cost, and nothing is moved between them

### Requirement: Copilot calls and results
A Copilot tool call SHALL be each entry of `delta.tool_calls` of a request's chunk, known by its `id`. Its result is the `tool` object whose `tool_call_id` is that id, else the `content` of the chunk that repeats the call. That repeat chunk's `created` is the call's end. A call without either has no result and no end.

#### Scenario: A result and its repeat
- **WHEN** a `tool` object with `tool_call_id` `toolu_X` and 4,000 characters of `content` is followed by a chunk that repeats call `toolu_X`
- **THEN** the call's result is the 4,000 characters of the `tool` object, and the call ends at the chunk's time

#### Scenario: No tool object
- **WHEN** a call is repeated with `content` but no `tool` object names it
- **THEN** the repeated `content` is its result

### Requirement: Copilot call outcomes
A Copilot call SHALL have failed when its result's `toolTelemetry.properties.shell_error_category` is `command_nonzero_exit`, `invalid_input` or `shell_unknown`, and a safety check denied it when it is `kill_command_blocked`. `command_timeout`, a command still running when the call returned, is no failure. The error line of a failure with `metrics.exit_code` is the exit line (see Error line of a failed call).

#### Scenario: A failed test run
- **WHEN** a `bash` result has `shell_error_category` `command_nonzero_exit` and `exit_code` 1
- **THEN** the call failed

#### Scenario: A blocked command
- **WHEN** a `bash` result has `shell_error_category` `kill_command_blocked`
- **THEN** a permission rule denied the call

### Requirement: Copilot subagents
Copilot chunks and objects with an `agentId` SHALL belong to a subagent chain of the same session, one per `agentId`, except the markers. The chain is named `subagent: <name>` after the `name` of the `task` call whose begin marker comes right before the subagent's first user message, else `subagent`.

#### Scenario: A task subagent
- **WHEN** a `task` call with `name` `check-tests` is followed by its begin marker, a user message with `agentId` `sub-1`, and 12 chunks with that `agentId`
- **THEN** those chunks are requests of the chain `subagent: check-tests`, and the `task` call runs from its chunk to the chunk that repeats it

#### Scenario: A custom agent
- **WHEN** a log has the marker `selected-custom-agent-helper-start`
- **THEN** it marks no subagent, and the main conversation's requests stay in the chain `main`

### Requirement: Copilot environment setup
The calls `run_custom_setup_step` and `run_setup` SHALL be setup steps, made by no request, except a `run_setup` whose `name` starts with `▶️ Begin subagent: `, `⏹️ End subagent: ` or `❌ Failed subagent: `, which is a marker. A step starts at its first chunk and ends at the chunk that repeats it. A step `Start '<server>' MCP server` started that MCP server.

#### Scenario: A custom setup step
- **WHEN** a chunk at 10:00:00 calls `run_custom_setup_step` with `name` `Warm up the build`, and the repeat chunk comes at 10:04:15
- **THEN** it is a setup step of the kind environment setup that took 255 s

#### Scenario: A marker
- **WHEN** a `run_setup` call's `name` is `⏹️ End subagent: helper`
- **THEN** it is neither a call nor a request

### Requirement: Copilot prompts and turns
A Copilot session's typed message SHALL be the first user message of its main conversation, at the time of the log's first chunk. Later user messages are reminders. The main conversation's last response ends the session's one turn. A subagent's first user message is its brief.

#### Scenario: The prompt and the setup
- **WHEN** a log's first chunk, a setup step, is at 10:00:00, and the first user message comes after the setup steps
- **THEN** the session's typed message is at 10:00:00

#### Scenario: A request from the platform
- **WHEN** a user message of the platform that asks for the pull request's title and description comes before the last response
- **THEN** it is a reminder, and no turn starts there

### Requirement: Copilot project and title
A Copilot session's project SHALL be `github.com/<owner>/<repo>` after the setup step `Clone repository <owner>/<repo>`, else empty; it is not a folder on this machine. Its title is its record's `name`.

#### Scenario: The repository
- **WHEN** a log's setup step is `Clone repository octo/app`
- **THEN** the session's project is `github.com/octo/app`

### Requirement: Sizes the log gives
A source SHALL give the size in tokens that its log names for a piece of content, as the agent counted it, where the log names one. The Copilot source gives a tool result's `toolTelemetry.metrics.result_token_count` (seen in logs of 2026-11-15 to 2026-11-21). Other sources give none. These sizes are used only for requests without usage (see tool-breakdown, Sizes per call).

#### Scenario: A result with its tokens
- **WHEN** a Copilot `tool` object has `metrics.result_token_count` 1787
- **THEN** the result's size is 1,787 tokens

#### Scenario: An older log
- **WHEN** a Copilot `tool` object has no `result_token_count`
- **THEN** the result has no size
