# Design

## Context

The analysis already has, per session, the typed messages of the main conversation (`Session::typed`) and the requests of each chain with their times. It has no end of turn, no interrupt time, no message typed while the agent worked, no question and no wake as events. The study of 2026-11-03 read those from the raw logs with a script; its numbers are in the proposal.

Constraints that shape the design:
- Only a source may name its agent's fields, tools and values. The core gets neutral events.
- The library reads no clock and no environment variable. The CLI passes the time, the local offset and the temporary folder.
- `day` is a console view. It must not read all 8.5 GB of logs.

## Goals / Non-Goals

**Goals:**
- One set of rules for turns, work, waits, sittings and the start of the day, used by `day`, the pause report and the X-ray.
- The same numbers as the study's script on the same logs, within the tolerances of the proposal.

**Non-Goals:**
- A persistent cache of the start of the day. It is cheap to compute from the logs each view reads anyway.
- Per-conversation rhythm inside one session. A session's main conversations are read as one timeline; in the author's logs a session has one.

## Decisions

1. **Hand-offs are neutral events on the session.**
   - A session gets a list of hand-offs, each with a time and a kind: end of turn, interrupt, typed while working, question, answer, wake. It also gets a flag for a session that a program ran.
   - The sources fill them; the core decides what a hand-off means for a turn.
   - Rejected: deriving ends of turn in the core from the last request before a typed message. That loses the time the agent stopped, the self-started turns and the interrupts.
2. **Claude Code fields, seen in 2.1.310–2.1.316.**
   - End of turn: a main `assistant` line whose `message.stop_reason` is set and is not `tool_use`, the `<synthetic>` error messages included. The lines of one `message.id` give one end, moved to the latest of their times.
   - Interrupt: a main `user` line whose text starts with `[Request interrupted`.
   - Wake: a main `user` line whose text starts with `<task-notification>` or `<cross-session-message`.
   - Typed while working: an `attachment` line with `attachment.type` `queued_command` and `attachment.origin.kind` `human`. Without an `origin`, a prompt that is not `isMeta` and does not start with `<`. A text-only rule also took 12 `[Cross-session delivery notice]` prompts that Claude Code 2.1.310 queues itself with `isMeta`, and missed a slash command typed while the agent worked. The line filter that skips uninteresting lines also lets `"queued_command"` through.
   - Question: a main `tool_use` of `AskUserQuestion` or `ExitPlanMode`; the tool names live in the source's tools module. Answer: the `tool_result` for a remembered question id.
   - A program ran it: the first `entrypoint` is `sdk-cli`.
3. **Codex fields, seen in 0.147–0.166.**
   - End of turn: `event_msg` of type `task_complete`. Interrupt: `event_msg` of type `turn_aborted`. Both only in a main thread, not in a thread with `parent_thread_id`.
   - A program ran it: `session_meta.originator` is `codex_exec`.
   - Codex logs no questions, wakes or messages typed while it works.
4. **A timeline per session, then pure functions.**
   - The scan folds each main session into a small timeline: its agent, id, title, working directory, the flag, the user's actions, the hand-offs, and the times of the main conversations' requests.
   - Rules work on merged timelines after the scan: turns, work, waits, sittings, breaks, the start of the day, the lines and the strip.
   - Rejected: computing the rhythm inside the analysis sink. The sink sees requests one by one, without the hand-offs or other sessions.
5. **Local time comes in as a function.** The rules take an offset from UTC for an instant. The CLI builds it from the system's time zone, so a clock change in the middle of the 30 days moves nothing. Tests pass a fixed offset.
6. **The start of the day uses the logs each view reads.**
   - `day`: the 30 days before the end of the shown day. Because the start of the day decides which day "today" is, `day` reads the main logs last written in the 31 days before now, or in the 30 days before the end of DATE, and picks the day after computing the start.
   - `pauses`: the 30 days before `--until`, or before now.
   - The X-ray: the 30 days before the session's last action, from the session alone. A short session has fewer than 7 days and gets midnight.
7. **Sessions left out.** The CLI passes the temporary folders: `std::env::temp_dir()`, its canonical form, and `/tmp` and `/private/tmp` on Unix. A session whose working directory starts with one of them is left out, as is a session that a program ran.
8. **`day` reads less.** It keeps the main logs last written in its window and skips subagent logs. It runs the analysis with the day's window and the pause report's collector, so the cache line counts cold returns by exactly the pause report's rules.
9. **The pause report** builds the timelines in the same scan as its collector. The rhythm section adds no read.
10. **The X-ray** builds the timeline of the session it already parsed.

## Risks / Trade-offs

- [A turn whose end the log lacks, after a crash or a killed process] → it ends at its last request, so its turn length is not inflated by the time until the next message.
- [Permission prompts are not logged] → a wait for one counts as work, or as a silence when it lasts 15 minutes or more. The proposal lists this as out of scope.
- [Claude Code changes the texts of notifications or interrupts] → session-logs names the versions; the conformance fixtures hold one line of each.
- [A user who travels] → past days shift with the machine's time zone, as all local times in tokprof do.
- [A session file that repeats lines of another after a resume] → hand-offs are kept per session; a repeated line in one file adds no second event, since equal neighbours are dropped.

## Parse time

- The sources add one substring check per line (`"queued_command"`) and a few comparisons on lines they already parse. Estimated under 1% of the parse of the corpus.
- Timelines hold one entry per request and per action: about 70k entries for 30 days. The rules are linear after a sort.
- `day` reads 4.4 GB of main logs for 30 days, against 5.9 GB that `pauses --since 30d` reads. It should take no longer than `pauses`.
