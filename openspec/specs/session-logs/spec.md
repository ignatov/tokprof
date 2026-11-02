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
tokprof SHALL treat a session as one main log together with the logs of the subagents it spawned; a subagent log is never a session of its own. In Claude Code, any transcript below a `subagents` directory is a subagent log, and `<session>/subagents/` belongs to `<session>.jsonl`. In Codex, a rollout whose `session_meta` names a `parent_thread_id` (seen in Codex 0.146–0.163) belongs to the session of that thread, which is the topmost thread above it whose rollout is found.

#### Scenario: Claude Code subagent transcripts
- **WHEN** `<project>/s1.jsonl` and `<project>/s1/subagents/agent-a1.jsonl` exist
- **THEN** session `s1` has one subagent log
- **AND** `agent-a1.jsonl` is neither listed as a session nor picked as the latest session

#### Scenario: Codex subagent thread
- **WHEN** a rollout's `session_meta` has the `parent_thread_id` `P`
- **THEN** it belongs to the session of the rollout whose thread id is `P`, whether that rollout is in `sessions/` or in `archived_sessions/`

#### Scenario: Nested Codex subagent threads
- **WHEN** main thread `M` spawned subagent thread `A`, and `A` spawned thread `B`
- **THEN** `A` and `B` are both subagent logs of session `M`
- **AND** the X-ray of `M` counts both of them and their requests

#### Scenario: Parent log not on disk
- **WHEN** a Codex rollout names a parent thread whose rollout is not found
- **THEN** it is not listed as a session
- **AND** `summary`, `misses`, `whatif` and `flame` still count its requests

#### Scenario: Rollout without session_meta
- **WHEN** a rollout's first line is not a `session_meta` line with an `id`
- **THEN** the rollout is a main log

### Requirement: One-session target
`scan` and `handoff` SHALL work on the session their target names or, without a target, like the default command, on the main log modified most recently among the selected sessions. A target is the path of an existing file, or else a session id prefix: the start of a main session's id, ignoring case. A prefix that several selected main sessions share MUST fail rather than pick one of them.

#### Scenario: Latest session
- **WHEN** `tokprof` runs without a command
- **THEN** it X-rays the main log with the newest modification time
- **AND** only the modification times of main logs count, not those of subagent logs

#### Scenario: Target is a file
- **WHEN** the target is the path of an existing file
- **THEN** that file is read wherever it lies, as a Codex rollout when its name matches `rollout-*.jsonl` or `rollout-*.jsonl.zst`, and as a Claude Code transcript otherwise, whatever `--agent` says

#### Scenario: Target is an id prefix
- **WHEN** `tokprof scan 3f951de0` runs and one main session's id starts with `3f951de0`
- **THEN** that session is used
- **AND** text from the middle of an id, such as `14bc-4966`, or the date in a Codex rollout's name matches nothing

#### Scenario: Nothing matches
- **WHEN** no main log matches the target `abcdef`
- **THEN** the command fails with ``no session file or session id matches "abcdef"; see `tokprof sessions` ``

#### Scenario: Several sessions share the prefix
- **WHEN** `tokprof scan 01a197e1` runs and six Codex sessions' ids start with `01a197e1`
- **THEN** the command fails with `"01a197e1" matches 6 sessions; give more of the id:`
- **AND** then lists them newest first, one per line with the short id, the main log's local `YYYY-MM-DD HH:MM`, the agent and the project, at most 10 of them and `… and <n> more`

### Requirement: Session id and project
Each log SHALL carry a session id and a project read from the log. The id is the `sessionId` of Claude Code `user` and `assistant` lines, or the `id` of the Codex `session_meta` payload. A Codex subagent rollout carries the id of the session it belongs to (see Sessions and subagent logs) instead of its own. The project is the first `cwd` of those Claude Code lines, or of Codex `session_meta` and `turn_context` lines, written with `~` for the home directory.

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

### Requirement: Chains
tokprof SHALL split a session's requests into chains, one per cache lineage, each with a name and a kind: main, subagent or side. Claude Code requests with request-audit lines form one chain per `agentId` and `querySource` pair; requests without them form chains by `agentId` and `isSidechain`, apart from the audited ones. Each Codex rollout is one chain. A subagent chain is named `subagent: <task>` when its log names the task (see cost-attribution).

