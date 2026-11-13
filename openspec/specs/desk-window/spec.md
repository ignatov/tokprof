# desk-window Specification

## Purpose
`tokprof-desk` shows the session browser in a native window laid out as a profiler: the day's lanes on top, the sessions below them on the left, and an inspector of the selected session on the right, drawn as native controls in the system font. It shows the same sessions, costs, X-rays and days as `tokprof ui`, and adds the mouse.

## Requirements

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
In Day, the bar SHALL hold the shown day's name with buttons to the day before and after, a Today button, the filter, and a `Live` mark while the window follows the logs. The day's caption and the zoom are in the lanes' header. In Sessions, the bar holds `All sessions` with the date of the first log and the count of listed sessions instead. In Week, Month, Quarter and Year it holds the period's name, its buttons, Today and the period's extent instead, and the filter.

#### Scenario: Other days
- **WHEN** the user clicks the button before the day's name
- **THEN** the lanes show the day before, and Today goes back to the day of the latest action

#### Scenario: Never past the latest day
- **WHEN** the lanes show the day of the latest action and the user clicks the button after its name
- **THEN** the lanes stay on that day

#### Scenario: In Reports
- **WHEN** Month, which took Reports' place, is shown on 12 November 2026
- **THEN** the bar holds the switch, `Sessions`, `‹ November 2026 ›`, Today, `1–12 Nov so far · vs 1–12 Oct`, the filter, the status and `Live`, and no day controls

#### Scenario: In Calendar
- **WHEN** Year, which took Calendar's place, is shown on 12 Nov 2026
- **THEN** the bar holds the switch, `Sessions`, `‹ 2026 ›`, Today, `logs since 6 Aug · 99 days`, the filter, the status and `Live` (see The year in the bar)

#### Scenario: In Sessions
- **WHEN** Sessions is shown on logs that start on 3 Mar with 412 listed sessions
- **THEN** the bar reads `All sessions` and `since 3 Mar · 412 sessions`, then Today, the filter and `Live`

#### Scenario: In Week
- **WHEN** Week is shown
- **THEN** the bar holds the switch, the `Sessions` button, the week's name with its buttons, `Today`, the week's summary, the filter, the status and `Live`, and no day controls and no zoom

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

### Requirement: Keys in the inspector
While the inspector has the keyboard, ← and → (and `h`, `l`) SHALL show the tab before or after outside `Flame` and `Requests`, and Shift+← and Shift+→ in every tab; the digits 1 to 9 show the tab of that place, and 0 `Flame`. In a tab with rows (bars, tables, prompts, misses), ↑ and ↓ (and `k`, `j`) move a cursor by a row, PgUp and PgDn by ten, Home and End to the ends, and the inspector scrolls to show it. In `Summary` they scroll it.

#### Scenario: Through the prompts
- **WHEN** the inspector has the keyboard on `Prompts` and the user presses ↓ three times
- **THEN** the fourth prompt's row is marked, and it is on the screen

#### Scenario: Another tab
- **WHEN** the inspector has the keyboard and the user presses 7
- **THEN** it shows `Tools`, with the cursor on its first row

#### Scenario: The flame graph by its key
- **WHEN** the inspector has the keyboard and the user presses 0
- **THEN** it shows `Flame`, with the cursor on the root

#### Scenario: Leaving the flame graph
- **WHEN** the inspector has the keyboard on `Flame` and the user presses Shift+←
- **THEN** it shows `Cache`

#### Scenario: Requests take the arrows
- **WHEN** the inspector has the keyboard on `Requests` and the user presses ←
- **THEN** the request before is selected, and the tab stays `Requests`

### Requirement: Inspector
The inspector SHALL show the selected session's X-ray as native controls under tabs: `Summary`, `Requests`, `Spend`, `Work`, `Waits`, `Prompts`, `Tools`, `Context`, `Cache` and `Flame`. Every number in it is the one `tokprof scan` prints for the session at the same moment, and in `Flame` the one `tokprof flame --session <id>` writes. Texts can be selected with the mouse, and ⌘C copies the selection.

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
The tabs from `Spend` to `Cache` SHALL show session-xray's blocks of those names: the spend by component and by chain, work by origin and by activity, waits and slow requests, the prompts, the tool table, what the context holds and what cost the most to keep in it, and the cache misses. `Requests` shows the request timeline (see Request timeline). A click on a tab shows it.

#### Scenario: Prompts
- **WHEN** the user clicks `Prompts`
- **THEN** the inspector lists each typed message with its cost, its requests and its two costliest activities, as the X-ray's prompts block does

### Requirement: Handoff in the inspector
The inspector SHALL have a `Copy handoff line` button that does what session-browser's Handoff key does, in Day and in the Sessions inspector. A double click on a lane shows the inspector full size, as ⏎ does; a double click on a row of Sessions opens it in Day (see desk-sessions).

#### Scenario: Copying the handoff
- **WHEN** the user clicks `Copy handoff line`
- **THEN** the clipboard holds the line that starts a new session from the selected one, and the bar says so

#### Scenario: A double click on a lane
- **WHEN** the user double-clicks a session's lane
- **THEN** that session is selected and its inspector shows full size under the bar, until esc

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

### Requirement: Color themes
The window SHALL carry the 41 color themes of Rex Beta 0.1.0 (build 1829), 14 light and 27 dark, each with its id, name and appearance as Rex lists them. It draws in the chosen light theme when the appearance is light and in the chosen dark theme when it is dark, with the colors Rex draws for the theme.

#### Scenario: The defaults
- **WHEN** nothing is chosen yet and the system is light
- **THEN** the window is drawn in Merino Light, and in Merino Dark when the system is dark

#### Scenario: Colors as Rex shows them
- **WHEN** the window is drawn in Merino Light
- **THEN** its background and accent are the colors Rex draws for Merino Light's canvas and accent

### Requirement: Marks in a theme
A theme SHALL give the window's background, panels, lines, text and accent, and the colors of its marks: green for work and warm caches, red for cold caches, the now line and stops, yellow for waits and cache writes, blue for cache reads and requests, cyan for output and messages, magenta for questions. On a dark theme, the large filled marks keep 60% of their color over the background, as the eye sees it; text keeps the theme's colors.

#### Scenario: A cold cache
- **WHEN** a listed session's cache is cold
- **THEN** its next-request cell is drawn in the theme's red

#### Scenario: Work on a dark theme
- **WHEN** the window is drawn in a dark theme
- **THEN** the bars of work, the request bars and the parts of the spend are their colors 60% of the way from the background in sRGB, and a warm countdown is the theme's green itself

### Requirement: Settings panel
`Settings…` in the app menu and ⌘, SHALL open the settings panel, a rounded card in the middle of the window over the window dimmed and blurred. While it is open, the keys and clicks go to the panel only, and ⌘Q still quits. ⌘, again, esc, its `Done` button and a click outside the card close it, and the window is as it was before it opened.

#### Scenario: Opening the settings
- **WHEN** the user presses ⌘,
- **THEN** the settings panel opens over the window, with the current choices marked

#### Scenario: Keys while it is open
- **WHEN** the settings panel is open and the user types `q`
- **THEN** the window stays open and nothing behind the panel changes

#### Scenario: Closing the settings
- **WHEN** the user presses esc in the settings panel
- **THEN** the panel closes, and the same session, tab and day are shown as before it opened

### Requirement: Appearance and theme settings
The settings panel SHALL offer `Appearance` as three tiles, `As in system` (the default), `Light` and `Dark`, each with a small picture of the window in it, and `Light theme` and `Dark theme` as grids of theme cards, each a picture of the theme's background with a prompt in its text and dots of its accent, green, yellow, red and blue, its name under it. A click picks a choice, and the window takes it at once. Under `As in system` the window follows the system, also when it changes.

#### Scenario: A dark theme on a light system
- **WHEN** the system is light and the user picks `Dark` and then `Ink Meridian` in `Dark theme`
- **THEN** the window turns dark at once and is drawn in Ink Meridian while the system stays light

#### Scenario: A light theme while dark is shown
- **WHEN** the window is dark and the user picks `Alpine Milk` in `Light theme`
- **THEN** the window stays dark, and it is drawn in Alpine Milk once it turns light

#### Scenario: The system's buttons
- **WHEN** the system is light and the window is drawn in a dark theme
- **THEN** the close, minimize and zoom buttons keep the system's light appearance

### Requirement: Keys in the settings
In the settings panel, Tab and Shift+Tab SHALL move the keyboard between `Appearance` and the theme grids, the window's appearance's grid first, where the keyboard starts, the order kept while it is open. The part with it is ringed in the accent; ← and → pick the appearance; in a grid, ← → go to the card before and after, ↑ ↓ to the card above and below, Home and End to the first and last, each picking the theme it reaches, scrolled into view; ⏎ closes the panel. The foot names these keys.

#### Scenario: The next dark theme
- **WHEN** `Dark theme` has the keyboard on Merino Dark and the user presses →
- **THEN** the dark theme after Merino Dark is picked and the window takes it at once

#### Scenario: The card below
- **WHEN** `Light theme` has the keyboard on its first card and the user presses ↓
- **THEN** the first card of the grid's next row is picked

#### Scenario: Opening in a dark window
- **WHEN** the window is dark and the user opens the settings
- **THEN** `Dark theme` comes right after `Appearance` and has the keyboard, and picking `Light` keeps the grids where they are

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

### Requirement: Settings in a frame without a window
A frame without a window (`--snap`, `--tree`) SHALL NOT read or write the saved settings: its appearance comes from `--theme`, or from the system without it, and its theme from `--colors`, or the default themes without it. `--scene settings` draws the frame with the settings panel open on the default choices.

#### Scenario: A saved dark choice
- **WHEN** the saved appearance is `Dark` and the user runs `tokprof-desk --snap out.png --theme light`
- **THEN** `out.png` is drawn in Merino Light

#### Scenario: The panel in a frame
- **WHEN** the user runs `tokprof-desk --snap out.png --scene settings`
- **THEN** `out.png` shows the settings panel over the window, with `As in system`, `Merino Light` and `Merino Dark` marked

### Requirement: Command palette
⌘K and `Command Palette…` in the app menu SHALL open the command palette: a rounded card 640 points wide near the top of the window, over the window dimmed and blurred as under the settings panel. It holds a query field with what is typed and a caret, and under it the entries that match, grouped under small headings, ten rows high at most and scrolling. ⌘K again, esc and a click outside the card close it, and the window is as it was.

#### Scenario: Opening the palette
- **WHEN** the user presses ⌘K
- **THEN** the palette opens with an empty query, its first entry highlighted

#### Scenario: A filter being typed
- **WHEN** the user presses ⌘K while a filter is being typed
- **THEN** the filter is kept as ⏎ keeps it, and the palette opens

#### Scenario: Closing with a click outside
- **WHEN** the palette is open and the user clicks a list row outside the card
- **THEN** the palette closes and the row is not selected

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

