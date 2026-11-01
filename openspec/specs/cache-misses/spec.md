# Cache Misses Specification

## Purpose
Find the requests that processed again a prefix the prompt cache should have held, name the most likely cause of each, price the avoidable premium, and report the misses (`tokprof misses`) and what other cache lifetimes would have cost (`tokprof whatif`).

## Requirements

### Requirement: Miss rule
A request SHALL count as a cache miss when it re-processed at least 2,000 tokens and more than 5% of the prefix it could have read from the cache. That prefix is the smaller of the previous request's prompt and its own prompt; the re-processed tokens are that prefix minus the request's `cache_read` tokens. This is the rule of Claude Code's `/usage` prompt-cache line, so the counts are comparable.

#### Scenario: A cold return
- **WHEN** the previous request's prompt was 100,000 tokens and the next one sends 101,000, reading none of them from the cache
- **THEN** it re-processed 100,000 tokens and is a miss

#### Scenario: Under 2,000 tokens
- **WHEN** the previous prompt was 100,000 tokens and the next request reads 98,500 from the cache
- **THEN** it re-processed 1,500 tokens and is not a miss

#### Scenario: Not more than 5%
- **WHEN** the previous prompt was 500,000 tokens and the next request reads 480,000 from the cache
- **THEN** it re-processed 20,000 tokens, not more than 5% of 500,000 (25,000), and is not a miss

#### Scenario: A small prompt at the threshold
- **WHEN** the previous prompt was 30,000 tokens and the next request reads 28,000 from the cache
- **THEN** it re-processed 2,000 tokens, more than 5% of 30,000 (1,500), and is a miss

#### Scenario: A shrunken prompt
- **WHEN** the previous prompt was 200,000 tokens and the next request sends 50,000, reading 10,000 from the cache
- **THEN** the prefix is 50,000 tokens, it re-processed 40,000 and is a miss

### Requirement: Each chain is its own cache lineage
tokprof SHALL judge a request only against the previous request of the same chain (the main conversation, a subagent or a side stream, see session-logs) that got a response; the first request of a chain is never a miss. Advisor calls are never judged and never serve as the previous request. A request before `--since` still serves as the previous request of the first one in the window.

#### Scenario: A subagent's first request
- **WHEN** a subagent starts and writes its 30,000-token prompt to the cache
- **THEN** that request is not a miss, whatever the main conversation's cache holds
- **AND** later misses of the subagent are listed with its chain, such as `subagent:general-purpose`

#### Scenario: A cancelled request
- **WHEN** a request got no response and logged no usage
- **THEN** it is not judged, and the next request is compared with the one before it

#### Scenario: An advisor call
- **WHEN** a response consulted the advisor, which reads the conversation with another model
- **THEN** the advisor request is not a miss, and the next request of the conversation is compared with the one before the advisor call

#### Scenario: The start of a time window
- **WHEN** `--since` starts the window in the middle of a chain
- **THEN** the first request in the window is compared with the last one before it

### Requirement: One cause per miss
tokprof SHALL give each miss exactly one cause: the first that fits, checked from the most to the least certain in this order: `model switch`, `compaction`, `idle > TTL`, `params changed`, `agent updated`, `tools changed` or `system prompt changed`, then `tool results cleared`, `images removed`, `thinking dropped` or `history edited` for an edited history, then `lookback > 20 blocks`, `after assistant message`, and `unknown (server side?)`.

#### Scenario: A model switch after a break
- **WHEN** a request after two idle hours also switched the model
- **THEN** its cause is `model switch`

#### Scenario: A compaction after a break
- **WHEN** the first request after an idle hour also follows a compaction
- **THEN** its cause is `compaction`

#### Scenario: A cause with a detail
- **WHEN** a cause has a detail, such as the idle time
- **THEN** the list of misses shows it as `<cause>: <detail>`, such as `idle > TTL: 3.3h idle, 1h TTL`

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

### Requirement: Idle beyond the cache lifetime
The cause SHALL be `idle > TTL` when more time passed since the previous request than the cache lifetime. For Claude Code the lifetime is 1 hour when the previous request wrote more tokens with the 1-hour lifetime than with the 5-minute one, else 5 minutes; for Codex it is assumed to be 30 minutes. The detail reads `<idle time> idle, <lifetime> TTL`, the idle time in seconds, minutes, or hours or days to one decimal.

#### Scenario: The 1-hour cache
- **WHEN** the previous request wrote 30,000 tokens with the 1-hour lifetime and 2,000 with the 5-minute one, and the next request comes 3 hours 18 minutes later
- **THEN** its cause reads `idle > TTL: 3.3h idle, 1h TTL`

#### Scenario: The 5-minute cache of a subagent
- **WHEN** a subagent's previous request wrote only 5-minute cache tokens and its next request comes 6 minutes later
- **THEN** its cause reads `idle > TTL: 6m idle, 5m TTL`

#### Scenario: Codex
- **WHEN** a Codex request comes 45 minutes after the previous one and misses
- **THEN** its cause reads `idle > TTL: 45m idle, 30m TTL`

