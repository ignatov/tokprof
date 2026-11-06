# Proposal

## Why

The window has grown: day lanes, a list of sessions, an inspector with nine tabs, a settings panel, and many keys. The author wants to drive it from the keyboard, but the keys are spread over three panes and a README table, and there is no way to reach a session, a tab or a day by its name.

## What Changes

- **A command palette in `tokprof-desk`**: ⌘K, or `Command Palette…` in the app menu, opens a card near the top of the window, over the window dimmed and blurred as under the settings. It has a query field and, under it, what matches, grouped under `Actions`, `Tabs`, `Sessions` and `Days`.
  - `Sessions`: every listed session, with its title, project and cost; a pick selects it in the list, scrolls to it and shows its `Summary`.
  - `Tabs`: the inspector's tabs; a pick shows the tab.
  - `Days`: `Today` and the days that have listed sessions, newest first, named as the list names them; a pick shows that day in the lanes.
  - `Actions`: what the window does from a key today: the settings, the key help, the handoff line, the JFR and HTML profiles, the filter, the sorts, `Fit` and `24 h`, the day before and after, the selected session's day, the full-size inspector, the panes, closing and quitting. Each row names its key.
- **Matching**: case-insensitive, all typed letters in order. Tight matches rank first: a match at the start, at word starts and in a run. The matched letters are drawn in the accent. With nothing typed, the palette shows the actions and tabs, then the most recent sessions, then the days.
- **Keys in the palette**: ↑ ↓ and Ctrl+P, Ctrl+N move the highlight, ⏎ runs it and closes the palette, esc or ⌘K closes it, Backspace edits the query; a click on a row runs it, a click outside closes the palette. No key reaches the window behind it, but ⌘Q quits.
- **Key help**: `?` opens a card that lists every key of the window, grouped by where it works (`Anywhere`, `List`, `Lanes`, `Inspector`, `Settings`, `Palette`), each key drawn as a keycap like the settings panel's foot. esc or `?` closes it.
- **One overlay at a time**: the settings panel, the palette and the key help never show together.
- **Frames**: `--scene palette` draws the palette open with a query typed, and `--scene keys` the key help.

## Capabilities

### New Capabilities
None.

### Modified Capabilities
- `desk-window`: the window gains the command palette, the key help and their scenes.

## Impact

- `crates/tokprof-desk`: a new module for the palette and its matcher, a new module for the key help, a shared module for what the overlays draw alike (the scrim, keycaps), small hooks in the keys, the view and the app menu.
- No change to `tokprof`, `tokprof ui`, the browser package or any log source. No new dependency.
- `crates/tokprof-desk/README.md`: the palette and the key help.

## How it is checked on the real logs

- With the installed window on the author's logs: ⌘K, type `tok`, and the sessions of the tokprof project are listed, with `Tools` and `Today` among the matches and the letters `t`, `o`, `k` in the accent. ⏎ on a session selects it in the list, scrolled to, with its `Summary`; its cost in the palette is the cost of its row in the list.
- Type `cache` and ⏎: the inspector shows `Cache`. Type `yest` and ⏎: the lanes show yesterday, as ← from today does.
- The `Days` group has one entry per day separator in the list in time order, with the same names.
- `?` opens the key help; every key in it does what it says when tried in the window.
- `--scene palette` and `--scene keys` render in Merino Light and Merino Dark; the frames are looked at.

## Out of scope

- Searching the text of prompts, requests or tools inside sessions.
- Remembering the picks made in the palette to rank them higher next time.
- Changing a key or adding keys of one's own.
- A palette and a key help in `tokprof ui` in the terminal.
