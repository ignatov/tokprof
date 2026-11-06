# Proposal

## Why

The window shows one day, the session list and one session. The reports over a period are only in the terminal: `tokprof summary` (spend by model and component, by client, by origin; what cache misses cost), `tokprof pauses` (cold returns by pause, sessions by largest context). On the author's last 30 days they say: $11,571 spent, $1,318 (11.4%) avoidable on cache misses, 218 cold returns of Claude Code ($749 avoidable), 62.6% of Claude Code's spend in sessions over 800k context. The author should see this in the window, and go from a day or a project in it straight to its sessions.

## What Changes

- **A Reports view in `tokprof-desk`**: a `Day | Reports` switch in the window's bar, and ⌘1 and ⌘2. Reports takes the place of the lanes, the list and the inspector; the bar stays. Esc goes back to Day.
- **A period**: pills `7 days`, `30 days`, `90 days`, 30 by default, inside the window's command-line filters (agents, project, session, client, until), as the list keeps them.
- **Cards** in soft rounded groups, in the look of the settings panel:
  1. Key numbers: spend, avoidable on cache misses and its share, cold returns with their avoidable dollars, sessions, requests.
  2. Spend by day: a bar per day, stacked by the five cost components of `tokprof summary`; the pointer or the cursor names the day's numbers; a click opens that day in Day view.
  3. By project: the top 10 as bars with dollars and share; a click filters the session list to the project and shows Day view.
  4. By origin (`prompt`, `agent:…`, `skill:…`): the top 10 as bars.
  5. By model: the rows of the summary table.
  6. Cold returns by pause, per agent: the rows of `tokprof pauses`.
  7. Sessions by largest context, per agent: the rows of `tokprof pauses`.
- **Computed in the background**: the window reads the period's logs on a worker while it shows `computing the last 30 days… <n> of <m> logs`, keeps each period's result, and computes it again after the logs change with the back-off the day beside the list uses. A frame never waits for it, and the window at rest does nothing.
- **Keys**: Tab moves between the period and the cards; ← → pick the period and move a cursor over the days (⏎ opens the day); ↑ ↓ move a cursor through a card's rows (⏎ drills down as a click does).
- **The same numbers as the CLI**: the window and `tokprof summary`/`tokprof pauses` take their numbers from the same code in `tokprof-core`. The part of that code that lived in the CLI (which logs a report reads, the total and avoidable lines, the cold-return totals) moves to the core, and the CLI's output stays byte for byte the same.
- **A scene for frames**: `--scene reports` draws Reports for the last 30 days, waiting for it.

## Capabilities

### New Capabilities
None.

### Modified Capabilities
- `desk-window`: the window gains the Reports view, its switch in the bar, its period, cards and keys, and the scene `reports`; the bar's day controls, the layout and esc change in Reports.

## Impact

- `crates/tokprof-core`: a module that collects a period's report in one pass (summary's collector, the pauses, and the spend by day and by project), a scan that counts the logs it read, and the totals moved from the CLI.
- `crates/tokprof-browse`: the reports worker with its cache and back-off; the browser lets a screen set its filter and read its count of log changes.
- `crates/tokprof`: uses the moved functions; no change to any output.
- `crates/tokprof-desk`: `src/reports.rs` with the view and its keys, small hooks in `main.rs` and `view.rs`, the scene; README.
- No change to `tokprof ui`, the log sources or the parse.

## How it is checked on the real logs

- `tokprof-desk --snap … --scene reports` against `tokprof summary --since 30d` and `tokprof pauses --since 30d`, run at the same time: spend and avoidable (with its share) equal the summary's `Total priced` line; the by-model rows equal its table; the origins equal its origin table; the cold-return rows and totals and the largest-context rows equal the pauses tables, per agent. The day bars sum to the spend, and the projects too. The numbers are recorded in tasks.md.
- `tokprof summary` and `tokprof pauses` with fixed `--since`/`--until` dates give the same bytes before and after the change.
- The window at rest after a report arrives draws no frames.

## Out of scope

- `tokprof misses`, `tokprof tools` and `whatif` in the window; the clients table; tasks, triggers and activities.
- Periods other than 7, 30 and 90 days, and custom ranges.
- A report that updates incrementally as one log grows: a change of the logs reads the period again.
- Reports in `tokprof ui` in the terminal.
