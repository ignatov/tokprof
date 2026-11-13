# Spec Delta

## ADDED Requirements

### Requirement: Lanes header
Over the hours row, the lanes SHALL have a header line with the day's caption at its left and the zoom, `Fit` or `24 h`, at its right. The caption gives the day, when its first action was, its end or `now`, its sittings, how many sessions have a lane, as `<n> in the lanes`, and the Spend card's dollars. The current day is named by its weekday, another day by its date.

#### Scenario: Today
- **WHEN** the current day is Thursday, its first action was at 09:24, and it had 7 sittings, 11 sessions with a lane and $146.00 of spend so far
- **THEN** the header reads `Thursday: 09:24 to now, 7 sittings, 11 in the lanes, $146.00`

#### Scenario: Another day
- **WHEN** the lanes show Tue 10 Nov, which ran from 08:12 to 23:40
- **THEN** the caption starts `Tue 10 Nov: 08:12 to 23:40`

#### Scenario: The zoom
- **WHEN** the user clicks `Fit` in the header
- **THEN** the lanes show the hours Now and zoom gives for `Fit`, and `Fit` is marked

### Requirement: Details pane
Under the lanes, the details pane SHALL show the day summary while no session is selected. Otherwise it shows the selected session's inspector, headed by a `‹ The day` button with esc as its keycap and by the tabs. `‹ The day` and esc deselect the session. Selecting another lane keeps the tab shown. Every other way of selecting a session shows its `Summary` from the top.

#### Scenario: Back to the day
- **WHEN** a session is selected and the user clicks `‹ The day`
- **THEN** no lane is highlighted and the details pane shows the day summary

#### Scenario: Another lane
- **WHEN** the inspector shows `Cache` and the user clicks another lane
- **THEN** the inspector shows `Cache` of that lane's session

#### Scenario: From the day summary
- **WHEN** the user clicks a row of `Top sessions of the day`
- **THEN** its session is selected, its lane highlighted, and the inspector shows its `Summary` from the top

### Requirement: Day summary
With no session selected, the details pane SHALL show `The day` with the day's date. Six cards follow in a row: Spend, Sittings, Agents, Answers, Queue and Cache. Under them, `Top sessions of the day` takes the left column. The right column holds `Cold returns of the day` and, on the current day, `Waiting for you now`. When the window is narrower than 1,200 points, the cards go three in a row.

#### Scenario: The current day
- **WHEN** the window starts on the current day
- **THEN** the details pane shows the six cards, the top sessions, the cold returns and who waits now

#### Scenario: A past day
- **WHEN** the lanes show yesterday with no session selected
- **THEN** the summary shows yesterday's cards, top sessions and cold returns, and no `Waiting for you now`

### Requirement: Spend card
The Spend card SHALL give the day's spend: the cost of the requests inside the day's bounds (work-rhythm, Start of the day), subagents and the sessions the rhythm leaves out included. Under it is a bar for each hour from the day's first hour with a request to its last. On the current day the card compares with the day before up to the same time of day, and on a past day with the whole day before.

#### Scenario: The same spend as the calendar
- **WHEN** the card shows a day whose logs were not deleted, and the calendar shows the same day
- **THEN** the card's spend equals the spend of that day's cell in the calendar, unless the two find another night, as activity-calendar's Calendar days allows

#### Scenario: Against yesterday
- **WHEN** it is 14:20, and the day before had spent $431 from its start to 14:20
- **THEN** the card reads `vs $431 by 14:20 yesterday`

#### Scenario: A past day
- **WHEN** the card shows Tue 10 Nov, and Mon 9 Nov spent $612
- **THEN** the card reads `vs $612 the day before`

#### Scenario: An hour under the pointer
- **WHEN** the pointer rests over the bar of 11:00
- **THEN** a tooltip names `11:00–12:00` and the spend of the requests in that hour

#### Scenario: No day before
- **WHEN** the store holds no request on the day before
- **THEN** the card has no comparison line

### Requirement: Rhythm cards
The Sittings, Agents, Answers, Queue and Cache cards SHALL each show one number large and, under it, the rest of the day's line of that name, as `tokprof day` prints it, without its label. Sittings shows the count of sittings large, Agents the agent-hours, Answers the median answer, Queue the share of answers that went to the session that waited least, and Cache the count of cold returns. A line that says there is nothing shows `—` large.

#### Scenario: The same words as the CLI
- **WHEN** `tokprof day 2026-11-12` prints `sittings  7, 1.8h at the keyboard, 34 messages to 16 sessions; median sitting 1.1m, median break 29m`
- **THEN** the Sittings card shows `7` large and `1.8h at the keyboard, 34 messages to 16 sessions; median sitting 1.1m, median break 29m` under it

#### Scenario: No answer among several
- **WHEN** the day's queue line reads `queue     no answer while another session waited`
- **THEN** the Queue card shows `—` large and `no answer while another session waited`

#### Scenario: Cold returns
- **WHEN** the day's cache line reads `cache     3 cold returns, $4.60 in their first requests, $4.14 of it avoidable`
- **THEN** the Cache card shows `3 cold returns` large in the theme's red, and the dollars under it

