# Spec Delta

## ADDED Requirements

### Requirement: Rhythm section
After the agent sections, `tokprof pauses` SHALL print one rhythm section for all agents together, over the selected time and sessions: the title `Your rhythm: when you sat at the agents and what they did meanwhile`, the `night` line and the `left out` line (see work-rhythm), the five rhythm lines of work-rhythm, and a table by hour of the day.

#### Scenario: The section
- **WHEN** `tokprof --since 30d pauses` runs
- **THEN** the rhythm section follows the last agent section, and its five lines count the actions and turns of the last 30 days

#### Scenario: Hours
- **WHEN** the day starts at 05:00
- **THEN** the hour table has the columns `hour`, `messages`, `turn ends` and `median answer`, and a row for each hour from 05 to 04 with a message or an end of turn in it, in that order
- **AND** an answer counts in the hour of the message, and an hour without answers shows `–`

#### Scenario: No messages
- **WHEN** the selected sessions hold no action of the user
- **THEN** the section is the title and `No messages in this time.`
