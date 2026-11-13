# desk-window Specification

## Purpose
`tokprof-desk` shows the session browser in a native window laid out as a profiler: the day's lanes on top, the sessions below them on the left, and an inspector of the selected session on the right, drawn as native controls in the system font. It shows the same sessions, costs, X-rays and days as `tokprof ui`, and adds the mouse.

## Requirements

### Requirement: Opening the window
`tokprof-desk` SHALL open one window titled `tokprof`, 1400 by 900 points the first time and no smaller than 960 by 640, listing every session that `tokprof ui` lists with the same `--project`, `--session`, `--since` and `--until` filters. It starts with the newest session selected and the day of the latest action in the lanes. Its colors are those of the theme the Color themes requirement picks.

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
- **WHEN** the Appearance setting is `As in system` and the system switches from light to dark while the window is open
- **THEN** the window switches to its dark theme without a restart

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
The bar SHALL hold the shown day's name with buttons to the day before and after, a Today button, the day's summary (when it started, its sittings and its sessions), the lanes' zoom (`Fit` or `24 h`), the filter, and a `Live` mark while the window follows the logs. In Reports and in Calendar the day's name, its buttons, Today, the summary, the zoom and the filter, which filters the list, are not shown.

#### Scenario: Other days
- **WHEN** the user clicks the button before the day's name
- **THEN** the lanes show the day before, and Today goes back to the day of the latest action

#### Scenario: Never past the latest day
- **WHEN** the lanes show the day of the latest action and the user clicks the button after its name
- **THEN** the lanes stay on that day

#### Scenario: In Reports
- **WHEN** Reports is shown
- **THEN** the bar holds the switch, the status and `Live`, and no day controls and no filter

#### Scenario: In Calendar
- **WHEN** Calendar is shown
- **THEN** the bar holds the switch, the status and `Live`, and no day controls and no filter

### Requirement: Window layout
In Day, the window SHALL show, under the bar, the lanes of the shown day across its whole width, at most 45% of its height with what is under them, and under them the list of sessions in a pane 380 points wide and the inspector of the selected session in the rest. Every text is in the system font; numbers that line up in columns use tabular figures.

#### Scenario: A narrow window
- **WHEN** the window is 960 points wide
- **THEN** the list keeps its 380 points and the inspector takes the remaining 580, with all ten tabs on one line

#### Scenario: Many lanes
- **WHEN** the day has more lanes than fit in 45% of the window
- **THEN** the lanes scroll under an hours row that stays on top, as many whole lanes show as fit, a line under them says how many more there are (`2 more lanes · scroll to see them`), and the list and the inspector keep their place

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
Under the lanes SHALL be the legend, and the `sittings`, `agents`, `answers`, `queue` and `cache` lines of the day as `tokprof day` prints them. A line under them names what is under the pointer: the time, the lane and what it shows there.

#### Scenario: The time under the pointer
- **WHEN** the pointer rests over a lane at 14:32
- **THEN** the line reads like `14:32 · ecad11 Chart of the interruptions · agent working`, with `agent working`, `waiting for your answer`, `your message`, `its question`, `you stopped it`, `you were there`, `cache warm` or nothing

#### Scenario: The queue line
- **WHEN** the shown day had 24 answers among several sessions waiting (see work-rhythm)
- **THEN** the `queue` line under `answers` reads as `tokprof day` prints it for that day

#### Scenario: The agents at work under the pointer
- **WHEN** the pointer rests over the `Agents at work` lane at 14:20, while three sessions worked
- **THEN** the line reads like `14:20 · 3 agents at work: ecad11 Chart of the…, 230043 Notes…, 66f6be Fresh…`, the sessions in the lanes' order

### Requirement: The selected session's day
Selecting a session whose last activity is on another day than the lanes show SHALL show that day in the lanes, as the local date of its last activity five hours earlier. When that day has no lane of the session, the lanes stay on it.

#### Scenario: A session of yesterday
- **WHEN** the user selects a session last active yesterday at 16:00
- **THEN** the lanes show yesterday, with its lane highlighted

### Requirement: Lanes and the filter
While a filter is set, the lanes SHALL show only the sessions it lists, and the bar counts them as `<n> of <m> sessions`; when it lists none of the day's, a line under the `You` lane says so. While a filter is being typed, the lanes stay on their day, wherever the selection goes. The filter field draws a thin caret after the text being typed, and a clear button while a filter is set.

#### Scenario: A filter that lists nothing of the day
- **WHEN** the user types the filter `zzzz` and no session matches
- **THEN** the lanes show only `You` and the line `no session of this day matches “zzzz”`, and the bar reads `0 of 6 sessions` on a day of six

#### Scenario: Clearing the filter
- **WHEN** the user clicks the clear button of the filter
- **THEN** the filter is empty, the list holds every session again, and every lane of the day is back

