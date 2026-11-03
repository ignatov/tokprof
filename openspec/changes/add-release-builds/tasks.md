# Tasks

The release workflow of dbd6f3d and 58c73e0 is the base. Pushing to GitHub, a run by hand there and pushing a tag are the author's calls: ask before each. Every group ends with `cargo test --workspace` green, in a state that can be committed.

## 1. Smoke run over the fixtures

- [x] 1.1 Add `ci/smoke.sh <archive> <out>`. It unpacks the archive into a new temporary folder, runs `tokprof --version`, then `tokprof share -o <out>` over the fixture logs. `HOME`, `XDG_CONFIG_HOME` and `TOKPROF_DIR` point into the folder, and `TZ=UTC` is set. Verify: on the author's Mac, a release build packed as the workflow packs it prints its version and writes a share file with both agents. Record the time in the results table.
- [x] 1.2 Add `ci/smoke.sh --compare <file>…`. It drops `platform` with `jq` and compares each file with the first. Verify: two copies of the file from 1.1 pass. A copy with one number changed fails, and the message names that copy.
- [x] 1.3 Run the smoke step in the build job of `release.yml` for every target but `x86_64-apple-darwin`, and upload each file as `smoke-<target>`. Add a `compare` job, and make `publish` need it. Verify: both workflow files parse as YAML, and in `release.yml` `publish` needs `checks`, `build` and `compare`.

## 2. Attestations

- [x] 2.1 In the publishing job, attest `dist/*.tar.gz` with `actions/attest@v4` before `gh release create`. Give the job `id-token: write`, `attestations: write` and `artifact-metadata: write`. Verify: the YAML parses, the step comes before the release is created, and no other job gets the new permissions.

## 3. README

- [x] 3.1 In Install, next to `SHA256SUMS`, add the check of where an archive was built: `gh attestation verify tokprof-<target>.tar.gz -R ignatov/tokprof`. Verify: the line matches the scenario "Checking where an archive was built".

## 4. Check

- [x] 4.1 Run `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings` and `openspec validate --all --strict`. Verify: all pass.

## Results

| check | value |
|---|---|
| 1.1 smoke run on the author's Mac: time, agents in the file | 2026-11-03: a release build packed as the workflow packs it; `tokprof 0.1.0`, read 2 Claude Code and 2 Codex fixture files, wrote a share file with `claude` and `codex` for 2026-10-30 to 2026-10-31, in 0.8 s |
| 1.2 the comparison: two copies, a changed copy | two copies: `the 2 files hold the same numbers`; with `agents.claude.sessions` raised by 1 in a third copy: exit 1, `share-changed.json holds other numbers than …` and the diff line |

## Workflow follow-up

- The author pushes and starts the release workflow by hand on GitHub. The smoke runs and the comparison must pass.
- After the first tag, check one archive with `gh attestation verify`.
- Archive the change with `openspec archive add-release-builds -y`, then run `openspec validate --specs --strict`.
