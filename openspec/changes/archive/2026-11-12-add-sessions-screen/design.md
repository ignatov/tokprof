# Design

## Context

See proposal.md for the why. Today the window (`crates/tokprof-desk`) draws one browser (`tokprof-browse`). The browser holds the list of every session, its filter and its sort (time, cost, next, waiting). It costs each session in the background with a "glance": one parse of the main log and its subagents' logs, about 14 s wall for 1,039 sessions on the author's logs. It also computes the day of the lanes on a worker, about 0.4 s. The day reads the main logs around the day, runs the rhythm and the cold-return count over them, and prices the requests. Calendar and Reports' comparisons read the day store (`days-2.json`). Refreshing the store takes 0.3–0.5 s when it is up to date with a few logs grown, and about 45 s when the store must be filled.

## Goals / Non-Goals

**Goals:**
- Every number on the Sessions screen and on the day summary comes from data the window already computes, or from the same parse, with no new pass over the logs.
- The numbers equal the CLI's: `tokprof sessions --cost`, `tokprof scan`, `tokprof pauses`, `tokprof day` and `tokprof calendar`.
- Day and Sessions share one selection, one filter and one browser, so that a session picked in one is the session shown in the other.

**Non-Goals:**
- No change to `tokprof ui`'s screen, keys or filter; no change to the day store's format.
- No layout B. Only the border's place is saved, in `prefs.json`.

## Decisions

### Where each number comes from

| Number | Source | New? |
|---|---|---|
| Sessions: last activity, agent, project, title, cost, Next | the browser's list and glance, as today | no |
| Sessions: requests, started, avoidable, cold returns, models | the glance, from the same parse: its totals already hold requests, components with their avoidable part, models and misses | new fields of the glance |
| Duration | started to last activity | new, computed |
| Running now | the glance, from the main log: the latest turn has no end and no open question, the latest request's time | new field; re-evaluated by the clock as the warm countdown is |
| Project rows, footer | sums over the browser's filtered list: the sessions' whole-life costs | new, computed in the view |
| Day headers' spend | the day store, the Spend card's query for each date shown, kept to the main session ids of the filtered set (the store names each entry's session, main session, project and agent) | new query over the store |
| Spend card, its hours, vs the day before, top sessions' day spend | the day store's minutes (spend per minute per log, with its main session id), inside the day's bounds that the day computation found | new query over the store |
| Rhythm cards | the day's rhythm lines, as `tokprof day` prints them | moved from text lines to cards |
| Cold returns of the day | the day computation's cold returns, which already carry time, pause, cost and premium, plus the session | the session is new on each return |
| Waiting for you now | the glance's `waiting since`, as the waiting order uses it | no |

Alternatives considered:
- *Per-session numbers from the day store.* The store also holds requests, avoidable dollars and cold-return times per log. But the store must be filled first, about 45 s with no store, and its costs would be a second source beside the glance's. The glance parses each session anyway. Adding the counts there costs no read and keeps one source per row.
- *Day spend from the day computation.* It reads only main logs, so subagents' spend would be missing, and it does not reach back to yesterday at 14:20. The store has both, with the calendar's rule, so the Spend card equals the calendar cell.

### Cold returns per session

A cold return is a main-conversation request whose miss has the cause `idle > TTL` (pause-report). The glance already gets every miss event of the session's chains through its sink. It counts those on the main chains with that cause, so the count equals `tokprof pauses --session <id>`. No typed-message analysis is needed for the count.

### Sort orders and filters in the browser

The browser's sort becomes a column plus a direction. `s` keeps cycling time, cost and next (and waiting) for `tokprof ui`, which shows only those. Unknown values go last in both directions. Ties go by last activity, newest first. Rows in place and `r` work unchanged, because they work on the order, not on its key. The quick filters are a set of predicates that the window gives the browser. The browser applies them with its text filter when it lists rows, so the selection, the rows in place and the counts behave as they do for the text filter today. The text filter takes a "match titles" flag that only the window sets. `tokprof ui` keeps agent, project and id.

### One selection, possibly no selection

Today the list always has a selected row. Day now starts with none. The window keeps "no session selected" as its own state over the browser. The browser still has a current row, used for the X-ray worker and for Sessions, and the window tells the X-ray worker not to compute while Day shows no session. A session with no lane on its day (a program ran it) is still selectable. The lanes then highlight nothing, and the header says so.

### Day layout

The lanes, their header, the legend and the pointer line form the top block. Its height is Room for the lanes, or the dragged border. The details pane takes the rest. The inspector is the existing one, drawn at full width. Its Requests table keeps its "760 pt or taller" rule, so on a 900-pt window it scrolls with its cards, unless the user drags the border up or opens the inspector full size with ⏎. The border is a drag region of 6 pt over the line between the blocks, using Rune's drag events as the Requests chart does. Its place is saved as `lanes_share` in `prefs.json` when a drag ends, as the other settings are written when a choice changes.

### esc no longer quits

esc now steps back at every level: full size, the selection, the Sessions inspector, the filter. If it also quit with nothing left to undo, a double esc from a selected lane would close the window. Quitting stays on `q`, ⌘Q and ⌘W.

### Sessions inspector

The inspector is the existing Summary drawn in a 360-pt column. The track takes that width, about 340 pt. The ways, the spend and the tips stack. No other tab is drawn there: Requests and Flame need the width that Day gives.

### Frames without a window

The new scenes are `sessions`, `sessions-projects` and `sessions-cost`. The default frame and `--tab` change: the default frame shows the day summary, and `--tab` selects the newest session. The golden-style checks compare `--tree` JSON in the desk's tests on synthetic sessions, as the existing frame tests do. On the real logs they are checked by eye and against the CLI's numbers.

## Log fields and agent versions

No new log field. The new columns use what the X-ray and the rhythm already read: request usage and model, the miss rule's fields (cache-misses), the end of turn and the question records (work-rhythm, Actions and hand-offs) and the titles. Each field is used in the agent versions that `sources/*/README.md` name for it.

## Parse time

There is no new read of any log. The glance adds a few counters to a parse it already makes, under 1% of its 48 s CPU on the author's logs. The store refresh while Day shows is the incremental one Calendar does: 0.3–0.5 s when the store is current, and at most once per 20 s and 4 times its own length while logs change. The full corpus parse of the CLI does not change.

## Risks / Trade-offs

- [A day header gives the day's spend, while its rows give whole-life costs, so they do not add up] → The header reads `$… spent`, a scenario says that a header is not the sum of its rows, and the footer totals the rows.
- [Folded projects hide sessions at first] → The footer still totals them, and a click, ⏎ or → unfolds.
- [The store is filled on first use, about 45 s] → The Spend card and the top sessions show progress, and the lanes and the rhythm cards show at once.
- [Losing esc-to-quit annoys muscle memory] → `q` still quits, and the key help says so.
- [A narrow window hides columns] → `Model`, `Started` and `Requests` go in that order, and the inspector closes with esc to give the table the width.

## Migration Plan

Ships in one release of the desk. `prefs.json` gains `lanes_share`; an older build ignores it. The day store keeps its format. Rollback is the previous app.

## Open Questions

See proposal.md, Open questions and Decided. None of them changes the data sources or the task order.
