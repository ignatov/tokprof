# Spec Delta

## MODIFIED Requirements

### Requirement: Checks of every push
Every push to `main` and every pull request SHALL run the workspace tests on Linux, macOS and Windows, `cargo fmt --all --check` with the repository's `rustfmt.toml`, clippy over the workspace with warnings as errors, and `openspec validate --all --strict`. The tests go on after a test binary fails, so one run shows every failing test. A release runs the same checks first, and publishes nothing when one fails.

#### Scenario: A failing check on a tag
- **WHEN** a clippy warning is on the tagged commit
- **THEN** the release workflow fails, and no release is published

#### Scenario: A test that fails only on Windows
- **WHEN** a test passes on Linux and macOS and fails on Windows
- **THEN** the checks of that push fail, and the Windows log names the test

#### Scenario: A push with unformatted code
- **WHEN** a push to `main` or a pull request holds a `.rs` file that `cargo fmt --all` would change, with the repository's `rustfmt.toml` of `max_width = 160` and `use_small_heuristics = "Max"`
- **THEN** the checks of that push fail before clippy runs, and the log names the file and the lines that differ
