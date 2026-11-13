# Design

## Context

Reports today (see the desk-window spec and `crates/tokprof-desk/src/reports.rs`, `changes.rs`, `deltas.rs`) has two sources:
- An overview over N rolling days. `tokprof-browse/src/reports.rs` reads it on a worker with `tokprof_core::overview::collect`, which runs `summary`, `pauses` and `share` in one pass over the logs. It is kept per period and read again after changes, at least 20 s apart and 4× its last run. On the author's logs it takes about 25 s (README of tokprof-desk, 8.8 GB). The CLI on 9.7 GB took, on 2026-11-12: `summary` for October 7.8 s, `pauses` 11.7 s, `share` 10.4 s, and `summary` for Q4 so far 14.1 s.
- The comparison with the period before, from the day store (`tokprof_core::compare`, `deltas.rs`).

The day store format 2 (`daystore.rs`) already holds everything the store-side cards need, per log entry: agent, session, main session, project, the minutes (spend, avoidable, requests, commits, read tokens, prompt tokens, saved dollars, unpriced), work spans, actions, misses by cause per hour, spend by model per hour, and cold-return times. `tokprof_core::month::Month::of` sums one month of it in about 0.1 s: the big numbers, the calendar cells of its dates, the busiest day and hour, the night share, the streak, sessions, and the month before at the same point. `tokprof month` prints that in 0.4 s on the author's store of 2,302 logs.

No log field is read that is not read today. The cards depend on the fields the store and the overview already use: Claude Code's `message.usage` and `model` (2.1.x, as the source README names them), Codex's `token_count` events, and OpenCode's message rows.

## Goals / Non-Goals

**Goals:**
- Show the store-side cards of a period within a second, and the log-side cards when they arrive, as Reports does now.
- Use one rule for "the same point of the period before" in Month, Quarter and `tokprof month`.
- Keep every number checkable against a CLI command for the same dates.

**Non-Goals:**
- No new field in the store, and no new store format.
- No change to `overview::collect` beyond its selection: it still reads every log. `pauses` and `share` look before the window, so skip-old-logs-in-windowed-reports does not apply to them.
- No Year, and no change to Calendar beyond its key.

## Decisions

### A period is a window of local time

`Period { Month(Ym) | Quarter(year, q) }` gives `from_ms` and `to_ms`, each a local midnight, as `rhythm::midnight` gives them for a month. For the current period `to_ms` is now. The period before is `previous()`. Its window is the same elapsed time from its start, clipped to its end. This is `month.rs`'s `same = (prev_from + (to − from)).min(from)`, lifted to a function both use. Alternative: the same date and time in the corresponding month. That matches the band's `1–12 Oct` but differs across a DST change and is a second rule. It was rejected so that the window and `tokprof month this` always agree.

### Store-side numbers: one pass per period

`Month::of` is generalized to `PeriodSums::of(entries, filters, period, now, offset)`. It returns the totals, the calendar cells of the period's dates, the week sums, the per-week trend sums (read tokens, prompt tokens, requests, avoidable, spend), the per-project spend by week, the per-minute count of main entries at work (for "most at once"), and the records. A quarter is at most 92 days × 1,440 minutes over the entries the filters keep. On the author's store that is a few times a month's 0.1 s. It runs on the calendar worker after the store refresh, so no frame waits for it. The months of the quarter reuse `Month::of` for each month, so their subtotals equal `tokprof month`.

Week sums add up calendar cells, not local midnights, so a week and Week show the same days. A quarter's months add up local midnights, as `tokprof month` does. The two may differ by the night across a month's first day: $139 in October 2026 (month-review's check found $11,632 against $11,492.99 of cells). Spend by day says so under the chart instead of hiding it.

The level cuts of the month grid are the card's (`Calendar::cuts` over the month's cells), so the grid and the card agree (`cuts at $121, $269, $591` for October).

### Log-side cards: the reports worker reads a calendar period

`tokprof-browse::reports` keys its jobs by `(period, filter text)` instead of days. `period_query` sets `since_ms`/`until_ms` to the period's window, cut by the window's `--since`/`--until`. Nothing else changes: the same worker, the same back-off (4×, at least 20 s), and the same `computing…` count, worded `reading the logs of <period>…`. Results are kept per key. On the author's logs each new key costs one full read, about 25 s at a normal load. Stepping through four months costs about 100 s of background reading, and after that they show at once.

