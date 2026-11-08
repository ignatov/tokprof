# Spec Delta

## ADDED Requirements

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
The grid SHALL draw each day as a rounded square, a column per week and a row per weekday, labeled `Mon`, `Wed` and `Fri`, with the month labels of the CLI above. Levels 1 to 4 are the accent mixed into the card's background at 30%, 55%, 80% and 100%, and level none is the theme's subtle stroke. The legend reads `less`, the five squares and `more`, with the cuts.

#### Scenario: A dark theme
- **WHEN** the window is drawn in Merino Dark
- **THEN** the squares go from the subtle stroke through the accent mixed into the dark card to the accent

#### Scenario: Metric pills
- **WHEN** Calendar opens
- **THEN** it shows the pills `Spend`, `Agent time`, `At the keyboard`, `Messages`, `Commits` and `Avoidable`, with `Spend` picked

#### Scenario: Another metric
- **WHEN** the user clicks `Messages`
- **THEN** the grid, the legend, the numbers and the punchcard show messages, without the store being read again

#### Scenario: Today
- **WHEN** the grid is drawn
- **THEN** the last day's square is ringed in the theme's ink

#### Scenario: Kept by the store
- **WHEN** the calendar is drawn
- **THEN** beside the grid, or under it in a window too narrow for both, the numbers end with `Kept by the store`: the days that come only from logs that are gone, or `no day yet`

### Requirement: A day in the calendar
The pointer over a day's square SHALL show a tooltip beside it with the day's date and weekday and every metric of that day: spend, avoidable, requests, sessions with a request, agent time, time at the keyboard with its sittings, messages, and commits, pushes and pull requests; and whether the day comes only or partly from logs that are gone. A click on a square shows Day with the lanes of that day. A day after the last day has no square.

#### Scenario: Hovering a day
- **WHEN** the pointer rests over the square of 5 November
- **THEN** a tooltip reads like `Thu 5 Nov 2026`, `$214 spent, $31.00 of it avoidable`, `1,820 requests in 8 sessions`, `agents worked 6.2h`, `2.5h at the keyboard in 14 sittings, 47 messages`, `3 commits, 1 push, no pull request`

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
The calendar SHALL be computed on a worker; no frame waits for it. It refreshes the day store (see activity-calendar) and computes every metric at once. While the store is being filled it reads `reading the logs into the store… <n> of <m> logs`. When the logs change while Calendar is shown, it is computed again with the back-off of Report computation, with `updating…` by the pills. While Calendar is not shown, nothing is computed for it.

#### Scenario: The first look
- **WHEN** the user opens Calendar for the first time with no store on disk
- **THEN** the pane reads `reading the logs into the store… 0 of 1958 logs`, the count grows while the window stays responsive, and the calendar replaces it when it arrives

#### Scenario: At rest
- **WHEN** the calendar has arrived and the logs do not change
- **THEN** the window draws no more frames for it

### Requirement: Calendar in a frame without a window
`--scene calendar` SHALL draw the frame in Calendar, waiting up to 120 seconds for it, and `--scene calendar-day` the same with the cursor a week before the last day and its tooltip. `--metric <name>` picks the metric by the names of `tokprof calendar --metric`.

#### Scenario: A frame of the calendar
- **WHEN** the user runs `tokprof-desk --snap out.png --scene calendar --metric keyboard`
- **THEN** `out.png` shows the calendar of the time at the keyboard

#### Scenario: An unknown metric
- **WHEN** the user runs `tokprof-desk --snap out.png --scene calendar --metric tokens`
- **THEN** the command fails, naming the metrics

## MODIFIED Requirements

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

### Requirement: Keys in Reports
In Reports, Tab and Shift+Tab SHALL move the keyboard through the period, by day, by project, by origin, by model, cold returns and largest context, ringed in the accent and scrolled into view. ← and → pick the period or move the day cursor; ↑ ↓ (`k`, `j`), Home and End move a row cursor; ⏎ opens the cursor's day or project as a click does. ⌘Q, ⌘W, ⌘,, ⌘K, `?`, Ctrl+C, ⌘1, ⌘2 and ⌘3 act as in Day; esc shows Day; the other keys do nothing.

#### Scenario: A day by keys
- **WHEN** by day has the keyboard, and the user presses ← twice and ⏎
- **THEN** the window shows Day with the lanes of the date two days before the last of the period

#### Scenario: The period by keys
- **WHEN** the period has the keyboard and the user presses ←
- **THEN** `7 days` is picked

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
