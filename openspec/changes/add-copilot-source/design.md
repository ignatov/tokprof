# Design

## Log fields

The log is the answer of the agent sessions API (`GET …/agents/sessions/<id>/logs`), saved as it came, often gzipped. The log names no agent version, so the fields below name the dates of the logs they were seen in: 867 logs from 2026-11-14 to 2026-11-21.

- **Lines:** server-sent events. A useful line is `data: <one JSON object>`. Empty lines (70,421), `data: [DONE]` and data that is not a JSON object are skipped. No other line kind was seen.
- **Chat chunks** (51,959): `id`, `created`, `model`, `object` (`chat.completion.chunk`), `choices` (always one), sometimes `agentId`. A choice has `finish_reason` (`tool_calls`, `stop` or none), `index` and `delta` with `role` (`assistant`), `content` and `tool_calls`. A tool call has `id`, `function.name`, `function.arguments` (a JSON string), and in newer chunks `type` (`function`) and `index`.
- **`created`:** epoch seconds, as a number or a string; 1,733 chunks of MCP server starts give epoch milliseconds (13 digits). The value is the time the line was written. After the units are read right, the only times out of order are MCP server starts that run side by side (645 chunks).
- **`model`:** empty or `unknown` in every chunk. The model comes from the record.
- **One response, several chunks.** All chunks of one model response share its `id`. The first chunk holds the response: its text in `content` and all its calls. Then, for each call that returned, a chunk with the same `id` holds that one call again, with the result as `content`. So the second chunk's `created` is the time the call ended. 15,763 of 17,068 such chunks repeat the tool result's text exactly; for `view`, `edit` and `create` the repeat is a longer, displayed form.
- **Tool results** (17,075): `role` `tool`, `tool_call_id` (the call's `id`), `content` and `toolTelemetry` with `metrics`, `properties`, `restrictedProperties` and `event`. They come right before the repeat chunk. Every call of the model has one, except 13 calls in 13 logs; setup steps have none.
- **Telemetry fields used:** `metrics.result_token_count` (10,705 results, in 571 logs from 2026-11-15 on), `metrics.context_current_tokens` and `metrics.context_token_limit` (10,850; limits 200,000 and 968,000), `metrics.exit_code`, `properties.shell_error_category` (`command_nonzero_exit` 417, `command_timeout` 196, `kill_command_blocked` 33, `invalid_input` 3, `shell_unknown` 1), `properties.executionMode` (`sync`, `async`) and `properties.detached`. `metrics.resultLength`, `resultForLlmLength`, `linesAdded` and `linesRemoved` (861 results) are not used.
- **User messages** (1,386): `role` `user`, `content`, sometimes `source` and `agentId`, never a time. The first one of the main conversation is the task's prompt. Later ones come from the platform: a custom agent's instructions, `Alter the PR title and description…` before the last response, `<skill-context …>` (`source` `skill-<name>`), time-limit warnings and `<system_reminder>`. A subagent's brief has `source` `agent-<session id>` and the subagent's `agentId`.
- **Other objects:** one, `{"kind": "model_call_failure", "callId", "turn", "modelCall": {"error"}, "modelCallDurationMs"}`. It is skipped.
- **Ids of responses:** a UUID for a model response; `turn-<n>` for the last answer, with `finish_reason` `stop` (863 logs have exactly one); `clone-…`, `runtime-…`, `github-mcp…`, `playwright…` for setup steps; `selected-custom-agent-<name>-start` and `-end` for the markers of a custom agent; a `toolu_…` call id for the markers of a `task` subagent.
- **Setup steps:** `run_custom_setup_step` (`arguments.name`, a step of the repository's `copilot-setup-steps.yml`, such as `Install the JDK`) and `run_setup` (`Clone repository <owner>/<repo>`, `Start '<server>' MCP server`), all before the first user message. A `run_setup` whose `name` starts with `▶️ Begin subagent: `, `⏹️ End subagent: ` or `❌ Failed subagent: ` is a marker, not a step.
- **Tokens:** no chunk or object carries the usage of a request. The only token counts are the telemetry fields above, which the agent counts for its own context management.

## Record fields

The session record of the agent tasks API, saved as `<session id>.json`: `id`, `task_id`, `name`, `prompt`, `model` (`sweagent-capi:claude-opus-5.5` 491, `sweagent-capi:claude-fable-5.2` 376), `state` (`completed` 863, `failed` 4), `created_at`, `updated_at`, `completed_at`, `head_ref`, `base_ref`, `owner`, `repository` and `user` (ids only), `error` (failed sessions), and `usage` (`amount`, `type` `ai_credits`) in 743 of 867 records. Credits are `amount / 1e9`. GitHub puts the credits of the later sessions of a task on the first session of that task: in 46 tasks the first session's `amount` grew after later sessions ran, and those later sessions have no `usage`.

## Decisions

1. **Agent name `copilot`, title `GitHub Copilot coding agent`.** The owner asked for `copilot`. The title names the cloud agent, since the Copilot CLI is another program with other logs. Rejected: `copilot-cloud`, which is more exact but not what the owner asked for (see open question 1).
2. **Not in the default `--agent` list.** These sessions ran on GitHub, not on this machine. In the default list, a freshly downloaded session would become the latest session of `tokprof`, and every report would gain rows without usage. Rejected: joining the default list as pi and OpenCode did; their logs are local work with usage.
3. **The folder comes from `$TOKPROF_COPILOT_DIR`, else `<tokprof folder>/copilot`.** The other sources read the agent's own variables (`CODEX_HOME`, `PI_CODING_AGENT_DIR`, `XDG_DATA_HOME`). The cloud agent writes no local logs and has no such variable: the logs are wherever the user saved them. tokprof's own folder (`$TOKPROF_DIR`, else `~/tokprof`, where `flame` and `share` write) is the place tokprof already owns. Rejected: a key in a config file (tokprof has none besides `prices.toml`, and adding one is a change of its own); a command-line flag (every command would need it); no default (the agent would then read nothing without a variable).
4. **One session per log; the record is found by its id.** The session id is the log's name without `.sse` or `.sse.gz`. The source walks the folder once and keeps every `<uuid>.json` it finds by its name; a JSON file whose `id` is not its name is not a record. So the record may lie beside the log (the planned archive) or in another folder of the tree (today's `logs/` and `meta/`). Rejected: only beside the log (today's corpus would have no records); a fixed `meta/` folder (a layout rule).
5. **One request per response id.** The first chunk of an `id` is the response: its time is when the response ended, its text and calls are what it produced. A chunk whose calls are all setup steps or markers is no request. Its send time is the latest time of its chain logged before it: the end of the last call it waited for, or of the last setup step, else its own time.
6. **No usage, no estimate.** Each request says the log gives no usage. No token count is made up from characters, and none is taken from `context_current_tokens` (see open question 3). Rejected: tokens as characters / 3.5 (the median ratio over 8,319 results), which is a guess that would flow into every token report; usage from the context counter, which gives the prompt but no cache split and no output, so its cache misses and dollars would be wrong.
7. **Cost from the record's credits.** `amount / 1e9` credits at $0.01 is the session's own cost, as Claude Code's cost counter is for Claude Code. It shows where a session's cost shows and is never spread over requests, models or tools. A session without a record or without `usage` has no cost, shown `–`, never $0. A first session's cost can include later sessions of its `task_id`; tokprof does not move credits between sessions, since the record does not say how much belongs to which. Rejected: spreading a task's credits over its sessions by their time or requests, which would be a guess.
8. **Sizes in tokens only where the log gives them.** A tool result's size is its `result_token_count`. Call inputs and results of logs before 2026-11-15 have no size, shown `–`. Rejected: sizes in characters for this agent only, which needs a new unit in tables that hold every agent's rows (see open question 4).
9. **Model: the record's model after its last `:`, with the dots of a `claude-` id's version as dashes.** `sweagent-capi:claude-fable-5.2` is `claude-fable-5-2`, the id Claude Code logs and `prices.toml` uses, so a model has one name in every table. Any other id keeps its form after the `:`. Without a record the model is `unknown`. Rejected: the id as the record gives it (a reader does not know `sweagent-capi`); the part after the `:` as it is (`claude-fable-5.2` and `claude-fable-5-2` would be two rows of one model).
10. **Tool results come from the `tool` objects.** They are what the model read. The repeat chunk gives only the end time, and the result when a call has no `tool` object. Setup steps' results are no prompt content.
11. **Environment setup is a kind of its own, run by no request.** The steps before the prompt took 3,372 minutes over 867 sessions, more than any tool but `bash`. tokprof's word `setup` already means the content every request carries (setup-breakdown), so the kind is `environment setup` and its group `environment`. Rejected: setup steps as `execute` calls (they would mix with the model's shell rows and, made by no request, count nowhere); a source-only line in the X-ray (no totals over sessions).
12. **Subagents are chains of the same log.** Chunks and objects with an `agentId` that is not a custom agent's marker id are a subagent's, one chain per `agentId`. The chain is `subagent: <name>` after the `name` of the `task` call whose begin marker comes right before the subagent's brief, else `subagent`. Its `task` call runs from the call to its repeat chunk, which the call log shows in `subagents`.
13. **A custom agent is not a subagent.** `selected-custom-agent-<name>-start` marks that the main conversation runs as the custom agent `<name>` (such as `helper`). The main chain stays `main`. Rejected: a subagent chain per custom agent, which would leave the main conversation empty.
14. **One turn per session.** The task's prompt is the one typed message, at the time of the log's first line, when the first setup step starts. Later user messages are reminders. The last response of the main conversation, normally `turn-<n>`, ends the turn. The queue before the first line is not the agent's work. Rejected: the record's `created_at` as the prompt's time (196 sessions waited over a minute in the queue); `scripted` (the log does not tell who wrote the prompt).
15. **The project is the repository the clone step names, as `github.com/<owner>/<repo>`.** It is not a path, so the catalog keeps it as it is (session-logs, Project of a folder). Rejected: `/home/runner/work/<repo>/<repo>`, which the log names only inside commands, and which looks like a folder of this machine; a local clone of the repository, which needs a rule that reads remotes, which Project of a folder forbids.
16. **MCP tools are `<server>-<tool>` of a server the session started.** The servers are those of the `Start '<server>' MCP server` steps: `github-mcp-server`, `runtime-tools`, `playwright`. `engine-tools-reply_to_comment` has no such step and is a tool of its own.

## Core changes

Named so that phase 2 can find them; the specs say only what they do.

- **`model::Request`** gains `no_usage: bool`, false by default: the log gives no usage for the request. Existing sources do not change.
- **`analysis::Walker::step`** today returns for a request with zero prompt and output, as one without a response. A request with `no_usage` and a response (`ended_ms`) goes on: it gets a `RequestRecord` and its tool shares and entered blocks, with zero tokens, no price and no verdict from `cache::judge`, and it is never the `prev` of a later judgement. `analysis::billed` and `live_chain` keep leaving it out.
- **`busy.rs`** tells answered requests by `r.usage.prompt() > 0 || r.usage.output > 0` (lines 207 and 264). It also takes a `no_usage` request with an `ended_ms` as answered. It gets the busy kind `environment setup` for the stretches in which a call of that kind runs, after `subagents` in the order of kinds.
- **`model::Session`** gains `logged_tokens: BTreeMap<usize, u64>`: the tokens the log gives for a block, by block index, as `call_ends` and `vcs` are kept. For a `no_usage` request the walker sizes entered blocks from it, and a block without an entry is unsized. `tools::BlockEntry` and `calllog::CallRow` carry `Option<f64>` tokens for that, and `ToolRow` counts the sized results and inputs apart, so a mean is over sized blocks and `–` without any.
- **`model::ToolClass`** gains `EnvironmentSetup`, a refinement of ACP's `execute`; **`tools::Group`** gains `Environment`, named `environment`; `GROUP_NAMES` lists it.
- **Calls no request made.** A source may put the setup steps' blocks in a message that no request produced and holds in no prompt, with their `CallRecord`s. The walker reports such calls to the sinks once per session, as tool shares with no request (`ToolShare.call` set, zero tokens), so `ToolTable` and the call log count them. The call log shows them with an empty `req`.
- **Own cost without usage.** The reconciliation line of `tokprof summary`, the X-ray header and the `cost` cell of `tokprof sessions --cost` use `Session.reported_usd` and `AgentInfo.cost_counter` when the session's requests are `no_usage`.
- **`tokprof-logio::open` and its line readers** read `.gz` with `flate2`'s `MultiGzDecoder` as they read `.zst`; reading on from an offset says `None` for `.gz`, as for `.zst`.
- **`tokprof-sources`:** a feature `copilot` and `sources.push(Box::new(tokprof_source_copilot::Copilot::from_env(home)))`; the CLI's `--agent` accepts `copilot`.

## The source

`sources/copilot`, crate `tokprof-source-copilot`, as `sources/pi`:
- **`Copilot: LogSource`** with `name` `copilot`, `detail` `Detail::Requests`, `discover` (one walk: logs and records), `session_at`, `claim` (`Claim::Yes` for `.sse` and `.sse.gz` under the folder, or anywhere when the first `data:` line is a `chat.completion.chunk`), `watched_paths` (the folder), `touched`, `head` (project from the clone step, session id from the name), `first_active` and `last_active`, `subagents` (none: subagents are chains of the log), `open` (`Records` over the log's lines through `tokprof_logio`), `decoder` and `activity`.
- **`AgentInfo`:** `short_id_len` 8; `cost_counter` `GitHub's billed credits`; `Capabilities { own_cost: OwnCost::Session, titles: true, subagent_tasks: true, prompt_composition: true, .. }`; `PublicNames` with the built-in tools seen; no pickup, no TTL settings.
- **`Parser: Decoder`** gets the record's model, credits and title from the source, since a decoder touches no file. It feeds on `data:` lines and builds the chains, messages, requests (`no_usage`), `CallRecord`s, `call_ends`, `vcs` (`report_progress` commits and pushes), `logged_tokens`, `mcp_servers`, `typed` and `handoffs`. Fed the lines in parts, it gives what it gives fed them at once (`testkit::conform`).
- **Kinds:** `bash` execute; `read_bash` execute output; `stop_bash` execute; `view` read; `edit` edit; `create` write; `grep`, `glob` search; `web_fetch`, `web_search` fetch; `task`, `read_agent` subagent; `skill` manage; `run_custom_setup_step`, `run_setup` environment setup; MCP tools by their server; `report_progress`, `codeql_checker`, `engine-tools-reply_to_comment`, `issue_read` and any other tool other.

## Reports

What each report shows for `--agent copilot` after this change, and why:

| Report | Works | Why |
|---|---|---|
| `tokprof sessions`, with `--cost` | yes | times, project, size; `cost` is the record's credits as dollars, marked as the agent's own |
| X-ray header, `tools`, `waits`, `failures`, requests list | yes | they rest on times, calls and outcomes |
| X-ray spend blocks, context fill, cache, forecast, advice | no; one line says the log gives no usage | they rest on tokens |
| `tokprof tools`, `scan --calls` | yes, but `rent`, `trips` and `after` are `–` and no row gets a hint | calls, times, failures and logged sizes are in the log; spend is not |
| busy time and its kinds | yes | request and call times; `environment setup` is the new kind |
| `tokprof summary` | rows with request counts and `–`; the own-cost line | no usage to price |
| `misses`, `whatif`, `bench` | nothing for this agent | no cache data |
| `flame` and the other profiles | nothing for this agent | their measures are tokens and dollars |
| `share`, `pool`, calendar, week, month, year, handoff note | not checked; out of scope | |

## Parse time

The 867 logs are 197 MB of text in 32 MB of gzip; gzip reads at a few hundred MB/s per thread, and logs are read in parallel. `--agent copilot` over them should take well under a second more than the parse itself. The default list does not read the folder, so `tokprof` with no arguments and the corpus of the other agents parse as before; the core changes add one flag test per request.

## Open questions, settled

1. **The name `copilot`.** The Copilot CLI writes local logs (`~/.copilot/session-state/`) in another format. The author kept `copilot` and the title `GitHub Copilot coding agent`; the CLI would get `copilot-cli` if it comes.
2. **The default folder.** The author kept `$TOKPROF_COPILOT_DIR`, else `~/tokprof/copilot`.
3. **Token estimates from the context counter.** In logs from 2026-11-15 on, `context_current_tokens` at the results of one response, plus their `result_token_count`, is close to the next request's prompt (the rest, median 492 tokens, is the next response and its hidden reasoning). Not in this change.
4. **Sizes of older logs.** 296 logs before 2026-11-15 have no `result_token_count`, so their sizes are `–`. The author accepted it.
5. **`cd <dir>; <command>`.** tokprof's command head skips `cd <dir> &&` but not `cd <dir>;`, and at least 1,477 of 12,230 `bash` calls start that way. A small change of its own, after this one.
6. **The phases of a session** (a custom agent's work, the pull request's title and description). Not in this change.
7. **Credits of a task's later sessions.** They are on the first session's record (46 tasks). Each record is shown as it is; see decision 7.

## Checked

On the 867 logs and their 867 records, with scripts outside the repository:
- the object shapes, keys and values listed above, and that no request carries usage;
- that every `tool` object is followed by the repeat chunk of its call (17,068 pairs), and that no call duration measured is negative once the units of `created` are read right;
- the sums of How it is checked in proposal.md: 15,184 responses, 26,653 calls, `bash` 12,230 calls in 4,391 minutes, setup steps 4,895 calls in 3,372 minutes, 421 failed and 33 denied results, 743 records with credits;
- that the setup steps all come before the first user message, and markers both before and after it.

Not checked: logs of repositories other than one, a session the person stopped, a log cut while it was being written, the Copilot CLI's logs.
