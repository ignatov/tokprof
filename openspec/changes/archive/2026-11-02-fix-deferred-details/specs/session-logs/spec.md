## MODIFIED Requirements

### Requirement: Sessions and subagent logs
tokprof SHALL treat a session as one main log together with the logs of the subagents it spawned; a subagent log is never a session of its own. In Claude Code, any transcript below a `subagents` directory is a subagent log, and `<session>/subagents/` belongs to `<session>.jsonl`. In Codex, a rollout whose `session_meta` names a `parent_thread_id` (seen in Codex 0.146–0.163) belongs to the session of that thread, which is the topmost thread above it whose rollout is found.

#### Scenario: Claude Code subagent transcripts
- **WHEN** `<project>/s1.jsonl` and `<project>/s1/subagents/agent-a1.jsonl` exist
- **THEN** session `s1` has one subagent log
- **AND** `agent-a1.jsonl` is neither listed as a session nor picked as the latest session

#### Scenario: Codex subagent thread
- **WHEN** a rollout's `session_meta` has the `parent_thread_id` `P`
- **THEN** it belongs to the session of the rollout whose thread id is `P`, whether that rollout is in `sessions/` or in `archived_sessions/`

#### Scenario: Nested Codex subagent threads
- **WHEN** main thread `M` spawned subagent thread `A`, and `A` spawned thread `B`
- **THEN** `A` and `B` are both subagent logs of session `M`
- **AND** the X-ray of `M` counts both of them and their requests

#### Scenario: Parent log not on disk
- **WHEN** a Codex rollout names a parent thread whose rollout is not found
- **THEN** it is not listed as a session
- **AND** `summary`, `misses`, `whatif` and `flame` still count its requests

#### Scenario: Rollout without session_meta
- **WHEN** a rollout's first line is not a `session_meta` line with an `id`
- **THEN** the rollout is a main log

### Requirement: Session id and project
Each log SHALL carry a session id and a project read from the log. The id is the `sessionId` of Claude Code `user` and `assistant` lines, or the `id` of the Codex `session_meta` payload. A Codex subagent rollout carries the id of the session it belongs to (see Sessions and subagent logs) instead of its own. The project is the first `cwd` of those Claude Code lines, or of Codex `session_meta` and `turn_context` lines, written with `~` for the home directory.

#### Scenario: Claude Code subagent transcript
- **WHEN** a subagent transcript's lines carry the parent's `sessionId`
- **THEN** its requests are reported under the parent session's id

#### Scenario: Codex subagent thread
- **WHEN** a Codex subagent rollout of session `M` is read
- **THEN** its requests are reported under `M`'s id, not under its own thread id
- **AND** `tokprof misses` lists its misses with `M`'s short id and the thread's chain, such as `subagent:review`

#### Scenario: Codex thread whose parent is not on disk
- **WHEN** a Codex rollout names the parent thread `P`, and no rollout of `P` is found
- **THEN** its requests are reported under `P`

#### Scenario: No id in the log
- **WHEN** a Claude Code transcript has no `sessionId`, or a Codex rollout has no `session_meta` id
- **THEN** the id comes from the file name: the transcript's name without `.jsonl`, or the 36-character thread id at the end of the rollout's name

#### Scenario: Project under the home directory
- **WHEN** the first `cwd` is `/Users/me/src/app` and the home directory is `/Users/me`
- **THEN** the project is `~/src/app`

### Requirement: Session filter
`--session` SHALL keep only the sessions whose id starts with the given text, ignoring case, together with their subagent logs.

#### Scenario: Claude Code session with subagents
- **WHEN** `--session 9f51246e` is given and that session has 9 subagent transcripts
- **THEN** 10 logs are read and all of them are kept, since the subagent transcripts carry the parent's id

#### Scenario: Codex session with subagent threads
- **WHEN** `--session` gives the id prefix of a Codex thread that spawned subagents
- **THEN** the parent's rollout and the rollouts of every thread below it are kept
- **AND** all of their requests are reported under the parent's id
