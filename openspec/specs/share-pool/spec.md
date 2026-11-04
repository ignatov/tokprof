# share-pool Specification

## Purpose
Pools the share files of several people into one report: where their money went, what could have been saved, which habits cost the most, and which tools carry the spend, without showing any one person's work. It also holds the guide and the prompt for running a survey.

## Requirements

### Requirement: Pool command
`tokprof pool <path>…` SHALL read share files (see share-export), given as files or as folders, and print one report over them. In a folder it reads the `.json` files, but not those of its subfolders. It reads no session logs and takes none of the global filters. When the paths hold no share file, it fails with `no share files in <paths>`.

#### Scenario: A folder
- **WHEN** `tokprof pool ~/survey` runs on a folder that holds 7 share files, a `notes.txt`, and a subfolder with more share files
- **THEN** the report covers the 7 files

#### Scenario: Nothing to pool
- **WHEN** `tokprof pool ~/survey` runs and the folder holds no share file
- **THEN** the command fails with `no share files in ~/survey`

#### Scenario: A filter
- **WHEN** `tokprof --since 7d pool ~/survey` runs
- **THEN** it fails with `pool takes no filters: each file was filtered when it was written`

### Requirement: Files the pool skips
The pool SHALL skip a file that is not a share file, one whose format is newer than it reads, and one whose content equals a file it already read. It reads formats 1 to 3. For each file it skips, it prints one line on stderr: `skipped <path>: not a tokprof share file`, `skipped <path>: format <n> needs a newer tokprof` or `skipped <path>: the same as <path>`.

#### Scenario: A file sent twice
- **WHEN** a folder holds `a.json` and `a (1).json` with the same content
- **THEN** the report counts one file, and stderr says `skipped a (1).json: the same as a.json`, with the paths as given

#### Scenario: Both formats
- **WHEN** one file has format 1 and another format 2
- **THEN** both are read

#### Scenario: A newer format
- **WHEN** a file has format 4
- **THEN** stderr says `skipped <path>: format 4 needs a newer tokprof`

### Requirement: Pool header
The report SHALL start with three lines:
- `pool`: the number of files, with `of <n>` and the clauses of `--where` when one is given (see Files by answer), the first and last day of their windows, and their survey labels;
- `agents`: the number of files with each agent;
- `spend`: the pooled spend, and the median and quarters of the spend per file.

Medians and quarters are taken as the pause report takes them (see pause-report).

#### Scenario: Five files
- **WHEN** 5 files of the survey `team-2026-11` cover 2026-10-04 to 2026-11-03, all 5 with Claude Code and 3 with Codex, and spent $2,140, $3,900, $6,310, $9,870 and $19,000
- **THEN** the report starts with these lines:
  - `pool    5 files · 2026-10-04 – 2026-11-03 · survey team-2026-11`
  - `agents  claude 5 · codex 3`
  - `spend   $41,220 in all · per file: median $6,310, quarters $3,900 and $9,870`

#### Scenario: Files of two surveys
- **WHEN** the files carry the labels `team-2026-11` and `all-2026-12`, and one file carries none
- **THEN** the first line ends with `survey team-2026-11, all-2026-12, none`

#### Scenario: A filtered pool
- **WHEN** `--where about.role=developer` keeps those 5 files of 8
- **THEN** the first line reads `pool    5 of 8 files where about.role=developer · 2026-10-04 – 2026-11-03 · survey team-2026-11`

### Requirement: Columns of a pooled table
Each table of the report SHALL have, after its row name:
- `cost`: the pooled dollars;
- `share`: of the pooled spend;
- `median file`: the median over all files of the row's share of each file's spend, where a file without the row counts 0;
- `files`: the files that have the row.

A table shows its 8 costliest rows, then `<n> more` with their sum, unless its own requirement orders it otherwise.

#### Scenario: A row that two files have
- **WHEN** 2 of 5 files spent on `skill:code-review`, 20% and 30% of their spend
- **THEN** its row shows `0.0%` as `median file` and `2` as `files`

### Requirement: Where the money went
The report SHALL print `Where the money went` and one table each by model, cost component, content, origin, activity, prompt size, session span and client. Their columns are those of a pooled table. The model and client tables name the agent in their first column; the others add the agents up. The origin table lists each kind with its names under it. The prompt-size and session-span tables keep the order of their ranges.

#### Scenario: Origins with names
- **WHEN** the files have the origin kind `agent` for $9,800, of which `agent:general-purpose` is $8,400
- **THEN** the `agent` row shows $9,800, and an indented `agent:general-purpose` row under it shows $8,400

#### Scenario: Ranges in order
- **WHEN** the prompt-size table is printed
- **THEN** its rows come in the order `under 50k`, `50–100k`, `100–200k`, `200–400k`, `400–800k`, `800k+`, whatever their cost

### Requirement: What could have been saved
The report SHALL print `What could have been saved`: one row per miss cause, with its avoidable dollars (see cache-misses), and the rows `cache lifetime: main` and `cache lifetime: subagents`. For each file, a cache-lifetime row adds its replay under the lifetime the bucket wrote most with (see share-export, Cache parts) minus the cheaper replay. The columns are `avoidable`, `share`, `median file`, and `files 5%+`: the files where the row is 5% or more of their spend.

#### Scenario: Cold returns in most files
- **WHEN** 4 of 5 files have `idle > TTL` misses whose avoidable dollars are 5% or more of their spend
- **THEN** the `idle > TTL` row shows `4` under `files 5%+`

#### Scenario: A file already on the cheaper lifetime
- **WHEN** a file's main conversations wrote mostly with the 1-hour lifetime, were logged at $4,476 of input, and were replayed at $4,259 under a 1-hour and $5,365 under a 5-minute cache
- **THEN** that file adds nothing to `cache lifetime: main`
- **AND** the $217 between the logged cost and the replay, which other misses cost, stays in the rows of their causes

