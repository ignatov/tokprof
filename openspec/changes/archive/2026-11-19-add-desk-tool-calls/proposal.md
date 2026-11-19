# Proposal

## Why

The author wants to see, in the window, which tools the agent called, grouped by where they come from, and how long they took. add-tool-call-log gives the data and the terminal view: a call's time, groups, a groups table and `tokprof scan --calls`. The window's `Tools` tab still shows only the table of sums per tool. To see the calls of a session, the author has to leave the window for the terminal.

This change depends on add-tool-call-log, which must be applied first.

## What Changes

- **Group cards on top of `Tools`.** One card per group of the session, its subagents included: the group's name, calls, failed calls, its time in calls and their median, and result tokens. Two thin bars give the group's share of the session's calls and of its time in calls. A server that was connected but never called has a dimmed card that says `never called`.
- **The tool table stays under the cards**, with the `time` and `med time` columns of add-tool-call-log.
- **The call log under the table**: every call of the main conversation as a row, with the columns of `tokprof scan --calls`, up to 300 rows a page.
- **A click on a card filters both tables to its group.** A second click on it, or esc, clears the filter.
- **From a call to its request.** A double click or ⏎ on a call row shows `Requests` with the request whose response made the call.
- **Keys.** ← → choose a card, ↑ ↓ move through the rows of both tables, ⏎ opens the request, esc clears the filter. The key help names them.
- **A frame without a window**: `--scene tools-group` draws `Tools` with the first card chosen.
- The look is tokprof's own: the window's panels, lines and theme colors (see desk-window, Marks in a theme). It copies no other product.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `desk-window`: `Other tabs` and `Keys in the inspector` change for `Tools`. New requirements for the Tools tab's layout, the group cards, the filter, the call log table, the step from a call to its request, the keys of Tools, and its scene.

## How it is checked on the real logs

- For three sessions with calls in several groups, one of them a Codex session, the cards give the numbers of the groups table of `tokprof scan <id> --calls`, and the call log's rows those of its rows.
- For a Claude Code session with an MCP server it never called, the dimmed card names the server, as the groups table's `never called` row does.
- For a session with at least 3,000 calls, `Tools` builds and draws its first frame in under 100 ms after the X-ray is ready. It is timed in a desk test on a synthetic X-ray of 3,000 calls, and on the real session by comparing `--snap --tab tools` with `--snap --tab cache` on it.
- Frames of `--tab tools` and `--scene tools-group`, light and dark, are looked at and fixed where they look wrong.

## Out of scope

- The terminal browser (`tokprof ui`). Its full-screen X-ray already gets the `time` and `med time` columns and the groups line from add-tool-call-log. A call list in it needs its own keys and scrolling. The cells it would use are in tokprof-browse, so it can follow later.
- Subagents' own calls as rows. They count in the cards and the tool table, as in the terminal.
- A timeline of calls on the request timeline of `Requests`, and marks of a chosen card's calls on it.
- A group's rent on its card.
- Any new number. Every number in the tab comes from add-tool-call-log.

## Impact

- tokprof-browse: the cells of a group card (with the shares of calls and time) and a filter by group for the tool cells and the call rows.
- tokprof-desk: the Tools tab's state, cards, filter, call log table, the step to `Requests`, the keys, the key help and the scene.
- README.md: the window's `Tools` tab.
