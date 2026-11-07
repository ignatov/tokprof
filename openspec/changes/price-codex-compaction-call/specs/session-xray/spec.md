## MODIFIED Requirements

### Requirement: How to carry on
When the latest prompt holds at least 100,000 tokens, the first advice items SHALL say how to carry on, in up to three items: resuming, a new session that refers to this one, and `/compact`. On a cold cache they give what resuming re-writes and costs, the pickup estimate and what `/compact` first costs. On a warm cache they give the time left, what `/compact` costs now and what the first request after a break would add. Dollars use today's prices; without a price only the token counts remain.

#### Scenario: Cold cache, a new session is cheaper
- **WHEN** a 950.0k-token Claude Code prompt on `claude-opus-5-5` with 1-hour writes is cold, its conversation's first prompt held 40,000 tokens, its tool schemas and system prompt hold 30,000, and it was never compacted
- **THEN** the items read "Resuming writes the whole 950.0k context again (+$7.41), then $0.19 per request.", "A new session that refers to this one is cheaper: ≈ $0.45 to pick up the work. `tokprof handoff 3f951de0` prints the line to start it with." and "/compact costs about as much as resuming (≈ $7.80 once), then ~50.0k per request."
- **AND** `+$7.41` is the prompt at the write price less the read price, `$0.19` the prompt at the read price, and the handoff command takes the session's short id
- **AND** `/compact` once is the prompt at the write price plus 10,000 output tokens, and `~50.0k` is the prompt right after the session's last compaction or, without one, the tool schemas and system prompt plus 20,000 tokens

#### Scenario: Cold cache, a new session is not cheaper
- **WHEN** the cache is cold and the pickup estimate is not below the extra cost of re-writing the prompt
- **THEN** the second item reads "A new session that refers to this one costs about as much (≈ <pickup>)." and gives no handoff command

#### Scenario: Warm cache, a new session is cheaper after a break
- **WHEN** the same prompt's cache stays warm for 50 more minutes
- **THEN** the items read "Cache warm for 50m: keep going now, or /compact before a break (≈ $0.39 now).", "Once it is cold, resuming writes the whole 950.0k context again (+$7.41)." and "Then a new session that refers to this one is cheaper: ≈ $0.45 to pick up the work (`tokprof handoff 3f951de0`)."
- **AND** `/compact` now is the prompt at the read price plus 10,000 output tokens

#### Scenario: Warm cache on Codex
- **WHEN** a 230.0k-token Codex prompt on `gpt-6-astra` stays warm for 20 more minutes
- **THEN** the first item reads "Cache warm for 20m: keep going now, or /compact before a break (≈ $3.38 now)."
- **AND** `/compact` now is the prompt at the write price plus 10,000 output tokens, (230,000 × 12.50 + 10,000 × 50) / 1,000,000, because Codex's compaction call reads nothing from the cache (see cache-misses, Compaction call)

#### Scenario: Warm cache, a new session is not cheaper
- **WHEN** the cache is warm and the pickup estimate is not below the extra cost of re-writing the prompt
- **THEN** the items end after "Once it is cold, resuming writes the whole <size> context again (+<extra>).", with no third item

#### Scenario: No price
- **WHEN** the model has no price today
- **THEN** there is one item: "Resuming writes the whole <size> context again." on a cold cache, or "Cache warm for <left>: keep going now." on a warm one

#### Scenario: Smaller context
- **WHEN** the latest prompt holds fewer than 100,000 tokens
- **THEN** there is no item on how to carry on

### Requirement: Cache miss advice
When the unexpected cache misses of the session, subagents included, have an avoidable cost of at least $5 or at least 5% of the session's spend, the advice SHALL give that cost, the cause whose misses cost the most with their count, and a fixed hint for that cause.

#### Scenario: Misses after idle breaks
- **WHEN** three misses after idle breaks cost $12.00 that could have been avoided
- **THEN** the item reads "Cache misses could have saved $12.00, mostly idle > TTL (3×): compact or wrap up a big session before a long break."

#### Scenario: Hints by cause
- **WHEN** the costliest cause is a model or parameter change, a change of tools, system prompt or agent version, rewritten history, or a step past the lookback window
- **THEN** the hint is, in turn, "switch model or effort in a new session, not mid-task", "avoid toggling MCP servers or plugins, or updating the agent, mid-task", "earlier messages were rewritten, and everything after them was written again" or "one step added more than 20 content blocks, past the cache lookback window"
- **AND** misses of unknown cause get "the logs do not show the cause"

#### Scenario: Compaction calls
- **WHEN** 162 compaction calls of a Codex session cost $250 that could have been avoided, more than any other cause
- **THEN** the item reads "Cache misses could have saved $250, mostly compaction call (162×): each compaction wrote the whole context again; start a new session from a handoff before the context fills."

#### Scenario: Small misses
- **WHEN** the unexpected misses could have saved $3.09 of a $107 session
- **THEN** there is no cache miss item
