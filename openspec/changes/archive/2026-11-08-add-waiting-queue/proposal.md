# Proposal

## Why

The day view shows the agents waiting for the user, but nothing helps the user answer them in a better order, and its step-by-step advice fires on an ordinary pace. A study of the author's last 30 days of logs (Claude Code and Codex, 1,797 messages, 350 sittings) found:
- **The user answers like a stack.** In 538 answers, at least one other session waited too and was answered later in the same sitting. In 71% of them the user answered the session that had waited least; random picks would give 43%. The oldest had waited a median 40m.
- **Two or more sessions wait for the user half the time at the keyboard.** Counting only waits the user later answered, it is 38%.
- **Rounds look better, but the evidence is mixed.** In sittings of 10–60 minutes, the ones with fewer messages per session made 1.5–2 times the edits and 1.4–3 times the commits per hour at the keyboard, at the same dollars per edit. They also had more sessions running, so part of this is parallel work.
- **Step-by-step steering costs messages, not dollars.** A step is an answer within 3 minutes to a turn under 3 minutes; 22% of messages are steps.
  - Steps cost no more per edit: $1.61 against $1.67, and a median ratio of 0.85 over 47 sessions with edits of both kinds.
  - The agent works 6.0 minutes per step and 13.7 minutes per other message.
  - Real stretches of 3 or more steps in a row are rare: 20 in 30 days, on 12 days.
  - The current advice rule, which uses medians, fired on 16 of 31 days.

The study is in the author's plans folder. Its numbers are for one person.

## What Changes

- **Waiting now.** A session waits for the user from the latest end of turn its agent logged until the user acts in it again. A wait older than 2 hours no longer counts as waiting.
- **The live part is an option.** tokprof profiles past data. So the waiting order turns on only with `tokprof ui --waiting`, and in the window with the setting `Waiting order` (`Off` by default). The `queue` line and the advice read the past and are always on.
- **A `waiting` order in the session list.** With the option, `s` in `tokprof ui` cycles time → cost → next request → waiting.
  - In waiting order, the sessions that wait come first, the one that has waited longest on top. Each shows its wait (`waits 16m`) in place of its time. The other sessions follow in time order.
  - The list title reads `by waiting, <n> wait`.
- **The window.** With the setting `On`, the list gets a `Waiting` button after `Next`. In that order, the labels `Waiting for you` and `Others` head the two groups, and a waiting row shows its wait in place of its time. The palette offers `Sort by Waiting`.
- **A `queue` rhythm line**, after `answers`, in `tokprof day`, the rhythm section of `tokprof pauses` and under the window's lanes: `queue     when several sessions waited, you answered the one that waited least first in 71% of 538 answers (43% by chance); the oldest had waited a median 40m`.
- **A new advice item, `newest first`.** It fires when the time shown has 10 or more such answers, 50% or more of them to the session that waited least, and the oldest waited a median of 10 minutes or more. It points to the waiting order.
- **The step-by-step advice gets a new rule.** It fires on stretches of 3 or more steps in a row in one session, not on medians. Its text speaks of messages, not money.

## Capabilities

### New Capabilities
None.

### Modified Capabilities
- `work-rhythm`:
  - adds `Waiting now`, `Several sessions waiting` and `Steps`;
  - modifies `Rhythm lines` (the `queue` line) and `Rhythm advice` (`newest first`, the new step-by-step rule).
- `session-browser`:
  - modifies `Sort` (the waiting order, with the option);
  - adds `Waiting option` and `Waiting rows`.
- `desk-window`:
  - adds `Waiting in the window` and `Waiting order setting`;
  - modifies `Menu bar countdown setting` (no longer the last setting);
  - modifies `What the lanes say` (the `queue` line).
- `pause-report`: modifies `Rhythm section` (six rhythm lines).

## Impact

- **Code:**
  - the rhythm module in tokprof-core: waits now, several sessions waiting, steps;
  - the costing pass of the browser, which also works out each session's wait;
  - the list of `tokprof ui` and of the window;
  - the rhythm rendering of the day view and the pauses report.
- **Logs:** no new field. The rhythm already reads ends of turn and actions (session-logs). These are Claude Code's `stop_reason: end_turn` and Codex's `task_complete`, as work-rhythm reads them today.
- **Check on the real logs:**
  - `tokprof --since 30d pauses` must print a `queue` line near the study: 71% of 538 answers, 43% by chance, oldest a median 40m.
  - The step-by-step item must fire on about 12 of 30 days with `tokprof day`, instead of 16.
  - With `tokprof ui --waiting` open during work, the waiting order must list the sessions whose lanes in the window end in a yellow wait today.
- **Out of scope:**
  - the menu bar;
  - telling an ask from a report at the end of a turn, which waits for `add-interrupt-cost`;
  - comparing a day with the user's own baseline over past days;
  - the share file and the pool;
  - notifications;
  - hiding a session from the waiting order by hand.
