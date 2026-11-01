# Tasks

## 1. Lifetime from the latest write

- [x] 1.1 Carry the cache lifetime forward from the latest request that wrote to the cache: a request that writes nothing keeps the lifetime of the one before it. Verify: a unit test with a 1-hour write, a request that writes nothing, and a miss 20 minutes later that is not `idle > TTL`.

## 2. Checks

- [x] 2.1 Run `cargo test`. Count `idle > TTL` misses with `5m TTL` and 5–60 minutes idle before and after, for main chains and subagents, and record the counts and the moved premium in this file.

## Results on the real logs (2026-11-01)

`tokprof --agent claude misses --top 1000000`, the snapshot commit against this change:
- 313 misses both times, with the same causes and premiums.
- Exactly one miss changed: 65e9d5a1 at 2026-11-01 21:36, `idle > TTL: 3.4h idle, 5m TTL` became `… 1h TTL`, since its previous request wrote nothing.
- No miss with 5–60 minutes idle had a `5m TTL` detail before or after, on main chains or subagents. On these logs the bug mislabeled one detail and moved no premium.
