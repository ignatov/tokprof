# Tasks

## 1. The count (tokprof-core)

- [ ] 1.1 Count the agents at work over a day's window from the kept lanes: steps, the peak and when it is first reached, the sessions at a moment, agent-hours, "some agent at work" in minutes as the calendar counts them, and the share of the sittings with 2 or more. Unit tests: overlapping sessions, a session that works twice, the peak's moment, the minute rule, and a day without work.
- [ ] 1.2 Write the `agents` line, and `agents    no agent worked`. Unit tests against the scenario texts.
- [ ] 1.3 Keep the count with the strip's lanes.

## 2. The terminal (tokprof-browse)

- [ ] 2.1 Print the `agents` line after `sittings` in the day text, and not in the pause report's rhythm section. Test the order, and that the other lines are unchanged.

## 3. The window (tokprof-desk)

- [ ] 3.1 Draw the `Agents at work` lane under `You`: a stepped area as high as the count, its label with the peak, and the hover text. Make room for it among the lanes. Desk tests for the area's peak and the hover text.
- [ ] 3.2 Show the `agents` line under the lanes, after `sittings`. Desk test against the day's text.
- [ ] 3.3 In `crates/tokprof-desk`, run `cargo fmt && cargo test && cargo clippy --all-targets -- -D warnings`.

## 4. Check on the real logs

- [ ] 4.1 On 2026-11-02, 11-03 and 11-06, compare "some agent at work" with the calendar's `agent` value of the same day, and the peak with a direct count. Record the numbers.
- [ ] 4.2 Check that `tokprof day` prints the same lines as before for those days, plus the `agents` line.
- [ ] 4.3 Snap 2026-11-01 and 2026-11-03, light and dark, into the night's shots folder, look at them and fix what looks wrong.

## 5. Finish

- [ ] 5.1 Update README.md (the day view's `agents` line and the lane).
- [ ] 5.2 Run fmt, tests and clippy in the root and the window's folder, and `openspec validate --all --strict`.
