# Spec Delta

## ADDED Requirements

### Requirement: Day view in the browser
`tokprof ui` SHALL show the day view of work-rhythm in the right pane when `d` is pressed, and the X-ray again on the next `d`. `[` and `]` step to the day before and after, never past the current day. The day is computed in the background, one at a time, and its strip fits the pane's width. While the current day is shown, it is computed again after a session writes to its log.

#### Scenario: The first press
- **WHEN** `d` is pressed
- **THEN** the right pane is titled ` day · Tue 2026-11-03 ` and shows the day of the user's latest action, as `tokprof day` prints it with the browser's filters
- **AND** until it is ready the pane reads `computing the day of 2026-11-03…`

#### Scenario: Stepping through days
- **WHEN** `[` is pressed twice and then `]` once
- **THEN** the pane shows the day before the current one, and a `]` on the current day does nothing

#### Scenario: Full screen
- **WHEN** `⏎` is pressed while the day is shown
- **THEN** the day shows full screen and scrolls as the full-screen X-ray does, and `[`, `]` and `d` work there too

#### Scenario: Keys in the footer
- **WHEN** the list or the day is shown
- **THEN** the footer names `d day` after the other keys, and `[ ] days` while the day is shown

#### Scenario: A live day
- **WHEN** the current day is shown and a session writes to its log
- **THEN** the day is computed again, after a pause at least as long as its last computation took, and the title adds ` · updating…` until the new one is ready

#### Scenario: Coming back
- **WHEN** a day that was computed is shown again and no session wrote to its log since
- **THEN** it shows at once
