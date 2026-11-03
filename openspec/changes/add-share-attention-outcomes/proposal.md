## Why

A survey file now says where the money went and what the cache lost, but not the questions the survey is for: how a person and the agents spend their time together, whether coming back to a cold session or starting a new one is cheaper for other people too, why requests wait for retries, and what the agents produced. The report is also hard to read: it has no summary, and its "active hour" counts the hours an agent worked, not the hours a person sat at it. The first wave goes out soon, and a file without these parts cannot be filled in later.

## What Changes

- **New parts in the share file** (additive, format 2):
  - `rhythm`: the person's time with the agents, as the rhythm counts it (see work-rhythm). It holds the time at the keyboard, the sittings, how fast the person answered an end of turn and a question, how long the agent's turns took, the ends of turn while the person was there, and what the agents did after the person left. The file holds it for all agents together and, in short, per client.
  - `bench`, per agent: the verdict of `tokprof bench` with its defaults. It holds the cold returns, what each way of coming back would have cost (resume, /compact first, a handoff note, heartbeats, heartbeats then compaction), and in how many returns each way was the cheapest.
  - `outcomes`, per agent and client: the commits, pushes and pull requests the agents made, counted from their shell calls that did not fail. Only these counts go in, no arguments.
  - `retried_by_error` in `time`: the requests that waited for retries, by the class of their last error (server error, rate limit, authentication, timeout, connection, other). No error text goes in.
- **The report** (`tokprof share` and `tokprof pool`):
  - `At a glance`, after the header: six lines on spend, what the cache lost, cold returns, problems, the person's time and what the agents made.
  - The header line `activity` also gives the hours at the keyboard and the dollars per hour at the keyboard.
  - A one-line note under every section title that says what its numbers mean.
  - New sections:
    - `Coming back after a cold break`: the bench verdict;
    - `Your time with the agents`;
    - `Working with the agent by client`, such as an IDE against the terminal;
    - `Commits and pull requests`.
  - `Working with the agent` counts per hour at the keyboard instead of per active hour.
  - `Problems` splits the requests that waited for retries by error class.
  - Counts of 1,000 or more have thousands separators in every table, the tool and version tables too.
- **The survey guide**: the prompt's command carries `--mcp-names <mcp servers>`, so that the IDE's MCP servers are counted by name. The guide says to drop it when no server should be.

## Capabilities

### New Capabilities

### Modified Capabilities

- `share-export`: the rhythm, bench, outcome and retry-class parts.
- `share-pool`: `At a glance`, the section notes, the four new sections, the activity line, `Working with the agent` per hour at the keyboard, `Problems` by error class, thousands separators, the order of the sections, and the survey guide's command.

## Impact

- Core: the share collector and the pool. The neutral command parser in the core finds commits, pushes and pull requests in a shell command; it already finds the class of a call.
- Sources: both log sources pass each shell command to that parser, as they do for the class of a call. They read no new log field.
- CLI: `tokprof share` builds the rhythm timelines and runs the bench scan in the same pass over the logs.
- Docs: README (the share fields and the pool sections) and `docs/survey.md`.
- Order: this change modifies requirements that `add-team-survey` and `add-share-friction` add. It is archived after both of them.

How it is checked on the author's 30 days (2026-10-04 to 2026-11-03):
- The rhythm parts equal the rhythm section of `tokprof --since 30d pauses`: the sittings, the hours at the keyboard, the median answer and question answer, and the `left` medians.
- The bench parts equal `tokprof --since 30d bench`. For Claude Code that is 203 returns: resume $1,644, /compact first $1,273, handoff note $518, heartbeats $1,268, heartbeats then compaction $809. The note is cheapest in 203 of 203. For Codex it is 18 returns and $77.89 to resume.
- The retry classes add up to the 92 requests that waited for retries. The raw logs hold 482 API errors, 380 of them 502s; the share of server errors is compared with that.
- The outcome counts equal a script count of the same shell calls in the logs, and stay at or under `git log` in the repositories they ran in.
- `share` takes at most 10% longer than its 24.9 s, the file stays under 150 KB, and the words outside tokprof's own are the same as before.

## Out of scope

- Linking the files of one person across waves, such as with an install id.
- New log sources (Junie, OpenCode).
- Outcomes beyond the agents' own commits, pushes and pull requests: merged pull requests, tests, lines changed, or the person's own commits.
- A questionnaire beside the file.
- Changes to `tokprof bench` or its defaults, and a bench verdict per client.
- Pushing and tagging the release.
