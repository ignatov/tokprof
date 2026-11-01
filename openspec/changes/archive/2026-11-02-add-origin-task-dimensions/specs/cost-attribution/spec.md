# Spec Delta

## ADDED Requirements

### Requirement: Origin and task of spend
Every share of spend SHALL also carry the origin and the task of the request it comes from, as two more dimensions that profiles stack by (see profile-export). Both come from the logs alone and need no setup. Every share of a request has the same origin and task, whatever block of the prompt it falls on.

#### Scenario: A review in the main conversation
- **WHEN** a main conversation titled `Fix the parser` sends one request for a typed prompt and then one that Claude Code attributes to the `code-review` skill
- **THEN** the first request's spend has origin `prompt`, the second's `skill:code-review`, and both have task `Fix the parser`
- **AND** the cache reads of the second request go to `skill:code-review` even for content an earlier prompt added

### Requirement: Origin of a request
A request's origin SHALL be the first that applies:
- `skill:<name>`, `mcp:<server>` or `plugin:<name>`, from Claude Code's `attributionSkill`, `attributionMcpServer` or `attributionPlugin` on the response;
- `agent:<type>` for a subagent;
- `command:/<name>` for a turn the user started with a slash command;
- the origin of a background task for the turn its result starts;
- `prompt`.

#### Scenario: A skill's subagent
- **WHEN** the `code-review` skill starts a `general-purpose` subagent whose responses carry `attributionSkill` `code-review`
- **THEN** the subagent's requests have origin `skill:code-review`

#### Scenario: A subagent without a skill
- **WHEN** a typed prompt starts an `Explore` subagent
- **THEN** the subagent's requests have origin `agent:Explore`, the `agentType` of its meta file, or else the type in its stream name

#### Scenario: Codex subagent threads
- **WHEN** a Codex thread was spawned with `spawn_agent` and no role, or is an auto-review (`guardian`) or `/review` thread
- **THEN** its requests have origin `agent:default`, `agent:guardian` or `agent:review`

#### Scenario: A slash command
- **WHEN** the user's message is `<command-message>goal</command-message>` with `<command-name>/goal</command-name>`
- **THEN** the requests of that turn have origin `command:/goal`

#### Scenario: Codex main threads
- **WHEN** a Codex main thread sends a request
- **THEN** its origin is `prompt`

### Requirement: Origin of a background task's follow-up
A Claude Code turn that starts with a `<task-notification>` SHALL take the origin of the task its `<task-id>` names:
- for a subagent of the session, the origin of the response that started it;
- else, when that response had no attribution, `agent:<type>` from the subagent's meta file;
- `notification` for a task that is not a subagent of the session, such as a background shell command.

#### Scenario: A skill's background subagent reports back
- **WHEN** a response attributed to `deep-research` starts a background subagent, and its result later arrives as a notification
- **THEN** the main conversation's requests that follow have origin `skill:deep-research`

#### Scenario: A background shell command finishes
- **WHEN** a notification names a task that has no subagent meta file
- **THEN** the turn's requests have origin `notification`

### Requirement: Turns that carry on
A Claude Code user message SHALL keep the origin of the turn before it when it is an interruption (`[Request interrupted`), the summary that continues a compacted conversation (`This session is being continued`), a shell command run from the prompt (`<bash-…>`), a reminder, or a message with tool results.

#### Scenario: An interrupted command
- **WHEN** the user interrupts a `/goal` turn and the agent goes on
- **THEN** the requests after the interruption still have origin `command:/goal`

### Requirement: Task of a request
A request's task SHALL be what its subagent was asked to do, else its session's title, else `untitled` followed by the session's 8-character id:
- a Claude Code subagent's task is the `description` in its `subagents/agent-<id>.meta.json`;
- a Codex thread's task is the last part of its `agent_path`, with underscores as spaces.

#### Scenario: A Claude Code subagent
- **WHEN** a subagent's meta file has `"description": "Angle C cross-file tracer"`
- **THEN** its requests have task `Angle C cross-file tracer`

#### Scenario: A Codex spawned thread
- **WHEN** a thread's `agent_path` is `/root/fix_flaky_tests`
- **THEN** its requests have task `fix flaky tests`

#### Scenario: No title
- **WHEN** a session has no title and is not a subagent
- **THEN** its requests have task `untitled 019ff5ca` for session `019ff5ca-…`

### Requirement: Session title
A session's title SHALL be:
- for Claude Code, the latest `custom-title` line, else the latest `ai-title` line, without a leading `[archived]` mark;
- for Codex, the `thread_name` of the thread in `~/.codex/session_index.jsonl` (under `$CODEX_HOME` when set), and for a subagent thread its parent's.

tokprof SHALL read the index again when its size changes.

#### Scenario: The user renamed a session
- **WHEN** a transcript has `ai-title` lines and then `"customTitle": "[archived] Mine"`, followed by another `ai-title`
- **THEN** the title is `Mine`

#### Scenario: A Codex thread from an IDE plugin
- **WHEN** the index has no entry for a thread
- **THEN** the thread has no title

### Requirement: Spend by origin
`tokprof summary` SHALL print `Spend by origin: the kind of work each request was part of` and a table of whole requests with `origin`, `requests`, `tokens`, `cost` and `share`, most expensive first. The table holds the top 15 origins; the rest fold into one `<n> more` row. Shares are of the cost, or of tokens when no request has a price.

#### Scenario: Shares
- **WHEN** code review cost $1,720 of $9,401 priced
- **THEN** its row reads `skill:code-review`, its request count, its tokens, `$1,720` and `18.3%`

#### Scenario: No price
- **WHEN** no request in the window has a price
- **THEN** the cost cells show tokens, such as `1.2M tok`, and the shares are of tokens

### Requirement: Most expensive tasks
`tokprof summary` SHALL print `Most expensive tasks: session titles, and what subagents were asked to do` and a table of the 15 most expensive tasks with `task`, `origin`, `requests`, `cost` and `share`. A task's origin is the one most of its cost came from. Task names longer than 60 characters are cut with `…`.

#### Scenario: One task, several origins
- **WHEN** a session titled `Fix the parser` cost $1 in typed prompts and $5 in code review
- **THEN** its task row shows `skill:code-review`, 2 requests and the $6 together

## MODIFIED Requirements

### Requirement: Summary report
`tokprof summary` SHALL print `Spend by model (API-equivalent USD; token counts where no price is known)` and a table with one row per agent and model: `agent`, `model`, `requests`, `prompt known`, the five components and `total`, sorted by total cost, highest first. Then come the totals line, the unpriced models (see pricing), the reconciliation line when there is one, the spend by origin and task, and the `Cache misses by cause` table (see cache-misses) without expected rebuilds.

#### Scenario: A model row
- **WHEN** the reported window holds 16,281 requests to `claude-opus-5-5`, all with a rebuilt prompt
- **THEN** its row starts with `claude`, `claude-opus-5-5`, `16281` and `100%`, followed by the dollars of each component and the total, such as `$1,192` or `$0.14`

#### Scenario: Requests and prompt known
- **WHEN** a model had 200 requests with usage in the window, advisor calls included, and 150 of them had a rebuilt prompt
- **THEN** its row shows `200` requests and `75%` prompt known

#### Scenario: Order of the parts
- **WHEN** the window holds priced requests and cache misses
- **THEN** the spend by origin and task comes after the totals and reconciliation lines and before `Cache misses by cause`
