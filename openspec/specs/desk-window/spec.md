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
The bar SHALL hold the shown day's name with buttons to the day before and after, a Today button, the day's summary (when it started, its sittings and its sessions), the lanes' zoom (`Fit` or `24 h`), the filter, and a `Live` mark while the window follows the logs. In Reports the day's name, its buttons, Today, the summary, the zoom and the filter, which filters the list, are not shown.

#### Scenario: Other days
- **WHEN** the user clicks the button before the day's name
- **THEN** the lanes show the day before, and Today goes back to the day of the latest action

#### Scenario: Never past the latest day
- **WHEN** the lanes show the day of the latest action and the user clicks the button after its name
- **THEN** the lanes stay on that day

#### Scenario: In Reports
- **WHEN** Reports is shown
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
Under the lanes SHALL be the legend, and the `sittings`, `answers`, `queue` and `cache` lines of the day as `tokprof day` prints them. A line under them names what is under the pointer: the time, the lane and what it shows there.

#### Scenario: The time under the pointer
- **WHEN** the pointer rests over a lane at 14:32
- **THEN** the line reads like `14:32 · ecad11 Инфографика прерываний · agent working`, with `agent working`, `waiting for your answer`, `your message`, `its question`, `you stopped it`, `you were there`, `cache warm` or nothing

#### Scenario: The queue line
- **WHEN** the shown day had 24 answers among several sessions waiting (see work-rhythm)
- **THEN** the `queue` line under `answers` reads as `tokprof day` prints it for that day

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
While the inspector has the keyboard, ← and → (and `h`, `l`) SHALL show the tab before or after outside `Flame`, and Shift+← and Shift+→ in every tab; the digits 1 to 9 show the tab of that place, and 0 `Flame`. In a tab with rows (requests, the bars of spend, work and waits, prompts, tools, the context's tables, misses), ↑ and ↓ (and `k`, `j`) move a cursor by a row, PgUp and PgDn by ten, Home and End to the ends, and the inspector scrolls to show it. In `Summary` they scroll it.

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
The tabs from `Requests` to `Cache` SHALL show session-xray's blocks of those names: the requests, the spend by component and by chain, work by origin and by activity, waits and slow requests, the prompts, the tool table, what the context holds and what cost the most to keep in it, and the cache misses. A click on a tab shows it.

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
`?` SHALL open the key help when no filter is being typed and no overlay is open: a rounded card over the window dimmed and blurred, listing every key of the window grouped under `Anywhere`, `List`, `Lanes`, `Inspector`, `Settings` and `Palette`, each key drawn as a keycap with what it does. esc or `?` close it, and no other key reaches the window behind it, but ⌘Q quits. The palette's `Key help` opens it too.

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
The bar SHALL hold a `Day | Reports` switch left of the day's name, and ⌘1 shows Day, ⌘2 Reports. Reports replaces the lanes, the list and the inspector under the bar with the report of a period in one scrolling pane, the cards of the Report cards requirement. The window starts in Day.

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
In Reports, Tab and Shift+Tab SHALL move the keyboard through the period, by day, by project, by origin, by model, cold returns and largest context, ringed in the accent and scrolled into view. ← and → pick the period or move the day cursor; ↑ ↓ (`k`, `j`), Home and End move a row cursor; ⏎ opens the cursor's day or project as a click does. ⌘Q, ⌘W, ⌘,, ⌘K, `?`, Ctrl+C, ⌘1 and ⌘2 act as in Day; esc shows Day; the other keys do nothing.

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
