# Cost Attribution Specification

## Purpose
Turn the token usage logged for every request into API-equivalent dollars and charge them to the content that caused them, the way prefix caching bills, so spend can be broken down by session, chain, turn, content kind, tool and target, and summed up by `tokprof summary`.

## Requirements

### Requirement: Prompt cost by position
tokprof SHALL charge a request's prompt by position, the way prefix caching bills: the first `cache_read` tokens at the read rate, the next `cache_write` tokens at the write rate and the rest at the input rate. The prompt is all the uncached, cache-read and cache-write tokens the agent logged for the request, and the rates are those that pricing gives for the request's model, time and prompt size.

#### Scenario: A 100k prompt with 80k read, 15k written and 5k uncached
- **WHEN** a `claude-opus-5-5` request (input $4, 5-minute write $5, read $0.20 per million tokens) logs 80,000 `cache_read_input_tokens`, 15,000 `cache_creation_input_tokens` with the 5-minute lifetime and 5,000 `input_tokens`
- **THEN** its prompt costs $0.016 in cache read, $0.075 in cache write and $0.02 in uncached input, $0.111 in all

#### Scenario: Codex usage
- **WHEN** a Codex usage record logs 50,000 `input_tokens`, of which 40,000 are `cached_input_tokens`, and no `cache_write_input_tokens`
- **THEN** 40,000 tokens are charged at the read rate and 10,000 at the input rate

#### Scenario: A request without usage
- **WHEN** a request was sent but no usage was logged for it, because it was cancelled or got no response
- **THEN** it is not charged and no report counts it

### Requirement: Prompt layout
tokprof SHALL lay out a rebuilt prompt in the order it was sent (tool schemas, then the system prompt, then the conversation messages in order) and give each block a span proportional to its size in characters, scaled so that the blocks add up to the logged prompt size. A block's token count is therefore an estimate, while the prompt's total is as logged.

#### Scenario: Old content is read, new content is written
- **WHEN** a 100,000-token prompt holds 20,000 tokens of tool schemas and system prompt, 60,000 of earlier messages, a 15,000-token tool result and a 5,000-token new prompt, and logs 80,000 read, 15,000 written and 5,000 uncached tokens
- **THEN** the tool schemas, the system prompt and the earlier messages are charged at the read rate, the tool result at the write rate and the new prompt at the input rate

#### Scenario: Characters scaled to tokens
- **WHEN** a rebuilt prompt has 400,000 characters and the agent logged 100,000 prompt tokens
- **THEN** a 40,000-character tool result in it counts as 10,000 tokens

#### Scenario: A block across a boundary
- **WHEN** a block spans tokens 75,000 to 85,000 of a prompt that read 80,000 tokens from the cache and wrote the rest
- **THEN** 5,000 of its tokens are charged as cache read and 5,000 as cache write

### Requirement: Content pays while it stays in the context
tokprof SHALL charge every block its share in each request whose prompt contains it, so content pays for as long as it stays in the context, not only when it arrives.

#### Scenario: A file read once
- **WHEN** a 10,000-token file read is written to the 5-minute cache at $5 per million tokens and then read from the cache by 50 more requests at $0.20
- **THEN** it is charged $0.05 in cache write and $0.10 in cache read

#### Scenario: Output that becomes input
- **WHEN** a response's tool call is sent back in the following requests
- **THEN** it is charged as output by the request that produced it and as input by every later request that contains it

### Requirement: Cost components
tokprof SHALL split spend into five components, each with a short name for filters: `cache read` (`read`), prompt tokens read from the cache; `cache write` (`write`), prompt tokens written to the cache; `rewrite after miss` (`rewrite`), the cached prefix that a miss processed again; `uncached input` (`uncached`), prompt tokens neither read nor written; `output` (`output`), generated tokens, reasoning included.

