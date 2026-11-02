# Cost Attribution Specification

## Purpose
Turn the token usage logged for every request into API-equivalent dollars and charge them to the content that caused them, the way prefix caching bills, so spend can be broken down by session, chain, turn, content kind, tool and target, and summed up by `tokprof summary`.

## Requirements

### Requirement: Prompt cost by position
tokprof SHALL charge a request's prompt by position, the way prefix caching bills: the first `cache_read` tokens at the read rate, the next `cache_write` tokens at the write rate and the rest at the input rate. The prompt is all the uncached, cache-read and cache-write tokens the agent logged for the request, and the rates are those that pricing gives for the request's model, time and prompt size.

#### Scenario: A 100k prompt with 80k read, 15k written and 5k uncached
- **WHEN** a `claude-opus-5-5` request (input $4, 5-minute write $5, read $0.20 per million tokens) logs 80,000 `cache_read_input_tokens`, 15,000 `cache_creation_input_tokens` with the 5-minute lifetime and 5,000 `input_tokens`
- **THEN** its prompt costs $0.016 in cache read, $0.075 in cache write and $0.02 in uncached input, $0.111 in all

#### Scenario: Codex usage
- **WHEN** a Codex usage record logs 50,000 `input_tokens`, of which 40,000 are `cached_input_tokens`, and no `cache_write_input_tokens`
- **THEN** 40,000 tokens are charged at the read rate and 10,000 at the input rate

#### Scenario: A request without usage
- **WHEN** a request was sent but no usage was logged for it, because it was cancelled or got no response
- **THEN** it is not charged and no report counts it

### Requirement: Prompt layout
tokprof SHALL lay out a rebuilt prompt in the order it was sent (tool schemas, then the system prompt, then the conversation messages in order) and give each block a span proportional to its size in characters, scaled so that the blocks add up to the logged prompt size. A block's token count is therefore an estimate, while the prompt's total is as logged.

#### Scenario: Old content is read, new content is written
- **WHEN** a 100,000-token prompt holds 20,000 tokens of tool schemas and system prompt, 60,000 of earlier messages, a 15,000-token tool result and a 5,000-token new prompt, and logs 80,000 read, 15,000 written and 5,000 uncached tokens
- **THEN** the tool schemas, the system prompt and the earlier messages are charged at the read rate, the tool result at the write rate and the new prompt at the input rate

#### Scenario: Characters scaled to tokens
- **WHEN** a rebuilt prompt has 400,000 characters and the agent logged 100,000 prompt tokens
- **THEN** a 40,000-character tool result in it counts as 10,000 tokens

#### Scenario: A block across a boundary
- **WHEN** a block spans tokens 75,000 to 85,000 of a prompt that read 80,000 tokens from the cache and wrote the rest
- **THEN** 5,000 of its tokens are charged as cache read and 5,000 as cache write

### Requirement: Content pays while it stays in the context
tokprof SHALL charge every block its share in each request whose prompt contains it, so content pays for as long as it stays in the context, not only when it arrives.

#### Scenario: A file read once
- **WHEN** a 10,000-token file read is written to the 5-minute cache at $5 per million tokens and then read from the cache by 50 more requests at $0.20
- **THEN** it is charged $0.05 in cache write and $0.10 in cache read

#### Scenario: Output that becomes input
- **WHEN** a response's tool call is sent back in the following requests
- **THEN** it is charged as output by the request that produced it and as input by every later request that contains it

### Requirement: Cost components
tokprof SHALL split spend into five components, each with a short name for filters: `cache read` (`read`), prompt tokens read from the cache; `cache write` (`write`), prompt tokens written to the cache; `rewrite after miss` (`rewrite`), the cached prefix that a miss processed again; `uncached input` (`uncached`), prompt tokens neither read nor written; `output` (`output`), generated tokens, reasoning included.

#### Scenario: A request with every component
- **WHEN** after a 100,000-token prompt a request reads 20,000 tokens from the cache, writes 85,000, sends 100 uncached and generates 2,000
- **THEN** its spend is 20,000 tokens of `cache read`, 80,000 of `rewrite after miss`, 5,000 of `cache write`, 100 of `uncached input` and 2,000 of `output`

