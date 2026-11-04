## MODIFIED Requirements

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

## ADDED Requirements

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
