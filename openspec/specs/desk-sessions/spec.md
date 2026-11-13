# desk-sessions Specification

## Purpose
The Sessions screen of `tokprof-desk`: one table of every listed session, to find a session, sort all of them by what they cost or wasted, see what a project cost, and open a session in Day.

## Requirements

### Requirement: Sessions screen
The bar SHALL have a `Sessions` button right of the switch, apart from it, and ⌘0 and the palette's `Sessions` show the Sessions screen from every screen. Under the bar it shows a toolbar, the table of sessions with its footer, and, while a session is selected, a narrow inspector at the right. `Day` in the switch, or ⌘1, shows Day as it was. The session selected in Sessions stays selected when Day shows.

#### Scenario: Switching
- **WHEN** Day is shown and the user presses ⌘0
- **THEN** the lanes and the details give way to the toolbar, the table and its footer, and `Sessions` is marked in the bar while no part of the switch is

#### Scenario: Back to Day
- **WHEN** Sessions is shown and the user presses ⌘1
- **THEN** Day shows the day and the selection it showed before

#### Scenario: From Reports
- **WHEN** Month or Quarter, which took Reports' place, is shown and the user clicks `Sessions`
- **THEN** the Sessions screen is shown

### Requirement: Table columns
The table SHALL have the columns `Last`, the agent's dot (no title), `Project`, `Title`, `Started`, `Duration`, `Requests`, `Cost`, `Avoidable`, `Cold`, `Next` and `Model`, in this order, under a header row that stays on top while the rows scroll. `Title` takes the room the others leave. When the columns do not fit, `Model`, then `Started`, then `Requests` are left out, in that order.

#### Scenario: A wide window
- **WHEN** the window is 1400 points wide and the inspector is open
- **THEN** every column shows

#### Scenario: A narrow window
- **WHEN** the window is 960 points wide and the inspector is open
- **THEN** `Model` and `Started` are left out first, and `Title` keeps the room that is left

