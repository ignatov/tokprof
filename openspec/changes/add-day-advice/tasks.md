# Tasks

Every group ends with `cargo test --workspace` green, in a state that can be committed.

## 1. The rules

- [x] 1.1 Build the advice items from a rhythm summary, cold returns and instruction files, with the thresholds as named constants. Verify: unit tests for "Cold returns", "Questions waited", "Out of work" and "Step by step", each with the case that gives no item.

## 2. The views

- [x] 2.1 Print the `advice` block after the day view's `cache` line, laid out as the X-ray's advice. Verify: unit tests for "The advice" and "Two items".
- [x] 2.2 Print the block after the rhythm lines of the pause report, from all its agents' cold returns. Verify: a unit test for "The advice" of pause-report.
- [x] 2.3 Describe the block in the README. Verify: the README names the four items and their thresholds.

## 3. Check on the real logs

- [x] 3.1 Run `tokprof day` for each of the 31 days to 2026-11-02. Verify: the items fire on about as many days as the proposal says; record the counts.
- [x] 3.2 Run `tokprof day 2026-10-27` and `tokprof --agent claude --since 30d pauses`. Verify: the items and numbers of the proposal; record them.
- [x] 3.3 Reinstall the binary with `cargo install --path crates/tokprof --root ~/.local`. Verify: `tokprof day` runs.

## Results

| check | proposal | after |
|---|---|---|
| days with the cold-returns item, of the 31 to 2026-11-02 | 21 | 21 |
| days with the questions item | 5 | 6. The proposal read the rounded numbers of the lines. On 2026-10-25 the questions waited a median just over 10 minutes, shown as `10m`. |
| days with the out-of-work item | 19 | 19 |
| days with the step-by-step item | 15 | 16. On 2026-11-01 the turns took a median just under 5 minutes, shown as `5.0m`. |
| items on a day | median 2; 0 to 4 items on 3, 6, 14, 6 and 2 days | median 2; 0 to 4 items on 2, 7, 13, 7 and 2 days |
| `tokprof day 2026-10-27` | cold returns, questions, step by step: 23 returns and $107, 1 question that waited 1.3h, a turn ended every 3.6m after turns of 3.5m | the same three items and numbers. The cold returns' breaks had a median of 3.7h. The questions item names `CLAUDE.md or AGENTS.md`, since the user also typed in a Codex session that day. |
| `tokprof --agent claude --since 30d pauses` | cold returns, out of work, step by step: $750, 22% of a median break, a turn ended every 4.2m after turns of 3.3m | the same three items: 213 returns and $763 after breaks of a median 3.2h, 23% of a median 34m break, 4.2m after 3.3m. The 30 days now end a day later. |
| `tokprof ui`, 190x50, the day of 2026-11-02 | the browser shows the advice with the day | the cold-returns item follows the `cache` line. The strip still takes the rows less 18, so on a day with many sessions the advice is further down and the day scrolls to it. |
| `tokprof day` after the reinstall | runs | runs: the day of 2026-11-03 from 04:24 |
| `cargo test --workspace`, clippy with `-D warnings` | green | green |
