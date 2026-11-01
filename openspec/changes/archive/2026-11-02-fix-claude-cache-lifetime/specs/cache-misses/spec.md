## MODIFIED Requirements

### Requirement: Idle beyond the cache lifetime
The cause SHALL be `idle > TTL` when more time passed since the previous request than the cache lifetime. For Claude Code the lifetime is 1 hour when the chain's latest request that wrote to the cache, up to the previous one, wrote more tokens with the 1-hour lifetime than with the 5-minute one, else 5 minutes; for Codex it is assumed to be 30 minutes. The detail reads `<idle time> idle, <lifetime> TTL`, the idle time in seconds, minutes, or hours or days to one decimal.

#### Scenario: The 1-hour cache
- **WHEN** the previous request wrote 30,000 tokens with the 1-hour lifetime and 2,000 with the 5-minute one, and the next request comes 3 hours 18 minutes later
- **THEN** its cause reads `idle > TTL: 3.3h idle, 1h TTL`

#### Scenario: The 5-minute cache of a subagent
- **WHEN** a subagent's previous request wrote only 5-minute cache tokens and its next request comes 6 minutes later
- **THEN** its cause reads `idle > TTL: 6m idle, 5m TTL`

#### Scenario: Codex
- **WHEN** a Codex request comes 45 minutes after the previous one and misses
- **THEN** its cause reads `idle > TTL: 45m idle, 30m TTL`

#### Scenario: Exactly the lifetime
- **WHEN** a request comes exactly 5 minutes after a request that wrote with the 5-minute lifetime
- **THEN** idleness is not its cause, and the later checks decide

#### Scenario: Days
- **WHEN** a request comes 26.4 hours after the previous one
- **THEN** its detail reads `1.1d idle`

#### Scenario: A previous request that wrote nothing
- **WHEN** a main conversation writes with the 1-hour lifetime, its previous request read everything from the cache and wrote nothing, and the next request comes 20 minutes later and misses
- **THEN** the lifetime is still 1 hour, so `idle > TTL` is not the cause and the later checks decide

#### Scenario: No write yet
- **WHEN** no request of the chain has written to the cache yet
- **THEN** the lifetime is 5 minutes
