# Tasks

## 1. Avoidable total

- [x] 1.1 Sum the totals line's avoidable premium over the misses that are not expected rebuilds, as the cause table does. Verify: a unit test with one idle miss and one compaction.

## 2. Reconciliation in a window

- [x] 2.1 Record each reported session's first and last request time, all chains included.
- [x] 2.2 With `--since` or `--until`, compare only sessions whose first and last requests fall inside the window, and add `inside the window` to the line. Leave the line out when none is left. Verify: unit tests with a session inside, one across the edge, and none inside.

## 3. Checks

- [x] 3.1 Run `cargo test`, then `tokprof summary` and `tokprof --since 7d summary` on the real logs, and record both lines in this file.

## Results on the real logs (2026-11-01)

| Command | Snapshot commit | This change |
|---|---|---|
| `summary`, totals | avoidable $2,606 (12.5%) | $2,500 (11.9%), the cause table's total |
| `summary`, counter | 278 sessions: $10,327 vs $10,441 | unchanged |
| `--since 7d summary`, totals | avoidable $533 (21.2%) | $491 (19.5%) |
| `--since 7d summary`, counter | 278 sessions: $10,327 vs $2,476 | 67 sessions inside the window: $1,524 vs $1,800 |

The windowed line now compares like with like, but it shows a gap that the whole history hides. By window:
- 2026-11-01: 35 sessions, $292 vs $301 (3%).
- 2026-10-02 to 2026-10-30: 240 sessions, $8,271 vs $8,369 (1.2%).
- 2026-10-25 to 2026-10-31: 31 sessions, $1,191 vs $1,460 (23%).

Why those 31 sessions differ is open. Possibly the counter restarts when a session is resumed, which would make Claude Code's figure the lower one. That is a question for cost-attribution, not for this change.
