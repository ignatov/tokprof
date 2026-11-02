## MODIFIED Requirements

### Requirement: Reconciliation with the agent's own cost counter
When a reported session logged Claude Code's own cost counter (`totalCostUSD` in `cost-state` lines), `tokprof summary` SHALL print `Claude Code's own cost counter for <n> sessions: <usd>; tokprof for the same sessions: <usd>`: the sum of the counters of those sessions, each taken as session-logs defines it, and tokprof's priced spend for the same sessions. With `--since` or `--until`, only sessions whose every request falls inside the window are compared.

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

#### Scenario: No counter
- **WHEN** no reported session logged a counter, as with Codex sessions only
- **THEN** the line is left out

#### Scenario: No session inside the window
- **WHEN** a time filter is given and every session with a counter has requests outside the window
- **THEN** the line is left out
