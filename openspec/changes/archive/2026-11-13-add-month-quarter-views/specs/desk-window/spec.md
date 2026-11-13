# Spec Delta

## RENAMED Requirements

- FROM: `### Requirement: Reports view`
- TO: `### Requirement: View switch`

- FROM: `### Requirement: Keys in Reports`
- TO: `### Requirement: Keys in Month and Quarter`

## MODIFIED Requirements

### Requirement: Day controls
In Day, the bar SHALL hold the shown day's name with buttons to the day before and after, a Today button, the filter, and a `Live` mark while the window follows the logs; the day's caption and the zoom are in the lanes' header. Sessions shows `All sessions` in place of the day's name and its buttons, and Week, Month and Quarter their own controls with the filter (see Week in the bar, Month and quarter periods). Calendar shows none of them and no filter.

#### Scenario: Other days
- **WHEN** the user clicks the button before the day's name
- **THEN** the lanes show the day before, and Today goes back to the day of the latest action

#### Scenario: Never past the latest day
- **WHEN** the lanes show the day of the latest action and the user clicks the button after its name
- **THEN** the lanes stay on that day

#### Scenario: In Reports
- **WHEN** Month, which took Reports' place, is shown on 12 November 2026
- **THEN** the bar holds the switch, the `Sessions` button, `‹ November 2026 ›`, Today, `1–12 Nov so far · vs 1–12 Oct`, the filter, the status and `Live`, and no day controls

#### Scenario: In Calendar
- **WHEN** Calendar is shown
- **THEN** the bar holds the switch, the `Sessions` button, the status and `Live`, and no day controls and no filter

#### Scenario: In Sessions
- **WHEN** Sessions is shown on logs that start on 3 Mar with 412 listed sessions
- **THEN** the bar reads `All sessions` and `since 3 Mar · 412 sessions`, then Today, the filter and `Live`

#### Scenario: In Week
- **WHEN** Week is shown
- **THEN** the bar holds the switch, the `Sessions` button, the week's name with its buttons, `Today`, the week's summary, the filter, the status and `Live`, and no day controls and no zoom

### Requirement: View switch
The bar SHALL hold a `Day | Week | Month | Quarter | Calendar` switch, and right of it, apart from it, the `Sessions` button. ⌘1 to ⌘5 show the views in that order, and ⌘0 Sessions (see desk-sessions). Month and Quarter replace what is under the bar with one scrolling pane of the period's cards (see Report cards). The window starts in Day.

#### Scenario: Switching
- **WHEN** the user clicks `Month` in the bar, or presses ⌘3
- **THEN** the lanes and the details pane give way to the month's cards, and `Month` is marked in the switch

#### Scenario: Back to the day
- **WHEN** Month is shown and the user clicks `Day`, or presses ⌘1
- **THEN** the window shows Day as it was: its day, its selection and its tab

#### Scenario: From the palette and the key help
- **WHEN** the user picks `Last month` in the command palette
- **THEN** Month shows the month before the current one; the palette also offers `Day`, `Week`, `Sessions`, `Month`, `Quarter`, `This month`, `This quarter`, `Last quarter` and `Month in review`, and the key help lists ⌘0 to ⌘5 and the keys of Month and Quarter

#### Scenario: Quarter by key
- **WHEN** the user presses ⌘4 on 12 November 2026
- **THEN** Quarter shows Q4 2026 so far, and `Quarter` is marked in the switch

### Requirement: Report computation
The cards of Month and Quarter that read the period's logs (see Sources of the period's cards) SHALL be computed in the background; no frame waits for them. While they run, each reads `reading the logs of October 2026… <n> of <m> logs`. Each result is kept by period and filter. When the logs change while a period is shown, it is computed again once 4 times its last computation and at least 20 seconds passed, with `updating…` by the period's name.

#### Scenario: The first look
- **WHEN** the user switches to Month for the first time
- **THEN** the key numbers, the grid and the day chart show from the day store, the cards that read the logs read `reading the logs of November 2026… 0 of 2,302 logs`, the count grows while the window stays responsive, and the cards replace it when they arrive

#### Scenario: Back to a period
- **WHEN** the user steps from November to October and back
- **THEN** November's cards show at once, without being computed again

#### Scenario: At rest
- **WHEN** a period's cards have arrived and the logs do not change
- **THEN** the window draws no more frames for them

#### Scenario: Not shown
- **WHEN** neither Month nor Quarter is shown
- **THEN** nothing is computed for them

### Requirement: Report cards
Month SHALL show, in rounded cards and in this order: the key numbers; the days of the month beside spend by day; by project, by origin and by model; cold returns by pause, sessions by largest context and what to change; the records of the month beside Month in review. Quarter shows the key numbers; the weeks of the quarter beside its months and project shifts; by project, by model and what to change; the records of the quarter beside its cards to share.

#### Scenario: The same numbers as the CLI
- **WHEN** Month shows October 2026, and the user runs `tokprof month 2026-10`, and `tokprof summary`, `tokprof pauses` and `tokprof share` with `--since 2026-10-01 --until 2026-11-01`, with the same filters at the same time
- **THEN** the key numbers and records equal the month's, and every amount, count and share of by origin, by model, the pause tables and what to change equals the one the commands print for the same rows

#### Scenario: A narrow window
- **WHEN** the window is 960 points wide
- **THEN** each row of cards wraps to two cards side by side, and no card is cut

### Requirement: Key numbers
The key numbers SHALL be seven tiles, counted from the day store over the period as `tokprof month` counts a month: spend; agent hours, the calendar's `agent` metric, each minute some agent worked counted once; hours at the keyboard; sessions, the main sessions with a request; avoidable on cache misses, with its share of spend and the count of cold returns under it; saved by the cache; and the agents' commits.

