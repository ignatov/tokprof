# Spec Delta

## Purpose
Sums one month of the day store into a card someone would post: dollars, hours, the cache and one thing to change, with no project in it.

## ADDED Requirements

### Requirement: The month's window
A month in review SHALL count what the day store (see activity-calendar) holds from local midnight on the month's first day to local midnight on the next month's first, or until now for this month, with the `--agent`, `--project` and `--session` filters. Its days are the calendar's cells of the month's dates, with the calendar's day rule.

#### Scenario: The same spend as the summary
- **WHEN** the user runs `tokprof month 2026-10` and `tokprof summary --since 2026-10-01 --until 2026-11-01` on logs that were not deleted
- **THEN** the month's spend equals `Total priced`, and its avoidable dollars equal `avoidable on cache misses`

#### Scenario: A night across the first
- **WHEN** agents worked from 00:00 to 04:00 on 1 October, inside the night of 30 September's day
- **THEN** that spend counts in October's spend, while it lies in the calendar's cell of 30 September, so it is not in October's days

### Requirement: More in the day store
Each minute of an entry SHALL also keep:
- the prompt tokens read from the cache, and all prompt tokens;
- the dollars the cache reads saved: the tokens read times the uncached input price less the cache read price;
- the requests without a price.

Each entry also keeps, per hour of UTC, the avoidable premium of each miss cause that is not expected and the spend of each model, and the times of its cold returns (see pause-report). This is format 2, `days-2.json`.

#### Scenario: A read without a price
- **WHEN** a request's model has no price
- **THEN** its cache reads save nothing in dollars, and it counts among the requests without a price

#### Scenario: The saving of a read
- **WHEN** a request reads 100,000 tokens from the cache at $5 per million uncached and $0.50 per million read
- **THEN** it saved $0.45

### Requirement: The month's numbers
The month SHALL sum spend, agent hours, hours at the keyboard, messages and the agents' commits over the calendar's days of its window, and count the main sessions with a request. It names the busiest day by spend, the local hour with the most agent time with its share, the share of agent time between 22:00 and 05:00, and the longest run of days with a request.

#### Scenario: The busiest hour
- **WHEN** 7% of October's agent time fell between 23:00 and 00:00, more than in any other hour
- **THEN** the card reads `23:00–00:00, 7% of agent time`

### Requirement: The month's cache and model
The month SHALL give the share of prompt tokens read from the cache, the dollars saved by the cache, the avoidable dollars with their share of spend, the cold returns, and the model with the most spend with its share of the priced spend.

#### Scenario: Requests without a price
- **WHEN** 1,947 requests of the month have no price
- **THEN** the card says `1,947 requests without a price are not counted in dollars`

### Requirement: The month before
The month SHALL compare its spend, agent hours and avoidable share with the month before, as an arrow and the difference, with the relative change for spend and agent hours. It shows no comparison when the month before has no request and no message.

#### Scenario: More spend
- **WHEN** October spent $11,632 and September $3,729
- **THEN** the comparison reads `spend ↑ $7,903 (+212%)`

### Requirement: Titles
The month SHALL take the first title whose rule holds, in this order: `Night owl` (33% or more of agent time between 22:00 and 05:00), `Early bird` (20% or more of the time at the keyboard between 05:00 and 09:00), `Conductor` (25% or more of agent time with 3 or more sessions working at once), `Marathon` (28 or more days in a row with a request), `Cache keeper` (under 5% of spend avoidable), `Shipper` (0.75 or more commits per agent hour). Else it has none.

#### Scenario: No rule holds
- **WHEN** no rule of the six holds for a month
- **THEN** the card has no title, and `tokprof month` prints `no title this month: no rule of the six held`

#### Scenario: The rule under the title
- **WHEN** 34% of a month's agent time fell between 22:00 and 05:00
- **THEN** its title is `Night owl`, and the rule under it, with the month's number, reads `34% of agent time 22:00–05:00 (title from 33%)`

#### Scenario: Two rules
- **WHEN** a month had a 31-day streak and 0.8 commits per agent hour
- **THEN** its title is `Marathon`

### Requirement: The line for next month
The month SHALL end with the miss cause that lost the most avoidable dollars among those tokprof has advice for, every cause but the unknown one, as `$873 went to idle > TTL. Compact or wrap up a big session before a long break.`, where the sentence is tokprof's advice for that cause (see cache-misses). The line is left out when no miss was avoidable.

#### Scenario: An unknown cause first
- **WHEN** misses of unknown cause lost $475 in August and compaction calls $120
- **THEN** the line names compaction calls and their $120

#### Scenario: Compaction calls
- **WHEN** compaction calls lost $347 in September, more than any other cause
- **THEN** the line reads `$347 went to compaction call. Each call wrote the whole context again; hand off to a new session before the context fills.`

### Requirement: Nothing private
A month in review SHALL name no project, session id, session title, path or user name. It names models, miss causes, dates and hours.

#### Scenario: A project filter
- **WHEN** the user runs `tokprof --project billing month`
- **THEN** the card counts only that project's sessions, and the word `billing` does not appear on it

### Requirement: tokprof month
`tokprof month [YYYY-MM]` SHALL print the month in review to the terminal, the last complete month by default and this month so far with `this`. It refreshes the day store as `tokprof calendar` does and prints the same line on what it read. `--since` and `--until` fail with `month picks its time by YYYY-MM; drop --since and --until`, and `--client` fails with `month cannot filter by client; drop --client`.

#### Scenario: The text
- **WHEN** the user runs `tokprof month 2026-10`
- **THEN** it prints `tokprof · month · October 2026 with agents`, the title and its rule, the six big numbers, the days of the month as a calendar of `·░▒▓█` marks by spend, the facts, the cache, the comparison with September, and the line for next month

#### Scenario: This month
- **WHEN** the user runs `tokprof month this` on 9 November
- **THEN** the headline reads `November 2026 with agents, so far`

#### Scenario: A bad month
- **WHEN** the user runs `tokprof month october`
- **THEN** it fails with `bad month "october", expected YYYY-MM or this`

#### Scenario: A month with nothing
- **WHEN** the store holds nothing in March 2026
- **THEN** it prints `No requests and no messages in this month.` under the headline
