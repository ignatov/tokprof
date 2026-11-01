# Tasks

## 1. Short ids

- [x] 1.1 One short-id rule for both agents: 8 characters for Claude Code, the first three groups (18 characters) for Codex. Use it in the session model, the session list's quick read and the handoff note header. Verify: a unit test with two Codex ids from the same minute.

## 2. Targets

- [x] 2.1 Match a target against the start of each main session's id (the Claude Code file stem, the thread id of a Codex rollout name), ignoring case. Drop the `-<text>` match. Verify: unit tests for an id prefix, the middle of an id and a rollout date.
- [x] 2.2 Fail on a prefix shared by several selected sessions. List up to 10, newest first, with short id, time, agent and project, then `… and <n> more`. Verify: a unit test with two Codex rollouts from the same minute.

## 3. Filters

- [x] 3.1 Normalize `--project` at startup: home directory to `~`, no trailing `/`. Verify: a unit test, and `default_profile_names_follow_the_filters` gains the trailing-slash case.
- [x] 3.2 A session filter for the session commands: project, id prefix, main log modified on or after `--since`, created before `--until`. Apply it in the default command, `scan`, `handoff`, `sessions` and `ui`, including sessions that the ui's watcher adds. Verify: unit tests for each filter.
- [x] 3.3 `--session` in reports keeps the subagent rollouts of a matched Codex session. Verify: `tokprof --session <codex id with subagents> summary` counts the subagent threads.

## 4. Handoff path

- [x] 4.1 `tokprof handoff <relative path>` prints the absolute path. Verify: a unit test.

## 5. Checks and docs

- [x] 5.1 Run `cargo test`, then the checks in the proposal on the real logs, and record the numbers in this file.
- [x] 5.2 README: the filters table says which commands take them, `--project` accepts an absolute path, and Codex examples use 18-character ids.

## Results on the real logs (2026-11-01)

| Check | Snapshot commit | This change |
|---|---|---|
| Codex short ids shared by several sessions, all 793 rollouts | 46 rows in `sessions` | 0 |
| `tokprof scan 01a197e1` | the newest of 6, silently | fails and lists the 6 |
| `tokprof scan 2026`, `scan 14bc-4966` | a rollout by its date, the middle of an id | no match |
| `tokprof --session 3f951de0` | X-ray of the latest session | the same X-ray as `scan 3f951de0` |
| `--project $HOME/src/tokprof summary` | $0.00 | $247, as with `'~/src/tokprof'` |
| `--session 019ff5ca summary` (9 subagent threads) | 1 file, $16.37 | 10 files, $62.01 |
| `--since 7d sessions` | all 758 sessions | 121, the oldest written 2026-10-26 |

With 13 characters, the first draft, 11 Codex sessions still shared a short id, all launched in the same millisecond; three groups leave none (see design.md).
