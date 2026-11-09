# Session X-ray Specification

## Purpose
The session X-ray answers in the terminal, with no setup, what one coding-agent session holds in its context, whether its prompt cache is still warm, what the next request will cost, where its money went and what to do next. `tokprof` alone X-rays the latest session.

## Requirements

### Requirement: Latest session by default
Without a target, `tokprof` and `tokprof scan` SHALL X-ray the main session with the latest last activity (see session-logs) among the sessions that `--agent` (default `claude,codex`) and the other filters select. Main logs are Claude Code transcripts outside a `subagents` directory and Codex rollouts whose `session_meta` line names no `parent_thread_id`. Where the logs are found belongs to session-logs.

#### Scenario: No command
- **WHEN** the user runs `tokprof` with no command
- **THEN** it prints the same X-ray as `tokprof scan` with no target

#### Scenario: A subagent wrote last
- **WHEN** the most recently modified log is a Claude Code subagent transcript or a Codex subagent rollout
- **THEN** the X-ray is of the main session with the latest last activity, not of the subagent log

#### Scenario: An idle session written last
- **WHEN** Claude Code appended only records without a time to an idle session's transcript after another session's latest request
- **THEN** the X-ray is of the other session

#### Scenario: One agent only
- **WHEN** the user runs `tokprof --agent codex` and a Claude Code session is newer than every Codex session
- **THEN** the X-ray is of the latest Codex session

#### Scenario: No session logs
- **WHEN** the chosen agents have no main session log
- **THEN** the command fails with the error `no session logs found` and a non-zero exit code

#### Scenario: A project filter
- **WHEN** the user runs `tokprof --project billing`
- **THEN** the X-ray is of the latest main session of a project whose path contains `billing`

### Requirement: Target resolution
`tokprof scan <target>` SHALL X-ray the file at `<target>` when it is an existing file, and otherwise the selected main session whose id starts with `<target>`, ignoring case, as session-logs resolves a target. A file named `rollout-*.jsonl` or `rollout-*.jsonl.zst` is read as a Codex rollout, any other file as a Claude Code transcript.

#### Scenario: Claude Code id prefix
- **WHEN** the user runs `tokprof scan 3f951de0`
- **THEN** the X-ray is of the transcript `3f951de0-14bc-4966-880b-81f649966ccf.jsonl`

#### Scenario: Codex thread id
- **WHEN** the user runs `tokprof scan 019ff5ca` and one Codex thread id starts with it
- **THEN** the X-ray is of the rollout `rollout-2026-08-12T13-44-47-019ff5ca-0db2-7180-8f61-e241e484f21a.jsonl`

#### Scenario: Several sessions share the prefix
- **WHEN** the target matches several main sessions, as Codex thread ids made minutes apart share their first 8 characters
- **THEN** the command fails and lists the matching sessions by their short ids, so a longer prefix can be given

#### Scenario: Compressed Codex rollout by path
- **WHEN** the target is the path of an archived `rollout-*.jsonl.zst`
- **THEN** the rollout is decompressed and read as a Codex session

#### Scenario: Subagent transcript by path
- **WHEN** the target is the path of a Claude Code subagent transcript
- **THEN** that transcript is X-rayed on its own, without a main conversation

#### Scenario: Nothing matches
- **WHEN** the user runs `tokprof scan zzzz` and no file or session id matches
- **THEN** the command fails with ``no session file or session id matches "zzzz"; see `tokprof sessions` `` and a non-zero exit code

### Requirement: Whole session with its subagents
The X-ray SHALL count the whole session: its main log and every subagent log it spawned (Claude Code transcripts in the session's `subagents` directory, Codex rollouts that name the session's thread as their parent), every chain in them, over the whole log. `--since` and `--until` do not narrow it.

#### Scenario: Session with subagents
- **WHEN** a session spawned 5 subagents
- **THEN** its `time` line ends with `· 5 subagents`
- **AND** the models, request count, spend, costly content and cache misses include the subagents' requests

#### Scenario: Date filters
- **WHEN** the user runs `tokprof --since 7d scan 3f951de0`
- **THEN** the X-ray is the same as without `--since`

### Requirement: Header
The X-ray SHALL open with the title `tokprof · session X-ray` and these lines:
- `session` with the agent, the project path (home shown as `~`) and the full session id;
- `title` with the session's title (see cost-attribution) cut to 82 characters;
- `client` with the clients of the session's requests (see session-logs);
- `models` with each model and its request count;
- `time` with the first and last request in local time, their span, the request count and the number of subagent logs.

#### Scenario: Session within one day
- **WHEN** the first and last requests fall on the same local date
- **THEN** the end shows only the time of day, as in `time      2026-11-01 08:49 → 18:15 (9.4h) · 657 requests`

#### Scenario: Session without subagents
- **WHEN** a session has no subagent logs
- **THEN** its `time` line ends with the request count and names no subagents

#### Scenario: Session over several days
- **WHEN** a session ran from one day into the next
- **THEN** both ends show the date, as in `time      2026-10-22 11:18 → 2026-10-23 19:57 (1.4d) · 7015 requests · 75 subagents`

#### Scenario: Session without requests
- **WHEN** a session's logs hold no request, as in a Codex rollout that stopped before its first response
- **THEN** there is no `models` line, and the time line reads `time      0 requests`
- **AND** no date from 1970 is shown: whenever no request has a time, the line gives only the counts

#### Scenario: Models
- **WHEN** a session used two models
- **THEN** the `models` line lists both by name with their counts, as in `claude-haiku-4-5 ×12, claude-opus-5-5 ×657`

#### Scenario: Span format
- **WHEN** the span is under a minute, under an hour, under a day, or longer
- **THEN** it shows in seconds (`59s`), whole minutes (`45m`), hours with one decimal (`9.4h`) or days with one decimal (`1.4d`)

#### Scenario: Title
- **WHEN** the session's title is `Ревью PR #1141 по чеклисту`
- **THEN** the line after `session` reads `title     Ревью PR #1141 по чеклисту`
- **AND** a session without a title has no `title` line

#### Scenario: One client
- **WHEN** every request of the session, its subagents included, has the client `sdk-ts`
- **THEN** the line after `title` reads `client    sdk-ts`

#### Scenario: Several clients
- **WHEN** a session has 412 requests with the client `cli` and 37 with `sdk-ts`
- **THEN** the line reads `client    cli ×412 · sdk-ts ×37`, with each client's request count, most requests first

#### Scenario: No client
- **WHEN** no request of the session has a client
- **THEN** the line reads `client    unknown`

