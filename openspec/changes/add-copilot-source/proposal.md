# Proposal

## Why

The author runs the GitHub Copilot coding agent (the cloud agent that works on a repository and opens pull requests) many times a day. He downloads the log of each session and the session's record from GitHub. tokprof cannot read them, so these sessions are in no report.

He wants to know, over hundreds of sessions: where the minutes of a session go (environment setup, Gradle, tests, reading, editing), the count and the time of each tool and shell command, the sizes of tool results, the model, and the cost of each session.

These logs differ from all other agents' logs in one way: they hold no token counts of the requests. A session's cost is in its record, as GitHub's billed credits. So this change also teaches the core what to do with a request that has no usage, instead of letting it count as zero.

On the author's machine, 867 logs (32 MB as gzip, 197 MB as text) from 2026-11-14 to 2026-11-21 hold 15,184 model responses (15,081 main, 103 of subagents), 26,653 tool calls (12,230 `bash`, 4,895 custom setup steps) and no usage. Each has a record; 743 records carry credits and 124 have none.

## What Changes

- **A new agent `copilot`**, title `GitHub Copilot coding agent`. Every command that takes `--agent` accepts it. It is not in the default list (see design.md, decision 2).
- **Where the logs are.** The folder `$TOKPROF_COPILOT_DIR`, else `copilot` in tokprof's own folder (`$TOKPROF_DIR`, else `~/tokprof`), read at any depth: `<session id>.sse` and `<session id>.sse.gz`, with the session record `<session id>.json` anywhere in the same folder tree. The planned archive `YYYY-MM-DD/<id>.sse.gz` with `YYYY-MM-DD/<id>.json` fits, and so does today's `logs/` and `meta/`. A log given as the target is read wherever it lies.
- **Requests without usage (core).** A request can say that its log gives no usage. Such a request has its times, model, prompt and calls, and counts as answered when its response is in the log. It adds no tokens and no dollars, and cache-misses never judges it. Today the same request would count as one without a response and vanish from every report.
- **The session's cost.** The record's `usage.amount / 1e9` is the session's credits; at $0.01 a credit it is the session's own cost. It shows in the X-ray, in `tokprof sessions --cost` (marked as the agent's own cost) and in `tokprof summary`, where no tokprof figure stands next to it. It is never turned into tokens. GitHub bills the later sessions of a task on its first session, so a first session's cost can include later sessions, which then have none; tokprof shows each record as it is.
- **Sizes.** Where the log gives a tool result's tokens (`toolTelemetry.metrics.result_token_count`, in logs from 2026-11-15 on), the tool tables use them. A size the log does not give stays unknown (`–`); tokprof does not estimate tokens from characters.
- **Model.** The record's `sweagent-capi:claude-fable-5.2` is shown as `claude-fable-5-2`, the id tokprof and Claude Code use for the same model.
- **Environment setup (core).** A new tool kind and group, `environment setup` and `environment`, for steps the agent's platform runs before the model works (clone, the repository's `copilot-setup-steps.yml`, starting MCP servers). They count in the tool tables, the call log and the busy time, though no request made them.
- **Calls and times.** Every tool call with its start and end (1 second precision), input head, outcome (failed, denied, in the background), kind and group; subagents (`task`) as subagent chains; MCP tools under their server.
- **Compressed logs (log reading).** `.gz` files are read like `.zst` files.
- **Where it shows:** `tokprof --agent copilot sessions`, `tools`, `summary`, `scan <id>` and `scan <id> --calls`, the busy time and waits of the X-ray, and the line about the logs read.
- **The source.** A crate `sources/copilot` with synthetic fixtures, the conformance test and a README that names the folder, the fields and the dates of the logs they were seen in (the log names no agent version).

## Capabilities

### New Capabilities
None.

### Modified Capabilities
- `session-logs`: changes `Log locations`, `Agent selection`, `Agent-reported cost`, `Call times`, `Calls that return before their work ends`, `Kind of a tool`, `Input of a call` and `Connected MCP servers`; adds `Copilot log folders`, `Copilot session records`, `Copilot log lines`, `Copilot requests`, `Copilot model`, `Copilot's billed credits`, `Copilot calls and results`, `Copilot call outcomes`, `Copilot subagents`, `Copilot environment setup`, `Copilot prompts and turns`, `Copilot project and title` and `Sizes the log gives`.
- `cost-attribution`: adds `Requests without usage`, `Summary rows without usage` and `Own cost without usage`.
- `cache-misses`: adds `Requests without usage are not judged`.
- `tool-breakdown`: changes `Sizes per call`, `Groups` and `Narrowing the call log`; adds `Calls no request made` and `Spend of calls without usage`.
- `agent-time`: changes `What the busy time went to`.
- `session-xray`: adds `Sessions without usage`.
- `session-browser`: adds `Own cost in the session list`.

## How it is checked

On the 867 logs and their records, with the numbers a script computes straight from the files (kept in the task notes):
- `tokprof --agent copilot sessions` lists 867 sessions; `read copilot 867 files (0.03 GB)`.
- `tokprof --agent copilot summary`: 15,184 requests, all without usage, of `claude-opus-5-5` and `claude-fable-5-2`; the own-cost line gives the sum of the records' credits as dollars, for the 743 sessions with credits, equal to the sum a script takes over the records.
- `tokprof --agent copilot tools`: `bash` 12,230 calls; `run_custom_setup_step` 4,895 calls and about 3,372 minutes; failed 421 calls; rules denied 33. Each row's time matches the script within the 1-second precision of the log.
- `tokprof --agent copilot scan <id> --calls` on the three sample logs: every call in log order, with the times the script gives.
- Claude Code, Codex, pi and OpenCode results do not change: the golden tests stay as they are, and `tokprof summary` on the author's corpus gives the same totals before and after.
- `tokprof` with no arguments answers in under a second as before, and the full corpus parses in about the same time.

## Out of scope

- Token or dollar estimates for Copilot requests. Reports that rest on usage show nothing for this agent (see design.md, Reports).
- The context fill of Copilot sessions from `context_current_tokens` and `context_token_limit`, and per-request token estimates from them (see design.md, open question 3).
- Downloading logs or records from GitHub: tokprof stays local and makes no network calls.
- The GitHub Copilot CLI, which writes local logs of another format.
- Share files, the pool, the calendar, the week, month and year views, `misses`, `whatif`, `bench`, `flame` and the handoff note for Copilot sessions.
- The queue time between the record's `created_at` and the first line of the log.
- Watching the folder for new logs in `tokprof ui` beyond what the source contract gives for free.

## Impact

- New crate `sources/copilot`; one feature and one line in `crates/tokprof-sources`; `copilot` in the `--agent` values of the CLI.
- `tokprof-core`: requests without usage, sizes the log gives, the `environment setup` kind, the `environment` group and busy kind, calls no request made, and the own-cost views for agents without usage.
- `tokprof-logio`: gzip reading (`flate2`, already in the workspace).
- `README.md`: the agent, its folder, and what its reports show.
- No change to the results of the other agents.
