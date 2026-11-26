# Proposal

## Why

The author now works in the GitHub Copilot CLI every day. It writes a local log of each session, `~/.copilot/session-state/<session id>/events.jsonl`, and tokprof does not read it. The agent `copilot` that tokprof has today is another program: the cloud coding agent, whose saved logs give no usage.

The CLI's logs are the opposite. Every API request has a `session.usage_record` line with its tokens, its cache reads and writes, its duration and the credits GitHub billed for it. So the CLI can have the full profile: tokens, dollars, cache misses, tools, time, and a check of tokprof's dollars against GitHub's own.

On the author's machine on 2026-11-26: 12 session folders, 10 with a log (14 MB), written by Copilot CLI 1.0.85 to 1.0.116. They hold about 450 usage records of `claude-fable-5.2`, `claude-opus-5.5` and `claude-sonnet-5.5`. Priced with tokprof's built-in prices, every one of these requests costs what GitHub billed for it, to the last digit.

## What Changes

- **A new agent, the GitHub Copilot CLI, named `copilot`.** It is in the default `--agent` list, as pi and OpenCode are: its sessions are local work with usage. `tokprof` with no arguments can pick a Copilot CLI session as the latest one.
- **BREAKING: the cloud coding agent is renamed from `copilot` to `copilot-cloud`.** `--agent copilot-cloud` reads the saved cloud logs. `--agent copilot` now reads the CLI's logs. The cloud agent's folder (`$TOKPROF_COPILOT_DIR`, else `~/tokprof/copilot`), its title and everything it shows stay as they are. It stays out of the default list. The line about the logs read says `read copilot-cloud …` for it.
- **Where the logs are.** `$COPILOT_HOME/session-state/<session id>/events.jsonl`, by default under `~/.copilot`. A session folder without `events.jsonl` is no session.
- **Requests and usage.** One request per `session.usage_record` line: uncached input, cache reads, cache writes, output (reasoning included), the model, the send time and the end time. A response of a log that has no usage record for it (Copilot CLI 1.0.85) is a request without usage, as cost-attribution already has them.
- **Own cost.** The credits of a request (`copilotUsage.totalNanoAiu / 1e9`) at $0.01 a credit are its own cost. `tokprof summary` compares the sum with tokprof's total, as it does for Claude Code's cost counter.
- **Compaction.** A `session.compaction_complete` line marks the next request as one after a compaction. Its `compactionTokensUsed` is a compaction call with its own usage and credits.
- **Calls.** Every tool call with its start and end in milliseconds, its input head, its kind, its outcome (failed, refused by the user, denied), calls in the background and calls that ask the user. MCP tools go under the server the log names.
- **Subagents.** Lines with an `agentId` are a subagent's chain in the same session, named after the `task` call that started it.
- **Prompts, turns, project, title, version.** The typed messages, the project from the session's `cwd`, the title from `workspace.yaml`, and the CLI version of each request.
- **Where it shows.** Every command and view that works for the default agents: `tokprof`, `scan`, `sessions`, `summary`, `misses`, `whatif`, `tools`, `pauses`, `flame`, `day`, the calendar and the reviews, `share`, `tokprof ui` and the window.
- **The crates follow the names.** The cloud agent's crate moves from `sources/copilot` to `sources/copilot-cloud`. The CLI's source is a new crate `sources/copilot` with synthetic fixtures, the conformance test and a README that names the folder, the fields and the CLI versions they were seen in.

## Capabilities

### New Capabilities
None.

### Modified Capabilities
- `session-logs`: changes `Agent selection`, `Log locations`, `Agent-reported cost` and `Compaction marker lines`; adds `Copilot CLI log locations`, `Copilot CLI session id, project and title`, `Copilot CLI requests and usage`, `Copilot CLI responses without a usage record`, `Copilot CLI's own cost`, `Copilot CLI compaction calls`, `Copilot CLI calls and results`, `Copilot CLI call outcomes`, `Copilot CLI tool kinds and input heads`, `Copilot CLI subagents` and `Copilot CLI prompts and turns`.
- `cost-attribution`: the scenarios of `Requests without usage` and `Summary rows without usage` name the cloud agent `copilot-cloud`.
- `cache-misses`: the scenario of `Requests without usage are not judged` names `copilot-cloud`.
- `tool-breakdown`: the scenarios of `Spend of calls without usage` name `copilot-cloud`.
- `session-xray`: the scenario of `Sessions without usage` names `copilot-cloud`.
- `session-browser`: the scenario of `Own cost in the session list` names `copilot-cloud`.

