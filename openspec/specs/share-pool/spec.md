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
The repository SHALL hold `docs/survey.md`: how to run a survey with tokprof, and a prompt to paste into a coding agent, with the placeholders `<tag>`, `<label>`, `<mcp servers>` and `<recipient>`. The guide also says how to collect the files and pool them with `tokprof pool`. It says to pass `--min-files 3` before a pooled report is shown to others.

#### Scenario: The steps of the prompt
- **WHEN** an agent follows the prompt with the placeholders filled in
- **THEN** it does these steps:
  - picks the archive for the machine's system and CPU (see release-builds);
  - downloads the archive and `SHA256SUMS` from the release `<tag>` into a new temporary folder, and asks the person to allow it when a sandbox blocks the network;
  - checks the archive's SHA-256 against its line in `SHA256SUMS` and stops when they differ, and also runs `gh attestation verify` when `gh` is installed;
  - unpacks the archive there and runs `tokprof --since 30d share --survey <label> --mcp-names <mcp servers> -o <folder>/tokprof-share.json`;
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
  - `Where the money went`;
  - `What could have been saved` with `Cold returns by client` and `Coming back after a cold break`;
  - `Problems`;
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
- **THEN** the report has neither `Problems`, `Where the agent's time went`, `Your time with the agents`, `Working with the agent`, `Coming back after a cold break`, `Commits and pull requests` nor `Highest effort by client`
- **AND** `At a glance` has only the lines whose parts format 1 has

#### Scenario: Files written before the rhythm parts
- **WHEN** every pooled file has format 2 but none has the rhythm, bench or outcome parts
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
- **THEN** the note of `Where the money went` ends with `; median file: the median of each file's own share`
- **AND** the note of `What could have been saved` ends with `; files 5%+: the files where a row is 5% of their spend or more`

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
