# Design

## Context

- Calendar in the window (`Mode::Calendar` in the desk) draws the calendar of `tokprof calendar`. That grid spans from the first data to the last day, 13 to 53 weeks, and has no notion of a year. Month in review is a card over Calendar.
- A month in review is computed from the day store alone (format 2, `days-2.json`). It counts a window from local midnight to local midnight, with the calendar's day rule for its days. It gives the six big numbers, busiest day and hour, night share, the share of minutes with 3 or more sessions at once, longest streak, top model, top miss cause, and the comparison with the period before, with its gaps. Every number Year needs is a sum of what the store already keeps.
- The store keeps, per log: per-minute spend, avoidable, requests, commits, cache tokens and saved dollars; per-hour bit masks of the minutes worked (main logs); the user's actions; per-hour avoidable by miss cause and spend by model; cold-return times; the agent, session, main session, project and the rhythm's left-out mark.
- The store does not keep cost components per model, requests per model, origins or the replay of cold returns. Those need the logs.
- The author keeps Claude logs for a year (`cleanupPeriodDays` 365), about 60 GB a year. Today's 9.7 GB read in 10.9 s on a cold store (`tokprof calendar`, measured 12 Nov 2026). A year of logs would take about a minute.
- Changes 1–3 of the chain (Sessions, Week, Month and Quarter) bring the bar with `‹ period ›`, the template of key numbers, the By project and What to change cards of Month, and the keys. Year builds on them (see the proposal's Depends on).

## Goals / Non-Goals

**Goals:**
- Year answers from the store in about a second, also for a year of 60 GB of logs, and stepping years reads no log.
- One computation of a year serves the window's Year, the Year in review card and `tokprof year`, so their numbers cannot drift.
- The year grid uses the same levels and cuts as `tokprof calendar` over the same days.

**Non-Goals:**
- No new store fields and no new store format.
- No change to `tokprof calendar`'s grid, which keeps its 13–53 weeks from the first data.
- No log reading for Year. The cards that need logs stay on Month and Quarter.

## Decisions

### A year is a period review over a window, made of months
The core's month in review is a sum over a window from one local midnight to another, built from `Calendar::of` over that window. That part is generalized to any window. A month and a year are then both a review over a window. The year adds its twelve months (each the month's own window, so a bar equals `tokprof month`), its records and its readiness. `Month` keeps its API for `tokprof month`.

Alternative: sum the twelve months into the year. That is rejected because the year's busiest hour, night share, streaks and records do not add up from months, and a streak across a month's border would break.

Cost: a year makes about 14 calendar passes over the store's entries (the year, the year before, twelve months), each over ~2,300 entries in memory. `tokprof month` today does 2 passes in 0.2–0.3 s, including reading the 5 MB store. A year is estimated well under a second. If it is not, the months come from one pass of the year's days, bucketed by the month windows. The parse time of the log corpus is not touched: Year reads no log beyond the store's usual refresh.

### When the logs begin: the earliest request or action of the asked agents
This is the earliest minute or action in any entry that the agent filter keeps, ignoring `--project` and `--session`. It follows the rule by which An incomplete period before ignores the project filter: a project that started later still had logs around it. It is computed in the same pass. Days before it are drawn as `no logs` and count nowhere, not in active days, levels or the pace per day.

Alternative: the first day of the filtered data. That is rejected because with a project filter, days of other projects' work would read `no logs`.

### "No logs" is a dashed outline
Level none is already an empty square with a hairline in dark themes (the existing rule against looking like level 1). A plain outline for `no logs` would match it. A dashed outline of the subtle stroke differs in every theme and stays quiet. Days to come are a dot with no square, as in the sketch. The quiet line goes in a tinted box inside the empty run when the run is 15 columns or more, which is about the width of the sentence at the smallest squares.

### Grid geometry: from the week of 1 January to the week of 31 December
That is 53 columns, or 54 in a leap year that starts on a Sunday (next in 2040). The squares fit the card's width (11–44 pt), so the numbers that stood beside Calendar's grid always go under it. The month bars sit in an SVG-like band under the grid. Each is centered on the mean column of its month, so it lines up with the month label above.

### Most at once from the store's minute masks
This is the highest count of kept main logs with a worked minute in the same minute, the count month-review's `Conductor` share already builds. It is by the minute, so it can exceed `tokprof day`'s peak (by intervals) by sessions that touched the same minute without overlapping. On the author's logs: 12 at Sun 1 Nov 17:08, against `tokprof day 2026-11-01` `peak 11 at 17:08`. The spec states the difference rather than hiding it.

