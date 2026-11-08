# Spec Delta

## ADDED Requirements

### Requirement: Waiting now
A session SHALL wait for the user from the latest end of turn its agent logged, or from its question that has no answer yet, until the user's next action in it, while no turn has started since. A turn that ended at an interrupt, or whose end is not logged, starts no wait. A wait counts for 2 hours from its end of turn (the waiting limit); after that the session no longer waits. Sessions the rhythm leaves out never wait.

#### Scenario: A turn ended
- **WHEN** the agent ended its turn at 14:20, the user has not acted in the session since, and it is 14:36
- **THEN** the session waits, and has waited 16m

#### Scenario: The user answered
- **WHEN** the agent ended its turn at 14:20 and the user typed in the session at 14:25
- **THEN** the session does not wait

#### Scenario: A turn the agent started itself
- **WHEN** the agent ended its turn at 14:20, a background task woke it at 14:30, and it ended that turn at 14:34
- **THEN** it did not wait from 14:30 to 14:34, and from 14:34 it waits, counted from 14:34

#### Scenario: A question
- **WHEN** the agent asked a question at 14:20 and the user has not answered it at 14:30
- **THEN** the session waits, and has waited 10m

#### Scenario: Still working
- **WHEN** the agent's last request was at 14:20, no end of turn is logged, and it is 14:30
- **THEN** the session does not wait

#### Scenario: The waiting limit
- **WHEN** the agent ended its turn at 12:00, the user has not acted since, and it is 14:00
- **THEN** the session does not wait any more, and at 13:59 it still waited

### Requirement: Several sessions waiting
An answer (see Answers) SHALL count as one among several when, at its message, another shown session waits for an answer that comes later in the same sitting. Such an answer went to the newest when its end of turn is no earlier than any other's then. Its chance is one over the number of sessions waiting, the answered included. Its oldest wait is the longest wait among them.

#### Scenario: The newest first
- **WHEN** session A ended its turn at 10:00, session B at 10:12, the user answered B at 10:13 and A at 10:20, in one sitting
- **THEN** the answer to B is one among several, it went to the newest, its chance is 1/2, and its oldest wait is 13m
- **AND** the answer to A is not one among several: no other session waited then

#### Scenario: A session answered in a later sitting
- **WHEN** session A ended its turn at 10:00, the user answered B at 10:13, and answered A only after a break, in the next sitting
- **THEN** the answer to B is not one among several

### Requirement: Steps
A step SHALL be a message that answers in less than 3 minutes (see Answers) the end of a turn the user started, which took less than 3 minutes. A stretch of steps is 3 or more steps in a row among the messages of one session. Its time runs from its first step to its last.

#### Scenario: A stretch
- **WHEN** in one session the user's turns took 1m, 2m and 1m, and each was answered within 2m
- **THEN** the 3 answers are a stretch of steps

#### Scenario: A long turn in between
- **WHEN** the answers in one session are a step, the answer to a turn of 10m, and two more steps
- **THEN** there is no stretch: the answer to the long turn is not a step

#### Scenario: The limits
- **WHEN** a turn took exactly 3m, or its answer came after exactly 3m
- **THEN** that answer is not a step

## MODIFIED Requirements

### Requirement: Rhythm lines
The day view and the rhythm section of the pause report SHALL print six lines on the time they show: `sittings`, `left`, `present`, `answers`, `queue` and `stops`. Durations read as the X-ray's span (see session-xray), except that from one to ten minutes they keep one decimal, as in `3.5m`. A median takes the upper of two middle values.

#### Scenario: Sittings
- **WHEN** there were 16 sittings with 3.7h at the keyboard and 58 messages to 11 sessions, a median sitting of 6m and a median break of 28m
- **THEN** the line reads `sittings  16, 3.7h at the keyboard, 58 messages to 11 sessions; median sitting 6.0m, median break 28m`
- **AND** with a single sitting it ends `median sitting 6.0m, no break`

#### Scenario: Left
- **WHEN** the agents stood still a median 8m after the user left and some agent worked through a median 28% of a break
- **THEN** the line reads `left      the agents stood still a median 8.0m after you left, and worked through 28% of a median break`
- **AND** without breaks it reads `left      no break between sittings`

