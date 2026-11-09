# activity-calendar Specification

## Purpose
Shows months of work with the agents at a glance, as a calendar of days with streaks and a punchcard, from a day store that answers in a second and keeps the days of logs the agents have deleted.

## Requirements

### Requirement: Day store
tokprof SHALL keep a day store: one file `days-2.json` in its cache folder, `~/Library/Caches/tokprof/` on macOS, `%LOCALAPPDATA%\tokprof\` on Windows, else `$XDG_CACHE_HOME/tokprof/` or `~/.cache/tokprof/`; `$TOKPROF_CACHE_DIR` replaces the folder. The `2` is the store's format. The store holds an entry per log. It holds numbers, times, ids and project paths, and no text of a message, a prompt, a tool call or a title.

#### Scenario: The first run
- **WHEN** `tokprof calendar` runs and no store exists
- **THEN** it reads every log of the agents asked for, writes `~/Library/Caches/tokprof/days-2.json`, and prints the calendar

#### Scenario: Another format
- **WHEN** the cache folder holds a store of another format, such as `days-1.json`, which an earlier build wrote
- **THEN** that file is left as it is, and `days-2.json` is used or started anew

### Requirement: What an entry names
An entry SHALL name its log's path, length and modification time, the fingerprint it was made with, the log's agent, its session id and the id of its main session, whether it is a subagent's log, its project, and whether the rhythm leaves its session out (see work-rhythm, Sessions left out).

#### Scenario: A subagent's log
- **WHEN** a subagent of session `3f951de0` wrote a log
- **THEN** its entry names `3f951de0` as its main session and is marked as a subagent's log

### Requirement: What an entry counts
An entry SHALL count by hour of UTC, keeping the minute of each amount inside the hour: the spend and its avoidable part, the requests, and the commits, pushes and pull requests the agent made. An entry of a main log also keeps the minutes the agent worked, by hour of UTC, and the time of each of the user's actions, marked when it is a message.

#### Scenario: A request
- **WHEN** a request at 2026-11-03 09:41:20 UTC costs $0.42 and its cache miss had $0.30 of avoidable premium
- **THEN** the entry's hour 2026-11-03 09:00 UTC holds, at minute 41, one request, $0.42 and $0.30 avoidable

#### Scenario: Hours across a change of the clock
- **WHEN** the local clock goes back from 03:00 to 02:00 and requests come in both 02:30s
- **THEN** they are in two different hours of UTC, and neither is lost or counted twice

### Requirement: Amounts of the store
The spend of a request SHALL be its cost as `tokprof summary` counts it in `Total priced`; a request of a model without a price is a request with no spend. Avoidable spend is the premium of the cache misses whose cause is not expected, as the summary's `avoidable on cache misses`. A commit, a push or a pull request counts as `tokprof share` counts outcomes: made by a shell call that worked.

#### Scenario: A call that failed
- **WHEN** the agent runs `git commit` and the call fails
- **THEN** no commit is counted

#### Scenario: A model without a price
- **WHEN** a request's model has no price
- **THEN** it counts as a request, with no spend and no avoidable spend

### Requirement: Work and actions of the store
The agent SHALL count as working in a minute when some work of its main conversation (see work-rhythm, Work) overlaps that minute by more than nothing. The user's actions are those of work-rhythm, Actions and hand-offs, at the time the log gives them.

#### Scenario: Work across minutes
- **WHEN** the agent works from 10:00:40 to 10:02:10
- **THEN** minutes 10:00, 10:01 and 10:02 count as worked

### Requirement: When a log is read again
On each run, tokprof SHALL use an entry while its log has the same path, length and modification time and the entry's fingerprint is the current one. The fingerprint names the tokprof version, the git commit it was built from (the version alone for a build without git), and the prices in use. Every other log is read again, in parallel, and its entry replaced, or kept when the log cannot be read.

#### Scenario: Unchanged logs
- **WHEN** no log changed since the last run
- **THEN** no log is read, and the store is not written

#### Scenario: A log that grew
- **WHEN** the agent appended to one log since the last run
- **THEN** only that log is read again

#### Scenario: New prices
- **WHEN** the user changed `~/.config/tokprof/prices.toml`, or a new tokprof version runs
- **THEN** every log that still exists is read again

#### Scenario: A build from main
- **WHEN** the author installs tokprof from a newer commit with the same version number
- **THEN** every log that still exists is read again once, about 45 s on the author's logs, so that a fix to the numbers reaches the old days too

### Requirement: Logs that are gone
An entry whose log no longer exists SHALL stay in the store, whatever its fingerprint, and count in the calendar. When a log of the same agent with the same session id and main session id was read at another path, the entry of the old path is dropped; while that log cannot be read, the old entry stays. The calendar says how many of its days come only from logs that are gone.

#### Scenario: Claude Code deleted old logs
- **WHEN** Claude Code deleted the logs of September after 30 days, and the store has their entries
- **THEN** the calendar still shows September, and the `kept` line reads like `kept      28 days come only from the store: their logs are gone`

#### Scenario: A log moved
- **WHEN** an agent moved a log to its archive folder
- **THEN** its days count once

### Requirement: Writing the store
tokprof SHALL write the store only when an entry changed: to a new file in the cache folder, flushed to disk, then renamed over the old one, so that a reader sees the old store or the new one. The last of two writers wins. A failed write removes its new file and warns after the output. A store that cannot be parsed is kept as `days-2.json.broken-<UTC time>`, never over an earlier one, and started anew.

#### Scenario: A store that cannot be read
- **WHEN** `days-2.json` exists but cannot be read, as with no read permission
- **THEN** the run counts from an empty store in memory, warns that it does, and does not write over the file

#### Scenario: Two broken stores
- **WHEN** the store is broken on one run and again on a later one
- **THEN** both are kept, as `days-2.json.broken-20261109T011500Z` and a later name, and the warning names the file

#### Scenario: The CLI and the window at once
- **WHEN** `tokprof calendar` and `tokprof-desk` refresh the store at the same moment
- **THEN** both finish, and the store is one of the two they wrote

### Requirement: Calendar days
A moment SHALL belong to a day of work-rhythm, Start of the day, with the days found from the user's actions in the entries that the filters keep and that the rhythm does not leave out. After the day of the latest action, each date whose noon has passed starts at its midnight. Before the first action, days are local dates. An amount belongs to the day its minute starts in; an action to the day of its moment.

#### Scenario: A night
- **WHEN** the user acted last at 00:42 on 2026-11-05 and next at 09:09, and the day of 2026-11-05 starts at 04:55
- **THEN** a request at 03:10 on 2026-11-05 counts in the cell of 2026-11-04

#### Scenario: The same day as the Day view
- **WHEN** `tokprof day 2026-11-05` prints `sittings  14, 2.5h at the keyboard, 47 messages to 8 sessions`
- **THEN** the calendar's 2026-11-05 has 47 messages and 2.5h at the keyboard
- **AND** they can differ only when `tokprof day`, which reads only the logs around the day, finds another night: when a log it does not read holds the user's last action before a night that starts after 18:00 the day before

#### Scenario: Away for days
- **WHEN** the user acted last on Thursday at 22:00, agents worked on Friday and Saturday, and it is Saturday 15:00
- **THEN** Friday and Saturday start at their midnights, and the work is in their cells

### Requirement: Calendar metrics
`tokprof calendar` SHALL draw one metric, chosen with `--metric`, `spend` by default: `spend` and `avoidable` in dollars, `agent` the minutes some agent worked, `keyboard` the time at the keyboard, `messages` the messages the user typed, and `commits` the commits the agents made. `agent`, `keyboard` and `messages` count only the sessions the rhythm keeps.

#### Scenario: Two sessions at once
- **WHEN** two sessions' agents worked from 10:00 to 10:30 at the same time
- **THEN** the `agent` metric counts 30 minutes, each minute once

#### Scenario: Time at the keyboard
- **WHEN** a day's actions make sittings of 10m, 30s and 1h
- **THEN** its `keyboard` value is 1h 11m: the sum of its sittings, each at least a minute (see work-rhythm, Sittings and breaks)

#### Scenario: An unknown metric
- **WHEN** the user runs `tokprof calendar --metric tokens`
- **THEN** it fails, naming `spend`, `agent`, `keyboard`, `messages`, `commits` and `avoidable`

### Requirement: Calendar filters
The calendar SHALL count the entries whose log `tokprof summary` would read with the same `--agent`, `--project` and `--session`, and of them the minutes and the actions inside `--since` and `--until`, a minute by its start. `--client` fails with `calendar cannot filter by client; drop --client`.

#### Scenario: The same total as the summary
- **WHEN** the user runs `tokprof calendar --since 2026-10-09 --until 2026-11-08` and `tokprof summary --since 2026-10-09 --until 2026-11-08` on logs that were not deleted
- **THEN** the calendar's total equals `Total priced`, to the cent

#### Scenario: Requests without a time
- **WHEN** a log gives no time for 3 requests that cost $5
- **THEN** no day holds them, the grid is not stretched for them, and the numbers end with `untimed   3 requests without a time are in no day ($5.00)`

#### Scenario: A day cut by the window
- **WHEN** `--since 2026-10-09` is local midnight and the day of 2026-10-08 runs until 04:30 on 2026-10-09
- **THEN** the cell of 2026-10-08 holds only what came from midnight to 04:30

### Requirement: Calendar grid
`tokprof calendar` SHALL print a title `tokprof · calendar · <metric>`, then a grid: a column per week from Monday to Sunday and a row per weekday, labeled `Mon` to `Sun`. The last column is the week of the last day, the day that holds now or the day before `--until`. The first is the week of the first day with a request or an action, but at least 12 weeks before the last and at most 52. Days after the last day are blank.

#### Scenario: Weeks shown
- **WHEN** the first data is on 2026-08-01, a Saturday, and today is Sunday 2026-11-08
- **THEN** the grid has 15 columns, from the week of Monday 2026-07-27 to the week of Monday 2026-11-02

#### Scenario: Little data
- **WHEN** the first data is three weeks ago
- **THEN** the grid still has 13 columns, the first ten all at level none

#### Scenario: A year and more
- **WHEN** the store holds 70 weeks of data
- **THEN** the grid has 53 columns, and the earlier weeks count in nothing the view prints

### Requirement: Grid labels and width
A column whose Monday is the first Monday of its month SHALL carry the month's short name above it, unless the name would touch the one before. A grid of at most 38 columns gives each week 2 characters, its mark and a space; a wider one gives each week 1.

#### Scenario: Months
- **WHEN** the grid starts on Monday 2026-07-27
- **THEN** `Aug` stands over the column of 2026-08-03, `Sep` over 2026-09-07, `Oct` over 2026-10-05 and `Nov` over 2026-11-02

### Requirement: Calendar levels
Each day SHALL be drawn at one of five levels: none for a day whose metric is zero, else 1 to 4 by the quartiles of the non-zero days shown. With those values in ascending order v1…vn, the cuts are v at ranks ⌈n/4⌉, ⌈n/2⌉ and ⌈3n/4⌉; a day at or below the first cut is level 1, at or below the second level 2, at or below the third level 3, else level 4.

#### Scenario: Cuts
- **WHEN** the non-zero days are $1, $2, $3, $4, $5, $6, $7 and $8
- **THEN** the cuts are $2, $4 and $6, $5 is level 3 and $8 level 4

#### Scenario: One day
- **WHEN** only one day has a non-zero value
- **THEN** it is level 1

### Requirement: Marks and legend
The levels SHALL be drawn as `·`, `░`, `▒`, `▓` and `█`. In a terminal that takes color, level none is dim and levels 1 to 4 are greens from dark to bright; without color the marks are the same and no escape code is written. A legend follows the grid with the cuts in the metric's unit, whole dollars from $10 up.

#### Scenario: The legend
- **WHEN** the cuts are $2, $4 and $6
- **THEN** the legend reads `less · ░ ▒ ▓ █ more   ░ up to $2.00  ▒ up to $4.00  ▓ up to $6.00  █ more`

### Requirement: Calendar numbers
Below the legend, the calendar SHALL print `total`, `active`, `streaks`, `busiest`, `weekday` and `kept` over the days shown: from the first day with a request or an action, or the first column's Monday when that is later, to the last day. `active` and `streaks` count the days whose metric is above zero; the current streak ends at the last day, or at the day before when the last day has none. `kept` counts the days whose every amount and action comes from logs that are gone.

#### Scenario: The lines
- **WHEN** the calendar of spend shows 98 days from 2026-08-03 to 2026-11-08
- **THEN** its lines read like
  `total     $11,603 from Mon 3 Aug to Sun 8 Nov 2026, 98 days`
  `active    87 of 98 days with spend`
  `streaks   of days with spend: current 34 days, longest 41 days (Mon 7 Sep – Sat 17 Oct)`
  `busiest   Tue 14 Oct, $812`
  `weekday   Tuesdays, $2,431`

#### Scenario: No day from the store alone
- **WHEN** no day shown comes only from logs that are gone
- **THEN** the last line reads `kept      no day comes only from the store yet; it keeps the days of logs the agents delete`

#### Scenario: Units
- **WHEN** the metric is `keyboard` or `agent`
- **THEN** its amounts read as hours with one decimal, as `142.5h`; `messages` and `commits` as whole numbers with their word, as `828 commits`

#### Scenario: Commits
- **WHEN** the calendar of commits has commits on 46 of 95 days, the latest 8 days in a row
- **THEN** it reads `active    46 of 95 days with commits` and `streaks   of days with commits: current 8 days, …`, whatever the days with a request

#### Scenario: Nothing
- **WHEN** the filters keep no request and no action
- **THEN** the calendar prints `No requests and no messages in this time.` after its title

### Requirement: Punchcard
After the numbers, the calendar SHALL print a punchcard of the same metric over the days shown: a row per weekday, `Mon` to `Sun`, and a column per hour of the local clock, `00` to `23`, labeled every 3 hours, each 2 characters wide, with the marks of the grid and levels computed over its non-zero cells. Weekday and hour are the local clock's at each minute, not the calendar day's.

#### Scenario: A late evening
- **WHEN** most of the spend came on Tuesdays from 22:00 to 23:00
- **THEN** the cell of `Tue` and `22` is level 4

#### Scenario: A sitting across hours
- **WHEN** a sitting runs from 09:50 to 10:20, and another of 20 seconds starts at 11:00
- **THEN** the keyboard punchcard counts 10 minutes at 09, 20 at 10 and 1 at 11

### Requirement: Calendar speed
On logs that did not change since the last run, `tokprof calendar` SHALL read no log and answer in about a second. It prints to stderr what it read: the agents' logs read again, as other commands name them, the logs taken from the store, and the logs that are gone.

#### Scenario: The second run
- **WHEN** `tokprof calendar` runs twice and no log changed in between
- **THEN** the second run reads no log and its stderr line reads like `read no log in 0.3s; 1,958 logs from the store, 31 gone from disk (~/Library/Caches/tokprof/days-2.json)`
