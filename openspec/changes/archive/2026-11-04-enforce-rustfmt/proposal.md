# Proposal

## Why

On 2026-11-03 the author asked to turn rustfmt on. The whole codebase is formatted once, after every other session has merged its work, and rustfmt is the default from then on. The code did not follow rustfmt's style: on main at `7bb51d3`, `cargo fmt --all --check` gave 1,643 diffs, and 2,121 of the 27,122 lines in `.rs` files were over 100 characters. Without a check, every session formats in its own way. A formatting pass rewrites lines in nearly every file, so it can land only while no other session holds work.

## What Changes

- **`rustfmt.toml`** at the repository root: `max_width = 160` and `use_small_heuristics = "Max"`. Of the five configs tried, it changes the fewest lines of the code as it was. rustfmt takes the edition, 2024, from `Cargo.toml`.
- **One formatting commit**, "Format the code with rustfmt": `cargo fmt --all` over every `.rs` file. It changes no behavior.
- **CI checks the formatting.** The clippy job installs rustfmt and runs `cargo fmt --all --check` before clippy. A push or a pull request with unformatted code fails the checks, and so does a release, which runs the same checks first.
- **AGENTS.md**: `cargo fmt --all` is the first command of "Build and check", and the text says that CI checks the formatting.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `release-builds`: "Checks of every push" adds `cargo fmt --all --check`.

## How it is checked

- Each config was tried on main at `7bb51d3`: `cargo fmt --all`, then `git diff --shortstat`, then a reset. Lines changed are the lines added plus the lines removed.

| config | files | lines added | lines removed | lines changed |
|---|---|---|---|---|
| `max_width = 100`, the default | 57 | 16,426 | 3,258 | 19,684 |
| `max_width = 120` | 56 | 12,300 | 2,416 | 14,716 |
| `max_width = 140` | 56 | 9,396 | 1,878 | 11,274 |
| `max_width = 160` | 56 | 7,499 | 1,601 | 9,100 |
| `max_width = 160`, `use_small_heuristics = "Max"` | 50 | 1,939 | 908 | 2,847 |

- After `cargo fmt --all` with the chosen config, these pass: `cargo fmt --all --check`, `cargo test --workspace --locked`, `cargo clippy --workspace --all-targets --locked -- -D warnings` and `openspec validate --all --strict`.
- The formatting commit touches only `.rs` files and `rustfmt.toml`.
- No behavior changes. On the author's real logs, `tokprof --since 2026-10-04 --until 2026-11-03 summary` prints the same text from release builds of main before and after the formatting. The share files that the two builds write over the log sources' fixture logs are the same, byte for byte.
- In a copy of `openspec/`, archiving the open changes in order, add-release-builds, add-windows-builds, add-team-survey, add-share-friction and then this one, leaves `openspec validate --specs --strict` green.

## Out of scope

- rustfmt options that only nightly rustfmt has, such as import grouping or comment wrapping.
- Formatting Markdown, YAML, TOML and shell scripts.
- A pre-commit hook and editor settings.
- Pushing, which the author does.

## Impact

- Every `.rs` file of the workspace: whitespace and line breaks only. Work that branched from an older main conflicts with the formatted lines. Rerun `cargo fmt --all` on the new main instead of merging those hunks by hand.
- New `rustfmt.toml`; `.github/workflows/ci.yml`; AGENTS.md.
- tokprof's behavior and output do not change.
