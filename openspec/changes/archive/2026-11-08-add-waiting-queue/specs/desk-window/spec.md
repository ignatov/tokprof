# Spec Delta

## ADDED Requirements

### Requirement: Waiting in the window
While the `Waiting order` setting is `On`, the list's head SHALL offer `Waiting` after `Time`, `Cost` and `Next`, sorting as session-browser's waiting order. In it, `Waiting for you` heads the waiting sessions and `Others` the rest, as the day labels head rows in time order, and a waiting row shows its wait, like `16m`, in place of its time. The palette offers `Sort by Waiting` then. With no session waiting, the list reads `Nobody waits for you` above `Others`.

#### Scenario: The setting off
- **WHEN** `Waiting order` is `Off`
- **THEN** the head offers `Time`, `Cost` and `Next`, `s` cycles those three, no row shows a wait, and the palette has no `Sort by Waiting`

#### Scenario: Three sessions wait
- **WHEN** `Waiting order` is `On` and the user clicks `Waiting` while three sessions wait, for 40m, 16m and 2m
- **THEN** the list starts with `Waiting for you` and rows reading `40m`, `16m` and `2m` in place of their times, then `Others` and the rest newest first

#### Scenario: From the palette
- **WHEN** the user picks `Sort by Waiting` in the palette
- **THEN** the list is sorted by waiting, as the `Waiting` button sorts it

#### Scenario: Nobody waits
- **WHEN** the user clicks `Waiting` and no session waits
- **THEN** the list reads `Nobody waits for you`, then `Others` and every session newest first

#### Scenario: A frame in waiting order
- **WHEN** the user runs `tokprof-desk --snap out.png --scene waiting`
- **THEN** `out.png` shows the list in waiting order, as with the setting `On`, without reading the saved settings

### Requirement: Waiting order setting
The settings panel SHALL offer `Waiting order`, `Off` or `On`, last, after `Menu bar countdown`, saved in the settings file as `waiting` (`off` or `on`). The default is `Off`; a value it does not know is `Off`. Picking `Off` while the list is sorted by waiting sorts it by time. Tab reaches it after `Menu bar countdown`, and ← → pick it.

#### Scenario: Turning it on
- **WHEN** the user picks `On` in `Waiting order`
- **THEN** the list's head offers `Waiting` at once, and the settings file holds `"waiting": "on"`

#### Scenario: Turning it off in waiting order
- **WHEN** the list is sorted by waiting and the user picks `Off`
- **THEN** the list is sorted by time and `Waiting` leaves the head

#### Scenario: No settings yet
- **WHEN** the window starts and the settings file has no `waiting`
- **THEN** the head offers `Time`, `Cost` and `Next` only

## MODIFIED Requirements

### Requirement: Menu bar countdown setting
The settings panel SHALL offer `Menu bar countdown`, `Off` or `On`, after both theme grids, saved in the settings file as `menu_bar` (`off` or `on`). The default is `Off`; a value it does not know is `Off`. With `On`, the window shows a status item in the macOS menu bar at once; with `Off` it removes it at once. A frame without a window never shows the item. Tab reaches it after the second grid, and ← → pick it.

#### Scenario: Turning it on
- **WHEN** the user picks `On` in `Menu bar countdown`
- **THEN** the status item appears in the menu bar, and the settings file holds `"menu_bar": "on"`

#### Scenario: No settings yet
- **WHEN** the window starts and the settings file has no `menu_bar`
- **THEN** no status item appears

#### Scenario: A frame without a window
- **WHEN** the settings file holds `"menu_bar": "on"` and the user runs `tokprof-desk --snap out.png`
- **THEN** no status item appears

### Requirement: What the lanes say
Under the lanes SHALL be the legend, and the `sittings`, `answers`, `queue` and `cache` lines of the day as `tokprof day` prints them. A line under them names what is under the pointer: the time, the lane and what it shows there.

#### Scenario: The time under the pointer
- **WHEN** the pointer rests over a lane at 14:32
- **THEN** the line reads like `14:32 · ecad11 Chart of the interruptions · agent working`, with `agent working`, `waiting for your answer`, `your message`, `its question`, `you stopped it`, `you were there`, `cache warm` or nothing

#### Scenario: The queue line
- **WHEN** the shown day had 24 answers among several sessions waiting (see work-rhythm)
- **THEN** the `queue` line under `answers` reads as `tokprof day` prints it for that day
