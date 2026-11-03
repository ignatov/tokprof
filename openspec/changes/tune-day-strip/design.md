# Design

## Context

The strip is built in the library as lines of characters, one mark per column, over the hours from the day's first action to its last. `tokprof day` prints it; the browser shows the same text in its right pane, and full screen after `⏎`. The X-ray already paints its text with ANSI codes through the library's style, and the browser turns ANSI into its own text with `ansi_to_tui`.

## Goals / Non-Goals

**Goals:**
- One scale for every day, and one axis while the browser steps through days.
- Colors that `tokprof day` and the browser both show, with the same code.

**Non-Goals:**
- Hour grid lines or two rows per lane.

## Decisions

1. **The axis is an input.** The strip takes the start of its 24 hours. By default that is the whole hour at or before the day's start. The browser passes the hour of the first day it showed, as a local hour, so later days start their strips at the same hour of their own dates.
   - Rejected: the median start of the days read. The day view reads two or three nights, so that median moves as much as the day's own start.
2. **Marks become cells, and painting is the last step.** The strip keeps each column as a kind: work, wait, message, question, interrupt, presence or empty. It renders runs of one kind with one color code, so a lane holds a few codes and not one per column. With color off it renders the plain marks.
3. **The colors are the 16 standard ANSI ones:** green, yellow, bold cyan, bold magenta, bold red and blue. They read on light and dark themes, and the X-ray already uses them.
4. **Color follows the output.** `tokprof day` paints when its output is a terminal and `NO_COLOR` is unset, as the X-ray does. The browser always paints and converts the codes as it does for X-rays.
5. **The day is a full-screen mode.** `d` sets the day view and the full-screen flag together; leaving the day clears both. The pane width it is laid out for is then the screen's width, so wide terminals get 10-minute columns.

## Risks / Trade-offs

- [`█`, `░` and `▓` have no width of their own in some East Asian fonts] → they are the blocks the X-ray's bars already use.
- [A day that starts before the kept hour] → its strip starts at that hour of its date, and what came before, the middle of its night, falls into the first column.

## Parse time

Nothing changes in reading or parsing. Painting adds a few codes per lane.
