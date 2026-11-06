# Tasks

## 1. The stacks in the X-ray

- [x] 1.1 Keep the profile stacks of the default stack in the X-ray, built in the X-ray's pass. Unit-test in the core that the X-ray's stacks of a fixture session equal the stacks `tokprof flame --session` builds for it.
- [x] 1.2 Time `tokprof` (the latest session's X-ray) on the author's logs before and after, and record it here.
  - 2026-11-06, release builds of 46cc395 and of this change: `tokprof scan` of the largest session (da18a2d8, a 2.2 GB log) took 5.9 to 6.4 s of user time before (5 runs, mean 6.15 s) and 5.4 to 6.6 s after (mean 6.15 s); wall time varied 4.5 to 13 s with the file cache on both. The newest session (ddd87db8, 45 MB) took 1.5 s before and 1.6 to 2.0 s after, within the noise.
  - The unit test is in `sources/claude-code/tests/catalog.rs`, since the core has no log source to read a fixture with: the X-ray's stacks of the fixture session, as folded lines, equal the file the profile export writes.

## 2. The tab

- [x] 2.1 Build the frame tree from the stacks: µ$ per stack as the `.collapsed` writer rounds, merged by name and tint, largest first, the shared head folded into the root. Unit-test the sums, the order, the fold and tokens without prices.
- [x] 2.2 Keep the tree beside the desk per X-ray, and the zoom, cursor and hover as paths with the session's id.
- [x] 2.3 Draw the tab: breadcrumb, the graph in a soft group, bars by share with labels and amounts, frames under 3 pt left out and counted, the line under the graph, the colors of the design.
- [x] 2.4 Clicks: a frame zooms, the shown root zooms out, a crumb zooms to it.
- [x] 2.5 Keys: ← → ↑ ↓ (`h` `l` `k` `j`) for the cursor, ⏎ to zoom in, esc and Backspace to zoom out, Shift+← → for tabs, `0` for `Flame`; `--tab flame`; the scene `flame-zoomed`.
- [x] 2.6 Runtime tests: widths proportional to dollars, zoom by click and ⏎, zoom out by crumb and esc, keys move the cursor, hidden narrow frames counted, the root's total equals the `.collapsed` file of the fixture session, Shift+← leaves the tab.

## 3. Check and document

- [x] 3.1 Run `cargo fmt`, `cargo test` and `cargo clippy --all-targets -- -D warnings` in `crates/tokprof-desk`, and `cargo fmt --all`, `cargo test --workspace` and `cargo clippy --workspace --all-targets -- -D warnings` at the root.
- [x] 3.2 Render `--tab flame` in light and dark and `--scene flame-zoomed`, look at them and fix what looks wrong.
  - Fixed after looking: bars under 12 pt were widened by their padding and overlapped their neighbours (padding now only on labelled bars); content read grey in light and loud green in dark (shares by appearance). Shots in `~/tokprof/plans/night-2026-11-06/shots/flame-{light,dark,zoomed-light,zoomed-dark}.png`.
- [x] 3.3 On the newest real session: compare the tab's root with the sum of `tokprof flame --session <id> -o out.collapsed` and the total of `tokprof scan <id>`; record the numbers here.
  - 2026-11-06 23:00: ddd87db8 (newest, live): `.collapsed` sums to 38,642,206 µ$, the tab's root says `$38.64 · 100.0% of the session`, `tokprof scan` `spend $38.64`. 99ba0950: 11,764,140 µ$, `$11.76`, `$11.76`. ffc9055b: 70,290,741 µ$, `$70.29`, `$70.29`. The fixture test checks the root equals the `.collapsed` sum to the µ$.
- [x] 3.4 Update `crates/tokprof-desk/README.md`: the tab, its keys and the scene.
