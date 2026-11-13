# Design

## Context

See proposal.md for why. The window has three views today: Day, Reports and Calendar.
- **Day** reads the main logs around one day (from 18:00 the day before to 12:00 the day after, by the times of their lines) into the rhythm's timelines. That gives the lanes, the waits, the rhythm lines and the cold returns of the day. On the author's logs, a day reads 0.2 GB in 0.28–0.52 s (`tokprof day 2026-11-02` … `2026-11-08`, measured 2026-11-12).
- **Reports** runs one overview per period on a worker, which reads every log: summary, pauses with the rhythm section, and the share rows. On 2,303 logs (9.7 GB), `tokprof summary` took 6.7 s over a week's window, `pauses` 8.0 s and `share` 10.5 s. The pending change `skip-old-logs-in-windowed-reports` leaves `pauses` and `share` reading everything.
- **Calendar** refreshes the day store (`days-2.json`, format 2) on a worker and sums it. With no log changed, the refresh takes 0.13 s.

The store keeps, per log:
- per minute: spend, avoidable, requests, cache read and prompt tokens, saved dollars, commits and unpriced requests;
- per hour: the minutes worked (bits), the avoidable dollars of each miss cause, and the spend of each model;
- the user's actions, with a mark for each message;
- the times of the cold returns;
- the agent, the session, the main session, the project, and whether the rhythm leaves the session out.

It has no ends of turn, no questions, no titles and no dollars per cold return.

## Goals / Non-Goals

**Goals:**
- Everything the store can give shows within a second of opening Week, and when stepping weeks.
- One background read per week gives the rest, and it scales with the week's logs, not with all history.
- Each number has one source, the same as the CLI command it is checked against.

**Non-Goals:**
- A new store format.
- A CLI view of the week.
- Matching session titles in Week's filter: the store holds no titles.

## Decisions

### Where each number comes from

