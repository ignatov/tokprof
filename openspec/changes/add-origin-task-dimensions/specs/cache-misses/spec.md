# Spec Delta

## MODIFIED Requirements

### Requirement: Each chain is its own cache lineage
tokprof SHALL judge a request only against the previous request of the same chain (the main conversation, a subagent or a side stream, see session-logs) that got a response; the first request of a chain is never a miss. Advisor calls are never judged and never serve as the previous request. A request before `--since` still serves as the previous request of the first one in the window.

#### Scenario: A subagent's first request
- **WHEN** a subagent starts and writes its 30,000-token prompt to the cache
- **THEN** that request is not a miss, whatever the main conversation's cache holds
- **AND** later misses of the subagent are listed with its chain, such as `subagent: Find handlers` for a subagent described as `Find handlers`, or `subagent:general-purpose` when it has no description

#### Scenario: A cancelled request
- **WHEN** a request got no response and logged no usage
- **THEN** it is not judged, and the next request is compared with the one before it

#### Scenario: An advisor call
- **WHEN** a response consulted the advisor, which reads the conversation with another model
- **THEN** the advisor request is not a miss, and the next request of the conversation is compared with the one before the advisor call

#### Scenario: The start of a time window
- **WHEN** `--since` starts the window in the middle of a chain
- **THEN** the first request in the window is compared with the last one before it
