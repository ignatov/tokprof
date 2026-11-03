# Spec Delta

## ADDED Requirements

### Requirement: How often it happens
After `Problems`, the report SHALL print `How often it happens`, from the prevalence parts (see share-export, Prevalence parts), with one row per count of the scenario below. The day rows come from the file's own prevalence part; the session rows from the agents' parts. The columns are `count`, `of` (the active days or main sessions of the same files) and `share`.

#### Scenario: The rows
- **WHEN** the report is printed
- **THEN** its rows are `days with a cold return`, `days a request waited for retries`, `days you stopped the agent`, `sessions with a cold return`, `sessions with spend after a failure`, `sessions with a context of 400k+`, `sessions longer than a day` and `sessions where the agent asked`

#### Scenario: One file
- **WHEN** a file had a cold return on 28 of its 30 active days
- **THEN** the row `days with a cold return` shows `28`, `30 days` and `93.3%`

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

## MODIFIED Requirements

### Requirement: Rows few files share
`--min-files <n>` SHALL fold every table row that fewer than n files have into one `other` row of its table. Names under an origin kind are left out instead, and their spend stays in the kind's row. Versions that fewer than n files carry are left out. A row of `How the sessions spread` with quantiles from fewer than n files is left out too. The default is 1, which folds nothing.

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
- **THEN** the report has neither `Problems`, `How often it happens`, `How the sessions spread`, `Where the agent's time went`, `Your time with the agents`, `Working with the agent`, `Coming back after a cold break`, `Commits and pull requests` nor `Highest effort by client`
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
- **THEN** the note of `Where the money went` ends with `; median file: the median of each file's own share`
- **AND** the note of `What could have been saved` ends with `; files 5%+: the files where a row is 5% of their spend or more`
- **AND** the note of `How often it happens` ends with `; files: where it happened at all; files 25%+: in a quarter of their days or sessions or more`
- **AND** the note of `How the sessions spread` ends with `; p50 and q1–q3: the median and quarters of the files' own p50; p90: the median of their p90`
