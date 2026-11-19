# Spec Delta

## MODIFIED Requirements

### Requirement: Other tabs
The tabs from `Spend` to `Cache` SHALL show session-xray's blocks of those names: the spend by component and by chain, work by origin and by activity, waits and slow requests, the prompts, the tool table, what the context holds and what cost the most to keep in it, and the cache misses. `Requests` shows the request timeline (see Request timeline). `Tools` adds group cards and the call log to the tool table (see Tools tab). A click on a tab shows it.

#### Scenario: Prompts
- **WHEN** the user clicks `Prompts`
- **THEN** the inspector lists each typed message with its cost, its requests and its two costliest activities, as the X-ray's prompts block does

### Requirement: Keys in the inspector
While the inspector has the keyboard, ← and → (and `h`, `l`) SHALL show the tab before or after outside `Flame`, `Requests` and `Tools`, and Shift+← and Shift+→ in every tab; the digits 1 to 9 show the tab of that place, and 0 `Flame`. In a tab with rows (bars, tables, prompts, misses), ↑ and ↓ (and `k`, `j`) move a cursor by a row, PgUp and PgDn by ten, Home and End to the ends, and the inspector scrolls to show it. In `Summary` they scroll it.

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

#### Scenario: Tools take the arrows
- **WHEN** the inspector has the keyboard on `Tools` and the user presses →
- **THEN** the first group card is chosen, and the tab stays `Tools`

## ADDED Requirements

### Requirement: Tools tab
`Tools` SHALL show, from the top: the group cards, the tool table, and the call log of the main conversation. Every number in it is the one `tokprof scan` and `tokprof scan --calls` print for the session at the same moment (see tool-breakdown). A line over the call log says `calls of the main conversation; the cards and the tool table count the subagents' calls too`.

#### Scenario: The same numbers
- **WHEN** the inspector shows `Tools` for a session
- **THEN** the cards hold the numbers of the groups table of `tokprof scan <id> --calls`, the tool table those of the X-ray's tool rows, and the call log the rows of `tokprof scan <id> --calls`

#### Scenario: No tool calls
- **WHEN** the session made no tool call
- **THEN** the tab says `No tool calls.`, with no cards and no tables

### Requirement: Group cards
The cards SHALL be one per group of the session's groups table, in its order, the never-called groups last. A card is 156 pt wide and holds the group's name, `<n> calls`, `<n> failed` in the theme's red when not zero, `<time> · med <med time>`, and the result tokens as `<n> tok`. Under them, two bars as wide as the group's share of the session's calls, in the requests' blue, and of its time in calls, in the work's green (see Marks in a theme). Cards that do not fit in one row wrap to the next.

#### Scenario: A shell card
- **WHEN** `shell` had 340 of a session's 472 calls, 12 failed, 42 of its 43 minutes in calls with a median of 0.6 s, and 1.9M result tokens
- **THEN** its card reads `shell`, `340 calls`, `12 failed` in red, `42m · med 0.6s` and `1.9M tok`, its calls bar fills 72% of the card's bar width, and its time bar 98%

#### Scenario: No time to count
- **WHEN** none of a group's calls counts for times (see tool-breakdown, Calls that count for times)
- **THEN** its card reads `– · med –` and has no time bar

#### Scenario: Never called
- **WHEN** the session had the MCP server `linear` and never called it
- **THEN** the last card reads `mcp linear` and `never called`, in the theme's secondary text and line colors, without bars

#### Scenario: A narrow inspector
- **WHEN** the inspector is 520 pt wide and the session has 6 groups
- **THEN** the cards fill as many rows as they need, 3 to a row with 8 pt between them

### Requirement: Filter by group
A click on a card SHALL choose its group: the card is ringed in the accent, the tool table shows only its group's rows, and the call log only its group's calls, with `showing <n> of <m> calls` over it. A click on the chosen card, or esc, clears the choice. A click on another card chooses that one. A never-called card cannot be chosen.

