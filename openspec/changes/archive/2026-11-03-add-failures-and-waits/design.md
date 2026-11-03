# Design

## Context

See proposal.md for why. What the code has today:
- A request has one time, `ts_ms`. Claude Code takes it from the `api-request` line when there is one, else from the first line of the response. Codex takes the time of its usage line. The cache rules (idle gaps, TTL) read this time, so its meaning must not change.
- A tool result is a block with its tool, target, class and size. The source does not record whether the call failed.
- Hand-offs say that a wake happened, but not what it reports.
- The rhythm (work-rhythm) builds turns from hand-offs and request times. Since add-browser-day-view it also cuts days at nights; this change reads only its turns.
- The analysis feeds sinks with each request, each fact, each tool block the first time a request holds it, and each share of spend charged to a tool. The tool table is such a sink.

Constraints:
- Only a source may name its agent's fields, tools and texts (`crates/tokprof/tests/architecture.rs`). The core gets neutral values.
- The library reads no clock. The X-ray and `pauses` already parse the logs they need, subagents included.

## Goals / Non-Goals

**Goals:**
- One set of rules for busy time, waits and slow requests, used by the X-ray and by `pauses`.
- Failures and their windows from the same analysis pass that builds the tool table, with no second read of the logs.
- On 3edfd2e2, waits that agree with the log where CodeBurn's labels do not (proposal, How it is checked).

**Non-Goals:**
- Changing what `ts_ms` means, or any cache or cost number.
- Tracking each background command from its launch to its end. A wake after an end of turn tells which kind of work the agent waited for, which is what the views show.

## Decisions

1. **New neutral fields, old ones untouched.**
   - A request gets three optional times: the send of its first attempt, the send of its answered attempt, and the end of its response. It also gets its number of failed attempts and the last error text. A request kept without a response, as a retried or cancelled attempt is kept today, gets its send too.
   - A tool result block gets an outcome: worked, failed, refused or denied. Failures are about 1% of calls, so their error lines go into a list on the session, keyed by block, rather than into every block.
   - A wake gets what it reports: a subagent, a background command, or something else.
   - Rejected: re-defining `ts_ms` as the send. Codex and the cache rules read it as the time of the response.
2. **Claude Code, seen in 2.1.284–2.1.316.**
   - Sends:
     - with request-audit lines, the `api-request` line's `timestamp`;
     - without them, the latest `user` line of the response's chain, as `(agentId, isSidechain)` or the subagent file. `attachment` lines are skipped, since Claude Code stamps some of them with the response.
   - End: the latest `timestamp` among the assistant lines of the response's `message.id`, never before the send. The study found 670 ends before their start. All of them used a previous line as the start, and only 5 were off by more than a second.
   - Retries:
     - A `system` line of `subtype` `api_error` marks the latest `api-request` of the same `agentId` as a failed attempt. When the next `api-request` of that stream gets a response, the first attempt's time and the count go with it. The error comes from `error.formatted`. Seen in 2.1.287–2.1.316.
     - In a transcript without `api-request` lines, the `api_error` lines between a send and a response are its retries. The answered attempt went at the last error's `timestamp` plus its `retryInMs`.
     - The line filter lets `"api_error"` through.
   - Outcomes:
     - `tool_result.is_error` marks a failure.
     - The refusal texts and `permissionDecision.decision` `reject` on the `user` line (seen in 2.1.300–2.1.316) turn it into a refusal or a denial.
     - The texts live in the source's tools module.
   - Wakes: the text of `<summary>` in a `<task-notification>`. `Agent "` is a subagent; `Background command "` and `Monitor ` are a background command. Seen in 2.1.291–2.1.316.
3. **Codex, seen in 0.147–0.166.**
   - Each history item keeps the time of its line. Items replayed from a compaction have none.
   - A request's end is the time of the last item its response added; its send is the time of the item before those.
   - The study found that `token_count` comes after the outputs of the tools the response called in 86% of responses. Its time would add tool time to the model's.
   - A `custom_tool_call_output` that starts with `Script failed` failed. So does one whose text items hold a JSON object with an `exit_code` other than 0; only items that contain `"exit_code"` are parsed.
   - A tool output that starts with `aborted by user` is a refusal.
   - Codex logs no retries and no wakes.
