# Spec Delta

## ADDED Requirements

### Requirement: Month in review in the window
Calendar SHALL offer `Month in review` as a button after the metric pills, as the key `r` and as a palette entry. Each opens the card of month-review for the month of the cell cursor, or of the last day, over the window dimmed and blurred. ← and → show the month before and after, never past this month. esc, `Close` or a click outside close it. While it is open, every key but ⌘Q is its own.

#### Scenario: Opening the card
- **WHEN** Calendar shows the last day of October under the cursor and the user presses `r`
- **THEN** the card of October 2026 opens over Calendar

#### Scenario: Keys stay with the card
- **WHEN** the card is open and the user types `q` and `m`
- **THEN** the window stays open and the metric does not change

### Requirement: The card's look
The card SHALL be 540 by 675 points, a 4:5 shape, in the theme's colors on its canvas. It shows the headline in large type, the title as an accent badge beside its rule, the big numbers three by two, and the month's days as squares in the calendar's levels of spend beside the facts. Then come the cache and the comparison as rows of small numbers, and the line for next month in a tinted box. The busiest day's square is ringed.

#### Scenario: The days' legend
- **WHEN** the card is drawn
- **THEN** under the month's squares a legend reads `spend per day · less`, the five squares, `more`, and `cuts at $121, $269, $591`

#### Scenario: No month before
- **WHEN** the month before has no data
- **THEN** the room of the comparison goes between the card's parts, and no empty band stands above the line for next month

#### Scenario: Plain words
- **WHEN** coming back after a cold cache lost $873, more than any other cause
- **THEN** the card's line reads `Coming back after the cache expired cost $873. Compact or wrap up a big session before a long break.`, while `tokprof month` keeps `$873 went to idle > TTL.`; requests without a price are a footnote

#### Scenario: Light and dark
- **WHEN** the window is drawn in Merino Light or Merino Dark
- **THEN** the card uses that theme's canvas, ink and accent, and every text stays inside it

### Requirement: Saving the card
`Save image` and `s` SHALL write the card alone as a 1080×1350 PNG to `~/Downloads/tokprof-<YYYY-MM>.png`, drawn in the window's theme without a window, with the window's filters. A file of that name is never overwritten: the next free of `-2`, `-3`… is used. The card and the bar then say `saved ~/Downloads/tokprof-2026-10.png`, or why it could not be saved. Nothing is sent anywhere.

#### Scenario: A second save
- **WHEN** `~/Downloads/tokprof-2026-10.png` exists and the user saves October again
- **THEN** the image goes to `~/Downloads/tokprof-2026-10-2.png`, and the window says so

### Requirement: The card in a frame without a window
`--scene month` SHALL draw Calendar with the card open, and `--scene month-card` the card alone, filling the frame. Both take the month from `--month YYYY-MM`, else the month of the last day. `--month` takes only YYYY-MM.

#### Scenario: The image of October
- **WHEN** the user runs `tokprof-desk --snap out.png --scene month-card --month 2026-10 --size 540x675`
- **THEN** `out.png` is the card of October, 1080 by 1350 pixels

#### Scenario: A bad month
- **WHEN** the user runs `tokprof-desk --snap out.png --scene month --month october`
- **THEN** the command fails with `--month takes YYYY-MM`
