## MODIFIED Requirements

### Requirement: Advice block
When there is a main conversation, the X-ray SHALL print an `advice` block after the `next` line, with the items of each rule that fires, in this order: how to carry on, context cost, tool results, tool calls, reminders, pasted prompts, thinking, cache misses. The rules are deterministic and every item carries the numbers behind it. Each item says one thing in plain sentences: what it is about, the numbers, and what to do.

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

### Requirement: How to carry on
When the latest prompt holds at least 100,000 tokens, the first advice items SHALL say how to carry on, in up to three items: resuming, a new session that refers to this one, and `/compact`. On a cold cache they give what resuming re-writes and costs, the pickup estimate and what `/compact` first costs. On a warm cache they give the time left, what `/compact` costs now and what the first request after a break would add. Dollars use today's prices; without a price only the token counts remain.

#### Scenario: Cold cache, a new session is cheaper
- **WHEN** a 950.0k-token Claude Code prompt on `claude-opus-5-5` with 1-hour writes is cold, its conversation's first prompt held 40,000 tokens, its tool schemas and system prompt hold 30,000, and it was never compacted
- **THEN** the items read "Resuming writes the whole 950.0k context again (+$7.41), then $0.19 per request.", "A new session that refers to this one is cheaper: ≈ $0.45 to pick up the work. `tokprof handoff 3f951de0` prints the line to start it with." and "/compact costs about as much as resuming (≈ $7.80 once), then ~50.0k per request."
- **AND** `+$7.41` is the prompt at the write price less the read price, `$0.19` the prompt at the read price, and the handoff command takes the session's 8-character id
- **AND** `/compact` once is the prompt at the write price plus 10,000 output tokens, and `~50.0k` is the prompt right after the session's last compaction or, without one, the tool schemas and system prompt plus 20,000 tokens

#### Scenario: Cold cache, a new session is not cheaper
- **WHEN** the cache is cold and the pickup estimate is not below the extra cost of re-writing the prompt
- **THEN** the second item reads "A new session that refers to this one costs about as much (≈ <pickup>)." and gives no handoff command

#### Scenario: Warm cache, a new session is cheaper after a break
- **WHEN** the same prompt's cache stays warm for 50 more minutes
- **THEN** the items read "Cache warm for 50m: keep going now, or /compact before a break (≈ $0.39 now).", "Once it is cold, resuming writes the whole 950.0k context again (+$7.41)." and "Then a new session that refers to this one is cheaper: ≈ $0.45 to pick up the work (`tokprof handoff 3f951de0`)."
- **AND** `/compact` now is the prompt at the read price plus 10,000 output tokens

#### Scenario: Warm cache, a new session is not cheaper
- **WHEN** the cache is warm and the pickup estimate is not below the extra cost of re-writing the prompt
- **THEN** the items end after "Once it is cold, resuming writes the whole <size> context again (+<extra>).", with no third item

#### Scenario: No price
- **WHEN** the model has no price today
- **THEN** there is one item: "Resuming writes the whole <size> context again." on a cold cache, or "Cache warm for <left>: keep going now." on a warm one

#### Scenario: Smaller context
- **WHEN** the latest prompt holds fewer than 100,000 tokens
- **THEN** there is no item on how to carry on

### Requirement: Pickup estimate
The pickup cost SHALL be a calibrated estimate of picking up the work in a new session that refers to this one, from what the reference bench measured (docs/reference-bench.md). The new session starts from the smaller of the conversation's first prompt and its prompt right after the last compaction. Claude Code then takes in 8,000 tokens over 4 requests and writes 2,000; Codex takes in 30,000 over 6 requests and writes 4,000.

#### Scenario: How the estimate adds up
- **WHEN** the pickup is estimated
- **THEN** it is the start and the taken-in tokens written once, plus the start and half the taken-in tokens read on each further request, plus the output
- **AND** it uses today's prices and the old conversation's cache-write lifetime

#### Scenario: Claude Code calibration
- **WHEN** a Claude Code session on `claude-opus-5-5` with 1-hour writes starts from 42,000 tokens
- **THEN** the estimate is within the $0.44–0.51 the reference bench measured

#### Scenario: Codex calibration
- **WHEN** a Codex session on `gpt-6-astra` starts from 18,000 tokens
- **THEN** the estimate is within the $0.70–1.26 the reference bench measured

#### Scenario: Forked session that starts big
- **WHEN** a session forked from another started at 171,000 tokens and holds 21,800 tokens right after its last compaction
- **THEN** the estimate starts from 21,800 tokens, ≈ $0.29 on `claude-opus-5-5` with 1-hour writes

#### Scenario: No estimate
- **WHEN** the model has no price today or the start is zero
- **THEN** there is no pickup estimate and no item on a new session, and a cold cache has no item on `/compact` either

### Requirement: Context cost advice
When the model has a price today and the latest prompt holds at least 200,000 tokens or fills at least 70% of the window, the advice SHALL say what the context costs. It gives the window share, the conversation's requests per prompt and what each prompt costs in context reads alone, or what each request re-reads when the context holds no turn, and adds `Auto-compaction is close.` at 85% of the window or more.

#### Scenario: Nearly full window
- **WHEN** a 950.0k-token prompt fills 95% of a 1M window at 30 requests per prompt on `claude-opus-5-5`
- **THEN** the item reads "The context fills 95% of the window. At 30 requests per prompt, each prompt costs ≈ $5.70 in reads alone. Auto-compaction is close."
- **AND** requests per prompt are the conversation's requests with a prompt divided by the turns in the latest prompt, and a prompt's cost is that many reads of the whole prompt

#### Scenario: Context without turns
- **WHEN** the latest prompt holds no turn
- **THEN** the item says what each request re-reads, as in "The context fills <share>% of the window. Each request re-reads <size> tokens (<cost>)."

