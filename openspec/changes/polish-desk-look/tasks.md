# Tasks

## 1. Data display

- [x] 1.1 Every row of a bar chart keeps the note's room when any row has a note; one meter for the inspector and Reports. Test: two rows' bar widths keep the order of their shares.
- [x] 1.2 The largest request's label goes left of its bar or is left out when it would cover the next request's label. Unit test next to it.
- [x] 1.3 Tables drop their least columns first when the card is narrow (Tools, Prompts, Cache); the text column keeps 120 points; paths are cut in the middle. Unit test of the order.

## 2. Bar and cards

- [x] 2.1 Pill segments without borders, circle steps, the Today pill, the filter as a pill with the `/` keycap and the ring while typing.
- [x] 2.2 The inspector's cards as Reports' cards, Summary's row equally tall, pill tabs, the handoff pill with the `c` keycap; the models give way before the project in the header.
- [x] 2.3 Shared radii next to `PILL`, used everywhere.

## 3. Selection and focus

- [x] 3.1 The list's selected row inset and rounded in the selection color; plain day labels.
- [x] 3.2 The ring on the selection of the pane with the keyboard, its title in the accent; the focus line removed; the list gives the ring up while a filter is typed. Tests for the list, the lanes and the inspector.
- [x] 3.3 The selected lane as a lighter inset band with an accent mark; waits with an edge.

## 4. Lanes and the day's lines

- [x] 4.1 A long project gives way before the title.
- [x] 4.2 The day's lines one under the other, the cold count and dollars alone in red. Unit test of the runs.

## 5. Flame, Reports and the overlays

- [x] 5.1 Flame's legend in the frames' colors, its keys as keycaps.
- [x] 5.2 Reports' axis labels on their lines, the label next to a month's gives way, the readout wraps, two-line sub-lines, tokens with `tok` in By model.
- [x] 5.3 The key help's foot of keycaps, the key column per group, one `esc` row in the list, fades on the key help and the palette when they scroll; the dark appearance tile's edge.
- [x] 5.4 One empty state for the Cache tab, the list and the inspector.
- [x] 5.5 Days match in the palette only by word runs; edit `add-desk-palette`'s delta. Test.

## 6. Sizes and colors

- [x] 6.1 The window's size from the root's layout, seeded from `--size`; the lanes' cap includes what is under them. Test at 960x640.
- [x] 6.2 Large marks mixed into the background on dark themes; text colors kept. Test.

## 7. Checks

- [x] 7.1 Frames of the review's set rendered again with the release build and looked at, on Merino light and dark, neon-arcade, sunlit-parchment, cathode-amber, alpine-milk and saffron-nightfall, at 1400x900, 960x640 and 1400x700.
- [x] 7.2 Update `crates/tokprof-desk/README.md`.
- [x] 7.3 `cargo fmt`, `cargo test`, `cargo clippy --all-targets -- -D warnings` in `crates/tokprof-desk`; `openspec validate --all --strict`.

## 8. Fixes from the code review

These restore specified behavior, so they need no proposal of their own; they are recorded here for the review.

- [ ] 8.1 Reports keeps the day cursor as a date, so a report with fewer dates (a DST day) cannot index past its end. Regression test with a report that shrinks from 31 to 30 dates.
- [ ] 8.2 The palette in Reports runs the Day-only actions after switching to Day. Test.
- [ ] 8.3 The key help lists the Flame keys and says what ← → and ⏎ do there; the key-help test checks them.
- [ ] 8.4 ⌫ on Flame at the top goes on to the browser, so it leaves full size.
- [ ] 8.5 Ctrl+C quits in Reports; `add-desk-reports`'s delta names the keys that still work there.
- [ ] 8.6 Flame's clicks find their frame by path, not by index.
- [ ] 8.7 Flame's tree cache is keyed by the session and a generation of the X-ray, not by its address.
- [ ] 8.8 The reports worker does not read a period twice in a row.
- [ ] 8.9 Not tonight: X-rays build the flame stacks for every session and are never evicted (review item 9). Build the stacks only for Flame, or keep the X-rays of a few sessions.
