# Spec Delta

## MODIFIED Requirements

### Requirement: Tools block
When the session, its subagents included, made tool calls, the X-ray SHALL print a `tools` block: `tools     calls, sizes, rent and trips by tool (see tokprof tools)`, the 8 rows of the tool breakdown (see tool-breakdown) with the most rent, each with its calls, time, median time, sizes, rent, trips, single share and hint, then `<n> more`, the total line and the groups line.

#### Scenario: A session that read code one piece at a time
- **WHEN** the session's top row is `Bash sed -n` with 56 calls that took 5.6 s in all, 0.1 s at the median, 1.3k-token results, $5.21 of rent and $3.75 of trips, 86% single
- **THEN** the block's first row shows those numbers, `5.6s` under `time`, `0.1s` under `med time`, and the hint `batch independent reads`

#### Scenario: No tool calls
- **WHEN** the session made no tool call
- **THEN** there is no `tools` block

## ADDED Requirements

### Requirement: Groups line
The `tools` block's last line SHALL be `groups    ` followed by the first 5 groups of the session's groups table (see tool-breakdown), its subagents included, each as `<group> <calls> (<time>, med <med time>)`, joined by ` · `, then `· <n> more` when there are more. A group none of whose calls counts for times leaves out the parentheses. Groups that were never called are not on the line.

#### Scenario: A session with six groups
- **WHEN** a session made 340 `shell` calls that took 42 minutes with a median of 0.6 s, 120 `files` calls of 24 s and 0.1 s, 30 `other` calls, 12 `mcp idea` calls of 18 s and 1.2 s, 5 `web` calls of 75 s and 12 s and 3 `subagents` calls of 13 minutes and 4.2 minutes
- **THEN** the line reads `groups    shell 340 (42m, med 0.6s) · files 120 (24s, med 0.1s) · mcp idea 12 (18s, med 1.2s) · web 5 (1.3m, med 12s) · subagents 3 (13m, med 4.2m) · 1 more`, since `other` comes last

#### Scenario: Only background calls
- **WHEN** all 4 `subagents` calls of a session ran in the background
- **THEN** the group shows as `subagents 4`, without a time

#### Scenario: A server never called
- **WHEN** a Claude Code session had the MCP server `linear` and never called it
- **THEN** `mcp linear` is not on the line
