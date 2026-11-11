# Proposal

## Why

OpenCode is one of the agents tokprof planned to read (`openspec/config.yaml`), and colleagues run it. tokprof reads Claude Code, Codex and pi, so OpenCode's tokens and dollars are in no report.

OpenCode keeps no log files. Since opencode 1.x it keeps every session in one SQLite store, `~/.local/share/opencode/opencode.db`, with two layouts in the same file:
- the `session`, `message` and `part` tables, which opencode 1.x writes (1.18.16 and 1.19.3 seen);
- the `session_v2` and `session_message` tables, which opencode 2.x writes (2.2.3 and 2.2.4 seen), and into which 2.x copies the 1.x sessions.

On the author's machine the store holds 2 sessions with usage, 4 requests, all on the free `big-pickle` model at $0. A scratch session of opencode 1.19.3 run for this change (`/tmp/oc-run`, isolated from the author's store) holds a main session and an `explore` subagent with 20 requests.

## What Changes

- **A new agent `opencode`.** tokprof finds OpenCode's stores and reads each session's requests, tokens, models, times, compactions, subagents and OpenCode's own cost. Every command that takes `--agent` accepts `opencode`, and the default list gains it.
- **Where the stores are.** `$XDG_DATA_HOME/opencode`, by default `~/.local/share/opencode`: `opencode.db` and the channel stores `opencode-<channel>.db`, but not `opencode-next.db`, which 2.x imports. `$OPENCODE_DB` names another store. A store is opened read-only and never written, also while OpenCode writes it.
- **A session in a store is a log.** Its size is the size of its message rows. Lists, the line about the logs read, the X-ray and the day view show it like a log file; paths printed for it are the store's.
- **Both layouts.** A session in both layouts is one session. Its messages are the union of both by id; where both hold a message, the 1.x row is read, since 2.x's copy drops the usage of compaction calls.
- **Requests.** A 2.x `assistant` message with non-zero `tokens` is one request; a 1.x assistant message gives one request per `step-finish` part, else one from its own `tokens`. `input` is uncached input, `cache.read` reads, `cache.write` short-lifetime writes, and `output` plus `reasoning` the output.
- **Compactions.** A 2.x `compaction` message and a 1.x user message with a `compaction` part mark the next request, with the kind `auto` or `manual`. The usage of the compaction is a compaction call.
- **Subagents.** A session with a `parent_id` is a subagent log of the topmost session above it, as the `task` tool starts one.
- **Forks.** A fork copies the messages of the session it came from with their times; messages older than the session are left out, as OpenCode's own stats do.
- **OpenCode's own cost.** The sum of `cost` over the requests is the session's reported cost, compared with tokprof's total in `tokprof summary`, never used as a price.
- **Live updates.** `tokprof ui` follows the stores: a session OpenCode writes to is costed again, the others in the store keep their rows, and a session deleted from its store leaves the list.
- **The source.** A new log source under `sources/opencode`, with synthetic fixtures built from SQL text at test time, the conformance test and a README that names the stores, the fields and the versions.

## Capabilities

### New Capabilities
None.

### Modified Capabilities
- `session-logs`:
  - changes `Log locations`, `Session id and project`, `Compaction marker lines`, `Agent-reported cost` and `Agent selection`;
  - adds `OpenCode stores`, `OpenCode sessions`, `OpenCode requests and usage`, `OpenCode compactions` and `OpenCode's own cost`.
- `session-browser`: changes `Live updates`.

## How it is checked

- `tokprof --agent opencode summary` on the author's store shows 2 sessions and 4 requests with 15,522 uncached, 19,219 read, 0 written and 296 output tokens, the sums `sqlite3` gives over the same rows.
- On the scratch store of opencode 1.19.3: 2 logs (one a subagent), 20 requests, 35,103 uncached, 142,400 read and 1,255 output tokens (1,130 output plus 125 reasoning), the sums over its `step-finish` parts.
- A synthetic store with Anthropic prices and `cost`: tokprof's dollars stay within 1.5% of OpenCode's own sum.
- `tokprof` with no arguments still answers in under a second, and the full corpus parses in about the same time.

## Out of scope

- The JSON files of opencode before 1.x (`storage/session`, `storage/message`, `storage/part`): none on this machine to check against, and current opencode no longer reads them.
- Rebuilt prompts, tools, triggers, hand-offs, interrupts and share names for OpenCode: `cache-misses` reports OpenCode's misses with the cause `unknown (server side?)`.
- Title-generation calls, which OpenCode does not store per call.

## Impact

- New crate `sources/opencode`, with a bundled SQLite library; one feature and one line in `crates/tokprof-sources`.
- The framework learns sessions that share one file: a log is known by its locator (file, directory or row of a store), not only by its path, in the kept logs, the live watcher and the session browser; a change of a store names the sessions it changed and all it holds. The watcher looks again only at the files a source called logs. File logs behave as before.
- `README.md`: OpenCode in the list of agents, with its store location.
- No change to Claude Code, Codex or pi results.
