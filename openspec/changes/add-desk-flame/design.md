# Design

## Context

`tokprof flame` builds its stacks in the profile sink of the core's collector: every share of spend (a fact) becomes a stack of frames by the dimensions of `--stack`, by default `agent,project,session,chain,turn,kind,tool,target,component`, and its tokens, dollars and avoidable dollars are added to that stack. Each frame has a name, a tint (structure, content, read, write, uncached, output, miss) and the dimension it comes from. The `.collapsed` writer prints one line per stack with its value in µ$, rounded per stack, and leaves out stacks of 0. `tokprof flame --session <id>` reads the session's log and its subagents' logs.

The window's inspector draws the X-ray that the browser package computes once per change of the session, and wraps the inspector in a `memo` whose version lists what it shows. The X-ray is built in one pass of the analysis over the same logs, with its own sink of totals; it does not keep the stacks.

This change reads no new log fields. The parse of the log corpus does not change: the stacks are built for one session's X-ray only, never in `tokprof scan --all`-like passes or in the costing of the list.

## Goals / Non-Goals

**Goals:**
- The tab's numbers are those of `tokprof flame --session <id>`, without running it.
- The tree is built once per X-ray; a hover, a key or a zoom only lays it out again.
- Calm, like the settings panel: soft group behind the graph, rounded bars, colors from the theme.

**Non-Goals:**
- Other stacks or measures, a search, a diff between sessions, and the tree in `tokprof ui`.
- Request counts per frame (see Decisions).

## Decisions

### The stacks in the X-ray

The X-ray's sink of totals gains the profile sink with the default stack, and the X-ray keeps its stacks. The X-ray reads the main log and its subagents' logs as `tokprof flame --session` does, in the same order, so the stacks are the same. A subagent log that cannot be read fails the X-ray, as it does today, where `flame` leaves it out.

Cost: one stack key per fact, about nine short strings. On the newest session of the author's logs the X-ray's time is measured before and after (tasks).

*Alternative.* A second pass in the window over the logs for the stacks (as the browser's profile export does): double the reading per selection, and a second worker to wait for.

### The frame tree

Built in the window from the X-ray's stacks, once per X-ray, and kept beside the desk with the X-ray it came from. Values are `Measure::Cost` of each stack (µ$ rounded per stack, as the `.collapsed` writer rounds), or tokens when the session has no price; stacks of 0 are left out, as there. Children merge by name and tint, as async-profiler's HTML writer merges them, and are ordered by value, largest first, then by name.

The leading frames that every stack shares (the agent, the project and the session, one child each) fold into the root, which is named by them: three bars of full width say nothing. The path shown on hover still names them.

### Layout

The graph is a box in a soft group (radius 12, the bar color of the theme) whose width comes from its last layout. Each depth is a row 22 pt high with 2 pt between rows. A frame's x and width are its offset and value scaled to the width plus one gap, less the gap, so neighbours stand 2 pt apart and the last bar ends at the edge. Bars are radius 6 (less for bars under 12 pt), placed absolutely. A frame under 3 pt is not drawn, nor its children; the count covers every frame of the shown root under 3 pt.

A bar has a label when its whole name fits, or at least three characters and the ellipsis do, by a rough measure of 6.3 pt a character at 11.5 pt inside 6 pt of padding; the amount follows only the whole name, with three characters of room between them. A bar's corner is 4 pt, and a quarter of its width on a narrower bar, so a sliver stays a line.

Every bar is keyed by its path in the tree, so a zoom keeps the keys of the frames that stay, and Rune's layout springs move them to their new places; frames that leave fade out.

### State

The zoom (the shown root) and the cursor are kept as paths of (name, tint) with the session's id, not as indices, so a refresh of the same session keeps them when the path still exists; another session shows its root. The hover is a path too, set when the pointer enters a bar and cleared when it leaves the graph. All three are in the inspector's memo version, with the graph's width.

### Colors

From the theme's palette only, each a share of its color mixed into the group's color in linear light: structure 20%, content 50%, the components 32 to 40%. On a dark theme the shares are 0.3 times as large, as the same share of a bright color reads much louder on a dark group. The text is in the ink:

| Frames | Color |
|---|---|
| structure (root, chain, turn, model, origin, …) | the tertiary ink (grey) |
| content (kind, tool, target) | success (green) |
| read | info (blue), as in `Spend` |
| write | warning (yellow), as in `Spend` |
| rewrite after miss, miss causes | danger (red), as in `Spend` |
| output | terminal cyan, as in `Spend` |
| uncached | terminal magenta: `Spend` draws it grey, which here is structure |

Hover mixes the bar 30% toward white. The cursor's ring is a 2 pt border in the accent inside the bar, over the bar's fill mixed 18% toward the accent, drawn while the inspector has the keyboard; inside, it never covers the gap or the frames beside.

### Keys

In `Flame`, ← → `h` `l` move between siblings, as in every flame graph viewer, so the tabs need other keys there: Shift+← and Shift+→ show the tab before or after, in every tab (they did already, as the inspector ignored Shift). `[` and `]` keep stepping the day, as they do from the inspector today. `0` shows `Flame`, the tenth tab. ⏎ zooms into the cursor's frame instead of showing the inspector full size; ⏎ in the list and a double click still do that. Esc and Backspace zoom out one level; esc goes back to the list (or out of full size) only at the top, as before.

### Request counts

The facts of a chain are summed over all its requests before they reach the sinks, so no request identity reaches a stack. Counting requests per frame would need a set of request ids per stack; left out, and the line under the graph says nothing of requests.

### The scene

`--scene flame-zoomed` shows `Flame` with the keyboard, zoomed two levels into the largest frames (the costliest chain, then its costliest turn) as soon as the X-ray is there.

## Risks / Trade-offs

- The X-ray grows by its stacks (a few thousand keys for a large session) → memory per X-ray in the browser's cache; a few MB at most.
- A layout that reads its width from the last frame draws the first frame at a guessed width → the inspector is laid out again at once, as the request track in `Summary` is.
