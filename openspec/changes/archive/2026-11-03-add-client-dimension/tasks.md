# Tasks

Every group ends with `cargo test --workspace` green, in a state that can be committed.

## 1. Before starting

- [x] 1.1 Record the baseline in a results table at the end of this file. It holds the median of 3 runs of `tokprof summary`, `Total priced` for `--agent claude --since 30d`, and the reconciliation line. Verify: the table has the numbers and the date.

## 2. Reading the client

- [x] 2.1 Give requests a client in the core model, with `unknown` as the name of a missing one. Verify: a unit test that a request without one reports `unknown`.
- [x] 2.2 In the Claude Code decoder, take each request's client from the `entrypoint` of its response line, reusing the previous string while the value repeats. Verify:
  - a fixture with `cli` responses followed by `sdk-ts` ones gives each request its own client (the "A session resumed in another client" scenario);
  - a subagent fixture gets `sdk-ts`;
  - the conformance test passes.
- [x] 2.3 In the Codex decoder, give every request of a rollout the `originator` of its `session_meta`. Verify: a fixture with `codex_exec` and one without `originator` (giving `unknown`); the conformance test passes.
- [x] 2.4 Make both sources' log heads report the first client in the lines they already read. Verify: unit tests for one Claude Code head and one Codex head.

## 3. Filter

- [x] 3.1 Put the client text into the report window and check each request against it, ignoring case, in the analysis and in `bench`. Keep the previous-request bookkeeping of the cache judgement unchanged. Verify: unit tests for "Part of a value", "Case" and "Requests without a client", and one test that a miss after a filtered-out request is still judged against it.
- [x] 3.2 Add `--client` to the command line. Apply it to the session picks and lists through the head. Verify: unit tests for "A session list" and "Latest session of a client", and that `prices` ignores it.

## 4. Views

- [x] 4.1 Collect `Spend by client` in the summary: rows per agent and client, sessions, requests, cost, share, and avoidable from the misses, which now carry their client. Print it after the reconciliation line. Verify: unit tests for "Two clients", "A session in two clients" and "Requests without a client"; the rows add up to `Total priced`.
- [x] 4.2 Compare only the sessions whose every request the filters keep in the reconciliation line. Verify: a unit test for "A client filter".
- [x] 4.3 Add the `client` line to the X-ray header. Verify: unit tests for "One client", "Several clients" and "No client"; the header order matches the spec.
- [x] 4.4 Update README: the `--client` filter, the table, the header line, and the values seen per agent with their versions. Verify: the README examples run as written.

## 5. Check on the real logs

- [x] 5.1 Run `tokprof summary --agent claude --since 30d`. Verify: `sdk-ts`, `cli` and `sdk-cli` are near the proposal's $8,534, $424 and $3.16 (allowing for new sessions); the rows add up to `Total priced`; record the table.
- [x] 5.2 Run `tokprof --client sdk-ts --agent claude --since 30d misses` and the same with `--client cli`. Verify: about 206 and 5 `idle > TTL` misses; record both.
- [x] 5.3 X-ray `da18a2d8`. Verify: the header shows `cli ×… · sdk-ts ×…`, with counts that fit the log's responses under each value; record the line.
- [x] 5.4 Run `tokprof summary --agent codex`. Verify: `codex_exec`, `codex-tui` and `Codex Desktop` show as rows, with IDE plugins under their own names; record them.
- [x] 5.5 Time `tokprof summary` 3 times. Verify: the median stays within 2% of the baseline in 1.1.
- [x] 5.6 Reinstall the binary with `cargo install --path crates/tokprof --root ~/.local`. Verify: `tokprof --client sdk-ts --agent claude --since 1d summary` runs.

## Results

| check | baseline (2026-11-03, binary at f56eb00) | after |
|---|---|---|
| `tokprof summary`, all logs, 3 runs | 25.37 s, 24.72 s, 24.93 s; median 24.93 s | |
| `--agent claude --since 30d` Total priced | $9,008, avoidable $1,031 (11.4%) | |
| reconciliation line, same filters | 307 sessions: counter $9,149, tokprof $8,793 | 308 sessions: $9,150 and $8,793 (new sessions since) |
| 5.1 `Spend by client`, `--agent claude --since 30d` | proposal: $8,534 / $424 / $3.16 | `sdk-ts` 277 sessions, 67,763 requests, $8,602, avoidable $874; `cli` 8, 2,260, $421, $157; `sdk-cli` 33, 34, $3.16, $0.07. Rows sum to $9,026 of Total priced $9,027; avoidable sums to the $1,031 of the totals line |
| 5.2 `idle > TTL` misses, `--client sdk-ts` / `--client cli` | proposal: about 206 / 5 | 210 / 6 |
| 5.3 X-ray of `da18a2d8` | log lines: 10,374 `cli`, 248 `sdk-ts` | `client    cli ×2100 · sdk-ts ×54` |
| 5.5 `tokprof summary`, all logs, 3 runs | median 24.93 s | 24.92 s, 24.78 s, 24.47 s; median 24.78 s (−0.6%) |
| 5.4 `--agent codex summary`, all history | `codex_exec`, `codex-tui`, `Codex Desktop`, IDE plugins | 8 rows: an IDE's own name, `codex-tui`, `Codex Desktop`, two IDE plugin names, `codex_exec`, the ACP adapter's package name |
