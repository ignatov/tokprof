# Spec Delta

## MODIFIED Requirements

### Requirement: Layout
The X-ray SHALL print, in this order and separated by blank lines: the header; the `context`, `cache` and `next` lines with the advice; `spend`; the `work`, `prompts`, `tools` and `setup` blocks when they are shown; `in context`; `what cost the most`; the cache misses; and the `profile` pointer (see the scenario The profile pointer). The parts about the live context need a main conversation.

#### Scenario: The profile pointer
- **WHEN** the X-ray ends
- **THEN** its last line is `profile: tokprof flame --session <id> → <dir>/<id>.jfr (IntelliJ: Open snapshot)`, where `<id>` is the session's short id (see session-logs) and `<dir>` is `~/tokprof`, or `$TOKPROF_DIR` when set

#### Scenario: Session with a main conversation
- **WHEN** the main log has a request with a prompt
- **THEN** every part is printed in the order above

#### Scenario: No main conversation
- **WHEN** the main log has no request with a prompt, for example when only its subagents made requests or the target is a subagent transcript
- **THEN** the `context`, `cache` and `next` lines, the advice and `in context` are left out
- **AND** the header, `spend`, the `work`, `prompts`, `tools` and `setup` blocks when they are shown, `what cost the most`, the cache misses and the pointer are still printed

#### Scenario: Session without requests
- **WHEN** the log holds no request with usage
- **THEN** there is no `models` line, the `time` line says `0 requests`, `spend` shows `0 tokens (no price)` and the misses line says `no misses`

## ADDED Requirements

### Requirement: Setup block
When the session's setup cost (see setup-breakdown), subagents included, is at least $1 and at least 5% of its spend, or at least 5% of its tokens when it has no price, the X-ray SHALL print a `setup` block. It starts with `setup     what every request carries before the work, subagents included: <cost>, <share>% of the session`, followed by the setup table.

#### Scenario: A code review with many subagents
- **WHEN** a session's setup cost $73.90 of $526
- **THEN** the block starts with `setup     what every request carries before the work, subagents included: $73.90, 14% of the session`

#### Scenario: A small setup
- **WHEN** the setup cost $0.40, or 3% of the session
- **THEN** there is no `setup` block

### Requirement: Setup table
The setup block SHALL hold a table with one row per setup source, most expensive first: `source`, `cost`, `subagents` for the part of the cost in subagent chains, `main starts` with the source's tokens in what the main conversation starts with, and `each subagent` with the mean tokens of what the subagents start with (see setup-breakdown). Up to 8 sources are rows; the rest fold into `<n> more`. A `total` row closes the table.

#### Scenario: Totals of a code review
- **WHEN** the setup cost $73.90, $70.23 of it in subagents, and 96 subagents start with 31.2k tokens on average, instructions 9.7k and skills 9.6k, with a mean brief of 1.8k
- **THEN** the `total` row shows $73.90, $70.23 and 31.2k under `each subagent`, followed by `96 subagents on average, and a 1.8k brief`

#### Scenario: No subagents
- **WHEN** the session has no subagent with a known first prompt and no setup cost in subagent chains
- **THEN** the table has no `subagents` and no `each subagent` column

#### Scenario: Unknown start
- **WHEN** the log does not show the main conversation's first prompt
- **THEN** `main starts` shows `–` in every row

#### Scenario: After a compaction
- **WHEN** the main conversation has a compaction whose next prompt the log shows (see setup-breakdown, What a compaction leaves)
- **THEN** a line after the table reads `after the compaction at <MM-DD HH:MM>, the next prompt held <size>:`, followed on the next line by its 4 largest parts by content kind or reminder source, each with its tokens, then `<n> more <tokens>`

#### Scenario: The hint
- **WHEN** the costliest setup source has a hint from the agent's log source (see setup-breakdown, Setup hints)
- **THEN** the block ends with `▸ <Source> cost the most of the setup: <hint>.`, wrapped like an advice item, as in "▸ Skills cost the most of the setup: lower skillListingBudgetFraction (default 0.01 of the context window), or turn unused skills off with skillOverrides in settings.json."
- **AND** with no hint for that source there is no such line
