## ADDED Requirements

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
A week in progress SHALL compare with the same point of last week: from last week's Monday 00:00 to seven days before now. A full week compares with the whole week before it. The rules of An incomplete period before and A small agent in the period before apply. There is no comparison when the week before has no request and no message; then Pace per day applies.

#### Scenario: Thursday afternoon
- **WHEN** it is Thursday 12 Nov 2026 at 14:20
- **THEN** this week compares with Mon 2 Nov 00:00 to Thu 5 Nov 14:20, and the notes read `against 2–5 Nov, to 14:20`

#### Scenario: A full week
- **WHEN** Week shows 2–8 Nov 2026
- **THEN** it compares with 26 Oct – 1 Nov, and the notes read `against 26 Oct – 1 Nov`

#### Scenario: No earlier data
- **WHEN** Week shows 2–8 Mar 2026 and the store holds no request and no message in 23 Feb – 1 Mar
- **THEN** no number has a comparison, the notes read `no logs in 23 Feb – 1 Mar`, and each tile shows its pace per day

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
- **THEN** Tuesday's row reads `$675`, `12.5h`, `7.3h` and `8 cold`, as Calendar's tooltip of 3 Nov and `tokprof day 2026-11-03` give them

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
Week's numbers from the day store SHALL be computed on a worker, which refreshes the store as Calendar computation does. No frame waits for it. While it runs, the days read `reading the logs into the store… <n> of <m> logs`. While Week is not shown, nothing is computed for it.

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
In Week, ⌘Q, ⌘W, ⌘,, ⌘K, `?`, Ctrl+C, ⌘0 and ⌘1 to ⌘4 SHALL act as in Day, and `/` starts the filter. esc gives the keyboard back to the days, then clears the filter, then shows Day. `q` and the other keys do nothing.

#### Scenario: q in Week
- **WHEN** `q` is pressed while Week is shown
- **THEN** the window stays open

#### Scenario: Esc in steps
- **WHEN** By project has the keyboard, a filter is set, and the user presses esc three times
- **THEN** the first gives the keyboard back to the days, the second clears the filter, and the third shows Day

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
When Week's numbers have no comparison (no earlier data, an incomplete period before, or `--since` or `--until` cutting the week), each tile SHALL show under it its pace per day instead: its number over the days the week has run, from the later of its Monday and the day of the store's first request, today counting as a whole day. The notes say why there is no comparison.

#### Scenario: A first week
- **WHEN** the store's first request is on Wed 4 Mar 2026 and Week shows 2–8 Mar 2026 with $700 spent
- **THEN** the spend tile reads `$700` with `$140 a day` under it, over the 5 days from 4 Mar

#### Scenario: Within --since
- **WHEN** the window was started with `--since 2026-11-04` and Week shows 2–8 Nov 2026
- **THEN** the week counts from 4 November, the notes read `within --since 2026-11-04`, and no number has a comparison

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

## MODIFIED Requirements

### Requirement: Reports view
The bar SHALL hold a `Day | Week | Reports | Calendar` switch, and right of it, apart from it, the `Sessions` button. ⌘1 shows Day, ⌘2 Week, ⌘3 Reports, ⌘4 Calendar and ⌘0 Sessions (see desk-sessions). Reports replaces what Day shows under the bar with the report of a period in one scrolling pane, the cards of the Report cards requirement. The window starts in Day.

#### Scenario: Switching
- **WHEN** the user clicks `Reports` in the bar, or presses ⌘3
- **THEN** the lanes and the details pane give way to the report, and `Reports` is marked in the switch

#### Scenario: Back to the day
- **WHEN** Reports is shown and the user clicks `Day`, presses ⌘1 or presses esc
- **THEN** the window shows Day as it was: its day, its selection and its tab

#### Scenario: From the palette and the key help
- **WHEN** the user picks `Reports: the last 90 days` in the command palette
- **THEN** Reports shows the last 90 days; the palette also offers `Reports`, `Day`, `Week`, `Sessions` and the other period, and the key help lists ⌘0 to ⌘4 and the keys of Reports
### Requirement: Report period
Reports SHALL show pills `30 days` and `90 days`, `30 days` to start with. A period of N days counts the requests from N × 24 hours before its computation started, as `tokprof summary --since <N>d` run then, with the window's `--agent`, `--project`, `--session`, `--client` and `--until` filters. A `--since` later than the period's start is kept instead. A calendar week is Week's to show, not Reports'.

