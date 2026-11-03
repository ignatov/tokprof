# Session Browser Specification

## Purpose
`tokprof sessions` lists the coding-agent sessions on this machine, and `tokprof ui` browses them in the terminal: the list on the left and the X-ray of the selected session on the right, with keys to filter, sort, write profiles and copy the handoff line. Both show what one more request in each session would cost now.

## Requirements

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

### Requirement: Costs in the session list
With `--cost`, `tokprof sessions` SHALL parse each listed session with its subagents and fill in two more columns: `cost`, the session's API-equivalent cost over its whole life, and `next request`, the next-request cell. Without `--cost`, both columns stay blank and the logs are read only as far as the row needs.

#### Scenario: Costs of the latest sessions
- **WHEN** the user runs `tokprof sessions --cost --limit 5`
- **THEN** each row shows the session's cost, like `$10.44`, and its next-request cell, like `$0.11   1%`

#### Scenario: Large amounts
- **WHEN** a session cost $1,234.56
- **THEN** its cost reads `$1,235`

#### Scenario: Date filters
- **WHEN** `--since 7d` is given with `--cost`
- **THEN** each cost still covers the session's whole life

#### Scenario: A log that cannot be parsed
- **WHEN** one of a session's logs fails to parse
- **THEN** its cost is left out of the session's cost and no error is shown

### Requirement: Next-request cell
The next-request cell, in `tokprof ui` and `tokprof sessions --cost`, SHALL show what one more request in the session's main conversation would cost now, as session-xray forecasts it, then that cost as a whole-number share of the session's cost so far, capped at 999%. It is green while the prompt cache is warm, red once it is cold and the request re-writes the whole context, and blank when there is no forecast.

#### Scenario: Warm cache
- **WHEN** the cache is warm, the forecast is $0.11 and the session cost $10.44 so far
- **THEN** the cell reads `$0.11   1%` in green

#### Scenario: Cold cache
- **WHEN** the cache is cold and the request that re-writes the context costs $2.76 in a $10.44 session
- **THEN** the cell reads `$2.76  26%` in red

#### Scenario: The cache goes cold while the ui is open
- **WHEN** the cache lifetime of a listed session runs out while `tokprof ui` runs
- **THEN** its cell switches to the cold cost in red at that moment, without the session being parsed again

#### Scenario: A session that cost nothing
- **WHEN** the session's cost so far is $0.00
- **THEN** the cell shows the forecast without a share

#### Scenario: No forecast
- **WHEN** the session's main conversation has no request with a prompt, or its model has no price
- **THEN** the cell is blank

#### Scenario: Output that is not a terminal
- **WHEN** `tokprof sessions --cost` writes to a pipe, or `NO_COLOR` is set
- **THEN** the cell carries no color codes

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

### Requirement: Background work
`tokprof ui` SHALL stay responsive while it parses logs: it costs every listed session in the background, in parallel, and computes X-rays one at a time for the session selected last, keeping each finished X-ray to show again when its session is selected.

#### Scenario: Costs fill in
- **WHEN** the ui starts
- **THEN** cost cells read `…` and the list title counts `costing <n>…` until every cost is known

#### Scenario: An X-ray being computed
- **WHEN** the selected session's X-ray is not ready
- **THEN** the right pane reads `computing the X-ray of <id> (<size> MB)…`

#### Scenario: Moving fast
- **WHEN** the selection moves past several sessions while an X-ray is being computed
- **THEN** only the X-ray of the session selected last is computed next

#### Scenario: Coming back
- **WHEN** a session whose X-ray was computed is selected again
- **THEN** its X-ray shows at once

#### Scenario: A failing X-ray
- **WHEN** the X-ray cannot be computed
- **THEN** the right pane shows `error: <reason>`

### Requirement: Moving through the list
In the list, `↑` or `k` and `↓` or `j` SHALL move the selection by one session, PgUp and PgDn by a page (the terminal height less 4 rows), `g` or Home to the first session and `G` or End to the last. The selection stops at both ends, and the X-ray of the newly selected session shows from its top.

#### Scenario: The last row
- **WHEN** the last session is selected and `↓` is pressed
- **THEN** the selection stays on it

#### Scenario: Jumping
- **WHEN** `G` and then `g` are pressed
- **THEN** the selection goes to the last session and back to the first

### Requirement: Full-screen X-ray
`⏎`, `→` or `l` SHALL show the selected session's X-ray full screen. There `↑` or `k` and `↓` or `j` scroll it by a line, PgUp, PgDn or space by a page, and `←`, `h`, Backspace or esc go back to the list. `f`, `o`, `c` and `q` work there as in the list.

#### Scenario: Scrolling
- **WHEN** `⏎` and then `↓` are pressed
- **THEN** the X-ray scrolls by one line and the same session stays selected

#### Scenario: Back to the list
- **WHEN** esc is pressed in the full-screen X-ray
- **THEN** the list shows again and the ui keeps running

#### Scenario: The end of the X-ray
- **WHEN** the X-ray is scrolled down to its last line
- **THEN** further `↓`, PgDn or space keep the last page on screen instead of scrolling past the end

### Requirement: Filter
`/` SHALL start a filter that keeps only the sessions whose agent, project or id contain the typed text, ignoring case, updated with every key. `⏎` ends typing and keeps the filter; esc while typing, or esc in the list while a filter is set, clears it. The selected session stays selected while it matches; otherwise the first match is selected.

#### Scenario: Typing a filter
- **WHEN** the user presses `/` and types `air`
- **THEN** only the sessions with `air` in their agent, project or id stay, and the footer reads `filter: air▌`
- **AND** Backspace removes the last character typed

