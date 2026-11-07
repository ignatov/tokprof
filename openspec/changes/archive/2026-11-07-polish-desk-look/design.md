# Design

## Context

The window is drawn with Rune. The settings panel and Reports already have the approved look; the main window (bar, lanes, list, inspector) predates it. The look's parts — the pill radius, the keycap and the hint — are in `overlay`, and Reports had its own copy of the meter and of the card. No log fields are involved: this change only draws what the window already has. It does not touch parsing, so the corpus's parse time is unchanged.

## Decisions

- **One set of parts.** `overlay` holds the radii (`R_CARD` 22, `R_GROUP` 16, `R_TILE` 12, `R_ROW` 8, `R_MARK` 3, `PILL`), the ring, the meter, the empty state and the fade. The inspector, Reports, Flame and the overlays use them. The inspector's card is Reports' card: the title above a soft group.
- **The ring marks the keyboard.** `ring` puts `outline(2, offset, accent)` on the element and names it with a trace name, so a test can find which element has the ring (Rune's element list does not carry outlines). Where it goes:
  - a row of the list: the inset cells, 1 point out;
  - a lane: its inset band, 0 points out, since lanes touch;
  - the inspector: the cursor's row, or the chosen tab on a tab with no rows;
  - Flame: its own ringed frame;
  - the filter while it is being typed.

  The list takes the ring only while no filter is being typed and the inspector is not full size. The 2-point focus line goes.
- **The selection is quiet without the keyboard.** The list's selected row is `sel` (accentDim) with ink text. The agent dot and the countdown keep their colors, so they no longer vanish into the accent on neon-arcade or sit on yellow on saffron. A lane's band is `sel` at 60% of its own alpha, under the marks, so waits still show on warm themes. The 3-point accent bar at the label marks the lane on every theme.
- **Dark marks.** `pal` mixes work, request bars, cache read, write, output and the rewrite part 60% from the canvas toward the color, in sRGB, where a share of the way reads as that share. Rune's own `mix` is in linear light, where the first 0.58 still read as about 0.8 and looked near neon on merino-dark and neon-arcade. Text colors (`warm`, `cold`) stay the theme's own. Rewrite has its own fill (`miss`) because `cold` is also the text red. Waits get a 1-point edge in the wait color, so that on monochrome themes (cathode-amber) they differ from work in shape too.
- **Sizes without a feedback loop.** The view reads the root's last laid-out size. Before any layout it reads the size `main` seeded from `--size` (or 1400x900). Headless frames are one frame, so the seed is what they use. The lanes' cap includes everything under the tracks: the legend, three day lines of 16 points, gaps, padding and the line, 85 points.
- **Columns by priority.** A table is given the names of the columns to drop, in order. Before drawing, it drops them while the fixed widths, the gaps and the text column's 120 points are more than the card's room. The room is the inspector pane's width less its padding and a card's: from the layout, or from the window size before it.
- **Labels without measuring.** Rune does not measure text before layout. Three places estimate a width from the character count (0.55 em per character, 7.2 points per character in the request track):
  - the lane label's project keeps that least width, up to 60% of the label;
  - the request track's two labels are checked for overlap;
  - the key help sizes its key column from the keycaps.
- **Days in the palette.** After the fuzzy match finds the best placement, a day keeps it only when the placement is runs of letters that each start a word, each run in the word after the last one. Sessions, tabs and actions do not use this check.

## The second review

A second design pass on 2026-11-07 found four regressions and the items left over. Decisions:

- **Whole lanes.** The scroller is as tall as the hours row and the whole lanes that fit. When some do not, a `SMALL` line under them says how many more there are, and it takes the room of one lane. It does not count from the scroll position, so it stays true while scrolling.
- **The lane's label.** A head that fits keeps its own width, so the title starts the label's gap after it. A head too long for its share is cut to it. When the head would leave the title under 60 points, the title is left out and the head takes the room.
- **Tabs on one line.** Their padding is computed from the pane's width and the labels' estimated widths, between 3 and 8 points. The digits show as keycaps only while the inspector has the keyboard and they fit.
- **Settings in a dark window.** The panel keeps the part with the keyboard and the grids' order unset until a key or a click fixes them. Until then, both follow the appearance the window is drawn in. This works for the frames without a window, whose scene opens the panel before anything is drawn. Picking an appearance fixes them first, so the grids do not jump under the pointer.
- **Flame names a frame once.** The breadcrumb above the graph has the path to the shown root. The line under the graph names the frame by its path under that root. On dark themes the legend's swatches are their color at 60%, because the frames' own fills sit too deep in the group for a 10-point square to show.
- **Waits.** A note goes under its label, so that every bar has one track and the numbers end at the card's edge.

## Risks / Trade-offs

- Estimated widths can be off by a few points for wide or narrow scripts. The worst case is a small gap after a short project name, or a label kept that touches the other one.
- Tables drop columns at 1400 points that used to overflow the card (the Tools hint). The text is still in `tokprof scan`.
