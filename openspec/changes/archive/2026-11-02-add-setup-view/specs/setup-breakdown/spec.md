# Spec Delta

## Purpose
Shows what an agent puts in front of the work in every request (tool schemas, the system prompt, instruction files, skill listings, deferred tool lists and other reminders), what that setup cost, what conversations and subagents start with, and which setting of the agent shrinks the largest part.

## ADDED Requirements

### Requirement: Setup content
Setup SHALL be the content an agent adds to requests besides the work: tool schemas, the system prompt, and reminders of every source except `background notifications`, `attached files` and `task list`, which carry the work's own results back. Content kinds are those of cost-attribution, and reminder sources are those of session-logs.

#### Scenario: A skill listing
- **WHEN** a request holds Claude Code's skill listing
- **THEN** it is setup, with source `skills`

#### Scenario: A background agent's result
- **WHEN** a request holds a background agent's result as a reminder
- **THEN** it is not setup

### Requirement: Setup cost by source
The setup cost SHALL be the spend that cost-attribution charges to setup content, summed by source: `tool schemas`, `system prompt`, or the reminder's source. It covers every request of every chain, subagents included, and keeps the part charged to subagent chains apart. With no price, it is counted in tokens.

#### Scenario: Subagents carry the setup
- **WHEN** 96 subagents each re-read 9.7k tokens of instructions in every request
- **THEN** the shares charged to those reminders count under `instructions`, and under its subagent part

#### Scenario: Several sessions
- **WHEN** the setup cost is summed over several sessions
- **THEN** each source adds up its shares over all of them

### Requirement: What a conversation starts with
What a conversation starts with SHALL be the setup in the first request of its chain whose prompt the log shows, in tokens by source. Each block is sized as session-xray sizes the blocks of the latest prompt: its share of the characters, scaled to the request's prompt tokens. For subagents, it is the mean over the subagent chains whose first prompt is known, with the mean of their briefs: the user prompt blocks of that first request.

#### Scenario: Subagents of a code review
- **WHEN** 96 subagent chains have a known first prompt, and the mean of their setup is 31.2k tokens with instructions 9.7k, skills 9.6k and tool schemas 6.4k
- **THEN** the subagents start with 31.2k tokens by those sources, and a mean brief of 1.8k

#### Scenario: A subagent the log does not show
- **WHEN** a subagent's log has no request with a known prompt, as before Claude Code 2.1.300
- **THEN** it is not counted among the subagents that start with the setup

#### Scenario: The main conversation
- **WHEN** the main conversation's first known prompt holds 36.0k tokens, of which the user's message is 0.6k
- **THEN** the main conversation starts with the 35.4k tokens of setup in it, by source

### Requirement: What a compaction leaves
What a compaction leaves SHALL be the prompt of the main conversation's first answered request after its latest compaction marker (see session-logs), when the log shows that prompt: its size and its tokens by content kind, reminders by source, sized as in What a conversation starts with.

#### Scenario: Restored content after a compaction
- **WHEN** the main conversation was compacted at 2026-10-13 15:31 and its next request sent 134.9k tokens, including background agents' results, files Claude Code restored and the skills invoked before the compaction
- **THEN** what the compaction left is 134.9k tokens, with `background notifications`, `attached files`, `skills` and `user prompt` among its parts

#### Scenario: No compaction
- **WHEN** the main conversation has no compaction marker, or the request after it has no known prompt
- **THEN** there is nothing on what a compaction leaves

### Requirement: Setup hints
Each agent's log source SHALL give a hint for the setup sources that it can name a remedy for, in the agent's own terms. Sources without a hint get none.

#### Scenario: Claude Code hints
- **WHEN** the agent is Claude Code
- **THEN** `skills` gets "lower skillListingBudgetFraction (default 0.01 of the context window), or turn unused skills off with skillOverrides in settings.json"
- **AND** `instructions` gets "trim CLAUDE.md and AGENTS.md, and keep the auto-memory MEMORY.md to short index lines"
- **AND** `deferred tool lists` gets "turn off the MCP servers this project does not use, with /mcp"
- **AND** `hook context` gets "make the hooks print less"

#### Scenario: Codex hints
- **WHEN** the agent is Codex
- **THEN** `instructions` gets "trim AGENTS.md", and no other source has a hint

### Requirement: Setup in the summary
`tokprof summary` SHALL print `Setup: what every request carries before the work` and a table with `agent`, `source`, `cost`, `subagents`, `share` and `hint`, with the rows of each agent's setup sources in the reported requests. `subagents` is the part of the cost charged to subagent chains, and `share` is the share of the agent's spend in the reported requests, to one decimal. Without a price, the cells hold tokens.

#### Scenario: A row
- **WHEN** Claude Code's skill listings cost $197 in the window, $150 of it in subagents, of a $9,174 spend
- **THEN** the row reads `claude`, `skills`, `$197`, `$150`, `2.1%` and the skills hint

#### Scenario: Nothing to show
- **WHEN** the reported requests hold no setup content with a cost or tokens
- **THEN** the section is left out

### Requirement: Setup rows per agent
The setup table SHALL group its rows by agent, the agent whose setup cost the most first, and list each agent's sources most expensive first. Up to 8 sources of an agent are rows; the rest of that agent's sources fold into one `<n> more` row with their summed cost, subagent part and share, and no hint.

#### Scenario: Many small sources
- **WHEN** Codex has 22 setup sources in the window and Claude Code has 7, and Claude Code's setup cost more
- **THEN** the 7 Claude Code rows come first, then the 8 most expensive Codex sources, then one row `codex  14 more` with the other 14 sources together

#### Scenario: A few sources
- **WHEN** an agent has 8 setup sources or fewer
- **THEN** each is a row and there is no `more` row for it

### Requirement: Setup total per agent
After the setup table, `tokprof summary` SHALL print one line per agent with setup: `<agent title> setup: <cost>, <share>% of its spend`.

#### Scenario: The agent line
- **WHEN** Claude Code's setup sources add up to $915 of a $9,174 spend
- **THEN** the line reads `Claude Code setup: $915, 10.0% of its spend`
