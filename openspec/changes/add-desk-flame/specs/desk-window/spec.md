# desk-window

## ADDED Requirements

### Requirement: Flame tab
`Flame` SHALL draw the selected session's spend as an icicle graph, the session on top and each frame's children below it, from the stacks and dollars of `tokprof flame --session <id>`. Each frame is a rounded bar as wide as its share of the shown root's dollars, the largest child first, with its name inside when it fits and its dollars when there is room. Frames narrower than 3 pt are not drawn.

#### Scenario: The same numbers as the profile
- **WHEN** the inspector shows `Flame` for a session
- **THEN** the root's dollars are the sum of the values of `tokprof flame --session <id> -o out.collapsed`, in µ$, and each frame's dollars are the sum of the stacks through it

#### Scenario: Widths
- **WHEN** a chain cost $3 of a $4 session
- **THEN** its bar is three quarters as wide as the root's, less the gap between bars

#### Scenario: Narrow frames
- **WHEN** 12 frames of the shown root are narrower than 3 pt
- **THEN** they are not drawn, and the line below the graph says `12 frames under 3 pt hidden`

#### Scenario: Shared head
- **WHEN** every stack starts with the same agent, project and session frames
- **THEN** they make one root frame, named by the three, instead of three bars of full width

#### Scenario: No prices
- **WHEN** no request of the session has a price
- **THEN** the frames are sized and labelled by tokens

### Requirement: Flame colors
The frames of `Flame` SHALL take their colors from the theme by what they stand for: grey for structure (the session, chains, turns), green for content (kinds, tools, targets), and for the components the colors of the `Spend` tab: blue for read, yellow for write, red for a rewrite after a miss, cyan for output, magenta for uncached input. The colors are soft tints, with the text in the theme's ink.

#### Scenario: A tool frame
- **WHEN** a frame names a tool
- **THEN** it is drawn in the green of content

### Requirement: Flame hover
The frame under the pointer SHALL lighten, and the line under the graph names its path under the shown root (the breadcrumb above the graph has the rest), its dollars, its share of the session and its share of its parent. Without a frame under the pointer, the line names the frame with the keyboard cursor.

#### Scenario: Hover
- **WHEN** the pointer rests on a turn frame that cost $1.20 of a $12.00 session and $3.00 of its chain
- **THEN** the line says its path under the shown root, `$1.20`, `10.0% of the session` and `40.0% of main`, with the chain's name

### Requirement: Flame zoom
A click on a frame SHALL show it as the root of the graph, full width, with the cursor on it. A breadcrumb above the graph shows the path from the session to the shown root; a click on a crumb shows that frame as the root. A click on the shown root zooms out one level. The zoom stays while the same session is shown, as long as its path still exists, and the bars glide to their new places.

#### Scenario: Zoom in and out
- **WHEN** the user clicks the chain `main` and then the first crumb
- **THEN** `main` fills the width with its turns below it, and then the whole session is shown again

#### Scenario: Another session
- **WHEN** the graph is zoomed and the user selects another session
- **THEN** `Flame` shows the new session whole

### Requirement: Keys in the flame graph
While the inspector has the keyboard on `Flame`, ← and → (and `h`, `l`) SHALL move a cursor to the drawn sibling before or after, ↓ (`j`) to the first drawn child, ↑ (`k`) to the parent, up to the shown root. ⏎ zooms into the cursor's frame, and esc and Backspace zoom out one level; esc gives the keyboard back to the list only when the graph is not zoomed. The cursor's frame is ringed in the accent.

#### Scenario: Down and right
- **WHEN** the cursor is on the root and the user presses ↓ and then →
- **THEN** the cursor is on the second child of the root, ringed, and the line under the graph names it

#### Scenario: Enter and esc
- **WHEN** the cursor is on a turn and the user presses ⏎ and then esc
- **THEN** the turn is shown as the root, and then its parent is

## MODIFIED Requirements

### Requirement: Keys in the inspector
While the inspector has the keyboard, ← and → (and `h`, `l`) SHALL show the tab before or after outside `Flame`, and Shift+← and Shift+→ in every tab; the digits 1 to 9 show the tab of that place, and 0 `Flame`. In a tab with rows (requests, the bars of spend, work and waits, prompts, tools, the context's tables, misses), ↑ and ↓ (and `k`, `j`) move a cursor by a row, PgUp and PgDn by ten, Home and End to the ends, and the inspector scrolls to show it. In `Summary` they scroll it.

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

### Requirement: Inspector
The inspector SHALL show the selected session's X-ray as native controls under tabs: `Summary`, `Requests`, `Spend`, `Work`, `Waits`, `Prompts`, `Tools`, `Context`, `Cache` and `Flame`. Every number in it is the one `tokprof scan` prints for the session at the same moment, and in `Flame` the one `tokprof flame --session <id>` writes. Texts can be selected with the mouse, and ⌘C copies the selection.

#### Scenario: The same numbers
- **WHEN** the inspector shows a session
- **THEN** its cost, context size, next-request cost, spend by component, prompts and tool rows are those of `tokprof scan <id>` at the same moment

#### Scenario: A session without requests
- **WHEN** the selected session has no requests
- **THEN** the inspector says `no requests`, with its title, project and id

### Requirement: Other tabs
The tabs from `Requests` to `Cache` SHALL show session-xray's blocks of those names: the requests, the spend by component and by chain, work by origin and by activity, waits and slow requests, the prompts, the tool table, what the context holds and what cost the most to keep in it, and the cache misses. A click on a tab shows it.

#### Scenario: Prompts
- **WHEN** the user clicks `Prompts`
- **THEN** the inspector lists each typed message with its cost, its requests and its two costliest activities, as the X-ray's prompts block does
