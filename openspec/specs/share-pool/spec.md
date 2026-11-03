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
The pool SHALL skip a file that is not a share file, one whose format is newer than it reads, and one whose content equals a file it already read. For each, it prints one line on stderr: `skipped <path>: not a tokprof share file`, `skipped <path>: format <n> needs a newer tokprof` or `skipped <path>: the same as <path>`.

#### Scenario: A file sent twice
- **WHEN** a folder holds `a.json` and `a (1).json` with the same content
- **THEN** the report counts one file, and stderr says `skipped a (1).json: the same as a.json`, with the paths as given

#### Scenario: Both formats
- **WHEN** one file has format 1 and another format 2
- **THEN** both are read

#### Scenario: A newer format
- **WHEN** a file has format 3
- **THEN** stderr says `skipped <path>: format 3 needs a newer tokprof`

### Requirement: Pool header
The report SHALL start with three lines:
- `pool`: the number of files, the first and last day of their windows, and their survey labels;
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
When the report covers one file, it SHALL leave out the `agents` line and the columns `median file`, `files` and `files 5%+`. Its first line reads `share   <first day> – <last day>`, followed by ` · survey <label>` when the file has a label.

#### Scenario: The report that share prints
- **WHEN** `tokprof share --survey team-2026-11` writes its file for 2026-10-04 to 2026-11-03
- **THEN** the report it prints starts with `share   2026-10-04 – 2026-11-03 · survey team-2026-11`, then `spend   ` and the file's spend
- **AND** the tables of money have only the row name, the dollars and `share`, and no table has a column that counts files

### Requirement: Rows few files share
`--min-files <n>` SHALL fold every table row that fewer than n files have into one `other` row of its table. Names under an origin kind are left out instead, and their spend stays in the kind's row. Versions that fewer than n files carry are left out. The default is 1, which folds nothing.

#### Scenario: A rare model
- **WHEN** only 1 of 8 files used the model `claude-haiku-4-5-20251001`, and `--min-files 3` is given
- **THEN** that model is not printed, and its spend shows in the `other` row of the model table

#### Scenario: A rare name
- **WHEN** only 1 of 8 files has spend on `skill:simplify`, and `--min-files 3` is given
- **THEN** `skill:simplify` is not printed, and its spend stays in the `skill` row

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
The repository SHALL hold `docs/survey.md`: how to run a survey with tokprof, and a prompt to paste into a coding agent, with the placeholders `<tag>`, `<label>` and `<recipient>`. The guide also says how to collect the files and pool them with `tokprof pool`. It says to pass `--min-files 3` before a pooled report is shown to others.

#### Scenario: The steps of the prompt
- **WHEN** an agent follows the prompt with the placeholders filled in
- **THEN** it does these steps:
  - picks the archive for the machine's system and CPU (see release-builds);
  - downloads the archive and `SHA256SUMS` from the release `<tag>` into a new temporary folder, and asks the person to allow it when a sandbox blocks the network;
  - checks the archive's SHA-256 against its line in `SHA256SUMS` and stops when they differ, and also runs `gh attestation verify` when `gh` is installed;
  - unpacks the archive there and runs `tokprof --since 30d share --survey <label> -o <folder>/tokprof-share.json`;
  - shows tokprof's output as it is;
  - tells the person where the file is, and that they should review it and send it to `<recipient>` themselves.

#### Scenario: What the prompt rules out
- **WHEN** an agent follows the prompt
- **THEN** it installs nothing, changes no settings and sends nothing anywhere
- **AND** it downloads nothing but the archive and `SHA256SUMS`, and reads neither the share file nor any log into the chat

#### Scenario: Kept MCP servers in a survey
- **WHEN** the person running the survey wants the calls of an MCP server counted by name
- **THEN** the guide says to add `--mcp-names <server>` to the command in the prompt, which also lists the server in each file's header

### Requirement: Activity line
The header SHALL have an `activity` line after `spend`. For one file it gives the active days, the active hours, the busy hours, and the dollars per active hour (see share-export, Activity parts). For a pool it gives the pooled active hours and the dollars per active hour of the pool, then the median per file of the active days, the active hours and the dollars per active hour. Only the files with the part count.

#### Scenario: One file
- **WHEN** a file has 18 active days, 141 active hours, 365 busy hours and $11,200 of spend
- **THEN** the line reads `activity  18 active days · 141 active hours · busy 365h · $79.43 per active hour`

#### Scenario: A pool
- **WHEN** 5 files with the part hold 700 active hours and $41,220 between them, and their medians are 16 active days, 120 active hours and $68.20 per active hour
- **THEN** the line reads `activity  700 active hours · $58.89 per active hour · per file: median 16 active days, 120 active hours, $68.20 per active hour`

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
The report SHALL print `Working with the agent`, with the rows of the scenario below. The columns are `pooled`, `per active hour`, `per 100 typed messages`, `median file` (of each file's value per active hour) and `files`. Only interrupts, messages typed while the agent worked, questions and answers have a value per 100 typed messages. Typed messages come from the pause parts.

#### Scenario: A file without active hours
- **WHEN** two files have 1,168 and 1,158 typed messages, and only the first has active hours, 395 of them
- **THEN** the `typed messages` row counts the first file only: it shows `1,168` and `2.96`, and says `(in 1 of 2 files)`

#### Scenario: The rows
- **WHEN** the report is printed
- **THEN** the rows are `typed messages`, `ends of turn`, `interrupts`, `typed while the agent worked`, `questions`, `answers`, `wakes: subagent`, `wakes: command` and `wakes: other`

#### Scenario: Interrupts
- **WHEN** a file has 1,030 typed messages, 41 interrupts and 141 active hours
- **THEN** the `interrupts` row shows `41`, `0.29` and `3.98`

#### Scenario: Ends of turn
- **WHEN** a file has 2,671 ends of turn, many after wakes, over 395 active hours
- **THEN** the `ends of turn` row shows `2,671`, `6.76` and `–` under `per 100 typed messages`

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
After the header, the report SHALL print, in this order:
- `Where the money went`;
- `What could have been saved` with `Cold returns by client`;
- `Problems`;
- `Habits that cost the most`;
- `Where the agent's time went`;
- `Working with the agent`;
- `Tools and commands`;
- `Highest effort by client`;
- `Versions`.

A section that no file has the parts for is left out.

#### Scenario: Format-1 files only
- **WHEN** every pooled file has format 1
- **THEN** the report has neither `Problems`, `Where the agent's time went`, `Working with the agent` nor `Highest effort by client`
