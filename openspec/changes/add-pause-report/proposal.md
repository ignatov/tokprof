# Proposal

## Why

Whether a prompt cache helps depends on how long the user pauses before typing again. tokprof does not show those pauses. The cold-session study of 2026-11-03 had to work them out with scripts, and they turned out to decide the choice of cache lifetime and of an idle policy.

The study used 961 typed messages in the author's IDE sessions over 30 days:
- 50% of them came more than 5 minutes after the previous request, and 17% more than an hour after it. On the 5-minute cache every second message finds the cache expired; on the 1-hour cache every sixth one does.
- The 206 cold returns cost $777 in their first requests, a median of $3.77 each.
  - 79 of them came after 1–2 hours, and 48 after 8–16 hours.
  - 166 started with a typed message, and the agent started the other 40 itself.
  - A median of 20 more requests followed each return, so the user came back to keep working.
- The 22 sessions that reached a context of 800k+ tokens made 60.5% of the spend.

These numbers decided the cache lifetime and idle-policy proposals of that day. They should come from tokprof, for any user's logs.

## What Changes

- **`tokprof pauses`**: a new report over the selected requests, with the global filters. It prints, per agent:
  - **Pauses before typed messages.** For the messages typed in main conversations, how long after the conversation's previous request each came, in fixed ranges from `under 1m` to `over 1d`. Each range has the count, the share and the median context.
  - **The share after each cache lifetime.** Claude Code: 5 minutes and 1 hour. Codex: 30 minutes, as cache-misses assumes.
  - **Cold returns by pause.** The `idle > TTL` misses of main conversations (see cache-misses), in ranges from the lifetime up to `over 2d`. Each range has the count, the first-request cost, the avoidable premium and the median context.
  - **Who started the cold returns**: a typed message, a background task, a tool result, or something else.
  - **What followed a cold return**: the median number of requests until the next cold pause or compaction, its quartiles, and how many returns ended within 3 requests.
  - **Sessions by largest context**, in ranges from `under 100k` to `800k+`. Each range has the sessions, the spend with subagents, the share and the cold returns.
- The README describes the report.

## Capabilities

### New Capabilities

- `pause-report`: the `tokprof pauses` command, its sections, and how pauses, returns and session sizes are counted.

### Modified Capabilities

- `session-logs`: the commands the filters apply to, and the commands that print the line about the logs read, now name `pauses` (and `tools`, which they had left out). The delta builds on the text of `add-client-dimension`, so this change is archived after it.

## How it is checked on the real logs

The baseline is the study of 2026-11-03, Claude Code `sdk-ts` requests over the 30 days to then. With `add-client-dimension` it runs as `--client sdk-ts`; without it, `--agent claude`, which differs by the few terminal sessions.

- The pause table matches the study's shares:

  | pause | share of typed messages |
  |---|---|
  | under 1m | 22.0% |
  | 1–5m | 28.5% |
  | 5–15m | 15.7% |
  | 15–30m | 8.9% |
  | 30–60m | 7.4% |
  | 1–2h | 5.8% |
  | 2–4h | 3.3% |
  | 4–8h | 1.2% |
  | 8–24h | 5.7% |
  | over 1d | 1.4% |

  The share after the lifetime is 50% for 5 minutes and 17% for 1 hour. Shares may move by a point as new sessions come in.
- The cold returns by pause:

  | pause | returns | first-request cost |
  |---|---:|---:|
  | 1–2h | 79 | $271 |
  | 2–4h | 34 | $138 |
  | 4–8h | 14 | $70 |
  | 8–16h | 48 | $189 |
  | 16–48h | 29 | $103 |
  | over 2d | 2 | $6 |

  In all, 206 returns, $777 of first requests and $740 avoidable.
- Who started the cold returns: 166 typed messages, 12 background tasks, 12 tool results, 16 others.
- What followed them: a median of 20 requests, quartiles 7 and 70, and 17 returns that ended within 3 requests.
- Sessions by largest context: `800k+` holds 22 sessions and 60.5% of the spend, `under 100k` 142 sessions and 3.1%.
- `tokprof pauses` takes no longer than `tokprof summary` on the same logs.

## Out of scope

- A pauses block in the X-ray, and pauses per session.
- Prices under other lifetimes or idle policies. `whatif` and `bench` already cover those.
- Pauses inside subagents. They only shape the subagent cache lifetime, which `whatif` already measures.

## Impact

- Core: a new collector over the requests and misses that the analysis already emits.
- CLI: the `pauses` command and its render.
- README: the new report.
