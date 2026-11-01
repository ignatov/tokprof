# Proposal

## Why

The `advice` block is the longest part of the X-ray pane in `tokprof ui`, and it pushes `spend` and the rest off the screen. Each item packs two or three sentences and wraps over two to four lines. On the author's 20 latest sessions the block took 276 lines for 110 items. For session 3f951de0 it took 16 lines for 6 items.

A first version wrote each item as one telegraphic line, such as `cold: resume +$6.38 (re-writes 817.6k), then $0.16/request`. It cut session 3f951de0 to 8 lines, and the 20 sessions to 149, but the author found it too terse to understand. The author chose plain sentences instead, a little shorter than today's.

## What Changes

- Every advice item says one thing, in plain sentences: what it is about, the numbers behind it, and what to do. Its fixed text keeps it within two lines of the X-ray pane. Session 3f951de0 then reads, in 14 lines in place of 16:

  ```
  advice    ▸ Resuming writes the whole 817.6k context again (+$6.38), then $0.16 per request.
            ▸ A new session that refers to this one is cheaper: ≈ $0.31 to pick up the work.
              `tokprof handoff 3f951de0` prints the line to start it with.
            ▸ /compact costs about as much as resuming (≈ $6.74 once), then ~44.2k per request.
            ▸ The context fills 82% of the window. At 12 requests per prompt, each prompt costs
              ≈ $1.99 in reads alone.
            ▸ Tool results take 45% of the context (Bash 299.9k, Read 63.9k): pipe long command
              output through tail or grep.
            ▸ Tool calls take 32% of the context (Bash 181.2k, Write 62.8k): write long inline
              scripts to a file and run the file.
            ▸ Pasted prompts (turn 1 14.6k, turn 3 29.6k) are re-read on every request: pass a
              file path instead.
            ▸ Thinking cost $16.18, 15% of the session, at effort max: use a lower effort for
              routine steps.
  ```

- How to carry on splits into up to three items: resuming, a new session, `/compact`.
- Each hint becomes one clause after the numbers, such as `pipe long command output through tail or grep` in place of "Long command output is re-read on every request: pipe it through tail or grep, use quiet flags."
- Content items name up to two leading sources instead of three.
- Pasted prompts list the three largest blocks and `and <n> more`, instead of every block.
- Items still wrap at 82 characters. An item takes more than two lines only with a very long tool or MCP name.
- **BREAKING** for anyone who matches the advice text. It is not a stable interface: no other command reads it.

The rules, their thresholds, their order and every number stay as they are. Only the wording and the split into items change. The X-ray pane of `tokprof ui` gets the change with no change of its own.

Out of scope:
- New rules, or changes to when a rule fires and what it computes.
- The `context`, `cache` and `next` lines, and the rest of the X-ray.
- The session title. Its `title` line in the X-ray header belongs to the change add-origin-task-dimensions.
- The text of `tokprof handoff`, `tokprof whatif` and the other reports.
- Color: the `▸` marker stays the only colored part of the block.

## How it is checked on the real logs

- `tokprof scan 3f951de0` prints the block above: 14 lines in place of 16. The numbers stay the same: $6.38, $0.16, $0.31, $6.74, 44.2k, 82%, 12, $1.99, 45%, 32%, $16.18, 15%. The cache age and the exact cents may move with the time of the run and today's prices.
- On the same 20 sessions as the baseline, no item takes more than two lines, and the blocks take fewer lines than the 276 of the baseline. The first estimate was at most 250 lines; the result was 264, and the author accepted it. The carry-on items take 4 lines in both versions, so the savings come from the content and thinking items, about 2 lines a session.
- The same 20 sessions give the same rules in the same order: every old item has its new item or items, and no item is added or lost. A session that is still being written may gain or lose an item between the runs; such a difference is checked by hand.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `session-xray`:
  - "Advice block": one thought per item in plain sentences, within two lines, and up to three items for how to carry on;
  - "How to carry on": the cold and warm items in the new wording;
  - "Pickup estimate": the No estimate scenario names the items that are left out;
  - "Context cost advice", "Hints on what fills the context", "Thinking cost advice", "Cache miss advice": the new wording, up to two sources and the shorter hints.

## Impact

Code:
- the wording of every rule in the advice;
- the line layout of the advice block in the X-ray, which already wraps at 82 characters;
- their unit tests;
- README, which gets a sample of the block.

There are no new dependencies and no new log reads. Parse time does not change.

The change detect-idle-compaction, planned in another session, changes which prompt the advice rules see after an idle compaction. It leaves the advice requirements and their wording to this change.
