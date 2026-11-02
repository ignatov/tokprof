# Spec Delta

## ADDED Requirements

### Requirement: Trigger of spend
Every share of spend SHALL also carry the trigger of the request it comes from: what made the agent send that request. The trigger is one more dimension that profiles stack by (see profile-export). Every share of a request has the same trigger, whatever block of the prompt it falls on, and output included.

#### Scenario: A lookup in a big context
- **WHEN** a response makes one `Read` call and the next request re-reads a 400,000-token prompt from the cache
- **THEN** every share of that next request, its cache reads and its output, has trigger `lookup`

#### Scenario: Stacks by trigger
- **WHEN** the user passes `--stack trigger,origin,component`
- **THEN** every stack starts with the request's trigger, such as `lookup`, then its origin

### Requirement: Trigger of a request
A request's trigger SHALL be `advisor` for an advisor call and `compaction` for the first request after a compaction. Otherwise it comes from the latest thing the chain logged after its previous response:
- a message the user typed, or a subagent's brief: `prompt`;
- a background task's result: `notification`;
- an interruption: `continuation`;
- else the previous response's calls: `edit`, `subagent`, `run`, `lookups` or `lookup`.

What the log does not show is `unknown`.

#### Scenario: Read-only calls
- **WHEN** the previous response made exactly one read-only call (see Read-only calls)
- **THEN** the request's trigger is `lookup`
- **AND** with two or more calls, all read-only, it is `lookups`

#### Scenario: Mixed calls
- **WHEN** the previous response made a `Read`, a `cargo test` and an `Edit`
- **THEN** the trigger is `edit`, since an edit counts before a subagent, a subagent before a run, and a run before lookups

#### Scenario: Edits
- **WHEN** the previous response called `Edit`, `MultiEdit`, `Write`, `NotebookEdit` or Codex `apply_patch`
- **THEN** the trigger is `edit`

#### Scenario: Subagents
- **WHEN** the previous response called Claude Code `Agent` or `Task`, or Codex `spawn_agent`
- **THEN** the trigger is `subagent`

#### Scenario: Other calls
- **WHEN** the previous response called a tool that is neither read-only, an edit nor a subagent, such as a shell command that is not read-only or an MCP tool
- **THEN** the trigger is `run`

#### Scenario: A typed prompt
- **WHEN** the request answers a message the user typed, a slash command included
- **THEN** its trigger is `prompt`
- **AND** the first request of a subagent chain, which answers the subagent's brief, also has trigger `prompt`

#### Scenario: A background task reports back
- **WHEN** a Claude Code turn starts with a `<task-notification>`
- **THEN** its first request has trigger `notification`

#### Scenario: After a compaction
- **WHEN** a compaction happened between the chain's previous request and this one
- **THEN** the trigger is `compaction`

#### Scenario: Continuations
- **WHEN** the request follows an interruption (`[Request interrupted`) with nothing typed after it, or, in Codex, a response that ended with an assistant message and no tool call
- **THEN** the trigger is `continuation`

#### Scenario: A prompt after an interruption
- **WHEN** the user interrupts a response and then types a new message
- **THEN** the next request has trigger `prompt`

#### Scenario: A retried request
- **WHEN** a request failed with an API error and was sent again
- **THEN** the retry has the trigger the failed request would have had

#### Scenario: Advisor calls
- **WHEN** a request is an advisor call
- **THEN** its trigger is `advisor`

### Requirement: Read-only calls
A tool call SHALL be read-only when its tool only reads, searches or fetches: Claude Code `Read`, `NotebookRead`, `Grep`, `Glob`, `LS`, `WebFetch` and `WebSearch`. A shell call, Claude Code `Bash` or Codex `exec_command` and `shell`, is read-only when its command is (see Read-only commands). A Codex `exec` call is read-only when every tool its code calls is.

#### Scenario: A file read
- **WHEN** a response calls `Read` on `src/main.rs`
- **THEN** the call is read-only

#### Scenario: Codex code that runs several commands
- **WHEN** a Codex `exec` call's code runs `tools.exec_command` twice, with `rg -n foo` and `sed -n 1,80p src/lib.rs`
- **THEN** the call is read-only, and it counts as two read-only calls when telling `lookup` from `lookups`

#### Scenario: A background shell's output
- **WHEN** a response calls `BashOutput`
- **THEN** the call is not read-only, and the trigger it gives is `run`

### Requirement: Read-only commands
A shell command SHALL be read-only when every part of it, split at `&&`, `||`, `;`, `|`, `&` and line breaks outside quotes, starts with a read-only program, and nothing writes to a file. `cd <dir>` parts, leading `NAME=value` assignments and shell keywords are allowed. A command with `>` or `>>` to anything but `/dev/null`, `&1` or `&2`, or with `<<`, `$(`, a backquote or an unclosed quote is not read-only.

