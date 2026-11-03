## Context

See proposal.md for why. Most of what the new parts need is already collected:
- **The pause collector** keeps, per agent and main session, the spend with subagents, the largest main prompt, the cold returns and the span. It keeps each cold return with its time and the cost of its first request. It keeps typed messages only as a list of pauses, without their session.
- **Busy time** counts the main requests that waited for retries, but not when.
- **The tool table** opens and closes failure windows while it walks a chain, and keeps one total of their spend.
- **The share collector** counts the hand-offs of each main log inside the window, but not their days or sessions.
- **The rhythm** of the share is one summary of all agents together, and one per client.

## Goals / Non-Goals

**Goals:**
- Each new count uses the rule of the part it sits next to, so the counts agree with the existing totals and tests can say so.
- One pass over the logs, as now.
- No value of a single session can be read from a file of few sessions.

**Non-Goals:**
- Changing any rule of an existing part.
- A distribution of each person's spend over days or weeks.

## Decisions

1. **Measures.** Five of the six from the review come from data already collected; one is new and cheap:
   - `session_usd` and `largest_context`: the pause collector's sessions, so their `n` is `main_sessions`.
   - `cold_return_usd`: the pause collector's cold returns, so its `n` is the sum of `cold_returns_by_pause`.
   - `typed_per_session`: a new counter per session in the pause collector. It counts each main-conversation request that a typed message started, the first one of a conversation too. The pause parts leave the first one out, since it has no pause before it.
   - `turn_ms` and `answer_ms`: a rhythm summary per agent, built over the same timelines.
   - Left out: the cost of a failure window. The tool table adds every window's requests into one total and a row's total, and a request may sit in the windows of two rows. A cost per window would need a new rule for that overlap.
2. **Quantiles.** The value at position ⌊n·p/100⌋ of the sorted values, counting from 0. This is how the file's medians and quarters are already taken (the upper of two middle values), so `p50` of `answer_ms` for a single agent equals `rhythm.answers.median_ms`. They are values that occurred, rounded as the file rounds others. With fewer than 5 values only `n` goes in.
   - Alternative: interpolated percentiles. Rejected: they differ from the file's own medians.
3. **The per-agent rhythm uses the days of all agents together.** The rhythm counts an answer only when it comes on the same day as the end of turn, and it finds days from all actions. A summary per agent with its own days gave one answer more than the file's on the author's logs (1,168 against 1,167). One view of all timelines, with the lanes split by agent, makes the agents add up to the file's `rhythm` exactly.
4. **Where the prevalence counts sit.** Each agent has all eight, out of its own `active_days` and `main_sessions`, so a later cut by agent needs no new field. The file also has the three day counts for all agents together, out of its `active_days`. A person who uses two agents on one day has that day once: summing the agents' day counts would count it twice, while main sessions belong to one agent and add up. The pool reads days from the file's part and sessions from the agents'.
5. **The rules of the counts.**
   - Days are local days, as `activity` counts them, and only active days count. A cold return is a main request, so its day is always active. A retried request without an answer, or an interrupt with no request that day, could fall on a day that is not active; such a day does not count, so a count never exceeds its base.
   - `days_with_retry_wait`: busy time now also keeps when each request it counts in `retried` was sent. Log fields: as for `time.retried`, Claude Code's `retries`, `retry_error` and send times (2.1.287–2.1.317).
   - `days_with_interrupt` and `sessions_with_question`: the hand-offs the `handoffs` part counts, inside the window, once the log has a selected request.
   - `sessions_with_failure_window`: the tool table counts the windows it opens; the share notes the main session of the log when that count grows during a request. A subagent's failure counts for its main session, as `failures` counts it.
   - `sessions_over_400k`: a largest main prompt of 400,000 tokens or more, the lower bound of `400–800k`. `sessions_over_1d`: a span of 24 hours or more, the lower bound of `1–3d`.
6. **The pool.**
   - `How often it happens` shows the pooled count over the pooled base, the median file's share, the files where it happened at all, and the files where it happened in 25% or more of their days or sessions. 25% is one day a week of a working month. The files are counted with their share of the files with the part, as `3 (75%)`. Its rows are fixed and count files, so `--min-files` leaves them as they are.
   - `How the sessions spread` shows, per agent and measure, the values of all files, the median and quarters of the files' `p50`, and the median of their `p90`. A file without quantiles counts in `<k> of <n>` files but not in the medians. `--min-files n` leaves out a row with quantiles from fewer than n files, so that a median does not show one person's sessions.
7. **Parse time.** The new work is a counter per request, a push per retried request, a counter per failure window, and one more rhythm summary per agent over timelines already built. Estimate: under 1% of the share's time; on the author's 30 days the runs stayed at about 38 s.
8. **File size.** About 30 numbers per agent for the distributions and 8 for the prevalence, and 3 in the file's part: about 2–3 KB of pretty JSON.

## Risks / Trade-offs

- [Scripted sessions] A program's session counts as a main session, as in the pause parts, so on the author's logs half of the sessions cost a cent or less and hold one typed message. The rhythm leaves such sessions out, so `turn_ms` and `answer_ms` count fewer sessions than `session_usd`. → The spread table names the number of values per row, and the pool compares like with like across files.
- [Codex] Codex logs no questions and few hand-offs, so its `sessions_with_question` is 0. → Counts stay per agent in the file, so a pool can be cut by agent later.
- [Quantiles are real values] From 5 values on, `p90` is one session's value when n is small. → 5 is the floor the review asked for; `--min-files` protects pooled rows.
