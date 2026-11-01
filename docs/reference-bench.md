# Picking up a cold session by reference

A big session whose prompt cache has expired is expensive to resume: the first request writes the whole context again. This experiment asks what a new session costs instead, when all it gets is a reference to the old one. It ran on 2026-11-01, with Claude Code 2.1.311 and Codex 0.164.0 on one developer's machine. It used the most extreme cold sessions in that machine's logs, four of Claude Code and two of Codex, each the most extreme by one measure. Each new session ran in the same agent as the old one. Picking up a session in the other agent is covered in [reference-cross.md](reference-cross.md).

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="reference-bench-dark.svg">
  <img alt="Getting back to a cold session. Claude Code, the longest session, 12.8 days with a 724k context: resuming it $6.37 (forecast); new sessions $0.41 with the id only (59 s), $0.38 with the id and log path (53 s), $0.32 with the id and a note (41 s). The fullest context, 953k tokens: resuming $8.40; new sessions $0.50 (39 s), $0.40 (43 s), $0.32 (33 s). The biggest log with a near-full context, 658 MB and 900k tokens: resuming $7.59; new sessions $0.27 (37 s), $0.33 (52 s), $0.26 (30 s). The most requests, 11,056 over 7 days: resuming $4.27; new sessions $0.49 (29 s), $0.32 (29 s), $0.32 (33 s). Codex, the longest session, 17.9 days with a 2.7 GB log: resuming $0.60; new sessions $0.21 (88 s), $0.22 (81 s), $0.12 (42 s). The most requests, 25,297 over 5.1 days with a 656 MB compressed log: resuming $1.10; new sessions $1.03 (270 s), $0.38 (149 s), $0.34 (113 s)." src="reference-bench.svg">
</picture>

In short:
- **Every new session picked up the work from a reference alone.** It found the old log and pulled what it needed out of it with a short script, even from a 2.7 GB log, a 658 MB one and a compressed 656 MB one.
- **For the four Claude Code sessions, a new session cost $0.26–0.50 and took 29–59 seconds.** Resuming them would have cost $4.27–8.40 for the first request alone. After that, every request would read the old context again.
- **Long Codex sessions are cheap to resume.** Auto-compaction keeps their context under the 258k window, however long they ran. A new session on the same model, given the log path, still cost less: $0.22 against $0.60, and $0.38 against $1.10.
- **With only the id, Codex paid for searching.** For the compressed 656 MB log, it had to find the log first and then decompressed it eight times. That cost $1.03 and four and a half minutes, about as much as resuming. Claude Code found its logs by the id at once.
- **The answers were about equally right with every reference:** 22, 22 and 21 facts of 24 for the id, the log path and the note.
  - In the session with the most requests, the Claude Code answers with the log path and with the note described only its last day of seven.
  - Once the note led Codex to "nothing is left" and missed two open items.
- **The note was the cheapest reference in every session, by a few cents.** With it, Claude Code did not open the log at all. The log path is still the safe choice, because the agent reads the log for itself. `tokprof handoff` prints that line, and `--note` adds the note.

## The sessions

| Agent | Session | Picked as | Project | Ran | Requests | Context at the end | Log | Resuming, tokprof's forecast |
|---|---|---|---|---|---|---|---|---:|
| Claude Code | 0f54de16 | the longest | ~/src/monorepo | 12.8 days | 293, 2 subagents | 724k, 72% of the window | 7.9 MB | $6.37 |
| Claude Code | 32c500c2 | the fullest context | ~/src/billing | 2.2 days | 1,981, 13 subagents | 953k, 95% of the window | 28.5 MB | $8.40 |
| Claude Code | 7bb3ca19 | the biggest log with a near-full context | ~/src/research | 1.5 days | 1,648 | 900k, 90% of the window | 658 MB | $7.59 |
| Claude Code | bec2995a | the most requests | ~/src/billing | 7.0 days | 11,056, 93 subagents | 530k, 53% of the window | 60.8 MB | $4.27 |
| Codex | 01a0eec6 | the longest | ~/src/firmware | 17.9 days | 15,541, 155 subagents, 185 compactions | 235k, 91% of the window | 2.7 GB | $0.60 |
| Codex | 01a0b3f1 | the most requests | ~/src/compiler | 5.1 days | 25,297, 240 compactions | 217k, 84% of the window | 656 MB, `.jsonl.zst` | $1.10 |

Each one is the most extreme by its measure among the 266 Claude Code sessions on the machine and the 14 Codex rollouts over 5 MB:
- 0f54de16 is also the longest because of a pause: 12 of its 12.8 days passed between two requests.
- The biggest Claude Code log, 2.2 GB, ends with a 47k context. That session is cheap to resume, so it is left out. 7bb3ca19 has the biggest log among the sessions near the end of their window.
- The second-longest Codex session ran 9.1 days, but its log is smaller than 20 MB. 01a0b3f1 has the most requests and the second-biggest log.

