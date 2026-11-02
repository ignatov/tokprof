# Tasks

Every group ends with `cargo test --workspace` green and an empty output diff (task 1.1), in a state that can be committed.

## 1. Baseline

- [x] 1.1 Build the current commit with `cargo build --release` and copy the binary to `~/tokprof/split-baseline/tokprof-base`. Keep the baseline and its outputs outside the repository, because they hold the author's sessions. Write `~/tokprof/split-baseline/compare.sh`. It runs the baseline and a new binary back to back with `NO_COLOR=1` and diffs stdout and stderr of these commands:
  - `summary`, `misses --expected --top 50`, `whatif`, `sessions --cost --limit 50`, `bench`, `prices`, `prices --at 2026-08-20`;
  - `flame` to `.collapsed` and `.html`, and to `.jfr`, compared by `jfr summary`;
  - `scan` of four sessions: two Claude Code, one with subagents; two Codex, one `.jsonl.zst`, one with child threads;
  - `handoff --note` of one Claude Code and one Codex session;
  - `ui --dump 160x45`.

  Seconds in the read line are masked. A difference in a line with a duration since now (`cold in`, `cold for`, `idle`) is checked by running again. Verify: the baseline against itself gives an empty diff.
- [x] 1.2 Record the baseline in the results table at the end of this file:
  - the median of 3 runs of `tokprof summary`;
  - the reconciliation line against Claude Code's cost counter, for the same 246 sessions;
  - the number of tests `cargo test` runs.

## 2. Workspace

- [x] 2.1 Turn the repository into a workspace. The root `Cargo.toml` gets `[workspace]`, `workspace.package` and `workspace.dependencies`. With `git mv`, on a clean tree and in one step:
  - every module except `main.rs`, `ui.rs` and `watch.rs` moves to `crates/tokprof-core/src`, and `prices.toml` and `assets/async-profiler/` to `crates/tokprof-core/`;
  - `main.rs`, `ui.rs` and `watch.rs` move to `crates/tokprof/src`, the bin package `tokprof`;
  - `lib.rs` stops exporting `ui` and `watch`;
  - `prost` and `flate2` go behind a default `export` feature.

  Verify: `cargo test --workspace` runs as many tests as task 1.2 recorded. `cargo tree -p tokprof-core -e normal` lists no `ratatui`, `ansi-to-tui`, `notify` or `clap`.
- [x] 2.2 Update the install line in README (two lines), AGENTS.md "Build and check" and the apply guidance in `openspec/config.yaml` to `cargo install --path crates/tokprof --root ~/.local`. Update README's links to `prices.toml` and the vendored flame graph. Verify:
  - the command installs a binary that passes `compare.sh`;
  - the README links resolve;
  - `openspec validate --all --strict` passes.

## 3. Cycles, environment and clock

- [x] 3.1 Add a `fmt` module with `tokens`, `usd`, `count`, `human_duration`, `wrap`, `clip_end` and `local_time`, and move their tests with them. Verify: `advice`, `handoff` and `bench` no longer import from `xray` or `report` for formatting.
- [x] 3.2 Add a `forecast` module with the cache state, the latest write lifetime, the pace, the next-request cost and the pickup estimate, taken out of `xray` and `advice`. `bench` uses it. Verify: the moved tests pass, and `bench` no longer imports `xray`.
- [x] 3.3 Put `Values`, `Frame`, `Stacks`, `Tint` and the timed stacks into one `profile` module that `report`, `export` and `jfr` share. Verify: `export` and `jfr` no longer import `report`, and `report` no longer imports `jfr`.
- [x] 3.4 Make `now_ms` a parameter of the X-ray, the glance, the cache state and `Pricing::context_window`. The CLI reads the clock. Tests pass a fixed time instead of `now_ms()`. Verify: `grep -rnE "now_ms\(\)|SystemTime::now|Utc::now" crates/tokprof-core/src` finds nothing outside test modules.
- [x] 3.5 Move these into the CLI:
  - the profile directory (`TOKPROF_DIR`);
  - the user price file path (`XDG_CONFIG_HOME`); `Pricing` gets its layers as name, path and text from the CLI;
  - `Style::detect` (`NO_COLOR`, the terminal check);
  - the clipboard copy;
  - creating the profile directory;
  - opening a written profile.

  Verify: `grep -rn "env::var" crates/tokprof-core/src` finds only the adapters, which leave in group 4, and `compare.sh` gives an empty diff.