#### Scenario: Two agents
- **WHEN** the store holds 211 cold returns of Claude Code and 51 of Codex in October 2026, as `tokprof pauses` counts them for the same dates
- **THEN** the avoidable tile reads `$1,452` with `12.5% of spend · 262 cold returns` under it

### Requirement: Spend by day
Month's day chart SHALL have a bar per date, its calendar cell's spend (see activity-calendar, Calendar days): useful in the accent, avoidable on top in the cold color. Only when the key numbers compare (see Pace per day), a line, its legend and its caption show the month before by weekday, the first Mondays facing; the line breaks over dates before the store's first request. The pointer or day cursor names the date, its spend, both parts and the date faced. A bar opens its date in Day.

#### Scenario: Hovering a day
- **WHEN** the pointer rests over the bar of 3 November
- **THEN** the line under the chart reads like `Tue 3 Nov · $412 · $381 useful, $31 avoidable · Tue 6 Oct $260`, since Mon 2 Nov faces Mon 5 Oct

#### Scenario: Opening a day
- **WHEN** the user clicks the bar of 3 November
- **THEN** the window shows Day with the lanes of 2026-11-03

#### Scenario: A day without requests
- **WHEN** a date of the month has no request
- **THEN** it has an empty column, so the dates keep their places

#### Scenario: Dates to come
- **WHEN** Month shows November on the 12th
- **THEN** 13 to 30 November have no bar and open nothing, a line in the now color marks the present, and October's line goes on through them to 28 November, which faces Sat 31 Oct

#### Scenario: A date the month before lacks
- **WHEN** Month shows November 2026, whose first Monday is 2 Nov, and October's first Monday is 5 Oct
- **THEN** 1 November faces Sun 4 Oct, 29 and 30 November face no date of October and have no point, 1 to 3 October face no date and are left out, and the line never wraps

#### Scenario: Ends at the start
- **WHEN** Month shows December 2026, whose first Monday is 7 Dec, and November's first Monday is 2 Nov
- **THEN** 1 to 5 December have no point, and 6 December faces Sun 1 Nov

#### Scenario: No line without a comparison
- **WHEN** Month shows October 2026 and the band notes `no comparison with September: claude's logs start 29 Sep`
- **THEN** spend by day draws no line of September, its legend reads only `useful` and `avoidable on cache misses`, its caption reads `useful and avoidable`, and the pointer over a bar names no date of September

#### Scenario: Before the first log
- **WHEN** November compares with October and the store's first request is on 10 October
- **THEN** 1 to 6 November, which face 4 to 9 October, have no point and no zero, the line starts at 7 November, and the pointer over 3 November reads `Tue 6 Oct no logs` for the date it faces

#### Scenario: Totals by date
- **WHEN** Month shows November on 12 November at 15:00
- **THEN** the line is aligned by weekday, while the key numbers still compare with 1 October 00:00 to 12 October 15:00 (see The same point of the period before)

