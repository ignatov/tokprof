# Spec Delta

## ADDED Requirements

### Requirement: Agents at work lane
Under the `You` lane, the lanes SHALL have an `Agents at work` lane: a stepped area in the work color, as high at each moment as work-rhythm's number of agents at work, its full height the peak. Its label says `Agents at work`, and under it `peak <n> at <HH:MM>`, or `none` on a day without work.

#### Scenario: A peak of seven
- **WHEN** seven sessions worked at once at 14:20, and never more
- **THEN** the area reaches its full height at 14:20, and the label reads `peak 7 at 14:20`

#### Scenario: Not a session
- **WHEN** the user clicks the `Agents at work` lane
- **THEN** the selection stays where it was

## MODIFIED Requirements

### Requirement: Day lanes
The lanes SHALL show the day as work-rhythm's Day strip with shapes, at the times of the log: a `You` lane with the sittings, the `Agents at work` lane, then a lane per session with actions that day, most actions first, labeled with its project and title. Its bars and marks take work-rhythm's colors, and the selected session's lane is highlighted.

#### Scenario: Bars and marks
- **WHEN** a session's agent worked, waited for the user's answer, asked a question, and the user typed and stopped it
- **THEN** the work and the wait are bars from their start to their end, at least 1 point wide, and the message, the question and the stop are marks at their moment

#### Scenario: A short piece of work
- **WHEN** the agent worked from 14:32:10 to 14:32:50
- **THEN** its lane has a green bar 40 seconds wide at 14:32

#### Scenario: A click on a lane
- **WHEN** the user clicks the lane of session `ecad11`
- **THEN** `ecad11` is selected in the list and the inspector shows it

### Requirement: What the lanes say
Under the lanes SHALL be the legend, and the `sittings`, `agents`, `answers`, `queue` and `cache` lines of the day as `tokprof day` prints them. A line under them names what is under the pointer: the time, the lane and what it shows there.

#### Scenario: The time under the pointer
- **WHEN** the pointer rests over a lane at 14:32
- **THEN** the line reads like `14:32 · ecad11 Инфографика прерываний · agent working`, with `agent working`, `waiting for your answer`, `your message`, `its question`, `you stopped it`, `you were there`, `cache warm` or nothing

#### Scenario: The queue line
- **WHEN** the shown day had 24 answers among several sessions waiting (see work-rhythm)
- **THEN** the `queue` line under `answers` reads as `tokprof day` prints it for that day

#### Scenario: The agents at work under the pointer
- **WHEN** the pointer rests over the `Agents at work` lane at 14:20, while three sessions worked
- **THEN** the line reads like `14:20 · 3 agents at work: ecad11 Инфографика…, 230043 Ордер…, 66f6be Новый…`, the sessions in the lanes' order
