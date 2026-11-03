# Tasks

Every group ends with `cargo fmt --all`, `cargo test --workspace` and `cargo clippy --workspace --all-targets -- -D warnings` green. Names from the author's logs never go into tests or docs.

## 1. Before starting

- [x] 1.1 Keep the file as it is now: build the release binary of main and run `tokprof --since 30d share -o /tmp/spread-before-30d.json`. Record its size and time in the results table. Verify: the file exists.

## 2. What the collectors keep

- [x] 2.1 Count each session's typed messages in the pause collector, the first of a conversation too. Verify: the unit test "typed messages per session count the first one too".
- [x] 2.2 Keep in busy time when each request it counts in `retried` was sent. Verify: the unit test "a day with a retry wait".
- [x] 2.3 Count the failure windows the tool table opens, and note in the share the main session of the log where one opened. Verify: the unit test "a session counts once with a failure window", with a failure in a session's last response that opens none.
- [x] 2.4 Note the days of the interrupts and the sessions with a question of each main log inside the window. Verify: the unit tests "prevalence agrees with the other parts" and "a day of two agents counts once in the file".
- [x] 2.5 Build one rhythm summary per agent from the share's timelines, with the days of all of them together. Verify: the unit test "each agent's turns and answers add up to the file's".

## 3. The share file

- [x] 3.1 Write `distributions` per agent, with quantiles only from 5 values on. Verify: the unit test "quantiles only from five values", with 4, 5 and 6 values and dollar rounding.
- [x] 3.2 Write `prevalence` per agent and the file's day counts. Verify: the unit test "prevalence agrees with the other parts" asserts each count against the part it agrees with.
- [x] 3.3 Add the new keys to tokprof's own words, and let the leak check walk the file's `prevalence`. Verify: the unit test "the new parts have only own words", and the existing test that a file with and without the extra parts has the same words outside the own ones.
- [x] 3.4 Update the share table of README. Verify: `distributions` and `prevalence` have a row.

## 4. The pool and the report

- [x] 4.1 Add `How often it happens`. Verify: the unit tests "how often it happens in one file" and "in a pool", with a file without the part.
- [x] 4.2 Add `How the sessions spread`, with `--min-files` leaving out rows with quantiles from fewer files. Verify: the unit test "how the sessions spread in one file and in a pool".
- [x] 4.3 Put both sections after `Problems`, with their notes. Verify: the order test lists them, and files of format 1 or from before this change have neither.
- [x] 4.4 Update README's pool list and `docs/survey.md`. Verify: both sections are described there.

## 5. Check on the real logs

- [x] 5.1 Run `tokprof --since 30d share -o /tmp/spread-30d.json` and check the new numbers against the existing parts. Verify: the numbers below.
- [x] 5.2 Time `share` against the binary of 1.1 and compare the sizes and the words. Verify: no noticeable slowdown, a few KB more, and the only new words are the new keys.
- [x] 5.3 Pool the new file with `~/tokprof/share-2026-11-03-30d.json`. Verify: both new sections say `in 1 of 2 files`.
- [x] 5.4 Run `cargo fmt --all`, `cargo test --workspace`, clippy and `openspec validate --all --strict`. Verify: all pass.

## Results

Over the author's last 30 days, 2026-10-05 to 2026-11-04.

| check | result |
|---|---|
| 1.1 / 5.2 size | before 146,820 bytes, after 149,474 bytes (+2.7 KB) |
| 1.1 / 5.2 time | the machine ran at a load of 11–13, so wall time swung both ways: before 30.1, 29.7, 33.1, 30.5, 31.8, 27.6, 26.8, 54.1 s; after 37.3, 33.7, 37.4, 35.1, 29.1, 34.2, 31.3, 37.9 s. CPU time is the same: user 62.1 s before against 62.6 s after, and 63.9 s against 58.5 s. Writing the file after the scan takes about 0.1 s in both |
| 5.2 words | the only new keys or strings are the 21 field names of the new parts; none is gone |
| 5.1 sessions | Claude Code 328 main sessions, Codex 172: the `n` of `session_usd`, `typed_per_session` and `largest_context` |
| 5.1 cold returns | Claude Code 212 cold returns in `cold_returns_by_pause` and in `cold_return_usd.n`, in 78 sessions on 28 of 30 days; Codex 39, in 12 sessions on 14 of 24 days; the file: 28 of 30 days |
| 5.1 large and long | `sessions_over_400k` 70 and 0, equal to `400–800k` plus `800k+`; `sessions_over_1d` 30 and 4, equal to `1–3d` plus `over 3d` |
| 5.1 hand-offs | Claude Code: 40 sessions with a question of 82 questions; interrupts on 20 days of 42 interrupts; Codex 0 and 5 days of 5; the file 21 days |
| 5.1 failures and retries | 140 and 22 sessions with a failure window, of 956 and 457 failed calls; retry waits on 14 days of 106 retried requests |
| 5.1 rhythm | turns 1,376 + 149 = 1,525 and answers 1,079 + 91 = 1,170, the file's `rhythm` counts. A summary per agent with its own days gave one answer fewer than the file's; with the days of all agents they agree |
| 5.1 spread | Claude Code: a session costs a median $0.50 ($0.01–$15.28, p90 $66.97); the first request after a cold return $3.77 (p90 $6.72); 1 typed message (p75 4, p90 10); turns 3.3m (p90 22m); answers 2.9m (p90 1.0h); largest context 53.8k (p90 691k). Codex: $0.01 a session, $0.43 a cold return, 1 typed message. Half of the sessions are scripted one-prompt runs, which the rhythm leaves out but the session measures count |
| 5.3 pool | both sections print with `, in 1 of 2 files` in the title; the rows carry no repeated note |
