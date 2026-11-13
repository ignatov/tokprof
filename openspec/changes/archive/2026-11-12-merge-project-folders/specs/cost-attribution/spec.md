## MODIFIED Requirements

### Requirement: Dimensions of spend
Every share of spend SHALL carry the dimensions that profiles stack by (see profile-export): agent (`claude` or `codex`), project (the session's project, see session-logs), session (its short id, see session-logs), chain (its chain per session-logs, or `advisor` for advisor calls), model (as logged), turn, component, cause (on `rewrite after miss` only), kind, tool (the tool's name) and target (what the tool worked on, or a reminder's source).

#### Scenario: Content kinds
- **WHEN** spend is broken down by kind
- **THEN** the kinds are `tool schemas`, `system prompt`, `user prompt`, `reminders`, `tool results`, `assistant text`, `thinking`, `tool calls`, `images` and `unknown`

#### Scenario: A tool result
- **WHEN** a Claude Code `Read` of `src/main.rs` in the project returns a result
- **THEN** its spend has kind `tool results`, tool `Read` and target `src/main.rs`, and a cause only on its `rewrite after miss` share

#### Scenario: A reminder
- **WHEN** Claude Code injects the CLAUDE.md instructions as a system reminder
- **THEN** its spend has kind `reminders`, target `instructions` and no tool

#### Scenario: An advisor call
- **WHEN** a response consulted the advisor, which reads the conversation with another model
- **THEN** the advisor request is charged to chain `advisor` under the advisor's model, apart from the conversation's own chain

#### Scenario: Sessions in a worktree and in a subfolder
- **WHEN** one session ran in `~/.branch-deck/pool/app-c821e1d7/4/app`, a worktree of `~/src/app`, and another in `~/src/app/plugins/web`
- **THEN** the spend of both has the project `~/src/app`, and a flame graph stacked by project shows both in the frame `~/src/app`

#### Scenario: Temporary folders
- **WHEN** 90 sessions ran in folders under `/private/var/folders/19/nlk_x/T`, the system's temporary folder
- **THEN** a flame graph stacked by project shows their spend in one frame `temporary folders`
