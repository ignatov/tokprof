# Proposal

## Why

`tokprof bench` gives wrong or meaningless output outside the default settings, and handles failures unlike the other reports:
- `--requests 5` prints a `10 requests` column before `5 requests`, with the same numbers in both; `--requests 10` repeats the column.
- `--requests 0` is accepted. The savings lines then read `$-1.83 of $0.00 (-182762620000%)`, and the SVG gets `NaN` coordinates.
- "/compact first … is cheaper than resuming from request k on" takes the first request where its mean cost dips below resuming. It does not check that it stays below, so "over all" can be negative right after the claim that it pays off.
- `--keep-warm 0.5m` is called `0h` in the report and the chart, and `90.5m` is called `90m`.
- With no logs, the bench prints `read  in 0.0s` and an empty report, and exits 0. Unreadable logs are skipped without the warning the other reports print.
- Small negative amounts read `$-1.83`, while amounts over $100 read `-$1,234`.

## What Changes

- `--requests` must be at least 1.
- The table shows the `10 requests` column only when N is over 10.
- /compact first pays off from request k only when its mean cost stays below resuming from k up to N. Otherwise the report says it does not pay off within N requests.
- A window that is not whole hours is named in minutes with up to one decimal: `90m`, `90.5m`, `0.5m`.
- No logs is an error, `no session logs found for <agents>`, and unreadable logs print warnings, as in the other reports. Every report prints at most 5 warnings, then how many more logs could not be read.
- Negative amounts put the sign before the dollar sign everywhere: `-$1.83`.
- The chart says `after 1 request`, not `after 1 requests`.

Out of scope: the replay model and its assumptions.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `return-bench`: the request limit, the table's columns, the /compact payoff, the window's name, and failures on missing or unreadable logs.
- `session-logs`: "Unreadable logs" covers `bench` and counts the warnings it does not print.

## How it is checked on the real logs

- `tokprof bench` with the defaults prints the same numbers as before; the payoff line stays `from request 8 on` for Claude Code.
- `tokprof bench --requests 5` has one column per request count; `--requests 0` fails; `--keep-warm 90.5m` names `90.5m`.
- `tokprof --session nosuchsession bench` fails with `no session logs found for claude, codex`.

## Impact

`src/bench.rs` (columns, payoff, window name, chart label), `src/main.rs` (the `--requests` range, the no-logs error and the warnings for bench), `src/report.rs` (the sign of negative amounts). README: the bench options.
