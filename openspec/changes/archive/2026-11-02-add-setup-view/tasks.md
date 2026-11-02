# Tasks

## 1. What counts as setup

- [x] 1.1 Add the reminder kinds attached files and task list to the core, map their labels, and add a rule that tells whether a block is setup (tool schemas, system prompt, reminders other than background notifications, attached files and task list); verify with unit tests next to the rule, and that the advice tests still pass unchanged
- [x] 1.2 Add the chain's kind to the facts that analysis emits; verify with a unit test that a subagent chain's shares carry the subagent kind

## 2. Setup cost

- [x] 2.1 Add a setup sink that sums the cost and tokens of setup shares by agent and source, with the subagent part apart; verify with unit tests for a main and a subagent share, for an unpriced share (tokens only) and for merging two sinks
- [x] 2.2 Feed the sink from the summary collector and from the X-ray's sessions; verify with a test that the X-ray's setup total equals the sum of its setup content

## 3. What a chain starts with and what a compaction leaves

- [x] 3.1 Turn the live context's prompt replay into a function that lays out the prompt of a given request of a chain, and make the live context use it; verify that the existing live-context tests pass unchanged
- [x] 3.2 Compute what a chain starts with (setup by source in its first request with a known prompt, and the brief) and the subagents' means; verify with unit tests for a subagent with and without a known first prompt
- [x] 3.3 Compute what the main conversation's latest compaction leaves (the first answered request after the marker, by kind and reminder source); verify with unit tests for a compaction with a known next prompt and for none

## 4. Hints from the sources

- [x] 4.1 Add setup hints to the agent facts, and fill them in the Claude Code source (skills, instructions, deferred tool lists, hook context) and the Codex source (instructions); verify with the sources' conformance tests and that `crates/tokprof/tests/architecture.rs` passes
- [x] 4.2 Record in `sources/claude-code/README.md` the settings the hints name and the version they were seen in (2.1.316), and in `sources/codex/README.md` the Codex hint; verify the READMEs name them

## 5. The X-ray block

- [x] 5.1 Build the X-ray's setup data: the threshold ($1 and 5%, or 5% of tokens), the rows by cost with up to 8 sources and `<n> more`, the subagent part, `main starts`, `each subagent`, the subagent count and mean brief, the compaction parts (4 largest and `<n> more`) and the hint of the costliest source; verify with unit tests for each scenario of the Setup block and Setup table requirements
- [x] 5.2 Render the block after `tools`, leaving out the subagent columns when there are none and showing `–` for an unknown main start, and wrap the hint like an advice item; verify with render tests in `render_xray.rs`, and with `tokprof ui --dump` that the block shows in the browser
- [x] 5.3 Update the README's X-ray section with the setup block and an example; verify the example matches a real run

## 6. The summary table

- [x] 6.1 Render `Setup: what every request carries before the work` after the spend by activity, with `agent`, `source`, `cost`, `subagents`, `share` and `hint`, and the line per agent; leave the section out when it is empty; verify with render tests in `render_report.rs`
- [x] 6.2 Update the README's report section for the new table; verify the example matches a real run

- [x] 6.3 Group the summary's setup rows by agent, the costliest agent first, and fold an agent's sources after its 8th into one `<n> more` row; verify with a render test in `render_report.rs` and on the 30-day summary, where Codex shows 8 rows and a `more` row
  - Added at the author's request after 7.1 showed 21 Codex rows. On the 30-day summary: Claude Code's 7 rows, then Codex's 8 and `codex  13 more  $10.42  $0.00  0.5%`.

## 7. Checks on the real logs

- [x] 7.1 Run `tokprof --since 30d summary` and record the setup rows against the proposal: Claude Code instructions about $249, tool schemas $198, skills $197, other $106, deferred tool lists $76, hook context $53, system prompt $36, $915 in total and 10% of its spend
  - Run on 2026-11-02. Claude Code: instructions $249 ($150 in subagents), tool schemas $199 ($64.88), skills $197 ($114), other $106 ($44.10), deferred tool lists $75.79 ($44.31), hook context $52.61 ($21.64), system prompt $36.42 ($11.58). `Claude Code setup: $915, 10.0% of its spend`. All match the proposal.
  - Codex: `Codex setup: $103, 5.3% of its spend`, led by the system prompt ($40.65). Its reminder sources gave 21 rows, most under $1, which made the table long; 6.3 folds them.
- [x] 7.2 Run `tokprof scan 03aee587`, `tokprof scan 0268e98f` and `tokprof scan 3edfd2e2`, and record the block against the proposal: about $74 (14%) with 96 subagents at 31.2k for 03aee587; 75 subagents at 26.6k for 0268e98f; 134.9k after the 2026-10-13 15:31 compaction for 3edfd2e2
  - 03aee587: `$73.89, 14% of the session`, subagents $70.24; 96 subagents start with 31.7k on average (instructions 10.9k, skills 9.4k, tool schemas 6.1k, deferred tool lists 2.9k) and a 2.1k brief; the main conversation starts with 38.0k. The hint is the one for instructions, which cost the most ($23.55 against $21.92 for skills).
  - 0268e98f: `$94.22, 12% of the session`; 75 subagents at 26.7k (instructions 12.2k, tool schemas 5.6k, skills 3.6k, deferred tool lists 2.9k); after the compaction at 10-23 11:00, the next prompt held 46.1k: tool schemas 14.2k, instructions 12.0k, attached files 5.3k, user prompt 5.1k, 6 more 9.4k.
  - 3edfd2e2: no block, as the spec says for a small setup. $300 of its $390 is `unknown`, from before the session wrote request-audit lines, and the setup the log shows is $0.96 (0.2%). Its compaction at 2026-10-13 15:31 also falls in that part, so the request after it has no known prompt. The proposal's expectation for it was wrong; 0268e98f shows the compaction line instead.
  - The subagent means are 2–12% off the proposal's script, which sized blocks from the raw blobs; tokprof sizes them from the replayed prompt, unresolved messages included.
- [x] 7.3 Measure the full `tokprof summary` run and `tokprof scan 03aee587` before and after, and check that the spend totals and the cost-state reconciliation line are unchanged
  - Full `tokprof summary`, the installed binary against the new one, alternating: 25.06 s and 25.66 s before, 25.15 s and 25.27 s after. `tokprof scan 03aee587`: 0.39–0.43 s before, 0.42–0.43 s after. No change beyond the run-to-run spread.
  - Apart from the new section, the two summaries differ only by the 3 requests a live session wrote between the runs. The reconciliation line reads `Claude Code's own cost counter for 286 sessions: $10,502; tokprof for the same sessions: $10,607` in both, as at the start.
- [x] 7.4 Run `cargo test --workspace` and `openspec validate --all --strict`, then reinstall the binary with `cargo install --path crates/tokprof --root ~/.local`
  - All tests pass, `cargo clippy --workspace --all-targets` is clean, `openspec validate --all --strict` passes 12 of 12, and `~/.local/bin/tokprof` was reinstalled; `tokprof scan 03aee587` shows the block.
