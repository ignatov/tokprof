# Tasks

## 1. The count (tokprof-core)

- [x] 1.1 Count the agents at work over a day's window from the kept lanes: steps, the peak and when it is first reached, the sessions at a moment, agent-hours, "some agent at work" in minutes as the calendar counts them, and the share of the sittings with 2 or more. Unit tests: overlapping sessions, a session that works twice, the peak's moment, the minute rule, and a day without work.
- [x] 1.2 Write the `agents` line, and `agents    no agent worked`. Unit tests against the scenario texts.
- [x] 1.3 Keep the count with the strip's lanes.

## 2. The terminal (tokprof-browse)

- [x] 2.1 Print the `agents` line after `sittings` in the day text, and not in the pause report's rhythm section. Test the order, and that the other lines are unchanged.

## 3. The window (tokprof-desk)

- [x] 3.1 Draw the `Agents at work` lane under `You`: a stepped area as high as the count, its label with the peak, and the hover text. Make room for it among the lanes. Desk tests for the area's peak and the hover text.
- [x] 3.2 Show the `agents` line under the lanes, after `sittings`. Desk test against the day's text.
- [x] 3.3 In `crates/tokprof-desk`, run `cargo fmt && cargo test && cargo clippy --all-targets -- -D warnings`.

## 4. Check on the real logs

- [x] 4.1 On 2026-11-02, 11-03 and 11-06, compare "some agent at work" with the calendar's `agent` value of the same day, and the peak with a direct count. Record the numbers.
- [x] 4.2 Check that `tokprof day` prints the same lines as before for those days, plus the `agents` line.
- [x] 4.3 Snap 2026-11-01 and 2026-11-03, light and dark, into the night's shots folder, look at them and fix what looks wrong.

Recorded 2026-11-09 on the author's logs:
- 4.1: "some agent at work" equals the calendar's `agent` value, to the minute: 2026-11-01 610 min (10.2h), 11-02 499 (8.3h), 11-03 750 (12.5h), 11-06 634 (10.6h). Agent-hours: 19.3, 11.4, 25.6 and 14.5. Peaks: 11 at 17:08, 3 at 12:49, 5 at 17:42 and 4 at 19:58. Each equals a direct count of the sessions whose work holds that moment, and of the shown lanes with work then. On 11-06, 12 sessions worked but only 10 had lanes: two worked with no action of the user that day.
- 4.2: `tokprof day` for the four days prints the same bytes as a build of dc00621, but for the one `agents` line after `sittings`.
- 4.3: frames in ~/tokprof/plans/night-2026-11-08/shots/concurrency-2026-11-0{1,3}-{light,dark}.png. 2026-11-01 shows 40 sessions with actions; the 62 include those left out. The look fixes: a 26 pt lane made the count read as noise, so the lane is 40 pt and shows the most at once in each 5 minutes as stepped bars.

## 5. Finish

- [x] 5.1 Update README.md (the day view's `agents` line and the lane).
- [x] 5.2 Run fmt, tests and clippy in the root and the window's folder, and `openspec validate --all --strict`.
