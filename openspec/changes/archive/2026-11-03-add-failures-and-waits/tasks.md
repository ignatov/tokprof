# Tasks

## 1. Model and the Claude Code source

- [x] 1.1 Add the neutral fields of design decision 1: a request's first send, answered send, end, failed attempts and last error; a tool result's outcome; the session's error lines by block; what a wake reports. Verify that `cargo test --workspace` passes and that `tokprof --since 30d summary` prints the same totals as before.
- [x] 1.2 Claude Code request times: the `api-request` send, the latest `user` line of the chain without audit lines (attachments skipped), and the latest line of the response as its end, never before the send. Verify with decoder tests for both log forms and for an end stamped before its send.
- [x] 1.3 Claude Code retries: pair each `api_error` line with the latest `api-request` of its `agentId`, carry the first send, the count and `error.formatted` to the answered attempt, apply `retryInMs` without audit lines, and keep the retries of a request that got no answer. Let `"api_error"` through the line filter. Verify with decoder tests built from the 07:04:05 example of the spec.
- [x] 1.4 Claude Code call outcomes and error lines: `is_error`, the refusal texts, `permissionDecision.decision` `reject` and the denial texts, `returnCodeInterpretation` without `is_error`, and the `Exit code N` rule, with the texts in the source's tools module. Verify with unit tests for each scenario of session-logs, Call outcomes and Error line of a failed call.
- [x] 1.5 Claude Code wakes: classify `<task-notification>` by its `<summary>` (`Agent "`, `Background command "`, `Monitor `) and everything else, `<cross-session-message` included. Verify with unit tests for each scenario of What a wake reports.
- [x] 1.6 Add to `sources/claude-code/tests/fixtures`: a retried request, a failed, a refused and a denied tool result, and an agent and a command notification. Name the fields and versions in `sources/claude-code/README.md`. Verify that the conformance test passes.

## 2. The Codex source

- [x] 2.1 Keep the time of each history item; give each request the end of the last item its response added and the send of the item before them. Verify with a decoder test where `token_count` comes after a tool output and the request still ends at its last item.
- [x] 2.2 Codex outcomes and error lines: `Script failed` with the line after `Script error:`, text items whose JSON `exit_code` is not 0 with `exit code <n>: ` and the output's last line, and `aborted by user` as a refusal. Verify with unit tests built from the spec's scenarios.
- [x] 2.3 Add a failed script, a non-zero exit code and a refusal to `sources/codex/tests/fixtures`, and name the fields and versions in `sources/codex/README.md`. Verify that the conformance test passes.

## 3. Busy time

- [x] 3.1 A core module for agent-time that builds the busy set from the turns of work-rhythm without question waits, the subagents' work after an end of turn, and the stretch to a wake that reports a background command. Verify with unit tests for each scenario of agent-time, Busy time.
- [x] 3.2 Paint the busy set in the order of the spec: model, API retries, subagents, the next request's activity, compaction, background commands, nothing. Keep the part after ends of turn. Verify with unit tests for the scenarios of What the busy time went to, What the agent waited on between requests and After the end of a turn.
- [x] 3.3 Waits of a minute or more with the three longest, and slow requests of every chain without advisor calls and without retry time. Verify with unit tests for the scenarios of Waits and Slow requests.

## 4. Failures in the tool table

- [x] 4.1 Count `failed` per row and the refused and denied calls of the table from each first-time tool result's outcome. Verify with unit tests for Failed, refused and denied calls.
- [x] 4.2 The failure windows of design decision 6: per chain and row, held-back spend, closed in full by a working call and with the first request only at an end of turn or the chain's end, each request counted once in the total. Verify with unit tests for each scenario of What came after a failure.
- [x] 4.3 `tokprof tools`: the `failed` and `after` columns, empty cells for rows without failures, `--by failed` with the usage error that lists `rent, trips, failed`, and the failures line. The X-ray's `tools` block keeps its columns. Verify with render tests, and describe the columns, the sort and the line in README.md.

## 5. X-ray blocks

- [x] 5.1 The `waits` block after `work`: the title with the busy time, one line per kind with bar, share, time and note, `<n> more` from the seventh, the slow line and the longest-waits line, with the rhythm's durations. Verify with render tests for each scenario of Waits block, Slow requests line and Longest waits line.
- [x] 5.2 The `failures` block after `tools`: up to 5 rows by `after` then `failed`, error lines cut to 60 characters, `<n> more`, the failures line, and the refusals-only case. Verify with render tests for each scenario of Failures block.
- [x] 5.3 The new layout order, with both blocks kept when there is no main conversation. Verify with a render test of a subagent transcript as the target. Describe both blocks in README.md.

## 6. The pauses part

