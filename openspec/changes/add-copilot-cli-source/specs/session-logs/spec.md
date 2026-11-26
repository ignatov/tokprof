# Spec Delta

## MODIFIED Requirements

### Requirement: Agent selection
`--agent` SHALL choose whose logs every command reads, as a comma-separated list of `claude`, `codex`, `pi`, `opencode`, `copilot` and `copilot-cloud`; the default is `claude,codex,pi,opencode,copilot`. `copilot` is the GitHub Copilot CLI, whose logs are local (see Copilot CLI log locations). `copilot-cloud` is the GitHub Copilot coding agent, whose logs the user saves (see Copilot log folders); every requirement that says Copilot without CLI is about it.

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
- **THEN** no session of the Copilot coding agent is listed, and `tokprof` with no arguments never picks one as the latest session

#### Scenario: Copilot CLI by default
- **WHEN** `tokprof sessions` runs without `--agent`, and `~/.copilot/session-state` holds session logs
- **THEN** the Copilot CLI sessions are listed with the agent `copilot`, and `tokprof` with no arguments picks one when it is the latest session

#### Scenario: The two Copilot agents
- **WHEN** `tokprof --agent copilot sessions` and `tokprof --agent copilot-cloud sessions` run
- **THEN** the first lists only Copilot CLI sessions, and the second only the saved sessions of the Copilot coding agent

### Requirement: Log locations
tokprof SHALL look for session logs only in the agents' log locations, at any depth, as Claude Code and Codex log locations, Pi log locations, OpenCode stores, Copilot CLI log locations and Copilot log folders name them. A location that does not exist adds no logs and causes no error.

#### Scenario: Default directories
- **WHEN** none of `CLAUDE_CONFIG_DIR`, `CODEX_HOME`, `PI_CODING_AGENT_DIR`, `PI_CODING_AGENT_SESSION_DIR`, `XDG_DATA_HOME`, `OPENCODE_DB` and `COPILOT_HOME` is set
- **THEN** logs are read from `~/.claude/projects`, `~/.codex/sessions`, `~/.codex/archived_sessions`, `~/.pi/agent/sessions` and `~/.copilot/session-state`, and sessions from the OpenCode stores in `~/.local/share/opencode`

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
- **WHEN** `tokprof --agent copilot-cloud sessions` runs and none of `TOKPROF_COPILOT_DIR` and `TOKPROF_DIR` is set
- **THEN** the logs of the Copilot coding agent are read from `~/tokprof/copilot`

### Requirement: Agent-reported cost
tokprof SHALL take a session's own cost, as the agent counted it, where its log reports one: Claude Code's from Claude Code's cost counter, pi's from Pi's own cost, OpenCode's from OpenCode's own cost, the Copilot CLI's from Copilot CLI's own cost, Copilot's from Copilot's billed credits. cost-attribution compares it with tokprof's total in `tokprof summary`, or shows it alone for sessions without usage; it is never used as a price. Codex logs no such figure.

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

### Requirement: Compaction marker lines
The compaction markers SHALL be Claude Code `system` lines with subtype `compact_boundary` (seen in 2.1.284–2.1.312), which apply to every chain of the transcript, Codex `compacted` lines or `event_msg` events whose type contains `compact` (none seen yet), pi `compaction` lines (seen in pi 0.84.2), the OpenCode compaction messages of OpenCode compactions, and Copilot CLI `session.compaction_complete` lines whose `success` is `true` (seen in Copilot CLI 1.0.115).

#### Scenario: pi compaction
- **WHEN** a pi `compaction` line comes between two answered requests
- **THEN** the later request is marked

#### Scenario: OpenCode compaction
- **WHEN** an OpenCode `compaction` message with `status` `completed` comes between two answered requests of a session
- **THEN** the later request is marked

#### Scenario: Copilot CLI compaction
- **WHEN** a Copilot CLI `session.compaction_complete` line with `success` `true` comes between two answered requests of the main conversation
- **THEN** the later request is marked

#### Scenario: A Copilot CLI compaction that failed
- **WHEN** a `session.compaction_complete` line has `success` `false`
- **THEN** it marks no request

## ADDED Requirements

### Requirement: Copilot CLI log locations
The Copilot CLI's session logs SHALL be the files `<session id>/events.jsonl` in `$COPILOT_HOME/session-state`, by default `~/.copilot/session-state` (seen in Copilot CLI 1.0.85–1.0.116). A session folder without `events.jsonl` is no session. No other file of the folder is a log.

