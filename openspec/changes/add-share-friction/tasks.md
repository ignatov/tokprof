# Tasks

Apply after `add-team-survey`. Every group ends with `cargo test --workspace` and `cargo clippy --workspace --all-targets -- -D warnings` green, in a state that can be committed. Names from the author's logs never go into tests or docs: use made-up names such as `walrus` and `zebra`.

## 1. Before starting

- [x] 1.1 With the current binary, over one closed window (`--since 2026-10-04 --until 2026-11-03`), record in the results table below:
  - the failures line of `tokprof tools`;
  - the busy tables and slow lines of `tokprof pauses`;
  - the `prompt known` column of `tokprof summary`;
  - the median of 3 runs and the size of `tokprof --since 30d share`;
  - and keep a format-2 file without the new parts: `tokprof --since 30d share -o /tmp/share-before.json`.

  Verify: the table has the numbers and the date, and the file exists.

## 2. The collector

- [x] 2.1 Forward `session` and `request` to the current agent's tool table. Write `failed` and `after` for each tool and program, and the `failures` part. Verify: unit tests for "The same numbers as the tools command" and "An error that names a project".
- [x] 2.2 Count the active hours of main-conversation requests per agent and per client, and the active days from them. Verify: unit tests for "Requests in two hours of one day" and "A subagent alone", and a test with a request at 23:30 UTC that counts its local day.
- [x] 2.3 Count the hand-offs of main logs inside the window, added at the log's first counted request. Verify: unit tests for "One session's hand-offs" and "A hand-off outside the window".
- [x] 2.4 Count the requests and dollars whose prompts the logs hold, per agent and per version. Verify: unit tests for "An old Claude Code" and "A version without request records".
- [x] 2.5 Add `effort` per client. Verify: a unit test for "A client at max".
- [x] 2.6 Add the new keys to tokprof's own words. Verify: a unit test for "Nothing new to review", over a file with every new part.

## 3. Busy time in the share command

- [x] 3.0 Give `busy::Parts` the window of the global filters: count requests sent inside it, and cut the painted intervals at its edges. Pass it in `tokprof pauses`; the X-ray stays without one. Verify:
  - a unit test for "A session that began before the window" and for the same session without a window;
  - `tokprof pauses` without filters prints the same busy tables as before.
- [x] 3.1 Collect the busy parts per session in the share command's scan, as `tokprof pauses` does, and paint them into the pause data. Write the `time` part without `retry_errors` and without the waits. Verify:
  - an integration test that `share` and `pauses` give the same busy numbers for a fixture with a retried request and a subagent;
  - a unit test for "A retry error that names a host".

## 4. The report

- [x] 4.1 Read the new parts as optional in the pool. Verify: a unit test that a format-2 file without them leaves the new tables at `in 1 of 2 files`, and "Format-1 files only".
- [x] 4.2 Add the activity line, and the `per active hour` column with `Cold returns by client`. Verify: unit tests for "One file", "A pool" and "A client's baseline".
- [x] 4.3 Add `Problems`, `Where the agent's time went` and `Working with the agent`. Verify: unit tests for "Failures of one person", "Slow requests", "Subagents after turns" and "Interrupts".
- [x] 4.4 Add `Highest effort by client`, the prompt coverage line, `failed` in the tool table, and the order of the sections. Verify: unit tests for "A client at max" (report), "One file with old transcripts", "A failing program" and the order.
- [x] 4.5 Check `--json` and `--by-file` over the new tables. Verify: an integration test that `--json` holds the new tables with numeric values, and `--by-file` adds a column per file to `Problems`.

## 5. Docs

- [x] 5.1 Update README:
  - the share section's field table, with `activity`, `failures`, `time`, `handoffs` and `prompts`, the client additions, and `failed` and `after` in `tools`;
  - the pool section, with the new sections of the report.

  Verify: the README examples run as written.
- [x] 5.2 Update `docs/survey.md` on what the file holds and on reading the report. Verify: each new section of the report is described there.

## 6. Check on the real logs