### Requirement: Session list
A row of the list SHALL show the last activity (`HH:MM`, after a label naming the day), a dot in the agent's color, the session's title or, while the title is not known, its project, and its cost with tabular figures (`…` while it is being costed). The selected row is a rounded row inset in the pane, in the theme's selection color with the text in the ink. The day labels show only in time order. A filter that lists no session says so in the list.

#### Scenario: A row
- **WHEN** the list shows a session titled `Find why the order export is slow` that cost $1.18
- **THEN** its row reads its time, the dot, the title and `$1.18`

#### Scenario: The selected row
- **WHEN** a session is selected
- **THEN** its row is rounded and inset 6 points from the pane's edges, in the selection color, its title, cost and dot in the colors of any other row

#### Scenario: A filter that lists nothing
- **WHEN** the user types the filter `zzzz` and no session matches
- **THEN** the list reads `No session matches “zzzz”` and, under it, `esc` as a keycap with `clears the filter`

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
One of the lanes, the list and the inspector SHALL have the keyboard at a time, the list to start with. The pane that has it rings its selection in the accent and draws its title in the accent; the other panes keep their selection without the ring. Tab moves the keyboard to the next of them in the order lanes, list, inspector, and Shift+Tab back; a click in one gives it the keyboard, and esc gives it back to the list. The keys of the list are those of Keys of the window; the others' follow.

#### Scenario: Tab
- **WHEN** the list has the keyboard and the user presses Tab
- **THEN** the inspector has it, and ↓ moves in the inspector instead of the list

#### Scenario: Esc
- **WHEN** the inspector has the keyboard and the user presses esc
- **THEN** the list has it again, and the window stays open

#### Scenario: What the ring marks
- **WHEN** the lanes, the list or the inspector has the keyboard
- **THEN** the ring is around the selected lane, the selected row, or the row under the inspector's cursor (the chosen tab on `Summary`; on `Flame`, the frame under its cursor), and nowhere else

#### Scenario: A filter being typed
- **WHEN** the user is typing a filter
- **THEN** the filter field has the ring, and the list's row does not

### Requirement: Keys in the lanes
While the lanes have the keyboard, ↑ and ↓ (and `k`, `j`) SHALL select the session of the lane above or below the selected session's, Home and End the first and the last lane's, as a click on the lane does; ← and → step the day as in the list.

#### Scenario: The lane below
- **WHEN** the lanes have the keyboard, the second lane's session is selected, and the user presses ↓
- **THEN** the third lane's session is selected in the list and shown in the inspector

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
The inspector SHALL have a `Copy handoff line` button that does what session-browser's Handoff key does, and a double click on a list row shows the inspector full size, as ⏎ does.

#### Scenario: Copying the handoff
- **WHEN** the user clicks `Copy handoff line`
- **THEN** the clipboard holds the line that starts a new session from the selected one, and the bar says so

### Requirement: Quitting the window
⌘Q, ⌘W, Ctrl+C and closing the window SHALL quit `tokprof-desk`. In Day, `q` quits too, and esc when the inspector is not full size and no filter is set; in Reports, `q` does nothing and esc goes back to Day. While a filter is being typed, `q` and esc act as session-browser's Quit says, and ⌘Q still quits.

#### Scenario: Closing the window
- **WHEN** the user closes the window
- **THEN** the program exits

#### Scenario: Typing q into a filter
- **WHEN** `q` is pressed while a filter is being typed
- **THEN** `q` is added to the filter

#### Scenario: q in Reports
- **WHEN** `q` is pressed while Reports is shown
- **THEN** the window stays open

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
For a frame without a window, `--size <W>x<H>` SHALL set the frame in points (default 1400x900), `--theme light` or `--theme dark` its appearance, `--colors <theme id>` its theme, `--day <date>` the day in the lanes, `--tab <name>` the inspector's tab, and `--tree <json>` write the frame's elements with their text and places.

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
The window SHALL keep its settings in `prefs.json` in `~/Library/Application Support/io.github.ignatov.tokprof/`: `appearance` as `system`, `light` or `dark`, and `light_theme` and `dark_theme` as theme ids. It writes the file only when a choice changes and reads it when it starts. A missing or broken file gives the defaults, and a value it does not know gives that setting's default.

#### Scenario: The next launch
- **WHEN** the user picks `Light` and `Alpine Milk`, quits the window and starts it again
- **THEN** the window opens light, in Alpine Milk

#### Scenario: No settings yet
- **WHEN** the window starts and the settings file does not exist
- **THEN** the window follows the system in Merino Light and Merino Dark, and no file is written until a choice changes

#### Scenario: The file cannot be written
- **WHEN** the user picks a theme and the settings file cannot be written
- **THEN** the window takes the theme until it quits, and the bar says `can't save settings`

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
The palette SHALL offer four groups. `Actions`: what the window does from a key, each with its key. `Tabs`: the inspector's tabs. `Sessions`: every listed session with its title, project and cost. `Days`: `Today`, the day of the latest action, and the days of the listed sessions, newest first, named as the list's separators. Picking a session selects it, scrolled to, with its `Summary`; a tab shows it; a day shows it in the lanes.