#### Scenario: A request with every component
- **WHEN** after a 100,000-token prompt a request reads 20,000 tokens from the cache, writes 85,000, sends 100 uncached and generates 2,000
- **THEN** its spend is 20,000 tokens of `cache read`, 80,000 of `rewrite after miss`, 5,000 of `cache write`, 100 of `uncached input` and 2,000 of `output`

#### Scenario: A request without a miss
- **WHEN** a request is not a cache miss
- **THEN** it has no `rewrite after miss` spend

### Requirement: Rewrite after a miss
When a request misses the cache (see cache-misses), tokprof SHALL charge its re-processed tokens as `rewrite after miss`, with the miss cause, instead of as cache write or uncached input. They are the span right after the cache-read tokens, priced at the write rate where it lies in the written span and at the input rate beyond it.

#### Scenario: A miss on a growing prompt
- **WHEN** the previous request's prompt was 100,000 tokens and the next `claude-opus-5-5` request sends 105,000, reads 20,000 from the cache and writes 85,000 with the 5-minute lifetime ($5)
- **THEN** 80,000 tokens are `rewrite after miss` for $0.40, with the miss cause
- **AND** the remaining 5,000 written tokens are `cache write` for $0.025

#### Scenario: A miss beyond the written span
- **WHEN** a Codex request after a 50,000-token prompt sends 52,000 tokens, of which 10,000 are cached and none written
- **THEN** 40,000 tokens are `rewrite after miss` at the input rate and 2,000 are `uncached input`

#### Scenario: The first request of a chain
- **WHEN** the first request of a chain writes its whole 30,000-token prompt to the cache
- **THEN** it is charged as `cache write`, since there was no cached prefix to miss

### Requirement: Write rate by cache lifetime
tokprof SHALL price a request's written tokens at the average of its 5-minute and 1-hour write rates, weighted by the tokens the log reports for each lifetime (`cache_creation.ephemeral_5m_input_tokens` and `ephemeral_1h_input_tokens`). Written tokens without a lifetime, and Codex's `cache_write_input_tokens`, count at the short-lifetime rate.

#### Scenario: Mixed lifetimes
- **WHEN** a `claude-opus-5-5` request writes 10,000 tokens with the 5-minute lifetime ($5) and 30,000 with the 1-hour lifetime ($8)
- **THEN** all 40,000 written tokens are charged at $7.25 per million, $0.29 in all

#### Scenario: No lifetime breakdown
- **WHEN** a Claude Code usage record has `cache_creation_input_tokens` but no `cache_creation` object
- **THEN** all of its written tokens are charged at the 5-minute write rate

### Requirement: Output charged to what the response produced
tokprof SHALL charge output tokens at the output rate to the blocks of the message the response produced, shared by size. When the agent logs reasoning tokens (Claude Code `output_tokens_details.thinking_tokens`, Codex `reasoning_output_tokens`), the thinking blocks share those and the other produced blocks share the rest; otherwise all produced blocks share the whole output.

#### Scenario: Reasoning reported
- **WHEN** a response logs 1,000 output tokens, 600 of them reasoning, and produced a thinking block, a 300-character text and a 100-character tool call
- **THEN** the thinking block gets 600 tokens, the text 300 and the tool call 100

#### Scenario: No reasoning reported
- **WHEN** a response logs 1,000 output tokens without a reasoning count and produced a 2,000-character thinking block and a 2,000-character text
- **THEN** each gets 500 tokens

#### Scenario: Hidden thinking
- **WHEN** 600 reasoning tokens are logged but the produced message holds no thinking content
- **THEN** the 600 tokens go to a `thinking` share with no tool, target or turn

#### Scenario: No produced message
- **WHEN** the log does not tie a response to the message it produced
- **THEN** its output goes to an `assistant text` share with no tool, target or turn, apart from logged reasoning tokens, which go to a `thinking` share

### Requirement: Unknown content
tokprof SHALL charge prompt tokens it cannot place to content of kind `unknown` rather than guess what they were. A request whose prompt cannot be rebuilt (see session-logs) puts its whole prompt in one `unknown` share. A message the log refers to but does not hold becomes an `unknown` block, its size estimated as the average of the prompt's resolved messages.