#### Scenario: A request without a miss
- **WHEN** a request is not a cache miss
- **THEN** it has no `rewrite after miss` spend

### Requirement: Rewrite after a miss
When a request misses the cache (see cache-misses), tokprof SHALL charge its re-processed tokens as `rewrite after miss`, with the miss cause, instead of as cache write or uncached input. They are the span right after the cache-read tokens, priced at the write rate where it lies in the written span and at the input rate beyond it.

#### Scenario: A miss on a growing prompt
- **WHEN** the previous request's prompt was 100,000 tokens and the next `claude-opus-5-5` request sends 105,000, reads 20,000 from the cache and writes 85,000 with the 5-minute lifetime ($5)
- **THEN** 80,000 tokens are `rewrite after miss` for $0.40, with the miss cause
- **AND** the remaining 5,000 written tokens are `cache write` for $0.025

#### Scenario: A miss beyond the written span
- **WHEN** a Codex request after a 50,000-token prompt sends 52,000 tokens, of which 10,000 are cached and none written
- **THEN** 40,000 tokens are `rewrite after miss` at the input rate and 2,000 are `uncached input`

#### Scenario: The first request of a chain
- **WHEN** the first request of a chain writes its whole 30,000-token prompt to the cache
- **THEN** it is charged as `cache write`, since there was no cached prefix to miss

### Requirement: Write rate by cache lifetime
tokprof SHALL price a request's written tokens at the average of its 5-minute and 1-hour write rates, weighted by the tokens the log reports for each lifetime (`cache_creation.ephemeral_5m_input_tokens` and `ephemeral_1h_input_tokens`). Written tokens without a lifetime, and Codex's `cache_write_input_tokens`, count at the short-lifetime rate.

#### Scenario: Mixed lifetimes
- **WHEN** a `claude-opus-5-5` request writes 10,000 tokens with the 5-minute lifetime ($5) and 30,000 with the 1-hour lifetime ($8)
- **THEN** all 40,000 written tokens are charged at $7.25 per million, $0.29 in all

#### Scenario: No lifetime breakdown
- **WHEN** a Claude Code usage record has `cache_creation_input_tokens` but no `cache_creation` object
- **THEN** all of its written tokens are charged at the 5-minute write rate

### Requirement: Output charged to what the response produced
tokprof SHALL charge output tokens at the output rate to the blocks of the message the response produced, shared by size. When the agent logs reasoning tokens (Claude Code `output_tokens_details.thinking_tokens`, Codex `reasoning_output_tokens`), the thinking blocks share those and the other produced blocks share the rest; otherwise all produced blocks share the whole output.

#### Scenario: Reasoning reported
- **WHEN** a response logs 1,000 output tokens, 600 of them reasoning, and produced a thinking block, a 300-character text and a 100-character tool call
- **THEN** the thinking block gets 600 tokens, the text 300 and the tool call 100

#### Scenario: No reasoning reported
- **WHEN** a response logs 1,000 output tokens without a reasoning count and produced a 2,000-character thinking block and a 2,000-character text
- **THEN** each gets 500 tokens

#### Scenario: Hidden thinking
- **WHEN** 600 reasoning tokens are logged but the produced message holds no thinking content
- **THEN** the 600 tokens go to a `thinking` share with no tool, target or turn

#### Scenario: No produced message
- **WHEN** the log does not tie a response to the message it produced
- **THEN** its output goes to an `assistant text` share with no tool, target or turn, apart from logged reasoning tokens, which go to a `thinking` share

### Requirement: Unknown content
tokprof SHALL charge prompt tokens it cannot place to content of kind `unknown` rather than guess what they were. A request whose prompt cannot be rebuilt (see session-logs) puts its whole prompt in one `unknown` share. A message the log refers to but does not hold becomes an `unknown` block, its size estimated as the average of the prompt's resolved messages.

#### Scenario: A transcript without request records
- **WHEN** a Claude Code transcript from before 2.1.300 has no request records
- **THEN** each request's whole prompt is one `unknown` share with no tool, target or turn, while its output still goes to the blocks it produced
- **AND** `tokprof summary` counts the request as one without a known prompt

