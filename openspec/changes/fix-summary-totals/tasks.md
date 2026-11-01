# Tasks

## 1. Avoidable total

- [ ] 1.1 Sum the totals line's avoidable premium over the misses that are not expected rebuilds, as the cause table does. Verify: a unit test with one idle miss and one compaction.

## 2. Reconciliation in a window

- [ ] 2.1 Record each reported session's first and last request time, all chains included.
- [ ] 2.2 With `--since` or `--until`, compare only sessions whose first and last requests fall inside the window, and add `inside the window` to the line. Leave the line out when none is left. Verify: unit tests with a session inside, one across the edge, and none inside.

## 3. Checks

- [ ] 3.1 Run `cargo test`, then `tokprof summary` and `tokprof --since 7d summary` on the real logs, and record both lines in this file.
