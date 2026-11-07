## MODIFIED Requirements

### Requirement: One cause per miss
tokprof SHALL give each miss exactly one cause: the first that fits, checked from the most to the least certain in this order: `model switch`, `compaction`, `compaction call`, `idle > TTL`, `params changed`, `agent updated`, `tools changed` or `system prompt changed`, then `tool results cleared`, `images removed`, `thinking dropped` or `history edited` for an edited history, then `lookback > 20 blocks` and `unknown (server side?)`.

#### Scenario: A model switch after a break
- **WHEN** a request after two idle hours also switched the model
- **THEN** its cause is `model switch`

#### Scenario: A compaction after a break
- **WHEN** the first request after an idle hour also follows a compaction
- **THEN** its cause is `compaction`

#### Scenario: A compaction call after a break
- **WHEN** a Codex compaction call comes two hours after the previous request
- **THEN** its cause is `compaction call`, since the call would have missed on a warm cache too

#### Scenario: A cause with a detail
- **WHEN** a cause has a detail, such as the idle time
- **THEN** the list of misses shows it as `<cause>: <detail>`, such as `idle > TTL: 3.3h idle, 1h TTL`

## ADDED Requirements

### Requirement: Compaction call
The cause SHALL be `compaction call`, with no detail, when the request is a compaction call (see session-logs) and no earlier cause fits. It is not an expected rebuild. The call sends the history that the previous request cached, so a call that kept the conversation's prefix would have read it from the cache. Its misses stay in the miss tables and in the avoidable dollars.

#### Scenario: A Codex compaction call
- **WHEN** the previous request cached a 242,713-token prompt, and a minute later the compaction call sends 232,687 tokens and reads none of them from the cache
- **THEN** it re-processed 232,687 tokens and its cause is `compaction call`
- **AND** `tokprof summary` and `tokprof misses` count it in their miss tables

#### Scenario: The request after the compaction
- **WHEN** the request after that compaction sends the 30,311-token compacted history and reads 17,200 tokens of it from the cache
- **THEN** its cause is `compaction`, an expected rebuild

### Requirement: Lookback and unknown misses
When no earlier cause fits, the cause SHALL be `lookback > 20 blocks` for a Claude Code request that appended more than 20 content blocks since the previous request, with detail `<n> blocks appended`, as the API looks back at most 20 blocks for an earlier cache entry; else `unknown (server side?)`.

#### Scenario: Many blocks at once
- **WHEN** a Claude Code request appends 34 blocks, such as the results of many parallel tool calls
- **THEN** its cause reads `lookback > 20 blocks: 34 blocks appended`

#### Scenario: Exactly 20 blocks
- **WHEN** a request appends 20 blocks
- **THEN** lookback is not its cause

#### Scenario: Codex never gets lookback
- **WHEN** a Codex request appends more than 20 blocks and misses
- **THEN** lookback is not its cause

#### Scenario: A Codex miss after an answer
- **WHEN** a Codex response ended with an assistant message, and the next request misses with no earlier cause
- **THEN** its cause is `unknown (server side?)`

## REMOVED Requirements

### Requirement: Lookback, assistant messages and unknown misses
**Reason**: `after assistant message` was taken for a Codex signature, but 171 of the 172 misses it named in 30 days of logs were Codex compaction calls. Given to the request after an assistant message, as this requirement said, it named none.
**Migration**: Those misses are `compaction call`, or `unknown (server side?)` when no cause fits. The lookback rule moves unchanged to "Lookback and unknown misses". Older share files keep the old name, and the pool shows it as it is.
