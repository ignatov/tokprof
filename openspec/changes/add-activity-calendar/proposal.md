# Proposal

## Why

tokprof answers about one session, one day or one period, but not about the shape of months of work: which days were heavy, which were off, how long the runs of active days are, at which hours of the week the money goes. GitHub, Devin and Claude Code's `/stats` show this as a heatmap of days, and the author wants to try it here before it goes into JetBrains Air.

Two facts shape it. Reading all the author's logs since 2026-08-01 (Claude Code 1,159 files, 6.6 GB; Codex 799 files, 2.6 GB) takes 66 s under load, so a calendar cannot read every log each time. And the agents delete their logs (Claude Code after `cleanupPeriodDays`, 30 days by default), while the Air order asks for daily numbers kept beyond the logs.

## What Changes

- **A day store**: tokprof keeps, per log file, what each UTC hour of it holds (spend, avoidable spend, requests, commits, pushes and pull requests by agents, the minutes the agent worked, the user's actions), in one versioned file in the platform's cache folder (`~/Library/Caches/tokprof/days-1.json` on macOS). A log is read again only when its length, its modification time, the tokprof version or the prices change. A log that is gone keeps its numbers, so the calendar shows days whose logs the agents already deleted, and says how many. The store holds numbers, ids and project paths, no text of any prompt.
- **`tokprof calendar`**: a grid of weeks (columns) by weekdays (rows, Monday first) with month labels, from the first week with data (13 weeks at least, 53 at most) to the current week. Each day is drawn in one of 5 levels, none and the 4 quartiles of the days that have any, in color in a terminal and as distinct characters without it. `--metric spend|agent|keyboard|messages|commits|avoidable`, spend by default. Below it: the total, active days of all days, the current and the longest streak, the busiest day and the busiest weekday; then a weekday × hour punchcard of the same metric. `--agent`, `--project`, `--session`, `--since` and `--until` work as for `tokprof summary`. On unchanged logs it answers in about a second.
- **Days as the Day view has them**: a cell is a work-rhythm day, which starts in the middle of the night before it, found from the user's actions kept in the store. So a cell's messages, sittings and time at the keyboard are those `tokprof day <date>` prints, and a click on a cell in the window opens that day.
- **A Calendar view in `tokprof-desk`**: a third view beside Day and Reports (`Day | Reports | Calendar`, ⌘3, the command palette and the key help). The grid drawn large with month labels, the metric as pills (`m` cycles), the legend `less ■■■■■ more`, the numbers of the CLI beside it, the punchcard below. Hovering a cell shows its date and every metric of the day; arrow keys move a cell cursor; ⏎ or a click opens the day in Day. The colors are a ramp from the theme's background to its accent. It is computed on a worker from the store, shows progress while the store is filled the first time, and is computed again after the logs change with the back-off of Reports. `--snap --scene calendar --metric <name>` draws it.

## Capabilities

### New Capabilities
- `activity-calendar`: the day store (what it keeps, where, when a log is read again, deleted logs), the calendar's days and metrics, `tokprof calendar` with its grid, levels, numbers and punchcard.

### Modified Capabilities
- `desk-window`: the Calendar view, its computation, keys, hover, clicks and frame; the bar's switch gets a third segment, the day controls are hidden in Calendar too, ⌘3 works in Reports, and the key help lists its Calendar group (and the Flame and Reports groups it already shows).

## Impact

- `crates/tokprof-core`: a module that turns one parsed log into its store entry (a sink over the analysis plus the rhythm's lane), the store's file format with its fingerprint, the refresh that reads only changed logs, and the calendar computed from the entries.
- `crates/tokprof-browse`: the store's path in the cache folder, the calendar's text, and the window's calendar worker with the back-off of Reports.
- `crates/tokprof`: the `calendar` command.
- `crates/tokprof-desk`: `src/calendar.rs` with the view and its keys, hooks in the bar, the palette and the key help, the scene; README.
- No change to the parse or to any other command's output.

## How it is checked on the real logs

- Spend: `tokprof calendar --since 2026-10-09 --until 2026-11-08` prints a total equal to `Total priced` of `tokprof summary --since 2026-10-09 --until 2026-11-08`, to the cent; the same for avoidable. Any difference is explained in tasks.md.
- A day: for three dates, the cell's messages and time at the keyboard equal the `sittings` line of `tokprof day <date>`.
- Speed: the first run of `tokprof calendar` (empty store) and the second (unchanged logs) are timed; the second takes about a second. `tokprof summary --since 2026-08-01` takes as long as before.
- Frames: `--scene calendar` in light and dark, looked at.

## Out of scope

- Sharing or exporting the calendar as an image or a file.
- Comparing people, or calendars in share files and the pool.
- The port to JetBrains Air.
- A `--client` filter: the store does not keep the client of each request.
- The calendar in `tokprof ui` in the terminal.
