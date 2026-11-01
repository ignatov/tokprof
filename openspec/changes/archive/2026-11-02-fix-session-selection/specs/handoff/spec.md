## MODIFIED Requirements

### Requirement: Handoff line
`tokprof handoff [target]` SHALL print one line, `Continue from the earlier session <id>. Its log: <path>.`, for the session the target names, resolved as session-xray resolves a target, or for the latest main session when no target is given. `<id>` is the full session id: a Claude Code transcript's file name without `.jsonl`, or the thread id that ends a Codex rollout's name. `<path>` is the log's absolute path with the home directory shown as `~`.

#### Scenario: Claude Code session
- **WHEN** the user runs `tokprof handoff 3f951de0`
- **THEN** it prints `Continue from the earlier session 3f951de0-14bc-4966-880b-81f649966ccf. Its log: ~/.claude/projects/<project>/3f951de0-14bc-4966-880b-81f649966ccf.jsonl.`

#### Scenario: Codex session
- **WHEN** the user runs `tokprof handoff 019ff5ca`
- **THEN** it prints `Continue from the earlier session 019ff5ca-0db2-7180-8f61-e241e484f21a. Its log: ~/.codex/sessions/2026/08/12/rollout-2026-08-12T13-44-47-019ff5ca-0db2-7180-8f61-e241e484f21a.jsonl.`

#### Scenario: Compressed Codex rollout
- **WHEN** the session's log is an archived `rollout-*.jsonl.zst`
- **THEN** the id is the thread id without `.jsonl.zst`, and the path is that of the `.jsonl.zst` file

#### Scenario: No target
- **WHEN** the user runs `tokprof handoff` without a target
- **THEN** the line is for the latest main session, the one `tokprof` X-rays

#### Scenario: Log outside the home directory
- **WHEN** the log's path is not under the home directory
- **THEN** the path is printed unchanged

#### Scenario: Unknown target or no logs
- **WHEN** the target matches nothing, or the chosen agents have no session logs
- **THEN** the command fails with the same errors as `tokprof scan`

#### Scenario: A relative path as the target
- **WHEN** the user runs `tokprof handoff logs/x.jsonl` in `/work`
- **THEN** the path in the line is `/work/logs/x.jsonl`, so a session started elsewhere can open it

### Requirement: Note header
The note SHALL open with `# Handoff from <agent> session <short id>`, where `<agent>` is `Claude Code` or `Codex` and `<short id>` the session's short id (see session-logs). Then come `Project <project>, branch <branch>; <first> → <last>.`, with the local `YYYY-MM-DD HH:MM` of the first and last message, and `The full log is <size> MB: search it only for details this note lacks.`, with the log file's size on disk.

#### Scenario: Claude Code header
- **WHEN** the log is a Claude Code transcript
- **THEN** the project is the first `cwd` in the log, with `~` for the home directory, and the branch is the latest non-empty `gitBranch`
- **AND** the first and last times are those of its `user` and `assistant` lines

#### Scenario: Codex header
- **WHEN** the log is a Codex rollout
- **THEN** the project and the branch come from `session_meta`, the branch marked as of the start, as in `Project ~/src/monorepo-2, branch monorepo-2 at the start; 2026-08-12 13:44 → 2026-08-12 14:29.`
- **AND** the first and last times are those of its `response_item` lines

#### Scenario: Unknown project, branch and times
- **WHEN** the log names no working directory, no branch and no times
- **THEN** the line reads `Project ?; ? → ?.`

#### Scenario: Compressed log size
- **WHEN** the log is a `.jsonl.zst` rollout
- **THEN** the size is that of the compressed file, as in `The full log is 0.2 MB: search it only for details this note lacks.`
