# Design

## Context

See proposal.md, Why. The state of the code today:

- A session's time is its main log's modification time, taken from file metadata at discovery. The session list, the ui and the pick of the latest session all sort by it, so any write moves a session.
- A session row reads only the first lines of its log, for the project and the id.
- The ui rebuilds and re-sorts its list on every change it is told about, and also in cost and next-request order whenever costs arrive. The cursor stays on its session, but the rows around it move.
- `ui.rs` and `xray.rs` are being edited by other sessions in the same checkout, so edits stay small and targeted.

## Goals / Non-Goals

**Goals:**
- Get a session's last activity cheaply enough for `tokprof` alone to stay under a second, and for the ui to have it before the first frame.
- Make every row move in the ui come from a key the user pressed, with one exception: sessions that join or leave the list.

**Non-Goals:**
- Skipping the re-costing and re-X-ray of a session that only got records without a time. A title change (`ai-title`) arrives that way too.
- A last activity for Codex beyond the last write.

## Decisions

### Last activity comes from the end of the main log, read when the row is built

The Claude Code source reads its transcript backward from the end until it reaches a line with a top-level `timestamp` that parses. Lines without `"timestamp"` are skipped unparsed. Everything else in the core treats the value as opaque: the source interface gains one call that returns a session's last activity or nothing, and nothing means the last write. Session rows take the earlier of that value and the last write. They are built when the list is made and when the watcher reports a change, so the ui has the value before its first frame and for every row a change brings.

Fields and versions:
- Timed: `user`, `assistant`, `system`, `attachment`, `api-request`, `api-request-shape`, `queue-operation`, `file-history-delta` and `pr-link` lines.
- Untimed: `cost-state`, `last-prompt`, `ai-title`, `custom-title`, `mode`, `permission-mode`, `tools-used`, `atis-latch`, `file-history-snapshot`, `api-request-blob`, `relocated`, `worktree-state` and `agent-name` lines.
- Both groups were counted in the 88 main logs written in the last 7 days. The logs on this machine come from Claude Code 2.1.284–2.1.316.
- 9,425 of the 9,644 `cost-state` lines written over a minute after a session's last activity are in sessions last run with 2.1.310 or 2.1.312.

Codex returns nothing, so its last write counts. In the 42 `.jsonl` and 40 `.jsonl.zst` rollouts checked, the last write equals the last line's `timestamp`. Those rollouts come from Codex 0.147–0.166. Compressed rollouts cannot be read backward anyway.

Alternatives considered:
- **The time from the costing pass.** The full parse knows the last request, but it arrives seconds later. The time cells would then change, and the rows move, exactly while results load. `tokprof sessions` without `--cost` does not parse at all.
- **Watching what the watcher sees appended.** That works only while the ui runs, and leaves `tokprof sessions` and `tokprof` wrong.
- **The last request's time.** That would leave out typed prompts, tool results and compactions. A session waiting on a long tool call would look idle.

### The neutral backward reader lives in tokprof-logio

It reads a plain file in 64 KB blocks from the end and hands lines to a callback, last line first, until the callback says stop. A cut-off last line is handed over like any other, and the source skips it because it does not parse. `.zst` files are not supported, and a source must not ask for them. This keeps log formats out of the framework, as `crates/tokprof/tests/architecture.rs` requires.

### Picking and listing stop early, using last activity ≤ last write

The logs stay sorted by last write. `tokprof`, `scan` and `handoff` without a target walk them in that order, build the row of each, and stop as soon as the next log's last write is older than the best last activity found so far. `tokprof sessions --limit N` does the same with the N-th best. In the usual case this reads one tail. In the 16:01 burst it reads the 20 touched sessions, about 2.5 MB. With `--project` or `--session`, which need every row's head anyway, all rows are built and then sorted.

### The ui keeps its order and only edits it

The list order is the one shown. A background change edits it in three ways only:
- a session that is gone is removed, and the rest keep their order;
- a new session is inserted before the first row that the current order puts after it;
- a changed session keeps its place.

Keys that sort (`s`, `r`) and filter changes rebuild the order as today. When an insertion or a removal happens above the first row on the screen, the list's scroll offset moves by one with it, so the rows on the screen stay. The selection always follows its session.

The out-of-date marker is checked each time the list is drawn. It tests every row against the next one in the current order, since a list is sorted exactly when each pair of neighbors is. The next-request order changes with the clock when a cache goes cold, so a check only when notes arrive would miss that. One pass over 765 rows every 100 ms frame costs well under a millisecond.

Alternatives considered:
- **Fix only the last activity, and keep re-sorting on real work.** On the last week's logs, the first row would still change 3.3 times per minute of activity.
- **Re-sort only the rows off the screen.** The rows that belong on the screen get parked at its edges, so the screen is as stale as with a frozen list, and harder to explain.
- **Re-sort once when costing ends.** That is still a jump, at a moment the user did not choose.
- **A freeze key, as in htop.** The default would still jump.

## Effect on parse time

Reports (`summary`, `misses`, `whatif`, `flame`, `bench`) read no tails, so the full-corpus parse is unchanged. `tokprof ui` and `tokprof sessions` read the tail of every listed Claude Code log back to its last timed record. Over the 287 logs that is 11.8 MB: 8 KB at the median, 78 KB at p90, 283 KB at p99 and 397 KB at most. That is a few milliseconds warm, next to the ui's background costing of about 50 s of CPU. `tokprof` alone usually reads one tail.

## Risks / Trade-offs

- [A Claude Code release writes a timed record without work, for example a `system` line on resume] → such a session moves as it does today. The source README names the timed types seen, and a fixture with an idle-resume tail keeps the current behavior tested.
- [A timed record's `timestamp` is the start of the event it describes] → the time can be early by that event's length. That matters little for a list shown to the minute.
- [Untimed tails grow without bound in sessions that the IDE resumes often] → 8c9a96cf holds 539 untimed lines, 320 KB, after 5.8 idle days. Reading stops at the first timed record, and a block read costs the same as now.
- [A frozen order hides a session that just came alive further down] → its time cell updates in place, and the title says `r re-sorts`.
- [Equal last activity, for example two sessions written in the same millisecond] → ties keep the current order, as the sort does today.

## Migration Plan

None. Rolling back means reverting the change. No file or setting holds the old behavior.
