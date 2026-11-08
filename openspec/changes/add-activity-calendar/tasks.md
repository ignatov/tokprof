# Tasks

## 1. The store in the core

- [ ] 1.1 Add the entry of one log: a sink over the analysis that keeps per minute the spend, the avoidable premium (misses whose cause is not expected), the requests and the commits, pushes and pull requests of worked shell calls; for a main log, the work minutes by UTC hour from its lane and the user's actions with the message flag; the log's ids, project, subagent and left-out flags. Unit-test next to it on the fixture logs: the minutes sum to the summary collector's `Total priced` and avoidable, a failed commit is not counted, work across minutes marks each minute.
- [ ] 1.2 Add the store: the format with its fingerprint (version and a hash of the price entries), load (a broken file renamed to `.broken`), the refresh that keeps unchanged entries, parses the rest with `scan_counted`, keeps the entries of gone logs and drops moved ones, and the write through a temp file and a rename. Unit-test: reuse by path, length and mtime; a changed fingerprint reads existing logs again and keeps gone ones; a moved log counts once; a broken store; two stores written at once leave a whole one.
- [ ] 1.3 Factor the sittings of a run of actions out of `Summary::of` so the calendar uses the same rule; the rhythm's output stays the same.

## 2. The calendar in the core

- [ ] 2.1 Add the calendar of the entries: filters, days (rhythm days, extended after the latest action, local dates before the first), per-day sums of every metric, per local weekday and hour, the days shown (13 to 53 weeks), levels with the quartile cuts, streaks, busiest day and weekday, and the days that come only from gone logs. Unit-test each rule of the spec next to it: the night's minute splits an hour, the window cuts a day, the cuts of 1…8, one day at level 1, streaks with and without a request on the last day, agent minutes of two sessions at once, keyboard time split over hours, the weeks shown with little data and with 70 weeks.

## 3. The command

- [ ] 3.1 Add the cache folder to `tokprof-browse::paths` (`$TOKPROF_CACHE_DIR`, macOS, Windows, XDG), and `render_calendar`: title, grid with month labels, marks with and without color, legend with cuts, the numbers, the punchcard, the empty case. Unit-test the text on a small calendar, with and without color.
- [ ] 3.2 Add `tokprof calendar --metric`, with `--client` failing, and the stderr line on what was read.
- [ ] 3.3 README: a `Your calendar` section with an example, the metrics, the days, the store and its location.

## 4. The window

- [ ] 4.1 Add the calendar worker in `tokprof-browse`, as the reports' worker, with progress and back-off. Unit-test the back-off and that a detached one reads nothing.
- [ ] 4.2 Add `Calendar` to the window's mode: the third segment, ⌘3, the bar without day controls, esc back, the poll ticking only while it is shown.
- [ ] 4.3 Draw `src/calendar.rs`: pills, grid with labels and the theme's ramp, legend, numbers, punchcard, progress and `updating…`, the tooltip on hover and at the cursor.
- [ ] 4.4 Keys and clicks: the cursor, `m` and ⇧M, ⏎ and a click opening Day on the date; palette entries `Calendar` and `Calendar: <metric>`; the key help's `Calendar` group.
- [ ] 4.5 `--scene calendar` waiting up to 120 s, and `--metric`.
- [ ] 4.6 Runtime tests in `src/tests.rs`: switching by click, ⌘3 and the palette, a cell click and ⏎ opening Day, the cursor's moves and limits, `m`, the tooltip's text, the progress text, `q` in Calendar.

## 5. Check and record

- [ ] 5.1 `cargo fmt --all`, `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings` at the root; `cargo fmt`, `cargo test`, `cargo clippy --all-targets -- -D warnings` in `crates/tokprof-desk`.
- [ ] 5.2 On the real logs, record here: the calendar's total and avoidable over `--since 2026-10-09 --until 2026-11-08` against `tokprof summary` with the same dates; three days' messages and keyboard time against `tokprof day <date>`; the times of the first and the second `tokprof calendar`; the store's size; `tokprof summary --since 2026-08-01` before and after.
- [ ] 5.3 Render `--scene calendar` in light and dark into `~/tokprof/plans/night-2026-11-08/shots/`, look at them, and fix what looks wrong.
- [ ] 5.4 `openspec validate --all --strict`.
