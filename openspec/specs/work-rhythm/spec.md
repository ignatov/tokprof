# work-rhythm Specification

## Purpose
Shows how the user and the agents hand work to each other over a day: when the user sat at the agents, how long the agents worked and waited, and what they did after the user left, so that the user can plan when to come back and how much work to leave.

## Requirements

### Requirement: Actions and hand-offs
The rhythm SHALL read the main conversations of the sessions it shows, as session-logs gives them. The user's actions are typed messages, messages typed while the agent worked, answers to the agent's questions and interrupts. The agent's hand-offs are ends of turn, questions and wakes. Its requests mark when it was active.

#### Scenario: Actions of the user
- **WHEN** a session holds 5 typed messages, 1 message typed while the agent worked, 1 answer to a question and 1 interrupt
- **THEN** the user has 8 actions in it, and 6 messages

#### Scenario: Subagents
- **WHEN** a session started a subagent
- **THEN** nothing in the subagent's log is an action or a hand-off

### Requirement: Turns
A turn SHALL start when the agent stands still and then gets work: at a typed message, at a wake, or at a request with neither before it. It ends at the next end of turn or interrupt. A turn whose end is not logged ends at its last request before the next turn starts. A turn that starts at a wake is one the agent started itself.

#### Scenario: A turn the user started
- **WHEN** the user types at 10:00, the agent makes requests until 10:03 and ends its turn at 10:04
- **THEN** that is one turn from 10:00 to 10:04, and the agent's turn took 4m

#### Scenario: A turn the agent started itself
- **WHEN** the agent stands still after an end of turn, and a background task's result wakes it
- **THEN** a turn starts at the wake, and it counts as a turn the agent started itself

#### Scenario: A wake during a turn
- **WHEN** a background task's result comes while the agent works
- **THEN** no new turn starts

#### Scenario: An end that is not logged
- **WHEN** the agent's last request is at 10:20, no end of turn follows, and the user types again at 10:30
- **THEN** the earlier turn ends at 10:20 and a new one starts at 10:30

### Requirement: Work
The agent SHALL count as working during its turns, except while it waits for the answer to its own question and except in silences. A silence is a stretch of 15 minutes or more inside a turn with no request, end, question or answer.

#### Scenario: A silence
- **WHEN** a turn starts at 10:00, has requests at 10:00 and 10:02 and then none until 10:40, and ends at 10:45
- **THEN** the agent worked from 10:00 to 10:02 and from 10:40 to 10:45, 7 minutes

#### Scenario: A question
- **WHEN** the agent asks a question at 10:05 and the user answers at 10:12
- **THEN** the agent does not count as working from 10:05 to 10:12

### Requirement: Answers
The user's answer SHALL be the time from an end of turn to the user's next message in the same session, when both fall in the same day. The agent's turn is the time from a typed message to the end of the turn it started. A wake between an end of turn and the message does not move the start of the answer: it is counted from the latest end before the message.

#### Scenario: An answer
- **WHEN** a turn ends at 10:04 and the user types in that session at 10:07
- **THEN** the user answered in 3.0m

#### Scenario: Across the start of the day
- **WHEN** a turn ends at 23:30, the day starts at 05:00, and the user types again at 10:00
- **THEN** that is no answer

### Requirement: Sittings and breaks
A sitting SHALL be a run of the user's actions, across all the sessions shown, each less than 15 minutes after the one before. A sitting lasts from its first action to its last, and at least one minute. The time at the keyboard is the sum of the sittings. A break is the time between two sittings of the same day.

#### Scenario: Two sittings
- **WHEN** the user acts at 10:00, 10:05 and 10:14 in one session and at 10:40 in another
- **THEN** there are two sittings, 10:00–10:14 and a one-minute one at 10:40, and a break of 26m

#### Scenario: The edge of the gap
- **WHEN** two actions are exactly 15 minutes apart
- **THEN** they are in different sittings

### Requirement: After the user left
For each break, the rhythm SHALL measure how long after the sitting ended the agents of all the sessions shown first stood still, and what share of the break at least one of them worked. The `left` line gives the median of each over the breaks.

#### Scenario: Agents that ran out of work
- **WHEN** a sitting ends at 10:00, one agent works until 10:06 and another from 10:20 to 10:24, and the next sitting starts at 10:40
- **THEN** the agents stood still 6.0m after the user left, and some agent worked 10m of the 40m break, 25%

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

