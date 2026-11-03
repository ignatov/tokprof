# share-export Specification

## Purpose
Writes the numbers of a user's agent sessions to a file that holds no names, so that several users can compare and pool their spend, cache misses and pauses without showing their work.

## Requirements

### Requirement: Share command
`tokprof share` SHALL write one JSON file of aggregates over the requests that the global filters select (see session-logs). The file goes to `share-<YYYY-MM-DD>.json` in the profile directory (`~/tokprof`, or `$TOKPROF_DIR` when set), or to the file `-o` names, with missing directories created. It then prints `wrote <path>`, the report of that one file (see share-pool), and `Nothing was sent anywhere: review the file, then send it yourself.`

#### Scenario: Default path
- **WHEN** `tokprof share` runs on 2026-11-03 without `-o`
- **THEN** it writes `~/tokprof/share-2026-11-03.json` and prints `wrote ~/tokprof/share-2026-11-03.json`

#### Scenario: Filters
- **WHEN** `tokprof --agent claude --since 30d share` runs
- **THEN** the file covers only the Claude Code requests of the last 30 days, and its `window` names the first and last day of those requests

#### Scenario: No requests
- **WHEN** the filters select no request
- **THEN** the command fails with `no requests found for these filters; nothing written`, and writes no file

#### Scenario: What it prints
- **WHEN** `tokprof --since 30d share` writes its file
- **THEN** stdout holds, in this order: `wrote <path>`; the report that `tokprof pool <path>` prints for that one file; and `Nothing was sent anywhere: review the file, then send it yourself.`
- **AND** it prints no list of the words in the file

### Requirement: File header
The file SHALL start with `format` (the number 2), `tokprof` (the version that wrote it), `platform` (the system and CPU it ran on, such as `macos-aarch64` or `linux-x86_64`), `window` (the first and last day of the selected requests, as `YYYY-MM-DD`), `survey` and `mcp_names` when they were given, and `agents`, one object per agent with requests, keyed by agent name.

#### Scenario: Two agents
- **WHEN** the selected requests come from Claude Code and Codex
- **THEN** `agents` has the keys `claude` and `codex`

#### Scenario: A survey file
- **WHEN** `tokprof share --survey team-2026-11 --mcp-names docs` runs on an Apple silicon Mac
- **THEN** the header has `"format": 2`, `"platform": "macos-aarch64"`, `"survey": "team-2026-11"` and `"mcp_names": ["docs"]`

#### Scenario: Without a label
- **WHEN** neither `--survey` nor `--mcp-names` is given
- **THEN** the header has no `survey` and no `mcp_names`

### Requirement: Spend parts
Each agent's object SHALL hold `spend`, by model and cost component in dollars and tokens, as the summary's model table counts it (see cost-attribution), and per model `priced`: whether it has a price (see pricing). It SHALL also hold `clients`, by client (see session-logs), with requests, cost, avoidable dollars, main sessions, cold returns (see pause-report), and the tokens written to the cache with the short and with the long lifetime.

#### Scenario: The same totals as the summary
- **WHEN** `tokprof summary` and `tokprof share` run with the same filters
- **THEN** each model's components and total in `spend` equal the summary's figures before rounding

#### Scenario: A model without a price
- **WHEN** the selected requests include 1,947 to `codex-auto-review`, which has no price
- **THEN** its row in `spend` has `"priced": false`, its tokens, and no dollars

#### Scenario: Clients
- **WHEN** the selected Claude Code requests ran under `sdk-ts` and `cli`
- **THEN** `clients` has the keys `sdk-ts` and `cli`, and their costs add up to the agent's spend

#### Scenario: Clients and the cache lifetime
- **WHEN** an agent's `sdk-ts` requests wrote 2,100,000 tokens with the short lifetime and none with the long one, in 9 main sessions with 12 cold returns
- **THEN** `clients.sdk-ts` has `"cache_writes": {"short": 2100000, "long": 0}`, `"main_sessions": 9` and `"cold_returns": 12`

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
- spend by kind of origin, the part of the origin before `:`, such as `skill` for `skill:code-review`, with the names that ship with the agent under their kind (see Names that ship with the agent);
- spend by trigger and by activity, as cost-attribution names them;
- setup by source, as setup-breakdown names them;
- `effort`: requests, output tokens and dollars by effort level, separately for main conversations and for subagents.

