# Spec Delta

## ADDED Requirements

### Requirement: Summary against the period before
`tokprof summary --since <date> --compare` SHALL print, after the `Total priced` line, `compared with <first day> – <last day>:`. Then `spend <arrow> <change> (<percent>)` and `avoidable <arrow> <change> (<percent>), <arrow> <points> points of spend`, against the window of the same length just before the one shown. The window ends at `--until`, or now. The numbers come from the day store (see activity-calendar), brought up to date first. Without `--compare`, the summary is unchanged.

#### Scenario: A month against the month before
- **WHEN** the user runs `tokprof summary --since 2026-10-10 --until 2026-11-09 --compare`, the month cost $5,500 with $540 avoidable, and the 30 days before cost $5,284 with $597 avoidable
- **THEN** after `Total priced` it prints `compared with 10 Sep – 9 Oct 2026: spend ↑ $216 (+4%) · avoidable ↓ $57.00 (−10%), ↓ 1.5 points of spend`

#### Scenario: No earlier data
- **WHEN** the store holds nothing of the window before
- **THEN** the line reads `compared with <first day> – <last day>: no earlier data`

#### Scenario: All of history
- **WHEN** the user runs `tokprof summary --compare` without `--since`
- **THEN** it fails with `--compare needs --since: all of history has nothing before it`

#### Scenario: A client filter
- **WHEN** the user runs `tokprof summary --since 30d --client sdk --compare`
- **THEN** it fails with `--compare cannot filter by client; drop --client`

#### Scenario: Without the flag
- **WHEN** the user runs `tokprof summary --since 30d`
- **THEN** the output is byte for byte what it was before this option
