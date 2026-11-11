## MODIFIED Requirements

### Requirement: Live updates
While it runs, `tokprof ui` SHALL follow the agents' log directories and stores: a new session joins the list, a session whose main or subagent logs change is costed and X-rayed again, and a session whose main log is deleted or moved away leaves the list. A session in a store that holds many is its own row, and the other sessions of the store keep theirs. Until the new X-ray is ready, the old one stays, with `· updating…` in its title. Where rows go when this happens belongs to Rows in place.

#### Scenario: A new session
- **WHEN** an agent starts a new session
- **THEN** it shows up where the current order puts it, at the top of the list sorted by time, and the selection stays on the session it was on

#### Scenario: A growing session
- **WHEN** the selected session's log grows
- **THEN** its cost and time update, the X-ray title reads `· updating…` until the new X-ray replaces the old one, and the session keeps its row

#### Scenario: A deleted session
- **WHEN** a session's main log is deleted
- **THEN** the session leaves the list, and if it was selected, the cursor stays on the same row

#### Scenario: Dropped change events
- **WHEN** the file system reports that it dropped change events
- **THEN** the ui scans every log directory again, removes the sessions that are gone and adds the ones that are there, and the sessions still listed keep their rows

#### Scenario: A directory created later
- **WHEN** a log directory such as `~/.codex/archived_sessions` does not exist yet
- **THEN** its parent is followed, and its sessions show up once it is created

#### Scenario: Busy logs
- **WHEN** an agent writes its log many times a second
- **THEN** changes are gathered for 500 ms, and a session that keeps changing is costed or X-rayed again only after as long as its last costing or X-ray took, so it uses at most about half a core

#### Scenario: The logs cannot be followed
- **WHEN** the log directories cannot be watched
- **THEN** the ui runs without live updates and the footer reads `not following the logs: <reason>`

#### Scenario: A session deleted from its store
- **WHEN** OpenCode deletes session `S` from a store that holds 10 other sessions, or the store itself is deleted
- **THEN** `S` leaves the list, or every session of the store does, and the others keep their rows

#### Scenario: A store that cannot be read for a moment
- **WHEN** a store is locked or cannot be opened while the ui looks at it
- **THEN** its sessions stay listed as they were

#### Scenario: An OpenCode session at work
- **WHEN** OpenCode writes a step of session `S` to its store's `-wal` file, and the store holds 10 other sessions
- **THEN** within about 2 s `S`'s cost and time update, and the 10 other sessions keep their rows and costs