### Requirement: Top sessions of the day
`Top sessions of the day` SHALL list the 5 sessions with the most spend inside the day's bounds, most first. Each row has the agent's dot, the project and the title, a bar as long as its share of the first, its spend that day and its `Next` cell. `<n> more sessions` follows with the spend of the rest. A session's spend that day counts its requests and its subagents' inside the day's bounds. A click on a row selects its session.

#### Scenario: A session from yesterday
- **WHEN** a session cost $90 yesterday and $28 today
- **THEN** its row on today's summary reads `$28.00`

#### Scenario: The rest
- **WHEN** the day had 16 sessions with spend
- **THEN** 5 rows show, then `11 more sessions` with their spend, and the rows and the rest add up to the Spend card's dollars

### Requirement: Cold returns of the day
`Cold returns of the day` SHALL list the day's cold returns (pause-report) in time order. Each row has the time, the session's dot, project and title, `after <pause>`, the first request's cost, and its avoidable dollars in the theme's red. With none, it reads `No cold returns.` A click on a row selects its session.

#### Scenario: A return after lunch
- **WHEN** a session came back at 13:08 after a pause of 1.2h with a first request of $2.40, $2.27 of it avoidable
- **THEN** its row reads `13:08`, the session, `after 1.2h`, `$2.40` and `$2.27`

#### Scenario: The same returns as the cache line
- **WHEN** the Cache card counts 3 cold returns
- **THEN** the list has 3 rows, and their costs and avoidable dollars add up to the card's

### Requirement: Waiting for you now
On the current day, `Waiting for you now` SHALL list the sessions that wait for the user now (work-rhythm, Waiting now), the longest wait first. Each row has the dot, the project, the title and `since <HH:MM>, <wait>`. With none, it reads `Nobody waits for you`. A click on a row selects its session. The list changes as waits begin and end, without the day being computed again.

#### Scenario: A session asked
- **WHEN** a session's agent ended its turn at 14:16 and it is 14:20
- **THEN** its row reads `since 14:16, 4m`

#### Scenario: The waiting limit
- **WHEN** a wait reaches 2 hours
- **THEN** its row leaves the list

### Requirement: Keys in the day summary
While the day summary has the keyboard, ↑ and ↓ (`k`, `j`) SHALL move a cursor over the rows of its three lists, the left list first, then the right top to bottom. Home and End go to the ends. ⏎ selects the cursor's session, as a click does. esc gives the keyboard back to the lanes.

#### Scenario: A cold return by keys
- **WHEN** the day summary has the keyboard with the cursor on the last top session, and the user presses ↓ and ⏎
- **THEN** the session of the first cold return is selected and shown in the inspector

### Requirement: Day summary computation
The day summary SHALL come from the day the lanes show and from the day store (activity-calendar). Day refreshes the store on a worker when a day is shown, and on the current day again when the logs change, once 4 times its last refresh and at least 20 seconds have passed. No frame waits for it. While the store is being filled, the Spend card and the top sessions read `reading the logs into the store… <n> of <m> logs`.

#### Scenario: No store yet
- **WHEN** the window starts with no store on disk
- **THEN** the lanes and the rhythm cards show as soon as the day is computed, and the Spend card and the top sessions show once the store is filled

#### Scenario: At rest
- **WHEN** the summary has arrived and the logs do not change
- **THEN** the window draws no more frames for it, except the countdowns and waits on the screen

#### Scenario: The day before
- **WHEN** the summary of a day has been computed once
- **THEN** stepping to another day and back shows it at once, from the same store

#### Scenario: A client filter
- **WHEN** the window runs with `--client`
- **THEN** the Spend card and the top sessions count every client, and one quiet line on the Spend card reads `every client: the day store has no client`

### Requirement: Border between lanes and details
The border between the lanes and the details pane SHALL be draggable. A drag moves it so that the lanes, their header and what is under them take from 25% to 75% of the height under the bar, and the details pane keeps at least 200 points. A double click on the border gives back the room that Room for the lanes gives. The place holds across days and screens, and is saved as `lanes_share` (see Saved settings) for the next launch.

#### Scenario: More lanes
- **WHEN** the user drags the border down to 70% of the height
- **THEN** more lanes show whole, and the details pane takes the rest

#### Scenario: Back to the default
- **WHEN** the user double-clicks the border
- **THEN** the lanes take the room Room for the lanes gives them, and the settings file no longer holds `lanes_share`

#### Scenario: The next launch
- **WHEN** the user drags the border to 70% of the height, quits the window and starts it again
- **THEN** the lanes take 70% of the height under the bar, and the settings file holds `"lanes_share": 0.7`

#### Scenario: A saved place that does not fit
- **WHEN** the settings file holds `lanes_share` 0.9, or the window is so low that the saved place leaves the details pane under 200 points
- **THEN** the border goes to the nearest place the drag allows

## MODIFIED Requirements

### Requirement: Saved settings
The window SHALL keep its settings in `prefs.json` in `~/Library/Application Support/io.github.ignatov.tokprof/`: `appearance` as `system`, `light` or `dark`, `light_theme` and `dark_theme` as theme ids, and the other settings by the names their requirements give, as `menu_bar`, `waiting` and `lanes_share`. It writes the file only when a choice changes and reads it when it starts. A missing or broken file gives the defaults, and a value it does not know gives that setting's default.

