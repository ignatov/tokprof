## ADDED Requirements

### Requirement: Folder in the inspector
When the selected session's folder differs from its project (see session-logs, Session id and project), `Summary` SHALL show the folder, home shown as `~`, under the project, after the label `worktree` or `folder` as the X-ray's folder line has it (see session-xray), clipped from the left with `…` to the inspector's width. A folder that is not found has no line here. The list, the palette, the menu bar menu and Reports name the project.

#### Scenario: A session in a worktree
- **WHEN** the selected session ran in `~/.branch-deck/pool/app-c821e1d7/4/app`, a worktree of `~/src/app`
- **THEN** `Summary` names the project `~/src/app` and under it `worktree ~/.branch-deck/pool/app-c821e1d7/4/app`
- **AND** Reports' project card counts its spend under `~/src/app`

#### Scenario: A session in a subfolder
- **WHEN** the selected session ran in `~/src/app/plugins/web`
- **THEN** `Summary` names the project `~/src/app` and under it `folder ~/src/app/plugins/web`

#### Scenario: A session at the repository's root
- **WHEN** the selected session ran in `~/src/app`
- **THEN** `Summary` shows no `worktree` or `folder` line