### Requirement: Layout
The X-ray SHALL print, in this order and separated by blank lines: the header; the `context`, `cache` and `next` lines with the advice; `spend`; the `work`, `waits`, `prompts`, `tools`, `failures` and `setup` blocks when they are shown; `in context`; `what cost the most`; the cache misses; and the `profile` pointer (see the scenario The profile pointer). The parts about the live context need a main conversation.

#### Scenario: The profile pointer
- **WHEN** the X-ray ends
- **THEN** its last line is `profile: tokprof flame --session <id> → <dir>/<id>.jfr (IntelliJ: Open snapshot)`, where `<id>` is the session's short id (see session-logs) and `<dir>` is `~/tokprof`, or `$TOKPROF_DIR` when set

#### Scenario: Session with a main conversation
- **WHEN** the main log has a request with a prompt
- **THEN** every part is printed in the order above

#### Scenario: No main conversation
- **WHEN** the main log has no request with a prompt, for example when only its subagents made requests or the target is a subagent transcript
- **THEN** the `context`, `cache` and `next` lines, the advice and `in context` are left out
- **AND** the header, `spend`, the `work`, `waits`, `prompts`, `tools`, `failures` and `setup` blocks when they are shown, `what cost the most`, the cache misses and the pointer are still printed

#### Scenario: Session without requests
- **WHEN** the log holds no request with usage
- **THEN** there is no `models` line, the `time` line says `0 requests`, `spend` shows `0 tokens (no price)` and the misses line says `no misses`

### Requirement: Main conversation
The context, cache, forecast and advice SHALL describe the main conversation that holds the session's latest request with a prompt, advisor calls left out. Chains of that conversation's name that follow one another in time count as one conversation; chains of that name that overlap it in time are other conversations. How chains are built belongs to session-logs.

#### Scenario: Conversation split by a Claude Code update
- **WHEN** a Claude Code session ran before its version wrote request-audit lines and was resumed after, so its main conversation sits in two chains, one after the other
- **THEN** the X-ray reads both chains as one conversation in time order
- **AND** the conversation's first prompt is the first prompt of the earlier chain, not the cold re-write that opens the later one

#### Scenario: Overlapping chain of the same name
- **WHEN** another chain named `main` overlaps both parts in time
- **THEN** it is not joined to the conversation

#### Scenario: Latest request
- **WHEN** the main chains end with an advisor call or a request record without usage
- **THEN** the live context is the prompt of the latest request that has a prompt and is not an advisor call

### Requirement: Context window fill
The `context` line SHALL show the latest prompt's size against the model's context window: a 32-cell bar, the share with one decimal and `<size> of <window>`. The window comes from the log when the agent records it (Codex) and else from the model's entry in the price table (see pricing). Without a known window the line shows only `<size> tokens`.

#### Scenario: Claude Code on a 1M window
- **WHEN** the latest prompt holds 817.6k tokens on `claude-opus-5-5`
- **THEN** the line reads `context   ██████████████████████████░░░░░░   81.8%  817.6k of 1.0M`

#### Scenario: Codex logs its window
- **WHEN** a Codex rollout records a context window of 258.4k tokens
- **THEN** the line shows the share of 258.4k, whatever the price table says

#### Scenario: Unknown window
- **WHEN** neither the log nor the price table gives the model's window
- **THEN** the line shows only the prompt size, as in `context   817.6k tokens`

### Requirement: Cache warm or cold
The `cache` line SHALL tell whether the main conversation's prompt cache is still warm. Idle time runs from the conversation's latest request to now, and the cache is cold once idle time reaches the lifetime: 1h for Claude Code when the conversation's latest cache write was mostly 1-hour writes, 5m for other Claude Code conversations, and 30m for Codex.

#### Scenario: Warm cache
- **WHEN** a Claude Code conversation on the 1-hour cache idled for 48 minutes
- **THEN** the line reads `cache     warm · 1h TTL · cold in 12m`

#### Scenario: Cold cache
- **WHEN** the same conversation idled for 3.9 hours
- **THEN** the line reads `cache     cold for 2.9h (1h TTL)`, counting the time past the lifetime

#### Scenario: Claude Code on the 5-minute cache
- **WHEN** the conversation's latest cache write was mostly 5-minute writes
- **THEN** the lifetime is `5m` and the cache is cold after 5 idle minutes

#### Scenario: Codex
- **WHEN** the session is a Codex rollout
- **THEN** the lifetime is `30m`, whatever its cache writes

### Requirement: Next-request forecast
When the model has a price today (see pricing), the `next` line SHALL forecast the next request at the conversation's pace: the mean new prompt tokens (cache writes and uncached input) and output of its latest 20 requests with a prompt. Warm, the latest prompt is read from the cache and the new tokens are written; cold, the prompt and the new tokens are written. Both add the output, and writes use the price of the conversation's cache lifetime.

#### Scenario: Warm cache
- **WHEN** the cache is warm
- **THEN** the line gives both costs, as in `next      ≈ $0.11 per request while warm · ≈ $1.31 once cold = 20% of the session's $6.57`

#### Scenario: Cold cache
- **WHEN** the cache is cold
- **THEN** the line gives the cold cost and the prompt it re-writes, as in `next      ≈ $6.57 for one request, re-writing 817.6k = 6% of the session's $107`

#### Scenario: Share of the session's cost
- **WHEN** the session has cost $0.005 or more so far
- **THEN** the line ends with the cold forecast as a share of the session's cost, subagents included
- **AND** when the session's cost is under $0.005, so that it would read `$0.00`, the share is left out, here and on the forecast after a trailing compaction

#### Scenario: No price today
- **WHEN** the latest request's model has no price today
- **THEN** there is no `next` line

### Requirement: Prompt after a trailing compaction
When the main conversation's latest compaction marker (see session-logs) comes after its latest request with a prompt, the X-ray SHALL take as the latest prompt an estimate of what the next request sends. The estimate is the prompt of the conversation's first request after its previous compaction, or else the tool schemas and system prompt plus 20,000 tokens. The `context` line shows the estimate with `~`, and the forecast and every advice rule use it.

#### Scenario: Compacted while idle, with an earlier compaction
- **WHEN** a conversation's latest request sent 440.1k tokens at 18:18, a compaction of kind `idle` at 19:13 follows it, and the first request after its previous compaction sent 55.8k tokens
- **THEN** the latest prompt is ~55.8k and the `context` line shows `~55.8k of 1.0M` with its share of the window

#### Scenario: The conversation's first compaction
- **WHEN** the compaction after the latest request is the conversation's first, and its tool schemas and system prompt hold 14.4k tokens
- **THEN** the latest prompt is ~34.4k

