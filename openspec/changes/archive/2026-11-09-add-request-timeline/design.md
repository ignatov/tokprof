# Design

## Context

The X-ray (tokprof-core) runs the cost pass over the session's logs. The pass walks each chain's requests and rebuilds each prompt from the log's messages, blocks and prompt deltas. It judges each request against the previous one of its chain for a miss, splits the request's input into read, write, rewrite after miss and uncached slices, and hands requests, misses and facts to a sink. The X-ray's sink keeps one row per request, but for the window it keeps only the time, the prompt size and the cost of the main requests. The pass already knows, at each request, which blocks entered the prompt for the first time and their size: the share of the prompt their text takes. It reports only tool blocks out of that, for the tool table.

See proposal.md for why.

## Goals / Non-Goals

**Goals:**
- One record per main request, built in the same pass: its parts, miss, entered blocks, changes, failures and commits. The terminal and the window both read it, so they give the same numbers.
- Parts that add up to the request's cost, and over the session to the X-ray's spend by component of the main chain.
- No extra parse and no work for the other reports.

**Non-Goals:**
- Diffing prompt texts, resending requests, or putting the subagents' requests on the timeline.
- The terminal ui.

## Decisions

### The pass hands each request's entered blocks to the sink
The sink gets a new call after each request's attribution. It receives the session, the request, the prompt shape in force, the previous request the miss check compared with, and the entered blocks with their tokens, or none when the prompt is unknown. Its default does nothing, so the other reports pay one empty call per request.

*Alternative:* rebuild prompts again in the X-ray for the main chains. It was rejected: it would duplicate the rebuild and could disagree with the pass.

### The parts of a request
The five parts follow the pass's own slicing:
- read is the cache read;
- the re-processed tokens of a verdict are taken first out of the cache write, then out of the uncached input, as rewrite after miss;
- the rest of the write is cache write, at the request's mix of 5-minute and 1-hour prices;
- the rest of the uncached input is uncached input, and the output is output.

A unit test holds their sum to the price of the usage, and holds the session's sums to the X-ray's components for the main chain.

### What the record keeps
- **Entered:** grouped by kind, tool and target, sorted by tokens, the first 12 kept and the rest summed. That bounds the memory: 7,015 requests × 12 groups is under 1 MB.
- **Changes:** from the previous request the miss check used:
  - model;
  - effort, and the first other setting that changed;
  - agent version;
  - the tool names of the shapes, added and removed;
  - the system prompt's characters;
  - a compaction between, from the request's flag and the kind of the latest marker in the gap;
  - the gap against the cache lifetime of the request's API for the previous request's write kind.
- **Failures:** a running count of failed results in the main conversation. A worked result resets it, and a refused or denied one leaves it. The request whose entered results take it to 3 gets the streak mark.
- **Commits:** the calls of the response the request produced, when their command committed and the call worked.
- **Session marks:** the main log's typed messages, interrupts (the hand-offs of kind interrupt) and compactions. The subagents' spans come from the first and last request of each subagent chain, with its task.

### The timeline in the window
- **The chart is one canvas.** The columns are fills, the context line is a stroke, and the markers are thin elements. Columns are bucketed into 4 pt slots on the axis. A slot's parts are the sums of its requests, and a click picks the costliest one in it.
- **Hover, click and drag come from one element.** It takes pointer moves and drags in its own points. A drag of less than 4 pt is a click. The hover card is drawn by the tab, not by a system tooltip, so it can hold rows.
- **State lives in one struct of the desk:** the session, the selected request, the range, the hovered slot and the unit. A selection of another session's X-ray is ignored, so the tab opens on the costliest request without resets in the list's code.
- **The cards stay put.** The timeline and its card sit above the inspector's scrolling pane on this tab only. The table scrolls under them, and the existing row cursor is the selected request's row.
- **Keys:**
  - `Requests` takes ← →, Shift+↑ ↓ and `t` before the inspector's own keys, as `Flame` does.
  - Shift+← → still change the tab.
  - esc clears a range before it gives the keyboard back, as `Flame`'s zoom does.

### Logs and versions
No new field. The record reads what the X-ray already reads:
- **Claude Code usage**, seen in 2.1.284–2.1.316: `message.usage` with `cache_read_input_tokens`, `cache_creation_input_tokens`, `cache_creation.ephemeral_5m_input_tokens` / `ephemeral_1h_input_tokens`, `input_tokens` and `output_tokens`.
- **Codex usage**, seen in 0.145–0.166: `token_usage_record` or `token_count`.
- **Compactions:** `compact_boundary` with `compactMetadata.trigger`, and the idle notices.
- **Interrupts:** the `[Request interrupted` user lines, and Codex `turn_aborted`.
- **Failed results:** `is_error`, and Codex's non-zero `exit_code`.
- **Commits:** the `git commit` commands of shell calls.
- **Effort and settings:** the request-audit lines.
- **Tools and the system prompt:** the shapes of the audited prompts.

Claude Code transcripts written before 2.1.300 do not show the prompt's content. For them, entered is unknown and the views say so.

### Parse time
The pass is not the parse. The new call costs one dynamic call per request, which does nothing outside the X-ray. In the X-ray, grouping the entered blocks touches each block once in the session, when it enters. Expect no change in `tokprof scan` time. The check is to time `tokprof scan` on a 7,000-request session before and after, and the full-corpus `tokprof summary`.
