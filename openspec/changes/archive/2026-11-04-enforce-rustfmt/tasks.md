# Tasks

The author's request of 2026-11-03 approves this change. The formatting waits until no other session holds work: `bd workspace` shows no active lease and no local changes. Formatting is regenerated, not merged: when main moves before the commits land, the lease moves onto the new main and `cargo fmt --all` runs again.

## 1. Format the code

- [x] 1.1 Wait until `bd workspace` shows no active lease and `local_changes` is 0, checking about every 10 minutes for up to 8 hours. Verify: the condition held, and held once more right before the lease was taken.
- [x] 1.2 On main at `7bb51d3`, try five configs in `rustfmt.toml`: `max_width` 100 (the default), 120, 140 and 160, and 160 with `use_small_heuristics = "Max"`. For each, run `cargo fmt --all`, record `git diff --shortstat`, and reset. Verify: the first results table.
- [x] 1.3 Write `rustfmt.toml` with the config that changes the fewest lines, with a one-line comment that says why, and run `cargo fmt --all`. Verify: `git diff --stat` touches only `.rs` files and `rustfmt.toml`; `cargo fmt --all --check`, `cargo test --workspace --locked`, `cargo clippy --workspace --all-targets --locked -- -D warnings` and `openspec validate --all --strict` pass.
- [x] 1.4 Build release binaries of main before and after the formatting. Run both on the real logs with `tokprof --since 2026-10-04 --until 2026-11-03 summary`, and both over the fixture logs with `tokprof share`, as `ci/smoke.sh` runs it. Verify: the summaries are the same text, and `ci/smoke.sh --compare` says the share files hold the same numbers.
- [x] 1.5 Commit as "Format the code with rustfmt", with the config and the numbers in the body. Verify: the commit holds only `.rs` files and `rustfmt.toml`.

## 2. Check the formatting in CI

- [x] 2.1 In `.github/workflows/ci.yml`, the clippy job installs the toolchain with `--component clippy,rustfmt` and runs `cargo fmt --all --check` before clippy. Its other steps stay as they are. Verify: the file parses as YAML, and the diff changes only the install line, the new step and the comment at the top.
- [x] 2.2 In AGENTS.md, `cargo fmt --all` is the first command of "Build and check", and the sentence that said rustfmt is not checked now says that CI checks the formatting. Verify: the commands are in that order, and AGENTS.md no longer says rustfmt is not checked.
- [x] 2.3 Write this change's delta of "Checks of every push" from its newest text, in add-windows-builds, and add the formatting check and its scenario. Verify: the description stays at 500 characters or less; `openspec validate --all --strict` passes; in a copy of `openspec/`, archiving the open changes in order (add-release-builds, add-windows-builds, add-team-survey, add-share-friction, then enforce-rustfmt) and then `openspec validate --specs --strict` passes.

## Results

Each config on `7bb51d3`. Lines changed are the lines added plus the lines removed.

| config | files | lines added | lines removed | lines changed |
|---|---|---|---|---|
| `max_width = 100`, the default | 57 | 16,426 | 3,258 | 19,684 |
| `max_width = 120` | 56 | 12,300 | 2,416 | 14,716 |
| `max_width = 140` | 56 | 9,396 | 1,878 | 11,274 |
| `max_width = 160` | 56 | 7,499 | 1,601 | 9,100 |
| `max_width = 160`, `use_small_heuristics = "Max"` | 50 | 1,939 | 908 | 2,847 |

| check | result |
|---|---|
| 1.1 the wait | 2026-11-03: four leases at the first check, 17:20; none and no local changes at 19:10, after 1 h 50 min; none again right before the lease |
| 1.3 `git diff --stat` | 50 of the 59 `.rs` files and `rustfmt.toml`, nothing else |
| 1.3 `cargo fmt --all --check` | 1,643 diffs before, none after |
| 1.3 `cargo test --workspace --locked` | 402 tests pass, none fail |
| 1.3 `cargo clippy --workspace --all-targets --locked -- -D warnings` | no warning |
| 1.3 `openspec validate --all --strict` | 19 of 19 items pass |
| 1.4 `summary` on the real logs, before and after | the same 142 lines; each run read 1,094 Claude Code logs (6.0 GB) and 797 Codex logs (2.6 GB) in about 25 s |
| 1.4 `share` over the fixtures, before and after | the same bytes; `ci/smoke.sh --compare`: `the 2 files hold the same numbers` |
| 2.1 `ci.yml` | parses as YAML; the clippy job runs checkout, the install with `clippy,rustfmt`, the cache, `cargo fmt --all --check`, then clippy |
| 2.3 the description of "Checks of every push" | 413 characters |
| 2.3 `openspec validate --all --strict` | 20 of 20 items pass |
| 2.3 the five archives in a copy, then `openspec validate --specs --strict` | each archive succeeds, this one with `~ 1 modified` in `release-builds`; 17 of 17 specs pass, and "Checks of every push" has the three scenarios |

## Workflow follow-up

- The author pushes. The clippy job on GitHub then runs the formatting check, and it must pass.
- Archive this change after add-release-builds and add-windows-builds: `openspec archive enforce-rustfmt -y`, then `openspec validate --specs --strict`.
