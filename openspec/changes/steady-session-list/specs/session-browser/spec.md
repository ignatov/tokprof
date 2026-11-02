# Spec Delta

## ADDED Requirements

### Requirement: Rows in place
`tokprof ui` SHALL put its rows in order only when it starts, when `s` or `r` is pressed and when the filter changes. Costs, X-rays and log changes that arrive in between update a row's cells, not its place. A new session is put where the current order puts it, and a session that leaves takes its row with it. Rows on the screen keep their places, except that a session joining or leaving among them moves the rows below it by one.

#### Scenario: Records without a time
- **WHEN** Claude Code appends `cost-state` lines to 20 idle sessions, five each within 100 seconds
- **THEN** no row moves, and each row keeps the time of its session's last activity

#### Scenario: Work in a session down the list
- **WHEN** the session in row 12 makes a request while the list is in time order
- **THEN** it stays in row 12, and its time and cost update there

#### Scenario: Costs that arrive in cost order
- **WHEN** `s` sorted the list by cost while 396 sessions were still being costed, and their costs arrive
- **THEN** each cost fills its row's cell where the row is, and no row moves

#### Scenario: A cache goes cold
- **WHEN** the list is by next request and a session's cache goes cold
- **THEN** its next-request cell turns to the cold cost in red, and the row stays where it is

#### Scenario: A new session above the screen
- **WHEN** the list is scrolled down and a new session's place in time order is the top
- **THEN** the session is put at the top, and the rows on the screen stay where they are

#### Scenario: A new session on the screen
- **WHEN** a new session's place is among the rows on the screen
- **THEN** it shows there, and the rows below it move down by one

#### Scenario: A session leaves
- **WHEN** a session above the rows on the screen leaves the list
- **THEN** the rows on the screen stay where they are
- **AND** when a session on the screen leaves, the rows below it move up by one

### Requirement: Re-sort key
`r` SHALL sort the list again in its current order and keep the selected session selected. While sorting again would change the order, the list title reads `by <order>, r re-sorts` in place of `by <order>`. The footer lists `r re-sort` after the sort key.

#### Scenario: The order goes out of date
- **WHEN** the session in row 12 makes a request while the list is in time order
- **THEN** the list title reads like ` 765 sessions · $16,206 · by time, r re-sorts · next = one more request `

#### Scenario: Re-sorting
- **WHEN** the session in row 12 made a request in time order and `r` is pressed
- **THEN** that session is in row 1, the selected session is still selected, and the list title reads `by time` again

#### Scenario: In order
- **WHEN** sorting again would not change the order
- **THEN** the list title reads `by <order>` without `r re-sorts`, and `r` changes nothing

## MODIFIED Requirements

### Requirement: Session list
`tokprof sessions` SHALL print a header and one row per session that the filters select (see session-logs), newest first by last activity (see session-logs), at most `--limit` rows (default 20). A row shows the last activity (local `YYYY-MM-DD HH:MM`, under the header `active`), the agent, the session (its short id, see session-logs), the size of its main and subagent logs (`X.X MB`), the project, and `+N subagents` when it has any.

#### Scenario: Default list
- **WHEN** the user runs `tokprof sessions`
- **THEN** it prints at most 20 rows, newest first, under a header that names the columns, the first of them `active`

#### Scenario: A session resumed without work
- **WHEN** an idle session's transcript got only records without a time at 16:01, and its last record with a time is from 09:27
- **THEN** its row shows `09:27`, below the sessions that were active after 09:27

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
- **THEN** each row has the session's last activity (local `MM-DD HH:MM`), the agent, the project clipped from the left with `…`, the session's cost in bold, and the next-request cell

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

### Requirement: Sort
`s` SHALL cycle the order of the list from time (latest last activity first, the order it starts in) to cost (most expensive session first) to next request (dearest next request now first) and back to time, and sort the list in the new order. Sessions whose value is not known yet go last, and the selected session stays selected. Values that arrive later move no row (see Rows in place).

#### Scenario: By cost
- **WHEN** `s` is pressed once
- **THEN** the most expensive session is on top, the sessions still being costed are at the bottom, and the list title reads `by cost`

#### Scenario: By next request
- **WHEN** `s` is pressed twice
- **THEN** the session whose next request would cost the most now is on top, and the list title reads `by next request`

#### Scenario: Back to time
- **WHEN** `s` is pressed three times
- **THEN** the list is newest first again

### Requirement: Live updates
While it runs, `tokprof ui` SHALL follow the agents' log directories: a new session joins the list, a session whose main or subagent logs change is costed and X-rayed again, and a session whose main log is deleted or moved away leaves the list. Until the new X-ray is ready, the old one stays, with `· updating…` in its title. Where rows go when this happens belongs to Rows in place.

#### Scenario: A new session
- **WHEN** an agent starts a new session
- **THEN** it shows up where the current order puts it, at the top of the list sorted by time, and the selection stays on the session it was on

#### Scenario: A growing session
- **WHEN** the selected session's log grows
- **THEN** its cost and time update, the X-ray title reads `· updating…` until the new X-ray replaces the old one, and the session keeps its row

#### Scenario: A deleted session
- **WHEN** a session's main log is deleted
- **THEN** the session leaves the list, and if it was selected, the cursor stays on the same row

#### Scenario: Dropped change events
- **WHEN** the file system reports that it dropped change events
- **THEN** the ui scans every log directory again, removes the sessions that are gone and adds the ones that are there, and the sessions still listed keep their rows

#### Scenario: A directory created later
- **WHEN** a log directory such as `~/.codex/archived_sessions` does not exist yet
- **THEN** its parent is followed, and its sessions show up once it is created

#### Scenario: Busy logs
- **WHEN** an agent writes its log many times a second
- **THEN** changes are gathered for 500 ms, and a session that keeps changing is costed or X-rayed again only after as long as its last costing or X-ray took, so it uses at most about half a core

#### Scenario: The logs cannot be followed
- **WHEN** the log directories cannot be watched
- **THEN** the ui runs without live updates and the footer reads `not following the logs: <reason>`
