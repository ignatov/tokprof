# Spec Delta

## ADDED Requirements

### Requirement: Tools block
When the session, its subagents included, made tool calls, the X-ray SHALL print a `tools` block: `tools     calls, sizes, rent and trips by tool (see tokprof tools)`, the 8 rows of the tool breakdown (see tool-breakdown) with the most rent, each with its calls, sizes, rent, trips, single share and hint, then `<n> more` and the total line.

#### Scenario: A session that read code one piece at a time
- **WHEN** the session's top row is `Bash sed -n` with 56 calls, 1.3k-token results, $5.21 of rent and $3.75 of trips, 86% single
- **THEN** the block's first row shows those numbers and the hint `batch independent reads`

#### Scenario: No tool calls
- **WHEN** the session made no tool call
- **THEN** there is no `tools` block

## MODIFIED Requirements

### Requirement: Layout
The X-ray SHALL print, in this order and separated by blank lines: the header; the `context`, `cache` and `next` lines with the advice; `spend`; the `work`, `prompts` and `tools` blocks when they are shown; `in context`; `what cost the most`; the cache misses; and the `profile` pointer (see the scenario The profile pointer). The parts about the live context need a main conversation.

#### Scenario: The profile pointer
- **WHEN** the X-ray ends
- **THEN** its last line is `profile: tokprof flame --session <id> → <dir>/<id>.jfr (IntelliJ: Open snapshot)`, where `<id>` is the session's short id (see session-logs) and `<dir>` is `~/tokprof`, or `$TOKPROF_DIR` when set

#### Scenario: Session with a main conversation
- **WHEN** the main log has a request with a prompt
- **THEN** every part is printed in the order above

#### Scenario: No main conversation
- **WHEN** the main log has no request with a prompt, for example when only its subagents made requests or the target is a subagent transcript
- **THEN** the `context`, `cache` and `next` lines, the advice and `in context` are left out
- **AND** the header, `spend`, the `work`, `prompts` and `tools` blocks when they are shown, `what cost the most`, the cache misses and the pointer are still printed

#### Scenario: Session without requests
- **WHEN** the log holds no request with usage
- **THEN** there is no `models` line, the `time` line says `0 requests`, `spend` shows `0 tokens (no price)` and the misses line says `no misses`
