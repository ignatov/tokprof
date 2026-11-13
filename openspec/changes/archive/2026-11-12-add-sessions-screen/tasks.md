# Tasks

## 1. Data: the glance and the day's spend

- [x] 1.1 In `tokprof-core`, the glance returns requests, first request time, avoidable dollars, cold returns (main chains, cause `idle > TTL`) and the models by request count, from its existing parse. Verify with unit tests next to the glance on a synthetic session with two models, an idle miss and a subagent.
- [x] 1.2 The glance also says whether the session runs now: the latest turn is open, there is no open question, and the time of the latest request. A test covers a turn left open, an ended turn, a question, and the 15-minute silence.
- [x] 1.3 The day store query gives the spend inside given bounds, by local hour and by main session, subagents included, and the spend of any date for the Sessions headers. A unit test on a synthetic store checks a session whose subagent spent inside the day, and the sum against the calendar's cell.
- [x] 1.4 Each cold return of the day computation names its session. A test checks that the returns of a synthetic day name their sessions and add up to the `cache` line.
- [x] 1.5 Measure on the real logs: the summed requests, avoidable dollars and cold returns of all glances against `tokprof summary` and `tokprof pauses`. Record the numbers in this task (last measured: 1,039 sessions, $24,527 against $24,529 priced, $2,715 avoidable, cold returns 275 claude + 310 codex). Record the time of the glances against `tokprof sessions --cost --limit 100000` (14.4 s before the change). Measured 12 Nov 2026 with this build: 1,108 sessions; glances $24,862 against `Total priced` $24,864; 222,217 requests against the summary's 222,193 (live sessions grew between the runs); $2,739 avoidable against $2,739; cold returns 275 claude + 311 codex against `tokprof pauses` 275 + 311. Glances of all sessions 6.0–6.9 s wall; `tokprof sessions --cost --limit 100000` 5.9–6.1 s wall and 45 s CPU, the same as the installed build before the change (5.5–6.1 s, 45 s CPU) on the same machine and load.

## 2. Browser: sorts, quick filters, title filter

- [x] 2.1 In `tokprof-browse`, the sort becomes a column plus a direction for every column of desk-sessions' Table columns, with unknown values last and ties by last activity. `s` keeps its cycle and `tokprof ui` its texts. Verify with browser tests for each column in both directions, and with `TOKPROF_BLESS=1 cargo test -p tokprof --test golden` showing no diff.
- [x] 2.2 The quick-filter predicates are running now, warm, this week, a set of agents and a project. They combine with the text filter, keep the selection rule of session-browser's Filter, and drop rows as the clock turns a session cold or idle. Tests cover `Warm` with `codex`, two agents, and Monday 00:00.
- [x] 2.3 A title-matching flag for the text filter, set only by the window. Tests check that `tokprof ui` still ignores titles.

## 3. Sessions screen

- [x] 3.1 The `Sessions` button, ⌘0, the palette's `Sessions`, and the bar in Sessions (`All sessions`, `since <date> · <n> sessions`, Today, filter, Live). A desk test clicks the button and presses ⌘0 and ⌘1 from Day, Reports and Calendar.
- [x] 3.2 The table: header row, columns with their widths and the order in which narrow windows leave them out, the cells and their formats (Times in the table), `…` while costing, and the `Next` countdown. Desk tests on synthetic sessions read the tree at 1400 and 960 points wide.
- [x] 3.3 Day headers with the count and the day's spend from the store (`…` while it is filled); one list in other orders; a click on a header opens Day. Desk tests cover time order, cost order, a header click, a session that spent on two days (its header is not the sum of its rows), and the headers' spend under a project, under `Warm` with `codex` (it drops when a session goes cold), and under a filter that matches titles only.
- [x] 3.4 Sorting by a header click, `↓`/`↑`, rows in place and `r`. Desk tests click `Avoidable` twice and check rows in place when a cost arrives.
- [x] 3.5 The toolbar: `Sessions | Projects`, the pills with their counts, the project picker, and `Waiting first` with the setting on. Desk tests combine pills and check the counts.
- [x] 3.6 The footer over the whole filtered set, summing whole-life costs, with `costing <n>…`. A desk test with more sessions than fit checks that the footer totals all of them.
- [x] 3.7 Projects mode: project rows with subtotals, groups folded at the start, fold and unfold by click and ⏎, → unfolds and ← folds. Desk tests check the order by cost and the subtotals.
- [x] 3.8 The Sessions inspector: Summary in 360 pt, `Open in Day`, `Copy handoff line`, close, the cache line, and the note about the other tabs. A desk test checks its numbers against Day's Summary of the same session.
- [x] 3.9 Keys in Sessions and the mouse: ↑↓, PgUp/PgDn, Home/End, ⏎, double click, esc (inspector, then filter), `t`/Today, `/`, Tab, `s`, `r`, `f`, `o`, `c`, `q`. Desk tests press each.
- [x] 3.10 The scenes `sessions`, `sessions-projects` and `sessions-cost`, and `waiting` now in Sessions. Desk tests compare their `--tree` text on synthetic sessions. Run them on the real logs and record that the `sessions` footer equals 1.5. Measured 12 Nov 2026 21:02 with this build, the scene and the CLI one after the other: footer `1,124 sessions · all time`, $24,915, $2,746 avoidable (11.0%), 586 cold returns; `tokprof sessions --cost --limit 100000` 1,124 sessions summing $24,914 by their rounded rows, `tokprof summary` `Total priced` $24,915 and $2,746 (11.0%) avoidable, `tokprof pauses` 275 + 311 cold returns. The logs grew since 1.5 (1,108 sessions, $24,862, $2,739, 275 + 311).

## 4. Day layout A

