# Tasks

## 1. The rule

- [ ] 1.1 Add to the query the test "a log may hold a request of the window": false only when `--since` is set and the log's last write is known and before it. Verify with unit tests beside it: before, at and after `--since`; a creation time after `--until`; no modification time; `--until` alone; no window.

## 2. The reports

- [ ] 2.1 In the report path of `summary`, `misses`, `whatif`, `tools` and `flame`, keep the "no session logs found" check on the full list, then scan and count in the read line only the logs the rule keeps. Verify with a test over the fixture logs: with `--since` after the fixtures' last write the command prints `read  in 0.0s` and an empty report with exit status 0; with `--since` before it the output equals the run without the rule.
- [ ] 2.2 Leave `pauses`, `bench`, `share`, `day` and the browser reading as before. Verify that their golden files do not change.
- [ ] 2.3 Add golden files for `summary` and `tools` with a `--since` that leaves some fixture logs unread, and verify that their stdout equals a run without the rule and that the read line counts fewer files.
- [ ] 2.4 Update the paragraph on the filters in `README.md`: a report with `--since` reads only the logs written since, and a damaged older log gets no warning then. Verify that the command in it runs as written.

## 3. On the real logs

- [ ] 3.1 Run `summary`, `misses`, `tools`, `whatif` and `flame` with `--since 2026-11-05 --until 2026-11-10` before and after the change and record that stdout is byte-identical.
- [ ] 3.2 Record wall time of `summary --since 1d` and `summary --since 7d` before and after (expected: about 27 s to under 2 s).
- [ ] 3.3 Run `cargo fmt --all`, `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings` and `openspec validate --all --strict`, and reinstall the binary.

## Workflow follow-up

- Archive the change after the author's review, and run `openspec validate --all --strict` on the merged specs.
