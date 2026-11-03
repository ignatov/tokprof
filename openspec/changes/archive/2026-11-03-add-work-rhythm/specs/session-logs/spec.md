# Spec Delta

## ADDED Requirements

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

## MODIFIED Requirements

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
`summary`, `misses`, `whatif`, `tools`, `pauses`, `flame`, `bench` and `day` SHALL print one line to stderr after reading the logs: `read <agent> <N> files (<X.X> GB), … in <S.S>s`, for each agent with at least one file, with sizes on disk in units of 10^9 bytes. It counts every log file read, including subagent logs and logs that the project, id or time filters then leave out; `bench` and `day` read no subagent logs, and `day` only logs written in the 30 days before its day.

#### Scenario: Both agents
- **WHEN** 1,234 Claude Code files of 5.6 GB and 78 Codex files of 0.9 GB are read in 3.2 seconds
- **THEN** the line is `read claude 1234 files (5.6 GB), codex 78 files (0.9 GB) in 3.2s`

#### Scenario: Compressed rollouts
- **WHEN** some of the rollouts read are `.jsonl.zst`
- **THEN** they count at their compressed size

#### Scenario: Filter that matches nothing
- **WHEN** `--project` matches no session
- **THEN** the line still counts every log that was read
