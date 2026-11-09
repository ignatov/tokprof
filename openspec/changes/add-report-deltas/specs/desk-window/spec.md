# Spec Delta

## ADDED Requirements

### Requirement: Key numbers against the period before
Under each key number, Reports SHALL compare it with the period of the same length just before the one shown, from the day store (see activity-calendar). The comparison reads `<arrow> <change> (<percent>)`, or `= <zero>` when the number did not change, and the card's title notes `against <first day> – <last day>`. The avoidable dollars, their share of the spend in points, and the cold returns are green when they fall and red when they rise. Spend, sessions and requests have no colour.

#### Scenario: More spend
- **WHEN** the last 30 days cost $12,203 and the 30 days before cost $11,000, from 10 Sep to 9 Oct
- **THEN** the spend reads `↑ $1,203 (+11%)` in the text's colour, and the card notes `against 10 Sep – 9 Oct 2026`

#### Scenario: Fewer misses
- **WHEN** the avoidable dollars fell from $640 to $597, and their share from 12.5% to 11.3% of spend
- **THEN** the line under them reads `↓ $43.00 (−7%) · ↓ 1.2 points` in green

#### Scenario: More cold returns
- **WHEN** the cold returns rose from 90 to 106
- **THEN** their line reads `↑ 16 (+18%)` in red, comparing the count only

#### Scenario: No earlier data
- **WHEN** the store holds no request, miss or cold return in the period before
- **THEN** no number has a comparison, and the card's title has the note `no earlier data`

#### Scenario: A client filter
- **WHEN** the window runs with `--client`
- **THEN** no number has a comparison, and the note reads `no comparison with --client`

### Requirement: Causes against the period before
Each row of `Could have been saved` SHALL give, after its dollars, the cause's avoidable dollars in the period before and the change in percent: `was <dollars> · <arrow> <percent>`. A cause with no dollars before reads `new`. The change is green when it falls and red when it rises. With no earlier data, or with `--client`, the rows have no comparison.

#### Scenario: Fewer idle misses
- **WHEN** `idle > TTL` could have saved $796, and $1,012 in the period before
- **THEN** the row reads `idle > TTL` `$796` `was $1,012 · ↓ 21%`, the change in green

#### Scenario: A new cause
- **WHEN** `tools changed` could have saved $12 and had nothing in the period before
- **THEN** its row reads `new`
