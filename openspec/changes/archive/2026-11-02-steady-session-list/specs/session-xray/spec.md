# Spec Delta

## MODIFIED Requirements

### Requirement: Latest session by default
Without a target, `tokprof` and `tokprof scan` SHALL X-ray the main session with the latest last activity (see session-logs) among the sessions that `--agent` (default `claude,codex`) and the other filters select. Main logs are Claude Code transcripts outside a `subagents` directory and Codex rollouts whose `session_meta` line names no `parent_thread_id`. Where the logs are found belongs to session-logs.

#### Scenario: No command
- **WHEN** the user runs `tokprof` with no command
- **THEN** it prints the same X-ray as `tokprof scan` with no target

#### Scenario: A subagent wrote last
- **WHEN** the most recently modified log is a Claude Code subagent transcript or a Codex subagent rollout
- **THEN** the X-ray is of the main session with the latest last activity, not of the subagent log

#### Scenario: An idle session written last
- **WHEN** Claude Code appended only records without a time to an idle session's transcript after another session's latest request
- **THEN** the X-ray is of the other session

#### Scenario: One agent only
- **WHEN** the user runs `tokprof --agent codex` and a Claude Code session is newer than every Codex session
- **THEN** the X-ray is of the latest Codex session

#### Scenario: No session logs
- **WHEN** the chosen agents have no main session log
- **THEN** the command fails with the error `no session logs found` and a non-zero exit code

#### Scenario: A project filter
- **WHEN** the user runs `tokprof --project billing`
- **THEN** the X-ray is of the latest main session of a project whose path contains `billing`
