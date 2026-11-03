# pause-report Specification

## Purpose
Shows how long the user pauses before typing again and what those pauses cost through the prompt cache, so that a cache lifetime or an idle policy can be chosen from the user's own logs.

## Requirements

### Requirement: Pause report
`tokprof pauses` SHALL report on the requests that the global filters select (see session-logs), one section per agent with requests, Claude Code first. Each section has these parts, in order: pauses before typed messages; the share after each cache lifetime; cold returns by pause; who started them; what followed them; and sessions by largest context.

#### Scenario: Both agents
- **WHEN** the selected requests come from Claude Code and Codex
- **THEN** the report has a `claude` section followed by a `codex` section, each with all six parts

#### Scenario: No typed message with a pause
- **WHEN** an agent's main conversations hold no typed message after an earlier request
- **THEN** its pause table and lifetime line are replaced by `No typed messages after an earlier request.`, and the other parts are printed as usual

### Requirement: Pauses before typed messages
A typed message's pause SHALL be the time from the previous request of its main conversation to the request the message triggered (trigger `prompt`, see cost-attribution). Same-name chains are joined as session-logs joins them. The first message of a conversation has no pause. The table has one row per range: `under 1m`, `1–5m`, `5–15m`, `15–30m`, `30–60m`, `1–2h`, `2–4h`, `4–8h`, `8–24h` and `over 1d`, each with `messages`, `share` and `median context`.

#### Scenario: A message seven minutes later
- **WHEN** a typed message's request comes 7 minutes after the previous request of its conversation
- **THEN** it counts in the `5–15m` row

#### Scenario: A pause on a boundary
- **WHEN** the pause is exactly 5 minutes
- **THEN** it counts in `5–15m`, since each range includes its lower bound and excludes its upper one

#### Scenario: The first message
- **WHEN** a typed message opens its conversation
- **THEN** it counts in no row

#### Scenario: Median context
- **WHEN** the `1–2h` row holds three messages whose requests had prompts of 300k, 420k and 600k tokens
- **THEN** its median context reads `420.0k`

### Requirement: Share after each cache lifetime
Below the pause table, the report SHALL print the share of those typed messages whose pause is longer than each cache lifetime the agent can use. For Claude Code these are 5 minutes and 1 hour; for Codex, 30 minutes, as cache-misses assumes. Shares are whole percents.

#### Scenario: Claude Code
- **WHEN** 476 of 961 typed messages came more than 5 minutes after the previous request, and 168 more than an hour after it
- **THEN** the line reads `On a 5-minute cache, 50% of these messages come after it expired; on a 1-hour cache, 17%.`

#### Scenario: Codex
- **WHEN** the section is Codex's and 40 of 400 typed messages came more than 30 minutes after the previous request
- **THEN** the line reads `On a 30-minute cache, 10% of these messages come after it expired.`

### Requirement: Cold returns by pause
A cold return SHALL be a request of a main conversation whose miss has the cause `idle > TTL` (see cache-misses). The table has one row per range of the pause before it: `under 1h`, `1–2h`, `2–4h`, `4–8h`, `8–16h`, `16–48h` and `over 2d`. Each row has `returns`, `first request` (the return's own cost), `avoidable` (its premium, see cache-misses) and `median context` (the previous prompt). Empty ranges are left out.

#### Scenario: A return after the night
- **WHEN** a request misses the cache 9.5 hours after the previous one, with a 1-hour lifetime and a previous prompt of 878k tokens
- **THEN** it counts in the `8–16h` row, with its own cost and premium

#### Scenario: The total line
- **WHEN** an agent has 206 cold returns whose first requests cost $777, $740 of it avoidable
- **THEN** the table is followed by `206 cold returns: $777 in first requests, $740 of it avoidable.`

#### Scenario: Subagents
- **WHEN** a subagent's request misses the cache after a pause longer than its lifetime
- **THEN** it is not a cold return in this report

#### Scenario: No cold returns
- **WHEN** an agent's main conversations have no `idle > TTL` miss
- **THEN** the table, its total line and the lines on who started the returns and what followed them are replaced by `No cold returns.`

### Requirement: Who started the cold returns
The report SHALL sort the cold returns by the trigger of their request (see cost-attribution):
- `prompt` is a typed message;
- `notification` is a background task;
- `lookup`, `lookups`, `edit`, `run` and `subagent` are a tool result;
- every other trigger is other.

#### Scenario: The study's returns
- **WHEN** of 206 cold returns 166 have the trigger `prompt`, 12 `notification`, 11 `run`, 1 `lookup` and 16 `unknown`
- **THEN** the line reads `Started by: 166 typed messages, 12 background tasks, 12 tool results, 16 other.`

### Requirement: What followed a cold return
For each cold return, the report SHALL count the requests of its conversation from the return up to the first later request that comes after a compaction, or more than one cache lifetime after the request before it. It prints the median count, the counts at one and three quarters of the sorted list, and how many returns were followed by at most 3 requests. A median takes the upper of two middle values; each quarter position is rounded down.

#### Scenario: The line
- **WHEN** the counts have a median of 20, quarters at 7 and 70, and 17 counts of 3 or fewer
- **THEN** the line reads `After a cold return: a median of 20 requests until the next cold pause or compaction (quarters 7 and 70); 17 returns ended within 3 requests.`

#### Scenario: A compaction right after the return
- **WHEN** the request after a return follows a compaction
- **THEN** that return counts 1 request

### Requirement: Sessions by largest context
Each main session with a selected request SHALL fall in the range of the largest prompt of its main conversations: `under 100k`, `100–200k`, `200–400k`, `400–800k` or `800k+`. Each row has `sessions`, `spend` (the session's selected requests, subagents included), `share` of the agent's spend and `cold returns`. Empty ranges are left out.

#### Scenario: A session that filled its window
- **WHEN** a session's largest main prompt was 966k tokens, its requests cost $412 with subagents, and it had 14 cold returns
- **THEN** it counts in the `800k+` row with $412 and 14 cold returns

#### Scenario: Shares
- **WHEN** an agent's selected requests cost $8,534 and the `800k+` sessions $5,161 of it
- **THEN** that row's share reads `60.5%`

### Requirement: Rhythm section
After the agent sections, `tokprof pauses` SHALL print one rhythm section for all agents together, over the selected time and sessions: the title `Your rhythm: when you sat at the agents and what they did meanwhile`, a `nights` line with the median start of a day, the `left out` line (see work-rhythm), the five rhythm lines of work-rhythm, and a table by hour of the day.

#### Scenario: The section
- **WHEN** `tokprof --since 30d pauses` runs
- **THEN** the rhythm section follows the last agent section, and its five lines count the actions and turns of the last 30 days

#### Scenario: Nights
- **WHEN** the days of the selected time started, each in the middle of the night before it (see work-rhythm), at a median of 05:14
- **THEN** the line reads `nights    a day runs from the middle of one night to the middle of the next; days started at a median 05:14`

#### Scenario: Hours
- **WHEN** days started at a median 05:14
- **THEN** the hour table has the columns `hour`, `messages`, `turn ends` and `median answer`, and a row for each hour from 05 to 04 with a message or an end of turn in it, in that order
- **AND** an answer counts in the hour of the message, and an hour without answers shows `–`

#### Scenario: No messages
- **WHEN** the selected sessions hold no action of the user
- **THEN** the section is the title and `No messages in this time.`

