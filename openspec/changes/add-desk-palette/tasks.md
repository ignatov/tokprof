# Tasks

## 1. The overlays' look

- [x] 1.1 Add `overlay.rs` with the scrim, the card, `keycap` and `hint`, moved from the settings panel, and draw the settings panel with them; the settings tests still pass.

## 2. The palette

- [x] 2.1 Write the fuzzy matcher in `palette.rs` as the design says. Unit-test: case and spaces ignored, letters out of order fail, a prefix beats a word start, a word start beats a run inside a word, a run beats scattered letters, the places are those of the best placement.
- [x] 2.2 Build the entries: actions, tabs, sessions, days, with the empty-query order and limits and the query's order and limits; the actions offered by the state (filter, full size). Unit-test the limits and the order on a desk of 150 sessions.
- [x] 2.3 Run a pick: a session (also from full size), a tab, a day, each action through the code its key or button runs.
- [x] 2.4 Keys: ⌘K and `Command Palette…` in the app menu; typed text, Backspace, ↑ ↓, Ctrl+P, Ctrl+N, ⏎, esc; ⌘Q quits; nothing else reaches the window. A filter being typed ends when the palette opens.
- [x] 2.5 Draw the palette as the design says, with the matched letters in the accent, the click on a row and the click outside.
- [x] 2.6 Runtime tests: ⌘K opens; typing filters and ranks; ↓ ↓ ⏎ runs the third entry; a session pick selects and scrolls to it with its Summary; a tab pick; an action pick; esc closes; `q` and `j` do not leak; a click outside closes without selecting; ⌘, does nothing while it is open and ⌘K does nothing while the settings are.

## 3. The key help

- [x] 3.1 Write the key groups in `keys.rs` from `on_key`, `lanes_key`, `inspector_key`, `settings_key`, the palette's keys and the browser's `on_key`.
- [x] 3.2 `?` opens it where no filter is typed and no overlay is open; esc and `?` close it; other keys stay with it; ⌘Q quits. Draw it as the design says.
- [x] 3.3 Runtime tests: `?` opens and closes it; `?` while typing a filter is typed; it lists the keys the browser and the window handle.

## 4. Check and document

- [x] 4.1 Add the scenes `palette` and `keys`; render both in light and dark with the release build into `~/tokprof/plans/night-2026-11-06/shots/` and look at them.
  - 2026-11-06: `palette-light.png`, `palette-dark.png`, `keys-light.png`, `keys-dark.png`, and `settings-light.png` to check the panel after the move to the shared look. Fixed after looking: the key help's card collapsed to its head (it now takes the height its groups need), its columns were unbalanced, loose matches and a second `Today` buried the tight ones in the palette.
- [x] 4.2 Run `cargo fmt`, `cargo test` and `cargo clippy --all-targets -- -D warnings` in `crates/tokprof-desk`, and `cargo test -p tokprof --test architecture` at the root.
- [ ] 4.3 On the real logs, check what the proposal's "How it is checked" lists, and record it here.
  - 2026-11-06, headless on the author's logs (838 listed sessions), with `--scene palette` and the query changed for the run: `tok` lists the sessions with `tok` in their title or project, `Tokprof v0.1.0 log survey dry-run` first, with the costs their rows show; `yest` puts `Yesterday` (5 November, 4 sessions, $27.39) first; `cache` puts the `Cache` tab first. Building the entries took 4 to 7 ms per view run. Left for the author: the same by hand in the installed window, and trying each key of the key help.
- [x] 4.4 Update `crates/tokprof-desk/README.md`: the palette, the key help, the scenes, the new files.
- [ ] 4.5 Reinstall the window the author runs with `bundle.sh`, once the author has landed the change.
