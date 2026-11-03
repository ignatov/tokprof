# Proposal

## Why

The author's logs show where one person's money goes: cold returns to big sessions, contexts near the window, review skills that fan out into subagents, effort at `max`, setup that every request carries. The author wants to know whether this holds for other people. The plan is a survey: colleagues paste a prompt into their coding agent, the agent downloads tokprof and writes a share file, and the colleague sends the file back.

`tokprof share` (format 1) is the start of this, but it cannot answer the survey's main questions yet:
- **What fills the context.** On the author's 30 days (2026-10-04 to 11-03, $11,123), tool results took 39% of the spend, tool calls 17%, thinking 17% and reminders 8%. None of this is in the file.
- **Which tools, commands and skills carry the spend.** The file cuts every origin to its kind. So `skill:code-review` ($1,720, 15.5%) and `agent:general-purpose` ($2,236, 20.1%) show only as `skill` and `agent`, although both names ship with Claude Code. The file has no tools at all.
- **How big the prompts got, and how long the sessions ran.**
- **Which agent versions ran,** and what another cache lifetime would have saved.

There is also no way to read many files together. And a colleague who runs the survey gets back nothing but a JSON file, although seeing their own numbers is what makes it worth their time.

## What Changes

- **Format 2 of the share file.** New parts for each agent:
  - `content`: spend by kind of content;
  - `prompt_sizes`: requests and spend by the size of the prompt, for main conversations and for subagents;
  - `session_spans`: main sessions by how long they ran, with spend and cold returns;
  - `tools`: calls, sizes, rent and trips per tool, and per program for shell tools;
  - `tool_hints`: the tool hints, with the rows, calls, rent and trips they apply to;
  - `compactions`: by trigger;
  - `versions`: requests and spend by agent version;
  - `whatif`, for Claude Code: input cost as logged and under a 5-minute and a 1-hour cache.
- **Changed parts.**
  - `clients` add main sessions, cold returns and cache writes by lifetime.
  - `origins` keep their kind totals and add the names that ship with the agent.
  - `effort` adds dollars.
- **The names that stay.**
  - Names that ship with the agent: built-in tools, slash commands, bundled skills and built-in subagent types, as each log source lists them.
  - Shell programs from a fixed list. Any other program is `other`.
  - MCP tools count as `mcp`, unless `--mcp-names` names servers to keep.
  - A user's own skills, commands, subagent types and scripts never appear.
- **A label and a platform.** `--survey <label>` writes a label into the header, so the files of one survey do not mix with another's. The header also names the platform.
- **A leak check.** Before writing, `tokprof share` searches the file for this machine's home path, user name and host name, and for the ids and titles of the selected sessions. When it finds one, it writes nothing. After writing, it lists the words in the file that tokprof did not make (model, client and version names, the label, the kept MCP servers) for the person to review.
- **`tokprof pool <files or folders>`** reads share files of format 1 and 2 and prints one report:
  - the files, windows and labels, and the spend per file;
  - where the money went: by model, cost component, content, origin, activity, prompt size, session span and client, pooled and as the median file's share;
  - what could have been saved: avoidable dollars by miss cause and by cache lifetime, and in how many files each is 5% of the spend or more;
  - habits that cost the most: prompts of 400k and more, sessions longer than a day, single lookups, thinking, setup, the highest effort, and each tool hint;
  - tools and commands, and agent versions.

  `--json` writes the same numbers, `--by-file` adds a column per file, and `--min-people <n>` folds rows that fewer than n files share into `other`.
- **`tokprof share` prints the report of the file it wrote,** as `tokprof pool` would for that one file, before the words to review and `Nothing was sent anywhere…`.
- **`docs/survey.md`: how to run a survey,** and the prompt a colleague pastes into their agent, with the release tag, the label and the recipient filled in. The prompt tells the agent to:
  - download the archive for the machine from that release (see `add-release-builds`);
  - check it against `SHA256SUMS`, and with `gh attestation verify` when `gh` is installed;
  - run `tokprof --since 30d share --survey <label>`;
  - show tokprof's output, and tell the person to review the file and send it themselves.

  The agent installs nothing, sends nothing, and reads neither the file nor the logs into the chat.

## Capabilities

### New Capabilities

- `share-pool`: `tokprof pool`, the report it prints over one or many share files, and the survey guide with its prompt.

### Modified Capabilities

- `share-export`:
  - format 2;
  - the new parts;
  - the names that ship with the agent, shell programs and kept MCP servers;
  - the label and the platform;
  - the leak check and the words to review;
  - the command prints the report of its file.
- `session-logs`: each request carries the agent version. That is new for Codex, from `session_meta.cli_version`.

## How it is checked on the real logs

- **Totals.** On the author's 30 days, the parts equal the reports for the same filters:
  - `content` adds up to the spend ($11,123 in `tokprof summary`);
  - `tools` gives the totals of `tokprof tools` (rent $6,266, trips $8,551);
  - `whatif` equals `tokprof whatif`;
  - `prompt_sizes` adds up to the spend, and `session_spans` to `main_sessions`.
- **Names.**
  - The distinctive-names test of format 1 grows: a custom slash command, a custom subagent type, a user skill, an MCP server and a script `./zebra.sh`. None of them may appear.
  - On the real logs, a search of the file for the author's own skill, command and MCP server names (from `tokprof flame --stack agent,origin` and `--stack agent,kind,tool`) finds none.
  - `code-review`, `general-purpose`, `/goal`, `Bash` and `git` appear, provided the source lists them.
- **The leak check.** A test that puts the home path into a model name makes the command write nothing.
- **Size and time.** The 30-day file stays under 150 KB (27.5 KB in format 1). The first estimate was 60 KB; on the author's 30 days the file is 105 KB, and its 150 tool and program rows are most of it. The run stays within 5% of format 1's time.
- **The pool.**
  - `tokprof pool` over the author's file gives the summary's spend by model and misses by cause.
  - Three copies of the file pool to three times the dollars, with the same shares. A format-1 file next to a format-2 one marks the parts it lacks.
  - 50 files pool in under a second.
- **The prompt.** A dry run in a new Claude Code session and a new Codex session on the author's Mac, against the dry-run archives of `add-release-builds`. Each agent finds the archive, checks it, runs tokprof and reports. Record each session's time and cost. The aim is under 5 minutes and under $1.

## Out of scope

- Sending the file anywhere, or collecting files automatically. tokprof stays local, and each person sends their own file.
- Failures, waits and retries. They join the file as a part once the change that adds them (`add-failures-and-waits`) has landed.
- Per-day or per-session rows, noise added to the numbers, and agents tokprof does not read.
- The names of a user's MCP servers, skills, commands and subagent types, except the MCP servers the person keeps with `--mcp-names`.
- Charts of the pooled report.

## Impact

- **Core.** The share collector adds content kinds, prompt sizes, session spans, compactions and versions. It takes the tool table and the what-if replay from the summary's collector, which the same scan already fills. Also new: the leak check, a pool reader and the report.
- **Sources.** Each lists the names that ship with its agent, with the versions they were checked in, in code and in its README. Codex reads `cli_version`.
- **CLI.** `--survey` and `--mcp-names` on `share`, and the `pool` command.
- **Docs.** `docs/survey.md`, and the README's share section.
- **Order.** Apply after `add-release-builds`: the survey guide links its archives.
