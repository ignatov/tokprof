# Spec Delta

## ADDED Requirements

### Requirement: Messages the user typed
Each source SHALL keep the messages the user typed in a session's main conversation, in order, with their time and text, for the prompts block of session-xray. A typed message is one that starts a turn: not a reminder, a tool result, an interruption, a background task's result, the summary after a compaction or a shell command run from the prompt.

#### Scenario: Claude Code
- **WHEN** a main transcript has a `user` line whose text is not a reminder and holds no tool result, outside a sidechain and with no `agentId`
- **THEN** it is a typed message, and a slash command keeps its `<command-name>` and `<command-args>` as its text, such as `/goal finish the parser`

#### Scenario: Codex
- **WHEN** a main thread's `response_item` is a `message` of role `user` whose text is not injected context
- **THEN** it is a typed message

#### Scenario: Subagents
- **WHEN** a subagent's log starts with its brief
- **THEN** the brief is not a typed message of the session

## MODIFIED Requirements

### Requirement: Claude Code prompts
Claude Code prompts SHALL be rebuilt from request-audit lines (seen in Claude Code 2.1.300–2.1.312). An `api-request` keeps `keep` messages of the previous request of its stream and appends its `tail` hashes, matched to `api-request-blob` messages and to transcript `user` and `assistant` messages by the SHA-256 of their canonical JSON. Its `shapeHash` names the `api-request-shape` with the tools and system prompt. A response finds its request by `requestRef`.

#### Scenario: Transcript without request-audit lines
- **WHEN** a transcript, or the part of it written before the audit lines, has no `api-request` lines
- **THEN** its requests carry usage and model only
- **AND** cost-attribution reports their whole prompt as `unknown` content

#### Scenario: Message split across lines
- **WHEN** a response was written one content block per line under one `message.id`
- **THEN** a `tail` hash matches the merged message as well as any single line

#### Scenario: Hash not found
- **WHEN** a `tail` hash matches no blob and no transcript message
- **THEN** that message stays in the prompt as `unknown` content, which cost-attribution sizes as an average resolved message of the same prompt

#### Scenario: Request without a response
- **WHEN** an `api-request` has no assistant line with its `requestRef`, as when the request was cancelled or retried
- **THEN** it is not counted as a request
- **AND** later requests of its stream still apply their `keep` to its prompt

#### Scenario: Request parameters
- **WHEN** an `api-request` carries `params`
- **THEN** `output_config.effort`, `thinking`, `betas` (in any order), `context_management`, `speed` and the line's `version` are kept for the cause rules of cache-misses

#### Scenario: Reminders of role system
- **WHEN** an `api-request-blob` message has role `system`, as Claude Code's environment notes, hook context and file-change notes do since 2.1.300
- **THEN** its text is reminder content, labeled like a `<system-reminder>`, and it does not start a turn (see cost-attribution)

