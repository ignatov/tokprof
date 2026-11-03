## ADDED Requirements

### Requirement: Failed, refused and denied calls
A row's `failed` SHALL count its calls that failed (see session-logs, Call outcomes). Calls that the user refused or a permission rule denied are not failures, and are counted apart for the whole table.

#### Scenario: A missing file
- **WHEN** 3 of a session's 120 `Read` calls returned `File does not exist`
- **THEN** the `Read` row shows `3` under `failed`

#### Scenario: A refused edit
- **WHEN** the user refused one `Edit` call
- **THEN** the `Edit` row does not count it as failed, and the table counts one refused call

### Requirement: What came after a failure
After a failed call, the window SHALL hold the requests of its chain from the first that carried the failure up to the first whose response made a call that worked of the same tool, or of any shell tool after a shell call, within one turn of a main conversation (see work-rhythm, Turns) or within a subagent's chain. A row's `after` is the spend of its windows. Failures before the working call share one window.

#### Scenario: Read again, then edited
- **WHEN** an `Edit` fails because its text was not found, the next request reads the file, and the request after it makes an `Edit` that works
- **THEN** the window holds those two requests, and their spend counts in the `Edit` row's `after`

#### Scenario: Tried again at once
- **WHEN** a `Bash cargo test` call fails, and the request that carried the failure runs `cargo test` again, which passes
- **THEN** the window holds that one request

#### Scenario: Another command of the shell
- **WHEN** a `Bash cargo test` call fails, and the request that carried the failure runs `git diff`, which works
- **THEN** the window holds that one request, since the shell worked again
- **AND** a command that appears much later in the turn does not stretch the window over the work between
- **AND** a Codex `write_stdin` that reads a failed command's exit ends its window at the next `exec_command` that works, since both are shell calls

#### Scenario: Two failures in a row
- **WHEN** a `Bash git push` call fails twice and the third works
- **THEN** the two failures share one window, from the first request that carried a failure to the request that made the call that worked

#### Scenario: Never worked again
- **WHEN** a call fails and no call of its tool works before the turn ends
- **THEN** the window is the one request that carried the failure

#### Scenario: Windows of two rows
- **WHEN** the windows of two rows hold the same request
- **THEN** each row's `after` counts it, and the failures line counts it once

### Requirement: Failures line
After the total line, `tokprof tools` SHALL print how many calls failed, how many the user refused and how many permission rules denied, and the spend of the failure windows of all rows, each request counted once. Counts that are zero are left out, and with no such call at all there is no line.

#### Scenario: The line
- **WHEN** 1,021 calls failed, the user refused 52, rules denied 101, and the windows hold $141.80 of spend
- **THEN** the line reads `failed 1,021 calls, after them $142 until the tool worked again · you refused 52 · rules denied 101`, with dollars written as everywhere in tokprof

#### Scenario: Only failures
- **WHEN** 3 calls failed and none was refused or denied
- **THEN** the line reads `failed 3 calls, after them $0.42 until the tool worked again`

## MODIFIED Requirements

### Requirement: The tools command
`tokprof tools` SHALL print `Spend by tool: calls, sizes, what keeping them cost (rent) and what they made the agent do (trips)` and the table over the requests the filters select (see session-logs): `tool`, `calls`, `avg result`, `avg input`, `rent`, `trips`, `single`, `failed`, `after` and `hint`. Rows are sorted by rent, by trips with `--by trips`, or by `failed` and then `after` with `--by failed`. The top 25 rows are shown, then `<n> more`, the total line and the failures line.

#### Scenario: Sorted by trips
- **WHEN** the user runs `tokprof --since 7d tools --by trips`
- **THEN** the row whose calls triggered the most spend comes first

#### Scenario: Sorted by failures
- **WHEN** the user runs `tokprof --since 30d tools --by failed`
- **THEN** the row with the most failed calls comes first

#### Scenario: Unknown sort
- **WHEN** the user passes `--by size`
- **THEN** the command fails with a usage error that lists `rent, trips, failed`

#### Scenario: No prices
- **WHEN** nothing in the window is priced
- **THEN** `rent`, `trips` and `after` show tokens and the rows are sorted by them

#### Scenario: No failures
- **WHEN** no call of a row failed
- **THEN** its `failed` and `after` cells are empty