#### Scenario: The bars and the key number
- **WHEN** October's key number reads `$11,632` and the night of 1 October from 00:00 lies in 30 September's cell
- **THEN** the bars sum to the October cells' $11,493, and the line under the chart says `$139 of October fell in the night of 30 Sep's cell`

### Requirement: Spend by project and by origin
The project card SHALL show the 10 projects with the most spend in the period, from the day store. The origin card shows the 10 origins of the summary's origin table (`prompt`, `agent:…`, `skill:…`). Each row is a bar as long as its share of the first, with its dollars and its share of all; the rest is one row `<n> more`. Quarter has the project card only. A click on a project, not on `more`, sets the window's filter to it and stays on the period.

#### Scenario: A project
- **WHEN** Month shows November and the user clicks the bar of `zebrafish`
- **THEN** Month still shows November, the filter reads `~/src/zebrafish`, and every card counts only that project (see The filter in Month and Quarter)

### Requirement: Spend by model and the pause tables
The model card SHALL hold the summary table's rows in its order: agent, model, requests, a bar of the five components' shares, and the total; the pointer over a row names each component's dollars and the share of prompt known. Per agent, Month's cold-return card holds the rows of the cold-return table of `tokprof pauses` (pause, returns, first request, avoidable, median context) with its totals line, and its context card the rows of its largest-context table.

#### Scenario: No cold returns
- **WHEN** an agent has no cold return in the month
- **THEN** its part of the card reads `No cold returns.`

#### Scenario: Quarter's model card
- **WHEN** Quarter is shown
- **THEN** it has the model card and no pause tables

### Requirement: Keys in Month and Quarter
In Month and Quarter, Tab and Shift+Tab SHALL move the keyboard through the cards with days, weeks or rows, ringed in the accent and scrolled into view. ← and → (`[`, `]`) step the period while no card has the keyboard, and `t` shows the current one. In a card, ← → move its day or week cursor; ↑ ↓ (`k`, `j`), Home and End move a row cursor; ⏎ opens what is under the cursor as a click does. esc gives the keyboard back to the page, then clears the filter, then shows Day.

#### Scenario: A day by keys
- **WHEN** the days of the month have the keyboard with the cursor on 12 November, and the user presses ← twice and ⏎
- **THEN** the window shows Day with the lanes of 2026-11-10

#### Scenario: The period by keys
- **WHEN** Month shows November, no card has the keyboard, and the user presses ←
- **THEN** Month shows October 2026

#### Scenario: Other keys
- **WHEN** the user presses ⌘Q, ⌘W, ⌘,, ⌘K, `?`, Ctrl+C or ⌘0 to ⌘5 in Month
- **THEN** they act as in Day; `/` starts the filter, `r` in Month opens Month in review, and a key this requirement does not name does nothing

#### Scenario: Sessions from Reports
- **WHEN** Month or Quarter, which took Reports' place, is shown and the user presses ⌘0
- **THEN** the window shows Sessions

#### Scenario: Esc in steps
- **WHEN** by project has the keyboard, a filter is set, and the user presses esc three times
- **THEN** the first gives the keyboard back to the page, the second clears the filter, and the third shows Day

### Requirement: Quitting the window
⌘Q, ⌘W, Ctrl+C and closing the window SHALL quit `tokprof-desk`. In Day and in Sessions, `q` quits too; in the other views it does nothing. esc never quits: in Day and Sessions it closes the full-size inspector, deselects the session, closes the Sessions inspector or clears the filter; in the other views it acts as their keys say. While a filter is being typed, `q` and esc act as session-browser's Quit says, and ⌘Q still quits.

#### Scenario: Closing the window
- **WHEN** the user closes the window
- **THEN** the program exits

#### Scenario: Typing q into a filter
- **WHEN** `q` is pressed while a filter is being typed
- **THEN** `q` is added to the filter

#### Scenario: q in Reports
- **WHEN** `q` is pressed while Month or Quarter, which took Reports' place, is shown
- **THEN** the window stays open

#### Scenario: Esc with nothing to step back from
- **WHEN** Day shows no selected session, no filter is set, and the user presses esc
- **THEN** the window stays open and nothing changes

### Requirement: Key help
`?` SHALL open the key help when no filter is being typed and no overlay is open: a rounded card over the window dimmed and blurred, listing every key of the window grouped under `Anywhere`, `Lanes`, `The day`, `Palette`, `Settings`, `Sessions`, `Inspector`, `Flame`, `Week`, `Month and Quarter` and `Calendar`, each key drawn as a keycap with what it does. esc or `?` close it, and no other key reaches the window behind it, but ⌘Q quits. The palette's `Key help` opens it too.

#### Scenario: Opening the key help
- **WHEN** the lanes have the keyboard and the user presses `?`
- **THEN** the key help opens, and it names ⌘K, ⌘,, ⌘0, `/`, `s`, `r`, `f`, `o`, `c` and `q`

#### Scenario: A filter being typed
- **WHEN** a filter is being typed and the user presses `?`
- **THEN** `?` is added to the filter

#### Scenario: Closing the key help
- **WHEN** the key help is open and the user presses `?`
- **THEN** it closes, and the same session, tab and day are shown

#### Scenario: The keys of Week
- **WHEN** the key help is open
- **THEN** its `Week` group names ↑ ↓, ⏎, ← →, `t`, Tab, `/` and esc with what they do in Week

#### Scenario: The keys of a period
- **WHEN** the key help is opened in Quarter
- **THEN** `Month and Quarter` names ← →, `[` `]`, `t`, Tab, ⏎, `/`, `r` and esc

### Requirement: What to change
After the breakdown cards, Month and Quarter SHALL show three cards with the rows `tokprof share` prints for the period and filters: `Could have been saved` (a row per cause with the avoidable dollars, the share of spend and the dollars per active hour), `Coming back after a cold break` (the five ways with their dollars, share of resuming and the returns each was cheapest in, the cheapest marked) and `Habits that cost the most` (cost and share, with the note that the rows overlap).

#### Scenario: The same rows as share
- **WHEN** Month shows October 2026 and the user runs `tokprof share --since 2026-10-01 --until 2026-11-01` with the same filters
- **THEN** every row of `What could have been saved`, `Coming back after a cold break` and `Habits that cost the most` shows the same name, dollars, share, dollars per active hour and cheapest count, as `idle > TTL` `$873` `7.5%` `$2.27` and `new session + handoff note` `$609`, cheapest in `205 of 231`

#### Scenario: No cold returns
- **WHEN** the period has no cold return
- **THEN** the ways card reads `No cold returns to replay.`

### Requirement: Advice of a row
Under a cause, a habit and the ways back, Month and Quarter SHALL show the sentence tokprof's advice says about it: the cache-miss hint of the cause, the agents' cache lifetime settings, the thinking advice for effort and thinking, the lookup advice, the setup hint of the costliest source, a tool hint's own words, and the rhythm's advice on cold returns. A row tokprof has no advice for shows none.

#### Scenario: Returns after the cache expired
- **WHEN** `idle > TTL` is a row
- **THEN** under it reads `Compact or wrap up a big session before a long break.`

#### Scenario: A row without advice
- **WHEN** `prompts of 400k+` is a row
- **THEN** no sentence is under it

### Requirement: Keys of what to change
In Month and Quarter, Tab SHALL reach the savings, the ways back and the habits after the breakdown cards and before the records. In them ↑ and ↓ (`k`, `j`), PgUp, PgDn, Home and End move the row cursor over the rows and the open sessions, and ⏎ opens or closes a row or opens a session. The key help names these keys.

#### Scenario: A session by keys
- **WHEN** the savings have the keyboard on a row with sessions and the user presses ⏎, ↓ and ⏎
- **THEN** the window shows Day with the row's first session selected

### Requirement: What to change in a frame without a window
`--scene month-advice` SHALL draw the frame in Month on the month of `--month`, else the current month, waiting up to 120 seconds for its cards, with the sessions of the first cause that has any open and scrolled into view.

#### Scenario: The frame
- **WHEN** the user runs `tokprof-desk --snap out.png --scene month-advice --month 2026-10`
- **THEN** `out.png` shows October's what to change with the top sessions of the first such cause

### Requirement: Picks in the menu bar menu
A session's item SHALL bring the window forward and open the session in Day (see desk-sessions, Opening a session in Day), with its `Summary` from the top; an open settings panel, palette or key help closes. `Open tokprof` brings the window forward as it is. `Settings…` brings it forward with the settings panel open. A session no longer listed brings the window forward as it is.

#### Scenario: A session from Reports
- **WHEN** the window shows Month or Quarter, which took Reports' place, hidden behind another app, and the user picks a session in the menu
- **THEN** the window comes forward in Day on that session's day, with that session selected

### Requirement: Calendar view
⌘5 or `Calendar` in the bar's switch SHALL show Calendar: in one scrolling pane under the bar, the metric as pills, the calendar of activity-calendar drawn large, its legend and its numbers, and its punchcard, in rounded cards as in Month. It counts what `tokprof calendar --metric <metric>` counts with the window's `--agent`, `--project`, `--session`, `--since` and `--until`. esc, ⌘1 or `Day` show Day as it was.

#### Scenario: Switching
- **WHEN** the user presses ⌘5
- **THEN** the lanes and the details pane give way to the calendar, and `Calendar` is marked in the switch

#### Scenario: The same numbers as the CLI
- **WHEN** Calendar shows spend and the user runs `tokprof calendar` with the same filters at the same time
- **THEN** every cell's level, the cuts, the total, active days, streaks, the busiest day and weekday, and the punchcard's levels are the same

#### Scenario: From the palette and the key help
- **WHEN** the user picks `Calendar: messages` in the command palette
- **THEN** Calendar shows messages; the palette also offers `Calendar` and each other metric, and the key help lists ⌘5 and the keys of Calendar

### Requirement: Keys in Calendar
In Calendar, a cursor SHALL ring one day, the last day to start with. ↑ and ↓ (`k`, `j`) move it a day back and forward, ← and → (`h`, `l`) a week; Home and End go to the first and the last day; it never leaves the days shown. The day under the cursor shows its tooltip, and ⏎ opens it as a click does. `m` picks the next metric and ⇧M the one before. ⌘Q, ⌘W, ⌘,, ⌘K, `?`, Ctrl+C and ⌘0 to ⌘5 act as in Day; esc shows Day; the other keys do nothing.

#### Scenario: A day by keys
- **WHEN** the cursor is on the last day and the user presses ← and ⏎
- **THEN** the window shows Day with the lanes of the date a week before the last day

#### Scenario: The metric by keys
- **WHEN** Calendar shows spend and the user presses `m` twice
- **THEN** it shows the time at the keyboard

#### Scenario: Sessions from Calendar
- **WHEN** Calendar is shown and the user presses ⌘0
- **THEN** the window shows Sessions

### Requirement: Month in review in the window
Month and Calendar SHALL offer `Month in review` as a button, as the key `r` and as a palette entry. Each opens the card of month-review over the window dimmed and blurred. In Month it is the card of the month shown when that month is complete, else of the last complete month. In Calendar it is the month of a cell cursor a key moved, else the last complete month. ← and → step months, never past this month. esc, `Close` or a click outside close it. While it is open, every key but ⌘Q is its own.

#### Scenario: Opening the card
- **WHEN** Calendar shows the last day of October under the cursor and the user presses `r`
- **THEN** the card of October 2026 opens over Calendar

#### Scenario: From the current month
- **WHEN** Month shows November 2026 so far and the user presses `r`
- **THEN** the card of October 2026 opens over Month, and → shows November so far

#### Scenario: From a complete month
- **WHEN** Month shows September 2026 and the user clicks `Open card`
- **THEN** the card of September 2026 opens

#### Scenario: Another month
- **WHEN** the card shows October and the user presses ←
- **THEN** September is computed in the background, and the card shows October until September arrives

#### Scenario: Keys stay with the card
- **WHEN** the card is open and the user types `q` and `m`
- **THEN** the window stays open and the metric does not change

### Requirement: The card in a frame without a window
`--scene month-review` SHALL draw Month with the card open as `r` opens it there, and `--scene month-card` the card alone, filling the frame. `--month YYYY-MM` picks the month Month shows, and the card's month for `month-card`, else the month of the last day. `--month` takes only YYYY-MM.

#### Scenario: The image of October
- **WHEN** the user runs `tokprof-desk --snap out.png --scene month-card --month 2026-10 --size 540x675`
- **THEN** `out.png` is the card of October, 1080 by 1350 pixels

#### Scenario: A bad month
- **WHEN** the user runs `tokprof-desk --snap out.png --scene month --month october`
- **THEN** the command fails with `--month takes YYYY-MM`

#### Scenario: The card over Month
- **WHEN** the user runs `tokprof-desk --snap out.png --scene month-review --month 2026-11` on 12 November
- **THEN** `out.png` shows Month on November 2026 with the card of October 2026 over it

### Requirement: Key numbers against the period before
Under each key number, Month and Quarter SHALL compare it with the same point of the period before (see The same point of the period before), from the day store. The comparison reads `<arrow> <change> (<percent>)`, or `= <zero>` when the number did not change, and the band's title notes `against <first day> – <last day>`. The avoidable dollars, their share of the spend in points, and the cold returns are green when they fall and red when they rise. The other numbers have no colour.

#### Scenario: More spend
- **WHEN** Month shows November on Thu 12 Nov at 15:00 with $3,954 spent, and the same point of October spent $2,156
- **THEN** the spend reads `↑ $1,798 (+83%)` in the text's colour, and the band notes `against 1–12 Oct, to 15:00`

#### Scenario: Fewer misses
- **WHEN** the avoidable dollars fell from $640 to $597, and their share from 12.5% to 11.3% of spend
- **THEN** the line under them reads `↓ $43.00 (−7%) · ↓ 1.2 points` in green

#### Scenario: More cold returns
- **WHEN** the cold returns rose from 90 to 106
- **THEN** their line reads `↑ 16 (+18%)` in red, comparing the count only

#### Scenario: No earlier data
- **WHEN** the store holds no request, miss, cold return or message in the period before
- **THEN** no number has a comparison, the band notes `no logs in September 2026`, and each tile shows its pace per day (see Pace per day)

#### Scenario: A client filter
- **WHEN** the window runs with `--client`
- **THEN** the key numbers count every client and keep their comparison, and one quiet line reads `the key numbers and charts count every client: the day store has no client` (see Sources of the period's cards)

### Requirement: An incomplete period before
The comparison SHALL be incomplete when an agent with requests in the period shown, and at least 10% of its spend, has its first request in the whole store after the period before starts, as when the agent deleted its older logs before the store kept them. The window's filters do not move that first request. Then there is no comparison: the notes read `no comparison with <period>: <agent>'s logs start <day>`, and Pace per day applies, as month-review's card drops its changes.