#### Scenario: Picking a session
- **WHEN** the user picks a session in the palette while another is selected
- **THEN** the palette closes, the session is selected in the list and on the screen, and the inspector shows its `Summary` from the top

#### Scenario: Picking a session from the full-size inspector
- **WHEN** the inspector is full size and the user picks a session
- **THEN** the list is drawn again with that session selected

#### Scenario: Picking a day
- **WHEN** the user picks `Yesterday`
- **THEN** the lanes show the day before today, and the selection stays

#### Scenario: Picking an action
- **WHEN** the user picks `Sort by Cost`
- **THEN** the list is sorted by cost, as the `Cost` button sorts it

#### Scenario: Actions that do nothing now
- **WHEN** no filter is set
- **THEN** `Clear the filter` is not offered; and `Full-size inspector` is offered only while the inspector is not full size, `Back to the list` only while it is

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
`?` SHALL open the key help when no filter is being typed and no overlay is open: a rounded card over the window dimmed and blurred, listing every key of the window grouped under `Anywhere`, `Lanes`, `Palette`, `Settings`, `List`, `Inspector`, `Flame`, `Reports` and `Calendar`, each key drawn as a keycap with what it does. esc or `?` close it, and no other key reaches the window behind it, but ⌘Q quits. The palette's `Key help` opens it too.

#### Scenario: Opening the key help
- **WHEN** the list has the keyboard and the user presses `?`
- **THEN** the key help opens, and it names ⌘K, ⌘,, `/`, `s`, `r`, `f`, `o`, `c`, `d` and `q`

#### Scenario: A filter being typed
- **WHEN** a filter is being typed and the user presses `?`
- **THEN** `?` is added to the filter

#### Scenario: Closing the key help
- **WHEN** the key help is open and the user presses `?`
- **THEN** it closes, and the same session, tab and day are shown

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
While the inspector has the keyboard on `Flame`, ← and → (and `h`, `l`) SHALL move a cursor to the drawn sibling before or after, ↓ (`j`) to the first drawn child, ↑ (`k`) to the parent, up to the shown root. ⏎ zooms into the cursor's frame, and esc and Backspace zoom out one level; esc gives the keyboard back to the list only when the graph is not zoomed. The cursor's frame is ringed in the accent.

#### Scenario: Down and right
- **WHEN** the cursor is on the root and the user presses ↓ and then →
- **THEN** the cursor is on the second child of the root, ringed, and the line under the graph names it

#### Scenario: Enter and esc
- **WHEN** the cursor is on a turn and the user presses ⏎ and then esc
- **THEN** the turn is shown as the root, and then its parent is

### Requirement: Reports view
The bar SHALL hold a `Day | Reports | Calendar` switch left of the day's name, and ⌘1 shows Day, ⌘2 Reports, ⌘3 Calendar. Reports replaces the lanes, the list and the inspector under the bar with the report of a period in one scrolling pane, the cards of the Report cards requirement. The window starts in Day.

#### Scenario: Switching
- **WHEN** the user clicks `Reports` in the bar, or presses ⌘2
- **THEN** the lanes, the list and the inspector give way to the report, and `Reports` is marked in the switch

#### Scenario: Back to the day
- **WHEN** Reports is shown and the user clicks `Day`, presses ⌘1 or presses esc
- **THEN** the window shows the lanes, the list and the inspector as they were

#### Scenario: From the palette and the key help
- **WHEN** the user picks `Reports: the last 7 days` in the command palette
- **THEN** Reports shows the last 7 days; the palette also offers `Reports`, `Day` and the other periods, and the key help lists ⌘1, ⌘2 and the keys of Reports

### Requirement: Report period
Reports SHALL show pills `7 days`, `30 days` and `90 days`, `30 days` to start with. A period of N days counts the requests from N × 24 hours before its computation started, as `tokprof summary --since <N>d` run then, with the window's `--agent`, `--project`, `--session`, `--client` and `--until` filters. A `--since` later than the period's start is kept instead.

#### Scenario: Another period
- **WHEN** the user clicks `7 days`
- **THEN** the report shows the last 7 days, computed then or kept from before

#### Scenario: With a project filter
- **WHEN** the window was started with `--project json-profiler`
- **THEN** each period counts only the requests that `tokprof summary --project json-profiler --since <N>d` counts

### Requirement: Report computation
A report SHALL be computed in the background; no frame waits for it. While it runs, Reports reads `computing the last <N> days… <n> of <m> logs`. Each period's report is kept. When the logs change while Reports shows a period, it is computed again once 4 times its last computation and at least 20 seconds passed since it arrived, with `updating…` by the period. While Day is shown, nothing is computed for reports.

#### Scenario: The first look
- **WHEN** the user switches to Reports for the first time
- **THEN** the pane reads `computing the last 30 days… 0 of 1937 logs`, the count grows while the window stays responsive, and the cards replace it when the report arrives

