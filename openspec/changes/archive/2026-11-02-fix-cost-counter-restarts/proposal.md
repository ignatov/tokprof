# Proposal

## Why

`tokprof summary` checks tokprof's cost against Claude Code's own counter, and it takes that counter from the last `cost-state` line of a session. The last line can cover only the latest part of the session. When Claude Code resumes a session in a new process, it restores the counter from the latest `cost-state` line, `startTime` included. A process that starts before the previous one has written its state on exit finds no line and counts from zero under a new `startTime`. Session 2cc7400f lost $24.06 this way on 2026-11-02. Its last line says $32.89, and tokprof prices the session at $57.49, so the check points at tokprof when the counter is what is wrong.

## What Changes

- Claude Code's own cost counter of a session becomes the sum over the counter's runs. The `cost-state` lines that share a `startTime` are one run, and each run counts with its highest `totalCostUSD`.
- A process that restored the counter keeps its run's `startTime`, so its spend is not counted twice.
- The `tokprof summary` line keeps its wording. Only the counter figure changes, and only for sessions with more than one run.

Out of scope:
- Spend the counter never wrote down: a process that was killed before its exit write, or one that is still running.
- Requests that Claude Code counts but does not log in the transcript. They keep the counter above tokprof in some sessions; ecad11b9 counts 419k uncached input tokens, $2.63, that its log does not have.
- The cost figures that other tools take from Claude Code's live counter.
- The 23% gap for 2026-10-25..31 that fix-summary-totals left open. None of those sessions has more than one run, so this change does not move it.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `session-logs`: "Agent-reported cost". The counter is the sum of the highest value of each run, not the last value.
- `cost-attribution`: "Reconciliation with the agent's own cost counter". It sums each session's counter as session-logs defines it, not each session's last value.

## How it is checked on the real logs

- `tokprof summary --agent claude` reads `Claude Code's own cost counter for 282 sessions: $10,409; tokprof for the same sessions: $10,548` today, so tokprof is 1.3% above. Expected after: $10,448 against $10,548, 1.0%, still within the 1.5% that tokprof holds against `cost-state`.
- `tokprof summary --session 2cc7400f`: the counter goes from $32.89 to $56.95, against tokprof's $57.49.
- The four other sessions with more than one run:
  - 7e640abc: $24.70 becomes $26.11, against tokprof's $24.83. A terminal process and an IDE process ran at once, and the last value left out the $1.41 the terminal counted. Unlogged requests keep the counter above tokprof.
  - ecad11b9: $0.00 becomes $13.38, against tokprof's $20.29. Its live process has spent $9.54 since 12:10 and has not written its counter yet.
  - bef96bff: $0.31 becomes $0.33, against $0.32.
  - a6228df8: stays $0.00, against $0.16.
- The other 277 sessions with a counter keep their figure.

## Impact

- The Claude Code log source: reading `cost-state` lines (`sources/claude-code/src/decoder.rs`) and its unit tests.
- `sources/claude-code/README.md`: `startTime` joins the fields it reads.
- README.md does not describe the counter, so it does not change.
