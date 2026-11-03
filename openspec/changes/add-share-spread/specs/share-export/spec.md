# Spec Delta

## ADDED Requirements

### Requirement: Distribution parts
Each agent's object SHALL hold `distributions`, with one entry per measure of its main sessions: `session_usd`, the dollars of each main session with its subagents, as the pause parts count a session's spend; `typed_per_session`, its typed messages, the first one of a conversation included; and `largest_context`, its largest main prompt, in tokens.

#### Scenario: The values of each measure
- **WHEN** an agent has 328 main sessions
- **THEN** `session_usd`, `typed_per_session` and `largest_context` have `n` 328

#### Scenario: The first message of a session
- **WHEN** five main sessions have 1, 2, 3, 4 and 5 typed messages
- **THEN** `typed_per_session` is `{"n": 5, "p25": 2, "p50": 3, "p75": 4, "p90": 5}`, while the pause parts count 10 typed messages, since they leave out each session's first

### Requirement: Distributions of returns, turns and answers
`distributions` SHALL also hold `cold_return_usd`, the dollars of the first request of each cold return (see pause-report). When the file has the rhythm parts, it holds `turn_ms`, each turn the person started, and `answer_ms`, each answer to an end of turn, as the rhythm parts count them, with the days of all agents together.

#### Scenario: Cold returns
- **WHEN** an agent's `cold_returns_by_pause` counts 212 returns
- **THEN** its `cold_return_usd` has `n` 212

#### Scenario: Two agents
- **WHEN** the file's `rhythm` counts 1,523 turns and 1,168 answers over Claude Code and Codex
- **THEN** the `n` of `turn_ms` of the two agents add up to 1,523, and those of `answer_ms` to 1,168

### Requirement: Quantiles of a distribution
Each measure of `distributions` SHALL hold `n`, the number of values. From 5 values on, it also holds `p25`, `p50`, `p75` and `p90`: for `p`, the value at position ⌊n·p/100⌋ of the values sorted from least, counting from 0. So `p50` is the upper of two middle values, as the file's medians are. With fewer than 5 values the measure holds only `n`, so that no single session's value can be read from it. Dollars have 4 decimals; times in ms, tokens and counts are whole.

#### Scenario: Few values
- **WHEN** an agent has 4 cold returns
- **THEN** `cold_return_usd` is `{"n": 4}`

#### Scenario: Six values
- **WHEN** a measure has the values 1, 2, 3, 4, 5 and 6
- **THEN** its `p50` is 4

### Requirement: Prevalence parts
Each agent's object SHALL hold `prevalence`, with counts of the agent's active days (see Activity parts): `days_with_cold_return`, days with a cold return of the pause parts; `days_with_retry_wait`, days with a main request that waited for retries, as `time.retried` counts them; and `days_with_interrupt`, days on which the user stopped the agent, as `handoffs.interrupts` counts it. A day is a local day, as the window names it. A day that is not an active day does not count.

#### Scenario: Days agree with the other parts
- **WHEN** an agent's part is written
- **THEN** every day count is at most `active_days`
- **AND** `days_with_retry_wait` and `days_with_interrupt` are at most `time.retried` and `handoffs.interrupts`

#### Scenario: Two cold returns two days apart
- **WHEN** a session has a cold return at 14:00 and another session one 57 hours later
- **THEN** `days_with_cold_return` is 2

### Requirement: Prevalence of main sessions
`prevalence` SHALL also count the agent's main sessions: `sessions_with_cold_return`, with a cold return; `sessions_with_failure_window`, in which a failure window opened, its subagents included (see tool-breakdown, What came after a failure); `sessions_over_400k`, with a largest main prompt of 400,000 tokens or more; `sessions_over_1d`, that ran 24 hours or more from their first to their last selected request; and `sessions_with_question`, in which the agent asked a question inside the window.

#### Scenario: Sessions agree with the other parts
- **WHEN** an agent's part is written
- **THEN** `sessions_with_cold_return` is at most `main_sessions` and the cold returns of `cold_returns_by_pause`
- **AND** `sessions_over_400k` equals the sessions of `400–800k` and `800k+` in `sessions_by_largest_context`, and `sessions_over_1d` those of `1–3d` and `over 3d` in `session_spans`

#### Scenario: Two questions in one session
- **WHEN** the agent asked two questions in one main session and none in two others
- **THEN** `sessions_with_question` is 1, and `handoffs.questions` is 2

#### Scenario: A failure at the end of a session
- **WHEN** a session's last response made a call that failed
- **THEN** no window opened, and the session does not count in `sessions_with_failure_window`

### Requirement: Prevalence of the file
The file SHALL also hold `prevalence` next to `activity`: `days_with_cold_return`, `days_with_retry_wait` and `days_with_interrupt` over all its agents together, a day counted once however many agents had it, out of the file's `active_days`. Counts of main sessions stay in the agents' parts, since a main session belongs to one agent.

#### Scenario: An interrupt in two agents on one day
- **WHEN** the user stopped Claude Code and Codex on the same day, and on no other
- **THEN** each agent's `days_with_interrupt` is 1, and so is the file's

### Requirement: Words of the spread parts
The keys of the distribution and prevalence parts SHALL be tokprof's own words (see Own words): the part names, the measure and count names, `n` and the quantile names.

#### Scenario: Nothing new outside the own words
- **WHEN** a file with these parts is written
- **THEN** its words outside tokprof's own are the same as those of the file without them