## 4. Adapters behind one interface

- [x] 4.1 Add these to the core:
  - `LogSource`, `Decoder` and `Records`;
  - `AgentId`, a shared name;
  - `SessionRef` with `Locator`, `Claim`, `Touched`, `Head` with the agent's own session id, and `Activity`;
  - `AgentInfo`, `Harness` and `Capabilities`;
  - `Catalog`.

  `Catalog` holds the sources in registry order and orders agents as design.md, section 4, says. It finds an agent by name and resolves a path by claims, where `Yes` beats `Fallback`. It keys sessions by agent and the agent's own session id, and keeps one source per key by the rule of section 4a. Verify with unit tests next to `Catalog`:
  - claim order;
  - name lookup, and the unknown-agent error, which keeps its text `unknown agent "x"`;
  - an agent found only at run time sorts after the declared ones;
  - two fake sources reporting one session count it once, the one with requests wins, and the other fills in when the first has no log.
- [x] 4.2 Create `tokprof-logio`, `sources/claude-code` (`tokprof-source-claude-code`), `sources/codex` (`tokprof-source-codex`) and the registry `tokprof-sources`, as design.md, section 1, lays out. Move `adapters/claude.rs` and `adapters/codex.rs` into the two sources and implement the interface. Split `adapters/util.rs`:
  - file reading, line scanning, JSON helpers and hashes go to `tokprof-logio`;
  - `display_path`, `command_head` and `url_host` go to the core's labels;
  - `tool_target` becomes one table per source, each with only its agent's tool names.
  - Each parser becomes a pure decoder: the source opens the log and feeds its lines, and the decoder only reduces them.
  - The Codex thread index and thread names become fields of the Codex source.
  - Each adapter reads its environment variable in its constructor.
  - `tokprof_sources::catalog()` builds the registry, Claude Code then Codex, with a default feature per source.

  Verify:
  - the moved adapter tests pass;
  - a new test builds two Codex sources over two temporary directories and checks that each sees only its own threads, which the `static` could not do;
  - a test feeds each decoder bytes with no file at all;
  - `cargo build -p tokprof-sources --no-default-features --features codex` builds without the Claude Code source.
- [x] 4.3 Replace the `Agent` enum in the core with `AgentId`, and make `Fact.agent` an `AgentId`. Remove the fallback that books an unparsed agent to Claude Code. Verify: `grep -rnE "Agent::(Claude|Codex)" crates/tokprof-core/src crates/tokprof/src` finds nothing outside test modules.
- [x] 4.4 Replace the agent-specific helpers with adapter calls:
  - `head` replaces `quick_meta`;
  - `claim` replaces the path guess in target resolution;
  - `main_of`, `subagents`, `is_subagent` and `log_location` replace `session_of`, `SourceFile::is_subagent`, `subagent_files` and `handoff::id_of`;
  - the short-id length in `AgentInfo` replaces `model::short_id`.

  Verify: the existing tests for target resolution, short ids and ambiguous prefixes pass unchanged.
- [x] 4.5 Move the handoff note's log parsing (`claude_line`, `codex_line`, `TodoWrite`, `update_plan`) into each adapter's `activity`. The note builder in the core builds the note from `Activity`, and its header takes the agent's title from `AgentInfo`. Verify: the two handoff note tests pass with their expected texts unchanged.
- [x] 4.6 The CLI's watcher uses `watched_paths` and `touched`. The default of `--agent` and the agent list in errors come from the catalog. Verify: the watch tests pass, and `tokprof --help` prints the same text as the baseline.

## 5. APIs and vocabularies

- [x] 5.1 Add `ApiId` to requests. The Claude Code adapter sets `anthropic` and the Codex adapter `openai`. Add the cache-rules table of design.md, section 5. The miss classifier (lifetime, lookback), the X-ray cache line, the forecast, the bench lifetimes, the heartbeat note and the whatif scope all read it. Verify with unit tests next to the table:
  - the 1-hour and 5-minute choice for `anthropic`;
  - 30 minutes for `openai`;
  - lookback only for `anthropic`;
  - an API without rules gets no lifetime and no `idle > TTL` cause.
