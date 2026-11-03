# share-export Specification

## Purpose
Writes the numbers of a user's agent sessions to a file that holds no names, so that several users can compare and pool their spend, cache misses and pauses without showing their work.

## Requirements

### Requirement: Share command
`tokprof share` SHALL write one JSON file of aggregates over the requests that the global filters select (see session-logs). The file goes to `share-<YYYY-MM-DD>.json` in the profile directory (`~/tokprof`, or `$TOKPROF_DIR` when set), or to the file `-o` names, with missing directories created. It then prints `wrote <path>`, a list of the parts it holds, and `Nothing was sent anywhere: review the file, then send it yourself.`

#### Scenario: Default path
- **WHEN** `tokprof share` runs on 2026-11-03 without `-o`
- **THEN** it writes `~/tokprof/share-2026-11-03.json` and prints `wrote ~/tokprof/share-2026-11-03.json`

#### Scenario: Filters
- **WHEN** `tokprof --agent claude --since 30d share` runs
- **THEN** the file covers only the Claude Code requests of the last 30 days, and its `window` names the first and last day of those requests

#### Scenario: No requests
- **WHEN** the filters select no request
- **THEN** the command fails with `no requests found for these filters; nothing written`, and writes no file

### Requirement: File header
The file SHALL start with `format` (the number 1), `tokprof` (the version that wrote it), `window` (the first and last day of the selected requests, as `YYYY-MM-DD`), and `agents`, one object per agent with requests, keyed by agent name.

#### Scenario: Two agents
- **WHEN** the selected requests come from Claude Code and Codex
- **THEN** `agents` has the keys `claude` and `codex`

### Requirement: Spend parts
Each agent's object SHALL hold `spend`, by model and cost component in dollars and tokens, as the summary's model table counts it (see cost-attribution). It SHALL also hold `clients`, by client (see session-logs), with requests, cost and avoidable dollars.

#### Scenario: The same totals as the summary
- **WHEN** `tokprof summary` and `tokprof share` run with the same filters
- **THEN** each model's components and total in `spend` equal the summary's figures before rounding

#### Scenario: Clients
- **WHEN** the selected Claude Code requests ran under `sdk-ts` and `cli`
- **THEN** `clients` has the keys `sdk-ts` and `cli`, and their costs add up to the agent's spend

### Requirement: Cache parts
Each agent's object SHALL hold `cache_writes`, the tokens written with the short and the long lifetime, separately for main conversations and for subagents. It SHALL also hold `misses`, by cause, with counts, re-processed tokens and avoidable dollars, as `Cache misses by cause` counts them (see cache-misses).

#### Scenario: Lifetimes
- **WHEN** an agent's main conversations wrote 190.7M tokens with the long lifetime and none with the short one, and its subagents 154.6M and 0.3M
- **THEN** `cache_writes` holds those four numbers under `main` and `subagents`

#### Scenario: The same misses as the summary
- **WHEN** `tokprof summary` and `tokprof share` run with the same filters
- **THEN** each cause in `misses` has the summary's count, tokens and avoidable dollars

### Requirement: Work parts
Each agent's object SHALL hold:
- spend by kind of origin, the part of the origin before `:`, such as `skill` for `skill:code-review`;
- spend by trigger and by activity, as cost-attribution names them;
- setup by source, as setup-breakdown names them;
- `effort`: requests and output tokens by effort level as the log names it, with `unknown` for none, separately for main conversations and for subagents.

#### Scenario: Origins without names
- **WHEN** the selected requests have the origins `skill:code-review`, `skill:release-notes` and `agent:general-purpose`
- **THEN** the file has the origin kinds `skill` and `agent` with their summed spend, and neither `code-review`, `release-notes` nor `general-purpose`

#### Scenario: Effort
- **WHEN** 20,183 main-conversation requests ran at `max` and 6,278 logged no effort
- **THEN** `effort.main` has `max` with 20,183 requests and `unknown` with 6,278

### Requirement: Pause parts
Each agent's object SHALL hold the parts of the pause report (see pause-report):
- the typed messages per pause range;
- how many of them came after each cache lifetime, with their total, so the share follows;
- the cold returns per pause range, with first-request cost and avoidable dollars;
- the counts of who started them;
- the follow counts' median, quarters and short returns;
- the sessions per largest-context range, with spend and cold returns.

#### Scenario: The same numbers as the pause report
- **WHEN** `tokprof pauses` and `tokprof share` run with the same filters
- **THEN** every count and dollar figure of the pause parts equals the report's

### Requirement: Session counts
Each agent's object SHALL hold `sessions`, the number of logs with a selected request, subagent logs included, and `main_sessions`, the number of main sessions among them.

#### Scenario: A session with subagents
- **WHEN** one main session and its 9 subagent transcripts have selected requests
- **THEN** `sessions` counts 10 and `main_sessions` counts 1

### Requirement: No names in the file
The file SHALL contain none of:
- project paths or names, file paths or other tool targets;
- session ids, titles or subagent tasks;
- prompt, reminder or tool text;
- skill, MCP server, plugin, command, subagent-type or tool names;
- host or user names.

Model names, client names, cause, trigger, activity and setup-source names, and effort levels are the only words it holds.

#### Scenario: Distinctive names
- **WHEN** the logs hold a session in `~/src/zebra-billing`, titled `Fix quokka export`, with the skill `walrus-review`, the MCP server `narwhal`, a subagent of type `ocelot`, a read of `/tmp/heron.txt` and the prompt `ping the yak`
- **THEN** none of `zebra`, `quokka`, `walrus`, `narwhal`, `ocelot`, `heron` and `yak` appears in the file

#### Scenario: Home directory
- **WHEN** the file is written for a user whose home directory is `/Users/me`
- **THEN** the file contains neither `/Users/` nor `~/`
