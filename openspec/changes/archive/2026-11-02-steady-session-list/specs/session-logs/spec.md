# Spec Delta

## ADDED Requirements

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

## MODIFIED Requirements

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
