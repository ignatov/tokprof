## MODIFIED Requirements

### Requirement: /compact first
The /compact replay SHALL add, before the first request back, a compaction call that outputs a summary of an estimated 10k tokens. Claude Code's call, which its logs do not hold, is modeled as a fork that shares the conversation's cache: on the cold cache it reads what the return still read from the cache (at most the base) and writes the rest at the write price of the cache lifetime in use. Codex's call reads nothing from the cache and writes the whole context (see cache-misses).

#### Scenario: A 500k-token context on the 1-hour cache
- **WHEN** a `claude-opus-5-5` return has a 500k-token context on the 1-hour cache and read 10k tokens from the cache
- **THEN** the compaction costs (10,000 × 0.20 + 490,000 × 8 + 10,000 × 20) / 1,000,000 = $4.122 before the first request

#### Scenario: Codex
- **WHEN** a `gpt-6-astra` Codex return has a 200k-token context and read 15k tokens from the cache
- **THEN** the compaction writes all 200k tokens at the model's short-lifetime write price and costs (200,000 × 12.50 + 10,000 × 50) / 1,000,000 = $3.00 before the first request

#### Scenario: Too few requests to pay off
- **WHEN** that 500k-token return with a 50k base is followed for 4 requests
- **THEN** after them resuming as is costs $4.37, /compact first $4.62, the handoff note $0.80 and heartbeats $0.75

### Requirement: Heartbeats, then compaction
The fifth replay SHALL send the pings of the heartbeat replay, and at the next ping time past `--keep-warm`, while the break lasts, a compaction instead of a ping. The compaction reads the context at the cache-read price of its time, or writes it for Codex, and outputs the 10k-token summary. A return before the compaction is priced as a return kept warm. After it, the followed requests run on the base, as /compact first runs them, and the compaction counts before the first request.

#### Scenario: A return inside the window
- **WHEN** the 500k-token `claude-opus-5-5` return on the 1-hour cache comes 119 minutes after the last request, with `--keep-warm 2h`
- **THEN** pings went out at 50 and 100 minutes for $0.204, no compaction went out, and the return is priced warm, as the heartbeat replay prices it

#### Scenario: A return after the compaction
- **WHEN** the same return comes 3 hours after the last request, with `--keep-warm 2h`
- **THEN** the pings at 50 and 100 minutes and a compaction at 150 minutes, (500,000 × 0.20 + 10,000 × 20) / 1,000,000 = $0.30, are spent before the first request
- **AND** the followed requests run on the base, as they do for /compact first

#### Scenario: A Codex return after the compaction
- **WHEN** a 200k-token `gpt-6-astra` Codex return comes after the compaction
- **THEN** the compaction costs (200,000 × 12.50 + 10,000 × 50) / 1,000,000 = $3.00, as much as it would on a cold cache

#### Scenario: No window
- **WHEN** the bench runs with `--keep-warm 0`
- **THEN** no ping goes out, and the compaction goes out at the first ping time: 50 minutes after the last request on the 1-hour cache, 4 minutes 10 seconds on the 5-minute one

### Requirement: The report states its assumptions
The report SHALL end with notes on its assumptions: the base of each agent with returns, as a median of so many compactions or as a first prompt plus a note; that a compaction re-writes the cold context and outputs 10k tokens and a handoff note is 10k tokens written while the cache was warm; how often a heartbeat goes out, what it reads and outputs, and what it assumes for Codex; and when heartbeats then compaction compacts, and what that compaction reads or writes.

#### Scenario: The reference run
- **WHEN** the bench runs on the logs behind `docs/bench.txt`
- **THEN** the report ends with `The alternatives start from what a compaction leaves: claude 52.5k (median of 31 compactions), codex 20.8k (median of 791 compactions).`
- **AND** `A compaction re-writes the cold context and outputs 10.0k tokens; a handoff note is 10.0k tokens written while the cache was warm.`
- **AND** `A heartbeat goes out at 5/6 of the cache lifetime, every 50m on a 1h cache, reads the context and outputs 100 tokens; for Codex this assumes it resets the cache timer as it does in Claude Code.`
- **AND** `Heartbeats, then compaction sends the same pings and, at the next ping time past the window, a compaction while the cache is warm that outputs 10.0k tokens: it reads the context in Claude Code and writes it again in Codex, whose compaction calls read nothing from the cache.`

#### Scenario: Claude Code alone
- **WHEN** only Claude Code has returns
- **THEN** the last note reads `Heartbeats, then compaction sends the same pings and, at the next ping time past the window, a compaction that reads the warm context and outputs 10.0k tokens.`

### Requirement: Chart labels
The chart SHALL say what every number and mark is: a title and subtitle naming the measure and the returns it covers; a legend naming each way, as logged or replayed, drawn like its line; for each agent with returns, a panel headed with its return count and median context; a dollar axis and a request axis; the cost of the first request; each way's cost after N requests; a tooltip for every request; and notes on the prices, the compaction model and heartbeats.

#### Scenario: The reference chart
- **WHEN** the bench writes the chart of the run in `docs/bench.txt`
- **THEN** it is titled `Coming back to a big session after its prompt cache expired`, with the subtitle `Mean cost per return over the requests that follow, for cold returns to sessions of 100k+ tokens in local logs`
- **AND** the legend reads `resume as is (as logged)`, `/compact first (replayed)`, `new session + handoff note (replayed)`, `heartbeats, then resume (replayed)` and `heartbeats, then compaction (replayed)`
- **AND** the Claude Code panel is headed `Claude Code · 218 returns, median context 488k tokens`, its request axis is labeled `requests after coming back`, the resume line is marked `1st request $3.96` at request 1, and the lines end at $8.19, $6.33, $6.30, $4.03 and $2.55
- **AND** the notes read `tokprof bench · requests priced at their time · a compaction call is modeled as writing the cold context again plus a 10k-token summary`, `heartbeats: a ping at 5/6 of the cache lifetime, every 50m on a 1h cache, for up to 6h after the last request; a return within the last ping's lifetime reads the cache` and `heartbeats, then compaction: the same pings, then at the next ping time past 6h a compaction while the cache is warm, which reads the context in Claude Code and writes it again in Codex; a later return starts from what it leaves`

#### Scenario: Line styles
- **WHEN** the chart draws the five ways
- **THEN** resume as is is a solid line, /compact first and the handoff note are dashed, heartbeats are dotted and heartbeats then compaction is dash-dotted, in both themes

#### Scenario: Two agents on one scale
- **WHEN** both agents have returns
- **THEN** their panels stand side by side, Claude Code first, on one dollar axis whose step is 1, 2 or 5 times a power of ten, in whole dollars from a $1 step and in cents below it
- **AND** the request axis is marked every N / 6 requests, rounded up

#### Scenario: Hovering a request
- **WHEN** the pointer is over the column of request 10 in the Claude Code panel
- **THEN** a crosshair shows, and the tooltip starts `Claude Code, after 10 requests:` and gives each way's mean cost, such as `resume as is $5.08`
- **AND** the tooltip of request 0 says `before the first request` and gives what each way spends before it

#### Scenario: End values close together
- **WHEN** the costs of two ways after N requests lie closer than a line of text
- **THEN** their labels are kept a line apart, and a label moved off its line gets a leader to its point

#### Scenario: Screen readers
- **WHEN** a screen reader reads the chart
- **THEN** it gets the title and a description with each agent's return count and each way's mean cost after N requests
