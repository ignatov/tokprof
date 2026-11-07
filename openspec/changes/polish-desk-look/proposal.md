# Proposal

## Why

A design review of the window on 2026-11-06 (27 items, frames in the author's plans folder) found that the main window still has the first, boxy look the author called "очень прямоугольный": 6-7 point radii, a 1-point line around every control and card, the selected row filled in solid accent. The settings panel and Reports already have the approved look: soft rounded groups without borders, tiles, pills, keycaps, and an accent ring on the part that has the keyboard. Next to them, the main window looks like another app.

It also found faults that change what the window says:
- In `Waits`, rows with a note got a shorter bar track than rows without one, so a smaller share could draw a longer bar.
- The pane with the keyboard was marked only by a 2-point line along its top, which is hard to see.
- On a dark theme, work bars and the stacked spend are neon bright. The author has rejected bright dark themes once already.
- Frames without a window, and the first frame of a small window, were laid out for a 900-point height. At 960x640, the settings card ran off the window and the lanes took 62% of the height instead of 45%.

## What Changes

- **The bar**: Day/Reports, Fit/24 h and the list's sorts become pill segments without borders. The steps to the day before and after become circles, and Today becomes a pill. The filter is a pill with a `/` keycap while idle and the accent ring while typing.
- **The inspector's cards** become Reports' cards: the title above a soft group (radius 16, no border). The three cards in Summary's row are equally tall. The tabs become pills. The handoff button is a pill with a `c` keycap.
- **Selection and keyboard**: the selected row of the list is a rounded row inset 6 points, in the selection color with ink text. The selected lane is a lighter inset band with a 3-point accent mark at its label. The pane with the keyboard rings its selection (the row, the lane, the inspector's cursor row, or the chosen tab where a tab has no rows) and draws its title in the accent. The focus line goes. While a filter is being typed, the filter field has the ring and the list does not.
- **Lanes**: a long project name gives way before the title is cut to nothing. The day's `sittings`, `answers` and `cache` lines go one under the other. In the cache line, only the count of cold returns and the dollars are red.
- **Data display**: every row of a bar chart keeps the note's room when any row has a note. In the request track, the largest request's label moves left of its bar, or is left out, rather than cover the next request's label. Narrow tables drop their least columns first (Tools: hint, after, failed, single; Prompts: what it paid for; Cache: chain). Reports' axis labels sit on their lines, the regular day label next to a month's label gives way, the readout wraps, and the key numbers' sub-lines take two lines.
- **Overlays**: the key help ends with a foot of keycaps like the settings panel's. The key column is as wide as each group's keys, and the two `esc` rows of the list are one. The key help and the palette fade their last row when they scroll.
- **Flame**: the legend's swatches are the frames' colors, and the keys are keycaps.
- **Empty states**: one soft group with a line in the ink and a quieter one. It is used for a Cache tab without misses, for a list with nothing matching the filter, and for an inspector with no session.
- **Dark themes**: work, request bars and the parts of the spend are mixed 60% of the way from the background in sRGB, as the eye sees it (first 58% in linear light; the second review found that still too bright). The themes' colors and all text colors stay.
- **Sizes**: the window's size comes from the root's layout. Before the first layout it comes from `--size`, or 1400x900. The lanes' 45% includes the legend and the day's lines under them.
- **Radii** come from shared constants: card 22, group 16, tile 12, row 8, mark 3, pill.
- **Palette**: a day matches only by runs of letters that each start a word, the words one after another. Typing `to` no longer lists `Thu 8 Oct` (T…O). Sessions, tabs and actions match as before. This edits the `add-desk-palette` change's delta, which is not archived yet.

## Capabilities

### New Capabilities
None.

### Modified Capabilities
- `desk-window`: Session list (inset selected row, empty filter state), Keyboard focus (the accent ring instead of the line), Marks in a theme (large filled marks lowered on dark).

## How it is checked

- Headless frames at 1400x900, 960x640 and 1400x700, in light and dark, on Merino and on neon-arcade, sunlit-parchment, cathode-amber, alpine-milk and saffron-nightfall, compared with the review's frames.
- At 960x640, the settings title and `Done` are on the window, and the lanes with what is under them take at most 45% of the height under the bar.
- Tests:
  - Two bar rows keep the order of their shares whether or not they have a note.
  - The ring is on the selection of the pane with the keyboard only.
  - Narrow tables drop columns in their order.
  - The largest request's label keeps clear of the next one's.
  - Days match only in word runs.
  - Dark themes lower the marks and keep the text colors.
  - The first frame takes its height from `--size`.

## Second review

A second design pass (2026-11-07) found four regressions: lanes cut in half, tabs off the window at 960, an uneven gap after a lane's project, and a narrow table into the card's padding. It also found the items left over. Tasks section 9 fixes them:
- whole lanes with a count of the rest;
- tabs that fit, with digit keycaps while the inspector has the keyboard;
- the settings opening on the window's appearance;
- Flame naming a frame once;
- calmer dark marks;
- the Cache tab with only expected rebuilds;
- the key help's descriptions aligned.

## Out of scope

- The themes' colors themselves: the window draws them as Rex does.
- The Reports cards and the Reports group of the key help, which another change is editing.
