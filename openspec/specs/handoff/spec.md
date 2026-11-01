# Handoff Specification

## Purpose
Hand the work of an earlier coding-agent session to a new session instead of resuming it on a cold cache. `tokprof handoff` prints the line that starts the new session, with the earlier session's id and log path; `--note` adds a note built from the log without a model.

## Requirements

### Requirement: Handoff line
`tokprof handoff [target]` SHALL print one line, `Continue from the earlier session <id>. Its log: <path>.`, for the session the target names, resolved as session-xray resolves a target, or for the latest main session when no target is given. `<id>` is the full session id: a Claude Code transcript's file name without `.jsonl`, or the thread id that ends a Codex rollout's name. `<path>` is the log's absolute path with the home directory shown as `~`.

#### Scenario: Claude Code session
- **WHEN** the user runs `tokprof handoff 3f951de0`
- **THEN** it prints `Continue from the earlier session 3f951de0-14bc-4966-880b-81f649966ccf. Its log: ~/.claude/projects/<project>/3f951de0-14bc-4966-880b-81f649966ccf.jsonl.`

#### Scenario: Codex session
- **WHEN** the user runs `tokprof handoff 019ff5ca`
- **THEN** it prints `Continue from the earlier session 019ff5ca-0db2-7180-8f61-e241e484f21a. Its log: ~/.codex/sessions/2026/08/12/rollout-2026-08-12T13-44-47-019ff5ca-0db2-7180-8f61-e241e484f21a.jsonl.`

#### Scenario: Compressed Codex rollout
- **WHEN** the session's log is an archived `rollout-*.jsonl.zst`
- **THEN** the id is the thread id without `.jsonl.zst`, and the path is that of the `.jsonl.zst` file

#### Scenario: No target
- **WHEN** the user runs `tokprof handoff` without a target
- **THEN** the line is for the latest main session, the one `tokprof` X-rays

#### Scenario: Log outside the home directory
- **WHEN** the log's path is not under the home directory
- **THEN** the path is printed unchanged

#### Scenario: Unknown target or no logs
- **WHEN** the target matches nothing, or the chosen agents have no session logs
- **THEN** the command fails with the same errors as `tokprof scan`

#### Scenario: A relative path as the target
- **WHEN** the user runs `tokprof handoff logs/x.jsonl` in `/work`
- **THEN** the path in the line is `/work/logs/x.jsonl`, so a session started elsewhere can open it

### Requirement: Clipboard copy in a terminal
When stdout is a terminal, `tokprof handoff` SHALL also copy the text it printed, the note included, to the clipboard with the first of `pbcopy`, `wl-copy` and `xclip -selection clipboard` that runs and succeeds. It then says on stderr `copied to the clipboard: paste it into a new session`, or `no clipboard tool found (pbcopy, wl-copy, xclip): copy the text above` when none worked.

#### Scenario: macOS terminal
- **WHEN** the user runs `tokprof handoff` in a terminal on macOS
- **THEN** `pbcopy` gets the printed text
- **AND** stderr says `copied to the clipboard: paste it into a new session`

#### Scenario: Linux with xclip only
- **WHEN** `pbcopy` and `wl-copy` are missing or fail and `xclip` works
- **THEN** `xclip -selection clipboard` gets the text

#### Scenario: No clipboard tool
- **WHEN** none of the three tools runs and succeeds
- **THEN** stderr says `no clipboard tool found (pbcopy, wl-copy, xclip): copy the text above`
- **AND** stdout is the same as with a clipboard

#### Scenario: Output not to a terminal
- **WHEN** stdout is piped or captured, as when an agent runs the command
- **THEN** nothing is copied and nothing is printed to stderr

### Requirement: Note from the log
With `--note`, `tokprof handoff` SHALL print `Continue from the earlier session <id>. A note about it follows; its log is <path>.`, a blank line, and a note built from the session's main log alone, without a model or a network call. The note reads compressed `.jsonl.zst` rollouts, skips lines that are not JSON, and shows each of its sections only when the log has content for it.

#### Scenario: First line with a note
- **WHEN** the user runs `tokprof handoff 019ff5ca --note`
- **THEN** the output starts with `Continue from the earlier session 019ff5ca-0db2-7180-8f61-e241e484f21a. A note about it follows; its log is ~/.codex/sessions/2026/08/12/rollout-2026-08-12T13-44-47-019ff5ca-0db2-7180-8f61-e241e484f21a.jsonl.`
- **AND** a blank line and the note follow