### Requirement: Values of the columns
`Project` SHALL name the project as Day's lanes do, cut at its end. `Started` is the time of the session's first request, or of its first timed record without one. `Duration` runs from it to the last activity (session-xray's span). `Requests` counts those of all its logs, subagents included. `Avoidable` is the premium of its unexpected misses (cache-misses). `Cold` counts its cold returns (pause-report). `Model` names the model of most of its requests. Each reads `…` while it is being costed.

#### Scenario: The same numbers as the X-ray
- **WHEN** the table shows a session and `tokprof scan <id>` runs at the same moment
- **THEN** its cost, requests and avoidable dollars are those of the X-ray, and its cold returns are those `tokprof pauses --session <id>` counts

#### Scenario: The project's name
- **WHEN** a session ran in `~/src/zebrafish`, and another in the temporary folders
- **THEN** their `Project` cells read `zebrafish` and `temporary folders`, which fits at the default width of 1400 points, while the inspector's `worktree` or `folder` line keeps the path

#### Scenario: Several models
- **WHEN** a session made 1,465 requests with claude-opus-5-5 and 141 with claude-fable-5-2
- **THEN** its `Model` cell reads `claude-opus-5-5 +1`

#### Scenario: Nothing to avoid
- **WHEN** a session has no unexpected miss and no cold return
- **THEN** its `Avoidable` and `Cold` cells read `—` in the dim ink, and otherwise their values are in the theme's red

#### Scenario: Being costed
- **WHEN** a session's logs have not been costed yet
- **THEN** its `Requests`, `Cost`, `Avoidable`, `Cold` and `Model` cells read `…`

### Requirement: Times in the table
Under day headers, `Last` SHALL read the time of the last activity as `HH:MM`; in one list, its day and time, as `11 Nov 23:12`, with the year when it is not this year. `Started` reads `HH:MM` when the session started on the day of its last activity (see Day groups), and else its day and time in the same form.

#### Scenario: A long session
- **WHEN** a session started on 9 Nov at 15:20 and was last active on 11 Nov at 22:48, in time order
- **THEN** its row is under Wednesday's header and reads `22:48` and `9 Nov 15:20`, and its duration reads as session-xray's span of 55.5 hours

### Requirement: Day groups
In time order, newest first, the table SHALL put sessions under a header row per day of their last activity, as Day's days run (work-rhythm, Start of the day). The header names the day (`Today`, `Yesterday`, the weekday, the date), the count of its rows, and that day's spend in the day store of the sessions the filters keep. With no filter, it is Day's Spend card of that date. Sorted any other way, the table is one list without headers. A click on a header shows that day in Day.

#### Scenario: Time order
- **WHEN** the table is in time order, yesterday holds 52 listed sessions, and Day's Spend card of yesterday reads $818
- **THEN** a header row reads `Yesterday` and `52 sessions · $818 spent`, and the 52 rows follow it

#### Scenario: A header is not the sum of its rows
- **WHEN** a session spent $90 on Monday and $28 today, and was last active today
- **THEN** its row sits under `Today` and reads `$118`, its whole-life cost, while today's header counts $28 of it and Monday's header the other $90

#### Scenario: The same number as Day and Week
- **WHEN** the table shows the header of Tue 3 Nov 2026 and no filter is set
- **THEN** it reads `$675 spent`, as Day's Spend card of that date and Tuesday's row of Week on 2–8 Nov 2026 read

#### Scenario: A project picked
- **WHEN** the picker reads `~/src/zebrafish`, and its sessions spent $212 on Tue 10 Nov, $40 of it in a session last active today
- **THEN** the header of Tue 10 Nov reads `$212 spent`, the $40 included, and counts only the zebrafish rows under it

#### Scenario: Quick filters and the bar's filter
- **WHEN** `Warm` and `codex` are on, or the bar's filter is set
- **THEN** each header reads that day's spend of the sessions the filters keep, by their session, project and agent in the store, and changes when a session goes cold and leaves the set

#### Scenario: A title match
- **WHEN** the bar's filter is `python` and it keeps 2 sessions by their titles only
- **THEN** the headers count the store's spend of those 2 sessions' ids, their subagents included

#### Scenario: Last active at 02:00
- **WHEN** a session was last active at 02:00 on Thu 12 Nov, before the night's middle that starts that day at 04:21
- **THEN** its row is under the header of Wed 11 Nov and reads `02:00`, Wednesday's header counts it and its spend before 04:21, and `Open in Day` shows Wed 11 Nov

#### Scenario: The store being filled
- **WHEN** the day store is being filled
- **THEN** each header gives its count and reads `…` for the day's spend until the store is filled, and the rows go by the local date of their last activity five hours earlier, as desk-window's The selected session's day finds a day

#### Scenario: Sorted by cost
- **WHEN** the user sorts the table by `Cost`
- **THEN** no header row shows, and each `Last` cell carries its day

#### Scenario: Opening a day
- **WHEN** the user clicks the header of Tue 10 Nov
- **THEN** the window shows Day with the lanes of 2026-11-10 and no session selected

### Requirement: Sort by a column
A click on a column's header SHALL sort the table by that column, and another click on it reverses the order. A first click sorts numbers and times greatest first and text (`Project`, `Title`, `Model`, the agent) from A to Z. The header marks the sorted column with `↓` or `↑`. Values not known yet go last in both directions, ties go newest last activity first, and the rows stay in place as session-browser's Rows in place and Re-sort key say.

#### Scenario: By avoidable
- **WHEN** the user clicks `Avoidable`
- **THEN** the session with the most avoidable dollars is on top, the header reads `Avoidable ↓`, and the sessions still being costed are at the bottom

#### Scenario: Reversed
- **WHEN** the table is sorted by `Avoidable` and the user clicks `Avoidable` again
- **THEN** the session with the fewest avoidable dollars is on top, sessions still being costed are still at the bottom, and the header reads `Avoidable ↑`

#### Scenario: Back to time order
- **WHEN** the table is sorted by `Last` oldest first and the user clicks `Last`
- **THEN** it is newest first, with its day headers

#### Scenario: The sort key
- **WHEN** the user presses `s` in Sessions
- **THEN** the table cycles through time, cost and next request, and waiting with the setting on, as session-browser's Sort does, and the header marks that column

### Requirement: Quick filters
The toolbar SHALL hold the pills `All`, `Running now`, `Warm`, `This week`, one per agent with listed sessions, and a project picker. A click on a pill turns it on or off. The agent pills keep any of their agents; the other pills, the picker and the bar's filter each narrow the set, so that `Warm` and `codex` keep the warm Codex sessions. `All` turns every pill and the picker off. Each pill shows how many listed sessions the bar's filter and it alone keep.

#### Scenario: Two pills
- **WHEN** the user turns on `Warm` and `codex`
- **THEN** the table holds only the Codex sessions whose cache is warm, and the footer totals them

#### Scenario: Two agents
- **WHEN** the user turns on `claude` and `codex`
- **THEN** the table holds the sessions of either agent

#### Scenario: All
- **WHEN** pills are on and the user clicks `All`
- **THEN** every listed session that the bar's filter keeps is in the table again, and `All` is marked

### Requirement: Running and warm sessions
A session SHALL run now while its agent works as work-rhythm's Work counts it: its latest turn has not ended, it does not wait for the answer to its own question, and its latest request is less than 15 minutes old. A session is warm while its cache is warm, as its `Next` cell counts down. Both change with the clock, without the session being costed again.

#### Scenario: Still working
- **WHEN** a session's agent sent a request at 14:16 and has not ended its turn, and it is 14:20
- **THEN** `Running now` keeps it

#### Scenario: A silence
- **WHEN** no request, end, question or answer followed that request, and it is 14:31
- **THEN** `Running now` no longer keeps it, and the table drops it at once

#### Scenario: Gone cold
- **WHEN** `Warm` is on and a listed session's cache goes cold
- **THEN** the session leaves the table at that moment, and the footer no longer counts it

### Requirement: This week and the project picker
`This week` SHALL keep the sessions whose last activity is at or after 00:00 local time on Monday of the current week. The project picker reads `Project: any` and offers the projects of the listed sessions by their names as `Project` cells read them, the most sessions first, each with its count; picking one keeps that project's sessions only, and `any` keeps every project.

#### Scenario: On a Thursday
- **WHEN** it is Thursday 12 November 2026 and the user turns on `This week`
- **THEN** the table holds the sessions last active since Monday 9 November 00:00

#### Scenario: A project
- **WHEN** the user picks `~/src/zebrafish` in the picker
- **THEN** the table holds only that project's sessions, and the picker reads `zebrafish`

### Requirement: Footer
Under the table, a footer SHALL total the whole set the filters keep, not only the rows on the screen: `Total`, the count of sessions and what keeps them (`all time`, the pills' names, or the bar's filter), then the sum of the sessions' costs, each its whole life as in its `Cost` cell, of the avoidable dollars and of the cold returns, each in its column, and the avoidable share of the cost. While sessions are being costed, it adds `costing <n>…` and sums what is known.

#### Scenario: All sessions
- **WHEN** nothing narrows the set and every session is costed
- **THEN** the footer's count is that of `tokprof sessions --limit 100000`, and its cost, avoidable dollars and cold returns are the sums of the rows' cells

#### Scenario: Rows off the screen
- **WHEN** `This week` keeps 152 sessions and 20 rows show
- **THEN** the footer reads `152 sessions` and totals all 152

#### Scenario: Not the days' spend
- **WHEN** `This week` is on, and some of its sessions spent before Monday
- **THEN** the footer's cost is the sum of the rows' `Cost` cells, their whole-life costs, and may be more than the week's spend in Week

### Requirement: Projects mode
The toolbar SHALL start with a `Sessions | Projects` switch. In `Projects`, the table puts the sessions under a row per project, the projects with the most cost first, the sessions under each in the table's order. Groups start folded: a row per project with its subtotals. A click on a project's row, or ⏎ on it, folds or unfolds it; → unfolds the project under the cursor and ← folds it. The mode, the folds and the order are kept while the window runs.

#### Scenario: Grouped
- **WHEN** the user clicks `Projects`
- **THEN** each project of the filtered set has one row, the costliest first, with its sessions folded under it

#### Scenario: Unfolding a project
- **WHEN** the user clicks the row of `~/src/zebrafish`
- **THEN** its sessions show under it, each `Last` cell with its day, and another click folds them again

#### Scenario: Folding by keys
- **WHEN** the cursor is on the folded row of `~/src/zebrafish` and the user presses →, then ←
- **THEN** → shows its sessions under it, and ← folds them again with the cursor on the project's row

#### Scenario: A filter in Projects
- **WHEN** the user turns on `This week` in Projects
- **THEN** only the projects with sessions this week have rows, and their subtotals count only those sessions

### Requirement: Project rows
A project's row SHALL show the last activity of its newest session, a fold mark (`▸` folded, `▾` open), the project, `<n> sessions`, a bar as long as its cost's share of the costliest project's, `<p>% of spend` of the set's cost, and the sum of its sessions' costs, avoidable dollars and cold returns in their columns.

#### Scenario: A project's subtotals
- **WHEN** `~/src/zebrafish` has 24 listed sessions that cost $11,320 of $24,527, with $862 avoidable and 77 cold returns
- **THEN** its row reads `24 sessions`, `46% of spend`, `$11,320`, `$862` and `77`

### Requirement: Sessions inspector
While a session is selected, a column 360 points wide at the right SHALL show its `Summary` (see desk-window, Summary tab) laid out in one column. On top are `Open in Day` with ⏎ as its keycap, `Copy handoff line` with `c`, and a close button. After the spend comes the cache: its cold returns and its avoidable dollars with their share of its cost. Then a line says `Requests, Spend, Work, Waits, Prompts, Tools, Context, Cache and Flame open in Day.`

#### Scenario: Selecting a row
- **WHEN** the user clicks a row
- **THEN** the inspector shows that session's Summary from its top, with the same numbers as Day's Summary of it

#### Scenario: Closing it
- **WHEN** the inspector is open and the user presses esc or clicks the close button
- **THEN** the inspector closes, the row stays selected, and the table takes the width

### Requirement: Opening a session in Day
⏎ on a selected row, a double click on a row, and `Open in Day` SHALL show Day on the day of the session's last activity as Day groups finds it, the day its row is listed under in time order, with that session selected and its Summary from the top in the details pane.

#### Scenario: A session of yesterday
- **WHEN** the user presses ⏎ on a session last active yesterday at 22:48
- **THEN** Day shows yesterday, with that session's lane highlighted and its Summary under the lanes

#### Scenario: A session nobody typed in
- **WHEN** the user opens a session that a program ran today
- **THEN** Day shows today with no lane highlighted, and the details pane shows that session's inspector

### Requirement: Keys in Sessions
In Sessions, the table SHALL have the keyboard to start with. ↑ and ↓ (`k`, `j`) select the row above or below, PgUp and PgDn by a page, Home and End the first and last, skipping day headers and opening the inspector. ⏎ opens the session in Day, or folds a project's row; → unfolds it and ← folds the cursor's project. esc closes the inspector, then clears the filter. `t` and `Today` sort by time and show the top. `/` starts the filter; Tab moves to the inspector and back.

#### Scenario: Moving down
- **WHEN** the table has the keyboard on a row and the user presses ↓
- **THEN** the next session's row is selected, the inspector shows its Summary, and day headers are passed over

#### Scenario: Esc twice
- **WHEN** a filter is set, the inspector is open, and the user presses esc twice
- **THEN** the first esc closes the inspector and the second clears the filter, and the window stays open

#### Scenario: Today
- **WHEN** the table is sorted by cost and the user presses `t`
- **THEN** it is in time order, newest first, scrolled to its top under `Today`

#### Scenario: The session keys
- **WHEN** a row is selected and the user presses `c`, `f` or `o`
- **THEN** they copy the handoff line or write the profile of that session, as in Day

### Requirement: Sessions in a frame without a window
`--scene sessions` SHALL draw the frame in Sessions in time order, with the newest session selected and its inspector, after every listed session is costed, waiting up to 120 seconds. `--scene sessions-projects` draws it in `Projects` with every group folded, and `--scene sessions-cost` sorted by cost with no inspector. `--tree` holds the text of every row, header, pill and footer cell.

#### Scenario: A frame of the table
- **WHEN** the user runs `tokprof-desk --snap out.png --tree out.json --scene sessions`
- **THEN** `out.png` shows the toolbar, the day headers, the rows, the footer and the inspector of the newest session, and `out.json` holds the footer's totals

#### Scenario: Projects in a frame
- **WHEN** the user runs `tokprof-desk --snap out.png --scene sessions-projects`
- **THEN** `out.png` shows one row per project, the costliest first, with its subtotals
