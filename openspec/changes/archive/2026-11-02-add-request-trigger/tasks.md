# Tasks

Every group ends with `cargo test --workspace` green, in a state that can be committed.

## 1. Before starting

- [x] 1.1 Check that `split-core-and-agent-adapters` is applied: `openspec list` shows it complete or archived, and `crates/tokprof-core` has the neutral tool classes. Verify: `cargo test --workspace` passes on the starting commit.
- [x] 1.2 Record the baseline in the results table at the end of this file:
  - the median of 3 runs of `tokprof summary`;
  - the reconciliation line against Claude Code's cost counter;
  - `Total priced` for `--agent claude --since 30d` and `--agent codex --since 30d`.

  Verify: the table has the four numbers and the date of the run.
- [x] 1.3 List the 200 most frequent shell command heads in the author's Claude Code and Codex logs of the last 30 days, with a scratch script outside the repo. Mark the read-only ones under the spec's rule, and note any common read-only program the list misses. Verify: the note is in the results section. A missed program that is frequent changes the spec's list before group 2 starts.

## 2. Read-only rule and call kinds

- [x] 2.1 Add the read-only command rule to the core: split at `&&`, `||`, `;`, `|` and line breaks outside quotes; allow `cd` parts and `NAME=value` prefixes; check the program list, `sed -i`, the `find` actions and the git subcommands; reject redirects other than to `/dev/null` and `&1`/`&2`, `tee`, `<<`, `$(` and backquotes. Verify: unit tests next to the rule cover every scenario of "Read-only commands", and 20 real commands from task 1.3.
- [x] 2.2 Add call kinds (read-only, edit, subagent, run) and the precedence that turns the calls of a response into `edit`, `subagent`, `run`, `lookups` or `lookup`. Verify: unit tests for one read-only call, several read-only calls, the "Mixed calls" scenario, and a response with no calls.
- [x] 2.3 Add the trigger to the request model, with `unknown` when it is not set. The core applies `advisor` from the advisor flag and `compaction` from `compacted_before`, before the decoder's value. Verify: unit tests for each flag, and that advisor counts before compaction.

## 3. Decoders

- [x] 3.1 Claude Code: classify each `tool_use` by its tool and, for `Bash`, its command. Group calls by `message.id`, and set the trigger of the chain's next request from them. Read typed prompts, slash commands, `<task-notification>` and `[Request interrupted` from the lines in between; the latest wins. A subagent chain's first request is `prompt`. Verify: decoder tests for parallel results in separate lines, a notification turn, an interruption, a prompt after an interruption, a retry, a subagent's first request and a legacy transcript without request-audit lines.
- [x] 3.2 Codex: classify `function_call` and `custom_tool_call` items. `apply_patch` is an edit and `spawn_agent` a subagent. For `exec`, count the inner `tools.<name>` calls and read `cmd:` string literals for `tools.exec_command`. Verify: decoder tests for an `exec` with two read-only commands (`lookups`), one with `apply_patch` (`edit`), one whose `cmd` is not a literal (`run`), and a continuation after an assistant message.

## 4. Summary and profiles

- [x] 4.1 Add the `trigger` dimension: its name in the dimension list, `--help` and the unknown-dimension message, and its frame in every format. Verify: a unit test that `--stack trigger,origin` puts the trigger frame first; `tokprof flame --since 7d --stack trigger,component -o /tmp/t.collapsed` writes frames named after the triggers.
- [x] 4.2 Add the `Spend by trigger` table to `tokprof summary`, after the work tables and before the cache misses, with `trigger`, `requests`, `avg prompt`, `cost`, `per request` and `share`. Verify: a unit test of the row format from the "A lookup row" scenario, and one that the rows sum to the total.
- [x] 4.3 Update README: the trigger in "How cost is attributed", the table in "Reports over all sessions", `trigger` in the `--stack` list of "Flame graphs and profiles". Verify: the README examples run as written.

## 5. X-ray advice

- [x] 5.1 Add the lookups rule after the tool-call hints: requests with trigger `lookup`, subagents included, at least 20 of them, costing at least $5 or 10% of the session. Verify: unit tests for every scenario of "Lookup advice", and that the "Advice block" order holds when the tool-call, lookups and reminders items all fire.
- [x] 5.2 Update README where the X-ray advice is described. Verify: the lookups item appears in the README's list of advice.

## 6. Check on the real logs

- [x] 6.1 Run `tokprof --agent claude --since 30d summary` and record the `Spend by trigger` table. Verify: `lookup` holds about 31.8k requests and about $3.4k, within 15%; the rows sum to `Total priced`; the reconciliation stays within 1.5%.
- [x] 6.2 Run `tokprof --agent codex --since 30d summary` and record its table. Verify: triggers after tool calls (`lookup`, `lookups`, `edit`, `run`, `subagent`) hold about 90% of the cost.
- [x] 6.3 Run `tokprof summary` 3 times. Verify: the median parse time is within 2% of task 1.2.
- [x] 6.4 Scan the session with the most `lookup` cost in the last 30 days. Verify: the X-ray shows the lookups item, and its numbers match `tokprof --session <id> summary`.
- [x] 6.5 Reinstall the binary with `cargo install --path crates/tokprof --root ~/.local`. Verify: `tokprof --version` runs from `~/.local/bin`, and `openspec validate --all --strict` passes.

## Results

Run on 2026-11-02 on the author's logs. The baseline binary was built from commit 5217081.

| Check | Baseline | After |
|---|---|---|
| `tokprof summary` median parse time, 3 runs each, interleaved | 24.10 s | 24.43 s (+1.4%) |
| Reconciliation with Claude Code's counter, all history | 282 sessions: $10,409 vs $10,542 | 282 sessions: $10,409 vs $10,545 (one live session grew in between) |
| Claude Code, 30d, `Total priced` | $9,226 | $9,235 (same as the baseline binary run at the same time) |
| Claude Code, 30d, reconciliation | 280 sessions: $8,685 vs $8,814 | unchanged, 1.5% |
| Codex, 30d, `Total priced` | $1,979 | $1,979 |
| Claude Code, 30d, `lookup` | 31.8k requests, $3.4k (measurement script) | 37,169 requests, $3,718, 40.3% |
| Claude Code, 30d, rows sum to `Total priced` | | $9,235.05 for $9,235 |
| Codex, 30d, triggers after tool calls | about 90% | 95.5% (`run` 71.2%, `edit` 13.5%, `lookups` 7.1%, `lookup` 3.7%) |
| X-ray of the session with the most `lookup` cost | | "Single lookups made 4953 requests, each re-reading ≈ 204.8k tokens: $432, 82% of the session", the same as its `summary` row |

Task 1.3: the most frequent programs in 30 days of Claude Code shell commands are `git`, `head`, `awk`, `cd`, `echo`, `for`, `sed`, `if`, `cat`, `cut`, `python3`, `tail`, `grep`, `ls`. The spec's first list missed `tr`, `basename`, `rg.cmd` and other read-only filters, and loops and conditions (`for` and `if` start about 24k parts). The list and the keyword rule were added before group 2.

Task 6.1: `lookup` has 17% more requests than the measurement script found, and 9.6% more cost. The script matched only the first command against a shorter list. The rule also counts `echo`, filters such as `tr` and `jq`, loops and conditions around reads, and `rg.cmd`. The cost is within 15%, the request count is not.

## Workflow follow-up

- `/opsx:archive` merges the deltas into `openspec/specs/cost-attribution` and `openspec/specs/session-xray`.
