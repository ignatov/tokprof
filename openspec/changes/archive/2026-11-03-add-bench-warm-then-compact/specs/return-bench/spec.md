# Spec Delta

## ADDED Requirements

### Requirement: Heartbeats, then compaction
The fifth replay SHALL send the pings of the heartbeat replay, and at the next ping time past `--keep-warm`, while the break lasts, a compaction instead of a ping. The compaction reads the context at the cache-read price of its time and outputs the 10k-token summary. A return before the compaction is priced as a return kept warm. After it, the followed requests run on the base, as /compact first runs them, and the compaction counts before the first request.

#### Scenario: A return inside the window
- **WHEN** the 500k-token `claude-opus-5-5` return on the 1-hour cache comes 119 minutes after the last request, with `--keep-warm 2h`
- **THEN** pings went out at 50 and 100 minutes for $0.204, no compaction went out, and the return is priced warm, as the heartbeat replay prices it

#### Scenario: A return after the compaction
- **WHEN** the same return comes 3 hours after the last request, with `--keep-warm 2h`
- **THEN** the pings at 50 and 100 minutes and a compaction at 150 minutes, (500,000 × 0.20 + 10,000 × 20) / 1,000,000 = $0.30, are spent before the first request
- **AND** the followed requests run on the base, as they do for /compact first

#### Scenario: No window
- **WHEN** the bench runs with `--keep-warm 0`
- **THEN** no ping goes out, and the compaction goes out at the first ping time: 50 minutes after the last request on the 1-hour cache, 4 minutes 10 seconds on the 5-minute one

### Requirement: Breaks that were not cold returns
For heartbeats and for heartbeats then compaction, the bench SHALL also price what they send during breaks that are not counted returns. That covers each break in the main conversations it reads that follows a prompt of at least `--min-context` tokens, starts within `--since` and `--until`, and lasts at least one ping interval. A conversation's last request opens a break up to the latest request the bench read, or `--until` when it is earlier.

#### Scenario: The cache was still warm
- **WHEN** a 300k-token conversation on the 1-hour cache idles for 55 minutes and its next request reads the cache
- **THEN** heartbeats price one ping, at 50 minutes, for that break

#### Scenario: The end of the logs
- **WHEN** a conversation's last request came 10 hours before the latest request the bench read, with the default window
- **THEN** heartbeats price seven pings for that break, the last at 5 hours 50 minutes
- **AND** heartbeats then compaction price the same seven pings and a compaction at 6 hours 40 minutes

#### Scenario: A small context
- **WHEN** a break follows a 60k-token prompt with the default `--min-context`
- **THEN** neither way prices anything for it

#### Scenario: A short break
- **WHEN** a break on the 1-hour cache lasts 40 minutes
- **THEN** neither way prices anything for it, since the first ping would go out at 50 minutes

### Requirement: What heartbeats then compaction would have saved
After the lines on heartbeats the report SHALL say how many returns heartbeats then compaction kept warm and how many it compacted, what its pings and compactions cost, and what it saved or added over all returns.

#### Scenario: The reference run
- **WHEN** the bench runs on the logs behind `docs/bench.txt`
- **THEN** the Claude Code section prints `Heartbeats for up to 6h, then a compaction, would have kept 133 of 218 returns warm and compacted 85, for $116 of pings and compactions, and saved $907 of $1,786 (51%).`

#### Scenario: The line for heartbeats then compaction
- **WHEN** with `--keep-warm 2h` heartbeats then compaction kept 2 of 3 returns warm and compacted 1, for $0.91 of pings and compactions, and saved $9.10 of $11.73
- **THEN** the report prints `Heartbeats for up to 2h, then a compaction, would have kept 2 of 3 returns warm and compacted 1, for $0.91 of pings and compactions, and saved $9.10 of $11.73 (78%).`

### Requirement: What the breaks that were not cold returns add
After the line on heartbeats against the note, and after the line on heartbeats then compaction, the report SHALL say how many more pings, and for the latter compactions, the breaks that were not cold returns would have taken, what they cost, and what the way saves or adds with them.

#### Scenario: The reference run
- **WHEN** the bench runs on the logs behind `docs/bench.txt`
- **THEN** the Claude Code section prints `Breaks that were not cold returns would have taken another 1000 pings for $86.60; with them, heartbeats save $325 (18%).`
- **AND** `Breaks that were not cold returns would have taken another 1000 pings and 133 compactions for $124; with them, it saves $783 (44%).`

#### Scenario: The line for heartbeats
- **WHEN** with `--keep-warm 2h` heartbeats kept 2 of 3 returns warm for $0.61 of pings and saved $7.20 of $11.73, and breaks that were not cold returns took 12 more pings for $1.20
- **THEN** the line on heartbeats against the note is followed by `Breaks that were not cold returns would have taken another 12 pings for $1.20; with them, heartbeats save $6.00 (51%).`

#### Scenario: The line for heartbeats then compaction
- **WHEN** with `--keep-warm 2h` heartbeats then compaction saved $9.10 of $11.73, and breaks that were not cold returns took 12 more pings and 2 compactions for $1.80
- **THEN** its line is followed by `Breaks that were not cold returns would have taken another 12 pings and 2 compactions for $1.80; with them, it saves $7.30 (62%).`

