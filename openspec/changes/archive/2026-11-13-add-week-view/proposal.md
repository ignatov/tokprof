# Proposal

## Why

The window has a day (Day) and rolling periods of 7, 30 and 90 days (Reports). It has no view of a calendar week. So it cannot answer "how did my week go": when I sat at the keyboard, when the agents worked without me, where they waited for me, and where I came back to a cold cache. Reports' `7 days` is a rolling window that starts at the same hour as now, so it cannot be compared with "the same point of last week". The author picked variant A of the Week sketch with a narrow column from variant C (plans/ui-redesign-2026-11-12/week.html). This is change 2 of 4 of the desk redesign.

## What Changes

- **A Week view in the window.** The switch becomes `Day | Week | Reports | Calendar`, with ⌘1–⌘4. Week shows a calendar week, Monday to Sunday, this week to start with. ← and → step weeks, and `t` and `Today` go back to this week. The bar names the week, as `9–15 Nov 2026`, and holds the window's filter.
- **The week's window.** It runs from local midnight on Monday to local midnight on the next Monday, or until now. A week in progress compares with the same point of last week, from Monday 00:00 to the same weekday and time. A full week compares with the whole week before. When there is no comparison (no data in the week before, an agent whose logs start inside it, or `--since`/`--until`), each tile and total shows its pace per day, as Month in review does, and the notes say why.
- **Seven day strips (left).** Each row is a day of the week, Monday first, by Day's rule for the start and end of a day: a strip draws exactly what that day's Day view holds, so the night after midnight stays in its day's row. The seven strips share one clock axis under one row of hour labels, so the rows compare by eye. The axis runs from the whole hour at or before the week's earliest day start (not before 00:00) to its latest end, at most 30 hours; what falls outside is cut with a mark. A strip shows:
  - when some agent worked;
  - when a session waited for your answer and no agent worked;
  - when you were there;
  - the cold returns, as ticks;
  - a thin band with the number of agents at work.

  Each row ends with the day's spend, agent hours, hours at the keyboard and cold returns: its whole calendar cell. The days after now are empty, and today has the now line. A day before the first log reads `no logs`.