### Requirement: Palette matching
An entry SHALL match when every typed letter, ignoring case and spaces, is in its name in order; a session also matches by its project, a day by its date, a day only in runs of letters that each start a word, the words in a row. Matches rank by tightness: a letter at the start, at a word's start or next to the one before ranks higher. Groups go in the order of their best match, entries by rank; loose matches are left out while tighter ones exist. Matched letters are in the accent.

#### Scenario: A prefix first
- **WHEN** the user types `to`
- **THEN** `Tools`, `Today` and the sessions whose title starts with `To` rank above `Keyboard to the list`

#### Scenario: Loose matches
- **WHEN** the user types `to` and `Tools` matches
- **THEN** `Copy the handoff line`, where `t` and `o` lie far apart, is not shown

#### Scenario: A short group first
- **WHEN** the best tab and the best session match `to` equally well
- **THEN** `Tabs` is shown above `Sessions`

#### Scenario: Word starts
- **WHEN** the user types `nd`
- **THEN** `Next day` ranks above entries where `n` and `d` are inside a word

#### Scenario: Days by chance
- **WHEN** the user types `to` and the list has sessions of Thursday 8 October
- **THEN** `Today` is shown and `Thu 8 Oct 2026`, whose `T` and `O` start words two apart, is not, while `8oct` shows it

#### Scenario: At most
- **WHEN** a query matches more than 100 sessions or 30 days
- **THEN** the palette shows the 100 and the 30 that rank highest

#### Scenario: Nothing typed
- **WHEN** the query is empty
- **THEN** the palette shows `Actions`, then `Tabs`, then the 20 most recent sessions, then `Today` and the 14 newest days

#### Scenario: No match
- **WHEN** nothing matches the query
- **THEN** the palette says `Nothing matches` with the query, and ⏎ does nothing

### Requirement: Keys in the palette
While the palette is open, typed characters SHALL go to its query and Backspace takes the last one away; each change highlights the first entry. ↑ and ↓, and Ctrl+P and Ctrl+N, move the highlight, scrolled into view; ⏎ runs the highlighted entry and closes the palette; a click on a row runs it. No key reaches the window behind it, but ⌘Q quits. The palette's foot names these keys.

#### Scenario: Keys stay in the palette
- **WHEN** the palette is open and the user types `q` and `j`
- **THEN** the window stays open, the selection does not move, and the query is `qj`

#### Scenario: Moving the highlight
- **WHEN** the user presses ↓ twice and ⏎
- **THEN** the third entry runs

#### Scenario: At the ends
- **WHEN** the highlight is on the last entry and the user presses ↓
- **THEN** it stays on the last entry

### Requirement: Key help
`?` SHALL open the key help when no filter is being typed and no overlay is open: a rounded card over the window dimmed and blurred, listing every key of the window grouped under `Anywhere`, `Lanes`, `The day`, `Palette`, `Settings`, `Sessions`, `Inspector`, `Flame`, `Week`, `Month and Quarter` and `Year`, each key drawn as a keycap with what it does. esc or `?` close it, and no other key reaches the window behind it, but ⌘Q quits. The palette's `Key help` opens it too.

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

#### Scenario: The keys of Year
- **WHEN** the key help is open
- **THEN** its `Year` group names ↑ ↓ ← → in the grid, Home and End, `m` and ⇧M, `r`, `s` in Year in review, and ⏎; and no group is named `Calendar` or `Reports`

### Requirement: One overlay at a time
The settings panel, the command palette and the key help SHALL never show together. While one is open, the keys and menu items that open another do nothing; an entry of the palette that opens the settings or the key help closes the palette first.

#### Scenario: Settings over the palette
- **WHEN** the palette is open and the user presses ⌘,
- **THEN** the palette stays open and the settings do not open

#### Scenario: The palette over the settings
- **WHEN** the settings panel is open and the user presses ⌘K
- **THEN** the panel stays open and the palette does not open

### Requirement: Overlays in a frame without a window
`--scene palette` SHALL draw the frame with the palette open and `to` typed into its query, and `--scene keys` with the key help open.

#### Scenario: The palette in a frame
- **WHEN** the user runs `tokprof-desk --snap out.png --scene palette`
- **THEN** `out.png` shows the palette over the window with the entries that match `to`

### Requirement: Flame tab
`Flame` SHALL draw the selected session's spend as an icicle graph, the session on top and each frame's children below it, from the stacks and dollars of `tokprof flame --session <id>`. Each frame is a rounded bar as wide as its share of the shown root's dollars, the largest child first, with its name inside when it fits and its dollars when there is room. Frames narrower than 3 pt are not drawn.

#### Scenario: The same numbers as the profile
- **WHEN** the inspector shows `Flame` for a session
- **THEN** the root's dollars are the sum of the values of `tokprof flame --session <id> -o out.collapsed`, in µ$, and each frame's dollars are the sum of the stacks through it

#### Scenario: Widths
- **WHEN** a chain cost $3 of a $4 session
- **THEN** its bar is three quarters as wide as the root's, less the gap between bars

#### Scenario: Narrow frames
- **WHEN** 12 frames of the shown root are narrower than 3 pt
- **THEN** they are not drawn, and the line below the graph says `12 frames under 3 pt hidden`

#### Scenario: Shared head
- **WHEN** every stack starts with the same agent, project and session frames
- **THEN** they make one root frame, named by the three, instead of three bars of full width

#### Scenario: No prices
- **WHEN** no request of the session has a price
- **THEN** the frames are sized and labelled by tokens

### Requirement: Flame colors
The frames of `Flame` SHALL take their colors from the theme by what they stand for: grey for structure (the session, chains, turns), green for content (kinds, tools, targets), and for the components the colors of the `Spend` tab: blue for read, yellow for write, red for a rewrite after a miss, cyan for output, magenta for uncached input. The colors are soft tints, with the text in the theme's ink.

#### Scenario: A tool frame
- **WHEN** a frame names a tool
- **THEN** it is drawn in the green of content

### Requirement: Flame hover
The frame under the pointer SHALL lighten, and the line under the graph names its path under the shown root (the breadcrumb above the graph has the rest), its dollars, its share of the session and its share of its parent. Without a frame under the pointer, the line names the frame with the keyboard cursor.

#### Scenario: Hover
- **WHEN** the pointer rests on a turn frame that cost $1.20 of a $12.00 session and $3.00 of its chain
- **THEN** the line says its path under the shown root, `$1.20`, `10.0% of the session` and `40.0% of main`, with the chain's name

### Requirement: Flame zoom
A click on a frame SHALL show it as the root of the graph, full width, with the cursor on it. A breadcrumb above the graph shows the path from the session to the shown root; a click on a crumb shows that frame as the root. A click on the shown root zooms out one level. The zoom stays while the same session is shown, as long as its path still exists, and the bars glide to their new places.

#### Scenario: Zoom in and out
- **WHEN** the user clicks the chain `main` and then the first crumb
- **THEN** `main` fills the width with its turns below it, and then the whole session is shown again

#### Scenario: Another session
- **WHEN** the graph is zoomed and the user selects another session
- **THEN** `Flame` shows the new session whole

### Requirement: Keys in the flame graph
While the inspector has the keyboard on `Flame`, ← and → (and `h`, `l`) SHALL move a cursor to the drawn sibling before or after, ↓ (`j`) to the first drawn child, ↑ (`k`) to the parent, up to the shown root. ⏎ zooms into the cursor's frame, and esc and Backspace zoom out one level; esc gives the keyboard back to the lanes only when the graph is not zoomed. The cursor's frame is ringed in the accent.

#### Scenario: Down and right
- **WHEN** the cursor is on the root and the user presses ↓ and then →
- **THEN** the cursor is on the second child of the root, ringed, and the line under the graph names it

#### Scenario: Enter and esc
- **WHEN** the cursor is on a turn and the user presses ⏎ and then esc
- **THEN** the turn is shown as the root, and then its parent is

### Requirement: View switch
The bar SHALL hold a `Day | Week | Month | Quarter | Year` switch left of the period's name, and right of it, apart from it, the `Sessions` button. ⌘1 to ⌘5 show the periods in that order, and ⌘0 shows Sessions (see desk-sessions). Month and Quarter (see Report cards) and Year replace what is under the bar with one scrolling pane of the period's cards. The window starts in Day.

#### Scenario: Switching
- **WHEN** the user clicks `Month` in the bar, or presses ⌘3
- **THEN** what was under the bar gives way to the month's cards, and `Month` is marked in the switch

#### Scenario: Back to the day
- **WHEN** Year is shown and the user clicks `Day`, or presses ⌘1
- **THEN** the window shows Day as it was

#### Scenario: From the palette and the key help
- **WHEN** the user picks `Year` in the command palette
- **THEN** Year shows this year; the palette also offers `Day`, `Week`, `Month`, `Quarter`, `Sessions`, `This month`, `Last month`, `This quarter`, `Last quarter`, `Month in review` and `Year in review`, and the key help lists ⌘0 to ⌘5 and the keys of Month and Quarter and of Year

#### Scenario: Quarter by key
- **WHEN** the user presses ⌘4 on 12 November 2026
- **THEN** Quarter shows Q4 2026 so far, and `Quarter` is marked in the switch

#### Scenario: Year by key
- **WHEN** the user presses ⌘5 on 12 November 2026
- **THEN** Year shows 2026 so far, and `Year` is marked in the switch; no `Calendar` or `Reports` is left in the switch

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

### Requirement: Sessions behind a row
A click or ⏎ on a cause or a habit SHALL open its top 5 sessions by that cost under it, each with its title and dollars, and close them again; one row is open at a time. The cache lifetime rows and the tool hints are not split by session and open nothing. A click or ⏎ on a session shows Day with it selected and its day in the lanes; a session the list does not hold says `session <id> is not in the list` in the bar.

#### Scenario: Opening a cause
- **WHEN** the user clicks `idle > TTL`
- **THEN** its 5 sessions with the most premium on returns after the cache expired show under it, most first

#### Scenario: A session
- **WHEN** the user clicks a session under a row
- **THEN** the window shows Day with that session selected and its day in the lanes

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

### Requirement: Menu bar countdown setting
The settings panel SHALL offer `Menu bar countdown`, `Off` or `On`, after both theme grids, saved in the settings file as `menu_bar` (`off` or `on`). The default is `Off`; a value it does not know is `Off`. With `On`, the window shows a status item in the macOS menu bar at once; with `Off` it removes it at once. A frame without a window never shows the item. Tab reaches it after the second grid, and ← → pick it.

#### Scenario: Turning it on
- **WHEN** the user picks `On` in `Menu bar countdown`
- **THEN** the status item appears in the menu bar, and the settings file holds `"menu_bar": "on"`

#### Scenario: No settings yet
- **WHEN** the window starts and the settings file has no `menu_bar`
- **THEN** no status item appears

#### Scenario: A frame without a window
- **WHEN** the settings file holds `"menu_bar": "on"` and the user runs `tokprof-desk --snap out.png`
- **THEN** no status item appears

### Requirement: Sessions the menu bar counts
The menu bar SHALL count a listed session while its cache is warm and the next request would cost at least $1.00 once the cache is cold (the threshold). A session that would cost less cold, one whose cache is already cold, one still being costed, and one the filter hides do not count. The list's order and scroll do not matter.

