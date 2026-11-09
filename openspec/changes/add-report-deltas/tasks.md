# Tasks

## 1. The comparison (tokprof-core)

- [x] 1.1 Total a window of the store's entries: spend, avoidable dollars, cold returns, sessions, requests, the dollars of each cause, and whether anything falls in it. Unit tests for the window's edges, the filters and an empty window.
- [x] 1.2 Write the change from one number to another (`↑ $1,203 (+11%)`, `↓ 2 (−20%)`, `= 0`), whether it is better or worse for numbers that are better when they fall, share points, and a cause's `was … · ↓ 21%` or `new`. Unit tests for each form.

## 2. The terminal (tokprof)

- [x] 2.1 Add `--compare` to `tokprof summary`: bring the store up to date, total the window before, and print the `compared with` line after `Total priced`. It fails without `--since` and with `--client`. Tests for the line, the failures, and output unchanged without the flag.

## 3. The window (tokprof-desk)

- [x] 3.1 While Reports shows, ask the calendar's worker for the store. Compare the key numbers with the window before, coloured by sense, or say `no earlier data`. Desk tests with a store put in, for each case.
- [x] 3.2 Give each `Could have been saved` row the dollars before and the change, or `new`. Desk test.
- [x] 3.3 In `crates/tokprof-desk`, run `cargo fmt && cargo test && cargo clippy --all-targets -- -D warnings`.

## 4. Check on the real logs

- [x] 4.1 Total the store for 2026-09-10 to 2026-10-10, and compare it with `tokprof summary`, the overview of that window, `tokprof misses` and `tokprof share` for the same dates. Record the numbers.
- [x] 4.2 Snap Reports on 30 days, light and dark, into the night's shots folder. Look at the frames and fix what looks wrong.

Recorded 2026-11-09 on the author's logs (days-2 store, 1,962 logs):
- 4.1: for 2026-09-10 to 2026-10-10 the store and the overview of that window agree exactly. Spend $5,283.82, avoidable $597.36 (11.3%), cold returns 106, sessions 62, requests 44,832. `tokprof summary` prints `Total priced: $5,284   avoidable on cache misses: $597 (11.3%)`, and its requests add up to 44,832.
  - The cause rows equal the overview's `saved` rows to the cent: compaction call $367.17, idle > TTL $223.40, model switch $4.41, unknown $2.33, params changed $0.06.
  - `tokprof misses` for the same dates gives compaction call $367, idle > TTL $163 + $60.22, model switch $4.41, unknown $2.20 + $0.13, params changed $0.06.
  - The two `cache lifetime` rows are not miss causes and have no comparison.
  - Totalling a window over the store takes 0.5 ms. `tokprof summary` without `--compare` prints the same bytes as before. `tokprof summary --since 2026-10-10 --until 2026-11-09 --compare` reads `compared with 10 Sep – 9 Oct 2026: spend ↑ $6,722 (+127%) · avoidable ↑ $725 (+121%), ↓ 0.3 points of spend`.
- 4.2: frames ~/tokprof/plans/night-2026-11-08/shots/deltas-{light,dark}.png. The look fix: `vs <dates>` in every tile overflowed the avoidable tile, so the dates moved to the card's note `against …`.

## 5. Finish

- [x] 5.1 Update README.md (Reports' deltas, `tokprof summary --compare`).
- [x] 5.2 Run fmt, tests and clippy in both workspaces, `openspec validate --all --strict`, and the archive simulation of the five changes in order.