- **Under the strips.** A footer row of the days gives the week's totals over its window (Monday 00:00 to Monday 00:00), under the days' numbers; their changes against the same point of last week are in the tiles. Lines under it name the week compared with and the last full week. When a night crosses one of those Mondays, the rows add up to another sum, and a line says by how much. Then come the week's rhythm lines: `sittings`, `agents`, `answers`, `queue`, `night` and `cache`. Then comes the rhythm's advice on cold returns.
- **Clicks.** A day row (or ↑ ↓ and ⏎) opens Day of that date. A cold-return tick opens Day with that session selected, on its `Cache` tab.
- **The column (right, about 340 pt).**
  - Six tiles against the same point of last week: spend, agent hours (the calendar's `agent` metric: each minute some agent worked counts once), hours at the keyboard, avoidable on misses, cache hit and cold returns.
  - `What to change`: the top three causes of `Could have been saved` and the top three `Habits that cost the most`, with their advice and their sessions, as in Reports.
  - The `Next week` line: month-review's line for next month, counted over the week.
  - A `By project` card: spend, share and agent hours of the top projects, each minute once per project, as the card's head says. A click on a project sets the window's filter and stays on the week.
- **The filter in Week.** The bar's filter counts every part of Week, and the week before, by agent, project or session id. The store has no titles, so when the text matches sessions only by title, a note says so and points to Sessions. `/` starts it; esc steps back from a card, then clears the filter, then shows Day.
- **`--client`.** The store's numbers count every client, and one quiet line says so. The parts read from the week's logs count only that client.
- **Reports loses `7 days`.** Its pills become `30 days` and `90 days`. **BREAKING** for `--scene reports-week`: it is removed, and `--scene week` replaces it.
- **A Week frame without a window.** `--scene week` with `--week <date>` draws Week for tests.

Where the numbers come from (see design.md for each one):
- **From the day store at once (format 2):** spend, avoidable, cache hit, agent time, the time at the keyboard and the sittings, messages, cold-return times and their sessions, miss causes by hour, and projects.
- **From the week's logs, read in the background:** the waits, the `answers` and `queue` lines, the dollars of the cold returns, the advice, and What to change (`tokprof share`). The read covers only the logs around the week, by Day's rule.
- **No change to the store format** in this change.

Checked on the author's logs, for the full week of 2–8 Nov 2026 (measured 2026-11-12 with tokprof 0.1.6):
- **Totals.** Spend $1,843 equals `tokprof summary --since 2026-11-02 --until 2026-11-09` (`Total priced` $1,843). Avoidable $89.89 (4.9%) equals its `avoidable on cache misses`. A script over `days-2.json` gave $1,842.94 and $89.89.
- **Rhythm lines.** The `sittings` line reads `82, 1.2d at the keyboard, 490 messages to 65 sessions; median sitting 12m, median break 28m`. `answers` reads `2.7m` and `2.7m`. `queue` reads `67% of 142 answers (46% by chance)`, with the oldest at `24m`. These are the lines of the rhythm section of `tokprof pauses --since 2026-11-02 --until 2026-11-09`.
- **Cold returns.** There are 37: $78.73 in first requests, $75.12 of it avoidable ($77.00 + $1.73 and $73.71 + $1.41 in `tokprof pauses`). The store also counts 37 cold-return times in the window.
- **Strip axis.** Over the 8 weeks from 21 Sep to 12 Nov, the axis the rule gives is 30, 30, 30 (capped from 36), 26, 28, 27, 28 and 26 hours (`tokprof day` for each date). For 2–8 Nov it runs from 03:00 to 07:00 the next morning.
- **Day rows.** Each row's agent time and time at the keyboard agree with `tokprof day <date>` and with the calendar cell of that date. For example, Tue 3 Nov reads 12.5h of some agent at work and 7.3h at the keyboard. Sun 8 Nov reads 3.2h and 13m.
- **What to change.** It shows the rows of `tokprof share --since 2026-11-02 --until 2026-11-09`:
  - `Could have been saved`: idle > TTL $88.57, cache lifetime: subagents $19.82, history edited $1.32;
  - `Habits that cost the most`: effort max or xhigh $1,265, prompts of 400k+ $908, single lookups $596.
- **By project.** With the projects of merge-project-folders, 2–8 Nov has 10 projects with spend. `~/src/zebrafish` reads $1,215 (66%) and 36.5h, each minute once, its worktree leases folded in. The week's agent hours are 52.5h. (A script over `days-2.json` that folds the week's worktree, subfolder and chat paths as merge-project-folders does.)
- **Next week.** It reads `$88.57 went to idle > TTL. Compact or wrap up a big session before a long break.`
- **Comparisons.** Against 26 Oct – 1 Nov ($2,507 and $487 avoidable in the store), spend reads `↓ $664 (−26%)`. This week so far (Mon 9 Nov 00:00 to Thu 12 Nov 15:00) read $1,753 in the store, against $1,079 at the same point of last week.
- **Cost of the background read.** Reading every log, as Reports does today, took 6.7 s (`summary`), 8.0 s (`pauses`) and 10.5 s (`share`) over 2,303 logs (9.7 GB). Day's rule reads 0.2 GB in 0.28–0.52 s per day. For 2–8 Nov, at most 170 logs (3.5 GB, the upper bound by modification time) qualify, so we expect 1–4 s.

Out of scope:
- variant B of the sketch (projects by day, threads across days) as a screen;
- the `Week in review` share card, a later idea;
- Month, Quarter and Year (changes 3 and 4), and the Sessions screen (change 1);
- a `tokprof week` command: a later, separate change;
- turn ends in the day store (format 3), so that weeks whose logs are gone keep their waits and answers (a later stage, see design.md);

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `desk-window`:
  - adds Week and its parts: days, strips, totals, rhythm lines, tiles, What to change, Next week, By project, computation, keys and frame;
  - adds the filter in Week and the pace per day of a week without a comparison;
  - changes the switch and the keys of Reports and Calendar, Day controls, Key help, Report period, Report computation, and the rules on an incomplete or small period before;
  - removes the week of Reports in a frame.

## Depends on

