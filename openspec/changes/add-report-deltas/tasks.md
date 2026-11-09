# Tasks

## 1. The comparison (tokprof-core)

- [ ] 1.1 Total a window of the store's entries: spend, avoidable dollars, cold returns, sessions, requests, the dollars of each cause, and whether anything falls in it. Unit tests for the window's edges, the filters and an empty window.
- [ ] 1.2 Write the change from one number to another (`↑ $1,203 (+11%)`, `↓ 2 (−20%)`, `= 0`), whether it is better or worse for numbers that are better when they fall, share points, and a cause's `was … · ↓ 21%` or `new`. Unit tests for each form.

## 2. The terminal (tokprof)

- [ ] 2.1 Add `--compare` to `tokprof summary`: bring the store up to date, total the window before, and print the `compared with` line after `Total priced`. It fails without `--since` and with `--client`. Tests for the line, the failures, and output unchanged without the flag.

## 3. The window (tokprof-desk)

- [ ] 3.1 While Reports shows, ask the calendar's worker for the store. Compare the key numbers with the window before, coloured by sense, or say `no earlier data`. Desk tests with a store put in, for each case.
- [ ] 3.2 Give each `Could have been saved` row the dollars before and the change, or `new`. Desk test.
- [ ] 3.3 In `crates/tokprof-desk`, run `cargo fmt && cargo test && cargo clippy --all-targets -- -D warnings`.

## 4. Check on the real logs

- [ ] 4.1 Total the store for 2026-09-10 to 2026-10-10, and compare it with `tokprof summary`, the overview of that window, `tokprof misses` and `tokprof share` for the same dates. Record the numbers.
- [ ] 4.2 Snap Reports on 30 days, light and dark, into the night's shots folder. Look at the frames and fix what looks wrong.

## 5. Finish

- [ ] 5.1 Update README.md (Reports' deltas, `tokprof summary --compare`).
- [ ] 5.2 Run fmt, tests and clippy in both workspaces, `openspec validate --all --strict`, and the archive simulation of the five changes in order.