#### Scenario: The next launch
- **WHEN** the user picks `Light` and `Alpine Milk`, quits the window and starts it again
- **THEN** the window opens light, in Alpine Milk

#### Scenario: No settings yet
- **WHEN** the window starts and the settings file does not exist
- **THEN** the window follows the system in Merino Light and Merino Dark, and no file is written until a choice changes

#### Scenario: The file cannot be written
- **WHEN** the user picks a theme and the settings file cannot be written
- **THEN** the window takes the theme until it quits, and the bar says `can't save settings`

#### Scenario: The border between lanes and details
- **WHEN** the user drags the border between the lanes and the details pane
- **THEN** the settings file holds its place as `lanes_share`, the share of the height under the bar, from 0.25 to 0.75

### Requirement: Opening the window
`tokprof-desk` SHALL open one window titled `tokprof`, 1400 by 900 points the first time and no smaller than 960 by 640, listing every session that `tokprof ui` lists with the same `--project`, `--session`, `--since` and `--until` filters. It starts in Day on the day of the latest action with no session selected, the day summary under the lanes. Its colors are those of the theme the Color themes requirement picks.

#### Scenario: Default start
- **WHEN** the user runs `tokprof-desk`
- **THEN** a window opens with the lanes of the latest day on top and that day's summary under them, and no lane is highlighted

#### Scenario: Filters
- **WHEN** the user runs `tokprof-desk --project zebrafish --since 7d`
- **THEN** the Sessions table holds only the sessions that `tokprof ui --project zebrafish --since 7d` lists

#### Scenario: No logs
- **WHEN** no session logs are found
- **THEN** no window opens, and the command fails with `no session logs found`

#### Scenario: The system turns dark
- **WHEN** the Appearance setting is `As in system` and the system switches from light to dark while the window is open
- **THEN** the window switches to its dark theme without a restart

### Requirement: Title bar
The window SHALL draw its own bar along the top, 46 points high, with the window's close, minimize and zoom buttons at its left and nothing of the window's own under them. Dragging the bar where it has no control moves the window, and a double click on it zooms the window as the system's setting says. The latest status message shows at the bar's right for 6 seconds.

#### Scenario: The buttons
- **WHEN** the window opens
- **THEN** the close, minimize and zoom buttons sit at the left of the bar, and the switch starts right of them

#### Scenario: Moving the window
- **WHEN** the user drags the bar where it has no control
- **THEN** the window moves

#### Scenario: Writing a profile
- **WHEN** `o` is pressed
- **THEN** the bar reads `wrote <n> frames to ~/tokprof/<id>.html` and the page opens, as in `tokprof ui`

### Requirement: Day controls
In Day, the bar SHALL hold the shown day's name with buttons to the day before and after, a Today button, the filter, and a `Live` mark while the window follows the logs. The day's caption and the zoom are in the lanes' header. In Sessions, the bar holds `All sessions` with the date of the first log and the count of listed sessions in place of the day's name and its buttons. In Reports and in Calendar, the day's name, its buttons, Today and the filter are not shown.

#### Scenario: Other days
- **WHEN** the user clicks the button before the day's name
- **THEN** the lanes show the day before, and Today goes back to the day of the latest action

#### Scenario: Never past the latest day
- **WHEN** the lanes show the day of the latest action and the user clicks the button after its name
- **THEN** the lanes stay on that day

#### Scenario: In Reports
- **WHEN** Reports is shown
- **THEN** the bar holds the switch, the `Sessions` button, the status and `Live`, and no day controls and no filter

#### Scenario: In Calendar
- **WHEN** Calendar is shown
- **THEN** the bar holds the switch, the `Sessions` button, the status and `Live`, and no day controls and no filter

#### Scenario: In Sessions
- **WHEN** Sessions is shown on logs that start on 3 Mar with 412 listed sessions
- **THEN** the bar reads `All sessions` and `since 3 Mar · 412 sessions`, then Today, the filter and `Live`

### Requirement: Window layout
In Day, the window SHALL show, under the bar, the lanes' header and the lanes of the shown day across its whole width, and under them the details pane across its whole width (see Details pane). In Sessions, it shows the Sessions screen (see desk-sessions). Every text is in the system font; numbers that line up in columns use tabular figures.

#### Scenario: A narrow window
- **WHEN** the window is 960 points wide
- **THEN** the details pane is 960 points wide, and the inspector in it has all ten tabs on one line

#### Scenario: Many lanes
- **WHEN** the day has more lanes than fit in their room
- **THEN** the lanes scroll under an hours row that stays on top, as many whole lanes show as fit, a line under them says how many more there are (`2 more lanes · scroll to see them`), and the details pane keeps its place

### Requirement: Day lanes
The lanes SHALL show the day as work-rhythm's Day strip with shapes, at the times of the log: a `You` lane with the sittings, the `Agents at work` lane, then a lane per session with actions that day, most actions first, labeled with its project and title. Its bars and marks take work-rhythm's colors, and the selected session's lane is highlighted.