| Part | Source | New? |
|---|---|---|
| Green bars, the blue band (sittings), red ticks and their sessions | store: `work` bits of kept main logs, `actions`, `cold` with the entry's session | Sums over seven days. Today only Calendar's cells do this |
| Agents-at-work band | store: per minute, the kept main sessions whose work bits touch it | new: per-minute count, deduplicated by (agent, session) |
| Yellow (a wait while no agent works) | read: the rhythm's waits (Day strip's wait), minus the minutes in which some agent works | new: a union over sessions |
| Row numbers: spend, agent hours (each minute once), keyboard, cold count | store: the whole calendar cells of the seven dates | no: Calendar's cells |
| The line on rows and totals | store: the week's window minus the seven cells | new |
| Totals, tiles (but the cold returns' dollars) | store over the window, as `tokprof month` sums a month | new: a week's window and "same point last week" |
| Comparison notes, incomplete or small agent | store, by An incomplete period before | the rule is reused |
| `sittings` line | store actions over the window. It must equal `tokprof pauses`' line: 82 sittings, 490 messages to 65 sessions for 2–8 Nov, and the store script agrees | the formatting is reused |
| `agents` line | store work bits, with the peak from the read | new: `on <weekday>` in the peak |
| `answers`, `queue`, `cache` dollars, advice | read: the rhythm over the window, as the pauses rhythm section | reused |
| `night` line | store work bits, as month-review's night share | new line |
| What to change (savings, habits, their sessions and advice) | read: share rows over the window, as Reports' What to change | reused |
| Next week | store `misses` per hour, as month-review's line | reused rule, a new window |
| By project | store: spend per minute, and work bits by the entry's project, each minute once per project | new card |
| Pace per day (no comparison) | the same store sums over the days the week has run, from the later of Monday and the store's first request | new for Week; Month and Quarter reuse it in add-month-quarter-views |

### One read per week, by Day's rule

The read takes the logs that `logs_around` picks for the window from 18:00 on the Sunday before to 12:00 on the Monday after. It adds the subagent logs of the picked main sessions, so that share counts subagents as Reports does. Then it runs the overview with the query's `since` at Monday 00:00 and `until` at the next Monday 00:00, or now. On these logs it builds the rhythm's timelines, as Day does.

We expect the same numbers as reading every log, because a log whose last timed line is before Sunday 18:00 holds no request and no action in the week. The 18:00 margin keeps the night before Monday, as Day keeps it. Task 2.4 checks this on the real logs against `tokprof pauses` and `share` with `--since 2026-11-02 --until 2026-11-09` and `--since 2026-10-26 --until 2026-11-02`.

Alternative: run Reports' overview over all logs. It is correct by construction, but it took 6.7–10.5 s here and grows with history: at ~60 GB a year it would take about a minute per week stepped to. It stays the fallback if task 2.4 finds a difference, and then the spec's `The week's logs` is corrected in the same edit.

### The filter in Week

A project click in By project sets the window's filter, as the period views do after add-month-quarter-views. The filter's text keeps the store entries whose agent, project, session id or main session id contains it, ignoring case. Titles are not in the store, so the window counts the listed sessions whose title alone holds the text, and the note names that count. The filter is a text field next to the calendar's `--agent`, `--project` and `--session` filters. The week's read applies the same text per session, so `tokprof share` with `--project <text>` checks a project filter. The read's results are kept by (week, filter text), and typing starts no read until ⏎ or a click; a newer key replaces a pending job. add-month-quarter-views reuses this text filter for months and quarters.

### Midnight window, rows by calendar day

The week's numbers use local midnight, as the author decided and as month-review does for a month. So they equal `tokprof summary --since <Mon> --until <next Mon>` to the cent.

The rows use the calendar's day rule, so a row agrees with `tokprof day` and with Calendar's tooltip. The night after a midnight stays in the row of its day (the author's answer). The night after Monday 00:00 that belongs to last Sunday's day lies in this week's window but in last week's Sunday row, and this Sunday's night after the next Monday 00:00 lies in this week's Sunday row but not in its window. So the rows and the totals can differ, and a line under the totals states both amounts, as Month's spend by day states the night across the first.

Alternative: run the week from the start of Monday's day to the start of next Monday's day. The rows would add up, but no CLI command could check the totals, and it breaks the author's "Mon 00:00" rule.

### Strips by Day's rule, on one clock axis

The author chose Day's rule over the sketch's fixed 05:00 axis, with one clock axis for the week. Each strip draws exactly its day's span, so a strip never shows another day's moments and the night after midnight stays in its day's row. The axis is in hours from each row's midnight, so 13:42 sits at one place in every row, under one row of hour labels.

The axis starts at the whole hour at or before the week's earliest day start, never before 00:00, and ends at the whole hour at or after the latest day end, at most 30 hours later. The cap is measured with `tokprof day` on every date from 21 Sep to 12 Nov 2026, 8 weeks:

| Week of | Day starts | Latest end | Axis |
|---|---|---|---|
| 21 Sep | 00:00–08:06 | +5:05 next day | 30h |
| 28 Sep | 03:47–08:33 | +8:33 | 30h |
| 5 Oct | Fri 19:29 (Sat's day) – 11:56 | Sun 11:56 | 36h (41h without the 00:00 floor), capped to 30h |
| 12 Oct | 04:27–06:00 | +6:00 | 26.0h |
| 19 Oct | 03:26–06:13 | +6:13 | 28h |
| 26 Oct | 03:27–10:10 | +5:41 | 27h |
| 2 Nov | 03:41–05:49 | +6:50 | 28h |
| 9 Nov (so far) | 05:05–06:36 | +6:35 | 26h |

Seven weeks fit in 30 hours whole, two of them exactly. The eighth needs 36 hours for one day without a night pause: Sat 10 Oct ran from Fri 19:29 to Sun 11:56, 40.5 hours. A 36-hour axis would narrow every hour of a usual 28-hour week by about a fifth, for one quiet day. With the cap, that day is cut at both edges, and its marks name `from Fri 19:29` and `until Sun 11:56`. At 960 points the strips get about 400 points, about 4.5 minutes a point over 30 hours.

### Workers

The store sums run on the Calendar worker, which already refreshes the store. The week's read runs on its own worker, like the Reports worker:
- one job at a time, and the newest asked-for week wins;
- results are kept per week (by its Monday);
- it re-reads after log changes with Reports' back-off, four times the last duration and at least 20 s;
- nothing runs while Week is not shown.

Stepping weeks quickly asks for one read, the one of the week shown last.

### Cost and effect on the corpus parse

The full corpus parse is unchanged: no reader, decoder or store field changes. Opening Week costs:
- **Store:** 0.13 s for the refresh when no log changed, plus summing seven days of entries. The store is 5 MB, and the script over it ran in well under a second.
- **Read:** the logs around the week. For 2–8 Nov, Day's rule read 0.2 GB per day. Only 170 logs (3.5 GB) were modified in the week's span at all, which bounds the read at about 3.5 s. This week so far is at most 353 logs (0.7 GB). Expected: 1–4 s per week, against 6.7–10.5 s for Reports' every-log overview.

### Log fields

No new field. The read uses what work-rhythm and session-logs already read, with the versions those specs name:
- the typed messages, interrupts and answers to questions;
- ends of turn and wakes;
- the requests' times and usage;
- the `idle > TTL` misses.

### Later stage: turn ends in the store (format 3)

When the logs of a week are gone, its waits, answers and What to change are gone too (see `A week whose logs are gone`). A later change can add to the store, per main log: the ends of turn, the questions with their answers, and each cold return's first-request and avoidable dollars. That is format 3, `days-3.json`, which reads every log again once, about 45 s. Then the strips, `answers`, `queue` and `cache` need no read, and only What to change still needs one. This change ships stage 1 and keeps format 2.

## Risks / Trade-offs

- [Day's rule misses a log the share rows need, such as an earlier log of a resumed session that sets a session's span] → Task 2.4 compares the rows with `tokprof share` on two real weeks. Any difference switches the read to every log, as Reports reads, and the spec is corrected.
- [The agents band counts by minute, so two sessions that touch the same minute count as two] → The band is 6 points high and only a shape. The `agents` line's peak comes from the read, as Day's peak.
- [A day without a night pause is longer than the 30-hour axis] → It is cut with marks at the edges that name the times it reaches, and Day shows it whole. On the last 8 weeks this happened once (Sat 10 Oct).
- [The rows and the totals differ by the nights across the week's Mondays (the author's Sunday 8 Nov ran until Mon 06:50)] → A line under the totals states both amounts.
- [Project paths used to split worktrees, subfolders and temporary folders: 18 paths for 2–8 Nov] → merge-project-folders lands before this change and merges them: 10 projects for 2–8 Nov, 35 over all logs. `By project` shows 6 rows and `<n> more`.
- [A partial week compares one or two days early in the week] → The notes say `against <dates>, to <time>`, as Month in progress does.
- [The 340-pt column and a 960-pt window leave about 620 pt for the days, so the strips get about 400 pt for up to 30 hours (4.5 min per point)] → Bars are at least 1 point wide, and the pointer line names what is under it.

## Migration Plan

- `--scene reports-week` is removed. The desk's tests and README move to `--scene week --week <date>`.
- Reports' kept periods drop 7; a saved state does not hold the period, so there is nothing to migrate.
- Rollback: revert the change. The store format did not change.
