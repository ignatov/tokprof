# Spec Delta

## ADDED Requirements

### Requirement: Waiting option
`tokprof ui --waiting` SHALL add the waiting order to the list (see Sort) and show waits in it (see Waiting rows). That order puts the sessions that wait for the user first, the longest wait first, then the others by time. Without `--waiting`, `s` cycles the three other orders and no row shows a wait. The footer names the orders `s` cycles through.

#### Scenario: Without the option
- **WHEN** the user runs `tokprof ui` and presses `s` three times
- **THEN** the list is in time order again, and the footer reads `s sort: time, cost, next`

#### Scenario: With the option
- **WHEN** the user runs `tokprof ui --waiting`
- **THEN** the footer reads `s sort: time, cost, next, waiting`, and the list still starts in time order

### Requirement: Waiting rows
With the waiting option, `tokprof ui` SHALL work out, when it costs a session, whether it waits for the user now and since when (see work-rhythm, Waiting now). In waiting order, a waiting row shows `waits <span>` in place of its time, the span as the X-ray's (see session-xray), counted from the end of turn to now and redrawn when it changes. Once the wait ends, or the waiting limit passes, the row shows its time again and keeps its place.

#### Scenario: A waiting row
- **WHEN** the list is by waiting and a session's agent ended its turn 16 minutes ago
- **THEN** its row reads `waits 16m` where the other rows show `MM-DD HH:MM`

#### Scenario: The user answers
- **WHEN** the list is by waiting with 3 sessions waiting, and the user types in one of them
- **THEN** once the session is costed again its row shows its time and stays in its place, and the list title reads `by waiting, 2 wait, r re-sorts`

#### Scenario: Another order
- **WHEN** the list is by time
- **THEN** no row shows a wait

## MODIFIED Requirements

### Requirement: Sort
`s` SHALL cycle the order of the list from time (latest last activity first, the order it starts in) to cost (most expensive session first) to next request (dearest next request now first), with the waiting option to waiting (see Waiting option), and back to time, and sort the list in the new order. Sessions whose value is not known yet go last, and the selected session stays selected. Values that arrive later move no row (see Rows in place).

#### Scenario: By cost
- **WHEN** `s` is pressed once
- **THEN** the most expensive session is on top, the sessions still being costed are at the bottom, and the list title reads `by cost`

#### Scenario: By next request
- **WHEN** `s` is pressed twice
- **THEN** the session whose next request would cost the most now is on top, and the list title reads `by next request`

#### Scenario: By waiting
- **WHEN** `tokprof ui --waiting` runs, `s` is pressed three times, and three sessions wait, for 40m, 16m and 2m
- **THEN** the three are on top in that order, the other sessions follow newest first, the sessions still being costed are at the bottom, and the list title reads `by waiting, 3 wait`
- **AND** with no session waiting the title reads `by waiting, none waits` and the list is in time order

#### Scenario: The top of the new order
- **WHEN** the list is scrolled down, the selected session is the second in the new order, and `s` is pressed
- **THEN** the list shows from its first row, with the selected session second

#### Scenario: Back to time
- **WHEN** `s` is pressed three times, or four times with the waiting option
- **THEN** the list is newest first again
