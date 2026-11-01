# Session Logs Specification

## Purpose
Find the session logs that Claude Code and Codex write on this machine and read them into sessions, chains and requests, with token usage and rebuilt prompts, for every other tokprof capability to analyze. It also owns the global flags that select which logs and requests a report covers.

## Requirements

### Requirement: Log locations
tokprof SHALL look for session logs only in the agents' log directories, at any depth. Claude Code: files ending in `.jsonl` under `$CLAUDE_CONFIG_DIR/projects`, by default `~/.claude/projects`. Codex: files named `rollout-*.jsonl`, or `rollout-*.jsonl.zst` when zstd-compressed, under `$CODEX_HOME/sessions` and `$CODEX_HOME/archived_sessions`, by default under `~/.codex`. Compressed rollouts are decompressed while reading.

#### Scenario: Default directories
- **WHEN** neither `CLAUDE_CONFIG_DIR` nor `CODEX_HOME` is set
- **THEN** logs are read from `~/.claude/projects`, `~/.codex/sessions` and `~/.codex/archived_sessions`

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

### Requirement: Sessions and subagent logs
tokprof SHALL treat a session as one main log together with the logs of the subagents it spawned; a subagent log is never a session of its own. In Claude Code, any transcript below a `subagents` directory is a subagent log, and `<session>/subagents/` belongs to `<session>.jsonl`. In Codex, a rollout whose `session_meta` names a `parent_thread_id` (seen in Codex 0.146–0.163) belongs to the thread with that id.

#### Scenario: Claude Code subagent transcripts
- **WHEN** `<project>/s1.jsonl` and `<project>/s1/subagents/agent-a1.jsonl` exist
- **THEN** session `s1` has one subagent log
- **AND** `agent-a1.jsonl` is neither listed as a session nor picked as the latest session

#### Scenario: Codex subagent thread
- **WHEN** a rollout's `session_meta` has the `parent_thread_id` `P`
- **THEN** it belongs to the session of the rollout whose thread id is `P`, whether that rollout is in `sessions/` or in `archived_sessions/`

#### Scenario: Parent log not on disk
- **WHEN** a Codex rollout names a parent thread whose rollout is not found
- **THEN** it is not listed as a session
- **AND** `summary`, `misses`, `whatif` and `flame` still count its requests

#### Scenario: Rollout without session_meta
- **WHEN** a rollout's first line is not a `session_meta` line with an `id`
- **THEN** the rollout is a main log

### Requirement: One-session target
`scan` and `handoff` SHALL work on the session their target names or, without a target, like the default command, on the main log modified most recently among the selected agents. A target is the path of an existing file, or else a session id prefix matched against the names of main logs, newest first: a name without its extension that starts with the text, or that contains `-` followed by it, as the thread id ends a Codex rollout name.

#### Scenario: Latest session
- **WHEN** `tokprof` runs without a command
- **THEN** it X-rays the main log with the newest modification time
- **AND** only the modification times of main logs count, not those of subagent logs

#### Scenario: Target is a file
- **WHEN** the target is the path of an existing file
- **THEN** that file is read wherever it lies, as a Codex rollout when its name matches `rollout-*.jsonl` or `rollout-*.jsonl.zst`, and as a Claude Code transcript otherwise, whatever `--agent` says

#### Scenario: Target is an id prefix
- **WHEN** `tokprof scan 3f951de0` runs and two main logs match
- **THEN** the one modified more recently is used

#### Scenario: Nothing matches
- **WHEN** no main log matches the target `abcdef`
- **THEN** the command fails with ``no session file or session id matches "abcdef"; see `tokprof sessions` ``

### Requirement: Session id and project
Each log SHALL carry a session id and a project read from the log. The id is the `sessionId` of Claude Code `user` and `assistant` lines, or the `id` of the Codex `session_meta` payload. The project is the first `cwd` of those Claude Code lines, or of Codex `session_meta` and `turn_context` lines, written with `~` for the home directory. A short id is the first 8 characters of the id.

#### Scenario: Claude Code subagent transcript
- **WHEN** a subagent transcript's lines carry the parent's `sessionId`
- **THEN** its requests are reported under the parent session's id

#### Scenario: Codex subagent thread
- **WHEN** a Codex subagent rollout is read
- **THEN** its requests are reported under its own thread id, not under its parent's

#### Scenario: No id in the log
- **WHEN** a Claude Code transcript has no `sessionId`, or a Codex rollout has no `session_meta` id
- **THEN** the id comes from the file name: the transcript's name without `.jsonl`, or the 36-character thread id at the end of the rollout's name

