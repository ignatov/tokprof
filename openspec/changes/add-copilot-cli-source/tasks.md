# Tasks

## 1. Rename the cloud agent

- [x] 1.1 Move `sources/copilot` to `sources/copilot-cloud` with `git mv`; name the crate `tokprof-source-copilot-cloud` and its feature in `crates/tokprof-sources` `copilot-cloud`. In it, make the agent's name, the source's name and `SessionRef.source` `copilot-cloud`; keep the title, the folder and `TOKPROF_COPILOT_DIR`. Update its unit tests and `tests/conformance.rs`; verify with `cargo test -p tokprof-source-copilot-cloud`.
- [x] 1.2 Accept `copilot-cloud` wherever the CLI, `tokprof ui` and the window list agent names (help, the `unknown agent` hint). Rename the cloud golden tests in `crates/tokprof/tests/golden.rs` to `--agent copilot-cloud` and bless them; verify that the diff of `crates/tokprof/tests/golden` shows only the agent name.
- [x] 1.3 Update `sources/copilot-cloud/README.md` and the cloud agent's section and `--agent` row of `README.md` to `copilot-cloud`; verify that the documented commands run on the fixtures as written.

## 2. The Copilot CLI source

- [x] 2.1 Create the new `sources/copilot` (`tokprof-source-copilot`) with `Cargo.toml` and add it to the workspace; verify with `cargo build -p tokprof-source-copilot`.
- [x] 2.2 Write the log source: the root from `COPILOT_HOME`, `discover`, `session_at`, `claim`, `watched_paths`, `touched`, `head` (id, project, title from `workspace.yaml`), `first_active`, `last_active`, and the agent facts of design.md. Unit tests next to the code: a folder without `events.jsonl`; `COPILOT_HOME` set; a log claimed outside the folder; a title, and a folder without `workspace.yaml`.
- [x] 2.3 Write the decoder for requests: usage records joined to their responses, the usage fields, model, times, cache lifetime, version, own cost, responses without a record, messages without an `apiCallId`. Unit tests next to the code, one per scenario of `Copilot CLI requests and usage`, `Copilot CLI responses without a usage record` and `Copilot CLI's own cost`.
- [x] 2.4 Add compaction: the marker and the compaction call. Unit tests: the scenario of `Copilot CLI compaction calls`; a failed compaction marks nothing; the request after a compaction is marked.
- [x] 2.5 Add calls: times, results, outcomes, background, questions, kinds, input heads and MCP tools. Unit tests, one per scenario of `Copilot CLI calls and results`, `Copilot CLI call outcomes` and `Copilot CLI tool kinds and input heads`.
- [x] 2.6 Add subagent chains and the typed messages. Unit tests, one per scenario of `Copilot CLI subagents` and `Copilot CLI prompts and turns`.
- [x] 2.7 Add synthetic fixtures under `tests/fixtures/session-state`: a session with two models, a subagent, a compaction, a failed, a denied and a refused call and a command in the background; a session of an older version without usage records; a folder without a log. Add `tests/conformance.rs` over them; verify with `cargo test -p tokprof-source-copilot`.
- [x] 2.8 Write `sources/copilot/README.md`: the location, every field read, the kinds and input heads table, what is not read, and Copilot CLI 1.0.85–1.0.116; verify that every field the decoder reads is named in it.

## 3. Registry and CLI

- [x] 3.1 Add the feature `copilot` and its line in `crates/tokprof-sources`, in the default list, after OpenCode and before the cloud source. Add the source's words to the allowed list of `crates/tokprof/tests/architecture.rs` if it needs one; verify with `cargo test -p tokprof --test architecture`.
- [x] 3.2 Make `copilot` a default agent in the CLI's `--agent` help. Give the test home a `COPILOT_HOME` that points at the new fixtures, and remove the variable from the tests that must see no CLI logs. Verify that `tokprof --agent foo sessions` still fails with `unknown agent "foo"`.
- [x] 3.3 Add golden tests of the CLI's fixtures: `summary`, `tools`, `misses` and `scan <id> --calls` with `--agent copilot` (`sessions` prints local times, so it has no golden). Price the fixtures' models in `tests/golden/prices.toml`. Bless them and review the diff: in the other golden files, the rows of the other agents keep their numbers, and only totals, shares and the line about the logs read change, since the CLI's fixtures join the default agents.
- [x] 3.4 Update `README.md`: the first line, the `--agent` row and its default, a section for the Copilot CLI (where the logs are, what its reports show, the own-cost comparison), and the note on which agents give call times. Verify that the documented commands run as written.

