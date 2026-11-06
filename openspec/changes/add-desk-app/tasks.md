# Tasks

## 1. The bundle

- [x] 1.1 Draw the icon in `crates/tokprof-desk/app/icon.svg`, render it to `app/AppIcon.icns` with `app/icon.sh`, and look at it at 1024 and 32 points.
  - Looked at the 1024-point render: three session lanes and the `You` lane under a red now line, on a dark tile.
- [x] 1.2 Write the `Info.plist` template (name, identifier, executable, icon, version, minimum macOS, high resolution, category) and `bundle.sh <target>`, which builds, assembles, signs ad hoc, checks and zips. Verify on the author's Mac: `plutil -lint`, `codesign --verify --strict`, `open` starts it, and `lsappinfo` names it `tokprof` with its identifier.
  - 2026-11-06: `plutil -lint` OK, `codesign --verify --strict` valid on disk; opened from `~/Applications`, `lsappinfo` gave `LSDisplayName "tokprof"`, `io.github.ignatov.tokprof`, type Foreground, ARM64, and the app quit on an Apple event.

## 2. Leaving the terminal

- [x] 2.1 Add `--wait`, and the start without the terminal: the checks first, then `open -a` for a binary inside a bundle and a detached process for any other, with its output in `~/Library/Logs/tokprof-desk.log`. Unit-test which way each start takes, and that the errors come before it.
  - A start counts as from a terminal when its standard input is one: Launch Services and the detached process start it without, so they run the window.
- [x] 2.2 Check on the author's Mac: `tokprof-desk` from a terminal returns at once and the window opens; `--wait` waits; `--snap` writes its frame; a terminal closed after the start leaves the window open.
  - Under `script` (a pseudo-terminal): through the bundle, the prompt came back in 0.36 s and Launch Services ran `tokprof`; the bare binary came back in 0.31 s and its window ran on as a process of its own, in a session of its own, so a closing terminal does not reach it. `--wait` stayed until it was ended; `--snap` wrote its frame from the linked command.

## 3. The release

- [x] 3.1 Add the app to the macOS jobs of the release workflow, the version of `tokprof-desk` to its tag check, and a line on the app to the notes.
- [x] 3.2 Write the install of the app in README.md (the archives, Applications, the first launch) and in `crates/tokprof-desk/README.md`, and the bundle to AGENTS.md's window checks.
- [x] 3.3 Run the root and the window's checks and `openspec validate --all --strict`; install the bundle in `~/Applications` and link `~/.local/bin/tokprof-desk` to its binary; commit.
  - The window: 36 tests pass, clippy clean; the root's format and architecture checks pass; `openspec validate --all --strict` passes with add-desk-window, which this commit adds as well. The bundle is in `~/Applications`, and `~/.local/bin/tokprof-desk` links to its binary.
- [x] 3.4 Push `main`, run the release workflow by hand, and check the app archives it keeps: they unzip to `tokprof.app`, and the arm64 one opens here.
  - 2026-11-06: main pushed (7b18a81..39626ae); the run by hand, 42277126975, passed every job, both macOS builds with the app among them, and published nothing. The archives are checked on the release itself in 3.5.
- [x] 3.5 Bump `tokprof` and `tokprof-desk` to 0.1.5, tag `v0.1.5`, and check the release: the eight archives, `SHA256SUMS` and `gh attestation verify` on the app archives, and the downloaded app opens after its quarantine is removed. Record it here.
  - 2026-11-06: tag `v0.1.5` on 22fde88. The first attempt of run 42279338246 had every job pass but `Checks / Test on macos-latest`, which no runner took ("not acquired by Runner"); `gh run rerun --failed` passed it and published the release. It has the six archives, the two app archives and `SHA256SUMS`. Downloaded with `gh`: `shasum -c` OK for the app and the Apple silicon archive; `gh attestation verify` on the app archive names `release.yml@refs/tags/v0.1.5` and commit 22fde88; the app unzips to `tokprof.app` (`tokprof`, `io.github.ignatov.tokprof`, 0.1.5), `codesign --verify --strict` passes, and its binary prints `tokprof-desk 0.1.5` and renders the author's logs. `gh` sets no quarantine, so the browser's first-launch refusal was not seen here; the README and the notes say how to pass it.
