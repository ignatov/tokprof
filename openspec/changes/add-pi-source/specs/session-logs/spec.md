## MODIFIED Requirements

### Requirement: Log locations
tokprof SHALL look for session logs only in the agents' log directories, at any depth. Claude Code: files ending in `.jsonl` under `$CLAUDE_CONFIG_DIR/projects`, by default `~/.claude/projects`. Codex: files named `rollout-*.jsonl`, or `rollout-*.jsonl.zst` when zstd-compressed, under `$CODEX_HOME/sessions` and `$CODEX_HOME/archived_sessions`, by default under `~/.codex`. Compressed rollouts are decompressed while reading. pi: files ending in `.jsonl` under `$PI_CODING_AGENT_SESSION_DIR`, else `$PI_CODING_AGENT_DIR/sessions`, by default `~/.pi/agent/sessions` (seen in pi 0.84.2).

#### Scenario: Default directories
- **WHEN** none of `CLAUDE_CONFIG_DIR`, `CODEX_HOME`, `PI_CODING_AGENT_DIR` and `PI_CODING_AGENT_SESSION_DIR` is set
- **THEN** logs are read from `~/.claude/projects`, `~/.codex/sessions`, `~/.codex/archived_sessions` and `~/.pi/agent/sessions`

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

#### Scenario: pi session directory set by the environment
- **WHEN** `PI_CODING_AGENT_DIR=/data/pi` is set and `PI_CODING_AGENT_SESSION_DIR` is not
- **THEN** pi logs are read from `/data/pi/sessions` only

#### Scenario: pi store that is not a log
- **WHEN** a pi-durable app keeps its conversations in a SQLite file
- **THEN** it is not picked up when tokprof looks for session logs

### Requirement: Session id and project
Each log SHALL carry a session id and a project read from the log. The id is the `sessionId` of Claude Code `user` and `assistant` lines, the `id` of the Codex `session_meta` payload, or the `id` of the pi `session` line that starts the log. A Codex subagent rollout carries the id of the session it belongs to (see Sessions and subagent logs) instead of its own. The project is the first `cwd` of those Claude Code lines, of Codex `session_meta` and `turn_context` lines, or of the pi `session` line, written with `~` for the home directory.

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

#### Scenario: pi log without an id
- **WHEN** a pi log's first line has no `id`
- **THEN** the id is the UUID at the end of the file name, after the `_`, such as `01a00c3b-cc83-7a8d-a89b-262bd2f8ecac` in `2026-08-16T20-20-40-708Z_01a00c3b-cc83-7a8d-a89b-262bd2f8ecac.jsonl`

### Requirement: Compaction markers
A request SHALL be marked as following a compaction when a compaction marker comes after the previous answered request of its conversation (see Main conversations across chains) and no later than the request. Markers are Claude Code `system` lines with subtype `compact_boundary` (seen in 2.1.284–2.1.312), which apply to every chain of the transcript, Codex `compacted` lines or `event_msg` events whose type contains `compact` (none seen yet), and pi `compaction` lines (seen in pi 0.84.2). cache-misses counts the rebuild as expected.

#### Scenario: Claude Code compaction
- **WHEN** a `compact_boundary` line at 10:05 falls between answered requests at 10:00 and 10:10
- **THEN** the 10:10 request is marked

#### Scenario: Unanswered request after the boundary
- **WHEN** the first request after the boundary got no response
- **THEN** the next answered request is still marked

#### Scenario: Main chain that continues an earlier one
- **WHEN** unaudited `main` requests answered at 10:00 and 10:10 have a `compact_boundary` at 10:05 between them, and an audited `main` chain starts at 11:00
- **THEN** the 10:10 request is marked and the 11:00 request is not, since its conversation's previous answered request came after the boundary

#### Scenario: Codex compaction
- **WHEN** a `compacted` line comes before a rollout's next usage line
- **THEN** the request of that usage line is marked

#### Scenario: pi compaction
- **WHEN** a pi `compaction` line comes between two answered requests
- **THEN** the later request is marked

