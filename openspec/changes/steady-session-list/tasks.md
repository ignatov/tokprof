# Tasks

## 1. Baseline

- [x] 1.1 Before any code changes, write a scratch pty check outside the repo, in Python with pyte. It drives `tokprof ui --agent claude` over a copy of 40 Claude Code sessions in a temp `CLAUDE_CONFIG_DIR`. For 60 s it appends one `cost-state` line a second to each of 5 idle sessions, and one timed `user` line a second to each of 2 others. It records the screen every 0.5 s and counts the rows that changed place on the screen. Run it against the installed `~/.local/bin/tokprof` and record the count here.
  - Script: `/tmp/tokprof-steady/check.py`; the binary before the change is kept as `/tmp/tokprof-steady/tokprof-before`.
  - Before: 395 row places changed in 60 s, in 54 of 120 snapshots. The 7 written sessions took turns in rows 1–7, and `r` did nothing.

## 2. Last activity

- [x] 2.1 Add a backward line reader to `tokprof-logio` for plain files: it reads 64 KB blocks from the end and hands over lines, last first, until told to stop. Verify with unit tests next to it: lines that cross a block boundary, a last line without a newline, an empty file, and a line longer than a block. `cargo test -p tokprof-logio` passes.
- [x] 2.2 Give the source interface a call that returns a session's last activity, where returning nothing means the last write. Session rows carry the earlier of that value and the last write. Make `testkit::conform` fail a source whose last activity is later than a fixture's last write. Verify with `cargo test -p tokprof-core` and the architecture test.
- [x] 2.3 In the Claude Code source, return the `timestamp` of the last line that has one, skipping lines without `"timestamp"` and lines that do not parse. Verify with unit tests next to it: an idle-resume tail of `cost-state`, `last-prompt` and `ai-title` lines gives the time of the timed line before it; a cut-off last line is skipped; a transcript with no timed line gives nothing. Name the field, the untimed records and Claude Code 2.1.284–2.1.316 in `sources/claude-code/README.md`. `cargo test -p tokprof-source-claude-code` passes.

## 3. Listing and picking by last activity

- [x] 3.1 Make `tokprof sessions` order rows by last activity and stop reading tails early, as design.md describes, and rename its first column to `active`. Verify with a unit test: on synthetic rows, the early stop gives the same rows as sorting every row, and an idle session written last ranks below a session with a newer timed record. `cargo test --workspace` passes.
- [x] 3.2 Make `tokprof`, `scan` and `handoff` without a target pick the main session with the latest last activity. The ambiguous-prefix error lists its matches by last activity, with that time. Verify with a unit test over a temp transcript directory: the log written last got only `cost-state` lines, and the pick is the other session.
- [x] 3.3 Update README.md so that `tokprof sessions` reads "newest first by last activity", and say in one sentence that records Claude Code writes without a time (such as `cost-state`) do not count as activity. Verify that `tokprof sessions --help` and the README agree.

## 4. Rows in place in the ui

- [x] 4.1 In `crates/tokprof/src/ui.rs`, stop re-sorting the list on background notes. Gone sessions are removed, new sessions are inserted where the current order puts them, and changed sessions keep their place. Costs no longer re-sort. The scroll offset moves with insertions and removals above the screen. Verify with unit tests next to it:
  - a change that keeps the last activity moves nothing;
  - a grown session keeps its row and its time updates;
  - a new session goes on top in time order, and the selection stays on its session;
  - with the list scrolled down, an insertion or removal above the screen leaves the rows on the screen as they were;
  - a rescan after dropped events keeps the listed rows in place.

  The existing test `the_list_follows_the_logs` expects "a session that grew moves up"; change it to the new behavior.
- [x] 4.2 Add the `r` key, the title marker `by <order>, r re-sorts` while sorting again would change the order, and `r re-sort` in the list footer. Verify with unit tests: a request in a session down the list marks the order out of date; `r` puts it on top and keeps the selection; the marker is gone after `r`; in next-request order the marker appears when a cache goes cold, with no new note. `tokprof ui --dump 160x45` shows the footer hint.
- [x] 4.3 Update the README ui section. A session that is being written keeps its row while its cost, time and X-ray update. New sessions join the list where the order puts them. Add `r` to the keys table. Verify that the README no longer says a session moves to the top.

## 5. Checks on the real logs

- [x] 5.1 Run `cargo test --workspace` and `openspec validate --all --strict`, then reinstall the author's binary with `cargo install --path crates/tokprof --root ~/.local`.
- [x] 5.2 Run `tokprof sessions --limit 30` and record the rows of a82b7617 and 8c9a96cf. Expect `2026-11-02 09:27` and `2026-10-27 20:29`, unless they have worked since. Compare the order with `/tmp/tokprof-scratch/activity.py` run at the same moment (recreate the script if it is gone), and record how many of the 30 rows match: expected 30.
  - a82b7617 reads `2026-11-02 09:27`, and 8c9a96cf reads `2026-10-27 20:29`. The binary from before the change showed 8c9a96cf at `2026-11-02 16:30`, because the IDE resumed it again at 16:30.
  - The comparison script is `/tmp/tokprof-scratch/order.py`. It covers both agents, and for Claude Code it takes the last line with a timestamp, as the spec says. 30 of 30 rows match it: id, time, agent and order.
- [x] 5.3 Run `tokprof` with no arguments and check that it X-rays the session with the newest timed record, as the script finds it. Record its wall time with `time`; expected under a second.
  - It X-rays cfa955ce, the session `order.py` puts first, active at 17:09.
  - Wall time is 0.37–0.41 s over 3 runs, against 0.37–0.39 s for the binary from before the change.
- [x] 5.4 Run the pty check from 1.1 with the new binary and record the count of rows moved on the screen, expected 0. Check that the title reads `by time, r re-sorts` after the first timed line, and that `r` puts the 2 working sessions in rows 1 and 2.
  - After the change, 0 row places changed in 60 s, in 0 of 120 snapshots; before, 395.
  - The title read `by time, r re-sorts`, and the 2 working sessions stayed in rows 16 and 26 with their times updated. `r` moved them to rows 1 and 2, and the title went back to `by time`.
  - The 5 idle sessions kept their rows and their times from 2026-11-01 17:08.
- [x] 5.5 Time `tokprof sessions --limit 100000 > /dev/null` before and after the change, using the installed binary from 1.1 or a build of the base commit, and record both. The expected increase is a few milliseconds, from reading 11.8 MB of tails.
  - Medians of 5 runs, two alternating rounds: 0.50 s and 0.51 s before, 0.52 s and 0.51 s after. That is within the run-to-run spread of 0.48–0.56 s.
  - `tokprof sessions` with the default 20 rows: 0.35 s before, 0.36 s after.
  - At 16:41, on a less busy machine, the binary from before took 0.40 s for the full list.

## Workflow follow-up

- Archive with `openspec archive steady-session-list -y` once the author accepts the result, then run `openspec validate --all --strict`.
