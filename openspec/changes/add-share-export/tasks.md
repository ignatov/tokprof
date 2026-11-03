# Tasks

Apply after `add-client-dimension` and `add-pause-report`. Every group ends with `cargo test --workspace` green, in a state that can be committed.

## 1. Before starting

- [x] 1.1 Check that both earlier changes are applied: `tokprof --client sdk-ts pauses` runs. Record the median of 3 runs of `tokprof summary` and of `tokprof pauses` in a results table at the end of this file. Verify: the table has the numbers and the date.

## 2. New aggregates

- [x] 2.1 Pass each request's effort, as the log names it, to the sinks. Verify: a unit test that a Claude Code request whose `api-request` line has `params.output_config.effort` `max`, and a Codex turn whose `turn_context` has `effort` `xhigh`, reach the sink with those values.
- [x] 2.2 Add the share collector: cache writes by lifetime and chain kind; requests and output by effort level and chain kind; sessions and main sessions. Verify: unit tests for "Lifetimes", "Effort" and "A session with subagents".

## 3. Writer and command

- [x] 3.1 Write the JSON from the summary, pause and share collectors with only the allowed keys: origins cut to their kind, setup sources from the fixed list, dollars with 4 decimals. Verify: unit tests for "Two agents", "Origins without names", "Clients", and the totals scenarios against the summary and pause collectors.
- [x] 3.2 Add the "Distinctive names" fixture test and the "Home directory" test, and run them over the whole written file. Verify: both pass, and they fail when a task label is put into the file on purpose.
- [x] 3.3 Add `tokprof share` with `-o`, the default path in the profile directory, the printed lines, and the no-request failure. Verify: unit tests for "Default path", "Filters" and "No requests".
- [x] 3.4 Update README: the command, each field, and how to run a pilot. That means asking for `--since 30d`, reading the file before sending it, and summing the files. Verify: the README example runs as written.

## 4. Check on the real logs

- [x] 4.1 Run `tokprof --since 30d share`. Verify:
  - spend, misses and setup equal `tokprof --since 30d summary`;
  - the pause parts equal `tokprof --since 30d pauses`;
  - record the file size.
- [x] 4.2 Search the written file for `/Users/`, `~/`, the names of the author's projects and the first 8 characters of 20 session ids. Verify: nothing is found.
- [x] 4.3 Time `tokprof share` 3 times. Verify: its median is at most the sum of the medians of `summary` and `pauses` from 1.1.
- [x] 4.4 Reinstall the binary with `cargo install --path crates/tokprof --root ~/.local`. Verify: `tokprof share -o /tmp/share-check.json` writes the file.

## Results

All on 2026-11-03.

| check | baseline | after |
|---|---|---|
| 1.1 `tokprof --client sdk-ts pauses` | runs (applied in a00b301) | |
| 1.1 `tokprof summary` and `tokprof pauses`, all logs, 3 runs each | medians 24.85 s and 24.55 s (measured for add-pause-report; this change does not touch them) | |
| 2.1 effort reaching the sink | | Claude Code `params.output_config.effort` (decoder test `rebuilds_prompts_from_request_records`), Codex `turn_context` `effort` (`a_turns_effort_reaches_its_requests`), and the analysis passes it on (`cache_writes_by_lifetime_and_effort_by_level`) |
| 3.2 the names test against a file with task labels in it | | fails with `walrus leaked into …`; passes again once they are taken out |
| 3.3 `Default path`, `Filters`, `No requests` | | integration tests in `crates/tokprof/tests/share.rs`, run against the built binary with a temporary home |
| 4.1 `--since 2026-10-04 --until 2026-11-03`, share against summary | Total priced $11,042 | spend totals $11,041.51; all 10 cause rows equal; setup $917.64 against $918 (Claude Code) and $102.99 against $103 (Codex) |
| 4.1 same window, share against pauses | Claude Code 1,009 typed messages, 213 cold returns, $801; Codex 99, 38, $34.88 | the same counts; $800.55 and $34.88 |
| 4.1 file size, `--since 30d` | under 50 KB | 27.5 KB |
| 4.2 search for `/Users/`, `~/`, project names, 20 session ids | | 0 matches each |
| 4.3 `tokprof share`, all logs, 3 runs | at most 24.85 s + 24.55 s = 49.40 s | 25.27 s, 25.19 s, 25.20 s; median 25.20 s |
| 4.4 `tokprof share -o /tmp/share-check.json`, all logs | | 30.8 KB written |
