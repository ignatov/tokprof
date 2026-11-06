# Design

## Context

The window's keys go through one `on_key`: ⌘Q, ⌘, and the open settings panel first, then ⌘W, then Tab and esc between the panes, then the keys of the pane with the keyboard (`lanes_key`, `inspector_key`), and the rest to the browser's `on_key`, whose keys are those of `tokprof ui` with the window's mode on top (`beside_key`). The settings panel (`settings.rs`) is an `Option<Panel>` in `Desk` that, while set, takes every key but ⌘Q and ⌘,; it is drawn as a scrim over the whole window (the background at 35%, backdrop blur σ 10) whose press closes it, holding a card that keeps its own presses. Rune's `El::modal` is not used because it takes Tab itself while nothing inside is focusable.

Rune at rev `781e2b6d` has what the palette needs: `rich(text, spans)` colors byte ranges of a text, so matched letters can be drawn in the accent inside one text that still truncates; `tx.scroll_into_view(key)` scrolls the nearest scroll view to a row; `App::menu_item` with a key equivalent. The window draws its own text fields, as the filter in the bar does: the text and a thin caret after it.

This change reads no log fields and does not touch parsing; the parse time of the log corpus does not change. Building the palette's entries walks the listed sessions each time the view runs while it is open (on a key, and every second while a countdown shows): on the author's 838 listed sessions it took 4 to 7 ms in a release build, measured with `--scene palette`. It costs nothing while the palette is closed.

## Goals / Non-Goals

**Goals:**
- Every session, tab, day and action of the window is a few letters and ⏎ away.
- The key help is the truth about the keys: it is written next to the code that handles them and tested against it.
- The palette and the key help look like the settings panel the author approved.

**Non-Goals:**
- Searching inside sessions, ranking by past picks, user key bindings.

## Decisions

### State and keys

`Desk` gains `palette: Option<palette::Palette>` (the query and the highlighted place) and `help: bool`. `on_key` checks, after ⌘Q: ⌘K (toggles the palette unless the settings or the key help are open), ⌘, (toggles the settings unless the palette or the key help are open), then the open overlay takes every key. `?` opens the key help where no filter is being typed, before the panes take their keys. The menu items call the same toggles, so a chord that reaches both the menu and `on_key` opens the overlay once.

Opening the palette ends a filter being typed as ⏎ does, so the actions that send a browser key are not typed into the filter.

### Entries

`palette::entries(desk, query)` builds the list the view draws and the keys walk, so both agree on what ⏎ runs. An entry has its group, what it runs (`Pick`: a session id, a tab, a day or an action), its title and detail, its key caps, its score and the places of the matched letters in the title or the detail.

- Sessions: each listed row, named by its title or, without one, its project; the detail is the project, and the cost is on the right with the agent's dot. A row with a title matches by title, else by project at a rank 2 lower.
- Days: the local dates of the listed sessions' last activity, as the list's separators group them, named by `day_label`, with the count of sessions and their known cost; a day named Yesterday or by its weekday carries its date (`5 November`) as its detail, which matches too, 2 lower. `Today` runs what the bar's Today button runs, so the Actions group has no `Today` of its own; another date is shown with the browser's `show_day` and then stepped by 0 days, which turns the latest date into the browser's "latest day", so › and Today are disabled there as after ←.
- Actions run the code their key runs: the browser's keys `c`, `f`, `o`, `/`, `r`, `d`, ⏎ and esc through the same path as a key, so the list scrolls and the inspector resets as after the key; the sorts, `Fit`, `24 h`, the day steps and `Today` call what their buttons call; the panes set the pane; `Close the window` and `Quit` ask Rune.

With an empty query: all actions, all tabs, the 20 most recent sessions by last activity, `Today` and the 14 newest days. With a query: the matches, at most 100 sessions and 30 days. A match scoring 0 or less is loose (its letters far apart or deep in the name) and is left out while any match scores above 0. Groups are ordered by their best score; among groups as good, actions and tabs go first, then the group with the better second score, so that thirty days that match loosely do not bury the sessions; then the order Actions, Tabs, Sessions, Days. Entries go by score, then the shorter name, then the more recent, then their order.

### Matching

`palette::fuzzy(query, text) -> Option<(score, places)>`. Spaces in the query are dropped, and both sides are lowercased char by char. A dynamic program over (query letter, text place) finds the best placement of the letters in order: each letter scores 1, plus 10 at the text's start, 8 at a word's start (after a char that is not a letter or digit, or an upper case letter after a lower case one), plus 8 right after the letter before; a gap between two letters costs 3, and 1 more per skipped char, and the first letter costs 1 per char before it, at most 10. A running maximum keeps it at O(query × text). The places come from walking the choices back. These constants are tuned by the unit tests' cases: a prefix beats a word start, a word start beats a run inside a word, a run beats scattered letters, a run at the start beats two word starts.

### Look

`overlay.rs` holds what the overlays share: the scrim (the background at 35%, blur σ 10, a press closes), the card (fill of the raised surface, a 1-point line, a soft shadow, the corners clipped), pills, and `keycap` and `hint`, moved there from the settings panel, which now uses them.

The palette's card: 640 points wide, radius 20, 72 points from the top, centred. Its head is the query field: 52 points high, the typed text at 16 points with a 1.5-point caret after it, or `Search sessions, tabs, days and actions` in the tertiary text while empty; a `⌘K` keycap on the right. Under a line, the list scrolls, at most 10 rows of 34 points; each group is a small heading in the tertiary text, and each row a pill-rounded box: the title with its matched letters in the accent and bold, the detail in the secondary text, and on the right the cost (sessions), the count and cost (days) or the keycaps (actions, tabs 1–9). The highlighted row is filled with the dimmed accent. The foot names ↑ ↓ `move`, ⏎ `open`, esc `close`, and the count of matches.

The key help's card: 860 points wide, at most the window's height less 80, radius 20, centred, scrolling. Its head says `Keys` with a pill `? or esc to close`; its body is two columns of soft groups (radius 16, the bar's color), each a heading and rows of keycaps on the left (130 points) and what they do. `keys.rs` holds the groups as data, which the view draws and the tests read.

### Scenes

`palette` opens the palette with `to` typed; `keys` opens the key help.

## Risks / Trade-offs

- [The key help drifts from the code] → A test sends every printable character to the browser and fails when one that changes something, or quits, is not named in the key help (`c`, `f` and `o` copy and write files, so they are only checked by name); another checks that the window's own keys are named.
- [Thousands of sessions in the list] → Only the capped entries are drawn; the walk over the sessions is a match per row, a few milliseconds per view run for the author's 838 sessions.
- [⌘K reaches the menu item before `on_key`] → Both call the same toggle, so the palette opens or closes once.
- [A Flame tab is being added in parallel] → The tabs come from the inspector's list of tabs, so a new tab joins the palette with no change here; tabs past the ninth have no digit key.
