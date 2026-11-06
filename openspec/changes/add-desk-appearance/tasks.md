# Tasks

## 1. The themes

- [x] 1.1 Write `crates/tokprof-desk/themes/import.py`: read Rex's `themes.json`, convert the semantic roles the design lists from Display P3 to sRGB hex (with an alpha byte when not opaque), keep Rex's order, write `themes/themes.json` with its `source`, and fail on a missing role. Run it on Rex Beta 0.1.0 (1829) and commit the file.
- [x] 1.2 Load the file once in the window (`serde`, `serde_json`). Unit-test: 41 themes, 14 light and 27 dark, unique ids, every role present and parsed, and Merino Light's canvas `#f6f7f9` and accent `#0061f6`.
- [x] 1.3 Build `Pal` from a theme as the design's table says, replacing the built-in pair. Verify `cargo test` passes and render `--snap` frames in Merino Light and Merino Dark; look at them next to Rex in the same themes.

## 2. The settings

- [x] 2.1 Add `Appearance { System, Light, Dark }` with its stored names and `dark(system_dark)`, and the two theme ids with Merino Light and Merino Dark as defaults; unknown values give the defaults. Unit-test the names both ways, the three `dark` answers, and an unknown theme id.
- [x] 2.2 Keep a `Prefs` in `Desk`: `Prefs::open("io.github.ignatov.tokprof")` in a window, `Prefs::memory()` without one. Read the three settings at start; a pick writes only a changed value; a write error sets the status `can't save settings: <error>`. Unit-test with `Prefs::at` in a temporary folder: a pick writes `appearance` and the theme id, the same pick again leaves the file's modified time, and a new `Desk` over the file starts with them.
- [x] 2.3 In the view, take `dark` from the setting and the theme from `dark`; add the theme id to the memo versions of the lanes and the inspector. Verify in a `rune::Runtime` test that `Dark` with Ink Meridian under a light system draws Ink Meridian's canvas, and that `System` follows a system change.
- [x] 2.4 Add `--colors <theme id>` for frames without a window, failing with `--colors takes one of:` and the ids on an unknown one. Test both.

## 3. The panel

- [x] 3.1 Add `Settings…` with ⌘, to the app menu, and take ⌘, in `on_key` too; both open and close the panel.
- [x] 3.2 Draw the panel as the design says: the scrim, the card, `Appearance`, the two theme lists with names and swatches, `Done`; mark the current choices. Clicks pick.
- [x] 3.3 Send every key to the open panel: Tab between its parts, ← and → for the appearance, ↑ and ↓ through a list (scrolled into view), esc, ⌘, and ⏎ close, ⌘Q quits. Test in the runtime that `q` and `j` change nothing behind it, that → from `As in system` picks `Light`, that ↓ in `Dark theme` picks the next dark theme, and that esc closes it with the same session and tab shown.
- [x] 3.4 Add the scene `settings`. Test that a headless desk ignores a saved file and starts on the defaults.

## 4. Check and document

- [x] 4.1 Run `cargo fmt`, `cargo test` and `cargo clippy --all-targets -- -D warnings` in `crates/tokprof-desk`, and `cargo test --workspace` at the root.
- [x] 4.2 Render `--scene settings` in light and dark and look at them: the card in the middle, the choices marked, the lists scroll, the scrim dims the window.
- [ ] 4.3 On the real logs, with the installed app: pick `Dark` and `Ink Meridian` on a light system, quit, start again, and check it opens so and `prefs.json` holds the three choices; pick `As in system`, switch the system to dark and back, and check that the window moves between the two themes. Check that the inspector of the newest session shows the totals of `tokprof scan`. Record it here.
- [x] 4.4 Update `crates/tokprof-desk/README.md`: the settings, ⌘,, where they are kept, and the themes with their source and how to regenerate them.
- [x] 4.5 Reinstall the window the author runs (`cargo install --path . --root ~/.local` and the app bundle as add-desk-app's `bundle.sh` builds it).
  - 2026-11-06: `bundle.sh` built and signed the app, installed as `~/Applications/tokprof.app`, which `~/.local/bin/tokprof-desk` links to; `cargo install` would replace that link, so it was not run.
