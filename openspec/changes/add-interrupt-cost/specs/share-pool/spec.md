## ADDED Requirements

### Requirement: How turns end
The report SHALL print `How turns end` from the turn parts (see share-export, Turn parts), one row per end kind, the most turns first. The columns are `turns` (pooled), `share` (of the turns), `cost`, `median file` (of each file's share of its turns), `under 5m` and `none` (the shares of what came next), and `files`. A line under the table gives the pooled `session ends` by kind, as shares of the ended sessions.

#### Scenario: Changed and waiting
- **WHEN** 2 files have 400 and 600 turns, of which 120 and 180 ended `changed`, and 40% of those were followed by a message within 5 minutes
- **THEN** the row `changed` shows `300`, `30.0%`, `30.0%` under `median file` and `40%` under `under 5m`

### Requirement: Stopped turns
The report SHALL print `Stopped turns`, one row per class of the last call, the most turns first, at most 8, then `<n> more`. The columns are `turns`, `cost`, `median file` (of each file's share of its spend), and `files`. A line under the table gives the pooled median time until the stop and the share of stopped turns followed by a message within 5 minutes.

#### Scenario: During shell calls
- **WHEN** 22 of 36 pooled stopped turns were stopped during calls of class `file and system commands`
- **THEN** that row comes first, with `22` under `turns`

### Requirement: Run length
The report SHALL print `Run length`, one row per length range in order, then `goal runs` when any file has one. The columns are `turns`, `cost`, `share` (of the pooled spend), `cost per edit call`, `commits`, `stopped` and `files`. `cost per edit call` is the range's pooled dollars over its pooled edit calls; it is `–` for a range with no edit call. When the files weigh the same (see Files without totals), it is a share of one file's spend, as a rate.

#### Scenario: Long runs
- **WHEN** the pooled turns of `15m–1h` cost $300 for 750 edit calls
- **THEN** that row shows `$0.40` under `cost per edit call`

## MODIFIED Requirements

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
  - `How turns end` with `Stopped turns` and `Run length`;
  - `Commits and pull requests`;
  - `Tools and commands`;
  - `Highest effort by client`;
  - `Versions`.

#### Scenario: Format-1 files only
- **WHEN** every pooled file has format 1
- **THEN** the report has neither `Pains side by side`, `What people said`, `Problems`, `How often it happens`, `How the sessions spread`, `Where the agent's time went`, `Your time with the agents`, `Working with the agent`, `Coming back after a cold break`, `Commits and pull requests`, `How turns end`, `Stopped turns`, `Run length` nor `Highest effort by client`
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
  - `How turns end`: `how the agents' turns ended, and how soon the person typed again in that session`
  - `Stopped turns`: `the turns the person stopped, by what the agent was doing at the stop`
  - `Run length`: `the turns the person started, by how long the agent then worked on its own`
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
  - `interrupts`: `days you stopped the agent`; `stopped turns` of `Stopped turns`;
  - `the agent asks`: `sessions where the agent asked`; none, since the files hold the median of the answers to questions, not their total;
  - `effort max or xhigh`: the habit `effort max or xhigh`, for both.

#### Scenario: One file
- **WHEN** `tokprof share` prints the report of the file it wrote
- **THEN** the report has no `Pains side by side`

#### Scenario: The order of the rows
- **WHEN** `effort max or xhigh` ($9,073), `cold returns` ($792) and `failures` ($226) each touch 7 of 8 people, and `contexts of 400k+` touches 5
- **THEN** the rows come in that order, and a row with time or without money comes after the dollar rows that touch as many people

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
- **THEN** `the agent asks` shows `–` under `kind`, `median file per active hour` and `pooled`
- **AND** `interrupts` shows `spent`, and shows `–` when no file has the turn parts

### Requirement: The pains of each answer
The answers of `bother` SHALL have these pains:
- `cost`: every pain of `Pains side by side` whose kind is `avoidable` or `spent`;
- `waiting`, the person waiting for the agent: `waits for retries`. The agent's questions and the agents standing still after the person left are the agent waiting for the person, so they are not its pains;
- `context`: `contexts of 400k+` and `sessions longer than a day`;
- `quality`: `interrupts`;
- `setup`, `limits` and `nothing`: none, shown as `–`.

#### Scenario: Waiting is the person's
- **WHEN** a person said `waiting`, requests waited for retries on 2 of their 20 active days, and in 12 of their 40 breaks the agents stood still under 5 minutes after they left
- **THEN** the row `waiting` counts them in n but not in k

#### Scenario: Quality
- **WHEN** a person said `quality` and stopped the agent on 9 of their 30 active days
- **THEN** the row `quality` counts them in k, since a stopped turn is work the person did not want

#### Scenario: An answer without a measured pain
- **WHEN** 2 people said `setup`
- **THEN** its row shows `–` under `the pain touches`
