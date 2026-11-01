## MODIFIED Requirements

### Requirement: Default file name follows the filters
The default file name SHALL be built from the global filters (session-logs), joined with `-`, then `.jfr`: the agent when exactly one is given; the `--session` value; the last segment of `--project` as session-logs reads it; `all` when neither `--session` nor `--project` is given; `since-<value>`; `until-<value>`; and `tokens` for `--value tokens` or `misses` for `--value avoidable`. In each part, characters other than ASCII letters, digits and `.` become `-`, trimmed at both ends.

#### Scenario: All sessions
- **WHEN** the user runs `tokprof flame`
- **THEN** the file is `all.jfr`

#### Scenario: One session
- **WHEN** the user runs `tokprof --session 3f951de0 flame`
- **THEN** the file is `3f951de0.jfr`

#### Scenario: A project over the last week
- **WHEN** the user runs `tokprof --project ~/src/billing --since 7d flame`
- **THEN** the file is `billing-since-7d.jfr`

#### Scenario: One agent's avoidable cost
- **WHEN** the user runs `tokprof --agent codex flame --value avoidable`
- **THEN** the file is `codex-all-misses.jfr`

#### Scenario: Tokens until a date
- **WHEN** the user runs `tokprof --until 2026-10-16 flame --value tokens`
- **THEN** the file is `all-until-2026-10-16-tokens.jfr`

#### Scenario: A project with a trailing slash
- **WHEN** the user runs `tokprof --project ~/src/billing/ --since 7d flame`
- **THEN** the file is `billing-since-7d.jfr`