#### Scenario: Claude Code main conversation
- **WHEN** an `api-request` in a main transcript has no `agentId` and a `querySource` of `repl_main_thread`, `sdk`, `main` or none
- **THEN** its chain is `main`, of kind main

#### Scenario: Claude Code subagent stream
- **WHEN** the `querySource` is `agent:builtin:Explore` and the subagent has no meta file with a `description`
- **THEN** the chain is `subagent:Explore`, of kind subagent, named after the part after the last `:`, so `agent:custom` gives `subagent:custom`
- **AND** a main-thread `querySource` with an `agentId`, or in a subagent transcript, gives the chain `subagent`

#### Scenario: Claude Code subagent with a task
- **WHEN** a subagent's meta file has `"description": "Find handlers"`
- **THEN** every subagent chain of its transcript is named `subagent: Find handlers`

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

#### Scenario: Codex spawned thread
- **WHEN** `session_meta.source.subagent` is `thread_spawn` with `"agent_path": "/root/fix_flaky_tests"` and no `agent_role`
- **THEN** its role is `default`, and its chain is `subagent: fix flaky tests`
- **AND** without an `agent_path` the chain is `subagent:default`

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

#### Scenario: Reminders of role system
- **WHEN** an `api-request-blob` message has role `system`, as Claude Code's environment notes, hook context and file-change notes do since 2.1.300
- **THEN** its text is reminder content, labeled like a `<system-reminder>`, and it does not start a turn (see cost-attribution)

### Requirement: Reminders joined in one message
A Claude Code `api-request-blob` message of role `system` SHALL be split into the reminders it joins: a new reminder starts after a blank line, at a known opening of another source (seen in 2.1.300–2.1.316). Each part, its separators included, is labeled by its opening, and the parts add up to the whole text. A reminder whose opening is not known keeps the label its first words give.

#### Scenario: Openings and their sources
- **WHEN** a part starts with `[SYSTEM NOTIFICATION`, `<task-notification>` or `Background agent "`
- **THEN** it is `background notifications`
- **AND** `The following deferred tools` and `The following tools just became available` give `deferred tool lists`
- **AND** `The following skills are available` and `The following skills were invoked` give `skills`
- **AND** `Codebase and user instructions are shown below` and `Instruction files were re-read` give `instructions`
- **AND** a first line that starts with `Called the ` and holds ` tool with the following input`, or starts with `Note: ` and holds `was read before the last conversation was summarized`, gives `attached files`
- **AND** a first line that holds ` hook additional context:` gives `hook context`
- **AND** `# MCP Server Instructions` followed by a blank line and `The following MCP servers`, `# Environment` followed by `You have been invoked`, `# Advisor Tool` followed by a blank line and ``You have access to an `advisor` ``, `You are powered by the model`, `Available agent types`, `While bypass permissions mode is active` and `Today's date is` give `other`

#### Scenario: A message after a compaction
- **WHEN** one `system` message holds 33 background agents' results, each followed by a hook's context, then files Claude Code restored after the compaction, the skills invoked before it, the deferred tool list and the environment
- **THEN** each of these is its own reminder with its own source, instead of the whole message counting as background notifications

#### Scenario: Neighbours of one source
- **WHEN** two parts in a row have the same source, such as a background-task header and its `<task-notification>`
- **THEN** they stay one reminder

#### Scenario: A heading inside a reminder
- **WHEN** an instructions reminder holds a paragraph that starts with `# Environment` but is not followed by `You have been invoked`
- **THEN** the reminder is not split there

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
A request SHALL be marked as following a compaction when a compaction marker comes after the previous answered request of its conversation (see Main conversations across chains) and no later than the request. Markers are Claude Code `system` lines with subtype `compact_boundary` (seen in 2.1.284–2.1.312), which apply to every chain of the transcript, and Codex `compacted` lines or `event_msg` events whose type contains `compact` (none seen yet). cache-misses counts the rebuild as expected.

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

### Requirement: Compaction kind
Each Claude Code compaction marker SHALL have a kind. It is `idle` when a `system` line with subtype `informational` whose content starts with `Compacted while idle` comes after the `compact_boundary` and before the next answered request of the transcript (seen in 2.1.302–2.1.310). Otherwise it is the boundary's `compactMetadata.trigger`: `auto` (seen in 2.1.284–2.1.312) or `manual`. Codex markers have no kind.

