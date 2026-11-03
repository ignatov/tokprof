# Spec Delta

## MODIFIED Requirements

### Requirement: Busy time part
The last part of each agent section SHALL be `Where the agent's busy time went: what it waited on in its turns, and while its own background work ran after them` (see agent-time): a table of the busy time inside the window, of the selected sessions and their subagents, those that work-rhythm leaves out included. It has one row per kind of busy time, most time first: `kind`, `time`, `share` and `after turns`, the part after the end of a turn. A line on the slow requests follows.

#### Scenario: A month of Claude Code
- **WHEN** the agent's subagents kept it busy 43 hours after ends of turn, of 60 hours for subagents in all
- **THEN** the `subagents` row shows `60.0h` under `time` and `43.0h` under `after turns`
- **AND** a kind with no time after a turn shows `–` under `after turns`

#### Scenario: The slow line
- **WHEN** 2,652 of 81,324 requests took a minute or more, and the longest took 23.3 minutes
- **THEN** the line after the table reads `2,652 of 81,324 requests took a minute or more (3.3%); the longest took 23m.`

#### Scenario: Not busy
- **WHEN** the agent's selected sessions hold no busy time
- **THEN** the part reads `No busy time.` after its title

#### Scenario: A session that began before the window
- **WHEN** `--since` falls inside a session that was busy 10 hours before it and 2 hours after it, with 40 slow requests before it and 3 after
- **THEN** the table counts the 2 hours, and the slow line counts the 3 requests among the requests sent inside the window
- **AND** without `--since` and `--until`, the whole session counts