#### Scenario: Back to a period
- **WHEN** the user goes from `30 days` to `7 days` and back
- **THEN** the 30-day report shows at once, without being computed again

#### Scenario: At rest
- **WHEN** a report has arrived and the logs do not change
- **THEN** the window draws no more frames for it

### Requirement: Report cards
Reports SHALL show cards in rounded groups: key numbers; spend by day; by project; by origin; by model; cold returns by pause; sessions by largest context. The numbers are those `tokprof summary` and `tokprof pauses` print for the same period and filters; an amount of a model or agent without a price is in tokens, as there.

#### Scenario: The same numbers as the CLI
- **WHEN** Reports shows the last 30 days and the user runs `tokprof summary --since 30d` and `tokprof pauses --since 30d` at the same time
- **THEN** every amount, count and share the cards show for the same rows equals the one the commands print

### Requirement: Key numbers
The key numbers SHALL be: spend, the summary's `Total priced`; avoidable on cache misses with its share of spend, as the summary's line; cold returns, their count and avoidable dollars summed over the agents' totals of `tokprof pauses`; sessions, the main sessions with a counted request; and requests, the counted requests.

#### Scenario: Two agents
- **WHEN** `tokprof pauses` counts 218 cold returns with $759 avoidable for Claude Code and 40 with $31.99 for Codex
- **THEN** the cold returns read `258` and `$791 avoidable`

### Requirement: Spend by day
The day chart SHALL have a bar for each local calendar date of the period, each request's cost on its date, stacked by cache read, cache write, rewrite after miss, uncached input and output, with a legend. The bars sum to the spend. The pointer over a bar, or the day cursor, names under the chart the date, the day's spend, each component's and the requests. A click on a bar shows Day with that date's lanes.

#### Scenario: Hovering a day
- **WHEN** the pointer rests over the bar of 3 November
- **THEN** the line under the chart reads like `Mon 3 Nov · $412 · 1,820 requests` and each component's amount

#### Scenario: Opening a day
- **WHEN** the user clicks the bar of 3 November
- **THEN** the window shows Day with the lanes of 2026-11-03

#### Scenario: A day without requests
- **WHEN** a date of the period has no request
- **THEN** it has an empty column, so the dates keep their places

### Requirement: Spend by project and by origin
The project card SHALL show the 10 projects with the most spend, and the origin card the 10 origins of the summary's origin table (`prompt`, `agent:…`, `skill:…`), each a bar as long as its share of the first with its dollars and its share of all; the rest as one row `<n> more`. A click on a project, not on `more`, sets the list's filter to the project and shows Day.

#### Scenario: A project
- **WHEN** the user clicks the bar of `~/src/json-profiler`
- **THEN** the window shows Day, the filter reads `~/src/json-profiler`, and the list holds only its sessions

### Requirement: Spend by model and the pause tables
The model card SHALL hold the summary table's rows in its order: agent, model, requests, prompt known, the five components and the total. Per agent, the cold-return card holds the rows of the cold-return table of `tokprof pauses` (pause, returns, first request, avoidable, median context) with its totals line, and the context card the rows of its largest-context table (sessions, spend, share, cold returns).

#### Scenario: No cold returns
- **WHEN** an agent has no cold return in the period
- **THEN** its part of the card reads `No cold returns.`

### Requirement: Keys in Reports
In Reports, Tab and Shift+Tab SHALL move the keyboard through the period, by day, by project, by origin, by model, cold returns and largest context, ringed in the accent and scrolled into view. ← and → pick the period or move the day cursor; ↑ ↓ (`k`, `j`), Home and End move a row cursor; ⏎ opens the cursor's day or project as a click does. ⌘Q, ⌘W, ⌘,, ⌘K, `?`, Ctrl+C, ⌘1, ⌘2 and ⌘3 act as in Day; esc shows Day; the other keys do nothing.

#### Scenario: A day by keys
- **WHEN** by day has the keyboard, and the user presses ← twice and ⏎
- **THEN** the window shows Day with the lanes of the date two days before the last of the period

#### Scenario: The period by keys
- **WHEN** the period has the keyboard and the user presses ←
- **THEN** `7 days` is picked

### Requirement: Reports in a frame without a window
`--scene reports` SHALL draw the frame in Reports on the last 30 days, waiting up to 120 seconds for the report.

#### Scenario: A frame of the report
- **WHEN** the user runs `tokprof-desk --snap out.png --scene reports`
- **THEN** `out.png` shows the cards of the last 30 days

### Requirement: What to change
Under the key numbers, Reports SHALL show three cards with the rows `tokprof share` prints for the period and filters: `Could have been saved` (a row per cause with the avoidable dollars, the share of spend and the dollars per active hour), `Coming back after a cold break` (the five ways with their dollars, share of resuming and the returns each was cheapest in, the cheapest marked) and `Habits that cost the most` (cost and share, with the note that the rows overlap).