#### Scenario: Compacted while idle
- **WHEN** a `compact_boundary` with trigger `auto` at 19:13:39 is followed by the notice "Compacted while idle, before the prompt cache expired" at 19:13:40, and the next answered request comes at 19:19
- **THEN** the marker's kind is `idle`

#### Scenario: Compacted because the window filled up
- **WHEN** a `compact_boundary` with trigger `auto` has no such notice before the next answered request
- **THEN** the marker's kind is `auto`

#### Scenario: Compacted on request
- **WHEN** a `compact_boundary` has trigger `manual`
- **THEN** the marker's kind is `manual`

#### Scenario: A notice after the next request
- **WHEN** the notice comes only after the next answered request
- **THEN** it does not change the kind of the earlier marker

#### Scenario: Codex compaction
- **WHEN** a Codex rollout has a `compacted` line
- **THEN** its marker has no kind

### Requirement: Agent-reported cost
tokprof SHALL take a Claude Code session's own cost counter from the `cost-state` lines in its transcript (seen in transcripts of Claude Code 2.1.284–2.1.312). The lines that share a `startTime` are one run of the counter, and lines without a `startTime` are one run together. The counter is the sum, over the runs, of the highest `totalCostUSD` in each. cost-attribution compares it with tokprof's total in `tokprof summary`. Codex logs no such figure.

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
`--project` SHALL keep only the logs whose project, written with `~` for the home directory, contains the given text. A value that starts with the home directory is read with `~` in its place, and a trailing `/` is dropped, so a path the shell expanded matches as typed.

#### Scenario: Part of the path
- **WHEN** `--project billing` is given
- **THEN** the sessions of `~/src/billing` are kept

#### Scenario: Absolute path under the home directory
- **WHEN** `--project /Users/me/src/app` is given, as a shell expands an unquoted `~/src/app`, and the home directory is `/Users/me`
- **THEN** it is read as `~/src/app` and the sessions of `~/src/app` match

#### Scenario: Trailing slash
- **WHEN** `--project ~/src/billing/` is given
- **THEN** it matches the sessions of `~/src/billing`, as `--project ~/src/billing` does

### Requirement: Session filter
`--session` SHALL keep only the sessions whose id starts with the given text, ignoring case, together with their subagent logs.

#### Scenario: Claude Code session with subagents
- **WHEN** `--session 9f51246e` is given and that session has 9 subagent transcripts
- **THEN** 10 logs are read and all of them are kept, since the subagent transcripts carry the parent's id

#### Scenario: Codex session with subagent threads
- **WHEN** `--session` gives the id prefix of a Codex thread that spawned subagents
- **THEN** the parent's rollout and the rollouts of every thread below it are kept
- **AND** all of their requests are reported under the parent's id

### Requirement: Commands the filters apply to
`--since`, `--until`, `--project` and `--session` SHALL apply to every command but `prices`. The reports `summary`, `misses`, `whatif`, `flame` and `bench` count the requests they select. The default command, `scan`, `handoff`, `sessions` and `ui` pick or list only the sessions they select: project and id as for the reports, a main log last written on or after `--since`, and created before `--until`. An X-ray still covers its session's whole log.

#### Scenario: Default command with a session filter
- **WHEN** `tokprof --session 3f951de0` runs without a command
- **THEN** it X-rays `3f951de0`, as `tokprof scan 3f951de0` does
- **AND** `tokprof --project billing` X-rays the latest session of `~/src/billing`

#### Scenario: Session list with a time filter
- **WHEN** `tokprof --since 7d sessions` runs
- **THEN** it lists only the sessions whose main log was written in the last 7 days

#### Scenario: Sessions started before a date
- **WHEN** `tokprof --until 2026-10-01 sessions` runs
- **THEN** it lists only the sessions whose main log was created before 2026-10-01 00:00 local time

#### Scenario: A target outside the filters
- **WHEN** `tokprof --project billing scan 3f951de0` runs and `3f951de0` belongs to another project
- **THEN** the command fails as for a target that matches nothing

#### Scenario: Prices
- **WHEN** `tokprof --since 7d prices` runs
- **THEN** it prints the same prices as `tokprof prices`

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
`summary`, `misses`, `whatif`, `flame` and `bench` SHALL carry on with the logs that can be read, and print `warning: <path>: <error>` to stderr, after the read line, for at most 5 logs that could not be opened, read or decompressed, then `warning: <n> more logs could not be read` for the rest. Inside a log, lines that are not valid JSON are skipped without a warning, and invalid UTF-8 is replaced.

