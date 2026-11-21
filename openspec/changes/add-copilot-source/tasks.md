# Tasks

## 1. Settle the open questions

- [x] 1.1 Settle open questions 1, 2, 4 and 6 of design.md with the author; update the spec deltas if an answer changes them. Fold in the credits of a task's later sessions on its first session.
- [x] 1.2 Leave open questions 3 and 5 to changes of their own.

## 2. Log reading

- [ ] 2.1 Read `.gz` logs in `tokprof-logio` (`open`, `for_each_line`, `for_each_line_back`) with `flate2`, as `.zst`; reading on from an offset gives `None` for them. Unit tests next to the code: a gzip log, a log of several gzip members, a file that is no gzip.

## 3. Core

- [ ] 3.1 Add the flag of a request whose log gives no usage. Let the analysis pass such a request with a response: no tokens, no price, no cache verdict, never the base of a later verdict. Unit tests: a request without usage reaches the sinks with its calls; it adds no tokens or dollars; it is not judged and the request after it is judged against the request before it.
- [ ] 3.2 Count a request without usage and with a response as answered in busy time, slow requests and the call log's `running` and `no result`. Unit tests next to the code.
- [ ] 3.3 Add the sizes the log gives, by block. Use them for blocks entered by requests without usage; leave a block without one unsized, out of the means, and show `–` when a row has no sized block. Unit tests: the mean over sized blocks; a row without any.
- [ ] 3.4 Add the kind `environment setup`, the group `environment` (and its name in the usage error of `--group`) and the busy kind `environment setup` after `subagents`. Unit tests: the group of the kind; busy time while a setup call runs.
- [ ] 3.5 Count calls that no request made in the tool table, the groups table and the call log, with an empty `req`. Unit tests next to the code.
- [ ] 3.6 Show `–` for `rent`, `trips` and `after` of rows without spend from requests with usage, and no hint for them.
- [ ] 3.7 Show the own cost of sessions without usage: the summary rows and the own-cost line, the X-ray's `cost` line and the blocks it leaves out, and the `*` cells and the note of `tokprof sessions --cost`.
- [ ] 3.8 Check that the golden tests of the other agents do not change: `cargo test -p tokprof --test golden` without `TOKPROF_BLESS`.

## 4. The source

- [ ] 4.1 Add `sources/copilot` with `Cargo.toml` and `src/lib.rs`: its log source (folder of Copilot log folders, records of Copilot session records, the claim of a target) and its decoder (Copilot log lines, requests, model, billed credits, calls and results, call outcomes, subagents, environment setup, prompts and turns, project and title, sizes). Unit tests next to the code: milliseconds and string times; a response with two calls; a call without a `tool` object; a marker; a custom agent; a subagent of a `task` call; a record in another folder; a record without `usage`; the model id rules.
- [ ] 4.2 Add synthetic fixtures in `sources/copilot/tests/fixtures/`, in the planned layout `YYYY-MM-DD/<id>.sse.gz` and `<id>.json`, and one plain `.sse` without a record: setup steps, MCP server starts, parallel calls, a failed and a denied call, a background `bash`, a `task` subagent with its markers, a custom agent, the platform's last prompt and a `turn-<n>` answer. No text from real logs.
- [ ] 4.3 Add `tests/conformance.rs`, which runs `tokprof_core::testkit::conform` over the fixtures. Verify: `cargo test -p tokprof-source-copilot`.
- [ ] 4.4 Add the feature and the line in `crates/tokprof-sources`, and `copilot` to `--agent`, outside the default list. Check that `crates/tokprof/tests/architecture.rs` passes.
- [ ] 4.5 Write `sources/copilot/README.md`: the folder and its variables, the fields read, the kind and the input field of each tool, and the dates of the logs they were seen in.

## 5. Check on the real logs

- [ ] 5.1 With `TOKPROF_COPILOT_DIR` set to the author's archive, run the binary of `target/`: `tokprof --agent copilot sessions --cost`, `summary`, `tools` and `scan <id> --calls` on three sample sessions. Record the numbers and compare them with the script's: sessions, requests (15,184 on 2026-11-21), calls per tool and their minutes, failed and denied calls, the records' credits.
- [ ] 5.2 Check by hand on one session that the busy time's `environment setup`, `model` and `building and testing` add up to the time from the first line to the last answer.
- [ ] 5.3 Time `tokprof --agent copilot summary` over the 867 logs, and the full corpus parse and `tokprof` with no arguments before and after; record all three.
- [ ] 5.4 Run `tokprof summary` on the author's corpus before and after; the totals of the other agents must be the same.
- [ ] 5.5 Since `tokprof-browse` lists the agents, run the tokprof-desk checks in its folder (fmt, test, clippy). The author reinstalls the binary and the app himself.

## 6. Docs and checks

- [ ] 6.1 Update `README.md`: the agent, its folder, that its logs give no usage, and which reports show what for it.
- [ ] 6.2 Run `cargo fmt --all`, `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings` and `openspec validate --all --strict`.
