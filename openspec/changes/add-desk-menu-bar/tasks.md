# Tasks

## 1. The rules

- [x] 1.1 `src/menubar.rs`: the constants (threshold $1.00, nudge 5:00, 8 items, 40 characters), the sessions that count, the soonest, the title, the menu rows newest first, and the nudge once per session per warm period. Unit-test each, with the edges: exactly $1.00, exactly 5:00, a cache gone cold, nine sessions, a long title, a new warm period, a start inside the last 5 minutes.
- [x] 1.2 The warm sessions from the browser: listed, warm, the cold price at the moment the cache goes cold.

## 2. The setting

- [x] 2.1 `menu_bar` in the settings (`off` by default, unknown values off), saved as `off`/`on`; the `Menu bar countdown` part of the panel last, after both grids, Tab and ← →; the key help's Settings line names it. Tests: the default, a saved `on` read back, the keys.

## 3. The menu bar

- [x] 3.1 macOS glue: the status item with a monospaced-digit title, the menu kept in place, the target class for picks, the Dock bounce, the App Nap activity while a countdown shows; removed when the setting turns off. Only with a window, on the main thread.
- [x] 3.2 `poll`: update the item, wake at the next second while a countdown shows, nothing more at rest; picks select the session in Day, close any card and bring the window forward through the app handle; `Open tokprof` and `Settings…`.
- [x] 3.3 Runtime tests: a pick selects the session from Reports and closes a card; `Settings…` opens the panel; a frame without a window makes no item.
- [x] 3.4 Fix from code review 2: while a session was warm but none counted (under $1, or still being costed), `poll` passed over the whole list every frame. The pass now runs once a second whenever anything is warm, and wakes nothing at rest; `the_list_is_passed_over_once_a_second_while_anything_is_warm` tests the throttle.

## 4. Check and document

- [x] 4.1 `cargo fmt`, `cargo test`, `cargo clippy --all-targets -- -D warnings` in `crates/tokprof-desk`; `cargo test -p tokprof --test architecture` at the root.
  - 2026-11-07: the window's 121 tests pass (7 rules in `menubar.rs`, 1 setting in `settings.rs`, 2 runtime tests), fmt and clippy clean; the architecture test passes. The lockfile gains no new crate: objc2 0.6.5, objc2-app-kit 0.3.2 and objc2-foundation 0.3.2 become direct dependencies.
- [x] 4.2 Run the release build with `menu_bar` on in a scratch settings folder on the real logs for about 60 s: it starts, makes the item, sets the title that the list's next cell agrees with, and quits cleanly when killed. Record the titles.
  - 2026-11-07, `RUNE_PREFS_DIR` with `"menu_bar": "on"`, the release build with the title logged to stderr (logging removed before the commit): the item was made on the first poll with `❄` alone while the sessions were costed, then `❄ 59:16` with one row `<title A>  59:16 · $6.51 cold` within 0.4 s; a second session joined, and at 60 s the title read `❄ 55:05` (the soonest, `<title B>  55:05 · $5.79 cold`), the menu `<title A>  59:29 · $6.55 cold` first as the newer (titles left out here). 61 distinct titles in 60 s, one a second. `tokprof ui --dump` at the same time shows `≈ $5.79 once cold` for that session. No nudge (both had over 5 minutes). SIGTERM ended the process (status 143), none left. With the setting off, no item was made. `display link creation failed: -6661` is printed with the setting off too.
- [x] 4.3 README of the window: the menu bar countdown, its menu, the nudge and the setting.
- [ ] 4.4 The author, in the morning: turn `Menu bar countdown` on, look at the real menu bar (the glyph, the countdown not shifting, the price in the last 5 minutes, the menu and its picks, the bounce at 5:00 with the window behind another app), and decide whether it should be on by default.
