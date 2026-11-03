## Why

A share file holds sums over 30 days: one number per person for each pain. A pool of such files shows how big a pain is, but not how many people have it or how often. "One session for $400" and "forty sessions for $10" give the same sum, and so do "cold returns every day" and "a bad week". The survey is meant to pick the pains to work on, and for that it must say how many people a pain touches and how often. This is gap 1 and gap 2 of the PM review of the survey (2026-11-04).

## What Changes

- **New parts in the share file** (additive, format stays 2):
  - `distributions`, per agent: for six measures, how many values there are and, from 5 values on, their quarters and 90th percentile (`n`, `p25`, `p50`, `p75`, `p90`). The measures: the cost of a main session with its subagents, the cost of the first request after a cold return, the typed messages per main session, the length of a turn the person started, the person's answer to an end of turn, and the largest context of a main session. With fewer than 5 values only `n` goes in, so no single session's value can be read.
  - `prevalence`, per agent: the active days with a cold return, with a request that waited for retries, and with an interrupt; and the main sessions with a cold return, with a failure window, with a context of 400k tokens or more, longer than a day, and with a question from the agent. The file also holds the three day counts for all agents together, a day counted once.
- **The report** (`tokprof share` and `tokprof pool`) gets two sections after `Problems`:
  - `How often it happens`: per count, the days or sessions, their share, and for a pool the median file's share, the files where it happened at all and the files where it happened in a quarter of their days or sessions or more.
  - `How the sessions spread`: per agent and measure, the number of values, and the quantiles for one file; for a pool, the median and quarters of the files' `p50` and the median of their `p90`.

## Capabilities

### New Capabilities

### Modified Capabilities

- `share-export`: the distribution and prevalence parts, and their words.
- `share-pool`: the two new sections, their notes, their place in the order of the sections, and `--min-files` for the spread rows.

## Impact

- Core: the share collector and the pool. The pause collector counts the typed messages of each session, busy time keeps when each retried request was sent, and the tool table counts the failure windows it opens. No rule of an existing part changes.
- CLI: `tokprof share` builds one rhythm summary per agent from the timelines it already has.
- Docs: README (the share fields and the pool sections) and `docs/survey.md` (reading the report).

How it is checked on the author's 30 days:
- The counts agree with the existing parts: `sessions_with_cold_return` is at most `main_sessions` and the cold returns of the pause parts, `cold_return_usd.n` equals those cold returns, `sessions_over_400k` equals the sessions of `400–800k` and `800k+`, `sessions_over_1d` those of `1–3d` and `over 3d`, the day counts are at most `active_days`, and the turns and answers of the agents add up to the file's `rhythm`.
- `share` takes no noticeably longer than its ~38 s, and the file grows by a few KB at most.
- `tokprof pool` over the new file and the file of 2026-11-03 prints both sections with `in 1 of 2 files`.

## Out of scope

- Self-report (role, years with agents, billing), coverage of the window, total waiting minutes, and the other gaps 3 to 8 of the review.
- The cost of a failure window as a distribution: the table keeps one total, not the cost of each window.
- New rows in `At a glance`.
- Per-client distributions or prevalence.
