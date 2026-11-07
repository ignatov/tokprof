# Proposal

## Why

tokprof's edge is dollars by cause with advice on what to do next. Reports in the window shows where the money went, but not what to change. `tokprof share --since 30d` says it on the author's last 30 days: $1,711 (14.6%) could have been saved, $788 of it on returns after the cache expired; the 222 cold returns cost $1,734 as resumed, and a new session from a handoff note would have cost $562 and been cheapest in 202 of them; and the habits that cost the most are effort max or xhigh, sessions longer than a day, prompts of 400k+ and single lookups. The author should see this in Reports, with what to do about each row and the sessions behind it.

## What Changes

- **What to change, in Reports**, right under the key numbers, in the look of the other cards:
  1. `Could have been saved`: a row per cause with the avoidable dollars, the share of spend and the dollars per active hour, and under it the sentence tokprof's advice already says about it.
  2. `Coming back after a cold break`: the five ways of `tokprof bench` (resume as is, `/compact` first, a new session with a handoff note, heartbeats then resume, heartbeats then compaction) with their dollars, the share of resuming and the returns each was cheapest in; the cheapest marked; the rhythm's advice under them.
  3. `Habits that cost the most`: the rows with cost and share, the note that they overlap, and the advice tokprof has for a row (a tool hint is its own sentence).
- **The sessions behind a row**: a click or ⏎ on a cause or a habit that can be split by session opens its top 5 sessions by that cost under it; a click or ⏎ on a session opens it in Day, selected, with its day in the lanes.
- **The same numbers as `tokprof share`**: the window builds the share file of the period in the reports pass, without its rhythm, and takes the three tables from the same report over it that `share` prints. The texts of advice that were inside format strings (cache-miss hints, the thinking and lookup advice, the rhythm's return advice, the cache lifetime settings) become named constants and functions in the core, so the window and the CLI say the same words. The CLI's output stays byte for byte the same.
- **Keys**: Tab reaches the three cards after the period; ↑↓ move through their rows and open sessions; ⏎ opens or closes a row and opens a session. The key help says so. A scene `reports-advice` draws Reports with the sessions of the first cause open.

## Capabilities

### New Capabilities
None.

### Modified Capabilities
- `desk-window`: Reports gains what to change, its drill-down and its keys. The Reports requirements are still in the open change `add-desk-reports`, so this change adds requirements and changes none of them.

## Impact

- `crates/tokprof-core`: a module `savings` (the habits by session, the three tables from the share report, the sessions behind each row, the advice of each row); the overview pass also feeds the share sink and the bench scan; the advice texts named.
- `crates/tokprof-browse`: the browser finds a listed session by its short id.
- `crates/tokprof-desk`: `src/changes.rs` with the cards, hooks in `src/reports.rs` and `main.rs`, the key help, the scene, tests; README.
- `crates/tokprof`: `tokprof whatif` takes its settings line from the core.
- No change to the log sources or what the CLI prints.

## How it is checked on the real logs

- The window started with `--until 2026-11-07` and `tokprof share --since 30d --until 2026-11-07`, run at the same moment: every row of the three tables, the totals line of the savings and the returns count are equal. Recorded in tasks.md.
- `tokprof share`, `whatif`, `pauses` and `summary` over fixed dates, and the share file, are byte for byte the same before and after.
- The time Reports takes to read 30 days stays about the same (25.6–27.7 s before and after, beside the costing of every row).

## Out of scope

- Per-session splits of the cache lifetime rows and the tool hints (see design.md).
- The rest of `tokprof share`'s report: problems, prevalence, spread, time, rhythm.
- New advice: a row whose advice tokprof does not have shows none.
