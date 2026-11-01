# tokprof

A token, cost and prompt-cache profiler for AI coding-agent session logs (Claude Code, Codex).
It reads the logs the agents already write. It needs no proxy and no account, and nothing leaves the machine.

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="docs/bench-dark.svg">
  <img alt="Mean cost per return to a big session after its prompt cache expired, over the next 60 requests. Claude Code, 210 returns: resume as is $8.27, /compact first $6.38, new session with a handoff note $2.58, heartbeats for up to 6 hours and then resume $6.37; the first request alone costs $3.98. Codex, 204 returns: $3.98, $2.77, $1.80 and $4.04." src="docs/bench.svg">
</picture>

## Why

A long session is cheap while its prompt cache is warm, because each request reads the context from the cache at a tenth of the input price or less. The cache lives 5 minutes or an hour in Claude Code and about 30 minutes in Codex. After a longer break, the first request writes the whole context again. For a 500k-token Claude Code session, that one request costs about $4, and every request after it still reads the old context.

`tokprof bench` measured this on one developer's logs: 210 returns to Claude Code sessions and 204 returns to Codex sessions, each with a context of 100k+ tokens. It follows each return for up to 60 requests, until the next break or compaction, and replays the same requests three other ways:
- **`/compact` first.** The compaction call writes the cold context again and outputs a summary. The requests then run on top of what a compaction leaves: a median of 53k tokens for Claude Code and 21k for Codex.
- **A new session with a handoff note.** The old session writes the note before the break, while its cache is still warm. A new session then runs the requests on top of the same base.
- **Heartbeats, then resume.** While the session is idle, a ping at 5/6 of the cache lifetime reads the context, which keeps the cache warm. That is every 50 minutes on Claude Code's 1-hour cache, for up to 6 hours after the last request. A return within the lifetime of the last ping reads its context from the cache. A later one is cold anyway, after the pings.

| Mean per return over 60 requests, API-equivalent | resume as is | /compact first | new session + handoff note | heartbeats, then resume |
|---|---:|---:|---:|---:|
| Claude Code: 210 returns, median context 488k | $8.27 | $6.38 | $2.58 | $6.37 |
| Codex: 204 returns, median context 163k | $3.98 | $2.77 | $1.80 | $4.04 |

What this means:
- A handoff note written before the break would have saved 69% of what the Claude Code returns cost ($1,195 of $1,737), and 55% for Codex ($445 of $812).
- The note was cheaper in every one of the 414 returns. The median return would have saved $5.09 (72%) in Claude Code and $2.13 (59%) in Codex.
- Once the cache is cold, `/compact` first pays off only from the 8th request in Claude Code and the 4th in Codex.
- Heartbeats would have kept 129 of the 210 Claude Code returns warm, for $88 of pings, and saved 23% ($398). They save only the cold write of the first request: every request after it still reads the whole context, so the note was cheaper in 190 of the 210 returns. In Codex they would have cost $12 more than resuming. Its cache lives 30 minutes, so the pings go out every 25 minutes, and a cache write there costs 10 to 25 reads, against 40 on Opus 5.5's 1-hour cache.

tokprof shows this for each session. The X-ray says when the cache goes cold, what the next request will cost and which way of carrying on is cheapest. Its forecast of the first request after a break was within 10% of the logged cost for 91% of the Claude Code returns and 96% of the Codex returns.

To run the bench on your own logs: `tokprof bench --svg docs/bench.svg`. The run behind this chart is in [docs/bench.txt](docs/bench.txt).

### A new session that refers to the old one

The replay assumes the new session starts from a handoff note. A live experiment showed that a reference to the old session is enough. It used the most extreme cold sessions in the logs, four of Claude Code and two of Codex: the longest, the fullest context, the biggest log, the most requests. A new session in the same agent got the old session's id, its id and log path, or a note built from the log, and a read-only task: find out what was done and what is left. Each new session:
- found the old log and pulled the prompts and messages out of it with a short script;
- named the task, the work done and what was left;
- checked the project for what had changed since.

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="docs/reference-bench-dark.svg">
  <img alt="Getting back to a cold session. Claude Code, the longest session, 12.8 days with a 724k context: resuming it $6.37 (forecast); new sessions $0.41 with the id only (59 s), $0.38 with the id and log path (53 s), $0.32 with the id and a note (41 s). The fullest context, 953k tokens: resuming $8.40; new sessions $0.50 (39 s), $0.40 (43 s), $0.32 (33 s). The biggest log with a near-full context, 658 MB and 900k tokens: resuming $7.59; new sessions $0.27 (37 s), $0.33 (52 s), $0.26 (30 s). The most requests, 11,056 over 7 days: resuming $4.27; new sessions $0.49 (29 s), $0.32 (29 s), $0.32 (33 s). Codex, the longest session, 17.9 days with a 2.7 GB log: resuming $0.60; new sessions $0.21 (88 s), $0.22 (81 s), $0.12 (42 s). The most requests, 25,297 over 5.1 days with a 656 MB compressed log: resuming $1.10; new sessions $1.03 (270 s), $0.38 (149 s), $0.34 (113 s)." src="docs/reference-bench.svg">
