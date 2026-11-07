# Proposal

## Why

`tokprof flame` writes where a session's money went as profiles for other tools: a JFR, async-profiler's HTML flame graph, collapsed stacks, pprof. To look at one, the author leaves the window, runs the command and opens the file elsewhere. The window already shows the session's X-ray under tabs, but none of them shows the spend as a whole tree, from the chain down to the turn, the content, the tool, the file and the component.

## What Changes

- **A `Flame` tab** in the inspector of `tokprof-desk`, after `Cache`. It draws the selected session's spend as an icicle graph: the session on top, its chains below, then turns, content kinds, tools, targets and components, the stacks and the dollars that `tokprof flame --session <id>` writes.
- **Each frame is a rounded bar** as wide as its share of the shown root's dollars, with its name inside when it fits, and its dollars when there is room. Frames narrower than 3 pt are not drawn; the line below the graph says how many.
- **Colors by what a frame is**, from the theme: structure (the session, chains, turns) grey, content (kinds, tools, targets) green, and the components in the colors the `Spend` tab gives them.
- **Hover** lightens a frame and the line under the graph names its path, its dollars, its share of the session and of its parent.
- **Zoom**: a click on a frame shows it as the root, full width; a breadcrumb above the graph shows the path, and a click on a crumb, esc or Backspace zooms out.
- **Keys**: while the inspector has the keyboard on `Flame`, ← and → move a cursor to the sibling before or after, ↓ to the first child, ↑ to the parent, ⏎ zooms into the cursor's frame, esc and Backspace zoom out. Shift+← and Shift+→ show the tab before or after, in every tab. `0` shows `Flame`.
- **A scene** for frames without a window, `--scene flame-zoomed`, and `--tab flame`.

## Capabilities

### New Capabilities
None.

### Modified Capabilities
- `desk-window`: the inspector gains the `Flame` tab, and the keys of the inspector gain `0` and Shift+← and Shift+→, with ← and → moving through the frames in `Flame`.

## Impact

- `crates/tokprof-core`: the X-ray carries the session's profile stacks, built by the default stack of `tokprof flame` in the same pass that builds the rest of the X-ray.
- `crates/tokprof-desk`: a new file for the tab (the frame tree, its layout, the keys), the tab in the inspector, the key routing.
- `crates/tokprof-desk/README.md`: the tab and its keys.
- No change to `tokprof flame`, `tokprof scan`'s output, `tokprof ui` or any log source.

## How it is checked on the real logs

- On the newest session of the author's logs: the root of the `Flame` tab shows the sum of `tokprof flame --session <id> -o out.collapsed` in µ$, as dollars, and that equals the session total of `tokprof scan <id>` to the cent.
- The time of `tokprof` (the X-ray of the latest session) before and after the change, on the same session.
- Frames rendered with `--snap … --tab flame` in light and dark and `--scene flame-zoomed`, looked at by eye.

## Out of scope

- Other stacks than the default one, other measures (tokens, avoidable cost), and a search or highlight box.
- Request counts per frame: the spend of a frame is summed over the content in every prompt, and no request identity reaches the stacks.
- The frame tree in `tokprof ui` in the terminal.