#### Scenario: The same rows as share
- **WHEN** Reports shows the last 30 days and the user runs `tokprof share --since 30d` with the same filters at the same time
- **THEN** every row of `What could have been saved`, `Coming back after a cold break` and `Habits that cost the most` shows the same name, dollars, share, dollars per active hour and cheapest count

#### Scenario: No cold returns
- **WHEN** the period has no cold return
- **THEN** the ways card reads `No cold returns to replay.`

### Requirement: Advice of a row
Under a cause, a habit and the ways back, Reports SHALL show the sentence tokprof's advice says about it: the cache-miss hint of the cause, the agents' cache lifetime settings, the thinking advice for effort and thinking, the lookup advice, the setup hint of the costliest source, a tool hint's own words, and the rhythm's advice on cold returns. A row tokprof has no advice for shows none.

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
In Reports, Tab SHALL reach the savings, the ways back and the habits after the period and before by day. In them ↑ and ↓ (`k`, `j`), PgUp, PgDn, Home and End move the row cursor over the rows and the open sessions, and ⏎ opens or closes a row or opens a session. The key help names these keys.

#### Scenario: A session by keys
- **WHEN** the savings have the keyboard on a row with sessions and the user presses ⏎, ↓ and ⏎
- **THEN** the window shows Day with the row's first session selected

### Requirement: What to change in a frame without a window
`--scene reports-advice` SHALL draw the frame in Reports on the last 30 days, waiting up to 120 seconds, with the sessions of the first cause that has any open.

#### Scenario: The frame
- **WHEN** the user runs `tokprof-desk --snap out.png --scene reports-advice`
- **THEN** `out.png` shows what to change with the top sessions of the first such cause

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
A session's item SHALL bring the window forward, shown from Day, with that session selected and the inspector from its top, as a click on its row does; an open settings panel, palette or key help closes. `Open tokprof` brings the window forward as it is. `Settings…` brings it forward with the settings panel open. A session no longer listed brings the window forward as it is.

#### Scenario: A session from Reports
- **WHEN** the window shows Reports, hidden behind another app, and the user picks a session in the menu
- **THEN** the window comes forward in Day with that session selected

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
While the `Waiting order` setting is `On`, the list's head SHALL offer `Waiting` after `Time`, `Cost` and `Next`, sorting as session-browser's waiting order. In it, `Waiting for you` heads the waiting sessions and `Others` the rest, as the day labels head rows in time order, and a waiting row shows its wait, like `16m`, in place of its time. The palette offers `Sort by Waiting` then. With no session waiting, the list reads `Nobody waits for you` above `Others`.

#### Scenario: The setting off
- **WHEN** `Waiting order` is `Off`
- **THEN** the head offers `Time`, `Cost` and `Next`, `s` cycles those three, no row shows a wait, and the palette has no `Sort by Waiting`

#### Scenario: Three sessions wait
- **WHEN** `Waiting order` is `On` and the user clicks `Waiting` while three sessions wait, for 40m, 16m and 2m
- **THEN** the list starts with `Waiting for you` and rows reading `40m`, `16m` and `2m` in place of their times, then `Others` and the rest newest first

#### Scenario: From the palette
- **WHEN** the user picks `Sort by Waiting` in the palette
- **THEN** the list is sorted by waiting, as the `Waiting` button sorts it

#### Scenario: Nobody waits
- **WHEN** the user clicks `Waiting` and no session waits
- **THEN** the list reads `Nobody waits for you`, then `Others` and every session newest first

#### Scenario: A frame in waiting order
- **WHEN** the user runs `tokprof-desk --snap out.png --scene waiting`
- **THEN** `out.png` shows the list in waiting order, as with the setting `On`, without reading the saved settings

### Requirement: Waiting order setting
The settings panel SHALL offer `Waiting order`, `Off` or `On`, last, after `Menu bar countdown`, saved in the settings file as `waiting` (`off` or `on`). The default is `Off`; a value it does not know is `Off`. Picking `Off` while the list is sorted by waiting sorts it by time. Tab reaches it after `Menu bar countdown`, and ← → pick it.

#### Scenario: Turning it on
- **WHEN** the user picks `On` in `Waiting order`
- **THEN** the list's head offers `Waiting` at once, and the settings file holds `"waiting": "on"`

#### Scenario: Turning it off in waiting order
- **WHEN** the list is sorted by waiting and the user picks `Off`
- **THEN** the list is sorted by time and `Waiting` leaves the head

#### Scenario: No settings yet
- **WHEN** the window starts and the settings file has no `waiting`
- **THEN** the head offers `Time`, `Cost` and `Next` only

### Requirement: Calendar view
⌘3 or `Calendar` in the bar's switch SHALL show Calendar: in one scrolling pane under the bar, the metric as pills, the calendar of activity-calendar drawn large, its legend and its numbers, and its punchcard, in rounded cards as in Reports. It counts what `tokprof calendar --metric <metric>` counts with the window's `--agent`, `--project`, `--session`, `--since` and `--until`. esc, ⌘1 or `Day` show Day as it was.

