# Proposal

## Why

On a wide screen the browser's day view uses a small corner of it. A screen 190 columns wide and 50 rows high shows 72 columns of 20 minutes, labels cut to 20 characters, 8 lanes and the line `8 more sessions`, and lanes packed one against the other. The rest of the screen stays empty. The 72 columns are a bug: the browser capped the strip at the width `tokprof day` uses, though the spec lets it take the width the browser leaves.

## What Changes

- **Columns fill the width.** The browser no longer caps the strip at 72 columns. At 190 columns a column covers 10 minutes. This restores what the spec says.
- **Labels grow with the screen.** In the browser a lane's label takes a fifth of the screen's width, at least 20 characters and at most 40.
- **Lanes fill the height.** In the browser the day shows as many lanes as the screen has rows for, and folds the rest into the `more sessions` line only when they do not fit.
- **Room between lanes.** When the screen has rows to spare, each lane is followed by a row of dim `·` marks at the hours, so the lanes stand apart and the hours can be read down the screen.
- `tokprof day` keeps its layout: 20-character labels, 72 columns, 8 lanes.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `work-rhythm`: the strip takes its label width, columns and rows from where it is shown, and can space its lanes.
- `session-browser`: the day view fits the screen.

## How it is checked on the real logs

- In `tokprof ui` at 190x50, the day of 2026-10-27 shows 10-minute columns, labels of 38 characters, all 16 lanes with a row of hour marks after each, and no `more sessions` line.
- At 120x30 it shows 20-minute columns (93 columns do not hold the 96 of 15 minutes) and as many lanes as fit, then `N more sessions`.
- `tokprof day 2026-10-27` prints the same strip as before this change.

## Out of scope

- Fitting `tokprof day` to the terminal it runs in.
- Columns of less than 10 minutes.

## Impact

- Core: the strip takes a layout: label width, columns and rows.
- CLI: the browser computes the layout from the screen's size and asks for the day again when the screen changes.
