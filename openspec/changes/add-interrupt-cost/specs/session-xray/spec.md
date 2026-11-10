## ADDED Requirements

### Requirement: Ends line
Below the `rhythm` line, the X-ray SHALL print an `ends` line with the session's turns by end kind, as turn-outcomes counts them, the most turns first and ties in the order `answered`, `changed`, `committed`, `waiting`, `asked`, `failed`, `stopped`, with the dollars of the stopped turns. A kind with no turns is left out.

#### Scenario: Ends
- **WHEN** a session had 34 turns: 14 `answered`, 12 `changed`, 4 `committed`, 2 `asked` and 2 `stopped` that cost $4.10
- **THEN** the line after `rhythm` reads `ends      14 answered · 12 changed · 4 committed · 2 asked · 2 stopped ($4.10)`

#### Scenario: No rhythm line
- **WHEN** the X-ray prints no `rhythm` line
- **THEN** it prints no `ends` line either
