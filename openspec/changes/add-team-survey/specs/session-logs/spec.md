# Spec Delta

## ADDED Requirements

### Requirement: Agent version of a request
Each request SHALL carry the version of the agent that wrote its log, when the log names one. For Claude Code, this is the `version` of the request's line, which Claude Code requests and usage already keeps (seen in 2.1.284–2.1.316). For Codex, it is the `cli_version` of the rollout's first `session_meta` payload, for every request of that rollout (seen in 0.147–0.166).

#### Scenario: A Codex rollout
- **WHEN** a rollout's first line is a `session_meta` whose payload has `cli_version` `0.166.0-alpha.5.1`
- **THEN** every request of that rollout carries `0.166.0-alpha.5.1`

#### Scenario: A later session_meta
- **WHEN** a rollout holds a second `session_meta` with another `cli_version`
- **THEN** its requests still carry the first one's version
- **AND** no Codex miss gets the cause `agent updated` (see cache-misses) from it

#### Scenario: A log without a version
- **WHEN** a Codex `session_meta` has no `cli_version`
- **THEN** the rollout's requests carry no version