## MODIFIED Requirements

### Requirement: Only the old conversation shrinks
In the /compact and handoff replays, and in heartbeats then compaction after its compaction, each followed request SHALL keep its own split into cache reads, cache writes and uncached tokens while only the old conversation shrinks: the part of its prompt from what the return still read from the cache (at most the base) up to the context size is scaled by (base − that) / (context − that). The tools and system prompt before it and the content added since the return stay as logged.

#### Scenario: The cold request back
- **WHEN** the context is 500k tokens, the return read 10k from the cache and wrote 492k, and the base is 50k, so the old 490k tokens shrink to 40k
- **THEN** the replayed return reads 10k and writes 42k, the 2k of new content included

#### Scenario: A warm request after it
- **WHEN** a later request of that span read 502k tokens and wrote 2k
- **THEN** its replay reads 52k and writes 2k

#### Scenario: A context no bigger than the base
- **WHEN** a return's context is not bigger than the base
- **THEN** its requests are replayed as logged, and /compact first and the handoff note only add what they cost before the first request

#### Scenario: The return read more than the base from the cache
- **WHEN** the return still read more tokens from the cache than the base holds
- **THEN** the whole old conversation beyond the base is left out of every replayed prompt

### Requirement: Per-agent table
For each agent with returns, Claude Code first, the report SHALL give the number of returns, the `--min-context` threshold, the median context and break, the request limit and the median number of requests followed, then a table of the mean cost per return after the 1st request, after the 10th when N is over 10, and after the Nth, and the total over all returns after N, for each of the five ways, each cost including what the way spends before the first request.

#### Scenario: The reference run
- **WHEN** the bench runs on the logs behind `docs/bench.txt`
- **THEN** the Claude Code section starts `claude 218 returns to contexts of 100k+ tokens (median 488.0k) after a break of 3.2h (median);` and `each followed for up to 60 requests, until the next break or compaction (median 23)`
- **AND** its table has the columns `mean per return`, `1st request`, `10 requests`, `60 requests` and `all returns`, and the rows `resume as is`, `/compact first`, `new session + handoff note`, `heartbeats, then resume` and `heartbeats, then compaction`

#### Scenario: What a way spends first is included
- **WHEN** the 500k-token example return is replayed with /compact first
- **THEN** its cost after the 1st request is $4.48: the $4.122 compaction plus $0.358 for the replayed return

#### Scenario: A return followed for fewer than N requests
- **WHEN** a return was followed for 4 requests
- **THEN** it counts with the cost of its 4 requests in the `10 requests`, `60 requests` and `all returns` columns

#### Scenario: An agent with no returns
- **WHEN** one agent has no returns
- **THEN** the report has no section for it, and the notes do not list its base

#### Scenario: No returns at all
- **WHEN** no agent has a return
- **THEN** below its title line the report prints only `No cold returns to contexts of 100k+ tokens in these logs.`

#### Scenario: A short follow
- **WHEN** the bench runs with `--requests 5`
- **THEN** the table has the columns `mean per return`, `1st request`, `5 requests` and `all returns`, with no `10 requests` column

#### Scenario: Following one request
- **WHEN** the bench runs with `--requests 1`
- **THEN** the table has the columns `mean per return`, `1st request` and `all returns`

### Requirement: What the alternatives would have saved
After each agent's table the report SHALL compare the alternatives with resuming after N requests: what a handoff note would have saved over all returns, and per return as a median, in dollars and percent, and in how many returns it was cheaper; from which request on /compact first stays cheaper on average up to the Nth, and what it saves over all; how many returns heartbeats kept warm, what the pings cost and what they saved or added; and in how many returns heartbeats cost less than the note.

#### Scenario: The reference run
- **WHEN** the bench runs on the logs behind `docs/bench.txt`
- **THEN** below the Claude Code table it prints `A handoff note before the break would have saved $1,229 of $1,786 (69%).` and `Per return it saves a median of $4.87 (72%), and it is cheaper in 218 of 218 returns.`
- **AND** `/compact first costs more up front and is cheaper than resuming from request 8 on ($405 over all).`
- **AND** `Heartbeats for up to 6h would have kept 134 of 218 returns warm, for $90.23 of pings, and saved $412 of $1,786 (23%).` and `They cost less than a handoff note in 21 of 218 returns.`

#### Scenario: Heartbeats that cost more than resuming
- **WHEN** heartbeats cost more over all returns than resuming, as for the reference run's Codex returns
- **THEN** the heartbeat line ends `and cost $12.41 more than resuming.`

#### Scenario: /compact first never catches up
- **WHEN** the mean cost of /compact first is not below that of resuming after any request up to N = 60
- **THEN** the report says `/compact first does not pay off within 60 requests.`

#### Scenario: Two separate medians
- **WHEN** the report gives the median saving per return
- **THEN** the median in dollars and the median in percent are taken separately, so they can come from different returns

