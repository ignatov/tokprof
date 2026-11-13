# Spec Delta

## MODIFIED Requirements

### Requirement: Start of the day
A day SHALL run from the middle of the night before it to the middle of the night after it, or until now. The night before a day is the longest pause between the user's consecutive actions, across the sessions shown, whose middle falls between 18:00 the day before and 12:00 the day, in local time. Before that noon, only a pause of 4 hours or more counts. A day with no such pause starts at midnight.

#### Scenario: The author's night
- **WHEN** the user acted last at 00:22 on 2026-11-02 and next at 08:20, and no longer pause has its middle between 18:00 on 2026-11-01 and 12:00 on 2026-11-02
- **THEN** the day of 2026-11-02 starts at 04:21

#### Scenario: A pause in the afternoon
- **WHEN** the night before a day ran from 01:00 to 08:00, and the user paused from 10:00 to 18:00 the same day
- **THEN** the day starts at 04:30: the afternoon pause has its middle at 14:00, outside the window

#### Scenario: A night that is not empty
- **WHEN** the user worked through a night with breaks of at most 40 minutes, the longest from 03:10 to 03:50
- **THEN** the day starts at 03:30, in the middle of that break

#### Scenario: Too few days
- **WHEN** the logs hold no action from 18:00 the day before to 12:00 the day, as around a day off
- **THEN** the day starts at midnight

#### Scenario: A day with one action
- **WHEN** the user acted on 2026-10-31 at 22:00 and at no other time from 18:00 on 2026-10-30 to 12:00 on 2026-11-01
- **THEN** the day of 2026-10-31 starts in the middle of the pause that ends at 22:00

#### Scenario: The night line
- **WHEN** the day of 2026-11-02 has the night 00:22–08:20 before it and 22:26–10:03 after it, or the current day has only the night before it
- **THEN** the line reads `night     before the day 00:22–08:20 (8.0h), after it 22:26–10:03 (11.6h); a day runs from the middle of one night to the middle of the next`, or `night     before the day 00:22–08:20 (8.0h); the day runs until now`
- **AND** a day with no pause before it reads `night     no pause before the day; it starts at midnight`, and a day whose next day starts at midnight ends its line with `; the next day starts at midnight`

### Requirement: Day view
`tokprof day [DATE]` SHALL show one day: the day of the user's latest action, or the day of DATE (YYYY-MM-DD). It reads the main logs whose last line with a time is at or after 18:00 the day before, and that were created before 12:00 the day after. It applies the project, session and client filters, and prints the title `tokprof · day`, a `day` line, the `night` line, the `left out` line when there is one, the strip, the rhythm lines and the `cache` line.

#### Scenario: The logs of a day
- **WHEN** a main log was created on 2026-10-20, its last line with a time is at 2026-11-02 09:00, and the IDE appended lines without a time to it on 2026-11-03
- **THEN** it is read for the day of 2026-11-02 and not for 2026-11-03
- **AND** a log created on 2026-11-03 at 13:00 is not read for 2026-11-02

#### Scenario: The day line
- **WHEN** the day of 2026-11-02 runs from 04:21 to 04:15 the next morning
- **THEN** the line reads `day       Mon 2026-11-02 04:21 → Tue 2026-11-03 04:15`
- **AND** the current day's line ends with `→ now`, as in `day       Tue 2026-11-03 04:24 → now`

#### Scenario: Today before the start of the day
- **WHEN** `tokprof day` runs at 02:10 on 2026-11-03 and the user's latest action was at 01:50
- **THEN** it shows the day of 2026-11-02, since no night has come since that day began

#### Scenario: A day with no message
- **WHEN** the user did not act in the day
- **THEN** after the `night` line the view reads `No messages on 2026-11-02.`

#### Scenario: Time filters
- **WHEN** `tokprof --since 7d day` runs
- **THEN** it fails with `day picks its time by DATE; drop --since and --until`

### Requirement: Day strip
The day view SHALL draw the day as a strip: a row of hours, a `you` lane that marks the sittings with `#`, and a lane per session with the user's actions that day, most actions first. A column covers the shortest of 10, 15, 20, 30 and 60 minutes that fits the hours from the day's first action to its last into 72 columns, or into the width the browser leaves for it. A legend follows.

#### Scenario: Columns
- **WHEN** the day's first action is at 08:20 and its last at 22:26
- **THEN** the strip runs from 08:00 to 23:00 in 15-minute columns, 60 of them

#### Scenario: A narrow pane
- **WHEN** the browser leaves 50 columns for the strip of a day from 08:00 to 23:00
- **THEN** a column covers 20 minutes, and the strip has 45

#### Scenario: Marks
- **WHEN** a column of a lane holds an interrupt, a message, a question, work, or a wait for the user's answer
- **THEN** it shows the first that applies of `!`, `*`, `?`, `=` and `-`, and else a space
- **AND** a wait for the answer lasts from an end of turn to the user's next message in that session in the same day

#### Scenario: Lane labels
- **WHEN** a session's id is `ecad11b9-…` and its title is `Chart of the interruptions`
- **THEN** its lane starts with the first 6 characters of the id and the title, cut to 20 characters: `ecad11 Chart of the…`

#### Scenario: Many sessions
- **WHEN** 11 sessions had actions in the day
- **THEN** the strip has 8 lanes and then the line `3 more sessions`

#### Scenario: Hours row
- **WHEN** columns are 15 minutes
- **THEN** each hour's label sits over its first column; with 30- or 60-minute columns, a label is left out when it would start less than 3 columns after the previous one

#### Scenario: Legend
- **WHEN** the strip is drawn
- **THEN** it ends with `= agent working   - waiting for your answer   * your message   ? its question   ! you stopped it   # you were there`
