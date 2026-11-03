# Spec Delta

## ADDED Requirements

### Requirement: Activity parts
Each agent's object SHALL hold `activity`: `active_days`, the days with at least one selected request of a main conversation; `active_hours`, the clock hours with one; and `busy_ms`, the agent's busy time (see Time parts). Days are local days, as the window names them. Hours are whole hours of UTC, so that a person's time zone does not change the count.

#### Scenario: Requests in two hours of one day
- **WHEN** a main conversation's selected requests come at 10:05, 10:50 and 11:10 UTC on one day
- **THEN** `activity` has `active_days` 1 and `active_hours` 2

#### Scenario: A subagent alone
- **WHEN** only a subagent's requests come in an hour
- **THEN** that hour is not an active hour

### Requirement: Activity of the file
The file SHALL also hold `activity` next to `window`: the active days and active hours of all its agents together, a day or an hour counted once however many agents worked in it, and the sum of their busy time. The rates of the report divide by these.

#### Scenario: Two agents in one hour
- **WHEN** Claude Code and Codex both sent main-conversation requests between 10:00 and 11:00 UTC, and in no other hour
- **THEN** each agent's `activity` has `active_hours` 1, and so has the file's

### Requirement: Failure parts
Each agent's object SHALL hold `failures`: the calls that failed, the calls the user refused, the calls a permission rule denied, and `after`, the dollars and tokens of the failure windows, each request counted once (see tool-breakdown). Each tool in `tools`, and each program of a shell tool, SHALL also hold its own `failed` and `after`. No error text goes into the file.

#### Scenario: The same numbers as the tools command
- **WHEN** `tokprof tools` and `tokprof share` run with the same filters
- **THEN** the counts and dollars of `failures` add up, over the agents, to the failures line of `tokprof tools`

#### Scenario: An error that names a project
- **WHEN** a failed call's error reads `cannot open ~/src/zebra-billing/a.rs`
- **THEN** the file counts the failure and holds no `zebra`

### Requirement: Time parts
Each agent's object SHALL hold `time`, the busy time inside the window of the agent's selected sessions, as the pause report counts it (see pause-report, Busy time part). Per kind of busy time, it holds the milliseconds and the part after ends of turn. It also holds the requests, the slow ones, the slow ones of the main conversations, the time of the longest request, and the main requests that waited for retries. The errors of the retries are left out.

#### Scenario: The same numbers as the pause report
- **WHEN** `tokprof pauses` and `tokprof share` run with the same filters
- **THEN** each kind's time and the part after turns, the requests, the slow requests and the longest request equal the agent's busy table and slow line

#### Scenario: A long thread
- **WHEN** a thread that started months before the window was busy inside it for 2 hours
- **THEN** `time` counts the 2 hours, and the requests sent inside the window

#### Scenario: A retry error that names a host
- **WHEN** a request waited for retries after the error `502 Bad Gateway from walrus.internal`
- **THEN** `time` counts the retried request, and the file holds no `walrus`

### Requirement: Hand-off parts
Each agent's object SHALL hold `handoffs`: in the main conversations of the selected sessions and inside the window, how many times the agent ended its turn, the user stopped it, the user typed while it worked, the agent asked a question, and the user answered. It also counts the wakes by what each reported: `subagent`, `command` or `other` (see session-logs).

#### Scenario: One session's hand-offs
- **WHEN** a session ends 10 turns, is stopped twice, asks 3 questions that get 3 answers, and gets 4 wakes from background commands
- **THEN** `handoffs` has `turn_ends` 10, `interrupts` 2, `questions` 3, `answers` 3 and `wakes` `{"command": 4}`

#### Scenario: A hand-off outside the window
- **WHEN** a session ended a turn the day before `--since`
- **THEN** that end of turn does not count

### Requirement: Prompt coverage parts
Each agent's object SHALL hold `prompts`: the requests and dollars of the selected requests whose prompts the log holds (see cost-attribution, Unknown content), and those of all selected requests. Each version in `versions` SHALL also hold `held`, its requests whose prompts the log holds.

#### Scenario: An old Claude Code
- **WHEN** 11,242 of 74,095 selected requests come from transcripts with no request records
- **THEN** `prompts` has 62,853 requests held of 74,095

#### Scenario: A version without request records
- **WHEN** no request of version `2.1.296` has a request record
- **THEN** `versions["2.1.296"].held` is 0

### Requirement: Client activity and effort
Each client in `clients` SHALL also hold `active_hours`, the clock hours with at least one selected main-conversation request of that client; `cold_returns_avoidable_usd`, the avoidable dollars of its cold returns (see pause-report); and `effort`: requests and dollars by effort level, with the levels Work parts names.

#### Scenario: Cold returns of a client
- **WHEN** a client's misses could have saved $887, of which its 207 cold returns $740
- **THEN** its `avoidable_usd` is $887 and its `cold_returns_avoidable_usd` is $740

#### Scenario: A client at max
- **WHEN** 900 of a client's requests ran at `max` for $800, and 100 at `medium` for $50
- **THEN** its `effort` holds `max` with 900 requests and $800, and `medium` with 100 requests and $50

### Requirement: Words of the new parts
The keys of the new parts SHALL be tokprof's own words (see Own words): the part and field names, the kinds of busy time as agent-time names them, and the kinds of hand-off and of wake.

#### Scenario: No new words outside the own ones
- **WHEN** a file with the new parts is written
- **THEN** its words outside tokprof's own are the same as those of the file without them
