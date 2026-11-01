# Reference bench

How Claude Code and Codex carry on from an old session when a new session gets only a reference to it, and what that costs against resuming the old session. The write-up is [docs/reference-bench.md](../../docs/reference-bench.md). The runs in which one agent picks up the other's session are in [docs/reference-cross.md](../../docs/reference-cross.md).

For each old session, `run.py` starts three new headless sessions with the same read-only task: find out what we did in the old session, what is done and what is left, and answer in at most 15 lines. Each new session gets a different reference in front of the task:

| Variant | The new session gets |
|---|---|
| `id` | the old session's id |
| `path` | the line `tokprof handoff` prints: the id and the path of the log |
| `note` | what `tokprof handoff --note` prints: the line and a note built from the log without a model |

The new sessions run in the old session's project, with the agent's default model or the one given after the log as `@model`. Other settings are the defaults:
- Claude Code runs as `claude -p` with `--permission-mode dontAsk`. It may use Read, Grep, Glob, Bash and subagents, but not Edit or Write, and it has a budget of $5.
- Codex runs as `codex exec --sandbox read-only`.

The `id` runs go first and alone. Each prompt is written only when its runs start. So an `id` run that searches for the id cannot come across the other prompts, which name the log. An `id` run whose tool results still show such a prompt is marked `contaminated`.

tokprof then measures each new session from its own log. It also records its forecast of resuming the old session.

```sh
python3 -I bench/reference/run.py ~/tokprof/reference-bench/$(date +%F) \
  ~/.claude/projects/<project>/<session>.jsonl \
  ~/.codex/sessions/<yyyy>/<mm>/<dd>/rollout-<...>.jsonl.zst@gpt-5.6-sol
python3 -I bench/reference/chart.py docs/reference-bench.svg bench/reference/results/*.json \
  --sessions claude-0f54de16,claude-32c500c2,claude-7bb3ca19,claude-bec2995a,codex-01a0eec6,codex-01a0b3f1
```

- `--variants id` runs only some variants, for example to repeat one that failed for reasons outside the agent.
- `--with claude|codex` starts the new sessions in that agent instead of the old session's own.
- `--sessions` picks the old sessions by name, `<agent>-<first 8 characters of the id>`, and sets their order. `NAME:AGENT` charts the runs in which that agent picked up the session.
- `chart.py` skips rows marked `failed` or `contaminated`, and charts the repeat.
- A `pick` in a session's `resume` row says why the session is in the bench, for example "the longest session". The chart puts it in the title of the session's group.

For every run, the output directory gets:
- `<agent>-<id>-<variant>.prompt.md`, where `<id>` is the first 8 characters of the old session's id: the prompt;
- `.answer.md`: the final answer;
- `.xray.txt`: `tokprof scan` of the new session;
- `.trace.txt`: every tool call in order;
- `.stdout`: the CLI output.

`results.json` has one row per run with spend, requests, final context, wall time and tool calls, and a `resume` row per old session with tokprof's forecast. `chart.py` draws it as an SVG, with a dark-theme version next to it.

The runs are real sessions. They cost money, or subscription usage, and they show up in the agents' session lists.

`sessionlog.py LOG` prints a session's agent, id and project, and every tool call in order. It reads Claude Code transcripts and Codex rollouts, `.jsonl` or `.jsonl.zst`. `run.py` uses it to find the old sessions and to trace the new ones.

`results/` keeps the numbers of each run:
- `2026-11-01-claude.json`: the four Claude Code sessions of the write-up;
- `2026-11-01-long.json`: its two Codex sessions;
- `*cross*.json`: the runs in the other agent.

The answers are not kept there, because they quote the work of the old sessions.
