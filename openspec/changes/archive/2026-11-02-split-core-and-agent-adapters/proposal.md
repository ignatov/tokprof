# Proposal

## Why

tokprof needs a library that other programs can use: Rust code, and JSON clients such as the IntelliJ side, a statusline and the cache-tax mod. Today the package has a `lib.rs`, but the library is the CLI cut into modules:
- it exports the TUI;
- its main functions return ANSI text;
- the scan pipeline and the session filters live in `main.rs`;
- the Codex thread index is a global `static` bound to `CODEX_HOME`.

Agent-specific logic is spread over the core and the UI:
- the cache lifetime is chosen by agent;
- the advice knows Claude Code and Codex tool names;
- the handoff note parses both log formats itself;
- the UI picks colors by agent.

Adding Junie or OpenCode today would touch about 10 files outside the new adapter. Three of those places fail silently: a file path is read as a Claude Code transcript unless it is a Codex rollout, the session list reads both formats in its quick read, and spend of an unknown agent is booked to Claude Code.

## What Changes

- The repository becomes a Cargo workspace. The framework lives in `crates/`, and every agent in its own crate under `sources/`:
  - `tokprof-core`, the library: the session model, pricing, analysis, the X-ray, advice and bench as data, and the profile writers;
  - `tokprof-logio`: neutral helpers for reading logs;
  - `sources/claude-code` and `sources/codex`: one crate per log source. A later agent is one more crate there, and nothing outside it changes;
  - `tokprof-sources`: the registry of sources, a feature per source;
  - `tokprof`: the CLI and the TUI.
- Every piece of agent-specific logic moves behind one adapter interface: a log source, which finds and opens logs, and a pure decoder, which turns their records into sessions. A source may report sessions of several agents, and one session seen through two sources is counted once. An IDE host's log of every ACP session is such a source (design.md, Context). The core and the UI only see neutral data:
  - what the agent is called;
  - what its log can tell (capabilities);
  - its harness facts, such as the pickup calibration;
  - neutral kinds of tools, which follow ACP's tool kinds, and of reminders, origins and compaction triggers.
- Prompt-cache rules (lifetimes, the 20-block lookback, two write lifetimes for whatif) belong to the API a request was sent to, not to the agent. Each adapter sets the API per request: Claude Code sends to the Anthropic API, Codex to the OpenAI API. Results stay the same for both agents today.
- Reports become data first. The CLI renders text from that data. The future `--json` output will serialize the same data.
- The library reads no environment variables and no clock. Paths and `now` come from the caller. The profile directory, the user price file, colors and the clipboard move to the CLI.
- The install command becomes `cargo install --path crates/tokprof --root ~/.local`. README, AGENTS.md and `openspec/config.yaml` change with it.

The user-visible outcome is that nothing a command prints changes. That covers `tokprof`, `scan`, `sessions`, `summary`, `misses`, `whatif`, `flame`, `bench`, `prices`, `handoff` and the `ui`.

### How it is checked on the real logs

- **Output.** Before the first phase, a binary built from the current commit records the outputs of every command on the author's logs. These are the 8.5 GB in `~/.claude` and `~/.codex`. After each phase, the new binary's outputs must be byte-identical. The only exception is X-ray lines that depend on the minute they run (`cold in`, `cold for`).
- **Profiles.** `flame` writes the same `.collapsed` and `.html`. The `.jfr` gives the same `jfr summary` counts.
- **Speed.** Parse time of `tokprof summary` stays within 5% of the baseline, about 8 s, as a median of 3 runs.
- **Cost.** The reconciliation with Claude Code's cost counter stays within 1.5% for the same 246 sessions.
- **Tests.** `cargo test` passes with at least as many tests as before.

## Capabilities

### New Capabilities

None. The library API is described in rustdoc. Specs describe observable behavior, which does not change.

### Modified Capabilities

None. The change sets `skip_specs: true`.

The rule that cache lifetimes follow the API is behavior only once an agent can send to several APIs. The specs change in that change, with the first such adapter.

## Out of scope

- The `--json` output and its error codes. That is the next change, `add-json-output`. It serializes the data types this change introduces.
- Publishing to crates.io, `cargo-semver-checks` and typed library errors. `anyhow` stays for now.
- Junie, OpenCode and IDE host sources. The interface was checked against their logs on this machine (design.md, Context), but none of them is built.
- Feeding only the appended lines of a growing log to the TUI or a statusline. The decoders allow it; this change keeps parsing whole logs.
- Reading Codex's logged model provider, so that a Codex request to a non-OpenAI API gets that API's rules.
- Drawing the TUI from data. It keeps converting the ANSI X-ray, as today.
- Moving agent-specific requirements into per-agent specs.

## Impact

- **Code:** every file under `src/` moves. `main.rs`, `ui.rs` and `watch.rs` go to the CLI; `adapters/claude.rs` and `adapters/codex.rs` become the two source crates, and `adapters/util.rs` splits between `tokprof-logio`, the core's labels and the two sources. `prices.toml` and `assets/async-profiler/` move into `tokprof-core`, which embeds them.
- **Dependencies:**
  - `ratatui`, `ansi-to-tui`, `notify` and `clap` leave the library.
  - `prost` and `flate2` sit behind a default `export` feature of `tokprof-core`.
  - `serde_json`, `ruzstd` and `sha2` are used for log reading in `tokprof-logio` and the sources.
- **Process:** the move touches every source file. It is done in one step while no other session has uncommitted work in `src/`.
- **Docs:** the install lines in README, AGENTS.md and the apply guidance in `openspec/config.yaml`, and README's links to `prices.toml` and the vendored flame graph.
