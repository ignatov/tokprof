# Tasks

## 1. The data in the core

- [x] 1.1 Save the outputs of `tokprof summary` and `tokprof pauses` with fixed `--since`/`--until` dates (and once with `--project`) from the build before the change.
- [x] 1.2 Add `scan::scan_counted` (a callback after each log) and `scan::report_logs`; `scan` calls `scan_counted`. Use `report_logs` in the CLI's summary, pauses, share and default paths.
- [x] 1.3 Move the summary's order of models and its `Total priced`/avoidable sums into `ModelSpend::by_cost`, `ModelSpend::priced_usd`, `ModelTotals::usd` and `Collector::avoidable_usd`, and the cold-return totals into `AgentPauses::return_totals`; `render_report` calls them.
- [x] 1.4 Add `overview`: the `Spread` sink (cost per local date and component, per project, the sessions and requests) and `collect`, one pass into `(Collector, (Pauses, Spread))` with progress. Unit-test next to it: the days and the projects sum to the collector's total and each component to the summary's column, a miss moves the rebuilt tokens into rewrite at the write and input prices, and the sessions count subagent logs with their session.
- [x] 1.5 Diff the outputs of 1.1 against the new build: byte for byte the same.
  - 2026-11-06: `tokprof summary --since 2026-10-07 --until 2026-11-06`, the same with `--project json-profiler`, and `tokprof pauses --since 2026-10-07 --until 2026-11-06`, built at 46cc395 and at 6bd5c8e: `cmp` finds the three pairs identical.

## 2. The worker in the browser package

- [x] 2.1 Add `Browser::changes` and `Browser::set_filter`; `show_day` of the latest date in the window's mode shows the latest day, as Today does.
- [x] 2.2 Add `reports::Reports`: the period's query, one worker taking the newest job, progress in atomics with a wake at most every 250 ms, a cache by period, `tick` asking for the shown period when missing or stale after `max(4 × took, 20 s)`, `next_due`, `settle` for frames, and a detached form with `put` for tests. Unit-test the period's query and the back-off.

## 3. The view

- [x] 3.1 Add the mode to the window: the `Day | Reports` switch in the bar with the day controls and the filter hidden in Reports, ⌘1 and ⌘2, and esc back to Day; the poll ticks the reports only while Reports is shown, and in Reports wakes neither every second nor every minute.
- [x] 3.2 Draw `src/reports.rs`: the head with the pills and the progress, the key numbers, the day chart with its legend and readout, projects, origins, models, cold returns and largest context, in the settings panel's look with colors from `Pal`.
- [x] 3.3 The keys of Reports: Tab and Shift+Tab through the parts, ← → in the period and the days, ↑ ↓ `k` `j` Home End through rows, ⏎ to drill down, esc back; scroll the part and the cursor into view.
- [x] 3.4 Clicks: a day opens Day on its date, a project sets the filter and opens Day.
- [x] 3.5 Add the scene `reports`, waiting up to 120 s for the 30-day report.
- [x] 3.6 Runtime tests in `src/tests.rs`: the switch by click and keys, the pills, a day click opening Day on its date, a project click filtering, the keys, the progress text, the pointer's readout, the numbers of the cards, and `q` in Reports.
- [x] 3.7 After the command palette and the Flame tab landed: `Day`, `Reports` and the three periods as palette actions (a session, a tab or a day picked from Reports shows Day), ⌘1/⌘2 and a `Reports` group in the key help, and `?` from Reports. Test the palette's picks and `?`.

## 4. Check and document

- [x] 4.1 Run `cargo fmt`, `cargo test` and `cargo clippy --all-targets -- -D warnings` in `crates/tokprof-desk`, and `cargo fmt --all`, `cargo test --workspace` and `cargo clippy --workspace --all-targets -- -D warnings` at the root.
  - 2026-11-06, after the rebase onto the palette and the Flame tab: the window's 92 tests and the root's 501 pass; fmt and clippy are clean in both.
- [x] 4.2 Render `--scene reports` in light and dark on the real logs, look at them, and record the window's numbers against `tokprof summary --since 30d` and `tokprof pauses --since 30d` here.
  - 2026-11-06 23:16, light (`~/tokprof/plans/night-2026-11-06/shots/reports-light.png`, 1400x2100), run at the same moment as `~/.local/bin/tokprof summary --since 30d` and `pauses --since 30d`. The window read 1,937 logs in 40.4 s, beside the costing of every row.

    | number | window | CLI |
    |---|---|---|
    | spend | $11,603 | `Total priced: $11,603` |
    | avoidable on cache misses | $1,318, 11.4% of spend | `$1,318 (11.4%)` |
    | requests | 91,965 | the model table's requests sum to 91,965 |
    | by model | 11 rows, every cell equal (claude-opus-5-5: 72,899, 91%, $4,283, $1,764, $1,100, $0.60, $1,735, $8,882) | same |
    | cold returns, Claude Code | 6 rows equal; `218 cold returns: $786 in first requests, $749 of it avoidable.` | same |
    | cold returns, Codex | 7 rows equal; `39 cold returns: $32.91 …, $29.12 of it avoidable.` | same |
    | cold returns, key number | 257, $778 avoidable | 218 + 39, $749 + $29.12; the pauses' advice says `257 returns … cost $778` |
    | largest context | every row equal; Claude Code 800k+: 30 sessions, $6,055, 62.5% | same |
    | origins | prompt $5,385 46.4%, agent:general-purpose $2,125 18.3%, skill:code-review $1,720 14.8% | same |
    | spend by day | bars sum to $11,603: cache read $5,271, write $2,377, rewrite $1,436, uncached $141, output $2,379 | the summary's columns over the priced models |

    After the rebase, `reports-light.png` (1400x2100) and `reports-dark.png` (1400x900) in the same folder, looked at: the cards, the chart and the tables read in both themes; the logs had grown a little since (spend $11,613, 258 cold returns).
- [x] 4.3 Update `crates/tokprof-desk/README.md` and the window's paragraph in `README.md`.
