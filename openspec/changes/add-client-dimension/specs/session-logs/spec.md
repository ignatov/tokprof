# Spec Delta

## ADDED Requirements

### Requirement: Client of a request
Each request SHALL carry its client, the program that ran the agent, as the log names it. Claude Code names it as `entrypoint` on the line of the response, seen in 2.1.284–2.1.316 as `sdk-ts`, `cli` and `sdk-cli`. Codex names it as `payload.originator` of the rollout's `session_meta`, for all of its requests, seen in 0.147–0.166 as `codex_exec`, `codex-tui`, `Codex Desktop` and the names of IDE plugins. A request whose log names none has the client `unknown`.

#### Scenario: A session resumed in another client
- **WHEN** a Claude Code transcript has responses whose lines carry `entrypoint` `cli`, and later ones that carry `sdk-ts`
- **THEN** each request carries the client of its own response

#### Scenario: A subagent transcript
- **WHEN** the lines of a Claude Code subagent transcript carry `entrypoint` `sdk-ts`
- **THEN** its requests have the client `sdk-ts`

#### Scenario: A Codex rollout
- **WHEN** a rollout's `session_meta` has `originator` `codex_exec`
- **THEN** every request of that rollout has the client `codex_exec`

#### Scenario: No client in the log
- **WHEN** a log names no client
- **THEN** its requests have the client `unknown`

### Requirement: Client filter
`--client` SHALL keep only the requests whose client contains the given text, ignoring case. The commands that pick or list sessions SHALL keep a session when the first client its main log names contains the text.

#### Scenario: Part of a value
- **WHEN** `--client sdk` is given
- **THEN** the requests with the clients `sdk-ts` and `sdk-cli` are kept
- **AND** the requests with the client `cli` are left out

#### Scenario: Case
- **WHEN** `--client "codex desktop"` is given
- **THEN** the requests with the client `Codex Desktop` are kept

#### Scenario: Requests without a client
- **WHEN** `--client unknown` is given
- **THEN** only the requests whose log names no client are kept

#### Scenario: A session list
- **WHEN** `tokprof --client sdk-ts sessions` runs
- **THEN** it lists only the sessions whose main log first names a client that contains `sdk-ts`

#### Scenario: A text inside a longer value
- **WHEN** `--client cli` is given
- **THEN** the requests with the clients `cli` and `sdk-cli` are kept, since both contain the text

## MODIFIED Requirements

### Requirement: Commands the filters apply to
`--since`, `--until`, `--project`, `--session` and `--client` SHALL apply to every command but `prices`. The reports `summary`, `misses`, `whatif`, `tools`, `flame` and `bench` count the requests they select. The default command, `scan`, `handoff`, `sessions` and `ui` pick or list only the sessions they select: project and id as for the reports, the client as the Client filter requirement says, a main log last written on or after `--since`, and created before `--until`. An X-ray still covers its session's whole log.

#### Scenario: Default command with a session filter
- **WHEN** `tokprof --session 3f951de0` runs without a command
- **THEN** it X-rays `3f951de0`, as `tokprof scan 3f951de0` does
- **AND** `tokprof --project billing` X-rays the latest session of `~/src/billing`

#### Scenario: Session list with a time filter
- **WHEN** `tokprof --since 7d sessions` runs
- **THEN** it lists only the sessions whose main log was written in the last 7 days

#### Scenario: Sessions started before a date
- **WHEN** `tokprof --until 2026-10-01 sessions` runs
- **THEN** it lists only the sessions whose main log was created before 2026-10-01 00:00 local time

#### Scenario: A target outside the filters
- **WHEN** `tokprof --project billing scan 3f951de0` runs and `3f951de0` belongs to another project
- **THEN** the command fails as for a target that matches nothing

#### Scenario: Prices
- **WHEN** `tokprof --since 7d prices` runs
- **THEN** it prints the same prices as `tokprof prices`

#### Scenario: Latest session of a client
- **WHEN** `tokprof --client sdk-ts` runs without a command
- **THEN** it X-rays the latest session whose main log first names a client that contains `sdk-ts`
