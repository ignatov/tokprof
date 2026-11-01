# Return Bench Specification

## Purpose
`tokprof bench` measures on the local logs what coming back to a big session after its prompt cache expired cost, and replays the same work as `/compact` first, as a new session started from a handoff note, and with heartbeats that keep the cache warm, so that the four ways can be compared in dollars per return.

## Requirements

### Requirement: Returns come from main conversations
`tokprof bench` SHALL look for returns only in the main conversations of main session logs, as the session-logs capability finds, reads and joins them, after the global `--agent`, `--project` and `--session` filters. Subagent logs are not read.

#### Scenario: Subagent logs are left out
- **WHEN** a Claude Code transcript lies under a `subagents/` directory, or a Codex rollout names a parent thread
- **THEN** the bench does not read it
- **AND** its requests neither make returns nor count toward the base

#### Scenario: A session with two main chains of one name
- **WHEN** a Claude Code session spans the release that added request-audit lines, so its main conversation is in two chains of one name, the first ending before the second starts, and a 2-hour break falls between them
- **THEN** the chains are one conversation, and the first request of the second chain is a return whose context is the last prompt of the first chain

#### Scenario: Two main chains of one name that overlap in time
- **WHEN** two main chains of one name overlap in time
- **THEN** they are separate conversations, each with its own returns and its own first prompt

#### Scenario: A log that cannot be read
- **WHEN** one log fails to parse
- **THEN** the bench leaves it out, goes on with the others, and prints a warning as the other reports do (session-logs)

#### Scenario: No logs at all
- **WHEN** no session log is found, for example because `--session` matches none
- **THEN** the bench fails with `no session logs found for <agents>`, as the other reports do

### Requirement: What counts as a cold return
A request SHALL count as a return when it comes more than one cache lifetime after the previous request of its conversation, misses the cache on that request's prompt by the cache-misses rule, uses the same model, does not follow a compaction, is sent within `--since` and `--until`, and that previous prompt, the return's context, has at least `--min-context` tokens (default 100000). Requests without prompt tokens and advisor calls are skipped.

#### Scenario: A cold return
- **WHEN** a Claude Code conversation on the 1-hour cache sends a 500k-token prompt, idles for 119 minutes, and its next request reads 10k tokens from the cache and writes 492k
- **THEN** that request is a return with a context of 500k tokens after a break of 119 minutes

#### Scenario: The cache was still warm
- **WHEN** a request comes 2 hours after the previous one but reads all but 1,500 tokens of the previous prompt from the cache
- **THEN** it is not a return, because it re-processed fewer than 2,000 tokens of that prompt

#### Scenario: Another model after the break
- **WHEN** the first request after a long break uses another model than the request before it
- **THEN** it is not a return

#### Scenario: A context below the threshold
- **WHEN** the prompt before a cold break has 80k tokens
- **THEN** the break is not a return with the default `--min-context`, and is one with `--min-context 50000`

#### Scenario: The first request after a compaction
- **WHEN** the first request after a long break comes right after a compaction
- **THEN** it is not a return

#### Scenario: A response that consulted the advisor
- **WHEN** the last response before a break includes advisor calls, which read the conversation with another model
- **THEN** the advisor calls are skipped, and the context and the break are measured from the last request before the break that is not an advisor call

#### Scenario: Date filters select returns by their own time
- **WHEN** the bench runs with `--since 2026-10-01 --until 2026-10-15`
- **THEN** only returns sent from 2026-10-01 up to but not including 2026-10-15 count
- **AND** the requests after such a return are followed even when they are sent on 2026-10-15 or later

### Requirement: Assumed cache lifetimes
The bench SHALL assume that the prompt cache lives 1 hour in a Claude Code conversation whose latest request with a cache write wrote more tokens with the long lifetime than with the short one, 5 minutes in any other Claude Code conversation, and 30 minutes in Codex, the OpenAI default for current models.