#### Scenario: A log cannot be opened
- **WHEN** a log is removed or made unreadable between discovery and reading
- **THEN** a warning names it and the report covers the other logs

#### Scenario: Reading fails partway
- **WHEN** reading or decompressing a log fails partway
- **THEN** none of that log's requests are counted and a warning names it

#### Scenario: Many unreadable logs
- **WHEN** 12 logs cannot be read
- **THEN** 5 warnings name them, and a sixth reads `warning: 7 more logs could not be read`

#### Scenario: Line still being written
- **WHEN** the last line of a log is incomplete because the agent is still writing it
- **THEN** that line is skipped without a warning

#### Scenario: Other commands
- **WHEN** `bench` meets a log that it cannot read
- **THEN** it skips the log and prints a warning, as the other reports do
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

### Requirement: Claude Code origin and task fields
tokprof SHALL read the origin, task and title of Claude Code work (see cost-attribution) from:
- `attributionSkill`, `attributionMcpServer` and `attributionPlugin` on assistant lines;
- `<command-name>` and `<task-notification>` with its `<task-id>` at the start of user messages;
- `agentType`, `description` and `toolUseId` in the `agent-<id>.meta.json` next to a subagent transcript;
- `ai-title` and `custom-title` lines.

These fields were seen in Claude Code 2.1.284–2.1.312.

#### Scenario: A subagent transcript without a meta file
- **WHEN** `subagents/agent-a1.jsonl` has no `agent-a1.meta.json` next to it
- **THEN** the subagent has no task, its chain keeps its stream name, and its origin comes from the stream's type

### Requirement: Codex origin and task fields
tokprof SHALL read the origin, task and title of Codex work (see cost-attribution) from:
- `session_meta.source.subagent`: `thread_spawn` with `agent_path` and `agent_role`, or a role such as `{"other": "guardian"}` or `"review"`;
- `id` and `thread_name` in `session_index.jsonl` under `$CODEX_HOME`, or `~/.codex` when it is not set.

These fields were seen in Codex 0.146–0.163. The Codex app and TUI write the index; threads started from IDE plugins and `codex exec` were not in it.

#### Scenario: No index
- **WHEN** `session_index.jsonl` does not exist
- **THEN** Codex sessions have no title and the rest of the analysis is unchanged

### Requirement: Short session id
Wherever tokprof shows a session by a short id, the short id SHALL be the first 8 characters of a Claude Code id and the first three groups of a Codex id, 18 characters. A Codex thread id is a UUIDv7: its first 13 characters are the millisecond it was made, which threads launched together share, and the third group adds 12 random bits.

#### Scenario: Claude Code session
- **WHEN** a Claude Code session has the id `3f951de0-14bc-4966-880b-81f649966ccf`
- **THEN** its short id is `3f951de0`

#### Scenario: Codex sessions started in the same minute
- **WHEN** two Codex threads have the ids `01a197e1-4d9f-76b1-b25d-cdd77b01e10f` and `01a197e1-4dc9-7fa1-8e33-599c9d21e8c2`
- **THEN** their short ids are `01a197e1-4d9f-76b1` and `01a197e1-4dc9-7fa1`

#### Scenario: Codex sessions launched in the same millisecond
- **WHEN** four Codex threads launched together have ids that start `01a1978f-f718-7312`, `01a1978f-f718-7691`, `01a1978f-f718-7ff1` and `01a1978f-f718-7991`
- **THEN** each keeps its own short id

### Requirement: Messages the user typed
Each source SHALL keep the messages the user typed in a session's main conversation, in order, with their time and text, for the prompts block of session-xray. A typed message is one that starts a turn: not a reminder, a tool result, an interruption, a background task's result, the summary after a compaction or a shell command run from the prompt.

#### Scenario: Claude Code
- **WHEN** a main transcript has a `user` line whose text is not a reminder and holds no tool result, outside a sidechain and with no `agentId`
- **THEN** it is a typed message, and a slash command keeps its `<command-name>` and `<command-args>` as its text, such as `/goal finish the parser`

#### Scenario: Codex
- **WHEN** a main thread's `response_item` is a `message` of role `user` whose text is not injected context
- **THEN** it is a typed message

#### Scenario: Subagents
- **WHEN** a subagent's log starts with its brief
- **THEN** the brief is not a typed message of the session
