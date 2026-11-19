# Spec Delta

## ADDED Requirements

### Requirement: Call times
The Claude Code and Codex sources SHALL give each tool call the time of the line that made it and the time of the line that brought its result. A call without a result line has no end. pi and OpenCode give no tool calls, so they give no call times.

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

### Requirement: Calls that return before their work ends
A source SHALL mark a call that returned while the work it started went on. Only Claude Code tells this.

#### Scenario: A background command
- **WHEN** a `Bash` call's `user` line has `toolUseResult.backgroundTaskId` (seen in 2.1.284–2.1.328)
- **THEN** the call is marked as one that ran in the background

#### Scenario: A subagent in the background
- **WHEN** an `Agent` call's `user` line has `toolUseResult.isAsync` set to `true` and `toolUseResult.status` set to `async_launched` (seen in 2.1.284–2.1.328)
- **THEN** the call is marked as one that ran in the background

#### Scenario: Codex
- **WHEN** a Codex `exec_command` call returns while its command still runs, and later `write_stdin` calls read its output
- **THEN** no call is marked, and each call keeps its own times

### Requirement: Calls that ask the user
A source SHALL mark a call that asks the user a question. Claude Code and Codex tell this by the tool's name.

#### Scenario: A question
- **WHEN** a Claude Code call is an `AskUserQuestion` or `ExitPlanMode` call (seen in 2.1.310–2.1.326)
- **THEN** the call is marked as one that asked the user

#### Scenario: A Codex question
- **WHEN** a Codex call is a `request_user_input` or `request_user_input_async` call (the second seen in 0.159)
- **THEN** the call is marked as one that asked the user, though the second returns at once and the answer comes later as a message

### Requirement: Kind of a tool
Each source SHALL give each of its tools one kind: read, edit, write, delete, move, search, execute, execute output, fetch, subagent, think, switch mode, manage or other. Manage is a tool with which the agent manages itself and not the work. A tool the source does not know is other. The source's README lists the kind of each tool it knows.

#### Scenario: Claude Code
- **WHEN** a Claude Code session calls `Read`, `NotebookRead`, `Edit`, `MultiEdit`, `NotebookEdit`, `Write`, `Grep`, `Glob`, `LS`, `Bash`, `BashOutput`, `WebFetch`, `WebSearch`, `Agent`, `Task` and `SendMessage`
- **THEN** their kinds are read, read, edit, edit, edit, write, search, search, search, execute, execute output, fetch, fetch, subagent, subagent and subagent

#### Scenario: Claude Code's own tools
- **WHEN** a Claude Code session calls `TodoWrite`, `ExitPlanMode`, `EnterWorktree`, `Skill`, `ToolSearch`, `ScheduleWakeup`, `AskUserQuestion` and the API's `advisor`
- **THEN** their kinds are think, switch mode, switch mode, manage, manage, manage, manage and manage

#### Scenario: Codex
- **WHEN** a Codex session calls `exec_command`, `write_stdin`, `wait`, `apply_patch`, `view_image`, `spawn_agent` and the hosted `web_search_call`
- **THEN** their kinds are execute, execute output, execute output, edit, read, subagent and fetch, as `wait` waits for the output of a running code cell (seen in 0.147–0.167)

#### Scenario: Codex's own tools
- **WHEN** a Codex session calls `update_plan`, `request_user_input` and `sleep`
- **THEN** their kinds are think, manage and manage

#### Scenario: A Codex `exec` call with one inner tool
- **WHEN** a Codex `exec` call's code calls only `tools.apply_patch`
- **THEN** the call has the kind of `apply_patch`, edit, as it shows as that tool (see tool-breakdown)

#### Scenario: An unknown tool
- **WHEN** a session calls a tool its source does not list, such as `Monitor`
- **THEN** its kind is other

### Requirement: Input of a call
The Claude Code and Codex sources SHALL give each tool call a short head of its input: at most 60 characters of one line, taken from the field the source's README names for the tool, or none.

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

### Requirement: Connected MCP servers
The Claude Code source SHALL tell the MCP servers a session had: the servers of the tools in its tool lists, and of the tools named in its deferred-tool lines. A tool named `mcp__<server>__<tool>` names its server. Other sources tell none.

#### Scenario: A tool list
- **WHEN** an `api-request-shape` line's tool list holds `mcp__linear__list_issues`
- **THEN** the session had the server `linear`

#### Scenario: Deferred tools
- **WHEN** an `attachment` line of `type` `deferred_tools_delta` has `mcp__sentry__search_issues` in `addedNames` (seen in 2.1.284–2.1.328)
- **THEN** the session had the server `sentry`

#### Scenario: Codex, pi and OpenCode
- **WHEN** a session comes from Codex, pi or OpenCode
- **THEN** its source tells no connected servers, whatever MCP tools it called