- [x] 6.1 Over the closed window of 1.1, check that `failures` equals the tools failures line, `time` equals the pause report, and `prompts` equals the `prompt known` counts. Record the numbers.
- [x] 6.2 Record the active days and active hours of the author's 30 days, and the active hours of the IDE's client, next to the 312 of the cold-sessions study. Verify: the numbers are recorded with the window.
- [x] 6.3 Search the new file for the error texts of the author's failed calls and retries, from the X-ray's failure lines and `tokprof pauses`. Verify: none is found, and the words to review are the same as in `/tmp/share-before.json`.
- [x] 6.4 Pool the new file with `/tmp/share-before.json`. Verify: each new section says `in 1 of 2 files`. Record the share report of the new file in the results.
- [x] 6.5 Time `share` 3 times, and record the size. Verify: the median is within 5% of 1.1, and the size is under 150 KB.
- [x] 6.6 Reinstall the binary with `cargo install --path crates/tokprof --root ~/.local`, then run `cargo test --workspace`, clippy and `openspec validate --all --strict`. Verify: all pass.

## 7. Fixes after the first look at the report

- [x] 7.1 A cache-lifetime row adds the replay under the lifetime the bucket wrote most with, minus the cheaper replay. It used the logged cost minus the cheaper replay, and the logged cost holds the other misses too, which their own rows count (add-team-survey, share-pool). Verify: the unit test that a cache lifetime saves the difference of the replays from the one in use, with the scenarios "A file already on the cheaper lifetime" and "A file on the dearer lifetime".
- [x] 7.2 Each client in `clients` holds `cold_returns_avoidable_usd`, and `avoidable per active hour` in `Cold returns by client` uses those dollars, not those of all the client's misses. Verify: the unit tests for "Cold returns of a client" (share) and "A client's baseline" (pool).
- [x] 7.3 `Working with the agent` gives the hand-offs per active hour, and per 100 typed messages only for interrupts, messages typed while the agent worked, questions and answers. Ends of turn came to 229.66 per 100 typed messages, because many turns end after a wake. Verify: the unit tests for "Interrupts" and "Ends of turn".
- [x] 7.4 Run `share` over the author's 30 days again. Verify: `cache lifetime: main` is $0, `cache lifetime: subagents` is the 1-hour replay minus the 5-minute one of `whatif`, the avoidable dollars per active hour of each client are those of its cold returns, and ends of turn have no value per 100 typed messages. Record the numbers.

## 8. Fixes after the second look at the report

- [x] 8.1 `Problems` prints no row for the longest request. Codex logs no send times, so a request starts at the latest tool output. On 2026-10-09 the computer slept 1h43m between a tool's output and the model's answer, which came with a cold cache and 1,168 output tokens, and made a request of 2.0h. The file keeps `longest_ms`, which `tokprof pauses` shows for one person. Verify: the unit test of "Failures of one person" finds no `longest request`.
- [x] 8.2 `Highest effort by client` gives each client's cost, costliest first, and shows the clients with 1% of the pooled spend or more, at most 8, then `<n> more`. It listed 8 of 10 clients by their share at max, so a client of $0.91 came second, above one of $8,999, and two clients were left out without a `more`. Verify: the unit tests for "A client at max" and "A small client".
- [x] 8.3 Under the model table, the report names the models without a price with their tokens, from the `priced` flag that the file already wrote and that the spend parts of share-export now name. Verify: the unit tests for "Codex's auto-review" and "A pool", and that `--min-files 2` leaves out a model that one file has.
- [x] 8.4 Render the author's file of 2026-11-03 with the binary before and after the fixes, and run `share` again. Verify: the report differs only in the three places, and the new file has the same keys and words to review. Record the numbers.

## Results