#### Scenario: Origins without names
- **WHEN** the selected requests have the origins `skill:walrus-review`, `skill:release-notes` and `agent:ocelot`, and the source lists none of these names
- **THEN** the file has the origin kinds `skill` and `agent` with their summed spend, and neither `walrus-review`, `release-notes` nor `ocelot`

#### Scenario: Names under their kind
- **WHEN** the requests have the origins `agent:general-purpose` for $2,236 and `agent:ocelot` for $40, and the Claude Code source lists `general-purpose` as a built-in subagent type
- **THEN** `origins.agent` has $2,276, and its `names` hold only `general-purpose`, with $2,236

#### Scenario: Effort
- **WHEN** 20,183 main-conversation requests ran at `max` and 6,278 logged no effort
- **THEN** `effort.main` has `max` with 20,183 requests and `unknown` with 6,278, each with its output tokens and dollars

#### Scenario: Effort levels
- **WHEN** a request logs the effort `minimal`, `low`, `medium`, `high`, `xhigh` or `max`
- **THEN** it counts under that level
- **AND** a request that logs another value counts under `other`, and one that logs none under `unknown`

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
- host or user names;
- skill, MCP server, plugin, command, subagent-type, tool or program names, except those that Names that ship with the agent, Shell programs and Kept MCP servers allow.

#### Scenario: Distinctive names
- **WHEN** the logs hold a session in `~/src/zebra-billing`, titled `Fix quokka export`, with the skill `walrus-review`, the slash command `/walrus-sync`, the MCP server `narwhal`, a subagent of type `ocelot`, a read of `/tmp/heron.txt`, the command `./yak-deploy.sh --fast` and the prompt `ping the yak`
- **THEN** none of `zebra`, `quokka`, `walrus`, `narwhal`, `ocelot`, `heron` and `yak` appears in the file

#### Scenario: Home directory
- **WHEN** the file is written for a user whose home directory is `/Users/me`
- **THEN** the file contains neither `/Users/` nor `~/`

#### Scenario: The words the file holds
- **WHEN** a file is written
- **THEN** its keys and strings are only:
  - model, client and version names;
  - the survey label and the kept MCP servers;
  - the names the requirements above allow;
  - tokprof's own names for parts, components, causes, triggers, activities, setup sources, content kinds, ranges, hints, compaction triggers and effort levels.