#### Scenario: Exactly the lifetime
- **WHEN** a request comes exactly 5 minutes after a request that wrote with the 5-minute lifetime
- **THEN** idleness is not its cause, and the later checks decide

#### Scenario: Days
- **WHEN** a request comes 26.4 hours after the previous one
- **THEN** its detail reads `1.1d idle`

### Requirement: Settings and agent version changes
The cause SHALL be `params changed` when the request's effort, thinking, speed, betas or context management setting differs from the previous request's, both logged, with the first that differs in that order as detail; and `agent updated` when the logged agent version differs, with detail `<previous version> -> <version>`.

#### Scenario: Effort changed
- **WHEN** the effort went from `high` to `xhigh` between two requests and the second missed
- **THEN** its cause reads `params changed: effort`

#### Scenario: Claude Code updated
- **WHEN** a session goes on within the cache lifetime after Claude Code was updated from 2.1.310 to 2.1.311
- **THEN** a miss reads `agent updated: 2.1.310 -> 2.1.311`

#### Scenario: Settings not logged
- **WHEN** either request has no logged settings, such as a Claude Code request without a request record
- **THEN** `params changed` is not considered

### Requirement: Tool and system prompt changes
When the tool schemas and system prompt of both requests are known and differ, ignoring cache markers, the cause SHALL be `tools changed` if the list of tool names differs, with detail `+<added> -<removed> tools`, and otherwise `system prompt changed`, with the signed change in system prompt characters as detail.

#### Scenario: An MCP server connects
- **WHEN** two tools appear in the request's tool list
- **THEN** its cause reads `tools changed: +2 -0 tools`

#### Scenario: The system prompt grows
- **WHEN** the tool names stay the same and the system prompt grew by 120 characters
- **THEN** its cause reads `system prompt changed: +120 chars`

#### Scenario: Only cache markers moved
- **WHEN** the tool schemas and system prompt differ only in their `cache_control` markers
- **THEN** neither cause applies

### Requirement: Edited history
When the request dropped or replaced messages that the previous prompt held, the cause SHALL be, comparing the dropped messages with those now in their place: `tool results cleared` if they held over twice the tool-result characters and over 1,000 more; else `images removed` if they held more images; else `thinking dropped` if they held over 1,000 more thinking characters; else `history edited`. The detail reads `at message <index> of <count>`.

#### Scenario: Cleared tool results
- **WHEN** the messages from index 12 of 40 are replaced, and they held 50,000 characters of tool results where the new ones hold 2,000
- **THEN** its cause reads `tool results cleared: at message 12 of 40`

#### Scenario: Under the clearing threshold
- **WHEN** the replaced messages held 1,500 characters of tool results and the new ones 600
- **THEN** the results do not count as cleared, since the difference is not over 1,000 characters

#### Scenario: Dropped thinking
- **WHEN** the replaced messages held 8,000 characters of thinking and the new ones none, with the same tool results and images
- **THEN** its cause is `thinking dropped`

#### Scenario: A rewind
- **WHEN** the user rewinds the conversation and the new messages differ in other content
- **THEN** its cause is `history edited`

### Requirement: Lookback, assistant messages and unknown misses
When no earlier cause fits, the cause SHALL be `lookback > 20 blocks` for a Claude Code request that appended more than 20 content blocks since the previous request, with detail `<n> blocks appended`, as the API looks back at most 20 blocks for an earlier cache entry; else `after assistant message` when the previous response ended with an assistant message, a Codex signature; else `unknown (server side?)`.

#### Scenario: Many blocks at once
- **WHEN** a Claude Code request appends 34 blocks, such as the results of many parallel tool calls
- **THEN** its cause reads `lookback > 20 blocks: 34 blocks appended`

#### Scenario: Exactly 20 blocks
- **WHEN** a request appends 20 blocks
- **THEN** lookback is not its cause

#### Scenario: Codex after an assistant message
- **WHEN** a Codex response logged an assistant message and the next request misses with no earlier cause
- **THEN** its cause is `after assistant message`

#### Scenario: Codex never gets lookback
- **WHEN** a Codex request appends more than 20 blocks and misses
- **THEN** lookback is not its cause

### Requirement: Expected rebuilds
Misses caused by `compaction` or `tool results cleared` are rebuilds the agent does on purpose. `tokprof summary` and `tokprof misses` SHALL leave them out of their miss tables, except `tokprof misses --expected`, which lists them too and marks them `(expected)` in the cause table. They are still charged as `rewrite after miss`, with their premium (see cost-attribution).

#### Scenario: The default view
- **WHEN** the window holds 12 compaction misses
- **THEN** neither `tokprof summary` nor `tokprof misses` lists them

#### Scenario: Asking for expected rebuilds
- **WHEN** the user runs `tokprof misses --expected`
- **THEN** the cause table has rows such as `compaction (expected)` and `tool results cleared (expected)`
- **AND** the list of the most expensive misses includes them

