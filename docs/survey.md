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

The header can also hold `about`, the person's answers to three questions, when they give them: `role`, `agents` (how long they have used coding agents) and `bother` (one or two things that bother them most when they work with agents). The answers come from fixed lists, so they hold no name, and a skipped question leaves its key out.

Before writing, tokprof searches the file for the home path, the user and host names, and the ids and titles of the sessions, and writes nothing if it finds one. After writing, it prints what the file says.

The file holds no dollars: every dollar amount is a share of the person's own spend, and every token amount a share of their tokens. `share` prints the two totals on the person's screen and does not write them, so the file can be sent under one's name. Nothing else is lost: with the two totals, every number comes back to the cent.

What the file still shows as it is: counts, active days and hours, the time at the keyboard, pauses, answers and the agent's turns. Those stay because hiding them would lose information: they are small whole numbers, so any rescaling still gives them away. The spend can also be guessed roughly from the requests and the sizes of the contexts.

Whoever collects the files knows who sent which. For pooled dollars, add `--dollars` to the command in step 5: those files show each person's spend, so collect them in a shared folder or a form that does not show the sender.

## The prompt

Fill in these four things, then send the prompt. Each person pastes it into their coding agent, such as Claude Code or Codex.
- `<tag>`: a release, such as `v0.2.0`.
- `<label>`: a label for this survey, such as `team-2026-11`.
- `<mcp servers>`: the MCP servers to count by name, comma separated, such as the servers of the IDE the group works in. When no server should be counted by name, delete `--mcp-names <mcp servers>` from the command in step 5.
- `<recipient>`: where the file goes.

Leave `<answers>` as it is: the agent fills it in from the person's answers to the three questions of step 1.

```
Please run a survey of my coding-agent logs with tokprof <tag>, an open-source profiler
(https://github.com/ignatov/tokprof). It reads the Claude Code and Codex logs on this machine
and writes one JSON file of totals with no names in it. It makes no network calls.

1. Ask me these three questions in one message, and say I can skip any of them:
   - My role: developer, qa, devops, manager, designer, researcher or other.
   - How long I have used coding agents: under 3 months, 3 to 12 months, or over a year.
   - What bothers me most with agents, one or two: cost, waiting (for the agent), quality (of the work),
     context (it loses track), setup, limits (rate or plan), or nothing.
   Turn my answers into <answers>, such as role=developer,agents=over-1y,bother=waiting+cost
   (agents is under-3m, 3-12m or over-1y). Leave out what I skip, and drop --about <answers>
   below if I skip all three.
2. Make a new temporary folder, and pick the archive for this machine:
   - Mac with Apple silicon: tokprof-aarch64-apple-darwin.tar.gz
   - Mac with Intel: tokprof-x86_64-apple-darwin.tar.gz
   - Linux on x86-64: tokprof-x86_64-unknown-linux-gnu.tar.gz
   - Linux on ARM64: tokprof-aarch64-unknown-linux-gnu.tar.gz
   - Windows on x86-64: tokprof-x86_64-pc-windows-msvc.zip
   - Windows on ARM64: tokprof-aarch64-pc-windows-msvc.zip
3. Download the archive and SHA256SUMS from https://github.com/ignatov/tokprof/releases/download/<tag>/
   into the folder. If a sandbox blocks the network, ask me to allow these two downloads.
4. Check the archive's SHA-256 against its line in SHA256SUMS. If they differ, stop and tell me.
   If gh is installed, also run: gh attestation verify <archive> -R ignatov/tokprof
5. Unpack the archive in the folder, and from there run
   ./tokprof --since 30d share --survey <label> --about <answers> --mcp-names <mcp servers> -o <folder>/tokprof-share.json
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
tokprof pool --where about.role=developer ~/survey   # only the files of people who said they are developers
```

`--where about.<key>=<value>` cuts the report by an answer about the person, such as `about.agents=under-3m` or `about.bother=waiting`, and may be given several times. Files without that answer are left out, and the first line says how many files were kept.

The pool skips a file that is not a share file, one of a newer format, and a second copy of a file, with a line for each. It reads files of format 1 and 2 too: the tables built from the parts that format 1 lacks say how many files they count.

When a file holds no dollars, the report weighs every person the same, and each dollar prints as a share: `pooled` is the mean of the people's shares, such as `8.0%` of their spend on cold returns, and a rate per active hour is a share of one person's spend per hour. The header says so. Counts, times and the people of each pain are the same as in dollars. To get a team's money, multiply its bill by a share only when the people spend alike; one heavy user moves the bill but not the mean share.

Before you show a pooled report to anyone outside the group, use `--min-files 3`. Then no row describes one or two people. In a small group, a pooled report still shows each person's range, so ask the group first.

## Reading the report

