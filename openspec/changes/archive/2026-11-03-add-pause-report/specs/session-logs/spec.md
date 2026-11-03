# Spec Delta

## MODIFIED Requirements

### Requirement: Commands the filters apply to
`--since`, `--until`, `--project`, `--session` and `--client` SHALL apply to every command but `prices`. The reports `summary`, `misses`, `whatif`, `tools`, `pauses`, `flame` and `bench` count the requests they select. The default command, `scan`, `handoff`, `sessions` and `ui` pick or list only the sessions they select: project and id as for the reports, the client as the Client filter requirement says, a main log last written on or after `--since`, and created before `--until`.

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

### Requirement: Line about the logs read
`summary`, `misses`, `whatif`, `tools`, `pauses`, `flame` and `bench` SHALL print one line to stderr after reading the logs: `read <agent> <N> files (<X.X> GB), … in <S.S>s`, for each agent with at least one file, with sizes on disk in units of 10^9 bytes. It counts every log file read, including subagent logs and logs that the project, id or time filters then leave out; `bench` does not read subagent logs.

#### Scenario: Both agents
- **WHEN** 1,234 Claude Code files of 5.6 GB and 78 Codex files of 0.9 GB are read in 3.2 seconds
- **THEN** the line is `read claude 1234 files (5.6 GB), codex 78 files (0.9 GB) in 3.2s`

#### Scenario: Compressed rollouts
- **WHEN** some of the rollouts read are `.jsonl.zst`
- **THEN** they count at their compressed size

#### Scenario: Filter that matches nothing
- **WHEN** `--project` matches no session
- **THEN** the line still counts every log that was read
