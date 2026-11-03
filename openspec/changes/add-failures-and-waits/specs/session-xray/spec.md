## ADDED Requirements

### Requirement: Waits block
When the session's agent was busy for a minute or more (see agent-time), the X-ray SHALL print a `waits` block after the `work` block. It starts with `waits     busy <time>: what the agent waited on in its turns, and while its own background work ran after them`. One line per kind of busy time follows, most time first, with a bar, its share and its time. The seventh and later kinds fold into one `<n> more` line.

#### Scenario: The lines
- **WHEN** the agent was busy 6.1h, 1.5h of it for subagents, 1.1h of that after the end of a turn
- **THEN** the block starts with `waits     busy 6.1h: …` and has a `subagents` line with a bar, `24.6%`, `1.5h` and the note `1.1h after the turn ended`

#### Scenario: Notes
- **WHEN** the block has the kinds `model` and `API retries`
- **THEN** the `model` line ends with the number of main requests, as `412 requests`
- **AND** the `API retries` line ends with the number of requests that waited and the last error that most of them had, cut to 50 characters, as `6 requests, most often 502 local proxy: upstream request failed`

#### Scenario: A short session
- **WHEN** the agent was busy for 40 seconds
- **THEN** there is no `waits` block

#### Scenario: Durations
- **WHEN** a time is shown in the block
- **THEN** it reads like the rhythm's: `45s` under a minute, `6.5m` under 10 minutes, `34m` under an hour, `1.8h` under a day, and `6.9d` beyond

### Requirement: Slow requests line
The waits block SHALL go on with a line on the slow requests of the session, its subagents included (see agent-time, Slow requests): how many there were, how many of them were in the main conversation, and how long the longest one took and how many tokens it wrote.

#### Scenario: Slow requests
- **WHEN** 103 requests took a minute or more, 17 of them in the main conversation, and the longest took 6m 12s with 41.2k output tokens
- **THEN** the line reads `  slow: 103 requests took a minute or more, 17 of them in the main conversation; the longest took 6.2m and wrote 41.2k tokens`

#### Scenario: No slow request
- **WHEN** no request took a minute or more
- **THEN** the line reads `  slow: no request took a minute or more`

### Requirement: Longest waits line
The waits block SHALL end with the three longest waits of the session (see agent-time, Waits): each with its time, its kind and its start as `MM-DD HH:MM`, joined by ` · `. With no wait there is no such line.

#### Scenario: The line
- **WHEN** the longest waits are 1.8h for background commands from 2026-10-14 10:12, 25m for subagents from 2026-10-12 20:59 and 14m for background commands from 2026-10-12 20:40
- **THEN** the line reads `  longest waits: 1.8h for background commands at 10-14 10:12 · 25m for subagents at 10-12 20:59 · 14m for background commands at 10-12 20:40`

#### Scenario: One wait
- **WHEN** the session has one wait
- **THEN** the line names that one

### Requirement: Failures block
When a call of the session, its subagents included, failed, or the user refused or a rule denied one (see tool-breakdown), the X-ray SHALL print a `failures` block after the `tools` block: `failures  calls that failed, and what the requests after them cost until the tool worked again`. Up to 5 rows follow, most `after` first and then most `failed`, with `tool`, `failed`, `of calls`, `after` and `latest error`. Then come `<n> more` and the failures line (see tool-breakdown).

#### Scenario: A tool that never worked
- **WHEN** `advisor` failed in 15 of 15 calls, its latest error line was `Error: No such tool available: advisor`, and its windows cost $32.92
- **THEN** its row reads `advisor`, `15`, `100%`, `$32.92` and `Error: No such tool available: advisor`

#### Scenario: A long error line
- **WHEN** the latest error line of a row is longer than 60 characters
- **THEN** it is cut to 60 characters with `…`

#### Scenario: Refusals only
- **WHEN** no call failed and the user refused 2 calls
- **THEN** the block has no rows, and its only line after the title is `  you refused 2`

#### Scenario: No failures
- **WHEN** every call of the session worked
- **THEN** there is no `failures` block

#### Scenario: The tools block
- **WHEN** the X-ray prints the `tools` block
- **THEN** its rows keep the columns that block names, without `failed` and `after`

## MODIFIED Requirements

### Requirement: Layout
The X-ray SHALL print, in this order and separated by blank lines: the header; the `context`, `cache` and `next` lines with the advice; `spend`; the `work`, `waits`, `prompts`, `tools`, `failures` and `setup` blocks when they are shown; `in context`; `what cost the most`; the cache misses; and the `profile` pointer (see the scenario The profile pointer). The parts about the live context need a main conversation.

#### Scenario: The profile pointer
- **WHEN** the X-ray ends
- **THEN** its last line is `profile: tokprof flame --session <id> → <dir>/<id>.jfr (IntelliJ: Open snapshot)`, where `<id>` is the session's short id (see session-logs) and `<dir>` is `~/tokprof`, or `$TOKPROF_DIR` when set

#### Scenario: Session with a main conversation
- **WHEN** the main log has a request with a prompt
- **THEN** every part is printed in the order above

#### Scenario: No main conversation
- **WHEN** the main log has no request with a prompt, for example when only its subagents made requests or the target is a subagent transcript
- **THEN** the `context`, `cache` and `next` lines, the advice and `in context` are left out
- **AND** the header, `spend`, the `work`, `waits`, `prompts`, `tools`, `failures` and `setup` blocks when they are shown, `what cost the most`, the cache misses and the pointer are still printed

#### Scenario: Session without requests
- **WHEN** the log holds no request with usage
- **THEN** there is no `models` line, the `time` line says `0 requests`, `spend` shows `0 tokens (no price)` and the misses line says `no misses`
