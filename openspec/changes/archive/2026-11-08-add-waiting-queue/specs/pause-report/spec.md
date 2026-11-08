# Spec Delta

## MODIFIED Requirements

### Requirement: Rhythm section
After the agent sections, `tokprof pauses` SHALL print one rhythm section for all agents together, over the selected time and sessions: the title `Your rhythm: when you sat at the agents and what they did meanwhile`, a `nights` line with the median start of a day, the `left out` line (see work-rhythm), the six rhythm lines of work-rhythm, the advice of work-rhythm over the selected time, and a table by hour of the day.

#### Scenario: The section
- **WHEN** `tokprof --since 30d pauses` runs
- **THEN** the rhythm section follows the last agent section, and its six lines count the actions and turns of the last 30 days

#### Scenario: The advice
- **WHEN** `tokprof --since 30d pauses` runs
- **THEN** the `advice` block follows the rhythm lines and the hour table follows it, and its cold returns are those of all agents in the report

#### Scenario: Nights
- **WHEN** the days of the selected time started, each in the middle of the night before it (see work-rhythm), at a median of 05:14
- **THEN** the line reads `nights    a day runs from the middle of one night to the middle of the next; days started at a median 05:14`

#### Scenario: Hours
- **WHEN** days started at a median 05:14
- **THEN** the hour table has the columns `hour`, `messages`, `turn ends` and `median answer`, and a row for each hour from 05 to 04 with a message or an end of turn in it, in that order
- **AND** an answer counts in the hour of the message, and an hour without answers shows `–`

#### Scenario: No messages
- **WHEN** the selected sessions hold no action of the user
- **THEN** the section is the title and `No messages in this time.`
