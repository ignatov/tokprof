# Proposal

## Why

The survey asks where people's money goes and what problems they have with their agents. Share format 2 answers the first question well, but it is blind to the problems. Since `add-failures-and-waits`, tokprof finds them in the logs, but none of them reaches the share file. On the author's 30 days:
- 1,390 tool calls failed, and the requests after them cost $226 until the tool worked again;
- the user refused 55 calls, and permission rules denied 91;
- the agent waited 23.6 hours for API retries;
- 3,001 of 83,890 requests took a minute or more, and the longest took 23 minutes;
- of the agent's busy time, the model took 149 hours, subagents 67 hours and background commands 44 hours.

Two more gaps make 20 files hard to compare:
- **Activity.** The file does not say how many days and hours a person worked with the agents. So $500 over three days looks the same as $500 over a month. The author's own baseline for changes in the IDE, cold returns and avoidable dollars per active hour, cannot be computed.
- **Interaction.** How often a person interrupts the agent, how often the agent asks, and whether the person types while it works are signals of friction. tokprof reads them already, as hand-offs.

Smaller gaps:
- **Prompt coverage.** The share of spend whose prompts the logs hold. Without it, a colleague on an old Claude Code shows a large `unknown` that reads like content.
- **Effort by client,** to test whether one client's default drives the highest effort.

Files cannot be collected twice, so these parts must be in the file before the first survey's tag.

## What Changes

- **New parts of the share file**, added to format 2 without a new format number. A format-2 file without them reads as one that lacks the parts.
  - `activity`: active days and active hours. An active hour is a clock hour with at least one selected main-conversation request. The part also holds the busy time.
  - `failures`: failed calls and the spend after them until the tool worked again, refused calls and denied calls (see tool-breakdown). Each tool in `tools` also gets its own `failed` and `after`.
  - `time`: busy time by what the agent waited on, with the part after ends of turn (see agent-time). It also holds the requests, the slow ones, the longest one, and the main requests that waited for retries. No error text goes into the file.
  - Busy time and its requests count only inside the window. Until now the pause report counted whole sessions: on the author's 30 days, its busy table held 107,256 Codex requests against about 15,700 sent in the window, because long Codex threads reach months back. The user chose to clip both the file and `tokprof pauses`.
  - `handoffs`: in the main conversations, the ends of turn, interrupts, questions, answers, messages typed while the agent worked, and wakes by source (see session-logs).
  - `prompts`: the requests and dollars whose prompts the logs hold, of all; and per version in `versions`.
  - Per client: active hours, and requests and dollars by effort level.
- **The report**, for `tokprof share` and `tokprof pool`:
  - an `activity` line in the header: active days, active hours, busy hours, and dollars per active hour;
  - `What could have been saved` gets a `per active hour` column, and a table of cold returns by client per active hour, the baseline for changes in a client;
  - a new section `Problems`: failed, refused and denied calls, the spend after failures, the time waiting for retries and slow requests, pooled and per active hour;
  - a new section `Where the agent's time went`: busy time by kind;
  - a new section `Working with the agent`: the hand-offs, per 100 typed messages;
  - a table of the highest effort by client, costliest first;
  - a line under the model table that names the models without a price;
  - a line under the content table with the share of spend whose prompts the logs hold, when it is below 100%;
  - the `Tools and commands` table gets `failed`.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `pause-report`: "Busy time part" counts only the busy time and the requests inside the window.
- `share-export`: the parts above. They are new requirements next to the ones `add-team-survey` adds; no requirement of format 2 changes.
- `share-pool`: the activity line, the per-hour column, the cold returns by client, the sections `Problems`, `Where the agent's time went` and `Working with the agent`, effort by client, the coverage line, and `failed` in the tool table.

## How it is checked on the real logs

- **The same numbers as the reports.** Over one closed window, the file must agree with the reports:
  - `failures` with the failures line of `tokprof tools` (1,390 failed, $226, 55 refused, 91 denied over 30 days on 2026-11-03);
  - `time` with the busy table and slow line of `tokprof pauses`;
  - `handoffs` with a count of the hand-offs that the sources read for the same sessions (see session-logs);
  - `prompts` with the `prompt known` column of `tokprof summary`.
- **Active hours.** Record the active hours of the author's 30 days, and those of the IDE's client alone, against the 312 the cold-sessions study counted for that client.
- **No names.** The distinctive-names test grows by a failed call whose error names `zebra-billing`, and a retry error that names a host. Neither may reach the file. A search of the real file for error texts finds none.
- **Size and time.** The file stays under 150 KB, and `share` stays within 5% of its time before the change.
- **The report.** The share report of the author's file shows each new section. A pool of format-2 files with and without the new parts says `in <k> of <n> files` for the sections the older files lack.

## Out of scope

- Error texts, the commands that failed, and the longest waits with their times, which could identify work.
- Per-session or per-day rows.
- The bench and the X-ray.

## Impact

- **Core.** The share collector counts active hours, hand-offs, prompt coverage and effort by client. It writes the failure fields of its per-agent tool table and the busy time of the pause collector. The pool reads the new parts, and the report gets its new tables.
- **CLI.** `tokprof share` paints the busy time of each session, as `tokprof pauses` does, before it writes the file.
- **Docs.** The README's share and pool sections, and `docs/survey.md` on reading the report.
- **Order.** Apply after `add-team-survey`, whose requirements this change adds to.
