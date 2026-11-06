# Design

## Context

`crates/tokprof-desk` builds one binary, `tokprof-desk`, in a workspace of its own so that the root builds and CI never fetch Rune (see add-desk-window). It is installed with `cargo install` and started from a terminal, which it then holds: Rune runs its event loop in the process the shell started. macOS gives such a process a Dock tile named after the binary, with a generic icon, because no bundle around it names the app.

The release workflow builds the CLI for six targets; its two macOS jobs run on Apple silicon runners (the Intel one cross-builds). The author's Mac has no code signing certificate.

## Goals / Non-Goals

**Goals:**
- A bundle that opens like any Mac app, with its own name and icon.
- `tokprof-desk` in a terminal starts the app and gives the terminal back.
- The bundle in the releases, built by the workflow, with checksums and attestations.

**Non-Goals:**
- Developer ID signing and notarization, a DMG, a Homebrew cask, updates.
- Fetching Rune in the checks of every push.

## Decisions

### The bundle is assembled by a script from a template

`crates/tokprof-desk/bundle.sh <target>` builds the release binary for the target in the window's workspace, makes `tokprof.app/Contents/{MacOS/tokprof-desk, Info.plist, Resources/AppIcon.icns}`, writes the version of the `tokprof-desk` package into the `Info.plist` template, signs the bundle ad hoc (`codesign --force --sign -`), checks it (`plutil -lint`, `codesign --verify --strict`) and zips it with `ditto -c -k --keepParent`, which keeps the bundle's extended attributes as Finder's own archives do. The same script runs on the author's Mac and in the release workflow, so a local bundle and a released one are made the same way.

*Alternatives.* `cargo-bundle` adds a tool to install in CI for a few lines of shell; an Xcode project would be a second build system.

### The icon is drawn once and committed

`app/icon.svg` is the source: a dark rounded tile with three lanes of green work and amber waits, a green tail and a red now line, the window's own marks. `app/icon.sh` renders it to the iconset sizes with a headless browser, `sips` and `iconutil`, and the resulting `app/AppIcon.icns` is committed, so neither the release workflow nor a build needs a browser.

### Leaving the terminal

`main` parses its flags first, as before. Then, unless it renders a frame (`--snap`, `--tree`), it was asked to `--wait`, or it already is the detached process, it lists the sessions once to fail with `no session logs found` in the terminal, and hands the window over:
- When its executable (symlinks resolved) is inside `<name>.app/Contents/MacOS/`, it runs `open -a <bundle> --args <its arguments>`, so Launch Services starts the app as Finder does: the Dock tile, the name in the menu bar, and one running copy, which comes to the front when it is opened again.
- Otherwise it starts its own executable again with its arguments and `TOKPROF_DESK_DETACHED=1`, in a session of its own (`setsid`), with its input from `/dev/null` and its output appended to `~/Library/Logs/tokprof-desk.log`, and exits. Closing the terminal then does not end the window.

The detached process does the work the window always did; the parent only decides and starts it. A user who wants the old behavior, for a log in the terminal, passes `--wait`.

### The app in the release workflow

The two macOS jobs of the build matrix gain three steps after the CLI's: build the app with `bundle.sh` for the job's target, run its binary's `--version` on the Apple silicon job (the Intel one only goes through `file` and `codesign --verify`), and upload `tokprof-desk-<target>.zip` as an artifact named `tokprof-desk-<target>`. The publish job already downloads every `tokprof-*` artifact, writes `SHA256SUMS` over `*.zip`, and attests `dist/*.zip`, so the app archives get both without a change there; its notes gain a line on the app. The tag check also compares the tag with the version of `tokprof-desk`, read with `cargo metadata --no-deps`, which fetches nothing. Only these two jobs fetch Rune, and only when a release runs; their timeout grows from 30 to 45 minutes for its build.

### The version

`tokprof-desk` keeps a version of its own in its own workspace, set to the version of `tokprof` whenever a release bumps it. The release skill bumps both, and the workflow refuses a tag that differs from either.

## Risks / Trade-offs

- [An ad-hoc signed app downloaded from GitHub is refused by Gatekeeper the first time] → The README and the release notes say how to open it. Notarization needs a Developer ID the author does not have yet.
- [Rune's build on the release runners takes minutes and may break with a runner image] → It runs only for releases; a dry run by hand tries it before a tag.
- [A detached process hides its errors] → The parent checks the flags and the logs in the terminal first, and the detached one writes to `~/Library/Logs/tokprof-desk.log`.

## Migration Plan

1. Build and check the bundle on the author's Mac, and install it in `~/Applications`, with `~/.local/bin/tokprof-desk` linked to the bundle's binary.
2. Push `main`, run the release workflow by hand, and look at the app archives it keeps.
3. Bump the versions to 0.1.5 and tag. Rollback: delete the release and the tag; the bundle changes nothing else.