#### Scenario: A session folder
- **WHEN** `~/.copilot/session-state/8d362bc6-27c5-4206-97e9-2807b9610b79/events.jsonl` exists
- **THEN** it is the log of a Copilot CLI session

#### Scenario: Directory set by the environment
- **WHEN** `COPILOT_HOME=/data/copilot` is set
- **THEN** Copilot CLI logs are read from `/data/copilot/session-state` only

#### Scenario: A folder without a log
- **WHEN** a session folder holds `workspace.yaml`, `checkpoints/` and `files/` and no `events.jsonl`
- **THEN** it adds no session

#### Scenario: A log given as the target
- **WHEN** `tokprof scan /backup/8d362bc6/events.jsonl` runs on a log outside the folder whose first line has the `type` `session.start` and the `data.producer` `copilot-agent`
- **THEN** it X-rays that log as a Copilot CLI session

### Requirement: Copilot CLI session id, project and title
A Copilot CLI session's id SHALL be the `data.sessionId` of its `session.start` line, else the name of its folder. Its short id is the first 8 characters. Its project is the `data.context.cwd` of that line. Its title is the `name` of the `workspace.yaml` beside the log, when that file has one.

#### Scenario: A session
- **WHEN** a log's `session.start` line has `sessionId` `8d362bc6-27c5-4206-97e9-2807b9610b79` and `context.cwd` `/Users/me/src/app`, with `/Users/me` the home directory
- **THEN** the session is `8d362bc6` of the project `~/src/app`

#### Scenario: A title
- **WHEN** the `workspace.yaml` beside the log has the line `name: Add Copilot Agent Support`
- **THEN** the session's title is `Add Copilot Agent Support`

#### Scenario: No workspace file
- **WHEN** the folder has no `workspace.yaml`, or the file has no `name`
- **THEN** the session has no title

### Requirement: Copilot CLI requests and usage
Each `session.usage_record` line SHALL be one request, with `data.usage` read so (seen in Copilot CLI 1.0.106–1.0.116): `inputTokens` less `cacheReadTokens` and `cacheWriteTokens` is uncached input, never below 0; `cacheReadTokens` reads; `cacheWriteTokens` writes; `outputTokens` output, reasoning included. Its response is the `assistant.message` line whose `data.apiCallId` is the record's `accounting.usageId`. Its model, times, cache lifetime and version are as the scenarios say.

#### Scenario: Usage fields
- **WHEN** a record has `inputTokens` 176733, `cacheReadTokens` 174203, `cacheWriteTokens` 2528 and `outputTokens` 4773, of which `reasoningTokens` 4440
- **THEN** the request has 2 uncached, 174,203 cache-read, 2,528 cache-written and 4,773 output tokens

#### Scenario: Model
- **WHEN** a record's `model` is `claude-fable-5.2`
- **THEN** the request's model is `claude-fable-5-2`, by the rule of Copilot model for an id without a `:`

#### Scenario: Times
- **WHEN** a record's line has the `timestamp` `2026-11-25T22:05:49.917Z` and `duration` 59272
- **THEN** the response ended at 22:05:49.917 and the request was sent 59,272 ms before, at 22:04:50.645

#### Scenario: Cache lifetime
- **WHEN** a record's `cacheTtlSeconds`, else its `cacheExpiresAt` less its line's `timestamp`, is over 1,800 seconds
- **THEN** its written tokens are long-lifetime writes, and short-lifetime ones otherwise, as with the 300 seconds every record seen has

#### Scenario: Version
- **WHEN** the log's `session.start` line has `copilotVersion` `1.0.116`
- **THEN** every request of the log carries the agent version `1.0.116`

#### Scenario: A model changed in the session
- **WHEN** the records of one log name `claude-sonnet-5.5` and then, after a `session.model_change` line, `claude-fable-5.2`
- **THEN** each request has the model of its own record

### Requirement: Copilot CLI responses without a usage record
An `assistant.message` line with an `apiCallId` that no usage record names SHALL be a request without usage (see cost-attribution, Requests without usage), of its `data.model`, ended at its `timestamp` and sent at the `assistant.turn_start` line before it (every response of Copilot CLI 1.0.85). An `assistant.message` line without an `apiCallId` is no request: the CLI wrote it itself, and its calls are calls no request made (see tool-breakdown).

