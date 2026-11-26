# Spec Delta

## MODIFIED Requirements

### Requirement: Own cost in the session list
With `--cost`, the `cost` cell of a session whose requests have no usage (see cost-attribution) SHALL show its own cost (see session-logs, Agent-reported cost) followed by `*`, or `–` without one, and its `next request` cell stays blank. When a row has `*`, a line under the list reads `* the agent's own cost; its log gives no usage`.

#### Scenario: Copilot sessions
- **WHEN** the user runs `tokprof --agent copilot-cloud sessions --cost --limit 3`, and the sessions' records billed 150, 80 and no credits
- **THEN** their cost cells read `$1.50*`, `$0.80*` and `–`, and the line under the list explains the `*`
