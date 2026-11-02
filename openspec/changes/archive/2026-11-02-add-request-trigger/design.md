# Design

## Context

Today a request carries its origin and task, which the decoders set from the log, and flags the core reads: `compacted_before`, `after_assistant_message` (Codex) and `advisor`. Spend is charged to prompt blocks by position. A tool call's block keeps only a short target, the first words of a shell command, so the full command is gone after decoding.

`split-core-and-agent-adapters` moves agent-specific logic behind one adapter interface. It adds neutral tool classes after ACP's tool kinds: `Execute`, `ExecuteOutput`, `Read`, `Fetch`, `Search`, `Edit`, `Write`, `Delete`, `Move`, `Think`, `SwitchMode` and `Other`. It keeps Codex `apply_patch` and the subagent tools in `Other`, so that today's hints do not change. This change starts once the split has landed.

## Goals / Non-Goals

**Goals:**
- The trigger is set during the one decoding pass. It needs no second read of the log.
- The trigger works for transcripts without request-audit lines. It needs only the order of messages, not the rebuilt prompt.
- One read-only rule for every agent, in the core.

**Non-Goals:**
- Changing the tool classes of the split or any existing hint.
- Keeping full commands in the session model.

## Decisions

1. **Decoders classify calls, the core picks the trigger.**
   - Each decoder already sees the full input of every call.
   - It maps each call to one of four call kinds: read-only, edit, subagent and run. It uses the neutral tool class and its own tool names. Codex `apply_patch` is an edit; Claude Code `Agent` and `Task` and Codex `spawn_agent` are subagents.
   - A core function turns the calls of the previous response into `edit`, `subagent`, `run`, `lookups` or `lookup`, in that order of precedence.
   - The decoder also says whether a typed prompt, a notification or an interruption came after those calls; the latest wins.
   - The core then applies the flags it already has: `advisor` first, then `compaction` from `compacted_before`.
   - Codex `continuation` is decided by the decoder: the previous response produced an assistant message, no call, and nothing was typed after it.
   - Alternative: compute the trigger in the core from prompt blocks. Rejected, because blocks keep only the command head, and transcripts without request-audit lines have no blocks.
   - Alternative: add a `Subagent` tool class to the split. Rejected, because it would change the split's mapping, which must reproduce today's hints exactly.

2. **The read-only rule is shell syntax, so it lives in the core.**
   - The core splits a command at `&&`, `||`, `;`, `|` and line breaks, and skips `cd` parts and `NAME=value` prefixes.
   - It checks the first word of each part against the spec's list. `sed -i`, `find -delete|-exec|-execdir` and git subcommands outside the list fail.
   - Any `>` or `>>` to a target other than `/dev/null` and `&1`/`&2` fails the command. So do `tee`, `<<`, `$(` and backquotes.
   - The scan ignores quoted text when it looks for separators and redirects, so `grep -n 'a|b'` stays one part.
   - It is conservative: what it cannot read is `run`. A wrong `run` makes the lookup figure smaller, and the advice says less than it could. A wrong `lookup` would advise batching a write.

3. **Codex `exec` code is read by its inner calls.**
   - The Codex decoder already lists `tools.<name>` calls in the JavaScript of an `exec` call. Here it counts every call, not each name once.
   - For `tools.exec_command` it takes the string literal after `cmd:`. A call whose command is not a plain string literal is `run`.
   - The number of inner calls tells `lookup` (one) from `lookups` (more).

4. **Claude Code groups calls by response.**
   - Parallel calls of one response arrive as several `tool_result` user lines. The request after the last of them is the one they triggered.
   - The decoder keeps the call kinds of the chain's latest response, by `message.id`, and resolves them when the next request of that chain appears.
   - A typed prompt, a `<task-notification>` turn and an interruption (`[Request interrupted`) are read from the user lines in between. These are the same prefixes the origin rules already match. The latest of them wins, and the calls count only when none came after them.
   - A failed request has no usage and makes no request in the model, so the trigger waits for the retry.
   - The first request of a subagent chain is `prompt`.

5. **Aggregation reuses the origin path.**
   - The summary's work tables already sum whole requests by origin. A trigger map sits next to them, with request count, prompt tokens and cost.
   - The X-ray already sums the session with its subagents for the thinking advice. The lookups rule reads the same sums, filtered to `lookup`.
   - `trigger` joins the dimension list. The profile-export error message lists every name, so it picks the new one up.

Log fields this depends on:
- Claude Code: `message.id`, `tool_use` `name` and `input.command`, and `tool_result` lines; user text prefixes `<task-notification>` and `[Request interrupted`.
- Codex: `function_call` and `custom_tool_call` `name`, `arguments` and `input`; `token_usage_record` order.
- These were seen in Claude Code 2.1.284–2.1.312 and Codex 0.146–0.164.

Parse time: one classification per tool call, a scan of the command string. The 30-day corpus has about 83k Claude Code calls. This adds well under 1% to the ~8 s full parse.

## Risks / Trade-offs

- [The read-only list misses common commands, so lookups are undercounted] → Tasks include a pass over the 200 most frequent command heads in the author's logs. The unit tests take real commands from them.
- [A trigger is not waste: dependent lookups cannot be batched] → The advice says "independent reads" and promises no saving. The table is labeled "what made the agent send each request".
- [Claude Code changes how parallel results are logged] → Grouping uses `message.id`, which origins and outputs already rely on. A request with no previous response in the chain is `unknown`, not a guess.
- [Codex JavaScript in `exec` is free-form] → Only string-literal `cmd:` values are read; everything else is `run`.
- [The X-ray block grows by an item] → The item wraps to at most two lines, and it fires only at $5 or 10% over 20 requests.

## Open Questions

- Codex tools that wait for a spawned thread are not in the author's recent rollouts. They count as `run` until a log shows their names. Adding them later is a new name in the subagent scenario.