#### Scenario: The 1-hour Claude Code cache
- **WHEN** the latest cache write before a break wrote mostly 1-hour entries
- **THEN** a break of exactly 60 minutes is not a cold return, and a 61-minute break with a cache miss is

#### Scenario: The 5-minute Claude Code cache
- **WHEN** the latest cache write before a break wrote mostly 5-minute entries, or the conversation has written nothing to the cache yet
- **THEN** a 6-minute break with a cache miss is a cold return

#### Scenario: Codex
- **WHEN** a Codex conversation idles for 31 minutes and then misses the cache
- **THEN** it is a cold return, while a 29-minute break is not

### Requirement: Following a return
The bench SHALL follow each return for up to `--requests` requests (default 60, at least 1), the return being the first, and stop before the first later request that comes after a compaction or after a cold break: more than one cache lifetime after the request before it, with a cache miss, the lifetime being that of the latest cache write so far.

#### Scenario: A long warm run
- **WHEN** a return is followed by 80 requests on a warm cache
- **THEN** the return and the next 59 requests are followed, 60 in all

#### Scenario: A compaction inside the span
- **WHEN** a return is followed by three warm requests and then a compaction
- **THEN** the return is followed for 4 requests, and the request after the compaction is neither followed nor a return itself

#### Scenario: The next break
- **WHEN** a second cold break comes 24 requests after a return
- **THEN** the first return is followed for 24 requests
- **AND** the request after the second break is a return of its own when its context, model and time qualify, and belongs to no span otherwise

#### Scenario: A pause the cache survived
- **WHEN** a request in the span comes more than one cache lifetime after the one before it but still reads that prompt from the cache
- **THEN** the span goes on through it

#### Scenario: No requests to follow
- **WHEN** the bench runs with `--requests 0`
- **THEN** it fails with a usage error before reading any log

### Requirement: Costs priced at their time
The bench SHALL price every cost through the pricing capability at the time it falls: a followed request at its own time and with the prompt size it has in the replay, a compaction call at the return, a handoff note at the last request before the break, and a ping at its own time. Resuming as is costs the followed requests as logged.

#### Scenario: Resume as is
- **WHEN** a return on `claude-opus-5-5` (read $0.20, 1-hour write $8, output $20 per million tokens) reads 10k tokens from the cache, writes 492k with the 1-hour lifetime and outputs 1k
- **THEN** resuming as is pays (10,000 × 0.20 + 492,000 × 8 + 1,000 × 20) / 1,000,000 = $3.958 for that first request

#### Scenario: A replayed prompt under a long-context threshold
- **WHEN** a 300k-token request to a model with long-context rates above 272k tokens is replayed on a 60k-token prompt
- **THEN** the replayed request is priced at the normal rates

#### Scenario: No price at the return
- **WHEN** the model of a return has no price at the time of the return
- **THEN** the return is left out of the bench

#### Scenario: A followed request without a price
- **WHEN** a request in a span has no price at its time
- **THEN** it adds nothing to any of the four ways

#### Scenario: No price before the break
- **WHEN** the model had no price at the last request before the break but has one at the return
- **THEN** the handoff note is priced at the return's prices

### Requirement: The base the alternatives start from
The /compact and handoff replays SHALL start from a base per agent: the median prompt of the first request after a compaction of a context of at least `--min-context` tokens, over all main conversations read, whatever the date filters. An agent without such a compaction uses the median first prompt of its main conversations plus the estimated 10k-token note.

#### Scenario: A base measured on the logs
- **WHEN** the logs behind `docs/bench.txt` hold 32 Claude Code and 791 Codex compactions of contexts of 100k+ tokens
- **THEN** the bases are the medians of the prompts right after them, 52.8k and 20.8k tokens

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

### Requirement: Only the old conversation shrinks
In the /compact and handoff replays each followed request SHALL keep its own split into cache reads, cache writes and uncached tokens while only the old conversation shrinks: the part of its prompt from what the return still read from the cache (at most the base) up to the context size is scaled by (base − that) / (context − that). The tools and system prompt before it and the content added since the return stay as logged.

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

