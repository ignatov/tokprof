# Tasks

## 1. The ask rule and the flag in the sources

- [ ] 1.1 Put the rule of turn-outcomes, A response that asks, with Words that ask and Words that offer, in tokprof-logio as one function from a paragraph to yes or no. Port it from `~/tokprof/plans/turn-ends/clf.py`. Unit tests use the scenarios and synthetic phrases only, no text from real logs:
  - one per word list;
  - a link with `?`;
  - a trailing code fence;
  - a report.

  Verify: `cargo test -p tokprof-logio`.
- [ ] 1.2 Add a neutral flag "asks" on a response to the model. Set it in `sources/claude-code` from the last paragraph of the text blocks of the response that ended a turn, and in `sources/codex` from the assistant `output_text` of the last message of a turn. Add a fixture for an ask, an offer and a report to each source. Verify: `cargo test -p tokprof-source-claude-code -p tokprof-source-codex`, conformance included. Write the fields and versions in both READMEs.
- [ ] 1.3 Re-run the holdout check with the Rust rule. Use `ends.py` and the labels in `~/tokprof/plans/turn-ends/`, with the author's logs on the author's machine. It must give the same 0.87 / 0.80 as the Python rule. Record it in the apply notes. Check that `crates/tokprof/tests/architecture.rs` still passes.

## 2. Turn outcomes in the core

- [ ] 2.1 Join the requests of each session, main and subagent, to the rhythm's turns by time, in one pass. Give each turn its dollars, calls, edit calls (class `editing`), commits (as Outcome parts count them) and the class of its last call. Unit tests next to the code:
  - a subagent inside a turn;
  - a subagent request between turns;
  - the sum of the turns plus the requests between them equals the sessions' spend.
- [ ] 2.2 End kinds in the order `stopped`, `failed`, `asked`, `committed`, `changed`, `answered`. Unit tests cover:
  - the four scenarios of turn-outcomes, How a turn ends;
  - a turn whose last call is `AskUserQuestion`;
  - a failed last call.
- [ ] 2.3 What came after a turn: the five ranges and `none`, session ends, and open sessions (one hour before the end of the selected time). Unit tests: the scenarios of What came after a turn and Open sessions.
- [ ] 2.4 Stopped turns (`no call` when none) and run length with goal runs from the origin `command:/goal`. Unit tests: the scenarios of Stopped turns and Run length.

## 3. Views

- [ ] 3.1 X-ray `ends` line under `rhythm`, ties in the order of the spec. Verify: a render test with the session-xray scenario, and `tokprof` on the latest session prints it.
- [ ] 3.2 `tokprof pauses`: the section `How your turns end`, with `Stopped turns` and `Run length`. Under the first table, a line with the turns that had no logged end. Verify: render tests for the three pause-report scenarios.
- [ ] 3.3 `tokprof share`: the `turns` part, with its keys among the own words and `usd` turned into shares. Verify:
  - the share-export scenarios as tests;
  - the leak check passes on a fixture;
  - `crates/tokprof/tests/share.rs` covers a file with and without `--dollars`.
- [ ] 3.4 `tokprof pool`: the sections `How turns end`, `Stopped turns` and `Run length`, the new order and notes, `interrupts` with `spent` money in `Pains side by side`, and `quality` with `interrupts`. Verify:
  - render tests for each modified scenario;
  - a pool with one file from v0.1.4 prints `in 1 of 2 files`.

## 4. Check on the real logs

- [ ] 4.1 Run `tokprof --since 30d pauses` on the author's logs and record in the apply notes:
  - turns per end kind;
  - stopped turns against the rhythm's interrupts (52 at proposal time);
  - cost per edit call per length range.

  Compare with the prototype (52% answered, 30% changed, 9% committed, 4.5% asked, 1.9% stopped; 18/37/62/75/50 edit calls per 1% of spend). Explain any kind that differs by more than 5 points.
- [ ] 4.2 Check that the dollars of all turns plus the subagent requests between turns equal the spend of the kept sessions within $0.01, and that Claude Code totals stay within 1.5% of cost-state. Record both.
- [ ] 4.3 Sample 40 fresh ends of the author's Codex sessions and 20 `changed` turns, and check them by hand. Record precision and recall of `asked`, and how many `changed` turns are wrong.
- [ ] 4.4 Time the full parse before and after (`time tokprof --since 365d pauses`) and record both. The change must cost under 2%.
- [ ] 4.5 Run `tokprof pool` over `~/tokprof/survey/pilot-2026-11/` and the author's new share file. Check that the new sections say `in 1 of 8 files` and that `interrupts` has money only from that file.

## 5. Docs and checks

- [ ] 5.1 Update README.md: the `ends` line, the section in `pauses`, and the new pool sections. Update docs/survey.md: what `turns` holds, and that it holds no text.
- [ ] 5.2 Run `cargo fmt --all`, `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, `openspec validate --all --strict`, and `cargo install --path crates/tokprof --root ~/.local`.

## Workflow follow-up

- Simulate the archive in a copy of `openspec/` and validate with `--strict` before `/opsx:archive`: the MODIFIED `Section notes` and `The pains of each answer` sit near the 500-character limit.
- Release a version and switch the pilot prompt to it before wave 1.
- Separate task: ask colleagues to run the ask rule on their own logs, label about 50 of their own turn ends, and post only the counts (precision, recall, language, agent) to a GitHub issue. Texts stay on their machines.
