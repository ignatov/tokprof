# Spec Delta

## Purpose

Shows, per tool and shell command, how often an agent called it, how big its results and inputs were, what they cost and which habit to change, so the user can tell whether a session could have been cheaper.

## ADDED Requirements

### Requirement: Rows by tool and command
The tool breakdown SHALL have one row per tool, the session's subagents included. A shell tool, such as Claude Code `Bash` or Codex `exec_command`, gets one row per command head (see cost-attribution): `Bash sed -n`, `exec_command cargo test`. A Codex `exec` call whose code calls one inner tool shows as that tool, as cost-attribution names it; one that calls several shows as `exec`.

#### Scenario: Shell commands
- **WHEN** a session ran `sed -n 1,80p a.rs` 56 times and `grep -n foo src` 40 times through `Bash`
- **THEN** the rows are `Bash sed -n` with 56 calls and `Bash grep -n` with 40 calls

#### Scenario: Other tools
- **WHEN** a session called `Read` on 3 different files
- **THEN** they make one `Read` row with 3 calls, whatever the files

### Requirement: Sizes per call
`avg result` and `avg input` SHALL be the mean tokens of a row's tool results and of its call inputs, each block counted once, at the size it had in the first request that held it. A row whose sizes the log cannot rebuild shows `–`.

#### Scenario: A large read
- **WHEN** one `Read` returned a 16,900-token file
- **THEN** its row shows `1` call and `16.9k` as `avg result`

### Requirement: Rent
A row's `rent` SHALL be the spend charged to its tool results and tool calls by cost-attribution: every component, over every request that held them, rewrites after misses included.

#### Scenario: Results kept for a long time
- **WHEN** 56 `sed -n` results of 1,300 tokens each stayed in a growing context for the rest of the session
- **THEN** the `Bash sed -n` row's rent is the sum of what each result cost in every request after it

### Requirement: Trips
A row's `trips` SHALL be the input spend of the requests that its calls' results triggered, everything but their output, plus the output of the responses that made the calls. A request is triggered by the tool results that its prompt carries for the first time. When a response made several calls, its spend and the next request's are split equally between them.

#### Scenario: Calls in one response
- **WHEN** a response made a `Read` and a `Bash grep -n` call, its output cost $0.10, and the next request's input cost $0.30
- **THEN** each row gets $0.20 of trips from that response

#### Scenario: A prompt without composition
- **WHEN** a request's prompt cannot be rebuilt from the log
- **THEN** no row gets its input spend as trips

#### Scenario: Both views add up
- **WHEN** the rows' rent is added to the spend of content no tool produced, and the rows' trips to the input of requests no tool result triggered and the output of responses without calls
- **THEN** each sum equals the spend

### Requirement: Two views of the same money
The table SHALL never add rent and trips. Its last line SHALL give the session's or window's spend, how much of it the rows' rent covers, and how much the rows' trips cover, so that each view can be compared with the whole.

#### Scenario: The total line
- **WHEN** a session cost $27.37, its rows' rent sums to $21.40, and their trips to $24.90
- **THEN** the line reads `spend $27.37 · rent of these tools $21.40 · trips of these tools $24.90 (two views of the same money; do not add them)`

### Requirement: Single calls
A row's `single` SHALL be the share of its calls that were the only call of their response.

#### Scenario: Reads one at a time
- **WHEN** 48 of 56 `sed -n` calls were alone in their response
- **THEN** the row shows `86%`

### Requirement: Hints
Each row SHALL get the first hint whose rule applies, in the order of the rule scenarios below, or none. The rules apply only to rows with at least 5 calls and at least $1 of rent or trips.

#### Scenario: Rule 1, reads made one per response
- **WHEN** a read-only row, a read, search or fetch tool or a shell row whose command head is read-only (see cost-attribution), has `single` at 60% or more
- **THEN** its hint is `batch independent reads`

#### Scenario: Rule 2, long inline scripts
- **WHEN** a shell row that is not read-only has an `avg input` of 400 tokens or more
- **THEN** its hint is `save long scripts to a file; edit files with the edit tool`

#### Scenario: Rule 3, large reads
- **WHEN** a read row, such as `Read`, has an `avg result` of 5,000 tokens or more
- **THEN** its hint is `read a range or search instead of the whole file`

#### Scenario: Rule 4, long command output
- **WHEN** a shell row has an `avg result` of 2,000 tokens or more
- **THEN** its hint is `filter the output with head or grep`

#### Scenario: Rule 5, whole-file rewrites
- **WHEN** a whole-file write row, such as `Write`, has an `avg input` of 2,000 tokens or more
- **THEN** its hint is `edit instead of rewriting the file`

#### Scenario: Reads one per response
- **WHEN** `Bash sed -n` has 56 calls, 86% single, and $5.21 of rent
- **THEN** its hint is `batch independent reads`

#### Scenario: Inline scripts
- **WHEN** `Bash python3 -I` has 60 calls with an `avg input` of 702 tokens and $4.32 of rent
- **THEN** its hint is `save long scripts to a file; edit files with the edit tool`

#### Scenario: A large read, once
- **WHEN** `Read` has 1 call of 16,900 tokens
- **THEN** it has no hint, since rules need at least 5 calls

#### Scenario: Small rows
- **WHEN** a row has 4 calls, or less than $1 of both rent and trips
- **THEN** it has no hint

### Requirement: The tools command
`tokprof tools` SHALL print `Spend by tool: calls, sizes, what keeping them cost (rent) and what they made the agent do (trips)` and the table over the requests the filters select (see session-logs): `tool`, `calls`, `avg result`, `avg input`, `rent`, `trips`, `single` and `hint`. Rows are sorted by rent, or by trips with `--by trips`. The top 25 rows are shown, then `<n> more`, then the total line.

#### Scenario: Sorted by trips
- **WHEN** the user runs `tokprof --since 7d tools --by trips`
- **THEN** the row whose calls triggered the most spend comes first

#### Scenario: Unknown sort
- **WHEN** the user passes `--by size`
- **THEN** the command fails with a usage error that lists `rent, trips`

#### Scenario: No prices
- **WHEN** nothing in the window is priced
- **THEN** `rent` and `trips` show tokens and the rows are sorted by them
