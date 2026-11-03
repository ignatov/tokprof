# Spec Delta

## ADDED Requirements

### Requirement: Rhythm parts
The file SHALL hold `rhythm`, the person's time with all its agents together inside the window, as the rhythm counts it (see work-rhythm). It holds `keyboard_ms`, `sittings`, `breaks`, `messages` and `sessions`; `ends`, with `present` and `started_by_agent`; `interrupts`, `typed_while_working` and `questions`; the waits `answers`, `question_answers`, `turns` and `stood_still`; and `worked_share_median`. The sessions the rhythm leaves out are left out here too.

#### Scenario: The same numbers as the rhythm section
- **WHEN** `tokprof pauses` and `tokprof share` run with the same filters
- **THEN** `rhythm` holds the rhythm section's sittings, time at the keyboard, messages and sessions
- **AND** the medians of its answers, question answers, turns and the time the agents stood still equal those of the rhythm lines

#### Scenario: Two sittings
- **WHEN** on an afternoon the person acts at 14:00, 14:05 and 14:14 in one session and at 14:40 in another
- **THEN** `rhythm` has `"sittings": 2`, `"breaks": 1` and `"keyboard_ms": 900000`

#### Scenario: Sessions nobody typed in
- **WHEN** a program ran a session, or it ran in a temporary folder
- **THEN** none of its events count in `rhythm`

### Requirement: Waits in the rhythm parts
Each wait in `rhythm` SHALL hold `count`, `median_ms` and `by_pause`, its counts by the pause ranges of the pause report (see pause-report). `answers` runs from an end of turn to the person's next message, `question_answers` from a question to its answer, `turns` over the turns the person started, and `stood_still` from the end of a sitting until the agents first stood still (see work-rhythm, Answers and After the user left).

#### Scenario: Agents after the person left
- **WHEN** the agents worked 10 minutes of a 40-minute break
- **THEN** that break's share is 25%, and `worked_share_median` is the median of these shares over the breaks

#### Scenario: An answer
- **WHEN** a turn ends at 10:04 and the person types in that session at 10:07
- **THEN** `answers` counts it as 180,000 ms, in the range `1–5m`

#### Scenario: A median of two
- **WHEN** a file has two answers, of 2 and 6 minutes
- **THEN** `answers.median_ms` is 360000, the upper of the two middle values, as the rhythm takes a median

### Requirement: Rhythm per client
Each client in `clients` SHALL also hold `rhythm`, counted over the sessions of that client only. It holds `keyboard_ms`, `sittings`, `messages`, `interrupts`, `typed_while_working` and `questions`; `ends` with `present`; and the `count` and `median_ms` of `answers` and `question_answers`. A session's client is the first client its main log names, as the client filter reads a session (see session-logs, Client filter).

#### Scenario: An IDE and a terminal
- **WHEN** the person typed 40 messages in `sdk-ts` sessions and 10 in `cli` sessions
- **THEN** `clients.sdk-ts.rhythm.messages` is 40, `clients.cli.rhythm.messages` is 10, and the file's `rhythm.messages` is 50

#### Scenario: Two clients at once
- **WHEN** the person worked in an `sdk-ts` and a `cli` session in the same sitting
- **THEN** that sitting counts in the `keyboard_ms` of both clients and once in the file's `rhythm`

### Requirement: Bench parts
Each agent's object SHALL hold `bench`, the cold returns inside the window as `tokprof bench` finds them with its defaults (see return-bench). It holds `returns`, `requests` (60), `min_context` (100000) and `keep_warm_ms` (21600000). Under `ways`, each of `resume`, `compact`, `handoff`, `heartbeats` and `heartbeats_then_compaction` holds `usd` and `cheapest`, the returns where it cost least. It also holds `kept_warm`, `ping_usd` and `elsewhere`.

#### Scenario: The same numbers as the bench
- **WHEN** `tokprof bench` and `tokprof share` run with the same filters
- **THEN** each agent's `bench.returns` equals the bench's returns, and each way's `usd` equals its `all returns` cost (see return-bench, Per-agent table)