#### Scenario: Bars and marks
- **WHEN** a session's agent worked, waited for the user's answer, asked a question, and the user typed and stopped it
- **THEN** the work and the wait are bars from their start to their end, at least 1 point wide, and the message, the question and the stop are marks at their moment

#### Scenario: A short piece of work
- **WHEN** the agent worked from 14:32:10 to 14:32:50
- **THEN** its lane has a green bar 40 seconds wide at 14:32

#### Scenario: A click on a lane
- **WHEN** the user clicks the lane of session `ecad11`
- **THEN** `ecad11` is selected, its lane is highlighted, and the details pane shows its inspector

### Requirement: What the lanes say
Under the lanes SHALL be the legend and a line that names what is under the pointer: the time, the lane and what it shows there. The day's `sittings`, `agents`, `answers`, `queue` and `cache` lines are on the day summary's cards (see Rhythm cards).

#### Scenario: The time under the pointer
- **WHEN** the pointer rests over a lane at 14:32
- **THEN** the line reads like `14:32 · ecad11 Chart of the interruptions · agent working`, with `agent working`, `waiting for your answer`, `your message`, `its question`, `you stopped it`, `you were there`, `cache warm` or nothing

#### Scenario: The queue line
- **WHEN** the shown day had 24 answers among several sessions waiting (see work-rhythm)
- **THEN** the Queue card reads the `queue` line that `tokprof day` prints for that day, without its label

#### Scenario: The agents at work under the pointer
- **WHEN** the pointer rests over the `Agents at work` lane at 14:20, while three sessions worked
- **THEN** the line reads like `14:20 · 3 agents at work: ecad11 Chart of the…, 230043 Notes…, 66f6be Fresh…`, the sessions in the lanes' order

### Requirement: The selected session's day
Selecting a session whose last activity is on another day than the lanes show SHALL show that day in Day: the local date of its last activity five hours earlier. Sessions opens a session on the day of its row instead (see desk-sessions). When that day has no lane of the session, the lanes stay on that day with no lane highlighted, and the details pane shows the session's inspector all the same.

#### Scenario: A session of yesterday
- **WHEN** the user selects a session last active yesterday at 16:00
- **THEN** the lanes show yesterday, with its lane highlighted

#### Scenario: A session without a lane
- **WHEN** the user opens, from Sessions, a session that a program ran today
- **THEN** the lanes show today with no lane highlighted, the details pane shows that session's inspector, and the lanes' header adds `no lane: nobody typed in <id> this day`

### Requirement: Lanes and the filter
While a filter is set, the lanes SHALL show only the sessions it lists, and the lanes' header counts them as `<n> of <m> in the lanes`; when it lists none of the day's, a line under the `You` lane says so. While a filter is being typed, the lanes stay on their day, wherever the selection goes. The filter field draws a thin caret after the text being typed, and a clear button while a filter is set.

#### Scenario: A filter that lists nothing of the day
- **WHEN** the user types the filter `zzzz` and no session matches
- **THEN** the lanes show only `You` and the line `no session of this day matches “zzzz”`, and the lanes' header reads `0 of 6 in the lanes` on a day of six

#### Scenario: Clearing the filter
- **WHEN** the user clicks the clear button of the filter
- **THEN** the filter is empty, the Sessions table holds every session again, and every lane of the day is back

### Requirement: Session list
A row of the Sessions table SHALL show its cells in the columns of desk-sessions' Table columns: the title or, while the title is not known, the project, and the cost with tabular figures (`…` while it is being costed). The selected row is a rounded row inset in the table, in the theme's selection color with the text in the ink. Day headers show only in time order. A filter that lists no session says so in the table.

#### Scenario: A row
- **WHEN** the table shows a session titled `Find why the order export is slow` that cost $1.18
- **THEN** its row reads its time, the dot, its project, the title and `$1.18`

#### Scenario: The selected row
- **WHEN** a session is selected
- **THEN** its row is rounded and inset 6 points from the table's edges, in the selection color, its title, cost and dot in the colors of any other row

#### Scenario: A filter that lists nothing
- **WHEN** the user types the filter `zzzz` and no session matches
- **THEN** the table reads `No session matches “zzzz”` and, under it, `esc` as a keycap with `clears the filter`

### Requirement: Next-request countdown
The `Next` cell of a row SHALL show, while the session's cache is warm, the minutes and seconds it stays warm in green, counting down every second; once it is cold, the next request's cost in red; and `none` for a session without requests.

#### Scenario: A warm session
- **WHEN** a listed session's cache stays warm for 24 minutes and 47 seconds more
- **THEN** its cell reads `24:47` in green, and a second later `24:46`

#### Scenario: A cache goes cold
- **WHEN** the cache of a listed session goes cold while the window is open
- **THEN** its cell switches to the cold cost in red at that moment, and its row stays where it is

### Requirement: Browser behavior in the window
The window SHALL list, cost, follow, filter, sort, re-sort, export and copy as session-browser specifies in Background work, Next-request cell, Live updates, Rows in place, Filter, Sort, Re-sort key, Profiles from the browser and Handoff key, with the same keys and texts. The window's filter also matches the session's title, in Day and in Sessions.

