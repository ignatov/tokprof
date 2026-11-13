# Spec Delta

## MODIFIED Requirements

### Requirement: Day strip
The day view SHALL draw the day as a strip: a row of hours, a `you` lane that marks the sittings, and a lane per session with the user's actions that day, most actions first. The strip covers 24 hours from the whole hour at or before the day's start, in the shortest column of 10, 15, 20, 30 and 60 minutes that fits its width. Its labels, width and rows depend on where it is shown. A legend follows.

#### Scenario: Columns
- **WHEN** the day of 2026-11-02 starts at 04:21 and `tokprof day` draws it
- **THEN** the strip runs from 04:00 to 04:00 the next morning in 20-minute columns, 72 of them

#### Scenario: A narrow pane
- **WHEN** the browser leaves 50 columns for the strip
- **THEN** a column covers 30 minutes, and the strip has 48

#### Scenario: A wide screen
- **WHEN** the browser leaves 179 columns for the strip
- **THEN** a column covers 10 minutes, and the strip has 144

#### Scenario: Marks
- **WHEN** a column of a lane holds an interrupt, a message, a question, work, or a wait for the user's answer
- **THEN** it shows the first that applies of `!`, `*`, `?`, `█` and `░`, and else a space; the `you` lane shows `▓` where the user sat
- **AND** a wait for the answer lasts from an end of turn to the user's next message in that session in the same day

#### Scenario: Colors
- **WHEN** the strip goes to a terminal that takes color
- **THEN** `█` is green, `░` yellow, `*` bold cyan, `?` bold magenta, `!` bold red and `▓` blue
- **AND** without color the marks are the same and no escape code is written

#### Scenario: In a terminal
- **WHEN** `tokprof day` draws the strip
- **THEN** labels take 20 characters, the strip 72 columns, and at most 8 lanes show

#### Scenario: Lane labels
- **WHEN** a session's id is `ecad11b9-…` and its title is `Chart of the interruptions`
- **THEN** its lane starts with the first 6 characters of the id and the title, cut to the label's width: `ecad11 Chart of the…` in 20 characters

#### Scenario: Many sessions
- **WHEN** 11 sessions had actions in the day and the strip has rows for 9 lines
- **THEN** it has 8 lanes and then the line `3 more sessions`
- **AND** with rows for 11 lanes or more it has all 11 and no such line

#### Scenario: Room between lanes
- **WHEN** the browser shows the strip and it has rows for twice its lanes
- **THEN** each lane is followed by a row of dim `·` marks in the first column of each hour
- **AND** `tokprof day` never spaces its lanes

#### Scenario: Hours row
- **WHEN** columns are 20 minutes or shorter
- **THEN** each hour's label sits over its first column; with 30- or 60-minute columns, a label is left out when it would start less than 3 columns after the previous one

#### Scenario: Legend
- **WHEN** the strip is drawn
- **THEN** it ends with `█ agent working   ░ waiting for your answer   * your message   ? its question   ! you stopped it   ▓ you were there`, each mark in its color
