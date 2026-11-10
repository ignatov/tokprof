# Design

## Context

- **Turns.** work-rhythm already builds turns from each session's timeline: start, end, who started it, and whether it ended logged, interrupted or unlogged. A timeline holds only times of events, not the requests or their calls.
- **Requests.** Requests carry their usage, price, activity and trigger. Their calls carry outcomes (failed, refused, denied) and version-control actions (commits).
- **Assistant text.** The model keeps content blocks for sizes and kinds. It does not keep the assistant's text, and it must not start keeping it: the share file and every view stay free of message text.

See proposal.md for why.

## Goals / Non-Goals

**Goals:**
- One pass that gives every turn its requests, dollars, calls, end kind and what came next. The X-ray, `pauses`, `share` and `pool` all read from this one pass.
- The same turn boundaries as the rhythm, so the counts agree with the `stops` line and the hand-off parts.

**Non-Goals:**
- Splitting a turn's spend by what happened inside it, such as before and after a question.
- Following a turn across sessions, such as a handoff into a new session.

## Decisions

1. **Requests join turns by time, not by chain links.**
   - A main request belongs to the turn whose interval holds its time. A subagent request of the same session goes the same way.
   - Alternative: follow each subagent to the call that started it. It is more exact for a subagent that outlives its turn, but it needs the call-to-agent link per source, and the time rule already matches how agent-time counts busy time.
   - A subagent request between turns belongs to no turn, and `pauses` counts it in the total, so the sums still close.
2. **End kinds come in a fixed order of precedence**: `stopped`, `failed`, `asked`, `waiting`, `committed`, `changed`, `answered`.
   - The person's act comes first.
   - Then come the agent's two reasons to wait for the person, a failure and a question, and then its wait for its own background work.
   - Then what the turn left behind: a commit says the work reached a checkpoint, an edit without one says it waits for a look.
   - Alternative: several kinds per turn. That makes the shares overlap and the table harder to read.
3. **`asked` is one flag that a small rule sets from the last paragraph; the core never sees text.**
   - The source passes the last paragraph of the response that ended a turn to one shared rule in tokprof-logio, and keeps only the answer. Both sources use the same rule, and no agent name or log field is in it.
   - The rule is four regular expressions over the last paragraph: a `?` in the last three sentences outside a link, words of permission, request words in the last three sentences, and offers in the last two (see turn-outcomes, Words that ask and Words that offer).
   - It was built on the author's 30 days of Claude Code (2,486 logged ends):
     - On 146 hand-labelled ends, it found 39 of 41 asks with 1 false. This sample is the one the rules were tuned on.
     - On a fresh holdout of 76 ends, it found 20 of 25 asks with 3 false: precision 0.87, recall 0.80.
     - Ending with `?` found 14 of 25 there with no false, a recall of 0.56.
     - The misses are requests without a request word ("утром нажми …", "две вещи ждут твоего ответа"). The false hits are request words that speak of a third party ("ждут твоего одобрения" about another session).
   - Over all ends, the rule marks 27% as asking, against 13% for a trailing `?`.
   - Questions and offers are one kind. Both leave the next step to the person, and on the labelled sample the rule told them apart only half the time.
   - Alternatives:
     - classifying with a model: rejected, because tokprof is local only and never guesses;
     - the trailing `?` alone: rejected, because it misses about half of the asks;
     - only the question tools: rejected, because most asks are in the text.
   - `waiting` comes from the timeline, not the text: the next hand-off is a wake. On the labelled sample this has precision 0.86 and recall 0.81 for "the agent waits for its own work". A wake is the one sign that does not depend on the language.
4. **What came next uses five ranges** (`under 1m`, `1–5m`, `5–15m`, `15–60m`, `1h+`) and `none`.
   - The pause report has ten ranges, too many for one row per end kind.
   - Views fold them further: `pauses` into `under 5m`, `5–60m`, `1h+` and `none`, and `pool` into `under 5m` and `none`.
   - A session counts as open for one hour after its last request, so a turn the person is still reading is not called a session end.
