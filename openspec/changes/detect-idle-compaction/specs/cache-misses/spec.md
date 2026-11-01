# Spec Delta

## MODIFIED Requirements

### Requirement: Model switch and compaction
The cause SHALL be `model switch`, with detail `<previous model> -> <model>`, when the request's model differs from the previous request's; and `compaction` when the agent logged a compaction after the previous request that got a response: a Claude Code `compact_boundary`, or a Codex `compacted` item or compaction event. A compaction cause has the marker's kind (see session-logs) as its detail: `while idle` for `idle`, else the kind; a marker with no kind gives no detail.

#### Scenario: Switching models
- **WHEN** a request to `claude-opus-5-5` follows one to `claude-sonnet-5-5` and re-processes the prefix
- **THEN** its cause reads `model switch: claude-sonnet-5-5 -> claude-opus-5-5`

#### Scenario: A compaction
- **WHEN** Claude Code compacts a 400,000-token conversation and the next request sends 50,000 tokens, reading only the 20,000 tokens of tool schemas and system prompt from the cache
- **THEN** it re-processed 30,000 tokens and its cause is `compaction`

#### Scenario: A cancelled request after the compaction boundary
- **WHEN** a `compact_boundary` is followed by a request that got no response, and then by an answered request that misses
- **THEN** the answered request's cause is `compaction`

#### Scenario: A compaction while idle
- **WHEN** the marker before a miss has the kind `idle`
- **THEN** the list of misses shows its cause as `compaction: while idle`
- **AND** misses by cause still count it under `compaction`

#### Scenario: An auto-compaction
- **WHEN** the marker before a miss has the kind `auto`
- **THEN** the list of misses shows `compaction: auto`

#### Scenario: A Codex compaction
- **WHEN** the marker before a miss is a Codex `compacted` line
- **THEN** the cause is `compaction`, with no detail
