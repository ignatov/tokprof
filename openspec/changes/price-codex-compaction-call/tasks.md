# Tasks

Every group ends with `cargo test --workspace` green, in a state that can be committed.

## 1. Before starting

- [ ] 1.1 Record the baseline in the results table at the end of this file, with a release build of main at 5824258 or later:
  - `tokprof --agent codex --since 30d misses` and `tokprof --agent claude --since 30d misses`, the cause tables;
  - the total avoidable of `tokprof --since 30d summary`;
  - the tables and savings lines of `tokprof --agent codex --since 30d bench` and of `tokprof --agent claude --since 30d bench`;
  - the median of 3 timed runs of `tokprof summary` over all logs.

  Verify: the table has the numbers and the date.

## 2. The compaction call in the Codex source

- [ ] 2.1 On a `compacted` line with `compaction_response_id`, mark the stream-0 request whose `response_id` matches as a compaction call. Verify: decoder unit tests for "A Codex compaction call" and "An id no usage line has": no id, and an id that no usage line has.
- [ ] 2.2 In `sources/codex/README.md`, name `compacted.compaction_response_id` and the versions: matched in 0.153–0.159, missing in 0.152, and earlier rollouts with `token_count` only. Also say that the compaction calls seen are local and sent without the conversation's tools, so they read nothing from the cache. Verify: the README has the field, the versions and the reason.

## 3. The cause

- [ ] 3.1 Add the cause `compaction call` after `compaction`, outside the expected rebuilds. Remove `after assistant message`, the request field behind it, the decoder's `ended_with_message`, and the decoder test added in 5824258. Verify: unit tests next to the cause rules for "A Codex compaction call", "The request after the compaction", "A compaction call after a break" and "A Codex miss after an answer"; `ended_with_text` still gives the `continuation` trigger in its existing tests.
- [ ] 3.2 Give `compaction call` the hint "each compaction wrote the whole context again; start a new session from a handoff before the context fills". Verify: a unit test of the cache miss advice with the spec's "Compaction calls" line; a What to change test where a `compaction call` row gets the hint as a sentence; "Hints by cause" gives "the logs do not show the cause" for `unknown (server side?)`.

## 4. A warm compaction that writes the context

- [ ] 4.1 Add the agent capability "its compaction call reads the cache": true for Claude Code, false for Codex. Verify: the conformance tests and `crates/tokprof/tests/architecture.rs` pass.
- [ ] 4.2 In the X-ray, price a warm `/compact` at the write price of the conversation's cache lifetime when the capability is false. Verify: a unit test for "Warm cache on Codex", ≈ $3.38 for a 230.0k-token `gpt-6-astra` prompt; the Claude Code scenarios of How to carry on keep their numbers.
- [ ] 4.3 In the bench, with the capability false: /compact first reads nothing from the cache, and the warm compaction writes the context, both in the fifth way and on breaks that were not returns. Verify: unit tests for the "Codex" scenario of /compact first and for "A Codex return after the compaction", $3.00 each; the existing Claude Code bench tests keep their numbers.
- [ ] 4.4 Write the report's last note and the chart's notes as the return-bench delta says. The heartbeats-then-compaction note names, by the agents' titles, where the warm compaction reads the context and where it writes it. A run with only Claude Code keeps today's line. Verify: render_bench tests with the spec's lines for "The reference run", "Claude Code alone" and the reference chart's notes.
- [ ] 4.5 In the README's bench section, say that in Codex the compaction call writes the whole context, warm or cold, in the bullets on /compact first and on heartbeats then compaction. Where the README describes how Codex rollouts are read, add that the compaction call is the request `compaction_response_id` names. Verify: both places say so.

## 5. Check on the real logs

- [ ] 5.1 Run `tokprof --agent codex --since 30d misses` and the Claude Code one. Verify: `compaction call` has 171 misses and about $264; `unknown (server side?)` has 3 and about $1; the other causes and the total avoidable of `summary` equal the baseline of 1.1. Record the cause tables.
- [ ] 5.2 Run `tokprof scan 01a0eec6`, the Codex session with the most compaction calls. Verify: its cache miss advice names `compaction call` with the new hint. Record the line.
- [ ] 5.3 Run `tokprof --agent codex --since 30d bench` and the Claude Code one. Verify: the Codex saving of heartbeats then compaction falls from $13.85 by $3–17; /compact first stays near $45.85; the Claude Code output equals the baseline. Record the Codex table and lines.
- [ ] 5.4 Time `tokprof summary` over all logs 3 times. Verify: the median is within 2% of the baseline.

## 6. A fresh reference run

- [ ] 6.1 Make `docs/bench.txt`, `docs/bench.svg` and `docs/bench-dark.svg` again with `tokprof bench --svg docs/bench.svg` over all logs, with the date in the first line of `docs/bench.txt`. Verify: the files are written; render a square-viewBox copy of the light chart and look at it.
- [ ] 6.2 Update the README's bench numbers to the new run: the chart's alt text, the table and the bullets under it. Verify: every number in that section appears in `docs/bench.txt`.
- [ ] 6.3 Update the numbers that return-bench quotes from the reference run. They go in this change's MODIFIED blocks for "The report states its assumptions" and "Chart labels". Add MODIFIED blocks, each copied whole from the main spec, for those of the other six requirements whose quoted numbers changed:
  - The base the alternatives start from
  - Per-agent table
  - What the alternatives would have saved
  - Forecast check
  - What heartbeats then compaction would have saved
  - What the breaks that were not cold returns add

  Verify: `openspec validate price-codex-compaction-call --strict` passes, and each quoted number appears in `docs/bench.txt`.

## 7. Finish

- [ ] 7.1 Run `cargo fmt --all`, `cargo test --workspace` and `cargo clippy --workspace --all-targets -- -D warnings`, then `cargo build` in `crates/tokprof-desk`. Verify: all pass.
- [ ] 7.2 Reinstall the binary with `cargo install --path crates/tokprof --root ~/.local`. Verify: `tokprof --agent codex --since 30d misses` lists `compaction call`.

## Workflow follow-up

- Simulate the archive in a copy of `openspec/` first. Then archive with `openspec archive price-codex-compaction-call -y` and run `openspec validate --all --strict`.

## Results

| check | baseline | after |
|---|---|---|
