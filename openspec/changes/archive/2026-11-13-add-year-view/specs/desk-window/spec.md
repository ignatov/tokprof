# Spec Delta

## RENAMED Requirements

- FROM: `### Requirement: Calendar view`
- TO: `### Requirement: Year view`

- FROM: `### Requirement: Calendar grid in the window`
- TO: `### Requirement: Year grid in the window`

- FROM: `### Requirement: A day in the calendar`
- TO: `### Requirement: A day in the year`

- FROM: `### Requirement: Keys in Calendar`
- TO: `### Requirement: Keys in Year`

- FROM: `### Requirement: Calendar computation`
- TO: `### Requirement: Year computation`

- FROM: `### Requirement: Calendar in a frame without a window`
- TO: `### Requirement: Year in a frame without a window`

## MODIFIED Requirements

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

### Requirement: Week computation
Week's numbers from the day store SHALL be computed on a worker, which refreshes the store as Year computation does. No frame waits for it. While it runs, the days read `reading the logs into the store… <n> of <m> logs`. While Week is not shown, nothing is computed for it.

#### Scenario: The first look
- **WHEN** the user opens Week and the store is up to date
- **THEN** the strips without the waits, the numbers, the totals, the tiles, the line for next week and By project show within a second; the cold returns' dollars come with the week's logs

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

### Requirement: Sources of the period's cards
The key numbers, their comparison, the days of the month, spend by day, the weeks, months, trends and shifts of the quarter, by project and the records but the largest prompt SHALL come from the day store, refreshed as Year computation refreshes it, and show without reading the logs again. By origin, by model, the pause tables, what to change and the largest prompt read the period's logs. With `--client`, the store's numbers count every client, and the band says so.

#### Scenario: Unchanged logs
- **WHEN** no log changed since the store was last written and the user steps to October
- **THEN** the key numbers, the grid and the day chart show within a second, before the log cards

#### Scenario: A client filter
- **WHEN** the window runs with `--client sdk-ts`
- **THEN** the band reads `the key numbers and charts count every client: the day store has no client`, and by origin, by model, the pause tables and what to change count only `sdk-ts`

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

## ADDED Requirements

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
