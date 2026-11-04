# Tasks

Every group ends with `cargo fmt --all`, `cargo test --workspace` and `cargo clippy --workspace --all-targets -- -D warnings` green. Names from the people's files never go into tests or docs.

## 1. The file

- [x] 1.1 In the core, rescale every `usd`/`*_usd` number by the file's spend and every `tokens`/`*_tokens` number by its tokens, to 7 significant digits, leaving context sizes and every other number as they are. Verify: the unit tests "every amount is a share of the two totals", "context sizes stay in tokens" and "counts stay as they are".
- [x] 1.2 Write format 3, and `totals` with `--dollars`. Verify: the unit test "shares times the totals give the amounts back within $0.005 and 1 token per million" on the fixture logs.
- [x] 1.3 Write one line per object whose values hold no object. Verify: the unit test "the layout holds the same value as the pretty file" (parse both, compare).
- [x] 1.4 Print the report in dollars from the totals in memory, then the totals line. Verify: a test in `crates/tokprof/tests/share.rs` for both forms of the line, and that the file holds no `totals` without `--dollars`.

## 2. The pool

- [x] 2.1 Read format 3; divide a file with totals by them when a pooled file lacks totals, and weigh each file as one unit. Verify: the unit test "a heavy file and a light one give the mean share when one lacks totals" (10% of $11,000 and 30% of $300 give 20.0%).
- [x] 2.2 Print dollars as shares and dollars per active hour as shares per hour in that case, and the `spend` line of the header. Verify: a render test that the report of a pool without totals prints no `$`, and the header line of the spec.
- [x] 2.3 Keep today's report when every file holds dollars. Verify: the existing pool tests pass unchanged.

## 3. Release smoke and docs

- [x] 3.1 Make `ci/smoke.sh` write `share --dollars`. Verify: running it on a local build writes a file with `totals`.
- [x] 3.2 Update `docs/survey.md`: shares instead of dollars, what stays and why, `--dollars` for pooled dollars and how to collect those. Verify: the page says all of these.
- [x] 3.3 Update README's share and pool parts. Verify: format 3, `--dollars`, the totals line and the pool without totals are described.

## 4. Checks on the real logs

- [x] 4.1 Run `openspec validate --all --strict`, and archive this change in a copy of `openspec/`. Verify: both pass, with no requirement over 500 characters after the merge.
- [x] 4.2 Run `tokprof --since 30d share -o /tmp/s.json` and `share --dollars -o /tmp/d.json` on the author's logs. Verify: the shares times the printed totals give every number of `/tmp/d.json` back within $0.005 and 1 token per million, every key and name is the same, and record both sizes.
- [x] 4.3 Pool each file alone. Verify: every share of every table is the same in both reports (`idle > TTL` 7.0%, `sessions longer than a day` 64.5%, `effort max or xhigh` 80.4%, `skill:code-review` 15.0%).
- [x] 4.4 Write the mock team (the author's 30 days cut by project into 8 files) with and without `--dollars`. Verify: `people` of every pain is the same in both pools, and record them below.
- [x] 4.5 Reinstall the binary with `cargo install --path crates/tokprof --root ~/.local`. Verify: `tokprof share --help` lists `--dollars`.

## Results

Author's logs, 2026-10-05 – 2026-11-04 (`--since 2026-10-05 --until 2026-11-04`), $11,400.00 and 23.50B tokens:

- Sizes: tokprof 0.1.2 writes 149,063 bytes in 5,925 lines; `share` writes 81,359 bytes in 568 lines; `share --dollars` 81,416 bytes.
- Against the file of tokprof 0.1.2 over the same days: every key, name, count, day, hour and duration is the same; the shares times the totals give every dollar back within $0.00057 (worst: a row of `prompt_sizes`) and every token amount within 0.46 per million.
- `tokprof pool` over the file of `--dollars` prints the same report, byte for byte, as tokprof 0.1.2 over its own file. Over the file of shares: `idle > TTL` 7.0%, `skill:code-review` 15.1%, `effort max or xhigh` 80.7%, `sessions longer than a day` 64.9%, the same shares as in dollars, and no `$` in any line.
- Mock team (8 files by project, same days), `people` in `Pains side by side`, in dollars and in shares alike: effort max or xhigh 7 of 8, cold returns 7 of 8, failures 7 of 8, sessions longer than a day 5 of 8, contexts of 400k+ 5 of 8, interrupts 4 of 8, waits for retries 3 of 8, the agent asks 2 of 8. In shares, `pooled` is the mean share: effort 74.6%, sessions longer than a day 46.3%, cold returns 8.0% of spend.
- `ci/smoke.sh` on a local archive writes a file with `totals` over the fixture logs ($0.29, 180.2k tokens).
- The archive in a copy of `openspec/` validates: 17 specs, no requirement over 500 characters.
