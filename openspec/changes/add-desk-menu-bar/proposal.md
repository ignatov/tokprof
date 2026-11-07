# Proposal

## Why

On the author's last 30 days, 218 cold returns cost $749 that could have been avoided. 83 of them came after a pause of 1–2 hours and cost $260: the prompt cache lives one hour, and the author came back just after it died. The window already shows, per session, how long its cache stays warm and what one more request costs once it is cold. But the window is often hidden behind other apps, so the countdown is not seen when it matters. A countdown in the macOS menu bar is visible over any app. It lets the author come back, compact or hand off in time.

## What Changes

- **A menu bar countdown**, a status item of the window's process in the macOS menu bar. While a listed session's cache is warm and one more request would cost at least $1.00 once it is cold, its title is a snowflake and the countdown of the session that goes cold soonest, `❄ 12:40`. In its last 5 minutes the cold price joins it: `❄ 4:12 · $7.19`. With nothing warm and costly, the title is the snowflake alone.
- **Its menu**: one item per such session, newest first, at most 8, with the title cut to 40 characters, the countdown and the cold price. A click brings the window forward with that session selected, as a click on its row does. Then a separator, `Open tokprof` and `Settings…`.
- **A nudge**: when the soonest of those sessions reaches 5:00 left, the Dock icon bounces once (an informational attention request). Once per session per warm period. No notifications: they need a permission prompt.
- **A setting**: `Menu bar countdown` in the settings panel, `Off` or `On`, saved as `menu_bar` (`off`, `on`). Off by default. A frame without a window never makes the item.
- **Updates**: once a second while a countdown shows, also while the window is hidden or covered; nothing at rest.

## Capabilities

### New Capabilities
None.

### Modified Capabilities
- `desk-window`: adds the menu bar countdown, its menu, the nudge and the setting. No existing requirement changes; the settings panel gains one part (see design.md).

## Impact

- `crates/tokprof-desk`: `src/menubar.rs` (the rules, unit-tested, and the AppKit glue, macOS only), the setting in `src/settings.rs`, hooks in `src/main.rs` (`poll`, the app handle), README. objc2 0.6.5, objc2-app-kit 0.3.2 and objc2-foundation 0.3.2 become direct dependencies on macOS, the versions Rune already builds.
- No change to the browser, the core, the log sources or the CLI.

## How it is checked on the real logs

- Unit tests of the rules: which sessions count ($1.00 threshold), the soonest, the title at 12:40, 5:00 and 4:12 left, the menu rows and their order, the nudge firing once per session per warm period.
- The window from a release build, with `menu_bar` on in a scratch settings folder, on the author's logs for about 60 s: the item appears, its title follows the soonest warm session that costs $1.00 or more cold, as the list's next cell shows it, and the process quits cleanly. Recorded in tasks.md.
- The author looks at the real menu bar in the morning (an unticked task).

## Out of scope

- Notifications, sounds, and a countdown in the Dock icon.
- Linux and Windows trays (Rune has them; the window ships for macOS only).
- A configurable threshold or nudge time: both are constants for now.
- Keeping the process alive after the window closes: closing the window still quits, and the item goes with it.
