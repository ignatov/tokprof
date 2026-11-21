## MODIFIED Requirements

### Requirement: Sizes per call
`avg result` and `avg input` SHALL be the mean tokens of a row's tool results and of its call inputs, each block counted once, at the size it had in the first request that held it. For a request without usage (see cost-attribution), a block's size is the one its log gives (see session-logs, Sizes the log gives), and a block without one is left out of the mean. A row whose sizes the log cannot rebuild shows `–`.

#### Scenario: A large read
- **WHEN** one `Read` returned a 16,900-token file
- **THEN** its row shows `1` call and `16.9k` as `avg result`

#### Scenario: Sizes a Copilot log gives
- **WHEN** a Copilot `bash` row has 3 results with `result_token_count` 1000, 1500 and 1400, and one result of a log without it
- **THEN** the row shows `4` calls and `1.3k` as `avg result`, the mean of the three, and `–` as `avg input`

### Requirement: Groups
Every tool call SHALL belong to one group, by where its tool comes from. An MCP tool belongs to `mcp <server>`, one group per server. Any other tool belongs to the group of its kind (see session-logs, Kind of a tool): `files` for read, edit, write, delete and move; `search`; `shell` for execute and execute output; `web` for fetch; `subagents` for subagent; `agent` for think, switch mode and manage; `environment` for environment setup; `other` for the rest.

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

#### Scenario: Copilot setup steps
- **WHEN** a Copilot session ran 8 `run_custom_setup_step` steps and 4 `run_setup` steps
- **THEN** the 12 calls belong to the group `environment`

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
- **THEN** the command fails with a usage error that lists `files, search, shell, web, subagents, agent, environment, mcp, mcp <server>, other`

#### Scenario: Without the call log
- **WHEN** the user passes `--failed`, `--group` or `--tool` without `--calls`
- **THEN** the command fails with a usage error that says they need `--calls`

## ADDED Requirements

### Requirement: Calls no request made
Calls that a session's log gives outside any request, such as Copilot's setup steps (see session-logs, Copilot environment setup), SHALL count in the tool table, the groups table and the call log like other calls, with their times and outcomes. They hold no spend, and their call log rows show an empty `req`.

#### Scenario: Setup steps in the call log
- **WHEN** a Copilot session ran 8 setup steps before its first request
- **THEN** the call log starts with their 8 rows in the group `environment`, each with an empty `req` and its duration under `took`

#### Scenario: Setup steps in the tool table
- **WHEN** 867 Copilot sessions ran 4,895 `run_custom_setup_step` calls
- **THEN** the tool table has a row `run_custom_setup_step` with 4,895 calls and their time

### Requirement: Spend of calls without usage
A row whose calls all come from requests without usage, or from no request, SHALL show `–` as its `rent`, `trips` and `after`, and get no hint, since hints need spend. The total line and the failures line count spend of requests with usage only.

#### Scenario: A Copilot row
- **WHEN** `tokprof --agent copilot tools` shows the row `bash gradlew test` with 300 calls, 40 of them failed
- **THEN** its `rent`, `trips` and `after` cells read `–`, its `failed` cell reads `40`, and it has no hint
