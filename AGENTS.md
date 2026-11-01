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
cargo test
cargo install --path . --root ~/.local    # the binary the author runs
```