All six had been cold for days or weeks. The forecast is the X-ray's cost of the next request, which writes the whole context again, at the session's own model:
- claude-opus-5-5 for the four Claude Code sessions. After the first request, each request reads the context again for $0.11–0.19.
- gpt-6.1-sol for 01a0eec6, its last model of four.
- gpt-5.6-sol for 01a0b3f1.

Each of the two Codex sessions cost about $2,200 over its life.

## How it ran

For each old session, [run.py](../bench/reference/run.py) started three new headless sessions in the old session's project:
- The Claude Code id runs went first and alone, so that their search could not find the prompts of the other runs, which name the log. The log path and note runs followed in parallel.
- The Codex runs ran all three at once. Their id runs found the log by its file name.

The new sessions could only read:
- Claude Code: `claude -p … --output-format json --permission-mode dontAsk`. It could use Read, Grep, Glob, Bash and subagents, but not Edit or Write, within a budget of $5.
- Codex: `codex exec --sandbox read-only --json`.

Each new session ran on the old session's model: claude-opus-5-5 for Claude Code, which is its default `opus[1m]`, and gpt-6.1-sol and gpt-5.6-sol for Codex. So its cost compares with the resume forecast directly. Reasoning effort stayed at the default maximum.

Every new session got the same task:

> Разберись, чем мы в ней занимались, что уже сделано и что осталось сделать, и ответь коротко, не длиннее 15 строк. Ничего не меняй: ни файлы, ни git, ни задачи в трекере.

In English: "Find out what we did in it, what is done and what is left, and answer briefly, in at most 15 lines. Change nothing: no files, no git, no tracker issues."

In front of the task, each session got a different reference:

| Variant | The new session gets |
|---|---|
| `id` | `Continue from the earlier session <id>.` |
| `path` | the line `tokprof handoff` prints: `Continue from the earlier session <id>. Its log: <path>.` |
| `note` | what `tokprof handoff --note` prints: the line and the note |

The note is built from the log without a model. It holds the task, the latest prompts, where the work stopped, the task list, the files changed and read, and the latest commands.

tokprof measured every new session from its own log. The answers were checked against four facts of each old session: the task, the main work, the last step, and what was left.

## Results

| Old session | Reference | Model | Time | Requests | Tool calls | Final context | Cost, API-equivalent | Facts right |
|---|---|---|---:|---:|---:|---:|---:|---:|
| Claude Code 0f54de16, the longest | id | claude-opus-5-5 | 59 s | 6 | 5 | 53.8k | $0.41 | 4 of 4 |
| | path | | 53 s | 5 | 4 | 51.6k | $0.38 | 4 of 4 |
| | note | | 41 s | 2 | 1 | 49.5k | $0.32 | 4 of 4 |
| Claude Code 32c500c2, the fullest context | id | claude-opus-5-5 | 39 s | 5 | 4 | 50.9k | $0.50 | 3 of 4 |
| | path | | 43 s | 4 | 3 | 54.3k | $0.40 | 4 of 4 |
| | note | | 33 s | 3 | 2 | 48.3k | $0.32 | 4 of 4 |
| Claude Code 7bb3ca19, the biggest log | id | claude-opus-5-5 | 37 s | 6 | 5 | 35.3k | $0.27 | 4 of 4 |
| | path | | 52 s | 5 | 4 | 43.3k | $0.33 | 4 of 4 |
| | note | | 30 s | 4 | 3 | 40.1k | $0.26 | 4 of 4 |
| Claude Code bec2995a, the most requests | id | claude-opus-5-5 | 29 s | 4 | 3 | 52.8k | $0.49 | 3 of 4 |
| | path | | 29 s | 3 | 2 | 49.3k | $0.32 | 2 of 4 |
| | note | | 33 s | 3 | 2 | 48.6k | $0.32 | 2 of 4 |
| Codex 01a0eec6, the longest | id | gpt-6.1-sol | 88 s | 8 | 7 | 54.0k | $0.21 | 4 of 4 |
| | path | | 81 s | 8 | 7 | 56.4k | $0.22 | 4 of 4 |
| | note | | 42 s | 4 | 3 | 36.8k | $0.12 | 4 of 4 |
| Codex 01a0b3f1, the most requests | id | gpt-5.6-sol | 270 s | 27 | 26 | 72.0k | $1.03 | 4 of 4 |
| | path | | 149 s | 9 | 8 | 44.9k | $0.38 | 4 of 4 |
| | note | | 113 s | 9 | 8 | 33.4k | $0.34 | 3 of 4 |

Notes on the table:
- The costs are tokprof's, from each new session's own log. For the Claude Code runs, Claude Code's own `total_cost_usd` is within 2 cents.
- The 32c500c2 `id` answer described only the last two days, a part of the report and a triage. It left out the task: making the repository shareable with colleagues.
- The bec2995a `path` and `note` answers described only the last of the seven days, the triage criteria, and left out the audit the session was about. The `id` answer covered the whole week but said nothing was left. It missed the open questions about the criteria that the session had recorded in the project's journal.
- The 01a0b3f1 `note` answer took the note's last message as the state: the demo was done and pushed. It missed two checklist items left open the day before. The `id` and `path` answers found them in the log.
- The first 01a0b3f1 `id` run stopped after 35 requests and $1.54, before an answer. The model gateway had started to answer 401 when it failed to refresh its token. The table shows the repeat. Both runs are in the results.

