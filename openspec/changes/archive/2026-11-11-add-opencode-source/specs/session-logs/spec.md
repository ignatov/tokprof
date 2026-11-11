## MODIFIED Requirements

### Requirement: Log locations
tokprof SHALL look for session logs only in the agents' log locations, at any depth, as Claude Code and Codex log locations, Pi log locations and OpenCode stores name them. A location that does not exist adds no logs and causes no error.

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

### Requirement: Session id and project
Each log SHALL carry a session id and a project read from the log, as Claude Code and Codex session ids, Pi session id and project and OpenCode sessions say. The project is written with `~` for the home directory.

#### Scenario: Claude Code subagent transcript
- **WHEN** a subagent transcript's lines carry the parent's `sessionId`
- **THEN** its requests are reported under the parent session's id

#### Scenario: Codex subagent thread
- **WHEN** a Codex subagent rollout of session `M` is read
- **THEN** its requests are reported under `M`'s id, not under its own thread id
- **AND** `tokprof misses` lists its misses with `M`'s short id and the thread's chain, such as `subagent:review`

#### Scenario: Codex thread whose parent is not on disk
- **WHEN** a Codex rollout names the parent thread `P`, and no rollout of `P` is found
- **THEN** its requests are reported under `P`

#### Scenario: No id in the log
- **WHEN** a Claude Code transcript has no `sessionId`, or a Codex rollout has no `session_meta` id
- **THEN** the id comes from the file name: the transcript's name without `.jsonl`, or the 36-character thread id at the end of the rollout's name

#### Scenario: Project under the home directory
- **WHEN** the first `cwd` is `/Users/me/src/app` and the home directory is `/Users/me`
- **THEN** the project is `~/src/app`

### Requirement: Compaction marker lines
The compaction markers SHALL be Claude Code `system` lines with subtype `compact_boundary` (seen in 2.1.284–2.1.312), which apply to every chain of the transcript, Codex `compacted` lines or `event_msg` events whose type contains `compact` (none seen yet), pi `compaction` lines (seen in pi 0.84.2), and the OpenCode compaction messages of OpenCode compactions.

#### Scenario: pi compaction
- **WHEN** a pi `compaction` line comes between two answered requests
- **THEN** the later request is marked

#### Scenario: OpenCode compaction
- **WHEN** an OpenCode `compaction` message with `status` `completed` comes between two answered requests of a session
- **THEN** the later request is marked

### Requirement: Agent-reported cost
tokprof SHALL take a session's own cost, as the agent counted it, where its log reports one: Claude Code's from Claude Code's cost counter, pi's from Pi's own cost, OpenCode's from OpenCode's own cost. cost-attribution compares it with tokprof's total in `tokprof summary`; it is never used as a price. Codex logs no such figure.

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

### Requirement: Agent selection
`--agent` SHALL choose whose logs every command reads, as a comma-separated list of `claude`, `codex`, `pi` and `opencode`; the default is `claude,codex,pi,opencode`.

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

### Requirement: No logs found
A command that has no log to read SHALL fail with an error: `summary`, `misses`, `whatif` and `flame` with `no session logs found for <agents>`, where `<agents>` are the `--agent` values joined by `, `; the default command, `scan` and `handoff` without a target, and `ui`, with `no session logs found`.

#### Scenario: Report without logs
- **WHEN** `tokprof --agent claude summary` finds no Claude Code transcript
- **THEN** it fails with `no session logs found for claude`

#### Scenario: Session filter that matches no path
- **WHEN** no session id starts with the `--session` text and the default agents are selected
- **THEN** the report fails with `no session logs found for claude, codex, pi, opencode`

#### Scenario: Latest session without logs
- **WHEN** `tokprof` runs and the selected agents have no main log
- **THEN** it fails with `no session logs found`

#### Scenario: Commands that do not fail
- **WHEN** `sessions` or `bench` finds no log
- **THEN** `sessions` prints only its header line and `bench` prints `read  in 0.0s` and its empty report, both with exit status 0

## ADDED Requirements

### Requirement: OpenCode stores
OpenCode's sessions SHALL be read from its SQLite stores in `$XDG_DATA_HOME/opencode`, by default `~/.local/share/opencode`: `opencode.db` and `opencode-<channel>.db`, but not `opencode-next.db`, which opencode 2.x imports into `opencode.db`. `$OPENCODE_DB` names one more store, relative to that directory unless absolute. Each session in a store is one log, whose size is the bytes of its message rows in the layout that holds more (seen in opencode 1.18.16, 1.19.3 and 2.2.4).

#### Scenario: The default store
- **WHEN** `~/.local/share/opencode/opencode.db` holds 2 sessions
- **THEN** 2 OpenCode logs are found, and the line about the logs read says `opencode 2 files` with the size of their message rows

#### Scenario: Data directory set by the environment
- **WHEN** `XDG_DATA_HOME=/data` is set
- **THEN** OpenCode stores are read from `/data/opencode` only

