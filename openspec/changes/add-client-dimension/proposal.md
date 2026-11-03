# Proposal

## Why

tokprof does not say where a session ran: in an IDE through the agent SDK, in the terminal, or headless. For cache costs that matters most. In the last 30 days of the author's Claude Code logs, 206 of 211 cold returns were in IDE sessions (entrypoint `sdk-ts`) and 5 in the terminal (`cli`). The terminal compacts a large idle session before its cache expires; SDK sessions never do. To see this, the cold-session study had to grep `entrypoint` out of 1,073 logs by hand.

Spend splits the same way: $8,534 in `sdk-ts`, $424 in `cli` and $3.16 in `sdk-cli` (`claude -p`). Codex names its client as `originator`, as in `codex_exec`, `codex-tui`, `Codex Desktop` or the name of an IDE plugin.

## What Changes

- **Each request knows its client**: the program that ran the agent, as the log names it.
  - Claude Code: the `entrypoint` of the response's line.
  - Codex: the `originator` of the rollout's `session_meta`.
  - The value is shown as the log writes it, with no mapping. A request without one has the client `unknown`.
- **`--client <text>`** is a new global filter, like `--project`. It ignores case.
  - The reports count only the requests whose client contains the text.
  - The commands that pick or list sessions take the client from the first lines of the main log.
- **`tokprof summary`** gets a `Spend by client` table, after the reconciliation line and before the spend by origin. It has one row per agent and client: `sessions`, `requests`, `cost`, `share` and `avoidable` on cache misses.
- **The X-ray header** gets a `client` line. One client shows alone, as in `client    sdk-ts`. When a session ran under several, each one shows with its request count, as in `client    cli ×10374 · sdk-ts ×248`.
- The README names the values seen so far and what they mean.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `session-logs`: the client of a request, read from both agents' logs, and the `--client` filter with the commands it applies to.
- `cost-attribution`: the `Spend by client` table and its place in the summary report.
- `session-xray`: the `client` line in the header.

## How it is checked on the real logs

- **30 days of Claude Code.** `tokprof summary --agent claude --since 30d` shows `sdk-ts` at about $8,534 with 67,097 requests, `cli` at about $424 with 2,292, and `sdk-cli` at about $3.16 with 32. These are the scratch dump's numbers of 2026-11-03; new sessions since then shift them a little.
- **The filter.** `tokprof --client sdk-ts --agent claude --since 30d misses` lists about 206 `idle > TTL` misses, and `--client cli` lists about 5.
- **A mixed session.** The X-ray of `da18a2d8`, which was resumed in another client, shows `client    cli ×… · sdk-ts ×…`, with counts that match the log's 10,374 `cli` and 248 `sdk-ts` lines in proportion to their responses.
- **Codex.** Over all history, the `originator` values of the rollouts show up as `codex_exec`, `codex-tui`, `Codex Desktop` and the names of IDE plugins.
- **Totals and speed.** The rows of `Spend by client` add up to `Total priced`. The parse time of `tokprof summary` stays within 2% of the baseline.

## Out of scope

- Friendly names or groups for client values, such as calling `sdk-ts` "IDE". The log does not say which SDK host ran a session, so tokprof would have to guess.
- A client column in `sessions` and `ui`.
- What-ifs per client, such as idle compaction for `sdk-ts` only. The study's scripts already answer that; a bench strategy is a separate change.
- Claude Code's `x-client-app` request header. It is not in the transcript.

## Impact

- Sources: the Claude Code and Codex decoders and their log heads read one more field.
- Core: requests carry a client, and the report window filters by it. The summary collector gets one more table, and the X-ray header one more line.
- CLI: the `--client` flag, and the render of the new table and line.
- README: the filter, the table and the values.