#### Scenario: Keeping the filter
- **WHEN** `⏎` is pressed after typing
- **THEN** the keys move through the list again and the footer shows `filter: air`

#### Scenario: Clearing the filter
- **WHEN** esc is pressed in the list while the filter is set
- **THEN** every session is listed again

#### Scenario: Nothing matches
- **WHEN** no session matches the filter
- **THEN** the right pane reads `no session matches the filter`

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

### Requirement: Quit
`q` and Ctrl+C SHALL quit `tokprof ui` and restore the terminal from any view, Ctrl+C also while a filter is being typed; esc quits too when the list is shown and no filter is set.

#### Scenario: Esc in the list
- **WHEN** esc is pressed in the list with no filter set
- **THEN** the ui quits

#### Scenario: Esc elsewhere
- **WHEN** esc is pressed in the full-screen X-ray, or in the list while a filter is set
- **THEN** it goes back to the list or clears the filter instead of quitting

#### Scenario: Typing q into a filter
- **WHEN** `q` is pressed while a filter is being typed
- **THEN** `q` is added to the filter, and only Ctrl+C quits

### Requirement: Profiles from the browser
`f` SHALL write the selected session's JFR to `<profile dir>/<id>.jfr` and `o` its HTML flame graph to `<profile dir>/<id>.html`, in the background, where the profile directory is the one profile-export uses (`$TOKPROF_DIR`, or `~/tokprof`) and is created when missing. The profile covers the whole session with its subagents, with profile-export's default stack and threads, measured in cost. A subagent log that cannot be read is left out of the profile, as `tokprof flame` leaves it out.

#### Scenario: Writing the JFR
- **WHEN** `f` is pressed
- **THEN** the footer reads `writing ~/tokprof/<id>.jfr…` and then `wrote <n> samples (1 = $<unit>) to ~/tokprof/<id>.jfr`

#### Scenario: Writing and opening the HTML
- **WHEN** `o` is pressed
- **THEN** the footer reads `wrote <n> frames to ~/tokprof/<id>.html`, and the page is titled `tokprof: <agent> <project> <id>`
- **AND** tokprof then tries to open the page with `open` on macOS, and with `xdg-open` elsewhere

#### Scenario: No way to open the page
- **WHEN** the `open` command is missing or fails
- **THEN** the file is still written and no error is shown

#### Scenario: The export fails
- **WHEN** the profile directory cannot be created, or the session's main log cannot be parsed
- **THEN** the footer reads `export failed: <reason>`
- **AND** a subagent log that cannot be parsed does not fail the export; the footer adds `, <n> unreadable logs left out`

#### Scenario: An existing file
- **WHEN** the file is left from an earlier export
- **THEN** it is overwritten

### Requirement: Handoff key
`c` SHALL copy the selected session's handoff line (handoff) to the clipboard with the first of `pbcopy`, `wl-copy` and `xclip` that works, after which the footer reads `copied the handoff line for <id>: paste it into a new session`.

#### Scenario: No clipboard tool
- **WHEN** none of the three tools works
- **THEN** nothing is copied and the footer reads `no clipboard tool (pbcopy, wl-copy, xclip); run: tokprof handoff <id>`

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

### Requirement: One frame as text
The hidden option `tokprof ui --dump <width>x<height>` SHALL render one frame of that size as plain text on standard output and exit, without a terminal and without following the logs. It waits up to 20 s for the X-ray of the newest session, then 0.5 s more for costs, and prints every row with trailing spaces trimmed.

#### Scenario: Checking the layout
- **WHEN** the user runs `tokprof ui --dump 160x45`
- **THEN** it prints 45 lines with the list, the X-ray of the newest session and the footer, and costs not known yet read `…`

#### Scenario: A bad size
- **WHEN** the user runs `tokprof ui --dump 160` or `tokprof ui --dump 160xabc`
- **THEN** the command fails with `--dump takes WIDTHxHEIGHT`

#### Scenario: Not in the help
- **WHEN** the user runs `tokprof ui --help`
- **THEN** `--dump` is not listed

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

### Requirement: Day view in the browser
`tokprof ui` SHALL show the day view of work-rhythm full screen when `d` is pressed. `d`, esc, `←`, `h` or Backspace go back to the list with the X-ray. `[` and `]` step to the day before and after, never past the current day, and keep the hours of the first day shown. The day is computed in the background, one at a time, and its strip fits the screen's width. While the current day is shown, it is computed again after a session writes to its log.

#### Scenario: The first press
- **WHEN** `d` is pressed
- **THEN** the screen is titled ` day · Tue 2026-11-03 ` and shows the day of the user's latest action, as `tokprof day` prints it with the browser's filters
- **AND** until it is ready the screen reads `computing the day of 2026-11-03…`

#### Scenario: Stepping through days
- **WHEN** `[` is pressed twice and then `]` once
- **THEN** the screen shows the day before the current one, and a `]` on the current day does nothing
- **AND** when the first day shown starts at 04:24 and the day before at 05:14, both strips run from 04:00

#### Scenario: Full screen
- **WHEN** the day is shown
- **THEN** it takes the whole screen and scrolls as the full-screen X-ray does, and esc goes back to the list with the X-ray

#### Scenario: Keys in the footer
- **WHEN** the list or the day is shown
- **THEN** the footer names `d day` after the other keys, and `[ ] days` while the day is shown

#### Scenario: A live day
- **WHEN** the current day is shown and a session writes to its log
- **THEN** the day is computed again, after a pause at least as long as its last computation took, and the title adds ` · updating…` until the new one is ready

#### Scenario: Coming back
- **WHEN** a day that was computed is shown again and no session wrote to its log since
- **THEN** it shows at once

