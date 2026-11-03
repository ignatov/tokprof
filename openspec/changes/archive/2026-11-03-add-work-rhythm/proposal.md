# Proposal

## Why

tokprof shows what agent work costs, but not how the user and the agents hand work to each other during a day. Without that, the user cannot plan the day: when to come back, how much work to leave the agents before a break, and which hours run in short loops. The study of 2026-11-03 had to find this out with scripts.

It measured 30 days of the author's Claude Code logs: 187 sessions with 1,326 typed messages, after leaving out 92 commit-message sessions that the IDE ran in temporary folders.
- A turn of the agent after a typed message took a median of 3.5 minutes. The user answered a median of 3.7 minutes after the turn ended.
- The user worked in sittings, runs of messages less than 15 minutes apart: about 10 a day, a median of 5 minutes long, with 2 messages to 2 sessions. Breaks between sittings had a median of 36 minutes.
- After the user left, all agents stood still after a median of 8 minutes. At least one agent worked for 28% of a median break.
- While the user was there, an agent ended a turn every 4 minutes. 37% of those ends were of turns the agent started itself, after a background task.
- The user rarely stopped an agent: 39 interrupts, 46 messages typed while the agent worked, and 74 questions from the agent, answered in a median of 2.3 minutes.
- No message came between 02:00 and 08:00 on any of the 30 days. A day that starts at midnight cuts 5 sittings in two; one that starts at 05:00 cuts none.

## What Changes

- **Hand-offs in the logs.** The sources report when the work passed between the user and the agent in a main conversation: the agent ended its turn, the user stopped it, the user typed while it worked, the agent asked the user a question, the user answered it, or something else gave the agent work (a background task or another session). They also mark sessions that a program ran with nobody typing.
- **`tokprof day [DATE]`.** One day, today by default:
  - a strip with a lane per session, showing in each column whether the agent worked, waited for the user's answer, or got a message or an interrupt;
  - five lines on the day's rhythm: sittings, what the agents did after the user left, ends of turn while the user was there, the agent's turns and the user's answers, and interrupts;
  - a cache line with the day's cold returns and what their first requests cost.
- **A rhythm section in `tokprof pauses`.** The same five lines over the selected period, for all agents together, and a table by hour of day.
- **A `rhythm` line in the X-ray**, under `time`: the session's turns, the agent's working time, the medians of its turns and of the user's answers, and the interrupts and questions.
- **The day starts in the middle of the user's night.** tokprof finds the night in the logs: the longest run of half hours with the fewest days on which the user typed then, over the last 30 days. With fewer than 7 days of messages, the day starts at midnight. Every view prints where the day starts.
- **Sessions with nobody typing are left out of the rhythm:** sessions that a program ran, and sessions whose working directory is in the system's temporary folder.
- The README describes the day view, the rhythm section and the rhythm line.

## Capabilities

### New Capabilities

- `work-rhythm`: hand-offs become turns, work, waits, sittings and breaks; the start of the day; the sessions left out; `tokprof day`; the rhythm lines that `day` and `pauses` share.

### Modified Capabilities

- `session-logs`: new requirements for the hand-offs and the sessions a program ran, with the log fields of each agent. `day` joins the commands the filters apply to and the commands that print the line about the logs read.
- `pause-report`: a new requirement for the rhythm section.
- `session-xray`: a new requirement for the `rhythm` line below the header's `time` line. The header requirement itself stays as it is.

## How it is checked on the real logs

The baseline is the study of 2026-11-03 over the 30 days to that morning. The checks run over the 30 days to the day they run, so counts may grow by the sessions since then.

- `tokprof --agent claude --since 30d pauses` prints a rhythm section that matches the study:

  | line | study |
  |---|---|
  | sittings | 306, median 5m, median break 36m |
  | left | stood still after a median 8m; one worked 28% of a median break |
  | present | a turn ended every 4m; 37% of the ends came from turns the agent started |
  | answers | agent's turn median 3.5m; answer median 3.7m |
  | stops | 39 interrupts, 46 typed while the agent worked, 74 questions answered in a median 2.3m |

  Medians may move by half a minute, shares by 3 points and counts by the sessions added since.

  The study's script differs from the specs in these rules:
  - it kept a turn open when the user typed before its end was logged, and took only `end_turn` as an end;
  - it counted the notices that Claude Code queues itself as typing;
  - its sittings held only messages, not answers and interrupts;
  - its breaks and answers ended at a cap of 10 and 8 hours, not at the start of the day.

  Where these rules change a number, the check compares with the script re-run under the specs' rules: 310 sittings with a median of 6.5m, a median break of 34m, `left` at 7m and 23%, 35% of turns started by the agent, answers at a median of 2.9m, and 35 messages typed while the agent worked (46, less 12 notices, plus a slash command the text rule missed). For 2026-11-02 the re-run gives 3.9h at the keyboard and a turn ended every 6.3m.
- Both `day` and `pauses` say that days start at 05:00, with a night of 02:00–08:00.
- `tokprof --agent claude day 2026-11-02` matches the study's numbers for that day: 16 sittings, 3.7h at the keyboard, 58 messages to 11 sessions, a median break of 28m, a turn ended every 7m while the user was there, and 1 of 56 ends came from a turn the agent started itself.
- `tokprof day` takes no longer than `tokprof --since 30d pauses`, and `pauses` takes at most 5% longer than before this change.

## Out of scope

- An SVG of the day, in light and dark. It comes in a later change, once the terminal view has settled.
- A what-if of coming back every N minutes.
- Shares of time by who waited on whom.
- Subagents. Their turns are the agent's own work, not hand-offs with the user.
- Permission prompts. Neither agent logs them, so a wait for one counts as work, or as a silence when it lasts 15 minutes or more.

## Impact

- Model: a session gets its hand-offs and whether a program ran it.
- Sources: the Claude Code and Codex decoders, their fixtures and READMEs.
- Core: a rhythm module, read by `day`, by the pause report and by the X-ray.
- CLI: the `day` command, the rhythm section of `pauses`, the X-ray line.
- README.
