# Design

## Context

See proposal.md for the motivation. The reports take the list of logs of the selected agents, read each one in parallel and fold the requests the window keeps. The list is also what the line about the logs read counts and what the "no session logs found" error looks at.

Constraints:
- A record is written after what it records, so a log's last write is never earlier than its last request. The conformance checks of the log sources hold this for each source (the last activity of a log is not later than its last write).
- A log's creation time tells nothing: Codex rollouts compressed later (`.jsonl.zst`) are created after their first record.
- For an OpenCode session the last write is the latest time of its rows in the store, not the store file's time; a session whose rows are all before `--since` holds no later request.
- A file system that keeps no modification time gives 0: such a log is always read.

## Goals / Non-Goals

**Goals:**
- `summary`, `misses`, `whatif`, `tools` and `flame` with `--since` read only the logs last written on or after it.
- Byte-identical stdout for those reports.

**Non-Goals:**
- The other commands, `--until`, and the day store (see proposal.md, out of scope).
- Telling the user which logs were left unread.

## Decisions

- **The rule lives with the query**: "a log may hold a request of the window" is a question about a log's last write and `--since`, next to the query's other tests of a log. Alternative: a flag on the scan that skips logs inside it. Rejected: the scan would count as read what it did not read, and the line about the logs read would name files that were never opened.
- **The report path filters its list before the scan.** The list of all logs still decides the "no session logs found" error, so a window without logs stays an empty report and does not become an error. The filtered list is what is scanned and what the line counts. Alternative: prune inside the list the library hands to every report. Rejected: `pauses`, `bench` and `share` use the same list and need the logs before the window; their output changed in a trial.
- **Only the last write, compared with `--since`.** `--until` and the creation time are not used (see Context).

Effect on the parse time of the full corpus: none without `--since`. With it, the time falls with the share of logs written since: measured 27 s → 0.4–0.5 s for one day and 26 s → 0.9–1.4 s for seven days.

## Risks / Trade-offs

- [A log whose modification time was set back by a tool that restores files with older times than their content] → the log is left unread and its requests are missing from a windowed report. A run without `--since` still counts them. Accepted: agents append to their logs, and a copy that keeps times keeps the time of the last write.
- [A damaged old log is no longer reported in windowed runs] → stated in the spec; a run without `--since` reports it.
- [A source that reports a last write earlier than its last request] → the conformance checks fail for it.
