# Tasks

Every group ends with `cargo test --workspace` green, in a state that can be committed.

## 1. The strip

- [x] 1.1 Give the strip a layout of label width, columns and rows, with lanes that fill the rows and hour-mark rows when there is room. Verify: unit tests for "In a terminal", "Lane labels", "Many sessions" and "Room between lanes".

## 2. The browser

- [x] 2.1 Compute the layout from the screen, drop the cap of 72 columns, and ask for the day again when the screen's size changes. Verify: unit tests for "Fitting the screen".
- [x] 2.2 Update the README on the browser's day view. Verify: it names how the day fits the screen.

## 3. Check on the real logs

- [x] 3.1 Open `tokprof ui` at 190x50 and 120x30 and press `d`. Verify: the strips match "How it is checked" in the proposal; record what they show.
- [x] 3.2 Run `tokprof day 2026-10-27`. Verify: the same strip as before the change.
- [x] 3.3 Reinstall the binary with `cargo install --path crates/tokprof --root ~/.local`. Verify: `tokprof ui` runs.

## Results

Measured on 2026-11-03 at about 16:50 on the author's logs, the day of 2026-10-27.

| check | before | after |
|---|---|---|
| `tokprof ui` at 190x50 | 72 columns of 20 minutes (the 72-column cap), labels of 20, 8 lanes and `8 more sessions` | 144 columns of 10 minutes, labels of 38, all 16 lanes each with a row of hour marks |
| `tokprof ui` at 120x30 | – | 72 columns of 20 minutes, labels of 24, 11 lanes and `5 more sessions` |
| `tokprof day 2026-10-27` | – | byte for byte the same as before |
