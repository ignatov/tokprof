# Design

## Context

See proposal.md for the motivation. Today the agent `copilot` is the cloud coding agent (`sources/copilot`, change `add-copilot-source`). That change settled that the CLI "would get `copilot-cli` if it comes". The author now wants the CLI to be `copilot`, so the cloud agent moves to `copilot-cloud`.

The source contract already carries everything the CLI's logs give: requests with usage, requests without usage, per-request own cost, compaction calls, chains of one log, call times and outcomes, calls no request made. So this change is one new source crate, a rename of the cloud agent and its crate, and the lists of agent names.

## Log fields

The log is `events.jsonl`, one JSON object per line: `type`, `data`, `id`, `parentId`, `timestamp` (ISO 8601 with milliseconds), and `agentId` on a subagent's lines. Surveyed on 2026-11-26: 10 logs, 14 MB, Copilot CLI 1.0.85 (2 logs), 1.0.106 (2), 1.0.115 (4) and 1.0.116 (2). The counts below are of that day; one of the logs was still growing.

- **`session.start`** (one per log, the first line): `data.sessionId`, `producer` (`copilot-agent`), `copilotVersion`, `startTime`, `context.cwd`, `context.gitRoot`, `context.repository`, `context.branch`.
- **`session.usage_record`** (about 450, in 1.0.106–1.0.116; none in 1.0.85): `data.usage` with `model`, `inputTokens`, `outputTokens`, `cacheReadTokens`, `cacheWriteTokens`, `reasoningTokens`, `duration` (ms), `cacheExpiresAt`, `cacheTtlSeconds` (11 records, all 300), `accounting.usageId`, `copilotUsage.totalNanoAiu`, `copilotUsage.tokenDetails`, `initiator` (`user`, `agent`, `sub-agent`), `interactionType` (`conversation-agent`, `conversation-subagent`), `apiEndpoint` (`/v1/messages`), `maxPromptTokens` (200,000 and 952,000), `aiCreditsStatus` (`complete`).
- **How the usage fields add up.** `inputTokens` is the whole prompt: in the first record of a session, `inputTokens` 45,714 is `cacheWriteTokens` 45,710 plus 4 uncached. `outputTokens` includes `reasoningTokens`. Both follow from the credits: with tokprof's prices for `claude-fable-5-2`, `claude-opus-5-5` and `claude-sonnet-5-5`, `(input − read − write) × input price + write × short write price + read × read price + output × output price` equals `totalNanoAiu / 1e11` dollars on every record, with a largest difference of 4e-16. No record has `read + write` above `inputTokens`.
- **Record and response.** The record comes right before its `assistant.message` line (452 of 452), and its `accounting.usageId` is that line's `data.apiCallId`. Every record has a response.
- **`assistant.message`** (464): `data.apiCallId`, `model`, `content`, `toolRequests` (`toolCallId`, `name`, `arguments` as an object), `outputTokens`, `turnId`, `interactionId`. 7 messages of the two 1.0.85 logs have an `apiCallId` and no record. 5 messages have no `apiCallId`, no `model` and one tool request (`read_bash` 4, `read_agent` 1): the CLI wrote them to read a finished background task.
- **`assistant.turn_start`, `assistant.turn_end`:** `data.turnId`; they bracket one response and its calls. A record's `timestamp` less its `duration` falls a median 74 ms after its `turn_start`.
- **`tool.execution_start`** (470): `data.toolCallId`, `toolName`, `arguments`, and for an MCP tool `mcpServerName` and `mcpToolName` (2, `github-mcp-server` and `web_search`).
- **`tool.execution_complete`** (469): `data.toolCallId`, `success`, `result.content`, `error` (`code`, `message`), `shellExecution.exitCode`, `toolTelemetry.properties.executionMode` (`sync` 367, `async` 2) and `detached`. `success` is `false` in 11: `failure` 6, `denied` 4, `rejected` 1. 14 shell results have `success` `true` and a non-zero exit code.
- **`user.message`** (58): `data.content`, `transformedContent`, `source` (none 50, `memory` 6, `autopilot` 1, `agent-<session id>` 1), `delivery` (`idle`, `steering`), `isAutopilotContinuation`.
- **Subagents:** `subagent.started` (`data.toolCallId`, `agentName`, `agentDisplayName`, `model`), `subagent.completed`, and every line of the subagent with the envelope's `agentId` (one subagent: 14 records, 21 calls, in 1.0.116).
- **Compaction:** `session.compaction_start` (`trigger`, `currentTokens`, `tokenLimit`) and `session.compaction_complete` (`success`, `compactionTokensUsed` shaped like `usage`, `preCompactionTokens`, `postCompactionTokens`, `summaryContent`); one of each, in 1.0.115.
- **Totals the CLI writes:** `session.usage_checkpoint` (`totalNanoAiu`) now and then, and `session.shutdown` (`totalNanoAiu`, `modelMetrics`, `tokenDetails`) when a session ends (4 logs). In the 4 ended sessions, the shutdown total equals the sum of the records. In session `54061b9a` the checkpoint (583.54624) is the records (576.34362) plus the compaction call (7.20262).
- **Beside the log:** `workspace.yaml` with `id`, `cwd`, `repository`, `branch`, `name`, `created_at`, `updated_at`, as plain `key: value` lines.
- **Not read:** `session.model_change`, `session.info`, `session.tool_baseline`, `permission.*`, `skill.*`, `instruction.*`, `system.message`, `system.notification`, `model.*`, `session.context_changed`, `session.task_complete`.

