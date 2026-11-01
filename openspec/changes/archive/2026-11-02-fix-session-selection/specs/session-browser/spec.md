## MODIFIED Requirements

### Requirement: Session list
`tokprof sessions` SHALL print a header and one row per session that the filters select (see session-logs), newest first by the last write to its main log, at most `--limit` rows (default 20). A row shows when the main log was modified (local `YYYY-MM-DD HH:MM`), the agent, the session (its short id, see session-logs), the size of its main and subagent logs (`X.X MB`), the project, and `+N subagents` when it has any.

#### Scenario: Default list
- **WHEN** the user runs `tokprof sessions`
- **THEN** it prints at most 20 rows, newest first, under a header that names the columns

#### Scenario: A session with subagents
- **WHEN** a session spawned 5 subagents
- **THEN** its row ends with `+5 subagents` and its size includes their logs
- **AND** the subagents' logs (session-logs) get no rows of their own

#### Scenario: A shorter list
- **WHEN** the user passes `--limit 5`
- **THEN** at most 5 rows are printed

#### Scenario: Other filters
- **WHEN** `--project`, `--session`, `--since` or `--until` is given
- **THEN** the list holds only the sessions they select

#### Scenario: No logs
- **WHEN** no session logs are found
- **THEN** only the header is printed and the command succeeds

#### Scenario: Codex sessions started in the same minute
- **WHEN** six Codex sessions started within one minute
- **THEN** each row shows a different 18-character short id

### Requirement: Browser layout
`tokprof ui` SHALL show every session that `tokprof sessions` would list, without a limit, in a 74-column list on the left, the X-ray (session-xray) of the selected session on the right, and a footer line with the keys, the active filter and the latest status message. It starts with the newest session selected.

#### Scenario: A list row
- **WHEN** the list is shown
- **THEN** each row has the main log's last write time (local `MM-DD HH:MM`), the agent, the project clipped from the left with `…`, the session's cost in bold, and the next-request cell

#### Scenario: Pane titles
- **WHEN** the ui is drawn
- **THEN** the list title reads like ` 756 sessions · $2,065 · by time · costing 396… · next = one more request `, where the amount is the total of the costs known so far
- **AND** the X-ray title reads `<agent> <id> · <project>`

#### Scenario: Filters on the command line
- **WHEN** `tokprof ui` is started with `--project`, `--session`, `--since` or `--until`
- **THEN** it lists only the sessions they select, and a session that starts while it runs joins the list only when they select it

#### Scenario: No logs
- **WHEN** no session logs are found
- **THEN** `tokprof ui` fails with `no session logs found`
