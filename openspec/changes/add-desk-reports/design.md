# Design

## Context

`tokprof summary` and `tokprof pauses` are two passes of the CLI over the logs a report reads: the CLI discovers every log of the chosen agents, keeps those the session filter holds, and folds each parsed session into a sink with `analysis::analyze`, a `Collector` for summary and `Pauses` for pauses. The sinks are in `tokprof-core`; the discovery, the `Total priced … avoidable on cache misses` arithmetic and the cold-return totals are in `crates/tokprof`. The window cannot use the CLI crate, so that part moves to the core.

Neither report has spend by day or by project. Facts, the per-component shares the collector sums, carry the project but are emitted per chain at its end and timed by when their content entered the context, not by the request, so they cannot give a day. Request records give the time, usage and price of each request; a miss event right after its request gives the tokens rebuilt.

The browser already runs workers (the day, the X-ray, the costing of rows) that send notes and wake the window; the day beside the list waits `max(4 × its last computation, 20 s)` before it is computed again after a log change.

Log fields: none new. The report reads what `tokprof summary` and `tokprof pauses` read, from the agent versions their sources name (Claude Code up to 2.1.316, Codex as its source README lists).

## Goals / Non-Goals

**Goals:**
- The window's numbers are the CLI's numbers, computed by the same functions.
- One pass per period for all seven cards.
- No frame waits; the window at rest does no work.

**Non-Goals:**
- Incremental reports; parsing faster.

## Decisions

### One pass, three sinks

A period's report is `analyze` into the tuple `(Collector, (Pauses, Spread))`. `Spread` is new: per request it adds the cost to the local calendar date of the request, split by component the way the analysis splits it (cache read, cache write, uncached at their prices, output at the output price, and for a request that missed, the tokens rebuilt moved from write and uncached into "rewrite after miss" at the same prices); per request also the session's project, and the session key `(agent, short id)` for the session count. The sum over days equals the summary's total up to float rounding, which a unit test checks against the collector on the fixture logs and tasks.md on the real logs.

Alternative: date facts by their own time. Rejected: a cache read of old content would land on the day the content entered.

### What moves to the core

- `scan::report_logs(catalog, select)`: the logs a report reads (discover, then the session filter). The CLI's summary, pauses, share and default paths use it.
- `scan::scan_counted`: `scan` with a callback after each log; `scan` calls it with none.
- `ModelSpend::by_cost()` (the summary table's order), `ModelTotals::usd()`, `Collector::priced_usd()` and `Collector::avoidable_usd()`, which `render_report::summary` now calls, summing in the same order so the text is the same.
- `AgentPauses::return_totals()`, the line under the cold-return table.
- `overview::collect(catalog, select, pricing, progress)`: the one pass above, with `progress(done, total)` after each log.

### The period

A period of N days is the window's query with `since = max(--since, start − N × 24 h)`, where start is when its computation begins, so it is `--since <N>d` run at that moment. Project, session, client and until stay.

### The worker

`tokprof-browse::reports::Reports` holds one worker thread, a cache by period, the job pending and the progress (`done`, `total`, the period being read) in atomics. The worker takes the newest job waiting, runs `collect` on the global rayon pool, and wakes the window at most every 250 ms while it reads and once at the end. `tick(period, changes)` is called by the window's poll only while Reports is shown: it takes in results and asks for the shown period when it has none, or when the logs changed since it was computed and its pause `max(4 × took, 20 s)` has passed. `next_due` gives the window the moment to wake for that. While Day is shown nothing is asked.

### The view

`src/reports.rs` draws one scroll pane: a head with the pills and what the result read, then the cards as groups in the settings panel's look (radius 16, `Pal::bar`, accent outline while the card has the keyboard). The day chart is boxes laid out in a row, one column per date, each a stack of component boxes; a hover sets the day the readout under it names. The bars of projects and origins are the inspector's meter shape. The whole pane is memoised by the report, the period, the cursor and focus, the hover and the theme.

### Keys

⌘1 and ⌘2: `r` is the browser's re-sort, digits switch the inspector's tabs, and ⌘1/⌘2 are free in the window, the palette and the browser. In Reports every key but ⌘Q, ⌘W and ⌘, goes to it: Tab/Shift+Tab through period, by day, by project, by origin, by model, cold returns, largest context; ← → in the period and the days; ↑ ↓ `k` `j` Home End in rows; ⏎ drills down from a day or a project; esc goes back to Day.

### A frame without a window

`--scene reports` switches to Reports and waits up to 120 s for the 30-day report, as `settle_day` waits for the day.

## Effect on parse time

The CLI's commands do one extra atomic increment and an empty call per log; their parse time does not change. The window reads nothing for reports until Reports is shown. A period's pass is one summary pass: about 25 s on the author's 8.8 GB (`tokprof summary --since 30d` read 1,937 logs in 25.3 s; the files a period reads are chosen by their last write, so 7, 30 and 90 days read nearly the same logs). While Reports is shown and an agent keeps writing, the report is read again at most every `max(4 × 25 s, 20 s)` = 100 s.

## Risks / Trade-offs

- [The pass takes all cores for ~25 s, beside the costing of rows at start] → only when Reports is shown, and at most every 100 s after changes.
- [A day in the chart is the calendar date, not the day of the lanes, which runs from night to night] → the readout names the date; a click opens the lanes of that date.
- [A project click filters by text, so `~/src/app` also lists `~/src/app2`] → the browser's filter is text; acceptable.
