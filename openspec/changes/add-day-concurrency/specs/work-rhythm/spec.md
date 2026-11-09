# Spec Delta

## ADDED Requirements

### Requirement: Agents at work
At each moment of a day, the number of agents at work SHALL be the number of kept main sessions whose agent worked then, as Work defines it. A subagent is part of its session's work and is never counted as another agent. The peak is the highest number, at the first moment it is reached. Sessions count whether or not the user acted in them that day.

#### Scenario: Two sessions overlap
- **WHEN** session A worked from 10:00 to 10:30 and session B from 10:20 to 10:50
- **THEN** one agent was at work from 10:00, two from 10:20 to 10:30, one until 10:50, and the peak is 2 at 10:20

#### Scenario: A subagent
- **WHEN** a session's agent worked from 10:00 to 10:30 and its subagent ran from 10:05 to 10:25
- **THEN** one agent was at work from 10:00 to 10:30

### Requirement: Agents line
The day view SHALL print `agents` after `sittings`: the agent-hours, the time at the keyboard, the time some agent was at work, the share of the sittings with 2 or more at work, and the peak. Agent-hours add up, over sessions, the minutes each one's work touches. "Some agent at work" counts each minute once, as the calendar's `agent` metric does. Both count a minute whose start is in the day. Hours have one decimal; durations read as in Rhythm lines.

#### Scenario: A busy day
- **WHEN** the sessions worked 1,278 minutes in all and some agent worked 750 of them, the user sat 7.3h, two or more worked for 38% of it, and seven at once at 14:20
- **THEN** the line reads `agents    21.3 agent-hours over 7.3h at the keyboard, some agent at work 12.5h; 2 or more worked 38% of the time you were there; peak 7 at 14:20`

#### Scenario: No work
- **WHEN** no agent worked in the day
- **THEN** the line reads `agents    no agent worked`

#### Scenario: The same as the calendar
- **WHEN** the calendar's `agent` metric gives 12.5h for a day
- **THEN** the line of that day says `some agent at work 12.5h`

## MODIFIED Requirements

### Requirement: Day view
`tokprof day [DATE]` SHALL show one day: the day of the user's latest action, or the day of DATE (YYYY-MM-DD), with the project, session and client filters. It prints the title `tokprof · day`, a `day` line, the `night` line, the `left out` line when there is one, the strip, the rhythm lines with the `agents` line after `sittings`, the `cache` line and the advice.

#### Scenario: The logs read
- **WHEN** `tokprof day 2026-11-02` runs
- **THEN** it reads the main logs whose last line with a time is at or after 18:00 on 2026-11-01, and that were created before 12:00 on 2026-11-03

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

#### Scenario: The advice
- **WHEN** a rule of Rhythm advice holds for the day
- **THEN** its item follows the `cache` line in the `advice` block

#### Scenario: Time filters
- **WHEN** `tokprof --since 7d day` runs
- **THEN** it fails with `day picks its time by DATE; drop --since and --until`