#### Scenario: Another period
- **WHEN** the user clicks `90 days`
- **THEN** the report shows the last 90 days, computed then or kept from before

#### Scenario: With a project filter
- **WHEN** the window was started with `--project zebrafish`
- **THEN** each period counts only the requests that `tokprof summary --project zebrafish --since <N>d` counts

#### Scenario: No week in Reports
- **WHEN** Reports is shown
- **THEN** it has no `7 days` pill, and the palette offers no `Reports: the last 7 days`

### Requirement: Report computation
A report SHALL be computed in the background; no frame waits for it. While it runs, Reports reads `computing the last <N> days… <n> of <m> logs`. Each period's report is kept. When the logs change while Reports shows a period, it is computed again once 4 times its last computation and at least 20 seconds passed since it arrived, with `updating…` by the period. While Day is shown, nothing is computed for reports.

#### Scenario: The first look
- **WHEN** the user switches to Reports for the first time
- **THEN** the pane reads `computing the last 30 days… 0 of 1937 logs`, the count grows while the window stays responsive, and the cards replace it when the report arrives

#### Scenario: Back to a period
- **WHEN** the user goes from `30 days` to `90 days` and back
- **THEN** the 30-day report shows at once, without being computed again

#### Scenario: At rest
- **WHEN** a report has arrived and the logs do not change
- **THEN** the window draws no more frames for it

### Requirement: Keys in Reports
In Reports, Tab and Shift+Tab SHALL move the keyboard through the period, by day, by project, by origin, by model, cold returns and largest context, ringed in the accent and scrolled into view. ← and → pick the period or move the day cursor; ↑ ↓ (`k`, `j`), Home and End move a row cursor; ⏎ opens the cursor's day or project as a click does. ⌘Q, ⌘W, ⌘,, ⌘K, `?`, Ctrl+C, ⌘0 and ⌘1 to ⌘4 act as in Day; esc shows Day; the other keys do nothing.

#### Scenario: A day by keys
- **WHEN** by day has the keyboard, and the user presses ← twice and ⏎
- **THEN** the window shows Day with the lanes of the date two days before the last of the period

#### Scenario: The period by keys
- **WHEN** the period has the keyboard on `30 days` and the user presses →
- **THEN** `90 days` is picked, and ← picks `30 days` again

#### Scenario: Sessions from Reports
- **WHEN** Reports is shown and the user presses ⌘0
- **THEN** the window shows Sessions
### Requirement: Day controls
In Day, the bar SHALL hold the shown day's name with buttons to the day before and after, a Today button, the filter, and a `Live` mark while the window follows the logs; the day's caption and the zoom are in the lanes' header. Sessions shows `All sessions` with the date of the first log and the count of listed sessions in place of the day's name and its buttons, and Week its own controls with the filter (see Week in the bar). Reports and Calendar show none of them and no filter.

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

#### Scenario: In Week
- **WHEN** Week is shown
- **THEN** the bar holds the switch, the `Sessions` button, the week's name with its buttons, `Today`, the week's summary, the filter, the status and `Live`, and no day controls and no zoom
### Requirement: Calendar view
⌘4 or `Calendar` in the bar's switch SHALL show Calendar: in one scrolling pane under the bar, the metric as pills, the calendar of activity-calendar drawn large, its legend and its numbers, and its punchcard, in rounded cards as in Reports. It counts what `tokprof calendar --metric <metric>` counts with the window's `--agent`, `--project`, `--session`, `--since` and `--until`. esc, ⌘1 or `Day` show Day as it was.

#### Scenario: Switching
- **WHEN** the user presses ⌘4
- **THEN** the lanes and the details pane give way to the calendar, and `Calendar` is marked in the switch

#### Scenario: The same numbers as the CLI
- **WHEN** Calendar shows spend and the user runs `tokprof calendar` with the same filters at the same time
- **THEN** every cell's level, the cuts, the total, active days, streaks, the busiest day and weekday, and the punchcard's levels are the same

