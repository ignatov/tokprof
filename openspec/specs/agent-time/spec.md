# agent-time Specification

## Purpose
Shows where an agent's time went while it was busy: the model, API retries, its subagents, its background commands, the calls it made, or nothing at all. It also shows which requests were slow, so that the user can tell what a session waited on.

## Requirements

### Requirement: Busy time
A session's agent SHALL count as busy from the start of each turn of its main conversations to the turn's end (see work-rhythm, Turns), except while it waits for the answer to its own question. After an end of turn, it also counts as busy while a subagent of the session works, and until a wake that reports a background command when no turn starts first (see session-logs, What a wake reports).

#### Scenario: A turn and the answer after it
- **WHEN** the user types at 10:00, the agent ends its turn at 10:04, and the user types again at 10:10
- **THEN** the agent is busy from 10:00 to 10:04 and from 10:10, and not from 10:04 to 10:10

#### Scenario: A question
- **WHEN** in a turn from 10:00 to 10:09 the agent asks a question at 10:02 and the user answers at 10:07
- **THEN** the agent is busy from 10:00 to 10:02 and from 10:07 to 10:09

#### Scenario: A command in the background
- **WHEN** the agent starts tests in the background, ends its turn at 10:04, and a wake that reports the tests comes at 10:19
- **THEN** the agent is busy until 10:19

#### Scenario: Subagents in the background
- **WHEN** the agent ends its turn at 10:04, a subagent of the session works until 10:30, and the user types at 11:00
- **THEN** the agent is busy until 10:30, and not from 10:30 to 11:00

#### Scenario: A message from another session
- **WHEN** the agent ends its turn at 10:04 with nothing of its own running, and another session's message wakes it at 12:00
- **THEN** the agent is not busy from 10:04 to 12:00

#### Scenario: The user comes back first
- **WHEN** the agent ends its turn at 10:04 with a command running in the background, the user types at 10:10, and the wake that reports the command comes at 10:19
- **THEN** the agent is not busy from 10:04 to 10:10, and is busy in the turn that starts at 10:10

### Requirement: What the busy time went to
Each moment of busy time SHALL go to the first kind that applies: `model` while a request of a main conversation runs; `API retries` while such a request waits to be sent again after an API error; `subagents` while a subagent of the session works; then what the agent waited on before its next main request (see What the agent waited on between requests); and `nothing` when none applies.

#### Scenario: A request
- **WHEN** a main request is sent at 10:00:00 and its response ends at 10:00:40
- **THEN** those 40 seconds go to `model`
- **AND** a request runs from the send of its answered attempt to the end of its response (see session-logs, Request times)

#### Scenario: Retries
- **WHEN** a request's first attempt is sent at 10:00:00, two attempts fail with API errors, and its answered attempt is sent at 10:00:20 and ends at 10:00:30
- **THEN** 20 seconds go to `API retries` and 10 seconds to `model`

#### Scenario: A request that got no response
- **WHEN** a main request is sent at 10:00 and no response comes, and the user interrupts at 10:03
- **THEN** the 3 minutes go to `model`, since a request with no response runs until the next request or hand-off of its conversation
- **AND** when the request was retried after API errors and no attempt was answered, that time goes to `API retries`

#### Scenario: A subagent while the main conversation waits
- **WHEN** a main response starts a subagent at 10:01, the subagent works until 10:20, and the next main request is sent at 10:21
- **THEN** the 20 minutes go to `subagents`
- **AND** a subagent works from the send of a request to the end of the next one's response while it runs the calls of the first, or when the next comes within 15 minutes

#### Scenario: A subagent resumed later
- **WHEN** a subagent works from 10:04 to 10:10, and a new message resumes it from 12:00 to 12:05
- **THEN** it works for 11 minutes, not for the time between

#### Scenario: A subagent while a main request runs
- **WHEN** a subagent works while a main request runs
- **THEN** that time goes to `model`

### Requirement: What the agent waited on between requests
Busy time from the end of a main response to the send of the next main request SHALL go to that request's activity (see cost-attribution, Activity of a request) when calls triggered it, the activity `subagents` counting as `subagents`. It goes to `compaction` when that request's trigger is `compaction`, and to `background commands` from an end of turn to a wake that reports a background command.

#### Scenario: A test run
- **WHEN** a response that called `cargo test` ends at 10:01, and the next request, whose activity is `building and testing`, is sent at 10:04
- **THEN** the 3 minutes go to `building and testing`

#### Scenario: A command in the background
- **WHEN** the agent ends its turn at 10:04 and a wake that reports its background tests comes at 10:19
- **THEN** the 15 minutes go to `background commands`

#### Scenario: A compaction
- **WHEN** the first request after a compaction is sent 2 minutes after the response before it ended
- **THEN** those 2 minutes go to `compaction`

#### Scenario: Nothing ran
- **WHEN** the user types at 10:00 and the turn's first request is sent at 10:00:05
- **THEN** the 5 seconds go to `nothing`
- **AND** so does busy time before a request whose trigger is `continuation` or `unknown`

#### Scenario: Nothing for a long time
- **WHEN** a stretch that would go to `nothing` lasts 15 minutes or more, the length of a silence in work-rhythm, as after a local command such as `/model` that sent no request
- **THEN** it is no busy time, since nothing of the agent's ran

### Requirement: After the end of a turn
The busy time after an end of turn and before the next turn starts SHALL be told apart from the busy time in turns: it is the time that the agent's own background work kept it busy.

#### Scenario: Subagents after the turn
- **WHEN** of 1.5 hours of `subagents`, 1.1 hours came after the end of a turn while the main conversation stood still
- **THEN** 1.1 hours of `subagents` are after the end of a turn

#### Scenario: Background commands
- **WHEN** busy time goes to `background commands`
- **THEN** all of it is after the end of a turn

### Requirement: Waits
A wait SHALL be a stretch of busy time of one minute or more that goes, without a break, to one kind other than `model` and `API retries`. The longest waits are the ones the views name.

#### Scenario: A long test run
- **WHEN** a response calls tests at 10:01 and the next request is sent at 10:05
- **THEN** that is a wait of 4 minutes for `building and testing` that starts at 10:01

#### Scenario: A short call
- **WHEN** the next request is sent 40 seconds after the response
- **THEN** that is no wait

#### Scenario: A wait cut by a request
- **WHEN** a subagent works after an end of turn from 10:04 to 10:30, the user types at 10:20, and the main request that follows runs from 10:20 to 10:21
- **THEN** there is a wait of 16 minutes for `subagents` from 10:04, and another of 9 minutes from 10:21

### Requirement: Slow requests
A request SHALL be slow when its answered attempt runs a minute or more, from its send to the end of its response. Requests of every chain count, subagents included; an advisor call is part of the response it ran in and does not count apart. The time a request waited for retries is not part of it.

#### Scenario: A long answer
- **WHEN** a request is sent at 10:00:00 and its response ends at 10:03:29 with 21.7k output tokens
- **THEN** it is slow, and runs 3m 29s

#### Scenario: A retried request
- **WHEN** a request waits 50 seconds for retries and its answered attempt runs 20 seconds
- **THEN** it is not slow, and its 50 seconds go to `API retries` when it is a main request