#### Scenario: Advice after the compaction
- **WHEN** the estimate is under 100,000 tokens
- **THEN** the advice has no "How to carry on" item, by that requirement's own threshold, and offers neither a full re-write nor `/compact`

#### Scenario: Content hints after the compaction
- **WHEN** the latest logged prompt was 40% tool results, and the latest prompt is now an estimate
- **THEN** the advice gives no hints on what fills the context, since the compaction replaced that content
- **AND** the advice rules that look at the whole session, such as thinking cost and cache misses, are given as before

#### Scenario: What the context held
- **WHEN** the latest prompt is an estimate
- **THEN** the `in context` part still breaks down the latest logged prompt, and its heading ends with `, before the compaction at 19:13`

#### Scenario: A request after the compaction
- **WHEN** a request with a prompt follows the compaction
- **THEN** the X-ray describes that request's prompt as before, with no estimate

### Requirement: Cache line after a trailing compaction
When the latest prompt is an estimate after a compaction, the `cache` line SHALL say when the conversation was compacted, how, and what the next request re-builds, in place of warm or cold. The kind `idle` reads `while idle`; other kinds are shown in parentheses, and a marker with no kind shows none. The line has no color.

#### Scenario: Compacted while idle
- **WHEN** the compaction at 19:13 has the kind `idle` and the estimate is ~55.8k
- **THEN** the line reads `cache     compacted while idle at 19:13 · the next request re-builds ~55.8k`

#### Scenario: Auto-compaction
- **WHEN** the compaction at 19:13 has the kind `auto`
- **THEN** the line reads `cache     compacted at 19:13 (auto) · the next request re-builds ~55.8k`

#### Scenario: Codex
- **WHEN** a Codex rollout's `compacted` line comes after its latest usage line
- **THEN** the line reads `cache     compacted at 19:13 · the next request re-builds ~<estimate>`

### Requirement: Forecast after a trailing compaction
When the latest prompt is an estimate after a compaction and the model has a price today, the `next` line SHALL forecast one request. It reads the tool schemas and system prompt from the cache, writes the rest of the estimate plus the conversation's mean new tokens (see Next-request forecast), and adds the mean output. Writes use the price of the conversation's cache lifetime. There is no warm figure.

#### Scenario: Forecast after an idle compaction
- **WHEN** the estimate is 55,800 tokens, 14,400 of them tool schemas and system prompt, the pace adds 2,000 new tokens and 1,000 output tokens per request, on `claude-opus-5-5` with 1-hour writes, in a session that cost $414
- **THEN** the forecast is (14,400 × $0.20 + 43,400 × $8 + 1,000 × $20) per million = $0.37
- **AND** the line reads `next      ≈ $0.37 for one request, re-building ~55.8k = 0% of the session's $414`

### Requirement: Spend by component
The `spend` part SHALL show the session's API-equivalent cost as `<total> (API-equivalent)`, one row per cost component (cache read, cache write, rewrite after miss, uncached input, output) with a bar, its share and its amount, and, when the session has more than one chain, a line with the cost of each chain, all subagent chains together as `subagents`. How components are charged belongs to cost-attribution.

#### Scenario: Priced session
- **WHEN** a session's requests have prices
- **THEN** each component row reads like `  cache read          ████████████████████░░░░░░░░░░░░   62.3%     $66.69`
- **AND** components under 0.05% of the total are left out

#### Scenario: Chains
- **WHEN** the session has a main chain and subagents
- **THEN** a line such as `main $16.37 · subagents $45.64` follows the rows
- **AND** a session with one chain has no such line

#### Scenario: No prices
- **WHEN** none of the session's requests has a price
- **THEN** the headline reads `<tokens> tokens (no price)` and the rows show tokens instead of dollars

### Requirement: What the context holds now
The `in context` part SHALL break down the latest prompt of the main conversation: `<size> tokens, <n> turns, as of <HH:MM>`, one row per content kind with a bar, share and tokens, then up to 6 `largest blocks` with tokens, kind, tool and target, and `setup` for tool schemas and the system prompt or else the turn the block entered in. How the prompt is rebuilt belongs to session-logs.

#### Scenario: Rebuilt prompt
- **WHEN** the log shows what the latest prompt holds
- **THEN** the rows follow a fixed order: tool schemas, system prompt, user prompt, reminders, tool results, assistant text, thinking, tool calls, images, unknown
- **AND** a largest block reads like `      15.4k  tool results   Bash  for f                                        turn 2`

#### Scenario: Turn count
- **WHEN** the latest prompt holds one turn, or none
- **THEN** the head says `1 turn`, or leaves the turn count out

#### Scenario: Log without the prompt's content
- **WHEN** the log does not show the content of the prompt, as in Claude Code transcripts written before 2.1.300
- **THEN** the head is followed only by `claude logs do not show what the prompt holds yet`

### Requirement: Content that cost the most
Under `what cost the most to keep in the context (each source with every request that re-read it)`, the X-ray SHALL list the 8 pieces of content that cost the most over the whole session, subagents included. Content is grouped by kind, tool and target, ranked by dollars, or by tokens when nothing is priced. Each row gives the amount, the kind, the tool and target, and for tool calls and results the number of calls behind it.

#### Scenario: Priced session
- **WHEN** the session has prices
- **THEN** rows read like `     $10.13  tool results   exec_command  sed -n   412 calls`
- **AND** the count is the number of calls with that tool and target in the session, subagents included, and `1 call` for one

#### Scenario: No prices
- **WHEN** nothing in the session is priced
- **THEN** the rows are ranked by tokens and show tokens

### Requirement: Cache misses section
The X-ray SHALL summarize the session's cache misses on a `cache` line: the count of unexpected misses, the tokens they re-processed, their avoidable cost when priced and the count of expected rebuilds. Up to 5 unexpected misses follow, ranked by avoidable cost and then re-processed tokens, each with its time, cause and detail, tokens, avoidable cost (`-` when unpriced) and chain. The miss rule and the causes belong to cache-misses.

#### Scenario: No misses
- **WHEN** the session has no miss of any kind
- **THEN** the line reads `cache     no misses`

#### Scenario: Unexpected misses
- **WHEN** a session had one miss after an idle break and one expected rebuild
- **THEN** the line reads `cache     1 misses · re-processed 395.6k tokens · $3.09 avoidable · 1 expected rebuilds`
- **AND** the miss row reads like `  11-01 15:15  idle > TTL: 1.8h idle, 1h TTL                  395.6k     $3.09  main`

