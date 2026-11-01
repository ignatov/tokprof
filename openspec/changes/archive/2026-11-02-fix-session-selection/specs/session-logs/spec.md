## ADDED Requirements

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

## MODIFIED Requirements

### Requirement: Session id and project
Each log SHALL carry a session id and a project read from the log. The id is the `sessionId` of Claude Code `user` and `assistant` lines, or the `id` of the Codex `session_meta` payload. The project is the first `cwd` of those Claude Code lines, or of Codex `session_meta` and `turn_context` lines, written with `~` for the home directory.

#### Scenario: Claude Code subagent transcript
- **WHEN** a subagent transcript's lines carry the parent's `sessionId`
- **THEN** its requests are reported under the parent session's id

#### Scenario: Codex subagent thread
- **WHEN** a Codex subagent rollout is read
- **THEN** its requests are reported under its own thread id, not under its parent's

#### Scenario: No id in the log
- **WHEN** a Claude Code transcript has no `sessionId`, or a Codex rollout has no `session_meta` id
- **THEN** the id comes from the file name: the transcript's name without `.jsonl`, or the 36-character thread id at the end of the rollout's name

#### Scenario: Project under the home directory
- **WHEN** the first `cwd` is `/Users/me/src/app` and the home directory is `/Users/me`
- **THEN** the project is `~/src/app`

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
- **THEN** the parent's rollout and the rollouts of its subagent threads are kept
- **AND** the subagent threads are still reported under their own ids
