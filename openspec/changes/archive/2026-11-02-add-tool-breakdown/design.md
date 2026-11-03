# Design

## Context

Cost attribution already charges every share of a request to a prompt block by position. Each block has a kind, a tool, a target (for shell calls, the command head) and a neutral tool class. The X-ray totals facts by kind, tool and target. That is `rent` per target, but without call counts or sizes.

The walker sees each block the first time a request holds it, when it records the block's turn and time. It knows each request's spans of tokens and the message the response produced. The prompt delta of a request lists the messages it appended, and the tool results among them are the ones it carries for the first time.

`add-work-breakdown` adds the `prompts` block and changes the X-ray layout. It is applied but not archived, and this change's layout delta builds on its version.

## Goals / Non-Goals

**Goals:**
- One analysis pass gives rent, sizes, calls, single shares and trips for every row.
- Rent and trips each sum to the spend, so the total line can be checked.
- The hints are pure functions of a row's numbers.

**Non-Goals:**
- Splitting a Codex `exec` with several inner tools into rows. Blocks keep one tool per call, and rebuilding the inner calls' sizes is not possible.
- Rows by file or argument.

## Decisions

1. **The row key comes from blocks.**
   - A row's key is the tool, plus the target when the tool's class is `execute` or `execute output`. So shell rows are split by command head and other tools are not.
   - Rent is the facts' spend on `tool results` and `tool calls` blocks under that key. That is the same allocation `what cost the most` uses, so no new attribution is needed.

2. **Sizes and single shares come from a new sink event.**
   - When the walker first places a block, it reports the block's kind, tool, target, class and tokens in that request. Tokens are the block's span scaled to the request's prompt.
   - For the message a response produced, it reports how many tool-call blocks the message holds. That gives `single`.
   - The sink trait gets default no-op methods, so the existing sinks need no change.
   - Alternative: re-derive sizes from characters at four per token. Rejected, because attribution already has the real scale per request.

3. **Trips are charged to first-time tool results.**
   - In each request with a prompt delta, the tool-result blocks of the appended messages that no earlier request held are its triggers.
   - The request's input spend, everything but output, is split equally between their rows.
   - The output of the response that made calls is split equally between the rows of the tool-call blocks of its produced message.
   - Requests without triggers and responses without calls go to `not from a tool` lines that the total line counts. This keeps both views summing to the spend.
   - Alternative: use the trigger of each request. Rejected, because the trigger is one value per request and has no row key.

4. **The table is a view in the core, rendered by the CLI.**
   - A `ToolTable` collector holds rows and totals and can merge per-session parts, as the other collectors do.
   - `tokprof tools` runs the scan with it alone. The X-ray adds it to its sink, so the X-ray needs no extra pass.

5. **Hints are rules over a row.**
   - The rules use the row's numbers, its tool class, and whether its command head is read-only, through the read-only rule of cost-attribution.
   - They run in spec order, and the first one that applies wins.
   - The thresholds are constants next to the rules: 5 calls, $1, 60% single, 400 and 2,000 tokens of input, 5,000 and 2,000 tokens of result.

Log fields: none new. Rows use the tool names and targets that sources already set, and the classes from the split.

Parse time: one more map update per new block and per request with a delta. That is about 83k calls in 30 days of Claude Code, so the effect should be under 1% of the ~24 s full parse.

## Risks / Trade-offs

- **[Splitting a response's spend equally between its calls is a convention]** → The spec states it. Equal shares keep a batched response cheaper per call, which is the point of the batching hint.
- **[A hint fires on a habit that was necessary, such as long scripts that had to be inline]** → A hint names the habit and its size, not a saving. Rows need 5 calls and $1 before any hint.
- **[First-time tool results miss requests whose prompt the log cannot rebuild]** → Their input goes to `not from a tool`. The total line still adds up, and older transcripts show less in trips.
- **[The layout delta depends on an unarchived change]** → Task 1.1 archives `add-work-breakdown` first.

## Open Questions

None.
