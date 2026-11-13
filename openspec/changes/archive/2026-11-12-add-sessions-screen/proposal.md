# Proposal

## Why

Day in `tokprof-desk` carries a 380-pt list of every session ever (1,039 sessions, $24,527 on the author's logs on 12 Nov) under the day's lanes. The list squeezes the inspector into the rest of the width, and its rows show only time, title and cost: there is no place to sort all sessions by avoidable dollars, to see what a project cost, or to find yesterday's session by its title. This is change 1 of 4 of the desk redesign (sketches in `~/tokprof/plans/ui-redesign-2026-11-12/`: `sessions.html`, Day layout A of `index.html`).

## What Changes

- **Sessions screen**, its own screen in the window: a `Sessions` button in the bar, apart from the switch, and ⌘0. A table of every listed session with the columns last activity, agent, project, title, started, duration, requests, cost, avoidable, cold returns, next request and model.
  - In time order, rows sit under day headers (`Today`, `Yesterday`, …) with the day's count and that day's spend in the day store of the sessions the filters keep; with no filter, the same number as Day's Spend card and Week's row. Rows keep each session's whole-life cost, so a header is not the sum of its rows. Sorted by any other column, or oldest first, the table is one list.
  - A click on a column header sorts by it; a second click reverses it.
  - Quick filters that combine: `All`, `Running now`, `Warm`, `This week` (the calendar week, Mon–Sun), one pill per agent, and a project picker. The bar's filter field matches titles too.
  - A footer totals the whole filtered set (sessions, their whole-life costs, avoidable, its share, cold returns), not only the rows on the screen.
  - A `Sessions | Projects` mode: in Projects, the table starts folded, a row per project with its subtotals. A click or → unfolds one, ← folds it.
  - A narrow inspector on the right shows the selected session's `Summary`. ⏎, a double click or `Open in Day` open Day on the date of the session's last activity, with the session selected.
  - Keys: ↑/↓, ⏎, esc, `t` and `/`, plus the list keys that exist today (`s`, `r`, `f`, `o`, `c`, `q`).
- **Day, layout A**: the session list leaves Day. The lanes stay on top across the full width. Under them a details pane runs across the full width:
  - with no session selected, it shows the day: cards for spend (by-hour bars, against yesterday at the same time), sittings, agents, answers, queue and cache, then the top sessions of the day by that day's cost, the cold returns of the day, and who waits for the user now;
  - with a session selected, it shows the inspector with all ten tabs, as today. esc or `‹ The day` deselect.
  - The day's caption and `Fit` / `24 h` move from the bar to a header line over the lanes.
  - The border between the lanes and the details can be dragged. Its place is saved in `prefs.json` with the other settings, and a double click restores the default.
  - The window starts in Day with no session selected.
- **Choosing a session without the list.** On any day, a session is chosen by its lane, by a row of the day's tables, by the Sessions screen, by the palette or by the menu bar. A session with no lane on its day (one that a program ran, say) still opens in the details pane. Palette and menu-bar picks open Day on the session's day with that session selected. A palette pick made while Sessions is shown selects the row there.
- **The bar** in Day: switch `Day | Reports | Calendar` (⌘1–⌘3, unchanged), `Sessions` (⌘0), `‹ day ›`, `Today`, filter, `Live`. Reports' project bars now open Sessions with that project picked, instead of Day with the list filtered. In the lanes, `t` goes to the day of the latest action, as Today does.
- **BREAKING** for the author's habits: esc no longer quits the window. It steps back: full-size inspector, then the selection, then the filter. `q`, ⌘Q and ⌘W still quit. `d` (the selected session's day) goes away, since the selection is always shown on its day.