#### Scenario: Logs deleted before the store
- **WHEN** Month shows October 2026, and the store's first Claude Code request is on 29 Sep
- **THEN** the band notes `no comparison with September: claude's logs start 29 Sep`, as `tokprof month 2026-10` does, no key number and no cause has a change, and the spend tile reads `$11,632` with `$375 a day` under it

#### Scenario: A complete period before
- **WHEN** Month shows November so far, and every agent's data in the store starts before 1 Oct
- **THEN** the band notes only `against 1–12 Oct, to 15:00`, and the changes are coloured

#### Scenario: A new project
- **WHEN** Month shows November with the filter `~/src/new`, the project's first request is on 5 Nov, and the store's first Claude Code request is on 29 Sep
- **THEN** the comparison is complete and coloured

#### Scenario: Two agents start late
- **WHEN** Quarter shows Q4 2026, the store's first Claude Code request is on 29 Sep and its first Codex request on 6 Aug, and each has 10% or more of Q4's spend
- **THEN** the band notes `no comparison with Q3 2026: claude's logs start 29 Sep, codex's logs start 6 Aug`

#### Scenario: Logs deleted before the store, in Week
- **WHEN** Week shows 28 Sep – 4 Oct 2026, the store's first Claude Code request is on 29 Sep, and claude has 10% or more of the week's spend
- **THEN** its notes read `no comparison with 21–27 Sep: claude's logs start 29 Sep`, no number has a change, and each tile shows its pace per day

