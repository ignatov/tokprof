# Design

## Log fields (pi 0.84.2, session format version 3)

- **Header:** the first line, `type: "session"`, with `version`, `id` (a UUID), `timestamp` and `cwd`. A fork names `parentSession`, the path of the file it came from.
- **Entries:** every line has `type`, `id`, `parentId` and an ISO `timestamp`. The entries form a tree: going back with `/tree` adds a branch to the same file. Every line is still a call that was made, so tokprof counts all branches, not only the active one.
- **Responses:** `type: "message"`, `message.role: "assistant"`, with `provider`, `model`, `api` (such as `anthropic-messages`, `openai-completions`), `responseId`, `stopReason` (`stop`, `toolUse`, `aborted`, `error`), `durationMs` and `usage`.
- **Usage:** `input` (uncached; `totalTokens` = `input` + `output` + `cacheRead` + `cacheWrite` in the logs seen), `output`, `cacheRead`, `cacheWrite`, `reasoning`, `cacheWrite1h` (seen in pi-durable only), and `cost` with `input`, `output`, `cacheRead`, `cacheWrite`, `total` and `totalFromProvider`.
- **Model:** `model_change` lines (`provider`, `modelId`) and each response's own `model`. A request uses its response's `model`.
- **Compaction:** `type: "compaction"` with `summary`, `firstKeptEntryId`, `tokensBefore` and `usage`; `branch_summary` has `usage` too. Neither names a model.
- **Aborted responses:** `stopReason: "aborted"` with all-zero usage, such as a request stopped before its first token. It adds no request.

## Choices

- **Session id:** the header's `id`, else the UUID at the end of the file name. **Project:** the header's `cwd`, with `~`.
- **A compaction call's model:** the `model` of the latest response before it in the file, else the latest `model_change`, else `unknown`. pi compacts with the session's current model.
- **Cost:** pi's `cost.total` is only compared, never used as tokprof's price.
- **Price key:** the model name alone, as for the other agents. `provider: "local"` models get no built-in price.

## Parse time

pi lines are read with the same line reader and a field scan like Codex rollouts. The 7 MB of `~/.pi` adds well under 0.1 s to the 8.5 GB corpus. The other agents' parse is unchanged.

## Open questions, settled

1. **Default list.** pi is in the default `--agent` list, `claude,codex,pi`: a missing `~/.pi` adds nothing, and no reason to leave it out came up. The author kept it.
2. **Forks.** Yes, a fork copies. In pi 0.84.2's `session-manager.js`, `/fork` and `/clone` call `createBranchedSession`, which writes the path from the root to the chosen entry into a new file, each entry with its own `id` and `timestamp` (only `parentId` is re-chained); `--fork` calls `forkFrom`, which copies every entry as it is. The new header names the old file in `parentSession`. So tokprof leaves out an entry of a fork whose `id` and `timestamp` are in the log `parentSession` names (at its path, else next to the fork); a copy still tells the model a later compaction ran on. When that log is gone, the copies count in the fork. The ids are 8 hex characters, so the key has the time too.
3. **Reasoning.** Part of `output`: pi-ai's `Usage` says `reasoning` is "a subset of `output`: `output` already includes these tokens". tokprof counts `output` once and `reasoning` as its share.
4. **pi-durable.** A next change `add-pi-durable-source`: where tokprof finds the stores (no fixed path; a flag or a config list), a read-only SQLite reader with WAL, and how `byTaskId` and child conversations map to subagents.
5. **Messages with role `system`.** One log has a `system` message with `sections` from extensions; it is not a request, and prompts are out of scope here.

## Checked

- `tokprof --agent pi summary` on `~/.pi/agent/sessions`: 2 files, 42 requests (39 replies, 3 compactions), 70.9k uncached + 10.0k rewritten after a miss = 80,859 uncached input, 752.7k read, 0 written, 38.2k output; pi's own cost $0.00 for 1 session, the model unpriced. `tokprof --agent pi sessions` lists both sessions.
- The Anthropic fixtures: tokprof's dollars are within 1.5% of pi's `cost.total` (`tests/conformance.rs`).
- The full corpus (claude 6.7 GB, codex 2.6 GB) parses in 26.5 s before and 26.3 s after; `tokprof` with no arguments answers in 0.3 s.
