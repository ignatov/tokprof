# Tasks

Every group ends with `cargo test --workspace` green, in a state that can be committed.

## 1. Before starting

- [x] 1.1 Record the baseline in the results table: the median of 3 runs of `tokprof day` and of `tokprof day 2026-11-02`, the GB each reads, the lines of 2026-11-02, and the rhythm lines of `tokprof --agent claude --since 30d pauses`. Verify: the table has the numbers and the date.

## 2. Nights and the strip

- [x] 2.1 Find each day's night from the actions around it, with the day's bounds and the night line, and remove the half-hour count. Verify: unit tests for "The author's night", "A pause in the afternoon", "A night that is not empty", "Too few days", "A day with one action" and the three lines of "The night line".
- [x] 2.2 Use these days in the summary's same-day rules, the X-ray's rhythm line, and the pause report's `nights` line and hour table. Verify: unit tests for "Nights" and "Hours", and the existing rhythm tests still pass.
- [x] 2.3 Let the strip take the most columns it may use. Verify: unit tests for "Columns" with 72 and for "A narrow pane".

## 3. The day view reads around its day

- [x] 3.1 Pick the main logs by their write time, creation and last line with a time, for a day or for the latest action's day. Verify: unit tests for "The logs of a day" and "Today before the start of the day".
- [x] 3.2 Move the building of the day view's text into one function that `tokprof day` and the browser call. Verify: `tokprof day 2026-11-02` prints the baseline's lines, from its new start at 04:21.

## 4. The browser

- [x] 4.1 Add the day worker, the `d`, `[` and `]` keys, the pane title and the waiting text, the full-screen day, and the footer keys. Verify: unit tests for "The first press", "Stepping through days", "Full screen" and "Keys in the footer".
- [x] 4.2 Compute the current day again after a session writes to its log, and show a computed day again at once. Verify: unit tests for "A live day" and "Coming back".
- [x] 4.3 Update README: how the nights cut the days, what `day` reads, and the day view in the browser with its keys. Verify: the README examples run as written.

## 5. Check on the real logs

- [x] 5.1 Run `tokprof day` 3 times and `tokprof day 2026-11-02` once. Verify: today reads about 0.1 GB in under a second, and 2026-11-02 prints the baseline's lines from about 0.2 GB; record the numbers.
- [x] 5.2 Run `tokprof --agent claude --since 30d pauses`. Verify: the rhythm lines are within the proposal's tolerances of the baseline, and the `nights` line reads a median near 05:14; record it.
- [x] 5.3 Open `tokprof ui`, press `d`, then `[`. Verify: today shows within a second, and 2026-11-02 shows the lines of `tokprof day 2026-11-02`; record what the pane shows.
- [x] 5.4 Reinstall the binary with `cargo install --path crates/tokprof --root ~/.local`. Verify: `tokprof day` and `tokprof ui` run.

## Results

Measured on 2026-11-03 between 14:40 and 15:30 on the author's logs; "before" is the binary of 0142c0f.

| check | before | after |
|---|---|---|
| `tokprof day`, 3 runs | 14.82s, 14.81s, 14.63s; read 320 + 177 logs, 6.2 GB | 0.62s, 0.49s, 1.12s; read 50 + 3 logs, 0.2 GB (other runs 0.38–0.42s) |
| `tokprof day 2026-11-02`, 3 runs | 14.81s, 14.14s, 14.69s; 6.2 GB | 1.18s, 1.27s, 0.79s; 49 + 3 logs, 0.2 GB |
| day of 2026-11-02 | 05:00 → 05:00 | 04:21 → 04:24 (night 00:22–08:20 before, 22:45–10:03 after) |
| its lines | 16 sittings, 4.1h, 59 messages to 12 sessions, sitting 11m, break 28m; left 19m, 68%; every 6.6m, 3%; turn 6.4m, answer 3.2m; 2 interrupts, 3 questions in 2.7m; 5 cold returns, $15.48, $14.91 | the same |
| `--agent claude --since 30d pauses` | night 02:00–08:00, start 05:00; 313 sittings, 6.6m, 34m; left 6.5m, 22%; every 4.2m, 32%; turn 3.3m, answer 2.9m; 40, 35, 75 in 2.4m | nights: median start 05:14; the same lines but left 6.4m |
| `tokprof ui`, `d` | – | today after 0.62s (2.89s before the day worker got a pool of its own, behind the costing of every session) |
| `tokprof ui`, `[` | – | 2026-11-02 after 0.51s, 16 sittings, 4.1h, 59 messages to 12 sessions |
