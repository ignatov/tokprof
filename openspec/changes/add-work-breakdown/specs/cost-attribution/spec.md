# Spec Delta

## ADDED Requirements

### Requirement: Activity of spend
Every share of spend SHALL also carry the activity of the request it comes from: what the agent was doing that led to the request. The activity is one more dimension that profiles stack by (see profile-export). Every share of a request has the same activity, its output included.

#### Scenario: Stacks by activity
- **WHEN** the user passes `--stack activity,component`
- **THEN** every stack starts with the request's activity, such as `reading` or `building and testing`

### Requirement: Activity of a request
A request's activity SHALL come from the same calls as its trigger (see Trigger of a request). Each call has a class, and the request takes the first class among its calls in this order: `editing`, `subagents`, `building and testing`, `version control`, `running scripts`, `file and system commands`, `other tools`, `web`, `reading`. A request whose trigger comes from no call takes its trigger's name.

#### Scenario: Classes of tools
- **WHEN** a call is an edit, a subagent, a fetch or web search, or a read-only call other than those (see Read-only calls)
- **THEN** its class is `editing`, `subagents`, `web` or `reading`
- **AND** a tool that is none of these and is not a shell is `other tools`, such as an MCP tool

#### Scenario: Mixed calls
- **WHEN** the previous response ran `cargo test` and read two files
- **THEN** the request's activity is `building and testing`

#### Scenario: No calls
- **WHEN** a request's trigger is `prompt`, `notification`, `compaction`, `continuation`, `advisor` or `unknown`
- **THEN** its activity has the same name

### Requirement: Activity of a shell command
A shell call's class SHALL be `reading` when its command is read-only (see Read-only commands). Otherwise the class comes from its first program, skipping `cd` parts, `NAME=value` assignments, shell keywords and the wrappers `time`, `timeout <duration>`, `env` and `nice`, with the program's path and a `.cmd` or `.exe` suffix dropped. A command that writes a file through `>`, `tee`, a heredoc or an in-place edit is `editing` when its first program reads or prints.

#### Scenario: Building and testing
- **WHEN** the first program is `cargo`, `npm`, `pnpm`, `yarn`, `gradle`, `gradlew`, `mvn`, `make`, `bazel`, `go`, `pytest`, `tsc`, `jest`, `vitest`, `swift`, `xcodebuild` or `dotnet`, or `bun` with anything but `-e`
- **THEN** the class is `building and testing`

#### Scenario: Version control
- **WHEN** the first program is `git`, with a subcommand that is not read-only, or `gh`
- **THEN** the class is `version control`

#### Scenario: Running scripts
- **WHEN** the first program is `python`, `python3`, `node`, `deno`, `ruby`, `perl`, `bash`, `sh`, `zsh`, `uv` or `bun -e`, or is given by a path, such as `scripts/report` or `./tools/build`, and is not a program the other lists name
- **THEN** the class is `running scripts`

#### Scenario: Writing files from the shell
- **WHEN** the command is `cat > notes.md <<'EOF'`, `echo x >> log.txt`, `sed -i 's/a/b/' f.rs` or `printf x | tee out.txt`
- **THEN** the class is `editing`

#### Scenario: Everything else
- **WHEN** the first program is any other, such as `mkdir`, `rm`, `cp`, `curl` or `ffmpeg`
- **THEN** the class is `file and system commands`

### Requirement: Spend by activity
`tokprof summary` SHALL print `Spend by activity: what the agent was doing before each request` and a table of whole requests with `activity`, `requests`, `avg prompt`, `cost`, `per request` and `share`, most expensive first, with every activity that has requests. Shares are of the cost, or of tokens when no request has a price.

#### Scenario: The table sums to the total
- **WHEN** the summary is printed
- **THEN** the costs of the activity rows add up to the `Total priced` figure

#### Scenario: Reading and lookups
- **WHEN** every lookup of a window read files and none fetched a page
- **THEN** the `reading` row has the requests and cost of the `lookup` and `lookups` rows of the trigger table together

## MODIFIED Requirements

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
- **WHEN** the previous response called `Edit`, `MultiEdit`, `Write`, `NotebookEdit` or Codex `apply_patch`, or ran a shell command whose class is `editing` (see Activity of a shell command), such as `cat > notes.md <<'EOF'`
- **THEN** the trigger is `edit`

#### Scenario: Subagents
- **WHEN** the previous response called Claude Code `Agent` or `Task`, or Codex `spawn_agent`
- **THEN** the trigger is `subagent`

#### Scenario: Other calls
- **WHEN** the previous response called a tool that is neither read-only, an edit nor a subagent, such as a shell command that neither only reads nor writes a file, or an MCP tool
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

### Requirement: Summary report
`tokprof summary` SHALL print `Spend by model (API-equivalent USD; token counts where no price is known)` and a table with one row per agent and model: `agent`, `model`, `requests`, `prompt known`, the five components and `total`, sorted by total cost, highest first. Then come the totals line, the unpriced models (see pricing), the reconciliation line when there is one, the spend by origin, task, trigger and activity, and the `Cache misses by cause` table (see cache-misses) without expected rebuilds.

#### Scenario: A model row
- **WHEN** the reported window holds 16,281 requests to `claude-opus-5-5`, all with a rebuilt prompt
- **THEN** its row starts with `claude`, `claude-opus-5-5`, `16281` and `100%`, followed by the dollars of each component and the total, such as `$1,192` or `$0.14`

#### Scenario: Requests and prompt known
- **WHEN** a model had 200 requests with usage in the window, advisor calls included, and 150 of them had a rebuilt prompt
- **THEN** its row shows `200` requests and `75%` prompt known

#### Scenario: Order of the parts
- **WHEN** the window holds priced requests and cache misses
- **THEN** the spend by origin and task comes after the totals and reconciliation lines, then the spend by trigger and the spend by activity, all before `Cache misses by cause`
