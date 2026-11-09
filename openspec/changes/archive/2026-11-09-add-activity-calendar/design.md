# Design

## Context

Every report today parses the logs it needs, in parallel, each log on its own into a session (`scan` over `catalog.parse`), and folds it into sinks with `analysis::analyze`. A log is analyzed alone: a subagent's log is its own session for the summary, the time window only decides which requests reach the sinks, and the walker's state does not depend on it. So the numbers of one log, kept by time, can be summed later over any window and give what the summary gives for that window. This is what makes a store per log possible.

What one log gives, from code that exists:
- Per request: `RequestRecord` with the time, the usage and the price; its cost is `price.cost(usage)`, which is what the summary's `Total priced` sums (the Reports' day chart relies on the same identity). Right after it, `tool_share` events of the same request, whose `vcs` says what a worked shell call did to version control (the share's outcome rule), and the `MissEvent` with `premium_usd` and the cause (`cause.expected()` is left out of avoidable, as in `Collector::avoidable_usd`).
- Per main log: `Timeline::of(session, temp)` and `Lane::of`, with the user's actions (`Lane::actions`), the work intervals (`Lane::work`) and `left_out`.

The day of work-rhythm is found from all the user's actions with `Days::of`; the sittings of a day from the actions inside its bounds (`Summary::of`). Both need the actions' exact times, not counts per hour.

Log fields: none new. The store keeps what the summary, the share's outcomes and the rhythm read, from the agent versions their sources name (Claude Code up to 2.1.316; Codex as its source README lists).

## Goals / Non-Goals

**Goals:**
- A second `tokprof calendar` on unchanged logs in about a second; a first one as long as one `tokprof summary` over all logs.
- The calendar's spend over a window equals the summary's over it; a cell's messages and keyboard time equal `tokprof day`'s.
- History past the agents' log cleanup.
- No change to any other command and to the parse.

**Non-Goals:**
- A faster parse; an incremental parse of a growing log (a grown log is read whole again).
- Keeping the client of each request (no `--client`).

## Decisions

### Minutes inside hours, and exact action times

PM decision 1 asks for per-hour aggregates by UTC hour. The calendar's days start in the middle of the night, at any minute (04:55 on the author's 2026-11-05), so an hour that holds a day's start must be split. The store keeps each hour with the minute of each amount inside it: rows of (minute, spend, avoidable, requests, commits, pushes, pull requests) for the minutes that had any, grouped by hour. An hour's total is the sum of its rows. Agent work is a 60-bit set of minutes per hour, as asked. The user's actions are kept as their exact times in ms with a message flag, not as a bitmap of minutes, because the rhythm's days and sittings are computed from exact times: a bitmap would move sittings by up to a minute and break the 15-minute gap at its edge, so the keyboard time could not equal `tokprof day`'s.

Alternative: whole hours only, and a day boundary rounded to the hour. Rejected: the spend of the hour around the night's middle would land on the wrong day, and keyboard time would differ from the Day view.

Size, estimated from the author's logs (≈ 92k requests in 30 days, ≈ 2,000 logs): a few hundred thousand minute rows over a year, a store of 10–20 MB as JSON. It loads in about 0.1–0.2 s.

### The format

One JSON file, `days-2.json`: `{ "format": 2, "fingerprint": "...", "logs": [entry...] }`. An entry: `path`, `len`, `mtime_ns`, `fingerprint`, `agent`, `session`, `main`, `subagent`, `project`, `left_out`, `minutes` (`[[minute since epoch, usd, avoidable_usd, requests, commits, pushes, pull_requests], ...]`), `work` (`[[hour since epoch, minute bits], ...]`) and `actions` (`[[ms, 1 when a message else 0], ...]`). Format 2 (from add-month-review, before any release) adds to each minute the tokens read from the cache, all prompt tokens, the dollars the cache reads saved and the unpriced requests, and to each entry the avoidable premium by miss cause and the spend by model per hour, and the times of its cold returns. Spend stays `f64`, so sums equal the summary's sums up to the order of addition. serde with derive, as the share file uses.

The fingerprint is `tokprof <version>; prices <sha-256 of the price entries in use>`, the version of the workspace and the `Debug` text of every price entry and period after all layers are applied. The temp folders that decide `left_out` are not in it: they change only with the machine.

### Refresh