#### Scenario: Only expected rebuilds
- **WHEN** every miss of the session is an expected rebuild
- **THEN** the line counts `0 misses` and the expected rebuilds, with no miss rows

### Requirement: Advice block
When there is a main conversation, the X-ray SHALL print an `advice` block after the `next` line, with the items of each rule that fires, in this order: how to carry on, context cost, tool results, tool calls, lookups, reminders, pasted prompts, thinking, cache misses. The rules are deterministic and every item carries the numbers behind it. Each item says one thing in plain sentences: what it is about, the numbers, and what to do.

#### Scenario: Several rules fire
- **WHEN** three rules fire, one item each
- **THEN** the first item's line carries the `advice` label, the other items are indented under it, and each starts with `▸`

#### Scenario: A session with six rules
- **WHEN** session 3f951de0 is cold, with 817.6k tokens in its latest prompt, and the carry-on, context cost, tool results, tool calls, pasted prompts and thinking rules fire
- **THEN** the block takes 14 lines:
  ```
  advice    ▸ Resuming writes the whole 817.6k context again (+$6.38), then $0.16 per request.
            ▸ A new session that refers to this one is cheaper: ≈ $0.31 to pick up the work.
              `tokprof handoff 3f951de0` prints the line to start it with.
            ▸ /compact costs about as much as resuming (≈ $6.74 once), then ~44.2k per request.
            ▸ The context fills 82% of the window. At 12 requests per prompt, each prompt costs
              ≈ $1.99 in reads alone.
            ▸ Tool results take 45% of the context (Bash 299.9k, Read 63.9k): pipe long command
              output through tail or grep.
            ▸ Tool calls take 32% of the context (Bash 181.2k, Write 62.8k): write long inline
              scripts to a file and run the file.
            ▸ Pasted prompts (turn 1 14.6k, turn 3 29.6k) are re-read on every request: pass a
              file path instead.
            ▸ Thinking cost $16.18, 15% of the session, at effort max: use a lower effort for
              routine steps.
  ```

#### Scenario: Wrapping
- **WHEN** an item is longer than 82 characters
- **THEN** it wraps at 82 characters, and its next lines start 12 columns in, under its text, so the X-ray fits the X-ray pane of `tokprof ui`
- **AND** the fixed texts keep every item within two lines unless a tool or reminder source has a very long name

#### Scenario: No rule fires
- **WHEN** no rule fires, as for a small session with a balanced context
- **THEN** there is no `advice` block

### Requirement: How to carry on
When the latest prompt holds at least 100,000 tokens, the first advice items SHALL say how to carry on, in up to three items: resuming, a new session that refers to this one, and `/compact`. On a cold cache they give what resuming re-writes and costs, the pickup estimate and what `/compact` first costs. On a warm cache they give the time left, what `/compact` costs now and what the first request after a break would add. Dollars use today's prices; without a price only the token counts remain.

#### Scenario: Cold cache, a new session is cheaper
- **WHEN** a 950.0k-token Claude Code prompt on `claude-opus-5-5` with 1-hour writes is cold, its conversation's first prompt held 40,000 tokens, its tool schemas and system prompt hold 30,000, and it was never compacted
- **THEN** the items read "Resuming writes the whole 950.0k context again (+$7.41), then $0.19 per request.", "A new session that refers to this one is cheaper: ≈ $0.45 to pick up the work. `tokprof handoff 3f951de0` prints the line to start it with." and "/compact costs about as much as resuming (≈ $7.80 once), then ~50.0k per request."
- **AND** `+$7.41` is the prompt at the write price less the read price, `$0.19` the prompt at the read price, and the handoff command takes the session's short id
- **AND** `/compact` once is the prompt at the write price plus 10,000 output tokens, and `~50.0k` is the prompt right after the session's last compaction or, without one, the tool schemas and system prompt plus 20,000 tokens

#### Scenario: Cold cache, a new session is not cheaper
- **WHEN** the cache is cold and the pickup estimate is not below the extra cost of re-writing the prompt
- **THEN** the second item reads "A new session that refers to this one costs about as much (≈ <pickup>)." and gives no handoff command

#### Scenario: Warm cache, a new session is cheaper after a break
- **WHEN** the same prompt's cache stays warm for 50 more minutes
- **THEN** the items read "Cache warm for 50m: keep going now, or /compact before a break (≈ $0.39 now).", "Once it is cold, resuming writes the whole 950.0k context again (+$7.41)." and "Then a new session that refers to this one is cheaper: ≈ $0.45 to pick up the work (`tokprof handoff 3f951de0`)."
- **AND** `/compact` now is the prompt at the read price plus 10,000 output tokens

#### Scenario: Warm cache on Codex
- **WHEN** a 230.0k-token Codex prompt on `gpt-6-astra` stays warm for 20 more minutes
- **THEN** the first item reads "Cache warm for 20m: keep going now, or /compact before a break (≈ $3.38 now)."
- **AND** `/compact` now is the prompt at the write price plus 10,000 output tokens, (230,000 × 12.50 + 10,000 × 50) / 1,000,000, because Codex's compaction call reads nothing from the cache (see cache-misses, Compaction call)

#### Scenario: Warm cache, a new session is not cheaper
- **WHEN** the cache is warm and the pickup estimate is not below the extra cost of re-writing the prompt
- **THEN** the items end after "Once it is cold, resuming writes the whole <size> context again (+<extra>).", with no third item

#### Scenario: No price
- **WHEN** the model has no price today
- **THEN** there is one item: "Resuming writes the whole <size> context again." on a cold cache, or "Cache warm for <left>: keep going now." on a warm one

#### Scenario: Smaller context
- **WHEN** the latest prompt holds fewer than 100,000 tokens
- **THEN** there is no item on how to carry on

### Requirement: Pickup estimate
The pickup cost SHALL be a calibrated estimate of picking up the work in a new session that refers to this one, from what the reference bench measured (docs/reference-bench.md). The new session starts from the smaller of the conversation's first prompt and its prompt right after the last compaction. Claude Code then takes in 8,000 tokens over 4 requests and writes 2,000; Codex takes in 30,000 over 6 requests and writes 4,000.

#### Scenario: How the estimate adds up
- **WHEN** the pickup is estimated
- **THEN** it is the start and the taken-in tokens written once, plus the start and half the taken-in tokens read on each further request, plus the output
- **AND** it uses today's prices for a prompt of the start plus the taken-in tokens, and the old conversation's cache-write lifetime

#### Scenario: Old prompt over a long-context tier
- **WHEN** a Codex session on `gpt-6-astra`, whose prices change for prompts over 272,000 tokens, holds a 300,000-token prompt and starts from 18,000 tokens
- **THEN** the pickup is priced at the standard prices, as for any 48,000-token prompt, while resuming and `/compact` are priced at the long-context ones

