# Proposal

## Why

The author also runs the pi coding agent (`@earendil-works/pi-coding-agent`), and Megachat runs on pi through `@earendil-works/pi-durable`. tokprof reads only Claude Code and Codex, so pi's tokens and dollars are in no report.

pi writes a JSONL session log that tokprof can read like the others. On the author's machine:
- `~/.pi/agent/sessions/` holds 2 logs of pi 0.84.2, both on a local Qwen model: 42 responses with usage, 3 compactions, and $0 by pi's own count.
- Megachat's pi-durable store (a SQLite file) holds 239 Anthropic responses, $32.89 by pi's own count.

This change reads the JSONL logs. A later step reuses its record shape for pi-durable stores.

## What Changes

- **A new agent `pi`.** tokprof finds pi session logs and reads their requests, tokens, models, times, compactions and pi's own cost. Every command that takes `--agent` accepts `pi`, and the default list gains it.
- **Where the logs are.** `$PI_CODING_AGENT_SESSION_DIR`, else `$PI_CODING_AGENT_DIR/sessions`, else `~/.pi/agent/sessions`; files ending in `.jsonl`, at any depth (pi 0.84.2 writes `--<path>--/<time>_<uuid>.jsonl`).
- **Requests.** Each `message` line whose message has role `assistant` and a non-zero `usage` is one request. So is the `usage` of a `compaction` or `branch_summary` line: a compaction call. `input` is uncached input, `cacheRead` reads, `cacheWrite` short-lifetime writes, `cacheWrite1h` (when present) long-lifetime writes, `output` output.
- **pi's own cost.** The sum of `usage.cost.total` is the session's reported cost, compared with tokprof's total in `tokprof summary` like Claude Code's counter. A model with no tokprof price, such as a local one, stays unpriced; pi's $0 is not taken as a price.
- **Compactions.** A `compaction` line marks the next request as following a compaction.
- **Where it shows:** `tokprof sessions`, `tokprof summary`, the X-ray of `tokprof` and `tokprof ui`, `tokprof flame` and the line about the logs read, all with the agent `pi`.
- **The source.** A new log source of its own under `sources/pi`, with synthetic fixtures, the conformance test and a README that names the locations, the fields and pi 0.84.2.

## Capabilities

### New Capabilities
None.

### Modified Capabilities
- `session-logs`:
  - changes `Log locations`, `Session id and project`, `Compaction markers`, `Agent-reported cost` and `Agent selection`, which now name each agent's requirement instead of holding every agent's fields, so that each stays under the 500 characters of a requirement;
  - adds `Pi log locations`, `Pi session id and project`, `Pi's own cost`, `Pi requests and usage`, `Pi compaction calls` and `Pi forks`;
  - adds `Claude Code and Codex log locations`, `Claude Code and Codex session ids`, `Compaction marker lines` and `Claude Code's cost counter`, which hold the text the changed requirements gave up, unchanged.

## How it is checked

- `tokprof --agent pi summary` on `~/.pi/agent/sessions` shows 2 sessions and 42 requests with 80,859 uncached, 752,658 read, 0 written and 38,177 output tokens, the sums a script takes over the same lines.
- A synthetic fixture with Anthropic usage and `usage.cost.total` copied in shape from the pi-durable store: tokprof's dollars stay within 1.5% of pi's own sum.
- `tokprof` with no arguments still answers in under a second, and the full corpus parses in about the same 8 s.

## Out of scope

- pi-durable SQLite stores, such as Megachat's `conversation.sqlite`. They have no fixed location and need a SQLite reader; a later change.
- Rebuilt prompts and miss causes for pi: `cache-misses` reports pi's misses with the cause `unknown (server side?)` until pi's prompts are rebuilt.
- Tools, triggers, hand-offs, interrupts, version-control actions, share names and the handoff note for pi.
- The `sessionDir` setting of pi's `settings.json` and the `--session-dir` flag of one pi run.

## Impact

- New crate `sources/pi`; one feature and one line in `crates/tokprof-sources`.
- `README.md`: pi in the list of agents, with its log location.
- No change to Claude Code or Codex results.