- [x] 6.1 In the pauses scan, compute busy time per main session with its subagents, the sessions that work-rhythm leaves out included, and add it up per agent. Print the part with `kind`, `time`, `share` and `after turns`, the slow line, and `No busy time.` when there is none. Verify with render tests for the scenarios of Busy time part and Pause report, and describe the part in README.md.

## 7. Checks on the real logs

Record each number next to its task when it is done.

- [x] 7.1 Run `tokprof --agent claude --since 30d tools`. Expect 1,021 failed and 153 refused or denied calls, give or take the calls since 2026-11-03. Re-run /tmp/tokprof-why-study/scripts with the windows of the spec (same tool, within a turn, first request only when never worked again) and expect the failures line's spend within 2% of it.
  - Recorded 2026-11-03. The study picked sessions by the mtime of their logs, with all their requests, and left out `claude -p` and temporary-folder sessions; `--since 30d` keeps the requests since 2026-10-04 only, 910 failed calls. Over all history, tokprof counts 1,038 failed calls (+1.7%), 54 refused and 103 denied (157 against 153).
  - The study re-run with the spec's windows (`cc_spec_check.py`): 1,021 failed, windows of 1,302 requests and $142.50. tokprof over all history: $146, +2.5% in all and +0.8% per failed call; the rest is the sessions the study left out.
  - The first draft's rule (same row) had given $326; the spec now ends a window at the same tool, or any shell call after a shell call.
- [x] 7.2 Run `tokprof --agent claude --since 30d pauses`. Expect 3.3% of requests slow, 45 requests and 10.8h of API retries, about 43h of subagents and 41h of background commands after turns, each within 5% or half a point. Where the spec's rules differ from the study's, compare with the study script re-run under the spec's rules.
  - Recorded 2026-11-03 over 2026-10-04..11-03: 2,979 of 82,757 requests slow (3.6%, +0.3 points); background commands 42.8h after turns (+3.4%).
  - API retries are 23.5h: the spec also counts the requests that no attempt answered, as in the outages of 10-26 and 10-27; the study's 10.8h counted answered requests only.
  - Subagents are 67.1h, 53.7h of them after turns, against the study's 42.8h. The study counted only the stand-stills that a wake ended, and gave the user's time to subagent work that a typed message ended. In 3edfd2e2, 35 review subagents worked from 16:46 to 17:28, until the user typed a status question. Not re-run under the spec's rules, since the study script has no turns of work-rhythm.
  - Busy time in all is 361h; `nothing` is 1.1h.
- [x] 7.3 Run `tokprof scan 3edfd2e2`. Expect 17 slow main requests, 25m for subagents and 27m for background commands in prompt 2's span, and the 1h 50m background command of 2026-10-14 among the longest waits.
  - Recorded 2026-11-03: 17 slow main requests (133 with subagents), as CodeBurn listed.
  - Prompt 2 (20:23–21:49 local) has 25m and 2.6m for subagents and 15m, 8.5m and 1.2m for background commands. The 2.6m wait had a subagent and a command running, and subagents come first, so the split is about 28m and 24m.
  - The longest waits: 1.8h for background commands at 10-14 10:12, 42m for subagents at 10-13 16:47, 25m for subagents at 10-12 20:59.
  - CodeBurn's "nothing running" 13m of prompt 6 is 13m for background commands at 10-12 23:39 here. Its 4 "waiting on helpers" of prompt 5 are 14m for a subagent and 11m, 10m and 5.4m for background commands.
- [x] 7.4 Run `tokprof --agent codex --since 30d tools`. Expect at least 43 failed scripts and 992 calls with a non-zero exit code counted as failed.
  - Recorded 2026-11-03 over all history: 1,532 failed calls, after them $314; 14 refused.
  - The rows are `write_stdin` 660, `exec` 300, `apply_patch` 89, `exec_command python -` 74 and `exec_command bazel.cmd test` 46.
  - `write_stdin` is now a shell call. Before that, its windows ran to the next `write_stdin` and took $707 of $881.
- [x] 7.5 Time `tokprof --since 30d summary`, `tokprof --since 30d pauses` and `tokprof scan 3edfd2e2` before and after the change. Expect at most +3%, +5% and +0.1 s.
  - Recorded 2026-11-03, best of two, over 2026-10-04..11-03: summary 24.28s → 24.46s (+0.7%), pauses 24.03s → 24.28s (+1.0%), scan 3edfd2e2 0.31s → 0.30s.
- [x] 7.6 Run `cargo test --workspace` and `openspec validate --all --strict`, then reinstall with `cargo install --path crates/tokprof --root ~/.local`.
  - All tests pass; 15 items valid; a simulated archive validates 15 specs. The binary in ~/.local/bin is from this change, and it replaced the one another session had installed.

## Workflow follow-up

- Before archiving, copy `openspec/` to /tmp, archive there and run `openspec validate --specs --strict`.
- Archive with `openspec archive add-failures-and-waits -y` when the author asks; commit only when the author asks.
