# tokprof-source-claude-code

tokprof's source of Claude Code transcripts.

- **Where:** `$CLAUDE_CONFIG_DIR/projects/<project>/<session>.jsonl`, by default under `~/.claude`. A session's subagents are in `<session>/subagents/agent-<id>.jsonl`, with what started each in `agent-<id>.meta.json` (`agentType`, `description`, `toolUseId`).
- **Requests:** `assistant` lines with `message.usage` (`input_tokens`, `cache_read_input_tokens`, `cache_creation_input_tokens`, `cache_creation.ephemeral_5m_input_tokens` and `ephemeral_1h_input_tokens`, `output_tokens`, `iterations` for advisor calls). Since 2.1.300 the prompts come from the request-audit lines `api-request`, `api-request-shape` and `api-request-blob`. Blobs of role `system` are Claude Code's reminders (environment, hook context, file-change notes), not prompts; seen in 2.1.300–2.1.312.
- **Session facts:** `sessionId` and `cwd` from the first lines, `compact_boundary` with `compactMetadata.trigger`, the idle-compaction notice seen in 2.1.302–2.1.310, `cost-state` (`totalCostUSD`, `startTime`, both seen in 2.1.284–2.1.312: a resumed process restores the counter with its `startTime`, and a process that found no `cost-state` counts from zero under a new one, so the counter is the sum of the highest `totalCostUSD` per `startTime`), `ai-title` and `custom-title`.
- **Triggers:** the `tool_use` items of each response, grouped by `message.id` (the tool `name`, and `input.command` for `Bash`), and the user lines after them: typed prompts and slash commands, `<task-notification>` and `[Request interrupted`. Seen in 2.1.284–2.1.312.
- **Handoff note:** user prompts, the `TodoWrite` list, `Edit`, `Write`, `MultiEdit` and `NotebookEdit` file paths, `Read`, and `Bash` commands.
- **API:** every request goes to the Anthropic API.

The fields and versions are the ones the session-logs spec names; `tests/fixtures` holds a synthetic transcript with a subagent and a compaction.
