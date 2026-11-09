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

### Requirement: Incomplete summary comparison
`tokprof summary --compare` SHALL judge a comparison as desk-window's An incomplete period before and A small agent in the period before do. An incomplete line names the agents and gives signed differences with no arrows: `compared with <dates>, incomplete (<agent> only from <day>): spend +<change> (<percent>) · …`. A small agent is named in parentheses after the dates.

#### Scenario: Claude Code's logs before 29 Sep are gone
- **WHEN** the window before is 10 Sep – 9 Oct 2026 and the store's first Claude Code request is on 29 Sep
- **THEN** the line reads `compared with 10 Sep – 9 Oct 2026, incomplete (claude only from 29 Sep): spend +$216 (+4%) · avoidable −$57.00 (−10%), −1.5 points of spend`

#### Scenario: A small new agent
- **WHEN** Codex's first request is on 5 Nov and it has 3% of the week's spend
- **THEN** the line starts `compared with 26 Oct – 1 Nov 2026 (codex only from 5 Nov, 3% of spend, not judged): spend ↑` or `↓`, judged as usual

### Requirement: The window the summary compares
`tokprof summary --compare` SHALL check, before it reads any log, that `--since` comes before `--until` and before now, and that the window before it can be written as dates. After the store is brought up to date, it prints to stderr the calendar's line about what it read and the store's warnings.

#### Scenario: Since after until
- **WHEN** the user runs `tokprof --since 2026-11-05 --until 2026-11-01 summary --compare`
- **THEN** it fails at once with `--compare needs --since before --until and before now`

#### Scenario: An unreadable store
- **WHEN** the store cannot be read
- **THEN** the comparison is made from the logs read into memory, and its warning is printed to stderr

### Requirement: Causes gone in the summary comparison
When miss causes had avoidable dollars in the window before and none in the window shown, the `compared with` line SHALL end with `; gone:` and each such cause with its dollars before, most first, comma separated.

#### Scenario: A cause gone
- **WHEN** compaction calls could have saved $367 in the window before and nothing in the window shown
- **THEN** the line ends with `; gone: compaction call $367`
