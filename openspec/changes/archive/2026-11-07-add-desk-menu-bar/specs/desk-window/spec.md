# Spec Delta

## ADDED Requirements

### Requirement: Menu bar countdown setting
The settings panel SHALL offer `Menu bar countdown`, `Off` or `On`, last, after both theme grids, saved in the settings file as `menu_bar` (`off` or `on`). The default is `Off`; a value it does not know is `Off`. With `On`, the window shows a status item in the macOS menu bar at once; with `Off` it removes it at once. A frame without a window never shows the item. Tab reaches it after the second grid, and ← → pick it.

#### Scenario: Turning it on
- **WHEN** the user picks `On` in `Menu bar countdown`
- **THEN** the status item appears in the menu bar, and the settings file holds `"menu_bar": "on"`

#### Scenario: No settings yet
- **WHEN** the window starts and the settings file has no `menu_bar`
- **THEN** no status item appears

#### Scenario: A frame without a window
- **WHEN** the settings file holds `"menu_bar": "on"` and the user runs `tokprof-desk --snap out.png`
- **THEN** no status item appears

### Requirement: Sessions the menu bar counts
The menu bar SHALL count a listed session while its cache is warm and the next request would cost at least $1.00 once the cache is cold (the threshold). A session that would cost less cold, one whose cache is already cold, one still being costed, and one the filter hides do not count. The list's order and scroll do not matter.

#### Scenario: Below the threshold
- **WHEN** the only warm listed session would cost $0.84 cold
- **THEN** the menu bar counts no session

#### Scenario: Exactly the threshold
- **WHEN** a warm listed session would cost $1.00 cold
- **THEN** the menu bar counts it

### Requirement: Menu bar title
The status item's title SHALL be a snowflake `❄` and the minutes and seconds until the cache of the counted session that goes cold soonest goes cold, as the list's countdown shows them. In the last 5 minutes (the nudge time, 5:00 included) its cold price follows after ` · `. With no session counted, the title is the snowflake alone.

#### Scenario: A warm session
- **WHEN** the soonest counted session stays warm 12 minutes 40 seconds more
- **THEN** the title reads `❄ 12:40`

#### Scenario: The last minutes
- **WHEN** it stays warm 4 minutes 12 seconds more and would cost $7.19 cold
- **THEN** the title reads `❄ 4:12 · $7.19`

#### Scenario: The cache goes cold
- **WHEN** the cache of the only counted session goes cold
- **THEN** the title reads `❄` at that second

### Requirement: Menu bar menu
A click on the status item SHALL open a menu: one item per counted session, newest activity first, at most 8, each its title cut to 40 characters (its project while it has none), its countdown and its cold price; then a separator, `Open tokprof` and `Settings…`. With no session counted, the first item reads `No warm session costs $1.00 or more cold`, disabled.

#### Scenario: Nine counted sessions
- **WHEN** nine sessions count
- **THEN** the menu lists the eight with the newest activity, newest first

#### Scenario: A long title
- **WHEN** a counted session's title is `Find why the order export is slow on large accounts`
- **THEN** its item reads `Find why the order export is slow on la…  12:40 · $7.19 cold`

### Requirement: Picks in the menu bar menu
A session's item SHALL bring the window forward, shown from Day, with that session selected and the inspector from its top, as a click on its row does; an open settings panel, palette or key help closes. `Open tokprof` brings the window forward as it is. `Settings…` brings it forward with the settings panel open. A session no longer listed brings the window forward as it is.

#### Scenario: A session from Reports
- **WHEN** the window shows Reports, hidden behind another app, and the user picks a session in the menu
- **THEN** the window comes forward in Day with that session selected

### Requirement: Nudge before the cache goes cold
When the soonest counted session has 5:00 or less left, the window SHALL ask for the user's attention once: the Dock icon bounces once, while the app is not active. It asks once per session per warm period: a new request that keeps the cache warm longer starts a new period. It shows no notification and plays no sound.

#### Scenario: Reaching 5:00
- **WHEN** the soonest counted session reaches 5:00 left while the user works in another app
- **THEN** the Dock icon bounces once, and not again at 4:59

#### Scenario: Warm again
- **WHEN** the session sends a request at 3:00 left, and later again reaches 5:00 left
- **THEN** the Dock icon bounces once more

#### Scenario: Started late
- **WHEN** the window starts while the soonest counted session has 2:30 left
- **THEN** the Dock icon bounces once

### Requirement: Menu bar updates
The status item SHALL update once a second while its title shows a countdown, also while the window is hidden, minimized or covered, and the system does not delay these updates while one shows. With no session counted it sets no timer of its own; it updates when the logs change and when a counted cache goes cold.

#### Scenario: A hidden window
- **WHEN** the window is hidden and the title reads `❄ 12:40`
- **THEN** a second later it reads `❄ 12:39`
