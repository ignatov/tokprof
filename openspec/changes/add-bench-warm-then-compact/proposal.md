# Proposal

## Why

The cold-session study of 2026-11-03 looked for the idle policy an IDE should run for its agent sessions. The best one was to keep the cache warm with pings for 2 hours, then compact before the cache expires. `tokprof bench` cannot show this, for two reasons:

- **No such way.** Its four ways are resume as is, /compact first, a handoff note, and heartbeats then resume.
- **Pings are counted only on returns.** It counts the pings of a heartbeat policy only on the breaks that ended in a cold return. An IDE does not know which breaks will. It pings after every big session's last request too, and on breaks where the user came back while the cache was still warm.

The study's replay counted only the first request after each break (Opus 5.5, IDE sessions, 30 days). It gave:

| policy | pings and compactions | of it on breaks with no cold return | saved | net |
|---|---:|---:|---:|---:|
| pings up to 2 h | $62 | $23 on breaks without a return | $330 | +$269 |
| compaction 54 minutes into a break | $89 | $30 | $655 | +$566 |
| pings up to 2 h, then compaction | $120 | $53 | $695 | +$574 |

For the same 2 hours of pings, `tokprof bench --keep-warm 2h` reports $35 instead of $62.

## What Changes

- **A fifth way: `heartbeats, then compaction`.**
  - The pings are the same as heartbeats, up to `--keep-warm`. At the next ping time after that, a compaction goes out instead of a ping, while the cache is still warm.
  - A return before the compaction is priced as a warm return, as heartbeats price it.
  - A return after it runs on the base that /compact first and the handoff note use. The way pays for its pings and the warm compaction before the first request.
  - With `--keep-warm 0` it becomes "compact at the first ping time": 50 minutes on a 1-hour cache, close to the terminal's idle compaction.
- **Pings on breaks that were not cold returns.**
  - For heartbeats and the fifth way, the bench also prices what they spend on such breaks in big main conversations. That covers breaks after which the next request was warm or not a return, and the time from a conversation's last request to the end of the logs read.
  - The savings lines add these costs and print the net.
- **The table, the savings lines, the notes and the chart** get the fifth way. The chart draws it dash-dotted in the next palette color.
- `docs/bench.txt`, `docs/bench.svg` and the README's numbers are regenerated.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `return-bench`: the fifth way, pings on breaks that were not cold returns, and the table, savings lines, notes and chart that list the ways.

## How it is checked on the real logs

- **The fifth way, 30 days of Claude Code.** `tokprof bench --agent claude --since 30d --keep-warm 2h`:
  - It keeps about 93 returns warm, as heartbeats do.
  - It compacts the other returns. Their requests then cost about what the handoff note's do for the same returns.
  - Its first-request saving over all returns comes close to the study's $695.
- **Pings on breaks that were not cold returns.**
  - With `--keep-warm 2h`, the pings come to about $62 in all, against the $35 counted on returns now.
  - With the default 6h, about $168 in all, against $84 now.
  - The net saving of heartbeats falls accordingly.
- **The unchanged ways.** Resume, /compact first and the handoff note give the same numbers as before the change on the same logs.
- **Speed.** The bench takes no more than 10% longer.

## Out of scope

- Compaction after a fixed delay other than the next ping time. `--keep-warm` already moves it.
- A ping policy that depends on the time of day or on IDE activity. The logs do not tell when the IDE was open.
- Codex: the fifth way applies to it as heartbeats do, with the same assumption that a ping resets OpenAI's cache timer.

## Impact

- Core: the bench's scan also collects the breaks that were not cold returns, and its replay gets the fifth way.
- CLI: the report and the chart render a fifth row and series.
- Docs: `docs/bench.*` and the README numbers.
