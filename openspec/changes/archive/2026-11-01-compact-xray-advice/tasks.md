# Tasks

## 1. Baseline

- [x] 1.1 Before any code change, save the advice blocks of the 20 latest sessions from the current binary: `tokprof scan <id>` with `NO_COLOR=1` for each id in `tokprof sessions`, into a scratch file outside the repository. Verify: the file holds 20 blocks, about 275 lines and 110 items; record the exact counts in this task.
  - Recorded 2026-11-01 22:57 from the binary installed at 22:32, in /tmp/tokprof-advice/before.txt, with the ids in ids.txt: 20 blocks, 276 lines, 110 items.

## 2. Plain sentences, one thought per item

- [x] 2.1 Move the 82-character advice width into one constant used by the X-ray wrap, counted in characters. Verify: `cargo test` passes and `tokprof scan 3f951de0` prints the same block as before this task.
- [x] 2.2 How to carry on: up to three items in the spec's sentences, with the no-price, no-pickup and not-cheaper variants. The code already splits the items, from the one-line version; only the wording changes. Verify: the advice unit tests assert the cold, warm, not-cheaper and no-price items of the spec's scenarios, including "Resuming writes the whole 950.0k context again (+$7.40), then $0.19 per request." for the test's inputs.
- [x] 2.3 Context cost: the spec's sentences, "Auto-compaction is close." at 85% or more, and the no-turns and unknown-window variants. Verify: the unit test asserts "The context fills 95% of the window. At 30 requests per prompt, each prompt costs ≈ $5.70 in reads alone. Auto-compaction is close."
- [x] 2.4 Content hints: tool results, tool calls and reminders with up to two sources, always, and the spec's hints. Remove the rule that drops the second source on a long line. Verify: unit tests for two leading sources and one hint per table entry.
- [x] 2.5 Pasted prompts: the three largest blocks in turn order, then `and <n> more`, in the spec's sentence. Verify: a unit test with five blocks asserts "Pasted prompts (turn 1 14.6k, turn 3 29.6k, turn 12 110.2k and 2 more) are re-read on every request: pass a file path instead."
- [x] 2.6 Thinking and cache misses: the spec's sentences and miss hints by cause. Verify: unit tests assert "Thinking cost $20.00, 20% of the session, at effort max: use a lower effort for routine steps.", the no-effort variant, and "Cache misses could have saved $12.00, mostly idle > TTL (3×): compact or wrap up a big session before a long break."
- [x] 2.7 A unit test renders every item of the tests through the X-ray's wrap. Verify: none takes more than two lines.
- [x] 2.8 Search the repository for the phrases of both earlier versions: today's ("Resuming re-writes", "Cheapest: a new", "in context reads alone", "auto-compaction is close", "A lower effort", "Cache misses: $") and the one-line version's ("cold: resume", "/request", "of reads each", "tail/grep", "auto-compaction soon", "pasted:", "misses $"). Verify: none is left outside openspec/specs/, which the archive updates, and the artifacts that quote them on purpose.
  - Code and tests have none. The test `advice_after_a_compaction_sees_the_next_prompt` of detect-idle-compaction in src/xray.rs matched the one-line prefixes; it now matches "Resuming writes", "Cache warm", "Tool results", "Thinking cost" and "Cache misses". README is left for 2.9. The proposal of detect-idle-compaction quotes the text from before both versions, as history.
- [x] 2.9 README: replace the one-line sample of the advice block with the 14-line block of session 3f951de0, and reword the list of what each item means to match. Keep the reasons the hints leave out. Verify: the block in README matches `tokprof scan 3f951de0` apart from the cache age and today's prices.
  - The block matches the release build line for line. The list under it names the items in words instead of the one-line labels.

## 3. Check on the real logs

- [x] 3.1 Run `cargo test`, then `cargo install --path . --root ~/.local`. Verify: both succeed; record the time and the test count here.
  - 2026-11-01 23:25: 62 passed, 1 ignored; installed. The tree also holds other sessions' work, so the binary has it too.
- [x] 3.2 Run `tokprof scan 3f951de0` and compare with the baseline from 1.1. Verify: 14 lines in place of 16, with the same $6.38, $0.16, $0.31, $6.74, 44.2k, 82%, 12, $1.99, 45%, 32%, $16.18 and 15%, unless the cache age or prices moved between the runs.
  - 14 lines, every number the same; the block is the one in the spec and in README.
- [x] 3.3 Run the 20 sessions of 1.1 again and compare with the baseline. Verify: fewer lines than the baseline (first estimate: at most 250), no item over two lines, and every old item maps to its new item or items. A session still being written may differ by an item; check such a difference by hand. Record the counts here.
  - The one-line version, for comparison: 149 lines and 149 items, 0 wrapped; 19 sessions with the same rules, and ddce1894, the session that ran the check, with one more `tool calls` item because it grew between the runs.
  - Sentences, 23:25: 264 lines and 151 items, against 276 and 110; no item over two lines. The 250 estimate is missed; the author accepted 264 on 2026-11-01. The carry-on rule takes 4 lines either way (1 + 2 + 1 cold, 1 + 1 + 2 warm), so the savings come only from the content and thinking items, about 2 lines a session.
  - 18 sessions give the same rules in the same order. ddce1894 and f6bc32a3 gained a `tool calls` item and ddce1894 a context-cost item: both were written after the baseline (f6bc32a3 at 23:17), and their context grew.
- [x] 3.4 Check the X-ray pane of `tokprof ui` with `tokprof ui --dump 170x40`. Verify: the advice block of the selected session shows each item within two lines, with no cut-off text.
  - Every item within two lines, nothing cut off. The wrap had split `≈` from its amount (`costs ≈` / `$0.22`); it now keeps them together, with a unit test. Over the 20 sessions no line ends with `≈`.

## Workflow follow-up

- Archive the change with `/opsx:archive` after the author approves the result, then run `openspec validate --all --strict`.
