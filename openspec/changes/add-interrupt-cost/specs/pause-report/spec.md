## ADDED Requirements

### Requirement: How your turns end
After the rhythm section, `tokprof pauses` SHALL print the section `How your turns end`, over the turns of the sessions the rhythm keeps, as turn-outcomes counts them. Its first table has one row per end kind, the most turns first, with `turns`, `share`, `cost`, `of spend`, `median turn`, and the shares of what came next: `under 5m`, `5–60m`, `1h+` and `none`. The tables `Stopped turns` and `Run length` follow it.

#### Scenario: The section
- **WHEN** `tokprof --since 30d pauses` runs
- **THEN** `How your turns end` follows the rhythm section's hour table, and its `stopped` turns are the rhythm's interrupts that ended a turn

#### Scenario: No turns
- **WHEN** the rhythm section prints `No messages in this time.`
- **THEN** there is no `How your turns end`

### Requirement: Stopped turns table
The table `Stopped turns` SHALL have one row per class of the last call before the stop, the most turns first, with `turns`, `cost`, `median until the stop` and `typed again in 5m`, the share of those turns followed by a message within 5 minutes. With no stopped turn, the table is the line `No turn was stopped.`

#### Scenario: Stopped during tests
- **WHEN** 5 turns were stopped during calls of class `building and testing`, 4 of them followed by a message within 5 minutes
- **THEN** that row shows `5` under `turns` and `80%` under `typed again in 5m`

### Requirement: Run length table
The table `Run length` SHALL have one row per length range of turn-outcomes, in order, with `turns`, `cost`, `of spend`, `cost per edit call`, `commits` and `stopped`, then a `goal runs` row when there are goal runs. `cost per edit call` is the range's dollars over its edit calls, and `–` when it has none.

#### Scenario: Run length
- **WHEN** turns of 15m–1h cost $300 for 750 edit calls, and turns of 1–5m cost $140 for 140
- **THEN** their rows show `$0.40` and `$1.00` under `cost per edit call`
