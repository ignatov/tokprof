# tokprof

A token, cost and prompt-cache profiler for coding-agent session logs, in Rust. README.md says what it does and how to use it.

## Changes go through OpenSpec

`openspec/specs/` describes what tokprof does now, one capability per folder (`openspec list --specs`). `openspec/config.yaml` holds the project constraints and the rules for each artifact.

1. A change of behavior starts as a proposal, before any code: `/opsx:propose <idea>` in Claude Code, `$openspec-propose` in Codex. Use `/opsx:explore` first when the idea is not settled yet.
2. The author approves the proposal. Then `/opsx:apply` implements its tasks.
3. `/opsx:archive` merges the change's deltas into `openspec/specs/`.

A change to existing behavior is a MODIFIED delta of the requirement in `openspec/specs/<capability>/spec.md`, not a new requirement next to it.

A fix that restores specified behavior needs no proposal. If the code is right and the spec is wrong, fix the spec in the same edit.

`openspec validate --all --strict` must pass.

## Build and check

```sh
cargo test --workspace
cargo install --path crates/tokprof --root ~/.local    # the binary the author runs
```

## Layout

The framework is in `crates/`: `tokprof-core` (the library), `tokprof-logio` (neutral log reading), `tokprof-sources` (the registry) and `tokprof` (the CLI). Every agent's log source is its own crate under `sources/`. Only a source may name its agent, its tools or its log fields; `crates/tokprof/tests/architecture.rs` fails otherwise, and it also checks the dependency edges.

A new log source:
1. `sources/<name>/` with `Cargo.toml`, `src/lib.rs` (its `LogSource` and `Decoder`), `tests/fixtures/` with synthetic logs, and `tests/conformance.rs`, which runs `tokprof_core::testkit::conform` over the fixtures.
2. A `README.md` in it that names the log locations, the fields it reads and the agent versions they were seen in.
3. A feature and one line in `crates/tokprof-sources`.
