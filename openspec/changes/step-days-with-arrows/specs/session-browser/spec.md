# Spec Delta

## MODIFIED Requirements

### Requirement: Day view in the browser
`tokprof ui` SHALL show the day view of work-rhythm full screen when `d` is pressed. Esc, `d` or Backspace go back to the list with the X-ray. `←` or `h` steps to the day before and `→` or `l` to the day after, as do `[` and `]`, never past the current day, keeping the hours of the first day shown. The day is computed in the background, one at a time, and its strip fits the screen. While the current day is shown, it is computed again after a session writes to its log.

#### Scenario: The first press
- **WHEN** `d` is pressed
- **THEN** the screen is titled ` day · Tue 2026-11-03 ` and shows the day of the user's latest action, as `tokprof day` prints it with the browser's filters
- **AND** until it is ready the screen reads `computing the day of 2026-11-03…`

#### Scenario: Stepping through days
- **WHEN** `←` is pressed twice and then `→` once
- **THEN** the screen shows the day before the current one, and a `→` on the current day does nothing
- **AND** `h`, `[`, `l` and `]` step the same way
- **AND** when the first day shown starts at 04:24 and the day before at 05:14, both strips run from 04:00

#### Scenario: Full screen
- **WHEN** the day is shown
- **THEN** it takes the whole screen and scrolls as the full-screen X-ray does, and esc goes back to the list with the X-ray
- **AND** `←` steps to the day before rather than back to the list

#### Scenario: Fitting the screen
- **WHEN** the screen is 190 columns by 50 rows and the day has 16 sessions
- **THEN** labels take 38 characters, a fifth of the width, and the strip 149 columns of which 144 hold 10-minute columns
- **AND** all 16 lanes show, each followed by a row of hour marks, since the 32 rows the screen leaves for lanes fit them
- **AND** labels take at least 20 characters and at most 40, and the screen leaves for lanes its rows less 18

#### Scenario: Keys in the footer
- **WHEN** the list is shown
- **THEN** the footer names `d day` after the other keys
- **AND** while the day is shown it reads ` ↑↓ PgUp PgDn scroll · ← → days · esc back · f JFR · o HTML · c handoff · q quit`

#### Scenario: A live day
- **WHEN** the current day is shown and a session writes to its log
- **THEN** the day is computed again, after a pause at least as long as its last computation took, and the title adds ` · updating…` until the new one is ready

#### Scenario: Coming back
- **WHEN** a day that was computed is shown again and no session wrote to its log since
- **THEN** it shows at once