#### Scenario: A transcript without request records
- **WHEN** a Claude Code transcript from before 2.1.308 has no request records
- **THEN** each request's whole prompt is one `unknown` share with no tool, target or turn, while its output still goes to the blocks it produced
- **AND** `tokprof summary` counts the request as one without a known prompt

#### Scenario: A missing message
- **WHEN** a rebuilt prompt refers to a message that is not in the log, and its resolved messages are 3,000 and 5,000 characters long
- **THEN** the missing message is an `unknown` block counted as 4,000 characters

#### Scenario: No resolved message
- **WHEN** none of the messages a rebuilt prompt refers to is in the log
- **THEN** each of them counts as one character, so the tool schemas and the system prompt take nearly the whole prompt

### Requirement: Dimensions of spend
Every share of spend SHALL carry the dimensions that profiles stack by (see profile-export): agent (`claude` or `codex`), project (the session's working directory, home shown as `~`), session (the first 8 characters of its id), chain (its chain per session-logs, or `advisor` for advisor calls), model (as logged), turn, component, cause (on `rewrite after miss` only), kind, tool (the tool's name) and target (what the tool worked on, or a reminder's source).

#### Scenario: Content kinds
- **WHEN** spend is broken down by kind
- **THEN** the kinds are `tool schemas`, `system prompt`, `user prompt`, `reminders`, `tool results`, `assistant text`, `thinking`, `tool calls`, `images` and `unknown`

#### Scenario: A tool result
- **WHEN** a Claude Code `Read` of `src/main.rs` in the project returns a result
- **THEN** its spend has kind `tool results`, tool `Read` and target `src/main.rs`, and a cause only on its `rewrite after miss` share

#### Scenario: A reminder
- **WHEN** Claude Code injects the CLAUDE.md instructions as a system reminder
- **THEN** its spend has kind `reminders`, target `instructions` and no tool

#### Scenario: An advisor call
- **WHEN** a response consulted the advisor, which reads the conversation with another model
- **THEN** the advisor request is charged to chain `advisor` under the advisor's model, apart from the conversation's own chain

### Requirement: Turn of content
tokprof SHALL give content the turn in which it entered the context: the number of user-typed messages in the prompt up to and including its own message, in the first request of the reported window that contained it. Output takes the turn of the request that produced it. The tool schemas and system prompt that precede the conversation have no turn and show as `setup`.

#### Scenario: Content charged in later turns
- **WHEN** a tool result arrives after the third user prompt and is read from the cache until the seventh
- **THEN** all of its spend has turn 3

#### Scenario: After a compaction
- **WHEN** a compaction replaces the conversation with a summary
- **THEN** the turns of new content are counted within the new prompt, from its first user-typed message on

#### Scenario: Content before the first user prompt
- **WHEN** instructions are injected into the conversation before the first user-typed message
- **THEN** they have turn 0

#### Scenario: Shares without content of their own
- **WHEN** a share is `unknown` prompt content or output not tied to a produced message
- **THEN** it has no turn

### Requirement: Spend is dated when its content entered the context
tokprof SHALL date each share of spend when its content entered the context: the first request of the reported window whose prompt held it, or for output the request that produced it. Spend over time is thus charged back to the content that caused it, not spread over the requests that kept paying for it.

#### Scenario: A file read in the morning
- **WHEN** a file is read at 10:00 and stays in the context, read from the cache, until 12:00
- **THEN** all of its spend is dated 10:00

#### Scenario: Shares without content of their own
- **WHEN** a share is `unknown` prompt content or output not tied to a produced message
- **THEN** it is dated at the chain's first request in the reported window

### Requirement: Measures
Every share of spend SHALL carry three measures: tokens; cost in USD, absent when its model has no price; and avoidable cost in USD, which is the miss premium (see cache-misses) on `rewrite after miss` shares, expected rebuilds included, and zero on every other share.

#### Scenario: A rewrite share
- **WHEN** 80,000 tokens are `rewrite after miss` at a $5 write rate with a $0.20 read rate
- **THEN** the share has 80,000 tokens, a cost of $0.40 and an avoidable cost of $0.384

#### Scenario: A cache read share
- **WHEN** a share is `cache read`
- **THEN** its avoidable cost is zero

#### Scenario: A compaction rebuild
- **WHEN** a miss has the cause `compaction`
- **THEN** its `rewrite after miss` share still carries its premium as avoidable cost

### Requirement: Models without a price
tokprof SHALL attribute requests to a model without a price (see pricing) in tokens only: their shares have no cost and no avoidable premium, and `tokprof summary` shows token counts in their cells, `<tokens> tok` as their total, and leaves them out of `Total priced` and of the avoidable total.

#### Scenario: A model with no public price
- **WHEN** the logs hold `codex-auto-review` requests and no price file prices that model
- **THEN** its summary row shows token counts such as `90.5M` in the component columns and `101.3M tok` as the total
- **AND** its cache misses show `-` as avoidable

#### Scenario: A Codex request without a model
- **WHEN** a Codex rollout logs usage before it names a model
- **THEN** the request is attributed to model `unknown`, which has no price

### Requirement: Summary report
`tokprof summary` SHALL print `Spend by model (API-equivalent USD; token counts where no price is known)` and a table with one row per agent and model: `agent`, `model`, `requests`, `prompt known`, the five components and `total`, sorted by total cost, highest first. Then come the totals line, the unpriced models (see pricing), the reconciliation line when there is one, and the `Cache misses by cause` table (see cache-misses) without expected rebuilds.

#### Scenario: A model row
- **WHEN** the reported window holds 16,281 requests to `claude-opus-5-5`, all with a rebuilt prompt
- **THEN** its row starts with `claude`, `claude-opus-5-5`, `16281` and `100%`, followed by the dollars of each component and the total, such as `$1,192` or `$0.14`

#### Scenario: Requests and prompt known
- **WHEN** a model had 200 requests with usage in the window, advisor calls included, and 150 of them had a rebuilt prompt
- **THEN** its row shows `200` requests and `75%` prompt known

### Requirement: Summary totals line
After the spend table, `tokprof summary` SHALL print `Total priced: <usd>   avoidable on cache misses: <usd> (<share>%)`: the cost and the avoidable premium summed over every priced model, with the premium's share of the cost to one decimal.

#### Scenario: Expected rebuilds in the total
- **WHEN** the window holds $506 of premium on unexpected misses and $42 on compactions and cleared tool results, out of $2,628 priced
- **THEN** the line reads `Total priced: $2,628   avoidable on cache misses: $548 (20.9%)`, while the cause table below it lists only the $506

### Requirement: Reconciliation with the agent's own cost counter
When a reported session logged Claude Code's own cost counter (`totalCostUSD` in `cost-state` lines), `tokprof summary` SHALL print `Claude Code's own cost counter for <n> sessions: <usd>; tokprof for the same sessions: <usd>`: the sum of the last counter value of each such session, and tokprof's priced spend for the same sessions.

#### Scenario: All history
- **WHEN** 275 sessions logged a counter, summing to $10,241, and tokprof prices the same sessions at $10,353
- **THEN** the line reads `Claude Code's own cost counter for 275 sessions: $10,241; tokprof for the same sessions: $10,353`

#### Scenario: A session with subagent transcripts
- **WHEN** a session's main transcript and its subagent transcripts are read
- **THEN** they count as one session, with the spend of all of them

#### Scenario: A time filter
- **WHEN** `--since` or `--until` is given
- **THEN** each counter still covers its session's whole life, and sessions without requests in the window still count, while tokprof's figure covers only the requests in the window

#### Scenario: No counter
- **WHEN** no reported session logged a counter, as with Codex sessions only
- **THEN** the line is left out
