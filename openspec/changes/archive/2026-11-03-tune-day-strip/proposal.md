# Proposal

## Why

The day strip is hard to read. In the browser it sits in the right pane, so its columns are wide and the lanes run together. Its marks (`=`, `-`, `*`) have no color and blur into one line. Its hours follow the day's first and last actions, so each day has its own scale and the hours move when the user steps from one day to the next.

## What Changes

- **24 hours.** The strip always covers 24 hours from the whole hour at or before the day's start. A column covers the shortest of 10, 15, 20, 30 and 60 minutes that fits 24 hours into the width: 20 minutes in the 72 columns of `tokprof day`, 10 minutes on a screen 200 columns wide.
- **One axis while stepping.** In the browser, the hour the first day shown starts in is kept while `[` and `]` step through days, so hours stay in their columns.
- **Blocks and colors.** Work is `█` in green, a wait for the user's answer `░` in yellow, a message `*` in bold cyan, a question `?` in bold magenta, an interrupt `!` in bold red, and the `you` lane `▓` in blue. The legend uses the same colors. Without color (a pipe, `NO_COLOR`), the marks stay and the colors go.
- **The day full screen.** In `tokprof ui`, `d` shows the day on the whole screen. `d`, esc, `←`, `h` or Backspace go back to the list with the X-ray. The day scrolls as the full-screen X-ray does.
- The README shows the new strip.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `work-rhythm`: the day strip's span, columns, marks, colors and legend.
- `session-browser`: the day view shows full screen, keeps its hours while stepping, and goes back to the list.

## How it is checked on the real logs

- `tokprof day 2026-11-02` draws 04:00 to 04:00 in 20-minute columns, and its rhythm lines are as before: 16 sittings, 4.1h at the keyboard, 59 messages to 12 sessions.
- `tokprof day` in a terminal shows the marks in color; `tokprof day | cat` shows them without escape codes.
- In `tokprof ui` at 200 columns, `d` shows today full screen in 10-minute columns, and `[` shows 2026-11-02 with the same hours in the same columns.

## Out of scope

- An SVG of the day.
- Colors chosen by the terminal's theme; the 16 standard colors work on light and dark ones.

## Impact

- Core: the strip's axis, marks and colors.
- CLI: `day` passes the terminal's color setting; the browser shows the day full screen and keeps its axis hour.
- README.
