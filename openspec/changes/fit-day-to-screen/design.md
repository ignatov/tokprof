# Design

## Context

The strip takes the most columns it may use. Its label width (20) and lane count (8) are constants. The browser computes the columns as the pane's width less 23, but caps them at the 72 that `tokprof day` uses. Since the day became full screen, that cap leaves most of the screen empty.

## Goals / Non-Goals

**Goals:**
- One layout input for the strip, so that `tokprof day` and the browser differ only in the numbers they pass.

**Non-Goals:**
- Fitting `tokprof day` to its terminal.

## Decisions

1. **A layout carries three numbers and a switch:** the label width, the strip's columns, the rows the lane block may take (lanes, hour-mark rows and the `more sessions` line), and whether lanes may be spaced. `tokprof day` passes 20, 72, 9 and no spacing, so its output does not change.
2. **The browser computes them from the screen.** The label is a fifth of the width, from 20 to 40 characters. The strip has the width less the label, a space and the two borders. The lanes have the height less 18 rows: the footer, the borders, the lines above and below the strip, and its hours, `you` and legend rows.
3. **Lanes fill the rows, most active first.** With fewer rows than sessions, the last row holds `N more sessions`. With rows for twice the lanes shown, each lane is followed by a row of dim `·` in the first column of each hour.
4. **The day is asked for again when the screen changes.** The browser's cache of day views already keys on the width; the height joins it.

## Risks / Trade-offs

- [A short screen with many sessions] → the lane block shrinks to the rows it has, and the `more sessions` line counts the rest.

## Parse time

Nothing changes in reading or parsing.