| check | before | after |
|---|---|---|
| 1.1 tools failures line | 2026-10-04 to 11-03: failed 1,311 calls, after them $215 until the tool worked again · refused 55 · rules denied 92 | |
| 1.1 pauses busy and slow lines | whole sessions: Claude Code 3,002 of 83,990 requests a minute or more (3.6%), longest 23m; Codex 2,640 of 107,256 (2.5%), longest 3.4h | |
| 1.1 summary prompt known | claude-opus-5-5 85%, claude-fable-5-1 48%, claude-sonnet-5-5 96%, claude-haiku-5-5 1%; every Codex model 100% | |
| 1.1 share time and size | 30 days: 24.89, 23.93, 23.73 s, median 23.93 s; 106,460 bytes (/tmp/share-before.json) | |
| 6.1 failures, time, prompts against the reports | | 2026-10-04 to 11-03: failures 1,311, $214.62, 55 refused, 92 denied, as the tools line ($215); time inside the window equals `tokprof pauses`: Claude Code 2,381 of 70,464 requests a minute or more, Codex 1,165 of 15,643 (the whole sessions gave 2,640 of 107,256); prompts held: Claude Code 59,514 of 70,550 (84.4%), about 59,500 from the summary's percentages; Codex 15,643 of 15,643 |
| 6.2 active days and hours | | 2026-10-04 to 11-03: 29 active days and 380 active hours for both agents, busy 438h; Claude Code 325 hours, of them `sdk-ts` 314 (the cold-sessions study counted 312) and `cli` 41; Codex 155 hours, `Codex Desktop` 139 |
| 6.3 error texts in the file | | no key or string holds `error`, `502`, `Bad Gateway`, `Traceback`, `No such file`, `Permission denied`, `cannot`, `Script failed` or `exit code`; the new keys are the new fields and the busy kinds only |
| 6.4 pool with the file before | | each new section and the activity line say `in 1 of 2 files`; the new file's report: activity 29 active days · 380 active hours · busy 438h · $29.06 per active hour; `sdk-ts` 0.65 cold returns and $2.79 avoidable per active hour; 3.45 failed calls and $0.56 after failures per active hour; 3.88 interrupts per 100 typed messages |
| 6.5 time and size | 23.93 s, 106,460 bytes | 30 days: 25.60, 24.92, 24.09 s, median 24.92 s (+4.1%); 134,111 bytes: the tools and programs carry `failed` and `after` now |
| 7.4 the fixes on the real logs | 2026-10-04 to 11-03: `cache lifetime: main` $218 and `cache lifetime: subagents` $341; avoidable per active hour `sdk-ts` $2.78, `Codex Desktop` $2.14, `cli` $3.73; ends of turn 229.66 per 100 typed messages | the same window: `cache lifetime: main` $0, as the main conversations wrote 229M tokens with the 1-hour lifetime and 17k with the 5-minute one, and replay at $4,267 under 1h and $5,378 under 5m; `cache lifetime: subagents` $405, the $2,886 under 1h minus the $2,480 under 5m of `tokprof whatif`; avoidable per active hour from the cold returns: `sdk-ts` $2.35, `Codex Desktop` $0.22, `cli` $0.29; the clients' cold-return dollars add up to those of the pause parts: Claude Code $762.71 over 213 returns, Codex $30.98 over 38; ends of turn 6.78 per active hour and `–` per 100 typed messages, interrupts 0.11 and 3.77; 135,158 bytes |
| 8.4 the second fixes on the real logs | `~/tokprof/share-2026-11-03.json` (2026-10-04 to 11-03) with f7c1bd9: `longest request 2.0h`; the effort table listed 8 of 10 clients by their share at max, a client of $0.91 second; `codex-auto-review` (101.3M tokens, $0) named nowhere | the same file: no longest request; the effort table lists `sdk-ts` $8,999, `Codex Desktop` $1,900 and `cli` $421, then `7 more` $35.93, which add up to the $11,356 of spend; under the model table `models without a price: codex-auto-review (101.3M tokens); their tokens count as $0`, and pooled with `/tmp/share-before.json` `(202.5M tokens, 2 files)`; a new `share` over 30 days has the same keys and words to review, 135,169 bytes |

## Workflow follow-up

- Archive after `add-team-survey` with `openspec archive add-share-friction -y`, then run `openspec validate --specs --strict`.
