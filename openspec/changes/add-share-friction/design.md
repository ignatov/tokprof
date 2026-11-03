# Design

## Context

- Share format 2 (add-team-survey) comes from one scan into three sinks: the summary collector, the pause collector, and the share collector. The share collector keeps a tool table per agent.
- add-failures-and-waits added what this change needs, and it is already computed elsewhere:
  - **Call outcomes.** `ToolShare` carries each call's outcome and error. The tool table counts failed, refused and denied calls and their `after` windows. Since that change, the tool table also needs `session` and `request` to open and close the windows. The share collector forwards only `fact`, `block_entered` and `tool_share` to its per-agent table, so its `after` stays empty today.
  - **Busy time.** It is painted per session from the main log and its subagent logs (`busy::Parts::of_main`, `of_subagent`, `finish`). `tokprof pauses` collects the parts during its scan, paints them at the end, and keeps the result in each agent's pause data as `busy`: kinds with the part after turns, the requests, the slow ones, the longest, the retried ones, and `retry_errors`, the error texts.
  - **Hand-offs.** `Session.handoffs` gives the main conversation's ends of turn, interrupts, messages typed while working, questions, answers, and wakes with their source (`subagent`, `command`, `other`).
- Prompt coverage: each request record says whether the prompt was rebuilt (`composition`). The summary's model table counts it as `prompt known`.
- The cold-sessions study defined an active hour as a clock hour with at least one main-conversation request of the client. It counted 312 for the IDE's client over 30 days.

## Goals / Non-Goals

**Goals:**
- The file answers "what problems do people have, and how much of their time and money do they take", per person and per active hour.
- The report shows it, both to the person who runs `share` and to whoever pools the files.

**Non-Goals:**
- A new format number. The parts are added, nothing is renamed, and the pool reads a format-2 file without them as one that lacks the parts.
- Anything that names what failed: error texts, commands, files, the longest waits with their times.

## Decisions

1. **The per-agent tool table gets the whole sink.**
   - The share collector forwards `session` and `request` to the tool table of the current agent too, so its failure windows open and close exactly as in `tokprof tools`.
   - The writer then adds `failed` and `after` to each tool and program, and a `failures` part with the table's `failed`, `refused`, `denied` and `after`.
   - It never reads `latest_error`.
   - The cost is one more tool table's window bookkeeping per request. It is constant work, next to what the summary's own table already does.

2. **Active hours are UTC hours of main-conversation requests.**
   - A set of `ts_ms / 3_600_000` per agent and per client, filled by the selected requests of main chains, advisor calls excluded. This is the study's definition, so its baseline can be computed from a share file.
   - Days are the local days, through the same `day` function that names the window, of the start of each active hour.
   - UTC hours keep the count the same for two people in different zones. A half-hour zone moves no request into a second hour.

3. **Busy time is clipped to the window, in `share` and in `pauses`.**
   - `busy::Parts` gets the window of the global filters when it is built for a report.
   - Requests count, as requests, slow, longest or retried, only when they were sent inside it.
   - The painted intervals are cut at its edges before the sweep.
   - The X-ray of one session builds its parts without a window, so it is unchanged.
   - Why: the busy time of the selected sessions included what they did before `--since`. A Codex thread that ran for months put 107,256 requests into a 30-day busy table, against about 15,700 sent in the window. Rates per active hour need both counts over the same window. At the user's choice, `tokprof pauses` follows too, so the file and the report still agree.

4. **Busy time is painted in the share command as in the pause command.**
   - The share command's scan collects `busy::Parts` per session, exactly like `tokprof pauses`. After the scan it paints them into each agent's pause data.
   - The writer copies, under `time`: the kinds with their milliseconds and part after turns, `requests`, `slow`, `slow_main`, `longest_ms` and `retried`.
   - `retry_errors` and the `waits` (which carry start times) are not copied.
   - The kinds are the busy kinds that agent-time names: `model`, `API retries`, `subagents`, `compaction`, `background commands`, `nothing`, and the activity names. They are all tokprof's own words.

5. **Hand-offs are counted when the log's first request counts.**
   - As compactions are: at `session`, the collector counts the main log's hand-offs inside the filter's window by kind. They are added when the log's first selected request comes, so a session the filters leave out adds none.
   - Wakes are counted by source.

6. **Prompt coverage counts what the summary counts.**
   - Per agent and per version, the requests and dollars whose record says the prompt was rebuilt, next to all of them. The writer adds `held` to each version and a `prompts` part.

7. **Effort by client reuses the levels of the effort part.**
   - Unknown and other levels fold the same way.

8. **The report reads the new parts as optional.**
   - The pool's reader returns `None` for a missing part, as it does for `content` in format 1. Each new table counts only the files that have its part, and says `in <k> of <n> files` like the others.
   - Rates:
     - **per active hour** is a row's pooled value over the pooled active hours of the files that have both parts;
     - **median file** is the median of each file's own rate;
     - **per 100 typed messages** uses the typed messages of the pause parts.
   - `Cold returns by client` takes each client's `active_hours`, `cold_returns` and `avoidable_usd`.
   - `Highest effort by client` takes the client's `effort`.
   - The coverage line takes `prompts`.

9. **Own words.**
   - The new field names, the busy kinds, the hand-off kinds and the wake sources join tokprof's own words. No new word falls outside tokprof's own.

## Log fields and versions

There are no new log fields. The change reads what session-logs already names: call outcomes (Claude Code `is_error` and permission results, 2.1.287–2.1.316; Codex exit codes and `Script failed`), the hand-offs (2.1.310–2.1.316 for Claude Code), the API retry lines (`system`/`api_error`, 2.1.287–2.1.316), and the request records of Claude Code 2.1.300+.

## Parse time

- **Busy painting.** One timeline and one sweep per main session: a few hundred per 30 days, next to a parse of 8.5 GB. `tokprof pauses`, which does the same, takes as long as `share` today (about 25 s). Expected: under 3%.
- **The tool table's windows.** Constant work per request.
- **Active hours.** One set insert per main request.
- **Budget.** 5% over the share time measured before the change.

## Risks / Trade-offs

- [A rate per active hour flatters people who leave agents running alone] → Busy hours stand next to active hours in the header, and the pool shows both.
- [Hand-offs exist only where the sources read them] → They are counted where they are logged. A file with none says so through zeros, and the report counts the files with any.
- [A wait that crosses the edge of the window is cut in two] → Only its part inside counts, as for any other busy time. The longest waits are not in the file.