#### Scenario: From the palette and the key help
- **WHEN** the user picks `Calendar: messages` in the command palette
- **THEN** Calendar shows messages; the palette also offers `Calendar` and each other metric, and the key help lists ⌘4 and the keys of Calendar
### Requirement: Keys in Calendar
In Calendar, a cursor SHALL ring one day, the last day to start with. ↑ and ↓ (`k`, `j`) move it a day back and forward, ← and → (`h`, `l`) a week; Home and End go to the first and the last day; it never leaves the days shown. The day under the cursor shows its tooltip, and ⏎ opens it as a click does. `m` picks the next metric and ⇧M the one before. ⌘Q, ⌘W, ⌘,, ⌘K, `?`, Ctrl+C, ⌘0 and ⌘1 to ⌘4 act as in Day; esc shows Day; the other keys do nothing.

#### Scenario: A day by keys
- **WHEN** the cursor is on the last day and the user presses ← and ⏎
- **THEN** the window shows Day with the lanes of the date a week before the last day

#### Scenario: The metric by keys
- **WHEN** Calendar shows spend and the user presses `m` twice
- **THEN** it shows the time at the keyboard

#### Scenario: Sessions from Calendar
- **WHEN** Calendar is shown and the user presses ⌘0
- **THEN** the window shows Sessions
### Requirement: Key help
`?` SHALL open the key help when no filter is being typed and no overlay is open: a rounded card over the window dimmed and blurred, listing every key of the window grouped under `Anywhere`, `Lanes`, `The day`, `Palette`, `Settings`, `Sessions`, `Inspector`, `Flame`, `Week`, `Reports` and `Calendar`, each key drawn as a keycap with what it does. esc or `?` close it, and no other key reaches the window behind it, but ⌘Q quits. The palette's `Key help` opens it too.

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
### Requirement: An incomplete period before
The comparison SHALL be incomplete when an agent with requests in the period shown, and at least 10% of its spend, has its first request in the whole store after the period before starts, as when the agent deleted its older logs before the store kept them. The project and session filters do not move that first request. Reports then notes `against <dates>, incomplete: <agent> only from <day>` and greys every change; Week shows the pace per day instead (see Pace per day).

#### Scenario: Logs deleted before the store
- **WHEN** Reports shows 10 Oct – 9 Nov, and the store's first Claude Code request is on 29 Sep
- **THEN** the card notes `against 10 Sep – 10 Oct 2026, incomplete: claude only from 29 Sep`, and `↑ $6,779 (+128%)` and the other changes are grey

#### Scenario: A complete period before
- **WHEN** Week shows 2–8 Nov 2026, and every agent's data in the store starts before 26 Oct
- **THEN** the notes read only `against 26 Oct – 1 Nov`, and the changes are coloured

#### Scenario: A new project
- **WHEN** Week shows 2–8 Nov 2026 with `--project ~/src/new`, the project's first request is on 1 Nov, and the store's first Claude Code request is on 29 Sep
- **THEN** the comparison is complete and coloured

#### Scenario: Logs deleted before the store, in Week
- **WHEN** Week shows 28 Sep – 4 Oct 2026, the store's first Claude Code request is on 29 Sep, and claude has 10% or more of the week's spend
- **THEN** its notes read `no comparison with 21–27 Sep: claude's logs start 29 Sep`, no number has a change, and each tile shows its pace per day

### Requirement: A small agent in the period before
An agent whose data starts after the period before starts, but that has less than 10% of the period shown's spend (of its requests, when nothing is priced), SHALL NOT make the comparison incomplete. The card names it after the dates: `; <agent> only from <day>, <share>% of spend, not judged`. Week's notes do the same.

#### Scenario: Trying a new agent
- **WHEN** OpenCode's first request in the store is on 29 Oct and it has 3% of the spend of 2–8 Nov 2026
- **THEN** Week's notes read `against 26 Oct – 1 Nov; opencode only from 29 Oct, 3% of spend, not judged`, and the changes stay coloured

### Requirement: Causes gone since the period before
After its rows, `Could have been saved` SHALL list each miss cause that had avoidable dollars in the period before and none in the period shown, most first. Such a row has `$0.00`, no share bar, nothing to open, and reads `was <dollars> · gone`: green, or grey when the comparison is incomplete.

#### Scenario: Compaction calls stopped
- **WHEN** compaction calls could have saved $367 in the 30 days before and nothing in the last 30 days
- **THEN** the last row reads `compaction call` `$0.00` `was $367 · gone` in green

## REMOVED Requirements

### Requirement: A week of Reports in a frame without a window
**Reason**: Reports no longer has a `7 days` period. The calendar week is Week's, so `--scene reports-week` has nothing to draw.
**Migration**: Use `--scene week`, with `--week <date>` for a given week (see Week in a frame without a window).
