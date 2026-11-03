# Design

## Context

- `.github/workflows/ci.yml` checks every push to `main` and every pull request: the tests on Linux and macOS, clippy with warnings as errors, and `openspec validate --all --strict`. The release workflow calls it too.
- `.github/workflows/release.yml` (dbd6f3d, 58c73e0) works like this:
  - It checks the tag against the version of the `tokprof` package, and makes a tag with a hyphen a pre-release.
  - It builds four targets: both Mac targets on the Apple silicon runner, Linux x86-64 on `ubuntu-22.04` and Linux ARM64 on `ubuntu-22.04-arm`. Debug info is stripped.
  - It packs `tokprof-<target>.tar.gz` and runs the packed binary's `--version`, except for the Intel Mac binary, which it only inspects with `file`.
  - The publishing job writes `SHA256SUMS` and notes from the commit subjects, and creates the release with `gh release create`.
- The fixture logs already lie as the agents' log directories: `sources/claude-code/tests/fixtures/projects/…` and `sources/codex/tests/fixtures/sessions/…`. With `CLAUDE_CONFIG_DIR` and `CODEX_HOME` set to those folders, any built binary reads them.

## Goals / Non-Goals

**Goals:**
- An archive's origin can be checked, not only its integrity.
- A release never ships a binary that reads the fixture logs differently from the others.

**Non-Goals:**
- Changing the targets, the archive names or the glibc floor. See proposal.md, Out of scope.

## Decisions

1. **`actions/attest@v4` in the publishing job.**
   - `attest-build-provenance` is now a wrapper of `actions/attest`, and its README sends new users there.
   - The step runs on `dist/*.tar.gz` after the archives are downloaded and before `gh release create`. So an attestation that fails leaves no release.
   - The job gets `id-token: write`, `attestations: write` and `artifact-metadata: write`, next to its `contents: write`. The other jobs keep `contents: read`.
   - GitHub's own actions are pinned by major version, as the workflow already does for `actions/checkout`.

2. **`ci/smoke.sh` runs the archive, not the build output.**
   - It unpacks the archive into a new temporary folder and runs `tokprof --version`. Then it runs `tokprof share -o <out>`, with `CLAUDE_CONFIG_DIR` and `CODEX_HOME` pointing at the fixture folders.
   - It also points `HOME`, `XDG_CONFIG_HOME` and `TOKPROF_DIR` into the temporary folder, so no price file or profile directory of the machine is read. `TZ=UTC` keeps the window's days the same on every runner.
   - It takes no `--since`, since the fixtures carry fixed dates.
   - A second mode, `ci/smoke.sh --compare <file>…`, drops the fields that name the platform with `jq` and compares every file with the first, printing the name of the first one that differs.
   - One bash script serves the workflow and the author's Mac alike.

3. **Where it runs.**
   - In the build job, right after packaging, for every target but `x86_64-apple-darwin`. Each run uploads its share file as the artifact `smoke-<target>`.
   - A `compare` job downloads them and runs the compare mode.
   - `publish` needs `checks`, `build` and `compare`. A run by hand runs `compare` too, so a dry run checks everything that a release does.

4. **The fields that name the platform** are `platform`, which `add-team-survey` adds to the share file. Today's format 1 has none, so `jq 'del(.platform)'` is a no-op until then.

## Parse time

tokprof's code does not change. The fixtures are a few hundred kilobytes, so a smoke run takes well under a second on each runner.

## Risks / Trade-offs

- [The runner labels or `actions/attest` change] → They sit in one place each in `release.yml`, and a run by hand shows a break before a tag does.
- [The Intel Mac binary is never run] → It comes from the same commit and toolchain as the Apple silicon one, which runs. Running it would need Rosetta on the runner, or an Intel runner.
- [The workflow cannot be run before a push] → Locally, the script runs against an archive packed the same way, and both YAML files are parsed. The first run by hand after the push checks the rest.

## Migration Plan

1. Land the script and the workflow change on `main` locally.
2. The author pushes and starts a run by hand, then pushes the first tag. Rolling back means deleting the release and its tag. The attestations of a deleted release stay in the attestation store and refer only to its archives.