### Requirement: Agent-reported cost
tokprof SHALL take a Claude Code session's own cost counter from the `cost-state` lines in its transcript (seen in transcripts of Claude Code 2.1.284–2.1.312). The lines that share a `startTime` are one run of the counter, and lines without a `startTime` are one run together. The counter is the sum, over the runs, of the highest `totalCostUSD` in each. cost-attribution compares it with tokprof's total in `tokprof summary`. A pi session's own cost is the sum of `usage.cost.total` over the requests of Pi requests and usage (seen in pi 0.84.2). It is compared, never used as a price. Codex logs no such figure.

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

#### Scenario: pi's own cost
- **WHEN** a pi log's responses carry `usage.cost.total` of $0.0049, $0.0120 and $0.0031
- **THEN** the session's reported cost is $0.02

#### Scenario: pi's cost of a local model
- **WHEN** every response of a pi log is from `provider` `local` and carries `cost.total` 0
- **THEN** the session's reported cost is $0.00 and its requests stay unpriced unless the user's prices name the model

### Requirement: Agent selection
`--agent` SHALL choose whose logs every command reads, as a comma-separated list of `claude`, `codex` and `pi`; the default is `claude,codex,pi`.

#### Scenario: One agent
- **WHEN** `tokprof --agent codex sessions` runs
- **THEN** only Codex sessions are listed

#### Scenario: Unknown agent
- **WHEN** `--agent foo` is given
- **THEN** the command fails with `unknown agent "foo"` before reading any log

#### Scenario: pi only
- **WHEN** `tokprof --agent pi sessions` runs
- **THEN** only pi sessions are listed

## ADDED Requirements

### Requirement: Pi requests and usage
Each pi API request SHALL come from a `message` line whose `message.role` is `assistant` and whose `message.usage` has a non-zero `input`, `output`, `cacheRead` or `cacheWrite` (seen in pi 0.84.2). Every branch of the log's tree counts. `input` is uncached input, `cacheRead` reads, `cacheWrite` short-lifetime writes less `cacheWrite1h`, the long-lifetime ones, and `output` output, `reasoning` included. The model is the response's `model`. A request's time is the line's `timestamp`.

#### Scenario: Usage fields
- **WHEN** a response has `input` 13, `output` 77, `cacheRead` 1544 and `cacheWrite` 0
- **THEN** the request has 13 uncached, 1,544 cache-read, 0 cache-written and 77 output tokens

#### Scenario: Long-lifetime writes
- **WHEN** a response has `cacheWrite` 1342 and `cacheWrite1h` 1000
- **THEN** the request has 342 short-lifetime and 1,000 long-lifetime written tokens

#### Scenario: Stopped before the first token
- **WHEN** a response has `stopReason` `aborted` and all usage fields 0
- **THEN** it adds no request

#### Scenario: A branch in the tree
- **WHEN** a log has two responses with the same `parentId`, after the person went back with `/tree`
- **THEN** both are requests

### Requirement: Pi compaction calls
The `usage` of a pi `compaction` or `branch_summary` line SHALL be one compaction call, with its usage read as a response's (see Pi requests and usage). It uses the model of the latest response before it, else of the latest `model_change` line, else `unknown`.

#### Scenario: A compaction call
- **WHEN** a `compaction` line carries `usage` with `input` 1735 and `output` 897, after a response of `claude-opus-5-5`
- **THEN** it is a compaction call of `claude-opus-5-5` with 1,735 uncached and 897 output tokens

### Requirement: Pi forks
A pi request SHALL count once across a log and its forks. A fork's `session` line names the log it came from in `parentSession`, and the fork holds copies of that log's entries with their `id` and `timestamp` (`/fork`, `/clone` and `--fork` in pi 0.84.2). A copied entry counts in the log it came from, not in the fork, while that log is on disk, at its path or next to the fork.

#### Scenario: A fork
- **WHEN** a pi log's `session` line names log `A` in `parentSession`, and it holds 3 responses with the `id` and `timestamp` of responses of `A` and 1 of its own
- **THEN** the fork has 1 request and `A` keeps its 3

#### Scenario: A fork of a log that is gone
- **WHEN** the log a fork names in `parentSession` is not on disk
- **THEN** the copied responses are requests of the fork
