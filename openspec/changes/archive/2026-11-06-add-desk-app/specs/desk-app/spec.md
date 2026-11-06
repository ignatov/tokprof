# Spec Delta

## Purpose

`tokprof.app` is the window of desk-window as a macOS app: a bundle with its own name and icon that opens like any other Mac app, a `tokprof-desk` command that starts it without holding the terminal, and an install from the releases.

## ADDED Requirements

### Requirement: The app bundle
`tokprof.app` SHALL be a macOS app bundle named `tokprof`, with the bundle identifier `io.github.ignatov.tokprof`, the version of its release, an icon of its own and `tokprof-desk` as its executable, signed ad hoc. It opens the window of desk-window from Finder, the Dock, Spotlight and `open -a tokprof`, and shows its name and icon in the Dock and as the name of its menu.

#### Scenario: Opening it from Finder
- **WHEN** the user opens `tokprof.app` in Finder
- **THEN** the window opens, and the Dock shows `tokprof` with the app's icon
- **AND** the menu bar's first menu is named `tokprof`

#### Scenario: Its version
- **WHEN** the `CFBundleShortVersionString` of the bundle of release `v0.1.5` is read
- **THEN** it is `0.1.5`, and `tokprof.app/Contents/MacOS/tokprof-desk --version` prints `tokprof-desk 0.1.5`

### Requirement: Started from a terminal
`tokprof-desk` started from a terminal SHALL check its flags and that there are session logs, then start the window without the terminal and return at once: a binary inside an app bundle opens its bundle through Launch Services with the same arguments, and any other starts itself as a process of its own. Its errors show in the terminal before it returns. `--snap`, `--tree` and `--wait` stay in the terminal until they end.

#### Scenario: From a terminal
- **WHEN** the user runs `tokprof-desk` in a terminal
- **THEN** the prompt comes back in under a second and the window opens
- **AND** closing the terminal leaves the window open

#### Scenario: Waiting
- **WHEN** the user runs `tokprof-desk --wait`
- **THEN** the terminal waits until the window closes

#### Scenario: An error
- **WHEN** the user runs `tokprof-desk` where there are no session logs
- **THEN** the terminal shows `no session logs found`, and no window opens

#### Scenario: A frame without a window
- **WHEN** the user runs `tokprof-desk --snap out.png`
- **THEN** it writes the frame and exits, as desk-window's One frame without a window says

### Requirement: Installing the app
The README SHALL say how to install the app from a release: unzip the archive of the Mac's processor and move `tokprof.app` to Applications. It also says how to open an app signed ad hoc the first time: Open Anyway in System Settings, Privacy & Security, or `xattr -dr com.apple.quarantine` on the app.

#### Scenario: The first launch
- **WHEN** someone downloads the app archive, unzips it and opens `tokprof.app`
- **THEN** macOS refuses it once, and the README's steps open it from then on
