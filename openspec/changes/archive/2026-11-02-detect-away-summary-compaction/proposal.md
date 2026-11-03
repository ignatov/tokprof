# Proposal

## Why

Claude Code has a second kind of idle compaction, for the sessions it runs without its terminal UI, such as VS Code and the desktop app (since 2.1.314). In 2.1.316 it writes the summary during the break and switches to it at the start of the first request after the return. It then logs the notice "Compacted using a summary written while you were away". tokprof knows only the terminal's notice, "Compacted while idle". So it labels these compactions `auto`, as if the context window had filled up.

None of the author's logs has the new notice yet. It matters now because of the tickets filed on 2026-11-02, claude-agent-acp#1518 and IJAI-5688. They ask to turn on idle compaction for IDE sessions run through the Agent SDK. Those sessions have no terminal UI, so they would get this mode. The change detect-idle-compaction left it out of scope.

## What Changes

- A Claude Code compaction marker is `idle` when either notice follows it before the next answered request:
  - "Compacted while idle" (seen in 2.1.302–2.1.310);
  - "Compacted using a summary written while you were away" (found in the 2.1.316 binary, not yet seen in a log).
- The outcome shows where the kind `idle` already shows:
  - in `tokprof misses --expected` and in the X-ray's list of misses, the rebuild after such a compaction reads `compaction: while idle` instead of `compaction: auto`;
  - when such a compaction is the session's latest event, the X-ray's `cache` line reads `compacted while idle at …` instead of `compacted at … (auto)`. In this mode that happens only when the first request after the switch got no answer.
- There is no new kind. Claude Code treats both modes as one feature: one setting, `idleCompaction`, and both notices end with the same hint to turn it off.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `session-logs`: the requirement "Compaction kind" also accepts the second notice.

## Impact

- **Code:** the Claude Code source only: the notices it looks for, its line filter, a unit test and its README.
- **Views:** `tokprof misses`, and the X-ray in `tokprof`, `tokprof scan` and `tokprof ui`. Totals and cost attribution do not change.
- **How it is checked on the real logs:**
  - Today `tokprof misses --agent claude --expected` lists 6 rebuilds as `compaction: while idle` and 25 as `compaction: auto`. After the change, both counts stay the same, because the logs hold no line with the new notice.
  - A unit test with the lines of the new mode gets the kind `idle`.
  - The same command takes 6.7 s today. It stays within the noise.
- **Out of scope:**
  - The X-ray of a session that is still idle. In this mode, Claude Code writes nothing to the transcript until the return, so the X-ray still forecasts a full re-write. No forecast changes: the forecast after a compaction does not depend on its kind.
  - The cost of the call that writes the summary during the break. Claude Code does not log compaction calls.
  - Telling the two modes apart in the views.
  - Codex, which has no idle compaction.
