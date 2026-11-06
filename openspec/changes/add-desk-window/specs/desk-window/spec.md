# Spec Delta

## Purpose

`tokprof-desk` shows the session browser in a native window laid out as a profiler: the day's lanes on top, the sessions below them on the left, and an inspector of the selected session on the right, drawn as native controls in the system font. It shows the same sessions, costs, X-rays and days as `tokprof ui`, and adds the mouse.

## ADDED Requirements

### Requirement: Opening the window
`tokprof-desk` SHALL open one window titled `tokprof`, 1400 by 900 points the first time and no smaller than 960 by 640, listing every session that `tokprof ui` lists with the same `--project`, `--session`, `--since` and `--until` filters. It starts with the newest session selected and the day of the latest action in the lanes. The window follows the system's light or dark appearance, also when it changes while it is open.

#### Scenario: Default start
- **WHEN** the user runs `tokprof-desk`
- **THEN** a window opens with the lanes of the latest day on top, the list of sessions below them, and the inspector of the newest session

#### Scenario: Filters
- **WHEN** the user runs `tokprof-desk --project json-profiler --since 7d`
- **THEN** the list holds only the sessions that `tokprof ui --project json-profiler --since 7d` lists

#### Scenario: No logs
- **WHEN** no session logs are found
- **THEN** no window opens, and the command fails with `no session logs found`

#### Scenario: The system turns dark
- **WHEN** the system switches from light to dark while the window is open
- **THEN** the window switches to its dark colors without a restart

### Requirement: Title bar
The window SHALL draw its own bar along the top, 46 points high, with the window's close, minimize and zoom buttons at its left and nothing of the window's own under them. Dragging the bar where it has no control moves the window, and a double click on it zooms the window as the system's setting says. The latest status message shows at the bar's right for 6 seconds.

#### Scenario: The buttons
- **WHEN** the window opens
- **THEN** the close, minimize and zoom buttons sit at the left of the bar, and the day's name starts right of them

#### Scenario: Moving the window
- **WHEN** the user drags the bar where it has no control
- **THEN** the window moves

#### Scenario: Writing a profile
- **WHEN** `o` is pressed
- **THEN** the bar reads `wrote <n> frames to ~/tokprof/<id>.html` and the page opens, as in `tokprof ui`

### Requirement: Day controls
The bar SHALL hold the shown day's name with buttons to the day before and after, a Today button, the day's summary (when it started, its sittings and its sessions), the lanes' zoom (`Fit` or `24 h`), the filter, and a `Live` mark while the window follows the logs.

#### Scenario: Other days
- **WHEN** the user clicks the button before the day's name
- **THEN** the lanes show the day before, and Today goes back to the day of the latest action

#### Scenario: Never past the latest day
- **WHEN** the lanes show the day of the latest action and the user clicks the button after its name
- **THEN** the lanes stay on that day

### Requirement: Window layout
The window SHALL show, under the bar, the lanes of the shown day across its whole width, at most 45% of its height, and under them the list of sessions in a pane 380 points wide and the inspector of the selected session in the rest. Every text is in the system font; numbers that line up in columns use tabular figures.

#### Scenario: A narrow window
- **WHEN** the window is 960 points wide
- **THEN** the list keeps its 380 points and the inspector takes the remaining 580

#### Scenario: Many lanes
- **WHEN** the day has more lanes than fit in 45% of the window
- **THEN** the lanes scroll under an hours row that stays on top, and the list and the inspector keep their place

### Requirement: Day lanes
The lanes SHALL show the day as work-rhythm's Day strip with shapes, at the times of the log: a `You` lane with the sittings, then one lane per session with actions that day, most actions first, labeled with its project and title. Work and waits for the user's answer are bars from their start to their end, at least 1 point wide, and the user's messages, the agent's questions and the user's stops are marks at their moment, in work-rhythm's colors. The selected session's lane is highlighted.

#### Scenario: A short piece of work
- **WHEN** the agent worked from 14:32:10 to 14:32:50
- **THEN** its lane has a green bar 40 seconds wide at 14:32

#### Scenario: A click on a lane
- **WHEN** the user clicks the lane of session `ecad11`
- **THEN** `ecad11` is selected in the list and the inspector shows it

### Requirement: Cache tails
After each session's last request, its lane SHALL show a tail in the warm color that fades out over the time its cache stays warm, and a mark where the cache went cold.

#### Scenario: A warm cache
- **WHEN** a session's last request was at 13:58, its cache lives an hour, and it is 14:33
- **THEN** its lane has a tail from 13:58 to 14:58 that crosses the now line

#### Scenario: A cold cache
- **WHEN** a session's last request was at 11:09 and its cache lived an hour
- **THEN** its tail ends at 12:09 with a mark where the cache went cold