### Requirement: Avoidable premium
The avoidable premium of a miss SHALL be what its re-processed tokens cost above the read rate: those in the written span times the write rate minus the read rate, plus those beyond it times the input rate minus the read rate. A miss on a model without a price has no premium, shown as `-`.

#### Scenario: A rebuild written to the cache
- **WHEN** a `claude-opus-5-5` miss re-processed 80,000 tokens, all written with the 5-minute lifetime ($5), at a read rate of $0.20
- **THEN** its premium is $0.384

#### Scenario: A rebuild at the input rate
- **WHEN** a Codex miss re-processed 40,000 tokens charged at a $4 input rate, with a $0.40 read rate
- **THEN** its premium is $0.144

#### Scenario: No price
- **WHEN** the model of a miss has no price
- **THEN** its re-processed tokens are still counted and its avoidable column shows `-`

### Requirement: Misses by cause
The `Cache misses by cause` table SHALL have one row per agent and cause, with the columns `agent`, `cause`, `misses`, `re-processed` (tokens) and `avoidable` (USD, or `-` when none of the row's misses had a price), sorted by avoidable premium and then by re-processed tokens, highest first.

#### Scenario: A week of idle misses
- **WHEN** Claude Code had 63 idle misses that re-processed 31.7M tokens with $248 of premium
- **THEN** the table has a row with `claude`, `idle > TTL`, `63`, `31.7M` and `$248`

#### Scenario: Unpriced misses
- **WHEN** every miss of a row was on a model without a price
- **THEN** its avoidable column shows `-`

### Requirement: The misses command
`tokprof misses` SHALL print the `Cache misses by cause` table and then `Most expensive misses`: up to `--top` misses (default 20), by avoidable premium and then re-processed tokens, highest first, with the columns `when` (local `YYYY-MM-DD HH:MM`), `agent`, `project`, `session`, `chain`, `context` (the request's prompt tokens), `re-processed`, `avoidable` and `cause`, which adds `: <detail>` when there is one.

#### Scenario: The default list
- **WHEN** the user runs `tokprof misses`
- **THEN** it lists at most 20 misses, the most expensive first

#### Scenario: A shorter list
- **WHEN** the user runs `tokprof misses --top 5`
- **THEN** it lists at most 5 misses

#### Scenario: A cold return to a big session
- **WHEN** a Claude Code request at 12:36 on 2026-10-26 re-processed 950,200 tokens of a 966,800-token prompt, written with the 1-hour lifetime, after 3.3 idle hours
- **THEN** its row shows `2026-10-26 12:36`, `claude`, the project, the session's 8-character id, `main`, `966.8k`, `950.2k`, `$7.41` and `idle > TTL: 3.3h idle, 1h TTL`

### Requirement: Cache lifetime replay
`tokprof whatif` SHALL replay each priced Claude Code request in the window, advisor calls excluded, under a 5-minute and a 1-hour cache: when the previous request of its chain came at most that long before, the smaller of the previous and the current prompt is read at the read rate and the rest written at that lifetime's write rate; otherwise the whole prompt is written. Output is not counted, and no other miss cause is.

#### Scenario: A 10-minute gap
- **WHEN** a `claude-opus-5-5` request sends 100,000 tokens 10 minutes after a 90,000-token request of its chain
- **THEN** it costs $0.50 under the 5-minute cache, all written at $5, and $0.098 under the 1-hour cache, 90,000 read at $0.20 and 10,000 written at $8

#### Scenario: The first request of a chain
- **WHEN** the first request of a chain sends 100,000 tokens to `claude-opus-5-5`
- **THEN** it costs $0.50 under the 5-minute cache and $0.80 under the 1-hour cache

#### Scenario: Requests left out
- **WHEN** the window holds Codex requests, advisor calls or requests to a model without a price
- **THEN** the replay leaves them out

### Requirement: Whatif report
`tokprof whatif` SHALL print one row each for `main conversation` and `subagents and side requests`, with `requests`, `actual` (the logged input cost, output excluded), `simulated 5m` and `simulated 1h`; then `Cheapest: main conversation <lifetime> (saves <usd> over the other), subagents <lifetime> (saves <usd>).` and the Claude Code settings that set each lifetime, `promptCacheTtl` and `subagentPromptCacheTtl`.

#### Scenario: A caveat first
- **WHEN** the user runs `tokprof whatif`
- **THEN** the report opens with `Claude input cost under each cache TTL, replaying the same requests.` and `The model counts only TTL expiry, so compare the simulated columns with each other.`

#### Scenario: Subagents and side streams
- **WHEN** a request belongs to a subagent or a side stream such as `compact`
- **THEN** it counts in `subagents and side requests`

#### Scenario: A week of Claude Code
- **WHEN** the main conversation's replay costs $1,916 under 5 minutes and $1,478 under 1 hour, and the subagents' $417 and $442
- **THEN** the line reads `Cheapest: main conversation 1h (saves $438 over the other), subagents 5m (saves $25.00).`

#### Scenario: A tie
- **WHEN** both replays of a bucket cost the same, as when it has no requests
- **THEN** its cheapest lifetime reads `1h`
