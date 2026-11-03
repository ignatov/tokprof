# Tasks

Every group ends with `cargo test --workspace` green, in a state that can be committed.

## 1. Before starting

- [x] 1.1 Record the baseline in the results table at the end of this file: the median of 3 runs of `tokprof --since 30d pauses`, and the study's numbers from the proposal. Verify: the table has the numbers and the date.

## 2. Hand-offs in the model and the sources

- [x] 2.1 Add the hand-offs and the flag for a session a program ran to the session model. Verify: `cargo test -p tokprof-core` passes and the architecture test still passes.
- [x] 2.2 Claude Code: emit ends of turn (one per response, at its latest line), interrupts, wakes, messages typed while working, questions and answers in main conversations only; mark `sdk-cli` sessions. Verify: unit tests for each scenario of "Hand-offs between the user and the agent" and "Sessions a program ran", a subagent transcript with none, and the conformance fixtures updated with one line of each kind.
- [x] 2.3 Codex: emit ends of turn and interrupts in main threads; mark `codex_exec` rollouts. Verify: unit tests for the "Codex" and "Codex exec" scenarios, and a subagent thread with none.
- [x] 2.4 Name the new fields and versions in both sources' READMEs. Verify: each README lists the fields of decisions 2 and 3 of the design.

## 3. Rhythm rules

- [x] 3.1 Build a timeline per session and the turns, work and waits from it. Verify: unit tests for every scenario of "Turns", "Work" and "Answers".
- [x] 3.2 Sittings, breaks and what the agents did after the user left. Verify: unit tests for "Two sittings", "The edge of the gap" and "Agents that ran out of work".
- [x] 3.3 The start of the day from the actions of the 30 days before the end of what is shown. Verify: unit tests for "The author's night", "A night that is not empty", "Too few days" and "The night line", with a fixed local offset.
- [x] 3.4 Leave out sessions a program ran and sessions in a temporary folder, with the `left out` line. Verify: unit tests for "Commit-message sessions" and "Print mode".
- [x] 3.5 The five rhythm lines and the duration format. Verify: unit tests for every scenario of "Rhythm lines".

## 4. Views

- [x] 4.1 Add `tokprof day [DATE]`: the read of the 31 or 30 days of main logs, the day and night lines, the strip, the rhythm lines and the cache line; fail on `--since` and `--until`. Verify: unit tests for every scenario of "Day view", "Day strip" and "Cache line of the day"; `tokprof day --help` says what the view shows.
- [x] 4.2 Add the rhythm section to `tokprof pauses`, with the hour table. Verify: unit tests for the scenarios of "Rhythm section".
- [x] 4.3 Add the `rhythm` line to the X-ray header. Verify: unit tests for the "Rhythm" and "No turns" scenarios of "Rhythm line".
- [x] 4.4 Update README: the day view, the rhythm section, the rhythm line, and how the start of the day is found. Verify: the README examples run as written.

## 5. Check on the real logs

- [x] 5.1 Run `tokprof --agent claude --since 30d pauses`. Verify: the rhythm section matches the study's numbers within the proposal's tolerances, and the night line says 02:00–08:00 and 05:00; record the output.
- [x] 5.2 Run `tokprof --agent claude day 2026-11-02`. Verify: sittings, time at the keyboard, messages, sessions, median break, the rate of ends while present and the self-started share match the proposal's numbers for that day; record the output.
- [x] 5.3 Time `tokprof day` and `tokprof --since 30d pauses` 3 times each, before and after. Verify: `day` is not slower than `pauses`, and `pauses` is at most 5% slower than the baseline.
- [x] 5.4 Reinstall the binary with `cargo install --path crates/tokprof --root ~/.local`. Verify: `tokprof day` and `tokprof scan` run.

## Workflow follow-up

- Archive after `add-client-dimension` and `add-pause-report`, which were archived on 2026-11-03: the session-logs delta restates the archived versions of "Commands the filters apply to" and "Line about the logs read".

## Results

Measured on 2026-11-03 between 12:40 and 13:05 on the author's logs. "Re-run" is the study's script with the specs' rules (see the proposal).

| check | study / baseline | re-run | after |
|---|---|---|---|
| `--agent claude --since 30d pauses`: night | 02:00–08:00, day at 05:00 | | 02:00–08:00, day at 05:00 |
| sittings | 306, median 5m, break 36m | 310, 6.5m, 34m | 312, 6.6m, 34m; 3.2d at the keyboard, 1330 messages to 187 sessions |
| left | 8m, 28% | 7m, 23% | 6.5m, 22% |
| present | every 4m, 37% self-started | every 4m, 35% | every 4.2m, 32% |
| answers | turn 3.5m, answer 3.7m | answer 2.9m | turn 3.2m, answer 2.9m |
| stops | 39 interrupts, 46 typed while working, 74 questions in 2.3m | 35 typed while working | 40, 35, 74 in 2.4m |
| left out | 92 in temporary folders, 33 print mode | | 127 |
| `--agent claude day 2026-11-02` | 16 sittings, 3.7h, 58 messages to 11 sessions, break 28m, every 7m, 1 of 56 self-started | 16, 3.9h, every 6.3m, 2 of 62 | 16, 3.9h, 58 to 11, 28m, every 6.3m, 3%; 5 cold returns, $15.48 |
| `--since 30d pauses`, both agents, 3 runs | 24.04s, 23.82s, 23.68s (median 23.82s) | | 23.83s, 23.97s, 23.82s (median 23.83s, +0.0%) |
| `tokprof day`, both agents, 3 runs | | | 14.39s, 14.20s, 14.21s (median 14.21s); `--agent claude` alone 4.2s |
| `--since 30d summary`, old binary, 3 runs | 23.95s, 23.86s, 23.83s | | |