#### Scenario: Project under the home directory
- **WHEN** the first `cwd` is `/Users/me/src/app` and the home directory is `/Users/me`
- **THEN** the project is `~/src/app`

### Requirement: Chains
tokprof SHALL split a session's requests into chains, one per cache lineage, each with a name and a kind: main, subagent or side. Claude Code requests with request-audit lines form one chain per `agentId` and `querySource` pair; requests without them form chains by `agentId` and `isSidechain`, apart from the audited ones. Each Codex rollout is one chain.

#### Scenario: Claude Code main conversation
- **WHEN** an `api-request` in a main transcript has no `agentId` and a `querySource` of `repl_main_thread`, `sdk`, `main` or none
- **THEN** its chain is `main`, of kind main

#### Scenario: Claude Code subagent stream
- **WHEN** the `querySource` is `agent:builtin:Explore`
- **THEN** the chain is `subagent:Explore`, of kind subagent, named after the part after the last `:`, so `agent:custom` gives `subagent:custom`
- **AND** a main-thread `querySource` with an `agentId`, or in a subagent transcript, gives the chain `subagent`

#### Scenario: Claude Code side streams
- **WHEN** the `querySource` contains `compact`
- **THEN** the chain is `compact`, of kind side
- **AND** any other `querySource` gives a side chain named after it

#### Scenario: Claude Code requests without audit lines
- **WHEN** a response has no `api-request` record
- **THEN** its chain is `subagent` when the line has an `agentId` or is in a subagent transcript, `sidechain` (kind side) when `isSidechain` is true, and `main` otherwise

#### Scenario: Codex rollout
- **WHEN** a rollout is a subagent thread whose `session_meta.source.subagent` is `{"other": "guardian"}`
- **THEN** its chain is `subagent:guardian`: the role is that field when it is a string, or the first string inside it when it is an object, and the chain is `subagent` when there is no role
- **AND** the chain of any other rollout is `main`

### Requirement: Main conversations across chains
Main chains of one name that follow one another in time SHALL be read as one conversation, in time order, while main chains of one name that overlap in time stay separate conversations. A Claude Code session that spans the release that added request-audit lines thus keeps its unaudited and audited requests in one conversation. The cache state of session-xray and the returns of return-bench are read from these conversations.

#### Scenario: Session resumed after the upgrade
- **WHEN** a transcript has unaudited `main` requests at 10:00 and 10:20 and audited `main` requests at 10:30 and 10:31
- **THEN** they form one conversation of four requests in time order

#### Scenario: Overlapping chains
- **WHEN** another `main` chain of the same transcript has requests at 10:15 and 10:35
- **THEN** it is a separate conversation, and the first conversation keeps its four requests

### Requirement: Claude Code requests and usage
Each Claude Code API request SHALL come from the assistant lines that carry `message.usage` and a model, identified by `requestRef`, else `requestId`, else `message.id`; a line whose identity was seen before updates that request instead of adding one. `input_tokens` is uncached input, `cache_read_input_tokens` cache reads, `cache_creation_input_tokens` cache writes and `output_tokens` output, of which `output_tokens_details.thinking_tokens` is reasoning.

#### Scenario: Response written one block per line
- **WHEN** three assistant lines share one `requestId`
- **THEN** they make one request with the usage of the last line

#### Scenario: Synthetic message
- **WHEN** an assistant line's model is `<synthetic>` or empty
- **THEN** it adds no request

#### Scenario: Cache write lifetimes
- **WHEN** `usage.cache_creation` gives `ephemeral_5m_input_tokens` and `ephemeral_1h_input_tokens`
- **THEN** cache writes split into 5-minute and 1-hour writes
- **AND** any part of `cache_creation_input_tokens` that the two do not cover counts as 5-minute writes

#### Scenario: No lifetime split
- **WHEN** `usage.cache_creation` is missing
- **THEN** all cache writes count as 5-minute writes

#### Scenario: Advisor iterations
- **WHEN** `usage.iterations` has more than one entry (seen in Claude Code 2.1.284–2.1.312)
- **THEN** each iteration is one request with its own usage and model: the iteration's `model`, else the response's
- **AND** iterations of type `advisor_message` are advisor requests, outside the chain's cache lineage
- **AND** the top-level usage is not counted on top, except its thinking tokens, which go to the last iteration that is not an advisor one

