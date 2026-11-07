# Spec Delta

## ADDED Requirements

### Requirement: What to change
Under the key numbers, Reports SHALL show three cards with the rows `tokprof share` prints for the period and filters: `Could have been saved` (a row per cause with the avoidable dollars, the share of spend and the dollars per active hour), `Coming back after a cold break` (the five ways with their dollars, share of resuming and the returns each was cheapest in, the cheapest marked) and `Habits that cost the most` (cost and share, with the note that the rows overlap).

#### Scenario: The same rows as share
- **WHEN** Reports shows the last 30 days and the user runs `tokprof share --since 30d` with the same filters at the same time
- **THEN** every row of `What could have been saved`, `Coming back after a cold break` and `Habits that cost the most` shows the same name, dollars, share, dollars per active hour and cheapest count

#### Scenario: No cold returns
- **WHEN** the period has no cold return
- **THEN** the ways card reads `No cold returns to replay.`

### Requirement: Advice of a row
Under a cause, a habit and the ways back, Reports SHALL show the sentence tokprof's advice says about it: the cache-miss hint of the cause, the agents' cache lifetime settings, the thinking advice for effort and thinking, the lookup advice, the setup hint of the costliest source, a tool hint's own words, and the rhythm's advice on cold returns. A row tokprof has no advice for shows none.

#### Scenario: Returns after the cache expired
- **WHEN** `idle > TTL` is a row
- **THEN** under it reads `Compact or wrap up a big session before a long break.`

#### Scenario: A row without advice
- **WHEN** `prompts of 400k+` is a row
- **THEN** no sentence is under it

### Requirement: Sessions behind a row
A click or ⏎ on a cause or a habit SHALL open its top 5 sessions by that cost under it, each with its title and dollars, and close them again; one row is open at a time. The cache lifetime rows and the tool hints are not split by session and open nothing. A click or ⏎ on a session shows Day with it selected and its day in the lanes; a session the list does not hold says `session <id> is not in the list` in the bar.

#### Scenario: Opening a cause
- **WHEN** the user clicks `idle > TTL`
- **THEN** its 5 sessions with the most premium on returns after the cache expired show under it, most first

#### Scenario: A session
- **WHEN** the user clicks a session under a row
- **THEN** the window shows Day with that session selected and its day in the lanes

### Requirement: Keys of what to change
In Reports, Tab SHALL reach the savings, the ways back and the habits after the period and before by day. In them ↑ and ↓ (`k`, `j`), PgUp, PgDn, Home and End move the row cursor over the rows and the open sessions, and ⏎ opens or closes a row or opens a session. The key help names these keys.

#### Scenario: A session by keys
- **WHEN** the savings have the keyboard on a row with sessions and the user presses ⏎, ↓ and ⏎
- **THEN** the window shows Day with the row's first session selected

### Requirement: What to change in a frame without a window
`--scene reports-advice` SHALL draw the frame in Reports on the last 30 days, waiting up to 120 seconds, with the sessions of the first cause that has any open.

#### Scenario: The frame
- **WHEN** the user runs `tokprof-desk --snap out.png --scene reports-advice`
- **THEN** `out.png` shows what to change with the top sessions of the first such cause
