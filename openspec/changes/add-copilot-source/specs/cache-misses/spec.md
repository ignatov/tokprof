## ADDED Requirements

### Requirement: Requests without usage are not judged
A request without usage (see cost-attribution, Requests without usage) SHALL never be judged as a hit or a miss, and never be the request a later one is judged against. `tokprof misses` and `tokprof whatif` show nothing for it.

#### Scenario: A Copilot session
- **WHEN** `tokprof --agent copilot misses` runs over Copilot sessions only
- **THEN** it lists no miss, and its totals are zero