`refresh(catalog, agents, pricing, store, progress)`:
1. Discover the logs of the agents asked for (all their logs; the project and session filters apply later to the entries).
2. Keep each entry whose key (the log's path, or `db#id` for a log in a database) has the same `len` and `mtime_ns` and the current fingerprint.
3. Parse the others with `scan_counted`, one entry per log, and replace or add them. A log that fails to parse keeps its old entry.
4. For an entry whose log was not discovered: if its agent was refreshed and a discovered log has the same agent, session id and main id, drop it (the log moved); else keep it, flagged gone for this run.
5. If any entry changed or was dropped, write the store: serialize to `days-2.json.<pid>.<n>.tmp` in the same folder and rename over `days-2.json`.

Two writers: each starts from the store it loaded and renames a whole file; the last one wins, and what the other added is read again next time. No lock: losing a refresh costs one re-read, while a stale lock could block the window.

An unparsable or wrong-format store is replaced with an empty one, after its file is renamed to `days-2.json.broken`, so a format bug does not destroy history silently.

### The calendar

`Calendar::of(entries, filters, now, offset)`:
1. Filter entries: agent in `--agent`, project contains `--project`, session or main id starts with `--session`.
2. Days: `Days::of` over the actions of the kept, not left-out main entries inside the window, as of `now` (or `--until`), then extended: after the latest action, each local date whose noon has passed starts at its midnight. Before the first day, a moment is on its local date.
3. Per day: sums of the minute rows by the day of the minute's start; agent minutes from the union of the work bits of the filtered entries by UTC hour, each minute to its day; actions by their day; sittings per day from the day's actions with the rhythm's gap and one-minute floor (the code of `Summary::of`, factored out so both use it); sessions with a request as the set of main ids.
4. Per local (weekday, hour): the same sums by the local clock of each minute; keyboard time of each sitting split over the hours it spans.
5. Days shown, levels, streaks, busiest day and weekday, as the spec says.

The calendar is computed for all six metrics at once; the view picks one.

### Day rule chosen

PM decision 3: a cell is a work-rhythm day. The calendar's days come from the store's actions of all logs, while `tokprof day` reads only the logs around its date. The two find different nights only when a log `tokprof day` skips holds the last action before a pause whose middle falls after 18:00 the day before. On the author's logs this is checked on three dates in tasks.md.

The extension after the latest action is new: `Days` lets the latest day run until now, which is right for one day view but would put days of agents working alone into one cell. It applies only to dates after the latest action, so it changes no number of `tokprof day`.

### The CLI view

`tokprof-browse::render_calendar` writes the text: title, grid, legend, numbers, punchcard. The 2-or-1 character cell rule keeps 53 weeks in 57 columns; 38 weeks in 80. Colors: dim for none, 256-color greens 22, 28, 34 and 40 for levels 1 to 4, with the same marks, so the levels read on light and dark terminals and without color.

### The window

`tokprof-browse::calendar::Calendars` follows `reports::Reports`: one worker thread, jobs `(changes)`, the newest taken, `refresh` then `Calendar::of`, progress (`done`, `total`) in atomics with a wake at most every 250 ms, the result kept with when it arrived and how long it took, and `tick`, `next_due` and `settle` with the same `max(4 × took, 20 s)` back-off. The mode enum of `reports` gains `Calendar`; the bar's switch gets a third segment.

`tokprof-desk/src/calendar.rs` draws one scroll pane: a head with the pills and the progress or `updating…`; a card with the grid (squares of 14 pt with 3 pt gaps and a radius of 3 pt, 53 weeks in 900 pt), weekday and month labels, the legend and the numbers beside the grid when the window is wide enough and below it otherwise; a card with the punchcard (7 × 24 squares). Colors: `mix_srgb(panel, accent, t)` for t = 0.3, 0.55, 0.8, 1.0, and `row_line` for none. The tooltip is a small rounded card at the hovered square, or at the cursor while keys move it. The pane is memoised by the calendar, the metric, the cursor, the hover and the theme, as Reports is.

## Effect on parse time

None on other commands: nothing in the parse or the analysis changes. The first `tokprof calendar` parses every log once with one extra sink, as `tokprof summary` does (66 s on the author's logs under tonight's load); its sink is lighter than the summary's collector. Later runs parse only the logs that changed. Loading and writing the store adds about 0.1–0.3 s.

## Risks / Trade-offs

- [The store grows with history] → minute rows only for minutes with requests; at the author's pace 10–20 MB a year. If it grows past that, a binary format can replace JSON under a new format number.
- [A grown log is read whole] → an active session of 100 MB costs a second or two each run; acceptable, and the window reads it on its worker.
- [Deleted logs keep old numbers] → their entries keep the fingerprint they were made with; a price change is not applied to them, since their logs are gone. The `kept` line names those days.
- [Two writers lose one refresh] → read again next time; no corruption.
- [Day starts can differ from `tokprof day`] → only in the edge case above; tasks.md records the check.
