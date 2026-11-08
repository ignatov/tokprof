# Design

## Context

The rhythm module of tokprof-core already builds, for each main session, its timeline (typed messages, hand-offs, requests) and its lane: turns with their start, end and how they ended, work spans, the user's actions, `answers` as (end of turn, message) pairs, and questions with their answers. The day view and the pauses rhythm section build one summary over the lanes of the time shown, with the sittings. The advice reads that summary.

The browser (tokprof-browse) costs each listed session on a worker thread by parsing its logs into a glance: cost, the next request warm and cold, when the cache goes cold, and the title. The list sorts by time, cost or next request. Rows never move between sorts (Rows in place). The terminal ui and the window share this browser.

See proposal.md for why.

## Goals / Non-Goals

**Goals:**
- One definition of waiting, in the rhythm module, used by the list, the `queue` line and the advice.
- No extra parse: the wait comes from the same parse that costs the session.
- The `queue` line and the stretch rule computed from the lanes the summary already has.

**Non-Goals:**
- Telling a turn that asks from one that reports (add-interrupt-cost).
- A baseline over past days in the day view.
- Any change to the share file.

## Decisions

### Waiting now comes from the lane
A lane gives "waiting since" from what it already holds:
1. Take the last turn. If it ended with a logged end and no user action came after that end, the session waits since that end.
2. Else, if the last question has no answer and no action came after it, the session waits since the question.
3. Else it does not wait.

The waiting limit (2 hours) is applied by the caller against now. It is not stored, so a session stops waiting without being parsed again.

An open last turn ends `Unlogged` at its last request in Lane, so it starts no wait. That covers an agent that is still working. A permission prompt is not logged either, so an agent stopped at one does not wait. This is the same limit the rhythm already has.

*Alternative:* a separate scan of the log tail for the last `end_turn`. It was rejected: it would duplicate the turn rules and miss the wake rule.

### The live part is opt-in
tokprof is a profiler of past data, so the waiting order is not in the default cycle.
- **`tokprof ui`:** a `--waiting` flag on the `ui` subcommand. The browser gets it at start.
- **The window:** a saved setting, `waiting` in prefs.json. It sits next to `menu_bar`, which is the window's other live option, and is shown last in the settings panel.
- **Without the option:** the cycle has three orders and no row shows a wait. The glance still works out the wait, since one lane costs well under 1% of a glance and it saves costing every session again when the window's setting turns on.
- **`--scene waiting`:** a frame without a window turns the option on for itself, since frames never read saved settings.

*Alternative:* a fourth order for everyone. It was rejected at review: the author wants the profiler of old data to stay the default.

### The glance carries the wait
The glance gets `waiting_since: Option<i64>`, computed from the main session's lane in the same parse.
- Sessions the rhythm leaves out never wait. These are scripted runs and sessions in the temporary folder, such as the 92 IDE commit-message sessions.
- The browser passes the temporary folders it already knows (`temp_folders`) into the glance.

### The waiting order
`Sort` gets a fourth value, `Waiting`, which the cycle reaches only with the option. Its key, with now taken at sort time:
- a session waiting within the limit sorts first, by its wait start, oldest first;
- then the others by last activity;
- then the ones not costed yet.

The list title adds `, <n> wait` from the glances and now when it is drawn.

The time cell shows `waits <span>` while the order is `Waiting` and the row's session waits. For the ui to redraw it, the browser exposes the next time a shown wait changes its span or passes the limit. This is like `next_cold`, which the ui already uses to wake for a cache going cold. In the window, the list already redraws every second while a countdown runs. Otherwise it uses the same next time.

### Several sessions waiting, in the summary
The summary already walks every lane's answers inside the window and keeps the sittings. For the queue:
1. Collect every answer as (end, message, lane, sitting of the message).
2. For each answer, the others waiting are the answers of other lanes with end ≤ message < their message, the same day, and their message in the same sitting.
3. Sort by end and sweep. A day has under 100 answers, so the plain pairwise check is also fine; the study's 30 days had 1,348 answers.

The line prints the share that went to the newest, the mean chance, the median oldest wait and the count.

### Steps from the lane
A step is an answer (end, message) where:
- the message came less than 3 minutes after the end;
- the turn that ended was started by a typed message;
- that turn took less than 3 minutes;
- the end and the message are in the same day, as Answers requires.

Stretches are runs of 3 or more steps among one lane's messages in order. A message that is not a step breaks the run, and so does a message typed while the agent worked. The summary keeps the stretches' count, steps and summed time. The old median rule and its constant `ADVICE_STEP_MS` are replaced.

### Advice
`newest first` goes between `questions` and `out of work`, with the constants 10 answers, 50% and 10 minutes. Its hint names `tokprof ui --waiting` and the key. The window does not show the day's advice today, so no window text is needed.

## Log fields and versions

There are no new fields. Everything here comes from the hand-offs that session-logs reads:
- **Claude Code 2.1.31x:** ends of turn from main-chain `stop_reason: end_turn`; interrupts from `[Request interrupted by user`; questions from the AskUserQuestion and ExitPlanMode tool calls and their results.
- **Codex 0.147–0.166:** `task_complete` and `turn_aborted`.

## Parse time

- **The full corpus:** no change, since `day` and `pauses` read the same lanes. The summary adds one sort and sweep over answers, which is microseconds against the 23–29 s of `tokprof --since 30d pauses`.
- **The browser:** one lane per costed session. That is a linear pass over the session's hand-offs, already parsed, and well under 1% of a glance.

## Risks / Trade-offs

- [A turn that only reports also counts as waiting, so finished sessions sit in the waiting group for up to 2 hours.] → The 2-hour limit. The `Others` label keeps them apart from history. A later change can drop reports once add-interrupt-cost gives end kinds.
- [`newest first` fired on 17 of 26 days in the study and may read as noise.] → It is the habit the change targets. Record how often it fires on the real logs (task), and revisit the thresholds after two weeks of use.
- [The rules come from one person's logs.] → The numbers in the specs are examples. Thresholds are stated as constants and are easy to change.
- [The span in the time cell changes every minute, so the ui must wake for it.] → One timer at the next minute boundary of the shortest shown wait, only in waiting order.

## Migration Plan

Nothing to migrate. Share files and settings are unchanged.