#### Scenario: Below the threshold
- **WHEN** the only warm listed session would cost $0.84 cold
- **THEN** the menu bar counts no session

#### Scenario: Exactly the threshold
- **WHEN** a warm listed session would cost $1.00 cold
- **THEN** the menu bar counts it

### Requirement: Menu bar title
The status item's title SHALL be a snowflake `❄` and the minutes and seconds until the cache of the counted session that goes cold soonest goes cold, as the list's countdown shows them. In the last 5 minutes (the nudge time, 5:00 included) its cold price follows after ` · `. With no session counted, the title is the snowflake alone.

#### Scenario: A warm session
- **WHEN** the soonest counted session stays warm 12 minutes 40 seconds more
- **THEN** the title reads `❄ 12:40`

#### Scenario: The last minutes
- **WHEN** it stays warm 4 minutes 12 seconds more and would cost $7.19 cold
- **THEN** the title reads `❄ 4:12 · $7.19`

#### Scenario: The cache goes cold
- **WHEN** the cache of the only counted session goes cold
- **THEN** the title reads `❄` at that second

### Requirement: Menu bar menu
A click on the status item SHALL open a menu: one item per counted session, newest activity first, at most 8, each its title cut to 40 characters (its project while it has none), its countdown and its cold price; then a separator, `Open tokprof` and `Settings…`. With no session counted, the first item reads `No warm session costs $1.00 or more cold`, disabled.

#### Scenario: Nine counted sessions
- **WHEN** nine sessions count
- **THEN** the menu lists the eight with the newest activity, newest first

#### Scenario: A long title
- **WHEN** a counted session's title is `Find why the order export is slow on large accounts`
- **THEN** its item reads `Find why the order export is slow on la…  12:40 · $7.19 cold`

### Requirement: Picks in the menu bar menu
A session's item SHALL bring the window forward and open the session in Day (see desk-sessions, Opening a session in Day), with its `Summary` from the top; an open settings panel, palette or key help closes. `Open tokprof` brings the window forward as it is. `Settings…` brings it forward with the settings panel open. A session no longer listed brings the window forward as it is.

#### Scenario: A session from Reports
- **WHEN** the window shows Month or Quarter, which took Reports' place, hidden behind another app, and the user picks a session in the menu
- **THEN** the window comes forward in Day on that session's day, with that session selected

### Requirement: Nudge before the cache goes cold
When the soonest counted session has 5:00 or less left, the window SHALL ask for the user's attention once: the Dock icon bounces once, while the app is not active. It asks once per session per warm period: a new request that keeps the cache warm longer starts a new period. It shows no notification and plays no sound.

#### Scenario: Reaching 5:00
- **WHEN** the soonest counted session reaches 5:00 left while the user works in another app
- **THEN** the Dock icon bounces once, and not again at 4:59

#### Scenario: Warm again
- **WHEN** the session sends a request at 3:00 left, and later again reaches 5:00 left
- **THEN** the Dock icon bounces once more

#### Scenario: Started late
- **WHEN** the window starts while the soonest counted session has 2:30 left
- **THEN** the Dock icon bounces once

### Requirement: Menu bar updates
The status item SHALL update once a second while its title shows a countdown, also while the window is hidden, minimized or covered, and the system does not delay these updates while one shows. With no session counted it sets no timer of its own; it updates when the logs change and when a counted cache goes cold.

#### Scenario: A hidden window
- **WHEN** the window is hidden and the title reads `❄ 12:40`
- **THEN** a second later it reads `❄ 12:39`

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

### Requirement: Year view
⌘5 or `Year` in the bar's switch SHALL show Year in one scrolling pane under the bar. From the top: the key numbers; the grid with the metric pills, its legend, its numbers and the month bars; the punchcard and the rhythm; By project, Agents and models and What to change; the records and Year in review. It counts what `tokprof year` counts, with the window's `--agent`, `--project`, `--session`, `--since` and `--until`. ⌘1 or `Day` show Day as it was, and esc as The keyboard in Year says.

#### Scenario: Switching
- **WHEN** the user presses ⌘5
- **THEN** the view under the bar gives way to Year of this year, and `Year` is marked in the switch

#### Scenario: The same numbers as the CLI
- **WHEN** Year shows 2026 with spend, and the user runs `tokprof calendar --since 2026-01-01 --until 2027-01-01` and `tokprof year 2026` with the same filters at the same time
- **THEN** every day's level, the cuts, the active days, the streaks, the busiest day and the punchcard's levels equal the calendar's, and the key numbers, months, records and rhythm equal those of `tokprof year`

#### Scenario: A client filter
- **WHEN** the window runs with `--client`
- **THEN** Year counts every client, and one quiet line under the pills reads `Year counts every client: the day store has no client`

#### Scenario: From the palette and the key help
- **WHEN** the user picks `Year: messages` in the command palette
- **THEN** Year shows messages; the palette also offers `Year`, each other metric and `Year in review`, and the key help lists ⌘5 and the keys of Year

### Requirement: Year grid in the window
The grid SHALL draw each day of the shown year as a rounded square, a column per week from the week of 1 January to the week of 31 December, and a row per weekday labeled `Mon`, `Wed` and `Fri`, with each month's short name above its first column. Levels 1 to 4 are the accent (the cold color for `Avoidable`) mixed into the card's background at 30%, 55%, 80% and 100%. Level none is the subtle stroke, or in a dark theme an empty square with a hairline of it.

#### Scenario: Squares that use the room
- **WHEN** Year is drawn in a window 1400 points wide, or 960
- **THEN** the year's 53 columns fill the card's width, with squares as large as that allows, at most 44 points and at least 11, and the numbers stand under the grid
- **AND** a year whose 1 January is a Sunday and that has 366 days, as 2040, has 54 columns, and the days of the first and last columns outside the year are blank

#### Scenario: A dark theme
- **WHEN** the window is drawn in Merino Dark
- **THEN** the squares go from the subtle stroke through the accent mixed into the dark card to the accent

#### Scenario: Metric pills
- **WHEN** Year opens
- **THEN** it shows one segmented group `Spend`, `Agent time`, `At the keyboard`, `Messages`, `Commits` and `Avoidable`, as the bar's switch, with `Spend` picked

#### Scenario: Nothing and a little in a dark theme
- **WHEN** the window is drawn in Merino Dark, Neon Arcade or Cathode Amber
- **THEN** a day without the metric is an empty outlined square, never filled like a day of level 1

#### Scenario: Numbers of the metric
- **WHEN** Year shows commits for 2026, with logs from 6 Aug, on 12 Nov
- **THEN** the numbers under the grid read `Days with commits 50 of 99`, the streaks of days with commits, and the busiest day, Wed 11 Nov, with `175 commits`

#### Scenario: No number twice
- **WHEN** Year shows spend
- **THEN** the numbers under the grid are `Days with spend`, the streaks, `Busiest day` and `Kept by the store`, with no total, which the Spend tile shows, and no busiest weekday, which the Rhythm card shows

#### Scenario: Another metric
- **WHEN** the user clicks `Messages`
- **THEN** the grid, the legend, the numbers, the month bars and the punchcard show messages, without the store being read again

#### Scenario: Today
- **WHEN** the grid is drawn
- **THEN** the last day's square has a small dot in the theme's ink, and only the cursor is ringed

#### Scenario: Kept by the store
- **WHEN** the grid is drawn
- **THEN** the numbers under it end with `Kept by the store`: the days that come only from logs that are gone, or `no day yet`

#### Scenario: The legend
- **WHEN** the grid is drawn
- **THEN** the legend reads `less`, the squares, `more` and the cuts, then `no logs` with its outlined square and `to come` with its dot

### Requirement: A day in the year
The pointer over a day's square SHALL show a tooltip with the day's date and weekday and every metric of that day: spend, avoidable, requests, sessions with a request, agent time, time at the keyboard with its sittings, messages, and commits, pushes and pull requests. It also says whether the day comes only or partly from logs that are gone. A click on a square shows Day with the lanes of that day. Days without logs and days to come have no tooltip and open nothing.

#### Scenario: Hovering a day
- **WHEN** the pointer rests over the square of 5 November
- **THEN** a tooltip reads like `Thu 5 Nov 2026`, `$214 spent, $31.00 of it avoidable`, `1,820 requests in 8 sessions, 5 you typed in`, `agents worked 6.2h`, `2.5h at the keyboard in 14 sittings, 47 messages`, `3 commits, 1 push, no pull request`
- **AND** `you typed in` counts the sessions the Day view counts, those with an action of the user

#### Scenario: Where the tooltip goes
- **WHEN** the pointer rests over a day of the grid's last column
- **THEN** the tooltip stands under the grid at that day's column, kept inside the card, so that it covers no day

#### Scenario: A day longer than 24 hours
- **WHEN** the day of 20 September ran from Sun 05:10 to Mon 07:16
- **THEN** its tooltip adds `the day ran 26.1h, from Sun 05:10 to Mon 07:16`, so that 24.6h of agent time in it reads right

#### Scenario: A day whose logs are gone
- **WHEN** every log of 5 September is gone and the store keeps its numbers
- **THEN** its tooltip ends with `only from the store: its logs are gone`, and with `partly from the store: some of its logs are gone` when only some are

#### Scenario: Opening a day
- **WHEN** the user clicks the square of 5 November
- **THEN** the window shows Day with the lanes of 2026-11-05

### Requirement: Keys in Year
While the grid has the keyboard, a cursor SHALL ring one day, the last day to start with. ↑ and ↓ (`k`, `j`) move it a day back and forward, and ← and → (`h`, `l`) a week. Home and End go to the first day with logs and the last day, and it never leaves those. The day under the cursor shows its tooltip, and ⏎ opens it as a click does. Wherever the keyboard is in Year, `m` picks the next metric, ⇧M the one before, and `r` opens Year in review.

#### Scenario: A day by keys
- **WHEN** the grid has the keyboard with the cursor on the last day, and the user presses ← and ⏎
- **THEN** the window shows Day with the lanes of the date a week before the last day

#### Scenario: The metric by keys
- **WHEN** Year shows spend and the user presses `m` twice
- **THEN** it shows the time at the keyboard

#### Scenario: Sessions from Calendar
- **WHEN** Year, which took Calendar's place, is shown and the user presses ⌘0
- **THEN** the window shows Sessions

#### Scenario: Not into the days without logs
- **WHEN** the logs begin on 6 Aug and the cursor is on 7 Aug, and the user presses ↑ twice
- **THEN** the cursor stays on 6 Aug

### Requirement: Year computation
Year SHALL be computed on a worker; no frame waits for it. It refreshes the day store (see activity-calendar) and computes all of the shown year at once, from the store alone. While the store is being filled it reads `reading the logs into the store… <n> of <m> logs`. When the logs change while Year is shown, it is computed again once 4 times its last computation and at least 60 seconds passed, with `updating…` by the pills. While Year is not shown, nothing is computed for it.