5. **Stopped turns count as `spent`, not `avoidable`.**
   - A stopped turn may have done useful work before the stop, and the person may keep its edits. So its dollars say what the stopped work cost, like the other `spent` pains.
6. **`quality` gets `interrupts` as its pain.**
   - A stopped turn is the one measured sign that the agent did work the person did not want. It is weak evidence, and the note of `What people said` already says what a pain touches means.
   - Failures stay a `cost` pain, as now.
7. **Run length is measured per edit call, not per commit.**
   - Commits often land in a short follow-up turn ("commit it"). The prototype's cost per commit made long turns look 1.6–2.2× worse for that reason.
   - Edit calls are counted in the turn that made them.
   - Only turns the person started count: a wake turn is the agent's own continuation and is short by nature.
8. **Goal runs come from the request's origin.**
   - cost-attribution already gives a typed slash command the origin `command:/goal`. A turn whose first request has that origin is a goal run.
   - Codex has no such command, so it has no goal runs.
9. **The share format stays 3.**
   - `turns` is a new part. Older files lack it, and the pool already prints `in <k> of <n> files` for a part some files lack.
   - Its `usd` keys become shares through the existing key rule, and its counts and `*_ms` stay.

**Log fields and versions:**
- **Claude Code 2.1.310–2.1.316:**
  - `message.stop_reason` (ends);
  - user lines starting `[Request interrupted` (stops);
  - assistant `message.content[].type == "text"`, only its last character after trimming (the flag);
  - `tool_use.name`, and `tool_result.is_error` with the refusal texts of session-logs (failed);
  - `AskUserQuestion` and `ExitPlanMode` (asked).
- **Codex 0.147–0.166:**
  - `event_msg` `task_complete` and `turn_aborted` (ends and stops);
  - `response_item` `message` with role `assistant` and its `output_text`, last character only (the flag);
  - `function_call` and `custom_tool_call` outcomes as session-logs reads them;
  - `apply_patch` for edits.

**Parse time:**
- Turns and requests are both in time order per session, so two pointers join them in one pass, O(requests).
- The flag costs one trim of each response's last text block, which the decoders already walk for sizes.
- Expected under 1% of the ~8 s over the author's ~8.5 GB, to be measured in the tasks.

## Risks / Trade-offs

- **The text rule knows Russian and English only, and was tuned on one person.**
  - Other people write other prompts, and the agent answers in their style. A pilot file can't show the error, since it holds no text.
  - Mitigation: the rule is in one place with its word lists. Before wave 1, check 40 fresh ends of the author's Codex sessions by hand. Keep the trailing `?` count beside it in `pauses`, so a big gap between the two shows.
- **Unlogged ends.** Some sessions log no end for a turn, as in some Air sessions. The prototype found 3% of the turns the person started ending this way.
  - Such a turn gets its kind from its calls like any other, since `stopped` needs a logged interrupt.
  - Mitigation: `pauses` prints how many turns had no logged end under the first table.
- **Time-joined subagents.** A subagent that runs on after its turn ended goes into no turn, or into a later turn of the same session.
  - Mitigation: the `All turns` check in turn-outcomes catches a gap. The share of such requests is printed in the apply notes.
- **A person's long run may simply be a big task.** Cost per edit call by length is not causal.
  - Mitigation: `Run length` only lists the numbers. Advice stays out of scope until a pool shows the same shape across people.

## Migration Plan

No migration is needed. Old share files keep working, and the new sections show `in <k> of <n> files` until people run v0.1.5 or later. Rollback is reverting the change. The format does not move.

## Open Questions

- Should `pauses` also print the end kinds per client (Air against CLI)? The share part could carry it later without changing these specs. It is left out until the first wave shows whether clients differ.
