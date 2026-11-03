# Spec Delta

## ADDED Requirements

### Requirement: Rhythm line
Below the `time` line of the header, the X-ray SHALL print a `rhythm` line with the session's turns as work-rhythm counts them: how many, how long the agent worked, the medians of its turns and of the user's answers, the turns it started itself, the interrupts and the questions.

#### Scenario: Rhythm
- **WHEN** a session had 34 turns with 2.1h of work, the agent's turns took a median 3.5m, the user answered in a median 4.0m, the agent started 12 turns itself, and the user stopped it twice
- **THEN** the line after `time` reads `rhythm    34 turns, 2.1h of work · the agent's turn median 3.5m · your answer median 4.0m · 12 it started itself · 2 interrupts`
- **AND** a part whose count is zero or whose median has no values is left out
- **AND** the day starts as work-rhythm says, from the session's own actions

#### Scenario: No turns
- **WHEN** a session has no turn, as a subagent's log alone or a session that a program ran
- **THEN** there is no `rhythm` line