### Requirement: Claude Code prompts
Claude Code prompts SHALL be rebuilt from request-audit lines (seen in Claude Code 2.1.300–2.1.312). An `api-request` keeps `keep` messages of the previous request of its stream and appends its `tail` hashes, matched to `api-request-blob` messages and to transcript `user` and `assistant` messages by the SHA-256 of their canonical JSON. Its `shapeHash` names the `api-request-shape` with the tools and system prompt. A response finds its request by `requestRef`.

#### Scenario: Transcript without request-audit lines
- **WHEN** a transcript, or the part of it written before the audit lines, has no `api-request` lines
- **THEN** its requests carry usage and model only
- **AND** cost-attribution reports their whole prompt as `unknown` content

#### Scenario: Message split across lines
- **WHEN** a response was written one content block per line under one `message.id`
- **THEN** a `tail` hash matches the merged message as well as any single line

#### Scenario: Hash not found
- **WHEN** a `tail` hash matches no blob and no transcript message
- **THEN** that message stays in the prompt as `unknown` content, which cost-attribution sizes as an average resolved message of the same prompt

#### Scenario: Request without a response
- **WHEN** an `api-request` has no assistant line with its `requestRef`, as when the request was cancelled or retried
- **THEN** it is not counted as a request
- **AND** later requests of its stream still apply their `keep` to its prompt

#### Scenario: Request parameters
- **WHEN** an `api-request` carries `params`
- **THEN** `output_config.effort`, `thinking`, `betas` (in any order), `context_management`, `speed` and the line's `version` are kept for the cause rules of cache-misses

### Requirement: Codex requests and usage
Each Codex API request SHALL come from a `token_usage_record` line (seen in Codex 0.152–0.166), counted once per `response_id`. A rollout without such lines uses `token_count` events (0.145–0.166) instead, counting `last_token_usage` only when `total_token_usage` changed. `input_tokens` includes cached and cache-written tokens: uncached input is `input_tokens` minus `cached_input_tokens` and `cache_write_input_tokens`, never below zero.

#### Scenario: Repeated usage record
- **WHEN** two `token_usage_record` lines carry the same `response_id`
- **THEN** they make one request

#### Scenario: Rollout from before usage records
- **WHEN** a rollout has no `token_usage_record` lines
- **THEN** each `token_count` event whose `total_token_usage` differs from the previous event's is one request
- **AND** events with `info: null` are skipped

#### Scenario: Both kinds present
- **WHEN** a rollout has `token_usage_record` lines and `token_count` events
- **THEN** only the records count as requests

#### Scenario: Usage fields
- **WHEN** a record has `input_tokens` 120, `cached_input_tokens` 100, `cache_write_input_tokens` 0, `output_tokens` 5 and `reasoning_output_tokens` 3
- **THEN** the request has 20 uncached, 100 cache-read and 0 cache-written tokens, and 5 output tokens of which 3 are reasoning
- **AND** cache writes count as short-lifetime writes

#### Scenario: Model and effort
- **WHEN** `turn_context` lines name a `model` and an `effort`
- **THEN** the requests after each line use the latest values
- **AND** a request before any `turn_context` has the model `unknown`

#### Scenario: Context window
- **WHEN** `token_count` events carry `info.model_context_window`
- **THEN** the requests after them record the latest value as their context window

### Requirement: Codex prompts
Codex prompts SHALL be rebuilt from the rollout: the `base_instructions.text` of `session_meta` is the system prompt and every `response_item` is one conversation item. A request's prompt is the conversation before the items its own response added (trailing reasoning, assistant messages and tool calls), which are what it produced. Tool schemas are known only from `additional_tools` items (seen in Codex 0.164–0.166).

#### Scenario: A response's own items
- **WHEN** a user message, a reasoning item and a `function_call` are followed by a `token_usage_record`
- **THEN** that request's prompt is the user message alone
- **AND** the reasoning item and the call are what the request produced

#### Scenario: Compaction
- **WHEN** a `compacted` line arrives (seen in Codex 0.145–0.159)
- **THEN** its `replacement_history` items replace the conversation

#### Scenario: World state
- **WHEN** a rollout has `world_state` lines (seen in Codex 0.145–0.166)
- **THEN** they add nothing to any prompt, since they repeat what the messages carry (AGENTS.md, environment)

#### Scenario: Completed-item events
- **WHEN** a line is an `item_completed` event
- **THEN** it is skipped, since it repeats a response item for the UI

#### Scenario: Other items
- **WHEN** a `response_item` has a type that ends in `_call` but is not a function or custom tool call
- **THEN** it is a tool call named after its type
- **AND** an item of any other unknown type is `unknown` content sized by its JSON

