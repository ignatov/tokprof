# Tasks

## 1. The record of a request (tokprof-core)

- [x] 1.1 Add the sink call after each request's attribution, carrying the session, the request, the shape, the previous request and the entered blocks (none when the prompt is unknown). Its default does nothing. Unit test that the entered blocks of two requests are what the second added.
- [x] 1.2 Compute the five parts of a request from its usage, price and verdict, as the pass slices them. Unit tests: they add up to the price of the usage, and over a synthetic session with a miss they equal the components of the main chain.
- [x] 1.3 Keep in the X-ray, per main request:
  - the parts, model, effort, gap, cache lifetime and miss (cause, detail, re-processed, avoidable);
  - the entered groups (12 kept, the rest summed);
  - the changes, the failed results, the streak mark and the commits.

  Unit tests for the grouping, the changes (model, effort, tools, version, compaction, gap past the lifetime), the streak (refused calls neither count nor break it) and a commit.
- [x] 1.4 Keep the main log's typed messages, interrupts and compactions, and each subagent's span and task. Unit test on a fixture with a subagent.
- [x] 1.5 Write the why of a request (miss, rebuild, first request, context change) once, for both views, with unit tests for each case.

## 2. The terminal (tokprof, tokprof-browse)

- [x] 2.1 Add `--requests` to `tokprof scan`. Print the head, the `requests` line, the unknown-prompt line, the rows, and the `entered` and `changed` lines. Unit tests against the scenario texts of Requests list, Request rows, Why of a request row, Entered and changed lines, and Prompts the log does not show.
- [x] 2.2 Check that `tokprof scan <id>` without the option prints byte for byte what it printed before, on three real sessions.

## 3. The window (tokprof-desk)

- [x] 3.1 Draw the timeline card: the five-part columns in 4 pt slots, the `$` / `tokens` pill, the left and right scales, the context line, and the warm and cold spans and next request of Summary's track. Desk tests: the parts add up, the slots merge, and the scales.
- [x] 3.2 Draw the markers, the event row and the subagent band, with the $0.50 red threshold and the 8 pt label rule. Desk tests: a miss line per miss of the main chain, red or not by the threshold, and a dashed compaction line.
- [x] 3.3 Add the hover card and the click from one pointer element, and the selection with the costliest request as its default. Desk tests for the hover's texts, a click, and another session.
- [x] 3.4 Draw the selected request's card (head, cost split, why, entered, changes) and the range's card. Add drag to select a range and Shift+↑ ↓ to extend it. Desk tests for each scenario.
- [x] 3.5 Add the table's new columns and why, the 300-row page that holds the selection, and the cards that stay above the scrolling table. Desk tests for the page note and the row that follows the chart.
- [x] 3.6 Add the keys: ← → and ↑ ↓ step, PgUp PgDn Home End, Shift+↑ ↓, `t`, and esc for the range. Put them in the key help. Add `--scene requests-hover` and `--scene requests-range`. Desk tests for the key scenarios, Shift+← → included.
- [x] 3.7 In `crates/tokprof-desk`, run `cargo fmt && cargo test && cargo clippy --all-targets -- -D warnings`.

## 4. Check on the real logs

- [x] 4.1 For three sessions with misses from `tokprof misses --since 7d`, check two things with `tokprof scan <id> --requests`:
  - each row's parts add up to its cost;
  - the `requests` line equals the X-ray's `main` (plus `advisor`) spend.

  Record the numbers.
- [x] 4.2 For the same sessions, compare the miss rows of the list and the timeline's miss lines with the X-ray's misses of the chain `main` (`tokprof misses --session <id> --top 100`). Record the counts.
- [x] 4.3 Time `tokprof scan <id>` before and after on the largest of them, and `tokprof summary` on all logs. Record that they do not change noticeably.
- [x] 4.4 Snap `--tab requests` light and dark for a session with misses and a compaction, and the two scenes, into the night's shots folder. Look at them and fix what looks wrong.

Recorded 2026-11-08 on the author's logs (release build of this change, against a release build of dc00621):
- 4.1: the parts of every request add up to its cost (worst gap 9e-16 dollars). The sums equal the X-ray's `main` spend to the cent: 230043e6 $83.55 (551 requests), ecad11b9 $39.55 (278), f2b772e4 $36.66 (222), ffc9055b $75.21 (547). For f2b772e4, which has no subagent, the five parts also equal the X-ray's spend by component: $18.31 · $5.06 · $7.35 · $0.00 · $5.94.
- 4.2: the miss lines equal the X-ray's unexpected misses of the chain `main`, with the same times and causes: 230043e6 2 of 2, ecad11b9 3 of 3, f2b772e4 2 of 2, ffc9055b 2 of 2. The subagent misses of ffc9055b are not on the timeline.
- 4.3: `tokprof scan` prints the same bytes as before on all four sessions and on 7bb3ca19 (1,648 requests). `tokprof scan 7bb3ca19` takes 1.58–1.81 s before and 1.59–1.60 s after. `tokprof summary` on all logs (9.2 GB) took 43.1 and 45.0 s before, and 29.2 and 35.6 s after, on a machine at load 60–80. The X-ray of 230043e6 takes 122 ms.
- 4.4: frames in ~/tokprof/plans/night-2026-11-08/shots/requests-*.png. The look fixes were: the pointer's card moved over the whole card, so the axis no longer draws over it; the chart is 124 pt tall; the context scale got its own 36 pt at the right, clear of the next request; the timeline stays above the table only in an inspector 760 pt tall or more, else the table is out of sight.

## 5. Finish

- [x] 5.1 Update README.md: `tokprof scan --requests` and the window's `Requests` tab.
- [x] 5.2 Run `cargo fmt --all`, `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings` and `openspec validate --all --strict`.
