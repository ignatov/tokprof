# Design

## Context

The bench reads the main conversations of main session logs and finds the cold returns in them. It replays each return four ways and prints a table, the savings lines, notes and an optional SVG chart.

The heartbeat replay already sends a ping at 5/6 of the cache lifetime for up to `--keep-warm`. A return within the last ping's lifetime gets a warm first request. The handoff note and /compact first already run the followed requests on the base: the median prompt after a compaction.

No new log field is read. The change reuses the lifetimes, prices, base and shrink rule that the bench already has.

## Decisions

1. **The fifth way is the heartbeat schedule with its next ping turned into a compaction.**
   - Pings go out at k × 5/6 of the lifetime while that is within `--keep-warm`.
   - The first slot past the window holds a compaction instead, if the break lasts that long.
   - The compaction falls inside the last ping's lifetime, so it always reads a warm cache. Its cost is the context at the read price of its time plus the 10k-token summary at the output price. This is the handoff note's model, which the bench already uses for a summary written warm.
2. **A return before the compaction slot is always warm.** It comes within the last ping's lifetime. It is priced exactly as heartbeats price it: the first request is warmed, and the rest run as logged.
3. **A return after the compaction** pays the pings and the compaction up front. Its followed requests are shrunk to the base, as /compact first and the handoff note do. It therefore differs from the handoff note only in the pings and in when the summary is written.
4. **Breaks that were not cold returns.**
   - The scan also records every break of a main conversation that:
     - follows a prompt of at least `--min-context` tokens;
     - starts within the window;
     - lasts at least one ping interval;
     - is not a counted return.
   - The last request of a conversation opens such a break. Its end is not known until all logs are read: the scan keeps the latest request time it saw, and the replay ends open breaks there, or at `--until` when that is earlier.
   - The replay prices pings for these breaks, and for the fifth way compactions too. The totals go into the savings lines, separately from the returns' costs, so the per-return table keeps its meaning.
5. **The lifetime of a break** is the one the bench already assumes for returns: 1 hour or 5 minutes for Claude Code by the latest cache write, and 30 minutes for Codex.
6. **Chart.** The fifth series uses the next color of the chart palette and a dash-dot line. The legend, end labels, tooltips and screen-reader description list it like the other four.
7. **Reference files.** `docs/bench.txt` and `docs/bench.svg` are regenerated on the current logs. The reference-run numbers in the return-bench spec are updated to match in the same edit, as earlier regenerations did.

## Risks / Trade-offs

- **The compaction model.**
  - A real compaction at 2 h 30 min would also cost the user's attention when they come back to a summary. The bench prices only tokens.
  - The quality of work after an early compaction is not measured. The fifth way is a price comparison, as the handoff note is.
- **Open breaks at the end of the logs** depend on when the bench runs. A session that is still active has a short open break, so it adds few pings.

## Parse time

- The scan adds a constant amount of work per request pair. It keeps one record per long break of a big conversation: a few thousand in the author's logs.
- Pricing them is negligible next to the replay of the returns.
- The tasks check that the bench takes at most 10% longer.
