# Running a survey

This page is for someone who wants to know where a group's coding-agent money goes. Each person's agent downloads tokprof, writes one share file of totals, and the person sends the file back. `tokprof pool` then reads all the files together.

## What a person sends

`tokprof share` reads the Claude Code and Codex logs on the machine and writes one JSON file. It holds:
- spend by model and cost component;
- spend by what fills the context, by kind of origin, by activity, by prompt size, by how long the sessions ran, and by client;
- cache misses by cause, cold returns, cache lifetimes, and what the other lifetime would have cost;
- tools and shell programs, with the tool hints and their failed calls;
- effort levels, compactions and agent versions;
- active days and active hours, the clock hours with a main-conversation request, and the busy time;
- failed, refused and denied calls, and the spend after the failures until the tool worked again;
- where the agent's time went: the model, retries after API errors, subagents, background commands, and the commands it waited on; the slow requests;
- the hand-offs: ends of turn, interrupts, questions, answers, messages typed while the agent worked, and wakes;
- the share of spend whose prompts the logs hold.

The file holds no project or file path, no session id or title, no prompt or tool text, no error text, and no user or host name. It keeps only these names:
- names that ship with the agent: built-in tools such as `Bash`, slash commands such as `/goal`, skills bundled with the agent, and built-in subagent types such as `general-purpose`;
- well-known programs such as `git` and `sed`;
- model, client and version names.

A person's own skills, commands, subagent types, MCP servers and scripts count only as `skill`, `command`, `agent`, `mcp` and `other`.

Before writing, tokprof searches the file for the home path, the user and host names, and the ids and titles of the sessions, and writes nothing if it finds one. After writing, it prints what the file says.

The file is anonymous in what it holds, not in how it is sent. Whoever collects the files knows who sent which, and sees each person's spend. When that matters, collect them in a shared folder or a form that does not show the sender.

## The prompt

Fill in these four things, then send the prompt. Each person pastes it into their coding agent, such as Claude Code or Codex.
- `<tag>`: a release, such as `v0.2.0`.
- `<label>`: a label for this survey, such as `team-2026-11`.
- `<mcp servers>`: the MCP servers to count by name, comma separated, such as the servers of the IDE the group works in. When no server should be counted by name, delete `--mcp-names <mcp servers>` from the command in step 5.
- `<recipient>`: where the file goes.

```
Please run a survey of my coding-agent logs with tokprof <tag>, an open-source profiler
(https://github.com/ignatov/tokprof). It reads the Claude Code and Codex logs on this machine
and writes one JSON file of totals with no names in it. It makes no network calls.

1. Make a new temporary folder.
2. Pick the archive for this machine:
   - Mac with Apple silicon: tokprof-aarch64-apple-darwin.tar.gz
   - Mac with Intel: tokprof-x86_64-apple-darwin.tar.gz
   - Linux on x86-64: tokprof-x86_64-unknown-linux-gnu.tar.gz
   - Linux on ARM64: tokprof-aarch64-unknown-linux-gnu.tar.gz
   - Windows on x86-64: tokprof-x86_64-pc-windows-msvc.zip
   - Windows on ARM64: tokprof-aarch64-pc-windows-msvc.zip
3. Download it and SHA256SUMS from https://github.com/ignatov/tokprof/releases/download/<tag>/
   into the folder. If a sandbox blocks the network, ask me to allow these two downloads.
4. Check the archive's SHA-256 against its line in SHA256SUMS. If they differ, stop and tell me.
   If gh is installed, also run: gh attestation verify <archive> -R ignatov/tokprof
5. Unpack the archive in the folder, and from there run
   ./tokprof --since 30d share --survey <label> --mcp-names <mcp servers> -o <folder>/tokprof-share.json
   On Windows, run .\tokprof.exe with the same arguments.
   It can take a few minutes on a long history, so allow it that long.
6. Show me tokprof's output as it is.
7. Tell me where the file is, and that I should look through it and send it to <recipient> myself.

Do not install tokprof, change any settings or send anything anywhere. Do not read the file
or my logs into this chat.
```

The agent downloads nothing but the archive and `SHA256SUMS`, and it sends nothing. The person reads tokprof's output in the chat and sends the file themselves.

A server named in `<mcp servers>` keeps its name in the file: its calls and their rent and trips, and the spend of the work it started. Its name also shows in each file's header. Every other server counts as `mcp`.