### Requirement: A small agent in the period before
An agent whose data starts after the period before starts, but that has less than 10% of the period shown's spend (of its requests, when nothing is priced), SHALL NOT make the comparison incomplete. The notes of Week and the band of Month and Quarter name it after the dates: `; <agent> only from <day>, <share>% of spend, not judged`.

#### Scenario: Trying a new agent
- **WHEN** OpenCode's first request in the store is on 5 Nov and it has 3% of November's spend so far
- **THEN** the band notes `against 1–12 Oct, to 15:00; opencode only from 5 Nov, 3% of spend, not judged`, and the changes stay coloured

#### Scenario: Trying a new agent in Week
- **WHEN** OpenCode's first request in the store is on 29 Oct and it has 3% of the spend of 2–8 Nov 2026
- **THEN** Week's notes read `against 26 Oct – 1 Nov; opencode only from 29 Oct, 3% of spend, not judged`, and the changes stay coloured

### Requirement: Causes against the period before
Each row of `Could have been saved` SHALL give, after its dollars, the cause's avoidable dollars in the period before and the change in percent: `was <dollars> · <arrow> <percent>`. A cause with no dollars before reads `new`. The change is green when it falls and red when it rises. When the key numbers have no comparison (see Pace per day), or with `--client`, the rows have none either.

#### Scenario: Fewer idle misses
- **WHEN** `idle > TTL` could have saved $796, and $1,012 in the period before
- **THEN** the row reads `idle > TTL` `$796` `was $1,012 · ↓ 21%`, the change in green

#### Scenario: A new cause
- **WHEN** `tools changed` could have saved $12 and had nothing in the period before
- **THEN** its row reads `new`

### Requirement: Causes gone since the period before
After its rows, `Could have been saved` SHALL list each miss cause that had avoidable dollars in the period before and none in the period shown, most first. Such a row has `$0.00`, no share bar, nothing to open, and reads `was <dollars> · gone` in green. With no comparison (see Pace per day), no such row is listed.

#### Scenario: Compaction calls stopped
- **WHEN** compaction calls could have saved $367 in October and nothing in November so far, against the same point of October
- **THEN** the last row reads `compaction call` `$0.00` `was $367 · gone` in green

### Requirement: Pace per day
When the numbers of Week, Month or Quarter have no comparison (no earlier data, an incomplete period before, `--since` or `--until`), each tile SHALL show under it its pace per day instead: its number over the days the period has run, from the later of its first day and the day of the store's first request, today counting as a whole day. The notes say why. The avoidable tile keeps its share of spend and its cold returns; saved by the cache shows its multiple of the spend.

#### Scenario: A first week
- **WHEN** the store's first request is on Wed 4 Mar 2026 and Week shows 2–8 Mar 2026 with $700 spent
- **THEN** the spend tile reads `$700` with `$140 a day` under it, over the 5 days from 4 Mar

#### Scenario: Within --since
- **WHEN** the window was started with `--since 2026-11-04` and Week shows 2–8 Nov 2026
- **THEN** the week counts from 4 November, the notes read `within --since 2026-11-04`, and no number has a comparison

#### Scenario: A quarter without a quarter before
- **WHEN** Quarter shows Q4 2026 on 12 November with $15,586 spent and no comparison
- **THEN** the spend tile reads `$362 a day`, over the 43 days from 1 October

#### Scenario: Saved by the cache
- **WHEN** October 2026 spent $11,632 and the cache saved $87,329, with no comparison
- **THEN** the tile reads `$87,329` with `7.5× the spend` under it

### Requirement: Window keys in Week
In Week, ⌘Q, ⌘W, ⌘,, ⌘K, `?`, Ctrl+C, ⌘0 and ⌘1 to ⌘5 SHALL act as in Day, and `/` starts the filter. esc gives the keyboard back to the days, then clears the filter, then shows Day. `q` and the other keys do nothing.

#### Scenario: q in Week
- **WHEN** `q` is pressed while Week is shown
- **THEN** the window stays open

#### Scenario: Esc in steps
- **WHEN** By project has the keyboard, a filter is set, and the user presses esc three times
- **THEN** the first gives the keyboard back to the days, the second clears the filter, and the third shows Day

