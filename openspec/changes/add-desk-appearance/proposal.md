# Proposal

## Why

The window has one pair of colors, and the author finds its light one too bright. The author uses Rex, a terminal whose 41 color themes (14 light, 27 dark) they like, Merino Light and Merino Dark among them, and wants the same choice in the window: the appearance, and a light and a dark theme, kept between launches.

## What Changes

- **The themes as a resource file of tokprof**: the 41 themes of Rex Beta 0.1.0 (build 1829), each with its name, its appearance and the colors the window uses, in a JSON file in the window's crate, built into the binary. A script regenerates the file from Rex's `themes.json`. The README says where the themes come from.
- **The window's colors come from the themes**: its background, panels, lines, text, accent and marks are the theme's roles (canvas, surface, stroke, content, accent, success, danger, warning, info, the terminal colors), as Rex shows them on the screen.
- **Settings in the window**: `Settings…` in the app menu and ⌘, open a settings panel over the window. ⌘, again, esc, `Done` or a click outside close it.
- **The choice, as in Rex**: `Appearance` with `As in system` (the default), `Light` and `Dark`; `Light theme` from the 14 light themes and `Dark theme` from the 27 dark ones, Merino Light and Merino Dark by default. Each theme shows its name and a strip of its colors. A pick applies at once, while the panel is open.
- **Kept between launches**: the three choices are saved in `~/Library/Application Support/io.github.ignatov.tokprof/prefs.json` and read at the next start.
- **Frames without a window** (`--snap`, `--tree`) do not read the saved choices: `--theme` sets the appearance as now, the default themes give the colors, and a new `--colors <theme>` picks another. A new scene, `--scene settings`, draws the panel open.

## Capabilities

### New Capabilities
None.

### Modified Capabilities
- `desk-window`: the window takes its colors from the chosen theme and appearance instead of one built-in pair that follows the system, it gains a settings panel, and its frames gain `--colors`. The `desk-window` capability comes from `add-desk-window`, so this change is archived after it.

## Impact

- `crates/tokprof-desk`: the themes file and its script, the mapping of a theme's roles onto the window's colors (which replaces the built-in pair in `pal.rs`), the settings panel, the menu item and ⌘,, and the saved choices.
- The window's crate gains `serde` and `serde_json`, which Rune already builds; nothing changes for `tokprof` or the root workspace.
- The window's binary grows by the themes file, about 50 KB.
- `crates/tokprof-desk/README.md`: the settings, the themes and their source.
- No change to `tokprof`, `tokprof ui`, the browser package or any log source.

## How it is checked on the real logs

- The file holds 41 themes, 14 light and 27 dark, with the ids and names Rex lists, and Merino Light's accent and background are the colors Rex draws for them.
- On the author's logs, with the system in light: pick `Dark` and `Ink Meridian`, and the whole window turns to Ink Meridian at once; quit and start again, and it opens so. Pick `As in system`, switch the system to dark and back, and the window moves between the light and the dark theme. `prefs.json` holds the three choices after each step.
- `tokprof-desk --snap out.png --theme light` draws Merino Light while the saved choice is something else, and `--colors alpine-milk` draws Alpine Milk.
- The numbers of the window do not change: the inspector of the newest session shows the same totals as `tokprof scan` before and after.

## Out of scope

- Editing themes or adding one's own, as Rex's theme editor does. The file is built so that a user file can be read later.
- Display P3: the window draws in sRGB, so the few theme colors outside sRGB show at the edge of sRGB.
- The parts the system draws: the close, minimize and zoom buttons keep the system's appearance, because Rune has no way to set a window's appearance.
- Themes for `tokprof ui` in the terminal, and the charts tokprof writes as files.
