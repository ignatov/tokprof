# tokprof-source-copilot

tokprof's source of the session logs of the GitHub Copilot CLI. Its agent is `copilot`, in the default list. The saved logs of the cloud coding agent are another agent, `copilot-cloud`, whose source is `sources/copilot-cloud`.

- **Where:** `<session id>/events.jsonl` in `$COPILOT_HOME/session-state`, by default `~/.copilot/session-state`. A session folder without `events.jsonl` is no session. A log elsewhere is read when given as the target and its first line is a `session.start` line whose `data.producer` is `copilot-agent`.
- **Lines:** one JSON object per line, with `type`, `data`, `timestamp` (ISO 8601 with milliseconds) and, on a subagent's lines, `agentId`. A line of a type the source does not read is skipped before its body is parsed.
- **Session:** the first `session.start` line gives the id (`data.sessionId`, else the folder's name), the project (`data.context.cwd`) and the CLI version (`data.copilotVersion`) of every request. The title is the `name` of the `workspace.yaml` beside the log, a file of plain `key: value` lines.
- **Requests:** each `session.usage_record` line is one request. Of its `data.usage`: `inputTokens` is the whole prompt, so uncached input is `inputTokens − cacheReadTokens − cacheWriteTokens`, never below 0; `cacheReadTokens` are reads; `cacheWriteTokens` are writes; `outputTokens` is output, `reasoningTokens` included. `model` is the model, with the dots of a `claude-` id as dashes (`claude-fable-5.2` is `claude-fable-5-2`). The response ended at the line's `timestamp` and was sent `duration` ms before. Writes are long-lifetime when `cacheTtlSeconds`, else `cacheExpiresAt` less the line's `timestamp`, is over 1,800 s; every record seen says about 300 s. `apiEndpoint` `/v1/messages` is Anthropic's API.
- **Responses:** the `assistant.message` line whose `data.apiCallId` is the record's `accounting.usageId` is the request's response: `content` its text, `reasoningText` its thinking, `toolRequests` its calls. The record comes right before it. A record whose response is not in the log is a request all the same.
- **Without usage:** an `assistant.message` line with an `apiCallId` and no record (every response of Copilot CLI 1.0.85) is a request without usage: it has its times (sent at the `assistant.turn_start` line before it), its model (`data.model`) and its calls, and no tokens, no dollars and no cache verdict. An `assistant.message` line without an `apiCallId` is no request: the CLI wrote it itself, to read a finished background task, and its calls are calls no request made. Their results are not counted in a prompt.
- **Own cost:** `copilotUsage.totalNanoAiu / 1e9` of a record are the credits GitHub billed for the request, at $0.01 a credit. A session's own cost is the sum over its records and compaction calls, subagents included; `tokprof summary` compares it with tokprof's cost as `GitHub's billed credits`. `session.usage_checkpoint` and `session.shutdown` carry the same totals and are not read. With the built-in prices of `claude-fable-5-2`, `claude-opus-5-5` and `claude-sonnet-5-5`, tokprof's cost of every request seen equals its credits.
- **Compaction:** a `session.compaction_complete` line with `success` `true` marks the next request of the main conversation as one after a compaction; its `summaryContent` replaces the conversation. Its `compactionTokensUsed`, shaped like `usage`, is a compaction call.
- **Calls:** a tool request (`toolCallId`, `name`, `arguments`) starts at the `timestamp` of the `tool.execution_start` line with its `toolCallId` and ends at that of its `tool.execution_complete` line, whose `data.result.content` is its result.
- **Outcomes:** `data.success` `false` with `data.error.code` `rejected` is a call the user refused, with `denied` one a permission rule denied, and with any other code a failure, with `data.error.message` as its error line. `success` `true` with a `data.shellExecution.exitCode` other than 0 is a failure too. `toolTelemetry.properties.executionMode` `async`, or `detached` `true`, is a call in the background. An `ask_user` call asks the user.
- **MCP tools:** a call whose `tool.execution_start` line has `mcpServerName` is the tool `<server>-<mcpToolName>` of that server. The log does not list the servers a session had.
- **Subagents:** the lines with an `agentId` are a subagent's chain in the same session, named `subagent: <name>` after `data.agentDisplayName`, else `data.agentName`, of its `subagent.started` line. Its `user.message` is its brief.
- **Prompts and turns:** a `user.message` line without an `agentId` and without a `data.source` is a typed message, sized by `transformedContent`, what the model read; with `delivery` `steering` it was typed while the agent worked. One with a `source` (`memory`, `autopilot`, `agent-<session id>`) comes from the CLI and is a reminder. A response of the main conversation without calls ends the agent's turn.
- **Kinds and input heads:**

  | tool | kind | input head |
  |---|---|---|
  | `bash` | execute | `command` (first line); its target is the command head |
  | `read_bash` | execute output | `shellId` |
  | `stop_bash` | execute | `shellId` |
  | `view` | read | `path` |
  | `edit` | edit | `path` |
  | `create` | write | `path` |
  | `grep`, `glob` | search | `pattern` |
  | `web_fetch` | fetch | `url` |
  | `task`, `read_agent`, `list_agents` | subagent | `name`, else `description`, else `agent_id` |
  | `skill` | manage | `skill` |
  | `ask_user`, `vote_memory` | manage | none |
  | `<server>-<tool>` of an MCP server | other, in `mcp <server>` | the first string value |
  | any other, such as `task_complete` | other | none |

- **Not read yet:** `maxPromptTokens` (the context limit), the `trigger` of `session.compaction_start`, `toolTelemetry.metrics.result_token_count`, `session.model_change`, `session.tool_baseline`, `system.message`, `permission.*`, `skill.*`, `model.*`, and `session-store.db`, the CLI's own index of sessions.

The fields above were seen in Copilot CLI 1.0.85, 1.0.106, 1.0.115 and 1.0.116; the usage records in 1.0.106–1.0.116. `tests/fixtures` holds synthetic logs: a session of two models with a subagent, a compaction, a failed, a refused and a denied call, a command in the background, a message the CLI wrote itself and a `workspace.yaml`; a session of an older CLI without usage records and without a `workspace.yaml`; and a session folder without a log.
