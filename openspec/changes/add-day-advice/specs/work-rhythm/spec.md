# Spec Delta

## ADDED Requirements

### Requirement: Rhythm advice
The day view and the pause report's rhythm section SHALL show an `advice` block with each item below whose rule holds over the time they show, in this order: cold returns, questions, out of work, step by step. The block is laid out as the X-ray's advice block, with items wrapped at 82 characters, and is left out when no rule holds.

#### Scenario: Cold returns
- **WHEN** the cold returns of the time shown (see pause-report) would have saved $10 or more on a warm cache: 23 returns, $107 of $111, after breaks of a median 1.4h
- **THEN** the item reads `▸ 23 returns after the cache expired cost $107 that a warm cache would have saved; their breaks had a median of 1.4h. Come back before the cache expires, or before a longer break run /compact or start the next session from a handoff (tokprof handoff).`
- **AND** below $10 there is no such item

#### Scenario: Questions waited
- **WHEN** the agent asked 1 question, it waited 1.3h for its answer, and the sessions shown are Claude Code's
- **THEN** the item reads `▸ The agent's questions waited a median 1.3h for your answer (1 question). Answer them as they come, or let the agent decide small things: add "when a choice is small, pick one, say which, and go on" to CLAUDE.md.`
- **AND** with Codex sessions too the file reads `CLAUDE.md or AGENTS.md`, and a median of 10 minutes or less gives no item

#### Scenario: Out of work
- **WHEN** the agents stood still a median 4.0m after the user left and worked through 19% of a median 38m break
- **THEN** the item reads `▸ After you left, the agents stood still a median 4.0m into a median 38m break and worked through 19% of it. Before a break, give each session its next steps too, or a plan it can work through without asking.`
- **AND** 30% or more, or no break, gives no item

#### Scenario: Step by step
- **WHEN** the agent's turns took a median 3.5m and, while the user was there, a turn ended every 3.6m
- **THEN** the item reads `▸ While you were there, an agent ended a turn every 3.6m, after turns of a median 3.5m: you led it step by step. Give it a goal and a way to check it, not a step, and leave between its turns.`
- **AND** a median turn of 5 minutes or more, or a turn ending every 5 minutes or less often, gives no item

#### Scenario: Two items
- **WHEN** the cold-returns and step-by-step rules hold
- **THEN** the block starts with `advice    ▸ 23 returns`, the next item with 10 spaces and `▸ `, and each wrapped line with 12 spaces

## MODIFIED Requirements

### Requirement: Day view
`tokprof day [DATE]` SHALL show one day: the day of the user's latest action, or the day of DATE (YYYY-MM-DD). It reads the main logs whose last line with a time is at or after 18:00 the day before, and that were created before 12:00 the day after. It applies the project, session and client filters, and prints the title `tokprof · day`, a `day` line, the `night` line, the `left out` line when there is one, the strip, the rhythm lines, the `cache` line and the advice.

#### Scenario: The logs of a day
- **WHEN** a main log was created on 2026-10-20, its last line with a time is at 2026-11-02 09:00, and the IDE appended lines without a time to it on 2026-11-03
- **THEN** it is read for the day of 2026-11-02 and not for 2026-11-03
- **AND** a log created on 2026-11-03 at 13:00 is not read for 2026-11-02

#### Scenario: The day line
- **WHEN** the day of 2026-11-02 runs from 04:21 to 04:15 the next morning
- **THEN** the line reads `day       Mon 2026-11-02 04:21 → Tue 2026-11-03 04:15`
- **AND** the current day's line ends with `→ now`, as in `day       Tue 2026-11-03 04:24 → now`

#### Scenario: Today before the start of the day
- **WHEN** `tokprof day` runs at 02:10 on 2026-11-03 and the user's latest action was at 01:50
- **THEN** it shows the day of 2026-11-02, since no night has come since that day began

#### Scenario: A day with no message
- **WHEN** the user did not act in the day
- **THEN** after the `night` line the view reads `No messages on 2026-11-02.`

#### Scenario: The advice
- **WHEN** a rule of Rhythm advice holds for the day
- **THEN** its item follows the `cache` line in the `advice` block

#### Scenario: Time filters
- **WHEN** `tokprof --since 7d day` runs
- **THEN** it fails with `day picks its time by DATE; drop --since and --until`
