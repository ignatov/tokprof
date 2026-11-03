# Spec Delta

## Purpose

Builds tokprof for macOS and Linux on every release tag, so that anyone, or their coding agent, can download a binary, check where it came from and run it without a Rust toolchain.

## ADDED Requirements

### Requirement: Release archives
A pushed tag `v<version>` SHALL produce a GitHub release of that tag with four archives: `tokprof-aarch64-apple-darwin.tar.gz`, `tokprof-x86_64-apple-darwin.tar.gz`, `tokprof-x86_64-unknown-linux-gnu.tar.gz` and `tokprof-aarch64-unknown-linux-gnu.tar.gz`. Each holds `tokprof`, `README.md` and `LICENSE` at its top level, with the binary's debug info stripped. The archive names carry no version.

#### Scenario: A release
- **WHEN** the tag `v0.2.0` is pushed
- **THEN** the release `v0.2.0` has the four archives
- **AND** `https://github.com/ignatov/tokprof/releases/download/v0.2.0/tokprof-aarch64-apple-darwin.tar.gz` downloads the Apple silicon archive

#### Scenario: The newest release
- **WHEN** a link names `https://github.com/ignatov/tokprof/releases/latest/download/tokprof-x86_64-unknown-linux-gnu.tar.gz`
- **THEN** it downloads the Linux x86-64 archive of the newest release

#### Scenario: What an archive holds
- **WHEN** an archive is unpacked
- **THEN** it gives `tokprof`, `README.md` and `LICENSE`, and no folder around them

### Requirement: Platforms of a release
The macOS binaries SHALL run on Apple silicon and on Intel Macs. The Linux binaries SHALL run on x86-64 and on ARM64 with glibc 2.35 or later, such as Ubuntu 22.04 and Debian 12. A release has no Windows binary.

#### Scenario: An older Linux
- **WHEN** the glibc versions that a Linux binary's dynamic symbols need are listed
- **THEN** none is above 2.35

#### Scenario: Windows
- **WHEN** someone looks for a Windows build
- **THEN** the release has none, and the Install section of the README says so

### Requirement: Checksums and provenance
Each release SHALL carry `SHA256SUMS`, with one `<sha256>  <archive>` line per archive in the form `sha256sum` writes. Each archive SHALL also have a build provenance attestation that names this repository, the release workflow and the tagged commit.

#### Scenario: Checking a downloaded archive
- **WHEN** a folder holds `SHA256SUMS` and one downloaded archive, and `shasum -a 256 -c SHA256SUMS --ignore-missing` runs there
- **THEN** it reports that archive as `OK`

#### Scenario: Checking where an archive was built
- **WHEN** `gh attestation verify tokprof-aarch64-apple-darwin.tar.gz -R ignatov/tokprof` runs on an archive of the release
- **THEN** it succeeds, and the attestation names the commit of the tag

#### Scenario: A changed archive
- **WHEN** an archive differs by even one byte from the one the release workflow built
- **THEN** its checksum does not match its line in `SHA256SUMS`, and `gh attestation verify` fails

### Requirement: Release notes
The notes of a release SHALL list the subjects of the commits since the previous tag, oldest first, without merge commits and without the `Archive <change>` commits of OpenSpec. A last line points to the archives, their checksums and the Install section.

#### Scenario: A release after another
- **WHEN** `v0.3.0` follows `v0.2.0`, and the commits between them are `Pool the share files` and `Archive add-team-survey`
- **THEN** the notes list `- Pool the share files`, and not the archive commit

### Requirement: Version of a release
The release workflow SHALL stop before it builds when the tag differs from `v` followed by the version of the `tokprof` package, and then publish nothing. A tag with a hyphen, such as `v0.2.0-rc.1`, makes a pre-release. The binaries of a release print `tokprof <version>` for `tokprof --version`.

#### Scenario: A tag that does not match
- **WHEN** the tag `v0.3.0` is pushed while the `tokprof` package has the version `0.2.0`
- **THEN** the workflow fails, and no release `v0.3.0` is made

#### Scenario: A pre-release
- **WHEN** the tag `v0.2.0-rc.1` is pushed and matches the package version
- **THEN** the release is marked as a pre-release

#### Scenario: The version a binary prints
- **WHEN** `tokprof --version` runs from an archive of the release `v0.2.0`
- **THEN** it prints `tokprof 0.2.0`

### Requirement: Checks of every push
Every push to `main` and every pull request SHALL run the workspace tests on Linux and macOS, clippy over the workspace with warnings as errors, and `openspec validate --all --strict`. A release runs the same checks first, and publishes nothing when one fails.

#### Scenario: A failing check on a tag
- **WHEN** a clippy warning is on the tagged commit
- **THEN** the release workflow fails, and no release is published

### Requirement: Smoke run of the archives
Before a release is published, each binary that can run on its build runner SHALL be unpacked from its archive and run `tokprof --version`, then `tokprof share` over the log sources' fixture logs. The Intel Mac binary, built on an Apple silicon runner, is only inspected. The share files of the binaries that ran SHALL be equal, apart from the fields that name the platform, or nothing is published.

#### Scenario: The same numbers
- **WHEN** the Apple silicon, Linux x86-64 and Linux ARM64 binaries write their share files over the fixtures
- **THEN** the three files are equal apart from the platform fields, and the release goes on

#### Scenario: A difference
- **WHEN** one of the files holds another number than the others
- **THEN** the comparison fails and names the file that differs
- **AND** no release is published

### Requirement: Dry run of a release
The release workflow SHALL also run by hand, without a tag. It then runs the checks, builds the four archives and their smoke runs, keeps the archives as artifacts of the run, and publishes no release.

#### Scenario: Trying the workflow
- **WHEN** the author starts the release workflow by hand on `main`
- **THEN** the run ends with the four archives as artifacts, after the smoke runs and the comparison
- **AND** the repository has no new release or tag