#### Scenario: Switching
- **WHEN** the user presses ⌘3
- **THEN** the lanes, the list and the inspector give way to the calendar, and `Calendar` is marked in the switch

#### Scenario: The same numbers as the CLI
- **WHEN** Calendar shows spend and the user runs `tokprof calendar` with the same filters at the same time
- **THEN** every cell's level, the cuts, the total, active days, streaks, the busiest day and weekday, and the punchcard's levels are the same

#### Scenario: From the palette and the key help
- **WHEN** the user picks `Calendar: messages` in the command palette
- **THEN** Calendar shows messages; the palette also offers `Calendar` and each other metric, and the key help lists ⌘3 and the keys of Calendar

### Requirement: Calendar grid in the window
The grid SHALL draw each day as a rounded square, a column per week and a row per weekday, labeled `Mon`, `Wed` and `Fri`, with the month labels of the CLI above. Levels 1 to 4 are the accent (the cold color for `Avoidable`) mixed into the card's background at 30%, 55%, 80% and 100%. Level none is the subtle stroke, or in a dark theme an empty square with a hairline of it. A dot marks the last day. The legend reads `less`, the squares and `more`, with the cuts.

#### Scenario: Squares that use the room
- **WHEN** the grid has 14 weeks in a window 1400 points wide
- **THEN** each square is 44 points, the most a square takes, with the numbers beside the grid; with more weeks the squares shrink to fit, down to 11 points, and the numbers go under the grid when squares beside them would be under 14 points

#### Scenario: A dark theme
- **WHEN** the window is drawn in Merino Dark
- **THEN** the squares go from the subtle stroke through the accent mixed into the dark card to the accent

#### Scenario: Metric pills
- **WHEN** Calendar opens
- **THEN** it shows one segmented group `Spend`, `Agent time`, `At the keyboard`, `Messages`, `Commits` and `Avoidable`, as the bar's switch, with `Spend` picked

#### Scenario: Nothing and a little in a dark theme
- **WHEN** the window is drawn in Merino Dark, Neon Arcade or Cathode Amber
- **THEN** a day without the metric is an empty outlined square, never filled like a day of level 1

#### Scenario: Numbers of the metric
- **WHEN** Calendar shows commits
- **THEN** the numbers beside the grid read `828 commits`, `Days with commits 46 of 95`, the streaks of days with commits, and the busiest day with `77 commits`

#### Scenario: Another metric
- **WHEN** the user clicks `Messages`
- **THEN** the grid, the legend, the numbers and the punchcard show messages, without the store being read again

#### Scenario: Today
- **WHEN** the grid is drawn
- **THEN** the last day's square has a small dot in the theme's ink, and only the cursor is ringed

#### Scenario: Kept by the store
- **WHEN** the calendar is drawn
- **THEN** beside the grid, or under it in a window too narrow for both, the numbers end with `Kept by the store`: the days that come only from logs that are gone, or `no day yet`

### Requirement: A day in the calendar
The pointer over a day's square SHALL show a tooltip with the day's date and weekday and every metric of that day: spend, avoidable, requests, sessions with a request, agent time, time at the keyboard with its sittings, messages, and commits, pushes and pull requests; and whether the day comes only or partly from logs that are gone. A click on a square shows Day with the lanes of that day. A day after the last day has no square.

#### Scenario: Hovering a day
- **WHEN** the pointer rests over the square of 5 November
- **THEN** a tooltip reads like `Thu 5 Nov 2026`, `$214 spent, $31.00 of it avoidable`, `1,820 requests in 8 sessions, 5 you typed in`, `agents worked 6.2h`, `2.5h at the keyboard in 14 sittings, 47 messages`, `3 commits, 1 push, no pull request`
- **AND** `you typed in` counts the sessions the Day view counts, those with an action of the user

#### Scenario: Where the tooltip goes
- **WHEN** the pointer rests over a day of the grid's last column
- **THEN** the tooltip stands beside the grid at that day's row, over the numbers, or under the grid when the numbers are under it, so that it covers no day

#### Scenario: A day longer than 24 hours
- **WHEN** the day of 20 September ran from Sun 05:10 to Mon 07:16
- **THEN** its tooltip adds `the day ran 26.1h, from Sun 05:10 to Mon 07:16`, so that 24.6h of agent time in it reads right

#### Scenario: A day whose logs are gone
- **WHEN** every log of 5 September is gone and the store keeps its numbers
- **THEN** its tooltip ends with `only from the store: its logs are gone`, and with `partly from the store: some of its logs are gone` when only some are

#### Scenario: Opening a day
- **WHEN** the user clicks the square of 5 November
- **THEN** the window shows Day with the lanes of 2026-11-05