Alternative: put by-model components, by origin and the pause tables into the store. That would mean a store format 3, about 45 s to rebuild for every author, and a larger file. Origin and pause tables have many rows per hour. It was rejected for now. The store already answers the first screen, and the log cards sit lower on the page.

### The window's filter counts the period

The bar's filter text keeps entries whose agent, project or session/main id contains it, ignoring case: the text filter that add-week-view adds for Week, reused here over a month or a quarter. For the log side, the query's same `text` is applied by `overview::collect` per session, as the browser's filter does for rows. A project row's click sets the text to the project's path, as Reports' click does today, but stays on the period.

### Spend by day: the month before by weekday

The line shifts the month before by the days between the two first Mondays, 28 or 35, so every date faces the same weekday. The bars stay calendar cells, and the key numbers keep the same point by date and time (The same point of the period before). A date whose facing date falls outside the month before has no point, and the line does not wrap. In November 2026 that is 29 and 30 November; in December 2026, 1 to 5 December.

### Project shifts rule

The rule works over the quarter's ended weeks (the current week is left out), on spend shares per week:
- `started`: the project's first request in the store comes after the quarter's first week, it reaches ≥ 5% of some week's spend, and every agent the project's requests use has data in the store from at least 28 days before that week's Monday. Without the last clause, any project would "start" on the first weeks of the author's store (claude from 29 Sep).
- `paused`: after a week at ≥ 5%, two or more consecutive whole weeks with no request.
- `grew` / `fell`: share over the second half of the ended weeks minus the first half, ≥ 5 points either way. It needs at least 4 ended weeks. With an odd count, the middle week is left out.
- At most 5 rows, in kind order started, paused, grew, fell, and by quarter spend inside a kind.

These thresholds come from the sketch's fake data, not from the author's logs. The author kept them as drafted. Task 6.3 measures them on Q4 2026, records what fires, and adjusts the spec's Project shifts in the same edit when a threshold fires on noise or misses a shift the weekly spend shows.

### Colors of the quarter's projects

The six projects use six fixed hues checked for contrast on Merino Light and Merino Dark, mixed with the card as the calendar levels are. `other` uses the subtle ink. A theme has no categorical palette of its own, so the hues are the same in every theme. This is the trade-off: the hues may clash with a strong theme like Neon Arcade.

### Keys

←/→ step the period only while the page itself has the keyboard. Once Tab gives a card the keyboard, the arrows move its cursor, as they do in Reports today. `[`/`]` always step. esc unwinds in steps (card → page → filter → Day). This keeps esc going to Day, as before, without losing a set filter by accident.

### Scenes

`--scene month` now draws Month. The old frame (Calendar with the card open) becomes `--scene month-review`, drawn over Month. `--quarter YYYY-Qn` is new. Frames of a complete period are stable while its logs do not change, so golden-style checks use a fixture store and a fixed clock. The desk's tests already build fixture stores for Calendar (`calendar_tests.rs`).

## Risks / Trade-offs

- [The first look at a period costs a full log read, about 25 s] → The store cards show at once. The log cards say what they wait for. Results are kept per period and filter for the session.
- [Each filter text is a new log read] → The key is per filter. Typing does not start a read until ⏎ or a click, and only the last key is read: the worker drops a pending job when a newer one comes, as it does for periods.
- [Month sums and day bars differ by the night across the first] → The line under the chart states the gap in dollars.
- [`--client` cannot apply to the store] → The store's numbers count every client, one quiet line says so, and the log cards still filter (decided).
- [Thresholds of project shifts tuned on fake data] → Task 6.3 measures them and corrects the spec in the same edit.
- [Removing Reports drops the rolling 30 days some may use] → `tokprof summary --since 30d` remains, and Month so far covers the usual question.

## Effect on the parse time of the full corpus

None for the CLI. In the window, the log cards read the same corpus once per period and filter, as Reports read it once per rolling period. The store side adds one quarter pass of about 0.3 s on the calendar worker. Nothing reads the logs while Day, Week or Calendar is shown.

## Migration Plan

One release. The scenes `reports` and `reports-advice` go away (change 2 removes `reports-week`), and `month` changes meaning. Only the desk's own tests and README use them. Rollback is a revert of the change. The store and its format do not change, so nothing is rebuilt.
