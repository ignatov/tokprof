# Proposal

## Why

Reports shows the last 7, 30 or 90 rolling days. Those windows match no bill, no Month in review card and no plan, and a rolling window cannot say "this month so far against the same point of last month". The author thinks in calendar periods. This change is the third of four in the desk redesign (`~/tokprof/plans/ui-redesign-2026-11-12/`, periods.html): Month and Quarter replace Reports.

## What Changes

- **BREAKING** Reports and its `7 days`, `30 days`, `90 days` pills go away. The switch becomes `Day | Week | Month | Quarter | Calendar`, ⌘1 to ⌘5; Calendar moves from ⌘4 to ⌘5.
- Month and Quarter share one layout. A band of seven key numbers comes first: spend, agent hours, hours at the keyboard, sessions, avoidable with its cold returns, saved by the cache, and commits. Each is compared with the same point of the period before. When the period before has no data or is incomplete, each number shows its pace per day instead. The period's main chart comes next, then breakdown cards, then records and the card to share.
- Month: a calendar grid of the month (spend and heat level per date, cold-return ticks, today outlined, dates to come empty, dates before the logs `no logs`). Beside it, spend by day split into useful and avoidable, with a line through the month before aligned by weekday (its first Monday under this month's first Monday); the totals still compare by date and time. Then today's Reports cards, counted over the month: by project, by origin, by model, cold returns by pause, sessions by largest context, and What to change. Then the records of the month. Last, the Month in review card: the month shown when it is complete, otherwise the last complete month (button and `r`).
- Quarter: a column per Monday-to-Sunday week that touches the quarter (13 or 14), stacked by the quarter's top six projects, the edge weeks cut at the quarter's bounds. Under them come brackets with month subtotals, then three weekly trends: share read from the cache, avoidable share, and average context. Project shifts (started, paused, grew, fell) follow by a stated rule, then by project, by model, What to change and the records of the quarter. There is no forecast and no card of its own. It links to the Month in review cards of its months.
- A click on a day opens Day, on a week (a Quarter column) opens Week, on a month (its name under the weeks) opens Month. A click on a project, in By project or in Quarter's legend, sets the window's filter and stays on the period. ‹ › and ← → step the period, Today and `t` go to the current one. The bar's filter counts every card of the period by agent, project or session id.
- Frames without a window: `--scene month`, `--scene quarter`, `--scene month-advice`, `--scene month-review`, with `--month YYYY-MM` and `--quarter YYYY-Qn`. **BREAKING** `--scene month` used to draw Calendar with the card open. That frame is now `--scene month-review`, drawn over Month. `--scene reports` and `reports-advice` go away (change 2 removes `reports-week`).

Data. The key numbers, the grid, the day chart, the weeks, the trends, the shifts, by project and the records come from the day store format 2 (activity-calendar, month-review). They show in under a second. By origin, by model, the pause tables, What to change and the largest prompt read the period's logs, as Reports reads them today: about 25 s for the author's 9.7 GB, in the background (see design). Nothing new is stored.

Checked on the real logs (2026-11-12, installed tokprof):
- `--scene month --month 2026-10` must show what `tokprof month 2026-10` prints: `$11,632` spent, `221h` agents, `78h` at the keyboard, `389` sessions, `486` commits, `$87,329` saved, `$1,452 (12.5%)` avoidable, `262` cold returns. Biggest day `Sun 25 Oct, $1,221`, busiest hour `23:00–00:00, 7%`, night share `31%`, streak `31 days`. The note must read `no comparison with September: claude's logs start 29 Sep`, with `$375 a day` under spend.
- The same frame must match `tokprof summary --since 2026-10-01 --until 2026-11-01`: `Total priced: $11,632`, origin `prompt $5,291 45.5%`, `claude-opus-5-5 67048 requests $8,392`. It must match `tokprof pauses` for the same dates: Claude `211 cold returns`, `$811` first requests, `$773` avoidable, `800k+ 21 sessions $6,351`; Codex `51` and `$47.91`. It must match `tokprof share --since 2026-10-01 --until 2026-11-01`: `idle > TTL $873 7.5% $2.27`, handoff `$609` cheapest in `205 of 231`.
- The day bars of October sum to the calendar cells' $11,493. The other $139 is the night of 1 October, which lies in 30 September's cell (as in month-review's check).
- November so far on 12 Nov must compare like `tokprof month this`: `$3,954`, `↑ $1,798 (+83%)` against 1–12 Oct.
- Q4 2026 on 12 Nov: spend `$15,586` and avoidable `$1,673 (10.7%)`, as `tokprof summary --since 2026-10-01 --until 2026-11-13` prints. It has 14 weeks, from the week of 28 Sep (Thu–Sun) to the week of 28 Dec. Its note reads `no comparison with Q3 2026: claude's logs start 29 Sep, codex's logs start 6 Aug`.

Out of scope:
- Year, and Calendar turning into Year (change 4, `add-year-view`); Calendar stays as it is, on ⌘5.
- Forecasts of any kind; a Quarter card to share; the "Since logs began" variant.
- Opening Sessions filtered by a miss cause from What to change: a row still opens its top 5 sessions under it.
- The command line: no `tokprof quarter`; `tokprof month` is unchanged.
- A store of the client per request, so that `--client` could filter the store's numbers. With `--client` they count every client, one quiet line says so, and the cards that read logs filter by client.

## Depends on

This change comes after change 1 (`add-sessions-screen`) and change 2 (`add-week-view`). Their drafts appeared while this one was being written. The shared requirements here are written on top of their versions as of 2026-11-12 15:30: `Reports view` (renamed here to `View switch`), `Day controls`, `Key help`, `Quitting the window`, `Calendar view`, `Keys in Calendar`, `Keys in Reports` (renamed `Keys in Month and Quarter`), `Report computation`, `Picks in the menu bar menu`, `An incomplete period before`, `A small agent in the period before`, `Causes gone since the period before`, and change 2's ADDED `Pace per day` and `Window keys in Week`. Every scenario they add is kept. This change removes `Report period`, which change 2 modifies. It leaves `A week of Reports in a frame without a window` to change 2, which removes it.

Checked in a scratch copy of `openspec/` on 2026-11-12, after the reconcile pass: archiving changes 1 to 4 in order succeeds, and `openspec validate --specs --strict` passes on the result.

Requirements ADDED by changes 1 and 2 that this change makes stale get a MODIFIED delta here:
- `Window keys in Week` (change 2): `⌘1 to ⌘4` becomes `⌘1 to ⌘5`.
- `Pace per day` (change 2, Week only) now covers Month and Quarter too.
- desk-sessions' `Sessions screen` (change 1): its `From Reports` scenario names Month and Quarter.

`By project in Week` (change 2) already sets the window's filter and stays on the week, so it needs no delta. Change 2 also brings the filter's text match (agent, project or session id) that this change reuses.

Change 1 makes a click on a Reports project open Sessions with that project picked. That holds only while Reports exists; here, a project click in Month and Quarter sets the window's filter and stays on the period.

## Open questions

1. Default chosen, author to confirm: a period before that is incomplete or without logs gives the pace per day in every view (Week, Month, Quarter, Year), as the Month in review card drops its changes. Reports used to grey them. A cause gone since the period before is then not listed either.
2. Default chosen, author to confirm: `t` goes to the current period in every view, `T` nowhere.

## Decided

- Spend by day: the month before's line is aligned by weekday. The first Monday of the month faces the first Monday of the month before, and each date the date as many days before. A date that faces no date of the month before has no point, the month before's dates that face none are left out, and the line never wraps. Totals and key numbers still compare the same point of the period before by date and time.
- Quarter: a click on a week column opens Week. The legend and the By project rows set the project filter and stay on the quarter. A month's name under the weeks opens Month.
- `--client`: the store's numbers count every client, and the view says so in one quiet line; the key numbers keep their comparison. The cards that read logs filter by client.
- Project shifts keep the drafted thresholds. Task 6.3 measures them on the real logs and adjusts the spec in the same edit.
- Agent hours in the key numbers count each minute some agent worked once (the calendar's `agent` metric, 221h in October). The `agents` rhythm line keeps the sum over sessions.
- The period filter matches agent, project and session id, not titles: the day store holds none. A match by title only gets Week's note with the view's name: `3 sessions match "python" only by title; Month counts agents, projects and session ids, not titles. Sessions (⌘0) lists them.` Day's lanes and Sessions match titles too.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `desk-window`: the switch, the bar's controls in a period, Reports' cards and their computation moved to Month and Quarter, the key numbers and their comparison with the same point of the period before, the incomplete period before, Month in review opening from Month, the keys, the key help, quitting, the menu bar's picks, Calendar's and Week's keys, the pace per day of Week extended to Month and Quarter, and the frames without a window. New: periods, the month grid, records, the quarter's weeks, months, trends, project shifts, the period filter, the quarter's cards to share.
- `desk-sessions`: the `From Reports` scenario of the Sessions screen names Month and Quarter.

## Impact

- `crates/tokprof-desk`: `reports.rs` and `changes.rs` become the Month and Quarter panes; `month.rs` (the card) opens from Month; `main.rs` scenes; `palette.rs`, `keys.rs`, the key help.
- `crates/tokprof-browse`: `reports.rs` reads a calendar period instead of N days and keeps its result per period and filter; new sums by month, week and quarter from the day store.
- `crates/tokprof-core`: the month's numbers from `month.rs` are reused for a quarter; `overview` takes the window's filter text.
- `crates/tokprof-desk/README.md` and `README.md`: Reports becomes Month and Quarter.