#### Scenario: Compressed rollout
- **WHEN** the log is a `rollout-*.jsonl.zst`
- **THEN** the note is built from its decompressed lines

#### Scenario: Subagent work
- **WHEN** the session ran subagents
- **THEN** the note leaves out their logs, and for Claude Code also the main log's lines flagged `isSidechain`

#### Scenario: Missing content
- **WHEN** the log holds no task list
- **THEN** the note has no `## Task list` section

### Requirement: Note header
The note SHALL open with `# Handoff from <agent> session <short id>`, where `<agent>` is `Claude Code` or `Codex` and `<short id>` the session's short id (see session-logs). Then come `Project <project>, branch <branch>; <first> → <last>.`, with the local `YYYY-MM-DD HH:MM` of the first and last message, and `The full log is <size> MB: search it only for details this note lacks.`, with the log file's size on disk.

#### Scenario: Claude Code header
- **WHEN** the log is a Claude Code transcript
- **THEN** the project is the first `cwd` in the log, with `~` for the home directory, and the branch is the latest non-empty `gitBranch`
- **AND** the first and last times are those of its `user` and `assistant` lines

#### Scenario: Codex header
- **WHEN** the log is a Codex rollout
- **THEN** the project and the branch come from `session_meta`, the branch marked as of the start, as in `Project ~/src/monorepo-2, branch monorepo-2 at the start; 2026-08-12 13:44 → 2026-08-12 14:29.`
- **AND** the first and last times are those of its `response_item` lines

#### Scenario: Unknown project, branch and times
- **WHEN** the log names no working directory, no branch and no times
- **THEN** the line reads `Project ?; ? → ?.`

#### Scenario: Compressed log size
- **WHEN** the log is a `.jsonl.zst` rollout
- **THEN** the size is that of the compressed file, as in `The full log is 0.2 MB: search it only for details this note lacks.`

### Requirement: Prompts in the note
The note SHALL take as prompts only what a person typed. For Claude Code these are `user` lines not flagged `isMeta`, `isCompactSummary` or `isSidechain`, with a slash command shown as its name and arguments and any other text that starts with `<` or `[Request interrupted` left out. For Codex they are `user` messages in `response_item` lines whose text does not start with `<` or `# AGENTS.md`.

#### Scenario: Slash command
- **WHEN** a Claude Code user line holds `<command-name>/model</command-name><command-args>opus</command-args>`
- **THEN** the prompt is `/model opus`

#### Scenario: Injected text
- **WHEN** a Claude Code user line holds only `<system-reminder>…</system-reminder>`
- **THEN** it is not a prompt

#### Scenario: Tool results
- **WHEN** a Claude Code user line carries only tool results
- **THEN** it is not a prompt

#### Scenario: Codex instructions
- **WHEN** a Codex user message starts with `# AGENTS.md` or with a tag such as `<environment_context>`
- **THEN** it is not a prompt

#### Scenario: An interruption marker
- **WHEN** a Claude Code user line holds `[Request interrupted by user]` or `[Request interrupted by user for tool use]`
- **THEN** it is not a prompt

### Requirement: Task and latest prompts
The note SHALL show the first prompt under `## Task`, on one line and cut to 900 characters, and up to 6 of the latest later prompts under `## Latest prompts`, oldest first, each as `- <MM-DD HH:MM> <prompt>` on one line and cut to 300 characters.

#### Scenario: Long session
- **WHEN** a session has 40 prompts
- **THEN** `## Task` holds the first, and `## Latest prompts` the 35th to the 40th in that order

#### Scenario: Cut text
- **WHEN** a prompt is longer than its limit
- **THEN** its line breaks and runs of spaces become single spaces, and it is cut to the limit with `…` as the last character

#### Scenario: One prompt
- **WHEN** the session has a single prompt
- **THEN** the note has `## Task` and no `## Latest prompts`

### Requirement: Where it stopped
The note SHALL show under `## Where it stopped` the last non-empty text the agent wrote in the main conversation, with its line breaks kept and cut to 2,500 characters.

#### Scenario: Claude Code
- **WHEN** the main agent's last text is `Fixed the race; the regression test is left.` and a subagent wrote later in a line flagged `isSidechain`
- **THEN** the section reads `Fixed the race; the regression test is left.`