- [x] 4.1 Remove the list from Day. Add the lanes' header with caption and zoom, move the day's name, buttons and Today to the bar, and add the details pane under the lanes. Update the desk tests that read the list in Day. Check `tokprof-desk --snap` at 1400x900 and 960x640 by eye.
- [x] 4.2 Selection with "none": the window starts with no session selected, esc deselects, `‹ The day`, a session without a lane on its day, and stepping days keeps or drops the selection. No X-ray is asked while none is selected. Desk tests cover each scenario of The selected session's day, Details pane and Keys of the window.
- [x] 4.3 `t` in the lanes shows the day of the latest action, as Today does, and `t` in the inspector still switches Requests to tokens. Keyboard focus between the lanes and the details pane, the day summary's cursor, the full-size inspector by ⏎ and double click, and esc never quitting. Desk tests cover Tab, esc, the ring and q.
- [x] 4.4 The draggable border between 25% and 75%, at least 200 pt of details, saved as `lanes_share` in `prefs.json`, and the double click back to the default, which removes the key. Desk tests drag it and count the whole lanes, read the saved place at the next start, clamp a saved 0.9, and check that a frame without a window ignores it.

## 5. Day summary

- [x] 5.1 The six cards. Spend has hour bars and the comparison with the day before, up to the same time on the current day and in full on a past day. The rhythm cards show their big numbers and the text of `tokprof day`'s lines. Unit tests split a line into its number and text. A desk test reads the cards of a synthetic day.
- [x] 5.2 `Top sessions of the day`, `Cold returns of the day` and `Waiting for you now`, with clicks that select. A desk test checks that the top rows and the rest add up to the Spend card, and that the cold returns add up to the Cache card.
- [x] 5.3 The store refresh while Day shows: on a worker, at most once per 20 s and 4 times its length, with the `reading the logs into the store…` progress and no frames at rest. A desk test with an empty store checks the progress text, then the cards.
- [x] 5.4 The default `--snap` frame shows the day summary and waits for the store, and `--tab` selects the newest session. Run on the real logs for today and for `--day` of two past days. Record each Spend card and the Sessions header of that date against the calendar's cell of that date, and each rhythm card against `tokprof day <date>`. On 12 Nov the cell was $508; the day showed 7 sittings, 3.6 agent-hours, a 4.4m median answer, 75% of 8 answers, and 3 cold returns with $4.14 avoidable. Measured 12 Nov 2026 21:50–21:54 with this build, the frames and the CLI on one store: Thu 12 Nov the Spend card $677 and the Sessions header `192 sessions · $677 spent` against the cell $676.83 (the logs grow live; $675 a minute before); Wed 11 Nov $906 and `33 sessions · $906 spent` against $906.11; Tue 10 Nov $281 and `36 sessions · $281 spent` against $281.42; the top sessions and the rest count 182, 40 and 39 sessions, the cells' counts. The rhythm cards read the lines of `tokprof day <date>` word for word: 12 Nov 13 sittings, 9.2 agent-hours, 3.4m, 71% of 17 answers, 5 cold returns with $9.55 and $8.66 avoidable; 11 Nov 15, 12.5, 3.4m, 79% of 24, 6 cold returns ($14.03, $13.57); 10 Nov 11, 12.9, 3.4m, 45% of 20, 16 cold returns ($12.09, $11.08).

## 6. Elsewhere in the window

- [x] 6.1 Palette: the `Sessions` action, session picks that open in Day or select in Sessions, the day names, `Sort by …` that shows Sessions, and `Back to the lanes`. Key help groups `The day` and `Sessions`, with ⌘0. Update the palette's tests.
- [x] 6.2 Reports' project bars open Sessions with the project picked. ⌘0 works in Reports and Calendar. The menu bar's session items open the session in Day. Desk tests cover each.
- [x] 6.3 README.md: the window's section describes the Sessions screen, Day layout A, the keys (⌘0, esc no longer quits, `d` gone) and the new scenes. Check it against `tokprof-desk --help` and the key help.
- [x] 6.4 In `crates/tokprof-desk`, `cargo fmt && cargo test && cargo clippy --all-targets -- -D warnings` pass. In the root, `cargo test --workspace` and `cargo clippy --workspace --all-targets -- -D warnings` pass. `openspec validate --all --strict` passes.
- [x] 6.5 Run `./bundle.sh` and install the app to `~/Applications/tokprof.app`. Open it on the real logs, then go from Day to Sessions and back with a session, a past day and a session nobody typed in.

## 7. After the review

- [x] 7.1 Day headers follow Day's days: a row goes under the day its last activity belongs to by the day store's days (work-rhythm, Start of the day), before the store by the date five hours earlier; `Started`, the palette's days and `Open in Day` use the same day. Desk tests cover a session last active at 02:00 under the day before, its header's spend and its day in Day. Measured 12 Nov 2026 22:41 on the real logs: the Today header read `186 sessions · $713 spent`, the day summary's top sessions and the rest 5 + 181 = 186, and the Spend card $713 (before the change the header counted 196: 10 sessions last active between 00:04 and 04:21 belong to Wednesday).
- [x] 7.2 Day's caption counts `<n> in the lanes`, and `<n> of <m> in the lanes` under a filter. On 12 Nov: `Thursday: 09:24 to now, 13 sittings, 31 in the lanes, $713`.
- [x] 7.3 The `Project` cell, the project rows and the picker name the project as the lanes do; the column fits `temporary folders` at 1400 points and shrinks in a narrow window. Checked in `--scene sessions` frames at 1500x950 and 1100x700.
- [x] 7.4 `crates/tokprof-desk/README.md` follows the root README's window section and the key help.

## Workflow follow-up

- After the author reviews, archive with `/opsx:archive`, after `add-week-view` is drafted on top of this change's desk-window deltas.