#### Scenario: Claude Code calibration
- **WHEN** a Claude Code session on `claude-opus-5-5` with 1-hour writes starts from 42,000 tokens
- **THEN** the estimate is within the $0.44–0.51 the reference bench measured

#### Scenario: Codex calibration
- **WHEN** a Codex session on `gpt-6-astra` starts from 18,000 tokens
- **THEN** the estimate is within the $0.70–1.26 the reference bench measured

#### Scenario: Forked session that starts big
- **WHEN** a session forked from another started at 171,000 tokens and holds 21,800 tokens right after its last compaction
- **THEN** the estimate starts from 21,800 tokens, ≈ $0.29 on `claude-opus-5-5` with 1-hour writes

#### Scenario: No estimate
- **WHEN** the model has no price today or the start is zero
- **THEN** there is no pickup estimate and no item on a new session, and a cold cache has no item on `/compact` either

### Requirement: Context cost advice
When the model has a price today and the latest prompt holds at least 200,000 tokens or fills at least 70% of the window, the advice SHALL say what the context costs. It gives the window share, the conversation's requests per prompt and what each prompt costs in context reads alone, or what each request re-reads when the context holds no turn, and adds `Auto-compaction is close.` at 85% of the window or more.

#### Scenario: Nearly full window
- **WHEN** a 950.0k-token prompt fills 95% of a 1M window at 30 requests per prompt on `claude-opus-5-5`
- **THEN** the item reads "The context fills 95% of the window. At 30 requests per prompt, each prompt costs ≈ $5.70 in reads alone. Auto-compaction is close."
- **AND** requests per prompt are the conversation's requests whose prompt the log shows, divided by the prompts the user sent in them, and a prompt's cost is that many reads of the whole prompt
- **AND** each request adds as many prompts as its prompt holds turns beyond the previous request's, while the first request after a compaction adds none, since the compaction re-sends earlier prompts
- **AND** the first request with a prompt in a main chain that continues an earlier main chain of its name (see session-logs) adds none either, since its turns were sent in the earlier chain

#### Scenario: After a compaction
- **WHEN** a Codex conversation sent 700 requests over 300 prompts, its compaction re-sent 20 of those prompts, and it then sent 20 requests over 2 new prompts
- **THEN** the item gives 2 requests per prompt (720 over 302), not 33 (720 over the 22 turns of the latest prompt)

#### Scenario: Context without turns
- **WHEN** the latest prompt holds no turn
- **THEN** the item says what each request re-reads, as in "The context fills <share>% of the window. Each request re-reads <size> tokens (<cost>)."

#### Scenario: Unknown window
- **WHEN** the prompt holds 200,000 tokens or more and the window is unknown
- **THEN** the item leaves out the window sentence, as in "At 30 requests per prompt, each prompt costs ≈ $5.70 in reads alone."

#### Scenario: Below the thresholds
- **WHEN** the prompt holds fewer than 200,000 tokens and fills less than 70% of the window
- **THEN** there is no context cost item

### Requirement: Hints on what fills the context
The advice SHALL add one item for each kind of content in the latest prompt that passes its threshold: tool results at 30% of the prompt or more, tool calls at 20% or more, reminders at 30,000 tokens or 5% or more, and user prompt blocks of 10,000 tokens or more, reported as pasted prompts. Each item gives the share or size, up to two leading sources with their tokens, and a fixed hint chosen by the source that holds the most.

#### Scenario: Bash output leads the tool results
- **WHEN** tool results fill 84% of the prompt and Bash holds all of them
- **THEN** the item reads "Tool results take 84% of the context (Bash 800.0k): pipe long command output through tail or grep."
- **AND** the same hint goes with `BashOutput`, `exec_command` and `shell`

#### Scenario: Two leading sources
- **WHEN** tool results fill 45% of the prompt, with Bash 299.9k, Read 63.9k and ListAgents 1.3k
- **THEN** the item reads "Tool results take 45% of the context (Bash 299.9k, Read 63.9k): pipe long command output through tail or grep."

#### Scenario: Other leading tool results
- **WHEN** `Read`, `WebFetch` or `WebSearch`, or `Grep` or `Glob` lead the tool results
- **THEN** the hint is "read only the ranges you need", "ask for a summary instead of the page" or "narrow the pattern or the path"
- **AND** any other leading tool gets "use narrower calls, or a subagent for exploration"

#### Scenario: Inline scripts lead the tool calls
- **WHEN** tool calls fill 20% of the prompt or more and `Bash`, `exec_command` or `shell` lead them
- **THEN** the item starts with "Tool calls take <share>% of the context (<sources>):" and gives "write long inline scripts to a file and run the file"

#### Scenario: File writes lead the tool calls
- **WHEN** `Write`, or `Edit` or `MultiEdit`, lead the tool calls
- **THEN** the hint is "prefer small edits to whole-file writes" or "smaller edits are cheaper"
- **AND** any other leading tool gets "keep tool inputs short"

#### Scenario: Instruction files lead the reminders
- **WHEN** reminders hold 30,000 tokens or 5% of the prompt and instruction files hold the most of them
- **THEN** the item starts with "Reminders hold <tokens> tokens (<sources>):" and gives "trim CLAUDE.md and AGENTS.md, or move rarely needed parts to docs read on demand"

#### Scenario: Other leading reminders
- **WHEN** background notifications, hook context, deferred tool lists or skills lead the reminders
- **THEN** the hint is "run fewer or quieter background jobs", "make the hooks print less", "use fewer MCP servers or tools" or "load fewer skills per session"
- **AND** any other leading source gets "trim what the task does not need"

#### Scenario: Pasted prompts
- **WHEN** a user prompt block of 120.0k tokens entered in turn 2
- **THEN** the item reads "Pasted prompts (turn 2 120.0k) are re-read on every request: pass a file path instead."

#### Scenario: Many pasted prompts
- **WHEN** five user prompt blocks hold 10,000 tokens or more each
- **THEN** the item lists the three largest in turn order, each with its turn and size, then `and 2 more`, as in "Pasted prompts (turn 1 14.6k, turn 3 29.6k, turn 12 110.2k and 2 more) are re-read on every request: pass a file path instead."

### Requirement: Thinking cost advice
When thinking costs at least 10% of the session's spend, subagents included, the advice SHALL say what it cost and what share of the session that is, with the effort level the conversation's requests used most, and suggest a lower effort for routine steps.