#### Scenario: Unknown window
- **WHEN** the prompt holds 200,000 tokens or more and the window is unknown
- **THEN** the item leaves out the window sentence, as in "At 30 requests per prompt, each prompt costs ≈ $5.70 in reads alone."

#### Scenario: Below the thresholds
- **WHEN** the prompt holds fewer than 200,000 tokens and fills less than 70% of the window
- **THEN** there is no context cost item

### Requirement: Hints on what fills the context
The advice SHALL add one item for each kind of content in the latest prompt that passes its threshold: tool results at 30% of the prompt or more, tool calls at 20% or more, reminders at 30,000 tokens or 5% or more, and user prompt blocks of 10,000 tokens or more, reported as pasted prompts. Each item gives the share or size, up to two leading sources with their tokens, and a fixed hint chosen by the source that holds the most.

#### Scenario: Bash output leads the tool results
- **WHEN** tool results fill 84% of the prompt and Bash holds all of them
- **THEN** the item reads "Tool results take 84% of the context (Bash 800.0k): pipe long command output through tail or grep."
- **AND** the same hint goes with `BashOutput`, `exec_command` and `shell`

#### Scenario: Two leading sources
- **WHEN** tool results fill 45% of the prompt, with Bash 299.9k, Read 63.9k and ListAgents 1.3k
- **THEN** the item reads "Tool results take 45% of the context (Bash 299.9k, Read 63.9k): pipe long command output through tail or grep."

#### Scenario: Other leading tool results
- **WHEN** `Read`, `WebFetch` or `WebSearch`, or `Grep` or `Glob` lead the tool results
- **THEN** the hint is "read only the ranges you need", "ask for a summary instead of the page" or "narrow the pattern or the path"
- **AND** any other leading tool gets "use narrower calls, or a subagent for exploration"

#### Scenario: Inline scripts lead the tool calls
- **WHEN** tool calls fill 20% of the prompt or more and `Bash`, `exec_command` or `shell` lead them
- **THEN** the item starts with "Tool calls take <share>% of the context (<sources>):" and gives "write long inline scripts to a file and run the file"

#### Scenario: File writes lead the tool calls
- **WHEN** `Write`, or `Edit` or `MultiEdit`, lead the tool calls
- **THEN** the hint is "prefer small edits to whole-file writes" or "smaller edits are cheaper"
- **AND** any other leading tool gets "keep tool inputs short"

#### Scenario: Instruction files lead the reminders
- **WHEN** reminders hold 30,000 tokens or 5% of the prompt and instruction files hold the most of them
- **THEN** the item starts with "Reminders hold <tokens> tokens (<sources>):" and gives "trim CLAUDE.md and AGENTS.md, or move rarely needed parts to docs read on demand"

#### Scenario: Other leading reminders
- **WHEN** background notifications, hook context, deferred tool lists or skills lead the reminders
- **THEN** the hint is "run fewer or quieter background jobs", "make the hooks print less", "use fewer MCP servers or tools" or "load fewer skills per session"
- **AND** any other leading source gets "trim what the task does not need"

#### Scenario: Pasted prompts
- **WHEN** a user prompt block of 120.0k tokens entered in turn 2
- **THEN** the item reads "Pasted prompts (turn 2 120.0k) are re-read on every request: pass a file path instead."

#### Scenario: Many pasted prompts
- **WHEN** five user prompt blocks hold 10,000 tokens or more each
- **THEN** the item lists the three largest in turn order, each with its turn and size, then `and 2 more`, as in "Pasted prompts (turn 1 14.6k, turn 3 29.6k, turn 12 110.2k and 2 more) are re-read on every request: pass a file path instead."

### Requirement: Thinking cost advice
When thinking costs at least 10% of the session's spend, subagents included, the advice SHALL say what it cost and what share of the session that is, with the effort level the conversation's requests used most, and suggest a lower effort for routine steps.

#### Scenario: Thinking at a high effort
- **WHEN** thinking cost $20.00 of a $100.00 session and most requests used effort `max`
- **THEN** the item reads "Thinking cost $20.00, 20% of the session, at effort max: use a lower effort for routine steps."

#### Scenario: No effort in the log
- **WHEN** the log records no effort level
- **THEN** the item leaves out `, at effort <level>`, as in "Thinking cost $20.00, 20% of the session: use a lower effort for routine steps."

#### Scenario: Unpriced session
- **WHEN** the session's spend is zero because nothing is priced
- **THEN** there is no thinking item

### Requirement: Cache miss advice
When the unexpected cache misses of the session, subagents included, have an avoidable cost of at least $5 or at least 5% of the session's spend, the advice SHALL give that cost, the cause whose misses cost the most with their count, and a fixed hint for that cause.

#### Scenario: Misses after idle breaks
- **WHEN** three misses after idle breaks cost $12.00 that could have been avoided
- **THEN** the item reads "Cache misses could have saved $12.00, mostly idle > TTL (3×): compact or wrap up a big session before a long break."

#### Scenario: Hints by cause
- **WHEN** the costliest cause is a model or parameter change, a change of tools, system prompt or agent version, rewritten history, or a step past the lookback window
- **THEN** the hint is, in turn, "switch model or effort in a new session, not mid-task", "avoid toggling MCP servers or plugins, or updating the agent, mid-task", "earlier messages were rewritten, and everything after them was written again" or "one step added more than 20 content blocks, past the cache lookback window"
- **AND** misses after an assistant message or of unknown cause get "the logs do not show the cause"

#### Scenario: Small misses
- **WHEN** the unexpected misses could have saved $3.09 of a $107 session
- **THEN** there is no cache miss item