### Requirement: Names that ship with the agent
Each log source SHALL list the tool names, slash commands, bundled skills and built-in subagent types that ship with its agent, with the agent versions it checked them in (see the source's README). The file keeps a name that is on its agent's list. Any other name of these kinds counts under its kind only.

#### Scenario: A built-in subagent type
- **WHEN** a Claude Code request's origin is `agent:general-purpose`
- **THEN** the file holds `general-purpose` under the origin kind `agent`

#### Scenario: A user's own names
- **WHEN** the origins are `skill:walrus-review`, `command:/walrus-sync` and `agent:ocelot`, and none of these names is on the list
- **THEN** the file counts them only as `skill`, `command` and `agent`

#### Scenario: A name that a newer agent added
- **WHEN** the logs call a built-in tool that a newer agent version added, and the source's list does not have it yet
- **THEN** its calls count under the tool `other` until the list adds it

### Requirement: Shell programs
A shell tool's calls SHALL be split by the program of their command head (see tool-breakdown): the head's first word without its path. A program keeps its name when cost-attribution names it in Read-only commands or Activity of a shell command, when it is a shell keyword or one of the wrappers `time`, `timeout`, `env` and `nice`, or when it is one of the common programs below. Any other program counts as `other`.

#### Scenario: Common programs
- **WHEN** the first word is `mkdir`, `rm`, `cp`, `mv`, `ln`, `touch`, `chmod`, `curl`, `wget`, `tar`, `unzip`, `zip`, `xargs`, `sleep`, `kill`, `pkill`, `ps`, `lsof`, `date`, `open`, `xdg-open`, `ssh`, `scp`, `rsync`, `docker`, `kubectl`, `brew`, `pip`, `pip3`, `npx`, `java`, `javac`, `kotlinc`, `rustc`, `rustup`, `cmake`, `clang`, `gcc`, `ffmpeg`, `sqlite3`, `psql` or `osascript`
- **THEN** the program keeps its name

#### Scenario: Programs of the other lists
- **WHEN** the command heads are `sed -n`, `python3 -I`, `git grep`, `/usr/bin/grep -n` and `for f`
- **THEN** they count under `sed`, `python3`, `git`, `grep` and `for`

#### Scenario: The user's own scripts
- **WHEN** the command head is `./yak-deploy.sh --fast` or `zebra-build all`
- **THEN** it counts under `other`

### Requirement: Kept MCP servers
`--mcp-names <name>,…` SHALL keep the named MCP servers, as each log source names its MCP tools' servers. Their tools count under `mcp:<server>` instead of `mcp`, their origins under the name `<server>` of the kind `mcp`, and the header lists the names in `mcp_names`. Without the option, every MCP tool counts under `mcp`.

#### Scenario: Without the option
- **WHEN** the logs call tools of the MCP servers `docs` and `narwhal`
- **THEN** the file has one `mcp` tool with the calls of both, and neither name

#### Scenario: With the option
- **WHEN** `tokprof share --mcp-names docs` runs over the same logs
- **THEN** `mcp:docs` has the calls of `docs`, `mcp` has the calls of `narwhal`, and `narwhal` does not appear

### Requirement: Survey label
`--survey <label>` SHALL write the label into the header as `survey`. A label is 1 to 40 characters, each a letter, a digit, `.`, `_` or `-`. Any other value fails with `--survey takes 1 to 40 letters, digits, '.', '_' or '-'` before any log is read.

#### Scenario: A label
- **WHEN** `tokprof share --survey team-2026-11` runs
- **THEN** the header has `"survey": "team-2026-11"`

#### Scenario: A label with a space
- **WHEN** `tokprof share --survey "team 11"` runs
- **THEN** the command fails with that message and writes no file

### Requirement: Content parts
Each agent's object SHALL hold `content`: dollars and tokens by kind of content, as cost-attribution names the kinds, with every cost component included.

#### Scenario: The kinds
- **WHEN** a file is written
- **THEN** the keys of `content` are among `tool schemas`, `system prompt`, `user prompt`, `reminders`, `tool results`, `assistant text`, `thinking`, `tool calls`, `images` and `unknown`

#### Scenario: Content adds up to the spend
- **WHEN** the dollars of an agent's `content` are added up
- **THEN** they equal the sum of the totals in its `spend`

### Requirement: Prompt size parts
Each agent's object SHALL hold `prompt_sizes`: requests, dollars and tokens by the size of each request's prompt, under `main` for main conversations and under `subagents` for subagents and side requests. The ranges are `under 50k`, `50–100k`, `100–200k`, `200–400k`, `400–800k` and `800k+` tokens, each with its lower bound. Empty ranges are left out.

#### Scenario: A prompt at a bound
- **WHEN** a main-conversation request's prompt is 400,000 tokens
- **THEN** it counts in `main` under `400–800k`

#### Scenario: Prompt sizes add up to the spend
- **WHEN** the dollars of an agent's `prompt_sizes` are added up
- **THEN** they equal the sum of the totals in its `spend`

### Requirement: Session span parts
Each agent's object SHALL hold `session_spans`: the main sessions by the time from their first to their last selected request, subagents included. The ranges are `under 1h`, `1–4h`, `4–24h`, `1–3d` and `over 3d`, each with its lower bound. Each range has sessions, dollars and cold returns (see pause-report). Empty ranges are left out.

#### Scenario: A session over days
- **WHEN** a main session's selected requests ran from 2026-10-20 09:00 to 2026-10-22 18:00, cost $412 with its subagents, and had 14 cold returns
- **THEN** it counts in `1–3d` with $412 and 14 cold returns

#### Scenario: Spans add up
- **WHEN** an agent's `session_spans` are added up
- **THEN** the sessions equal its `main_sessions`, and the dollars the sum of the totals in its `spend`

### Requirement: Tool parts
Each agent's object SHALL hold `tools`: per tool, calls, single calls, the number and tokens of results and of inputs, rent and trips, as tool-breakdown counts them. A tool keeps its name as Names that ship with the agent allows. MCP tools count under `mcp` (see Kept MCP servers), and any other tool under `other`. A shell tool also holds `programs`: the same numbers per program (see Shell programs).

#### Scenario: The same totals as the tools command
- **WHEN** `tokprof tools` and `tokprof share` run with the same filters
- **THEN** the rent and the trips of all tools in the file add up to the figures of the total line of `tokprof tools`

#### Scenario: A shell tool
- **WHEN** `Bash` ran `sed -n` 4,046 times and `sed -i` 20 times
- **THEN** `tools.Bash.programs.sed` has 4,066 calls

### Requirement: Tool hint parts
Each agent's object SHALL hold `tool_hints`: for each hint of tool-breakdown, the number of rows that got it, with their calls, rent and trips. The rows and their hints are those that `tokprof tools` shows for the same filters, before the rows are joined by program.

#### Scenario: Reads one per response
- **WHEN** `tokprof tools` gives `batch independent reads` to the rows `Bash sed -n` and `Bash git grep`, and to no other row
- **THEN** `tool_hints` has that hint with 2 rows and the sum of their calls, rent and trips

### Requirement: Compaction parts
Each agent's object SHALL hold `compactions`: the compactions inside the window of the main sessions with a selected request, by trigger: `auto`, `manual`, `idle` and `unknown` (see session-logs), and `other` for any other trigger a log names.

#### Scenario: Compactions of one session
- **WHEN** a session was compacted twice because its context was full and once while it sat idle
- **THEN** `compactions` has `auto` 2 and `idle` 1

### Requirement: Version parts
Each agent's object SHALL hold `versions`: requests and dollars by the agent version that each request carries (see session-logs). A value counts as `other` unless it is digits and dots, optionally followed by `-` and a suffix of letters, digits and dots. A request without a version counts as `unknown`.

#### Scenario: Versions of two agents
- **WHEN** the Claude Code requests carry `2.1.316` and the Codex ones `0.166.0-alpha.5.1`
- **THEN** `agents.claude.versions` has the key `2.1.316` and `agents.codex.versions` the key `0.166.0-alpha.5.1`

#### Scenario: A value that is not a version
- **WHEN** a request's version is `build of zebra`
- **THEN** it counts under `other`

### Requirement: What-if parts
The Claude Code object SHALL hold `whatif`: under `main` for main conversations and under `subagents` for subagents and side requests, the requests, the input cost as logged, and the input cost replayed under a 5-minute and under a 1-hour cache, as `tokprof whatif` counts them (see cache-misses). The objects of other agents have no `whatif`.

#### Scenario: The same numbers as the whatif report
- **WHEN** `tokprof whatif` and `tokprof share` run with the same filters
- **THEN** each figure under `whatif` equals the report's before rounding

### Requirement: Leak check
Before it writes, `tokprof share` SHALL search every key and string of the file that is not one of tokprof's own words (see Own words). It looks for this machine's home path, for the ids, short ids and titles of the selected sessions, and for the user name and host name as whole words of 4 or more characters. When it finds one, it writes nothing and fails with `the file would hold <what> in <field>; nothing written`.

#### Scenario: A home path in a model name
- **WHEN** a log names the model `/Users/me/models/m1`, and the home directory is `/Users/me`
- **THEN** the command fails with `the file would hold the home path in agents.claude.spend; nothing written`
- **AND** no file is written

#### Scenario: A name that is also one of tokprof's words
- **WHEN** the user name is `user`, or a session is titled `thinking`
- **THEN** the content kinds `user prompt` and `thinking` do not stop the file, since tokprof's own words are not searched

#### Scenario: Nothing found
- **WHEN** the search finds nothing
- **THEN** the file is written

### Requirement: Own words
tokprof's own words SHALL be the fixed names of its parts, components, causes, triggers, activities, setup sources, content kinds, ranges, hints, compaction triggers and effort levels, and the names that the requirements above allow. Every other key or string of the file is a word that the leak check searches: model names, client names, versions, the survey label, the kept MCP servers, and any other. `tokprof share` SHALL NOT print these words.

#### Scenario: Models, clients and versions
- **WHEN** the file holds the models `claude-opus-5-5` and `gpt-6-astra`, the clients `sdk-ts` and `codex_exec`, and the versions `2.1.316` and `0.166.0`
- **THEN** these six are the words outside tokprof's own, the leak check searches them, and the command prints none of them

#### Scenario: A string of no known kind
- **WHEN** the file holds a key or string that is neither one of tokprof's own words nor a model, client, version, label or kept MCP server
- **THEN** the leak check searches it too
