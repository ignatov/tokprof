# Tasks

Every group ends with `cargo test --workspace` green, in a state that can be committed.

## 1. Before starting

- [x] 1.1 Record the baseline in the results table at the end of this file, with a release build of main at 5824258 or later:
  - `tokprof --agent codex --since 30d misses` and `tokprof --agent claude --since 30d misses`, the cause tables;
  - the total avoidable of `tokprof --since 30d summary`;
  - the tables and savings lines of `tokprof --agent codex --since 30d bench` and of `tokprof --agent claude --since 30d bench`;
  - the median of 3 timed runs of `tokprof summary` over all logs.

  Verify: the table has the numbers and the date.

## 2. The compaction call in the Codex source

- [x] 2.1 On a `compacted` line with `compaction_response_id`, mark the stream-0 request whose `response_id` matches as a compaction call. Verify: decoder unit tests for "A Codex compaction call" and "An id no usage line has": no id, and an id that no usage line has.
- [x] 2.2 In `sources/codex/README.md`, name `compacted.compaction_response_id` and the versions: matched in 0.153–0.159, missing in 0.152, and earlier rollouts with `token_count` only. Also say that the compaction calls seen are local and sent without the conversation's tools, so they read nothing from the cache. Verify: the README has the field, the versions and the reason.

## 3. The cause

- [x] 3.1 Add the cause `compaction call` after `compaction`, outside the expected rebuilds. Remove `after assistant message`, the request field behind it, the decoder's `ended_with_message`, and the decoder test added in 5824258. Verify: unit tests next to the cause rules for "A Codex compaction call", "The request after the compaction", "A compaction call after a break" and "A Codex miss after an answer"; `ended_with_text` still gives the `continuation` trigger in its existing tests.
- [x] 3.2 Give `compaction call` the hint "each call wrote the whole context again; hand off to a new session before the context fills". Verify: a unit test of the cache miss advice with the spec's "Compaction calls" line; a What to change test where a `compaction call` row gets the hint as a sentence; "Hints by cause" gives "the logs do not show the cause" for `unknown (server side?)`.

## 4. A warm compaction that writes the context

- [x] 4.1 Add the agent fact "its compaction call reads the cache" (`Harness::compaction_reads_cache`, see design 4): true for Claude Code, false for Codex. Verify: the conformance tests and `crates/tokprof/tests/architecture.rs` pass.
- [x] 4.2 In the X-ray, price a warm `/compact` at the write price of the conversation's cache lifetime when the capability is false. Verify: a unit test for "Warm cache on Codex", ≈ $3.38 for a 230.0k-token `gpt-6-astra` prompt; the Claude Code scenarios of How to carry on keep their numbers.
- [x] 4.3 In the bench, with the capability false: /compact first reads nothing from the cache, and the warm compaction writes the context, both in the fifth way and on breaks that were not returns. Verify: unit tests for the "Codex" scenario of /compact first and for "A Codex return after the compaction", $3.00 each; the existing Claude Code bench tests keep their numbers.
- [x] 4.4 Write the report's last note and the chart's notes as the return-bench delta says. The heartbeats-then-compaction note names, by the agents' titles, where the warm compaction reads the context and where it writes it; on the chart that is a fourth note. A run with only Claude Code keeps today's line. Verify: render_bench tests with the spec's lines for "The reference run", "Claude Code alone", the reference chart's notes and "Notes for Claude Code alone".
- [x] 4.5 In the README's bench section, say that in Codex the compaction call writes the whole context, warm or cold, in the bullets on /compact first and on heartbeats then compaction. Where the README describes how Codex rollouts are read, add that the compaction call is the request `compaction_response_id` names. Verify: both places say so.

## 5. Check on the real logs

- [x] 5.1 Run `tokprof --agent codex --since 30d misses` and the Claude Code one. Verify: `compaction call` has 171 misses and about $264; `unknown (server side?)` has 3 and about $1; the other causes and the total avoidable of `summary` equal the baseline of 1.1. Record the cause tables.
- [x] 5.2 Run `tokprof scan 01a0eec6`, the Codex session with the most compaction calls. Verify: its cache miss advice names `compaction call` with the new hint. Record the line.
- [x] 5.3 Run `tokprof --agent codex --since 30d bench` and the Claude Code one. Verify: the Codex saving of heartbeats then compaction falls from $13.85 by $3–17; /compact first stays near $45.85; the Claude Code output equals the baseline. Record the Codex table and lines.
- [x] 5.4 Time `tokprof summary` over all logs 3 times. Verify: the median is within 2% of the baseline.

## 6. A fresh reference run