### Requirement: Sessions left out
The rhythm SHALL leave out the sessions that a program ran (see session-logs) and the sessions whose working directory is in the system's temporary folder. When it leaves any out, it says how many.

#### Scenario: Commit-message sessions
- **WHEN** an IDE ran 92 sessions with working directories in the temporary folder
- **THEN** none of their messages counts, and the line `left out  92 sessions that nobody typed in: a program ran them, or they ran in a temporary folder` follows the `night` line

#### Scenario: Print mode
- **WHEN** a program ran Claude Code in print mode
- **THEN** that session is left out

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
The day view SHALL draw the day as a strip: a row of hours, a `you` lane that marks the sittings, and a lane per session with the user's actions that day, most actions first. The strip covers 24 hours from the whole hour at or before the day's start. A column covers the shortest of 10, 15, 20, 30 and 60 minutes that fits 24 hours into 72 columns, or into the width the browser leaves for it. A legend follows.

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

#### Scenario: Lane labels
- **WHEN** a session's id is `ecad11b9-…` and its title is `Инфографика прерываний`
- **THEN** its lane starts with the first 6 characters of the id and the title, cut to 20 characters: `ecad11 Инфографика…`

#### Scenario: Many sessions
- **WHEN** 11 sessions had actions in the day
- **THEN** the strip has 8 lanes and then the line `3 more sessions`

#### Scenario: Hours row
- **WHEN** columns are 20 minutes or shorter
- **THEN** each hour's label sits over its first column; with 30- or 60-minute columns, a label is left out when it would start less than 3 columns after the previous one

#### Scenario: Legend
- **WHEN** the strip is drawn
- **THEN** it ends with `█ agent working   ░ waiting for your answer   * your message   ? its question   ! you stopped it   ▓ you were there`, each mark in its color

### Requirement: Rhythm lines
The day view and the rhythm section of the pause report SHALL print five lines on the time they show: `sittings`, `left`, `present`, `answers` and `stops`. Durations read as the X-ray's span (see session-xray), except that from one to ten minutes they keep one decimal, as in `3.5m`. A median takes the upper of two middle values.

#### Scenario: Sittings
- **WHEN** there were 16 sittings with 3.7h at the keyboard and 58 messages to 11 sessions, a median sitting of 6m and a median break of 28m
- **THEN** the line reads `sittings  16, 3.7h at the keyboard, 58 messages to 11 sessions; median sitting 6.0m, median break 28m`
- **AND** with a single sitting it ends `median sitting 6.0m, no break`

#### Scenario: Left
- **WHEN** the agents stood still a median 8m after the user left and some agent worked through a median 28% of a break
- **THEN** the line reads `left      the agents stood still a median 8.0m after you left, and worked through 28% of a median break`
- **AND** without breaks it reads `left      no break between sittings`

#### Scenario: Present
- **WHEN** 999 turns ended in 75h at the keyboard, and 37% of all the turns that ended were turns the agent started itself
- **THEN** the line reads `present   an agent ended a turn every 4.5m while you were there; it started 37% of its turns itself`
- **AND** with no turn ending in a sitting the first part reads `no turn ended while you were there`

#### Scenario: Answers
- **WHEN** the agent's turns took a median 3.5m and the user answered in a median 3.7m
- **THEN** the line reads `answers   the agent's turn took a median 3.5m; you answered in a median 3.7m`
- **AND** without turns or answers that part reads `no turns you started` or `no answers`

#### Scenario: Stops
- **WHEN** there were 39 interrupts, 46 messages typed while the agent worked, and 74 questions answered in a median 2.3m
- **THEN** the line reads `stops     39 interrupts, 46 messages typed while the agent worked, 74 questions from the agent answered in a median 2.3m`
- **AND** counts of one are singular, and with no questions the end reads `no questions from the agent`

### Requirement: Cache line of the day
The day view SHALL end with the day's cold returns, as the pause report counts them (see pause-report), and what their first requests cost: `cache     4 cold returns, $14.20 in their first requests, $13.50 of it avoidable`. Without cold returns it reads `cache     no cold returns`.

#### Scenario: Without prices
- **WHEN** no request of the day had a price
- **THEN** the line gives the count only, as in `cache     4 cold returns`

#### Scenario: One cold return
- **WHEN** the day had one cold return whose first request cost $3.29, $3.17 of it avoidable
- **THEN** the line reads `cache     1 cold return, $3.29 in its first request, $3.17 of it avoidable`
