# Design

## Context

How tokprof reads idle compactions today:
- The Claude Code source records the time of each `system` line with subtype `informational` whose `content` starts with `Compacted while idle`.
- Its line filter passes such lines only between a `compact_boundary` and the next `assistant` line, so no other line pays for the search.
- After parsing, a boundary is `idle` when a notice falls between it and the next answered request of any chain. Otherwise it takes its `compactMetadata.trigger`.
- The views downstream read only the kind: the miss detail (cache-misses) and the X-ray's `cache` line after a trailing compaction (session-xray).

What Claude Code 2.1.316 does. This was read from its binary: the strings and the control flow are there, but no log has the second mode yet.
- The idle timer fires at 90% of the cache lifetime after the last activity, which is 54 minutes on the one-hour cache. It runs only in attended sessions, and only when the context holds at least 200k tokens. `CLAUDE_CODE_IDLE_COMPACT_MIN_TOKENS` changes that threshold, down to 100k.
- **With a compact host**, which only the interactive terminal registers, Claude Code compacts at once. Then it appends "Compacted while idle, before the prompt cache expired · turn off with "idleCompaction": false in settings.json".
- **Without a host**, as in sessions driven over stream-json, it writes a summary in the background. At the start of the next turn, the auto-compaction step swaps the summary in. It emits the compaction messages, then the notice "Compacted using a summary written while you were away · turn off with …", and only then sends the request. Before that turn, nothing about the summary is written to the transcript.
- The same call builds both notices, with level `notice`. The first one is logged like this (2.1.302, 2.1.310): `{"type":"system","subtype":"informational","level":"notice","isMeta":false,"content":"Compacted while idle, before the prompt cache expired · turn off with \"idleCompaction\": false in settings.json"}`. The separator before "turn off" is a no-break space.

## Goals / Non-Goals

**Goals:**
- Recognize the second notice with the same rule and at the same cost as the first.

**Non-Goals:**
- No change to which time a marker has. In the second mode the boundary is logged at the return, so the marker carries the return time. That is when the context actually shrank.
- No reading of the setting `idleCompaction` or of the entry point to guess whether a compaction will come.

## Decisions

### Two prefixes, one list of notice times
The source matches `content` against two prefixes: `Compacted while idle` and `Compacted using a summary written while you were away`. The line filter and the line handler use the same pair. Notices of both modes go into one list of times, and the rule that pairs a notice with a boundary stays as it is.

Alternatives considered:
- A shorter shared prefix, `Compacted `. Rejected: it is too broad for a label that changes the miss detail.
- The shared hint, `"idleCompaction": false`. Rejected: the hint decorates the message and could be dropped or reworded on its own, while each lead sentence states what happened. Full lead phrases are also what the spec names.

### One kind, `idle`
Both modes get the kind `idle`. Claude Code treats them as one feature: one setting, `idleCompaction`, and the same hint in both notices. For the user, both mean that the break caused the compaction. The rebuild after it is expected either way, and no number or advice depends on the mode.

Alternative considered: a kind `away`, with the miss detail `compaction: summary from the break` and its own wording in the `cache` line. Rejected: it adds wording to three capabilities for a distinction that changes no figure.

### Order of the lines in the second mode
The binary emits the compaction messages, the notice and then the request, so the notice falls between the boundary and the next answered request, as the rule needs. No `assistant` line comes between them, so the filter's window after a boundary is still open when the notice arrives.

### Parse time
The extra check is one more substring test, on lines between a boundary and the next response only. The author's logs have a few dozen compactions, so the test runs on a few hundred lines. No measurable effect is expected. Today `tokprof misses --agent claude --expected` takes 6.7 s, and a task re-measures it.

## Risks / Trade-offs

- [The second notice's line is inferred, not observed] → It comes from the same call as the first notice, whose logged shape is known. If the real line differs, for example in another subtype or before the boundary, these compactions keep their trigger's kind, which is today's behavior. When a log with it appears, its line replaces the synthetic one in the test, and the README and spec name the version it was seen in.
- [Claude Code rewords a notice] → Only the prefix is matched, and the kind falls back to the trigger. The test pins both texts.
- [The summary call of the second mode runs during the break and is not logged] → No change: Claude Code logs no compaction call in either mode, and tokprof already models none.

## Migration Plan

None. tokprof parses the logs on every run and keeps no cache, so old sessions are re-read with the new rule. No old log has the new notice.
