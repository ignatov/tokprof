# Spec Delta

## MODIFIED Requirements

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
