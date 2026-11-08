# Spec Delta

## ADDED Requirements

### Requirement: Request timeline
`Requests` SHALL open with a `Request timeline` card: the requests of the main conversation (the main log's main chains and advisor calls) on the time axis of Summary's track. Each request is a column of five stacked parts, the X-ray's components in the Spend card's colors: cache read, cache write, rewrite after miss, uncached input and output. They add up to its cost. A pill `$` / `tokens` switches the parts to tokens. Requests within the same 4 pt of the axis share one column.

#### Scenario: The parts add up
- **WHEN** a request read 412.3k tokens from the cache, re-wrote 490.3k after an idle break, sent 3 uncached tokens and wrote 1.2k of output
- **THEN** its column stacks its cache read, rewrite after miss, uncached input and output in dollars, their sum is the cost the table gives for it, and over the session the columns add up to the X-ray's `main` spend, plus `advisor` when the session has advisor calls

#### Scenario: Tokens
- **WHEN** the user clicks `tokens`, or presses `t` while the inspector has the keyboard
- **THEN** the columns show the same parts in tokens, the scale at the left is in tokens, and the pill shows `tokens` as chosen

#### Scenario: Requests close together
- **WHEN** several requests fall within the same 4 pt of the axis
- **THEN** they share one column whose parts are their sums, and the hover names how many requests it holds

#### Scenario: What stays
- **WHEN** `Requests` shows a session
- **THEN** the axis, the warm and cold spans and the dashed next request are those of Summary's track, and the times under the axis are those of Summary

### Requirement: Context line
The timeline SHALL draw each request's context as a line on a second scale at its right, from 0 to the context window of the session's latest model, or to the largest context when no window is known.

#### Scenario: A known window
- **WHEN** the latest model's context window is 1.0M
- **THEN** the right scale reads `1.0M` at its top and `0` at its foot, and the line passes through each column's context at the height of its share of the window

#### Scenario: No known window
- **WHEN** no context window is known for the session's latest model
- **THEN** the right scale goes up to the largest context and reads `largest` at its top

### Requirement: Miss and compaction markers
The timeline SHALL draw a line at each unexpected cache miss of the main conversation, labelled by the name of its cause. The line is red when the miss's avoidable premium is $0.50 or more, and in the cache write's color otherwise. A compaction of the main log is a dashed line labelled `compacted` with its kind. An expected rebuild has no miss line. A label is drawn when it keeps 8 pt clear of the label before it.

#### Scenario: A costly idle miss
- **WHEN** a request at 14:00 missed after 1.0h idle on a 1h cache, with $3.82 avoidable
- **THEN** a red line stands at 14:00, labelled `idle > TTL`, and its column shows the rewrite after miss in red

#### Scenario: A small miss
- **WHEN** a request missed because its tools changed, with $0.12 avoidable
- **THEN** its line is in the cache write's color, labelled `tools changed`

#### Scenario: Expected rebuilds
- **WHEN** the conversation compacted at 16:49 and the next request rebuilt its prompt
- **THEN** a dashed line stands at 16:49 labelled with the kind, such as `compacted (auto)`, and no miss line marks the rebuild

#### Scenario: The markers and the X-ray
- **WHEN** the X-ray of the session lists cache misses of the chain `main`
- **THEN** the timeline has one miss line for each of them, at its time and with its cause, and none for a subagent's misses

### Requirement: Event markers
A row of small marks above the columns SHALL show, from the main log:
- each message the user typed, in the message color;
- each time the user stopped the agent, in the stop color;
- each failure streak, as a red triangle at the request that read the third failed result. A streak is 3 or more tool calls in a row whose results failed; refused and denied calls neither count nor break it;
- each agent's commit, a shell call that ran `git commit` and worked, in green.

#### Scenario: A streak
- **WHEN** three `Read` calls in a row failed, then one worked
- **THEN** one red triangle stands at the request that read the third failure

#### Scenario: A commit
- **WHEN** the agent ran `git commit` and it worked
- **THEN** a green mark stands at the request that made the call

### Requirement: Subagent band
A band under the columns SHALL show each subagent's span, from its first request to its last, with the subagent's task as its hover. The subagents' requests are not columns of the timeline.

#### Scenario: A subagent
- **WHEN** a subagent made requests from 12:29 to 12:37
- **THEN** the band under the columns covers 12:29 to 12:37, and its hover names the subagent's task

### Requirement: Request under the pointer
A card over the timeline SHALL describe the column under the pointer: the request's time, model and effort; its five parts in dollars and tokens and its cost; its context; the cause and detail when it missed, with the avoidable dollars; and the markers at its place. For a shared column, it gives the number of requests, their first and last time, and the sums.

#### Scenario: A miss
- **WHEN** the pointer rests on the column of a request that missed after an idle break
- **THEN** the card reads its time, `claude-opus-5-5`, its parts, its context, and `missed: idle > TTL: 1.0h idle, 1h TTL · $3.82 avoidable`

#### Scenario: Leaving the chart
- **WHEN** the pointer leaves the chart
- **THEN** the card goes away

### Requirement: Selected request
One request of the timeline SHALL be selected, the costliest when the tab opens for a session. A click on a column selects its request, the costliest of a shared column. The timeline marks its column in the accent. A card under the timeline shows it: its number, time, model, effort and cost; its cost split as a bar and a row per part in dollars and tokens; and the why, what entered and what changed of the requirements below.

#### Scenario: Another session
- **WHEN** the user selects another session and shows `Requests`
- **THEN** that session's costliest request is selected

#### Scenario: A click
- **WHEN** the user clicks a column of the timeline
- **THEN** its request is selected and the card shows it

### Requirement: Why a request cost what it did
The selected request's card SHALL say what the request read from the cache, what it wrote and why. The reason is the first that applies:
- the miss, with its cause and detail and the avoidable dollars;
- an expected rebuild, with its cause;
- the first request of its chain;
- the context growing or shrinking by the difference from the previous request.

#### Scenario: Why a request cost $3.95
- **WHEN** the selected request read 12.3k, re-wrote 490.3k after 1.0h idle on a 1h cache and cost $3.95
- **THEN** the why reads that it read 12.3k from the cache and wrote 490.3k because it missed the cache: idle > TTL: 1.0h idle, 1h TTL, with $3.82 avoidable

#### Scenario: A growing context
- **WHEN** a request without a miss wrote 5.1k tokens and its context is 5.1k larger than the previous request's
- **THEN** the why reads that it wrote 5.1k as the context grew by 5.1k

### Requirement: What entered since the previous request
The selected request's card SHALL list the blocks that entered the context since the previous request of its chain, grouped by kind, tool and target, with tokens, largest first: up to 8, then `<n> more` with their tokens. A block's size is the share of the prompt its text takes, as in `Context`. For the first request of a chain the list is what its prompt holds. When the log does not show the prompt, it reads `unknown: the log does not show this prompt`.

#### Scenario: What entered
- **WHEN** a tool result of `Bash cargo test` of 6.1k tokens, the call of 200 tokens and 400 tokens of assistant text entered since the previous request
- **THEN** the card lists `6.1k tool results Bash cargo test`, then `400 assistant text` and `200 tool calls Bash cargo test`

#### Scenario: An old log
- **WHEN** the log does not show what the request's prompt held
- **THEN** the list reads `unknown: the log does not show this prompt`, and no size is guessed

### Requirement: What changed since the previous request
The selected request's card SHALL list what changed since the previous request of its chain: the model, the effort, another setting the log names, the agent's version, the tools added and removed, the system prompt's size, and a compaction between them. It also gives the time since the previous request against the cache's lifetime, in red when it is longer.

#### Scenario: A long break
- **WHEN** the selected request came 1.0h after the previous one, and the cache lives 1h
- **THEN** the card reads `1.0h after the previous request, past the 1h cache` in red

#### Scenario: New tools
- **WHEN** two tools were added since the previous request and the effort went from `high` to `max`
- **THEN** the card reads `effort high → max` and `tools +2 −0` with the names of the two tools

### Requirement: Range of requests
A drag of 4 pt or more across the timeline SHALL select the requests whose columns it covers, shaded on the chart. While a range is selected, the card under the timeline totals it instead:
- the first and last request, their times and the span;
- the number of requests and the five parts in dollars and tokens;
- the unexpected misses in it and their avoidable dollars, the compactions, and the messages the user typed.

A click on a column, a plain step, or esc clears the range.

#### Scenario: Dragging over an hour
- **WHEN** the user drags from 13:00 to 14:05 over 40 requests that cost $12.40 with one miss of $3.82 avoidable
- **THEN** the card reads `40 requests`, `13:00 → 14:05`, `$12.40` with its parts, and `1 miss · $3.82 avoidable`

#### Scenario: Esc
- **WHEN** a range is selected, the inspector has the keyboard, and the user presses esc
- **THEN** the range is cleared and the card shows the selected request again, and the inspector keeps the keyboard

### Requirement: Requests table
Under the cards, `Requests` SHALL list the requests in a table: `#`, `sent`, `after` (red when longer than the cache lives), `in context`, then `read`, `write`, `uncached` and `output` in tokens as the log gives them, `cost`, and `why`. The why is `missed: <cause>`, `rebuilt: <cause>` for an expected rebuild, `first request`, or `context +<n>` or `context −<n>`. Where the inspector is 760 pt tall or more, the timeline and the cards stay above the table while it scrolls.

#### Scenario: A miss in the table
- **WHEN** request 57 missed after an idle break
- **THEN** its row's why reads `missed: idle > TTL`

### Requirement: Rows of the requests table
The table SHALL hold up to 300 rows: all of them, the 300 that end at the latest request, or the 300-row page that holds the selected request. A note says how many earlier or later requests are left out. The selected request's row is marked and kept on the screen, and a click on a row selects its request.

#### Scenario: The row follows the chart
- **WHEN** the user clicks a column of the timeline
- **THEN** that request's row is marked and scrolled into view, and the card shows it

#### Scenario: An early request
- **WHEN** a session has 622 requests and the user selects the 10th
- **THEN** the table shows requests 1 to 300 and a note `and 322 later requests`

### Requirement: Keys in Requests
While the inspector has the keyboard on `Requests`, the keys SHALL be:
- ← and → (and `h`, `l`) select the request before or after, as ↑ and ↓ (and `k`, `j`) do;
- PgUp and PgDn move by ten requests, Home and End to the first and the last;
- Shift+↑ and Shift+↓ start or extend a range from the selected request;
- `t` switches between dollars and tokens;
- esc clears a range before it gives the keyboard back;
- Shift+← and Shift+→ show the tab before or after, as in every tab.

#### Scenario: Stepping
- **WHEN** the costliest request is the 57th and the user presses →
- **THEN** the 58th request is selected, in the chart, the card and the table

#### Scenario: Leaving the tab
- **WHEN** the user presses Shift+→ on `Requests`
- **THEN** the inspector shows `Spend`

### Requirement: Requests in a frame without a window
`--scene requests-hover` SHALL draw the frame on `Requests` with the inspector's keyboard and the pointer's card over the selected request's column. `--scene requests-range` SHALL draw it with a range from the selected request over the 20 requests before it. Both use `--session` to pick a session, as the list's filter does.

#### Scenario: A session's timeline
- **WHEN** the user runs `tokprof-desk --snap out.png --session 230043e6 --tab requests`
- **THEN** `out.png` shows that session's timeline with its costliest request selected

## MODIFIED Requirements

### Requirement: Other tabs
The tabs from `Spend` to `Cache` SHALL show session-xray's blocks of those names: the spend by component and by chain, work by origin and by activity, waits and slow requests, the prompts, the tool table, what the context holds and what cost the most to keep in it, and the cache misses. `Requests` shows the request timeline (see Request timeline). A click on a tab shows it.

#### Scenario: Prompts
- **WHEN** the user clicks `Prompts`
- **THEN** the inspector lists each typed message with its cost, its requests and its two costliest activities, as the X-ray's prompts block does

### Requirement: Keys in the inspector
While the inspector has the keyboard, ← and → (and `h`, `l`) SHALL show the tab before or after outside `Flame` and `Requests`, and Shift+← and Shift+→ in every tab; the digits 1 to 9 show the tab of that place, and 0 `Flame`. In a tab with rows (bars, tables, prompts, misses), ↑ and ↓ (and `k`, `j`) move a cursor by a row, PgUp and PgDn by ten, Home and End to the ends, and the inspector scrolls to show it. In `Summary` they scroll it.

#### Scenario: Through the prompts
- **WHEN** the inspector has the keyboard on `Prompts` and the user presses ↓ three times
- **THEN** the fourth prompt's row is marked, and it is on the screen

#### Scenario: Another tab
- **WHEN** the inspector has the keyboard and the user presses 7
- **THEN** it shows `Tools`, with the cursor on its first row

#### Scenario: The flame graph by its key
- **WHEN** the inspector has the keyboard and the user presses 0
- **THEN** it shows `Flame`, with the cursor on the root

#### Scenario: Leaving the flame graph
- **WHEN** the inspector has the keyboard on `Flame` and the user presses Shift+←
- **THEN** it shows `Cache`

#### Scenario: Requests take the arrows
- **WHEN** the inspector has the keyboard on `Requests` and the user presses ←
- **THEN** the request before is selected, and the tab stays `Requests`