## Collecting and pooling

Put the files in one folder. You can name them as you like: the names show only with `--by-file`.

```sh
tokprof pool ~/survey                   # the report over every .json file in the folder
tokprof pool --min-files 3 ~/survey     # the same, with rows that fewer than 3 files have folded into other
tokprof pool --by-file ~/survey         # a column per file, headed by its name
tokprof pool --json ~/survey            # the numbers as JSON, to chart them
```

The pool skips a file that is not a share file, one of a newer format, and a second copy of a file, with a line for each. It reads files of format 1 too: the tables built from the parts that format 1 lacks say how many files they count.

Before you show a pooled report to anyone outside the group, use `--min-files 3`. Then no row describes one or two people. In a small group, a pooled report still shows each person's range, so ask the group first.

## Reading the report

- **The header**: how many files, the days they cover, the survey labels, the spend in all and per file at the median and quarters, and the activity: active hours, the hours at the keyboard, and the dollars per hour of each, pooled and at the median file. An active hour is a clock hour in which an agent sent a request of a main conversation, so it counts the agents' hours; the hours at the keyboard are the person's (see the rhythm in the README).
- **At a glance**: six lines that sum up the sections below: spend, what the cache lost, cold returns, problems, the person's time, and what the agents made.
- **Where the money went**: spend by model, cost component, content of the context, origin, activity, prompt size, how long the sessions ran, and client. `share` is of the pooled spend. `median file` is the median of each file's own share, which a single big spender cannot move. `files` counts the files with the row. Under the content table, a line says what share of the spend the logs hold the prompts of; the rest is `unknown`, as for Claude Code before 2.1.300. Under the model table, a line names the models without a price, such as Codex's `codex-auto-review`: their tokens count as $0, so the spend is that much short.
- **What could have been saved**: avoidable dollars by cause of cache miss, and what the cheaper cache lifetime would have saved over the one in use, in all and per active hour. `files 5%+` counts the files where the row is 5% or more of their spend. `Cold returns by client` gives each client's active hours, its cold returns, and their avoidable dollars per active hour: the number to compare before and after a change in a client. It lists the 8 clients with the most active hours, then `<n> more`. `Coming back after a cold break` gives what the cold returns cost as resumed, against `/compact` first, a new session from a handoff note, and heartbeats, as `tokprof bench` replays them, and how often each way was the cheapest.
- **Problems**: failed, refused and denied calls, the spend after failures, the requests that waited for retries and how long, and the requests of a minute or more, pooled and per active hour, with the median file's rate. Under the retried requests, a row per class of their last error: server errors, rate limits, authentication, timeouts, connection, and `computer asleep`, waits that the computer's sleep caused rather than the API.
- **Habits that cost the most**: the spend in prompts of 400k tokens or more, in sessions longer than a day, in single lookups, in thinking, in setup, at the highest effort, and in each tool hint. These rows show where to look, not what could be saved: they overlap.
- **Where the agent's time went**: the busy time by what the agent waited on, with the part after the end of a turn, while its own subagents or background commands still ran.
- **Your time with the agents**: the time at the keyboard and the sittings, how fast the person answered an end of turn and a question, and the share of answers within 5 minutes, how long the agent's turns took, the ends of turn per hour at the keyboard, and how long the agents stood still after the person left. A median of a pool is the pause range that holds it; `median file` is the median of the files' own medians.
- **Working with the agent**: the hand-offs per hour at the keyboard: how often people stop the agent, how often it asks, and how often background work wakes it. Interrupts, messages typed while the agent worked, questions and answers also come per 100 typed messages. Ends of turn and wakes do not, because many turns end after a wake, not after a message. `Working with the agent by client` gives the same per client, such as an IDE against a terminal.
- **Commits and pull requests**: the commits, pushes and pull requests the agents' own shell calls made, per hour at the keyboard, and the dollars of spend per commit: a rough baseline to compare people and waves, not the price of the work.
- **Tools and commands**: the tools and shell programs with the most rent, and their calls, failed calls and trips (see the `tools` command in the README).
- **Highest effort by client**: the share of each client's spend and requests at `max` or `xhigh`, costliest first. The clients under 1% of the spend fold into `<n> more`.
- **Versions**: the agent versions in use.

Each section starts with a line that says what it counts.
