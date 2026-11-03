# Design

## Context

`tokprof day` reads the main logs last written in the 31 days before now, to find the night by counting half hours, and then shows one day. The library knows each log's last write, its creation where the file system keeps it, and, read from the end of the log, its last line with a time (the session list's last activity).

On 2026-11-03 the logs last written since 18:00 the day before were 56 logs of 2.55 GB. Picked by their last line with a time, 35 logs of 0.10 GB were active then. The rest were old logs whose write time moved when the IDE resumed them and Claude Code appended lines without a time.

`tokprof ui` has one background worker for X-rays and parallel workers that cost every session. Its right pane shows the X-ray text of the selected session.

## Goals / Non-Goals

**Goals:**
- A day view that reads only the logs active around its day, with no state kept between runs.
- One rule for the days in `day`, the pause report and the X-ray.
- One function that builds the day view's text, called by `tokprof day` and by the browser.

**Non-Goals:**
- Reading part of a log. A log active around the day is read whole.

## Decisions

1. **The nights come from the actions around each day.**
   - Each local date gets a night: the longest pause between consecutive actions, of all the sessions shown, whose middle falls in the window from 18:00 the day before to 12:00 the day. The day starts at the pause's middle.
   - Over the last 30 days these middles had a median of 05:14, and fell between 03:26 and 06:29 on the days with work on both sides.
   - The middle has to be in the window, not only the pause. A pause that merely overlaps the window would take a long afternoon break, or the 23.6h pause before 2026-10-31, as the night of the next day too.
   - Rejected: a night stored from 30 days of half-hour counts. It needs a file, a weekly 30-day read, and one night for every day.
   - Rejected: cutting at the first message after a pause of N hours. It needs a threshold and leaves agent work at night with no rule.
2. **Logs by their last activity.**
   - `day` first takes the main logs last written at or after the window's start, 18:00 the day before, and created before its end, 12:00 the day after. That is a cheap metadata check.
   - Of those, it keeps the logs whose last line with a time is in or after the window. The same backward read gives the session list its last activity.
   - The day's view then uses the timelines of these logs.
3. **The default day is the day of the latest action.** It needs only the logs active since 18:00 the day before that action's date, which the same rule picks. Before the morning, the latest action still belongs to the previous day.
4. **The night line** names the pause before the day and, when there is one, the pause after it. The pause report's `nights` line gives the median start of its days, and its hour table starts at that hour.
5. **The X-ray** finds the nights in the session's own actions, so a session's answers across a night are not counted.
6. **The strip takes a width.** The strip function takes the most columns it may use. `tokprof day` passes 72; the browser passes the pane's inner width less the lane labels.
7. **The browser's day view.**
   - A second background worker builds day views one at a time, the latest request replacing the waiting ones, as the X-ray worker does. It calls the same function as `tokprof day`, with the browser's filters and the pane's width.
   - The app keeps the shown day as a local date, and the texts it has computed, by date.
   - While the current day is shown, a change to any session's log marks its text out of date. It is requested again once a pause as long as its last computation has passed, as for X-rays.

## Risks / Trade-offs

- [A long log active around the day is read whole] → on 2026-11-02 that is about 0.2 GB in all. Reading from the day's first line is out of scope.
- [An all-night session leaves no pause] → the longest pause in the window still cuts the day; a day with no pause at all starts at midnight.
- [The day's edges now move from day to day] → the `day` line prints them, and agent work at night falls to the day whose night it is in.
- [The browser's day worker competes with the costing of every session] → it reads about 0.1 GB per day.

## Parse time

Parsing does not change. `tokprof day` reads about 0.1 GB on 2026-11-03 instead of 6.2 GB, plus one backward read per log picked by its write time, which stops at its last line with a time. The pause report reads what it read before.
