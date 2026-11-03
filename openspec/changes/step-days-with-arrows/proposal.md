# Proposal

## Why

In the browser's day view, `←` goes back to the list, and the days step with `[` and `]`. The author reaches for the arrows to step through days and for esc to leave.

## What Changes

- In the day view of `tokprof ui`, `←` or `h` steps to the day before and `→` or `l` to the day after, never past the current day. `[` and `]` keep doing the same.
- Esc, `d` and Backspace go back to the list with the X-ray. `←` and `h` no longer do.
- While the day is shown, the footer reads ` ↑↓ PgUp PgDn scroll · ← → days · esc back · f JFR · o HTML · c handoff · q quit`.
- The README's table of keys says so.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `session-browser`: the keys of the day view and its footer.

## How it is checked on the real logs

- In `tokprof ui` at 190x50 on 2026-11-03: `d` shows ` day · Tue 2026-11-03 `, `←` twice shows Sun 2026-11-01, `→` twice shows Tue 2026-11-03 again, one more `→` stays there, and esc shows the list with the X-ray.

## Out of scope

- The keys of the list and the full-screen X-ray, where `←` still goes back.
- Stepping by a week or jumping to a date.

## Impact

- CLI: the key handling and the footer of `tokprof ui`.
- README.