### Requirement: Keys in Calendar
In Calendar, a cursor SHALL ring one day, the last day to start with. ↑ and ↓ (`k`, `j`) move it a day back and forward, ← and → (`h`, `l`) a week; Home and End go to the first and the last day; it never leaves the days shown. The day under the cursor shows its tooltip, and ⏎ opens it as a click does. `m` picks the next metric and ⇧M the one before. ⌘Q, ⌘W, ⌘,, ⌘K, `?`, Ctrl+C, ⌘1, ⌘2 and ⌘3 act as in Day; esc shows Day; the other keys do nothing.

#### Scenario: A day by keys
- **WHEN** the cursor is on the last day and the user presses ← and ⏎
- **THEN** the window shows Day with the lanes of the date a week before the last day

#### Scenario: The metric by keys
- **WHEN** Calendar shows spend and the user presses `m` twice
- **THEN** it shows the time at the keyboard

### Requirement: Calendar computation
The calendar SHALL be computed on a worker; no frame waits for it. It refreshes the day store (see activity-calendar) and computes every metric at once. While the store is being filled it reads `reading the logs into the store… <n> of <m> logs`. When the logs change while Calendar is shown, it is computed again once 4 times its last computation and at least 60 seconds passed, with `updating…` by the pills. While Calendar is not shown, nothing is computed for it.

#### Scenario: The first look
- **WHEN** the user opens Calendar for the first time with no store on disk
- **THEN** the pane reads `reading the logs into the store… 0 of 1958 logs`, the count grows while the window stays responsive, and the calendar replaces it when it arrives

#### Scenario: At rest
- **WHEN** the calendar has arrived and the logs do not change
- **THEN** the window draws no more frames for it

#### Scenario: The store was reset
- **WHEN** the store could not be parsed, or could not be read or written (see activity-calendar, Writing the store)
- **THEN** the line by the pills says so, as the command line warns

#### Scenario: Prices edited while the window runs
- **WHEN** the user edits `~/.config/tokprof/prices.toml` while Calendar is shown
- **THEN** the next computation reads the price files again and uses the new prices, so the window and `tokprof calendar` make the same fingerprint and do not read each other's logs again

### Requirement: Calendar in a frame without a window
`--scene calendar` SHALL draw the frame in Calendar, waiting up to 120 seconds for it, and `--scene calendar-day` the same with the cursor a week before the last day and its tooltip. `--metric <name>` picks the metric by the names of `tokprof calendar --metric`.

#### Scenario: A frame of the calendar
- **WHEN** the user runs `tokprof-desk --snap out.png --scene calendar --metric keyboard`
- **THEN** `out.png` shows the calendar of the time at the keyboard

#### Scenario: An unknown metric
- **WHEN** the user runs `tokprof-desk --snap out.png --scene calendar --metric tokens`
- **THEN** the command fails, naming the metrics

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
- **THEN** the day has no `Agents at work` lane, and the `agents` line stays under the lanes

#### Scenario: A few agents
- **WHEN** at most two agents worked at once
- **THEN** the area reaches half of the lane's height, so that days compare when stepping with ← and →

#### Scenario: Not a session
- **WHEN** the user clicks the `Agents at work` lane
- **THEN** the selection stays where it was

### Requirement: Room for the lanes
The lanes and what is under them SHALL take 45% of the window's height under the bar, or more, up to 55%, when that shows fewer session lanes than 8 in a window 900 points high and 3 in one 640 high, and in between as many as a straight line between them gives. Lanes are 24 points high, and the day's lines under them 15.

#### Scenario: A busy day
- **WHEN** the window is 1400 by 900 and the day of 2026-11-01 has 40 sessions
- **THEN** 8 session lanes show whole under `You` and `Agents at work`, and the line under them says how many more scroll

#### Scenario: The smallest window
- **WHEN** the window is 960 by 640 on the same day
- **THEN** 3 session lanes show whole

### Requirement: Month in review in the window
Calendar SHALL offer `Month in review` as a button after the metric pills, as the key `r` and as a palette entry. Each opens the card of month-review over the window dimmed and blurred: for the month of a cell cursor a key moved, else for the last complete month, as `tokprof month`. ← and → show the month before and after, never past this month. esc, `Close` or a click outside close it. While it is open, every key but ⌘Q is its own.

#### Scenario: Opening the card
- **WHEN** Calendar shows the last day of October under the cursor and the user presses `r`
- **THEN** the card of October 2026 opens over Calendar

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
`--scene month` SHALL draw Calendar with the card open, and `--scene month-card` the card alone, filling the frame. Both take the month from `--month YYYY-MM`, else the month of the last day. `--month` takes only YYYY-MM.

#### Scenario: The image of October
- **WHEN** the user runs `tokprof-desk --snap out.png --scene month-card --month 2026-10 --size 540x675`
- **THEN** `out.png` is the card of October, 1080 by 1350 pixels

