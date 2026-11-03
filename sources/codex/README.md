# tokprof-source-codex

tokprof's source of Codex rollouts.

- **Where:** `$CODEX_HOME/sessions/YYYY/MM/DD/rollout-*.jsonl` and `$CODEX_HOME/archived_sessions/`, by default under `~/.codex`; `.jsonl.zst` when Codex compressed a rollout. Thread names come from `$CODEX_HOME/session_index.jsonl` (`id`, `thread_name`).
- **Requests:** `token_usage_record` lines deduplicated by `response_id`; older rollouts only have `event_msg` `token_count` events, whose `last_token_usage` counts when the total changed.
- **Prompts:** `session_meta.base_instructions`, every `response_item`, and `compacted` with its `replacement_history`.
- **Threads:** a subagent thread names its parent in `session_meta.parent_thread_id`, and belongs to the session of the topmost thread above it.
- **Triggers:** `function_call` and `custom_tool_call` items: `exec_command` and `shell` commands, `apply_patch`, `spawn_agent`, and the `tools.<name>(...)` calls inside the JavaScript of `exec`, with string-literal `cmd:` values; user messages, and assistant messages with no call. Seen in 0.146–0.164.
- **Hand-offs:** only in a main thread, not in one with `session_meta.parent_thread_id`. An `event_msg` of type `task_complete` ends the turn, and one of type `turn_aborted` is an interrupt, at the line's `timestamp`. Seen in 0.147–0.166. Codex logs no questions, wakes or messages typed while it works.
- **Sessions a program ran:** `session_meta.originator` is `codex_exec`, as `codex exec` writes it. Seen in 0.164.0.
- **Handoff note:** user and assistant messages, `update_plan`, `exec_command` and `shell` commands, and `apply_patch` file paths, also inside the JavaScript of the `exec` tool.
- **API:** every request goes to the OpenAI API; Codex's configured provider is not read yet.
- **Setup hints:** `AGENTS.md` for instructions. Codex logs no tool schemas other than `additional_tools`, so it gives no hint for them.

The fields are the ones the session-logs spec names; `tests/fixtures` holds a synthetic rollout with an end of turn and an interrupt, and a compressed child thread.
