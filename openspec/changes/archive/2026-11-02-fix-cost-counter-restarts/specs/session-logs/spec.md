## MODIFIED Requirements

### Requirement: Agent-reported cost
tokprof SHALL take a Claude Code session's own cost counter from the `cost-state` lines in its transcript (seen in transcripts of Claude Code 2.1.284–2.1.312). The lines that share a `startTime` are one run of the counter, and lines without a `startTime` are one run together. The counter is the sum, over the runs, of the highest `totalCostUSD` in each. cost-attribution compares it with tokprof's total in `tokprof summary`. Codex logs no such figure.

#### Scenario: Several cost-state lines
- **WHEN** a transcript's `cost-state` lines share one `startTime` and read $10.00, $30.00 and $28.00
- **THEN** the session's reported cost is $30.00

#### Scenario: A run that started from zero
- **WHEN** a transcript has `cost-state` lines with one `startTime` up to $24.06, then lines with another `startTime` from $2.66 up to $32.89
- **THEN** the session's reported cost is $56.95

#### Scenario: Runs that interleave
- **WHEN** two processes wrote to one transcript at once, and its `cost-state` lines read $1.34 under one `startTime`, $1.41 under another, then $24.70 under the first
- **THEN** the session's reported cost is $26.11

#### Scenario: No cost-state line
- **WHEN** a transcript has no `cost-state` line
- **THEN** the session has no reported cost and is left out of the comparison