#### Scenario: A bad month
- **WHEN** the user runs `tokprof-desk --snap out.png --scene month --month october`
- **THEN** the command fails with `--month takes YYYY-MM`

### Requirement: Key numbers against the period before
Under each key number, Reports SHALL compare it with the period of the same length just before the one shown, from the day store (see activity-calendar). The comparison reads `<arrow> <change> (<percent>)`, or `= <zero>` when the number did not change, and the card's title notes `against <first day> – <last day>`. The avoidable dollars, their share of the spend in points, and the cold returns are green when they fall and red when they rise. Spend, sessions and requests have no colour.

#### Scenario: More spend
- **WHEN** on 9 Nov the last 30 days cost $12,203 and the 30 days before them cost $11,000
- **THEN** the spend reads `↑ $1,203 (+11%)` in the text's colour, and the card notes `against 10 Sep – 10 Oct 2026`: the period before ends at the hour the period shown starts

#### Scenario: Fewer misses
- **WHEN** the avoidable dollars fell from $640 to $597, and their share from 12.5% to 11.3% of spend
- **THEN** the line under them reads `↓ $43.00 (−7%) · ↓ 1.2 points` in green

#### Scenario: More cold returns
- **WHEN** the cold returns rose from 90 to 106
- **THEN** their line reads `↑ 16 (+18%)` in red, comparing the count only

#### Scenario: No earlier data
- **WHEN** the store holds no request, miss or cold return in the period before
- **THEN** no number has a comparison, and the card's title has the note `no earlier data`

#### Scenario: A client filter
- **WHEN** the window runs with `--client`
- **THEN** no number has a comparison, and the note reads `no comparison with --client`

### Requirement: Causes against the period before
Each row of `Could have been saved` SHALL give, after its dollars, the cause's avoidable dollars in the period before and the change in percent: `was <dollars> · <arrow> <percent>`. A cause with no dollars before reads `new`. The change is green when it falls and red when it rises. With no earlier data, or with `--client`, the rows have no comparison.

#### Scenario: Fewer idle misses
- **WHEN** `idle > TTL` could have saved $796, and $1,012 in the period before
- **THEN** the row reads `idle > TTL` `$796` `was $1,012 · ↓ 21%`, the change in green

#### Scenario: A new cause
- **WHEN** `tools changed` could have saved $12 and had nothing in the period before
- **THEN** its row reads `new`

### Requirement: An incomplete period before
The comparison SHALL be incomplete when an agent with requests in the period shown, and at least 10% of its spend, has its first request in the whole store after the period before starts, as when the agent deleted its older logs before the store kept them. The project and session filters do not move that first request. Then the card notes `against <dates>, incomplete: <agent> only from <day>`, and every change of the key numbers and causes is grey.

#### Scenario: Logs deleted before the store
- **WHEN** Reports shows 10 Oct – 9 Nov, and the store's first Claude Code request is on 29 Sep
- **THEN** the card notes `against 10 Sep – 10 Oct 2026, incomplete: claude only from 29 Sep`, and `↑ $6,779 (+128%)` and the other changes are grey

#### Scenario: A complete period before
- **WHEN** Reports shows 7 days from 2 Nov, and every agent's data in the store starts before 26 Oct
- **THEN** the card notes only `against 26 Oct – 2 Nov 2026`, and the changes are coloured

#### Scenario: A new project
- **WHEN** Reports shows 7 days with `--project ~/src/new`, the project's first request is on 1 Nov, and the store's first Claude Code request is on 29 Sep
- **THEN** the comparison is complete and coloured

### Requirement: A small agent in the period before
An agent whose data starts after the period before starts, but that has less than 10% of the period shown's spend (of its requests, when nothing is priced), SHALL NOT make the comparison incomplete. The card names it after the dates: `; <agent> only from <day>, <share>% of spend, not judged`.

#### Scenario: Trying a new agent
- **WHEN** Codex's first request in the store is on 5 Nov and it has 3% of the last 7 days' spend
- **THEN** the card notes `against 26 Oct – 2 Nov 2026; codex only from 5 Nov, 3% of spend, not judged`, and the changes stay coloured

### Requirement: A week of Reports in a frame without a window
`--scene reports-week` SHALL draw the frame in Reports on the last 7 days, waiting up to 120 seconds for the report and for the day store, as `--scene reports` does for 30 days.

#### Scenario: The frame
- **WHEN** the user runs `tokprof-desk --snap out.png --scene reports-week`
- **THEN** `out.png` shows Reports on the last 7 days, compared with the 7 days before

### Requirement: Causes gone since the period before
After its rows, `Could have been saved` SHALL list each miss cause that had avoidable dollars in the period before and none in the period shown, most first. Such a row has `$0.00`, no share bar, nothing to open, and reads `was <dollars> · gone`: green, or grey when the comparison is incomplete.

#### Scenario: Compaction calls stopped
- **WHEN** compaction calls could have saved $367 in the 7 days before and nothing in the last 7 days
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