#### Scenario: The first look
- **WHEN** the user opens Year for the first time with no store on disk
- **THEN** the pane reads `reading the logs into the store… 0 of 2303 logs`, the count grows while the window stays responsive, and the year replaces it when it arrives

#### Scenario: At rest
- **WHEN** the year has arrived and the logs do not change
- **THEN** the window draws no more frames for it

#### Scenario: The store was reset
- **WHEN** the store could not be parsed, or could not be read or written (see activity-calendar, Writing the store)
- **THEN** the line by the pills says so, as the command line warns

#### Scenario: Prices edited while the window runs
- **WHEN** the user edits `~/.config/tokprof/prices.toml` while Year is shown
- **THEN** the next computation reads the price files again and uses the new prices, so the window and `tokprof calendar` make the same fingerprint and do not read each other's logs again

#### Scenario: Another year
- **WHEN** the user steps from 2027 to 2026
- **THEN** 2026 is counted from the store as it is, and no log is read

### Requirement: Year in a frame without a window
`--scene year` SHALL draw the frame in Year, waiting up to 120 seconds for it. `--scene year-day` draws the same with the grid's cursor a week before the last day, showing its tooltip. `--year YYYY` picks the year, this year by default, and `--metric <name>` picks the metric by the names of `tokprof calendar --metric`. `--year` takes only YYYY.

#### Scenario: A frame of the calendar
- **WHEN** the user runs `tokprof-desk --snap out.png --scene year --metric keyboard`
- **THEN** `out.png` shows Year of this year with the time at the keyboard

#### Scenario: An unknown metric
- **WHEN** the user runs `tokprof-desk --snap out.png --scene year --metric tokens`
- **THEN** the command fails, naming the metrics

#### Scenario: A bad year
- **WHEN** the user runs `tokprof-desk --snap out.png --scene year --year 26`
- **THEN** the command fails with `--year takes YYYY`

#### Scenario: The old scene
- **WHEN** the user runs `tokprof-desk --snap out.png --scene calendar`
- **THEN** the command fails as for any unknown scene, naming the scenes, `year` among them