#### Scenario: A file on the dearer lifetime
- **WHEN** a file's subagents wrote mostly with the 1-hour lifetime, and were replayed at $2,884 under a 1-hour and $2,479 under a 5-minute cache
- **THEN** that file adds $405 to `cache lifetime: subagents`

### Requirement: Habits that cost the most
The report SHALL print `Habits that cost the most`: one row per habit in the scenario below, with the spend that the habit names, the costliest first. The columns are `cost`, `share`, `median file` and `files 5%+`. These rows show where to look, not what could be saved: they overlap, and the table adds no total.

#### Scenario: The habits
- **WHEN** the report is printed
- **THEN** the habit rows are:
  - `prompts of 400k+`: the requests whose prompt was 400k tokens or more (see share-export, Prompt size parts);
  - `sessions longer than a day`: the main sessions in `1–3d` and `over 3d`;
  - `single lookups`: the requests whose trigger is `lookup` (see cost-attribution);
  - `thinking`: the content kind `thinking`;
  - `setup`: every setup source;
  - `effort max or xhigh`: the requests at either level;
  - one row per tool hint, named `tool hint: <hint>`, with its trips for `batch independent reads` and its rent for every other hint.

#### Scenario: A format-1 file
- **WHEN** a format-1 file is pooled with format-2 files
- **THEN** it counts in `setup` and `single lookups`, which format 1 has the parts for
- **AND** the other habit rows count only the format-2 files (see Parts some files lack), `effort max or xhigh` among them, since format 1 counts no dollars per effort level

### Requirement: Tools and commands
The report SHALL print `Tools and commands`: the 12 rows with the most rent among the tools and the programs of the shell tools, with `calls`, `rent`, `trips` and `files`. A program's row is named `<tool> <program>`, such as `Bash git`. A shell tool whose programs are listed has no row of its own.

#### Scenario: Rows of a shell tool
- **WHEN** `Bash` has the programs `sed`, `git` and `other`
- **THEN** the table can list `Bash sed`, `Bash git` and `Bash other`, and has no `Bash` row

### Requirement: Versions in use
The report SHALL print `Versions`: for each agent, the 5 versions with the most requests, with `requests` and `files`, then `<n> more`.

#### Scenario: Many versions
- **WHEN** the files carry 14 Claude Code versions
- **THEN** the table lists the 5 with the most requests, then `9 more`

### Requirement: Parts some files lack
When some files lack a part, such as `content` in a format-1 file, every table built from that part SHALL say `in <k> of <n> files` after its title, and its `share`, `median file` and file counts SHALL count only those k files.

#### Scenario: A format-1 file among format-2 files
- **WHEN** 1 of 5 files has format 1
- **THEN** the content table's title ends with `in 4 of 5 files`, and its shares are of the spend of those 4 files

### Requirement: Report of one file
When the report covers one file, it SHALL leave out the `agents` line and the columns `median file`, `files` and `files 5%+`. Its first line reads `share   <first day> – <last day>`, followed by ` · survey <label>` when the file has a label. When the file has `about`, the next line reads `about   ` and its answers in the form `--about` takes them, in the order role, agents, bother.

#### Scenario: The report that share prints
- **WHEN** `tokprof share --survey team-2026-11` writes its file for 2026-10-04 to 2026-11-03
- **THEN** the report it prints starts with `share   2026-10-04 – 2026-11-03 · survey team-2026-11`, then `spend   ` and the file's spend
- **AND** the tables of money have only the row name, the dollars and `share`, and no table has a column that counts files

#### Scenario: The answers about the person
- **WHEN** `tokprof share --about bother=waiting+cost,role=developer` writes its file
- **THEN** the second line of the report reads `about   role=developer,bother=waiting+cost`

### Requirement: Rows few files share
`--min-files <n>` SHALL fold every table row that fewer than n files have into one `other` row of its table. Names under an origin kind are left out instead, and their spend stays in the kind's row. Versions that fewer than n files carry are left out. A row of `How the sessions spread` with quantiles from fewer than n files is left out too. The answers of `What people said` fold as Answers few files share says. The default is 1, which folds nothing.

#### Scenario: A rare model
- **WHEN** only 1 of 8 files used the model `claude-haiku-4-5-20251001`, and `--min-files 3` is given
- **THEN** that model is not printed, and its spend shows in the `other` row of the model table

#### Scenario: A rare name
- **WHEN** only 1 of 8 files has spend on `skill:simplify`, and `--min-files 3` is given
- **THEN** `skill:simplify` is not printed, and its spend stays in the `skill` row

#### Scenario: A spread of two people
- **WHEN** only 2 of 8 files have quantiles of `cost of a session`, and `--min-files 3` is given
- **THEN** that row is not printed, so its medians do not show one person's sessions

### Requirement: Per-file columns
`--by-file` SHALL add one column per file to each table, headed by the file's name without `.json`, with the row's share of that file's spend. The columns come in the order in which the files were read.

#### Scenario: Three files
- **WHEN** `tokprof pool --by-file a.json b.json c.json` runs
- **THEN** each table has the columns `a`, `b` and `c` after its last pooled column

### Requirement: JSON of the pooled report
`--json` SHALL print the report's numbers as one JSON object instead of the tables: the header's numbers, then each table's rows with their dollars, shares, `median file` and files, unrounded, so that another tool can chart them.

#### Scenario: Shares as numbers
- **WHEN** `tokprof pool --json ~/survey` runs
- **THEN** stdout is one JSON object, and each share is a fraction such as `0.731`, not text

### Requirement: Survey guide
The repository SHALL hold `docs/survey.md`: how to run a survey with tokprof, and a prompt to paste into a coding agent, with the placeholders `<tag>`, `<label>`, `<mcp servers>` and `<recipient>`. It says how to collect and pool the files, what `about` holds, that the file holds shares instead of dollars, and that its counts, days, hours and durations stay, since hiding them would lose information. It says to pass `--min-files 3` before a pooled report is shown to others.