#### Scenario: Thinking at a high effort
- **WHEN** thinking cost $20.00 of a $100.00 session and most requests used effort `max`
- **THEN** the item reads "Thinking cost $20.00, 20% of the session, at effort max: use a lower effort for routine steps."

#### Scenario: No effort in the log
- **WHEN** the log records no effort level
- **THEN** the item leaves out `, at effort <level>`, as in "Thinking cost $20.00, 20% of the session: use a lower effort for routine steps."

#### Scenario: Unpriced session
- **WHEN** the session's spend is zero because nothing is priced
- **THEN** there is no thinking item

### Requirement: Cache miss advice
When the unexpected cache misses of the session, subagents included, have an avoidable cost of at least $5 or at least 5% of the session's spend, the advice SHALL give that cost, the cause whose misses cost the most with their count, and a fixed hint for that cause.

#### Scenario: Misses after idle breaks
- **WHEN** three misses after idle breaks cost $12.00 that could have been avoided
- **THEN** the item reads "Cache misses could have saved $12.00, mostly idle > TTL (3×): compact or wrap up a big session before a long break."

#### Scenario: Hints by cause
- **WHEN** the costliest cause is a model or parameter change, a change of tools, system prompt or agent version, rewritten history, or a step past the lookback window
- **THEN** the hint is, in turn, "switch model or effort in a new session, not mid-task", "avoid toggling MCP servers or plugins, or updating the agent, mid-task", "earlier messages were rewritten, and everything after them was written again" or "one step added more than 20 content blocks, past the cache lookback window"
- **AND** misses of unknown cause get "the logs do not show the cause"

#### Scenario: Compaction calls
- **WHEN** 162 compaction calls of a Codex session cost $250 that could have been avoided, more than any other cause
- **THEN** the item reads "Cache misses could have saved $250, mostly compaction call (162×): each call wrote the whole context again; hand off to a new session before the context fills."

#### Scenario: Small misses
- **WHEN** the unexpected misses could have saved $3.09 of a $107 session
- **THEN** there is no cache miss item

### Requirement: Color carries the cache state
The X-ray SHALL use color only when stdout is a terminal and `NO_COLOR` is not set. The colors that carry meaning are green for a warm cache, the warm forecast and a session with no misses, and red for a cold cache, the cold forecast and the line of unexpected misses.

#### Scenario: Terminal
- **WHEN** the user runs `tokprof` in a terminal
- **THEN** `warm` is green, or `cold` and the cold forecast are red

#### Scenario: Piped or NO_COLOR
- **WHEN** stdout is piped into another program, or `NO_COLOR` is set
- **THEN** the X-ray has no escape codes

### Requirement: Answer quickly
The X-ray SHALL read in full only the chosen session's main log and its subagent logs, so its run time follows that session's size and not the size of the whole log history. `tokprof` with no arguments SHOULD answer about the latest session in under a second, the project's console-first target.

#### Scenario: Large history, small latest session
- **WHEN** the machine holds gigabytes of older logs and the latest session's logs are a few megabytes
- **THEN** `tokprof` answers in under a second
- **AND** it does not parse the other sessions' logs

### Requirement: Work block
After `spend`, the X-ray SHALL print a `work` block when the session, its subagents included, has more than one origin, task or activity (see cost-attribution). The block starts with `work      by origin: the kind of work each request was part of`. Then comes one line per origin, most expensive first, with a bar, its share and its cost. The sixth and later origins fold into one `<n> more` line.

#### Scenario: A code-review session
- **WHEN** a session ran the `code-review` skill with 98 subagents, at a cost of $508 of its $526
- **THEN** the first origin line reads `skill:code-review` with a nearly full bar, `96.5%` and `$508`, followed by `prompt` and the session's other origins

#### Scenario: Typed prompts only
- **WHEN** every request of a session without subagents comes from typed prompts, with no tool call, and the session has one title
- **THEN** there is no `work` block

### Requirement: Tasks in the work block
When the session has more than one task, the work block SHALL go on with `by task: the session title, and what each subagent was asked to do`. Up to 6 tasks follow, most expensive first, each with its cost, the task cut to 50 characters and its main origin. A last line `<n> more tasks` counts the rest.

#### Scenario: Reviewers by name
- **WHEN** the code-review subagents were described as `Angle C cross-file tracer` and `Angle A line-by-line scan`
- **THEN** the task lines read, for example, `$57.46  Angle C cross-file tracer` with `skill:code-review` dimmed after it

### Requirement: Lookup advice
When the session's requests with trigger `lookup` (see cost-attribution), subagents included, number at least 20 and cost at least $5 or at least 10% of the session's spend, the advice SHALL say how many requests single lookups made, how many tokens each re-read on average, what they cost and what share of the session that is, and suggest batching independent reads or exploring in a subagent.

#### Scenario: Lookups in a big context
- **WHEN** 412 requests with trigger `lookup` re-read 380,200 tokens on average and cost $58.10 of a $276.67 session
- **THEN** the item reads "Single lookups made 412 requests, each re-reading ≈ 380.2k tokens: $58.10, 21% of the session. Batch independent reads, or explore in a subagent."

#### Scenario: Lookups in subagents
- **WHEN** the main conversation made 5 single lookups and its subagents 300, for $30 of a $200 session
- **THEN** the item counts all 305 requests and their $30

#### Scenario: Few lookups
- **WHEN** 15 single lookups cost $9.00 of a $40.00 session
- **THEN** there is no lookups item, since it takes at least 20 requests

#### Scenario: Cheap lookups
- **WHEN** 200 single lookups cost $3.00 of a $60.00 session, 5%
- **THEN** there is no lookups item

#### Scenario: Unpriced session
- **WHEN** the session's spend is zero because nothing is priced
- **THEN** there is no lookups item

### Requirement: Prompts block
When the main conversation has at least two messages the user typed (see session-logs), the X-ray SHALL print a `prompts` block, `prompts   what each message you typed cost, subagents included`. Each line gives the message's number in the session, its cost, its requests, its label and its two costliest activities (see cost-attribution) with their cost.

#### Scenario: A message and its work
- **WHEN** the third typed message was followed by 74 requests that cost $8.53, $3.10 of them reading and $2.90 editing
- **THEN** its line reads `  3   $8.53   74 req  <label>` followed, dimmed, by `reading $3.10 · editing $2.90`

#### Scenario: What a message's cost covers
- **WHEN** a message is followed by main requests, a subagent started before the next typed message, and the turns of background results
- **THEN** its cost and requests cover all of them: main requests from its time to the next typed message, and every request of the subagent chains whose first request falls in that time
- **AND** requests before the first typed message count with the first