#### Scenario: Shell only
- **WHEN** the user clicks the `shell` card of a session whose main conversation made 412 calls, 280 of them in `shell`
- **THEN** the tool table shows only the `shell` rows, and the call log 280 rows under `showing 280 of 412 calls`

#### Scenario: Clearing
- **WHEN** a card is chosen and the user clicks it again
- **THEN** no card is chosen, and both tables show all their rows

#### Scenario: A never-called card
- **WHEN** the user clicks the `mcp linear` card that says `never called`
- **THEN** nothing is chosen, and the tables do not change

### Requirement: Call log table
The call log SHALL be a table with the columns of `tokprof scan --calls`: `#`, `started`, `req`, `group`, `tool`, `input`, `status`, `took` and `tokens`. `failed` is drawn in the theme's red, `running` in the work's green, `asked you` in the questions' magenta, and `refused`, `denied` and `background` in the secondary text color. It holds up to 300 rows: the page that holds the cursor. A note says how many earlier or later calls are left out.

#### Scenario: A long session
- **WHEN** a session's main conversation made 1,250 calls and the cursor is on the 10th
- **THEN** the table shows calls 1 to 300 and a note `and 950 later calls`

#### Scenario: A failed call
- **WHEN** call 17 failed
- **THEN** its status reads `failed` in red, and its `took` and `tokens` cells are those of the terminal

### Requirement: From a call to its request
A double click on a call row, or ⏎ while the cursor is on one, SHALL show `Requests` with the call's request selected, as a click on its column does (see Selected request). When `Tools` is shown again, its cursor and its chosen card are as they were.

#### Scenario: Opening the request
- **WHEN** the cursor is on call 17, made by request 5, and the user presses ⏎
- **THEN** the inspector shows `Requests`, request 5 is selected in the chart, the card and the table

#### Scenario: Coming back
- **WHEN** the user then presses 7
- **THEN** `Tools` shows with the cursor on call 17 and the same card chosen

#### Scenario: A row of the tool table
- **WHEN** the cursor is on a row of the tool table and the user presses ⏎
- **THEN** nothing happens

### Requirement: Keys in Tools
While the inspector has the keyboard on `Tools`, ← and → (and `h`, `l`) SHALL choose the card before or after. ↑ and ↓ (and `k`, `j`), PgUp, PgDn, Home and End move the cursor through the rows of the tool table and then those of the call log, as one list. ⏎ opens the request of the call under the cursor. esc clears the chosen card before it gives the keyboard back. Shift+← and Shift+→ show the tab before or after, as in every tab.

#### Scenario: No card chosen
- **WHEN** no card is chosen and the user presses ←
- **THEN** the last card that can be chosen is chosen; → would have chosen the first

#### Scenario: Stepping cards
- **WHEN** the `shell` card is chosen and the user presses →
- **THEN** the next card is chosen, and the tables show its group

#### Scenario: From one table to the other
- **WHEN** the cursor is on the last row of the tool table and the user presses ↓
- **THEN** it is on the first row of the call log, and the call log is on the screen

#### Scenario: Esc
- **WHEN** a card is chosen and the user presses esc
- **THEN** no card is chosen and the inspector keeps the keyboard; a second esc gives it back

#### Scenario: The key help
- **WHEN** the key help is open
- **THEN** its `Inspector` group names ← → for the cards of `Tools`, ⏎ for a call's request, and esc for clearing the card

### Requirement: Tools in a frame without a window
`--scene tools-group` SHALL draw the frame on `Tools` with the inspector's keyboard, the first card chosen and the cursor on the first row of the call log. It uses `--session` to pick a session, as the list's filter does.

#### Scenario: A session's tools
- **WHEN** the user runs `tokprof-desk --snap out.png --session 5e5510f0 --scene tools-group`
- **THEN** `out.png` shows that session's cards with the first one ringed, and both tables filtered to its group
