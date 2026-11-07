## ADDED Requirements

### Requirement: Compaction calls
A request SHALL be marked as a compaction call when the agent's log names it as the request that made a compaction. In Codex that is the request whose `token_usage_record` has the `response_id` that a later `compacted` line gives as `compaction_response_id`. This was seen in Codex 0.153–0.159. Earlier rollouts log usage only as `token_count` events, which carry no response id, and 0.152 logs no `compaction_response_id`. Claude Code's compaction call is not marked.

#### Scenario: A Codex compaction call
- **WHEN** a rollout logs the summary message, then a `token_usage_record` with `response_id` `resp_b`, then a `compacted` line whose `compaction_response_id` is `resp_b`
- **THEN** the request of `resp_b` is a compaction call
- **AND** the request after the `compacted` line follows a compaction, as Compaction markers say

#### Scenario: An id no usage line has
- **WHEN** a `compacted` line has no `compaction_response_id`, or names one that no `token_usage_record` of the rollout has
- **THEN** no request is marked as a compaction call for it
