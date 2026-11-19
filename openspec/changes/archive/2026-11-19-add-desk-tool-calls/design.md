# Design

## Context

See proposal.md for why. This change starts where add-tool-call-log ends:
- The X-ray holds the session's tool table with groups and durations, the call log of the main conversation, and the connected servers.
- tokprof-browse renders the groups table and the call log as text and as cells. The tool cells have the `time` and `med time` columns, and the window's Tools tab already shows them.
- The window's Tools tab is one boxed table built from the tool cells (inspector.rs:1020-1046). The inspector has a row cursor for tabs with rows, and `Requests` and `Flame` take the arrows before the inspector does (desk-window, Keys in the inspector).
- `Requests` keeps its state in one struct: the session, the selected request and the range (requests.rs:38-66). A selection of another session is ignored, so the tab falls back to the costliest request.

No log field and no source is touched. The data and its log fields are in add-tool-call-log's design.

## Goals / Non-Goals

**Goals:**
- The same numbers as `tokprof scan --calls`, from the same cells.
- Keys and mouse that work as in the other lists of the window.
- tokprof's own look: the window's panels, lines, radius and theme colors.

**Non-Goals:**
- The terminal browser. See proposal.md, Out of scope.
- New numbers, or subagents' calls as rows.

## Decisions

1. **The shared part lives in tokprof-browse.**
   - Card cells: for each group, its name, calls, failed, time and median time, and result tokens as text. They also hold the share of the session's calls and of its counted time, and whether the group was never called.
   - A filter by group for the tool cells and the call rows. The terminal can use the same filter in `tokprof scan --calls --group`.
   - The window only lays out and paints these cells. A unit test in tokprof-browse holds the card cells to the groups table's cells.
2. **The tab's state is one struct, like the one of `Requests`.** It holds the session, the chosen group, and the cursor as an index into the joined list of tool rows and call rows. A state of another session is ignored, so a new session opens with no card chosen and the cursor on the first row. The joined list makes ↑ ↓ cross from one table to the other without a second cursor.
3. **Cards are rows of fixed-width panels.** The number per row is the inspector's inner width divided by 156 pt plus the 8 pt gap. The rows are built from that number, so the layout does not depend on wrapping in the layout engine.
   - A card is the window's panel, with its line and corner radius, as the Spend card has.
   - The bars are 4 pt high and drawn as fills, as the Spend card's parts are. On a dark theme they keep 60% of their color, as Marks in a theme says for large filled marks. The bars are thin, but they follow the same rule, so that all of the window's fills look alike.
   - A chosen card gets the accent ring the window uses for the selection with the keyboard.
   - A never-called card uses the theme's secondary text and line colors, and has no bars.
4. **The filter is applied to the cells, not to the drawn rows.** The page of 300 rows and the note of left-out calls count the filtered rows. That is how `showing <n> of <m> calls` stays true.
5. **The step to `Requests` sets `Requests`' state.** Each call row carries its request's number from add-tool-call-log, so the step sets the selected request of `Requests` to it and shows the tab. The Requests table already pages to the selected request (desk-window, Rows of the requests table). The Tools state stays as it was, so coming back restores the cursor and the card.
   - Rejected: a single click that opens the request. In every other table of the window a click selects a row (desk-window, Mouse, and Rows of the requests table), and texts can be selected with the mouse. A double click or ⏎ is what opens a lane full size too.
6. **Keys.** `Tools` takes ← →, `h` `l`, ⏎ and esc before the inspector's own keys, as `Requests` and `Flame` do. Shift+← → still change the tab. The key help adds three lines to its `Inspector` group.
7. **Scene.** `tools-group` selects the session as the other inspector scenes do (main.rs:1028-1030), shows `Tools`, gives the inspector the keyboard, chooses the first card and puts the cursor on the first call row.

## Cost

- No parse and no change of the X-ray pass. The tab builds its cells from the X-ray when it is shown.
- A session with 3,000 calls builds 3,000 call rows of cells and draws 300. The cards are at most a dozen. The goal is a first frame in under 100 ms after the X-ray is ready.

## Risks / Trade-offs

- [Many MCP servers make many cards] → The cards wrap. A session with 12 groups takes 3 rows in a wide inspector. The tables move down, and the cursor keeps the row under it on the screen.
- [A median hides the long calls] → The card shows the group's time in calls next to the median, and the time bar shows its share.
- [The call log and the tool table count different calls] → The line over the call log says so, and the cards follow the tool table.

## Settled Questions

- No rent on the cards. The author asked for calls, failed, time and tokens. A card is 156 pt wide, and the tool table under the cards has rent per tool already.
- Choosing a card does not mark the `Requests` timeline. Marks of calls on it need a kind of mark of their own and a legend, and the step from a call to its request already links the two tabs. It can follow with a timeline of calls.
