# Tasks

Every group ends with `cargo test --workspace` green, in a state that can be committed.

## 1. Before starting

- [x] 1.1 Record the baseline in a results table at the end of this file: the median of 3 runs of `tokprof summary` and the study's numbers from the proposal. Verify: the table has the numbers and the date.

## 2. Collector

- [x] 2.1 Feed the main session id of each log to a new pause collector before its analysis. Verify: a unit test that a subagent log's spend lands on its main session.
- [x] 2.2 Count typed messages by pause range, with the median context, joining same-name chains across their gap. Verify: unit tests for "A message seven minutes later", "A pause on a boundary", "The first message", "Median context" and a split conversation.
- [x] 2.3 Compute the share after each lifetime of the agent's APIs. Verify: unit tests for the "Claude Code" and "Codex" scenarios of "Share after each cache lifetime".
- [x] 2.4 Collect cold returns from `idle > TTL` misses of main chains, by pause range, with first-request cost, premium and median context; sort them by starter. Verify: unit tests for "A return after the night", "The total line", "Subagents", "No cold returns" and "The study's returns".
- [x] 2.5 Count the requests that follow each cold return, with the lifetime tracked as cache-misses defines it. Verify: unit tests for "The line" and "A compaction right after the return".
- [x] 2.6 Range sessions by their largest main prompt, with spend, share and cold returns. Verify: unit tests for "A session that filled its window" and "Shares"; merging two partial collectors gives the same rows as one.

## 3. Command

- [x] 3.1 Add `tokprof pauses` with the global filters, the read line, the sections in spec order, and the empty-case lines. Verify: a unit test of the rendered parts for one small session; `tokprof pauses --help` names what the report shows.
- [x] 3.2 Update README: the command, its parts, and how to read the lifetime line. Verify: the README example runs as written.

## 4. Check on the real logs

- [x] 4.1 Run `tokprof --agent claude --since 30d pauses`, with `--client sdk-ts` when `add-client-dimension` is in. Verify: the shares, returns, starters, follow line and session ranges match the proposal's numbers within a point or a few returns; record the output.
- [x] 4.2 Run `tokprof --agent codex --since 30d pauses`. Verify: it prints a 30-minute lifetime line and its cold returns match the `idle > TTL` row of `tokprof misses` for Codex main conversations; record it.
- [x] 4.3 Time `tokprof pauses` and `tokprof summary` 3 times each. Verify: the median of `pauses` is not above that of `summary`.
- [x] 4.4 Reinstall the binary with `cargo install --path crates/tokprof --root ~/.local`. Verify: `tokprof --since 1d pauses` runs.

## Results

All on 2026-11-03.

| check | study / baseline | after |
|---|---|---|
| `tokprof summary`, all logs, 3 runs | median 24.78 s (after add-client-dimension) | 24.40 s, 24.92 s, 24.85 s; median 24.85 s |
| 4.3 `tokprof pauses`, all logs, 3 runs | not above `summary` | 24.55 s, 24.46 s, 24.76 s; median 24.55 s |
| 2.1 session key | | the short id; subagent logs carry their main session's, so their spend lands there (test `sessions_by_largest_context`) |
| 4.1 typed messages, `--agent claude --since 30d --client sdk-ts` | 961 | 970 |
| 4.1 shares by pause, under 1m … over 1d | 22.0, 28.5, 15.7, 8.9, 7.4, 5.8, 3.3, 1.2, 5.7, 1.4 % | 22.2, 28.5, 15.9, 9.0, 7.3, 5.8, 3.2, 1.2, 5.7, 1.3 % |
| 4.1 after the 5-minute / 1-hour lifetime | 50% / 17% | 49% / 17% |
| 4.1 cold returns 1–2h, 2–4h, 4–8h, 8–16h, 16–48h, over 2d | 79, 34, 14, 48, 29, 2 | 79, 33, 14, 47, 29, 2 |
| 4.1 cold returns in all | 206: $777, $740 avoidable | 204: $775, $738 avoidable |
| 4.1 started by typed / background / tool / other | 166 / 12 / 12 / 16 | 165 / 12 / 12 / 15 |
| 4.1 follow: median, quarters, ended within 3 | 20; 7 and 70; 17 | 21; 7 and 73; 17 |
| 4.1 sessions `800k+` / `under 100k` | 22, 60.5% / 142, 3.1% | 22, 59.7% / 143, 3.1% |
| 4.2 Codex, `--since 30d` | 38 `idle > TTL` misses in main chains (study dump), $30.98 avoidable in `summary` | 30-minute lifetime line; 38 cold returns, $30.98 avoidable |
