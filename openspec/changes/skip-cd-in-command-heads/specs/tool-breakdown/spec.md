## MODIFIED Requirements

### Requirement: Rows by tool and command
The tool breakdown SHALL have one row per tool, the session's subagents included. A shell tool, such as Claude Code `Bash` or Codex `exec_command`, gets one row per command head (see cost-attribution): `Bash sed -n`, `exec_command cargo test`. The head skips a leading `cd <dir>` up to the first `&&` or `;`. A Codex `exec` call whose code calls one inner tool shows as that tool, as cost-attribution names it; one that calls several shows as `exec`.

#### Scenario: Shell commands
- **WHEN** a session ran `sed -n 1,80p a.rs` 56 times and `grep -n foo src` 40 times through `Bash`
- **THEN** the rows are `Bash sed -n` with 56 calls and `Bash grep -n` with 40 calls

#### Scenario: Other tools
- **WHEN** a session called `Read` on 3 different files
- **THEN** they make one `Read` row with 3 calls, whatever the files

#### Scenario: A command that first changes the folder
- **WHEN** a session ran `cd /w/app && cargo test` and `cd /w/app; ./gradlew test --tests X` through its shell tool
- **THEN** their rows are those of `cargo test` and `gradlew test`, and no row is named after the `cd`
