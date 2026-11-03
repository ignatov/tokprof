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
cargo clippy --workspace --all-targets -- -D warnings
cargo install --path crates/tokprof --root ~/.local    # the binary the author runs
```

GitHub Actions check every push to main and every pull request (`.github/workflows/ci.yml`): the tests on Linux, macOS and Windows, the clippy line above, and `openspec validate --all --strict`. rustfmt is not checked, because the code does not follow its default style. On Windows a path prints with `\`, so a test must not expect `/` in a printed path.

## Releases

A release is a tag. Set `version` in the root `Cargo.toml`, build once so that `Cargo.lock` follows, and commit both. Push main, then a tag of that version: `git tag v0.2.0 && git push origin v0.2.0`. `.github/workflows/release.yml` checks the tag against the version, runs the CI checks, builds the macOS, Linux and Windows binaries and publishes them as a GitHub release with `SHA256SUMS`. The release notes list the commit subjects since the previous tag, without the `Archive <change>` commits. A tag with a hyphen, such as `v0.2.0-rc.1`, makes a pre-release. A run by hand (Actions → Release → Run workflow) builds the archives and keeps them as artifacts, without a release. GitHub's macOS runners are short at times: a job that no runner takes is cancelled after about 15 minutes with "The job was not acquired by Runner". Then run `gh run rerun <run id> --failed`; a tag's release is published once the jobs pass.

## Layout

The framework is in `crates/`: `tokprof-core` (the library), `tokprof-logio` (neutral log reading), `tokprof-sources` (the registry) and `tokprof` (the CLI). Every agent's log source is its own crate under `sources/`. Only a source may name its agent, its tools or its log fields; `crates/tokprof/tests/architecture.rs` fails otherwise, and it also checks the dependency edges.

A new log source:
1. `sources/<name>/` with `Cargo.toml`, `src/lib.rs` (its `LogSource` and `Decoder`), `tests/fixtures/` with synthetic logs, and `tests/conformance.rs`, which runs `tokprof_core::testkit::conform` over the fixtures.
2. A `README.md` in it that names the log locations, the fields it reads and the agent versions they were seen in.
3. A feature and one line in `crates/tokprof-sources`.
