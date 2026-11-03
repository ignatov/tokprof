# Spec Delta

## ADDED Requirements

### Requirement: Activity line
The header SHALL have an `activity` line after `spend`. For one file it gives the active days, the active hours, the busy hours, and the dollars per active hour (see share-export, Activity parts). For a pool it gives the pooled active hours and the dollars per active hour of the pool, then the median per file of the active days, the active hours and the dollars per active hour. Only the files with the part count.

#### Scenario: One file
- **WHEN** a file has 18 active days, 141 active hours, 365 busy hours and $11,200 of spend
- **THEN** the line reads `activity  18 active days · 141 active hours · busy 365h · $79.43 per active hour`

#### Scenario: A pool
- **WHEN** 5 files with the part hold 700 active hours and $41,220 between them, and their medians are 16 active days, 120 active hours and $68.20 per active hour
- **THEN** the line reads `activity  700 active hours · $58.89 per active hour · per file: median 16 active days, 120 active hours, $68.20 per active hour`

### Requirement: Savings per active hour
`What could have been saved` SHALL have a `per active hour` column, after `share`: the row's dollars over the active hours of the files that have both the row's part and the activity part. After that table, the report SHALL print `Cold returns by client`, one row per agent and client with `active hours`, `cold returns`, `per active hour`, and `avoidable per active hour`: the avoidable dollars of the client's cold returns.

#### Scenario: A client's baseline
- **WHEN** a client has 312 active hours and 206 cold returns that could have saved $740, and its other misses $150 more
- **THEN** its row in `Cold returns by client` shows `312`, `206`, `0.66` and `$2.37`

### Requirement: Problems
The report SHALL print `Problems`, with the rows `failed tool calls`, `spend after failures`, `calls the user refused`, `calls a rule denied`, `requests that waited for retries`, `time waiting for retries`, `requests of a minute or more` and `longest request`. The columns are `pooled`, `per active hour`, `median file` (of each file's value per active hour) and `files` (the files where the value is above zero). The longest request is the longest of all files, and has no rate.

#### Scenario: Failures of one person
- **WHEN** one file has 1,390 failed calls, $226 of spend after them, and 141 active hours
- **THEN** the row `failed tool calls` shows `1,390` and `9.86`, and the row `spend after failures` shows `$226` and `$1.60`

#### Scenario: Slow requests
- **WHEN** 3,001 of 83,890 requests took a minute or more
- **THEN** the row `requests of a minute or more` shows `3,001 (3.6%)` under `pooled`

### Requirement: Where the agent's time went
The report SHALL print `Where the agent's time went`, one row per kind of busy time (see agent-time), most time first, with `time` in hours, `share` of the busy time, `median file` (each file's share), `files` and `after turns`.

#### Scenario: Subagents after turns
- **WHEN** subagents took 67.5 hours of 365 busy hours, 54 hours of them after ends of turn
- **THEN** the `subagents` row shows `67.5h`, `18.5%` and `54.0h` under `after turns`

### Requirement: Working with the agent
The report SHALL print `Working with the agent`, with the rows of the scenario below. The columns are `pooled`, `per active hour`, `per 100 typed messages`, `median file` (of each file's value per active hour) and `files`. Only interrupts, messages typed while the agent worked, questions and answers have a value per 100 typed messages. Typed messages come from the pause parts.

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
The report SHALL print `Highest effort by client`, one row per agent and client, with the share of the client's spend and of its requests at `max` or `xhigh`, then `median file` (of the share of spend) and `files`.

#### Scenario: A client at max
- **WHEN** a client spent $800 of $850 at `max`, in 900 of 1,000 requests
- **THEN** its row shows `94.1%` and `90.0%`

### Requirement: Prompt coverage line
When the share of spend whose prompts the logs hold is below 100%, the report SHALL print, under the content table, `prompts the logs hold: <x>% of the spend; the rest is unknown`. It counts the files that have the part. For a pool, the line ends with `, in <k> files below 90%`.

#### Scenario: One file with old transcripts
- **WHEN** 88.2% of a file's spend comes from requests whose prompts its logs hold
- **THEN** the line reads `prompts the logs hold: 88.2% of the spend; the rest is unknown`

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
