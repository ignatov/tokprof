# Tasks

Every group ends with `cargo test --workspace` green, in a state that can be committed.

## 1. Before starting

- [x] 1.1 Record the baseline in a results table at the end of this file. Take the current bench output for `--agent claude --since 30d` with the default window and with `--keep-warm 2h`, and the median of 3 runs of `tokprof bench` over all logs. Verify: the table has the numbers and the date.

## 2. Breaks that were not cold returns

- [x] 2.1 Make the bench scan record the breaks that are not counted returns, and the latest request time it read. Verify: unit tests for "The cache was still warm", "A small context" and "A short break".
- [x] 2.2 Price their pings for heartbeats, and their pings and compactions for the fifth way, with open breaks ending at the latest request or `--until`. Verify: unit tests for "The end of the logs", with and without `--until`.

## 3. The fifth way

- [x] 3.1 Add `heartbeats, then compaction` to the replay: the heartbeat pings, the compaction at the next slot past the window, a warm first request before it, and the base after it. Verify: unit tests for "A return inside the window", "A return after the compaction" and "No window"; the four existing ways give the same costs in the existing tests.

## 4. Report and chart

- [x] 4.1 Print the fifth row in the per-agent table, the two new savings lines for heartbeats and the fifth way, and the new note. Verify: unit tests with the spec's lines for "The lines for pings on breaks that were not cold returns" and "The lines for heartbeats then compaction", and the reference-run row names.
- [x] 4.2 Draw the fifth series in the chart, dash-dotted in the next palette color, in the legend, end labels, tooltips and screen-reader description, in both themes. Verify: the chart tests cover five legend entries and the line style. Render a square-viewBox copy and look at it, as the chart notes in the README say.

## 5. Check on the real logs

- [x] 5.1 Run `tokprof bench --agent claude --since 30d --keep-warm 2h`. Verify:
  - heartbeats and the fifth way keep the same returns warm (about 93);
  - all pings of heartbeats come to about the study's $62;
  - the fifth way's compacted returns cost about what the handoff note's do;
  - record the output.
- [x] 5.2 Run the same with the default window. Verify: all pings of heartbeats come to about the study's $168; record the net lines.
- [x] 5.3 Check that resume, /compact first and the handoff note give the baseline's numbers from 1.1. Verify: equal to the cent.
- [x] 5.4 Regenerate `docs/bench.txt`, `docs/bench.svg` and `docs/bench-dark.svg`. Update the README numbers and the reference-run numbers in this change's return-bench delta to match. Verify: `openspec validate add-bench-warm-then-compact --strict` passes, and the README states the same numbers as `docs/bench.txt`.
- [x] 5.5 Time `tokprof bench` 3 times. Verify: the median is at most 10% above the baseline.
- [x] 5.6 Reinstall the binary with `cargo install --path crates/tokprof --root ~/.local`. Verify: `tokprof bench --keep-warm 2h --since 7d` prints five rows.

## Results

All on 2026-11-03; the baseline binary is at a00b301.

| check | baseline | after |
|---|---|---|
| `tokprof bench`, all logs, 3 runs | 22.54 s, 22.36 s, 22.58 s; median 22.54 s | 22.58 s, 21.90 s, 22.84 s; median 22.58 s (+0.2%) |
| `--agent claude --since 30d bench`, 202 returns, all returns | resume $1,631, /compact first $1,267, handoff note $511, heartbeats $1,264 | the same four to the cent; heartbeats then compaction $795 |
| same, heartbeats 6h | kept 122 warm, $83.95 of pings, saved $367 (23%) | the same, then 977 pings for $83.21 through other breaks: $167.16 of pings in all (study: $168), net $284 (17%) |
| same, heartbeats then compaction 6h | | kept 121 warm, compacted 81, $108 sent, saved $836 (51%); with 977 pings and 131 compactions elsewhere ($120), net $716 (44%) |
| same with `--keep-warm 2h`, heartbeats | $1,337; kept 93 warm, $34.98 of pings, saved $294 (18%) | the same, then 314 pings for $27.05 elsewhere: $62.03 of pings in all (study: $62), net $267 (16%) |
| same with `--keep-warm 2h`, heartbeats then compaction | | kept 90 warm and compacted 112, $68.97 sent, saved $949 (58%); with 314 pings and 133 compactions elsewhere ($64.65), net $884 (54%) |
| 5.1 warm returns: heartbeats against the fifth way | | 93 and 90: the fifth way compacts at 150 minutes, so 3 returns between 150 and 160 minutes, which the second ping keeps warm, come back to a compaction instead |
| 5.1 compacted returns against the handoff note | | the same requests on the same base (test `heartbeats_then_compaction_compacts_past_the_window`); the fifth way adds the pings before the compaction |
| 5.4 reference run, `tokprof bench --svg docs/bench.svg` | 2026-11-01: Claude Code 210 returns, Codex 204 | 2026-11-03: Claude Code 218 returns (resume $8.19, /compact $6.33, note $2.55, heartbeats $6.30, heartbeats then compaction $4.03 after 60 requests), Codex 204 (unchanged, fifth way $3.20) |
