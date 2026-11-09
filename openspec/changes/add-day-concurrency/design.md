# Design

## Context

The rhythm (work-rhythm) builds a lane per main session: its turns and its work, the turns without their silences and question waits. The day view builds the strip's lanes from the kept sessions, those not left out. It shows a lane only for a session with actions that day, but every kept session's work is at hand there. The calendar's `agent` metric counts, per day, the minutes that some kept session's work touches: a minute counts once when any work overlaps it by more than nothing.

## Goals / Non-Goals

**Goals:**
- One count, made where the strip's lanes are made, so that the terminal and the window agree.
- "Some agent at work" counted as the calendar counts `agent`, so that the two can be compared.

**Non-Goals:** a sparkline row in the terminal strip. The strip's rows are budgeted by the terminal browser and the spec (Day strip, Many sessions). A row would change that budget, so the line comes first.

## Decisions

### The count
- **The sessions:** every kept session's work spans, merged within the session and cut to the day's window.
- **The steps:** a sweep over their starts and ends gives the number at work from each moment on. The peak is the highest number and the first moment it is reached.
- **The sessions at work at a moment** are those whose spans hold it. The window's hover lists them by the strip's labels.

*Alternative:* count the strip's shown lanes only. Rejected: a session that worked that day without the user acting in it would be missed, and the calendar would disagree.

### The numbers of the line
- **Agent-hours:** the sum over sessions of the minutes each one's work touches, whose start is in the window.
- **Some agent at work:** the minutes any session's work touches, whose start is in the window. This is the calendar's rule, on the same work, so the two agree whenever the day view and the calendar draw the same day boundaries. Both take them from the rhythm's days.
- **Keyboard time:** the sittings line's.
- **The share:** the time of the sittings when 2 or more worked, over the time of the sittings.

### Where it lives
- The count is a field of the strip's lanes, so the window gets it with the day it already receives.
- The line is printed by the day text after `sittings`, not by the rhythm lines that the pause report shares.

### Logs and versions
No new field. The work comes from the rhythm's events:
- Claude Code: requests, `stop_reason: end_turn`, interrupts, the AskUserQuestion questions and answers;
- Codex: `task_complete`, `turn_aborted`.

They were seen in the versions work-rhythm names.

### Parse time
None: the count works on lanes already built. A day of 62 sessions has a few thousand spans, so it costs well under a millisecond.
