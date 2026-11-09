# Proposal

## Why

The Air order wants tokprof to reach people beyond its users. "Wrapped" cards from Cursor and tokscale get posted; none of them talks in dollars or about the prompt cache (market scan, idea 7). tokprof already keeps every day in the day store (add-activity-calendar), so a month can be summed at once. A card made to be posted, with no project in it, puts tokprof's numbers where people see them.

## What Changes

- **`tokprof month [YYYY-MM]`**: a month in review in the terminal. By default it shows the last complete month; `this` shows this month so far. It reads only the day store, so it answers at once on unchanged logs.
- **The card** reads "October 2026 with agents". It has:
  - a title with the rule it met;
  - big numbers: spend, agent hours, your hours at the keyboard, sessions, messages and the agents' commits;
  - the month's days at the calendar's levels of spend;
  - the busiest day and the busiest hour;
  - the share of agent time between 22:00 and 05:00, and the longest streak;
  - the cache: the share of input read from it, the dollars cache reads saved, the dollars lost to misses, and the cold returns;
  - the top model;
  - the change from the month before in spend, agent hours and the avoidable share;
  - one line for next month: the costliest miss cause, its dollars and tokprof's advice for it.
  It names no project, title or path.
- **One title**, the first of six whose rule holds: Night owl, Early bird, Conductor, Marathon, Cache keeper, Shipper. The thresholds are set on the author's months, so that August, September and October each get a different one. When no rule holds, the card has no title.
- **The day store, format 2**: each minute also keeps the tokens read from the cache and all prompt tokens, the dollars the cache reads saved, and the requests without a price. Each log also keeps its avoidable premium by miss cause and its spend by model, per hour, and the times of its cold returns. The store is not released yet, so format 1 is only left in place.
- **In the window**: Calendar gets a `Month in review` button, the key `r` and a palette entry. Each opens the card for the month under the cell cursor, over the window; ← and → step months. `Save image` writes the card as a 1080×1350 PNG to `~/Downloads/tokprof-<YYYY-MM>.png`, adding `-2`, `-3`… rather than overwriting, and says where it went. `--snap --scene month --month 2026-10` draws the card over Calendar, and `--scene month-card` draws the card alone.

## Capabilities

### New Capabilities
- `month-review`: the month's window and numbers, the extra numbers of the day store, the titles and their thresholds, and `tokprof month`.

### Modified Capabilities
- `desk-window`: the month in review over Calendar, its keys, `Save image` and its scenes.
- The unarchived change `add-activity-calendar`: its store is now `days-2.json`, with the fields above. This changes that change's text, not a released spec.

## Impact

- `crates/tokprof-core`: new fields in the day store, and a module that sums a month from the entries.
- `crates/tokprof-browse`: the month's texts, and the month computed from the store.
- `crates/tokprof`: the `month` command.
- `crates/tokprof-desk`: `src/month.rs` (the card, the overlay, saving the image), plus small, additive hooks in the Calendar view, the palette, the key help, the scenes and the view.

## How it is checked on the real logs

- October's spend and avoidable dollars equal `tokprof summary --since 2026-10-01 --until 2026-11-01`.
- Its messages and keyboard hours equal the sum of the calendar's October cells.
- Light and dark frames of October and September are looked at, and one PNG is saved as `Save image` writes it.
- The title each month gets is recorded in tasks.md.

## Out of scope

- Uploading or sharing anywhere: tokprof makes no network call.
- A year card.
- Comparing people.
- A `--client` filter, as in the calendar.
