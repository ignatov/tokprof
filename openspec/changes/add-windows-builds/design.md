# Design

## Context

- `add-release-builds` (aa59541) is on `main` and not archived. Its release workflow:
  - builds four targets;
  - packs each binary as `tokprof-<target>.tar.gz`;
  - smoke-runs every binary but the Intel Mac one with `ci/smoke.sh`, which expects a `.tar.gz` with `tokprof` in it;
  - compares the share files in a `compare` job;
  - attests `dist/*.tar.gz` in the publishing job.
- The Windows work was written before that change, on top of 58c73e0, and pushed to the branch `windows-build`:
  - 12ae95a: the workflows and `.gitattributes`;
  - 219b4c5: the two tests;
  - 9b36bde: README and AGENTS.md.

  The branch passed the checks there (see proposal.md, How it is checked). Rebased onto aa59541, 12ae95a conflicts in `release.yml`.
- GitHub's runner images, as of 2026-11-03:
  - `windows-latest` is Windows Server 2025 and `windows-11-arm` is Windows 11 on ARM64;
  - both have Rust 1.99.1 with rustup, Git Bash, `jq` 1.8.2 and 7-Zip 26.05, and no `unzip`;
  - both set the system time zone to UTC. On 2026-11-03 (the v0.1.1 run) `windows-11-arm` ran in a US zone instead.
- tokprof's code takes the home directory from `USERPROFILE` on Windows, not from `HOME`. The paths it prints use `\`.
- tokprof copies with the first of `pbcopy`, `wl-copy` and `xclip` that works, writing UTF-8 to its stdin. It opens a page with `open` on macOS and `xdg-open` elsewhere, and ignores a failure. On Windows none of these exist.

## Goals / Non-Goals

**Goals:**
- A Windows binary is held to the same checks as the others: the tests on every push, and the smoke run and comparison before a release.
- One `ci/smoke.sh` still serves every runner and the author's Mac.
- The clipboard and the browser work on Windows with what every Windows has, without a new dependency.

**Non-Goals:**
- A choice of clipboard tools on Windows. `clip.exe` is the one tool; see proposal.md, Out of scope.

## Decisions

1. **The MSVC targets, each built on its own architecture.**
   - `x86_64-pc-windows-msvc` builds on `windows-latest`, and `aarch64-pc-windows-msvc` on `windows-11-arm`. Both are native builds, so both binaries can run their smoke runs.
   - MSVC is Rust's default target on Windows, and its binaries need no MinGW runtime.
   - The alternative was cross-building ARM64 on the x86-64 runner. It would save no time, and its binary could not run there.
2. **Zip, packed with 7-Zip.**
   - Explorer opens a zip without extra tools.
   - The archive holds `tokprof.exe`, `README.md` and `LICENSE` at its top level, like the `.tar.gz` archives.
   - 7-Zip is on both Windows images. Git Bash's `tar` cannot write zips.
