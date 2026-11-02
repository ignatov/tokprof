# Spec Delta

## ADDED Requirements

### Requirement: Prompts block
When the main conversation has at least two messages the user typed (see session-logs), the X-ray SHALL print a `prompts` block, `prompts   what each message you typed cost, subagents included`. Each line gives the message's number in the session, its cost, its requests, its label and its two costliest activities (see cost-attribution) with their cost.

#### Scenario: A message and its work
- **WHEN** the third typed message was followed by 74 requests that cost $8.53, $3.10 of them reading and $2.90 editing
- **THEN** its line reads `  3   $8.53   74 req  <label>` followed, dimmed, by `reading $3.10 · editing $2.90`

#### Scenario: What a message's cost covers
- **WHEN** a message is followed by main requests, a subagent started before the next typed message, and the turns of background results
- **THEN** its cost and requests cover all of them: main requests from its time to the next typed message, and every request of the subagent chains whose first request falls in that time
- **AND** requests before the first typed message count with the first

#### Scenario: The label
- **WHEN** a message holds a pasted X-ray, a blank line and then `what else can we analyze?`
- **THEN** its label is `what else can we analyze?`, the last paragraph with its whitespace joined, cut to 40 characters with `…`

#### Scenario: Many messages
- **WHEN** the session has 30 typed messages
- **THEN** the block lists the 8 most expensive in session order, then `<n> more messages` with their cost and requests together

#### Scenario: One message
- **WHEN** the main conversation has one typed message
- **THEN** there is no `prompts` block

#### Scenario: No prices
- **WHEN** nothing in the session is priced
- **THEN** the amounts are tokens, and the most expensive messages are those with the most tokens

### Requirement: Activities in the work block
The work block SHALL end with `by activity: what the agent was doing before each request`, followed by one line per activity of the session, its subagents included, most expensive first, with a bar, its share and its cost. The seventh and later activities fold into one `<n> more` line.

#### Scenario: A coding session
- **WHEN** a session spent $5.42 on requests after reads, $4.71 after edits and $2.14 answering messages
- **THEN** the activity lines start with `reading`, `editing` and `prompt`, in that order, each with its bar, share and cost

## MODIFIED Requirements

### Requirement: Layout
The X-ray SHALL print, in this order and separated by blank lines: the header; the `context`, `cache` and `next` lines with the advice; `spend`; the `work` block when it is shown; the `prompts` block when it is shown; `in context`; `what cost the most`; the cache misses; and the pointer `profile: tokprof flame --session <id> → <dir>/<id>.jfr (IntelliJ: Open snapshot)`, with the session's short id (see session-logs) and `~/tokprof`, or `$TOKPROF_DIR` when set, as `<dir>`. The parts about the live context need a main conversation.

#### Scenario: Session with a main conversation
- **WHEN** the main log has a request with a prompt
- **THEN** every part is printed in the order above

#### Scenario: No main conversation
- **WHEN** the main log has no request with a prompt, for example when only its subagents made requests or the target is a subagent transcript
- **THEN** the `context`, `cache` and `next` lines, the advice and `in context` are left out
- **AND** the header, `spend`, the `work` block when it is shown, the `prompts` block when it is shown, `what cost the most`, the cache misses and the pointer are still printed

#### Scenario: Session without requests
- **WHEN** the log holds no request with usage
- **THEN** there is no `models` line, the `time` line says `0 requests`, `spend` shows `0 tokens (no price)` and the misses line says `no misses`

### Requirement: Work block
After `spend`, the X-ray SHALL print a `work` block when the session, its subagents included, has more than one origin, task or activity (see cost-attribution). The block starts with `work      by origin: the kind of work each request was part of`. Then comes one line per origin, most expensive first, with a bar, its share and its cost. The sixth and later origins fold into one `<n> more` line.

#### Scenario: A code-review session
- **WHEN** a session ran the `code-review` skill with 98 subagents, at a cost of $508 of its $526
- **THEN** the first origin line reads `skill:code-review` with a nearly full bar, `96.5%` and `$508`, followed by `prompt` and the session's other origins

#### Scenario: Typed prompts only
- **WHEN** every request of a session without subagents comes from typed prompts, with no tool call, and the session has one title
- **THEN** there is no `work` block

### Requirement: Content that cost the most
Under `what cost the most to keep in the context (each source with every request that re-read it)`, the X-ray SHALL list the 8 pieces of content that cost the most over the whole session, subagents included. Content is grouped by kind, tool and target, ranked by dollars, or by tokens when nothing is priced. Each row gives the amount, the kind, the tool and target, and for tool calls and results the number of calls behind it.

#### Scenario: Priced session
- **WHEN** the session has prices
- **THEN** rows read like `     $10.13  tool results   exec_command  sed -n   412 calls`
- **AND** the count is the number of calls with that tool and target in the session, subagents included, and `1 call` for one

#### Scenario: No prices
- **WHEN** nothing in the session is priced
- **THEN** the rows are ranked by tokens and show tokens
