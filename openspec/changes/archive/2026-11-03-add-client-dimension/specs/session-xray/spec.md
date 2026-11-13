# Spec Delta

## MODIFIED Requirements

### Requirement: Header
The X-ray SHALL open with the title `tokprof · session X-ray` and these lines:
- `session` with the agent, the project path (home shown as `~`) and the full session id;
- `title` with the session's title (see cost-attribution) cut to 82 characters;
- `client` with the clients of the session's requests (see session-logs);
- `models` with each model and its request count;
- `time` with the first and last request in local time, their span, the request count and the number of subagent logs.

#### Scenario: Session within one day
- **WHEN** the first and last requests fall on the same local date
- **THEN** the end shows only the time of day, as in `time      2026-11-01 08:49 → 18:15 (9.4h) · 657 requests`

#### Scenario: Session without subagents
- **WHEN** a session has no subagent logs
- **THEN** its `time` line ends with the request count and names no subagents

#### Scenario: Session over several days
- **WHEN** a session ran from one day into the next
- **THEN** both ends show the date, as in `time      2026-10-22 11:18 → 2026-10-23 19:57 (1.4d) · 7015 requests · 75 subagents`

#### Scenario: Session without requests
- **WHEN** a session's logs hold no request, as in a Codex rollout that stopped before its first response
- **THEN** there is no `models` line, and the time line reads `time      0 requests`
- **AND** no date from 1970 is shown: whenever no request has a time, the line gives only the counts

#### Scenario: Models
- **WHEN** a session used two models
- **THEN** the `models` line lists both by name with their counts, as in `claude-haiku-4-5 ×12, claude-opus-5-5 ×657`

#### Scenario: Span format
- **WHEN** the span is under a minute, under an hour, under a day, or longer
- **THEN** it shows in seconds (`59s`), whole minutes (`45m`), hours with one decimal (`9.4h`) or days with one decimal (`1.4d`)

#### Scenario: Title
- **WHEN** the session's title is `Review PR #412 against the checklist`
- **THEN** the line after `session` reads `title     Review PR #412 against the checklist`
- **AND** a session without a title has no `title` line

#### Scenario: One client
- **WHEN** every request of the session, its subagents included, has the client `sdk-ts`
- **THEN** the line after `title` reads `client    sdk-ts`

#### Scenario: Several clients
- **WHEN** a session has 412 requests with the client `cli` and 37 with `sdk-ts`
- **THEN** the line reads `client    cli ×412 · sdk-ts ×37`, with each client's request count, most requests first

#### Scenario: No client
- **WHEN** no request of the session has a client
- **THEN** the line reads `client    unknown`
