# Spec Delta

## ADDED Requirements

### Requirement: Spend by client
`tokprof summary` SHALL print `Spend by client: the program that ran the agent, as its log names it` and a table, one row per agent and client (see session-logs), most expensive first, with columns `agent`, `client`, `sessions`, `requests`, `cost`, `share` and `avoidable`. `sessions` counts the main sessions with a request of that client. `avoidable` is the premium of the misses that `Cache misses by cause` lists (see cache-misses). The rows add up to `Total priced`.

#### Scenario: Two clients
- **WHEN** the window holds 67,097 Claude Code requests with the client `sdk-ts` that cost $8,534, and 2,292 with the client `cli` that cost $424
- **THEN** the row `claude  sdk-ts` with `67097` requests and `$8,534` comes before the row `claude  cli` with `2292` requests and `$424`

#### Scenario: A session in two clients
- **WHEN** one session has requests with the client `cli` and others with `sdk-ts`
- **THEN** it counts in the `sessions` of both rows
- **AND** each of its requests counts in the row of its own client

#### Scenario: Requests without a client
- **WHEN** the logs of some requests name no client
- **THEN** those requests form a row with the client `unknown`

## MODIFIED Requirements

### Requirement: Summary report
`tokprof summary` SHALL print `Spend by model (API-equivalent USD; token counts where no price is known)` and a table, one row per agent and model, most expensive first: `agent`, `model`, `requests`, `prompt known`, five components and `total`. Then come the totals line, the unpriced models (see pricing), the reconciliation line if any, the spend by client, origin, task, trigger and activity, the setup table, and `Cache misses by cause` (see cache-misses) minus expected rebuilds.

#### Scenario: A model row
- **WHEN** the reported window holds 16,281 requests to `claude-opus-5-5`, all with a rebuilt prompt
- **THEN** its row starts with `claude`, `claude-opus-5-5`, `16281` and `100%`, followed by the dollars of each component and the total, such as `$1,192` or `$0.14`

#### Scenario: Requests and prompt known
- **WHEN** a model had 200 requests with usage in the window, advisor calls included, and 150 of them had a rebuilt prompt
- **THEN** its row shows `200` requests and `75%` prompt known

#### Scenario: Order of the parts
- **WHEN** the window holds priced requests and cache misses
- **THEN** the spend by client comes after the totals and reconciliation lines, then the spend by origin and task, then the spend by trigger, the spend by activity and the setup table, all before `Cache misses by cause`

### Requirement: Reconciliation with the agent's own cost counter
When a reported session logged Claude Code's own cost counter (`totalCostUSD` in `cost-state` lines), `tokprof summary` SHALL print `Claude Code's own cost counter for <n> sessions: <usd>; tokprof for the same sessions: <usd>`: the sum of the counters of those sessions, each taken as session-logs defines it, and tokprof's priced spend for the same sessions. With `--since`, `--until` or `--client`, only sessions whose every request the filters keep are compared.

#### Scenario: All history
- **WHEN** 275 sessions logged a counter, summing to $10,241, and tokprof prices the same sessions at $10,353
- **THEN** the line reads `Claude Code's own cost counter for 275 sessions: $10,241; tokprof for the same sessions: $10,353`

#### Scenario: A session with subagent transcripts
- **WHEN** a session's main transcript and its subagent transcripts are read
- **THEN** they count as one session, with the spend of all of them

#### Scenario: A time filter
- **WHEN** `--since 7d` is given, 40 sessions with a counter have all their requests in the last 7 days and 235 more have requests before it
- **THEN** the line compares those 40 sessions only and reads `Claude Code's own cost counter for 40 sessions inside the window: <usd>; tokprof for the same sessions: <usd>`
- **AND** a session that only partly falls in the window is left out of both figures

#### Scenario: A client filter
- **WHEN** `--client sdk-ts` is given and one session with a counter also has requests with the client `cli`
- **THEN** that session is left out of both figures, as a session that only partly falls in a time window is

#### Scenario: No counter
- **WHEN** no reported session logged a counter, as with Codex sessions only
- **THEN** the line is left out

#### Scenario: No session inside the window
- **WHEN** a time filter is given and every session with a counter has requests outside the window
- **THEN** the line is left out
