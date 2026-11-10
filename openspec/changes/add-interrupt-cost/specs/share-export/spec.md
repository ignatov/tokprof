## ADDED Requirements

### Requirement: Turn parts
The file SHALL hold `turns`, the person's turns with all agents together inside the window, as turn-outcomes counts them, over the sessions the rhythm keeps. It holds `ends`, per end kind, with `count`, `usd`, `median_ms` and `next`, the counts by what came after; and `session_ends`, per end kind, the sessions that ended after that kind of turn.

#### Scenario: One session
- **WHEN** a session has 3 `changed` turns for $9 and 1 `stopped` turn for $2, and the session ended after a `changed` turn
- **THEN** `turns.ends.changed` has `count` 3 and `usd` 9, and `turns.session_ends.changed` is 1

#### Scenario: Shares
- **WHEN** `tokprof share` writes the file without `--dollars`
- **THEN** every `usd` of `turns` is a share of the file's spend, as for every other amount, and the counts stay

### Requirement: Stopped turns and runs in the file
`turns` SHALL also hold `stopped`, with `count`, `usd`, `median_ms_to_stop`, `next`, and `by_class`, the `count` and `usd` per class of the last call; `runs`, per length range, with `count`, `usd`, `edits`, `commits` and `stopped`; and `goal`, with `count`, `usd` and `edits`.

#### Scenario: Stopped during tests
- **WHEN** the person stopped a turn for $2 during a call of class `building and testing`
- **THEN** `turns.stopped.by_class` has `building and testing` with `count` 1 and `usd` 2

### Requirement: Words of the turn parts
The keys of `turns` SHALL be tokprof's own words (see Own words): the part and field names, the end kinds, the ranges and the call classes. The file holds no text of any message.

#### Scenario: Nothing new outside the own words
- **WHEN** a file with `turns` is written
- **THEN** its words outside tokprof's own are the same as those of the file without it