#### Scenario: Codex
- **WHEN** a Codex rollout's last assistant message is `Specs updated, code unchanged.`
- **THEN** the section reads `Specs updated, code unchanged.`

#### Scenario: Long final message
- **WHEN** the last text is longer than 2,500 characters
- **THEN** its first 2,499 characters are kept, trailing whitespace trimmed, and `…` follows

### Requirement: Task list
The note SHALL show under `## Task list` the latest task list the agent wrote, from Claude Code's `TodoWrite` (`todos` with `status` and `content`) or Codex's `update_plan` (`plan` with `status` and `step`). Each item is one line, marked `[x]` when `completed`, `[~]` when `in_progress` and `[ ]` otherwise, and cut to 160 characters.

#### Scenario: Claude Code todos
- **WHEN** the last `TodoWrite` holds a completed `find the race` and a pending `add a regression test`
- **THEN** the section reads `- [x] find the race` and `- [ ] add a regression test`

#### Scenario: Codex plan
- **WHEN** the last `update_plan` holds `group the conflicts` in progress
- **THEN** the section reads `- [~] group the conflicts`

#### Scenario: Several lists
- **WHEN** the agent wrote several task lists
- **THEN** only the last one is shown

### Requirement: Files changed
The note SHALL list under `## Files changed, latest first` up to 15 distinct files the session changed, the most recently changed first: Claude Code `Edit`, `Write`, `MultiEdit` and `NotebookEdit` calls, and Codex patches (`*** Update File:`, `*** Add File:`, `*** Delete File:`), also inside the JavaScript of Codex's `exec` tool. Paths inside the project are shown relative to it, others with `~` for the home directory.

#### Scenario: Claude Code edit
- **WHEN** a session in `/w/p` edited `/w/p/src/parser.rs`
- **THEN** the section lists `- src/parser.rs`

#### Scenario: Codex patch inside exec
- **WHEN** Codex's `exec` tool ran JavaScript that applies a patch with `*** Update File: spec/a.md`
- **THEN** the section lists `- spec/a.md`

#### Scenario: A file changed several times
- **WHEN** a file was changed early and again at the end
- **THEN** it is listed once, at the place of its latest change

#### Scenario: Many files
- **WHEN** the session changed 40 files
- **THEN** only the 15 changed most recently are listed

#### Scenario: File outside the project
- **WHEN** a changed file lies outside the project but under the home directory
- **THEN** its path starts with `~`

### Requirement: Files read most
For Claude Code sessions, the note SHALL list under `## Files read most` up to 8 files read with the `Read` tool, the most read first and ties in the order they were first read, each as `- <path> ×<reads>`, with paths shown as in the files changed.

#### Scenario: Claude Code reads
- **WHEN** a session in `/w/p` read `/w/p/src/parser.rs` once
- **THEN** the section lists `- src/parser.rs ×1`

#### Scenario: Ties
- **WHEN** two files were read 3 times each
- **THEN** the one read first comes first

#### Scenario: Codex
- **WHEN** the log is a Codex rollout
- **THEN** the note has no `## Files read most` section

### Requirement: Latest commands
The note SHALL list under `## Latest commands` up to 8 distinct shell commands, the latest first, each as its first non-empty line, on one line, cut to 140 characters and set in backticks. They come from Claude Code `Bash` calls, Codex `exec_command` and `shell` calls, and `cmd:` strings in the JavaScript of Codex's `exec` tool.

#### Scenario: Multi-line command
- **WHEN** a Claude Code `Bash` call ran `cargo test parser` and `echo done` on two lines
- **THEN** the section lists ``- `cargo test parser` ``

#### Scenario: Codex commands
- **WHEN** Codex ran `git status --short` with `exec_command` and then `rg -n Done spec` through its `exec` tool
- **THEN** the section lists ``- `rg -n Done spec` `` before ``- `git status --short` ``

#### Scenario: Command as an argument list
- **WHEN** a Codex `shell` call gives its command as a list whose middle part is a flag, such as `bash -lc <script>`
- **THEN** the listed command is the script
- **AND** other lists are joined with spaces

#### Scenario: Repeated command
- **WHEN** the same command ran several times
- **THEN** it is listed once, at the place of its latest run
