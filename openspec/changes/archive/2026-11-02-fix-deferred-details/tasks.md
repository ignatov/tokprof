# Tasks

## 1. Codex subagent threads belong to their root session

- [x] 1.1 In the thread index, find a thread's root by following parents, with a 64-step cap, and list all threads below a thread, breadth first. The session of a rollout is its root's rollout. Verify: a unit test of an index with a thread two levels deep, a missing parent, and a cycle.
- [x] 1.2 Give a subagent rollout its root's id and its root's title when it is parsed. Verify: the existing Codex subagent test, extended to check the id.
- [x] 1.3 Check that `--session`, the X-ray's subagent logs and `tokprof ui`'s live updates use the new session of a rollout and the full list of threads below it. Verify: `tokprof scan 01a03fe1-681d-78e2` counts more than 39 subagents.

## 2. X-ray

- [x] 2.1 Leave out the share on the `next` line, including after a trailing compaction, when the session cost is under $0.005. Verify: a unit test with a session that cost $0.004.
- [x] 2.2 Without timed requests, print `time` with the counts only, and no `models` line without models. Verify: a unit test of the header of a session without requests.
- [x] 2.3 Price the pickup for its own largest prompt, the start plus the taken-in tokens, through a price lookup by prompt size. Verify: a unit test with a 300k prompt on `gpt-6-astra` whose pickup costs the same as with a 100k prompt.
- [x] 2.4 Count requests per prompt over the conversation's life: the requests whose prompt the log shows, over the turns each one adds to the previous one, none for the first request after a compaction. Verify: a unit test with a compaction that re-sends earlier prompts, as in the spec's scenario.

## 3. Checks

- [x] 3.1 Run `cargo test` and `cargo clippy --all-targets`.
- [x] 3.2 X-ray all sessions with the old and the new binary and compare them as the proposal says. Run `tokprof misses --agent codex --top 100000` and `tokprof summary --agent codex`. Record the numbers in this file.
- [x] 3.3 Update README.md where it describes these lines, if it does.
- [x] 3.4 Reinstall the binary with `cargo install --path . --root ~/.local`.

## Results on the real logs (2026-11-02)

Every session X-rayed with the old and the new binary, 759 sessions. Once the drift of the clock and of live sessions is taken out, 88 X-rays differ, all for the reasons below:
- `of the session's $0.00`: 125 X-rays before, 0 after.
- `time 1970-01-01`: 35 before, 0 after. These Codex rollouts without requests now show `time      0 requests` and no `models` line. A Claude Code transcript without requests, but with times, loses its empty `models` line.
- Nested Codex threads, 7 sessions: 01a03fe1-681d-78e2 has 48 subagents and $534 (39 and $458 before), 019ffb1b-8c5f-7752 24 and $388 (16 and $347), 019fff6b-229a-7671 15 and $269 (12 and $261).
- Review threads without a task now count under their session's task, so the `by task` rows `untitled <thread id>` merge into `untitled <session id>` in 6 sessions; their spend stays under `agent:review` in `by origin`.
- Requests per prompt changed in 36 sessions. Against a count of the prompts typed in the rollout: 01a0b3f1-109b-7132 gives 224 (165 before), with 25,297 requests over 115 typed prompts, 220 each; 01a0eec6-eed9-7a71 gives 148 (182 before), with 13,620 main requests over 98 typed prompts, 139 each. 01a01c11-a456-75e2 gives 787 (394 before): a goal run whose one prompt drove all 787 requests.
- Pickup estimates: unchanged in all 267 X-rays that give one.

`tokprof misses --agent codex --top 100000`: 1,349 misses, 179 of them under a subagent thread id before, 0 after.
`tokprof summary`: Codex $10,485 priced, $1,387 avoidable, and Claude Code $10,463 and $1,116, the same before and after. Codex reads in 17.6 s (17.5 s before), Claude Code in 6.3 s (6.4 s).

## Workflow follow-up

- Archive the change with `openspec archive fix-deferred-details -y`, then run `openspec validate --all --strict`.
