# Tasks

## 1. The store, format 2

- [x] 1.1 Add to each minute the tokens read from the cache, the prompt tokens, the dollars saved by cache reads and the requests without a price. Add to each entry the avoidable premium by cause and the spend by model per hour, and the cold returns. Rename the file to `days-2.json`. Unit-test the minute's array and the moved, grown and gone logs again.

## 2. Hardening the store (code review 1, PM decisions)

- [x] 2.1 Only "not found" starts an empty store. Any other read error gives a store in memory that is never written, with a warning. Test: `chmod 000` leaves the file as it was.
- [x] 2.2 A broken store is kept as `days-2.json.broken-<UTC>`, never over an earlier backup. Flush the temporary file before the rename, and remove it when the write fails. The window shows the reset. Tests: two broken stores keep both backups; a failed write leaves no temporary file; the window's line names the backup.
- [x] 2.3 The fingerprint names the git commit (`build.rs`), and the version alone without git. Test: the build id holds the 12-character commit.
- [x] 2.4 The window reads the price files again when they change. Test: a changed `--prices` file changes the worker's fingerprint.
- [x] 2.5 A moved log drops its old entry only after the log at the new key was read. Test: a moved log that cannot be read (`chmod 000`) keeps its entry.
- [x] 2.6 Requests without a time are in no day and do not stretch the grid. One `untimed` line in the CLI and one row in the window count them. Test: one ts-0 request beside a week of data leaves 13 weeks.
- [x] 2.7 Write only when an entry changed. The window refreshes no more than once a minute while logs change.
- [x] 2.8 A failed save does not fail `tokprof calendar` or `tokprof month`: the output comes first, then the warning. Test: a store path that is a folder still gives the calendar and a warning.

## 3. The month

- [x] 3.1 Add the month in the core: its window, the calendar's days, the busiest day and hour, the night, morning and at-once shares, the streak, sessions, cold returns, top model and top cause, and the month before. Add the six titles in order, with their thresholds. Unit-test the window (the night across the first and the 1 November minute left out), the deltas, the titles' order, models, causes, cold returns and sessions at once.
- [x] 3.2 Add the month's texts in the browser package and `tokprof month [YYYY-MM|this]`, which fails on `--since`, `--until` and `--client`. Unit-test the whole text, the month with no title and no month before, and the month with nothing.
- [x] 3.3 Add a README section with the real October output.

## 4. The window

- [x] 4.1 Add the card: the button in Calendar's head, `r`, the palette entry, ← → and esc, all keys kept while it is open, and the card's look in the theme.
- [x] 4.2 `Save image`: the free path in `~/Downloads`, the program run again with `--scene month-card` at 540×675, and the status in the card and the bar. Unit-test the free path.
- [x] 4.3 Add the scenes `month` and `month-card` and `--month`.
- [x] 4.4 Runtime tests: `r` opens the cursor's month, the keys stay with the card, ← and → stop at this month, esc closes it, the button and the palette open it.

## 5. Check and record

- [x] 5.1 fmt, test and clippy `-D warnings` at the root and in `crates/tokprof-desk`; `openspec validate --all --strict`.
  - Root: 575 tests pass. Desk: 143 pass. Both are clean.
- [x] 5.2 On the real logs (2026-11-09, load 20–105, a scratch `$TOKPROF_CACHE_DIR`):
  - **October against the summary.** `tokprof summary --since 2026-10-01 --until 2026-11-01` prints `Total priced: $11,632   avoidable on cache misses: $1,452 (12.5%)`. The card has $11,632 and $1,452: $11,631.8349 and $1,451.5425 to the cent. The summary's cost profile sums to $11,631.834870.
  - **October against its calendar cells.** The card's 1,431 messages and 78.088 h at the keyboard equal the sum of the calendar's October cells: 1,431 and 78.088 h. They also equal `tokprof calendar --since 2026-10-01 --until 2026-11-01` (`1,431` and `78.1h`). The cells' spend is $11,492.99; the other $138.84 is the night of 1 October from 00:00, which lies in 30 September's cell (see design).
  - **Speed.** The first `tokprof month` with an empty store took 59.5 s (`read … 9.2 GB in 58.3s`, load 105). The second took 0.55 s. The store is 4.9 MB.
  - **Titles.** August Night owl (34.8% of agent time at night). September Shipper (0.77 commits per agent hour). October Marathon (31 days). November so far Cache keeper (4.3% avoidable).
- [x] 5.3 Frames, looked at and fixed: the card first overflowed its height; the cache and deltas moved into rows of small numbers and the headline onto one line. Files are in `~/tokprof/plans/night-2026-11-08/shots/`:
  - `month-10-light.png`, `month-10-dark.png`, `month-09-light.png`, `month-09-dark.png`, `month-08-light.png`, `month-08-dark.png`: the 1080×1350 cards;
  - `month-window-dark.png` (October) and `month-window-light.png` (September): the card over Calendar;
  - `~/Downloads/tokprof-2026-10.png`, written by the command `Save image` runs, with a copy at `month-saved-2026-10.png`: 1080×1350, 3.3 s.