### Requirement: /compact first
The /compact replay SHALL add, before the first request back, the compaction call that Claude Code does not log, modeled as a fork that shares the conversation's cache: on the cold cache it reads what the return still read from the cache (at most the base), writes the rest of the context at the write price of the cache lifetime in use, and outputs a summary of an estimated 10k tokens.

#### Scenario: A 500k-token context on the 1-hour cache
- **WHEN** a `claude-opus-5-5` return has a 500k-token context on the 1-hour cache and read 10k tokens from the cache
- **THEN** the compaction costs (10,000 × 0.20 + 490,000 × 8 + 10,000 × 20) / 1,000,000 = $4.122 before the first request

#### Scenario: Codex
- **WHEN** a Codex return is replayed with /compact first
- **THEN** the compaction is modeled the same way and writes the context at the model's short-lifetime write price

#### Scenario: Too few requests to pay off
- **WHEN** that 500k-token return with a 50k base is followed for 4 requests
- **THEN** after them resuming as is costs $4.37, /compact first $4.62, the handoff note $0.80 and heartbeats $0.75

### Requirement: New session with a handoff note
The handoff replay SHALL add, before the first request back, a note of an estimated 10k tokens that the old session writes before the break while its cache is warm: it reads the whole context at the cache-read price and outputs the note. A new session then runs the followed requests on the same base as /compact first, so the two ways differ only in what they cost before the first request.

#### Scenario: A 500k-token context
- **WHEN** a `claude-opus-5-5` return has a 500k-token context
- **THEN** the note costs (500,000 × 0.20 + 10,000 × 20) / 1,000,000 = $0.30 before the first request

#### Scenario: The same requests as /compact first
- **WHEN** a return is replayed both ways
- **THEN** after any number of requests the two differ by the difference between the note and the compaction call

### Requirement: Heartbeats
The heartbeat replay SHALL send a ping every 5/6 of the cache lifetime after the last request before the break, while the break lasts and up to `--keep-warm` after that request (default 6h; a number of hours with or without `h`, or of minutes with `m`). Each ping reads the context at the cache-read price of its time and outputs an assumed 100-token reply.

#### Scenario: How often
- **WHEN** the cache lives 1 hour, 5 minutes, or 30 minutes in Codex
- **THEN** pings go out every 50 minutes, every 4 minutes 10 seconds, or every 25 minutes
- **AND** for Codex this assumes that a ping resets OpenAI's cache timer as it resets Anthropic's

#### Scenario: A 119-minute break on the 1-hour cache
- **WHEN** a `claude-opus-5-5` conversation with a 500k-token context idles for 119 minutes
- **THEN** two pings go out, at 50 and 100 minutes, each costing (500,000 × 0.20 + 100 × 20) / 1,000,000 = $0.102

#### Scenario: A break longer than the window
- **WHEN** a conversation on the 1-hour cache idles for 10 hours with the default window
- **THEN** seven pings go out, the last at 5 hours 50 minutes, and the cache expires before the return

#### Scenario: No window
- **WHEN** the bench runs with `--keep-warm 0`
- **THEN** no pings go out, and heartbeats cost the same as resuming

#### Scenario: Window formats
- **WHEN** the bench runs with `--keep-warm 90m` or `--keep-warm 1.5h`
- **THEN** pings go out for up to 90 minutes, and the report calls the window `90m`

#### Scenario: An invalid window
- **WHEN** the bench runs with `--keep-warm 30s`
- **THEN** it fails with `bad duration "30s", expected 6h or 90m`

#### Scenario: A window that is not whole minutes
- **WHEN** the bench runs with `--keep-warm 0.5m` or `--keep-warm 90.5m`
- **THEN** the report calls the window `0.5m` or `90.5m`, not `0h` or `90m`

### Requirement: A return kept warm
When a return comes within one cache lifetime of the last ping, the heartbeat replay SHALL price its first request as on a warm cache: the previous prompt, up to the request's own prompt size, is read from the cache, and only what is new stays written or uncached; the requests after it run as logged. A return the pings did not keep warm runs as logged, after the pings.

