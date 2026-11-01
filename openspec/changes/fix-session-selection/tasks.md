# Tasks

## 1. Short ids

- [ ] 1.1 One short-id rule for both agents: 8 characters for Claude Code, 13 for Codex. Use it in the session model, the session list's quick read and the handoff note header. Verify: a unit test with two Codex ids from the same minute.

## 2. Targets

- [ ] 2.1 Match a target against the start of each main session's id (the Claude Code file stem, the thread id of a Codex rollout name), ignoring case. Drop the `-<text>` match. Verify: unit tests for an id prefix, the middle of an id and a rollout date.
- [ ] 2.2 Fail on a prefix shared by several selected sessions. List up to 10, newest first, with short id, time, agent and project, then `… and <n> more`. Verify: a unit test with two Codex rollouts from the same minute.

## 3. Filters

- [ ] 3.1 Normalize `--project` at startup: home directory to `~`, no trailing `/`. Verify: a unit test, and `default_profile_names_follow_the_filters` gains the trailing-slash case.
- [ ] 3.2 A session filter for the session commands: project, id prefix, main log modified on or after `--since`, created before `--until`. Apply it in the default command, `scan`, `handoff`, `sessions` and `ui`, including sessions that the ui's watcher adds. Verify: unit tests for each filter.
- [ ] 3.3 `--session` in reports keeps the subagent rollouts of a matched Codex session. Verify: `tokprof --session <codex id with subagents> summary` counts the subagent threads.

## 4. Handoff path

- [ ] 4.1 `tokprof handoff <relative path>` prints the absolute path. Verify: a unit test.

## 5. Checks and docs

- [ ] 5.1 Run `cargo test`, then the checks in the proposal on the real logs, and record the numbers in this file.
- [ ] 5.2 README: the filters table says which commands take them, `--project` accepts an absolute path, and Codex examples use 13-character ids.