#### Scenario: Month from Week
- **WHEN** Week is shown and the user presses ⌘3
- **THEN** Month shows the current month

### Requirement: The week before
A week in progress SHALL compare with the same point of last week: from last week's Monday 00:00 to seven days before now. A full week compares with the whole week before it. The rules of An incomplete period before and A small agent in the period before apply. There is no comparison when the store holds no request, miss, cold return or message of the window's agents in the week before, whatever the other filters; then Pace per day applies.

#### Scenario: Thursday afternoon
- **WHEN** it is Thursday 12 Nov 2026 at 14:20
- **THEN** this week compares with Mon 2 Nov 00:00 to Thu 5 Nov 14:20, and the notes read `against 2–5 Nov, to 14:20`

#### Scenario: A full week
- **WHEN** Week shows 2–8 Nov 2026
- **THEN** it compares with 26 Oct – 1 Nov, and the notes read `against 26 Oct – 1 Nov`

#### Scenario: No earlier data
- **WHEN** Week shows 2–8 Mar 2026 and the store holds no request and no message in 23 Feb – 1 Mar
- **THEN** no number has a comparison, the notes read `no logs in 23 Feb – 1 Mar`, and each tile shows its pace per day

#### Scenario: A project new this week
- **WHEN** Week shows 2–8 Nov 2026 with the filter `~/src/zebrafish`, the project's first request is on Tue 3 Nov, and the store holds Claude Code's requests of other projects in 26 Oct – 1 Nov
- **THEN** the notes read `against 26 Oct – 1 Nov`, and each number's change counts from nothing, such as spend from $0, without a percent

## REMOVED Requirements

### Requirement: Report period
**Reason**: The rolling `7 days`, `30 days` and `90 days` pills give way to calendar months and quarters, which match a bill and the Month in review card, and compare with the same point of the period before.
**Migration**: Month and Quarter (⌘3, ⌘4) with ‹ › to step; see Month and quarter periods. `tokprof summary --since 30d` still gives a rolling window on the command line.

### Requirement: Reports in a frame without a window
**Reason**: Reports is gone; Month and Quarter draw their own frames.
**Migration**: `--scene month` and `--scene quarter`, with `--month YYYY-MM` and `--quarter YYYY-Qn`; see Month and Quarter in a frame without a window.


## ADDED Requirements

### Requirement: Month and quarter periods
Month SHALL show one calendar month and Quarter one calendar quarter (January–March, April–June, July–September, October–December): from local midnight on the first day to local midnight on the next period's first day, or until now for the current period, which is titled `so far`. Both open on the current period. ‹ and › step one period, never past the current one nor before the period of the store's first day; Today goes back to the current one.

#### Scenario: Stepping back
- **WHEN** Month shows November 2026 so far and the user clicks ‹
- **THEN** Month shows October 2026, the band reads `October 2026` and the bar `Thu 1 – Sat 31 Oct · vs September`

#### Scenario: Never past now
- **WHEN** Quarter shows Q4 2026 on 12 November and the user clicks ›
- **THEN** Quarter stays on Q4 2026, whose bar reads `1 Oct – 12 Nov, 43 of 92 days`

#### Scenario: Never before the logs
- **WHEN** the store's first request is on 6 Aug 2026 and Month shows August 2026
- **THEN** ‹ does nothing

#### Scenario: Between scales
- **WHEN** Month shows October 2026 and the user presses ⌘4, then ⌘3
- **THEN** Quarter shows Q4 2026, and Month then shows October 2026 again; from Quarter alone, Month shows the quarter's last month that has begun

#### Scenario: The window's filters
- **WHEN** the window was started with `--since 2026-10-09`
- **THEN** October counts only from 9 October, the band notes `within --since 2026-10-09`, and no number has a comparison

### Requirement: The same point of the period before
The period before SHALL be the month or the quarter just before the one shown. A complete period compares with the whole period before. A current period compares with the period before from its start for as long as the current one has run, never past its end, as month-review's The month before counts it. The window's filters and filter apply to both.

#### Scenario: A month so far
- **WHEN** Month shows November on 12 November at 15:00
- **THEN** the period before runs from 1 October 00:00 to 12 October 15:00, as `tokprof month this` compares

#### Scenario: A quarter so far
- **WHEN** Q4 2026 has run 42 days and 15 hours
- **THEN** the period before runs for 42 days and 15 hours from 1 July 00:00, to 12 August

#### Scenario: A longer month
- **WHEN** Month shows March on 31 March at 10:00
- **THEN** the period before is the whole of February

#### Scenario: A complete month
- **WHEN** Month shows October 2026
- **THEN** it compares with the whole of September 2026

### Requirement: Days of the month
Month SHALL draw its dates as a grid, a row per week from Monday to Sunday and a cell per date. Each cell holds its date and its calendar cell's spend, filled at the level of spend that the Month in review card gives it (see activity-calendar, Calendar levels, over the month's days). A cell has a tick in the cold color per cold return, up to 5, and `+<n>` beyond. Today's cell is outlined in the accent; dates to come show only their number. A legend names the cuts.

#### Scenario: Hovering a date
- **WHEN** the pointer rests over the cell of 11 November
- **THEN** a tooltip reads like `Wed 11 Nov 2026 · $906 spent, $56 avoidable`, `6 cold returns`, `click: open this day`

#### Scenario: Opening a date
- **WHEN** the user clicks the cell of 5 November
- **THEN** the window shows Day with the lanes of 2026-11-05

#### Scenario: Dates to come
- **WHEN** Month shows November on the 12th
- **THEN** the cells of 13 to 30 November show only their number, have no tooltip and open nothing

#### Scenario: Before the logs
- **WHEN** the window runs with `--agent claude`, the store's first Claude Code request is on 29 Sep, and Month shows September 2026
- **THEN** the cells of 1 to 28 September read `no logs` in the subtle ink, not level none, and open nothing

