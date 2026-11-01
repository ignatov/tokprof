## MODIFIED Requirements

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