#### Scenario: A new session
- **WHEN** an agent starts a new session while the window is open
- **THEN** it joins the Sessions table where the current order puts it, its lane joins the current day, and the selection stays on its session

#### Scenario: A title in the filter
- **WHEN** the user types the filter `python` and a session's title is `Python chat bugs`
- **THEN** that session stays in the Sessions table and its lane stays in Day, while `tokprof ui` keeps matching agent, project and id only

### Requirement: Keys of the window
In Day, while the lanes have the keyboard, ← and → (and `h`, `l`, `[`, `]`) SHALL step the lanes' day, and `t` shows the day of the latest action, as Today does. ⏎ shows the selected session's inspector full size under the bar until esc is pressed, and esc deselects the session. ⌘0 shows Sessions from every screen. `f`, `o` and `c` act on the selected session as in session-browser; `s` and `r` sort the Sessions table.

#### Scenario: Stepping days
- **WHEN** the user presses ←
- **THEN** the lanes show the day before; the selected session stays selected when it has a lane that day, and otherwise none is and the details pane shows the day

#### Scenario: Today by key
- **WHEN** the lanes show a day of last week and the user presses `t`
- **THEN** the lanes show the day of the latest action, as a click on Today does

#### Scenario: Esc in the lanes
- **WHEN** the lanes have the keyboard, a session is selected, and the user presses esc
- **THEN** no session is selected, the details pane shows the day summary, and the window stays open

### Requirement: Keyboard focus
One pane SHALL have the keyboard at a time. In Day, the lanes and the details pane take it, the lanes to start with. In Sessions, the table and its inspector take it, the table to start with. The pane with it rings its selection and draws its title in the accent; the other keeps its selection without the ring. Tab and Shift+Tab move the keyboard to the other pane, a click in one gives it the keyboard, and esc gives it back to the lanes or the table.

#### Scenario: Tab
- **WHEN** the lanes have the keyboard with a session selected and the user presses Tab
- **THEN** the inspector has it, and ↓ moves in the inspector instead of the lanes

#### Scenario: Esc
- **WHEN** the inspector has the keyboard and the user presses esc
- **THEN** the lanes have it again, the session stays selected, and the window stays open

#### Scenario: What the ring marks
- **WHEN** the lanes, the table, the day summary or the inspector has the keyboard
- **THEN** the ring is around the selected lane, the selected row, the row under the day summary's cursor, or the row under the inspector's cursor (the chosen tab on `Summary`; on `Flame`, the frame under its cursor), and nowhere else

#### Scenario: A filter being typed
- **WHEN** the user is typing a filter
- **THEN** the filter field has the ring, and the table's row and the selected lane do not

### Requirement: Keys in the lanes
While the lanes have the keyboard, ↑ and ↓ (and `k`, `j`) SHALL select the session of the lane above or below the selected session's, and Home and End the first and the last lane's, as a click on the lane does. With no session selected, ↓ selects the first lane's session and ↑ the last's. ← and → step the day as in Keys of the window, and esc deselects.

#### Scenario: The lane below
- **WHEN** the lanes have the keyboard, the second lane's session is selected, and the user presses ↓
- **THEN** the third lane's session is selected and the details pane shows it, on the same tab

#### Scenario: The first lane
- **WHEN** the lanes have the keyboard, no session is selected, and the user presses ↓
- **THEN** the first lane's session is selected and the details pane shows its inspector

### Requirement: Handoff in the inspector
The inspector SHALL have a `Copy handoff line` button that does what session-browser's Handoff key does, in Day and in the Sessions inspector. A double click on a lane shows the inspector full size, as ⏎ does; a double click on a row of Sessions opens it in Day (see desk-sessions).

#### Scenario: Copying the handoff
- **WHEN** the user clicks `Copy handoff line`
- **THEN** the clipboard holds the line that starts a new session from the selected one, and the bar says so

#### Scenario: A double click on a lane
- **WHEN** the user double-clicks a session's lane
- **THEN** that session is selected and its inspector shows full size under the bar, until esc

### Requirement: Quitting the window
⌘Q, ⌘W, Ctrl+C and closing the window SHALL quit `tokprof-desk`. In Day and in Sessions, `q` quits too; in Reports, `q` does nothing. esc never quits: it closes the full-size inspector, deselects the session, closes the Sessions inspector or clears the filter, and in Reports shows Day. While a filter is being typed, `q` and esc act as session-browser's Quit says, and ⌘Q still quits.

#### Scenario: Closing the window
- **WHEN** the user closes the window
- **THEN** the program exits

#### Scenario: Typing q into a filter
- **WHEN** `q` is pressed while a filter is being typed
- **THEN** `q` is added to the filter

#### Scenario: q in Reports
- **WHEN** `q` is pressed while Reports is shown
- **THEN** the window stays open

#### Scenario: Esc with nothing to step back from
- **WHEN** Day shows no selected session, no filter is set, and the user presses esc
- **THEN** the window stays open and nothing changes