#### Scenario: Read-only programs
- **WHEN** a part starts with `cat`, `head`, `tail`, `nl`, `less`, `sed` without an in-place flag (`-i`, `-i.bak`, `-ni`, `--in-place`), `awk`, `grep`, `rg`, `ag`, `ls`, `tree`, `find` without `-delete`, `-exec`, `-execdir`, `-ok`, `-okdir`, `-fprint`, `-fprint0`, `-fprintf` or `-fls`, `fd`, `wc`, `stat`, `file`, `du`, `pwd`, `echo`, `printf`, `sort`, `uniq`, `cut`, `tr`, `column`, `paste`, `comm`, `jq`, `diff`, `basename`, `dirname`, `realpath`, `which`, `zcat`, `zstdcat`, `xxd`, `test`, `[`, `true` or `read`
- **THEN** that part is read-only
- **AND** a program given with a path or a `.cmd` or `.exe` suffix, such as `/usr/bin/grep` or `rg.cmd`, counts as the program

#### Scenario: Git
- **WHEN** a part is `git` followed by any of `-C <dir>`, `-c <name=value>` and other options, then `show`, `log`, `diff`, `status`, `grep`, `ls-files`, `blame`, `rev-parse`, `cat-file`, `rev-list`, `merge-base`, `shortlog` or `describe`
- **THEN** that part is read-only
- **AND** any other git subcommand, such as `git commit` or `git checkout`, is not

#### Scenario: A filtered read
- **WHEN** the command is `cd /repo && git show HEAD:src/lib.rs | sed -n 1,120p 2>/dev/null`
- **THEN** it is read-only

#### Scenario: A write hidden in a chain
- **WHEN** the command is `cat a.txt > b.txt`, `grep -n foo src | tee hits.txt` or `ls && rm -rf build`
- **THEN** it is not read-only

#### Scenario: Loops and conditions around reads
- **WHEN** the command is `for f in a.rs b.rs; do echo "== $f"; sed -n 1,20p "$f"; done` or `if [ -f Cargo.toml ]; then cat Cargo.toml; fi`
- **THEN** it is read-only, since `for <name> in …`, `do`, `done`, `if`, `then`, `elif`, `else`, `fi`, `while`, `until` and `!` only frame the commands that are checked

#### Scenario: Scripts and substitutions
- **WHEN** the command runs `python3`, `node` or `cargo`, holds a heredoc, or runs `$(…)` or a backquoted command
- **THEN** it is not read-only

### Requirement: Spend by trigger
`tokprof summary` SHALL print `Spend by trigger: what made the agent send each request` and a table of whole requests with `trigger`, `requests`, `avg prompt`, `cost`, `per request` and `share`, most expensive first, with every trigger that has requests. `avg prompt` is the mean prompt size of the requests. Shares are of the cost, or of tokens when no request has a price.

#### Scenario: A lookup row
- **WHEN** 31,833 requests with trigger `lookup` averaged 263,000 prompt tokens and cost $3,393 of $9,096 priced
- **THEN** the row reads `lookup`, `31833`, `263.0k`, `$3,393`, `$0.11` and `37.3%`

#### Scenario: The table sums to the total
- **WHEN** the summary is printed
- **THEN** the costs of the trigger rows add up to the `Total priced` figure

#### Scenario: No price
- **WHEN** no request in the window has a price
- **THEN** the cost and per-request cells show tokens, and the shares are of tokens

## MODIFIED Requirements

### Requirement: Summary report
`tokprof summary` SHALL print `Spend by model (API-equivalent USD; token counts where no price is known)` and a table with one row per agent and model: `agent`, `model`, `requests`, `prompt known`, the five components and `total`, sorted by total cost, highest first. Then come the totals line, the unpriced models (see pricing), the reconciliation line when there is one, the spend by origin, task and trigger, and the `Cache misses by cause` table (see cache-misses) without expected rebuilds.

#### Scenario: A model row
- **WHEN** the reported window holds 16,281 requests to `claude-opus-5-5`, all with a rebuilt prompt
- **THEN** its row starts with `claude`, `claude-opus-5-5`, `16281` and `100%`, followed by the dollars of each component and the total, such as `$1,192` or `$0.14`

#### Scenario: Requests and prompt known
- **WHEN** a model had 200 requests with usage in the window, advisor calls included, and 150 of them had a rebuilt prompt
- **THEN** its row shows `200` requests and `75%` prompt known

#### Scenario: Order of the parts
- **WHEN** the window holds priced requests and cache misses
- **THEN** the spend by origin and task comes after the totals and reconciliation lines, the spend by trigger comes after it, and both come before `Cache misses by cause`
