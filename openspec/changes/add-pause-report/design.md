# Design

## Context

Everything the report needs is already produced by the analysis:
- for each request: its agent, chain and chain kind, trigger, usage, price, the time since the previous request of its chain, and the previous prompt;
- for each miss: its cause and premium, right after its request.

No new log field is read. The report adds a collector over these events and a command that prints it.

The study scripts of 2026-11-03, kept outside the repository, are the reference implementation of the numbers in the proposal.

## Decisions

1. **One collector, fed per session.**
   - The collector keys sessions by the short id of the session being analyzed. Both decoders give a subagent log the id of its main session, so subagent spend lands on its main session for the session-size table without a separate feed.
   - The analysis reports no chain boundary. Before each session, the collector therefore works out which chain each reported request comes from, by the analysis's own rule: a request with usage that the filter keeps.
   - Only main-chain requests feed the pause, return and follow parts.
2. **Typed messages** are main-chain requests whose trigger is `prompt`. Their pause is the request's time since the previous request of the conversation.
   - The analysis walks the two same-name chains of a split conversation separately. The second chain's first request then has no previous request in its own chain.
   - The collector keeps the last request time per conversation, as `Session::main_conversations` joins them. A message at the start of a later chain still gets its pause.
   - Cold returns still come only from misses, so such a request is a cold return only when the cache-misses rules judged it one.
3. **Lifetimes per agent** come from the cache rules of each request's API, the same rules that cache-misses uses: Anthropic 5 minutes and 1 hour, OpenAI 30 minutes. The line lists the distinct lifetimes of the agent's APIs, shortest first.
4. **Cold returns** are main-chain requests whose miss cause is `idle > TTL`. The collector holds the last main request of each chain until its miss arrives or the next request comes.
5. **The follow count** starts at a cold return. It ends before the first later request of the conversation that follows a compaction (trigger `compaction`), or that comes more than the conversation's current lifetime after the request before it.
   - The lifetime is tracked as cache-misses defines it: 1 hour when the latest request that wrote to the cache wrote more 1-hour than 5-minute tokens, else 5 minutes; 30 minutes for OpenAI.
   - This differs from the bench's follow, which needs a miss and stops at its request limit.
6. **Session sizes.**
   - The largest prompt of a session's main chains decides its range.
   - Its spend is every selected request of its logs, subagents included.
   - Its cold returns are counted as in decision 4.
7. **Quartiles and medians** reuse the bench's median rule (the upper of two middle values). Quarter positions are rounded down.

## Risks / Trade-offs

- **Pause start.** A pause is measured from the previous request, not from the end of the agent's answer. Responses that stream for minutes make pauses look a little longer. This matches how the cache lifetime is counted, which also starts at a request.
- **Background tasks and tools** also make long pauses. The report shows them in "who started"; only typed messages go into the pause table.

## Parse time

There is no extra parsing. The collector does constant work per request and per miss. `tokprof pauses` reads the same logs as `summary`, so it takes about as long. The tasks check that it takes no longer.