### Requirement: Mouse
In Sessions, a click on a row SHALL select its session and show its `Summary` from the top in the inspector. In Day, a click on a lane selects its session (see Details pane). A click on the filter starts typing a filter. The wheel and the trackpad scroll the pane under the pointer without moving the selection.

#### Scenario: A click
- **WHEN** the user clicks the fifth row of the Sessions table
- **THEN** its session is selected and the inspector shows its Summary from the top

#### Scenario: Scrolling the list
- **WHEN** the user scrolls the Sessions table with the wheel
- **THEN** the rows move and the selected session stays selected

### Requirement: One frame without a window
`tokprof-desk --snap <png>` SHALL render one frame to a PNG file and exit, without a window and without following the logs. It waits up to 20 s for the day and for the X-ray of the selected session when one is selected, up to 120 s for the day store of the day summary, then 0.5 s more for costs, as session-browser's One frame as text does.

#### Scenario: Checking the layout
- **WHEN** the user runs `tokprof-desk --snap out.png --tree out.json`
- **THEN** `out.png` shows the bar, the lanes' header, the lanes and the day summary, and `out.json` holds the text of every lane, card and row of the summary

### Requirement: Frame options
For a frame without a window, `--size <W>x<H>` SHALL set the frame in points (default 1400x900), `--theme light` or `--theme dark` its appearance, `--colors <theme id>` its theme, `--day <date>` the day in the lanes, `--tab <name>` that tab of the newest listed session, selected on its day, and `--tree <json>` write the frame's elements with their text and places.

#### Scenario: The day
- **WHEN** the user runs `tokprof-desk --snap day.png --day 2026-11-03`
- **THEN** the lanes show 2026-11-03

#### Scenario: A bad size
- **WHEN** the user runs `tokprof-desk --snap out.png --size 1400`
- **THEN** the command fails with `--size takes WIDTHxHEIGHT`

#### Scenario: Another theme
- **WHEN** the user runs `tokprof-desk --snap out.png --colors ink-meridian`
- **THEN** `out.png` is drawn in Ink Meridian, a dark theme, whatever `--theme` says

#### Scenario: An unknown theme
- **WHEN** the user runs `tokprof-desk --snap out.png --colors nosuch`
- **THEN** the command fails with `--colors takes one of:` and the ids of the themes

#### Scenario: A tab
- **WHEN** the user runs `tokprof-desk --snap out.png --tab cache`
- **THEN** the newest listed session is selected on its day, and the details pane shows its `Cache` tab

### Requirement: Palette entries
The palette SHALL offer four groups. `Actions`: what the window does from a key, each with its key. `Tabs`: the inspector's tabs. `Sessions`: every listed session with its title, project and cost. `Days`: `Today`, the day of the latest action, and the days of the listed sessions, newest first, named as Sessions' day headers. A session opens in Day (see desk-sessions), or is selected in Sessions while it is shown; a tab shows; a day shows in Day.

#### Scenario: Picking a session
- **WHEN** the user picks a session in the palette in Day while another is selected
- **THEN** the palette closes, Day shows the session's day with that session selected, and the details pane shows its `Summary` from the top

#### Scenario: Picking a session from the full-size inspector
- **WHEN** the inspector is full size and the user picks a session
- **THEN** the full-size inspector shows that session's `Summary` from the top

#### Scenario: Picking a session in Sessions
- **WHEN** Sessions is shown and the user picks a session
- **THEN** its row is selected and scrolled to, and the Sessions inspector shows its Summary

#### Scenario: Picking a day
- **WHEN** the user picks `Yesterday`
- **THEN** Day shows the day before today; the selection stays when the session has a lane that day

#### Scenario: Picking an action
- **WHEN** the user picks `Sort by Cost`
- **THEN** Sessions is shown, sorted by cost, as a click on the `Cost` header sorts it

#### Scenario: Actions that do nothing now
- **WHEN** no filter is set
- **THEN** `Clear the filter` is not offered; and `Full-size inspector` is offered only while a session is selected in Day and the inspector is not full size, `Back to the lanes` only while it is

### Requirement: Key help
`?` SHALL open the key help when no filter is being typed and no overlay is open: a rounded card over the window dimmed and blurred, listing every key of the window grouped under `Anywhere`, `Lanes`, `The day`, `Palette`, `Settings`, `Sessions`, `Inspector`, `Flame`, `Reports` and `Calendar`, each key drawn as a keycap with what it does. esc or `?` close it, and no other key reaches the window behind it, but ⌘Q quits. The palette's `Key help` opens it too.

#### Scenario: Opening the key help
- **WHEN** the lanes have the keyboard and the user presses `?`
- **THEN** the key help opens, and it names ⌘K, ⌘,, ⌘0, `/`, `s`, `r`, `f`, `o`, `c` and `q`

#### Scenario: A filter being typed
- **WHEN** a filter is being typed and the user presses `?`
- **THEN** `?` is added to the filter

#### Scenario: Closing the key help
- **WHEN** the key help is open and the user presses `?`
- **THEN** it closes, and the same session, tab and day are shown

