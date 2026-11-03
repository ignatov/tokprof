# Tasks

The Windows commits 12ae95a, 219b4c5 and 9b36bde on the branch `windows-build` are the base, and `add-release-builds` (aa59541) is the target. Pushing to GitHub, a run by hand there and pushing a tag are the author's calls: ask before each. Every group ends with `cargo test --workspace` green, in a state that can be committed.

## 1. Rebase

- [x] 1.1 In the lease that holds the branch, rebase the three commits onto `main`. Resolve `release.yml` as design decision 7 says: the six targets, zip packing and the CR strip of the tag check, next to the smoke step, the `compare` job and the attestation. Verify: actionlint reports nothing, the matrix has six targets, and `publish` needs `checks`, `build` and `compare`.

## 2. Smoke run of a zip

- [x] 2.1 Teach `ci/smoke.sh` the zip:
  - it unpacks a `.zip` with `7z`, or with `unzip` where `7z` is missing;
  - it runs `tokprof.exe` or `tokprof`, whichever the archive holds;
  - it sets `USERPROFILE` next to `HOME`;
  - under Git Bash, it passes Windows paths made with `cygpath -m`.

  Verify on the author's Mac: pack the Apple silicon release binary into a zip as the workflow packs a Windows binary. The script prints the version and writes a share file, and `--compare` accepts that file next to the one from the `.tar.gz`. Record the time in the results table.
- [x] 2.2 Run the smoke step on the Windows targets with their `.zip`. Only `x86_64-apple-darwin` skips it. Verify: actionlint reports nothing, and the step's archive path ends in `.zip` for the Windows targets.

## 3. Checksums and attestations

- [x] 3.1 Write `SHA256SUMS` over `*.tar.gz` and `*.zip`, and attest both patterns. Verify: actionlint reports nothing. Run the checksum line in a folder with one `.tar.gz` and one `.zip`: it writes two lines, and `shasum -a 256 -c SHA256SUMS` reports both `OK`.

## 4. Tests on Windows and the docs

- [x] 4.1 Keep the Windows test job, bash as the shell, `--no-fail-fast`, `.gitattributes` and the two test fixes from 12ae95a and 219b4c5. Verify: `cargo test --workspace` and `cargo clippy --workspace --all-targets -- -D warnings` pass on the Mac, and `git ls-files --eol` shows no CRLF in the index.
- [x] 4.2 README Install has:
  - both Windows rows and the `PATH` sentence;
  - the attestation line of `add-release-builds`;
  - no longer "There is no Windows build yet", nor the branch's sentence that tokprof cannot use the clipboard or open the browser on Windows.

  Verify: the text matches the scenario "Windows" of `release-builds`, and `grep -n "cannot use the clipboard" README.md` finds nothing.
- [x] 4.3 AGENTS.md says that CI tests on Windows, and that a test must not expect `/` in a printed path. Verify by reading it next to `ci.yml`.
- [x] 4.4 Run `openspec validate --all --strict`. Then, in a copy of `openspec/`, archive `add-release-builds` and then this change, and run `openspec validate --specs --strict`. Verify: all pass, and no requirement text in `release-builds`, `handoff` or `session-browser` is over 500 characters.

## 5. The clipboard and the browser on Windows

- [x] 5.1 Make `clip.exe` the fourth clipboard tool, fed UTF-16LE after `FF FE`. Name it in the no-tool messages of `tokprof handoff` and of `c` in `tokprof ui`. Verify:
  - a unit test next to the clipboard code checks the bytes for `tokprof · Привет`: `FF FE`, then each UTF-16 unit little-endian;
  - the tests that check either message still pass;
  - on the author's Mac, `tokprof handoff` in a terminal still says `copied to the clipboard`, and `pbpaste` gives the printed line back.
- [ ] 5.2 Add a Windows-only test that copies `tokprof · Привет` through the clipboard code and reads it back with `powershell -NoProfile -Command Get-Clipboard`, its output set to UTF-8. Verify: it passes on `windows-latest` in CI. If the runner has no clipboard, follow design.md, Risks, and record that here.
- [ ] 5.3 Open the HTML with `explorer` on Windows, with the path's `/` turned into `\`. Verify: a unit test checks the opener of the platform it runs on, `open`, `explorer` or `xdg-open`, and the path it gets on Windows.

## 6. On GitHub

- [ ] 6.1 Land the commits and this change with `bd pick`. Once the author says so, push `main` and delete the branch `windows-build`. Verify: the CI run of that push passes on Linux, macOS and Windows.
- [ ] 6.2 Once the author says so, run the release workflow by hand on `main`. Verify: the run ends with six archives as artifacts, five smoke runs and a passing comparison. Record the archive sizes and the job times in the results table.

## Results

| check | value |
|---|---|
| branch `windows-build`, CI run 41668226258 (before the rebase) | 283 of 283 tests on `windows-latest`, `ubuntu-latest` and `macos-latest`; the Windows test job took 130 s |
| branch `windows-build`, release run 41667570234 (before the rebase) | `tokprof 0.1.0` from both `tokprof.exe`; x86-64 zip 2,078,141 bytes, built in 2 min 47 s; ARM64 zip 1,920,195 bytes, built in 2 min 14 s |
| 2.1 smoke run of a zip on the author's Mac: time, comparison with the `.tar.gz` | 2026-11-03: the Apple silicon release binary, packed as `.tar.gz` and as a zip; `tokprof 0.1.0` from both, share files with `claude` and `codex` in 0.76 s and 0.36 s; `--compare`: `the 2 files hold the same numbers` |
| 5.1 `tokprof handoff` in a terminal on the author's Mac: message, `pbpaste` | 2026-11-03, on a pty over the real logs: stderr `copied to the clipboard: paste it into a new session`; `pbpaste` equals the printed line (176 characters) |
| 5.2 the clipboard round trip on `windows-latest` | |
| 6.2 run by hand on `main`: archives, smoke runs, comparison | |

There is no run of the Windows binaries on the real logs: the author's logs are on a Mac. The comparison over the fixtures stands in for it. The code that changes only copies and opens files, and 5.1 checks it on the Mac.

## Workflow follow-up

- After the author pushes `v0.1.0`, check the release:
  - it has six archives and six lines in `SHA256SUMS`;
  - `gh attestation verify tokprof-x86_64-pc-windows-msvc.zip -R ignatov/tokprof` succeeds.
- Archive `add-release-builds` first, then this change with `openspec archive add-windows-builds -y`, and run `openspec validate --specs --strict`.
- Then edit the Purpose of `openspec/specs/release-builds/spec.md`, which still says macOS and Linux, to name Windows too. A delta cannot change a Purpose.