### Requirement: Sizes of content without text
Content that the log does not show as text SHALL count with fixed or derived sizes, in characters: an image as 5,000 (about 1,400 tokens), a Codex image with `detail` `low` as 300 (85 tokens), Codex reasoning as 3/4 of the length of its `encrypted_content` plus its summary text, and Claude Code `redacted_thinking` as half the length of its `data`. These are estimates; cost-attribution scales all sizes to the prompt size each request reports.

#### Scenario: Image in a tool result
- **WHEN** a Claude Code tool result holds a screenshot
- **THEN** the screenshot counts as 5,000 characters, whatever its resolution

#### Scenario: Low-detail Codex image
- **WHEN** a Codex `input_image` has `detail: low`
- **THEN** it counts as 300 characters

#### Scenario: Encrypted reasoning
- **WHEN** a Codex reasoning item has 4,000 characters of `encrypted_content` and no summary
- **THEN** it counts as 3,000 characters

### Requirement: Compaction markers
A request SHALL be marked as following a compaction when a compaction marker comes after the previous answered request of its chain and no later than the request. Markers are Claude Code `system` lines with subtype `compact_boundary` (seen in 2.1.284–2.1.312), which apply to every chain of the transcript, and Codex `compacted` lines or `event_msg` events whose type contains `compact` (none seen yet). cache-misses counts the rebuild as expected.

#### Scenario: Claude Code compaction
- **WHEN** a `compact_boundary` line at 10:05 falls between answered requests at 10:00 and 10:10
- **THEN** the 10:10 request is marked

#### Scenario: Unanswered request after the boundary
- **WHEN** the first request after the boundary got no response
- **THEN** the next answered request is still marked

#### Scenario: Codex compaction
- **WHEN** a `compacted` line comes before a rollout's next usage line
- **THEN** the request of that usage line is marked

### Requirement: Agent-reported cost
tokprof SHALL take a Claude Code session's own cost counter from the `totalCostUSD` of the last `cost-state` line in its transcript (seen in transcripts of Claude Code 2.1.284–2.1.312). cost-attribution compares it with tokprof's total in `tokprof summary`. Codex logs no such figure.

#### Scenario: Several cost-state lines
- **WHEN** a transcript has several `cost-state` lines
- **THEN** the `totalCostUSD` of the last one is the session's reported cost

#### Scenario: No cost-state line
- **WHEN** a transcript has no `cost-state` line
- **THEN** the session has no reported cost and is left out of the comparison

### Requirement: Agent selection
`--agent` SHALL choose whose logs every command reads, as a comma-separated list of `claude` and `codex`; the default is `claude,codex`.

#### Scenario: One agent
- **WHEN** `tokprof --agent codex sessions` runs
- **THEN** only Codex sessions are listed

#### Scenario: Unknown agent
- **WHEN** `--agent foo` is given
- **THEN** the command fails with `unknown agent "foo"` before reading any log

### Requirement: Time window
`--since` and `--until` SHALL select requests, not sessions, by the request's timestamp: on or after `--since` and strictly before `--until`. `--since` takes `YYYY-MM-DD` or `<N>d` for the last N×24 hours; `--until` takes `YYYY-MM-DD` only. A date means local midnight at its start.

#### Scenario: Last days
- **WHEN** `--since 7d` is given
- **THEN** requests from exactly 7×24 hours before now onward are counted

#### Scenario: Date range
- **WHEN** `--since 2026-10-01 --until 2026-10-16` is given
- **THEN** requests from 2026-10-01 00:00 local time up to, but not including, 2026-10-16 00:00 local time are counted

#### Scenario: Session across the boundary
- **WHEN** a session has requests before and after the `--since` moment
- **THEN** only the later requests are counted
- **AND** the last request before the window is still the previous request when cache-misses judges the first request inside it

#### Scenario: Bad value
- **WHEN** `--since 7` is given
- **THEN** the command fails with `bad date "7", expected YYYY-MM-DD`
- **AND** `--until 7d` fails the same way, since `--until` takes no day count

### Requirement: Project filter
`--project` SHALL keep only the logs whose project, written with `~` for the home directory, contains the given text.

#### Scenario: Part of the path
- **WHEN** `--project billing` is given
- **THEN** the sessions of `~/src/billing` are kept

#### Scenario: Absolute path under the home directory
- **WHEN** `--project /Users/me/src/app` is given, as a shell expands an unquoted `~/src/app`
- **THEN** no session under the home directory matches, because their projects start with `~`
- **AND** the report is empty, without an error