#### Scenario: The legend
- **WHEN** Month shows October 2026
- **THEN** the legend reads `less`, the five fills, `more · cuts at $121, $269, $591`, as the card of October

#### Scenario: Many cold returns
- **WHEN** a date has 9 cold returns
- **THEN** its cell has 5 ticks and `+4`

### Requirement: Records of the month
Month SHALL list, from the day store: the biggest day by spend; the quietest weekday, the Monday-to-Friday date with the least spend above zero; the date with the most cold returns; the longest run of days with a request inside the month, `still going` when it ends today; and the busiest hour and the night share as month-review counts them. A tie goes to the earliest. A record with a date opens it in Day.

#### Scenario: October
- **WHEN** Month shows October 2026
- **THEN** the records read `Biggest day $1,221 Sun 25 Oct`, `Streak 31 days, Thu 1 Oct – Sat 31 Oct`, `Busiest hour 23:00–00:00, 7% of agent time` and `Night share 31%`, as `tokprof month 2026-10` prints

#### Scenario: A streak still going
- **WHEN** Month shows November on the 12th and every day of it had a request
- **THEN** the streak reads `12 days, since Sun 1 Nov, still going`

### Requirement: Month in review in Month
Month's last card SHALL show a small image of the Month in review card it opens (see Month in review in the window), its month's name, `Open card` and `Save image`. Save image acts as the card's own (see Saving the card). While the month shown is the current one, the card says when its own card is ready, as `November's card is ready on Tue 1 Dec`.

#### Scenario: The current month
- **WHEN** Month shows November 2026 so far
- **THEN** the card shows the image of October 2026 and reads `November's card is ready on Tue 1 Dec`

### Requirement: Weeks of the quarter
Quarter SHALL draw a column per Monday-to-Sunday week that touches it, 13 or 14, the edge weeks cut at the quarter's bounds: a week sums the calendar cells of its dates inside the quarter. A column stacks the spend of the quarter's 6 projects with the most spend, from the bottom, and the rest as `other`, with a legend. A click on a column opens Week on that week; a click on a project in the legend sets the filter and stays on the quarter. Weeks to come are a bare baseline; there is no forecast.

#### Scenario: Fourteen weeks
- **WHEN** Quarter shows Q4 2026, which runs from Thursday 1 October to Thursday 31 December
- **THEN** it has 14 columns, from the week of 28 Sep, labeled `Thu–Sun`, to the week of 28 Dec, which counts 28 to 31 December

#### Scenario: Thirteen weeks
- **WHEN** Quarter shows Q2 2024, which starts on Monday 1 April
- **THEN** it has 13 columns

#### Scenario: Hovering a week
- **WHEN** the pointer rests over the column of the week of 19 Oct
- **THEN** a tooltip names `19–25 Oct`, its spend, each project's spend in it, and `click: open this week`

#### Scenario: The current week
- **WHEN** Quarter is shown on Thursday 12 November
- **THEN** the column of the week of 9 Nov reads `so far`, and the columns from 16 Nov on have no bar, no number and open nothing

#### Scenario: A project filter
- **WHEN** the filter is set to `tokprof`
- **THEN** the columns stack only the projects that match, by the same rule

#### Scenario: A project in the legend
- **WHEN** the user clicks `zebrafish` in the legend
- **THEN** the filter reads `~/src/zebrafish`, and Quarter stays on its quarter

#### Scenario: The rest in the legend
- **WHEN** the quarter has 24 projects with spend
- **THEN** the legend names the six and then `other, 18 projects`

#### Scenario: A project's part of a column
- **WHEN** the user clicks the part of a column that is `zebrafish`
- **THEN** the window shows Week on that column's week, as a click anywhere on the column does, and the filter does not change

#### Scenario: Weeks before the logs
- **WHEN** a week of the quarter lies wholly before the store's first request
- **THEN** its column reads `no logs`, not a zero

### Requirement: Months of the quarter
Under the weeks, Quarter SHALL bracket each month of the quarter with its name, its spend and its pace per day, counted as `tokprof month` counts a month, or `has not begun`. Beside the chart, a Months card lists them with bars, and under it a line compares the current month's pace per day with the month before's. A click on a month's name under the weeks, or on its row in the Months card, opens Month on it.

#### Scenario: November against October
- **WHEN** Quarter shows Q4 2026 on 12 November, October spent $11,632 and November $3,954 so far
- **THEN** the line reads `November runs $330 a day against October's $375 (−12%)`, and December reads `has not begun`

#### Scenario: Opening a month
- **WHEN** the user clicks October in the Months card
- **THEN** Month shows October 2026

#### Scenario: A month under the weeks
- **WHEN** the user clicks `November` under the weeks
- **THEN** Month shows November 2026 so far, and no week is selected

### Requirement: Trends of the quarter
Under the months, Quarter SHALL draw three lines with a point per week that has requests: `Read from the cache`, the prompt tokens read from the cache over all prompt tokens; `Avoidable share`, avoidable spend over spend; and `Average context`, all prompt tokens over all requests, in thousands. Each line names its first and its last value. The pointer over a week names its three values, and a click opens Week on it.

#### Scenario: A week without requests
- **WHEN** a week of the quarter has no request
- **THEN** none of the three lines has a point there, and each line breaks

#### Scenario: The cache in October
- **WHEN** the store holds October's prompt tokens, 98% of them read from the cache
- **THEN** the October weeks of the first line lie near 98%, as the card of October reads `98% of input read from cache`

### Requirement: Project shifts
Beside the weeks, Quarter SHALL list how its projects moved over its ended weeks, at most 5 rows. `started`: the project's first request in the store falls after the quarter's first week, and it reaches 5% of a week's spend. `paused`: after a week at 5% or more, two or more whole weeks in a row without a request. `grew` or `fell`: its share of spend over the second half of the ended weeks differs by 5 points or more from the first half. A click on a row sets the filter.

