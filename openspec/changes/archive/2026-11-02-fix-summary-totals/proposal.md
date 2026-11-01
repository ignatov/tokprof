# Proposal

## Why

Two numbers in `tokprof summary` mislead:
- **The reconciliation line compares different spans.** With `--since` or `--until`, Claude Code's own counter still covers each session's whole life, and sessions with no request in the window still count. With `--since 7d` it read 275 sessions, $10,241 by Claude Code against $2,581 by tokprof, which looks like a 4× error in tokprof. Without a filter the same logs give $10,241 against $10,353. The line exists to check tokprof's cost, and with a window it checks nothing.
- **The avoidable total includes expected rebuilds.** `avoidable on cache misses` counts the premium of compactions and cleared tool results ($548). The cause table right below it and the X-ray leave those out ($506), so the two numbers on one screen disagree.

## What Changes

- **Reconciliation.** With a time filter, only sessions whose every request falls inside the window are compared, and the line says so: `Claude Code's own cost counter for <n> sessions inside the window: …`. The line is left out when no such session has a counter. Without a filter nothing changes.
- **Avoidable total.** The totals line sums the premium of the misses the cause table lists, so it matches the table and the X-ray. `tokprof misses --expected` still shows expected rebuilds with their premium.

Out of scope:
- Taking the counter's value at the window's edges. `cost-state` lines carry no timestamp, only a start time and a running duration, and it is not known whether the counter restarts when a session is resumed.
- The `avoidable` measure of `tokprof flame`, which keeps the premium of every rebuild.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `cost-attribution`: "Summary totals line" (avoidable total without expected rebuilds) and "Reconciliation with the agent's own cost counter" (only sessions inside the window).

## How it is checked on the real logs

- `tokprof summary` without a filter: the reconciliation line is unchanged, about $10,241 against $10,353 for about 275 sessions. The avoidable total equals the sum of the cause table.
- `tokprof --since 7d summary`: the line compares only sessions inside the window, and the two figures are within the 1.5% that tokprof holds against `cost-state` over all history.

## Impact

`src/report.rs` (summary totals, reconciliation, the session record that summary keeps) and `src/main.rs` (passing the window to the report). README: the `summary` line under "Reports over all sessions" if it mentions the reconciliation.