### Requirement: Request timeline
`Requests` SHALL open with a `Request timeline` card: the requests of the main conversation (the main log's main chains and advisor calls) on the time axis of Summary's track. Each request is a column of five stacked parts, the X-ray's components in the Spend card's colors: cache read, cache write, rewrite after miss, uncached input and output. They add up to its cost. A pill `$` / `tokens` switches the parts to tokens. Requests within the same 4 pt of the axis share one column.

#### Scenario: The parts add up
- **WHEN** a request read 412.3k tokens from the cache, re-wrote 490.3k after an idle break, sent 3 uncached tokens and wrote 1.2k of output
- **THEN** its column stacks its cache read, rewrite after miss, uncached input and output in dollars, their sum is the cost the table gives for it, and over the session the columns add up to the X-ray's `main` spend, plus `advisor` when the session has advisor calls

#### Scenario: Tokens
- **WHEN** the user clicks `tokens`, or presses `t` while the inspector has the keyboard
- **THEN** the columns show the same parts in tokens, the scale at the left is in tokens, and the pill shows `tokens` as chosen

#### Scenario: Requests close together
- **WHEN** several requests fall within the same 4 pt of the axis
- **THEN** they share one column whose parts are their sums, and the hover names how many requests it holds

#### Scenario: What stays
- **WHEN** `Requests` shows a session
- **THEN** the axis, the warm and cold spans and the dashed next request are those of Summary's track, and the times under the axis are those of Summary

### Requirement: Context line
The timeline SHALL draw each request's context as a line on a second scale at its right, from 0 to the context window of the session's latest model, or to the largest context when no window is known.

#### Scenario: A known window
- **WHEN** the latest model's context window is 1.0M
- **THEN** the right scale reads `1.0M` at its top and `0` at its foot, and the line passes through each column's context at the height of its share of the window

#### Scenario: No known window
- **WHEN** no context window is known for the session's latest model
- **THEN** the right scale goes up to the largest context and reads `largest` at its top

### Requirement: Miss and compaction markers
The timeline SHALL draw a thin line at each unexpected cache miss of the main conversation, from the label row down to just above its column, labelled by the name of its cause. The line is red when the miss's avoidable premium is $0.50 or more, and in the cache write's color otherwise. A compaction of the main log is a dashed line labelled `compacted` with its kind. An expected rebuild has no miss line.

#### Scenario: A costly idle miss
- **WHEN** a request at 14:00 missed after 1.0h idle on a 1h cache, with $3.82 avoidable
- **THEN** a red line stands at 14:00, labelled `idle > TTL`, and its column shows the rewrite after miss in red

#### Scenario: A small miss
- **WHEN** a request missed because its tools changed, with $0.12 avoidable
- **THEN** its line is in the cache write's color, labelled `tools changed`

#### Scenario: Which labels show
- **WHEN** labels of lines would come closer than 8 pt
- **THEN** they are placed costliest miss first and compactions last, and a label that cannot keep 8 pt clear of every label placed is left out

#### Scenario: Labels that collide
- **WHEN** a compaction at 16:28 stands 60 pt from a miss with $6.73 avoidable, and a $0.12 miss sits between them
- **THEN** only the $6.73 miss's label shows, and its line stops above its column so that the column's height reads

#### Scenario: Expected rebuilds
- **WHEN** the conversation compacted at 16:49 and the next request rebuilt its prompt
- **THEN** a dashed line stands at 16:49 labelled with the kind, such as `compacted (auto)`, and no miss line marks the rebuild

#### Scenario: The markers and the X-ray
- **WHEN** the X-ray of the session lists cache misses of the chain `main`
- **THEN** the timeline has one miss line for each of them, at its time and with its cause, and none for a subagent's misses

### Requirement: Event markers
A row of small marks above the columns SHALL show, from the main log:
- each message the user typed, as a dot in the message color;
- each time the user stopped the agent, in the stop color;
- each failure streak, as a red triangle at the request that read the third failed result. A streak is 3 or more tool calls in a row whose results failed; refused and denied calls neither count nor break it.

The agent's commits are diamonds on a row of their own just above the axis.

#### Scenario: A streak
- **WHEN** three `Read` calls in a row failed, then one worked
- **THEN** one red triangle stands at the request that read the third failure

#### Scenario: A commit
- **WHEN** the agent ran `git commit` and it worked
- **THEN** a diamond in ink stands on the commits' row at the request that made the call, apart from the typed messages' dots, in every theme
- **AND** a commit is a shell call that ran `git commit` and worked

#### Scenario: The legend lists what is there
- **WHEN** the session has no stop and no failure streak
- **THEN** the legend names neither

### Requirement: The time after the last request
The timeline and Summary's track SHALL keep the time from the last request to now to a tail of at most 8% of the axis, behind a break, with the cache's band along the axis (warm, then cold). The requests set the dollar scale. The next request is a dashed box at now with its cost over it, cut at the top with a mark when it is taller. Under the axis, now's label holds its time, and its day when the last request was on another day.

#### Scenario: Days later
- **WHEN** the last request was on Tue 3 Nov at 18:27 and it is Mon 9 Nov 03:41
- **THEN** now's label reads `Mon 9 Nov 03:41` over `now, 5.4d later`, and `18:27 last request` keeps its label before the cache's times do

#### Scenario: A dear next request
- **WHEN** the next request would cost $4.90 and the dearest column $1.20
- **THEN** the scale reads `$1.20`, the box reaches the top with a mark, and `$4.90` stands over it

#### Scenario: The legend
- **WHEN** the timeline is drawn
- **THEN** its legend names `cache warm, then cold, after the last request` and `next request, estimated`

### Requirement: Subagent band
A band under the columns SHALL show each subagent's span, from its first request to its last, with the subagent's task as its hover. The subagents' requests are not columns of the timeline.

#### Scenario: A subagent
- **WHEN** a subagent made requests from 12:29 to 12:37
- **THEN** the band under the columns covers 12:29 to 12:37, and its hover names the subagent's task

### Requirement: Request under the pointer
A card over the timeline SHALL describe the column under the pointer: the request's time, model and effort; its five parts in dollars and tokens and its cost; its context; the cause and detail when it missed, with the avoidable dollars; and the markers at its place. For a shared column, it gives the number of requests, their first and last time, and the sums.

#### Scenario: A miss
- **WHEN** the pointer rests on the column of a request that missed after an idle break
- **THEN** the card reads its time, `claude-opus-5-5`, its parts, its context, and `missed: idle > TTL: 1.0h idle, 1h TTL · $3.82 avoidable`

#### Scenario: Leaving the chart
- **WHEN** the pointer leaves the chart
- **THEN** the card goes away

### Requirement: Selected request
One request of the timeline SHALL be selected, the costliest when the tab opens for a session. A click on a column selects its request, the costliest of a shared column. The timeline marks its column in the accent. A card under the timeline shows it: its number, time, model, effort and cost; its cost split as a bar and a row per part in dollars and tokens; and the why, what entered and what changed of the requirements below.

#### Scenario: Another session
- **WHEN** the user selects another session and shows `Requests`
- **THEN** that session's costliest request is selected

#### Scenario: A click
- **WHEN** the user clicks a column of the timeline
- **THEN** its request is selected and the card shows it

### Requirement: Why a request cost what it did
The selected request's card SHALL say what the request read from the cache, what it wrote and why. The reason is the first that applies:
- the miss, with its cause and detail and the avoidable dollars;
- an expected rebuild, with its cause;
- the first request of its chain;
- the context growing or shrinking by the difference from the previous request.

#### Scenario: Why a request cost $3.95
- **WHEN** the selected request read 12.3k, re-wrote 490.3k after 1.0h idle on a 1h cache and cost $3.95
- **THEN** the why reads that it read 12.3k from the cache and wrote 490.3k because it missed the cache: idle > TTL: 1.0h idle, 1h TTL, with $3.82 avoidable

#### Scenario: A growing context
- **WHEN** a request without a miss wrote 5.1k tokens and its context is 5.1k larger than the previous request's
- **THEN** the why reads that it wrote 5.1k as the context grew by 5.1k

### Requirement: What entered since the previous request
The selected request's card SHALL list the blocks that entered the context since the previous request of its chain, grouped by kind, tool and target, with tokens, largest first: up to 8, then `<n> more` with their tokens. A block's size is the share of the prompt its text takes, as in `Context`. For the first request of a chain the list is what its prompt holds. When the log does not show the prompt, it reads `unknown: the log does not show this prompt`.

#### Scenario: What entered
- **WHEN** a tool result of `Bash cargo test` of 6.1k tokens, the call of 200 tokens and 400 tokens of assistant text entered since the previous request
- **THEN** the card lists `6.1k tool results Bash cargo test`, then `400 assistant text` and `200 tool calls Bash cargo test`

#### Scenario: An old log
- **WHEN** the log does not show what the request's prompt held
- **THEN** the list reads `unknown: the log does not show this prompt`, and no size is guessed

### Requirement: What changed since the previous request
The selected request's card SHALL list what changed since the previous request of its chain: the model, the effort, another setting the log names, the agent's version, the tools added and removed, the system prompt's size, and a compaction between them. It also gives the time since the previous request against the cache's lifetime, in red when it is longer.

#### Scenario: A long break
- **WHEN** the selected request came 1.0h after the previous one, and the cache lives 1h
- **THEN** the card reads `1.0h after the previous request, past the 1h cache` in red

#### Scenario: New tools
- **WHEN** two tools were added since the previous request and the effort went from `high` to `max`
- **THEN** the card reads `effort high → max` and `tools +2 −0` with the names of the two tools

### Requirement: Range of requests
A drag of 4 pt or more across the timeline SHALL select the requests whose columns it covers, shaded on the chart. While a range is selected, the card under the timeline totals it instead:
- the first and last request, their times and the span;
- the number of requests and the five parts in dollars and tokens;
- the unexpected misses in it and their avoidable dollars, the compactions, and the messages the user typed.

A click on a column, a plain step, or esc clears the range.

#### Scenario: Dragging over an hour
- **WHEN** the user drags from 13:00 to 14:05 over 40 requests that cost $12.40 with one miss of $3.82 avoidable
- **THEN** the card reads `40 requests`, `13:00 → 14:05`, `$12.40` with its parts, and `1 miss · $3.82 avoidable`

#### Scenario: Esc
- **WHEN** a range is selected, the inspector has the keyboard, and the user presses esc
- **THEN** the range is cleared and the card shows the selected request again, and the inspector keeps the keyboard

### Requirement: Requests table
Under the cards, `Requests` SHALL list the requests in a table: `#`, `sent`, `after` (red when longer than the cache lives), `in context`, then `read`, `write`, `uncached` and `output` in tokens as the log gives them, `cost`, and `why`. The why is `missed: <cause>`, `rebuilt: <cause>` for an expected rebuild, `first request`, or `context +<n>` or `context −<n>`. Where the inspector is 760 pt tall or more, the timeline and the cards stay above the table while it scrolls.

#### Scenario: A miss in the table
- **WHEN** request 57 missed after an idle break
- **THEN** its row's why reads `missed: idle > TTL`

### Requirement: Rows of the requests table
The table SHALL hold up to 300 rows: all of them, the 300 that end at the latest request, or the 300-row page that holds the selected request. A note says how many earlier or later requests are left out. The selected request's row is marked and kept on the screen, and a click on a row selects its request.

#### Scenario: The row follows the chart
- **WHEN** the user clicks a column of the timeline
- **THEN** that request's row is marked and scrolled into view, and the card shows it

#### Scenario: An early request
- **WHEN** a session has 622 requests and the user selects the 10th
- **THEN** the table shows requests 1 to 300 and a note `and 322 later requests`

### Requirement: Keys in Requests
While the inspector has the keyboard on `Requests`, the keys SHALL be:
- ← and → (and `h`, `l`) select the request before or after, as ↑ and ↓ (and `k`, `j`) do;
- PgUp and PgDn move by ten requests, Home and End to the first and the last;
- Shift+↑ and Shift+↓ start or extend a range from the selected request;
- `t` switches between dollars and tokens;
- esc clears a range before it gives the keyboard back;
- Shift+← and Shift+→ show the tab before or after, as in every tab.

#### Scenario: Stepping
- **WHEN** the costliest request is the 57th and the user presses →
- **THEN** the 58th request is selected, in the chart, the card and the table

#### Scenario: Leaving the tab
- **WHEN** the user presses Shift+→ on `Requests`
- **THEN** the inspector shows `Spend`

### Requirement: Requests in a frame without a window
`--scene requests-hover` SHALL draw the frame on `Requests` with the inspector's keyboard and the pointer's card over the selected request's column. `--scene requests-range` SHALL draw it with a range from the selected request over the 20 requests before it. Both use `--session` to pick a session, as the list's filter does.

#### Scenario: A session's timeline
- **WHEN** the user runs `tokprof-desk --snap out.png --session 230043e6 --tab requests`
- **THEN** `out.png` shows that session's timeline with its costliest request selected

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

### Requirement: Month in review in the window
Month SHALL offer `Month in review` as a button, as the key `r` and as a palette entry. Each opens the card of month-review over the window dimmed and blurred: the card of the month shown when that month is complete, else of the last complete month. ← and → step months, never past this month. esc, `Close` or a click outside close it. While it is open, every key but ⌘Q is its own. In Year, `r` opens Year in review instead.

#### Scenario: Opening the card
- **WHEN** Month shows October 2026 and the user presses `r`
- **THEN** the card of October 2026 opens over Month

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

### Requirement: The card's look
The card SHALL be 540 by 675 points, a 4:5 shape, in the theme's colors on its canvas. It shows the headline in large type, the title as an accent badge beside its rule, the big numbers three by two, and the month's days as squares in the calendar's levels of spend beside the facts. Then come the cache and the comparison as rows of small numbers, and the line for next month in a tinted box. The busiest day's square is ringed.

#### Scenario: The days' legend
- **WHEN** the card is drawn
- **THEN** under the month's squares a legend reads `spend per day · less`, the five squares, `more`, and `cuts at $121, $269, $591`

#### Scenario: No month before
- **WHEN** the month before has no data
- **THEN** the room of the comparison goes between the card's parts, and no empty band stands above the line for next month

#### Scenario: Plain words
- **WHEN** coming back after a cold cache lost $873, more than any other cause
- **THEN** the card's line reads `Coming back after the cache expired cost $873. Compact or wrap up a big session before a long break.`, while `tokprof month` keeps `$873 went to idle > TTL.`; requests without a price are a footnote

#### Scenario: Light and dark
- **WHEN** the window is drawn in Merino Light or Merino Dark
- **THEN** the card uses that theme's canvas, ink and accent, and every text stays inside it

### Requirement: Saving the card
`Save image` and `s` SHALL write the card alone as a 1080×1350 PNG to `~/Downloads/tokprof-<YYYY-MM>.png`, drawn in the window's theme without a window, with the window's filters, from the day store as it is and without listing any session. A file of that name is never overwritten: the next free of `-2`, `-3`… is used. The card and the bar then say `saved ~/Downloads/tokprof-2026-10.png`, or why it could not be saved. Nothing is sent anywhere.

#### Scenario: A card that cannot be drawn
- **WHEN** the day store cannot be read or is empty when the image is drawn
- **THEN** no file is written, and the card says `could not save the image: could not draw the month card: …`; no other picture is ever saved under the card's name

#### Scenario: A save that hangs
- **WHEN** drawing the image takes more than 60 seconds
- **THEN** it is stopped, the card says `could not save the image: it took more than 60 s, so it was stopped`, and `Save image` works again

#### Scenario: A second save
- **WHEN** `~/Downloads/tokprof-2026-10.png` exists and the user saves October again
- **THEN** the image goes to `~/Downloads/tokprof-2026-10-2.png`, and the window says so

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

### Requirement: Causes against the period before
Each row of `Could have been saved` SHALL give, after its dollars, the cause's avoidable dollars in the period before and the change in percent: `was <dollars> · <arrow> <percent>`. A cause with no dollars before reads `new`. The change is green when it falls and red when it rises. When the key numbers have no comparison (see Pace per day), or with `--client`, the rows have none either.

#### Scenario: Fewer idle misses
- **WHEN** `idle > TTL` could have saved $796, and $1,012 in the period before
- **THEN** the row reads `idle > TTL` `$796` `was $1,012 · ↓ 21%`, the change in green

#### Scenario: A new cause
- **WHEN** `tools changed` could have saved $12 and had nothing in the period before
- **THEN** its row reads `new`

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

### Requirement: Causes gone since the period before
After its rows, `Could have been saved` SHALL list each miss cause that had avoidable dollars in the period before and none in the period shown, most first. Such a row has `$0.00`, no share bar, nothing to open, and reads `was <dollars> · gone` in green. With no comparison (see Pace per day), no such row is listed.

#### Scenario: Compaction calls stopped
- **WHEN** compaction calls could have saved $367 in October and nothing in November so far, against the same point of October
- **THEN** the last row reads `compaction call` `$0.00` `was $367 · gone` in green

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

### Requirement: Week view
⌘2 or `Week` in the bar's switch SHALL show Week. Under the bar, the days of one calendar week are on the left and a column 340 points wide is on the right, in one scrolling pane. The days hold the strips, the totals and the week's rhythm lines. The column holds the tiles, What to change, the line for next week and By project. Week opens on this week. ⌘1 or `Day` show Day as it was, and esc as Window keys in Week says.

#### Scenario: Switching
- **WHEN** the user presses ⌘2
- **THEN** the window shows Week on this week, and `Week` is marked in the switch

#### Scenario: Back to the day
- **WHEN** Week is shown with no filter set and the days have the keyboard, and the user presses esc
- **THEN** the window shows Day as it was before Week

#### Scenario: A narrow window
- **WHEN** the window is 960 points wide
- **THEN** the column keeps its 340 points, and the days take the rest

#### Scenario: From the palette and the key help
- **WHEN** the user picks `Week: last week` in the command palette
- **THEN** Week shows the week before this one
- **AND** the palette also offers `Week` for this week, and the key help lists ⌘2 and the keys of Week

### Requirement: The week's window
A week SHALL run from local midnight on its Monday to local midnight on the next Monday, or until now for this week. Week counts what the day store (see activity-calendar) holds in that window, with the window's `--agent`, `--project` and `--session`. `--since` and `--until` cut it as they cut the calendar. A week in progress is partial, and its numbers say `so far`.

#### Scenario: The same spend as the summary
- **WHEN** Week shows 2–8 Nov 2026 and the user runs `tokprof summary --since 2026-11-02 --until 2026-11-09` on logs that were not deleted
- **THEN** the week's spend equals `Total priced`, $1,843, and its avoidable dollars equal `avoidable on cache misses`, $89.89

#### Scenario: A client filter
- **WHEN** the window runs with `--client`
- **THEN** Week's numbers from the day store count every client, one quiet line under the bar reads `Week counts every client: the day store has no client`, and the parts read from the week's logs count only that client

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

### Requirement: Week in the bar
In Week, the bar SHALL hold the switch, the `Sessions` button, the week's name with buttons to the week before and after, `Today`, the week's summary, the filter (see The filter in Week), the status and `Live`, and no zoom. The name reads like `9–15 Nov 2026`, `26 Oct – 1 Nov 2026` or `28 Dec 2026 – 3 Jan 2027`. The summary reads like `Mon–Thu so far · 39 sittings`, or `82 sittings` for another week; short of room, it drops the sittings, then goes, before the filter narrows.

#### Scenario: The week before
- **WHEN** Week shows this week and the user clicks the button before its name
- **THEN** Week shows last week, and `Today` goes back to this week

#### Scenario: Never past this week
- **WHEN** Week shows this week and the user clicks the button after its name
- **THEN** Week stays on this week

#### Scenario: Not before the logs
- **WHEN** Week shows the week that holds the store's first day with data, and the user clicks the button before its name
- **THEN** Week stays on that week

#### Scenario: A narrow bar
- **WHEN** Week shows this week on a Thursday in a window 1100 points wide, and then 960 points wide
- **THEN** the summary reads `Mon–Thu so far`, and then the bar has none, while the filter keeps its width

### Requirement: Days of the week
Week SHALL show a row per day, Monday first. A row has the weekday and the date, the day's strip and its numbers. A row holds the whole calendar day of its date (see activity-calendar, Calendar days), the span Day shows for that date, so a night after midnight stays in the row of the day it belongs to. The week's totals and tiles count the week's window (see The week's window).

