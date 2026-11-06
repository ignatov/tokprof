## MODIFIED Requirements

### Requirement: Session list
A row of the list SHALL show the last activity (`HH:MM`, after a label naming the day), a dot in the agent's color, the session's title or, while the title is not known, its project, and its cost with tabular figures (`…` while it is being costed). The selected row is a rounded row inset in the pane, in the theme's selection color with the text in the ink. The day labels show only in time order. A filter that lists no session says so in the list.

#### Scenario: A row
- **WHEN** the list shows a session titled `Find why the order export is slow` that cost $1.18
- **THEN** its row reads its time, the dot, the title and `$1.18`

#### Scenario: The selected row
- **WHEN** a session is selected
- **THEN** its row is rounded and inset 6 points from the pane's edges, in the selection color, its title, cost and dot in the colors of any other row

#### Scenario: A filter that lists nothing
- **WHEN** the user types the filter `zzzz` and no session matches
- **THEN** the list reads `No session matches “zzzz”` and, under it, `esc` as a keycap with `clears the filter`

### Requirement: Keyboard focus
One of the lanes, the list and the inspector SHALL have the keyboard at a time, the list to start with. The pane that has it rings its selection in the accent and draws its title in the accent; the other panes keep their selection without the ring. Tab moves the keyboard to the next of them in the order lanes, list, inspector, and Shift+Tab back; a click in one gives it the keyboard, and esc gives it back to the list. The keys of the list are those of Keys of the window; the others' follow.

#### Scenario: Tab
- **WHEN** the list has the keyboard and the user presses Tab
- **THEN** the inspector has it, and ↓ moves in the inspector instead of the list

#### Scenario: Esc
- **WHEN** the inspector has the keyboard and the user presses esc
- **THEN** the list has it again, and the window stays open

#### Scenario: What the ring marks
- **WHEN** the lanes, the list or the inspector has the keyboard
- **THEN** the ring is around the selected lane, the selected row, or the row under the inspector's cursor (the chosen tab on `Summary`; on `Flame`, the frame under its cursor), and nowhere else

#### Scenario: A filter being typed
- **WHEN** the user is typing a filter
- **THEN** the filter field has the ring, and the list's row does not

### Requirement: Marks in a theme
A theme SHALL give the window's background, panels, lines, text and accent, and the colors of its marks: green for work and warm caches, red for cold caches, the now line and stops, yellow for waits and cache writes, blue for cache reads and requests, cyan for output and messages, magenta for questions. On a dark theme, the large filled marks keep about 78% of their color over the background; text keeps the theme's colors.

#### Scenario: A cold cache
- **WHEN** a listed session's cache is cold
- **THEN** its next-request cell is drawn in the theme's red

#### Scenario: Work on a dark theme
- **WHEN** the window is drawn in a dark theme
- **THEN** the bars of work, the request bars and the parts of the spend are their colors mixed 58% into the background in linear light, and a warm countdown is the theme's green itself
