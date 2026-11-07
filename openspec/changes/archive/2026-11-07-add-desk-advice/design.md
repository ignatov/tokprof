# Design

## Context

`tokprof share` reads the logs once into the collector, the pauses and the share sink, scans the main sessions for the bench, builds the share file (`share::to_json_with`) and prints `pool::build` over that one file. `What could have been saved` comes from the file's misses by cause and from the what-if replay of the cache lifetimes; `Coming back after a cold break` from the bench replay; `Habits that cost the most` from the effort, prompt sizes, session spans, triggers, content, setup and tool hints of the file. The dollars per active hour divide by the hours with a main request.

The Reports pass (`overview::collect`) already feeds the collector and the pauses.

Log fields: none new. The rows read what `tokprof share` reads, from the agent versions its sources name (Claude Code up to 2.1.316; Codex as its source README lists), including the effort a request asked for (`Request.params.effort`).

## Goals / Non-Goals

**Goals:** the window's rows are `tokprof share`'s rows; one pass; advice only in words tokprof already has.

**Non-Goals:** a pooled report of several files; the rhythm in the window.

## Decisions

### The same pass, the same report

The pass also feeds `Share` and a new `BySession` sink, and merges `bench::scan` of each main log, as `tokprof share` does. At the end `savings::changes` replays the bench, builds the share file with `Extra { bench, ..default }` (no rhythm: no table of the three reads it), runs `pool::build` over it, and keeps the tables `saved` and `habits` and the bench table. So the numbers are not computed twice in two ways; they are the same function's.

Alternative: compute the rows from the sinks directly. Rejected: the pool rounds through the file and computes the lifetime savings and the habit rows from its parts; a second computation would drift.

### The sessions behind a row

- A cause: the premium of the collector's misses of that cause, by agent and short id.
- Effort max or xhigh, prompts of 400k+, single lookups: the cost of the session's requests of that kind, counted by `BySession` from the request records, as the share sink counts them.
- Thinking and setup: the session's facts of thinking, and of a setup source, from the same facts.
- Sessions longer than a day: the session's whole spend from the pauses' session sizes, whose span is a day or more.
- Not split: the cache lifetime rows (the what-if replay is one simulation per agent and bucket, not per session) and the tool hints (computed from the tool table per agent). They show no ▸ and open nothing.

A unit test holds the sessions of each split row to the row's dollars (to the file's four decimals).

A session is found in the list by its agent and short id; one the list does not hold (a filter left it out) says so in the bar.

### Advice

Each row shows a sentence tokprof already says:

| row | words |
|---|---|
| a cause | the cache-miss advice's hint for the cause (`advice::miss_hint`) |
| cache lifetime | the agents' settings line of `tokprof whatif` (`AgentInfo::ttl_line`) |
| effort max or xhigh, thinking | the thinking advice: use a lower effort for routine steps (`THINKING_HINT`) |
| single lookups | the lookup advice (`LOOKUP_HINT`) |
| setup | the X-ray's setup line for the costliest source with a hint |
| tool hint | its own words |
| the ways back | the rhythm's return advice (`RETURNS_HINT`) |

`sessions longer than a day` and `prompts of 400k+` have no advice of their own in tokprof, so they show none.

### The view

`src/changes.rs` draws the cards with `reports::card` and `reports::cursor_row`: the savings and the ways back side by side, the habits under them. Amounts in the accent; the meters in the accent; the hints in the tertiary ink. The open row is one `(part, name)` in the Reports state, closed when the period changes, and part of the memo's version. The cursor moves over the rows and the open sessions as lines.

## Effect on parse time

The share sink and the bench scan per main log add work to the pass, as `tokprof share` does: `share --since 30d` reads the 30 days in 25.0 s against 25.3 s for `summary`. The window read 30 days in 25.6–27.7 s with them and 25.8–27.9 s without, beside the costing of every row. `pool::build` over one file is milliseconds. Nothing changes for the CLI's commands.

## Risks / Trade-offs

- [The share sink holds more per thread] → a few MB on 30 days, freed with the period's report.
- [A session's short id could match two sessions of one agent] → the first listed is opened; short ids are 8 characters.