#### Scenario: The 119-minute break
- **WHEN** the 500k-token return that read 10k tokens, wrote 492k and output 1k comes 19 minutes after the last ping
- **THEN** its first request reads 500k and writes 2k, (500,000 × 0.20 + 2,000 × 8 + 1,000 × 20) / 1,000,000 = $0.136, on top of $0.204 of pings

#### Scenario: A first request smaller than the old context
- **WHEN** the first request back has a smaller prompt than the old context
- **THEN** all of its prompt is read from the cache

#### Scenario: The pings ran out
- **WHEN** a break on the 1-hour cache lasts 7 hours with the default window, more than an hour past the last ping at 5 hours 50 minutes
- **THEN** the return stays cold, and heartbeats cost the seven pings plus the requests as logged, more than resuming

### Requirement: Per-agent table
For each agent with returns, Claude Code first, the report SHALL give the number of returns, the `--min-context` threshold, the median context and break, the request limit and the median number of requests followed, then a table of the mean cost per return after the 1st request, after the 10th when N is over 10, and after the Nth, and the total over all returns after N, for each of the four ways, each cost including what the way spends before the first request.

#### Scenario: The reference run
- **WHEN** the bench runs on the logs behind `docs/bench.txt`
- **THEN** the Claude Code section starts `claude 210 returns to contexts of 100k+ tokens (median 488.4k) after a break of 3.2h (median);` and `each followed for up to 60 requests, until the next break or compaction (median 24)`
- **AND** its table has the columns `mean per return`, `1st request`, `10 requests`, `60 requests` and `all returns`, and the rows `resume as is`, `/compact first`, `new session + handoff note` and `heartbeats, then resume`

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
- **THEN** below the Claude Code table it prints `A handoff note before the break would have saved $1,195 of $1,737 (69%).` and `Per return it saves a median of $5.09 (72%), and it is cheaper in 210 of 210 returns.`
- **AND** `/compact first costs more up front and is cheaper than resuming from request 8 on ($398 over all).`
- **AND** `Heartbeats for up to 6h would have kept 129 of 210 returns warm, for $87.79 of pings, and saved $398 of $1,737 (23%).` and `They cost less than a handoff note in 20 of 210 returns.`

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

### Requirement: Forecast check
For each agent the report SHALL say for how many returns, and for what share, the session-xray capability's forecast of the next request on a cold cache, made from the conversation before the break and priced at the return, came within 10% of the logged cost of the first request back.

#### Scenario: The reference run
- **WHEN** the bench runs on the logs behind `docs/bench.txt`
- **THEN** it prints `The X-ray's forecast of the first request was within 10% of its logged cost for 191 of 210 returns (91%).` for Claude Code

#### Scenario: A forecast that misses
- **WHEN** the requests before the break of the 500k-token example added 250k tokens and output 1k on average, and the first request back cost $3.958
- **THEN** the forecast is ((500,000 + 250,000) × 8 + 1,000 × 20) / 1,000,000 = $6.02, and the return does not count as within 10%

### Requirement: The report states its assumptions
The report SHALL end with notes on its assumptions: the base of each agent with returns, as a median of so many compactions or as a first prompt plus a note; that a compaction re-writes the cold context and outputs 10k tokens and a handoff note is 10k tokens written while the cache was warm; and how often a heartbeat goes out, what it reads and outputs, and what it assumes for Codex.

#### Scenario: The reference run
- **WHEN** the bench runs on the logs behind `docs/bench.txt`
- **THEN** the report ends with `The alternatives start from what a compaction leaves: claude 52.8k (median of 32 compactions), codex 20.8k (median of 791 compactions).`
- **AND** `A compaction re-writes the cold context and outputs 10.0k tokens; a handoff note is 10.0k tokens written while the cache was warm.`
- **AND** `A heartbeat goes out at 5/6 of the cache lifetime, every 50m on a 1h cache, reads the context and outputs 100 tokens; for Codex this assumes it resets the cache timer as it does in Claude Code.`

