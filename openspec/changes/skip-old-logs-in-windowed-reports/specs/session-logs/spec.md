# Spec Delta

## MODIFIED Requirements

### Requirement: Time window
`--since` and `--until` SHALL select requests, not sessions, by the request's timestamp: on or after `--since` and strictly before `--until`. `--since` takes `YYYY-MM-DD` or `<N>d` for the last N×24 hours; `--until` takes `YYYY-MM-DD` only. A date means local midnight at its start. `summary`, `misses`, `whatif`, `tools` and `flame` SHALL NOT read a log last written before `--since`, as it holds no request on or after it; every other command reads the logs it read before.

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

#### Scenario: A log last written before the window
- **WHEN** `tokprof summary --since 2026-11-01` runs and a log was last written on 2026-10-20
- **THEN** that log is not read, and the report's numbers are those of a run that read it

#### Scenario: A log written inside the window
- **WHEN** a log was first written on 2026-10-20 and last written on 2026-11-02, and `--since 2026-11-01` is given
- **THEN** the log is read, and only its requests from 2026-11-01 on are counted

#### Scenario: A log created after the end of the window
- **WHEN** `--since 2026-10-01 --until 2026-10-16` is given and a log was created on 2026-10-20, as a rollout compressed afterwards is
- **THEN** the log is read: neither `--until` nor the time a log was created leaves a log unread

#### Scenario: Commands that read the logs before the window
- **WHEN** `tokprof pauses --since 7d` or `tokprof bench --since 7d` runs
- **THEN** the logs last written before the window are read as well, for the nights of the rhythm and the starts of the conversations

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

#### Scenario: Logs before the window
- **WHEN** `tokprof summary --since 7d` runs over 1,234 Claude Code files of which 40, of 0.3 GB, were last written in the last 7 days
- **THEN** the line is `read claude 40 files (0.3 GB) in <S.S>s`: the logs that were not read do not count

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

#### Scenario: An unreadable log before the window
- **WHEN** `tokprof summary --since 7d` runs and a log last written 30 days ago cannot be decompressed
- **THEN** no warning names it, since it is not read
- **AND** `tokprof summary` without `--since` prints the warning

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

#### Scenario: Logs only before the window
- **WHEN** `tokprof summary --since 1d` runs and every log was last written more than a day ago
- **THEN** it prints `read  in 0.0s` and its empty report, with exit status 0
