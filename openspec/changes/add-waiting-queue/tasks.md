# Tasks

## 1. Rhythm rules (tokprof-core)

- [x] 1.1 Add "waiting since" to the lane: the last turn's logged end, or the last unanswered question, with no user action after it. Add the 2-hour waiting limit as a constant applied against now. Unit tests for the five scenarios of `Waiting now` and the question scenario.
- [x] 1.2 Add steps and stretches to the summary: answers within 3 minutes to turns the user started that took under 3 minutes, and runs of 3 or more in one lane's messages. Keep count, steps and summed time. Unit tests for `A stretch`, `A long turn in between` and `The limits`.
- [x] 1.3 Add several-sessions-waiting to the summary: per answer, the other lanes' answers with end ≤ message < their message in the same day and the same sitting; newest share, mean chance, median oldest wait, count. Unit tests for `The newest first` and `A session answered in a later sitting`.
- [x] 1.4 Print the `queue` line after `answers` in the rhythm lines: plural, singular and none. Unit tests against the `Queue` scenario texts.
- [x] 1.5 Replace the step-by-step advice rule with the stretch rule and its new text, including `Once`. Add the `newest first` item between `questions` and `out of work`, with the constants 10, 50% and 10 minutes. Update the advice tests, the existing step-by-step test included.

## 2. Browser (tokprof-browse, tokprof ui)

- [x] 2.0 Add `--waiting` to `tokprof ui` and an option in the browser that adds the waiting order to the cycle. Tests for `Without the option` and `With the option`.
- [x] 2.1 Compute the glance's waiting-since from the main session's lane in the same parse. Pass the temporary folders, so that left-out sessions never wait. Test that a session in the temporary folder does not wait.
- [x] 2.2 Add `Sort::Waiting` after `Next`: the key (waiting oldest first within the limit, then by time, then not costed) and the title `by waiting, <n> wait` or `by waiting, none waits`. Make the `r re-sorts` mark work with it. Tests for the `By waiting` and `Back to time` scenarios.
- [x] 2.3 In waiting order, show `waits <span>` in the time cell of a waiting row. Expose the next time a shown span changes or a wait passes the limit, and wake the ui for it. Tests for `A waiting row`, `The user answers` and `Another order`.
- [x] 2.4 Make the footer name the orders `s` cycles: `time, cost, next`, and with the option `time, cost, next, waiting`.

## 3. Window (tokprof-desk)

- [x] 3.0 Add the `Waiting order` setting (`waiting` in prefs.json, `Off` by default) last in the settings panel, after `Menu bar countdown`, with Tab and ← →. Picking `Off` in waiting order sorts by time. Add `--scene waiting`. Desk tests for the setting's three scenarios and the frame scenario.
- [x] 3.1 With the setting `On`, add the `Waiting` segment after `Next`, the `Waiting for you`, `Others` and `Nobody waits for you` labels, and the wait in place of the time on waiting rows. Add `Sort by Waiting` to the palette. Desk tests for the scenarios of `Waiting in the window`, `The setting off` included.
- [x] 3.2 Show the `queue` line under the lanes between `answers` and `cache`. Test it against the day's `tokprof day` text.
- [x] 3.3 In `crates/tokprof-desk`, run `cargo fmt && cargo test && cargo clippy --all-targets -- -D warnings`. Then `./bundle.sh` and copy the app into ~/Applications. Snap a frame with `--snap out.png --scene waiting` and look at it.

## 4. Check on the real logs

- [x] 4.1 Run `tokprof --since 30d pauses` and record the `queue` line. Compare it with the study: 71% of 538 answers, 43% by chance, oldest a median 40m. Explain any gap by the rules.
- [x] 4.2 Run `tokprof day DATE` for each of the last 30 days. Record on how many days `step by step` fires (study: 12) and `newest first` fires (study: 17 of 26 days with such answers).
- [x] 4.3 Run `tokprof ui --waiting` while sessions work. Check that the waiting order lists the sessions whose lanes end in a yellow wait today, and that a row leaves the waiting group after an answer and an `r`. Check the result in a pty run.
- [x] 4.4 Time `tokprof --since 30d pauses` and `tokprof day` before and after. Record that the parse time does not change noticeably.

Recorded 2026-11-07 on the author's logs:
- 4.1: `queue     when several sessions waited, you answered the one that waited least first in 71% of 537 answers (43% by chance); the oldest had waited a median 39m`. The study had 538 and 40m; the one answer less is the window moving by a few hours.
- 4.2: of the last 30 days, `step by step` fires on 10 (the old median rule on 19; study 12) and `newest first` on 14 (study 17 of 26 days). Over 30 days in `pauses`: 18 stretches, 77 messages in 1.7h (study 20 and 83).
- 4.3: `tokprof ui --waiting` in a pty (190x30), `s` three times: title `by waiting, 1 wait`, first row `waits 42m`. Without `--waiting`, three presses are back to time, and the footer reads `s sort: time, cost, next`. The check found that a sort kept the list scrolled by the selection, which hid the first waiting row above the screen. A sort now shows the list from its first row, as far as the selection allows.
- 4.4: `tokprof --since 30d pauses` 26.7 s before, 26.3 s after. `tokprof day 2026-11-03` 0.52 s both.

## 5. Finish

- [x] 5.1 Update README.md: the waiting order behind `tokprof ui --waiting` and the window's `Waiting order` setting, the `queue` line in the day view example, and the new step-by-step and `newest first` advice.
- [x] 5.2 Run `cargo fmt --all`, `cargo test --workspace` and `cargo clippy --workspace --all-targets --locked -- -D warnings`. Then run `openspec validate --all --strict`.
- [x] 5.3 Reinstall the binary with `cargo install --path crates/tokprof --root ~/.local`.

## Workflow follow-up

- Archive with `/opsx:archive add-waiting-queue` after landing. Simulate the merge in a copy first, since MODIFIED `Rhythm advice` and `Rhythm lines` must stay within the length limit.
