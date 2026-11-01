## MODIFIED Requirements

### Requirement: Summary totals line
After the spend table, `tokprof summary` SHALL print `Total priced: <usd>   avoidable on cache misses: <usd> (<share>%)`: the cost summed over every priced model, and the avoidable premium of the misses that the cause table lists, so expected rebuilds are left out of it as they are left out of the table (see cache-misses). The share is the premium's share of the cost, to one decimal.

#### Scenario: Expected rebuilds in the total
- **WHEN** the window holds $506 of premium on unexpected misses and $42 on compactions and cleared tool results, out of $2,628 priced
- **THEN** the line reads `Total priced: $2,628   avoidable on cache misses: $506 (19.3%)`, the same $506 the cause table below it lists

### Requirement: Reconciliation with the agent's own cost counter
When a reported session logged Claude Code's own cost counter (`totalCostUSD` in `cost-state` lines), `tokprof summary` SHALL print `Claude Code's own cost counter for <n> sessions: <usd>; tokprof for the same sessions: <usd>`: the sum of the last counter value of each such session, and tokprof's priced spend for the same sessions. With `--since` or `--until`, only sessions whose every request falls inside the window are compared.

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