### Requirement: Keys in the flame graph
While the inspector has the keyboard on `Flame`, ← and → (and `h`, `l`) SHALL move a cursor to the drawn sibling before or after, ↓ (`j`) to the first drawn child, ↑ (`k`) to the parent, up to the shown root. ⏎ zooms into the cursor's frame, and esc and Backspace zoom out one level; esc gives the keyboard back to the lanes only when the graph is not zoomed. The cursor's frame is ringed in the accent.

#### Scenario: Down and right
- **WHEN** the cursor is on the root and the user presses ↓ and then →
- **THEN** the cursor is on the second child of the root, ringed, and the line under the graph names it

#### Scenario: Enter and esc
- **WHEN** the cursor is on a turn and the user presses ⏎ and then esc
- **THEN** the turn is shown as the root, and then its parent is

### Requirement: Reports view
The bar SHALL hold a `Day | Reports | Calendar` switch, and right of it, apart from it, the `Sessions` button. ⌘1 shows Day, ⌘2 Reports, ⌘3 Calendar and ⌘0 Sessions (see desk-sessions). Reports replaces what Day shows under the bar with the report of a period in one scrolling pane, the cards of the Report cards requirement. The window starts in Day.

#### Scenario: Switching
- **WHEN** the user clicks `Reports` in the bar, or presses ⌘2
- **THEN** the lanes and the details pane give way to the report, and `Reports` is marked in the switch

#### Scenario: Back to the day
- **WHEN** Reports is shown and the user clicks `Day`, presses ⌘1 or presses esc
- **THEN** the window shows Day as it was: its day, its selection and its tab

#### Scenario: From the palette and the key help
- **WHEN** the user picks `Reports: the last 7 days` in the command palette
- **THEN** Reports shows the last 7 days; the palette also offers `Reports`, `Day`, `Sessions` and the other periods, and the key help lists ⌘0, ⌘1, ⌘2 and the keys of Reports

### Requirement: Spend by project and by origin
The project card SHALL show the 10 projects with the most spend, and the origin card the 10 origins of the summary's origin table (`prompt`, `agent:…`, `skill:…`), each a bar as long as its share of the first with its dollars and its share of all; the rest as one row `<n> more`. A click on a project, not on `more`, shows Sessions with that project picked (see desk-sessions).

#### Scenario: A project
- **WHEN** the user clicks the bar of `~/src/zebrafish`
- **THEN** the window shows Sessions, the project picker reads `~/src/zebrafish`, and the table holds only its sessions

### Requirement: Keys in Reports
In Reports, Tab and Shift+Tab SHALL move the keyboard through the period, by day, by project, by origin, by model, cold returns and largest context, ringed in the accent and scrolled into view. ← and → pick the period or move the day cursor; ↑ ↓ (`k`, `j`), Home and End move a row cursor; ⏎ opens the cursor's day or project as a click does. ⌘Q, ⌘W, ⌘,, ⌘K, `?`, Ctrl+C, ⌘0, ⌘1, ⌘2 and ⌘3 act as in Day; esc shows Day; the other keys do nothing.

#### Scenario: A day by keys
- **WHEN** by day has the keyboard, and the user presses ← twice and ⏎
- **THEN** the window shows Day with the lanes of the date two days before the last of the period

#### Scenario: The period by keys
- **WHEN** the period has the keyboard and the user presses ←
- **THEN** `7 days` is picked

#### Scenario: Sessions from Reports
- **WHEN** Reports is shown and the user presses ⌘0
- **THEN** the window shows Sessions

### Requirement: Picks in the menu bar menu
A session's item SHALL bring the window forward and open the session in Day (see desk-sessions, Opening a session in Day), with its `Summary` from the top; an open settings panel, palette or key help closes. `Open tokprof` brings the window forward as it is. `Settings…` brings it forward with the settings panel open. A session no longer listed brings the window forward as it is.

#### Scenario: A session from Reports
- **WHEN** the window shows Reports, hidden behind another app, and the user picks a session in the menu
- **THEN** the window comes forward in Day on that session's day, with that session selected

### Requirement: Waiting in the window
While the `Waiting order` setting is `On`, the Sessions toolbar SHALL offer `Waiting first` after the quick filters, sorting the table as session-browser's waiting order. In it, `Waiting for you` heads the waiting sessions and `Others` the rest, as the day headers head rows in time order, and a waiting row shows its wait, like `16m`, in its `Last` cell. The palette offers `Sort by Waiting` then. With no session waiting, the table reads `Nobody waits for you` above `Others`.

#### Scenario: The setting off
- **WHEN** `Waiting order` is `Off`
- **THEN** the toolbar has no `Waiting first`, `s` cycles time, cost and next, no row shows a wait, and the palette has no `Sort by Waiting`

#### Scenario: Three sessions wait
- **WHEN** `Waiting order` is `On` and the user clicks `Waiting first` while three sessions wait, for 40m, 16m and 2m
- **THEN** the table starts with `Waiting for you` and rows reading `40m`, `16m` and `2m` in place of their times, then `Others` and the rest newest first

#### Scenario: From the palette
- **WHEN** the user picks `Sort by Waiting` in the palette
- **THEN** Sessions is shown, sorted by waiting, as `Waiting first` sorts it

#### Scenario: Nobody waits
- **WHEN** the user clicks `Waiting first` and no session waits
- **THEN** the table reads `Nobody waits for you`, then `Others` and every session newest first