4. **Busy time is painted, not summed.**
   - Per main session, the rules collect intervals and paint them in the order of agent-time: model, API retries, subagents, the stretches between requests, background commands, nothing. The busy set is the main conversations' turns without question waits, plus, after an end of turn, the subagents' work and the stretch to a wake that reports a background command.
   - A sweep over the sorted interval ends gives each kind its time, the part after ends of turn, and the waits. Cost: O(n log n) in requests and hand-offs.
   - Inputs: the turns of work-rhythm (`Timeline` and `Lane`), the main requests with their times, triggers and activities, and the first and last request times of each subagent log.
   - Rejected: CodeBurn's rule of gaps of 60 seconds or more between steps, labelled by what started the next step. It does not know what ran in the gap. Of the 5 waits it called "waiting on helpers" in 3edfd2e2's prompt 2, 4 were test runs in the background, and it called two more waits for background commands "nothing running".
5. **Between requests, the next request's activity decides.**
   - cost-attribution already gives each request the activity of the calls before it, for both agents and for Codex's code-mode `exec` with its inner calls. The stretch from a response's end to the next send goes to that activity. Time and money are then shown in the same categories as the X-ray's `work` block.
   - Rejected: a fixed split into commands and other tools by the tool's class. Codex code mode runs 3,799 of its calls as `exec` with several inner tools, so their class does not say whether they ran programs.
6. **Failure windows in the tool table.**
   - The tool table sink learns each first-time tool result with its outcome, in chain order, and each request's spend. Per chain and row it keeps an open window. A failed result opens one, or extends one that is open. The spend of the requests in it is held back. A worked result of the row closes it with all its requests. An end of turn or the chain's end closes it with only its first request.
   - Each closed request is remembered by chain and index, so the failures line counts it once.
   - The table gets `failed` per row and counters for refused and denied calls.
7. **Where each view gets its data.**
   - X-ray: from the session and subagents it already parses. The `tools` block keeps its columns; the `failures` block is new.
   - `pauses`: the scan already groups subagent logs under their main session for "Sessions by largest context". It also computes busy time per main session, then adds it up per agent.
   - `tokprof tools`: from the same tool table.

## Risks / Trade-offs

- [Permission prompts are not logged] → a wait for one counts as its call's time, as the proposal says.
- [Claude Code changes the summary texts of notifications or the refusal texts] → the README of the source names them with versions. The fixtures hold one line of each, and the conformance test runs over them.
- [An `api_error` of one stream lands between two streams' requests] → it is paired with the latest `api-request` of its `agentId`, which in the logs comes 5 ms before it. Two streams of one agent that interleave within that time are rare.
- [A background command whose notification never came, as when the session ended] → the stretch after the end of turn is not busy. The study found 8 such tasks.
- [The windows of a test run are development, not waste] → the views say "until the tool worked again" and never call the money lost.
- [A Codex `exec` that prints only its output hides the exit code] → such a call counts as one that worked. The study could not measure how often this happens.
- [A session that runs for days has many hand-offs] → the sweep is linear after one sort. 3edfd2e2 has 1,333 requests and a few hundred hand-offs.

## Parse time

- Claude Code: one more substring check per line (`"api_error"`), a timestamp parse per `user` line, a look at `is_error` and `permissionDecision` on lines that are already parsed, and error lines only for the 1% of results that failed. Under 1% of the parse of the corpus.
- Codex: the item times are parsed already. A substring check for `"exit_code"` runs on tool outputs only.
- Core: busy time is linear in requests after a sort, about 81k requests for 30 days. The windows add a hash lookup per tool result.
- Expected: `tokprof --since 30d summary` within 3% of today's time, the X-ray of 3edfd2e2 within 0.1 s, and `pauses` within 5%.