- [x] 6.1 Make `docs/bench.txt`, `docs/bench.svg` and `docs/bench-dark.svg` again with `tokprof bench --svg docs/bench.svg` over all logs, with the date in the first line of `docs/bench.txt`. Verify: the files are written; render a square-viewBox copy of the light chart and look at it.
- [x] 6.2 Update the README's bench numbers to the new run: the chart's alt text, the table and the bullets under it. Verify: every number in that section appears in `docs/bench.txt`.
- [x] 6.3 Update the numbers that return-bench quotes from the reference run. They go in this change's MODIFIED blocks for "The report states its assumptions" and "Chart labels". Add MODIFIED blocks, each copied whole from the main spec, for those of the other six requirements whose quoted numbers changed:
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

The baseline was taken on 2026-11-07 at 14:00 with a release build of 8b1f183. The after column is filled as the checks run.

| check | baseline | after |
|---|---|---|
| Codex misses, 30d | unknown (server side?) 174, 38.9M, $265; idle > TTL 49, 4.4M, $29.12; params changed 3, 406.8k, $0.98; model switch 3, 209.5k, $0.77 | compaction call 171, 38.7M, $264; idle > TTL 49, 4.4M, $29.12; params changed 3, 406.8k, $0.98; model switch 3, 209.5k, $0.77; unknown (server side?) 3, 213.4k, $0.51 |
| Claude Code misses, 30d | idle > TTL 230, 99.0M, $773; history edited 66, 29.2M, $228; images removed 8, 3.6M, $28.22; unknown (server side?) 5, 972.8k, $7.93; tools changed 1, 49.9k, $0.39 | the same, run beside the baseline binary at 14:20 |
| summary, 30d | avoidable on cache misses $1,333 (11.3%) of $11,842 | $1,333 with both binaries at 14:25, of $11,858 by then (11.2%) |
| Codex bench, 30d: table | 16 returns, median 180.5k. resume as is $1.43, $3.40, $4.08, $65.31; /compact first $1.88, $2.55, $2.87, $45.85; handoff note $0.61, $1.28, $1.59, $25.44; heartbeats, then resume $2.05, $4.03, $4.71, $75.31; heartbeats, then compaction $1.52, $2.61, $3.22, $51.46 | the same but heartbeats, then compaction $2.35, $3.45, $4.05, $64.81 |
| Codex bench, 30d: heartbeats then compaction | kept 8 of 16 warm and compacted 8, for $21.40 of pings and compactions, saved $13.85 of $65.31 (21%); breaks that were not returns: another 122 pings and 8 compactions for $9.16, then it saves $4.69 (7%) | for $34.75 of pings and compactions, saved $0.50 of $65.31 (1%), $13.35 less; breaks that were not returns: 122 pings and 8 compactions for $14.75, then it costs $14.25 more than resuming |
| Codex bench, 30d: /compact first | cheaper than resuming from request 3 on ($19.46 over all) | the same, $45.85 over all |
| Claude Code bench, 30d | 206 returns, median 480.3k. resume as is $3.85, $4.94, $8.11, $1,672; /compact first $4.39, $4.85, $6.29, $1,295; handoff note $0.73, $1.18, $2.62, $540; heartbeats, then resume $2.01, $3.10, $6.28, $1,294; heartbeats, then compaction $0.79, $1.62, $4.06, $836; it saved $836 of $1,672 (50%), and $693 (41%) with the other breaks | the same, run beside the baseline binary at 14:18 |
| summary over all logs, 3 runs | 49.3 s, 35.6 s, 26.1 s: median 35.6 s, with a load average of 27 from other work on the machine | base and new in turn, twice: wall 26.7, 34.1, 34.5 s against 33.4, 31.8, 31.0 s, then 35.4, 37.7, 45.8 s against 41.5, 30.7, 47.8 s, at load averages of 15 to 26; CPU in the second round 61.1, 64.0, 64.0 s against 64.1, 54.0, 66.2 s: medians 64.0 and 64.1 s (+0.2%). The wall times vary by 20% with the load, so the CPU medians are the measure |
| X-ray of 01a0eec6, cache miss advice | | Cache misses could have saved $357, mostly compaction call (185×): each call wrote the whole context again; hand off to a new session before the context fills. |
| Reference run over all logs (docs/bench.txt) | 2026-11-03: Claude Code 218 returns, Codex 204; Codex heartbeats, then compaction $653, saved $159 (20%), and with the other breaks it cost $53.73 more than resuming | 2026-11-07: Claude Code 234 returns (the logs grew), Codex 204; Codex heartbeats, then compaction $730, saved $81.44 (10%), and with the other breaks it costs $257 more; the other Codex ways are unchanged. The chart, rendered square, shows the four notes within its width |
