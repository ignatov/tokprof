# Design

## Context

- A compaction is kept only as a flag on the request after it (`compacted_before`). The Claude Code adapter collects `compact_boundary` timestamps per transcript, and the Codex adapter flags the request after a `compacted` line. A compaction after the latest request is therefore lost: no request carries the flag.
- The miss classifier already receives the session with each request. It returns `compaction` with an empty detail.
- The X-ray builds a `LiveContext` from the latest request with a prompt: its usage, content blocks, turns and window. The `cache` and `next` lines (`cache_state`, `next_request`) and the advice inputs all read that one value. The advice already has `after_compaction`, the prompt of the latest request flagged after a compaction, and the setup fallback.

## Goals / Non-Goals

**Goals:**
- Keep each session's compaction markers with their times and kinds, so any view can ask whether a compaction came after the latest request.
- Reuse the advice's post-compaction prompt and setup fallback for the estimate, so the advice and the X-ray agree on one number.

**Non-Goals:**
- No change to the advice format strings. They belong to `compact-xray-advice`; this change only changes what the advice rules are given.
- No new request field. The kind of a request's compaction comes from the session's marker list.

## Decisions

### Markers on the session
`Session` gets `compactions`: a sorted list of `(ts_ms, kind)`. `kind` is `Option<String>`: `"idle"`, the boundary's trigger, or `None` for Codex. The Claude Code adapter fills it from the main transcript; its markers apply to every chain of the transcript, as before. The Codex adapter fills it from `compacted` lines.

A list on the session over a field on each request: the trailing case has no request to carry it, and the classifier can look up the marker in `(previous request, request]` from the list. `compacted_before` stays as it is, so the bench and the Codex adapter don't change.

### Reading the idle notice
The Claude Code adapter's line filter also accepts lines that contain `Compacted while idle`. A `system` line with subtype `informational` whose `content` starts with that text records a notice time. After parsing, a boundary is `idle` when a notice falls between the boundary and the next answered request of any chain of the transcript. Otherwise the boundary's kind is its `compactMetadata.trigger`.

Log fields and versions:
- `type: system`, `subtype: compact_boundary`, `timestamp`, `compactMetadata.trigger` (`auto`; seen in 2.1.284–2.1.312). `preTokens`/`postTokens` are present, but this change does not use them.
- `type: system`, `subtype: informational`, `content: "Compacted while idle, before the prompt cache expired · turn off with \"idleCompaction\": false in settings.json"` (seen in 2.1.302 and 2.1.310, terminal sessions). In the logs it comes 0–1 s after its boundary.

Alternative considered: detect idle compaction by timing (a gap of about 54 minutes, a context of 200k+ tokens). Rejected, because the notice is logged, and timing would misread a manual `/compact` before a break.

### Cause detail
The classifier's `compaction` branch looks up the latest marker in `(prev.ts_ms, req.ts_ms]` and sets the detail to `while idle`, the trigger, or nothing. The misses-by-cause table groups by cause, so it does not change.

### X-ray estimate
After building `LiveContext`, the X-ray checks whether the session's latest marker is later than `live.ts_ms`. If so, it computes:
- `estimate`: the `after_compaction` value (the prompt of the conversation's latest request flagged after a compaction, which is the first request after the previous compaction), else `setup + 20,000`, the advice's existing fallback;
- `setup`: the tokens of the live prompt's tool-schema and system-prompt blocks.

Three views use these numbers:
- **`cache` line:** a new `CacheState` variant `Compacted { at_ms, kind, estimate }`. The cache line prints it with no color.
- **`next` line:** prices `setup` read, `estimate − setup + added` written and `output`, at the conversation's lifetime, with today's price.
- **Advice:** gets a derived `LiveContext` whose usage holds `estimate` tokens and whose blocks are only the setup blocks, with zero turns. So the carry-on rule (100k threshold), the context-cost rule (200k or 70%) and the content hints simply don't fire, and the session-wide rules are unchanged.

The `in context` part keeps the original `LiveContext` and adds `, before the compaction at HH:MM` to its heading.

Alternative considered: price the post-compaction prompt from the boundary's `postTokens` plus setup. Rejected: the logged next prompts (41–56k) are 4 to 6 times `postTokens` (7–15k), because Claude Code re-attaches files and plans after a compaction. The previous post-compaction prompt is closer, and the advice already uses it.

### Parse time
One more substring test per line in the Claude Code filter, and a JSON parse for the rare `informational` lines that pass it: 6 in the author's 4.6 GB of Claude Code logs. Expected cost: well under 1% of the ~8 s full parse. Task 4.4 measures it with `tokprof summary` before and after.

## Risks / Trade-offs

- [Claude Code changes the notice text] → Only the prefix is matched. If it changes, kinds fall back to the trigger (`auto`), which is today's behavior. A test pins the recorded line.
- [The first compaction of a conversation has no earlier post-compaction prompt] → The fallback, setup + 20,000 tokens, gave 32–42k where the logs show 41–56k. Expected error is under 2×.
- [Claude Code 2.1.314's precomputed summary for VS Code and Desktop logs differently] → Out of scope. Such compactions get their boundary's trigger, as today.
- [`compact-xray-advice` rewrites the advice in parallel] → This change edits only the advice inputs and the cache, next and in-context lines. Whichever change lands second rebases its edits in xray.rs.
