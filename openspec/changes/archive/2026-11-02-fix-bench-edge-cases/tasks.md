# Tasks

## 1. Options and failures

- [x] 1.1 Reject `--requests 0` at parse time. Verify: running it fails with a usage error.
- [x] 1.2 Fail with `no session logs found for <agents>` when the bench finds no log, and print the first warnings for unreadable logs as the other reports do. After 5 warnings, every report prints `warning: <n> more logs could not be read`. Verify: `tokprof --session nosuchsession bench`, and a unit test of the warning lines.

## 2. Report

- [x] 2.1 Show the `10 requests` column only when N is over 10, and never the same count twice. Verify: a unit test of the table with N = 1, 5, 10 and 60.
- [x] 2.2 /compact pays off from the first request k where its mean cost stays below resuming through N. Verify: a unit test with curves that cross and cross back.
- [x] 2.3 Name the window in whole hours, else in minutes with up to one decimal. Verify: a unit test for 6h, 90m, 90.5m and 0.5m.
- [x] 2.4 Put the sign before `$` for every negative amount, and write `after 1 request` in the chart. Verify: unit tests.

## 3. Checks

- [x] 3.1 Run `cargo test`, then the checks in the proposal on the real logs, and record the default run's numbers in this file. Do not overwrite `docs/bench.*` unless the numbers changed.

## Results on the real logs (2026-11-01)

- `tokprof bench` with the defaults prints exactly what the snapshot commit prints (`diff` is empty). /compact first still pays off from request 8 for Claude Code and from request 4 for Codex, so `docs/bench.*` stay as they are.
- `--requests 5`: columns `1st request`, `5 requests`, `all returns`.
- `--requests 0`: `error: invalid value '0' for '--requests <REQUESTS>': 0 is not in 1..18446744073709551615`.
- `--keep-warm 90.5m`: `Heartbeats for up to 90.5m would have kept 71 of 210 returns warm, for $21.36 of pings, and saved $219 of $1,739 (13%).`
- `tokprof --session nosuchsession bench`: `Error: no session logs found for claude, codex`.
