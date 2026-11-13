## MODIFIED Requirements

### Requirement: Header
The X-ray SHALL open with the title `tokprof · session X-ray` and these lines:
- `session` with the agent, the project path (home shown as `~`) and the full session id;
- `title` with the session's title (see cost-attribution) cut to 82 characters, when it has one;
- `models` with each model and its request count, if any;
- `time` with the first and last request in local time, the span between them, the request count and, when there are any, the number of subagent logs.

#### Scenario: Session within one day
- **WHEN** the first and last requests fall on the same local date
- **THEN** the end shows only the time of day, as in `time      2026-11-01 08:49 → 18:15 (9.4h) · 657 requests`

#### Scenario: Session over several days
- **WHEN** a session ran from one day into the next
- **THEN** both ends show the date, as in `time      2026-10-22 11:18 → 2026-10-23 19:57 (1.4d) · 7015 requests · 75 subagents`

#### Scenario: Session without requests
- **WHEN** a session's logs hold no request, as in a Codex rollout that stopped before its first response
- **THEN** there is no `models` line, and the time line reads `time      0 requests`
- **AND** no date from 1970 is shown: whenever no request has a time, the line gives only the counts

#### Scenario: Models
- **WHEN** a session used two models
- **THEN** the `models` line lists both by name with their counts, as in `claude-haiku-4-5 ×12, claude-opus-5-5 ×657`

#### Scenario: Span format
- **WHEN** the span is under a minute, under an hour, under a day, or longer
- **THEN** it shows in seconds (`59s`), whole minutes (`45m`), hours with one decimal (`9.4h`) or days with one decimal (`1.4d`)

#### Scenario: Title
- **WHEN** the session's title is `Review PR #412 against the checklist`
- **THEN** the line after `session` reads `title     Review PR #412 against the checklist`
- **AND** a session without a title has no `title` line

### Requirement: Next-request forecast
When the model has a price today (see pricing), the `next` line SHALL forecast the next request at the conversation's pace: the mean new prompt tokens (cache writes and uncached input) and output of its latest 20 requests with a prompt. Warm, the latest prompt is read from the cache and the new tokens are written; cold, the prompt and the new tokens are written. Both add the output, and writes use the price of the conversation's cache lifetime.

#### Scenario: Warm cache
- **WHEN** the cache is warm
- **THEN** the line gives both costs, as in `next      ≈ $0.11 per request while warm · ≈ $1.31 once cold = 20% of the session's $6.57`

#### Scenario: Cold cache
- **WHEN** the cache is cold
- **THEN** the line gives the cold cost and the prompt it re-writes, as in `next      ≈ $6.57 for one request, re-writing 817.6k = 6% of the session's $107`

#### Scenario: Share of the session's cost
- **WHEN** the session has cost $0.005 or more so far
- **THEN** the line ends with the cold forecast as a share of the session's cost, subagents included
- **AND** when the session's cost is under $0.005, so that it would read `$0.00`, the share is left out, here and on the forecast after a trailing compaction

#### Scenario: No price today
- **WHEN** the latest request's model has no price today
- **THEN** there is no `next` line

### Requirement: Pickup estimate
The pickup cost SHALL be a calibrated estimate of picking up the work in a new session that refers to this one, from what the reference bench measured (docs/reference-bench.md). The new session starts from the smaller of the conversation's first prompt and its prompt right after the last compaction. Claude Code then takes in 8,000 tokens over 4 requests and writes 2,000; Codex takes in 30,000 over 6 requests and writes 4,000.

#### Scenario: How the estimate adds up
- **WHEN** the pickup is estimated
- **THEN** it is the start and the taken-in tokens written once, plus the start and half the taken-in tokens read on each further request, plus the output
- **AND** it uses today's prices for a prompt of the start plus the taken-in tokens, and the old conversation's cache-write lifetime

#### Scenario: Old prompt over a long-context tier
- **WHEN** a Codex session on `gpt-6-astra`, whose prices change for prompts over 272,000 tokens, holds a 300,000-token prompt and starts from 18,000 tokens
- **THEN** the pickup is priced at the standard prices, as for any 48,000-token prompt, while resuming and `/compact` are priced at the long-context ones

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
- **AND** requests per prompt are the conversation's requests whose prompt the log shows, divided by the prompts the user sent in them, and a prompt's cost is that many reads of the whole prompt
- **AND** each request adds as many prompts as its prompt holds turns beyond the previous request's, while the first request after a compaction adds none, since the compaction re-sends earlier prompts

#### Scenario: After a compaction
- **WHEN** a Codex conversation sent 700 requests over 300 prompts, its compaction re-sent 20 of those prompts, and it then sent 20 requests over 2 new prompts
- **THEN** the item gives 2 requests per prompt (720 over 302), not 33 (720 over the 22 turns of the latest prompt)

#### Scenario: Context without turns
- **WHEN** the latest prompt holds no turn
- **THEN** the item says what each request re-reads, as in "The context fills <share>% of the window. Each request re-reads <size> tokens (<cost>)."

#### Scenario: Unknown window
- **WHEN** the prompt holds 200,000 tokens or more and the window is unknown
- **THEN** the item leaves out the window sentence, as in "At 30 requests per prompt, each prompt costs ≈ $5.70 in reads alone."

#### Scenario: Below the thresholds
- **WHEN** the prompt holds fewer than 200,000 tokens and fills less than 70% of the window
- **THEN** there is no context cost item
