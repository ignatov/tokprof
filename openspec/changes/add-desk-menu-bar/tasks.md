# Tasks

## 1. The rules

- [ ] 1.1 `src/menubar.rs`: the constants (threshold $1.00, nudge 5:00, 8 items, 40 characters), the sessions that count, the soonest, the title, the menu rows newest first, and the nudge once per session per warm period. Unit-test each, with the edges: exactly $1.00, exactly 5:00, a cache gone cold, nine sessions, a long title, a new warm period, a start inside the last 5 minutes.
- [ ] 1.2 The warm sessions from the browser: listed, warm, the cold price at the moment the cache goes cold.

## 2. The setting

- [ ] 2.1 `menu_bar` in the settings (`off` by default, unknown values off), saved as `off`/`on`; the `Menu bar countdown` part of the panel after `Dark theme`, Tab and ← →. Tests: the default, a saved `on` read back, the keys.

## 3. The menu bar

- [ ] 3.1 macOS glue: the status item with a monospaced-digit title, the menu kept in place, the target class for picks, the Dock bounce, the App Nap activity while a countdown shows; removed when the setting turns off. Only with a window, on the main thread.
- [ ] 3.2 `poll`: update the item, wake at the next second while a countdown shows, nothing more at rest; picks select the session in Day, close any card and bring the window forward through the app handle; `Open tokprof` and `Settings…`.
- [ ] 3.3 Runtime tests: a pick selects the session from Reports and closes a card; `Settings…` opens the panel; a frame without a window makes no item.

## 4. Check and document

- [ ] 4.1 `cargo fmt`, `cargo test`, `cargo clippy --all-targets -- -D warnings` in `crates/tokprof-desk`; `cargo test -p tokprof --test architecture` at the root.
- [ ] 4.2 Run the release build with `menu_bar` on in a scratch settings folder on the real logs for about 60 s: it starts, makes the item, sets the title that the list's next cell agrees with, and quits cleanly when killed. Record the titles.
- [ ] 4.3 README of the window: the menu bar countdown, its menu, the nudge and the setting.
- [ ] 4.4 The author, in the morning: turn `Menu bar countdown` on, look at the real menu bar (the glyph, the countdown not shifting, the price in the last 5 minutes, the menu and its picks, the bounce at 5:00 with the window behind another app), and decide whether it should be on by default.