</picture>

- **Claude Code.** A new session picked up the work for $0.26–0.50, in under a minute. Resuming these sessions would have cost $4.27–8.40 for the first request alone, and every request after it would read the old context again.
- **Codex.** Its sessions stay under the 258k window however long they run, so they are cheaper to resume: $0.60 and $1.10 here. With the log path, a new session on the same model still cost less: $0.22 and $0.38.
- **The reference.** The note built from the log was the cheapest in every session, by a few cents. But it holds only the start and the end of a session, and once it led to "nothing is left". With the log path, the agent reads what it needs itself.
- **The end of a log is not the whole session.** For the session with the most requests, a week of work, the answers built from the end of the log described only its last day.

`tokprof handoff` prints the line with the id and the log path, and the X-ray suggests it.

The write-up, with the prompts, the results of every variant and the limits, is in [docs/reference-bench.md](docs/reference-bench.md). The bench is in [bench/reference](bench/reference).

### One agent picking up the other's session

A separate experiment swapped the agents on the same extreme sessions, plus one more Codex session for a clean id-only run. Codex picked up Claude Code sessions, and Claude Code picked up Codex ones, from the same references.
- **Finding the log.** Each agent found the other's log by the id and read it with a short script. 18 of 19 answers got the task, the work, the last step and what was left right.
- **Codex picking up Claude Code sessions.** It cost $0.52–1.78, against $4.27–8.40 for resuming them in Claude Code. That holds although its default model, gpt-6-astra, costs about 2.5× as much per token.
- **Claude Code picking up Codex sessions.** It cost $0.24–0.50 with the log path or a note, against $0.60–1.10 for resuming them in Codex. With the id alone it first had to search `~/.codex` for the log, and cost about as much as resuming.

The runs, the way each agent found the other's log, and how the id runs were kept from seeing the other runs' prompts are in [docs/reference-cross.md](docs/reference-cross.md), with [its chart](docs/reference-cross.svg).

### What the Claude Code team says