### Requirement: Chart files
With `--svg FILE` the bench SHALL write the chart of the mean cost per return in a light theme to FILE and in a dark theme next to it, named after FILE's stem with `-dark.svg`, creating missing directories, and print `wrote <path>` for each file after the report, the light one first, with the home directory shown as `~`.

#### Scenario: The reference chart
- **WHEN** the bench runs with `--svg docs/bench.svg`
- **THEN** it writes `docs/bench.svg` and `docs/bench-dark.svg`, and prints `wrote docs/bench.svg` and `wrote docs/bench-dark.svg`

#### Scenario: A new directory under the home directory
- **WHEN** `--svg` names a file in a directory under the home directory that does not exist yet
- **THEN** the bench creates the directory, and the `wrote` lines show the paths starting with `~/`

#### Scenario: Another extension
- **WHEN** the bench runs with `--svg chart.png`
- **THEN** the light SVG chart is written to `chart.png` and the dark one to `chart-dark.svg`

#### Scenario: No returns
- **WHEN** there are no returns
- **THEN** both files are still written, with the title, the legend and the notes but no agent panel

#### Scenario: No file name
- **WHEN** the path given to `--svg` has no file name, such as `/`
- **THEN** the command fails with `the chart needs a file name` after printing the report

### Requirement: Chart labels
The chart SHALL say what every number and mark is: a title and subtitle naming the measure and the returns it covers; a legend naming each way, as logged or replayed, drawn like its line; for each agent with returns, a panel headed with its return count and median context; a dollar axis and a request axis; the cost of the first request; each way's cost after N requests; a tooltip for every request; and notes on the prices, the compaction model and heartbeats.

#### Scenario: The reference chart
- **WHEN** the bench writes the chart of the run in `docs/bench.txt`
- **THEN** it is titled `Coming back to a big session after its prompt cache expired`, with the subtitle `Mean cost per return over the requests that follow, for cold returns to sessions of 100k+ tokens in local logs`
- **AND** the legend reads `resume as is (as logged)`, `/compact first (replayed)`, `new session + handoff note (replayed)` and `heartbeats, then resume (replayed)`
- **AND** the Claude Code panel is headed `Claude Code · 210 returns, median context 488k tokens`, its request axis is labeled `requests after coming back`, the resume line is marked `1st request $3.98` at request 1, and the lines end at $8.27, $6.38, $6.37 and $2.58
- **AND** the notes read `tokprof bench · requests priced at their time · the compaction call is not logged: modeled as writing the cold context again plus a 10k-token summary` and `heartbeats: a ping at 5/6 of the cache lifetime, every 50m on a 1h cache, for up to 6h after the last request; a return within the last ping's lifetime reads the cache`

#### Scenario: Line styles
- **WHEN** the chart draws the four ways
- **THEN** resume as is is a solid line, /compact first and the handoff note are dashed and heartbeats are dotted, in both themes

#### Scenario: Two agents on one scale
- **WHEN** both agents have returns
- **THEN** their panels stand side by side, Claude Code first, on one dollar axis whose step is 1, 2 or 5 times a power of ten, in whole dollars from a $1 step and in cents below it
- **AND** the request axis is marked every N / 6 requests, rounded up

#### Scenario: Hovering a request
- **WHEN** the pointer is over the column of request 10 in the Claude Code panel
- **THEN** a crosshair shows, and the tooltip starts `Claude Code, after 10 requests:` and gives each way's mean cost, such as `resume as is $5.11`
- **AND** the tooltip of request 0 says `before the first request` and gives what each way spends before it

#### Scenario: End values close together
- **WHEN** the costs of two ways after N requests lie closer than a line of text
- **THEN** their labels are kept a line apart, and a label moved off its line gets a leader to its point

#### Scenario: Screen readers
- **WHEN** a screen reader reads the chart
- **THEN** it gets the title and a description with each agent's return count and each way's mean cost after N requests
