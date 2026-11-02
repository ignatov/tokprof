# Spec Delta

## ADDED Requirements

### Requirement: Lookup advice
When the session's requests with trigger `lookup` (see cost-attribution), subagents included, number at least 20 and cost at least $5 or at least 10% of the session's spend, the advice SHALL say how many requests single lookups made, how many tokens each re-read on average, what they cost and what share of the session that is, and suggest batching independent reads or exploring in a subagent.

#### Scenario: Lookups in a big context
- **WHEN** 412 requests with trigger `lookup` re-read 380,200 tokens on average and cost $58.10 of a $276.67 session
- **THEN** the item reads "Single lookups made 412 requests, each re-reading ≈ 380.2k tokens: $58.10, 21% of the session. Batch independent reads, or explore in a subagent."

#### Scenario: Lookups in subagents
- **WHEN** the main conversation made 5 single lookups and its subagents 300, for $30 of a $200 session
- **THEN** the item counts all 305 requests and their $30

#### Scenario: Few lookups
- **WHEN** 15 single lookups cost $9.00 of a $40.00 session
- **THEN** there is no lookups item, since it takes at least 20 requests

#### Scenario: Cheap lookups
- **WHEN** 200 single lookups cost $3.00 of a $60.00 session, 5%
- **THEN** there is no lookups item

#### Scenario: Unpriced session
- **WHEN** the session's spend is zero because nothing is priced
- **THEN** there is no lookups item

## MODIFIED Requirements

### Requirement: Advice block
When there is a main conversation, the X-ray SHALL print an `advice` block after the `next` line, with the items of each rule that fires, in this order: how to carry on, context cost, tool results, tool calls, lookups, reminders, pasted prompts, thinking, cache misses. The rules are deterministic and every item carries the numbers behind it. Each item says one thing in plain sentences: what it is about, the numbers, and what to do.

#### Scenario: Several rules fire
- **WHEN** three rules fire, one item each
- **THEN** the first item's line carries the `advice` label, the other items are indented under it, and each starts with `▸`

#### Scenario: A session with six rules
- **WHEN** session 3f951de0 is cold, with 817.6k tokens in its latest prompt, and the carry-on, context cost, tool results, tool calls, pasted prompts and thinking rules fire
- **THEN** the block takes 14 lines:
  ```
  advice    ▸ Resuming writes the whole 817.6k context again (+$6.38), then $0.16 per request.
            ▸ A new session that refers to this one is cheaper: ≈ $0.31 to pick up the work.
              `tokprof handoff 3f951de0` prints the line to start it with.
            ▸ /compact costs about as much as resuming (≈ $6.74 once), then ~44.2k per request.
            ▸ The context fills 82% of the window. At 12 requests per prompt, each prompt costs
              ≈ $1.99 in reads alone.
            ▸ Tool results take 45% of the context (Bash 299.9k, Read 63.9k): pipe long command
              output through tail or grep.
            ▸ Tool calls take 32% of the context (Bash 181.2k, Write 62.8k): write long inline
              scripts to a file and run the file.
            ▸ Pasted prompts (turn 1 14.6k, turn 3 29.6k) are re-read on every request: pass a
              file path instead.
            ▸ Thinking cost $16.18, 15% of the session, at effort max: use a lower effort for
              routine steps.
  ```

#### Scenario: Wrapping
- **WHEN** an item is longer than 82 characters
- **THEN** it wraps at 82 characters, and its next lines start 12 columns in, under its text, so the X-ray fits the X-ray pane of `tokprof ui`
- **AND** the fixed texts keep every item within two lines unless a tool or reminder source has a very long name

#### Scenario: No rule fires
- **WHEN** no rule fires, as for a small session with a balanced context
- **THEN** there is no `advice` block