### Requirement: Now and zoom
On the current day, a red line SHALL mark now in the lanes, with the time after it shaded. `Fit` shows the whole hours from the first action of the day to the end of the last tail or now, whichever is later, and `24 h` the 24 hours of work-rhythm's axis. The hours row has a mark at every hour, and at every half hour that has room.

#### Scenario: Fit
- **WHEN** the first action of the day was at 08:50, the last tail ends at 14:58, and `Fit` is chosen
- **THEN** the lanes show 08:00 to 15:00

### Requirement: What the lanes say
Under the lanes SHALL be the legend, and the `sittings`, `answers` and `cache` lines of the day as `tokprof day` prints them. A line under them names what is under the pointer: the time, the lane and what it shows there.

#### Scenario: The time under the pointer
- **WHEN** the pointer rests over a lane at 14:32
- **THEN** the line reads like `14:32 · ecad11 Инфографика прерываний · agent working`, with `agent working`, `waiting for your answer`, `your message`, `its question`, `you stopped it`, `you were there`, `cache warm` or nothing

### Requirement: The selected session's day
Selecting a session whose last activity is on another day than the lanes show SHALL show that day in the lanes, as the local date of its last activity five hours earlier. When that day has no lane of the session, the lanes stay on it.

#### Scenario: A session of yesterday
- **WHEN** the user selects a session last active yesterday at 16:00
- **THEN** the lanes show yesterday, with its lane highlighted

### Requirement: Session list
A row of the list SHALL show the last activity (`HH:MM`, after a separator row naming the day), a dot in the agent's color, the session's title or, while the title is not known, its project, and its cost with tabular figures (`…` while it is being costed). The selected row is filled with the accent color. The separators show only in time order.

#### Scenario: A row
- **WHEN** the list shows a session titled `Find why the order export is slow` that cost $1.18
- **THEN** its row reads its time, the dot, the title and `$1.18`

### Requirement: Next-request countdown
The last cell of a row SHALL show, while the session's cache is warm, the minutes and seconds it stays warm in green, counting down every second; once it is cold, the next request's cost in red; and `none` for a session without requests.

#### Scenario: A warm session
- **WHEN** a listed session's cache stays warm for 24 minutes and 47 seconds more
- **THEN** its cell reads `24:47` in green, and a second later `24:46`

#### Scenario: A cache goes cold
- **WHEN** the cache of a listed session goes cold while the window is open
- **THEN** its cell switches to the cold cost in red at that moment, and its row stays where it is

### Requirement: Browser behavior in the window
The window SHALL list, cost, follow, filter, sort, re-sort, export and copy as session-browser specifies in Background work, Next-request cell, Live updates, Rows in place, Filter, Sort, Re-sort key, Profiles from the browser and Handoff key, with the same keys and texts.

#### Scenario: A new session
- **WHEN** an agent starts a new session while the window is open
- **THEN** it joins the list where the current order puts it, its lane joins the current day, and the selection stays on its session

### Requirement: Keys of the window
While the list has the keyboard, ← and → (and `h`, `l`, `[`, `]`) SHALL step the lanes' day instead of opening and closing the X-ray, `d` shows the selected session's day, and ⏎ shows the inspector full size under the bar until esc is pressed; the other keys of session-browser keep their meaning.

#### Scenario: Stepping days
- **WHEN** the user presses ←
- **THEN** the lanes show the day before, and the list keeps its selection

### Requirement: Keyboard focus
One of the lanes, the list and the inspector SHALL have the keyboard at a time, the list to start with, marked by an accent line along its top. Tab moves the keyboard to the next of them in the order lanes, list, inspector, and Shift+Tab back; a click in one gives it the keyboard, and esc gives it back to the list. The keys of the list are those of Keys of the window; the others' follow.

#### Scenario: Tab
- **WHEN** the list has the keyboard and the user presses Tab
- **THEN** the inspector has it, and ↓ moves in the inspector instead of the list

#### Scenario: Esc
- **WHEN** the inspector has the keyboard and the user presses esc
- **THEN** the list has it again, and the window stays open

### Requirement: Keys in the lanes
While the lanes have the keyboard, ↑ and ↓ (and `k`, `j`) SHALL select the session of the lane above or below the selected session's, Home and End the first and the last lane's, as a click on the lane does; ← and → step the day as in the list.

#### Scenario: The lane below
- **WHEN** the lanes have the keyboard, the second lane's session is selected, and the user presses ↓
- **THEN** the third lane's session is selected in the list and shown in the inspector

