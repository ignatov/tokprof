# Tasks

## 1. The store in the core

- [x] 1.1 Add the entry of one log: a sink over the analysis that keeps per minute the spend, the avoidable premium (misses whose cause is not expected), the requests and the commits, pushes and pull requests of worked shell calls; for a main log, the work minutes by UTC hour from its lane and the user's actions with the message flag; the log's ids, project, subagent and left-out flags. Unit-test next to it on the fixture logs: the minutes sum to the summary collector's `Total priced` and avoidable, a failed commit is not counted, work across minutes marks each minute.
- [x] 1.2 Add the store: the format with its fingerprint (version and a hash of the price entries), load (a broken file renamed to `.broken`), the refresh that keeps unchanged entries, parses the rest with `scan_counted`, keeps the entries of gone logs and drops moved ones, and the write through a temp file and a rename. Unit-test: reuse by path, length and mtime; a changed fingerprint reads existing logs again and keeps gone ones; a moved log counts once; a broken store; two stores written at once leave a whole one.
- [x] 1.3 Factor the sittings of a run of actions out of `Summary::of` so the calendar uses the same rule; the rhythm's output stays the same.

## 2. The calendar in the core

- [x] 2.1 Add the calendar of the entries: filters, days (rhythm days, extended after the latest action, local dates before the first), per-day sums of every metric, per local weekday and hour, the days shown (13 to 53 weeks), levels with the quartile cuts, streaks, busiest day and weekday, and the days that come only from gone logs. Unit-test each rule of the spec next to it: the night's minute splits an hour, the window cuts a day, the cuts of 1…8, one day at level 1, streaks with and without a request on the last day, agent minutes of two sessions at once, keyboard time split over hours, the weeks shown with little data and with 70 weeks.

## 3. The command

- [x] 3.1 Add the cache folder to `tokprof-browse::paths` (`$TOKPROF_CACHE_DIR`, macOS, Windows, XDG), and `render_calendar`: title, grid with month labels, marks with and without color, legend with cuts, the numbers, the punchcard, the empty case. Unit-test the text on a small calendar, with and without color.
- [x] 3.2 Add `tokprof calendar --metric`, with `--client` failing, and the stderr line on what was read.
- [x] 3.3 README: a `Your calendar` section with an example, the metrics, the days, the store and its location.

## 4. The window

- [x] 4.1 Add the calendar worker in `tokprof-browse`, as the reports' worker, with progress and back-off. Unit-test the back-off and that a detached one reads nothing.
- [x] 4.2 Add `Calendar` to the window's mode: the third segment, ⌘3, the bar without day controls, esc back, the poll ticking only while it is shown.
- [x] 4.3 Draw `src/calendar.rs`: pills, grid with labels and the theme's ramp, legend, numbers, punchcard, progress and `updating…`, the tooltip on hover and at the cursor.
- [x] 4.4 Keys and clicks: the cursor, `m` and ⇧M, ⏎ and a click opening Day on the date; palette entries `Calendar` and `Calendar: <metric>`; the key help's `Calendar` group.
- [x] 4.5 `--scene calendar` waiting up to 120 s, and `--metric`.
- [x] 4.6 Runtime tests in `src/tests.rs`: switching by click, ⌘3 and the palette, a cell click and ⏎ opening Day, the cursor's moves and limits, `m`, the tooltip's text, the progress text, `q` in Calendar.

## 5. Check and record

- [x] 5.1 `cargo fmt --all`, `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings` at the root; `cargo fmt`, `cargo test`, `cargo clippy --all-targets -- -D warnings` in `crates/tokprof-desk`.
- [x] 5.2 On the real logs, record here: the calendar's total and avoidable over `--since 2026-10-09 --until 2026-11-08` against `tokprof summary` with the same dates; three days' messages and keyboard time against `tokprof day <date>`; the times of the first and the second `tokprof calendar`; the store's size; `tokprof summary --since 2026-08-01` before and after.
  - 2026-11-08/09 night, load 13–49, store in a scratch `$TOKPROF_CACHE_DIR`, built at the lease's HEAD.
  - Spend: `tokprof calendar --since 2026-10-09 --until 2026-11-08` prints `total $11,959`; `tokprof summary` with the same dates prints `Total priced: $11,959   avoidable on cache misses: $1,328`, and `--metric avoidable` prints `total $1,328`. To the cent: the store's minutes in the window sum to $11,959.2256 and $1,328.3719; the summary's cost profile (`tokprof flame --stack agent -o x.collapsed`, µ$) sums to $11,959.225586. Difference under $0.0001, the order of addition. (The flame's `avoidable` value, $1,397.79, counts expected rebuilds too, which the summary's line and the calendar leave out.)
  - Days against `tokprof day <date>`, `sittings` line (sittings, keyboard, messages) vs the calendar's day: 2026-11-02 16, 4.1h, 59 = 16, 4.07h, 59; 2026-11-03 16, 7.3h, 116 = 16, 7.32h, 116; 2026-11-05 14, 2.5h, 47 = 14, 2.53h, 47; 2026-11-06 11, 4.7h, 71 = 11, 4.74h, 71; 2026-10-25 15, 5.5h, 82 = 15, 5.53h, 82. All equal; no night differed.
  - Speed: first `tokprof calendar` with no store 45.0 s wall (`read claude 1161 files (6.6 GB), codex 799 files (2.6 GB) in 40.5s`, 507 MB peak); second 1.87 s while one active log was read again; with no log changed 0.40–0.74 s (`read no log in 0.4s; 1,960 logs from the store`), and 0.17–0.33 s for `--agent claude`. Most of it is listing the logs, not the store: `json.load` of the store takes 0.07 s. JSON stays; no compact encoding needed.
  - Store: 2.98 MB for 1,960 logs: 54,100 minute rows, 4,102 actions, 2,489 hours of work bits.
  - `tokprof summary --since 2026-08-01`: 27.0 s and 26.9 s with `~/.local/bin/tokprof`, 26.2 s and 31.4 s with this build, alternating, under load 13–21: the same within the noise; the parse did not change.
  - The window (release build, `--snap --scene calendar`): the first Calendar with an empty store arrived after 25.1 s (`from the day store in 25.1 s`), the second after 0.1 s.
- [x] 5.3 Render `--scene calendar` in light and dark into `~/tokprof/plans/night-2026-11-08/shots/`, look at them, and fix what looks wrong.
  - `calendar-light.png` (spend), `calendar-dark.png` (keyboard, `--scene calendar-day` with the tooltip), `calendar-light-narrow.png` (960x700, messages), `calendar-keys.png` (key help). Fixed after looking: the tooltip was drawn under the numbers beside the grid (now the card's last child); its second line wrapped (now two lines); the legend's cuts ran into the numbers in a narrow window (now on a line of their own).
- [x] 5.4 `openspec validate --all --strict`.
