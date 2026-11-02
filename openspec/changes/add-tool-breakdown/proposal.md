# Proposal

## Why

To decide whether a session could have been cheaper, the user needs the spend per tool, and the X-ray does not show it. `what cost the most` ranks content kinds. `prompts` and activities group requests by message and purpose. Neither answers: how many times was each tool called, how big were its results and inputs, what did they cost, and is there something to change.

We worked the table out by hand for one 6-message session: $27.37, 353 calls.

| Tool | Calls | Per call | Rent | Trips and output |
|---|---|---|---|---|
| `Bash python3 -I` | 60 | 702-token inputs | $4.32 | $7.04 |
| `Bash sed -n` | 56 | 1.3k-token results | $5.21 | $3.75 |
| `Bash grep -n` | 40 | | $1.84 | $2.55 |
| `Write` | 16 | 2.1k-token inputs | $3.75 | |

One `Read` of a 16.9k-token file cost $1.16 to keep.

Three fixed rules covered most of what could be saved, 20–25% of the session:
- batch independent reads;
- edit with an edit tool instead of inline scripts;
- grep a large file instead of reading all of it.

Every number comes from the logs, and the hints come from thresholds. No model is needed.

## What Changes

- **A tool breakdown.** One row per tool, with shell calls split by their command head, such as `Bash sed -n` or `exec_command cargo test`. Each row gives:
  - `calls`;
  - `avg result` and `avg input`: tokens per call;
  - `rent`: what keeping the results and inputs in the context cost, over every request that held them;
  - `trips`: the input of the requests that the calls' results triggered, plus the output of the responses that made the calls. A response's spend is split equally between its calls;
  - `single`: the share of calls made alone in their response;
  - `hint`: a fixed hint, or nothing.
- **Rent and trips are two views of the same money.** Each of them, over all rows plus what no tool caused, adds up to the spend. They are never added together, and the output says so.
- **Hints**, the first that applies, with thresholds in the spec:
  - reads made one per response: "batch independent reads";
  - shell calls with long inputs: "save long scripts to a file; edit files with the edit tool";
  - reads with large results: "read a range or search instead of the whole file";
  - shell calls with large results: "filter the output with head or grep";
  - whole-file writes of large inputs: "edit instead of rewriting the file".
- **`tokprof tools`** prints the table over the reported window, with the usual filters, sorted by rent, or by trips with `--by trips`. It shows the top 25 rows, then `<n> more`.
- **A `tools` block in the X-ray**, after `prompts`: the top 8 rows of the session, its subagents included.
- The README describes the table and the hints.

## Capabilities

### New Capabilities

- `tool-breakdown`: the per-tool table, how its columns are computed, its hints, and the `tokprof tools` command.

### Modified Capabilities

- `session-xray`: the `tools` block, which changes the layout.

## How it is checked on the real logs

- **The 6-message example session.** The `Bash python3 -I`, `Bash sed -n`, `Bash grep -n` and `Write` rows match the hand count above within 15%. The rows that should get hints get them: `Bash sed -n` gets the batching hint, `Bash python3 -I` the scripts hint, and the large `Read` the range hint.
- **Totals.** Over 30 days of Claude Code logs, rent over all rows plus content that no tool produced equals `Total priced`, and so do trips plus requests that no call triggered.
- **Codex.** Over 30 days, `exec` calls with one inner tool show as that tool's rows, such as `exec_command cargo test`.
- **Speed.** The parse time of `tokprof summary` stays within 2% of the baseline. `tokprof tools` takes no longer than `tokprof summary`.

## Out of scope

- Estimating savings in dollars. A hint names a habit and the size of the rows it applies to, not what changing it would save.
- Hints that read file contents, or that compare calls with each other: repeated reads, reverts.
- Model or effort choices per tool.

## Impact

- `tokprof-core`:
  - the analysis records each block's size when it enters the context;
  - it charges each request to the tool results it was the first to carry;
  - a tool table view with its hint rules.
- `tokprof` CLI: the `tools` command and the X-ray block.
- README.md.
- No source changes: tool names, command heads and classes are already on the blocks.
