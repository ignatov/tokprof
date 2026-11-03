# Design

## Context

See proposal.md, Why. Today the repository is one package with a `lib.rs` and a `main.rs`.

### How agent logic leaks today

- **Cache rules by agent:**
  - `analysis/cache.rs` picks the cache lifetime by agent, and allows the 20-block lookback only for Claude Code.
  - `report.rs` runs whatif only for Claude Code.
- **Tool and reminder names:** `advice.rs` matches Claude Code and Codex tool names, and the reminder labels.
- **Log parsing outside the adapters:**
  - `handoff.rs` parses both log formats line by line, with `TodoWrite` and `update_plan`;
  - `xray.rs` reads the first lines of both formats in `quick_meta`, and guesses the agent of a path.
- **Constants per agent:**
  - `model.rs` sets the short-id length per agent;
  - `advice.rs` sets the pickup calibration per agent;
  - `bench.rs` and `handoff.rs` print the agents' names.
- **Presentation:** `ui.rs` picks a row color per agent. `report.rs` books spend of an unparsed agent name to Claude Code.

### What the next two agents log

The other two planned agents were checked read-only on this machine on 2026-11-02. No adapter is built for them in this change; the findings shape the interface.

| | OpenCode 1.18.16 | Junie (builds 3579.5–3893.5 installed) |
|---|---|---|
| Where | SQLite `~/.local/share/opencode/opencode.db` | `~/.junie/sessions/<session id>/events.jsonl` and `state.json`; index `~/.junie/sessions/index.jsonl` (`sessionId`, `projectDir`, `taskName`, `createdAt`, `updatedAt`) |
| A session is | a row of `session`, with rows in `message` and `part` | a directory of files |
| Usage | `message.data`: `providerID`, `modelID`, `tokens.input`, `tokens.output`, `tokens.reasoning`, `tokens.cache.read`, `tokens.cache.write`, `cost` | `SessionA2uxEvent` → `event.agentEvent` of kind `LlmResponseMetadataEvent` → `modelUsage[]`: `model`, `cost`, `inputTokens`, `cacheInputTokens`, `cacheCreateTokens`, `outputTokens` |
| API of a request | logged per message (`providerID`; here `opencode`, model `big-pickle`) | not logged; the 963 usage entries name Qwen (871), OpenAI gpt-4.1 and gpt-5.4 (69) and Gemini (23) models |
| Own cost | per message, and a total per session | per usage entry |
| Subagents | `session.parent_id` | `agent.kind` `MainAgent` or `CustomAgent`, in the same file |
| Compaction | `session.time_compacting` | not seen |
| On this machine | 2 sessions | 227 sessions |

