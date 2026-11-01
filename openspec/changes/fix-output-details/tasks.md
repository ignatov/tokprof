# Tasks

## 1. tokprof flame

- [ ] 1.1 Create the default profile directory after the stack, component and thread options parse. Verify: `TOKPROF_DIR=/tmp/fresh tokprof flame --stack foo` leaves no `/tmp/fresh`.
- [ ] 1.2 Compare output extensions ignoring case. Verify: a unit test for `x.HTML`, `x.JFR`, `x.PB.GZ`.
- [ ] 1.3 List the valid names in the unknown component error. Verify: a unit test.
- [ ] 1.4 Fail when every `--stack` dimension is a `--threads` dimension of a JFR. Verify: a unit test.
- [ ] 1.5 Singular counts in the `wrote …` line. Verify: a unit test of the line with counts of 1.

## 2. tokprof ui

- [ ] 2.1 Leave unreadable subagent logs out of the ui export and count them in the footer. Verify: a unit test with a broken subagent file.
- [ ] 2.2 Open the page with `open` on macOS and `xdg-open` elsewhere.
- [ ] 2.3 `--dump` with a size that does not parse fails with `--dump takes WIDTHxHEIGHT`. Verify: a unit test.
- [ ] 2.4 Stop scrolling the full-screen X-ray at its last page. Verify: `tokprof ui --dump` or a unit test of the scroll bound.

## 3. Handoff and plurals

- [ ] 3.1 Leave `[Request interrupted …]` user lines out of the note's prompts. Verify: a unit test.
- [ ] 3.2 Singular counts in the X-ray (`1 miss`, `1 subagent`, `1 request`), `sessions` (`+1 subagent`) and the ui, outside the lines the open changes rewrite. Verify: unit tests.

## 4. Checks

- [ ] 4.1 Run `cargo test` and the checks in the proposal on the real logs, and record the results in this file.