#### Scenario: The steps of the prompt
- **WHEN** an agent follows the prompt with the placeholders filled in
- **THEN** it does these steps:
  - asks the person the three questions of `--about` with their answers, and says that each can be skipped;
  - picks the archive for the machine's system and CPU (see release-builds);
  - downloads the archive and `SHA256SUMS` from the release `<tag>` into a new temporary folder, and asks the person to allow it when a sandbox blocks the network;
  - checks the archive's SHA-256 against its line in `SHA256SUMS` and stops when they differ, and also runs `gh attestation verify` when `gh` is installed;
  - unpacks the archive there and runs `tokprof --since 30d share --survey <label> --about <answers> --mcp-names <mcp servers> -o <folder>/tokprof-share.json`, with the answers given and without `--about` when all three were skipped;
  - shows tokprof's output as it is;
  - tells the person where the file is, and that they should review it and send it to `<recipient>` themselves.

#### Scenario: What the prompt rules out
- **WHEN** an agent follows the prompt
- **THEN** it installs nothing, changes no settings and sends nothing anywhere
- **AND** it downloads nothing but the archive and `SHA256SUMS`, and reads neither the share file nor any log into the chat

#### Scenario: Kept MCP servers in a survey
- **WHEN** the person running the survey wants the calls of an IDE's MCP servers counted by name
- **THEN** the guide says to put their names, comma separated, in place of `<mcp servers>`, which also lists them in each file's header
- **AND** when no server should be counted by name, it says to delete `--mcp-names <mcp servers>` from the command

#### Scenario: Dollars in a survey
- **WHEN** the person running the survey wants pooled dollars
- **THEN** the guide says to add `--dollars` to the command, and that those files show each person's spend, so they are best collected without the sender

### Requirement: Activity line
The header SHALL have an `activity` line after `spend` (see share-export, Activity parts and Rhythm parts). For one file it gives the active days, the active hours, the hours at the keyboard, the busy hours, and the dollars per active hour and per hour at the keyboard. For a pool it gives the pooled hours of both kinds and their dollars per hour, then the median per file of the active days, the active hours and the dollars per active hour. Only the files with the parts count.

#### Scenario: One file
- **WHEN** a file has 18 active days, 141 active hours, 52 hours at the keyboard, 365 busy hours and $11,200 of spend
- **THEN** the line reads `activity  18 active days · 141 active hours · 52h at the keyboard · busy 365h · $79.43 per active hour · $215 per hour at the keyboard`

#### Scenario: A pool
- **WHEN** 5 files with the parts hold 700 active hours, 250 hours at the keyboard and $41,220 between them, and their medians are 16 active days, 120 active hours and $68.20 per active hour
- **THEN** the line reads `activity  700 active hours · 250h at the keyboard · $58.89 per active hour · $165 per hour at the keyboard · per file: median 16 active days, 120 active hours, $68.20 per active hour`

#### Scenario: A file without the rhythm part
- **WHEN** a file has the activity part and no rhythm part
- **THEN** its line leaves out the hours at the keyboard and the dollars per hour at the keyboard

### Requirement: Savings per active hour
`What could have been saved` SHALL have a `per active hour` column, after `share`: the row's dollars over the active hours of the files that have both the row's part and the activity part.

#### Scenario: Idle misses per hour
- **WHEN** a file has $740 of `idle > TTL` misses and 312 active hours
- **THEN** the row shows `$2.37` under `per active hour`

### Requirement: Cold returns by client
After `What could have been saved`, the report SHALL print `Cold returns by client`: one row per agent and client, the most active hours first, with `active hours`, `cold returns`, `per active hour`, and `avoidable per active hour`, the avoidable dollars of the client's cold returns. It shows at most 8 clients, then `<n> more` with the rest. A client that fewer files have than `--min-files` asks counts in `<n> more`.

#### Scenario: Ten clients
- **WHEN** a file has 10 clients with active hours
- **THEN** the table shows the 8 with the most active hours, then `2 more` with the active hours and cold returns of the other two

#### Scenario: A client's baseline
- **WHEN** a client has 312 active hours and 206 cold returns that could have saved $740, and its other misses $150 more
- **THEN** its row in `Cold returns by client` shows `312`, `206`, `0.66` and `$2.37`