3. **`ci/smoke.sh` takes both kinds of archive.**
   - It unpacks a `.zip` with `7z` when it is installed, and with `unzip` otherwise (the author's Mac). Then it runs whichever of `tokprof.exe` and `tokprof` the archive holds.
   - Next to `HOME`, it points `USERPROFILE` into the temporary folder, so the Windows run does not read the runner's home either.
   - Under Git Bash, it turns the folders it passes to the binary into Windows paths with `cygpath -m`. The native binary then gets `D:/a/...` instead of `/d/a/...`, without relying on Git Bash's guesses about which variables hold paths.
   - `TZ=UTC` stays. chrono reads `TZ` only on Unix, and the comparison caught a Windows runner that did not run in UTC (v0.1.1): the rhythm's `stood_still` and `worked_share_median` moved with the day boundary. The release workflow now sets the Windows system time zone to UTC with `Set-TimeZone -Id UTC` before the smoke run.
4. **Checksums and attestations take both patterns:**
   - `sha256sum -- *.tar.gz *.zip`;
   - `subject-path` lists `dist/*.tar.gz` and `dist/*.zip`. actions/attest accepts a list of paths.
5. **The tag check strips CR** from the version that `jq` prints, in case `jq.exe` ends its line with CR LF. Without that, the check would fail on Windows only.
6. **The tests on Windows:**
   - `windows-latest` joins the test matrix, and every job runs its steps in bash.
   - `cargo test --no-fail-fast` keeps going after a failing test binary. The first Windows run on the branch stopped at the first one, which hid what the later ones would say.
   - `.gitattributes` sets `* text=auto eol=lf`. All 281 text files in the index are LF already, so no file changes on macOS or Linux.
   - Both test fixes stay in the tests: the watch test turns `\` into `/` before it compares, and the share test sets `USERPROFILE` and expects the platform's separator.
7. **Resolving the rebase conflict.**
   - Both sides of `release.yml` stay: the Windows matrix entries and zip packing from 12ae95a, and the smoke step, `compare` job and attestation from aa59541, widened to zips.
   - The README's attestation line from aa59541 stays, next to the Windows rows.
8. **`clip.exe` for the clipboard, fed UTF-16LE without a byte order mark.**
   - `clip.exe` comes with Windows, so no install and no new crate is needed.
   - UTF-8 on its stdin does not survive outside ASCII, and the user's session titles are Cyrillic. UTF-16LE does.
   - A byte order mark in front stays in the clipboard as U+FEFF: the first Windows run read back `\u{feff}tokprof · Привет`. So the text goes without one, and the Windows test reads the clipboard back to check that.
   - It comes last in the list. A Mac or Linux tries it only after its own tools fail, and WSL without `wl-copy` or `xclip` copies to the Windows clipboard through it.
   - Alternatives:
     - PowerShell's `Set-Clipboard`: it starts in about half a second, and its stdin needs its encoding set first.
     - A clipboard crate: a new dependency for one call.
9. **`explorer` for the page.**
   - `explorer <file>` opens a file with the program registered for it, which for `.html` is the browser.
   - The alternative, `cmd /c start "" <file>`, would need cmd's own quoting for paths with `&` or `%`.
   - explorer's exit code means nothing, and the opener ignores it anyway.
   - The path goes with `\` only, since explorer misreads `/`.

No log field and no agent version is involved: this change reads no log differently. The parse time of the full corpus does not change: the code that changes runs only on a key press or after `tokprof handoff` prints. On the branch, the Windows test job took 130 s, against 23 s on Linux and 40 s on macOS. The jobs run in parallel, so a CI run ends about 1.5 minutes later. The Windows builds took 2 min 47 s (x86-64) and 2 min 14 s (ARM64).

## Risks / Trade-offs

- [GitHub renames `windows-11-arm` or drops 7-Zip from an image] → Each sits in one place in `release.yml`. A run by hand shows the break before a tag does.
- [`USERPROFILE` and `HOME` disagree in a test on Windows] → The share test and `ci/smoke.sh` set both to the same folder.
- [A temporary folder under an 8.3 name, such as `RUNNER~1`] → tokprof shortens a path by a plain string prefix. Both the home and the profile directory come from the same string in the test, so the prefix matches.
- [The Windows ARM64 binary never runs the test suite] → It is built from the same commit and passes the smoke run and the comparison.
- [The CI runner has no clipboard, as a service session may not] → The round-trip test then fails on `windows-latest`. In that case, run it only when `TOKPROF_CLIPBOARD_TEST` is set, keep the unit test of the bytes, and say in tasks.md that the round trip was not checked on CI.
- [A user's `TOKPROF_DIR` holds `/`] → The opener turns `/` into `\` on Windows before it calls explorer.
- [macOS jobs sometimes waited 5–15 minutes for a runner on 2026-11-03] → This is on GitHub's side and is not caused by this change. A run by hand shows the wait; the jobs that got a runner finished in 1–2 minutes.

## Migration Plan

1. In the lease that holds the branch, rebase 12ae95a..9b36bde onto `main` and resolve `release.yml` and README as in decision 7. Then land the commits and this change with `bd pick`.
2. The author pushes `main` and runs the release workflow by hand. The run must end with six archives, five smoke runs and a passing comparison. Then the author pushes the tag `v0.1.0`.
3. Delete the branch `windows-build` on GitHub once `main` has its commits.
4. Rolling back means reverting the commits. A release made with Windows archives keeps them until it is deleted.