- **The header**: how many files, the days they cover, the survey labels, the spend in all and per file at the median and quarters, and the activity: active hours, the hours at the keyboard, and the dollars per hour of each, pooled and at the median file. An active hour is a clock hour in which an agent sent a request of a main conversation, so it counts the agents' hours; the hours at the keyboard are the person's (see the rhythm in the README).
- **At a glance**: six lines that sum up the sections below: spend, what the cache lost, cold returns, problems, the person's time, and what the agents made.
- **Pains side by side**: for a pool, one row per pain to choose from: cold returns, contexts of 400k tokens or more, sessions longer than a day, failures, waits for retries, interrupts, questions from the agent, and the highest effort. `people` counts the files the pain touches: where it happened in a quarter of their active days or main sessions or more, or, for effort, where half of their spend or more was at max or xhigh. `kind` says whether the dollars could have been saved (`avoidable`) or were only spent in the pain (`spent`), or whether the row is time. `median file per active hour` and `pooled` are the numbers of the rows in the sections below, so they can be checked there. The rows overlap, so their money and time do not add up. The rows come with the most people first, then the most dollars.
- **What people said**: for a pool, how many people gave each answer to the three questions, and how many did not answer. Next to each answer of `bother`, `the pain touches` says how many of the people who gave it the related measured pains touch, by the rule of `Pains side by side`: `cost` any pain with dollars; `waiting` (for the agent) the waits for retries; `context` the contexts of 400k tokens or more and the sessions longer than a day. `quality`, `setup`, `limits` and `nothing` have no measured pain. This compares what people feel with what the logs show. With `--min-files`, answers that fewer files gave fold into `<n> more`.
- **Where the money went**: spend by model, cost component, content of the context, origin, activity, prompt size, how long the sessions ran, and client. `share` is of the pooled spend. `median file` is the median of each file's own share, which a single big spender cannot move. `files` counts the files with the row. Under the content table, a line says what share of the spend the logs hold the prompts of; the rest is `unknown`, as for Claude Code before 2.1.300. Under the model table, a line names the models without a price, such as Codex's `codex-auto-review`: their tokens count as $0, so the spend is that much short.
- **What could have been saved**: avoidable dollars by cause of cache miss, and what the cheaper cache lifetime would have saved over the one in use, in all and per active hour. `files 5%+` counts the files where the row is 5% or more of their spend. `Cold returns by client` gives each client's active hours, its cold returns, and their avoidable dollars per active hour: the number to compare before and after a change in a client. It lists the 8 clients with the most active hours, then `<n> more`. `Coming back after a cold break` gives what the cold returns cost as resumed, against `/compact` first, a new session from a handoff note, and heartbeats, as `tokprof bench` replays them, and how often each way was the cheapest.
- **Problems**: failed, refused and denied calls, the spend after failures, the requests that waited for retries and how long, and the requests of a minute or more, pooled and per active hour, with the median file's rate. Under the retried requests, a row per class of their last error: server errors, rate limits, authentication, timeouts, connection, and `computer asleep`, waits that the computer's sleep caused rather than the API.
- **How often it happens**: in how many of a person's active days and main sessions something happened at least once: a cold return, a wait for retries, an interrupt, spend after a failed call, a context of 400k tokens or more, a session longer than a day, a question from the agent. `files` counts the people it happened to at all, `files 25%+` those it happened to in a quarter of their days or sessions or more. This tells a pain most people have from one that is costly for a few. The session rows count only the sessions someone typed in; a line under the table says how many scripted or temporary-folder sessions they left out.
- **How the sessions spread**: per agent, the quarters and the 90th percentile of the cost of a session, of the first request after a cold return, of the typed messages per session, of the agent's turns and the person's answers, and of the largest context. A file gives them only from 5 values on. For a pool, `p50` is the median of the files' own medians and `q1–q3` their quarters, so one heavy user cannot move it; `p90` shows the rare expensive sessions. `--min-files` leaves out a row that fewer files have.
- **Habits that cost the most**: the spend in prompts of 400k tokens or more, in sessions longer than a day, in single lookups, in thinking, in setup, at the highest effort, and in each tool hint. These rows show where to look, not what could be saved: they overlap.
- **Where the agent's time went**: the busy time by what the agent waited on, with the part after the end of a turn, while its own subagents or background commands still ran.
- **Your time with the agents**: the time at the keyboard and the sittings, how fast the person answered an end of turn and a question, and the share of answers within 5 minutes, how long the agent's turns took, the ends of turn per hour at the keyboard, and how long the agents stood still after the person left. A median of a pool is the pause range that holds it; `median file` is the median of the files' own medians.
- **Working with the agent**: the hand-offs per hour at the keyboard: how often people stop the agent, how often it asks, and how often background work wakes it. Interrupts, messages typed while the agent worked, questions and answers also come per 100 typed messages. Ends of turn and wakes do not, because many turns end after a wake, not after a message. `Working with the agent by client` gives the same per client, such as an IDE against a terminal.
- **Commits and pull requests**: the commits, pushes and pull requests the agents' own shell calls made, per hour at the keyboard, and the dollars of spend per commit: a rough baseline to compare people and waves, not the price of the work.
- **Tools and commands**: the tools and shell programs with the most rent, and their calls, failed calls and trips (see the `tools` command in the README).
- **Highest effort by client**: the share of each client's spend and requests at `max` or `xhigh`, costliest first. The clients under 1% of the spend fold into `<n> more`.
- **Versions**: the agent versions in use.

Each section starts with a line that says what it counts.
