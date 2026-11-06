# Spec Delta

## ADDED Requirements

### Requirement: Command palette
⌘K and `Command Palette…` in the app menu SHALL open the command palette: a rounded card 640 points wide near the top of the window, over the window dimmed and blurred as under the settings panel. It holds a query field with what is typed and a caret, and under it the entries that match, grouped under small headings, ten rows high at most and scrolling. ⌘K again, esc and a click outside the card close it, and the window is as it was.

#### Scenario: Opening the palette
- **WHEN** the user presses ⌘K
- **THEN** the palette opens with an empty query, its first entry highlighted

#### Scenario: A filter being typed
- **WHEN** the user presses ⌘K while a filter is being typed
- **THEN** the filter is kept as ⏎ keeps it, and the palette opens

#### Scenario: Closing with a click outside
- **WHEN** the palette is open and the user clicks a list row outside the card
- **THEN** the palette closes and the row is not selected

### Requirement: Palette entries
The palette SHALL offer four groups. `Actions`: what the window does from a key, each with its key. `Tabs`: the inspector's tabs. `Sessions`: every listed session with its title, project and cost. `Days`: `Today`, the day of the latest action, and the days of the listed sessions, newest first, named as the list's separators. Picking a session selects it, scrolled to, with its `Summary`; a tab shows it; a day shows it in the lanes.

#### Scenario: Picking a session
- **WHEN** the user picks a session in the palette while another is selected
- **THEN** the palette closes, the session is selected in the list and on the screen, and the inspector shows its `Summary` from the top

#### Scenario: Picking a session from the full-size inspector
- **WHEN** the inspector is full size and the user picks a session
- **THEN** the list is drawn again with that session selected

#### Scenario: Picking a day
- **WHEN** the user picks `Yesterday`
- **THEN** the lanes show the day before today, and the selection stays

#### Scenario: Picking an action
- **WHEN** the user picks `Sort by Cost`
- **THEN** the list is sorted by cost, as the `Cost` button sorts it

#### Scenario: Actions that do nothing now
- **WHEN** no filter is set
- **THEN** `Clear the filter` is not offered; and `Full-size inspector` is offered only while the inspector is not full size, `Back to the list` only while it is

### Requirement: Palette matching
An entry SHALL match when every typed letter, ignoring case and spaces, is in its name in order; a session also matches by its project, a day by its date, a day only in runs of letters that each start a word, the words in a row. Matches rank by tightness: a letter at the start, at a word's start or next to the one before ranks higher. Groups go in the order of their best match, entries by rank; loose matches are left out while tighter ones exist. Matched letters are in the accent.

#### Scenario: A prefix first
- **WHEN** the user types `to`
- **THEN** `Tools`, `Today` and the sessions whose title starts with `To` rank above `Keyboard to the list`

#### Scenario: Loose matches
- **WHEN** the user types `to` and `Tools` matches
- **THEN** `Copy the handoff line`, where `t` and `o` lie far apart, is not shown

#### Scenario: A short group first
- **WHEN** the best tab and the best session match `to` equally well
- **THEN** `Tabs` is shown above `Sessions`

#### Scenario: Word starts
- **WHEN** the user types `nd`
- **THEN** `Next day` ranks above entries where `n` and `d` are inside a word

#### Scenario: Days by chance
- **WHEN** the user types `to` and the list has sessions of Thursday 8 October
- **THEN** `Today` is shown and `Thu 8 Oct 2026`, whose `T` and `O` start words two apart, is not, while `8oct` shows it

#### Scenario: At most
- **WHEN** a query matches more than 100 sessions or 30 days
- **THEN** the palette shows the 100 and the 30 that rank highest

#### Scenario: Nothing typed
- **WHEN** the query is empty
- **THEN** the palette shows `Actions`, then `Tabs`, then the 20 most recent sessions, then `Today` and the 14 newest days

#### Scenario: No match
- **WHEN** nothing matches the query
- **THEN** the palette says `Nothing matches` with the query, and ⏎ does nothing

### Requirement: Keys in the palette
While the palette is open, typed characters SHALL go to its query and Backspace takes the last one away; each change highlights the first entry. ↑ and ↓, and Ctrl+P and Ctrl+N, move the highlight, scrolled into view; ⏎ runs the highlighted entry and closes the palette; a click on a row runs it. No key reaches the window behind it, but ⌘Q quits. The palette's foot names these keys.

#### Scenario: Keys stay in the palette
- **WHEN** the palette is open and the user types `q` and `j`
- **THEN** the window stays open, the selection does not move, and the query is `qj`

#### Scenario: Moving the highlight
- **WHEN** the user presses ↓ twice and ⏎
- **THEN** the third entry runs

#### Scenario: At the ends
- **WHEN** the highlight is on the last entry and the user presses ↓
- **THEN** it stays on the last entry

### Requirement: Key help
`?` SHALL open the key help when no filter is being typed and no overlay is open: a rounded card over the window dimmed and blurred, listing every key of the window grouped under `Anywhere`, `List`, `Lanes`, `Inspector`, `Settings` and `Palette`, each key drawn as a keycap with what it does. esc or `?` close it, and no other key reaches the window behind it, but ⌘Q quits. The palette's `Key help` opens it too.

#### Scenario: Opening the key help
- **WHEN** the list has the keyboard and the user presses `?`
- **THEN** the key help opens, and it names ⌘K, ⌘,, `/`, `s`, `r`, `f`, `o`, `c`, `d` and `q`

#### Scenario: A filter being typed
- **WHEN** a filter is being typed and the user presses `?`
- **THEN** `?` is added to the filter

#### Scenario: Closing the key help
- **WHEN** the key help is open and the user presses `?`
- **THEN** it closes, and the same session, tab and day are shown

### Requirement: One overlay at a time
The settings panel, the command palette and the key help SHALL never show together. While one is open, the keys and menu items that open another do nothing; an entry of the palette that opens the settings or the key help closes the palette first.

#### Scenario: Settings over the palette
- **WHEN** the palette is open and the user presses ⌘,
- **THEN** the palette stays open and the settings do not open

#### Scenario: The palette over the settings
- **WHEN** the settings panel is open and the user presses ⌘K
- **THEN** the panel stays open and the palette does not open

### Requirement: Overlays in a frame without a window
`--scene palette` SHALL draw the frame with the palette open and `to` typed into its query, and `--scene keys` with the key help open.

#### Scenario: The palette in a frame
- **WHEN** the user runs `tokprof-desk --snap out.png --scene palette`
- **THEN** `out.png` shows the palette over the window with the entries that match `to`
