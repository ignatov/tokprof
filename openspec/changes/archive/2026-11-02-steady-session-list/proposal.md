# Proposal

## Why

In `tokprof ui` the session list jumps: rows move under the cursor while costs fill in. On 2026-11-02 at 16:01 the selected session drifted from row 1 to row 10 in 8 seconds, with no key pressed. Two things move the rows.

- **Writes that are not work.** Claude Code appends records without a time, such as `cost-state`, `last-prompt` and `ai-title`, when the IDE resumes a session that then does nothing. At 16:00–16:02, 20 sessions that had been idle for 2.3 hours to 5.8 days got such records. Each record moved its session to the top of the time order, about one move a second. This only happens while something resumes sessions, so the jumping does not always reproduce. The times shown were wrong too: a82b7617 read `16:01`, but its last record was written at 09:27. Over the last 30 days, 156 of 287 Claude Code main logs were last written more than a minute after their last record with a time, 23.7 hours later at the median. `tokprof sessions` dates these sessions wrong. `tokprof` alone can X-ray one of them instead of the session that worked last.
- **Real work.** The list re-sorts on every change. In the last 7 days the first row changed 3.3 times per minute of activity even when only records with a time are counted, because two or more sessions worked in the same minute in 36% of those minutes. In cost order, every batch of costs that arrives re-sorts the list again.

## What Changes

- A session's time is its **last activity**: the time of the last record in its main log that carries one. Records without a time do not move it. `tokprof sessions` orders by it and shows it in a column renamed from `modified` to `active`. `tokprof ui` orders and dates its rows by it. `tokprof`, `scan` and `handoff` without a target pick the session that was active last. Codex rollouts keep their last write, which equals their last record's time.
- **`tokprof ui` rows stay in place.** The list takes its order when the ui starts, when `s` or the new key `r` is pressed, and when the filter changes. Costs, X-rays and log changes that arrive later update cells, never places:
  - a session that grows keeps its row, and its time and cost update;
  - a cost that arrives in cost order fills its cell where the row is;
  - a new session is put where the current order puts it, at the top in time order;
  - a session that leaves takes its row with it.
- Rows on the screen keep their places, except that a new session put among them pushes the rows below it down by one, and a session that leaves pulls them up by one. A new or departed session above the screen moves nothing on it.
- `r` re-sorts the list in its current order and keeps the selected session selected. While the order is out of date, the list title reads `by time, r re-sorts` (likewise for cost and next request).

Out of scope:
- `--since` keeps selecting the sessions to pick or list by their main log's last write, as "Commands the filters apply to" says.
- A session that gets only records without a time is still costed and X-rayed again, with `· updating…` in the X-ray title meanwhile.
- Why Claude Code writes `cost-state` when the IDE resumes a session, and what the counter adds up.
- Subagent logs still neither date nor move their main session.
- The X-ray pane, which already keeps the old X-ray until the new one is ready.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `session-logs`: a new requirement "Last activity" defines a session's time. "One-session target" picks, and lists ambiguous matches, by last activity instead of the last write.
- `session-xray`: "Latest session by default" X-rays the main session that was active last.
- `session-browser`: "Session list", "Browser layout", "Sort" and "Live updates" order and date sessions by last activity, and stop moving rows on their own. A new requirement "Rows in place" covers the `r` key and the out-of-date title.

## How it is checked on the real logs

- `tokprof sessions` lists a82b7617 at `2026-11-02 09:27`, not `16:01`, and 8c9a96cf at `2026-10-27 20:29`, not `2026-11-02 16:01`. Its top 30 rows match the order of the last records with a time, which today puts only 5 of the 30 rows in the same place as the order by last write.
- `tokprof` with no arguments X-rays the session whose last record with a time is the newest.
- `tokprof ui` runs in a pty over a copy of the logs while a script appends `cost-state` lines to 5 idle sessions and timed records to 2 others, once a second for a minute. Before the change, every one of those writes moves a row on the screen. After it, none move, the title reads `by time, r re-sorts` after the first timed record, and `r` puts the 2 working sessions on top.
- `tokprof sessions` and the ui start no slower: listing reads 11.8 MB of log tails over the 287 Claude Code logs.

## Impact

- `crates/tokprof-logio`: reading a log's lines from its end.
- `crates/tokprof-core`: `LogSource` gets a way to tell a session's last activity, with the last write as the default. Session rows, the latest-session pick, the session list and the ambiguous-prefix error use it.
- `sources/claude-code`: the last record with a top-level `timestamp`. Its README names the field.
- `crates/tokprof/src/ui.rs`: the list keeps its order, gets the `r` key, the out-of-date title and a footer hint. `render_sessions.rs` gets the `active` column.
- README.md: the `tokprof sessions` column and the ui keys.
