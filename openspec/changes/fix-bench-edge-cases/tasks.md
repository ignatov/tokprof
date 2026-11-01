# Tasks

## 1. Options and failures

- [ ] 1.1 Reject `--requests 0` at parse time. Verify: running it fails with a usage error.
- [ ] 1.2 Fail with `no session logs found for <agents>` when the bench finds no log, and print the first warnings for unreadable logs as the other reports do. After 5 warnings, every report prints `warning: <n> more logs could not be read`. Verify: `tokprof --session nosuchsession bench`, and a unit test of the warning lines.

## 2. Report

- [ ] 2.1 Show the `10 requests` column only when N is over 10, and never the same count twice. Verify: a unit test of the table with N = 1, 5, 10 and 60.
- [ ] 2.2 /compact pays off from the first request k where its mean cost stays below resuming through N. Verify: a unit test with curves that cross and cross back.
- [ ] 2.3 Name the window in whole hours, else in minutes with up to one decimal. Verify: a unit test for 6h, 90m, 90.5m and 0.5m.
- [ ] 2.4 Put the sign before `$` for every negative amount, and write `after 1 request` in the chart. Verify: unit tests.

## 3. Checks

- [ ] 3.1 Run `cargo test`, then the checks in the proposal on the real logs, and record the default run's numbers in this file. Do not overwrite `docs/bench.*` unless the numbers changed.