- [Boris Cherny on Hacker News](https://news.ycombinator.com/item?id=47880089), April 2026, on why a cold return costs so much. While you work, every message but the latest is read from the cache. Once a session idles for more than an hour, the next prompt misses the cache on the whole conversation, so a 900k-token context is written to it at once. The team tried posts on X, an in-product tip to run `/clear`, and dropping parts of the context after an idle hour. Dropping old thinking worked best, and it caused the bug described in [Anthropic's postmortem](https://www.anthropic.com/research/april-23-postmortem).
- [Lessons from building Claude Code: Prompt caching is everything](https://claude.dev/blog/lessons-from-building-claude-code-prompt-caching-is-everything/), by Thariq Shihipar. The cache matches the prompt from its start, so Claude Code keeps the system prompt and tools fixed and does not switch models mid-session. Compaction runs as a fork with the same prefix and reads the conversation from the cache. tokprof prices `/compact` the same way.
- [Using Claude Code: session management and 1M context](https://claude.com/resources/articles/using-claude-code-session-management-and-1m-context), also by Thariq, on how to use sessions. A new task gets a new session. The article says when to continue, rewind, `/compact` with a hint of what comes next, `/clear` with a brief you write yourself, or hand the next step to a subagent. It also shows how "summarize from here" writes a handoff message.
- [How Claude Code uses prompt caching](https://code.claude.com/docs/en/prompt-caching), in the docs. The main conversation gets the 1-hour cache on a subscription within the plan's usage, and 5 minutes with an API key, a cloud provider or usage credits. Subagents get 5 minutes either way. `promptCacheTtl` sets the lifetime. The page also lists what breaks the cache, and says why `/compact` costs the most when you come back to an old session.

### Other ways to avoid a cold return

- **Give the session a finish line.** One task per session, with a result Claude can check itself: tests, a build, a page it opens in a browser. For very long tasks, Boris [has a Stop hook or a background agent check the work](https://x.com/bcherny/status/2007179858435281082), so Claude carries on without him. [`/goal`](https://code.claude.com/docs/en/goal) builds this in: a small model checks the condition after each turn, and Claude starts the next turn itself instead of waiting for you. A permission prompt still stops it. The cache stays warm until the task is done, and the next task starts in a new session. Such sessions are heavy, though: Boris [traced most fast usage](https://x.com/bcherny/status/2115592371224346734) to one or two sessions with xhigh effort, `/goal`, Workflows or Agent Teams.
- **Send heartbeats.** A request that reads the cache resets its timer. A ping sent before the timer runs out reads the context at the cache-read price instead of writing it again. On Opus 5.5 with the 1-hour cache, a ping to the bench's median 488k-token context costs about $0.10, and writing that context again $3.91. With a ping every 50 minutes, pings pay off for breaks of up to 33 hours. Over the median break of 3.2 hours, three pings cost $0.29.
  - [cache-tax](https://github.com/karanb192/cache-tax) is a mod that does this. `/keepwarm` before a break arms six hours of pings, one every 50 idle minutes. Each ping is a fork of the session, so the conversation does not change. A `/loop` every 50 minutes keeps the cache warm too, but each run adds a turn.
  - A heartbeat saves the cold write in the first request back, not the rest. In the bench above, heartbeats for up to 6 hours saved 23% of what the Claude Code returns cost, and 33% with `--keep-warm 24h`. The handoff note saved 69%.
  - On the 5-minute cache, pings go out every few minutes. On Opus 5.5, 25 of them cost as much as the write they save, so they pay off only for breaks under two hours. `tokprof whatif` shows what the 1-hour cache would have cost on your logs.
  - Pings need Claude Code running and the machine awake.
- **Let Claude Code compact the idle session.** Since [2.1.299](https://github.com/anthropics/claude-code/blob/main/CHANGELOG.md#21299), Claude Code compacts a terminal session of 200k+ tokens after about 54 idle minutes, while the 1-hour cache is still warm ([announcement](https://x.com/lydiahallie/status/2110484840256819380)). VS Code and the desktop app got it in 2.1.314. Sessions run through the Agent SDK, such as an IDE's agent integration, do not get it: in the logs behind the bench, it compacted 6 of 8 such breaks in the terminal and none of 132 in IntelliJ. In the bench, that is the handoff column, with the summary as the note. The session goes on from the summary instead of the full context. `"idleCompaction": false` in settings turns it off. tokprof marks these compactions: the rebuild after one is the miss cause `compaction: while idle`, and the X-ray of a session compacted since its last request says so and prices the next request from the smaller prompt.

## Install

tokprof builds with a recent stable Rust; its dependencies need 1.89 or later.

```sh
cargo install --git https://github.com/ignatov/tokprof --root ~/.local   # puts tokprof into ~/.local/bin
cargo install --path . --root ~/.local                                    # the same, from a clone
cargo uninstall tokprof --root ~/.local                                   # removes it
```

## Quick start

```sh
tokprof                     # X-ray the latest session on this machine
tokprof ui                  # browse all sessions interactively
tokprof sessions            # list session logs, newest first (--cost adds each session's cost)
tokprof scan 3f951de0       # X-ray one session by id prefix, or: tokprof scan path/to/session.jsonl
tokprof handoff 3f951de0    # the line to start a new session with, instead of resuming this one
```

`tokprof handoff` prints a line with the session's id and the path of its log:

```
Continue from the earlier session 3f951de0-…. Its log: ~/.claude/projects/…/3f951de0-….jsonl.
```

Paste it into a new session, followed by what to do next. The new agent reads what it needs from the log. In a terminal, `tokprof handoff` also copies the line to the clipboard. It takes the latest session when no session is given.

`--note` adds a note built from the log without a model: the task, the latest prompts, where the work stopped, the task list, the files changed and read, and the latest commands. Use it when the new agent cannot read the log itself, for example a compressed Codex rollout or another agent's log.

The `advice` lines come from deterministic rules in `src/advice.rs`. Each item says one thing: what it is about, the numbers behind it, and what to do. For session 3f951de0, with its cache cold:

```
advice    ▸ Resuming writes the whole 817.6k context again (+$6.38), then $0.16 per request.
          ▸ A new session that refers to this one is cheaper: ≈ $0.31 to pick up the work.
            `tokprof handoff 3f951de0` prints the line to start it with.
          ▸ /compact costs about as much as resuming (≈ $6.74 once), then ~44.2k per request.
          ▸ The context fills 82% of the window. At 12 requests per prompt, each prompt costs
            ≈ $1.99 in reads alone.
          ▸ Tool results take 45% of the context (Bash 299.9k, Read 63.9k): pipe long command
            output through tail or grep.
          ▸ Tool calls take 32% of the context (Bash 181.2k, Write 62.8k): write long inline
            scripts to a file and run the file.
          ▸ Pasted prompts (turn 1 14.6k, turn 3 29.6k) are re-read on every request: pass a
            file path instead.
          ▸ Thinking cost $16.18, 15% of the session, at effort max: use a lower effort for
            routine steps.
```

- Resuming and a new session: once the cache is cold, the first request writes the whole context again. A new session that refers to this one is compared with that, and when it is cheaper, the X-ray gives the `tokprof handoff` command that prints the line to start it. The pickup cost starts from the smaller of the session's first prompt and its prompt after a compaction, and adds what the reference experiment measured.
- `/compact`: on a cold cache it costs about as much as resuming, because the compaction call writes the context again too. After it, each request reads the summary instead of the context.
- While the cache is warm, the first items say how long it stays warm, what `/compact` costs before a break, and what resuming or a new session would cost after one.
- The window: what the context costs per prompt at the session's pace, and when auto-compaction is close.
- Tool results, tool calls, reminders and pasted prompts: what fills the context, with up to two leading sources and a hint for the one that holds the most. All of it is read again on every request, which is why trimming it pays.
- Thinking and cache misses: the thinking cost at the effort level used most, and the main cause of cache misses with what they could have saved.

`tokprof ui` shows the sessions on the left and the X-ray of the selected one on the right. Costs fill in while it runs. The list follows the log directories:
- a new session shows up at the top;
- a session that is still being written moves to the top, and its cost and X-ray are computed again. The X-ray title shows `updating…` until the new one is ready;
- a session whose log is deleted leaves the list.

A session that keeps growing is read again after a pause as long as its last read took, so a big log being written uses at most half of a core.

The list's last column is the cost of one more request and its share of the session's cost so far. Green means the cache is warm, red means it is cold and the request re-writes the whole context. `tokprof sessions --cost` shows the same column.

| Key | Action |
|---|---|
| `↑` `↓`, `j` `k`, PgUp PgDn, `g` `G`, Home End | Move through the sessions |
| `⏎`, `→` or `l` | Show the X-ray full screen; `↑` `↓`, `j` `k`, PgUp PgDn and space then scroll it |
| `←`, `h`, `esc` or backspace | Go back to the list |
| `/` | Filter by project, agent or id; `esc` clears it |
| `s` | Sort by time, cost, or next request |
| `f` | Write the session's JFR for IntelliJ Profiler to `~/tokprof/<id>.jfr`, or into `$TOKPROF_DIR` if set |
| `o` | Write the session's HTML flame graph to `~/tokprof/<id>.html`, or into `$TOKPROF_DIR`, and open it in the browser |
| `c` | Copy the `tokprof handoff` line for the session, to continue it in a new session |
| `q`, Ctrl+C, or `esc` in the list | Quit |

The X-ray shows:
- the session's title;
- how full the context window is;
- whether the prompt cache is still warm, and what the next request will cost: warm, and once the cache is cold, as a share of what the session has cost so far. When the agent compacted the session after its last request, the `cache` line says when and how, as in `compacted while idle at 19:13 · the next request re-builds ~55.8k`, and the forecast prices that smaller prompt;
- spend by cost component;
- where the money went: by origin, such as a skill, a subagent type or typed prompts, and by task, such as the session title or what each subagent was asked to do (see [Origin and task](#origin-and-task));
- what the context holds now and its largest blocks;
- the content that cost the most over the session;
- cache misses with their causes.

## Reports over all sessions

```sh
tokprof summary             # spend by model and component, by origin and task, misses by cause, reconciliation
tokprof misses --top 20     # most expensive cache misses (--expected adds compactions and cleared tool results)
tokprof whatif              # Claude input cost under 5m vs 1h cache lifetimes
tokprof bench               # cold returns to big sessions: resume vs /compact vs a new session vs heartbeats
tokprof prices              # prices in effect and where each came from
```

`tokprof bench` takes these options:
- `--requests N` follows each return for up to N requests. The default is 60.
- `--min-context T` counts only returns to contexts of at least T tokens. The default is 100000.
- `--keep-warm 6h` sends heartbeats for up to this long after the last request, in hours or minutes (`90m`). The default is 6h, as in the cache-tax mod.
- `--svg FILE.svg` writes the chart to FILE.svg, and a dark-theme version to FILE-dark.svg.

The bench replays the requests that followed each return; only the old conversation in each prompt shrinks to the base. Claude Code does not log the compaction call. tokprof models it as a fork that shares the conversation's cache, which is how Claude Code runs it, so on a cold cache it writes the context again and then outputs a 10k-token summary. The handoff note is 10k tokens, written while the cache was warm.

Heartbeats go out at 5/6 of the cache lifetime: every 50 minutes on a 1-hour cache, every 4 minutes 10 seconds on a 5-minute one, and every 25 minutes in Codex. Each reads the context at the price of its time and outputs 100 tokens. A return within the lifetime of the last ping reads the previous prompt from the cache and writes only what is new; the requests after it run as logged. For Codex this assumes a ping resets OpenAI's cache timer as it resets Anthropic's.

Every command but `tokprof prices` takes these filters:

| Filter | Example |
|---|---|
| `--agent` | `claude,codex` |
| `--since` | `2026-10-01`, or `7d` for the last seven days |
| `--until` | `2026-10-16`, excluded |
| `--project` | text the project path contains, such as `billing`; a path under the home directory works with or without `~` |
| `--session` | the start of a session id, with its subagents |
| `--prices` | a price file, `FILE.toml` |

The reports count the requests the filters select. `tokprof`, `scan`, `handoff`, `sessions` and `ui` pick or list the sessions they select: `--since` keeps sessions written since that day, `--until` sessions started before it. An X-ray still covers its session's whole log.

A session id can be shortened to its start. Codex ids are shown by their first three groups, such as `01a197e1-4d9f-76b1`, because threads launched together share the first two. A start that several sessions share is an error that lists them.

## Flame graphs and profiles

`tokprof flame` writes a JFR into `~/tokprof`, or into `$TOKPROF_DIR` if set. The file is named after the filters: `all.jfr`, `3f951de0.jfr` for `--session 3f951de0`, `billing-since-7d.jfr`. Existing files are overwritten.

`-o FILE` writes elsewhere, and its extension picks the format:

| Output | Open with |
|---|---|
| `x.jfr` | IntelliJ IDEA: Profiler > Open snapshot. Each session is a thread. One sample is $0.001, or 100 tokens, and ten times more as long as the profile would hold over 2 million samples. |
| `x.html` | Any browser. This is async-profiler's flame graph. |
| `x.collapsed` | async-profiler's `jfrconv`, for example `jfrconv --diff before.collapsed after.collapsed diff.html` |
| `x.pb.gz`, `x.pprof` or `x.pb` | `go tool pprof -http=: x.pb.gz` |
| other | Folded stacks for speedscope or flamegraph.pl. |

Options:
- `--stack` sets the stack dimensions, root first. Available: `agent`, `project`, `session`, `chain`, `origin`, `task`, `model`, `turn`, `component`, `cause`, `kind`, `tool`, `target`. Default: `agent,project,session,chain,turn,kind,tool,target,component`.
- `--value` picks the measure: `cost` (micro-dollars), `tokens`, or `avoidable` (the miss premium).
- `--component` keeps only some components: `read,write,rewrite,uncached,output`.
- `--threads` applies to `.jfr` and sets the dimensions that name a thread. Default: `agent,project,session`.

Examples:

```sh
tokprof flame                                                         # ~/tokprof/all.jfr, by session
tokprof --session 3f951de0 flame                                      # ~/tokprof/3f951de0.jfr
tokprof flame --threads agent --stack kind,tool,target,component -o ~/tokprof/content.jfr
tokprof flame --value avoidable --component rewrite --stack cause,project,session,kind,tool,target -o misses.html
tokprof --since 30d flame --stack origin,task,kind,tool,component -o work.html   # what kind of work the money went to
```

## How cost is attributed

Each request is charged by prompt position, the way prefix caching bills:
1. The first `cache_read` tokens at the read rate.
2. The next `cache_write` tokens at the write rate.
3. The rest at the full input rate.

Every block of the prompt gets the share that falls on it, in every request it is part of. A file read once therefore pays for as long as it stays in the context. Output is charged to what the response produced.

A request misses when it re-processes more than 5% and at least 2,000 tokens of the prefix the previous request had cached. That is Claude Code's own rule. The re-written span is charged as `rewrite after miss`, with a cause and the avoidable premium.

Coverage depends on what the logs contain:
- **Claude Code 2.1.300 and later** logs its requests, so prompts are rebuilt exactly. Older transcripts are attributed at the request level only and show up as `unknown` content.
- **Codex** prompts are rebuilt from the rollouts. The system prompt comes from `session_meta`, and every `response_item` is one conversation item: messages, reasoning, tool calls and their output, and `additional_tools`. A compaction swaps in its `replacement_history`.
  - Images count as a fixed size.
  - Tool schemas other than `additional_tools` are not logged.
  - `world_state` repeats the messages, so it is not counted.
  - Compressed rollouts (`.jsonl.zst`, in `sessions` and `archived_sessions`) are read too.
  - Subagent threads join their parent through `parent_thread_id`.

## Origin and task

Each request gets two labels from the logs, with nothing to set up:
- its **origin**, the kind of work it was part of;
- its **task**, what the work was.

`tokprof summary` sums whole requests by both, the X-ray shows them for one session, and `--stack origin,task` puts them into flame graphs.

The origin is the first of these that applies:
1. `skill:<name>`, `mcp:<server>` or `plugin:<name>`: what Claude Code attributes the response to (`attributionSkill`, `attributionMcpServer`, `attributionPlugin`). The subagents a skill starts carry the skill too.
2. `agent:<type>`: the requests of a subagent.
   - In Claude Code the type is `agentType` from `subagents/agent-<id>.meta.json`.
   - In Codex it is the thread's role: `guardian` for auto-review, `review` for `/review`, and `default` for a thread spawned without a role.
3. `command:/<name>`: a turn the user started with a slash command.
4. A turn that starts with a background task's result (`<task-notification>`) belongs to what started the task:
   - the skill of the request that started the subagent;
   - otherwise `agent:<type>`;
   - `notification` for a background shell command.
5. `prompt`: a turn the user typed. An interruption, the summary after a compaction and `!` shell commands carry on the turn before them.

The task is what a subagent was asked to do, else the session title, else `untitled <id>`:
- **Claude Code.** A subagent's task is the `description` in its meta file. The title is the latest `custom-title`, or else the latest `ai-title`, without the `[archived]` mark.
- **Codex.** A spawned thread's task is the end of its `agent_path`: `/root/fix_flaky_tests` becomes `fix flaky tests`.
  - The title is the `thread_name` in `~/.codex/session_index.jsonl`, the parent's for a subagent thread.
  - Codex writes it for threads started in its app and TUI. Threads from IDE plugins and `codex exec` stay untitled.

A subagent's chain is named after its task, for example `subagent: Find handlers`.

These fields were seen in Claude Code 2.1.284–2.1.312 and Codex 0.146–0.163.

## Prices

The built-in prices are in `prices.toml`, with sources and dates. To add or override models, put entries in `~/.config/tokprof/prices.toml`:

```toml
[models."codex-auto-review"]
input = 2.00
cache_write = 2.50
cache_read = 0.10
output = 10.00
```

Prices change over time, so a model can list periods, and each request is priced by the one in effect when it was sent. `until` is excluded:

```toml
[[models."gpt-5.6-sol"]]
since = 2026-08-21          # the 2026-08-21 price cut
input = 4.00
cache_read = 0.40
cache_write = 5.00
output = 20.00

[[models."gpt-5.6-sol"]]
until = 2026-08-21          # launch prices
input = 5.00
cache_read = 0.50
cache_write = 6.25
output = 30.00
```

`tokprof prices` lists every period; `tokprof prices --at 2026-08-15` shows the prices in effect on that day. The forecast of the next request uses today's prices.

Dollar figures are API-equivalent. On a subscription they measure usage, not a bill.

## License

MIT, see [LICENSE](LICENSE). `assets/async-profiler/flame.html` is async-profiler's flame graph page, under the [Apache License 2.0](assets/async-profiler/LICENSE).