#### Scenario: A frame in waiting order
- **WHEN** the user runs `tokprof-desk --snap out.png --scene waiting`
- **THEN** `out.png` shows the Sessions table in waiting order, as with the setting `On`, without reading the saved settings

### Requirement: Waiting order setting
The settings panel SHALL offer `Waiting order`, `Off` or `On`, last, after `Menu bar countdown`, saved in the settings file as `waiting` (`off` or `on`). The default is `Off`; a value it does not know is `Off`. Picking `Off` while the Sessions table is sorted by waiting sorts it by time. Tab reaches it after `Menu bar countdown`, and ← → pick it.

#### Scenario: Turning it on
- **WHEN** the user picks `On` in `Waiting order`
- **THEN** the Sessions toolbar offers `Waiting first` at once, and the settings file holds `"waiting": "on"`

#### Scenario: Turning it off in waiting order
- **WHEN** the Sessions table is sorted by waiting and the user picks `Off`
- **THEN** the table is sorted by time and `Waiting first` leaves the toolbar

#### Scenario: No settings yet
- **WHEN** the window starts and the settings file has no `waiting`
- **THEN** the Sessions toolbar has no `Waiting first`

### Requirement: Calendar view
⌘3 or `Calendar` in the bar's switch SHALL show Calendar: in one scrolling pane under the bar, the metric as pills, the calendar of activity-calendar drawn large, its legend and its numbers, and its punchcard, in rounded cards as in Reports. It counts what `tokprof calendar --metric <metric>` counts with the window's `--agent`, `--project`, `--session`, `--since` and `--until`. esc, ⌘1 or `Day` show Day as it was.

#### Scenario: Switching
- **WHEN** the user presses ⌘3
- **THEN** the lanes and the details pane give way to the calendar, and `Calendar` is marked in the switch

#### Scenario: The same numbers as the CLI
- **WHEN** Calendar shows spend and the user runs `tokprof calendar` with the same filters at the same time
- **THEN** every cell's level, the cuts, the total, active days, streaks, the busiest day and weekday, and the punchcard's levels are the same

#### Scenario: From the palette and the key help
- **WHEN** the user picks `Calendar: messages` in the command palette
- **THEN** Calendar shows messages; the palette also offers `Calendar` and each other metric, and the key help lists ⌘3 and the keys of Calendar

### Requirement: Keys in Calendar
In Calendar, a cursor SHALL ring one day, the last day to start with. ↑ and ↓ (`k`, `j`) move it a day back and forward, ← and → (`h`, `l`) a week; Home and End go to the first and the last day; it never leaves the days shown. The day under the cursor shows its tooltip, and ⏎ opens it as a click does. `m` picks the next metric and ⇧M the one before. ⌘Q, ⌘W, ⌘,, ⌘K, `?`, Ctrl+C, ⌘0, ⌘1, ⌘2 and ⌘3 act as in Day; esc shows Day; the other keys do nothing.

#### Scenario: A day by keys
- **WHEN** the cursor is on the last day and the user presses ← and ⏎
- **THEN** the window shows Day with the lanes of the date a week before the last day

#### Scenario: The metric by keys
- **WHEN** Calendar shows spend and the user presses `m` twice
- **THEN** it shows the time at the keyboard

#### Scenario: Sessions from Calendar
- **WHEN** Calendar is shown and the user presses ⌘0
- **THEN** the window shows Sessions

### Requirement: Agents at work lane
Under the `You` lane, on a day when two or more sessions worked at once, the lanes SHALL have an `Agents at work` lane, 28 points high: a stepped area in the work color, as high at each moment as work-rhythm's number of agents at work, its full height the peak or 4, whichever is more. Its label says `Agents at work` and `peak <n> at <HH:MM>`.

#### Scenario: A peak of seven
- **WHEN** seven sessions worked at once at 14:20, and never more
- **THEN** the area reaches its full height at 14:20, and the label reads `peak 7 at 14:20`

#### Scenario: One session at a time
- **WHEN** at most one session worked at any moment of the day
- **THEN** the day has no `Agents at work` lane, and the day summary's Agents card still reads the day's `agents` line

#### Scenario: A few agents
- **WHEN** at most two agents worked at once
- **THEN** the area reaches half of the lane's height, so that days compare when stepping with ← and →

#### Scenario: Not a session
- **WHEN** the user clicks the `Agents at work` lane
- **THEN** the selection stays where it was

### Requirement: Room for the lanes
The lanes, their header and what is under them SHALL take 45% of the window's height under the bar, or more, up to 55%, when that shows fewer session lanes than 8 in a window 900 points high and 3 in one 640 high, and in between as many as a straight line between them gives. The border under them can move this (see Border between lanes and details). Lanes are 24 points high.

#### Scenario: A busy day
- **WHEN** the window is 1400 by 900 and the day of 2026-11-01 has 40 sessions
- **THEN** 8 session lanes show whole under `You` and `Agents at work`, and the line under them says how many more scroll

#### Scenario: The smallest window
- **WHEN** the window is 960 by 640 on the same day
- **THEN** 3 session lanes show whole
