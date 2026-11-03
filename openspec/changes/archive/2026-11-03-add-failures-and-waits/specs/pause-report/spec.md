## ADDED Requirements

### Requirement: Busy time part
The last part of each agent section SHALL be `Where the agent's busy time went: what it waited on in its turns, and while its own background work ran after them` (see agent-time), with a table over the selected sessions of the agent and their subagents, the sessions that work-rhythm leaves out included. The table has one row per kind of busy time, most time first: `kind`, `time`, `share` and `after turns`, the part that came after the end of a turn. A line on the slow requests follows.

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

## MODIFIED Requirements

### Requirement: Pause report
`tokprof pauses` SHALL report on the requests that the global filters select (see session-logs), one section per agent with requests, Claude Code first. Each section has these parts, in order: pauses before typed messages; the share after each cache lifetime; cold returns by pause; who started them; what followed them; sessions by largest context; and where the agent's busy time went.

#### Scenario: Both agents
- **WHEN** the selected requests come from Claude Code and Codex
- **THEN** the report has a `claude` section followed by a `codex` section, each with all seven parts

#### Scenario: No typed message with a pause
- **WHEN** an agent's main conversations hold no typed message after an earlier request
- **THEN** its pause table and lifetime line are replaced by `No typed messages after an earlier request.`, and the other parts are printed as usual
