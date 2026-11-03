# Tasks

Every group ends with `cargo test --workspace` green, in a state that can be committed.

## 1. The strip

- [x] 1.1 Draw 24 hours from a given start, by default the whole hour at or before the day's start, in the shortest column that fits the width. Verify: unit tests for "Columns", "A narrow pane", "A wide screen" and "Hours row".
- [x] 1.2 Keep each column's kind, and render runs of a kind with its mark and, with color, its code; the legend the same way. Verify: unit tests for "Marks", "Colors" (with and without color) and "Legend".

## 2. The views

- [x] 2.1 Let `tokprof day` paint when its output is a terminal and `NO_COLOR` is unset. Verify: `tokprof day | cat` has no escape code.
- [x] 2.2 Show the day full screen in the browser on `d`, leave it with `d`, esc, `←`, `h` or Backspace, and keep the hour of the first day shown while stepping. Verify: unit tests for "The first press", "Stepping through days", "Full screen" and "Keys in the footer".
- [x] 2.3 Update the README's day example and the browser's keys. Verify: the README matches `tokprof day 2026-11-02` with the names left out.

## 3. Check on the real logs

- [x] 3.1 Run `tokprof day 2026-11-02`. Verify: 04:00 to 04:00 in 20-minute columns, and the rhythm lines as before; record them.
- [x] 3.2 Open `tokprof ui` at 200 columns, press `d`, then `[`. Verify: today full screen in 10-minute columns, and the day before with the same hours in the same columns; record it.
- [x] 3.3 Reinstall the binary with `cargo install --path crates/tokprof --root ~/.local`. Verify: `tokprof day` and `tokprof ui` run.

## Results

Measured on 2026-11-03 at about 16:00 on the author's logs.

| check | before | after |
|---|---|---|
| `tokprof day 2026-11-02`, strip | 08:00–23:00 in 15-minute columns, marks `= - * ? ! #` | 04:00–04:00 in 20-minute columns, 72; marks `█ ░ * ? ! ▓` |
| its rhythm lines | 16 sittings, 4.1h, 59 messages to 12 sessions; 5 cold returns, $15.48 | the same |
| `tokprof day 2026-11-02 \| cat` | – | no escape code |
| `tokprof ui` at 200x50, `d` | the day in the right pane | the day on the whole screen after 0.61s, hours from 04; 10-minute columns, since 200 less 23 columns of labels and borders fit 144 |
| then `[` 7 times | – | 2026-10-27, which starts at 05:14, drawn from 04 as the first day; `tokprof day 2026-10-27` alone starts at 05 |
