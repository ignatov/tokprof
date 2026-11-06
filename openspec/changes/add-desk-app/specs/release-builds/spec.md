# Spec Delta

## ADDED Requirements

### Requirement: App archives
A pushed tag SHALL also put `tokprof-desk-aarch64-apple-darwin.zip` and `tokprof-desk-x86_64-apple-darwin.zip` in the release, each holding `tokprof.app` of desk-app at its top level, built from the tagged commit. Each has a line in `SHA256SUMS` and an attestation, as the archives have. The release stops when the tag differs from `v` and the version of `tokprof-desk` too. On the Apple silicon runner the app's binary runs `--version`; the Intel one is only inspected.

#### Scenario: A release with the app
- **WHEN** the tag `v0.1.5` is pushed
- **THEN** the release `v0.1.5` also has the two app archives, and `https://github.com/ignatov/tokprof/releases/download/v0.1.5/tokprof-desk-aarch64-apple-darwin.zip` downloads the Apple silicon one

#### Scenario: What an app archive holds
- **WHEN** an app archive is unzipped
- **THEN** it gives `tokprof.app`, with no folder around it

#### Scenario: Checking a downloaded app
- **WHEN** `gh attestation verify tokprof-desk-aarch64-apple-darwin.zip -R ignatov/tokprof` runs on an app archive of the release
- **THEN** it succeeds, and the attestation names the commit of the tag

## MODIFIED Requirements

### Requirement: Dry run of a release
The release workflow SHALL also run by hand, without a tag. It then runs the checks, builds the six archives, their smoke runs and the two app archives, keeps the archives as artifacts of the run, and publishes no release.

#### Scenario: Trying the workflow
- **WHEN** the author starts the release workflow by hand on `main`
- **THEN** the run ends with the six archives and the two app archives as artifacts, after the smoke runs and the comparison
- **AND** the repository has no new release or tag