#### Scenario: Present
- **WHEN** 999 turns ended in 75h at the keyboard, and 37% of all the turns that ended were turns the agent started itself
- **THEN** the line reads `present   an agent ended a turn every 4.5m while you were there; it started 37% of its turns itself`
- **AND** with no turn ending in a sitting the first part reads `no turn ended while you were there`

#### Scenario: Answers
- **WHEN** the agent's turns took a median 3.5m and the user answered in a median 3.7m
- **THEN** the line reads `answers   the agent's turn took a median 3.5m; you answered in a median 3.7m`
- **AND** without turns or answers that part reads `no turns you started` or `no answers`

#### Scenario: Queue
- **WHEN** 538 answers were among several (see Several sessions waiting), 382 of them went to the newest, their chances average 43%, and their oldest waits had a median of 40m
- **THEN** the line reads `queue     when several sessions waited, you answered the one that waited least first in 71% of 538 answers (43% by chance); the oldest had waited a median 40m`
- **AND** with one such answer it reads `of 1 answer`, and with none `queue     no answer while another session waited`

#### Scenario: Stops
- **WHEN** there were 39 interrupts, 46 messages typed while the agent worked, and 74 questions answered in a median 2.3m
- **THEN** the line reads `stops     39 interrupts, 46 messages typed while the agent worked, 74 questions from the agent answered in a median 2.3m`
- **AND** counts of one are singular, and with no questions the end reads `no questions from the agent`

### Requirement: Rhythm advice
The day view and the pause report's rhythm section SHALL show an `advice` block with each item below whose rule holds over the time they show, in this order: cold returns, questions, newest first, out of work, step by step. The block is laid out as the X-ray's advice block, with items wrapped at 82 characters, and is left out when no rule holds.

#### Scenario: Cold returns
- **WHEN** the cold returns of the time shown (see pause-report) would have saved $10 or more on a warm cache: 23 returns, $107 of $111, after breaks of a median 1.4h
- **THEN** the item reads `▸ 23 returns after the cache expired cost $107 that a warm cache would have saved; their breaks had a median of 1.4h. Come back before the cache expires, or before a longer break run /compact or start the next session from a handoff (tokprof handoff).`
- **AND** below $10 there is no such item

#### Scenario: Questions waited
- **WHEN** the agent asked 1 question, it waited 1.3h for its answer, and the sessions shown are Claude Code's
- **THEN** the item reads `▸ The agent's questions waited a median 1.3h for your answer (1 question). Answer them as they come, or let the agent decide small things: add "when a choice is small, pick one, say which, and go on" to CLAUDE.md.`
- **AND** with Codex sessions too the file reads `CLAUDE.md or AGENTS.md`, and a median of 10 minutes or less gives no item

#### Scenario: Newest first
- **WHEN** 538 answers were among several, 71% of them went to the newest, and their oldest waits had a median of 40m
- **THEN** the item reads `▸ When several sessions waited, you answered the one that waited least first in 71% of 538 answers, while the oldest waited a median 40m. Go through them oldest first: run tokprof ui --waiting and press s until the list is by waiting.`
- **AND** fewer than 10 such answers, a share under 50%, or a median oldest wait under 10 minutes gives no item

#### Scenario: Out of work
- **WHEN** the agents stood still a median 4.0m after the user left and worked through 19% of a median 38m break
- **THEN** the item reads `▸ After you left, the agents stood still a median 4.0m into a median 38m break and worked through 19% of it. Before a break, give each session its next steps too, or a plan it can work through without asking.`
- **AND** 30% or more, or no break, gives no item

#### Scenario: Step by step
- **WHEN** the time shown has 4 stretches of steps (see Steps), with 14 steps in them and 22m between their first and last steps in all
- **THEN** the item reads `▸ 4 times you led a session step by step: 3 or more short turns in a row, each answered within 3m (14 messages in 22m). Give it a goal and a way to check it, not a step, and leave between its turns.`
- **AND** with one stretch it starts `▸ Once you led a session step by step`, and with no stretch there is no item, however short the median turn

#### Scenario: Two items
- **WHEN** the cold-returns and step-by-step rules hold
- **THEN** the block starts with `advice    ▸ 23 returns`, the next item with 10 spaces and `▸ `, and each wrapped line with 12 spaces
