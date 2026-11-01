# Proposal

## Why

To decide whether a Claude Code miss came from idling past the cache lifetime, tokprof takes the lifetime from the previous request alone. A request that read its whole prompt from the cache writes nothing, so it looks like a 5-minute request even in a conversation on the 1-hour cache. The next request after a 5–60 minute pause then gets `idle > TTL: 20m idle, 5m TTL` although the 1-hour cache was still warm, and the real cause is never looked for.

The X-ray and `tokprof bench` already take the lifetime from the latest request that wrote to the cache, so the same session gets 1h in the X-ray and 5m in `tokprof misses`.

## What Changes

- The miss classifier takes a Claude Code chain's lifetime from its latest request that wrote to the cache, up to the previous request, as the X-ray does. A chain that has written nothing yet gets 5 minutes, as now.
- Misses after a 5–60 minute pause on the 1-hour cache stop being `idle > TTL`, and the later checks give their cause: edited history, changed tools or settings, or `unknown`.

Out of scope:
- Codex, whose lifetime stays an assumed 30 minutes.
- Reading the lifetime from the agent's settings (`ENABLE_PROMPT_CACHING_1H`, `promptCacheTtl`) instead of from the writes.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `cache-misses`: "Idle beyond the cache lifetime" takes the lifetime from the chain's latest cache write.

## How it is checked on the real logs

- Count the Claude Code misses whose cause is `idle > TTL` with a `5m TTL` detail and an idle time from 5 to 60 minutes, before and after, on the main chains and on subagents. Main chains on the 1-hour cache should lose most of them; subagents, which write with the 5-minute lifetime, should keep theirs.
- `tokprof summary`: the cause table moves that premium from `idle > TTL` to the other causes, and the total stays the same.

## Impact

`src/analysis/mod.rs`, where the previous request's record keeps the lifetime. The cause names do not change, so the advice text and its tests (change `compact-xray-advice`) are not affected.