### Requirement: Keys in the inspector
While the inspector has the keyboard, ← and → (and `h`, `l`) SHALL show the tab before or after, and the digits 1 to 9 the tab of that place. In a tab with rows (the requests, the bars of spend, work and waits, the prompts, the tools, the context's tables and the misses), ↑ and ↓ (and `k`, `j`) move a cursor from row to row, PgUp and PgDn by ten rows, Home and End to the first and the last, and the inspector scrolls to keep it on the screen. In `Summary` they scroll it.

#### Scenario: Through the prompts
- **WHEN** the inspector has the keyboard on `Prompts` and the user presses ↓ three times
- **THEN** the fourth prompt's row is marked, and it is on the screen

#### Scenario: Another tab
- **WHEN** the inspector has the keyboard and the user presses 7
- **THEN** it shows `Tools`, with the cursor on its first row

### Requirement: Inspector
The inspector SHALL show the selected session's X-ray as native controls under tabs: `Summary`, `Requests`, `Spend`, `Work`, `Waits`, `Prompts`, `Tools`, `Context` and `Cache`. Every number in it is the one `tokprof scan` prints for the session at the same moment. Texts can be selected with the mouse, and ⌘C copies the selection.

#### Scenario: The same numbers
- **WHEN** the inspector shows a session
- **THEN** its cost, context size, next-request cost, spend by component, prompts and tool rows are those of `tokprof scan <id>` at the same moment

#### Scenario: A session without requests
- **WHEN** the selected session has no requests
- **THEN** the inspector says `no requests`, with its title, project and id

### Requirement: Summary tab
`Summary` SHALL show the title, agent, project, id, models, turns and time; a track of the session's requests on a time axis from its first request to now, each a bar as tall as its cost, with the time the cache stayed warm, the time it has been cold, and the next request at now; the next request's cost and the cache's state; the ways to continue that session-xray's advice prices, cheapest marked; the spend by component; and the rest of the advice as tips.

#### Scenario: The cheapest way
- **WHEN** the cache is cold and a new session from a handoff costs $0.52 against $0.84 to resume and $1.01 to compact
- **THEN** the handoff is marked as the cheapest

### Requirement: Other tabs
The tabs after `Summary` SHALL show session-xray's blocks of those names: the requests, the spend by component and by chain, work by origin and by activity, waits and slow requests, the prompts, the tool table, what the context holds and what cost the most to keep in it, and the cache misses. A click on a tab shows it.

#### Scenario: Prompts
- **WHEN** the user clicks `Prompts`
- **THEN** the inspector lists each typed message with its cost, its requests and its two costliest activities, as the X-ray's prompts block does

### Requirement: Handoff in the inspector
The inspector SHALL have a `Copy handoff line` button that does what session-browser's Handoff key does, and a double click on a list row shows the inspector full size, as ⏎ does.

#### Scenario: Copying the handoff
- **WHEN** the user clicks `Copy handoff line`
- **THEN** the clipboard holds the line that starts a new session from the selected one, and the bar says so

### Requirement: Quitting the window
⌘Q, ⌘W, closing the window and `q` SHALL quit `tokprof-desk`; esc quits when the inspector is not full size and no filter is set. While a filter is being typed, `q` and esc act as session-browser's Quit says, and ⌘Q still quits.

#### Scenario: Closing the window
- **WHEN** the user closes the window
- **THEN** the program exits

#### Scenario: Typing q into a filter
- **WHEN** `q` is pressed while a filter is being typed
- **THEN** `q` is added to the filter

### Requirement: Mouse
A click on a list row SHALL select its session and show its `Summary` from the top, and a click on the filter starts typing a filter. The wheel and the trackpad scroll the pane under the pointer without moving the selection.

#### Scenario: A click
- **WHEN** the user clicks the fifth row
- **THEN** its session is selected and the inspector shows its Summary from the top

#### Scenario: Scrolling the list
- **WHEN** the user scrolls the list with the wheel
- **THEN** the rows move and the selected session stays selected

### Requirement: One frame without a window
`tokprof-desk --snap <png>` SHALL render one frame to a PNG file and exit, without a window and without following the logs. It waits up to 20 s for the X-ray of the newest session and for the day, then 0.5 s more for costs, as session-browser's One frame as text does.

#### Scenario: Checking the layout
- **WHEN** the user runs `tokprof-desk --snap out.png --tree out.json`
- **THEN** `out.png` shows the bar, the lanes, the list and the inspector of the newest session, and `out.json` holds the text of every row and of the inspector

### Requirement: Frame options
For a frame without a window, `--size <W>x<H>` SHALL set the frame in points (default 1400x900), `--theme light` or `--theme dark` its colors, `--day <date>` the day in the lanes, `--tab <name>` the inspector's tab, and `--tree <json>` write the frame's elements with their text and places.

#### Scenario: The day
- **WHEN** the user runs `tokprof-desk --snap day.png --day 2026-11-03`
- **THEN** the lanes show 2026-11-03

#### Scenario: A bad size
- **WHEN** the user runs `tokprof-desk --snap out.png --size 1400`
- **THEN** the command fails with `--size takes WIDTHxHEIGHT`