- [x] 5.2 Add `ToolClass`, which follows ACP's tool kinds, and `ReminderKind`. The adapters map their names onto them as design.md, section 6, lists, and the advice matches classes. The instructions hint joins the instruction files of the registered agents in registry order. Verify:
  - unit tests of each adapter's mapping, including `BashOutput` to `ExecuteOutput`, `Write` to `Write` and `apply_patch` to `Other`;
  - a unit test that every class names its ACP kind;
  - the advice tests pass with their texts unchanged.
- [x] 5.3 Add `Origin` and `CompactionTrigger`, whose display gives today's strings. The miss detail and the X-ray's compaction line use them instead of matching `"idle"`. Verify: unit tests of the display strings, and the existing compaction-detail tests.
- [x] 5.4 Split `Params` into an `effort` field and an ordered list of cache-busting settings. Verify: a unit test that the first changed name is the same as today for each of the five settings.
- [x] 5.5 Fill `AgentInfo`, `Harness` and `Capabilities` for both agents. Take these from them instead of `match` on the agent:
  - the bench and handoff titles;
  - the reconciliation line's counter name;
  - the whatif settings line;
  - the pickup calibration;
  - the TUI's row color, by registry index.

  Verify: `grep -rnE "Claude Code|Codex" crates/tokprof-core/src crates/tokprof/src` finds nothing outside test modules and the API table.
- [x] 5.6 Run `compare.sh`, then the timing of task 1.2, because `Block` and `Request` changed. Verify: an empty diff, and a median within 5% of the baseline.

## 6. Views

- [x] 6.1 Add the `XRay` view and its text renderer in the CLI. The TUI draws the X-ray through that renderer and ansi-to-tui, as today. Verify: the X-ray tests assert on the view, the renderer tests on the text, and the `scan` and `ui --dump` outputs in `compare.sh` are unchanged.
- [x] 6.2 Make advice items typed, with their text built in the core. The CLI wraps them at 82 columns. Verify: the advice tests pass with their texts unchanged.
- [x] 6.3 Add the `Summary`, `Misses`, `WhatIf`, `PriceTable`, session list and `Glance` views, and their renderers in the CLI. Verify: the report tests are split into view tests and renderer tests, and the `summary`, `misses`, `whatif`, `prices` and `sessions` outputs are unchanged.
- [x] 6.4 Add the `Bench` view. The bench text and the SVG writer move to the CLI. Verify: the bench tests pass, and `tokprof bench --svg` writes the same light and dark SVGs as the baseline.

## 7. Query and scan

- [x] 7.1 Add `Query`, which replaces `xray::Select`, the `accepts` and `in_session` checks in `main.rs`, and the window part of `analysis::Filter`. Verify: the filter tests from `xray` and `main` move next to `Query` and pass.
- [x] 7.2 Add `scan()` with `on_unreadable` and `ScanStats`. Split `Collector` into the sinks of design.md, section 9, with a sink for tuples, and merge the X-ray's `Totals` into them. Reports, the X-ray, the glance, the TUI's profile export and the bench call `scan()`. Verify:
  - the unreadable-log tests pass: 5 warnings, then the count;
  - the X-ray still fails on an unreadable subagent log;
  - `compare.sh` diffs stderr too, and the diff is empty.
- [x] 7.3 Reduce `main.rs` to parsing arguments, building the catalog, pricing and query, calling the core and rendering. Verify: `main.rs` calls no adapter and no `rayon`.
- [x] 7.4 Run `compare.sh` and the timing of task 1.2. Verify: an empty diff, and a median within 5% of the baseline.

## 8. API surface and guards

