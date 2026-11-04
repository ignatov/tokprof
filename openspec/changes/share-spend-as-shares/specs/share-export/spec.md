## MODIFIED Requirements

### Requirement: Share command
`tokprof share` SHALL write one JSON file of aggregates over the requests the global filters select (see session-logs), with dollars and tokens as shares (see Dollars and tokens as shares), to `share-<YYYY-MM-DD>.json` in the profile directory (`~/tokprof`, or `$TOKPROF_DIR`) or to the file `-o` names. It then prints `wrote <path>`, the file's report in dollars (see share-pool), the totals line and `Nothing was sent anywhere: review the file, then send it yourself.`

#### Scenario: Default path
- **WHEN** `tokprof share` runs on 2026-11-03 without `-o`
- **THEN** it writes `~/tokprof/share-2026-11-03.json` and prints `wrote ~/tokprof/share-2026-11-03.json`

#### Scenario: Filters
- **WHEN** `tokprof --agent claude --since 30d share` runs
- **THEN** the file covers only the Claude Code requests of the last 30 days, and its `window` names the first and last day of those requests

#### Scenario: No requests
- **WHEN** the filters select no request
- **THEN** the command fails with `no requests found for these filters; nothing written`, and writes no file

#### Scenario: What it prints
- **WHEN** `tokprof --since 30d share` writes its file
- **THEN** stdout holds, in this order: `wrote <path>`; the report that `tokprof pool <path>` prints for that one file, in dollars; the totals line; and `Nothing was sent anywhere: review the file, then send it yourself.`
- **AND** it prints no list of the words in the file

### Requirement: File header
The file SHALL start with `format` (the number 3), `tokprof` (the version that wrote it), `platform` (the system and CPU it ran on, such as `macos-aarch64` or `linux-x86_64`), `window` (the first and last day of the selected requests, as `YYYY-MM-DD`), `survey`, `mcp_names` and `about` when they were given, `totals` with `--dollars` (see Totals in the file), and `agents`, one object per agent with requests, keyed by agent name.

#### Scenario: Two agents
- **WHEN** the selected requests come from Claude Code and Codex
- **THEN** `agents` has the keys `claude` and `codex`

#### Scenario: A survey file
- **WHEN** `tokprof share --survey team-2026-11 --mcp-names docs` runs on an Apple silicon Mac
- **THEN** the header has `"format": 3`, `"platform": "macos-aarch64"`, `"survey": "team-2026-11"` and `"mcp_names": ["docs"]`

#### Scenario: Without a label
- **WHEN** neither `--survey`, `--mcp-names`, `--about` nor `--dollars` is given
- **THEN** the header has no `survey`, no `mcp_names`, no `about` and no `totals`

## ADDED Requirements

### Requirement: Dollars and tokens as shares
A dollar amount SHALL be written as its share of the file's spend, and a token amount as its share of the file's tokens, both totals summed over the file's models, to 7 significant digits. Dollar amounts are the numbers keyed `usd` or `*_usd` and the quantiles of a distribution named `*_usd`; token amounts those keyed `tokens` or `*_tokens`. Context sizes stay in tokens, and every other number stays as it is.

#### Scenario: The author's spend
- **WHEN** the file's spend is $11,464.80 and the `idle > TTL` misses could have saved $808.2993
- **THEN** the sum of that row's `avoidable_usd` over the agents is `0.07050269`

#### Scenario: The cost of a session
- **WHEN** the median session cost $9.94 of $11,464.80 over 200 sessions
- **THEN** `session_usd` has `p50` `0.0008670015` and `n` 200

#### Scenario: Counts stay
- **WHEN** the agent's shell calls made 477 commits on 30 active days
- **THEN** the file has `commits` 477 and `active_days` 30

### Requirement: Totals on screen
After the report, `tokprof share` SHALL print `the file holds shares of $<spend> and <tokens> tokens, not dollars; these two totals stay on this machine`, or, with `--dollars`, `the file holds dollars: $<spend> and <tokens> tokens`.

#### Scenario: The default file
- **WHEN** `tokprof share` writes a file over $11,464.80 and 23,636,172,497 tokens
- **THEN** it prints `the file holds shares of $11,465 and 23.64B tokens, not dollars; these two totals stay on this machine`

### Requirement: Totals in the file
`--dollars` SHALL write the totals into the header as `totals` with `usd` and `tokens`. Multiplying every share by its total gives each amount back within $0.005 and 1 token per million, so a file with `totals` holds every number of a file of format 2.

#### Scenario: Rebuilding the dollars
- **WHEN** `share` and `share --dollars` write files over the same requests
- **THEN** each amount of the first, times the totals of the second, equals the same amount of the second within $0.005 and 1 token per million

### Requirement: Layout of the file
The file SHALL be written with one line per row of a table, that is per object whose objects hold no object, such as a tool with its `rent` and `trips`, and one line per key for the objects above them. It holds the same keys and values in the same order as written one number per line.

#### Scenario: A row of the activity table
- **WHEN** the file holds the activity `reading` of `claude`
- **THEN** its requests, tokens and dollars are on one line, such as `"reading": {"requests":41623,"tokens":0.4244027,"usd":0.3986049}`

#### Scenario: A tool
- **WHEN** the file holds the tool `Read` with its rent and trips
- **THEN** the tool is one line, and a tool with programs under it, such as `Bash`, has one line per key and one per program
