# Design

## Context

`add-request-trigger` gave every request a trigger. Sources map each tool call to one of four call kinds (read-only, edit, subagent, run), and the core picks the trigger from the calls of the previous response. Requests are summed by origin, task and trigger in the work tables, which the summary and the X-ray share.

The X-ray learns about requests through the analysis sink. A request record carries the origin, task, trigger, model, price and usage, but not the time or the chain name. The session model keeps message sizes, not text.

Turns were fixed just before this change. Claude Code `system` blobs had counted as typed messages, so a 6-message session showed 39 turns. Since the fix it shows 5. The sixth message, which had an image, is an unresolved message.

## Goals / Non-Goals

**Goals:**
- Activities come from the same decoding pass and the same calls as triggers.
- The prompts block needs no second read of the log.
- A typed message's label is its own words, kept short.

**Non-Goals:**
- Keeping whole message texts in the session model.
- Resolving the messages the request-audit lines do not resolve, such as messages with images.

## Decisions

1. **Call kinds become call classes.**
   - The four kinds become nine classes: reading, web, editing, subagents, building and testing, version control, running scripts, file and system commands, other tools.
   - The trigger maps them back: reading and web are read-only, editing is an edit, subagents is a subagent, the rest is a run. Triggers stay as they are, with one exception: a shell command that writes a file, such as `cat > f <<'EOF'`, was `run` and becomes `edit`. On 30 days of Claude Code logs this moves 2.7k requests and $310 from `run` to `edit`.
   - Sources classify tools by name. A shell command goes to one core function, which first applies the read-only rule, then the program lists.
   - The lists are shell vocabulary, so they live in the core next to the read-only rule. The architecture test forbids agent and tool names there, not program names.
   - Alternative: a separate classification pass in the core over prompt blocks. Rejected, because blocks keep only the command head.

2. **The request keeps its call class.**
   - Sources set the class of the calls that gave the trigger, using the same precedence. The core's activity is that class, or the trigger's name when no call gave the trigger.
   - `advisor` and `compaction` override both trigger and activity, as they already override the trigger.

3. **Typed messages are a list on the session.**
   - Each entry holds the time and the label: the last paragraph, with its whitespace joined.
   - The label is stored cut to 200 characters, and the X-ray cuts it further, to 40 characters.
   - Claude Code takes it from main transcript `user` lines whose event is `prompt`. A slash command is written as its name and arguments.
   - Codex takes it from `response_item` user messages with prompt text outside `compacted` histories, which repeat old messages.
   - Subagent sessions keep none.

4. **Request records gain time and chain.**
   - The X-ray's sink keeps one small row per request: chain name and kind, time, cost, tokens, activity.
   - The prompts block assigns main-chain requests to the latest typed message at or before them.
   - A subagent or side chain goes, whole, to the message before its first request.
   - Rows are only kept in the X-ray's sink, not in the summary's, so a full-history summary holds no per-request data.

5. **Call counts come from the session's blocks.**
   - The X-ray counts tool-call blocks by tool and target over the session and its subagent logs.
   - Messages are deduplicated by hash, so each call counts once however many prompts held it.

Log fields: the same as `add-request-trigger`, plus:
- Claude Code: `<command-args>` and the `role` of `api-request-blob` messages, seen in 2.1.300–2.1.312;
- Codex: `message` text of role `user`, seen in 0.146–0.164.

Parse time: classifying a call becomes a lookup in a few short lists, and typed messages are a few per session. The effect on the ~24 s full parse should be under 1%.

## Risks / Trade-offs

- **[`file and system commands` stays big, because project tools are called by path]** → A path counts as running scripts. Tasks include a pass over the 100 most frequent first programs in the last 30 days, before the lists are frozen.
- **[The last paragraph is not the request, for example a message that ends with a pasted log]** → The label is a hint next to the number and the cost, and it is cut to 40 characters. The full text is in the log.
- **[A subagent started in one message runs on after the next message]** → Its whole chain counts with the message that started it. That is the message that asked for the work.
- **[An unresolved typed message, such as one with an image, is still listed]** → The list comes from transcript lines, not from rebuilt prompts, so the message is there with its label. Its content stays `unknown` in `in context`.

## Open Questions

None.
