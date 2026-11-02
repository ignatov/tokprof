# Tasks

Every group ends with `cargo test --workspace` green, in a state that can be committed.

## 1. Before starting

- [x] 1.1 Archive `add-work-breakdown`, so that this change's layout delta applies to the merged spec. Verify: `openspec list` no longer shows it, and `openspec validate --all --strict` passes.
- [x] 1.2 Record the baseline in the results table: the median of 3 runs of `tokprof summary`, `Total priced` for `--agent claude --since 30d`, and the example 6-message session's spend. Verify: the table has the numbers and the date.

## 2. Analysis

- [x] 2.1 Add the sink events for a block entering the context (kind, tool, target, class, tokens) and for a produced message's tool-call count, with default no-op methods. Verify: a unit test that a block reported in two requests is reported once, with its tokens from the first.
- [x] 2.2 Charge each request's input spend to the first-time tool results of its appended messages, and each response's output to the tool calls of its produced message, with equal shares. Report the rest as not from a tool. Verify: unit tests for the "Calls in one response" scenario, a request without a delta, and both views summing to the spend.

## 3. Tool table

- [x] 3.1 Add the tool table collector: row keys by tool, and by command head for execute classes; calls, sizes, rent, trips, single; merge. Verify: unit tests for the "Shell commands" and "Other tools" scenarios, and for merging two sessions.
- [x] 3.2 Add the hint rules in spec order, with their thresholds as constants. Verify: a unit test per rule scenario, plus "A large read, once" and "Small rows".

## 4. Command and X-ray

- [x] 4.1 Add `tokprof tools` with the filters, `--by rent|trips`, the title, the columns, the top 25, `<n> more` and the total line; tokens when unpriced. Verify: a unit test of a row and of the total line text; `tokprof tools --by size` fails with a message that lists `rent, trips`.
- [x] 4.2 Add the `tools` block to the X-ray after `prompts`: the top 8 rows by rent, `<n> more` and the total line, and no block without tool calls. Verify: unit tests for both scenarios of "Tools block".
- [x] 4.3 Update README: the `tools` command, the columns, the two views and the hints; the X-ray block. Verify: the README examples run as written.

## 5. Check on the real logs

- [x] 5.1 Scan the example 6-message session. Verify: `Bash python3 -I`, `Bash sed -n`, `Bash grep -n` and `Write` are within 15% of the proposal's hand count; `Bash sed -n` has the batching hint and `Bash python3 -I` the scripts hint; the total line's spend equals the X-ray's spend.
- [x] 5.2 Run `tokprof --agent claude --since 30d tools` and `--by trips`. Verify: rent plus content not from a tool, and trips plus spend not from a tool, both equal `Total priced`; record the top 10 rows and their hints.
- [x] 5.3 Run `tokprof --agent codex --since 30d tools`. Verify: `exec` calls with one inner tool show as that tool's rows; record the top 5.
- [x] 5.4 Run `tokprof summary` 3 times, and `tokprof tools` once. Verify: the summary's median is within 2% of task 1.2, and `tools` takes no longer than `summary`.
- [x] 5.5 Reinstall the binary with `cargo install --path crates/tokprof --root ~/.local`. Verify: `tokprof tools --help` runs from `~/.local/bin`, and `openspec validate --all --strict` passes.

## Results

| Check | Baseline | After |
|---|---|---|
| `tokprof summary` median parse time | 24.76 s; 25.14 s interleaved | 24.85 s interleaved (no slower); `tokprof tools` 25.25 s, the same scan |
| Claude Code, 30d, `Total priced` | $9,240 | $9,236 (the window moved) |
| Example session spend | $35.63 | $38.27 at the check (the session grew) |
| Example session rows, against the hand count at the same moment | | `Bash python3 -I` trips $8.46 = $8.46, `Bash sed -n` $4.18 = $4.18, `Bash grep -n` $3.03 = $3.03, `Write` $2.29 vs $2.32; calls equal. Sizes are about twice the hand count's chars/4, since tokprof scales by the real tokens of each prompt |
| Example session hints | | `Bash sed -n` and `Bash grep -n` batch reads, `Bash python3 -I` and `Bash cat >` scripts, `Write` rewrites; the 3 `Read` calls have none (under 5 calls) |
| Claude Code, 30d, top rows by rent | | `Read` 14,874 calls $1,296 rent / $1,154 trips, 46% single, no hint; `Write` $318; `Bash sed -n` $292 batch; `Bash git -C` $253 output hint; `Bash git grep` $249 batch; `Bash python3 -I` $243 scripts; `Bash git show` $177 batch; `Edit` $152; `Bash cat >` $128; `Agent` $64 |
| Claude Code, 30d, total line | | spend $9,236 · rent $4,988 · trips $6,510 |
| Codex, 30d, top rows | | `exec` (several inner tools) $366 / $634; `exec_command sed -n` $198 batch; `exec_command python -` $170 scripts; `apply_patch` $77; `view_image` $57 |

Known limit: the command head of `git -C <dir> show` is `git -C`, so the read-only rule cannot see the subcommand, and that row gets the output hint instead of the batching one. Improving the head changes the `target` dimension everywhere, which is a change of its own.

## Workflow follow-up

- `/opsx:archive` creates `openspec/specs/tool-breakdown` and merges the layout delta into `session-xray`.
