# Spec Delta

## MODIFIED Requirements

### Requirement: Opening the window
`tokprof-desk` SHALL open one window titled `tokprof`, 1400 by 900 points the first time and no smaller than 960 by 640, listing every session that `tokprof ui` lists with the same `--project`, `--session`, `--since` and `--until` filters. It starts with the newest session selected and the day of the latest action in the lanes. Its colors are those of the theme the Color themes requirement picks.

#### Scenario: Default start
- **WHEN** the user runs `tokprof-desk`
- **THEN** a window opens with the lanes of the latest day on top, the list of sessions below them, and the inspector of the newest session

#### Scenario: Filters
- **WHEN** the user runs `tokprof-desk --project json-profiler --since 7d`
- **THEN** the list holds only the sessions that `tokprof ui --project json-profiler --since 7d` lists

#### Scenario: No logs
- **WHEN** no session logs are found
- **THEN** no window opens, and the command fails with `no session logs found`

#### Scenario: The system turns dark
- **WHEN** the Appearance setting is `As in system` and the system switches from light to dark while the window is open
- **THEN** the window switches to its dark theme without a restart

### Requirement: Frame options
For a frame without a window, `--size <W>x<H>` SHALL set the frame in points (default 1400x900), `--theme light` or `--theme dark` its appearance, `--colors <theme id>` its theme, `--day <date>` the day in the lanes, `--tab <name>` the inspector's tab, and `--tree <json>` write the frame's elements with their text and places.

#### Scenario: The day
- **WHEN** the user runs `tokprof-desk --snap day.png --day 2026-11-03`
- **THEN** the lanes show 2026-11-03

#### Scenario: A bad size
- **WHEN** the user runs `tokprof-desk --snap out.png --size 1400`
- **THEN** the command fails with `--size takes WIDTHxHEIGHT`

#### Scenario: Another theme
- **WHEN** the user runs `tokprof-desk --snap out.png --colors ink-meridian`
- **THEN** `out.png` is drawn in Ink Meridian, a dark theme, whatever `--theme` says

#### Scenario: An unknown theme
- **WHEN** the user runs `tokprof-desk --snap out.png --colors nosuch`
- **THEN** the command fails with `--colors takes one of:` and the ids of the themes

## ADDED Requirements

### Requirement: Color themes
The window SHALL carry the 41 color themes of Rex Beta 0.1.0 (build 1829), 14 light and 27 dark, each with its id, name and appearance as Rex lists them. It draws in the chosen light theme when the appearance is light and in the chosen dark theme when it is dark, with the colors Rex draws for the theme.

#### Scenario: The defaults
- **WHEN** nothing is chosen yet and the system is light
- **THEN** the window is drawn in Merino Light, and in Merino Dark when the system is dark

#### Scenario: Colors as Rex shows them
- **WHEN** the window is drawn in Merino Light
- **THEN** its background and accent are the colors Rex draws for Merino Light's canvas and accent

### Requirement: Marks in a theme
A theme SHALL give the window's background, panels, lines, text and accent, and the colors of its marks: green for work and warm caches, red for cold caches, the now line and stops, yellow for waits and cache writes, blue for cache reads and requests, cyan for output and messages, magenta for questions.

#### Scenario: A cold cache
- **WHEN** a listed session's cache is cold
- **THEN** its next-request cell is drawn in the theme's red

### Requirement: Settings panel
`Settings…` in the app menu and ⌘, SHALL open the settings panel, a card in the middle of the window over a dimmed window. While it is open, the keys and clicks go to the panel only, and ⌘Q still quits. ⌘, again, esc, its `Done` button and a click outside the card close it, and the window is as it was before it opened.

#### Scenario: Opening the settings
- **WHEN** the user presses ⌘,
- **THEN** the settings panel opens over the window, with the current choices marked

#### Scenario: Keys while it is open
- **WHEN** the settings panel is open and the user types `q`
- **THEN** the window stays open and nothing behind the panel changes

#### Scenario: Closing the settings
- **WHEN** the user presses esc in the settings panel
- **THEN** the panel closes, and the same session, tab and day are shown as before it opened

### Requirement: Appearance and theme settings
The settings panel SHALL offer `Appearance` with `As in system` (the default), `Light` and `Dark`, a list `Light theme` of the light themes and a list `Dark theme` of the dark ones, each row with the theme's name and a strip of its background, text, accent, green, yellow and red. A click picks a choice, and the window takes it at once. Under `As in system` the window follows the system, also when it changes. The close, minimize and zoom buttons keep the system's appearance.

#### Scenario: A dark theme on a light system
- **WHEN** the system is light and the user picks `Dark` and then `Ink Meridian` in `Dark theme`
- **THEN** the window turns dark at once and is drawn in Ink Meridian while the system stays light

#### Scenario: A light theme while dark is shown
- **WHEN** the window is dark and the user picks `Alpine Milk` in `Light theme`
- **THEN** the window stays dark, and it is drawn in Alpine Milk once it turns light

### Requirement: Keys in the settings
In the settings panel, Tab and Shift+Tab SHALL move the keyboard between `Appearance`, `Light theme` and `Dark theme`; ← and → pick the appearance; ↑ and ↓ move through a theme list, scrolled into view, and pick the theme they reach; ⏎ closes the panel.

#### Scenario: The next dark theme
- **WHEN** `Dark theme` has the keyboard on Merino Dark and the user presses ↓
- **THEN** the dark theme after Merino Dark is picked and the window takes it at once

### Requirement: Saved settings
The window SHALL keep its settings in `prefs.json` in `~/Library/Application Support/io.github.ignatov.tokprof/`: `appearance` as `system`, `light` or `dark`, and `light_theme` and `dark_theme` as theme ids. It writes the file only when a choice changes and reads it when it starts. A missing or broken file gives the defaults, and a value it does not know gives that setting's default.

#### Scenario: The next launch
- **WHEN** the user picks `Light` and `Alpine Milk`, quits the window and starts it again
- **THEN** the window opens light, in Alpine Milk

#### Scenario: No settings yet
- **WHEN** the window starts and the settings file does not exist
- **THEN** the window follows the system in Merino Light and Merino Dark, and no file is written until a choice changes

#### Scenario: The file cannot be written
- **WHEN** the user picks a theme and the settings file cannot be written
- **THEN** the window takes the theme until it quits, and the bar says `can't save settings`

### Requirement: Settings in a frame without a window
A frame without a window (`--snap`, `--tree`) SHALL NOT read or write the saved settings: its appearance comes from `--theme`, or from the system without it, and its theme from `--colors`, or the default themes without it. `--scene settings` draws the frame with the settings panel open on the default choices.

#### Scenario: A saved dark choice
- **WHEN** the saved appearance is `Dark` and the user runs `tokprof-desk --snap out.png --theme light`
- **THEN** `out.png` is drawn in Merino Light

#### Scenario: The panel in a frame
- **WHEN** the user runs `tokprof-desk --snap out.png --scene settings`
- **THEN** `out.png` shows the settings panel over the window, with `As in system`, `Merino Light` and `Merino Dark` marked