Checked on the real logs, with the numbers to compare (measured 12 Nov 2026 14:00 UTC with tokprof 0.1.6):
- The Sessions footer for `All` against `tokprof sessions --cost --limit 100000` and `tokprof summary`: 1,039 sessions; cost $24,527 (sum of the rows) against `Total priced` $24,529; avoidable $2,715 (11.1%) against the summary; cold returns 275 claude + 310 codex against `tokprof pauses`.
- The Spend card of a day, and the Sessions header of that date, against the calendar cell of that date (`tokprof calendar`). On 12 Nov the cell showed $508, equal to `tokprof summary --since 2026-11-12`; the busiest recent day was Wed 11 Nov at $906.
- The Sittings, Agents, Answers, Queue and Cache cards against the lines of `tokprof day <date>`. On 12 Nov: 7 sittings, 3.6 agent-hours, peak 4 at 11:54, answers in a median 4.4m, 75% of 8 answers, 3 cold returns with $4.14 avoidable.
- Time to cost every session for the table: 14.4 s wall (48 s CPU) with `tokprof sessions --cost`, which the window already spends today. The new columns add no log reads.

Out of scope:
- Week, Month, Quarter and Year (changes 2–4); the switch stays `Day | Reports | Calendar`.
- The Spend card's click that opens Week (added by `add-week-view`).
- The terminal `tokprof ui` and `tokprof sessions`, whose list, keys and filter stay as they are.
- Layout B (details beside the lanes), a column chooser, and saving the sort or the mode across launches.

## Depends on

`merge-project-folders`, which lands before this chain: it merges worktrees, named copies and subfolders into their repository root, and temporary folders and Codex chat folders into one project each (391 project paths become 35). The project picker, Projects mode and the project column use those projects. Otherwise this is the first desk change and is written against the current specs. This change also modifies `Saved settings` for the border's place. Later changes write on top of this change's versions of these shared desk-window requirements: `Reports view`, `Keys of the window`, `Palette entries`, `Key help`, `Window layout`, `Day controls`, `Keys in Reports`, `Keys in Calendar`, `Calendar view`, `One frame without a window` and `Frame options`.

## Open questions

1. Default chosen, author to confirm: the key for "today" is `t` in every view, `T` nowhere. Here `t` in the lanes shows the day of the latest action, and `t` in Sessions sorts by time and scrolls to `Today`. In the inspector, `t` keeps switching Requests between dollars and tokens.
2. Default chosen, author to confirm: a project bar of Reports opens Sessions with the project picked. This holds only until `add-month-quarter-views` replaces Reports; from then on, a project row or bar in Week, Month, Quarter and Year sets the window's filter and stays on the period.

## Decided

- esc never quits the window. It steps back: the full-size inspector, then the lane's selection, then the Sessions inspector or the filter. `q`, ⌘Q and ⌘W quit.
- Day headers in Sessions show that day's spend from the day store of the sessions the filters keep (quick filters, project picker, the bar's filter; for a title match, the matched sessions' ids). With no filter it is the same number as Day and Week. Rows keep each session's whole-life cost, and the footer totals those costs. A header is not the sum of its rows, and a scenario says so.
- Projects mode starts folded: a row per project with its subtotals. A click or → unfolds one, ← folds it.
- The filter matches session titles in Day's lanes and in Sessions. Week, Month, Quarter and Year filter by agent, project and session id only (add-week-view says what they show for a match by title only).
- The border between lanes and details is saved in `prefs.json` as `lanes_share` (Saved settings). A double click restores the default.

## Capabilities

### New Capabilities

- `desk-sessions`: the Sessions screen of the window: its table, columns, day groups, sort, quick filters, footer, Projects mode, narrow inspector, opening a session in Day, its keys, and its frames without a window.

### Modified Capabilities

- `desk-window`: Day layout A, with lanes over a full-width details pane, the day summary when no session is selected, and the lanes' header with the day's caption and zoom. Also the bar's controls and the `Sessions` button, focus and keys without the list, esc that no longer quits, the palette, menu-bar and Reports picks, the filter that matches titles, and frames of the new Day.

## Impact

- `crates/tokprof-desk`: a new Sessions view; Day's view, keys, focus, palette, key help, menu-bar picks and scenes.
- `crates/tokprof-browse`: more sort orders and the quick filters on the browser's list; the filter matches titles when the window asks for it; the day computation adds spend by session and by hour, read from the day store.
- `crates/tokprof-core`: the glance of a session also returns its requests, first request, avoidable dollars, cold returns and models, from the same parse.
- `README.md`: the window's section (Sessions screen, Day layout, keys).
- No change to the CLI's output, the day store's format, or prices.