#### Scenario: /compact first catches up and falls behind again
- **WHEN** the mean cost of /compact first drops below that of resuming at request 8 and is above it again at request N
- **THEN** the report says `/compact first does not pay off within <N> requests.`

### Requirement: The base the alternatives start from
The /compact and handoff replays, and heartbeats then compaction after its compaction, SHALL start from a base per agent: the median prompt of the first request after a compaction of a context of at least `--min-context` tokens, over all main conversations read, whatever the date filters. An agent without such a compaction uses the median first prompt of its main conversations plus the estimated 10k-token note.

#### Scenario: A base measured on the logs
- **WHEN** the logs behind `docs/bench.txt` hold 31 Claude Code and 791 Codex compactions of contexts of 100k+ tokens
- **THEN** the bases are the medians of the prompts right after them, 52.5k and 20.8k tokens

#### Scenario: An agent without compactions
- **WHEN** an agent has returns but no compaction of a big enough context, and the median first prompt of its main conversations is 35k tokens
- **THEN** its base is 45k tokens, and the report calls it `(first prompt + note)`

#### Scenario: The threshold also picks the compactions
- **WHEN** a compaction follows a 60k-token prompt
- **THEN** it does not count toward the base with the default `--min-context`, and does with `--min-context 50000`

#### Scenario: An even number of values
- **WHEN** four compactions leave prompts of 20k, 30k, 40k and 50k tokens
- **THEN** the base is 40k: the bench's medians take the upper of the two middle values

#### Scenario: Date filters
- **WHEN** the bench runs with `--since 7d`
- **THEN** the base still comes from the compactions of every date in the sessions read

### Requirement: Forecast check
For each agent the report SHALL say for how many returns, and for what share, the session-xray capability's forecast of the next request on a cold cache, made from the conversation before the break and priced at the return, came within 10% of the logged cost of the first request back.

#### Scenario: The reference run
- **WHEN** the bench runs on the logs behind `docs/bench.txt`
- **THEN** it prints `The X-ray's forecast of the first request was within 10% of its logged cost for 199 of 218 returns (91%).` for Claude Code

#### Scenario: A forecast that misses
- **WHEN** the requests before the break of the 500k-token example added 250k tokens and output 1k on average, and the first request back cost $3.958
- **THEN** the forecast is ((500,000 + 250,000) × 8 + 1,000 × 20) / 1,000,000 = $6.02, and the return does not count as within 10%

### Requirement: The report states its assumptions
The report SHALL end with notes on its assumptions: the base of each agent with returns, as a median of so many compactions or as a first prompt plus a note; that a compaction re-writes the cold context and outputs 10k tokens and a handoff note is 10k tokens written while the cache was warm; how often a heartbeat goes out, what it reads and outputs, and what it assumes for Codex; and when heartbeats then compaction compacts.

#### Scenario: The reference run
- **WHEN** the bench runs on the logs behind `docs/bench.txt`
- **THEN** the report ends with `The alternatives start from what a compaction leaves: claude 52.5k (median of 31 compactions), codex 20.8k (median of 791 compactions).`
- **AND** `A compaction re-writes the cold context and outputs 10.0k tokens; a handoff note is 10.0k tokens written while the cache was warm.`
- **AND** `A heartbeat goes out at 5/6 of the cache lifetime, every 50m on a 1h cache, reads the context and outputs 100 tokens; for Codex this assumes it resets the cache timer as it does in Claude Code.`
- **AND** `Heartbeats, then compaction sends the same pings and, at the next ping time past the window, a compaction that reads the warm context and outputs 10.0k tokens.`

### Requirement: Chart labels
The chart SHALL say what every number and mark is: a title and subtitle naming the measure and the returns it covers; a legend naming each way, as logged or replayed, drawn like its line; for each agent with returns, a panel headed with its return count and median context; a dollar axis and a request axis; the cost of the first request; each way's cost after N requests; a tooltip for every request; and notes on the prices, the compaction model and heartbeats.

#### Scenario: The reference chart
- **WHEN** the bench writes the chart of the run in `docs/bench.txt`
- **THEN** it is titled `Coming back to a big session after its prompt cache expired`, with the subtitle `Mean cost per return over the requests that follow, for cold returns to sessions of 100k+ tokens in local logs`
- **AND** the legend reads `resume as is (as logged)`, `/compact first (replayed)`, `new session + handoff note (replayed)`, `heartbeats, then resume (replayed)` and `heartbeats, then compaction (replayed)`
- **AND** the Claude Code panel is headed `Claude Code · 218 returns, median context 488k tokens`, its request axis is labeled `requests after coming back`, the resume line is marked `1st request $3.96` at request 1, and the lines end at $8.19, $6.33, $6.30, $4.03 and $2.55
- **AND** the notes read `tokprof bench · requests priced at their time · the compaction call is not logged: modeled as writing the cold context again plus a 10k-token summary`, `heartbeats: a ping at 5/6 of the cache lifetime, every 50m on a 1h cache, for up to 6h after the last request; a return within the last ping's lifetime reads the cache` and `heartbeats, then compaction: the same pings, then at the next ping time past 6h a compaction that reads the warm context; a later return starts from what it leaves`

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
