# Spec Delta

## ADDED Requirements

### Requirement: Prompt after a trailing compaction
When the main conversation's latest compaction marker (see session-logs) comes after its latest request with a prompt, the X-ray SHALL take as the latest prompt an estimate of what the next request sends. The estimate is the prompt of the conversation's first request after its previous compaction, or else the tool schemas and system prompt plus 20,000 tokens. The `context` line shows the estimate with `~`, and the forecast and every advice rule use it.

#### Scenario: Compacted while idle, with an earlier compaction
- **WHEN** a conversation's latest request sent 440.1k tokens at 18:18, a compaction of kind `idle` at 19:13 follows it, and the first request after its previous compaction sent 55.8k tokens
- **THEN** the latest prompt is ~55.8k and the `context` line shows `~55.8k of 1.0M` with its share of the window

#### Scenario: The conversation's first compaction
- **WHEN** the compaction after the latest request is the conversation's first, and its tool schemas and system prompt hold 14.4k tokens
- **THEN** the latest prompt is ~34.4k

#### Scenario: Advice after the compaction
- **WHEN** the estimate is under 100,000 tokens
- **THEN** the advice has no "How to carry on" item, by that requirement's own threshold, and offers neither a full re-write nor `/compact`

#### Scenario: Content hints after the compaction
- **WHEN** the latest logged prompt was 40% tool results, and the latest prompt is now an estimate
- **THEN** the advice gives no hints on what fills the context, since the compaction replaced that content
- **AND** the advice rules that look at the whole session, such as thinking cost and cache misses, are given as before

#### Scenario: What the context held
- **WHEN** the latest prompt is an estimate
- **THEN** the `in context` part still breaks down the latest logged prompt, and its heading ends with `, before the compaction at 19:13`

#### Scenario: A request after the compaction
- **WHEN** a request with a prompt follows the compaction
- **THEN** the X-ray describes that request's prompt as before, with no estimate

### Requirement: Cache line after a trailing compaction
When the latest prompt is an estimate after a compaction, the `cache` line SHALL say when the conversation was compacted, how, and what the next request re-builds, in place of warm or cold. The kind `idle` reads `while idle`; other kinds are shown in parentheses, and a marker with no kind shows none. The line has no color.

#### Scenario: Compacted while idle
- **WHEN** the compaction at 19:13 has the kind `idle` and the estimate is ~55.8k
- **THEN** the line reads `cache     compacted while idle at 19:13 · the next request re-builds ~55.8k`

#### Scenario: Auto-compaction
- **WHEN** the compaction at 19:13 has the kind `auto`
- **THEN** the line reads `cache     compacted at 19:13 (auto) · the next request re-builds ~55.8k`

#### Scenario: Codex
- **WHEN** a Codex rollout's `compacted` line comes after its latest usage line
- **THEN** the line reads `cache     compacted at 19:13 · the next request re-builds ~<estimate>`

### Requirement: Forecast after a trailing compaction
When the latest prompt is an estimate after a compaction and the model has a price today, the `next` line SHALL forecast one request. It reads the tool schemas and system prompt from the cache, writes the rest of the estimate plus the conversation's mean new tokens (see Next-request forecast), and adds the mean output. Writes use the price of the conversation's cache lifetime. There is no warm figure.

#### Scenario: Forecast after an idle compaction
- **WHEN** the estimate is 55,800 tokens, 14,400 of them tool schemas and system prompt, the pace adds 2,000 new tokens and 1,000 output tokens per request, on `claude-opus-5-5` with 1-hour writes, in a session that cost $414
- **THEN** the forecast is (14,400 × $0.20 + 43,400 × $8 + 1,000 × $20) per million = $0.37
- **AND** the line reads `next      ≈ $0.37 for one request, re-building ~55.8k = 0% of the session's $414`
