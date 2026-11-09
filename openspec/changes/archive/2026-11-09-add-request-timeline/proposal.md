# Proposal

## Why

The question is "why did this request cost $4, and what happened in this session", answered in two seconds. Today the window's `Requests` tab can't answer it. It shows a bar of cost per request over time and a table of time, gap, context and cost. `Context` shows only what the latest prompt holds, and `Cache` lists the misses apart from the requests they hit. To find why one request cost $4, the user has to read three tabs and match times by hand.

Other tools show the pieces (market scan of 2026-11-08, sections 1b, 3 and 5):
- dsh-context-lens compares each request with the previous one and ties a cache drop to what changed.
- Claude Code's `/cost` names the cause of a miss, but only for the main conversation and only until a resume.
- Devin colors a session timeline by events, and DevTools marks problems with red triangles.
- dsh-session-stats-viewer stacks input, cache read, cache write and output.

tokprof already knows all of this for every request:
- usage and price, model, effort and chain;
- the miss with its cause, detail and avoidable premium;
- compactions;
- the blocks that entered the context, with kind, tool, target and tokens;
- typed messages and interrupts;
- tool failures and the agents' commits.

It also keeps all of it across resumes and over the whole history.

## What Changes

- **A request timeline in `Requests`.** The bar chart becomes one stacked column per request of the main conversation. The parts are the X-ray's components: cache read, cache write, rewrite after miss, uncached input and output. The parts are in dollars by default, and in tokens with the `$ / tokens` pill or `t`.
  - A line shows the context size against the context window, on a second scale at the right.
  - The time axis, the cache's warm and cold spans and the dashed next request stay as they are today.
  - Requests that fall within 4 pt of each other share one column.
- **Markers on the timeline.** Each kind has its own color:
  - a compaction;
  - a cache miss, labelled by its cause and red when its avoidable premium is $0.50 or more;
  - a message the user typed;
  - the user stopping the agent;
  - a streak of 3 or more failed tool calls;
  - an agent's commit, in green.
  - A thin band under the columns shows when each subagent ran.
- **Hover a column** to see its time, model, the five parts in dollars and tokens, the context size, and the cause when it missed.
- **Select a request.** A click on a column selects it, and so do ← → or ↑ ↓ while the inspector has the keyboard. The tab opens on the costliest request. A panel under the chart then shows:
  - the request's cost split;
  - why it cost that: what it read, what it wrote and why (the miss and its cause, a compaction, or the context growing by N);
  - what entered the context since the previous request, largest first;
  - what changed since the previous request: model, effort or settings, the tool set, the agent's version, a compaction, and the idle time against the cache's lifetime.
- **Drag a range** over the chart, or press Shift+↑ ↓, to total the range in the same panel: requests, span, the five parts, misses and their avoidable dollars.
- **The table stays.** It gains `read`, `write`, `uncached` and `output` columns and a `why` column, and its selected row follows the chart.
- **Console first.** `tokprof scan [target] --requests` prints the same thing per request: the token split, context, cost and why, a line of what entered, and a line of what changed. `tokprof scan` without the option prints exactly what it prints today.

## Capabilities

### New Capabilities
None.

### Modified Capabilities
- `desk-window`:
  - modifies `Other tabs` (`Requests` is the timeline now) and `Keys in the inspector` (← → step requests in `Requests`);
  - adds `Request timeline`, `Timeline markers`, `Request under the pointer`, `Selected request`, `Range of requests`, `Requests table`, `Keys in Requests` and `Requests in a frame without a window`.
- `session-xray`: adds `Requests list` (`tokprof scan --requests`).

## Impact

- **Code:**
  - the cost pass in tokprof-core hands each request's newly entered blocks, and the previous request it was compared with, to the X-ray. The X-ray keeps one record per main request: its parts, miss, entered blocks, changes, failures and commits.
  - the X-ray also keeps the main log's typed messages, interrupts and compactions, and the subagents' spans;
  - a new terminal view in tokprof-browse;
  - the `Requests` tab of tokprof-desk.
- **Logs:** no new field. Everything comes from fields the X-ray already reads (see design).
- **Check on the real logs:**
  - For three real sessions with misses (from `tokprof misses --since 7d`), the parts of each request must add up to its cost. Over a session, the sum must equal the X-ray's `main` spend line to the cent.
  - The miss markers must be the X-ray's unexpected misses of the main chain, the same count with the same times and causes.
  - `tokprof scan <id>` must take the same time as before, and print byte-for-byte the same output.
  - Frames of the tab in light and dark for a session with misses and a compaction.
- **Out of scope:**
  - diffing the text of prompts: logs do not hold full prompts reliably, and tokprof never guesses;
  - resending a request;
  - the subagents' own requests on the timeline (only their band);
  - the terminal ui (`tokprof ui`).
