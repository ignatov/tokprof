# Proposal

## Why

`release-builds` (change `add-release-builds`, not archived yet) ships tokprof for macOS and Linux only, and says that a release has no Windows binary. Claude Code and Codex run on Windows too, and the survey of `add-team-survey` asks other users' agents to download tokprof. On 2026-11-03, before the first release, the author asked for Windows builds. The author also asked that the clipboard and the browser work there before the release.

## What Changes

- **Two Windows archives per release:** `tokprof-x86_64-pc-windows-msvc.zip` and `tokprof-aarch64-pc-windows-msvc.zip`. Each holds `tokprof.exe`, `README.md` and `LICENSE` at its top level. They are built on Windows x86-64 and Windows ARM64 runners, so a release has six archives. `SHA256SUMS` and the attestations cover the zips.
- **Smoke runs on Windows:** both Windows binaries run over the fixture logs like the others, so five share files must hold the same numbers before a release is published.
- **Tests on Windows:** every push to `main` and every pull request runs the workspace tests on Windows as well. A test run goes on after a failing test binary, so one run shows every failure.
- **LF everywhere:** text files check out with LF line endings on Windows too, so the fixture logs stay what the agents write.
- **Two tests** stop expecting `/` in a printed path, and the share test sets `USERPROFILE`, where Windows takes the home directory from.
- **The clipboard and the browser on Windows:**
  - `tokprof handoff` and `c` in `tokprof ui` try `clip.exe` after `pbcopy`, `wl-copy` and `xclip`. It gets the text as UTF-16LE after a byte order mark, so Cyrillic session titles survive. The messages for a machine without a tool name `clip.exe` too.
  - `o` in `tokprof ui` opens the HTML with `explorer`.
- **README:** Install names both Windows targets and says to put `tokprof.exe` on the `PATH`. **AGENTS.md:** CI tests on Windows, and a test must not expect `/` in a printed path.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `release-builds`: Release archives, Platforms of a release, Checks of every push, Smoke run of the archives and Dry run of a release now include Windows. The capability comes from `add-release-builds`, so this change is archived after it.
- `handoff`: Clipboard copy in a terminal tries `clip.exe` last, and its message names it.
- `session-browser`: Profiles from the browser opens the page with `explorer` on Windows, and Handoff key tries `clip.exe` like handoff.

## How it is checked

- **Already on GitHub**, on the branch `windows-build` (12ae95a, 219b4c5), before the rebase onto `add-release-builds`:
  - CI run 41668226258: 283 of 283 tests pass on `windows-latest`, as on Linux and macOS.
  - Release run by hand 41667570234: both zips built, and `tokprof.exe --version` printed `tokprof 0.1.0` on both runners. The x86-64 zip has 2,078,141 bytes (exe 5.4 MB), the ARM64 zip 1,920,195 bytes (exe 4.5 MB).
- **After the rebase:**
  - Locally: `ci/smoke.sh` unpacks a zip packed as the workflow packs it and writes a share file from the fixtures. The comparison accepts the file next to the one from the `.tar.gz` of the same binary.
  - On GitHub: a run by hand ends with six archives, five smoke runs and a passing comparison. The first tag's release has six archives, six lines in `SHA256SUMS`, and an attestation that `gh attestation verify` accepts for a zip.
- **The clipboard and the browser:**
  - A unit test checks the bytes `clip.exe` gets for a line with Cyrillic: `FF FE`, then UTF-16LE.
  - On `windows-latest`, a test copies `tokprof · Привет` through the clipboard and reads it back with PowerShell's `Get-Clipboard`.
  - A unit test checks which command opens a page on each platform.
  - On the author's Mac, `tokprof handoff` in a terminal still copies with `pbcopy`, and `pbpaste` gives the printed line back.
- **The real logs** are on the author's Mac, so the Windows binaries never read them. The comparison with the Mac and Linux share files over the same fixtures stands in for that.

## Out of scope

- Other clipboard tools on Windows, such as PowerShell's `Set-Clipboard`.
- Authenticode signing, winget or Scoop packages, an installer and a PowerShell install script.
- 32-bit Windows and the `windows-gnu` targets.
- Running the test suite on Windows ARM64. That binary gets the smoke run.

## Impact

- `.github/workflows/ci.yml`: a Windows test job, `--no-fail-fast`, bash as the shell on every runner.
- `.github/workflows/release.yml`: the two Windows targets, zip packing, the smoke runs of both, `SHA256SUMS` and attestations over the zips.
- `ci/smoke.sh`: zip archives, `tokprof.exe`, `USERPROFILE`, and Windows paths for the binary's environment.
- New `.gitattributes`. Two tests: `crates/tokprof/src/watch.rs` and `crates/tokprof/tests/share.rs`.
- `crates/tokprof/src/clipboard.rs`, and the clipboard messages and the page opener in `crates/tokprof/src/main.rs` and `crates/tokprof/src/ui.rs`.
- README.md and AGENTS.md.