#### Scenario: The night after midnight
- **WHEN** Tuesday's day ran until 03:40 on Wednesday
- **THEN** what came from Wed 00:00 to 03:40 is in Tuesday's row, and Wednesday's row starts at Wednesday's day start

#### Scenario: A night across Monday 00:00
- **WHEN** Sunday 1 Nov's day ran until Mon 2 Nov 04:21
- **THEN** what came from Mon 00:00 to 04:21 is in Sunday's row of 26 Oct – 1 Nov, not in the Monday row of 2–8 Nov, while the totals of 2–8 Nov count it

#### Scenario: Thursday afternoon
- **WHEN** it is Thursday 14:20
- **THEN** the rows of Friday, Saturday and Sunday are empty and have no numbers, and Thursday's strip has the now line at 14:20, with the time after it shaded

### Requirement: Days without logs
A day of the week before the store's first day with data SHALL read `no logs` across its row, with no strip and no numbers. A day with data before it and none of its own is drawn empty with zero numbers. A day after now opens nothing.

#### Scenario: The first week of the logs
- **WHEN** the store's first data is on Wed 4 Mar 2026 and Week shows 2–8 Mar 2026
- **THEN** the rows of Monday and Tuesday read `no logs`, and a later day without activity reads `$0.00` and zero hours

### Requirement: Day strips of the week
Each row's strip SHALL draw exactly its day's span by Day's rule (see work-rhythm, Start of the day), on one clock axis for the seven rows under one row of hour labels (see Clock axis of the week). In work-rhythm's colors it shows:
- green where some agent worked;
- yellow where a session waited for your answer, as the Day strip counts a wait, while no agent worked;
- a blue band where you sat;
- a red tick at each cold return.

Bars are at least 1 point wide.

#### Scenario: Work, then a wait
- **WHEN** an agent worked from 14:00 to 16:00, and from 16:00 a session waited for the user until 16:30 while no agent worked
- **THEN** the strip is green from 14:00 to 16:00 and yellow from 16:00 to 16:30

#### Scenario: A wait while another works
- **WHEN** one session waits for the user while another session's agent works
- **THEN** the strip is green there, not yellow

#### Scenario: A cold return
- **WHEN** a session came back to a cold cache at 13:42
- **THEN** the strip has a red tick at 13:42

#### Scenario: Returns after lunch line up
- **WHEN** sessions came back to a cold cache at 13:42 on Tuesday and at 13:42 on Wednesday
- **THEN** the two ticks stand one above the other

#### Scenario: Each day its own span
- **WHEN** Tuesday's day runs from 04:21 to Wed 05:05, and Wednesday's from 05:05
- **THEN** Tuesday's strip is drawn from 04:21 to 05:05 the next morning, Wednesday's from 05:05, and neither draws a moment of the other day

### Requirement: Clock axis of the week
The axis SHALL run, in hours from each row's midnight, from the whole hour at or before the week's earliest day start, but not before 00:00, to the whole hour at or after its latest day end, and be at most 30 hours long. Moments of a row outside the axis are not drawn. A small mark at that edge of its strip says that they exist, and the pointer over it names the time they reach.

#### Scenario: A usual week
- **WHEN** the days of 2–8 Nov 2026 start between 03:41 and 05:49 and the latest ends at 06:50 the next morning
- **THEN** the axis runs from 03:00 to 07:00 the next morning, 28 hours, the hour labels read from `03` to `06`, and no strip has a mark

#### Scenario: A day longer than the axis
- **WHEN** in 5–11 Oct 2026 the day of Sat 10 Oct, which had no night pause, ran from Fri 19:29 to Sun 11:56, and the other days start at 04:57 or later
- **THEN** the axis runs from 00:00 to 06:00 the next morning, the 30 hours allowed; Saturday's strip has a mark at its left edge reading `from Fri 19:29` and one at its right edge reading `until Sun 11:56`

#### Scenario: A night inside the axis
- **WHEN** Tuesday's agents worked until 02:30 on Wednesday, inside Tuesday's day, and the axis ends at 07:00
- **THEN** Tuesday's strip draws that work near its right end, with no mark

### Requirement: Agents at work in a strip
Under each strip SHALL be a band 6 points high. It is a stepped area in the work color, as high in each minute as the number of kept main sessions whose agent worked in that minute (see activity-calendar, Work and actions of the store). Its full height is the week's highest number or 4, whichever is more, the same for every row.

#### Scenario: A peak of six
- **WHEN** six sessions worked at once on Wednesday, and never more in the week
- **THEN** Wednesday's band reaches its full height there, and a day whose most was three reaches half of it

### Requirement: What the strips say
The pointer over a strip SHALL name, in a line under the days, the date, the time and what the strip shows there. It reads like `Tue 3 Nov · 16:36 · 5 agents at work · you were there`, `· waiting for your answer: 2 sessions`, or `· cold return: 51417f Speed up the…`, with the session's id and title as the list names them.

#### Scenario: Nobody there
- **WHEN** the pointer rests where no agent worked, no session waited and the user was not there
- **THEN** the line names only the date and the time

### Requirement: Numbers of a day
After its strip, a row SHALL show, in tabular figures:
- the day's spend;
- its agent hours, the calendar's `agent` metric, each minute some agent worked counted once;
- its time at the keyboard;
- its cold returns.

They are the calendar's numbers of that day, its whole cell, as Days of the week says.

#### Scenario: Tuesday 3 November
- **WHEN** Week shows 2–8 Nov 2026
- **THEN** Tuesday's row reads `$675`, `12.5h`, `7.3h` and `8 cold`, as Year's tooltip of 3 Nov and `tokprof day 2026-11-03` give them

### Requirement: The week's totals
Under Sunday's row, a footer row SHALL give the week's spend, agent hours (as Numbers of a day counts them), time at the keyboard and cold returns over the week's window, under the days' numbers and with no change; the tiles have the changes (see Week tiles). It is labelled `Week`, or `Mon–Thu so far` for a week in progress. Lines under it give the notes of the comparison, the last full week for a week in progress, and the line on rows and totals.

#### Scenario: The footer of a full week
- **WHEN** Week shows 2–8 Nov 2026
- **THEN** the footer reads `Week`, `$1,843`, `52.5h`, `29.3h` and `37 cold`, and the line under it `against 26 Oct – 1 Nov`

#### Scenario: This week so far
- **WHEN** it is Thursday 12 Nov 2026 at 15:00, this week has cost $1,753 so far, and Mon 2 Nov 00:00 to Thu 5 Nov 15:00 cost $1,079
- **THEN** the footer reads `Mon–Thu so far` and `$1,753`, the spend tile reads `↑ $674 (+62%)`, and a line under the footer starts `last week $1,843`

#### Scenario: Rows and the week's window
- **WHEN** 2–8 Nov 2026 spent $1,843, $28 of it after Mon 2 Nov 00:00 in the night of Sunday 1 Nov's day, and Sunday 8 Nov's day spent $21 after Mon 9 Nov 00:00
- **THEN** a line under the footer reads `The days add up to $1,836: $28 of the week fell in the night of Sun 1 Nov, and $21 of Sun 8 Nov came after the week.`

#### Scenario: Rows that add up
- **WHEN** no night crosses the week's Mondays at 00:00
- **THEN** the rows add up to the footer, and there is no such line

### Requirement: Rhythm lines of the week
Under the totals, Week SHALL print the `sittings`, `agents`, `answers`, `queue`, `night` and `cache` lines over the week's window. Then comes the rhythm's advice on cold returns (see work-rhythm, Rhythm advice). `sittings`, `answers` and `queue` are the lines of `tokprof pauses` with the window's `--since` and `--until`. `agents` and `cache` follow work-rhythm's Agents line and Cache line of the day; the `agents` line adds its agent-hours up over sessions, as Day's does.

#### Scenario: The same lines as pauses
- **WHEN** Week shows 2–8 Nov 2026 and the user runs `tokprof pauses --since 2026-11-02 --until 2026-11-09`
- **THEN** the week's `sittings`, `answers` and `queue` lines read as its rhythm section prints them, starting `sittings  82, 1.2d at the keyboard, 490 messages to 65 sessions`

#### Scenario: The agents line of a week
- **WHEN** the week's highest number of agents at work was 5, first reached on Tuesday at 17:42
- **THEN** the `agents` line ends `peak 5 on Tue at 17:42`

#### Scenario: The cache line of a week
- **WHEN** the week had 37 cold returns that cost $78.73 in their first requests, $75.12 of it avoidable
- **THEN** the line reads `cache     37 cold returns, $78.73 in their first requests, $75.12 of it avoidable`

### Requirement: Night line of the week
The `night` line SHALL give the share of agent time between 22:00 and 05:00, as month-review counts it, its hours, and the night with the most of it. A night is named by the evening it starts on.

#### Scenario: Tuesday night
- **WHEN** the agents worked 52.5h in the week, 15.2h of it between 22:00 and 05:00, and 3.6h of that from Tuesday 22:00 to Wednesday 05:00
- **THEN** the line reads `night     29% of agent time between 22:00 and 05:00, 15.2h; 3.6h of it on Tuesday night`

#### Scenario: No night work
- **WHEN** no agent worked between 22:00 and 05:00
- **THEN** the line reads `night     no agent worked between 22:00 and 05:00`

### Requirement: Opening a day from the week
A click on a day's row outside a tick SHALL show Day with the lanes of that date. A click on a cold-return tick shows Day of that date with the tick's session selected and its inspector on `Cache`. A session the list does not hold says `session <id> is not in the list` in the bar, and Day shows the date. Rows of days after now and `no logs` rows open nothing.

#### Scenario: A day
- **WHEN** the user clicks Wednesday's row of 9–15 Nov 2026
- **THEN** the window shows Day with the lanes of 2026-11-11

#### Scenario: A cold return
- **WHEN** the user clicks the tick of a cold return of session `51417f` on Tuesday
- **THEN** the window shows Day of that Tuesday with `51417f` selected and its inspector on `Cache` in the details pane

