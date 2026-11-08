# Tasks

## 1. The record of a request (tokprof-core)

- [ ] 1.1 Add the sink call after each request's attribution, carrying the session, the request, the shape, the previous request and the entered blocks (none when the prompt is unknown). Its default does nothing. Unit test that the entered blocks of two requests are what the second added.
- [ ] 1.2 Compute the five parts of a request from its usage, price and verdict, as the pass slices them. Unit tests: they add up to the price of the usage, and over a synthetic session with a miss they equal the components of the main chain.
- [ ] 1.3 Keep in the X-ray, per main request:
  - the parts, model, effort, gap, cache lifetime and miss (cause, detail, re-processed, avoidable);
  - the entered groups (12 kept, the rest summed);
  - the changes, the failed results, the streak mark and the commits.

  Unit tests for the grouping, the changes (model, effort, tools, version, compaction, gap past the lifetime), the streak (refused calls neither count nor break it) and a commit.
- [ ] 1.4 Keep the main log's typed messages, interrupts and compactions, and each subagent's span and task. Unit test on a fixture with a subagent.
- [ ] 1.5 Write the why of a request (miss, rebuild, first request, context change) once, for both views, with unit tests for each case.

## 2. The terminal (tokprof, tokprof-browse)

- [ ] 2.1 Add `--requests` to `tokprof scan`. Print the head, the `requests` line, the unknown-prompt line, the rows, and the `entered` and `changed` lines. Unit tests against the scenario texts of Requests list, Request rows, Why of a request row, Entered and changed lines, and Prompts the log does not show.
- [ ] 2.2 Check that `tokprof scan <id>` without the option prints byte for byte what it printed before, on three real sessions.

## 3. The window (tokprof-desk)

- [ ] 3.1 Draw the timeline card: the five-part columns in 4 pt slots, the `$` / `tokens` pill, the left and right scales, the context line, and the warm and cold spans and next request of Summary's track. Desk tests: the parts add up, the slots merge, and the scales.
- [ ] 3.2 Draw the markers, the event row and the subagent band, with the $0.50 red threshold and the 8 pt label rule. Desk tests: a miss line per miss of the main chain, red or not by the threshold, and a dashed compaction line.
- [ ] 3.3 Add the hover card and the click from one pointer element, and the selection with the costliest request as its default. Desk tests for the hover's texts, a click, and another session.
- [ ] 3.4 Draw the selected request's card (head, cost split, why, entered, changes) and the range's card. Add drag to select a range and Shift+↑ ↓ to extend it. Desk tests for each scenario.
- [ ] 3.5 Add the table's new columns and why, the 300-row page that holds the selection, and the cards that stay above the scrolling table. Desk tests for the page note and the row that follows the chart.
- [ ] 3.6 Add the keys: ← → and ↑ ↓ step, PgUp PgDn Home End, Shift+↑ ↓, `t`, and esc for the range. Put them in the key help. Add `--scene requests-hover` and `--scene requests-range`. Desk tests for the key scenarios, Shift+← → included.
- [ ] 3.7 In `crates/tokprof-desk`, run `cargo fmt && cargo test && cargo clippy --all-targets -- -D warnings`.

## 4. Check on the real logs

- [ ] 4.1 For three sessions with misses from `tokprof misses --since 7d`, check two things with `tokprof scan <id> --requests`:
  - each row's parts add up to its cost;
  - the `requests` line equals the X-ray's `main` (plus `advisor`) spend.

  Record the numbers.
- [ ] 4.2 For the same sessions, compare the miss rows of the list and the timeline's miss lines with the X-ray's misses of the chain `main` (`tokprof misses --session <id> --top 100`). Record the counts.
- [ ] 4.3 Time `tokprof scan <id>` before and after on the largest of them, and `tokprof summary` on all logs. Record that they do not change noticeably.
- [ ] 4.4 Snap `--tab requests` light and dark for a session with misses and a compaction, and the two scenes, into the night's shots folder. Look at them and fix what looks wrong.

## 5. Finish

- [ ] 5.1 Update README.md: `tokprof scan --requests` and the window's `Requests` tab.
- [ ] 5.2 Run `cargo fmt --all`, `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings` and `openspec validate --all --strict`.
