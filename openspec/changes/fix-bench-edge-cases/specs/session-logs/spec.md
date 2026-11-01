## MODIFIED Requirements

### Requirement: Unreadable logs
`summary`, `misses`, `whatif`, `flame` and `bench` SHALL carry on with the logs that can be read, and print `warning: <path>: <error>` to stderr, after the read line, for at most 5 logs that could not be opened, read or decompressed, then `warning: <n> more logs could not be read` for the rest. Inside a log, lines that are not valid JSON are skipped without a warning, and invalid UTF-8 is replaced.

#### Scenario: A log cannot be opened
- **WHEN** a log is removed or made unreadable between discovery and reading
- **THEN** a warning names it and the report covers the other logs

#### Scenario: Reading fails partway
- **WHEN** reading or decompressing a log fails partway
- **THEN** none of that log's requests are counted and a warning names it

#### Scenario: Many unreadable logs
- **WHEN** 12 logs cannot be read
- **THEN** 5 warnings name them, and a sixth reads `warning: 7 more logs could not be read`

#### Scenario: Line still being written
- **WHEN** the last line of a log is incomplete because the agent is still writing it
- **THEN** that line is skipped without a warning

#### Scenario: Other commands
- **WHEN** `bench` meets a log that it cannot read
- **THEN** it skips the log and prints a warning, as the other reports do
- **AND** the X-ray of session-xray fails with the read error when the session's main log or a subagent log cannot be read