- [x] 8.1 Re-export the library API at the root of `tokprof-core` and make the other modules `pub(crate)`. Add `#[non_exhaustive]` to the enums of design.md, section 7. Write a crate-level example in `tokprof-sources` that builds the catalog, scans a week, and prints the X-ray of the latest session. Verify: `cargo test --doc --workspace` runs the example, and `cargo doc --no-deps --workspace` prints no warnings.
- [x] 8.2 Add the conformance testkit to the core behind the `testkit` feature, with the checks of design.md, section 11, including feeding the records in two parts. Add synthetic fixtures:
  - a Claude Code transcript with a subagent and a compaction;
  - a Codex `.jsonl.zst` rollout with a child thread.

  Each source crate runs the testkit on its own fixtures in `tests/conformance.rs`. Verify: it passes, and a test with a broken fixture, whose short id is not a prefix of the id, fails as expected.
- [x] 8.3 Add the architecture test in `crates/tokprof/tests/architecture.rs`, as design.md, section 11, describes: names in the sources of the framework crates, and the dependency rules of section 1 from `cargo metadata`. Verify: it passes, and it fails once by hand for each rule. First an agent name is added to a core file; then `sources/codex` is made to depend on `sources/claude-code`.
- [x] 8.4 Add a short "Library" section to README: the packages, what `tokprof-core` and `tokprof-sources` give, the example of task 8.1, and the git dependency line. In AGENTS.md, add the checklist for a new source from design.md, section 1. Each source crate gets the `README.md` that the checklist asks for. Verify: the README example is the code that task 8.1 runs.

## 9. Checks on the real logs

- [x] 9.1 Run `cargo test --workspace`, `compare.sh`, the timing and the reconciliation, and fill in the results table. Verify: `openspec validate --all --strict` passes.
- [x] 9.2 Reinstall the author's binary with `cargo install --path crates/tokprof --root ~/.local` and run `tokprof` once. Verify: it X-rays the latest session.

## Results on the real logs

| Check | Baseline | This change |
|---|---|---|
| `compare.sh` differing lines | 0 of 20 commands (against itself) | 0 of 20 commands |
| `tokprof summary`, median of 3 | 24.4 s (24.9, 24.4, 24.4) | 24.0 s (24.2, 24.0, 23.9) |
| Reconciliation with Claude Code's cost counter | 278 sessions: $10,327 counted by Claude Code, $10,454 by tokprof (+1.2%) | the same |
| Tests run by `cargo test` | 81 passed, 1 ignored | 104 passed, 1 ignored (`cargo test --workspace`) |

How the baseline was taken (2026-11-02):
- The corpus is a frozen APFS clone (`cp -c`) of `~/.claude/projects`, `~/.codex/sessions`, `~/.codex/archived_sessions` and `session_index.jsonl` in `~/tokprof/split-baseline/corpus`: 1,037 Claude Code files (5.8 GB) and 793 Codex files (2.6 GB). Both binaries read it through `CLAUDE_CONFIG_DIR` and `CODEX_HOME`, so sessions that are still being written do not change the outputs between runs.
- The baseline binary is commit 5fa6232 plus one fix. Its Codex thread index ran rayon inside a `OnceLock` that rayon workers also waited on. A worker waiting for its own jobs could pick up a parse that entered the `OnceLock` again, and `flame` hung in about half of the runs. The fix reads the index on plain threads, which gives the same index. The same fix is in this change (`read_threads` in the Codex adapter).
- The summary time is bound by one 2.2 GB Claude Code transcript, which one thread parses: 24 s of wall time for 46 s of CPU on 18 cores. The binary installed before this change takes 24.1 s on the same corpus. So the "8 s for 8.5 GB" in `openspec/config.yaml` is out of date for this corpus.
- The `.html` flame graph sums the same values up its tree, so after the change its frame values differed by up to 8 µ$ from the baseline's; `normalize.py check-html` allows that. Every other output was byte-identical after every group.
- One ignored manual test on the author's live logs (`exports_the_latest_session`) was dropped; the TUI export test with fixtures covers the same code.
- Two outputs of the baseline vary between its own runs, and `compare.sh` normalizes them (`normalize.py`). The values in `.collapsed` can differ by 1 µ$, because float sums are added in a different order each run. The cost cells in `ui --dump` depend on how far the background costing got.

## Workflow follow-up

- Archive the change once the author has reviewed the results.
- Propose `add-json-output`: `--json` on the commands, serializing the views of group 6, with error codes.
