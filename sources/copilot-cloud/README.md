# tokprof-source-copilot-cloud

tokprof's source of the saved logs of the GitHub Copilot coding agent, the cloud agent that works on a repository and opens pull requests. Its agent is `copilot-cloud`, read only with `--agent copilot-cloud`: it is not in the default list. The agent `copilot` is the GitHub Copilot CLI, whose source is `sources/copilot`.

- **Where:** the agent writes no local logs. The user saves each session's log, the answer of the agent sessions API (`GET …/agents/sessions/<id>/logs`), as `<session id>.sse` or `<session id>.sse.gz`, at any depth under `$TOKPROF_COPILOT_DIR`, else under `copilot` in the profile directory (`$TOKPROF_DIR`, else `~/tokprof`). A log elsewhere is read when given as the target and its first data line is a chat chunk.
- **Record:** the session's record of the agent tasks API, `<session id>.json`, beside the log or anywhere under the same folder, whose `id` is the session id. It gives the title (`name`), the model (`model`, such as `sweagent-capi:claude-fable-5.2`, shown as `claude-fable-5-2`: after the last `:`, with the dots of a `claude-` id as dashes) and the billed credits (`usage.amount / 1e9` when `usage.type` is `ai_credits`). A session without a record has the model `unknown` and no cost.
- **Cost:** the credits at $0.01 a credit are the session's own cost, shown in `tokprof summary`, the X-ray and `tokprof sessions --cost`. A record without `usage` gives no cost, shown `–`. GitHub puts the credits of the later sessions of a task on the first session of that task (same `task_id`): the first session's amount grows after later sessions run, and those have no `usage`. tokprof shows each record as it is and moves no credits between sessions.
- **No usage:** no line of the log holds the token counts of a request. Every request is one without usage: it has its times, model, prompt and calls, and no tokens, no dollars and no cache verdict. Reports that rest on tokens show `–` or nothing for this agent.
- **Lines:** server-sent events; each `data: <JSON object>` line is a record, everything else (`data: [DONE]`, empty lines, data that is not an object) is skipped. `created` is epoch seconds as a number or a string, or epoch milliseconds with 13 digits or more.
- **Requests:** every chunk of a model response shares its `id`. The first chunk of an `id` is the request: its `content` is the response's text and its `delta.tool_calls` its calls. Its response ended at that chunk's `created`; it was sent at the latest time logged before it in its chain. A chunk whose calls are all setup steps or markers is no request. The chunks' `model` (empty or `unknown`) is not read.
- **Calls and results:** a `tool` object (`role` `tool`, `tool_call_id`) holds a call's result, and the next chunk of the same `id` that repeats the call ends it at its `created`. A call without a `tool` object takes its result from that repeat's `content`. The result's size is `toolTelemetry.metrics.result_token_count` when the log gives it (logs from 2026-11-15 on), else none.
- **Outcomes:** `toolTelemetry.properties.shell_error_category` `command_nonzero_exit`, `invalid_input` or `shell_unknown` is a failure, with the exit line when `metrics.exit_code` is there; `kill_command_blocked` is a call a safety check denied; `command_timeout` is no failure. `properties.executionMode` `async`, or `detached` `true`, is a call in the background.
- **Setup steps:** `run_custom_setup_step` (a step of the repository's `copilot-setup-steps.yml`) and `run_setup` (the clone, the start of each MCP server) before the first user message are calls no request made, of the kind environment setup, in the group `environment`. A `run_setup` whose `name` starts with `▶️ Begin subagent: `, `⏹️ End subagent: ` or `❌ Failed subagent: ` is a marker, not a step. `Clone repository <owner>/<repo>` gives the project `github.com/<owner>/<repo>`; `Start '<server>' MCP server` gives a connected MCP server, whose tools are named `<server>-<tool>`.
- **Subagents:** chunks and objects with an `agentId` are a subagent's chain in the same session, one per `agentId`, named `subagent: <name>` after the `task` call that started it. A custom agent (`selected-custom-agent-<name>`) runs the main conversation and is no subagent.
- **Prompts and turns:** the first user message of the main conversation is the typed message, at the time of the log's first chunk; later user messages come from the platform and are reminders (`source` `skill-<name>` as skills). The last response of the main conversation ends the session's one turn.
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
  | `web_search` | fetch | `query` |
  | `task`, `read_agent` | subagent | `name`, else `description`, else `agent_id` |
  | `skill` | manage | `skill` |
  | `run_custom_setup_step`, `run_setup` | environment setup | `name` |
  | `report_progress` | other; it commits and pushes | `commitMessage` |
  | `<server>-<tool>` of a started MCP server | other, in `mcp <server>` | the first string value |
  | any other, such as `codeql_checker` | other | none |

- **Not read yet:** `toolTelemetry.metrics.context_current_tokens` and `context_token_limit` (the agent's own count of its context), the `model_call_failure` objects, the phases of a session, the queue time before the first line, and the record's `state`, `created_at` and `completed_at`.

The log names no agent version; the fields above were seen in logs of 2026-11-14 to 2026-11-21. `tests/fixtures` holds synthetic logs: a first session of a task with setup steps, a custom agent, a failed and a passed test run and a commit, with credits on its record; a later session of the task whose record has no usage; and a plain log without a record, with a `task` subagent, a command in the background, a blocked command and a call without a `tool` object.