#### Scenario: A store OpenCode imported
- **WHEN** the data directory holds `opencode.db` and `opencode-next.db`
- **THEN** only `opencode.db` is read

#### Scenario: A store being written
- **WHEN** OpenCode has written a step to `opencode.db-wal` and not yet to `opencode.db`
- **THEN** the step is read, and the store is not written

#### Scenario: A file that is not a store
- **WHEN** `opencode.db` is not a SQLite file, or has neither the `session` nor the `session_v2` table
- **THEN** it adds no logs and causes no error

#### Scenario: A store given as the target
- **WHEN** `tokprof ~/.local/share/opencode/opencode.db` runs
- **THEN** it X-rays the main session of that store that was active last

### Requirement: OpenCode sessions
An OpenCode session SHALL be a row of the `session` table (opencode 1.x) or of `session_v2` (2.x); a session in both is one, read from `session_v2`. Its id is `id`, its project `directory`, its title `title`. Its messages are those of `message` and `session_message`, one per id, the `message` row kept when both have it. Messages created before the session are a fork's copies and are not read.

#### Scenario: A session copied to the 2.x tables
- **WHEN** a session's 4 assistant messages are in `message`, and 2.x copied 3 of them to `session_message` under the same ids and added 1 of its own
- **THEN** the session has 5 requests

#### Scenario: A fork
- **WHEN** a session created at 10:00 holds 3 messages created before 10:00 and 2 after
- **THEN** only the 2 messages after 10:00 are read

#### Scenario: Short id
- **WHEN** a session's id is `ses_e37fb3ed9ffeqcemWVth90sWuq`
- **THEN** its short id is `ses_e37fb3ed9ffeqcem`, the first 20 characters

### Requirement: OpenCode subagents
An OpenCode session with a `parent_id` SHALL be a subagent log of the topmost session above it that is in the store, as the `task` tool starts one. Its chain is `subagent: <title>`, without the title's ` (@<agent> subagent)`. A session whose parent is not in the store is a main session.

#### Scenario: A subagent
- **WHEN** session `S` has `parent_id` `M`, the title `List directory files (@explore subagent)` and 2 requests
- **THEN** `S` is a subagent log of `M`, and its 2 requests are in the chain `subagent: List directory files` of `M`'s X-ray

### Requirement: OpenCode requests and usage
Each OpenCode API request SHALL come from a `session_message` row of type `assistant` with non-zero `tokens`, or from a `message` row whose `role` is `assistant`: one request per `step-finish` part of the message, else one from its own non-zero `tokens`. `tokens.input` is uncached input, `cache.read` reads, `cache.write` short-lifetime writes, and `output` plus `reasoning` the output, `reasoning` its share. The model is `model.id` or `modelID` after its last `/`.

#### Scenario: Usage fields
- **WHEN** a step has `input` 6762, `output` 49, `reasoning` 5, `cache.read` 1792 and `cache.write` 0
- **THEN** the request has 6,762 uncached, 1,792 cache-read, 0 cache-written and 54 output tokens, 5 of them reasoning

#### Scenario: No usage
- **WHEN** an assistant message that failed before its first token has all its `tokens` at 0
- **THEN** it adds no request

#### Scenario: Request times
- **WHEN** an assistant message has `time.created` 10:00:01 and `time.completed` 10:00:04
- **THEN** its request is sent at 10:00:01 and ends at 10:00:04

#### Scenario: A model behind a router
- **WHEN** a step's model is `anthropic/claude-sonnet-4.5`
- **THEN** the request's model is `claude-sonnet-4.5`

### Requirement: OpenCode compactions
An OpenCode compaction marker SHALL be a `session_message` row of type `compaction` with `status` `completed`, of the kind of its `reason`, or a `message` row of a user whose parts hold a `compaction` part, `auto` when the part's `auto` is true, else `manual`. The `tokens` of a `compaction` row, completed or failed, and the steps of an assistant `message` with `summary` true are compaction calls. The request after a compaction call is marked, not the call.

#### Scenario: A compaction in 2.x
- **WHEN** a `compaction` row with `reason` `auto`, `status` `completed` and `tokens` `input` 9000 and `output` 800 comes between two assistant messages
- **THEN** it is a compaction call with 9,000 uncached and 800 output tokens, a marker of kind `auto`, and the later assistant's request is marked

#### Scenario: A compaction in 1.x
- **WHEN** a user message has a `compaction` part with `auto` false and the next assistant has `summary` true
- **THEN** the marker's kind is `manual`, the summary's steps are compaction calls, and the request after them is marked

### Requirement: OpenCode's own cost
An OpenCode session's own cost SHALL be the sum of `cost` over the requests and compaction calls of OpenCode requests and usage and OpenCode compactions.

#### Scenario: Free model
- **WHEN** every step of a session is from `big-pickle` with `cost` 0
- **THEN** the session's reported cost is $0.00 and its requests stay unpriced unless the user's prices name the model
