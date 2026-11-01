## MODIFIED Requirements

### Requirement: Prompts in the note
The note SHALL take as prompts only what a person typed. For Claude Code these are `user` lines not flagged `isMeta`, `isCompactSummary` or `isSidechain`, with a slash command shown as its name and arguments and any other text that starts with `<` or `[Request interrupted` left out. For Codex they are `user` messages in `response_item` lines whose text does not start with `<` or `# AGENTS.md`.

#### Scenario: Slash command
- **WHEN** a Claude Code user line holds `<command-name>/model</command-name><command-args>opus</command-args>`
- **THEN** the prompt is `/model opus`

#### Scenario: Injected text
- **WHEN** a Claude Code user line holds only `<system-reminder>…</system-reminder>`
- **THEN** it is not a prompt

#### Scenario: Tool results
- **WHEN** a Claude Code user line carries only tool results
- **THEN** it is not a prompt

#### Scenario: Codex instructions
- **WHEN** a Codex user message starts with `# AGENTS.md` or with a tag such as `<environment_context>`
- **THEN** it is not a prompt

#### Scenario: An interruption marker
- **WHEN** a Claude Code user line holds `[Request interrupted by user]` or `[Request interrupted by user for tool use]`
- **THEN** it is not a prompt
