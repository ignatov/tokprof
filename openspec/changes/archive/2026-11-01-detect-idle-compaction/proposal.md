# Proposal

## Why

Claude Code 2.1.299 and later compact a long idle terminal session about 54 minutes after its last request, while the one-hour cache is still warm. tokprof sees these compactions, but it counts them as ordinary auto-compactions, and its X-ray ignores a compaction that came after the last request.

So the X-ray of a session that went idle and was compacted still forecasts a cold re-write of the whole old context. That is the moment the user looks at it before coming back. The author's logs hold 6 such compactions:

| Idle compaction | Before → after the compaction | X-ray forecast today | Logged next request |
|---|---|---:|---:|
| monorepo 9428b80a, 2026-10-23 21:47 | 219.8k → 7.2k | $1.85 | $0.25 |
| research da18a2d8, 2026-10-28 00:01 | 784.7k → 10.7k | $6.60 | $0.27 |
| da18a2d8, 2026-10-28 18:14 | 376.2k → 10.8k | $3.18 | $0.35 |
| da18a2d8, 2026-10-29 03:43 | 529.1k → 15.0k | $4.46 | $0.48 |
| da18a2d8, 2026-10-29 12:36 | 399.9k → 11.6k | $3.24 | $0.37 |
| da18a2d8, 2026-10-29 19:13 | 440.1k → 8.7k | $3.73 | $0.22 |

Today's forecast is 7 to 17 times the logged cost. The advice on top of it says "Resuming re-writes 440.1k tokens first (+$3.43)" and offers `/compact` for a session that is already compacted.

## What Changes

- **Compaction kind.** Each Claude Code compaction gets a kind: `idle` when Claude Code logged that it compacted while idle, else the trigger it logged (`auto`, `manual`). Both kinds come from the logs; nothing is inferred from timing.
- **Miss detail.** In `tokprof misses` and the X-ray's list of misses, a rebuild after a compaction shows the kind as its detail: `compaction: while idle`, `compaction: auto`, `compaction: manual`. Misses by cause still group under `compaction`.
- **The X-ray after a compaction.** When the main conversation's latest compaction comes after its latest request:
  - the `cache` line says when it was compacted, and whether it was while idle;
  - the `context` line and the `next` forecast use the prompt the next request will send, estimated from the session's own earlier compactions;
  - the advice rules see that smaller prompt, so they no longer offer to resume a full re-write or to run `/compact`.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `session-logs`: compaction markers gain a kind, from the boundary's trigger or the idle-compaction notice.
- `cache-misses`: the `compaction` cause gets the marker's kind as its detail.
- `session-xray`: a new requirement covers a conversation whose latest compaction follows its latest request: the cache line, the context line, the forecast and the prompt the advice rules see. The wording of the advice items does not change. That wording belongs to the open change `compact-xray-advice`.

## Impact

- **Code:** the Claude Code adapter (reading the notice and the trigger), one field on the request record, the miss cause detail, and the X-ray's latest prompt, cache line and forecast.
- **Views:** `tokprof`, `tokprof scan` and the X-ray in `tokprof ui`; the miss list in `tokprof misses` and in the X-ray. Totals and cost attribution do not change.
- **How it is checked on the real logs:**
  - `tokprof misses --expected` lists the rebuilds after the 6 compactions above as `compaction: while idle`. Every other compaction in the logs is `compaction: auto`: 25 of the 31 Claude Code compactions.
  - The X-ray of each of the 6 sessions, cut right after its idle compaction, forecasts the next request within 2× of the logged cost in the table for at least 5 of the 6, and within 2.2× for all, against 7 to 17× today. The cache line names the compaction.
  - `tokprof summary` totals and the 1.5% match with Claude Code's own cost counter stay as they are.
- **Out of scope:**
  - Predicting whether Claude Code will compact a live session while idle. That depends on the entry point, settings and version, and is not logged.
  - `tokprof bench`. Its cold returns already stop at a compaction.
  - Codex, which has no idle compaction.
  - The summary that VS Code and the desktop app keep for use on return (Claude Code 2.1.314). It is not in the author's logs.
  - The wording of the advice items.
