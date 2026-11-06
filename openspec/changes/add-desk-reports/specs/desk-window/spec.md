# Spec Delta

## ADDED Requirements

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

## MODIFIED Requirements

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
In Day, the window SHALL show, under the bar, the lanes of the shown day across its whole width, at most 45% of its height, and under them the list of sessions in a pane 380 points wide and the inspector of the selected session in the rest. Every text is in the system font; numbers that line up in columns use tabular figures.

#### Scenario: A narrow window
- **WHEN** the window is 960 points wide
- **THEN** the list keeps its 380 points and the inspector takes the remaining 580

#### Scenario: Many lanes
- **WHEN** the day has more lanes than fit in 45% of the window
- **THEN** the lanes scroll under an hours row that stays on top, and the list and the inspector keep their place

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