#### Scenario: A log of an older version
- **WHEN** a log of Copilot CLI 1.0.85 holds 5 `assistant.message` lines with an `apiCallId` and no `session.usage_record` line
- **THEN** the session has 5 requests without usage, no tokens and no cost

#### Scenario: A message the CLI wrote
- **WHEN** an `assistant.message` line has no `apiCallId` and no `model`, and one `read_bash` tool request
- **THEN** it adds no request, and the `read_bash` call counts in the tool tables as a call no request made

### Requirement: Copilot CLI's own cost
A Copilot CLI request's own cost SHALL be its record's `copilotUsage.totalNanoAiu` divided by 10^9, the credits GitHub billed for it, at $0.01 a credit (seen in Copilot CLI 1.0.106–1.0.116). A session's own cost is the sum over its requests and compaction calls, subagents included. A request without `copilotUsage` has no own cost, and a session without any has none, shown `–`, not $0. The counter's name is `GitHub's billed credits`.

#### Scenario: A request's credits
- **WHEN** a record has `copilotUsage.totalNanoAiu` 25105660000
- **THEN** the request's own cost is 25.10566 credits, $0.25

#### Scenario: A session's credits
- **WHEN** a session's 6 records carry 20.61502 credits together
- **THEN** the session's reported cost is $0.21, and `tokprof summary` compares it with tokprof's cost of the same session

#### Scenario: The session's own totals
- **WHEN** a log also holds `session.usage_checkpoint` or `session.shutdown` lines with a `totalNanoAiu`
- **THEN** they are not read, and the session's own cost is still the sum over its records

### Requirement: Copilot CLI compaction calls
The `data.compactionTokensUsed` of a `session.compaction_complete` line SHALL be one compaction call (seen in Copilot CLI 1.0.115), with its usage and credits read as a usage record's, its `model` as the model and the line's `timestamp` as the end of its response, sent `duration` ms before. A line without `compactionTokensUsed` adds no request.

#### Scenario: A compaction call
- **WHEN** a `session.compaction_complete` line carries `compactionTokensUsed` with `inputTokens` 161982, `cacheReadTokens` 160307, `cacheWriteTokens` 1671, `outputTokens` 5181, `model` `claude-sonnet-5.5` and `copilotUsage.totalNanoAiu` 7202620000
- **THEN** it is a compaction call of `claude-sonnet-5-5` with 4 uncached, 160,307 cache-read, 1,671 cache-written and 5,181 output tokens, and an own cost of 7.20262 credits

### Requirement: Copilot CLI calls and results
The Copilot CLI source SHALL give every tool request of an `assistant.message` line (`data.toolRequests`, with `toolCallId`, `name` and `arguments`) as a call of that response. The call starts at the `timestamp` of the `tool.execution_start` line with its `toolCallId` and ends at that of the `tool.execution_complete` line with it, whose `data.result.content` is its result (seen in Copilot CLI 1.0.85–1.0.116). A call without a `tool.execution_complete` line has no end.

#### Scenario: A call and its result
- **WHEN** a `tool.execution_start` line of a `bash` call has the `timestamp` `21:12:22.697Z`, and the `tool.execution_complete` line with the same `toolCallId` has `21:12:22.778Z`
- **THEN** the call starts at 21:12:22.697, ends at 21:12:22.778 and took 81 ms

#### Scenario: Two calls of one response
- **WHEN** an `assistant.message` line holds two `bash` tool requests
- **THEN** both are calls of that request, each with the times of its own lines

#### Scenario: A call in the background
- **WHEN** a result's `toolTelemetry.properties.executionMode` is `async`, or its `detached` is `true`
- **THEN** the call is marked as one that ran in the background (see Calls that return before their work ends)

#### Scenario: A question
- **WHEN** a call's tool is `ask_user`
- **THEN** the call is marked as one that asked the user (see Calls that ask the user)

#### Scenario: A session still running
- **WHEN** the last line of a log is a `tool.execution_start` line
- **THEN** its call has a start and no end

### Requirement: Copilot CLI call outcomes
A Copilot CLI call SHALL have the outcome its `tool.execution_complete` line gives (seen in Copilot CLI 1.0.106–1.0.116). With `data.success` `false`: `data.error.code` `rejected` means the user refused it, `denied` means a permission rule denied it, and any other code means it failed, with `data.error.message` as its error line. With `success` `true`, a `data.shellExecution.exitCode` other than 0 means it failed, with the exit code in its error line. Any other call worked.