Five lessons for the interface:
1. **A session is not always one file.** It can be a directory (Junie) or rows of a database (OpenCode).
2. **One agent sends requests to several APIs.** Junie used three. OpenCode logs the API per message. So cache rules cannot follow the agent.
3. **Agents report their own cost per request,** not only per session as Claude Code's cost counter does.
4. **One log event can carry the usage of several models** (Junie's `modelUsage[]`).
5. **Cache writes may come without a lifetime split** (OpenCode, Junie), as Claude Code requests without `cache_creation` do today.

### An IDE host's log of every ACP session

An IDE host on this machine runs ACP agents in the IDE: Claude Code (claude-agent-acp), Codex (codex-acp), Junie, agents from the ACP registry such as GitHub Copilot, and cloud agents. It writes one log per thread, in one format for every agent. This was checked read-only on 2026-11-02 against the host's description of that format and its logs.

- **Where.** `<IDE system dir>/acp/projects/<hash of the project>/threads/<thread id>/events.jsonl`. On macOS the system directory is under `~/Library/Caches`. This machine has two IDE versions, with 351 threads (3.4 GB) and 101 (864 MB).
- **Records.** Every line has `id`, `threadId`, `seq`, `timestamp`, `source`, `type`, `payload`, `version` and `turnId`. The host's codec says the payload keys are part of the stored log, so changing one is a change of its log format.
- **Usage per turn, not per request.** `PromptTurnFinished` carries:
  - `inputTokens`, `outputTokens`, `cacheReadTokens`, `cacheWriteTokens`, `reasoningTokens`;
  - `tokenAttribution` (`EXACT`, `INFERRED` or `UNKNOWN`), `totalTokens`, `contextWindow`;
  - `costAmount` and `costCurrency`.

  `UsageUpdated` carries the running context fill: ACP's `used` and `size`, plus a cost when the agent sends one. Of 1,160 finished turns, 1,115 have exact counts.
- **The cost's kind is not stored.** The host's cost ledger takes the agent's own cumulative cost when it can, and otherwise prices the tokens with the LiteLLM catalogue. The payload stores the amount without saying which of the two it is.
- **Identity.**
  - `ThreadCreated` holds `agentId`, `title` and `cwd`.
  - `RuntimeSessionBound` holds `agentId` and `agentSessionId`, the agent's own session id. For Codex, 242 of 243 bound sessions still have their rollout. For Claude Code, 109 of 176 still have their transcript; the others are probably gone to Claude Code's transcript cleanup.
  - Agents from the ACP registry get ids found only at run time, such as `acp.dynamic.<hex of "acp.registry.github-copilot">`.
- **Other facts.**
  - `SubagentStarted` and `SubagentFinished` with `subagentSessionId`, `name`, `prompt` and `parentSubagentSessionId`.
  - A compaction event in the host's `_meta` extension, `contextCompaction`, with `trigger`, `preTokens`, `postTokens` and `durationMs`.
  - `ConfigOptionUpdated` with `session.model`. For Codex the value is the model plus its effort, such as `gpt-5.6-sol[max]`; for Claude Code the field is empty.
  - Tool events with ACP's tool `kind`: `execute`, `read`, `edit`, `fetch`, `think`, `other`.
- **The host's own analysis module** solves a neighboring problem the same way this design does. It has a provider-neutral model and one plain decoder per format. The caller finds and reads the files, and the decoder only reduces the records it gets. No clock and no random ids are used, and a full analysis must equal an incremental one.

Three more lessons for the interface:

6. **The reader of a log and the agent are different things.** One log source reports sessions of many agents, some known only at run time.
7. **One session can be seen through two sources:** the agent's own log and the host's. Counting both would double its spend.
8. **ACP already has a neutral tool vocabulary,** its tool kinds.

This change reads no new log fields. The Claude Code and Codex adapters keep reading the fields that session-logs names. The IDE host source is not built here; the interface leaves room for it.

## Goals / Non-Goals

**Goals:**
- Outside `sources/`, no code names an agent, a tool of an agent, or a log field.
- Adding an agent or a log source means one crate under `sources/`, one line in `tokprof-sources`, and fixtures that pass the conformance tests.
- The library's results are owned data that the next change can serialize to JSON unchanged.
- Every command's output stays byte-identical, and the full corpus parses no slower.

**Non-Goals:**
- A stable semver promise. The library stays 0.x and is used as a git dependency.
- Loading agents at run time (dylib, WASM). All adapters are compiled in.
- A generic configuration system for adapters. Each adapter reads its own environment variables, as today.

## Decisions

### 1. Packages: the framework in `crates/`, one crate per log source in `sources/`

```
crates/
  tokprof-core/       model, source interface, API cache rules, vocabularies, pricing, analysis,
                      aggregation, query and scan, forecast, X-ray, advice, bench, handoff note,
                      labels (display_path, command_head, url_host), testkit (feature), profile writers (feature `export`)
  tokprof-logio/      neutral log reading: open with zstd, lines with the incomplete-tail rule,
                      JSON field helpers, timestamps, canonical hashes
  tokprof-sources/    the registry: a feature per source, on by default; catalog()
  tokprof/            bin `tokprof`: clap, text renderers, Style, TUI, watch, clipboard, paths
sources/
  claude-code/        tokprof-source-claude-code
  codex/              tokprof-source-codex
  ...                 later ide-host/, junie/, opencode/, each its own crate
```

**Dependency rules.**
- `tokprof-core` and `tokprof-logio` depend on no other tokprof package.
- A source depends on `tokprof-core` and `tokprof-logio` only, never on another source.
- `tokprof-sources` depends on the sources, and the CLI only on `tokprof-sources` and `tokprof-core`.

So the compiler keeps agents apart from the core and from each other, and the architecture test (section 11) checks these edges in `cargo metadata`.

**Why one crate per source.** With many agents, one crate of adapters would grow into the place where everything agent-related lives:
- its modules could reach into each other's internals;
- its dependencies would pile up, such as `rusqlite` for OpenCode;
- its feature matrix would grow with every agent.

A crate per source keeps each agent's knowledge, dependencies, fixtures and tests in one directory. It also compiles in parallel, and an outside crate can add a source with the same interface.

**Adding a source** is a directory and one line:
1. `sources/<name>/` with `Cargo.toml`, `src/lib.rs` (the `LogSource` and its `Decoder`), `tests/fixtures/`, and `tests/conformance.rs`, which runs the testkit over the fixtures.
2. A `README.md` in it that names the log locations, the fields it reads and the agent versions they were seen in.
3. A feature and one line in `tokprof-sources`.

Nothing in `tokprof-core`, `tokprof-logio` or the CLI changes. The architecture test fails if it does: an agent name or a tool name there, or a new dependency edge. Common settings come from `workspace.package`, `workspace.dependencies` and `workspace.lints`, so a source's `Cargo.toml` is a few lines.

**Shared helpers split by what they know.** Today `adapters/util.rs` is shared by both adapters.
- The neutral parts move: file reading, line scanning, JSON helpers and hashes go to `tokprof-logio`; `display_path`, `command_head` and `url_host` go to the core's labels.
- `tool_target` knows tool names of both agents (`Read`, `Grep`, `Agent`, `Task`, `Skill`, `mcp__…`, `shell`, `exec_command`), so it splits into one table per source. Each table keeps only its agent's names. `compare.sh` shows whether a name of one agent ever matched in the other's logs.

**The bin package takes the name `tokprof`,** so `cargo install tokprof` and `cargo install --git` give the binary. `cargo install --git` works because only one package has a binary.

Alternatives considered:
- **One package with feature flags.** Nothing would stop the core from calling into an adapter or the CLI.
- **One crate for all adapters, a module and a feature per agent.** Fewer crates today. Rejected for the reasons above; it gets worse with every agent.
- **Sources as separate programs** that print normalized records, as `git` finds `git-*` commands. It would allow any language, for example the host's own Kotlin decoders. Deferred: it needs a stable wire format of the model, and `add-json-output` is where one could come from. Crossing a process boundary for 8.5 GB of logs also costs time.
- **Library `tokprof` and bin `tokprof-cli`,** as typst does. Rejected: the product is the console tool, so its name should install the binary.

### 2. Sessions are referenced, not assumed to be files

`SourceFile` becomes `SessionRef`:

```rust
pub struct SessionRef {
    pub agent: AgentId,
    pub locator: Locator,       // File(path) | Dir(path) | Row { db: path, id }
    pub size: u64,
    pub modified_ms: i64,
    pub created_ms: Option<i64>,
}
```

The core never opens a locator. It sorts and filters by size and times, and it prints the path the adapter gives for it. For Claude Code and Codex every locator is a `File`, so nothing they print changes.

### 3. The adapter interface: a source and a decoder

An adapter is a log source. It finds, opens and watches logs, and it knows their layout. The agent is a property of each session it reports, not of the source (lesson 6). Reading a log is split off into a decoder that only reduces the records it is given, as in the host's analysis module.

```rust
pub trait LogSource: Send + Sync {
    fn name(&self) -> &'static str;                      // "claude", "codex"; later "ide-host"
    fn agents(&self) -> Vec<AgentInfo>;                  // the agents it knows; it may report others found at run time

    fn discover(&self) -> Vec<SessionRef>;               // main sessions and subagent logs
    fn claim(&self, path: &Path) -> Claim;               // No | Yes | Fallback
    fn watched_paths(&self) -> Vec<PathBuf>;
    fn touched(&self, changed: &Path) -> Touched;        // which main session a changed path belongs to
    fn head(&self, s: &SessionRef) -> Head;              // agent, id, the agent's own session id, project, title
    fn is_subagent(&self, s: &SessionRef) -> bool;
    fn main_of(&self, s: &SessionRef) -> Option<SessionRef>;
    fn subagents(&self, s: &SessionRef) -> Vec<SessionRef>;
    fn log_location(&self, s: &SessionRef) -> String;    // the path the handoff line names
    fn open(&self, s: &SessionRef) -> Result<Box<dyn Records>>;  // lines, rows, or the files of a directory
    fn decoder(&self, s: &SessionRef) -> Box<dyn Decoder>;
    fn activity(&self, s: &SessionRef) -> Result<Activity>;      // for the handoff note
}

pub trait Decoder {
    fn feed(&mut self, record: &[u8]);                   // one record; a bad one becomes a diagnostic
    fn snapshot(&self) -> Session;                       // what the records so far give
}
```

- **The decoder is pure.** It touches no file, no clock and no environment, so a test feeds it bytes. Because it reduces records one at a time, the TUI and a statusline can later feed only the lines appended to a growing log, instead of parsing the whole log again. This change keeps parsing whole logs. The conformance tests (section 11) already check that feeding the records in two parts and taking `snapshot` gives the same session as feeding them at once.
- **Short ids, the pickup calibration and the other agent facts** come from `AgentInfo` and `Harness`, per agent, not per source (section 7).
- **Instances own their state.** The Codex thread index and thread names stop being `static` and live in the Codex source. `Catalog` holds the sources in registry order. That order is the default `--agent` list, the order of the read line and the bench tables, and the TUI's palette slot. `tokprof_sources::catalog()` builds it, and each source reads its own environment (`CLAUDE_CONFIG_DIR`, `CODEX_HOME`) in its constructor.
- **No default method bodies.** Adding a method makes the compiler ask every source for it. This replaces the exhaustive `match` on the `Agent` enum.
- **`claim` keeps today's target rule in the adapters.**
  - Codex claims `rollout-*.jsonl` and `rollout-*.jsonl.zst`.
  - Claude Code returns `Fallback` for any other file, as session-xray's Target resolution says.
  - The first adapter whose files could be taken for a transcript, such as Junie's `events.jsonl`, must narrow that rule in a spec change.
- **`activity` is a second, light pass over the log.** It returns typed prompts, plan updates, file edits and reads, and commands. The note builder in the core turns it into today's note. `TodoWrite` and `update_plan` stay inside the adapters. `Session` does not grow, so the main parse keeps its speed.
- **Watching.** `touched` lets the watcher in the CLI map a changed path to a session without knowing the layout. For OpenCode, a change of the database's WAL file would mean "look up sessions by `time_updated`".

### 4. Agent identity

`AgentId` is the agent's name as a shared string (`Arc<str>`). It replaces the `Agent` enum in the core.
- It is a name and not a registry index, because a source can report agents found only at run time (lesson 6), such as a registry agent in the IDE host's log.
- An agent no source declared gets an `AgentInfo` with only its name: no title beyond the name, no harness calibration, and no capabilities beyond what its records show.
- The catalog orders agents: declared agents in registry order, then the others by name.
- The printed name stays `claude` or `codex`, so profiles, the read line and the lists do not change.
- `Fact.agent` becomes an `AgentId`, which ends the fallback that booked an unparsed name to Claude Code.

### 4a. One session, several sources

`Head` carries the agent's own session id when the log names it: the Claude Code transcript id, the Codex thread id, and in the IDE host's log `RuntimeSessionBound.agentSessionId`. The catalog keys a session by agent plus that id. When two sources report the same key, it keeps one of them:
- The source with requests and prompts wins over one with only turns. The agent's own log beats the host's.
- The other source can still add what the winner lacks, such as the host's thread title, and it fills in the session when the agent's own log is gone, as for 67 of the 176 Claude Code sessions above.

Only one source per agent exists in this change, so the rule never runs here. The key and the rule are built now, so that an IDE host source cannot double the spend when it comes.

### 5. Two axes: the agent and the API

Each request carries `api: ApiId`, which the adapter sets. Claude Code requests go to `anthropic` and Codex requests to `openai`. Codex's own provider field is not read yet.

The core holds one table of cache rules per API. It is the only place in the core that names vendors.

| API | Lifetimes | Long lifetime when | Lookback | Whatif | Heartbeat assumption |
|---|---|---|---|---|---|
| `anthropic` | 5m, 1h | the latest write was mostly 1-hour writes | 20 blocks | replays it | documented |
| `openai` | 30m, assumed | — | none | skips it | assumed, as the bench note says |
| any other | none known | — | none | skips it | — |

- The miss classifier, the X-ray's cache line, the forecast and the bench all ask `rules(request.api)`.
- An API without rules gets no `idle > TTL` cause and no warm or cold line, because the rule is never guessed. No current agent has such an API, so this shows up only with a new adapter.
- Alternative considered: keeping the rules in `prices.toml` next to the models. Rejected for now, because it changes the validated price file format, which is behavior. The table can move there later without changing this design.
- `after_assistant_message` stays a neutral request fact. The Codex adapter sets it, as today.

### 6. Neutral vocabularies

These are enums in the core. Adapters map their own names onto them. Raw tool names and labels stay in `Block` for display, because profiles and the X-ray print them.

- **`ToolClass`.** It follows ACP's tool kinds (lesson 8), so a source of ACP sessions maps the logged `kind` straight onto it. Two classes refine an ACP kind, because today's hints tell them apart. The mapping of the current tool names must reproduce today's advice matches exactly:
  - `Execute`: `Bash`, `exec_command`, `shell`;
  - `ExecuteOutput`, a refinement of `execute`: `BashOutput`;
  - `Read`: `Read`;
  - `Fetch`: `WebFetch`, `WebSearch`;
  - `Search`: `Grep`, `Glob`;
  - `Edit`: `Edit`, `MultiEdit`;
  - `Write`, a refinement of `edit`: `Write`;
  - `Delete`, `Move`, `Think` and `SwitchMode`, for sources that log them;
  - `Other`: everything else.

  `ExecuteOutput` is its own class because the tool-result hint covers `BashOutput` and the tool-call hint does not. Codex `apply_patch` stays `Other`: today it gets the default hint, and giving it the edit hint would change output.
- **`ReminderKind`:** `Instructions`, `BackgroundNotifications`, `HookContext`, `DeferredToolLists`, `Skills`, `Other`, mapped from the labels the Claude Code adapter makes today. The instructions hint names the instruction files of the registered agents in registry order. That is `CLAUDE.md and AGENTS.md`, the same text as today.
- **`Origin`:** `Prompt`, `Skill`, `Subagent`, `Command`, `Notification`, `Mcp`, `Plugin`. It displays as today's strings: `prompt`, `skill:<name>`, `agent:<type>`, `command:/<name>`, `notification`, `mcp:<server>`, `plugin:<name>`.
- **`CompactionTrigger`:** `Idle`, `Auto`, `Manual`, `Logged(String)`, `Unknown`. `Idle` displays as `while idle`. The miss detail and the X-ray's `compacted …` line stop matching the string `"idle"`.
- **Params.** `effort` becomes a field of its own, since the thinking advice reads it. The other cache-busting settings become an ordered list of name and value. The adapters fill it in today's order (effort, thinking, speed, betas, context management), so the first changed name stays the same.

### 7. Agent facts are data

- **`AgentInfo`:**
  - the CLI name and the title (`Claude Code`, `Codex`);
  - the length of a short id (8 characters for Claude Code, 18 for Codex);
  - the instruction files;
  - the settings that set the cache lifetime, for the whatif report (`promptCacheTtl`, `subagentPromptCacheTtl`);
  - the name of its own cost counter, for the reconciliation line.
- **`Harness`:**
  - the pickup calibration (Claude Code 8k tokens over 4 requests with 2k output; Codex 30k over 6 with 4k);
  - how a compaction call is modeled. Claude Code's is a fork that reads the conversation's cache.
- **`Capabilities`:** whether the log has the agent's own cost (`None`, `Session`, `Request`), the compaction trigger, the context window, prompt composition, titles, subagent tasks and origins.

The rule is that when a capability is missing, the view shows nothing or "not known". It never borrows a value from another agent: an agent without a pickup calibration gets no pickup estimate. In this change, views use capabilities only where today's code already special-cases an agent, so the output does not change.

Model structs keep public fields and no `#[non_exhaustive]`, because adapters in another crate build them with struct literals. Open enums get `#[non_exhaustive]`: `BlockKind`, `ChainKind`, `Cause`, `Component`, `ToolClass`, `ReminderKind`, `Origin` and `CompactionTrigger`.

### 8. Data first, text in the CLI

The library returns owned views:
- `XRay`, `Advice`, `Glance`, the session list;
- `Summary`, `Misses`, `WhatIf`, `Bench`, `PriceTable`;
- the handoff line and note.

The rules for views:
- They hold no references into a `Session`, no ANSI codes and no wrapping.
- Field names carry units: `_usd`, `_tokens`, `_ms`.
- Advice wording stays in the core, because text and JSON share it. The CLI wraps it at 82 columns.
- Number formatting (`tokens`, `usd`, `count`, durations) moves to a `fmt` module in the core.
- Clock times in reports are formatted by the CLI from epoch ms.
- The TUI keeps drawing the X-ray through the CLI's ANSI renderer and ansi-to-tui, as today.

Views get `Serialize` in `add-json-output`, so the JSON shape is designed once, in that change.

### 9. Query and scan in the core

`Query { agents, project, session, since_ms, until_ms }` replaces three things: `xray::Select`, the `accepts` and `in_session` checks in `main.rs`, and the window part of `analysis::Filter`.

`scan(&Catalog, &Query, &Pricing, make_sink, on_unreadable)` runs today's parallel pipeline and returns the sink and `ScanStats`. `ScanStats` holds the files and bytes per agent and the unreadable logs with their errors.

`on_unreadable` keeps the specified behavior:
- reports skip the log and warn;
- the X-ray fails;
- a profile from the TUI skips unreadable subagent logs and counts them.

`Collector` splits into small sinks: `Totals`, `ModelTotals`, `Work`, `WhatIf`, `Profile`, `TimedProfile`, `Misses`. A tuple of sinks is a sink, so each command collects only what it shows. `Totals` in `xray.rs` merges into the same sinks.

### 10. No environment and no clock in the core

- **Time.** `now_ms` is a parameter of the X-ray, the glance, the cache state and `Pricing::context_window`.
- **The CLI owns:**
  - `TOKPROF_DIR` and the default profile names;
  - `XDG_CONFIG_HOME` and the user price file, which reach the core as layers of text;
  - `NO_COLOR` and the terminal check;
  - the clipboard;
  - opening a written profile.
- **Each source owns** its own environment variables: the Claude Code source `CLAUDE_CONFIG_DIR`, the Codex source `CODEX_HOME`. The home directory reaches the core's labels as a parameter.
- **Embedded files.** `prices.toml` and `assets/async-profiler/` move into `crates/tokprof-core/`, because `cargo package` leaves out files above the package root. Pricing's "the repository's `prices.toml`" stays true.

### 11. Guards

- **An architecture test** in the CLI package reads the sources of `tokprof-core`, `tokprof-logio`, `tokprof-sources` and the CLI. It fails on agent names, on the tool names of section 6, and on log field names, outside test modules. The API table of section 5 is the one allowed exception.
- **The dependency rules of section 1** are checked by the same test, from `cargo metadata`.
- **A conformance testkit** sits in the core behind a `testkit` feature. Each source crate runs it over its own small synthetic fixtures. It checks that:
  - `head` agrees with the decoded session on agent, id and project;
  - feeding the records in two parts and taking `snapshot` gives the same session as feeding them at once;
  - when the log names the agent's own session id, `head` reports it, so the catalog can key the session (section 4a);
  - the short id is a prefix of the id;
  - every subagent log's `main_of` is its main session;
  - requests of a chain are in time order;
  - the declared capabilities hold. For example, no compaction trigger means every compaction is `Unknown`, and no own cost means no reported cost.

  Fixtures are synthetic, because the repository is public.

### 12. Effect on parse time

- **Per-line work** does not change.
- **Dynamic calls** add one per session reference, about 1,600 for the corpus.
- **Struct sizes do not grow.** `Block` gains a one-byte `ToolClass` and `Request` a one-byte `ApiId`; both fit into existing padding.
- **The Codex index** is built once per `Catalog`, as the static was built once per run.

Expected change: none measurable. The check is a median of 3 runs of `tokprof summary` within 5% of the baseline.

For the later IDE host source: its logs here are 4.2 GB, mostly `MessageChunk` (270k) and `ToolCallUpdated` (127k) lines. Reading them all would add noticeably to a full scan. The plan is to read only the head of each thread, for the agent and its own session id. A thread whose agent has its own log is then skipped by section 4a. Only the other threads are read in full, and only their usage, subagent and compaction lines are parsed, picked out by a substring test as the idle-compaction notice is today.

### 13. What the implementation settled

The code follows this design with these choices, made while building it:
- **The interface.** `LogSource` has no `main_of`, `is_subagent` or `log_location`: a `SessionRef` carries `subagent` and `main_id` from discovery, and its path is what the handoff line names. It has `session_at`, so the watcher and target resolution get a reference for a path, and `detail`, which section 4a ranks sources by. `touched` takes `arrived`, as the watcher's rule for directories needs it.
- **The decoder** is fed records as text (`feed(&str)`), since the line reader already replaces invalid UTF-8, and has `finish` next to `snapshot`, so a whole-log parse does not clone its state. One dynamic call per record, about 4M for the corpus, costs well under 0.1 s.
- **Agent identity.** `AgentId` is a shared handle to the agent's `AgentInfo` (an `Arc`), compared, hashed and ordered by name, so the analysis, the views and the renderers read an agent's facts without a catalog.
- **Modification times** are kept in nanoseconds (`modified_ns`): sessions written within one millisecond kept the order the file system gives, and the session list depends on it.
- **Scanning.** `scan()` reads the corpus for the reports and the bench. The X-ray, the glance and the profile of one session read their few logs directly, with the X-ray failing on an unreadable log as specified.
- **The architecture test** reads `tokprof-core`, `tokprof-logio` and the CLI for names. `tokprof-sources` is left out of the name rule, since naming its sources is its job; its dependency edges are checked.
- **The broken fixture** of the conformance test is a transcript whose chain goes back in time. A short id is a prefix of its id by construction, so no fixture can break that check.

## Risks / Trade-offs

- **[The move touches every file while other sessions may edit `src/`]** → Phase 1 only moves files, with `git mv`, on a clean tree, in one step. Later phases edit in place.
- **[Output drifts during the refactor (map order, rounding)]** → After every phase, compare every command's output with the baseline binary, byte for byte.
- **[The X-ray depends on the current minute]** → Run the baseline and the new binary back to back. Accept differences only in lines with a duration since now (`cold in`, `cold for`, `idle`).
- **[The interface fits only the two agents it was cut from]** → It was checked against OpenCode's database, Junie's sessions and the IDE host's ACP log on this machine. That is where the locators, the API per request, the own cost per request, several models per event, sources of several agents, and one session in two sources come from.
- **[The host's log is an internal format of another product]** → The host's own codec treats its payload keys as a stored format, and every record has a `version`. The IDE host source pins what it reads with synthetic fixtures.
- **[The host's turn cost may be its own estimate]** → The payload does not say whether `costAmount` is the agent's figure or a LiteLLM price. An IDE host source prices the turn's tokens with tokprof's prices, and shows the host's amount only for reconciliation. Storing the cost's kind and the model next to the amount in `PromptTurnFinished` would remove the doubt; that is a request for the host, not part of this change.
- **[The host reports usage per turn, not per request]** → A host-only session gets turn totals, cache reads and writes, and context fill. It gets no per-request miss causes and no prompt composition, and its capabilities say so.
- **[Claude Code keeps claiming unknown files]** → Kept, because the spec says so. The adapter that could be confused with it narrows the rule in its own change.
- **[The exhaustive `match` on agents is lost]** → Trait methods without default bodies, plus the conformance tests.
- **[A library without a stability promise]** → 0.x and a git dependency with a tag. Semver checks come with publishing.

## Migration Plan

The work goes in eight phases, in the order of tasks.md:
1. Baseline.
2. Workspace.
3. Cut cycles, environment and clock.
4. Adapters.
5. APIs and vocabularies.
6. Views.
7. Query and scan.
8. Guards and docs.

Each phase ends with `cargo test` green and an empty output diff, in a state that can be committed. Rollback is reverting the commits of the last phases. Nothing outside the repository changes, except the install path of the author's binary.

## Open Questions

- Whether the API cache rules later move into `prices.toml` next to the models. This does not change the interface.
