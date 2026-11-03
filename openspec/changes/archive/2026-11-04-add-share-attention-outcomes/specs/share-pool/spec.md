# Spec Delta

## ADDED Requirements

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

## MODIFIED Requirements

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