#### Scenario: The label
- **WHEN** a message holds a pasted X-ray, a blank line and then `what else can we analyze?`
- **THEN** its label is `what else can we analyze?`, the last paragraph with its whitespace joined, cut to 40 characters with `…`

#### Scenario: Many messages
- **WHEN** the session has 30 typed messages
- **THEN** the block lists the 8 most expensive in session order, then `<n> more messages` with their cost and requests together

#### Scenario: One message
- **WHEN** the main conversation has one typed message
- **THEN** there is no `prompts` block

#### Scenario: No prices
- **WHEN** nothing in the session is priced
- **THEN** the amounts are tokens, and the most expensive messages are those with the most tokens

### Requirement: Activities in the work block
The work block SHALL end with `by activity: what the agent was doing before each request`, followed by one line per activity of the session, its subagents included, most expensive first, with a bar, its share and its cost. The seventh and later activities fold into one `<n> more` line.

#### Scenario: A coding session
- **WHEN** a session spent $5.42 on requests after reads, $4.71 after edits and $2.14 answering messages
- **THEN** the activity lines start with `reading`, `editing` and `prompt`, in that order, each with its bar, share and cost

### Requirement: Tools block
When the session, its subagents included, made tool calls, the X-ray SHALL print a `tools` block: `tools     calls, sizes, rent and trips by tool (see tokprof tools)`, the 8 rows of the tool breakdown (see tool-breakdown) with the most rent, each with its calls, sizes, rent, trips, single share and hint, then `<n> more` and the total line.

#### Scenario: A session that read code one piece at a time
- **WHEN** the session's top row is `Bash sed -n` with 56 calls, 1.3k-token results, $5.21 of rent and $3.75 of trips, 86% single
- **THEN** the block's first row shows those numbers and the hint `batch independent reads`

#### Scenario: No tool calls
- **WHEN** the session made no tool call
- **THEN** there is no `tools` block

### Requirement: Setup block
When the session's setup cost (see setup-breakdown), subagents included, is at least $1 and at least 5% of its spend, or at least 5% of its tokens when it has no price, the X-ray SHALL print a `setup` block. It starts with `setup     what every request carries before the work, subagents included: <cost>, <share>% of the session`, followed by the setup table.

#### Scenario: A code review with many subagents
- **WHEN** a session's setup cost $73.90 of $526
- **THEN** the block starts with `setup     what every request carries before the work, subagents included: $73.90, 14% of the session`

#### Scenario: A small setup
- **WHEN** the setup cost $0.40, or 3% of the session
- **THEN** there is no `setup` block

### Requirement: Setup table
The setup block SHALL hold a table with one row per setup source, most expensive first: `source`, `cost`, `subagents` for the part of the cost in subagent chains, `main starts` with the source's tokens in what the main conversation starts with, and `each subagent` with the mean tokens of what the subagents start with (see setup-breakdown). Up to 8 sources are rows; the rest fold into `<n> more`. A `total` row closes the table.

#### Scenario: Totals of a code review
- **WHEN** the setup cost $73.90, $70.23 of it in subagents, and 96 subagents start with 31.2k tokens on average, instructions 9.7k and skills 9.6k, with a mean brief of 1.8k
- **THEN** the `total` row shows $73.90, $70.23 and 31.2k under `each subagent`, followed by `96 subagents on average, and a 1.8k brief`

#### Scenario: No subagents
- **WHEN** the session has no subagent with a known first prompt and no setup cost in subagent chains
- **THEN** the table has no `subagents` and no `each subagent` column

#### Scenario: Unknown start
- **WHEN** the log does not show the main conversation's first prompt
- **THEN** `main starts` shows `–` in every row

#### Scenario: After a compaction
- **WHEN** the main conversation has a compaction whose next prompt the log shows (see setup-breakdown, What a compaction leaves)
- **THEN** a line after the table reads `after the compaction at <MM-DD HH:MM>, the next prompt held <size>:`, followed on the next line by its 4 largest parts by content kind or reminder source, each with its tokens, then `<n> more <tokens>`

#### Scenario: The hint
- **WHEN** the costliest setup source has a hint from the agent's log source (see setup-breakdown, Setup hints)
- **THEN** the block ends with `▸ <Source> cost the most of the setup: <hint>.`, wrapped like an advice item, as in "▸ Skills cost the most of the setup: lower skillListingBudgetFraction (default 0.01 of the context window), or turn unused skills off with skillOverrides in settings.json."
- **AND** with no hint for that source there is no such line

### Requirement: Rhythm line
Below the `time` line of the header, the X-ray SHALL print a `rhythm` line with the session's turns as work-rhythm counts them: how many, how long the agent worked, the medians of its turns and of the user's answers, the turns it started itself, the interrupts and the questions.

#### Scenario: Rhythm
- **WHEN** a session had 34 turns with 2.1h of work, the agent's turns took a median 3.5m, the user answered in a median 4.0m, the agent started 12 turns itself, and the user stopped it twice
- **THEN** the line after `time` reads `rhythm    34 turns, 2.1h of work · the agent's turn median 3.5m · your answer median 4.0m · 12 it started itself · 2 interrupts`
- **AND** a part whose count is zero or whose median has no values is left out
- **AND** the day starts as work-rhythm says, from the session's own actions

#### Scenario: No turns
- **WHEN** a session has no turn, as a subagent's log alone or a session that a program ran
- **THEN** there is no `rhythm` line

### Requirement: Waits block
When the session's agent was busy for a minute or more (see agent-time), the X-ray SHALL print a `waits` block after the `work` block. It starts with `waits     busy <time>: what the agent waited on in its turns, and while its own background work ran after them`. One line per kind of busy time follows, most time first, with a bar, its share and its time. The seventh and later kinds fold into one `<n> more` line.

#### Scenario: The lines
- **WHEN** the agent was busy 6.1h, 1.5h of it for subagents, 1.1h of that after the end of a turn
- **THEN** the block starts with `waits     busy 6.1h: …` and has a `subagents` line with a bar, `24.6%`, `1.5h` and the note `1.1h after the turn ended`

#### Scenario: Notes
- **WHEN** the block has the kinds `model` and `API retries`
- **THEN** the `model` line ends with the number of main requests, as `412 requests`
- **AND** the `API retries` line ends with the number of requests that waited and the last error that most of them had, cut to 50 characters, as `6 requests, most often 502 local proxy: upstream request failed`

#### Scenario: A short session
- **WHEN** the agent was busy for 40 seconds
- **THEN** there is no `waits` block