#### Scenario: The cheapest way
- **WHEN** a handoff note cost least in 203 of 203 returns
- **THEN** `ways.handoff.cheapest` is 203 and the other ways' `cheapest` are 0
- **AND** a tie goes to the way named first in the list above

#### Scenario: Breaks that were no return
- **WHEN** heartbeats would have sent 1,026 pings for $87.08 in breaks that were not cold returns, and heartbeats then compaction 136 compactions as well
- **THEN** `elsewhere` holds `"pings": 1026`, `"ping_usd": 87.08`, `"compactions": 136` and their dollars (see return-bench, Breaks that were not cold returns)

#### Scenario: No returns
- **WHEN** an agent has no cold return in the window
- **THEN** its `bench` has `"returns": 0` and no `ways`

### Requirement: Outcome parts
Each agent's object and each client SHALL hold `outcomes`: `commits`, `pushes` and `pull_requests`. They count the shell calls, in main conversations and subagents, that ran `git commit`, `git push` or `gh pr create` in some part of their command and worked: were not failed, refused or denied (see tool-breakdown, Failed, refused and denied calls). Options before the subcommand of `git`, such as `-C <dir>`, are skipped. A call that ran two of them counts for each. No argument goes into the file.

#### Scenario: A commit in a chain
- **WHEN** a call runs `git add -A && git commit -m "fix"` and does not fail
- **THEN** it adds 1 to `commits` and nothing to `pushes`

#### Scenario: A commit and a push
- **WHEN** a call runs `git commit -m x && git push`
- **THEN** it adds 1 to `commits` and 1 to `pushes`

#### Scenario: Another folder
- **WHEN** a call runs `git -C ../app commit -m x`
- **THEN** it adds 1 to `commits`

#### Scenario: A heredoc
- **WHEN** a call runs `cat > notes.md <<'EOF'`, with a body that mentions `git push`, and after the body `git add notes.md && git commit -m x`
- **THEN** it adds 1 to `commits` and nothing to `pushes`: a heredoc's body is data, not commands

#### Scenario: A failed push
- **WHEN** a call runs `git push` and fails
- **THEN** it adds nothing

#### Scenario: A pull request
- **WHEN** one call runs `gh pr create --fill` and another `gh pr view 12`
- **THEN** the first adds 1 to `pull_requests` and the second adds nothing

### Requirement: Retry error classes
The `time` part SHALL also hold `retried_by_error`: the requests that waited for retries, by the class of the last error before their answered attempt (see agent-time): `server`, `rate_limit`, `auth`, `timeout`, `connection`, `sleep` or `other`. A status number in the error counts before its words. The error text stays out of the file.

#### Scenario: The classes
- **WHEN** the last error of a request is classed
- **THEN** an HTTP status from 500 to 599 is `server`, 429 is `rate_limit`, 401 or 403 is `auth`, and 408 is `timeout`
- **AND** without a status, a text that says the computer was asleep is `sleep`, one that says the request timed out is `timeout`, one that says a connection failed, closed, was lost, reset or refused is `connection`, and any other is `other`

#### Scenario: The computer slept
- **WHEN** the last error reads `Connection lost while your computer was asleep`
- **THEN** the request counts under `sleep`, not `connection`: the wait was the computer's, not the API's

#### Scenario: A gateway error
- **WHEN** the last error before a request's answer reads `API Error: 502 Bad Gateway`
- **THEN** the request counts under `server`

#### Scenario: A timeout
- **WHEN** the last error reads `Request timed out.`
- **THEN** the request counts under `timeout`

#### Scenario: The classes add up
- **WHEN** 92 requests waited for retries
- **THEN** the counts of `retried_by_error` add up to 92, and a request whose log gives no error text counts under `other`

### Requirement: Words of the rhythm, bench and outcome parts
The keys of the parts this change adds SHALL be tokprof's own words (see Own words): the part and field names, the names of the ways and the error classes.

#### Scenario: Nothing new outside the own words
- **WHEN** a file with these parts is written
- **THEN** its words outside tokprof's own are the same as those of the file without them
