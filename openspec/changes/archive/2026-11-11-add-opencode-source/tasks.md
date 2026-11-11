# Tasks

## 1. The framework: logs that share a file

- [x] 1.1 Know a log by its locator, not its path, where tokprof keys logs: the session browser's rows and the kept logs. A session in a store is read whole each time, never kept and read on. Unit test: two sessions of one store are two rows of the browser.
- [x] 1.2 Let a source say which sessions a change of a store touched, and which files the watcher polls for a session (the store and its `-wal`), with file sources unchanged. Unit test in the watcher: a touched store gives the sessions its source names.
- [x] 1.3 Check that `crates/tokprof/tests/architecture.rs` passes: no agent's name, table or field outside its source.

## 2. The source

- [x] 2.1 Add `sources/opencode` with `Cargo.toml` (`rusqlite`, bundled) and `src/lib.rs`: its log source (OpenCode stores, OpenCode sessions, the target of a store, last activity, the sessions a change touched) and its decoder (OpenCode requests and usage, OpenCode compactions, OpenCode's own cost). Unit tests next to the code: usage and reasoning, all-zero steps, several `step-finish` parts, the model after `/`, both compaction layouts, a failed compaction with tokens, the union of both tables, a fork's copies, the subagent chain name.
- [x] 2.2 Add synthetic fixtures in `sources/opencode/tests/fixtures/` as SQL text: a 1.x store with tool steps, a subagent and a compaction; a 2.x store with a compaction and a session copied from 1.x; Anthropic usage with `cost`. No text from real logs. The tests build the stores in a temporary directory.
- [x] 2.3 Add `tests/conformance.rs`, which runs `tokprof_core::testkit::conform` over the fixtures and checks the counts. Verify: `cargo test -p tokprof-source-opencode`.
- [x] 2.4 Add the feature and the line in `crates/tokprof-sources`, so `--agent` takes `opencode`.
- [x] 2.5 Write `sources/opencode/README.md`: the stores, the tables and fields read, and the versions they were seen in.

## 3. Check on the real stores

- [x] 3.1 Run `tokprof --agent opencode summary` and `sessions` on the author's store, and on the scratch store of 1.19.3 with `XDG_DATA_HOME=/tmp/oc-run/data`. Compare requests and token sums with the numbers in proposal.md.
- [x] 3.2 Check that tokprof's dollars on the Anthropic fixture are within 1.5% of the sum of `cost`.
- [x] 3.3 Time `tokprof` with no arguments and the full corpus parse, before and after; record both.
- [x] 3.4 Run `tokprof ui` on the scratch store while opencode writes to it, and check that the session updates and the other sessions keep their rows.
- [x] 3.5 Reinstall the binary with `cargo install --path crates/tokprof --root ~/.local`. Since `tokprof-browse` changes, run the tokprof-desk checks in its folder and lock the new source there.

## 4. Docs and checks

- [x] 4.1 Update `README.md`: OpenCode in the agents, its store location and what is not read.
- [x] 4.2 Run `cargo fmt --all`, `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings` and `openspec validate --all --strict`.
