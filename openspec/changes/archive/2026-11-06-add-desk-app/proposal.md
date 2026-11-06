# Proposal

## Why

The author wants the window as a real Mac app, published on GitHub. Today `tokprof-desk` is a bare binary: it holds the terminal it was started from until the window closes, shows in the Dock as `tokprof-desk` with no icon of its own, and no release carries it, so nobody else can get it without a Rust toolchain and Rune's sources.

## What Changes

- **`tokprof.app`**, a macOS app bundle: named `tokprof`, bundle identifier `io.github.ignatov.tokprof`, the version of its release, an icon of its own (the day's lanes on a dark tile), signed ad hoc. It opens from Finder, the Dock, Spotlight and `open -a tokprof`, with its name and icon in the Dock and as the name of its menu.
- **`tokprof-desk` started in a terminal returns at once**, and the window lives on without it: a binary inside an app bundle opens its bundle through Launch Services, and any other starts itself as a process of its own. It first checks what it can in the terminal (its flags, that there are session logs), so errors still show there. `--snap`, `--tree` and a new `--wait` stay in the terminal.
- **The release workflow builds the app** for Apple silicon and Intel Macs, as `tokprof-desk-<target>.zip` with `tokprof.app` inside, with checksums and attestations like the other archives. Release v0.1.5 publishes it.
- **The README** says how to install the app from a release and how to open an app signed ad hoc the first time.

## Capabilities

### New Capabilities
- `desk-app`: the window as a macOS app: the bundle, how it starts from a terminal, and its install.

### Modified Capabilities
- `release-builds`: a release also carries the two app archives, and the dry run builds them too.

## Impact

- New in `crates/tokprof-desk`: `app/Info.plist` (a template), `app/AppIcon.icns` with its source `app/icon.svg`, and `bundle.sh`, which builds the bundle and zips it here and in the release workflow.
- `crates/tokprof-desk/src/main.rs`: the start from a terminal.
- `.github/workflows/release.yml`: the macOS jobs also build the app. Only they fetch Rune, and only for releases; the checks of every push stay as they are.
- The version of `tokprof-desk` follows the version of `tokprof`, and the release checks both against the tag.

## How it is checked on the real logs

- A bundle built here: `plutil -lint` passes on its `Info.plist`, `codesign --verify --strict` passes, `open` starts it, and `lsappinfo` names it `tokprof` with the bundle identifier above.
- `tokprof-desk` in a terminal returns in under a second and the window opens; with `--wait` the terminal waits until the window closes; `tokprof-desk --snap out.png` writes the frame as before.
- After the release: both app archives download, `shasum -a 256 -c SHA256SUMS --ignore-missing` reports them `OK`, `gh attestation verify` succeeds on them, and the unzipped app opens on the author's Mac after its quarantine is removed, showing the author's sessions.

## Out of scope

- Signing with a Developer ID and notarization: the author's Mac has no signing certificate. A later change can add them with the release secrets.
- A disk image, a Homebrew cask, automatic updates, a menu-bar or tray mode.
- The window on Windows and Linux.
- Keeping the app running with its window closed: closing the window quits it, as now.
