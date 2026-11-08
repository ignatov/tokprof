# Spec Delta

## ADDED Requirements

### Requirement: Requests list
`tokprof scan [target] --requests` SHALL print, in place of the X-ray, the requests of the target's main conversation: the main log's main chains and advisor calls, in time order, for the session `tokprof scan` would pick. The head is `tokprof · requests of the main conversation`, the X-ray's `session` line, and a `requests` line with the count, the cost, and the cost of each of the X-ray's five spend parts that is not zero. Without `--requests`, `tokprof scan` prints the X-ray unchanged.

#### Scenario: The totals
- **WHEN** the X-ray's spend line reads `main $83.55 · subagents $6.87`
- **THEN** the `requests` line gives `$83.55`, and the costs of the rows add up to it
- **AND** in a session with advisor calls, the line gives the sum of `main` and `advisor`

#### Scenario: The plain X-ray
- **WHEN** the user runs `tokprof scan 230043e6`
- **THEN** the output is the X-ray, byte for byte as before this option

#### Scenario: No requests
- **WHEN** the session has no request with usage
- **THEN** the head ends with `requests  0 requests`

### Requirement: Request rows
Under a header, the list SHALL print one row per request: `#`, the local time `MM-DD HH:MM:SS`, the cache read, cache write, uncached and output tokens as the log gives them, the context, the cost and the why. Without prices, the cost is in tokens.

#### Scenario: A miss
- **WHEN** request 57 at 11-03 14:00:12 read 12.3k, wrote 490.3k after 1.0h idle on a 1h cache, sent 3 uncached tokens and 1.2k of output, and cost $3.95 with $3.82 avoidable
- **THEN** its row reads like `    57  11-03 14:00:12   12.3k  490.3k         3    1.2k  505.0k   $3.95  missed: idle > TTL: 1.0h idle, 1h TTL · $3.82 avoidable`

#### Scenario: A growing context
- **WHEN** a request without a miss has a context 5.1k larger than the previous request's
- **THEN** its why reads `context +5.1k`

### Requirement: Why of a request row
A row's why SHALL be the first that applies: `missed: <cause>: <detail>`, with `· $<n> avoidable` when priced, for an unexpected miss; `rebuilt: <cause>` for an expected rebuild; `first request` for the first request of a chain; `context +<n>` or `context −<n>` for a change in size from the previous request; else nothing.

#### Scenario: A rebuild
- **WHEN** a request rebuilt its prompt after a compaction
- **THEN** its why reads `rebuilt: compaction`

### Requirement: Entered and changed lines
Under a row, an `entered` line SHALL give what entered the context since the previous request of its chain, grouped by kind, tool and target, largest first: up to 3 groups, then `<n> more <tokens>`. It is left out when nothing entered or the log does not show the prompt. A `changed` line follows when the model, the effort, another setting the log names, the agent's version or the tool set changed, or a compaction came between.

#### Scenario: What entered
- **WHEN** a tool result of `Bash cargo test` of 6.1k tokens, 400 tokens of assistant text, the call of 200 tokens and two more blocks of 300 tokens entered before a request
- **THEN** the line under its row reads `        entered  6.1k tool results Bash cargo test · 400 assistant text · 200 tool calls Bash cargo test · 2 more 300`

#### Scenario: A change
- **WHEN** the effort went from `high` to `max` and two tools were added since the previous request
- **THEN** the line under its row reads `        changed  effort high → max · tools +2 −0`

### Requirement: Prompts the log does not show
When the log does not show what some prompts of the list held, a line after the head SHALL count them, and their rows SHALL have no `entered` line.

#### Scenario: Prompts not in the log
- **WHEN** 40 requests come from a Claude Code transcript that does not show what the prompts held
- **THEN** a line after the head reads `          the log does not show what 40 prompts held`, and those rows have no `entered` line
