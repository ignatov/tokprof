# Tasks

## 1. Lifetime from the latest write

- [ ] 1.1 Carry the cache lifetime forward from the latest request that wrote to the cache: a request that writes nothing keeps the lifetime of the one before it. Verify: a unit test with a 1-hour write, a request that writes nothing, and a miss 20 minutes later that is not `idle > TTL`.

## 2. Checks

- [ ] 2.1 Run `cargo test`. Count `idle > TTL` misses with `5m TTL` and 5–60 minutes idle before and after, for main chains and subagents, and record the counts and the moved premium in this file.
