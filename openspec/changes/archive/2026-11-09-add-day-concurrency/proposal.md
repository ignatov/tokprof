# Proposal

## Why

"How many agents was I really running?" The day view shows a lane per session, but the eye cannot add twenty lanes up. AgentsView has a concurrency timeline that answers it. It is also the author's attention theme, and the question Air's view of several sessions raises. tokprof already knows when each session's agent worked (work-rhythm's Work), so it can count them at each moment.

## What Changes

- **An `Agents at work` lane** in the window's Day view, under `You at the keyboard`.
  - It is a stepped area as high as the number of sessions whose agent worked at each moment, on work-rhythm's definition of work. Only main conversations count, and a subagent counts as part of its session, not as another agent.
  - It is labelled with its peak, as in `peak 7 at 14:20`.
  - Hovering over it gives the time, the count and the sessions working then.
- **An `agents` line** after `sittings`, in `tokprof day` and under the window's lanes, e.g. `agents    21.3 agent-hours over 7.3h at the keyboard, some agent at work 12.5h; 2 or more worked 38% of the time you were there; peak 7 at 14:20`.
  - Agent-hours add up each session's time.
  - "Some agent at work" counts each minute once, as the calendar's `agent` metric does.
- `tokprof day` gets the line only, not a sparkline row in its strip (see design).

## Capabilities

### New Capabilities
None.

### Modified Capabilities
- `work-rhythm`:
  - adds `Agents at work` and `Agents line`;
  - modifies `Day view`, which now prints the `agents` line.
- `desk-window`:
  - adds `Agents at work lane`;
  - modifies `Day lanes` (the new lane) and `What the lanes say` (the `agents` line, and the pointer over the new lane).

## Impact

- **Code:**
  - a count of the agents at work in tokprof-core, made from the lanes the day strip already builds;
  - the `agents` line in the day text of tokprof-browse;
  - the lane in tokprof-desk.
- **Logs:** no new field. The count reads the rhythm's work, which comes from requests, ends of turn, questions and answers (work-rhythm).
- **Check on the real logs:**
  - On 2026-11-02, 11-03 and 11-06, "some agent at work" must equal the calendar's `agent` metric for the same day.
  - The peak must equal a direct count of the sessions whose work holds that moment.
  - The other lines of `tokprof day` must stay byte-identical.
  - Frames of 2026-11-01 (62 sessions) and 2026-11-03, light and dark.
- **Out of scope:**
  - subagents as separate agents;
  - a concurrency view over many days;
  - the strip row in the terminal;
  - the `tokprof pauses` rhythm section.
