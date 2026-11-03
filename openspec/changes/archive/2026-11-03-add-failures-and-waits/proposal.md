# Proposal

## Why

tokprof says what a session cost, but not what the agent waited on, which of its calls failed, or which requests were slow. CodeBurn's `sessions --why` shows these three, and the author found them useful. On session 3edfd2e2, CodeBurn's split of pauses is wrong for most of them: 4 of the 5 waits it calls "waiting on helpers" in prompt 2 (27 of 52 minutes) and its "nothing running" waits of 13 minutes and 1h 50m were all waits for test runs and scripts the agent had started in the background. The logs say so, and tokprof can read it.

A study on 2026-11-03 measured 30 days of the author's Claude Code logs: 189 sessions with 763 subagent logs, 81,324 requests and $10,552.
- **Failures.** 1,021 of 95,397 tool calls failed (1.07%). Another 153 were refused or denied: 101 by permission rules, 50 by the user, 2 cancelled. The requests that read the failures cost $102, and the requests until the same tool worked again cost $130. 11% of the failures never got a working call of their tool again in their turn. Several failures repeat:
  - the `advisor` tool failed 15 times out of 15 ("No such tool available");
  - 18 subagent `Write` calls were stopped with "Subagents should return findings as text";
  - 16 subagent calls hit the limit of concurrent subagents;
  - the IDE's MCP tools failed 9 times on a worktree path and 10 times on a tool name;
  - 21 `Read` calls asked for a missing file and 14 for a file that was too large;
  - 11 `Edit` calls did not find the text they were to replace.
- **Slow requests.** 3.3% of requests took a minute or more from send to the end of the response: 2.7% on Opus 5.5, 11.0% on Sonnet 5.5, 19.8% on Fable 5.1. Most of them write a lot: a median of 9,165 output tokens, at about 105 tokens per second. Separately, 45 requests waited 10.8 hours in all for retries after 464 API errors, 353 of them `502` from the local gateway. The longest such wait was 93 minutes.
- **Waits.** After ending its turn, the agent waited 436 times for work it had started itself, 84 hours in all: 41 hours for commands it ran in the background, 26 hours for subagents, and 17 hours for both. Inside its turns it stood still 203 times for a minute or more, 35 hours in all; 29.5 of these hours hold API retries, requests that got no answer, or compactions. 147 of 1,376 typed messages (11%) came while the agent's background work was still running.

Codex logs the same signals in other fields. In code mode, 43 `exec` calls ended in `Script failed` and 992 of 13,694 (7.2%) printed a non-zero exit code. Its requests took a median of 3.6 seconds, and 4.3% took a minute or more.

## What Changes

- **Request times.** The sources give each request the time it was sent and the time its response ended. A Claude Code request that was retried after API errors counts from its first attempt, with the number of retries and the last error.
- **Call outcomes.** The sources tell, for each tool result, whether the call failed, the user refused it, or a permission rule denied it, and keep the line that tells why it failed.
- **What a wake reports.** A Claude Code background notification tells whether a subagent or a background command finished, or something else.
- **Busy time.** A new view of where an agent's time went while it was busy: from the start of each turn to its end, and while its own background work ran after the turn. The time goes to the model, API retries, subagents, background commands, compactions, the activity of the calls before each request (`building and testing`, `reading`, `editing` and the rest, as cost-attribution names them), or `nothing` when nothing ran.
- **A `waits` block in the X-ray**, after `work`: busy time by what the agent waited on, the slow requests, and the three longest waits that were not the model.
- **A `failures` block in the X-ray**, after `tools`: the tools whose calls failed, how often, what the requests after the failures cost until the tool worked again, and the latest error; plus how many calls the user refused and rules denied.
- **`tokprof tools`** gets the columns `failed` and `after`, the sort `--by failed`, and a line with the totals of failed, refused and denied calls.
- **`tokprof pauses`** gets a part per agent: busy time by what the agent waited on, and the slow requests.
- The README describes the new blocks, columns and part.

## Capabilities

### New Capabilities

- `agent-time`: busy time and what it went to, slow requests, API retries, and the longest waits.

### Modified Capabilities

- `session-logs`: new requirements for request times and retries, call outcomes with their error lines, and what a wake reports, with the fields of each agent.
- `tool-breakdown`: new requirements for failed, refused and denied calls and for what the requests after a failure cost; the tools command gets two columns, a sort and a line.
- `session-xray`: the layout gets the `waits` and `failures` blocks; new requirements for both.
- `pause-report`: the report gets a part per agent on busy time.

## How it is checked on the real logs

The baseline is the study of 2026-11-03, over the 30 days to that morning. Its scripts are in /tmp/tokprof-why-study/scripts, and its numbers come from rules that differ from the specs in two places. A failure that never worked again in its turn kept its window to the end of the turn, where the specs keep only the request that carried it. It also counted waits from stand-stills, where the specs split busy time. Where these rules change a number, the check compares with the script re-run under the specs' rules.

A first draft of the specs ended a window at the next working call of the same row, the same command for a shell tool. On the real logs that stretched windows over unrelated work: 20 failed `mkdir -p` calls got $25.54 of "after", since `mkdir -p` ran again many requests later. The specs therefore end it at the next working call of the same tool, as the study and CodeBurn do.

- `tokprof --agent claude --since 30d tools` counts 1,021 failed calls and 153 refused or denied calls, give or take the calls added since. The `after` total matches the re-run script within 2%.
- `tokprof --agent claude --since 30d pauses`:
  - 3.3% of requests took a minute or more;
  - the API retries row holds 45 requests and 10.8 hours;
  - after the end of a turn, subagents hold 43 hours (26 alone plus 17 with commands) and background commands 41 hours.

  Shares may move by half a point and hours by 5%.
- `tokprof scan 3edfd2e2`:
  - the main conversation has 17 requests of a minute or more, the calls CodeBurn lists as slow;
  - prompt 2's waits are 25 minutes for a subagent and 27 minutes for background commands;
  - the longest waits name the 1h 50m background command of 2026-10-14.
- `tokprof --agent codex --since 30d tools` counts at least the 43 failed `exec` scripts and the 992 calls with a non-zero exit code.
- `tokprof --since 30d summary` takes at most 3% longer than before this change, and the X-ray of 3edfd2e2 at most 0.1 s longer.

## Out of scope

- Permission prompts. Neither agent logs them, so a wait for one counts as the time of its call.
- Time per tool row in `tokprof tools`.
- A background command that fails after it started. Claude Code reports its exit code only in the notification, so the call counts as one that worked.
- Codex polls of running commands (`write_stdin`, `wait`). They count by the activity cost-attribution gives them.
- The share export and `tokprof day`. They stay as they are.
- A hint for tools that fail often.

## Impact

- Model: requests get their send and end times, retries and the last API error; tool results get an outcome and an error line; wakes say what they report.
- Sources: the Claude Code and Codex decoders, their fixtures and READMEs.
- Core: a module for busy time; failures and the cost after them in the tool table.
- CLI: the X-ray blocks, the tools columns and sort, the pauses part.
- README.
