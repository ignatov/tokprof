# Tasks

Every group ends with `cargo test --workspace` green, in a state that can be committed.

## 1. Before starting

- [x] 1.1 Commit or keep the fix that reads Claude Code `system` blobs as reminders, so the baseline includes it. Record the baseline in the results table:
  - the median of 3 runs of `tokprof summary`;
  - the reconciliation line;
  - `Total priced` for `--agent claude --since 30d`;
  - the X-ray `in context` line of the 6-message example session.

  Verify: the table has the numbers and the date.
- [x] 1.2 List the 100 most frequent first programs of shell commands that are not read-only, over the last 30 days of Claude Code and Codex logs, with a scratch script outside the repo. Mark each with its class under the spec's lists. Verify: the note is in the results section; a frequent program that falls into `file and system commands` but plainly builds, tests or runs scripts changes the spec's lists before group 2.

## 2. Call classes

- [x] 2.1 Turn the four call kinds into the nine call classes, with the trigger mapping them back. Verify: the existing trigger tests pass unchanged, and a new test maps every class to its trigger.
- [x] 2.2 Add the shell command class to the core: the read-only rule first, then the first program after `cd`, assignments and keywords, with its path dropped, against the spec's lists. File writes through `>`, `tee`, heredocs and in-place edits give `editing`. Verify: unit tests cover every scenario of "Activity of a shell command", and 15 real commands from task 1.2.
- [x] 2.3 Add the activity to the request model: the class of the calls that gave the trigger, or the trigger's name. Advisor and compaction override it. Verify: unit tests for a call activity, a prompt, an advisor call and a compaction.

## 3. Sources

- [x] 3.1 Claude Code: classify each `tool_use` into a class, and keep the class with the trigger. Keep the typed messages of the main conversation with their time and label, including slash commands as name and arguments. Verify: decoder tests for mixed calls (`building and testing`), a heredoc write (`editing`), a typed message after a pasted block (its last paragraph), a slash command, and a subagent transcript with no typed messages.
- [x] 3.2 Codex: classify function calls and the inner calls of `exec`. Keep typed user messages, but not those replayed from a `compacted` history. Verify: decoder tests for an `exec` running `cargo test` (`building and testing`), `apply_patch` (`editing`), and a compaction that replays an old prompt without adding a typed message.

## 4. Summary and profiles

- [x] 4.1 Add the `activity` dimension: the dimension list, `--help`, the unknown-dimension message, and frames in every format. Verify: a unit test with `--stack activity,component`; `tokprof --since 7d flame --stack activity -o /tmp/a.collapsed` writes activity frames.
- [x] 4.2 Add the `Spend by activity` table after `Spend by trigger`. Verify: a unit test of the row format and of the rows summing to the total.
- [x] 4.3 Update README: activities in "What triggered a request", the table in "Reports over all sessions", `activity` in the `--stack` list. Verify: the README examples run as written.

## 5. X-ray

- [x] 5.1 Add time and chain to request records, and keep per-request rows in the X-ray's sink only. Verify: a unit test that a subagent chain started after the second message counts with it, and that requests before the first message count with the first.
- [x] 5.2 Add the `prompts` block after the work block: numbered lines with cost, requests, the 40-character label and the two costliest activities; the 8 most expensive in session order, then `<n> more messages`; no block for one message; tokens when unpriced. Verify: unit tests for every scenario of "Prompts block", and that the lines and `<n> more` add up to the session's spend.
- [x] 5.3 Add `by activity` to the work block and show the block when there is more than one activity. Verify: unit tests for the "A coding session" scenario and the "Typed prompts only" scenario.
- [x] 5.4 Change the title of `what cost the most`, and add the call count to tool rows. Verify: a unit test of a row with `412 calls` and one with `1 call`.
- [x] 5.5 Update README where the X-ray is described. Verify: the README's X-ray example shows the prompts block.

## 6. Check on the real logs

- [x] 6.1 Run `tokprof --agent claude --since 30d summary` and record `Spend by activity`. Verify: it sums to `Total priced`; `reading` is within 10 points of `lookup` plus `lookups`; `file and system commands` is under 10%; the reconciliation stays within 1.5%.
- [x] 6.2 Scan the 6-message example session. Verify: 6 prompt lines (or 5 and the one the log cannot resolve), with costs within 10% of $1.86, $8.89, $8.53 and $0.57, and lines that add up to the session's spend.
- [x] 6.3 Run `tokprof summary` 3 times. Verify: the median parse time is within 2% of task 1.1.
- [x] 6.4 Reinstall the binary with `cargo install --path crates/tokprof --root ~/.local`. Verify: `tokprof --version` runs from `~/.local/bin`, and `openspec validate --all --strict` passes.

## Results

| Check | Baseline | After |
|---|---|---|
| `tokprof summary` median parse time | 24.54 s (2026-11-02, after f78e755); 24.59 s interleaved | 24.79 s interleaved (+0.8%) |
| Reconciliation with Claude Code's counter | 282 sessions: $10,409 vs $10,552 | unchanged: the baseline binary gives the same figures at the same moment |
| Claude Code, 30d, `Total priced` | $9,252; 280 sessions: $8,685 vs $8,820 | $9,255; $8,685 vs $8,824, the same as the baseline binary run at the same time (the live session grew) |
| Example session, `in context` line | 430.9k tokens, 6 turns | 474.1k tokens, 6 turns |
| Claude Code, 30d, `Spend by activity` | | reading 49.0% (lookup + lookups 49.4%), prompt 10.6%, running scripts 10.4%, file and system commands 8.7%, editing 8.4%, version control 2.8%, other tools 2.4%, building and testing 1.2%; rows sum to $9,255.27 |
| Example session, prompts block | | 7 lines, every typed message (the one with an image too, since labels come from transcript lines): $1.86, $8.89, $8.53, $0.57, $0.44, $3.76, $3.31; they add up to the $27.37 spend |

Task 6.1: shell writes now give the trigger `edit` instead of `run` (spec updated in this change): `edit` went from 3,564 requests and $469 to 6,293 and $779.

Task 1.2: the first programs of commands that are not read-only, most frequent first: `git`, `awk`, `cat`, `sed`, `python3`, `echo`, `ls`, `bun` by path and as `bun.cmd`, `grep`, `mkdir`, `rg.cmd`, `gh`, `uv`, project tools by path, `perl`, `ffmpeg`, `zsh`, `curl`, `rm`, `node`, `export`, `cp`, `bash`, `cargo`, `ps`, `time`, `timeout`, `bazel.cmd`. Added before group 2: `bazel` to the build list, the wrappers `time`, `timeout`, `env` and `nice`, and the `.cmd` and `.exe` suffix for every list.

## Workflow follow-up

- `/opsx:archive` merges the deltas into `cost-attribution`, `session-xray` and `session-logs`.
