---
name: release
description: Cut a tokprof release (version bump, tag, GitHub release workflow) or a pre-release, and recover a release run whose macOS job was not acquired by a runner.
---

# Release tokprof

A release is a tag. Set `version` in the root `Cargo.toml`, build once so that `Cargo.lock` follows, and commit both. Push main, then a tag of that version: `git tag v0.2.0 && git push origin v0.2.0`.

`.github/workflows/release.yml` checks the tag against the version, runs the CI checks, builds the macOS, Linux and Windows binaries and publishes them as a GitHub release with `SHA256SUMS`. The release notes list the commit subjects since the previous tag, without the `Archive <change>` commits.

A tag with a hyphen, such as `v0.2.0-rc.1`, makes a pre-release. A run by hand (Actions → Release → Run workflow) builds the archives and keeps them as artifacts, without a release.

GitHub's macOS runners are short at times: a job that no runner takes is cancelled after about 15 minutes with "The job was not acquired by Runner". Then run `gh run rerun <run id> --failed`; a tag's release is published once the jobs pass.
