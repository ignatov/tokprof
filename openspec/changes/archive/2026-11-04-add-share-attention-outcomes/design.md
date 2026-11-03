## Context

See proposal.md for why. The pieces this change needs mostly exist already:
- **The rhythm.** It finds sittings, answers, question answers, turns, ends while the person was there, and breaks with what the agents did in them (see work-rhythm). `tokprof pauses` builds it from one timeline per main session, made from the typed messages, the hand-offs and the request times. `tokprof share` does not build it yet.
- **The bench.** It scans main sessions for cold returns during the pass over the logs, then prices the five ways at the end (see return-bench). Its defaults (60 requests, 100k context, 6 hours of heartbeats) live in the CLI's arguments.
- **Shell commands.** Both log sources already pass every full shell command to the core's neutral parser, which gives the call's class. The parser splits a command at `&&`, `;`, `|` and line breaks, and finds the program of each part. The model keeps only the command's head, two words. So `git add -A && git commit` is filed as `git add`, and a commit cannot be found from the head.
- **Retry errors.** Busy time already keeps, per agent, the text of each retried request's last error and its count. The share leaves the texts out.
- **The pool.** It already builds rate rows over the files that have both a value and its base, and prints `<n> more` for folded rows.

## Goals / Non-Goals

**Goals:**
- One pass over the logs, as now. Every new part comes from what the sources already read.
- The new numbers equal those of the commands people already trust: `tokprof pauses` for the rhythm, and `tokprof bench` for the returns.
- The file stays under 150 KB, and `share` takes at most 10% longer.

**Non-Goals:**
- Changing the rhythm, the bench or the parser's classes.
- Telling apart commits a person made by hand. Only the agents' shell calls count.

## Decisions

1. **The rhythm in the share reuses the rhythm, not new rules.** `tokprof share` builds timelines as `tokprof pauses` does: main sessions only, with the same client filter and the same temporary folders. It keeps each timeline with the client of its session's first main request. The rhythm summary over the window gives the file's part. One summary per client, over that client's timelines, gives the client's part.
   - Alternative: count waits from the hand-off counts the share already has. Rejected: they have no times, and two definitions of an answer would drift apart.
2. **Waits are stored with ranges and a median.** Each wait holds a count, the exact median, and counts by the pause ranges, so that a pool can add the ranges and find the range of the pooled median without guessing a number. A client's part keeps only counts and medians, to keep the file small.
   - Alternative: store each wait. Rejected: it would grow the file and could show one person's day.
3. **The bench defaults become named constants of the bench**, which the CLI and the share both use. The share's pass calls the bench scan for each main session that the client filter keeps, merges what it finds as `tokprof bench` does, and prices it at the end with the same pricing.
4. **Version-control actions come from the full command, in the parser.** A new neutral function of the core command parser looks at each part of a command. It reports `commit` for `git commit`, `push` for `git push`, and `pull request` for `gh pr create`, after skipping `git`'s global options (`-C`, `-c`, `--git-dir`, `--work-tree` and `--namespace` with their values, and flags).
   - Each source calls it where it already calls the class parser, and keeps the result on the call's block as a small set of flags.
   - The analysis passes the flags with each call's outcome to the sinks. The share counts the calls whose outcome is not a failure, per agent and per client of the request.
   - Log fields: Claude Code's `Bash` input `command` (2.1.284–2.1.317). Codex's `exec_command` `cmd`, `shell` and `container.exec` `command` (a string or an array), and the `tools.exec_command` calls inside the `exec` tool's code (0.147–0.166).
   - Alternative: read the command's head. Rejected: it misses every commit after `git add &&`, and `gh pr` cannot tell `create` from `view`.
5. **Error classes come from the kept texts at the end.** A neutral function maps a text to a class. A status number from 400 to 599 that stands alone wins first: 5xx is `server`, 429 `rate_limit`, 401 and 403 `auth`, 408 `timeout`. Then come the words: "asleep" is `sleep`, "timed out" or "timeout" is `timeout`, and "connection" with "error", "lost", "reset", "refused", "closed" or "failed" is `connection`. Anything else is `other`.
   - `sleep` was added during apply. 14 of the author's retried requests ended with `Connection lost while your computer was asleep`. Those waits are the computer's, not the API's, and must not read as API trouble.
   - Log fields: Claude Code's `system` lines of subtype `api_error`, `error.formatted` (2.1.287–2.1.316). On the author's logs they read `API Error: 502 …`, `Request timed out.`, `Connection error.` and similar. Codex logged no retries there.
6. **The report.**
   - `At a glance` is built from the same pool tables as the sections, so it cannot disagree with them.
   - The section notes are one table of title and note in the renderer. Titles lose their colon remarks, which move into the notes.
   - `Working with the agent` and the client table divide by the hours at the keyboard.
   - Counts in all tables go through the same thousands formatter as the money tables.
7. **Parse time.**
   - The new work runs on sessions already parsed. Timelines are a sort of a session's events; the bench scan is one walk over its main requests; the action parser runs only on shell commands, which the class parser already splits.
   - `tokprof bench` reads only main logs, in 22.5 s against 25.1 s for all logs, so nearly all of its time is parsing, which the share already does.
   - Estimate: 2–4% more than the share's 24.9 s on the author's 30 days, about 1 s.
8. **File size.** The file part of the rhythm is about 70 numbers. A client's is about 14, so ten clients add about 140. The bench is about 25 per agent, and outcomes and error classes about 50 in all. That is about 8–10 KB of pretty JSON on the author's 135 KB, so the file stays under 150 KB.

## Risks / Trade-offs

- [The rhythm's "same day" depends on the machine's local time] → The rhythm already works that way. Files from other time zones still pool, since only counts and durations go in.
- [Commits through a git GUI, an IDE action or a hook are invisible] → The section says it counts the agents' shell calls. `$ per commit` is a rough baseline to compare people and waves, not a price of work.
- [Parsing more of each shell command could slow the pass] → The parser already splits each command; the action check reads parts it has. Time `share` three times, as before.
- [A pooled median as a range is less exact than a number] → The `median file` column gives exact medians of each file, and the ranges are what can be added without guessing.
- [`Working with the agent` loses its rows for files from before this change, which have no hours at the keyboard] → Only the author's local files are of that kind; the survey uses the release that has the rhythm part.
- [This change modifies requirements that two open changes add] → Archive `add-team-survey`, then `add-share-friction`, then this one. A trial archive of all three in a copy of `openspec/` passes `openspec validate --specs --strict`.

## Migration Plan

The format stays 2, and every new part is additive. A pool of older files says `in <k> of <n> files` for the new sections, or leaves them out. Rollback is a revert: no file holds anything older readers cannot skip.
