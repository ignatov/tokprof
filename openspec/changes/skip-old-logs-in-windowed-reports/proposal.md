# Proposal

## Why

`tokprof summary --since 1d` takes as long as `tokprof summary` over all history: the reports read every log and only then leave out the requests before the window. On the author's logs (about 9 GB) both take about 27 s. A log last written before `--since` cannot hold a request inside the window, so reading it is wasted.

## What Changes

- `summary`, `misses`, `whatif`, `tools` and `flame` no longer read a log last written before `--since`. Their numbers do not change: such a log holds no request they count.
- The line about the logs read counts only the logs that were read, so with `--since` it names fewer files and a smaller size than before.
- A log last written before `--since` that could not be read gets no `warning:` line, since it is not opened. **BREAKING** for anyone who relied on that warning to find damaged old logs; a run without `--since` still prints it.
- When every log was last written before `--since`, the report is empty and the line reads `read  in 0.0s`, with exit status 0, as it is today for a window that holds no request.

Checked on the author's logs: stdout of the five reports is byte-identical with and without the change, over closed windows (`--since 2026-11-05 --until 2026-11-10`, `--since 2026-11-09 --until 2026-11-11`) and over the frozen copy of the logs with six windows each. Measured for `summary`: `--since 1d` 0.4–0.5 s instead of 27 s, `--since 7d` 0.9–1.4 s instead of 26 s.

Out of scope:
- `pauses`, `bench`, `share`, `day` and the session browser: they look at logs before the window (the starts of conversations, the nights of the rhythm), and their output changed when the same rule was tried on them.
- `--until`, and a log's creation time: a log written anew later, as one compressed afterwards, is created after its first record, so neither tells what the log holds.
- `calendar`, `month` and `summary --compare`, which read the day store.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `session-logs`: the time window (which logs the reports read), the line about the logs read, unreadable logs, and no logs found.

## Impact

- The report path of the command line and the query's rule for a log's last write.
- `README.md`: the paragraph on the filters says that a report with `--since` reads only the logs written since.
- No change to the logs' formats, to prices or to any number a report prints.
