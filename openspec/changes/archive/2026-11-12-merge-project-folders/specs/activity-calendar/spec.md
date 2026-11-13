## MODIFIED Requirements

### Requirement: What an entry names
An entry SHALL name its log's path, length and modification time, the fingerprint it was made with, the log's agent, its session id and the id of its main session, whether it is a subagent's log, its project, the session's folder when it differs from the project (see session-logs, Session id and project), and whether the rhythm leaves its session out (see work-rhythm, Sessions left out).

#### Scenario: A subagent's log
- **WHEN** a subagent of session `3f951de0` wrote a log
- **THEN** its entry names `3f951de0` as its main session and is marked as a subagent's log

#### Scenario: A log of a worktree
- **WHEN** a session ran in `~/.branch-deck/pool/app-c821e1d7/4/app`, a worktree of `~/src/app`
- **THEN** its entry names the project `~/src/app` and the folder `~/.branch-deck/pool/app-c821e1d7/4/app`

#### Scenario: A log of a temporary folder
- **WHEN** a session ran in `/private/tmp/ttl-check-run`
- **THEN** its entry names the project `temporary folders` and the folder `/private/tmp/ttl-check-run`

#### Scenario: A log of the repository's root
- **WHEN** a session ran in `~/src/app`, the root of its repository
- **THEN** its entry names the project `~/src/app` and no folder

## ADDED Requirements

### Requirement: Projects of kept entries
When the store is refreshed, an entry that names no folder SHALL have its project placed again by session-logs' Project of a folder, as the folder of a log of the entry's agent. When that gives another project, the entry names it as its project and its old project as its folder. An entry made anew from its log takes the project and folder of session-logs. The store's format stays `2`.

#### Scenario: An entry of a deleted log
- **WHEN** the store keeps the entry of a deleted log whose project is `~/.branch-deck/pool/app-c821e1d7/2/app`, and that folder is a worktree of `~/src/app`
- **THEN** after the refresh the entry names the project `~/src/app` and the folder `~/.branch-deck/pool/app-c821e1d7/2/app`
- **AND** `tokprof --project ~/src/app calendar` counts its days

#### Scenario: A deleted Codex chat
- **WHEN** the store keeps the entry of a deleted Codex log whose project is `~/Documents/Codex/2026-09-01/f`
- **THEN** after the refresh the entry names the project `codex chats` and the folder `~/Documents/Codex/2026-09-01/f`

#### Scenario: An entry whose folder is not known
- **WHEN** the store keeps the entry of a deleted log whose project is `~/.codex/worktrees/0dca/app`, which is gone and not remembered
- **THEN** the entry keeps `~/.codex/worktrees/0dca/app` as its project and names no folder

#### Scenario: A store an earlier build wrote
- **WHEN** `days-2.json` was written by a build without folders, and a new build refreshes it
- **THEN** the file stays `days-2.json`, the logs still on disk are read again, as after any change of the fingerprint, and their entries name their projects and folders by session-logs
