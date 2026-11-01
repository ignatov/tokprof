# Tasks

## 1. tokprof flame

- [x] 1.1 Create the default profile directory after the stack, component and thread options parse. Verify: `TOKPROF_DIR=/tmp/fresh tokprof flame --stack foo` leaves no `/tmp/fresh`.
- [x] 1.2 Compare output extensions ignoring case. Verify: a unit test for `x.HTML`, `x.JFR`, `x.PB.GZ`.
- [x] 1.3 List the valid names in the unknown component error. Verify: a unit test.
- [x] 1.4 Fail when every `--stack` dimension is a `--threads` dimension of a JFR. Verify: a unit test.
- [x] 1.5 Singular counts in the `wrote …` line. Verify: a unit test of the line with counts of 1.

## 2. tokprof ui

- [x] 2.1 Leave unreadable subagent logs out of the ui export and count them in the footer. Verify: a unit test with a broken subagent file.
- [x] 2.2 Open the page with `open` on macOS and `xdg-open` elsewhere.
- [x] 2.3 `--dump` with a size that does not parse fails with `--dump takes WIDTHxHEIGHT`. Verify: a unit test.
- [x] 2.4 Stop scrolling the full-screen X-ray at its last page. Verify: `tokprof ui --dump` or a unit test of the scroll bound.

## 3. Handoff and plurals

- [x] 3.1 Leave `[Request interrupted …]` user lines out of the note's prompts. Verify: a unit test.
- [x] 3.2 Singular counts in the X-ray (`1 miss`, `1 subagent`, `1 request`), `sessions` (`+1 subagent`) and the ui, outside the lines the open changes rewrite. Verify: unit tests.

## 4. Checks

- [x] 4.1 Run `cargo test` and the checks in the proposal on the real logs, and record the results in this file.

## Results on the real logs (2026-11-01)

- `TOKPROF_DIR=/tmp/tokprof-fresh tokprof flame --stack foo` fails with the unknown dimension error and leaves no `/tmp/tokprof-fresh`.
- `tokprof --session 3f951de0 flame -o /tmp/x.HTML`: `wrote 4958 frames to /tmp/x.HTML`, an HTML page.
- `--component read,foo`: `unknown component "foo"; use read, write, rewrite, uncached, output`.
- `--threads agent --stack agent -o z.jfr`: `--stack needs a dimension that --threads does not take`.
- `tokprof ui --dump 160xabc`: `--dump takes WIDTHxHEIGHT`.
- `tokprof handoff 3f951de0 --note`: 1 `[Request interrupted` prompt in the snapshot commit, 0 now.
- Session 9428b80a: `sessions` shows `+1 subagent`, and its X-ray reads `133 requests · 1 subagent` and `1 expected rebuild`.
- 2.2 (`xdg-open`) is checked by reading only: this Mac has no Linux desktop to run it on.
- 1.3, 1.4 and 2.3 are checked on the command line above, not by unit tests as planned: those checks sit in `main`, which has no test of its own for them.
