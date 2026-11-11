# Tasks

## 1. Check the open questions

- [x] 1.1 Fork and clone a short pi 0.84.2 session in a scratch directory and record whether the new file copies the earlier entries. Settle open question 2 in design.md. (Settled from pi 0.84.2's `session-manager.js` rather than a live fork: both copy.)
- [x] 1.2 Settle open questions 1 and 3 with the author; update the spec delta if an answer changes it.

## 2. The source

- [x] 2.1 Add `sources/pi` with `Cargo.toml` and `src/lib.rs`: its log source (locations of Log locations) and its decoder (Pi requests and usage, Session id and project, Compaction markers, Agent-reported cost). Unit tests next to the code: an aborted all-zero response, `cacheWrite1h`, a compaction call's model, a branch in the tree, a file without a header id.
- [x] 2.2 Add synthetic fixtures in `sources/pi/tests/fixtures/`: a session with tool calls, a compaction and a branch; an aborted response; Anthropic usage with `cost`. No text from real logs.
- [x] 2.3 Add `tests/conformance.rs`, which runs `tokprof_core::testkit::conform` over the fixtures. Verify: `cargo test -p tokprof-source-pi`.
- [x] 2.4 Add the feature and the line in `crates/tokprof-sources`, and `pi` to `--agent`. Check that `crates/tokprof/tests/architecture.rs` passes.
- [x] 2.5 Write `sources/pi/README.md`: the locations, the fields read and pi 0.84.2.

## 3. Check on the real logs

- [x] 3.1 Run `tokprof --agent pi summary` and `tokprof --agent pi sessions` on `~/.pi/agent/sessions`. Record the requests and token sums and compare them with the numbers in proposal.md.
- [x] 3.2 Check that tokprof's dollars on the Anthropic fixture are within 1.5% of the sum of `cost.total`.
- [x] 3.3 Time the full corpus parse before and after; record both.
- [x] 3.4 Reinstall the binary with `cargo install --path crates/tokprof --root ~/.local`. Since `tokprof-browse` lists the agents, run the tokprof-desk checks in its folder.

## 4. Docs and checks

- [x] 4.1 Update `README.md`: pi in the agents and its log location.
- [x] 4.2 Run `cargo fmt --all`, `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings` and `openspec validate --all --strict`.