### Requirement: Problems
The report SHALL print `Problems`, with the rows `failed tool calls`, `spend after failures`, `calls the user refused`, `calls a rule denied`, `requests that waited for retries`, `time waiting for retries` and `requests of a minute or more`, and no row for the longest request. The columns are `pooled`, `per active hour`, `median file` (of each file's value per active hour) and `files` (the files where the value is above zero).

#### Scenario: Failures of one person
- **WHEN** one file has 1,390 failed calls, $226 of spend after them, and 141 active hours
- **THEN** the row `failed tool calls` shows `1,390` and `9.86`, and the row `spend after failures` shows `$226` and `$1.60`

#### Scenario: Slow requests
- **WHEN** 3,001 of 83,890 requests took a minute or more
- **THEN** the row `requests of a minute or more` shows `3,001 (3.6%)` under `pooled`

#### Scenario: A request across a sleep
- **WHEN** a Codex request seems to take 2 hours, because the computer slept between a tool's output and the model's answer, and Codex logs no send times
- **THEN** the report has no row for the longest request, which such a request would decide, and the request counts once among the requests of a minute or more

### Requirement: Where the agent's time went
The report SHALL print `Where the agent's time went`, one row per kind of busy time (see agent-time), most time first, with `time` in hours, `share` of the busy time, `median file` (each file's share), `files` and `after turns`.

#### Scenario: Subagents after turns
- **WHEN** subagents took 67.5 hours of 365 busy hours, 54 hours of them after ends of turn
- **THEN** the `subagents` row shows `67.5h`, `18.5%` and `54.0h` under `after turns`

### Requirement: Working with the agent
The report SHALL print `Working with the agent`, with the rows of the scenario below. The columns are `pooled`, `per hour at the keyboard`, `per 100 typed messages`, `median file` (of each file's value per hour at the keyboard) and `files`. Only interrupts, messages typed while the agent worked, questions and answers have a value per 100 typed messages. Typed messages come from the pause parts, and the hours at the keyboard from the rhythm parts.

#### Scenario: A file without active hours
- **WHEN** two files have 1,168 and 1,158 typed messages, and only the first has the activity and rhythm parts, with 151 hours at the keyboard
- **THEN** the `typed messages` row counts the first file only: it shows `1,168` and `7.74`, and says `(in 1 of 2 files)`

#### Scenario: The rows
- **WHEN** the report is printed
- **THEN** the rows are `typed messages`, `ends of turn`, `interrupts`, `typed while the agent worked`, `questions`, `answers`, `wakes: subagent`, `wakes: command` and `wakes: other`

#### Scenario: Interrupts
- **WHEN** a file has 1,030 typed messages, 41 interrupts and 50 hours at the keyboard
- **THEN** the `interrupts` row shows `41`, `0.82` and `3.98`

#### Scenario: Ends of turn
- **WHEN** a file has 2,671 ends of turn, many after wakes, over 151 hours at the keyboard
- **THEN** the `ends of turn` row shows `2,671`, `17.69` and `–` under `per 100 typed messages`

### Requirement: Highest effort by client
The report SHALL print `Highest effort by client`, one row per agent and client, costliest first. The columns are `cost`, the client's pooled spend; `of spend` and `of requests`, the share of that spend and of its requests at `max` or `xhigh`; then `median file` (of the share of spend) and `files`. It shows the clients with 1% of the pooled spend or more, at most 8, then `<n> more` with the cost of the rest.

#### Scenario: A client at max
- **WHEN** a client spent $800 of $850 at `max`, in 900 of 1,000 requests
- **THEN** its row shows `$850`, `94.1%` and `90.0%`

#### Scenario: A small client
- **WHEN** a client spent $0.91 of a file's $11,356, 98.2% of it at `max`
- **THEN** it has no row of its own, and its cost counts in `<n> more`

### Requirement: Prompt coverage line
When the share of spend whose prompts the logs hold is below 100%, the report SHALL print, under the content table, `prompts the logs hold: <x>% of the spend; the rest is unknown`. It counts the files that have the part. For a pool, the line ends with `, in <k> files below 90%`.

#### Scenario: One file with old transcripts
- **WHEN** 88.2% of a file's spend comes from requests whose prompts its logs hold
- **THEN** the line reads `prompts the logs hold: 88.2% of the spend; the rest is unknown`

### Requirement: Models without a price
When a file has a model without a price (see share-export, Spend parts), the report SHALL print, under the model table, `models without a price:` and each such model with its tokens, then `; their tokens count as $0`. For a pool, each model also names its files, and `--min-files` leaves a model out as it does a version. The line leaves out the price file that pricing names for such models: each file was priced on its own machine (see pricing, Requests without a price).

#### Scenario: Codex's auto-review
- **WHEN** a file's requests to `codex-auto-review` used 101,273,848 tokens, and that model has no price
- **THEN** the line reads `models without a price: codex-auto-review (101.3M tokens); their tokens count as $0`

#### Scenario: A pool
- **WHEN** 3 of 5 files have requests to `codex-auto-review` without a price
- **THEN** its entry in the line ends with `, 3 files)`

### Requirement: Failures in the tool table
`Tools and commands` SHALL show `failed`, the failed calls of each row, after `calls`.

#### Scenario: A failing program
- **WHEN** `Bash git` had 120 failed calls
- **THEN** its row shows `120` under `failed`

### Requirement: Order of the sections
After the header, the report SHALL print its sections in the order of the scenario below. A section that no file has the parts for is left out.

#### Scenario: The order
- **WHEN** the files have every part
- **THEN** the sections come in this order:
  - `At a glance`;
  - `Pains side by side`, in a pool only;
  - `What people said`, in a pool only;
  - `Where the money went`;
  - `What could have been saved` with `Cold returns by client` and `Coming back after a cold break`;
  - `Problems`;
  - `How often it happens`;
  - `How the sessions spread`;
  - `Habits that cost the most`;
  - `Where the agent's time went`;
  - `Your time with the agents`;
  - `Working with the agent` with `Working with the agent by client`;
  - `Commits and pull requests`;
  - `Tools and commands`;
  - `Highest effort by client`;
  - `Versions`.

#### Scenario: Format-1 files only
- **WHEN** every pooled file has format 1
- **THEN** the report has neither `Pains side by side`, `What people said`, `Problems`, `How often it happens`, `How the sessions spread`, `Where the agent's time went`, `Your time with the agents`, `Working with the agent`, `Coming back after a cold break`, `Commits and pull requests` nor `Highest effort by client`
- **AND** `At a glance` has only the lines whose parts format 1 has

#### Scenario: Files written before the rhythm parts
- **WHEN** every pooled file has format 2 but none has the rhythm, bench, outcome, prevalence or distribution parts
- **THEN** the report has none of the sections built from them, and `Working with the agent` is left out, since its base is missing

### Requirement: At a glance
After the header, the report SHALL print `At a glance`: the lines `spend`, `cache`, `returns`, `problems`, `you` and `made`. Each sums up a section below, and each is left out when no file has the parts it needs. For a pool, a line gives the pooled values, with the median file's rates in brackets.

#### Scenario: One file
- **WHEN** a file spent $11,356 in 395 active hours and 151 hours at the keyboard, could have saved $1,750 with `idle > TTL` $812 and `cache lifetime: subagents` $405 its largest rows, and has the other parts as below
- **THEN** the section reads:
  - `spend     $11,356 · $28.75 per active hour · $75.20 per hour at the keyboard`
  - `cache     $1,750 could have been saved (15.4%): idle > TTL $812, cache lifetime: subagents $405`
  - `returns   221 cold returns cost $1,722 as resumed; a handoff note would have cost $550 and was cheapest in 221; heartbeats $1,358`
  - `problems  1,398 failed calls, $227 after them; 92 requests waited 23.3h for retries, 87 of them for server errors`
  - `you       151h at the keyboard in 190 sittings; you answered in a median 3.7m; the agents stood still a median 8.0m after you left`
  - `made      the agents made 410 commits, 95 pushes and 12 pull requests: $27.70 of spend per commit`

#### Scenario: A pool
- **WHEN** 5 files spent $41,220 in 700 active hours, and their median file spent $68.20 per active hour
- **THEN** the `spend` line starts `spend     $41,220 · $58.89 per active hour (median file $68.20)`

#### Scenario: A pool with a file from before this change
- **WHEN** of 2 files, only one has the rhythm, bench and outcome parts and classes its retried requests
- **THEN** the `returns`, `you` and `made` lines end with `(in 1 of 2 files)`, as does the hours-at-the-keyboard part of the `spend` and `activity` lines
- **AND** the `problems` line says how many of the requests with a class had the main one, such as `87 of the 92 with a class for server errors`

### Requirement: Section notes
Every section title SHALL be followed by one line that says what the section counts. An active hour is a clock hour in which an agent sent a main-conversation request (see share-export, Activity parts). An hour at the keyboard is time the person sat at the agents (see work-rhythm, Sittings and breaks). A title keeps its `in <k> of <n> files` when some files lack its parts.

#### Scenario: The notes
- **WHEN** the report is printed
- **THEN** the notes are:
  - `Where the money went`: `what the spend paid for: model, content, origin, activity, prompt size, session length and client`
  - `What could have been saved`: `dollars the prompt cache lost to misses, by cause, and what the cheaper cache lifetime would have kept`
  - `Cold returns by client`: `returns to a session after its cache expired, per active hour`
  - `Coming back after a cold break`: `what the cold returns cost as resumed, against the other ways to come back (see tokprof bench)`
  - `Problems`: `calls that failed, requests that waited for retries, and slow requests, per active hour`
  - `How often it happens`: `the active days and main sessions in which each happened at least once`
  - `How the sessions spread`: `per session, cold return, turn or answer: the quarters and the 90th percentile, given from 5 values on`
  - `Habits that cost the most`: `where to look, not what could be saved; the rows overlap`
  - `Where the agent's time went`: `what the agents waited on while busy; after turns: while their own background work ran on`
  - `Your time with the agents`: `your sittings and answers, and what the agents did after you left`
  - `Working with the agent`: `hand-offs per hour at the keyboard, and per 100 typed messages for those a message starts`
  - `Working with the agent by client`: `the same by the program that ran the agent, such as an IDE or a terminal`
  - `Commits and pull requests`: `what the agents committed, pushed and opened, from their shell calls that did not fail`
  - `Tools and commands`: `rent: what keeping a tool's results in the context cost; trips: the requests its calls made the agent send`
  - `Highest effort by client`: `the share of each client's spend and requests at max or xhigh`
  - `Versions`: `the agent versions in use, by requests`

#### Scenario: Notes of a pool
- **WHEN** the report covers several files
- **THEN** the note of `Pains side by side` is `each pain by the people it touches: in a quarter of their days, sessions or spend or more, or half of their spend for effort; spent is what that work cost, not what changing it would save; the rows overlap, so money and time are not added up`
- **AND** the note of `What people said` is `the answers to tokprof share --about; the pain touches: of the people who gave an answer of bother, those its measured pains touch, as Pains side by side counts them`
- **AND** the note of `Where the money went` ends with `; median file: the median of each file's own share`
- **AND** the note of `What could have been saved` ends with `; files 5%+: the files where a row is 5% of their spend or more`
- **AND** the note of `How often it happens` ends with `; files: where it happened at all; files 25%+: in a quarter of their days or sessions or more`
- **AND** the note of `How the sessions spread` ends with `; p50 and q1–q3: the median and quarters of the files' own p50; p90: the median of their p90`

### Requirement: Coming back after a cold break
After `Cold returns by client`, the report SHALL print `Coming back after a cold break`, from the bench parts of all agents (see share-export, Bench parts). It has one row per way, named as the bench names it, with `cost`, the pooled cost of all returns that way; `of resume`, that cost over the cost as resumed; and `cheapest in`, the returns where the way cost least, of all returns. For a pool, `median file` gives each file's `of resume`.

#### Scenario: Two agents
- **WHEN** a file's returns cost $1,644 and $77.89 as resumed and $518 and $32.22 with a handoff note, and the note was cheapest in 203 of 203 and 18 of 18
- **THEN** the row `new session + handoff note` shows `$550`, `32.0%` and `221 of 221`

#### Scenario: No returns
- **WHEN** no file has a cold return
- **THEN** the section is left out

### Requirement: Your time with the agents
The report SHALL print `Your time with the agents`, from the rhythm parts (see share-export, Rhythm parts), with the rows of the scenario below. A total or count is pooled. For a median, `pooled` is the pause range that holds the median of the pooled ranges, and `median file` is the median of the files' medians. Shares of answers within 5 minutes are pooled over all answers.

#### Scenario: The rows
- **WHEN** the report is printed
- **THEN** its rows are `time at the keyboard`, `sittings`, `you answered an end of turn in`, `… within 5 minutes`, `you answered a question in`, `… within 5 minutes`, `the agent's turn took`, `ends of turn while you were there`, `turns the agent started itself`, `the agents stood still after you left` and `the agents worked through a break`

#### Scenario: One file
- **WHEN** a file has 151 hours at the keyboard, 190 sittings, and answers to ends of turn with a median of 3.7 minutes, 62% of them within 5 minutes
- **THEN** its rows show `151.0h`, `190`, `3.7m` and `62%`

#### Scenario: Ends while the person was there
- **WHEN** 2,013 turns ended inside sittings that add up to 151 hours
- **THEN** the row `ends of turn while you were there` shows `13.33` per hour at the keyboard

### Requirement: Working with the agent by client
After `Working with the agent`, the report SHALL print `Working with the agent by client`: one row per agent and client with time at the keyboard, the most first. It shows those with 1% of the pooled time at the keyboard or more, at most 8, then `<n> more`. The columns are `at the keyboard`; `messages`, `interrupts`, `questions` and `commits` per hour at the keyboard; and `answered in`, the median answer to an end of turn, for a pool the median of the files'.

#### Scenario: An IDE against a terminal
- **WHEN** `sdk-ts` has 120 hours at the keyboard, 900 messages, 30 interrupts and 300 commits, and `cli` has 20 hours, 150 messages, 2 interrupts and 40 commits
- **THEN** the `sdk-ts` row shows `120.0h`, `7.50`, `0.25` and `2.50` under `commits`, and the `cli` row `20.0h`, `7.50`, `0.10` and `2.00`

#### Scenario: A client of minutes
- **WHEN** a client has 30 seconds at the keyboard, against 100 hours of another
- **THEN** it has no row of its own: its rates would read as if it were used all day, and it counts in `<n> more`

### Requirement: Commits and pull requests
The report SHALL print `Commits and pull requests`, from the outcome parts (see share-export, Outcome parts): the rows `commits`, `pushes` and `pull requests`, with `pooled`, `per hour at the keyboard`, `median file` and `files`. Under the table a line gives the dollars per commit: the spend of the files with the part over their commits.

#### Scenario: One file
- **WHEN** a file has 410 commits, 95 pushes and 12 pull requests, $11,356 of spend and 151 hours at the keyboard
- **THEN** the `commits` row shows `410` and `2.72`, and the line under the table reads `$27.70 of spend per commit`

#### Scenario: No commits
- **WHEN** the files have the outcome parts and no commit
- **THEN** the table shows zeros, and the line reads `no commits by the agents`

### Requirement: Retries by error class
`Problems` SHALL list, under `requests that waited for retries`, one indented row per error class with requests: `server errors`, `rate limits`, `authentication`, `timeouts`, `connection`, `computer asleep` and `other errors` (see share-export, Retry error classes). The rows have the columns of `Problems`.

#### Scenario: Mostly gateway errors
- **WHEN** 92 requests waited for retries, 87 after a server error and 5 after a timeout
- **THEN** `server errors` shows `87` and `timeouts` shows `5` under `requests that waited for retries`, and no other class has a row

### Requirement: Counts with separators
Every count of 1,000 or more in the report SHALL have thousands separators, as the tables of money already have them. This holds for the calls and failed calls of `Tools and commands` and the requests of `Versions` too.

#### Scenario: Calls of a tool
- **WHEN** `Read` had 15080 calls
- **THEN** its row shows `15,080`

### Requirement: How often it happens
After `Problems`, the report SHALL print `How often it happens`, from the prevalence parts (see share-export, Prevalence parts), with one row per count of the scenario below. The day rows come from the file's own prevalence part; the session rows from the agents' parts. The columns are `count`, `of` (the active days, or the main sessions someone worked in, of the same files) and `share`.

#### Scenario: The rows
- **WHEN** the report is printed
- **THEN** its rows are `days with a cold return`, `days a request waited for retries`, `days you stopped the agent`, `sessions with a cold return`, `sessions with spend after a failure`, `sessions with a context of 400k+`, `sessions longer than a day` and `sessions where the agent asked`

#### Scenario: One file
- **WHEN** a file had a cold return on 28 of its 30 active days
- **THEN** the row `days with a cold return` shows `28`, `30 days` and `93.3%`

### Requirement: Sessions the rows leave out
Under the tables of `How often it happens` and `How the sessions spread`, the report SHALL print `the session rows leave out <n> sessions that nobody typed in: a program ran them, or they ran in a temporary folder`, with the `left_out_sessions` of the files summed (see share-export, Sessions nobody typed in). With none left out, there is no such line.

#### Scenario: Scripted runs
- **WHEN** a file left out 129 Claude Code and 126 Codex sessions
- **THEN** both tables end with `the session rows leave out 255 sessions that nobody typed in: a program ran them, or they ran in a temporary folder`

### Requirement: Files in How often it happens
For a pool, the counts of `How often it happens` SHALL be pooled, and the columns `median file` (of each file's share), `files` (the files where the count is above zero) and `files 25%+` (the files where it is 25% or more of their days or sessions) follow. Each count of files comes with its share of the files that have the part. `--min-files` leaves these rows as they are, since they count files.

#### Scenario: A pool
- **WHEN** 4 files had a cold return on 15, 0, 6 and 30 of their 30 active days, and a fifth file has no prevalence part
- **THEN** the title reads `How often it happens, in 4 of 5 files`
- **AND** the row `days with a cold return` shows `51`, `120 days`, `42.5%`, `50.0%` under `median file`, `3 (75%)` under `files` and `2 (50%)` under `files 25%+`

### Requirement: How the sessions spread
After `How often it happens`, the report SHALL print `How the sessions spread`, from the distribution parts (see share-export, Distribution parts), with one row per agent and measure of the scenario below. A row needs one file with quantiles for it. `values` is the number of values, summed over the files. For one file the columns `p25`, `p50`, `p75` and `p90` give the file's quantiles.

#### Scenario: The measures
- **WHEN** the files have every measure
- **THEN** each agent's rows are `cost of a session`, `first request after a cold return`, `typed messages per session`, `the agent's turn`, `your answer to an end of turn` and `largest context of a session`

#### Scenario: One file
- **WHEN** a file's 100 Claude Code sessions have `p25` $0.01, `p50` $0.50, `p75` $15.00 and `p90` $67.00
- **THEN** the row `claude  cost of a session` shows `100`, `$0.01`, `$0.50`, `$15.00` and `$67.00`

#### Scenario: Too few values
- **WHEN** no file has 5 cold returns of Codex
- **THEN** there is no row `codex  first request after a cold return`

### Requirement: Spread over files
For a pool, `How the sessions spread` SHALL show `p50`, the median of the files' `p50`; `q1–q3`, their quarters; `p90`, the median of their `p90`; and `files`, the files with quantiles, as `<k> of <n>` when n files have the measure but only k have quantiles. Medians and quarters are taken as the pool header takes them.

#### Scenario: A pool
- **WHEN** 5 files' sessions cost a median $1, $2, $3, $4 and $5, each with a `p90` of $67 over 100 sessions, and a sixth file has 3 sessions and no quantiles
- **THEN** the row `claude  cost of a session` shows `503`, `$3.00`, `$2.00–$4.00`, `$67.00` and `5 of 6`

### Requirement: Pains side by side
For a pool, the report SHALL print `Pains side by side` right after `At a glance`, with one row per pain of the scenario below, the most people first, then the most dollars. The columns are `pain`, `people`, `kind`, `median file per active hour` and `pooled`. Each number is one that another section prints, and the section reads no other part of the files. The report of one file has no such section.

#### Scenario: The pains
- **WHEN** the files have every part
- **THEN** the rows, with the row their people come from and the row their money or time comes from, are:
  - `cold returns`: `days with a cold return` of `How often it happens`; `idle > TTL` of `What could have been saved`;
  - `contexts of 400k+`: `sessions with a context of 400k+`; `prompts of 400k+` of `Habits that cost the most`;
  - `sessions longer than a day`: `sessions longer than a day`; the habit `sessions longer than a day`;
  - `failures`: `sessions with spend after a failure`; `spend after failures` of `Problems`;
  - `waits for retries`: `days a request waited for retries`; `time waiting for retries` of `Problems`;
  - `interrupts`: `days you stopped the agent`; none;
  - `the agent asks`: `sessions where the agent asked`; none, since the files hold the median of the answers to questions, not their total;
  - `effort max or xhigh`: the habit `effort max or xhigh`, for both.

#### Scenario: One file
- **WHEN** `tokprof share` prints the report of the file it wrote
- **THEN** the report has no `Pains side by side`

#### Scenario: The order of the rows
- **WHEN** `effort max or xhigh` ($9,073), `cold returns` ($792) and `failures` ($226) each touch 7 of 8 people, and `contexts of 400k+` touches 5
- **THEN** the rows come in that order, and a row with time or without money comes after the dollar rows that touch as many people

### Requirement: People a pain touches
`people` SHALL read `<k> of <n>`. n is the files that have the fields the pain's people come from: for a count of `How often it happens`, as that section counts its files; for effort, the files with dollars per effort level. k is those where the pain happened in 25% or more of their active days or main sessions, as `files 25%+` counts them, or, for `effort max or xhigh`, that have 50% or more of their spend at max or xhigh. `--min-files` leaves these rows as they are.

#### Scenario: A file from before the prevalence parts
- **WHEN** 3 files had a cold return on 12, 12 and 3 of their 30 active days, and a fourth file has no prevalence part
- **THEN** the row reads `cold returns (in 3 of 4 files)` and shows `2 of 3` under `people`

#### Scenario: Effort at half of the spend
- **WHEN** 3 files spent 50%, 49% and 90% of their spend at max effort
- **THEN** `effort max or xhigh` shows `2 of 3`

#### Scenario: No file with the fields
- **WHEN** no file has dollars per effort level
- **THEN** there is no row `effort max or xhigh`

### Requirement: Size pains touch through their spend
`contexts of 400k+` and `sessions longer than a day` SHALL also touch a file whose spend in their row of `Habits that cost the most` is 25% or more of its spend. A few expensive sessions are a pain of their spend, not of their count.

#### Scenario: A few long sessions that hold most of the spend
- **WHEN** a file's sessions longer than a day are 10% of its main sessions and 60% of its spend
- **THEN** `sessions longer than a day` touches that file

#### Scenario: Neither many nor expensive
- **WHEN** a file's prompts of 400k+ are in 2 of its 40 main sessions and 10% of its spend
- **THEN** `contexts of 400k+` does not touch that file

### Requirement: Money and time of a pain
`kind` SHALL say what a row's money is: `avoidable`, dollars that could have been saved; `spent`, dollars spent in the pain; `time`, time waited; `–` for none. `pooled` is the pooled value of the row it comes from. `median file per active hour` is the median, over the files with that row's part and active hours, of each file's value over its active hours; for a row of `Problems`, its `median file`. The rows overlap, so the section adds up neither money nor time.

#### Scenario: Agreement with the other sections
- **WHEN** a pool has $792 of `idle > TTL`, and its `time waiting for retries` is 23.8h pooled and 1.8m per active hour in the median file
- **THEN** `cold returns` shows `avoidable` and `$792` under `pooled`
- **AND** `waits for retries` shows `time`, `1.8m` and `23.8h`

#### Scenario: Idle misses per active hour
- **WHEN** 3 files with 50 active hours each have $25, $50 and $100 of `idle > TTL`
- **THEN** `cold returns` shows `$1.00` under `median file per active hour`

#### Scenario: Pains without money
- **WHEN** the report is printed
- **THEN** `interrupts` and `the agent asks` show `–` under `kind`, `median file per active hour` and `pooled`

### Requirement: Files by answer
`--where about.<key>=<value>` SHALL pool only the files whose `about` holds that answer (see share-export, About the person). It may be given several times, and all must hold. A file holds `about.bother=<value>` when the value is one of its answers. The keys and answers are those of `--about`, and any other fails as there. When no file is kept, the command fails with `no share file has <clauses>`, the clauses joined with ` and `.

#### Scenario: Developers only
- **WHEN** `tokprof pool --where about.role=developer ~/survey` runs over 8 files, 5 of which have `"role": "developer"`
- **THEN** the report covers those 5 files, and its first line starts `pool    5 of 8 files where about.role=developer`

#### Scenario: Two conditions
- **WHEN** `--where about.role=developer --where about.bother=waiting` is given
- **THEN** the report covers the developers with `waiting` among their answers, and the first line says `where about.role=developer and about.bother=waiting`

#### Scenario: One file kept
- **WHEN** the filter keeps 1 of 8 files
- **THEN** the report is that of one file, and its first line ends with ` · 1 of 8 files where about.role=qa`

#### Scenario: No file kept
- **WHEN** no file has `"role": "qa"`
- **THEN** the command fails with `no share file has about.role=qa`

### Requirement: What people said
For a pool where a file has `about`, the report SHALL print `What people said` right after `Pains side by side`, with the columns `question`, `answer`, `people` and `the pain touches`, and the rows of Answers of a question. `people` is `<k> (<share>)` of the pooled files. The report of one file has no such section.

#### Scenario: No answers
- **WHEN** no pooled file has `about`
- **THEN** the report has no `What people said`

### Requirement: Answers of a question
Each question of `What people said` SHALL have one row per answer that some file gave, in the order of the lists (see share-export, Answers about the person), then `not answered`: the files without `about`, without that key, or with an answer outside the lists. A person may give two answers to `bother`, so its rows can add up to more than the files.

#### Scenario: Eight files
- **WHEN** of 10 files, 5 have `"role": "developer"`, 1 has `"role": "qa"`, 1 has `about` without `role`, and 3 have no `about`
- **THEN** the rows of `role` are `developer` with `5 (50%)`, `qa` with `1 (10%)` and `not answered` with `4 (40%)`

### Requirement: Pains behind what bothers people
For each answer of `bother`, `the pain touches` SHALL read `<k> of <n>`: n is the people who gave the answer and whose files have the fields of at least one of its pains (see The pains of each answer), and k those that at least one of its pains touches. A pain touches a file as `people` of `Pains side by side` counts it (see People a pain touches). The rows of `role`, `agents` and `not answered` leave the column empty.

#### Scenario: Cost
- **WHEN** 3 people said `cost`, and `Pains side by side` counts 2 of them among the people of `effort max or xhigh` and none of the third's pains
- **THEN** the row `cost` shows `2 of 3`

### Requirement: The pains of each answer
The answers of `bother` SHALL have these pains:
- `cost`: every pain of `Pains side by side` whose kind is `avoidable` or `spent`;
- `waiting`, the person waiting for the agent: `waits for retries`. The agent's questions and the agents standing still after the person left are the agent waiting for the person, so they are not its pains;
- `context`: `contexts of 400k+` and `sessions longer than a day`;
- `quality`, `setup`, `limits` and `nothing`: none, shown as `–`.

#### Scenario: Waiting is the person's
- **WHEN** a person said `waiting`, requests waited for retries on 2 of their 20 active days, and in 12 of their 40 breaks the agents stood still under 5 minutes after they left
- **THEN** the row `waiting` counts them in n but not in k

#### Scenario: An answer without a measured pain
- **WHEN** 2 people said `quality`
- **THEN** its row shows `–` under `the pain touches`

### Requirement: Answers few files share
With `--min-files <n>`, the answers of a question that fewer than n files gave SHALL fold into one `<k> more` row, k being how many answers folded, after the others and before `not answered`. Its `people` is the files that gave any of them, and for `bother` its `the pain touches` is `–`. `not answered` never folds.

#### Scenario: A rare role
- **WHEN** of 10 files 1 said `qa`, 1 said `designer`, 6 said `developer` and 2 did not answer, and `--min-files 3` is given
- **THEN** the rows of `role` are `developer` with `6 (60%)`, `2 more` with `2 (20%)` and `not answered` with `2 (20%)`

### Requirement: Files without totals
When a pooled file has no totals (see share-export, Totals in the file), the pool SHALL weigh every file the same, as if each spent one unit and used one unit of tokens; a file with totals is divided by them first. Every dollar of the report then prints as a share of the files' units of spend, such as `7.0%` for `$808` of `$11,465` in one file, and every rate of dollars per active hour as a share of one file's spend, such as `0.018%` under `per active hour`.

#### Scenario: Two files, one heavy
- **WHEN** a file spent 10% of its $11,000 on `idle > TTL`, another 30% of its $300, and one of them has no totals
- **THEN** `idle > TTL` shows `20.0%` pooled, not the 10.5% of the pooled dollars

#### Scenario: Every file with totals
- **WHEN** every pooled file has format 1 or 2, or format 3 with totals
- **THEN** the report prints dollars, as before this change

### Requirement: Spend line without totals
When the report weighs files the same, the `spend` line of its header SHALL read `spend   in shares: each file weighs the same, since <k> of <n> files hold no dollars`, or for one file `spend   in shares: the file holds no dollars`, and the `spend` line of `At a glance` starts `spend     in shares of each file's spend`, followed by the shares per hour.

#### Scenario: A pool of shares
- **WHEN** 8 files are pooled and 6 of them hold no totals
- **THEN** the header's third line reads `spend   in shares: each file weighs the same, since 6 of 8 files hold no dollars`

### Requirement: Same shares from both kinds
A file written by `share` and one written by `share --dollars` over the same requests SHALL give the same shares in every table of the report. The pains of `Pains side by side` touch the same people whether the files hold totals or not, since its rules read only shares.

#### Scenario: The author's 30 days
- **WHEN** the author's 30 days are written once with `--dollars` and once without, and each is pooled alone
- **THEN** both reports show `idle > TTL` at 7.0% and `effort max or xhigh` at 80.4% of spend