## How it is checked

On the author's logs in `~/.copilot/session-state`, with a script that reads the same files at the same moment (the session the check runs in keeps growing, so the script and tokprof run back to back):

- `tokprof --agent copilot sessions` lists one session per folder that has an `events.jsonl`: 10 on 2026-11-26. The 2 folders without a log are not listed.
- `tokprof --agent copilot summary`: the requests with usage equal the `session.usage_record` lines plus the compaction calls (about 450 plus 1). The uncached, cache-read, cache-written and output tokens equal the script's sums of `inputTokens − cacheReadTokens − cacheWriteTokens`, `cacheReadTokens`, `cacheWriteTokens` and `outputTokens`.
- The reconciliation line: tokprof's total equals the sum of the records' credits at $0.01 within $0.01 per session. The survey found them equal on every request.
- Four sessions have ended and carry GitHub's own total in a `session.shutdown` line. tokprof's cost of each must match it: `8d362bc6` 20.61502 credits ($0.21; 14 uncached, 230,797 read, 59,133 written and 3,521 output tokens in 6 requests), `a0ab881f` 14.00253 ($0.14), `f1e3d128` 105.1672 ($1.05), `d4722376` 740.46898 ($7.40).
- Session `54061b9a`: one compaction call of 161,982 prompt and 5,181 output tokens, 7.20262 credits; the request after it is marked as one after a compaction.
- `tokprof --agent copilot tools`: `bash` is the top row (364 of about 470 calls on 2026-11-26); 11 results with `success` false split into 6 failed, 4 denied and 1 the user refused.
- `tokprof --agent copilot scan ab66ecb4`: one subagent chain, `subagent: review-factory-template`, with 14 requests.
- Sessions `0c396a3a` and `5f1e2a67` (Copilot CLI 1.0.85): 7 requests without usage, no tokens and no dollars.
- `tokprof --agent copilot-cloud summary` and `tools` print what `--agent copilot` printed before, but for the agent name.
- Claude Code, Codex, pi and OpenCode results do not change: their golden tests stay as they are.
- `tokprof` with no arguments answers in under a second. The CLI's logs add 14 MB to a corpus of about 8.5 GB.

## Out of scope

- The session's context limit (`maxPromptTokens`) and the context fill from it.
- The trigger of a compaction (`trigger` of `session.compaction_start`).
- The MCP servers a session had. Only the servers of the tools it called are known.
- The sizes GitHub counts for tool results (`result_token_count`); requests with usage get their sizes as every other agent's do.
- Failed model calls (`model.model_call_failure`), which carry no usage.
- `session-store.db`, the CLI's own index of sessions, and sessions that are only in the cloud.
- Any change to what the cloud agent reads or shows, besides its name.
- Renaming the cloud agent's folder or its environment variable.

## Impact

- `sources/copilot` moves to `sources/copilot-cloud` (`tokprof-source-copilot-cloud`, feature `copilot-cloud`); its source and agent name become `copilot-cloud`; its README says so.
- New crate `sources/copilot` (`tokprof-source-copilot`, feature `copilot`) for the CLI; one line in `crates/tokprof-sources`.
- The CLI's `--agent` values, its help and its `unknown agent` hint; the same list in `tokprof ui` and the window.
- `crates/tokprof/tests`: the golden tests of the cloud agent use `--agent copilot-cloud`; new golden tests of the CLI's fixtures; the home of the tests gets a `COPILOT_HOME`.
- `README.md`: the agent list, the `--agent` row and the section on each Copilot agent.
- No change to `tokprof-core`: the contract of a source already covers everything the CLI's logs give.