#### Scenario: A missing message
- **WHEN** a rebuilt prompt refers to a message that is not in the log, and its resolved messages are 3,000 and 5,000 characters long
- **THEN** the missing message is an `unknown` block counted as 4,000 characters

#### Scenario: No resolved message
- **WHEN** none of the messages a rebuilt prompt refers to is in the log
- **THEN** each of them counts as one character, so the tool schemas and the system prompt take nearly the whole prompt

### Requirement: Dimensions of spend
Every share of spend SHALL carry the dimensions that profiles stack by (see profile-export): agent (`claude` or `codex`), project (the session's working directory, home shown as `~`), session (its short id, see session-logs), chain (its chain per session-logs, or `advisor` for advisor calls), model (as logged), turn, component, cause (on `rewrite after miss` only), kind, tool (the tool's name) and target (what the tool worked on, or a reminder's source).

#### Scenario: Content kinds
- **WHEN** spend is broken down by kind
- **THEN** the kinds are `tool schemas`, `system prompt`, `user prompt`, `reminders`, `tool results`, `assistant text`, `thinking`, `tool calls`, `images` and `unknown`

#### Scenario: A tool result
- **WHEN** a Claude Code `Read` of `src/main.rs` in the project returns a result
- **THEN** its spend has kind `tool results`, tool `Read` and target `src/main.rs`, and a cause only on its `rewrite after miss` share

#### Scenario: A reminder
- **WHEN** Claude Code injects the CLAUDE.md instructions as a system reminder
- **THEN** its spend has kind `reminders`, target `instructions` and no tool

#### Scenario: An advisor call
- **WHEN** a response consulted the advisor, which reads the conversation with another model
- **THEN** the advisor request is charged to chain `advisor` under the advisor's model, apart from the conversation's own chain

### Requirement: Turn of content
tokprof SHALL give content the turn in which it entered the context: the number of user-typed messages in the prompt up to and including its own message, in the first request of the reported window that contained it. Output takes the turn of the request that produced it. The tool schemas and system prompt that precede the conversation have no turn and show as `setup`.

#### Scenario: Content charged in later turns
- **WHEN** a tool result arrives after the third user prompt and is read from the cache until the seventh
- **THEN** all of its spend has turn 3

#### Scenario: After a compaction
- **WHEN** a compaction replaces the conversation with a summary
- **THEN** the turns of new content are counted within the new prompt, from its first user-typed message on

#### Scenario: Content before the first user prompt
- **WHEN** instructions are injected into the conversation before the first user-typed message
- **THEN** they have turn 0

#### Scenario: Shares without content of their own
- **WHEN** a share is `unknown` prompt content or output not tied to a produced message
- **THEN** it has no turn

### Requirement: Spend is dated when its content entered the context
tokprof SHALL date each share of spend when its content entered the context: the first request of the reported window whose prompt held it, or for output the request that produced it. Spend over time is thus charged back to the content that caused it, not spread over the requests that kept paying for it.

#### Scenario: A file read in the morning
- **WHEN** a file is read at 10:00 and stays in the context, read from the cache, until 12:00
- **THEN** all of its spend is dated 10:00

#### Scenario: Shares without content of their own
- **WHEN** a share is `unknown` prompt content or output not tied to a produced message
- **THEN** it is dated at the chain's first request in the reported window

### Requirement: Measures
Every share of spend SHALL carry three measures: tokens; cost in USD, absent when its model has no price; and avoidable cost in USD, which is the miss premium (see cache-misses) on `rewrite after miss` shares, expected rebuilds included, and zero on every other share.

#### Scenario: A rewrite share
- **WHEN** 80,000 tokens are `rewrite after miss` at a $5 write rate with a $0.20 read rate
- **THEN** the share has 80,000 tokens, a cost of $0.40 and an avoidable cost of $0.384

#### Scenario: A cache read share
- **WHEN** a share is `cache read`
- **THEN** its avoidable cost is zero

#### Scenario: A compaction rebuild
- **WHEN** a miss has the cause `compaction`
- **THEN** its `rewrite after miss` share still carries its premium as avoidable cost

### Requirement: Models without a price
tokprof SHALL attribute requests to a model without a price (see pricing) in tokens only: their shares have no cost and no avoidable premium, and `tokprof summary` shows token counts in their cells, `<tokens> tok` as their total, and leaves them out of `Total priced` and of the avoidable total.

#### Scenario: A model with no public price
- **WHEN** the logs hold `codex-auto-review` requests and no price file prices that model
- **THEN** its summary row shows token counts such as `90.5M` in the component columns and `101.3M tok` as the total
- **AND** its cache misses show `-` as avoidable

#### Scenario: A Codex request without a model
- **WHEN** a Codex rollout logs usage before it names a model
- **THEN** the request is attributed to model `unknown`, which has no price

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

### Requirement: Summary totals line
After the spend table, `tokprof summary` SHALL print `Total priced: <usd>   avoidable on cache misses: <usd> (<share>%)`: the cost summed over every priced model, and the avoidable premium of the misses that the cause table lists, so expected rebuilds are left out of it as they are left out of the table (see cache-misses). The share is the premium's share of the cost, to one decimal.

#### Scenario: Expected rebuilds in the total
- **WHEN** the window holds $506 of premium on unexpected misses and $42 on compactions and cleared tool results, out of $2,628 priced
- **THEN** the line reads `Total priced: $2,628   avoidable on cache misses: $506 (19.3%)`, the same $506 the cause table below it lists

### Requirement: Reconciliation with the agent's own cost counter
When a reported session logged Claude Code's own cost counter (`totalCostUSD` in `cost-state` lines), `tokprof summary` SHALL print `Claude Code's own cost counter for <n> sessions: <usd>; tokprof for the same sessions: <usd>`: the sum of the last counter value of each such session, and tokprof's priced spend for the same sessions. With `--since` or `--until`, only sessions whose every request falls inside the window are compared.

#### Scenario: All history
- **WHEN** 275 sessions logged a counter, summing to $10,241, and tokprof prices the same sessions at $10,353
- **THEN** the line reads `Claude Code's own cost counter for 275 sessions: $10,241; tokprof for the same sessions: $10,353`

#### Scenario: A session with subagent transcripts
- **WHEN** a session's main transcript and its subagent transcripts are read
- **THEN** they count as one session, with the spend of all of them

#### Scenario: A time filter
- **WHEN** `--since 7d` is given, 40 sessions with a counter have all their requests in the last 7 days and 235 more have requests before it
- **THEN** the line compares those 40 sessions only and reads `Claude Code's own cost counter for 40 sessions inside the window: <usd>; tokprof for the same sessions: <usd>`
- **AND** a session that only partly falls in the window is left out of both figures

#### Scenario: No counter
- **WHEN** no reported session logged a counter, as with Codex sessions only
- **THEN** the line is left out

#### Scenario: No session inside the window
- **WHEN** a time filter is given and every session with a counter has requests outside the window
- **THEN** the line is left out

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
A request's task SHALL be what its subagent was asked to do, else its session's title, else `untitled` followed by the session's short id (see session-logs):
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
- **THEN** its requests have task `untitled 019ff5ca-1a2b-7c3d` for Codex session `019ff5ca-1a2b-7c3d-…`

### Requirement: Session title
A session's title SHALL be:
- for Claude Code, the latest `custom-title` line, else the latest `ai-title` line, without a leading `[archived]` mark;
- for Codex, the `thread_name` of the thread in `~/.codex/session_index.jsonl` (under `$CODEX_HOME` when set), and for a subagent thread the title of the session it belongs to (see session-logs), however deep it is.

tokprof SHALL read the index again when its size changes.

#### Scenario: The user renamed a session
- **WHEN** a transcript has `ai-title` lines and then `"customTitle": "[archived] Mine"`, followed by another `ai-title`
- **THEN** the title is `Mine`

#### Scenario: A Codex thread from an IDE plugin
- **WHEN** the index has no entry for a thread
- **THEN** the thread has no title

#### Scenario: A thread spawned by a subagent
- **WHEN** main thread `M` is titled `Fix the parser` in the index, its subagent thread `A` spawned thread `B`, and neither `A` nor `B` is in the index
- **THEN** `A` and `B` both have the title `Fix the parser`

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
