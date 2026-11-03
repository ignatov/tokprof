# Proposal

## Why

The day view and the rhythm section of `tokprof pauses` give numbers, and the user has to work out what to change. Four patterns recur in the author's logs. Over the 31 day views of 2026-10-03 to 2026-11-02:
- on 5 days the agent's questions waited a median of over 10 minutes for an answer, 2.9h on 2026-10-19;
- on 19 days the agents worked through less than 30% of a median break, as on 2026-10-23, when they stood still 4 minutes into a 38-minute break;
- on 21 days the cold returns cost $10 or more that a warm cache would have saved, $775 over the 31 days and $107 on 2026-10-27;
- on 15 days the agent's turns took a median of under 5 minutes and it ended a turn more often than every 5 minutes while the user was there: the user led it step by step.

## What Changes

- **An `advice` block** in `tokprof day` (and so in the browser's day view) and in the rhythm section of `tokprof pauses`. It holds each of four items whose rule holds, in this order, with the numbers it rests on and one thing to do:
  - **Cold returns:** the returns after the cache expired, what a warm cache would have saved, and the median break before them. Come back before the cache expires, or before a long break run `/compact` or hand off with `tokprof handoff`.
  - **Questions waited:** how long the agent's questions waited. Answer them as they come, or let the agent decide small things through a line in the agent's instruction file.
  - **The agents ran out of work:** how soon they stood still after the user left, and what share of a break they worked. Before a break, give each session its next steps, or a plan it can work through without asking.
  - **Step by step:** how often a turn ended while the user was there, and how long turns took. Give the agent a goal and a way to check it, not a step.
- The README describes the block.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `work-rhythm`: a new requirement for the advice block and its four rules; the day view prints the block after its `cache` line.
- `pause-report`: the rhythm section prints the block after its rhythm lines.

## How it is checked on the real logs

- Over the 31 days to 2026-11-02, `tokprof day` gives the cold-returns item on 21 days, the questions item on 5, the out-of-work item on 19 and the step-by-step item on 15. A day has a median of 2 items.
- `tokprof day 2026-10-27` gives the cold-returns, questions and step-by-step items: 23 returns and $107, 1 question that waited 1.3h, turns ending every 3.6m after turns of 3.5m.
- `tokprof --agent claude --since 30d pauses` gives the cold-returns, out-of-work and step-by-step items, with its own numbers: $750 that a warm cache would have saved, 22% of a median break, turns ending every 4.2m after turns of 3.3m.

## Out of scope

- Advice from single lanes, such as a session that waited hours for an answer, or a day with many sessions: both hold on most days and would be noise.
- Changing settings or instruction files for the user.
- A what-if of coming back every N minutes.

## Impact

- Core: the advice rules over a rhythm summary and a day's cold returns.
- CLI: the day view and the pause report's rhythm section print the block; the browser shows it with the day.
- README.