## 4. Check on the real logs

- [x] 4.1 Write a script outside the repository that sums, per session and in total, the usage records of `~/.copilot/session-state` (the four token sums, the credits, the counts by model, the calls by tool and outcome). Run it and `tokprof --agent copilot summary`, `sessions --cost` and `tools` back to back; record both sets of numbers in the task notes. They must match as "How it is checked" of proposal.md says.
- [x] 4.2 Check the four ended sessions against their `session.shutdown` totals: 20.61502, 14.00253, 105.1672 and 740.46898 credits; record tokprof's figures.
- [x] 4.3 Run `tokprof --agent copilot scan ab66ecb4`, `scan 54061b9a` and `scan 0c396a3a`; record the subagent chain and its 14 requests, the compaction call and the marked request, and the requests without usage.
- [x] 4.4 Run `tokprof --agent copilot-cloud summary` and `tools` on the saved cloud logs; verify the totals of `add-copilot-source` (867 sessions on its date; more now) still hold for the same files, with only the agent name changed.
- [x] 4.5 Time `tokprof` with no arguments and `tokprof summary` over the full corpus before and after; record both. No noticeable change: under a second, and about 8 s.
- [x] 4.6 Run `cargo fmt --all`, `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, the same three in `crates/tokprof-desk`, and `openspec validate --all --strict`; all pass. Reinstall with `cargo install --path crates/tokprof --root ~/.local`.

## Workflow follow-up

- Archive the change after the author's review; run `openspec validate --all --strict` after the specs are merged.
- Name the rename of `--agent copilot` in the notes of the next release.

## Numbers recorded

On the author's `~/.copilot/session-state`, 2026-11-26 16:40, the script and tokprof run back to back (the session of this change was still growing):

- 12 folders, 10 logs, all 10 listed by `tokprof --agent copilot sessions`; `read copilot 10 files (0.0 GB)`.
- Script: 542 requests with usage (541 records and 1 compaction call) and 7 responses without a record; 11,584 uncached, 81,035,626 cache-read, 3,914,888 cache-written and 552,742 output tokens; 6,195.38 credits, $61.95. By model: `claude-fable-5.2` 291, `claude-sonnet-5.5` 150, `claude-opus-5.5` 101.
- `tokprof --agent copilot summary`: `claude-fable-5-2` 291 requests $50.27, `claude-opus-5-5` 101 $7.40, `claude-sonnet-5-5` 150 $4.28; `claude-fable-5-1` 5 and `claude-sonnet-5` 2 requests with `–`. `Total priced: $61.95`. `GitHub's billed credits for 8 sessions: $61.95; tokprof for the same sessions: $61.95`.
- Ended sessions, `sessions --cost` against `session.shutdown`: `8d362bc6` $0.21 (20.61502 credits), `a0ab881f` $0.14 (14.00253), `f1e3d128` $1.05 (105.1672), `d4722376` $7.40 (740.46898).
- `tokprof --agent copilot tools`: 575 calls, as many as `tool.execution_start` lines; shell 474 (script: `bash` 435, `read_bash` 36, `stop_bash` 3); `failed 22 calls` (script: 8 with code `failure` and 14 commands with a non-zero exit code), `you refused 1`, `rules denied 4`.
- `scan ab66ecb4`: `main $26.52 · subagents $3.42`; the chain `subagent: review-factory-template` holds the $3.42.
- `scan 54061b9a`: a `compaction` row of $0.11 in the spend; `misses --expected` shows `compaction (expected)` 1 miss.
- `scan 0c396a3a` (Copilot CLI 1.0.85): 2 requests of `claude-fable-5-1`, `cost      the log gives no usage`; `5f1e2a67` has the other 5.
- `tokprof --agent copilot-cloud summary`: 867 files, 6,689 and 8,495 requests (15,184), `GitHub's billed credits for 743 sessions: $1,294`; `tools`: `failed 416 calls · rules denied 29`. The same figures as before the rename.
- Time, the installed 0.1.6 against this build: `tokprof summary` over 10.3 GB of Claude Code and 2.6 GB of Codex logs 11.4 s against 10.4 s; `tokprof` with no arguments 0.5 s.
