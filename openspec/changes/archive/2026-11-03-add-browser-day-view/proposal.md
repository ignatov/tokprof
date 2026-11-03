# Proposal

## Why

`tokprof day` reads 6.2 GB of logs to show one day: the main logs last written in the 31 days before now, Claude Code's and Codex's, in 14 seconds. It needs the 30 days only to find the user's night by its half hours. And the last write is a poor guide to a log's activity: when the IDE resumes a session, Claude Code appends lines without a time to its log. Old, large logs then look written today.

The day itself needs little. Picked by their last line with a time, 35 main logs of 0.10 GB were active from 18:00 on 2026-11-02 to the morning of 2026-11-03. The nights show in the days around a day. For each of the last 30 days, the longest pause in the user's actions whose middle falls between 18:00 the day before and 12:00 the day was the night. Its middle was a median of 05:14 and fell between 03:26 and 06:29 on the days with work on both sides.

The rhythm is also missing from `tokprof ui`, where the user looks at sessions all day. The X-ray there has the session's `rhythm` line, but not the day across sessions.

## What Changes

- **Each night is found from the days around it.** The night before a day is the longest pause in the user's actions, across sessions, whose middle falls between 18:00 the day before and 12:00 the day. A day runs from the middle of the night before it to the middle of the night after it, or until now. This replaces the count of half hours over 30 days, and nothing is stored.
- **`tokprof day` reads only the logs around its day.** These are the main logs whose last line with a time is at or after 18:00 the day before, and that were created before 12:00 the day after. Today that is 0.10 GB instead of 6.2 GB.
- **Which day `tokprof day` shows.** Without a date, it shows the day of the user's latest action. The `night` line gives the night before the day, and the night after it once there is one.
- **The pause report** cuts its days the same way. Its `nights` line gives the median start of a day.
- **The X-ray's `rhythm` line** cuts days at the nights in the session's own actions.
- **A day view in `tokprof ui`:**
  - `d` shows the day in the right pane instead of the X-ray, and `d` again shows the X-ray.
  - `[` and `]` step to the day before and after, but not past the current day.
  - The day is computed in the background, one at a time, as X-rays are. While the current day is shown, it is computed again after a session writes to its log.
  - The strip fits the pane's width: with fewer than 72 columns free, a column covers 20, 30 or 60 minutes.
- The README describes the nights, what `day` reads, and the day view in the browser.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `work-rhythm`: the start of the day (each night from the days around it, with the night line), the day view (its read and its default day), and a strip that fits a given width.
- `session-logs`: the line about the logs read says what `day` reads now.
- `pause-report`: the rhythm section prints a `nights` line, and its hour table starts at the hour of the median day start.
- `session-browser`: a new requirement for the day view in the right pane, its keys and its background work.

## How it is checked on the real logs

- `tokprof day` on 2026-11-03 reads about 0.1 GB and takes under a second, against 14.4s before.
- `tokprof day 2026-11-02` runs from 04:21, the middle of the pause from 00:22 to 08:20. It prints the lines of the baseline: 16 sittings, 3.9h at the keyboard, 58 messages to 11 sessions, a median break of 28m, a turn ended every 6.3m, 3% of turns started by the agent, 5 cold returns for $15.48. It reads about 0.2 GB.
- `tokprof --agent claude --since 30d pauses` prints the rhythm lines of the baseline within its tolerances: medians within half a minute, shares within 3 points. Its `nights` line reads a median start near 05:14.
- In `tokprof ui`, `d` shows today within a second, and `[` shows 2026-11-02 with the lines of `tokprof day 2026-11-02`.

## Out of scope

- Reading only the end of a long log. A log active around the day is read whole.
- An SVG of the day.

## Impact

- Core: the nights from actions instead of the half-hour count; the strip takes the width it may use.
- CLI: `day` picks its logs by their last activity and shows the latest day by default; the pause report's `nights` line; the browser's day view, its keys and a background worker.
- README.
