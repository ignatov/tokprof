# Spec Delta

## ADDED Requirements

### Requirement: Work block
After `spend`, the X-ray SHALL print a `work` block when the session, its subagents included, has more than one origin or more than one task (see cost-attribution). The block starts with `work      by origin: the kind of work each request was part of`. Then comes one line per origin, most expensive first, with a bar, its share and its cost. The sixth and later origins fold into one `<n> more` line.

#### Scenario: A code-review session
- **WHEN** a session ran the `code-review` skill with 98 subagents, at a cost of $508 of its $526
- **THEN** the first origin line reads `skill:code-review` with a nearly full bar, `96.5%` and `$508`, followed by `prompt` and the session's other origins

#### Scenario: Typed prompts only
- **WHEN** every request of a session without subagents comes from typed prompts and the session has one title
- **THEN** there is no `work` block

### Requirement: Tasks in the work block
When the session has more than one task, the work block SHALL go on with `by task: the session title, and what each subagent was asked to do`. Up to 6 tasks follow, most expensive first, each with its cost, the task cut to 50 characters and its main origin. A last line `<n> more tasks` counts the rest.

#### Scenario: Reviewers by name
- **WHEN** the code-review subagents were described as `Angle C cross-file tracer` and `Angle A line-by-line scan`
- **THEN** the task lines read, for example, `$57.46  Angle C cross-file tracer` with `skill:code-review` dimmed after it

## MODIFIED Requirements

### Requirement: Header
The X-ray SHALL open with the title `tokprof · session X-ray` and these lines:
- `session` with the agent, the project path (home shown as `~`) and the full session id;
- `title` with the session's title (see cost-attribution) cut to 82 characters, when it has one;
- `models` with each model and its request count;
- `time` with the first and last request in local time, the span between them, the request count and, when there are any, the number of subagent logs.

#### Scenario: Session within one day
- **WHEN** the first and last requests fall on the same local date
- **THEN** the end shows only the time of day, as in `time      2026-11-01 08:49 → 18:15 (9.4h) · 657 requests`

#### Scenario: Session over several days
- **WHEN** a session ran from one day into the next
- **THEN** both ends show the date, as in `time      2026-10-22 11:18 → 2026-10-23 19:57 (1.4d) · 7015 requests · 75 subagents`

#### Scenario: Models
- **WHEN** a session used two models
- **THEN** the `models` line lists both by name with their counts, as in `claude-haiku-4-5 ×12, claude-opus-5-5 ×657`

#### Scenario: Span format
- **WHEN** the span is under a minute, under an hour, under a day, or longer
- **THEN** it shows in seconds (`59s`), whole minutes (`45m`), hours with one decimal (`9.4h`) or days with one decimal (`1.4d`)

#### Scenario: Title
- **WHEN** the session's title is `Review PR #412 against the checklist`
- **THEN** the line after `session` reads `title     Review PR #412 against the checklist`
- **AND** a session without a title has no `title` line

### Requirement: Layout
The X-ray SHALL print, in this order and separated by blank lines: the header; the `context`, `cache` and `next` lines with the advice; `spend`; the `work` block when it is shown; `in context`; `what cost the most`; the cache misses; and the pointer `profile: tokprof flame --session <id> → <dir>/<id>.jfr (IntelliJ: Open snapshot)`, with the session's 8-character id and `~/tokprof`, or `$TOKPROF_DIR` when set, as `<dir>`. The parts about the live context need a main conversation.

#### Scenario: Session with a main conversation
- **WHEN** the main log has a request with a prompt
- **THEN** every part is printed in the order above

#### Scenario: No main conversation
- **WHEN** the main log has no request with a prompt, for example when only its subagents made requests or the target is a subagent transcript
- **THEN** the `context`, `cache` and `next` lines, the advice and `in context` are left out
- **AND** the header, `spend`, the `work` block when it is shown, `what cost the most`, the cache misses and the pointer are still printed

#### Scenario: Session without requests
- **WHEN** the log holds no request with usage
- **THEN** the `models` line is empty, the `time` line says `0 requests`, `spend` shows `0 tokens (no price)` and the misses line says `no misses`
