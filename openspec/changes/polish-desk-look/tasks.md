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

- [x] 8.1 Reports keeps the day cursor as a date, so a report with fewer dates (a DST day) cannot index past its end. Regression test with a report that shrinks from 31 to 30 dates.
- [x] 8.2 The palette in Reports runs the Day-only actions after switching to Day. Test.
- [x] 8.3 The key help lists the Flame keys and says what ← → and ⏎ do there; the key-help test checks them.
- [x] 8.4 ⌫ on Flame at the top goes on to the browser, so it leaves full size.
- [x] 8.5 Ctrl+C quits in Reports; `add-desk-reports`'s delta names the keys that still work there.
- [x] 8.6 Flame's clicks find their frame by path, not by index.
- [x] 8.7 Flame's tree cache holds the X-ray it was built from (the browser hands out its Arc), so a later X-ray cannot take its address; the inspector's memo holds its X-ray for the same reason.
- [x] 8.8 The reports worker does not read a period twice in a row: only the report asked for ends the wait, and the worker skips a job it has just answered. Unit test of the wait.
- [ ] 8.9 Not tonight: X-rays build the flame stacks for every session and are never evicted (review item 9). Build the stacks only for Flame, or keep the X-rays of a few sessions.

## 9. The second design review

From `qa2/design-review-2.md`, 2026-11-07.

- [x] 9.1 The lanes show whole lanes, and a line under them says how many more scroll (regression N1). Unit test of the count.
- [x] 9.2 All ten tabs stay on one line at 960 points, and show their digits as keycaps while the inspector has the keyboard and there is room (N2, item 24). Unit test.
- [x] 9.3 A lane's head keeps its own width; a long one is cut to its share, and the title is left out under 60 points (N3, item 8). Unit test.
- [x] 9.4 Narrow tables drop up to `avg input` and `trips` in Tools, and keep the card's padding (N4). Work's origin column is 270 points from a 900-point card.
- [x] 9.5 The list's title without the costing count, which follows it, quieter; narrower sort segments (N5).
- [x] 9.6 Flame's legend swatches at 60% on dark themes (N6). The line under the graph names the frame by its path under the shown root; `add-desk-flame`'s delta and the README say so (item 9).
- [x] 9.7 Waits' notes under their labels, so there is no dead band on the right (N7).
- [x] 9.8 The Cache tab with only expected rebuilds shows the empty state with their count and causes (item 13).
- [x] 9.9 Large marks on dark themes mixed 60% in sRGB (item 14). Test.
- [x] 9.10 Settings opens on the grid of the window's appearance, right after Appearance, and keeps the order while open. The Dark tile's edge is drawn over its picture (item 21). Test of the order and the keys.
- [x] 9.11 The key help has one key column per card column, so the descriptions line up (item 23). The Reports group is not touched.
- [x] 9.12 Frames rendered into `shots/polish2/` and looked at; desk and root checks; `openspec validate --all --strict`.