### Requirement: Session filter
`--session` SHALL keep only the logs whose session id starts with the given text. Logs whose path does not contain the text are not read at all.

#### Scenario: Claude Code session with subagents
- **WHEN** `--session 9f51246e` is given and that session has 9 subagent transcripts
- **THEN** 10 logs are read and all of them are kept, since the subagent transcripts carry the parent's id

#### Scenario: Codex session with subagent threads
- **WHEN** `--session` gives the id prefix of a Codex thread that spawned subagents
- **THEN** the parent's rollout is kept
- **AND** its subagent threads are kept only when their own ids happen to start with the same text

### Requirement: Commands the filters apply to
`--since`, `--until`, `--project` and `--session` SHALL apply only to the reports over all sessions: `summary`, `misses`, `whatif`, `flame` and `bench`. The default command, `scan`, `sessions`, `handoff`, `ui` and `prices` accept these flags and ignore them.

#### Scenario: Default command with a session filter
- **WHEN** `tokprof --session 3f951de0` runs without a command
- **THEN** it X-rays the latest session, not `3f951de0`
- **AND** `tokprof scan 3f951de0` X-rays that session

#### Scenario: Session list with a time filter
- **WHEN** `tokprof --since 7d sessions` runs
- **THEN** it lists the same sessions as without `--since`

### Requirement: Line about the logs read
`summary`, `misses`, `whatif`, `flame` and `bench` SHALL print one line to stderr after reading the logs: `read <agent> <N> files (<X.X> GB), … in <S.S>s`, for each agent with at least one file, with sizes on disk in units of 10^9 bytes. It counts every log file read, including subagent logs and logs that the project, id or time filters then leave out; `bench` does not read subagent logs.

#### Scenario: Both agents
- **WHEN** 1,234 Claude Code files of 5.6 GB and 78 Codex files of 0.9 GB are read in 3.2 seconds
- **THEN** the line is `read claude 1234 files (5.6 GB), codex 78 files (0.9 GB) in 3.2s`

#### Scenario: Compressed rollouts
- **WHEN** some of the rollouts read are `.jsonl.zst`
- **THEN** they count at their compressed size

#### Scenario: Filter that matches nothing
- **WHEN** `--project` matches no session
- **THEN** the line still counts every log that was read

### Requirement: Unreadable logs
`summary`, `misses`, `whatif` and `flame` SHALL carry on with the logs that can be read, and print `warning: <path>: <error>` to stderr, after the read line, for at most 5 logs that could not be opened, read or decompressed. Inside a log, lines that are not valid JSON are skipped without a warning, and invalid UTF-8 is replaced.

#### Scenario: A log cannot be opened
- **WHEN** a log is removed or made unreadable between discovery and reading
- **THEN** a warning names it and the report covers the other logs

#### Scenario: Reading fails partway
- **WHEN** reading or decompressing a log fails partway
- **THEN** none of that log's requests are counted and a warning names it

#### Scenario: Many unreadable logs
- **WHEN** 12 logs cannot be read
- **THEN** only 5 warnings are printed

#### Scenario: Line still being written
- **WHEN** the last line of a log is incomplete because the agent is still writing it
- **THEN** that line is skipped without a warning

#### Scenario: Other commands
- **WHEN** `bench` meets a log that it cannot read
- **THEN** it skips the log without a warning
- **AND** the X-ray of session-xray fails with the read error when the session's main log or a subagent log cannot be read

### Requirement: No logs found
A command that has no log to read SHALL fail with an error: `summary`, `misses`, `whatif` and `flame` with `no session logs found for <agents>`, where `<agents>` are the `--agent` values joined by `, `; the default command, `scan` and `handoff` without a target, and `ui`, with `no session logs found`.

#### Scenario: Report without logs
- **WHEN** `tokprof --agent claude summary` finds no Claude Code transcript
- **THEN** it fails with `no session logs found for claude`

#### Scenario: Session filter that matches no path
- **WHEN** no log path contains the `--session` text and the default agents are selected
- **THEN** the report fails with `no session logs found for claude, codex`

#### Scenario: Latest session without logs
- **WHEN** `tokprof` runs and the selected agents have no main log
- **THEN** it fails with `no session logs found`

#### Scenario: Commands that do not fail
- **WHEN** `sessions` or `bench` finds no log
- **THEN** `sessions` prints only its header line and `bench` prints `read  in 0.0s` and its empty report, both with exit status 0