- `merge-project-folders`, which lands before this change. It merges worktrees, named copies and subfolders into their repository root, and temporary folders and Codex chat folders into one project each: 391 project paths become 35 over all logs, and 18 become 10 in 2–8 Nov 2026. By project, the filter and the checks below use those projects.
- `add-sessions-screen` (change 1). These MODIFIED texts are written on top of the versions in its desk-window delta, as drafted on 2026-11-12. If change 1 changes them again, the reconcile pass must redo them:
  - `Reports view` (the switch, change 1's `Sessions` button and ⌘0);
  - `Day controls` (change 1's `In Sessions` scenario is kept);
  - `Key help` (change 1's groups, with `Week` added);
  - `Keys in Reports`, `Calendar view` and `Keys in Calendar` (⌘0 and the details pane).
- A cold-return tick opens Day with that session selected in change 1's details pane, on `Cache` instead of `Summary`.
- A click on a project in `By project` sets the window's filter and stays on the week. Change 1 sends a click in Reports' project card to Sessions; this change leaves Reports as it is.
- Week has its own requirement for its keys, so change 1's `Quitting the window` is not modified here. `t` goes to this week, as `t` goes to today's day or period in every view (see open question 1). `Window layout` and `Palette entries` are not modified either: Week adds its palette entries through the `Actions` group, and its layout is in its own requirement.
- `Report period`, `Report computation`, `An incomplete period before`, `A small agent in the period before` and `Causes gone since the period before` are written against the current spec. Change 1 does not touch them.

## Open questions

1. **The key for today.** Default chosen, author to confirm: `t` goes to today's period in every view (Day, Week, Month, Quarter, Year; Sessions scrolls to `Today`), not `T`.
2. **No comparison.** Default chosen, author to confirm: a week before that is incomplete or without logs gives the pace per day under each tile and total, as Month in review drops the changes. Reports, while it lasts, keeps greying its changes. Before this default, Week greyed the changes as Reports does.

## Decided

- **Strips follow Day's rule, on one clock.** Each strip draws exactly its day's span, as that day's Day view does, not a fixed 05:00 axis; the night after midnight stays in its day's row. All seven share one clock axis with one row of hour labels, so a cold return after lunch lines up across the rows. The axis runs from the whole hour at or before the week's earliest day start (not before 00:00) to the whole hour after its latest end, capped at 30 hours; what falls outside is cut with a mark. The cap comes from the last 8 weeks of `tokprof day`: 7 of them need 26–30 hours. The week of 5 Oct would need 36, for one day without a night pause (Sat 10 Oct, Fri 19:29 to Sun 11:56).
- **Rows and totals.** The rows are whole calendar days; the totals and tiles keep the week's window from Monday 00:00. A night across Monday 00:00 lies in the Sunday row of the week before, so a line under the totals states the difference, as Month's spend by day does.
- **Agent hours.** The tiles, totals and rows count each minute some agent worked once (the calendar's `agent` metric, 52.5h for 2–8 Nov). The `agents` rhythm line keeps the sum over sessions (80.5h). By project says so in its head and counts each minute once per project.
- **The filter in Week.** It matches agent, project and session id, not titles, since the store has none. A match by title only gets the note `<n> sessions match "<text>" only by title; Week counts agents, projects and session ids, not titles. Sessions (⌘0) lists them.` A project in By project sets the filter and stays on the week.
- **`--client`.** The store's numbers count every client, and one quiet line says so. The parts read from the logs filter by client.
- **Console first.** `tokprof week` is not part of this change. It is a later, separate change.

## Impact

- **Code:**
  - `crates/tokprof-desk`: a Week view, its keys, its scene, the switch and the Reports pills;
  - `crates/tokprof-browse`: the week's worker, its store sums, and the read of the logs around the week;
  - `tokprof-core`: the sums over a week of the store and the rhythm over a window, reused.
- **Logs:** no new log field. The read uses what Day and `tokprof pauses` and `share` already read: ends of turn, questions, typed messages, interrupts and requests, with the fields and agent versions that session-logs and work-rhythm name.
- **Store:** format 2, unchanged.
- **README.md:** the window's section names Week, its keys and `--scene week`.
- **Tests:** the desk's frame tests, and unit tests of the week's window, rows and comparisons with a fixed now.