#### Scenario: Durations
- **WHEN** a time is shown in the block
- **THEN** it reads like the rhythm's: `45s` under a minute, `6.5m` under 10 minutes, `34m` under an hour, `1.8h` under a day, and `6.9d` beyond

### Requirement: Slow requests line
The waits block SHALL go on with a line on the slow requests of the session, its subagents included (see agent-time, Slow requests): how many there were, how many of them were in the main conversation, and how long the longest one took and how many tokens it wrote.

#### Scenario: Slow requests
- **WHEN** 103 requests took a minute or more, 17 of them in the main conversation, and the longest took 6m 12s with 41.2k output tokens
- **THEN** the line reads `  slow: 103 requests took a minute or more, 17 of them in the main conversation; the longest took 6.2m and wrote 41.2k tokens`

#### Scenario: No slow request
- **WHEN** no request took a minute or more
- **THEN** the line reads `  slow: no request took a minute or more`

### Requirement: Longest waits line
The waits block SHALL end with the three longest waits of the session (see agent-time, Waits): each with its time, its kind and its start as `MM-DD HH:MM`, joined by ` · `. With no wait there is no such line.

#### Scenario: The line
- **WHEN** the longest waits are 1.8h for background commands from 2026-10-14 10:12, 25m for subagents from 2026-10-12 20:59 and 14m for background commands from 2026-10-12 20:40
- **THEN** the line reads `  longest waits: 1.8h for background commands at 10-14 10:12 · 25m for subagents at 10-12 20:59 · 14m for background commands at 10-12 20:40`

#### Scenario: One wait
- **WHEN** the session has one wait
- **THEN** the line names that one

### Requirement: Failures block
When a call of the session, its subagents included, failed, or the user refused or a rule denied one (see tool-breakdown), the X-ray SHALL print a `failures` block after the `tools` block: `failures  calls that failed, and what the requests after them cost until the tool worked again`. Up to 5 rows follow, most `after` first and then most `failed`, with `tool`, `failed`, `of calls`, `after` and `latest error`. Then come `<n> more` and the failures line (see tool-breakdown).

#### Scenario: A tool that never worked
- **WHEN** `advisor` failed in 15 of 15 calls, its latest error line was `Error: No such tool available: advisor`, and its windows cost $32.92
- **THEN** its row reads `advisor`, `15`, `100%`, `$32.92` and `Error: No such tool available: advisor`

#### Scenario: A long error line
- **WHEN** the latest error line of a row is longer than 60 characters
- **THEN** it is cut to 60 characters with `…`

#### Scenario: Refusals only
- **WHEN** no call failed and the user refused 2 calls
- **THEN** the block has no rows, and its only line after the title is `  you refused 2`

#### Scenario: No failures
- **WHEN** every call of the session worked
- **THEN** there is no `failures` block

#### Scenario: The tools block
- **WHEN** the X-ray prints the `tools` block
- **THEN** its rows keep the columns that block names, without `failed` and `after`

### Requirement: Requests list
`tokprof scan [target] --requests` SHALL print, in place of the X-ray, the requests of the target's main conversation: the main log's main chains and advisor calls, in time order, for the session `tokprof scan` would pick. The head is `tokprof · requests of the main conversation`, the X-ray's `session` line, and a `requests` line with the count, the cost, and the cost of each of the X-ray's five spend parts that is not zero. Without `--requests`, `tokprof scan` prints the X-ray unchanged.

#### Scenario: The totals
- **WHEN** the X-ray's spend line reads `main $83.55 · subagents $6.87`
- **THEN** the `requests` line gives `$83.55`, and the costs of the rows add up to it
- **AND** in a session with advisor calls, the line gives the sum of `main` and `advisor`

#### Scenario: The plain X-ray
- **WHEN** the user runs `tokprof scan 230043e6`
- **THEN** the output is the X-ray, byte for byte as before this option

#### Scenario: No requests
- **WHEN** the session has no request with usage
- **THEN** the head ends with `requests  0 requests`

### Requirement: Request rows
Under a header, the list SHALL print one row per request: `#`, the local time `MM-DD HH:MM:SS`, the cache read, cache write, uncached and output tokens as the log gives them, the context, the cost and the why. Without prices, the cost is in tokens.

#### Scenario: A miss
- **WHEN** request 57 at 11-03 14:00:12 read 12.3k, wrote 490.3k after 1.0h idle on a 1h cache, sent 3 uncached tokens and 1.2k of output, and cost $3.95 with $3.82 avoidable
- **THEN** its row reads like `    57  11-03 14:00:12   12.3k  490.3k         3    1.2k  502.6k   $3.95  missed: idle > TTL: 1.0h idle, 1h TTL · $3.82 avoidable`

#### Scenario: A growing context
- **WHEN** a request without a miss has a context 5.1k larger than the previous request's
- **THEN** its why reads `context +5.1k`

### Requirement: Why of a request row
A row's why SHALL be the first that applies: `missed: <cause>: <detail>`, with `· $<n> avoidable` when priced, for an unexpected miss; `rebuilt: <cause>: <detail>` for an expected rebuild; `first request` for the first request of a chain; `context +<n>` or `context −<n>` for a change in size from the previous request; else nothing.

#### Scenario: A rebuild
- **WHEN** a request rebuilt its prompt after an automatic compaction
- **THEN** its why reads `rebuilt: compaction: auto`

### Requirement: Entered and changed lines
Under a row, an `entered` line SHALL give what entered the context since the previous request of its chain, grouped by kind, tool and target, largest first: up to 3 groups, then `<n> more <tokens>`. It is left out when nothing entered or the log does not show the prompt. A `changed` line follows when the model, the effort, another setting the log names, the agent's version or the tool set changed, or a compaction came between.

#### Scenario: What entered
- **WHEN** a tool result of `Bash cargo test` of 6.1k tokens, 400 tokens of assistant text, the call of 200 tokens and two more blocks of 300 tokens entered before a request
- **THEN** the line under its row reads `        entered  6.1k tool results Bash cargo test · 400 assistant text · 200 tool calls Bash cargo test · 2 more 300`

#### Scenario: A change
- **WHEN** the effort went from `high` to `max` and two tools were added since the previous request
- **THEN** the line under its row reads `        changed  effort high → max · tools +2 −0`

### Requirement: Prompts the log does not show
When the log does not show what some prompts of the list held, a line after the head SHALL count them, and their rows SHALL have no `entered` line.

#### Scenario: Prompts not in the log
- **WHEN** 40 requests come from a Claude Code transcript that does not show what the prompts held
- **THEN** a line after the head reads `          the log does not show what 40 prompts held`, and those rows have no `entered` line
