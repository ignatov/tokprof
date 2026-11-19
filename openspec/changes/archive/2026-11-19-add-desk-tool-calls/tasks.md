# Tasks

Start after add-tool-call-log is applied.

## 1. Shared cells (tokprof-browse)

- [x] 1.1 Add the card cells of design decision 1: per group, the name, calls, failed, time and median time, result tokens, the shares of calls and of counted time, and never called. Verify with unit tests that the cells hold the numbers of the groups table's cells, and that the shares of a 340-of-472 group are 72% of calls.
- [x] 1.2 Add the filter by group for the tool cells and the call rows. Use it for `tokprof scan --calls --group` too. Verify with unit tests, and check that the goldens and the render tests of add-tool-call-log do not change.
- [x] 1.3 Run `cargo fmt --all`, `cargo test --workspace` and `cargo clippy --workspace --all-targets -- -D warnings`.

## 2. The Tools tab (tokprof-desk)

- [x] 2.1 Add the tab's state of design decision 2: the session, the chosen group, and the cursor over the joined tool rows and call rows; a state of another session is ignored. Verify with desk tests for a new session and for the joined cursor.
- [x] 2.2 Draw the group cards: 156 pt panels in rows by the inspector's width, the name, calls, failed in red, `<time> · med <med time>`, `<n> tok`, the two 4 pt bars, and the dimmed never-called card. Verify with desk tests for each scenario of Group cards, the 520 pt inspector included, on light and dark themes.
- [x] 2.3 Keep the tool table under the cards, and add the call log table with its colors, the 300-row page that holds the cursor, the note of left-out calls, and the line over it. Verify with desk tests for Tools tab and Call log table.
- [x] 2.4 The filter: a click on a card chooses it with the accent ring and filters both tables, `showing <n> of <m> calls`, a second click or esc clears it, and a never-called card cannot be chosen. Verify with desk tests for each scenario of Filter by group.
- [x] 2.5 The step to `Requests`: a double click or ⏎ on a call row sets `Requests`' selected request and shows it, and `Tools` keeps its state. Verify with desk tests for each scenario of From a call to its request.
- [x] 2.6 The keys of Tools before the inspector's own, the change of Keys in the inspector, and three lines in the key help's `Inspector` group. Verify with desk tests for each scenario of Keys in Tools and of the Tools scenario of Keys in the inspector.
- [x] 2.7 Add `--scene tools-group`. Verify with a desk test of the frame's tree: the first card ringed, both tables filtered, the cursor on the first call row.
- [x] 2.8 Time the build and the first frame of `Tools` on a synthetic X-ray of 3,000 calls in a desk test. Expect under 100 ms.
- [x] 2.9 Describe the window's `Tools` tab, its cards, filter, call log and keys in README.md, and check that the README's key list matches the key help.
- [x] 2.10 In `crates/tokprof-desk`, run `cargo fmt && cargo test && cargo clippy --all-targets -- -D warnings`.

## 3. Checks on the real logs

Record each number next to its task when it is done.

- [x] 3.1 For three sessions with calls in several groups, one of them a Codex session, compare the cards and the call log of `--tab tools --tree` with `tokprof scan <id> --calls`. Record that the numbers match. Done 2026-11-19 with a script over `--tab tools --tree` against `tokprof scan <id> --calls`: a Claude Code session with 7 cards and 208 call rows, a second Claude Code session with 7 cards and 257 rows, and a Codex session with 4 cards and 183 rows; every card and every row matches.
- [x] 3.2 For a Claude Code session with an MCP server it never called, check the dimmed card against the groups table's `never called` row. In a third Claude Code session, the dimmed card of an MCP server says `never called`, as the groups table does; the two Claude Code sessions of task 3.1 likewise for their never-called servers.
- [x] 3.3 For a session with at least 3,000 calls, compare the time of `tokprof-desk --snap --tab tools` with `--snap --tab cache`. Record both. A Claude Code session with 5,997 calls with the subagents and 430 in the main conversation, median of 5 first frames with the machine at load 21: `--tab cache` 8.7 ms, `--tab requests` 44.4 ms, `--tab tools` 49.1 ms. Tools costs about what Requests does: both draw 300 rows. The desk test of 3,000 made-up calls takes 26 ms in an optimized build.
- [x] 3.4 Snap `--tab tools` and `--scene tools-group`, light and dark, for a session with an MCP server and a failed call. Look at the frames and fix what looks wrong. Frames of the first Claude Code session of task 3.1 (an MCP server, failed calls, a never-called server), light and dark, and of a Codex session, a 960 pt window and a session without calls. Fixed: long server names were cut, so a name now wraps to a second line; `never called` sat at the bottom of its card.

## 4. Finish

- [x] 4.1 Build and install the app with `./bundle.sh && ditto target/aarch64-apple-darwin/bundle/tokprof.app ~/Applications/tokprof.app` in `crates/tokprof-desk`, and run `openspec validate --all --strict`.
