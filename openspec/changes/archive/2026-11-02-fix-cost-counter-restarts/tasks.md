# Tasks

## 1. Counter runs

- [x] 1.1 In the Claude Code log source, keep the highest `totalCostUSD` of each `startTime`, with lines that have none sharing one run, and report the sum over the runs as the session's reported cost. Verify: unit tests next to the decoder for several lines of one run ($10.00, $30.00, $28.00 give $30.00), a run that started from zero ($24.06, then $2.66 up to $32.89, give $56.95), interleaved runs ($1.34, $1.41, $24.70 give $26.11) and a transcript with no `cost-state` line (no reported cost).
- [x] 1.2 In `sources/claude-code/README.md`, add `startTime` to the `cost-state` fields, with the versions it was seen in (2.1.284–2.1.312) and the resume behavior the rule relies on. Verify: the README names the field, the versions and the behavior.

## 2. Checks on the real logs

- [x] 2.1 Run `cargo test --workspace` and `openspec validate --all --strict`. Verify: both pass.
- [x] 2.2 Reinstall the binary with `cargo install --path crates/tokprof --root ~/.local`. Then run `tokprof summary --agent claude`, and `tokprof summary --session <id>` for 2cc7400f, 7e640abc, ecad11b9, bef96bff and a6228df8, and record the lines in this file. Verify: 282 sessions read about $10,448 against $10,548, and 2cc7400f reads $56.95 against $57.49.
- [x] 2.3 Time `tokprof summary --agent claude` before and after the change, and record both times in this file. Verify: both are about 7 s.

## Results on the real logs (2026-11-02)

| Command | Before (last value) | After (sum of runs) |
|---|---|---|
| `summary --agent claude` | 282 sessions: $10,409 vs $10,548 (1.3%) | 284 sessions: $10,485 vs $10,580 (0.9%) |
| `summary --session 2cc7400f` | $32.89 vs $57.49 | $58.13 vs $57.49 (its live process has since written $34.07 on the restored run) |
| `summary --session 7e640abc` | $24.70 vs $24.83 | $26.11 vs $24.83 |
| `summary --session ecad11b9` | $0.00 vs $20.29 | $32.62 vs $33.99 (the session went on since) |
| `summary --session bef96bff` | $0.31 vs $0.32 | $0.33 vs $0.32 |
| `summary --session a6228df8` | $0.00 vs $0.16 | $0.00 vs $0.16 |

`tokprof summary --agent claude` took 6.8 s before and 7.0 s after.

## Workflow follow-up

- Archive with `openspec archive fix-cost-counter-restarts -y` once the author approves, then run `openspec validate --all --strict`.
- Commit only when the author asks.