## How the agents worked

- **Both agents know where their logs are.** With only an id, Claude Code went straight to `~/.claude/projects/<project>/<id>.jsonl` in all four sessions. Codex searched its tool list for a session tool, then listed `~/.codex` and found the rollout.
- **Neither reads the log as is.** Each wrote a short script that pulls the user prompts and the agent's messages out of the JSONL. Claude Code used jq, bun or Python, and Codex used Python.
  - Claude Code streamed the 658 MB log through Python and answered in 37 seconds.
  - Codex streamed the 2.7 GB log through Python without loading it.
  - Codex read the compressed log with `zstdcat | jq`, one pass over the 656 MB per question. That is why those runs took longest. With only the id, it first searched `~/.codex` for the rollout and then made eight passes.
- **They checked the project too.** They read the project's own state: git, the journal, the research notes, the last run reports. So they saw what had changed since the old session.
- **With the note, Claude Code read only the project.** It did not open the log in any of the four sessions, so the note was its cheapest reference. Codex did the same for the 2.7 GB log and answered in half the time. For the compressed log, it opened the log anyway.
- **The end of a log is not the whole session.** bec2995a is a week of 11,056 requests:
  - the log path run printed the log's messages but kept only the last 20 KB of them;
  - the note run read the note, whose prompts are the latest ones, and the journal's latest entries;
  - so both described the last day;
  - the id run listed every prompt from the start and described the whole week.

## What changed in tokprof

- **`tokprof handoff [session]`** prints the line to start a new session with: the session's id and the path of its log. In a terminal it also copies the line to the clipboard. `--note` adds the note built from the log. It reads 2.7 GB in under 2 seconds and the 656 MB `.zst` in 7.
- **The X-ray** now says, for a big session whose cache is cold, that a new session which refers to it is the cheapest way on. It gives that session's pickup cost and the `tokprof handoff` command. While the cache is warm, it gives the same numbers for after a break.
- **`tokprof ui`** copies the line for the selected session with `c`.
- **The X-ray reads a session that spans a Claude Code update as one conversation.** 0f54de16 ran before Claude Code wrote request-audit lines and was resumed after. Its main conversation sat in two chains, and the X-ray read only the later one: three requests, the first a cold re-write of 721k tokens. So it forecast $8.70 instead of $6.37, and estimated the pickup at $6.32, about as much as resuming. The measured pickups were $0.32–0.41.

The pickup cost in the X-ray uses what the reference runs measured. A new session starts from the smaller of the old session's first prompt and its prompt after a compaction. The second bound matters for 01a0b3f1, which was forked from another session and started at 171k tokens. On top of that start, Claude Code takes in about 8k tokens over 4 requests and writes about 2k of output. Codex takes in about 30k over 6 requests and writes about 4k.

The estimate is for the line `tokprof handoff` prints, with the log path. For these sessions it compares with the measured runs like this:

| Old session | The X-ray's estimate | Measured with the log path |
|---|---:|---:|
| Claude Code 0f54de16 | $0.41 | $0.38 |
| Claude Code 32c500c2 | $0.46 | $0.40 |
| Claude Code 7bb3ca19 | $0.31 | $0.33 |
| Claude Code bec2995a | $0.33 | $0.32 |
| Codex 01a0eec6, gpt-6.1-sol | $0.19 | $0.22 |
| Codex 01a0b3f1, gpt-5.6-sol | $0.41 | $0.38, and $1.03 with the id only |

## Limits

- Each variant ran once. The gaps to resuming are clear, but the differences between the variants are within what a rerun could change.
- The task only reads. A session that goes on working pays for its own new context on top of the pickup.
- One reader graded the answers, against four facts per session.
- The Codex runs ran an hour and a half before the Claude Code runs, with the same prompts.

## Running it again

```sh
python3 -I bench/reference/run.py ~/tokprof/reference-bench/$(date +%F) <log>[@model]...
python3 -I bench/reference/chart.py docs/reference-bench.svg bench/reference/results/*.json \
  --sessions claude-0f54de16,claude-32c500c2,claude-7bb3ca19,claude-bec2995a,codex-01a0eec6,codex-01a0b3f1
```

`@model` runs the new sessions on that model, for example the old session's own. The runs are real sessions. They cost money, or subscription usage, and they show up in the agents' session lists. The answers stay in the output directory, next to each run's prompt, X-ray and tool-call trace. The numbers are in [bench/reference/results](../bench/reference/results): 2026-11-01-claude.json for Claude Code and 2026-11-01-long.json for Codex.