#### Scenario: A new project
- **WHEN** swing-rune had its first request in the week of 2 Nov and 12% of that week's spend
- **THEN** a row reads `swing-rune started` `week of 2 Nov`, with its spend in the quarter

#### Scenario: A pause
- **WHEN** airpods-motion had 5% of the week of 19 Oct and no request in the weeks of 26 Oct and 2 Nov
- **THEN** a row reads `airpods-motion paused` `26 Oct – 8 Nov`

#### Scenario: A shift between halves
- **WHEN** the quarter has 6 ended weeks and air had 12% of the first three weeks' spend and 6% of the last three
- **THEN** a row reads `air fell` `12% → 6%`

#### Scenario: Logs that start late
- **WHEN** a project's requests are all Claude Code's, and the store holds Claude Code's requests from less than 4 weeks before the project's first week
- **THEN** the project is not `started`, since the store cannot tell a new project from old logs that are gone

#### Scenario: Few weeks
- **WHEN** the quarter has fewer than 4 ended weeks
- **THEN** no project `grew` or `fell`

#### Scenario: Order and room
- **WHEN** more than 5 shifts hold
- **THEN** `started` rows come first, then `paused`, `grew` and `fell`, each kind by the project's spend in the quarter, and the first 5 show

#### Scenario: Nothing moved
- **WHEN** no shift holds
- **THEN** the card reads `No project started, paused or moved 5 points this quarter.`

### Requirement: Records of the quarter
Quarter SHALL list: the biggest day and the biggest week by spend; the longest run of days with a request; the most sessions whose agents worked at once, with when; and the week with the highest share read from the cache, from the day store. It also lists the largest prompt of any session, with its project and date, from the quarter's logs. A record with a day or a week opens it.

#### Scenario: Q4 2026
- **WHEN** Quarter shows Q4 2026 on 12 November
- **THEN** the biggest day reads `$1,221 Sun 25 Oct`, as `tokprof month 2026-10` names it, and the streak counts 43 days since Thu 1 Oct, `still going`

#### Scenario: Before the logs arrive
- **WHEN** the quarter's logs are still being read
- **THEN** the largest prompt reads `reading the logs…`, and the other records show

### Requirement: Cards to share of the quarter
Quarter SHALL have no card of its own. Its last card shows the image of the Month in review card of its last complete month, and a button per month of the quarter: a complete month opens its card, the current month opens its card so far, and a month to come is disabled.

#### Scenario: Q4 2026
- **WHEN** Quarter shows Q4 2026 on 12 November
- **THEN** the card shows October's image, with `Open October`, `November so far`, and `December` disabled

### Requirement: The filter in Month and Quarter
In Month and Quarter the bar's filter SHALL count, in every card and in the period before, only the logs whose agent, project or session id contains its text, ignoring case, as The filter in Week counts a week. Titles do not match, since the day store holds none, and the note for a match by title only names Month or Quarter. `/` starts it and esc clears it. The band's title names it. It is the window's one filter: Day's lanes and the Sessions table show what it keeps.

#### Scenario: A project row
- **WHEN** the user clicks `zebrafish` in by project
- **THEN** the filter reads `~/src/zebrafish`, the title reads `November 2026 so far · ~/src/zebrafish`, and the period stays November

#### Scenario: Nothing matches
- **WHEN** the filter is `zzz`
- **THEN** the pane reads `Nothing in November 2026 matches "zzz".`

#### Scenario: A match by title only
- **WHEN** Month shows November and the filter is `python`, which 3 listed sessions have only in their titles
- **THEN** a note reads `3 sessions match "python" only by title; Month counts agents, projects and session ids, not titles. Sessions (⌘0) lists them.`

#### Scenario: Back to Day
- **WHEN** a filter is set in Month and the user presses ⌘1
- **THEN** Day's lanes show only the sessions the filter keeps, and so does the Sessions table

### Requirement: Sources of the period's cards
The key numbers, their comparison, the days of the month, spend by day, the weeks, months, trends and shifts of the quarter, by project and the records but the largest prompt SHALL come from the day store, refreshed as Calendar refreshes it, and show without reading the logs again. By origin, by model, the pause tables, what to change and the largest prompt read the period's logs. With `--client`, the store's numbers count every client, and the band says so.

#### Scenario: Unchanged logs
- **WHEN** no log changed since the store was last written and the user steps to October
- **THEN** the key numbers, the grid and the day chart show within a second, before the log cards

#### Scenario: A client filter
- **WHEN** the window runs with `--client sdk-ts`
- **THEN** the band reads `the key numbers and charts count every client: the day store has no client`, and by origin, by model, the pause tables and what to change count only `sdk-ts`

### Requirement: Month and Quarter in a frame without a window
`--scene month` SHALL draw the frame in Month and `--scene quarter` in Quarter, on the month of `--month YYYY-MM` or the quarter of `--quarter YYYY-Qn`, else the current one. Each waits up to 120 seconds for the day store and the cards that read the logs. `--quarter` takes only YYYY-Qn, and a period after the current one fails.

#### Scenario: October
- **WHEN** the user runs `tokprof-desk --snap out.png --scene month --month 2026-10`
- **THEN** `out.png` shows October's cards, with `$11,632` in the spend tile

#### Scenario: A quarter
- **WHEN** the user runs `tokprof-desk --snap out.png --scene quarter --quarter 2026-Q4`
- **THEN** `out.png` shows Q4 2026 with its 14 weeks

#### Scenario: A bad quarter
- **WHEN** the user runs `tokprof-desk --snap out.png --scene quarter --quarter 2026-Q5`
- **THEN** the command fails with `--quarter takes YYYY-Qn`

#### Scenario: A period to come
- **WHEN** the user runs `tokprof-desk --snap out.png --scene month --month 2027-05` in November 2026
- **THEN** the command fails with `May 2027 has not begun`
