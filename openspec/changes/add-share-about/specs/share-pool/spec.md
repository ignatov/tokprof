# Spec Delta

## ADDED Requirements

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
- `waiting`: `waits for retries`, `the agent asks`, and the agents standing still: a file whose `rhythm.stood_still` holds waits is touched when 25% or more of them were under 5 minutes;
- `context`: `contexts of 400k+` and `sessions longer than a day`;
- `quality`, `setup`, `limits` and `nothing`: none, shown as `–`.

#### Scenario: Waiting from the breaks
- **WHEN** a person said `waiting`, no prevalence part touches them, and in 12 of their 40 breaks the agents stood still under 5 minutes after they left
- **THEN** the row `waiting` counts them among k, since 12 of 40 is 30%

#### Scenario: An answer without a measured pain
- **WHEN** 2 people said `quality`
- **THEN** its row shows `–` under `the pain touches`

### Requirement: Answers few files share
With `--min-files <n>`, the answers of a question that fewer than n files gave SHALL fold into one `<k> more` row, k being how many answers folded, after the others and before `not answered`. Its `people` is the files that gave any of them, and for `bother` its `the pain touches` is `–`. `not answered` never folds.

#### Scenario: A rare role
- **WHEN** of 10 files 1 said `qa`, 1 said `designer`, 6 said `developer` and 2 did not answer, and `--min-files 3` is given
- **THEN** the rows of `role` are `developer` with `6 (60%)`, `2 more` with `2 (20%)` and `not answered` with `2 (20%)`

## MODIFIED Requirements

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
- **THEN** the note of `Pains side by side` is `each pain by the people it touches: in a quarter of their days or sessions or more, or half of their spend for effort; the rows overlap, so money and time are not added up`
- **AND** the note of `What people said` is `the answers to tokprof share --about; the pain touches: of the people who gave an answer of bother, those its measured pains touch, as Pains side by side counts them`
- **AND** the note of `Where the money went` ends with `; median file: the median of each file's own share`
- **AND** the note of `What could have been saved` ends with `; files 5%+: the files where a row is 5% of their spend or more`
- **AND** the note of `How often it happens` ends with `; files: where it happened at all; files 25%+: in a quarter of their days or sessions or more`
- **AND** the note of `How the sessions spread` ends with `; p50 and q1–q3: the median and quarters of the files' own p50; p90: the median of their p90`

### Requirement: Survey guide
The repository SHALL hold `docs/survey.md`: how to run a survey with tokprof, and a prompt to paste into a coding agent, with the placeholders `<tag>`, `<label>`, `<mcp servers>` and `<recipient>`. The guide also says how to collect the files and pool them with `tokprof pool`, and what the `about` part of the header holds. It says to pass `--min-files 3` before a pooled report is shown to others.

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
