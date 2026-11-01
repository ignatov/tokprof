## MODIFIED Requirements

### Requirement: Latest session by default
Without a target, `tokprof` and `tokprof scan` SHALL X-ray the main session log that was modified most recently among the sessions that `--agent` (default `claude,codex`) and the other filters select (see session-logs). Main logs are Claude Code transcripts outside a `subagents` directory and Codex rollouts whose `session_meta` line names no `parent_thread_id`. Where the logs are found belongs to session-logs.

#### Scenario: No command
- **WHEN** the user runs `tokprof` with no command
- **THEN** it prints the same X-ray as `tokprof scan` with no target

#### Scenario: A subagent wrote last
- **WHEN** the most recently modified log is a Claude Code subagent transcript or a Codex subagent rollout
- **THEN** the X-ray is of the most recently modified main session log, not of the subagent log

#### Scenario: One agent only
- **WHEN** the user runs `tokprof --agent codex` and a Claude Code session is newer than every Codex session
- **THEN** the X-ray is of the latest Codex session

#### Scenario: No session logs
- **WHEN** the chosen agents have no main session log
- **THEN** the command fails with the error `no session logs found` and a non-zero exit code

#### Scenario: A project filter
- **WHEN** the user runs `tokprof --project billing`
- **THEN** the X-ray is of the latest main session of a project whose path contains `billing`

### Requirement: Target resolution
`tokprof scan <target>` SHALL X-ray the file at `<target>` when it is an existing file, and otherwise the selected main session whose id starts with `<target>`, ignoring case, as session-logs resolves a target. A file named `rollout-*.jsonl` or `rollout-*.jsonl.zst` is read as a Codex rollout, any other file as a Claude Code transcript.

#### Scenario: Claude Code id prefix
- **WHEN** the user runs `tokprof scan 3f951de0`
- **THEN** the X-ray is of the transcript `3f951de0-14bc-4966-880b-81f649966ccf.jsonl`

#### Scenario: Codex thread id
- **WHEN** the user runs `tokprof scan 019ff5ca` and one Codex thread id starts with it
- **THEN** the X-ray is of the rollout `rollout-2026-08-12T13-44-47-019ff5ca-0db2-7180-8f61-e241e484f21a.jsonl`

#### Scenario: Several sessions share the prefix
- **WHEN** the target matches several main sessions, as Codex thread ids made minutes apart share their first 8 characters
- **THEN** the command fails and lists the matching sessions by their short ids, so a longer prefix can be given

#### Scenario: Compressed Codex rollout by path
- **WHEN** the target is the path of an archived `rollout-*.jsonl.zst`
- **THEN** the rollout is decompressed and read as a Codex session

#### Scenario: Subagent transcript by path
- **WHEN** the target is the path of a Claude Code subagent transcript
- **THEN** that transcript is X-rayed on its own, without a main conversation

#### Scenario: Nothing matches
- **WHEN** the user runs `tokprof scan zzzz` and no file or session id matches
- **THEN** the command fails with ``no session file or session id matches "zzzz"; see `tokprof sessions` `` and a non-zero exit code
