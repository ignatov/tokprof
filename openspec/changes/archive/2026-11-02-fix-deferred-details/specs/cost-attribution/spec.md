## MODIFIED Requirements

### Requirement: Session title
A session's title SHALL be:
- for Claude Code, the latest `custom-title` line, else the latest `ai-title` line, without a leading `[archived]` mark;
- for Codex, the `thread_name` of the thread in `~/.codex/session_index.jsonl` (under `$CODEX_HOME` when set), and for a subagent thread the title of the session it belongs to (see session-logs), however deep it is.

tokprof SHALL read the index again when its size changes.

#### Scenario: The user renamed a session
- **WHEN** a transcript has `ai-title` lines and then `"customTitle": "[archived] Mine"`, followed by another `ai-title`
- **THEN** the title is `Mine`

#### Scenario: A Codex thread from an IDE plugin
- **WHEN** the index has no entry for a thread
- **THEN** the thread has no title

#### Scenario: A thread spawned by a subagent
- **WHEN** main thread `M` is titled `Fix the parser` in the index, its subagent thread `A` spawned thread `B`, and neither `A` nor `B` is in the index
- **THEN** `A` and `B` both have the title `Fix the parser`
