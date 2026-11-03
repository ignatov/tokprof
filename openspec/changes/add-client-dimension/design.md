# Design

## Context

Both agents already write which program ran them:

- **Claude Code:** `entrypoint` on its `user`, `assistant` and `attachment` lines. Seen in 2.1.284–2.1.316, also in subagent transcripts:

  | value | what runs Claude Code |
  |---|---|
  | `sdk-ts` | the TypeScript agent SDK, as IDE plugins that run Claude Code use it |
  | `cli` | the interactive terminal |
  | `sdk-cli` | `claude -p` |

  The 2.1.316 binary also knows `sdk-py`, `claude-vscode`, `claude-desktop` and others.
- **Codex:** `payload.originator` of the `session_meta` line that opens a rollout. Seen in 0.147–0.166:

  | value | what runs Codex |
  |---|---|
  | `codex_exec` | `codex exec` |
  | `codex-tui` | the terminal UI |
  | `Codex Desktop` | the desktop app |
  | an IDE plugin's own name | that plugin |

  `payload.source` (`exec`, `vscode`, `cli`) is coarser, and it is an object for subagent threads.

A session can change client. 2 of 1,073 Claude Code logs mix two values: `da18a2d8` has 10,374 `cli` lines and 248 `sdk-ts` lines, and `7e640abc` has 91 and 625. These are sessions resumed in another program.

## Goals / Non-Goals

**Goals:**
- Spend, misses and every report can be split and filtered by client, at request level.
- The value is the log's own.

**Non-Goals:**
- No grouping such as "IDE". `sdk-ts` is any agent-SDK host, and the transcript does not name which one.
- No new column in `sessions` and `ui`.

## Decisions

1. **The client is per request.** A Claude Code request takes the `entrypoint` of the line its response came from. A Codex request takes the rollout's `originator`. A per-session value would put the 248 `sdk-ts` responses of `da18a2d8` under `cli`.
2. **The value is kept as written.** A missing value is reported as `unknown`, as other dimensions report what the log does not say. There is no mapping table to keep up with agent releases.
3. **A cheap representation.** The request holds a shared string. The decoder keeps the last value it saw and reuses it while the value repeats, which it does on almost every line. Most sessions therefore hold one string, not one per request.
4. **The filter rides on the report window.** The window that already holds `--since` and `--until` also holds the client text. Its check moves from the time to the request.
   - The cache judgement still sees every request of a chain as the previous one, as it does for requests outside the time window. A `cli` request therefore remains the cache predecessor of the `sdk-ts` request after it.
   - `bench` uses the same check for the returns it picks.
5. **The lists use the log head.** `sessions`, `ui`, `scan`, `handoff` and the default command pick sessions without parsing them. Each source's head reader already scans the first 200 lines for the project and the id. Now it also takes the first client value it meets.
   - Claude Code: the first line with `entrypoint`. These are the same lines that carry `cwd`.
   - Codex: the `session_meta` line.
   - A session resumed in another client is listed under its first one. The spec says so.
6. **Summary table.** The collector groups priced requests by agent and client. It counts requests and cost and gathers the main session ids of each group, which gives `sessions`. Misses gain the client of their request, so `avoidable` sums per row with the same rule that `Cache misses by cause` uses.
7. **Reconciliation.** A session joins the comparison only when the filters keep every one of its requests. This is the rule `--since` already follows, so the counter and tokprof's sum cover the same requests.
8. **X-ray header.** The header counts the session's requests per client, subagents included. It prints one value alone, or each value with `×count`, most requests first.

## Risks / Trade-offs

- **Format drift.** Claude Code or Codex may rename the field or change the values. The README and the session-logs requirement name the versions they were seen in, and a missing value degrades to `unknown`, not to an error.
- **Lists and reports can disagree.** For a session resumed in another client, the list goes by the first value and a report counts each request under its own. This is rare (2 of 1,073 logs), and the spec states it.

## Parse time

- Claude Code reads one more string field on each response line. The JSON value is already parsed, and the string is reused while it repeats.
- Codex reads one field per rollout.
- The head readers look at lines they already read.
- The expected effect is under 1% on the full corpus; the tasks measure it against a 2% bound.
