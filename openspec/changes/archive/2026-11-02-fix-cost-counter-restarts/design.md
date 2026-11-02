# Design

## Context

See proposal.md, "Why". This change relies on what Claude Code 2.1.312 does with its counter, as its binary and the logs show:
- A process writes a `cost-state` line when it ends, and at some other points. The fields used are `totalCostUSD` (the counter) and `startTime` (epoch milliseconds of the run's start).
- On resume, a process restores the counter from the session's latest `cost-state` line, `startTime` included. With no such line it starts from zero, and its own start becomes the run's `startTime`.

So one `startTime` names one run of the counter. Every process that restored the counter continues the run, and the values it writes include the run's earlier spend.

In the corpus, all 9,942 `cost-state` lines of the 282 sessions that have them carry `startTime` (Claude Code 2.1.284–2.1.312). Five sessions have more than one run.

## Goals / Non-Goals

**Goals:**
- The counter figure is what Claude Code counted over the whole session, as far as the transcript tells.

**Non-Goals:**
- Finding restarts from request lines or process logs.
- Adding spend the counter never wrote down, or spend the transcript does not log.

## Decisions

- **A run is the set of lines with one `startTime`, wherever they are in the transcript.** In 7e640abc two processes wrote at once, and their lines interleave. Splitting runs by line order would count the interleaved run twice. The alternative, "a drop in the value starts a new run", fails in the same case.
- **Each run counts with its highest value, not its last.** Within one process the counter only grows, so the two agree. When two processes restore the same run, the later line can be the lower one, and the highest value is the least the run counted.
- **Lines without `startTime` are one run.** A transcript from a version without the field then gets its highest value, which for one process is the old last value.
- **The rule stays in the Claude Code log source.** The report still adds up one figure per session, and only the source names the log fields.

## Risks / Trade-offs

- [A later Claude Code release stops restoring `startTime`, so every resume opens a run] → Restored spend would count twice, and the reconciliation line would show the counter far above tokprof at once. The source README names the versions where the behavior was seen.
- [Two processes restore one run and both spend] → The highest value covers one of them only, so the counter stays low for that session. No run in the corpus ever drops in value, so this has not happened yet; the one session with concurrent processes, 7e640abc, has them under different `startTime`s.

## Parse time

The source already parses every `cost-state` line. Reading one more field on 9,942 lines out of 5.8 GB of Claude logs does not change the full read of about 7 s.