### Requirement: Week tiles
The column SHALL start with six tiles:
- spend;
- agent hours, as Numbers of a day counts them;
- time at the keyboard;
- avoidable on cache misses, with its share of spend;
- cache hit, the share of prompt tokens read from the cache (see month-review, The month's cache and model);
- cold returns, with their avoidable dollars.

Under each is its change against the week before, colored as in Key numbers against the period before, or else its pace per day. Cache hit is green when it rises.

#### Scenario: A week with fewer misses
- **WHEN** Week shows 2–8 Nov 2026, with $1,843 spent and $89.89 avoidable (4.9%), against $2,507 and $487 (19.4%) in 26 Oct – 1 Nov
- **THEN** the spend tile reads `↓ $664 (−26%)` in the text's color, and the avoidable tile reads `↓ $397 (−82%) · ↓ 14.5 points` in green

#### Scenario: Cold returns
- **WHEN** the week had 37 cold returns, against 61 the week before
- **THEN** the tile reads `37`, and under it `↓ 24 (−39%)` in green, comparing the count only; their avoidable dollars show once the week's logs are read

### Requirement: What to change in Week
Under the tiles, Week SHALL show `What to change`. It holds the three rows of `Could have been saved` and the three rows of `Habits that cost the most` with the most dollars, from what `tokprof share` prints for the week's window and filters. Each row has its dollars and share, its advice (see Advice of a row) and its sessions (see Sessions behind a row). The habits keep the note that their rows overlap.

#### Scenario: The same rows as share
- **WHEN** Week shows 2–8 Nov 2026 and the user runs `tokprof share --since 2026-11-02 --until 2026-11-09`
- **THEN** the savings read `idle > TTL` $88.57, `cache lifetime: subagents` $19.82 and `history edited` $1.32, and the habits `effort max or xhigh` $1,265, `prompts of 400k+` $908 and `single lookups` $596, as share prints them

#### Scenario: A session behind a row
- **WHEN** the user clicks `idle > TTL` and then its first session
- **THEN** its top 5 sessions open under it, and then the window shows Day with that session selected

### Requirement: The line for next week
Under What to change, Week SHALL show `Next week` with month-review's line for next month, counted from the store over the week's window. A line under it names the other avoidable causes with their dollars. With no avoidable miss in the week, the card reads `No avoidable misses this week.`

#### Scenario: Idle misses first
- **WHEN** idle > TTL lost $88.57 in the week and history edited $1.32
- **THEN** the card reads `$88.57 went to idle > TTL. Compact or wrap up a big session before a long break.`, and under it `The other avoidable causes: history edited $1.32.`

### Requirement: By project in Week
The column SHALL end with `By project`: the 6 projects with the most spend in the week, from the store's entries, named as the project picker names them (see desk-sessions). Each has a bar as long as its share of the first, its dollars, its share of spend and its agent hours, each minute some agent of it worked once. Its head says `agent hours, each minute once`. The rest is one row `<n> more`. A click on a project, not on `more`, sets the filter to it and stays on the week.

#### Scenario: The first project
- **WHEN** `~/src/zebrafish`, its worktrees included, cost $1,215 of the week's $1,843, and some agent of it worked in 36.5 hours of minutes
- **THEN** its row reads `zebrafish`, `$1,215`, `66%` and `36.5h`, with the longest bar

#### Scenario: Two projects of one name
- **WHEN** the week has `~/src/app` and `~/work/app`, and the temporary folders
- **THEN** their rows read `~/src/app`, `~/work/app` and `temporary folders`

#### Scenario: Hours against the tile
- **WHEN** agents of two projects worked in the same minute
- **THEN** that minute counts once in each project's hours and once in the agent hours tile, so no project has more hours than the tile, while the rows may add up to more

#### Scenario: More projects
- **WHEN** the week has 10 projects with spend
- **THEN** the card shows 6 of them and the row `4 more`

#### Scenario: A project
- **WHEN** Week shows 2–8 Nov 2026 and the user clicks `zebrafish`
- **THEN** Week still shows 2–8 Nov, the filter reads `~/src/zebrafish`, the note under the bar names `zebrafish`, and every row, line, tile and card counts only that project

### Requirement: Week computation
Week's numbers from the day store SHALL be computed on a worker, which refreshes the store as Year computation does. No frame waits for it. While it runs, the days read `reading the logs into the store… <n> of <m> logs`. While Week is not shown, nothing is computed for it.

#### Scenario: The first look
- **WHEN** the user opens Week and the store is up to date
- **THEN** the strips without the waits, the numbers, the totals, the tiles, the line for next week and By project show within a second; the cold returns' dollars come with the week's logs

### Requirement: The week's logs
The waits, `answers`, `queue`, `cache`, the advice, the cold returns' dollars, the peak and What to change SHALL come from one background read of the logs around the week. These are the logs whose last line with a time is at or after 18:00 on the Sunday before the week, and whose first line with a time is before 12:00 on the Monday after it, as `tokprof day` picks the logs of a day. While it runs, they read `reading the week's logs… <n> of <m> logs`.

#### Scenario: Before the read arrives
- **WHEN** the week's logs are being read
- **THEN** the strips have no yellow yet, and the lines that need the read show the count of logs read

#### Scenario: A week read again
- **WHEN** the logs change while Week shows this week
- **THEN** the read runs again once 4 times its last duration and at least 20 seconds have passed since it arrived, with `updating…` by the week's name

#### Scenario: Back to a week
- **WHEN** the user steps to last week and back
- **THEN** this week shows at once from what was read before, and is read again only when its logs changed

### Requirement: A week whose logs are gone
When some logs of the week are gone and the store keeps their entries, Week SHALL draw their work, sittings and cold returns from the store. A line under the rhythm lines then says that the waits, answers and What to change do not count them. When every log of the week is gone, those parts read `the logs of this week are gone; the store keeps only its numbers`.

#### Scenario: Every log of a week gone
- **WHEN** every log of a week is gone and the store keeps them
- **THEN** its strips show work, sittings and cold returns with no yellow, its numbers, totals and tiles show, and `answers`, `queue` and What to change read `the logs of this week are gone; the store keeps only its numbers`

#### Scenario: Some logs gone
- **WHEN** 3 logs of the week are gone and the store keeps them
- **THEN** the line reads `3 logs of this week are gone: the waits, answers and What to change do not count them`

### Requirement: Keys in Week
In Week, a cursor SHALL ring one day: today in this week, Monday in another. ↑ and ↓ (`k`, `j`) move it, and Home and End go to the first and the last day that can open. ⏎ opens its day as a click does. ← and → (`h`, `l`, `[`, `]`) step the week, and `t` shows this week. Tab and Shift+Tab move the keyboard among the days, What to change and By project. In What to change and By project, the keys of Keys of what to change apply.

#### Scenario: A day by keys
- **WHEN** the cursor is on Thursday and the user presses ↑ twice and ⏎
- **THEN** the window shows Day with the lanes of Tuesday's date

#### Scenario: Weeks by keys
- **WHEN** Week shows last week and the user presses `t`
- **THEN** it shows this week, with the cursor on today

#### Scenario: Not after now
- **WHEN** the cursor is on today, a Thursday, and the user presses ↓
- **THEN** it stays on Thursday

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

### Requirement: The filter in Week
In Week, the bar's filter SHALL count, in every part of Week and in the week before, only the logs whose agent, project or session id contains its text, ignoring case. The day store holds no titles, so here a title does not match, and the note under the bar says when the text matches sessions by title only. `/` starts it, and a click on a project in By project sets it to the project's path. The note names it, a project by its name. Day's lanes and the Sessions table show what it keeps there.

#### Scenario: A filter that matches nothing
- **WHEN** the filter is `zzz`
- **THEN** the days read `Nothing in 9–15 Nov 2026 matches "zzz".`

#### Scenario: The week's logs with a filter
- **WHEN** a filter is set and Week shows a week
- **THEN** the parts that need the week's logs read them for that filter, and each result is kept by week and filter

#### Scenario: A match by title only
- **WHEN** the filter is `python`, and 3 listed sessions have it in their titles but not in their agent, project or session id
- **THEN** Week counts none of them, the note under the bar reads `3 sessions match "python" only by title; Week counts agents, projects and session ids, not titles. Sessions (⌘0) lists them.`, and Day's lanes and the Sessions table keep them

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

### Requirement: Week in a frame without a window
`--scene week` SHALL draw the frame in Week, waiting up to 120 seconds for the store and for the week's logs. `--week <date>` picks the week that holds the date, in YYYY-MM-DD, this week by default. It takes the date of a week that has begun only.

#### Scenario: A frame of a week
- **WHEN** the user runs `tokprof-desk --snap out.png --tree out.json --scene week --week 2026-11-04`
- **THEN** `out.png` shows Week on 2–8 Nov 2026, and `out.json` holds the text of every row, line, tile and card

#### Scenario: A bad date
- **WHEN** the user runs `tokprof-desk --snap out.png --scene week --week 4.11`
- **THEN** the command fails with `--week takes YYYY-MM-DD`

#### Scenario: A week that has not begun
- **WHEN** the user runs `tokprof-desk --snap out.png --scene week --week 2027-05-03` in November 2026
- **THEN** the command fails with `the week of 2027-05-03 has not begun`

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
Quarter SHALL draw a column per Monday-to-Sunday week that touches it, 13 or 14: the spend of its window from Monday 00:00 to the next, cut at the quarter's first and last midnight, as Week's total counts it. A column stacks the quarter's 6 projects with the most spend and the rest as `other`, with a legend. A click on a column opens Week on that week; a click on a project in the legend sets the filter and stays on the quarter. Weeks to come are a bare baseline; there is no forecast.

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

#### Scenario: Bars add up to the quarter
- **WHEN** Quarter shows Q4 2026, and a night runs from Sunday 11 October into Monday 12 October
- **THEN** the columns add up to the quarter's Spend, the night's spend after midnight is in the column of 12 Oct, and a click on that column opens Week with the column's spend as its total

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
The key numbers, their comparison, the days of the month, spend by day, the weeks, months, trends and shifts of the quarter, by project and the records but the largest prompt SHALL come from the day store, refreshed as Year computation refreshes it, and show without reading the logs again. By origin, by model, the pause tables, what to change and the largest prompt read the period's logs. With `--client`, the store's numbers count every client, and the band says so.

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

### Requirement: Days without logs and days to come
In the year grid, a day before the logs begin (see year-review, When the logs begin) SHALL be drawn as a dashed outline in the subtle stroke with no fill, unlike a day of level none in any theme. A day after the last day is a small dot in the faint ink, with no square. Neither has a tooltip or opens anything. When the days without logs fill 15 or more columns, a quiet line in a tinted box inside them names when the logs begin.

#### Scenario: Logs from August
- **WHEN** Year shows 2026 on 12 Nov, with logs from Thu 6 Aug
- **THEN** 1 January to 5 August are dashed outlines, 13 November to 31 December are dots, and the line reads `Logs since 6 Aug. January to July have no logs, so these days are unknown, not idle.`

#### Scenario: A short gap
- **WHEN** the logs begin on 10 February
- **THEN** 1 January to 9 February are dashed outlines, and no line is drawn in them

#### Scenario: Not counted
- **WHEN** 2026 has 99 days of logs and 92 with spend
- **THEN** the numbers read `92 of 99 days with spend`, never `of 316`

### Requirement: Months under the grid
Under the grid, Year SHALL draw a bar for each month of the picked metric, centered under the month's columns, as tall as its share of the biggest month, with its amount above it and its short name below. The amounts are those of year-review, as `tokprof month` counts them. The month the logs begin reads `from <day>`, this month `so far`, and a month to come `not begun`. Months without logs have no bar and share one label. A click on a bar opens Month of that month.

#### Scenario: Months of a year with logs from August
- **WHEN** Year shows 2026 with spend on 12 Nov, with logs from Thu 6 Aug, and `tokprof month` gives $5,213, $3,729, $11,632 and $3,953 for August to November
- **THEN** the bars read `$5,213` (`from 6 Aug`), `$3,729`, `$11,632` and `$3,953 so far`; December reads `not begun`; and one label under January to July reads `no logs from January to 5 August`

#### Scenario: Another metric
- **WHEN** the user clicks `Commits`
- **THEN** the bars show each month's commits, and October's bar reads `486`

#### Scenario: Hovering a month
- **WHEN** the pointer rests over October's bar
- **THEN** a tooltip reads `October 2026 · $11,632 spent` and `click: open Month`

#### Scenario: Opening a month
- **WHEN** the user clicks October's bar
- **THEN** the window shows Month of October 2026

### Requirement: The keyboard in Year
In Year, the keys SHALL be those of Keys in Month and Quarter, over the year. Tab and Shift+Tab move the keyboard through the grid, the months, By project, What to change and the records. ← and → (`[`, `]`) step the year while no card has the keyboard, and `t` shows this year. In the months, ← and → move a month cursor, and ⏎ opens Month of that month. esc gives the keyboard back to the page, then clears the filter, then shows Day.

#### Scenario: Stepping by keys
- **WHEN** Year shows 2027, no card has the keyboard, and the user presses ←
- **THEN** it shows 2026

#### Scenario: A month by keys
- **WHEN** the months have the keyboard, the cursor is on this month, and the user presses ← and ⏎
- **THEN** the window shows Month of the month before

#### Scenario: Rows by keys
- **WHEN** By project or What to change has the keyboard
- **THEN** ↑ ↓ (`k`, `j`), Home, End and ⏎ act as in the cards of Month

### Requirement: The year in the bar
In Year, the period's name SHALL be the year, as `‹ 2026 ›`, and its extent reads `logs since 6 Aug · 99 days` in the year the logs begin, `316 of 365 days` in a later year in progress, and nothing for a past year. The buttons, and ← and → while no card has the keyboard, step the year. They never go past this year or before the year the logs begin. Today shows this year.

#### Scenario: The first year of logs
- **WHEN** Year shows 2026 and the logs begin on 6 Aug 2026
- **THEN** the button before the year does nothing, and ← keeps 2026, as Month and quarter periods stop at the store's first period

#### Scenario: Past this year
- **WHEN** Year shows this year and the user clicks the button after it
- **THEN** Year stays on this year

#### Scenario: Today
- **WHEN** Year shows 2026 in March 2027 and the user clicks Today
- **THEN** Year shows 2027 so far

### Requirement: Key numbers of the year
Year SHALL open with the seven key numbers of Month over the year's window (see year-review): spend, agent hours, hours at the keyboard, sessions, avoidable with its share and cold returns, saved by the cache, and commits. They compare with the same stretch of the year before (year-review, The year before), as Key numbers against the period before, An incomplete period before, A small agent in the period before and Pace per day say for Month.

#### Scenario: No logs in the year before
- **WHEN** Year shows 2026 on 12 Nov and the store holds nothing in 2025
- **THEN** spend reads `$24,527` with `$248 a day` under it, over the 99 days from 6 Aug, and the band notes `no logs in 2025`

#### Scenario: A year in progress
- **WHEN** Year shows 2027 on 12 Nov 2027 at 15:00 and 2026 is complete by the rule of An incomplete period before
- **THEN** each number is compared with 1 Jan – 12 Nov 2026 15:00, and the band notes `against Fri 1 Jan – Thu 12 Nov 2026 15:00`

#### Scenario: An incomplete year before
- **WHEN** Year shows 2027 and the store's first claude request is on 29 Sep 2026, and claude has 10% or more of 2027's spend
- **THEN** the band notes `no comparison with 2026: claude's logs start 29 Sep`, and each tile shows its pace per day

### Requirement: Rhythm in Year
Beside the punchcard, Year SHALL show a Rhythm card with year-review's Rhythm of the year: the busiest hour with its share of agent time, the night share of agent time from 22:00 to 05:00, the busiest weekday with its spend, and the weekend share of spend. The punchcard is the punchcard of `tokprof calendar` for the picked metric, drawn in the grid's levels, with the hours from 22:00 to 05:00 in the question color and a legend that names them.

#### Scenario: The rhythm
- **WHEN** Year shows 2026 with logs from 6 Aug
- **THEN** the Rhythm card's busiest weekday reads `Wednesday` and `$4,335 on Wednesdays`, as `tokprof calendar` prints `weekday   Wednesdays, $4,335`

#### Scenario: The punchcard follows the pills
- **WHEN** the user clicks `Agent time`
- **THEN** the punchcard shows agent time, and the Rhythm card does not change

### Requirement: By project and agents in Year
Year SHALL show By project as Month does: the 10 projects with the most spend in the year, and the rest as `<n> more`. It also shows Agents and models: a bar per agent with its spend and share, and a row per model with its agent, spend and share. Both come from the day store, with no cost components. The bar's filter counts in Year as The filter in Month and Quarter says, and a click on a project sets it, staying on the year.

#### Scenario: Agents
- **WHEN** Year shows 2026 and `tokprof summary --since 2026-01-01 --until 2027-01-01` gives claude $13,763 and codex $10,766 over its clients
- **THEN** the agents read claude $13,763 (56%) and codex $10,766 (44%), each within a dollar

#### Scenario: A project
- **WHEN** the user clicks the bar of `zebrafish`
- **THEN** the filter reads `~/src/zebrafish`, Year still shows 2026, and every card counts only that project

### Requirement: What to change in Year
Year SHALL show What to change from the day store: a row per miss cause that is not expected, most first. Each row has the year's avoidable dollars, their share of spend, the dollars of each month that has any, and the cause's advice (see cache-misses). A click or ⏎ on a row opens its top 5 sessions of the year by that cause's premium, as Sessions behind a row does. Year has no ways back and no habits.

#### Scenario: Idle in October
- **WHEN** `tokprof month 2026-10` says `$873 went to idle > TTL`
- **THEN** the `idle > TTL` row of 2026 names `Oct $873` among its months

#### Scenario: A session behind a row
- **WHEN** the user opens `idle > TTL` and clicks its first session
- **THEN** the window shows Day with that session selected and its day in the lanes

### Requirement: Records in Year
Year SHALL show the records of year-review's Records of the year as tiles, each with its label, its value and its date. A click on the biggest day or the most commits in a day opens Day of that date. A click on the most at once opens Day of its date at its minute. A click on the biggest week opens Week of that week.

#### Scenario: Records of 2026
- **WHEN** Year shows 2026 on 12 Nov, its biggest day is Sun 25 Oct with $1,221, every day since Sat 26 Sep had a request, and Wed 11 Nov had 175 commits
- **THEN** the tiles read `Biggest day` `$1,221` `Sun 25 Oct`, `Longest streak` `48 days` `since Sat 26 Sep, still going`, and `Most commits in a day` `175` `Wed 11 Nov`

#### Scenario: Opening the biggest week
- **WHEN** the user clicks `Biggest week`, which reads `$3,925` for Mon 12 – Sun 18 Oct
- **THEN** the window shows Week of that week, and its spend reads `$3,925`

### Requirement: Year in review in the window
Year SHALL offer `Year in review` as a button on its share card, as the key `r` and as a palette entry. Each opens the card of year-review over the window dimmed and blurred, for the shown year. ← and → show the year before and after, never past this year or before the year the logs begin. esc, `Close` or a click outside close it. While it is open, every key but ⌘Q is its own.

#### Scenario: Opening the card
- **WHEN** Year shows 2026 and the user presses `r`
- **THEN** the card of 2026 opens over Year

#### Scenario: The share card in the pane
- **WHEN** Year shows a year whose card is not ready
- **THEN** its share card shows the card small and dimmed, says `not ready yet` and the day it will be ready, and offers `Open preview` with `Save image` dimmed

#### Scenario: Keys stay with the card
- **WHEN** the card is open and the user types `q` and `m`
- **THEN** the window stays open and the metric does not change

### Requirement: The year card's look
The year card SHALL look as The card's look says for the month card, with the year as the headline, `2026 with agents`, and `so far` for this year. Beside the facts it shows the twelve months as bars of spend, none for a month without logs. The facts are the biggest day, the biggest week, the longest streak, the most at once, the busiest hour and the night share. The line for next year reads as the month card's, with `this year`. The card ends with `tokprof year 2026`.

#### Scenario: Plain words
- **WHEN** coming back after a cold cache lost $1,120 in 2026, more than any other cause
- **THEN** the card's line reads `Coming back after the cache expired cost $1,120 this year. Compact or wrap up a big session before a long break.`

#### Scenario: Not ready
- **WHEN** the logs begin on 10 Oct 2026, and the card of 2026 is not ready on 12 Nov 2026
- **THEN** its numbers are dimmed, and a band reads `Ready on Fri 1 Jan 2027` with `2026 has 34 days of logs, from 10 Oct`

#### Scenario: Logs that begin in the year
- **WHEN** the logs begin on 6 Aug 2026
- **THEN** the small numbers read `logs since 6 Aug`, and January to July have a hairline in place of a bar

#### Scenario: Light and dark
- **WHEN** the window is drawn in Merino Light or Merino Dark
- **THEN** the card uses that theme's canvas, ink and accent, and every text stays inside it

### Requirement: Saving the year card
`Save image` and `s` on Year in review SHALL save the card as Saving the card says for the month card, to `~/Downloads/tokprof-<YYYY>.png`, 1080×1350, never over a file of that name. A card that is not ready is not saved: `Save image` is dimmed, and `s` says when it will be ready.

#### Scenario: Saving 2026
- **WHEN** the card of 2026 is ready and the user presses `s`
- **THEN** it is written to `~/Downloads/tokprof-2026.png`, or `tokprof-2026-2.png` when that exists, and the card and the bar say where

#### Scenario: A card not ready
- **WHEN** the card of 2026 is not ready and the user presses `s`
- **THEN** no file is written, and the card says `the card of 2026 is ready on Fri 1 Jan 2027`

### Requirement: Year in review in a frame without a window
`--scene year-review` SHALL draw Year with Year in review open, and `--scene year-card` the card alone, filling the frame, both for the year of `--year`, else this year. A card that is not ready is drawn as the window shows it, dimmed under its band. `--scene year-card` with a year that has no logs fails with `could not draw the year card: <year> has no logs`.

#### Scenario: The image of 2026
- **WHEN** the user runs `tokprof-desk --snap out.png --scene year-card --year 2026 --size 540x675`
- **THEN** `out.png` is the card of 2026, 1080 by 1350 pixels

#### Scenario: A year without logs
- **WHEN** the user runs `tokprof-desk --snap out.png --scene year-card --year 2024` and the logs begin in 2026
- **THEN** no file is written, and the command fails with `could not draw the year card: 2024 has no logs`
