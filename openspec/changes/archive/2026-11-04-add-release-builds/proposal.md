# Proposal

## Why

The next step after `tokprof share` is a survey. Other users paste a prompt into their coding agent, and the agent downloads tokprof and writes the share file (see `add-team-survey`). For that, the agent needs a binary it can download in one step and check before it runs it. It also needs to know that this binary writes the same share file as the author's build.

GitHub Actions already build and publish the binaries (commits dbd6f3d and 58c73e0 of 2026-11-03). A pushed tag `vX.Y.Z` runs the CI checks and builds four archives: `tokprof-<target>.tar.gz` for Apple silicon and Intel Macs and for x86-64 and ARM64 Linux. It then publishes them with `SHA256SUMS`. Two things are still missing:
- **No check of where an archive was built.** A checksum next to the archive shows only that the download is whole.
- **The binaries are never run on a log.** Only `tokprof --version` runs, so a release could ship a binary that reads the logs differently on Linux.

These workflows also have no spec yet.

## What Changes

- **The `release-builds` capability** records what the release workflow does today:
  - the four archives and what each holds;
  - the platforms: macOS on both kinds of Mac, and Linux with glibc 2.35 or later;
  - `SHA256SUMS`, and the release notes from the commit subjects since the previous tag;
  - the tag that must match the version in `Cargo.toml`, and pre-releases for tags with a hyphen;
  - the run by hand that keeps the archives as artifacts and publishes nothing;
  - the CI checks of every push and pull request.
- **Provenance attestations.** Each archive of a release gets a build provenance attestation. Anyone can then check that this repository's workflow built it from the tagged commit: `gh attestation verify tokprof-aarch64-apple-darwin.tar.gz -R ignatov/tokprof`.
- **A smoke run over the fixture logs.** Every binary that can run on its build runner unpacks from its archive and runs `tokprof share` over the log sources' fixture logs. That is three of the four: the Intel Mac binary cannot run on the Apple silicon runner. The three files must hold the same numbers, or the release is not published. A run by hand does the same.
- **README.** The Install section adds the attestation check next to `SHA256SUMS`.

## Capabilities

### New Capabilities

- `release-builds`: the archives, platforms, checksums, attestations, notes and version of a tokprof release, and the checks before a release is published.

### Modified Capabilities

None.

## How it is checked

- **Locally.**
  - `ci/smoke.sh` runs on the author's Mac against an archive packed as the workflow packs it, and writes a share file from the fixtures.
  - The comparison accepts two copies of that file and refuses them once one number in one copy is changed.
  - Both workflow files parse as YAML.
- **On GitHub, once the author pushes.**
  - A run by hand builds the four archives and passes the three smoke runs and the comparison.
  - The first tag's release carries an attestation per archive, which `gh attestation verify` accepts.

## Out of scope

- Windows, one universal binary for both kinds of Mac, and Linux binaries for glibc older than 2.35. They can come later as their own change.
- Code signing, notarization, package managers, an install script and automatic updates.
- Pushing and tagging, which the author does.

## Impact

- `.github/workflows/release.yml`: attestations in the publishing job, the smoke run in the build job, and a comparison job.
- New `ci/smoke.sh`.
- README: one line in Install.
- tokprof's own code does not change.
