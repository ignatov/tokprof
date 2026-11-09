# Design

## Context

The day store (activity-calendar, format 2 since add-month-review) keeps one entry per log. Each entry holds:
- per minute of UTC with a request: the spend, the avoidable premium of the unexpected misses, and the requests;
- per hour: the avoidable premium of each unexpected miss cause, by name;
- the times of its cold returns;
- its agent, main session, project and session.

The calendar's worker brings the store up to date when the Calendar shows. Reports reads its own period from the logs on another worker. See proposal.md for why.

## Goals / Non-Goals

**Goals:**
- The period before comes from the store, in well under a second.
- Its numbers are the ones the overview would compute for that window.

**Non-Goals:**
- Any change to the store. It is the calendar's; a field it lacks is an open item.
- Verifying a fix on given sessions.

## Decisions

### The window before
For a window `[from, to)`, the window before is `[from − (to − from), from)`. Reports' window is its period up to now, so the window before ends where the shown period starts. For `tokprof summary`, `--since` and `--until` (or now) give the window, and `--compare` without `--since` fails: all of history has nothing before it.

### The numbers from the store
- **Spend, avoidable dollars and requests:** the minutes whose start is in the window. They equal the summary's `Total priced`, its avoidable dollars and its request count, minute for minute, because the store counts them in the same pass.
- **Sessions:** the distinct agent and main session of the entries with a request in the window. The overview counts a subagent's log with its session the same way.
- **Cold returns:** the cold returns whose time is in the window. The store keeps only their times, so only their count is compared.
- **Causes:** the hours whose start is in the window. A window that does not start on the hour, like Reports' rolling one, counts the miss dollars of its first partial hour with the window that holds the hour's start. A window that starts at a local midnight, like the summary's dates, starts on the hour in whole-hour time zones and counts them exactly.
- **The filters:** `--agent`, `--project` and `--session` are the store's own filters (calendar filters). With `--client` there is no comparison: the store keeps no client.
- **No earlier data:** the period before has no earlier data when it has no request, no miss and no cold return in the store.

### Where the store comes from
- **The window:** while Reports shows, the window asks the calendar's worker for the store, as the Calendar view does. It then compares the shown period with the store's window before.
- **The CLI:** `tokprof summary --compare` brings the store up to date first, as `tokprof calendar` does, and then reads it.

### Logs and versions
None beyond the store's (activity-calendar names its fields).

### Parse time
The plain summary is unchanged. `--compare` reads the logs that changed into the store, as `tokprof calendar` does; a store already up to date takes under a second. The totals of a window over the store took 0.5 ms on the author's 1,962 logs.
