# Tasks

Every group ends with `cargo test --workspace` green, in a state that can be committed.

## 1. The keys

- [x] 1.1 In the day view, step days with `←`, `h`, `[` and `→`, `l`, `]`, and go back with esc, `d` and Backspace. Verify: unit tests for "Stepping through days" and "Full screen".
- [x] 1.2 Show the day's keys in the footer. Verify: a unit test for "Keys in the footer".
- [x] 1.3 Update the README's table of keys. Verify: it names `←` `→` for days and esc for back.

## 2. Check on the real logs

- [x] 2.1 Drive `tokprof ui` at 190x50 in a pty: `d`, `←` twice, `→` three times, esc. Verify: the titles and the list of the proposal; record them.
- [x] 2.2 Reinstall the binary with `cargo install --path crates/tokprof --root ~/.local`. Verify: `tokprof ui` runs.

## Results

| check | proposal | after |
|---|---|---|
| `d` | ` day · Tue 2026-11-03 ` | ` day · Tue 2026-11-03 ` |
| `←`, `←` | Sun 2026-11-01 | Mon 2026-11-02, then Sun 2026-11-01 |
| `→`, `→`, `→` | Tue 2026-11-03, and the third stays there | Mon 2026-11-02, Tue 2026-11-03, Tue 2026-11-03 |
| esc | the list with the X-ray | the list with the X-ray, and the list's footer |
| the day's footer | ` ↑↓ PgUp PgDn scroll · ← → days · esc back · f JFR · o HTML · c handoff · q quit` | the same |
| `tokprof ui` after the reinstall | runs | runs |
| `cargo test --workspace`, clippy with `-D warnings` | green | green |
