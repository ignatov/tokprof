# Tasks

## 1. The data in the core

- [ ] 1.1 Save the outputs of `tokprof summary` and `tokprof pauses` with fixed `--since`/`--until` dates (and once with `--project`) from the build before the change.
- [ ] 1.2 Add `scan::scan_counted` (a callback after each log) and `scan::report_logs`; `scan` calls `scan_counted`. Use `report_logs` in the CLI's summary, pauses, share and default paths.
- [ ] 1.3 Move the summary's order of models and its `Total priced`/avoidable sums into `ModelSpend::by_cost`, `ModelTotals::usd`, `Collector::priced_usd` and `Collector::avoidable_usd`, and the cold-return totals into `AgentPauses::return_totals`; `render_report` calls them.
- [ ] 1.4 Add `overview`: the `Spread` sink (cost per local date and component, per project, the sessions and requests) and `collect`, one pass into `(Collector, (Pauses, Spread))` with progress. Unit-test next to it: on the fixture logs the days and the projects sum to the collector's total and each component to the summary's column, a miss moves the rebuilt tokens into rewrite at the write and input prices, and the sessions count subagent logs with their session.
- [ ] 1.5 Diff the outputs of 1.1 against the new build: byte for byte the same.

## 2. The worker in the browser package

- [ ] 2.1 Add `Browser::changes` and `Browser::set_filter`.
- [ ] 2.2 Add `reports::Reports`: the period's query, one worker taking the newest job, progress in atomics with a wake at most every 250 ms, a cache by period, `tick` asking for the shown period when missing or stale after `max(4 × took, 20 s)`, `next_due`, `settle` for frames, and a detached form with `put` for tests. Unit-test the period's query and the back-off.

## 3. The view

- [ ] 3.1 Add the mode to the window: the `Day | Reports` switch in the bar with the day controls hidden in Reports, ⌘1 and ⌘2, and esc back to Day; the poll ticks the reports only while Reports is shown.
- [ ] 3.2 Draw `src/reports.rs`: the head with the pills and the progress, the key numbers, the day chart with its legend and readout, projects, origins, models, cold returns and largest context, in the settings panel's look with colors from `Pal`.
- [ ] 3.3 The keys of Reports: Tab and Shift+Tab through the parts, ← → in the period and the days, ↑ ↓ `k` `j` Home End through rows, ⏎ to drill down, esc back; scroll the part and the cursor into view.
- [ ] 3.4 Clicks: a day opens Day on its date, a project sets the filter and opens Day.
- [ ] 3.5 Add the scene `reports`, waiting for the 30-day report.
- [ ] 3.6 Runtime tests in `src/tests.rs`: the switch by click and keys, the pills, a day click opening Day on its date, a project click filtering, the keys, the progress text, and `q` in Reports.

## 4. Check and document

- [ ] 4.1 Run `cargo fmt`, `cargo test` and `cargo clippy --all-targets -- -D warnings` in `crates/tokprof-desk`, and `cargo fmt --all`, `cargo test --workspace` and `cargo clippy --workspace --all-targets -- -D warnings` at the root.
- [ ] 4.2 Render `--scene reports` in light and dark on the real logs, look at them, and record the window's numbers against `tokprof summary --since 30d` and `tokprof pauses --since 30d` here.
- [ ] 4.3 Update `crates/tokprof-desk/README.md` and the window's paragraph in `README.md`.
