## ADDED Requirements

### Requirement: Folder line
The X-ray header SHALL show a line right after the `session` line, which names the project, when the session's folder differs from its project (see session-logs, Session id and project): `worktree` with the folder when the project came from a worktree, else `folder` with the folder, home shown as `~`. When the folder is the project and is not on disk, the line reads `folder    not found`. Otherwise there is no such line.

#### Scenario: A session in a Branch Deck lease
- **WHEN** the user X-rays a session that ran in `~/.branch-deck/pool/app-c821e1d7/4/app`, a worktree of `~/src/app`
- **THEN** the header reads `session   claude  ~/src/app  <id>` and then `worktree  ~/.branch-deck/pool/app-c821e1d7/4/app`

#### Scenario: A session in a subfolder
- **WHEN** the user X-rays a session that ran in `~/src/app/plugins/web`
- **THEN** the header reads `session   claude  ~/src/app  <id>` and then `folder    ~/src/app/plugins/web`

#### Scenario: A session in a temporary folder
- **WHEN** the user X-rays a session that ran in `/private/tmp/ttl-check-run`
- **THEN** the header reads `session   claude  temporary folders  <id>` and then `folder    /private/tmp/ttl-check-run`

#### Scenario: A folder that is gone and not known
- **WHEN** the user X-rays a session that ran in `~/.codex/worktrees/0dca/app`, which is gone and not remembered
- **THEN** the header reads `session   codex  ~/.codex/worktrees/0dca/app  <id>` and then `folder    not found`

#### Scenario: A session at the repository's root
- **WHEN** the user X-rays a session that ran in `~/src/app`, which exists
- **THEN** the header has no `worktree` or `folder` line

#### Scenario: The X-ray in tokprof ui
- **WHEN** `tokprof ui` shows the X-ray of a session that ran in a worktree of `~/src/app`
- **THEN** the pane's title names `~/src/app`, and the X-ray holds the `worktree` line