## Goals / Non-Goals

**Goals:**
- The CLI's sessions in every report the default agents have, with tokprof's dollars equal to GitHub's credits.
- No change to what the other agents print, the cloud agent included, besides its name.

**Non-Goals:**
- Reading `session-store.db` or `~/.copilot/logs`.
- A context limit per request (`maxPromptTokens`); a change of its own, with the X-ray's fill line.

## Decisions

1. **The CLI is `copilot`; the cloud agent becomes `copilot-cloud`.** The author chose `copilot` for the CLI. The cloud agent's new name is the one `add-copilot-source` called more exact. Its title `GitHub Copilot coding agent` stays; the CLI's title is `GitHub Copilot CLI`. Rejected: `copilot-cli` for the CLI (the earlier plan, no rename, but the author types `copilot` for the tool he uses daily); one agent `copilot` over both kinds of logs (one agent would be half in the default list, with and without usage).
2. **The crates follow the agents' names; the folder and the variable stay.** The cloud source moves to `sources/copilot-cloud` (`tokprof-source-copilot-cloud`, feature `copilot-cloud`) with `git mv`, so its history stays. The CLI's source is the new `sources/copilot` (`tokprof-source-copilot`, feature `copilot`). The author asked for this: ours is `copilot`, the other `copilot-cloud`. `$TOKPROF_COPILOT_DIR` and `~/tokprof/copilot` stay, since they hold the author's 867 logs and his download scripts. Rejected: `sources/copilot-cli` beside an unmoved `sources/copilot`, where the crate `copilot` would hold the agent `copilot-cloud`.
3. **The spec keeps the cloud requirements' names.** `Copilot log folders`, `Copilot requests` and the other eleven stay, and `Agent selection` says that Copilot without CLI means the coding agent. The new ones are `Copilot CLI …`. Rejected: renaming thirteen requirements and every reference to them for no change of behavior.
4. **In the default list.** The CLI's sessions are local work with usage, as pi's and OpenCode's. So `tokprof` with no arguments can show one.
5. **One request per usage record, not per response.** The record is what GitHub billed and has every figure. The response joins by `accounting.usageId` = `apiCallId` and gives the text and the calls. Rejected: one request per `assistant.message` with its `outputTokens`, which has no prompt side.
6. **Uncached input is `inputTokens − cacheReadTokens − cacheWriteTokens`.** Checked against the credits on every record (see Log fields). It is clamped at 0, since the fields come from a provider and another provider may count the prompt differently.
7. **Send time is the record's `timestamp` less `duration`.** `duration` is the API call's own length. Rejected: the `assistant.turn_start` line, which is 74 ms earlier at the median but up to 1.2 s, and is missing before 15 of 452 records. A request without a record has no duration, so it takes `turn_start`.
8. **Writes are short-lifetime unless the record says over 1,800 s.** Every record seen expires 294–298 s after its line, or says `cacheTtlSeconds` 300. A 1-hour cache has not been seen; the rule reads the same fields and puts it with the long writes, where its price is. The threshold is between the two lifetimes, so the few seconds between the response and the line do not matter.
9. **Own cost per request, from the record's credits.** $0.01 a credit, the rate `Copilot's billed credits` already uses. The session's own cost is the sum over records and compaction calls. Rejected: `session.shutdown`'s total (only ended sessions have it); the latest `session.usage_checkpoint` (it lags the records in a live session: 210 against 579 credits in one log).
10. **Responses without a record are requests without usage.** The 1.0.85 logs have responses and calls and no token counts at all. tokprof does not guess; core already shows such requests with `–`. Their rows come last in `tokprof summary`.
11. **A message without an `apiCallId` is no request.** No model was called. Its calls go to the session as calls no request made, which the tool tables and the call log already count.
12. **Subagents are chains of the same log,** as in the cloud source: one chain per `agentId`, named after `agentDisplayName` (the `name` the `task` call gave). The source's `subagents` gives no logs.
13. **A shell command that exits non-zero failed,** though the CLI logs `success` `true` for it. Claude Code's and the cloud agent's sources count it so, and one tool table holds all of them.
14. **The title comes from `workspace.yaml`,** read by the source when it makes the decoder, as the cloud source reads its record. One `name:` line is read; no YAML library is added. Rejected: the first typed message as the title (not what the CLI shows in its own session list).
15. **Tools.** `PublicNames` lists the built-in tools seen. Kinds and input heads are in the spec delta and the source's README, the same as the cloud source's where the tools are the same.

## The source

`sources/copilot`, laid out as the cloud source, now `sources/copilot-cloud`:

- **The log source:** name `copilot`; `discover` lists `<root>/session-state/*/events.jsonl`; `claim` says yes for an `events.jsonl` under the root, or for any file whose first line is a `session.start` of `copilot-agent`; `watched_paths` is `session-state`; `head` reads the first line; `last_active` reads the last line's `timestamp`; `subagents` is empty.
- **Agent facts:** short id 8; cost counter `GitHub's billed credits`; own cost per request; titles; subagent tasks; prompt composition; no pickup and no TTL settings.
- **The decoder** keeps a pending usage record until its response comes, the calls by `toolCallId`, and the chains by `agentId`. Fed the lines in parts, it gives what it gives fed at once (`testkit::conform`).
- **The rename** in `sources/copilot-cloud`: the crate, its library name and its feature; the agent's `name` and the source's `name` and `SessionRef.source` become `copilot-cloud`.

## Parse time

The CLI's logs are 14 MB, about 0.2% of the 8.5 GB corpus. Lines are parsed once; a usage checkpoint line can be 100 KB, but its type is tested before its body is read. The full corpus should parse in the same 8 s, and `tokprof` with no arguments in under a second. Both are measured in the tasks.

## Risks / Trade-offs

- [`--agent copilot` changes its meaning] → It is in the proposal as BREAKING, in the README and in the release notes. The author is the only user of the cloud agent's logs; his scripts that pass `--agent copilot` need `copilot-cloud`.
- [The crate `sources/copilot` changes its content] → The move and the new crate are separate commits, so `git log --follow` finds the cloud source's history. Another session with edits in `sources/copilot` must rebase them onto `sources/copilot-cloud`.
- [Models that are not Claude] → The survey has none with usage. Their ids are kept as logged; a model without a price shows `No price for`, as everywhere. The `input − read − write` rule is clamped at 0.
- [The log format changes between CLI releases] → The README names the fields and the versions. Fixtures are synthetic. A record without `accounting.usageId` still counts as a request, joined to the next response of its chain.
- [A live session] → The last line can be half written; `tokprof-logio` already skips a line that is not JSON.
- [Sessions run by the nightly price check] → They run on GitHub's runners and leave no local logs.

## Migration Plan

One release. The release notes say: `--agent copilot` is now the Copilot CLI; use `--agent copilot-cloud` for the saved cloud logs. No file moves. Rollback is a revert of the commit.

## Open Questions

- Should the cloud agent's folder and variable follow its new name later? Not needed for this change.
- `skill-<name>` as a `source` of a user message was seen in the cloud agent's logs, not yet in the CLI's. The rule treats any `source` as a reminder, so it does not depend on the answer.
