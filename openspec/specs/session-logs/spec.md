# Session Logs Specification

## Purpose
Find the session logs that Claude Code, Codex, pi and OpenCode write on this machine (OpenCode's in SQLite stores) and read them into sessions, chains and requests, with token usage and rebuilt prompts, for every other tokprof capability to analyze. It also owns the global flags that select which logs and requests a report covers.

## Requirements

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
`scan` and `handoff` SHALL work on the session their target names or, without a target, like the default command, on the main session with the latest last activity (see Last activity) among the selected sessions. A target is the path of an existing file, or else a session id prefix: the start of a main session's id, ignoring case. A prefix that several selected main sessions share MUST fail rather than pick one of them.

#### Scenario: Latest session
- **WHEN** `tokprof` runs without a command
- **THEN** it X-rays the main session with the latest last activity
- **AND** only main logs count, not subagent logs

#### Scenario: A resumed session wrote last
- **WHEN** the main log written most recently got only records without a time since 09:27, and another main log's last record with a time is from 16:13
- **THEN** `tokprof` X-rays the session of the other log

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
- **AND** then lists them by last activity, newest first, one per line with the short id, the local `YYYY-MM-DD HH:MM` of the last activity, the agent and the project, at most 10 of them and `… and <n> more`

### Requirement: Session id and project
Each log SHALL carry a session id and a folder read from the log, as Claude Code and Codex session ids, Pi session id and project and OpenCode sessions say; the project those requirements name is this folder. The session's project is placed from the folder (see Project of a folder). The folder and a project that is a path are written with `~` for the home directory.

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

#### Scenario: A session in a worktree
- **WHEN** the first `cwd` is `/Users/me/.branch-deck/pool/app-c821e1d7/4/app`, a worktree of `/Users/me/src/app`
- **THEN** the session's folder is `~/.branch-deck/pool/app-c821e1d7/4/app` and its project is `~/src/app`

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
A request SHALL be marked as following a compaction when a compaction marker comes after the previous answered request of its conversation (see Main conversations across chains) and no later than the request. The agents' markers are those of Compaction marker lines. cache-misses counts the rebuild as expected.

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
Each Claude Code compaction marker SHALL have a kind. It is `idle` when a `system` line with subtype `informational` whose content starts with `Compacted while idle` (seen in 2.1.302–2.1.310) or `Compacted using a summary written while you were away` (2.1.316 code, not yet in a log) comes between the `compact_boundary` and the transcript's next answered request. Otherwise it is the boundary's `compactMetadata.trigger`: `auto` (seen in 2.1.284–2.1.312) or `manual`. Codex markers have no kind.

#### Scenario: Compacted while idle
- **WHEN** a `compact_boundary` with trigger `auto` at 19:13:39 is followed by the notice "Compacted while idle, before the prompt cache expired" at 19:13:40, and the next answered request comes at 19:19
- **THEN** the marker's kind is `idle`

#### Scenario: Compacted with a summary written during the break
- **WHEN** after a three-hour break, a `compact_boundary` at 10:02:11 is followed by the notice "Compacted using a summary written while you were away" at 10:02:11, and the next answered request comes at 10:02:30
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
`--project` SHALL keep only the logs whose project or folder (see Session id and project), written with `~` for the home directory, contains the given text. A value that starts with the home directory is read with `~` in its place, and a trailing `/` is dropped, so a path the shell expanded matches as typed.

#### Scenario: Part of the path
- **WHEN** `--project billing` is given
- **THEN** the sessions of `~/src/billing` are kept

#### Scenario: Absolute path under the home directory
- **WHEN** `--project /Users/me/src/app` is given, as a shell expands an unquoted `~/src/app`, and the home directory is `/Users/me`
- **THEN** it is read as `~/src/app` and the sessions of `~/src/app` match

#### Scenario: Trailing slash
- **WHEN** `--project ~/src/billing/` is given
- **THEN** it matches the sessions of `~/src/billing`, as `--project ~/src/billing` does

#### Scenario: A repository with worktrees and subfolders
- **WHEN** `--project ~/src/app` is given, and sessions ran in `~/src/app`, in `~/src/app/plugins/web` and in the worktree `~/.branch-deck/pool/app-c821e1d7/4/app`
- **THEN** the sessions of all three are kept

#### Scenario: Text of the folder
- **WHEN** `--project branch-deck` is given
- **THEN** the sessions whose folder is `~/.branch-deck/pool/app-c821e1d7/4/app` are kept, though their project is `~/src/app`

#### Scenario: Temporary folders
- **WHEN** `--project temporary` is given
- **THEN** the sessions whose project is `temporary folders` are kept

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
`--since`, `--until`, `--project`, `--session` and `--client` SHALL apply to every command but `prices` and `day`. The reports `summary`, `misses`, `whatif`, `tools`, `pauses`, `flame` and `bench` count the requests they select. The default command, `scan`, `handoff`, `sessions` and `ui` pick or list only the sessions they select: project and id as for the reports, the client as the Client filter requirement says, a main log last written on or after `--since`, and created before `--until`.

#### Scenario: Default command with a session filter
- **WHEN** `tokprof --session 3f951de0` runs without a command
- **THEN** it X-rays `3f951de0`, as `tokprof scan 3f951de0` does
- **AND** `tokprof --project billing` X-rays the latest session of `~/src/billing`

#### Scenario: An X-ray covers the whole log
- **WHEN** `tokprof --since 7d` X-rays a session whose log began 10 days ago
- **THEN** the X-ray counts the requests of all 10 days, not only of the last 7

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

#### Scenario: Latest session of a client
- **WHEN** `tokprof --client sdk-ts` runs without a command
- **THEN** it X-rays the latest session whose main log first names a client that contains `sdk-ts`

#### Scenario: The pause report
- **WHEN** `tokprof --since 30d --client sdk-ts pauses` runs
- **THEN** it counts the pauses, cold returns and sessions of the `sdk-ts` requests of the last 30 days only

#### Scenario: The day view
- **WHEN** `tokprof --project billing day` runs
- **THEN** it shows the day of the sessions of `~/src/billing` only, and `tokprof --since 7d day` fails as work-rhythm says

### Requirement: Line about the logs read
`summary`, `misses`, `whatif`, `tools`, `pauses`, `flame`, `bench` and `day` SHALL print one line to stderr after reading the logs: `read <agent> <N> files (<X.X> GB), … in <S.S>s`, for each agent with at least one file, with sizes on disk in units of 10^9 bytes. It counts every log file read, including subagent logs and logs that the project, id or time filters then leave out; `bench` and `day` read no subagent logs, and `day` only the logs active around its day.

#### Scenario: Both agents
- **WHEN** 1,234 Claude Code files of 5.6 GB and 78 Codex files of 0.9 GB are read in 3.2 seconds
- **THEN** the line is `read claude 1234 files (5.6 GB), codex 78 files (0.9 GB) in 3.2s`

#### Scenario: Compressed rollouts
- **WHEN** some of the rollouts read are `.jsonl.zst`
- **THEN** they count at their compressed size

#### Scenario: Filter that matches nothing
- **WHEN** `--project` matches no session
- **THEN** the line still counts every log that was read

#### Scenario: The day view
- **WHEN** `tokprof day` runs and 35 main logs of 0.10 GB were active around the day
- **THEN** the line counts those 35 logs

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
- **WHEN** no session id starts with the `--session` text and the default agents are selected
- **THEN** the report fails with `no session logs found for claude, codex, pi, opencode`

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

### Requirement: Last activity
A session's last activity SHALL be the time of the last record in its main log that carries a time. For Claude Code that is a top-level `timestamp`, seen in 2.1.284–2.1.316. Records without one, such as `cost-state`, `last-prompt`, `ai-title`, `mode` and `tools-used`, do not count. A Codex rollout's last activity is its last write. A log with no record that carries a time takes its last write, and the last activity is never later than the last write.

#### Scenario: A session resumed without work
- **WHEN** a Claude Code transcript's last record with a `timestamp` is from 09:27, and `cost-state`, `last-prompt` and `ai-title` lines were appended at 16:01
- **THEN** the session's last activity is 09:27

#### Scenario: A session at work
- **WHEN** the last line of a Claude Code transcript is an `assistant` line with the timestamp 16:13:19
- **THEN** the session's last activity is 16:13:19

#### Scenario: A Codex rollout
- **WHEN** a Codex rollout was last written at 14:22
- **THEN** its session's last activity is 14:22

#### Scenario: No record with a time
- **WHEN** a transcript has no record with a `timestamp`
- **THEN** the session's last activity is the transcript's last write

#### Scenario: A time after the last write
- **WHEN** the last record's `timestamp` is later than the log's last write
- **THEN** the session's last activity is the last write

#### Scenario: A line that cannot be read
- **WHEN** a transcript's last line is cut off or is not valid JSON
- **THEN** it does not count, and the last readable record with a `timestamp` before it gives the last activity

#### Scenario: Subagents at work
- **WHEN** a subagent transcript of the session was written after the main transcript's last record with a `timestamp`
- **THEN** the session's last activity is still that record's time

### Requirement: Client of a request
Each request SHALL carry its client, the program that ran the agent, as the log names it. Claude Code names it as `entrypoint` on the line of the response, seen in 2.1.284–2.1.316 as `sdk-ts`, `cli` and `sdk-cli`. Codex names it as `payload.originator` of the rollout's `session_meta`, for all of its requests, seen in 0.147–0.166 as `codex_exec`, `codex-tui`, `Codex Desktop` and the names of IDE plugins. A request whose log names none has the client `unknown`.

#### Scenario: A session resumed in another client
- **WHEN** a Claude Code transcript has responses whose lines carry `entrypoint` `cli`, and later ones that carry `sdk-ts`
- **THEN** each request carries the client of its own response

#### Scenario: A subagent transcript
- **WHEN** the lines of a Claude Code subagent transcript carry `entrypoint` `sdk-ts`
- **THEN** its requests have the client `sdk-ts`

#### Scenario: A Codex rollout
- **WHEN** a rollout's `session_meta` has `originator` `codex_exec`
- **THEN** every request of that rollout has the client `codex_exec`

#### Scenario: No client in the log
- **WHEN** a log names no client
- **THEN** its requests have the client `unknown`

### Requirement: Client filter
`--client` SHALL keep only the requests whose client contains the given text, ignoring case. The commands that pick or list sessions SHALL keep a session when the first client its main log names contains the text.

#### Scenario: Part of a value
- **WHEN** `--client sdk` is given
- **THEN** the requests with the clients `sdk-ts` and `sdk-cli` are kept
- **AND** the requests with the client `cli` are left out

#### Scenario: Case
- **WHEN** `--client "codex desktop"` is given
- **THEN** the requests with the client `Codex Desktop` are kept

#### Scenario: Requests without a client
- **WHEN** `--client unknown` is given
- **THEN** only the requests whose log names no client are kept

#### Scenario: A session list
- **WHEN** `tokprof --client sdk-ts sessions` runs
- **THEN** it lists only the sessions whose main log first names a client that contains `sdk-ts`

#### Scenario: A text inside a longer value
- **WHEN** `--client cli` is given
- **THEN** the requests with the clients `cli` and `sdk-cli` are kept, since both contain the text

### Requirement: Hand-offs between the user and the agent
Each source SHALL keep, for a session's main conversation, the times at which the work passed between the user and the agent, in order: the agent ended its turn, the user stopped it, the user typed while it worked, the agent asked the user a question, the user answered it, and a wake, when a background task or another session gave the agent work. Subagent logs have none.

#### Scenario: Claude Code ends of turn
- **WHEN** an `assistant` line outside a sidechain and with no `agentId` has a `message.stop_reason` other than `tool_use`, as `end_turn` and `stop_sequence` were in 2.1.310–2.1.316
- **THEN** it is an end of turn, and so is a `<synthetic>` message that reports an API error
- **AND** the lines of one response give one end, at the time of the last of them

#### Scenario: Claude Code interrupts and wakes
- **WHEN** a main `user` line's text starts with `[Request interrupted`, or with `<task-notification>` or `<cross-session-message`
- **THEN** the first is an interrupt and the others are wakes

#### Scenario: Claude Code messages typed while the agent worked
- **WHEN** a main `attachment` line has `attachment.type` `queued_command` and `attachment.origin.kind` `human`, as in 2.1.310–2.1.316
- **THEN** it is a message the user typed while the agent worked, a slash command included
- **AND** with the origin `peer` or `task-notification` it is none
- **AND** in a log without `origin`, it is one when it is not `isMeta` and its `prompt`, a string or the first text of a list of blocks, does not start with `<`

#### Scenario: Claude Code questions
- **WHEN** a main response calls `AskUserQuestion` or `ExitPlanMode`
- **THEN** the call is a question, and the `tool_result` line for its id is the answer

#### Scenario: Codex
- **WHEN** a main thread logs an `event_msg` of type `task_complete` or `turn_aborted`, as Codex 0.147–0.166 does
- **THEN** the first is an end of turn and the second an interrupt; Codex logs no questions, wakes or messages typed while it works

### Requirement: Sessions a program ran
Each source SHALL mark a session that a program ran with nobody typing in it, when its log tells.

#### Scenario: Claude Code print mode
- **WHEN** the first `entrypoint` of a Claude Code transcript is `sdk-cli`, as `claude -p` writes it
- **THEN** a program ran the session

#### Scenario: Codex exec
- **WHEN** a rollout's `session_meta.originator` is `codex_exec`
- **THEN** a program ran the session

#### Scenario: An IDE session
- **WHEN** the `entrypoint` is `sdk-ts` or `cli`
- **THEN** the session is not marked

### Requirement: Request times
Each request SHALL have, when its log tells, the time its first attempt was sent, the time its answered attempt was sent, and the time its response ended. A request that was not retried has one send. When the log tells no time before a request's response, the request was sent at the first line of its response, and a response never ends before its send.

#### Scenario: Claude Code with request-audit lines
- **WHEN** a response's lines carry a `requestRef` that names an `api-request` line (seen in Claude Code 2.1.300–2.1.316)
- **THEN** its answered attempt was sent at that line's `timestamp`
- **AND** its response ended at the latest `timestamp` of the assistant lines of its `message.id`

#### Scenario: Claude Code without request-audit lines
- **WHEN** a transcript, or the part of it written before the audit lines, has no `api-request` lines (Claude Code 2.1.284–2.1.299)
- **THEN** a request was sent at the `timestamp` of the latest `user` line of its chain before the first line of its response
- **AND** `attachment` lines do not count, since Claude Code writes some of them with the response

#### Scenario: Codex
- **WHEN** a Codex request comes from a `token_usage_record` line or a `token_count` event
- **THEN** its response ended at the `timestamp` of the last item the response added (see Codex prompts), or at the request's own line when it added none
- **AND** it was sent at the `timestamp` of the latest `response_item` before those items, in Codex 0.147–0.166

### Requirement: Retried requests
A Claude Code request SHALL count as retried when API errors came before its answered attempt: an `api-request` line followed by a `system` line of `subtype` `api_error` is an attempt that failed, and the next `api-request` of its chain repeats it (seen in 2.1.287–2.1.316). The request keeps how many attempts failed and the `error.formatted` text of the last error.

#### Scenario: Three failed attempts
- **WHEN** `api-request` lines at 07:04:05, 07:04:06 and 07:04:07 are each followed by an `api_error` line with `retryAttempt` 1, 2 and 3, and the `api-request` at 07:04:09 gets the response
- **THEN** the request has 3 retries, its first attempt was sent at 07:04:05 and its answered attempt at 07:04:09
- **AND** its last error is `502 local proxy: upstream request failed` when that is the last line's `error.formatted`

#### Scenario: Without request-audit lines
- **WHEN** a transcript has no `api-request` lines, and `api_error` lines of a request's chain come after its send and before its response
- **THEN** they are its retries, and its answered attempt was sent at the last error's `timestamp` plus its `retryInMs`

#### Scenario: No attempt answered
- **WHEN** every attempt of a request failed and a `<synthetic>` message reports the error, in a transcript with `api-request` lines
- **THEN** the request has no response, and keeps its retries and its last error
- **AND** without `api-request` lines such a request leaves no request at all, as any request without a response does

#### Scenario: Codex
- **WHEN** a Codex rollout is read
- **THEN** its requests have no retries, since Codex 0.147–0.166 logs none

### Requirement: Call outcomes
Each source SHALL tell, for every tool result, whether its call failed, the user refused it, or a permission rule denied it. A result with none of these belongs to a call that worked.

#### Scenario: Claude Code failure
- **WHEN** a `tool_result` item has `is_error` set to `true` (seen in Claude Code 2.1.287–2.1.316), and the call was neither refused nor denied
- **THEN** the call failed

#### Scenario: Claude Code refusal
- **WHEN** a result with `is_error` set has a text that starts with `The user doesn't want to proceed with this tool use`, `User rejected`, `[Request interrupted by user for tool use]` or `MCP tool call has been cancelled likely by a user interaction`
- **THEN** the user refused the call

#### Scenario: Claude Code denial
- **WHEN** the `user` line that holds the result has `permissionDecision.decision` set to `reject` (seen in 2.1.300–2.1.316)
- **THEN** a permission rule denied the call
- **AND** so it did when the text starts with `Permission to use ` and holds ` has been denied`, or starts with `Permission for this command was denied by a built-in`

#### Scenario: A search that found nothing
- **WHEN** a `Bash` result has `toolUseResult.returnCodeInterpretation` `No matches found` and no `is_error`
- **THEN** the call worked

#### Scenario: Codex failure
- **WHEN** a `custom_tool_call_output` starts with `Script failed`, or one of its text items is a JSON object whose `exit_code` is not 0 (seen in Codex 0.147–0.166)
- **THEN** the call failed
- **AND** so did a direct command whose output has a line `Process exited with code <n>` with `n` other than 0, as `function_call_output` reports it (seen in 0.147–0.164)

#### Scenario: Codex refusal
- **WHEN** a Codex tool output starts with `aborted by user`
- **THEN** the user refused the call

### Requirement: Error line of a failed call
A failed call SHALL keep one line that tells why it failed, cut to 200 characters: the first non-empty line of its result's text. A result that tells an exit code gives `exit code <n>: ` followed by the last non-empty line of the command's output instead, and a Codex script error gives the first non-empty line after `Script error:`.

#### Scenario: A missing file
- **WHEN** a `Read` result's text is `File does not exist. Note: your current working directory is /Users/me/src/app.`
- **THEN** that is its error line

#### Scenario: A Claude Code command
- **WHEN** a `Bash` result's text is `Exit code 101` followed by test output whose last non-empty line is `error: test failed, to rerun pass --lib`
- **THEN** its error line is `exit code 101: error: test failed, to rerun pass --lib`

#### Scenario: A Codex command
- **WHEN** an `exec` result prints `"exit_code":1` with an `output` whose last non-empty line is `ModuleNotFoundError: No module named 'bs4'`
- **THEN** its error line is `exit code 1: ModuleNotFoundError: No module named 'bs4'`

#### Scenario: A Codex script error
- **WHEN** an `exec` result starts with `Script failed` and holds `Script error:` followed by `SyntaxError: Unexpected token ')'`
- **THEN** its error line is `SyntaxError: Unexpected token ')'`, the first non-empty line after `Script error:`

### Requirement: What a wake reports
Each source SHALL tell what each wake reports: that a subagent of the session finished or is waiting, that a command the agent ran in the background finished or printed, or something else.

#### Scenario: Claude Code subagent
- **WHEN** the `<summary>` of a `<task-notification>` starts with `Agent "` (seen in Claude Code 2.1.291–2.1.316)
- **THEN** the wake reports a subagent

#### Scenario: Claude Code background command
- **WHEN** the `<summary>` starts with `Background command "` or `Monitor ` (seen in 2.1.293–2.1.316)
- **THEN** the wake reports a background command

#### Scenario: Something else
- **WHEN** the wake is a goal check-in, a notice that Claude Code writes when a session resumes, such as `Background shell command didn't finish before the previous session end`, or a `<cross-session-message`
- **THEN** it reports something else

#### Scenario: Codex
- **WHEN** a Codex rollout is read
- **THEN** it has no wakes to report, since Codex logs none (see Hand-offs between the user and the agent)

### Requirement: Agent version of a request
Each request SHALL carry the version of the agent that wrote its log, when the log names one. For Claude Code, this is the `version` of the request's line, which Claude Code requests and usage already keeps (seen in 2.1.284–2.1.316). For Codex, it is the `cli_version` of the rollout's first `session_meta` payload, for every request of that rollout (seen in 0.147–0.166).

#### Scenario: A Codex rollout
- **WHEN** a rollout's first line is a `session_meta` whose payload has `cli_version` `0.166.0-alpha.5.1`
- **THEN** every request of that rollout carries `0.166.0-alpha.5.1`

#### Scenario: A later session_meta
- **WHEN** a rollout holds a second `session_meta` with another `cli_version`
- **THEN** its requests still carry the first one's version
- **AND** no Codex miss gets the cause `agent updated` (see cache-misses) from it

#### Scenario: A log without a version
- **WHEN** a Codex `session_meta` has no `cli_version`
- **THEN** the rollout's requests carry no version

### Requirement: Compaction calls
A request SHALL be marked as a compaction call when the agent's log names it as the request that made a compaction. In Codex that is the request whose `token_usage_record` has the `response_id` that a later `compacted` line gives as `compaction_response_id`. This was seen in Codex 0.153–0.159. Earlier rollouts log usage only as `token_count` events, which carry no response id, and 0.152 logs no `compaction_response_id`. Claude Code's compaction call is not marked.

#### Scenario: A Codex compaction call
- **WHEN** a rollout logs the summary message, then a `token_usage_record` with `response_id` `resp_b`, then a `compacted` line whose `compaction_response_id` is `resp_b`
- **THEN** the request of `resp_b` is a compaction call
- **AND** the request after the `compacted` line follows a compaction, as Compaction markers say

#### Scenario: An id no usage line has
- **WHEN** a `compacted` line has no `compaction_response_id`, or names one that no `token_usage_record` of the rollout has
- **THEN** no request is marked as a compaction call for it

### Requirement: Claude Code and Codex log locations
Claude Code's session logs SHALL be the files ending in `.jsonl` under `$CLAUDE_CONFIG_DIR/projects`, by default `~/.claude/projects`. Codex's SHALL be the files named `rollout-*.jsonl`, or `rollout-*.jsonl.zst` when zstd-compressed, under `$CODEX_HOME/sessions` and `$CODEX_HOME/archived_sessions`, by default under `~/.codex`. Compressed rollouts are decompressed while reading.

#### Scenario: Claude Code directory set by the environment
- **WHEN** `CLAUDE_CONFIG_DIR=/data/claude` is set
- **THEN** Claude Code transcripts are read from `/data/claude/projects` only

### Requirement: Pi log locations
pi's session logs SHALL be the files ending in `.jsonl` under `$PI_CODING_AGENT_SESSION_DIR`, else `$PI_CODING_AGENT_DIR/sessions`, by default `~/.pi/agent/sessions` (seen in pi 0.84.2).

#### Scenario: pi session directory set by the environment
- **WHEN** `PI_CODING_AGENT_DIR=/data/pi` is set and `PI_CODING_AGENT_SESSION_DIR` is not
- **THEN** pi logs are read from `/data/pi/sessions` only

#### Scenario: pi store that is not a log
- **WHEN** a pi-durable app keeps its conversations in a SQLite file
- **THEN** it is not picked up when tokprof looks for session logs

### Requirement: Claude Code and Codex session ids
The session id SHALL be the `sessionId` of Claude Code `user` and `assistant` lines, or the `id` of the Codex `session_meta` payload. A Codex subagent rollout carries the id of the session it belongs to (see Sessions and subagent logs) instead of its own. The project is the first `cwd` of those Claude Code lines, or of Codex `session_meta` and `turn_context` lines.

#### Scenario: Claude Code session
- **WHEN** a transcript's lines carry the `sessionId` `3f951de0-14bc-4966-880b-81f649966ccf` and first the `cwd` `/Users/me/src/app`, with `/Users/me` the home directory
- **THEN** its session is `3f951de0-14bc-4966-880b-81f649966ccf` of the project `~/src/app`

### Requirement: Pi session id and project
A pi log's session id SHALL be the `id` of the `session` line that starts it, else the UUID at the end of the file name, after the `_`. Its project is that line's `cwd`.

#### Scenario: pi log without an id
- **WHEN** a pi log's first line has no `id`
- **THEN** the id is the UUID at the end of the file name, after the `_`, such as `01a00c3b-cc83-7a8d-a89b-262bd2f8ecac` in `2026-08-16T20-20-40-708Z_01a00c3b-cc83-7a8d-a89b-262bd2f8ecac.jsonl`

### Requirement: Compaction marker lines
The compaction markers SHALL be Claude Code `system` lines with subtype `compact_boundary` (seen in 2.1.284–2.1.312), which apply to every chain of the transcript, Codex `compacted` lines or `event_msg` events whose type contains `compact` (none seen yet), pi `compaction` lines (seen in pi 0.84.2), and the OpenCode compaction messages of OpenCode compactions.

#### Scenario: pi compaction
- **WHEN** a pi `compaction` line comes between two answered requests
- **THEN** the later request is marked

#### Scenario: OpenCode compaction
- **WHEN** an OpenCode `compaction` message with `status` `completed` comes between two answered requests of a session
- **THEN** the later request is marked

### Requirement: Claude Code's cost counter
A Claude Code session's own cost SHALL be its counter in the `cost-state` lines of its transcript (seen in transcripts of Claude Code 2.1.284–2.1.312). The lines that share a `startTime` are one run of the counter, and lines without a `startTime` are one run together. The counter is the sum, over the runs, of the highest `totalCostUSD` in each.

#### Scenario: Lines without a start time
- **WHEN** a transcript's `cost-state` lines have no `startTime` and read $1.00, $3.00 and $2.00
- **THEN** the session's reported cost is $3.00

### Requirement: Pi's own cost
A pi session's own cost SHALL be the sum of `usage.cost.total` over the requests of Pi requests and usage (seen in pi 0.84.2).

#### Scenario: pi's own cost
- **WHEN** a pi log's responses carry `usage.cost.total` of $0.0049, $0.0120 and $0.0031
- **THEN** the session's reported cost is $0.02

#### Scenario: pi's cost of a local model
- **WHEN** every response of a pi log is from `provider` `local` and carries `cost.total` 0
- **THEN** the session's reported cost is $0.00 and its requests stay unpriced unless the user's prices name the model

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

### Requirement: Project of a folder
A session's project SHALL be placed from its folder by the first of these rules that gives an answer: a group that the log's own source names for the folder (see Codex chat folders); `temporary folders` (see Temporary folders); the repository of the folder (see Repository of a folder); a remembered repository (see Repositories remembered); else the folder itself. No other rule is tried, such as the layout of a path, a folder's name, a branch or a remote.

#### Scenario: A folder of no repository
- **WHEN** a session ran in `~/.blume/harnessrunsqdfpv5`, which is in no repository, no temporary folder and no group
- **THEN** its project is `~/.blume/harnessrunsqdfpv5`

#### Scenario: A temporary folder inside a repository
- **WHEN** a session ran in `/tmp/demo`, and `/tmp/demo` holds a `.git` folder
- **THEN** its project is `temporary folders`, since that rule comes first

### Requirement: Temporary folders
A folder at or below a temporary root SHALL have the project `temporary folders`. The roots are the system's temporary folder as the platform names it to tokprof (on macOS `$TMPDIR`, such as `/var/folders/19/nlk_…/T`), the same folder with its links resolved (`/private/var/folders/19/nlk_…/T`), and on macOS and Linux also `/tmp` and `/private/tmp`. These are the roots whose sessions the rhythm leaves out (see work-rhythm). No other folder is temporary.

#### Scenario: A harness run in the temporary folder
- **WHEN** `$TMPDIR` is `/var/folders/19/nlk_x/T/`, and a session ran in `/private/var/folders/19/nlk_x/T/cmsg-acp-3fa2`
- **THEN** its project is `temporary folders`, and its folder is `/private/var/folders/19/nlk_x/T/cmsg-acp-3fa2`

#### Scenario: A folder under /tmp
- **WHEN** a session ran in `/private/tmp/ttl-check-run`
- **THEN** its project is `temporary folders`

#### Scenario: Another user's temporary folder
- **WHEN** a session ran in `/var/folders/7k/other_y/T/run`, which is not under `$TMPDIR` or its resolved form
- **THEN** it is not a temporary folder, and the next rules place it

#### Scenario: A folder named tmp elsewhere
- **WHEN** a session ran in `~/src/app/tmp`
- **THEN** it is not a temporary folder

### Requirement: Repository of a folder
Walking up from the folder to the nearest existing folder with a `.git` entry, never reading one in the home directory or a folder that holds it, the project SHALL be: that folder, when `.git` is a folder or a file whose git folder has no `commondir`; for a worktree, whose `.git` file's `gitdir:` names a git folder with `commondir`, the folder above the git folder that `commondir` names, or that git folder when it is not named `.git`. Relative paths are read from the file's folder.

#### Scenario: The worktree's root
- **WHEN** `~/.branch-deck/pool/app-c821e1d7/4/app/.git` reads `gitdir: /Users/me/src/app/.git/worktrees/app3`, and that folder's `commondir` reads `../..`
- **THEN** a session in `~/.branch-deck/pool/app-c821e1d7/4/app` has the project `~/src/app`

#### Scenario: A named copy that is a worktree
- **WHEN** `~/src/app-2/.git` reads `gitdir: /Users/me/src/app/.git/worktrees/app-2`
- **THEN** a session in `~/src/app-2` has the project `~/src/app`

#### Scenario: A subfolder of a repository
- **WHEN** a session ran in `~/src/app/plugins/web`, and `~/src/app/.git` is a folder
- **THEN** its project is `~/src/app`, and its folder `~/src/app/plugins/web`

#### Scenario: A subfolder of a worktree
- **WHEN** a session ran in `~/.branch-deck/pool/app-c821e1d7/4/app/plugins/web`
- **THEN** its project is `~/src/app`

#### Scenario: A folder that is gone inside a repository that exists
- **WHEN** a session ran in `~/src/app/out/run-17`, that folder is gone, and `~/src/app` exists
- **THEN** its project is `~/src/app`

#### Scenario: A repository inside a worktree
- **WHEN** a session ran in `~/.branch-deck/pool/app-c821e1d7/5/app/out/demo`, which holds a `.git` folder of its own
- **THEN** its project is `~/.branch-deck/pool/app-c821e1d7/5/app/out/demo`

#### Scenario: A submodule
- **WHEN** a session ran in `~/src/app/lib/sub`, and `~/src/app/lib/.git` is a file whose `gitdir:` names `/Users/me/src/app/.git/modules/lib`, which has no `commondir` file
- **THEN** its project is `~/src/app/lib`

#### Scenario: A bare repository
- **WHEN** a worktree's `commondir` names `/Users/me/src/app.git`, a git folder that is not named `.git`
- **THEN** its sessions have the project `~/src/app.git`

#### Scenario: The home directory in git
- **WHEN** `~/.git` is a folder, and a session ran in `~/Documents/notes`, which is in no other repository
- **THEN** its project is `~/Documents/notes`, not `~`

#### Scenario: Nothing to read
- **WHEN** the `.git` file, its git folder or `commondir` cannot be read
- **THEN** this rule gives no answer

### Requirement: Repositories remembered
tokprof SHALL keep each repository root and each worktree root it finds, with the project it gave, in `repositories-1.json` in its cache folder (see activity-calendar, Day store). The file holds those paths and no other text. A folder that the rule of Repository of a folder cannot place, at or below a remembered root, takes the project of the nearest such root. A file that cannot be read counts as empty, and one that cannot be written stays as it is; neither makes a command fail.

#### Scenario: A worktree deleted later
- **WHEN** a session ran in `~/.air/worktrees/temporary/app-shiny-raven`, tokprof found it to be a worktree of `~/src/app` on 2026-11-12, and the worktree was removed the day after
- **THEN** the session's project is still `~/src/app` on 2026-11-20

#### Scenario: A worktree never seen
- **WHEN** a session ran in `~/.codex/worktrees/0dca/app`, removed before any tokprof run found it
- **THEN** its project is `~/.codex/worktrees/0dca/app`

#### Scenario: A root that git places elsewhere now
- **WHEN** `repositories-1.json` holds `~/w/1/app` with `~/src/app`, and `~/w/1/app` is now a worktree of `~/src/other`
- **THEN** the project is `~/src/other`, and the file holds `~/src/other` for `~/w/1/app` from then on

#### Scenario: A cache folder that cannot be written
- **WHEN** the cache folder is read-only
- **THEN** each command prints what it would print otherwise, with the repositories on disk placed, and none of the commands fails

#### Scenario: Another cache folder
- **WHEN** `$TOKPROF_CACHE_DIR` is `/data/cache`
- **THEN** the roots are kept in `/data/cache/repositories-1.json`

### Requirement: Codex chat folders
A Codex log whose folder is `<home>/Documents/Codex/<YYYY-MM-DD>/<name>`, or a folder below it, where `<YYYY-MM-DD>` is a date, SHALL have the project `codex chats`. Codex Desktop makes such a folder for each chat that has no project; its rollouts were seen with `originator` `Codex Desktop` and `cli_version` 0.147.0-alpha.6.5 to 0.166.0-alpha.5.1. The rule belongs to Codex logs only.

#### Scenario: A chat folder
- **WHEN** a Codex rollout's `cwd` is `/Users/me/Documents/Codex/2026-11-02/cltk`
- **THEN** its project is `codex chats`, and its folder `~/Documents/Codex/2026-11-02/cltk`

#### Scenario: Another agent in the same folder
- **WHEN** a Claude Code transcript's first `cwd` is `/Users/me/Documents/Codex/2026-11-02/cltk`
- **THEN** the rule does not apply, and the next rules place it

#### Scenario: Not a date
- **WHEN** a Codex rollout's `cwd` is `/Users/me/Documents/Codex/notes`
- **THEN** the rule does not apply