#### Scenario: A refusal
- **WHEN** a result has `success` `false` and `error` `{"message": "The user rejected this tool call.", "code": "rejected"}`
- **THEN** the user refused the call

#### Scenario: A denial
- **WHEN** a result has `success` `false` and `error.code` `denied`
- **THEN** a permission rule denied the call

#### Scenario: A failure
- **WHEN** a result has `success` `false` and `error` `{"message": "Invalid delay: 900. Please supply a valid delay between 0 and 600 seconds.", "code": "failure"}`
- **THEN** the call failed, with that message as its error line

#### Scenario: A command that exited with an error
- **WHEN** a `bash` result has `success` `true` and `shellExecution.exitCode` 1
- **THEN** the call failed

### Requirement: Copilot CLI tool kinds and input heads
The Copilot CLI source SHALL give its tools the kinds and input heads of the scenarios below (see Kind of a tool and Input of a call; seen in Copilot CLI 1.0.85–1.0.116). A call whose `tool.execution_start` line has `mcpServerName` is the tool `<server>-<tool>` of that MCP server, with `mcpToolName` as the tool; its input head is the first string value of its arguments. The source tells no connected MCP servers.

#### Scenario: Kinds
- **WHEN** a Copilot CLI session calls `bash`, `read_bash`, `stop_bash`, `view`, `edit`, `create`, `grep`, `glob`, `web_fetch`, `task`, `read_agent`, `list_agents`, `skill`, `ask_user`, `vote_memory` and `task_complete`
- **THEN** their kinds are execute, execute output, execute, read, edit, write, search, search, fetch, subagent, subagent, subagent, manage, manage, manage and other

#### Scenario: Input heads
- **WHEN** a `bash` call's arguments have `command`, a `view` call's have `path`, a `grep` call's have `pattern`, a `web_fetch` call's have `url`, a `task` call's have `name` and a `skill` call's have `skill`
- **THEN** their input heads are the first line of the command, the path, the pattern, the URL, the name and the skill

#### Scenario: An MCP tool
- **WHEN** a `web_search` call's `tool.execution_start` line has `mcpServerName` `github-mcp-server` and `mcpToolName` `web_search`
- **THEN** the tool is `github-mcp-server-web_search`, a tool of the MCP server `github-mcp-server`

#### Scenario: An unknown tool
- **WHEN** a session calls a tool the source does not list
- **THEN** its kind is other and it has no input head

### Requirement: Copilot CLI subagents
The lines of a Copilot CLI log that carry an `agentId` SHALL be a subagent's chain in the same session, one chain per `agentId` (seen in Copilot CLI 1.0.116). The chain is named `subagent: <name>` after `data.agentDisplayName`, else `data.agentName`, of its `subagent.started` line, else `subagent`. Its requests, calls and credits count in the session. The `task` call whose `toolCallId` that line names is the call that started it.

#### Scenario: A subagent
- **WHEN** 14 `session.usage_record` lines carry the `agentId` `61182870-96f0-437a-a237-edebceaefce8`, and the `subagent.started` line with it has `agentDisplayName` `review-factory-template`
- **THEN** the session has the chain `subagent: review-factory-template` with 14 requests, and the main chain does not count them

#### Scenario: The brief
- **WHEN** a `user.message` line carries the subagent's `agentId`
- **THEN** it is the subagent's brief, the first message of its chain, and no typed message of the session

### Requirement: Copilot CLI prompts and turns
A `user.message` line without an `agentId` and without a `data.source` SHALL be a message the user typed, with its `timestamp` and its `data.content`; it starts a turn (seen in Copilot CLI 1.0.85–1.0.116). A `user.message` line with a `source` (`memory`, `autopilot`, `skill-<name>`, `agent-<session id>`) comes from the CLI and is a reminder. `data.transformedContent` is what the model read as that message.

#### Scenario: A typed message
- **WHEN** a `user.message` line has no `source` and the `content` `fix the failing test`
- **THEN** it is a typed message with that text

#### Scenario: A message typed while the agent works
- **WHEN** a `user.message` line without a `source` has `delivery` `steering`
- **THEN** it is a typed message too

#### Scenario: A message the CLI sent
- **WHEN** a `user.message` line has `source` `autopilot` and `isAutopilotContinuation` `true`
- **THEN** it is a reminder and starts no turn
