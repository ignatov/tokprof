# Proposal

## Why

The author wants to know what sessions look like for other users of coding agents, and where their spend could shrink. That would show whether their own patterns hold for others: long IDE sessions, cold returns after lunch and overnight, a 5-minute cache, effort at `max`.

The cheapest first step is a pilot. Colleagues run tokprof on their own logs and send back the numbers. Today they could only send screenshots of `summary`. Those carry project paths, session titles and tasks, and they cannot be added up.

## What Changes

- **`tokprof share`** writes one JSON file of aggregates over the selected requests, with the usual filters.
  - It goes to `~/tokprof/share-<YYYY-MM-DD>.json` by default, or `$TOKPROF_DIR`, or to the file `-o` names.
  - It prints the path, a short list of what the file holds, and `Nothing was sent anywhere: review the file, then send it yourself.`
- **Per agent and client, the file holds:**
  - spend by model and cost component, with tokens;
  - cache writes split by lifetime, for main conversations and for subagents;
  - cache misses by cause: counts, tokens, avoidable dollars;
  - spend by kind of origin (`prompt`, `skill`, `agent`, `command`, `notification`, `mcp`, `plugin`), by trigger and by activity;
  - setup by source;
  - requests and output tokens by effort level, for main conversations and for subagents;
  - the parts of `tokprof pauses`: pauses before typed messages, the share after each lifetime, cold returns by pause, who started them, what followed them, and sessions by largest context;
  - the number of sessions and of main sessions, the window's first and last day, the tokprof version and a format number.
- **It holds no names.**
  - No project or file path, no session id or title, no task, no prompt or tool text.
  - No skill, MCP server, plugin, command or subagent-type name, no tool name and no host or user name.
  - Model names and client names stay, since they are public product names.
- The README describes the command and the fields. It also says how to collect the files in a pilot.

## Capabilities

### New Capabilities

- `share-export`: the `tokprof share` command, the content of the file, and what it must never contain.

### Modified Capabilities

None. The `--client` filter and the client split come from `add-client-dimension`, and the pause parts from `add-pause-report`. This change is applied after both.

## How it is checked on the real logs

- **Totals.** On the author's logs, the file's totals equal `tokprof summary` for the same filters: spend by model and component, misses by cause, and the setup table. Its pause parts equal `tokprof pauses`.
- **No names.** A test builds sessions whose project, title, task, skill, MCP server, plugin, subagent type, file paths and prompt text are distinctive strings. None of them may appear in the file. On the real logs, a search of the file for `/Users/`, `~/`, the author's project names and session id prefixes finds nothing.
- **Size and speed.** The 30-day file is under 50 KB. `tokprof share` takes no longer than `summary` and `pauses` run one after the other.

## Out of scope

- Sending the file anywhere. tokprof stays local; the user sends the file.
- Merging the files of several users. A small script in the README, or a later change, can sum them.
- Per-session or per-day rows, which could identify work.
- Anonymizing at finer levels than "no names", such as noise or rounding of dollar amounts.

## Impact

- Core: one collector that combines the summary's and the pause report's aggregates. It also needs the requests' effort, which is already in the model.
- CLI: the `share` command and its JSON writer.
- README: the command, the fields, and the pilot.