Alternative: rebuild intervals from the logs. That is rejected because it needs the logs, and a minute is the store's grain.

### The busiest hour once
The busiest hour is in the Rhythm card only; the records leave it out, so Year does not show it twice. The year card keeps it among its facts.

### Biggest week: Mon–Sun windows from Monday 00:00, cut at the year's edges
This matches Week (change 2), which is a calendar week. On the author's logs the biggest is 12–18 Oct, $3,925, equal to `tokprof summary --since 2026-10-12 --until 2026-10-19` (local dates; the day rule may move a night's minutes by a day at the week's edges).

Quarter's columns count the same windows, cut at the quarter's first and last midnight, so they partition the quarter: they add up to its Spend, and a column equals the total of the Week it opens. Week's rows stay calendar days, and Week explains their gap in its note.

### What to change and Agents and models from the store only
Per-hour avoidable by cause and spend by model are in format 2, so Year shows causes with their dollars per month and models by spend, without components. Reading a year of logs for `tokprof share` or `summary` would cost about a minute at the author's retention, and it would grow every year. The top 5 sessions behind a cause come from the store's per-entry misses. Their titles come from the window's session list, which the window already holds.

### Year in review: a new capability `year-review`, not a MODIFIED month-review
The month-review requirements name the month in their rules, CLI and errors. Making them generic would rewrite every requirement of a shipped capability and its scenarios. `year-review` refers to month-review by name for titles, the advice line, the cache numbers and Nothing private, and owns what is new: records, readiness and `tokprof year`. The card's renderer reuses the month card's layout code with a months strip in place of the days.

### Readiness: the year is over, or 90 days of logs
90 days is the plain reading of the sketch's "three months of logs". A quarter of data gives a heatmap worth posting. The ready-on date is the earlier of the 90th day of logs and 1 January. This gates only `Save image`. The preview and `tokprof year` always show the numbers so far.

### Keys: the period first, the grid on Tab
PLAN wants ←/→ to step the period. Calendar used ←/→ for the cursor's week. Year takes the keys of change 3's Keys in Month and Quarter. While no card has the keyboard (the start), ← and → (`[`, `]`) step the year and `t` goes to this year. Tab moves into the grid, where ← and → move by a week as in Calendar. esc steps back in the same order as in Month. `r` opens the review, as it did in Calendar. `m` and ⇧M keep picking the metric from anywhere in Year.

### `tokprof year`
It mirrors `tokprof month`: same store refresh and stderr line, the same option errors, the headline `tokprof · year · 2026 with agents`, and months as bars of `█` instead of a calendar of days. The default is this year so far when this year has any request or message, else the last year that has one, else this year so far. So `tokprof year` on 1 January, before the first request, still prints the year just ended, and the first year of use prints itself.

### Titles of a year
The six rules of month-review hold over the year, with one threshold raised: `Marathon` takes 100 days in a row. At the month's 28, the author's 99 days of logs already hold two such runs (32 and 48 days), so nearly every year would earn it. 100 is twice the longest run so far and more than 2026 can still reach (97 days from 26 Sep).

## Risks / Trade-offs

- [The changes before this one rename or reword the shared requirements (`Reports view` becomes `View switch` in change 3; `Day controls`, `Key help`), so this delta's MODIFIED blocks do not apply until changes 1–3 are archived] → The proposal lists them under Depends on. The reconcile pass rebases them, and archive is simulated in a copy before the real one.
- [Scenario names kept for the validator (`In Reports`, `In Calendar`, `A frame of the calendar`) describe Month, Quarter and Year] → They state in their WHEN which view replaced the old one. The reconcile pass may rename them where change 3 already dropped `In Reports`.
- [Most at once by minute reads one higher than the Day view's peak] → Stated in year-review with a scenario, and checked on the author's logs (12 against 11).
- [A year of store minutes in memory] → The store is already read whole (5 MB, 2,304 entries). Year adds per-minute maps for one year (most at once), about 0.5 M minutes at most. That is small.
- [The weekday and weekend numbers depend on the day rule, so nights count to the day before] → This is the same rule as the calendar's `weekday` line, which Year equals.
- [`--scene calendar` disappears, and scripts or golden checks that use it break] → The desk's tests and README move to `--scene year` in the same change. The old name fails loudly, naming the scenes.

## Migration Plan

This is a window and CLI change with no data migration. The store format stays 2. Rollback is reverting the change, and the store stays valid for the old build.
