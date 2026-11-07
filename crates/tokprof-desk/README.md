# tokprof-desk

The session browser of tokprof in a native window laid out as a profiler, drawn with [Rune](https://github.com/anomalyco/rune), a Rust UI framework with its own GPU renderer. It shows what `tokprof ui` shows, as controls in the system font rather than terminal text.

- **The day's lanes** along the top: a `You` lane with your sittings, then a lane per session with its work, its waits for your answer, your messages, its questions and your stops, at the times of the log. After each session's last request a green tail fades out over the time its cache stays warm, and a red line marks now. `Fit` shows the hours of the day's actions, `24 h` the whole day; the line under the lanes names the time under the pointer and what the lane shows there, and a click on a lane selects its session.
- **The sessions** below them on the left, by day: the last activity, the agent's dot, the title, the cost so far, and the next request: while the cache is warm, the time it stays warm counting down in green; once it is cold, what one more request costs, in red. The cell turns red at the moment the cache goes cold.
- **The inspector** of the selected session on the right, under tabs. `Summary` has the session's requests on a time axis up to now, with the time the cache stayed warm and has been cold and the next request at now; the next request's cost; the ways to continue (resume, a new session from a handoff, `/compact`) with the cheapest marked; the spend; and the advice. `Requests`, `Spend`, `Work`, `Waits`, `Prompts`, `Tools`, `Context` and `Cache` hold the X-ray's blocks of those names, with the numbers `tokprof scan` prints.
- **`Flame`**, the last tab, draws the session's spend as an icicle graph, the stacks and dollars `tokprof flame --session <id>` writes: the session on top, then its chains, turns, content kinds, tools, targets and components, each a rounded bar as wide as its share, the largest first. Structure is grey, content green, and the components in the colors of `Spend` (uncached in magenta). Frames under 3 pt are left out, and the line under the graph says how many; it also names the frame under the pointer, or under the cursor, with its path under the shown root, dollars, and shares of the session and of its parent. A click on a frame zooms into it, a click on the shown root or on a crumb of the path above the graph zooms out.
- **The bar** at the top is the window's own: the `Day | Reports` switch, the day's name with the days before and after, Today, the day's summary, the zoom, the filter and the status, right of the window's buttons.
- **Reports** (the switch, or ⌘2; ⌘1 or esc go back to Day) puts the report of the last 7, 30 or 90 days in place of the lanes, the list and the inspector, within the filters the window was started with: the key numbers (spend, avoidable on cache misses, cold returns, sessions, requests), the spend by day stacked by cost component, the top 10 projects and origins, the models, the cold returns by pause and the sessions by largest context, with the numbers `tokprof summary --since <N>d` and `tokprof pauses --since <N>d` print. A click on a day opens it in Day; one on a project filters the list to it. The report is read in the background, about 25 s on 8.8 GB of logs, with the count of logs read on the screen; each period's is kept, and read again after the logs change at most every 4 times as long as it took, at least 20 s. Tab moves between the period and the cards, ← → pick the period or a day, ↑↓ move through a card's rows, and ⏎ opens the day or the project.
- **What to change**, in Reports under the key numbers, holds the rows `tokprof share` prints for the period: what could have been saved by cause, with dollars per active hour; the ways to come back after a cold break with the cheapest marked; and the habits that cost the most. Under each row is what tokprof's advice says about it. A click or ⏎ on a cause or a habit opens its top 5 sessions under it (the cache lifetimes and the tool hints are not split by session), and a click or ⏎ on a session shows it in Day.
- **Keys** are those of `tokprof ui` (see the key table in the [README](../../README.md#quick-start)), except that ← and → step the day; ⏎ shows the inspector full size, `d` the selected session's day, ⌘Q and ⌘W quit. A click selects a session, a double click shows its inspector full size, and the wheel scrolls.
- **Every list takes the keyboard.** Tab and Shift+Tab move it between the lanes, the list and the inspector, and esc gives it back to the list. The pane that has it rings its selection in the accent (the lane, the row, the inspector's cursor row, or the chosen tab where a tab has no rows) and draws its title in the accent; the others keep their selection without the ring. In the lanes, ↑↓ (`k`, `j`), Home and End select the session of another lane. In the inspector, ← → (`h`, `l`), Shift+← → and 1–9 switch tabs, 0 shows `Flame`, and ↑↓, PgUp, PgDn, Home and End move a cursor through the tab's rows; in `Summary` they scroll. In `Flame`, ← → (`h`, `l`) move a ringed cursor to the frame before or after, ↓ (`j`) to the first child and ↑ (`k`) to the parent, ⏎ zooms into it, esc and Backspace zoom out, and Shift+← → leave for another tab; esc gives the keyboard back only when the graph is not zoomed.
- It follows the agents' log directories as `tokprof ui` does: new sessions join the list and the lanes, sessions that grow are costed again, and the rows on the screen stay where they are.
- **Settings** (`Settings…` in the app menu, ⌘,) open a panel over the window with the choice Rex offers: `Appearance` (`As in system`, `Light` or `Dark`), a `Light theme` and a `Dark theme`, Merino Light and Merino Dark by default. The appearances are tiles with a small window in each, and the themes grids of cards, each a picture of the theme's background with a prompt and dots of its colors; a pick applies at once. Tab moves between the three, ← → pick the appearance, ← → ↑ ↓, Home and End move through a grid, and the panel's foot names these keys; esc, ⏎, ⌘, again, `Done` or a click outside close the panel. The choices are kept in `~/Library/Application Support/io.github.ignatov.tokprof/prefs.json` (`appearance`, `light_theme`, `dark_theme`). The window's close, minimize and zoom buttons keep the system's appearance.

- **The command palette** (`Command Palette…` in the app menu, ⌘K) opens a card near the top of the window with a query field. What is typed picks, ignoring case, the entries whose names hold its letters in order, the tightest first (at the start, at word starts, in a run), with the matched letters in the accent; a day only by runs of letters that each start a word, so `to` finds Today but not Thu 8 Oct: `Actions` (what the window does from a key, each with its key), `Tabs`, `Sessions` (title, project and cost; a pick selects the session and shows its Summary) and `Days` (`Today` and the days of the listed sessions, named as the list names them; a pick shows the day in the lanes). With nothing typed it shows the actions and tabs, the 20 most recent sessions, `Today` and the 14 newest days. ↑ ↓ (Ctrl+P, Ctrl+N) move, ⏎ or a click runs an entry, esc, ⌘K or a click outside close it, and no other key reaches the window behind it.
- **The key help** (`?`, where no filter is being typed) lists every key of the window as keycaps, grouped by where it works: `Anywhere`, `Lanes`, `Palette`, `List`, `Inspector` and `Settings`. esc or `?` close it. Only one of the settings, the palette and the key help is open at a time.

## Install

The window is a Mac app, `tokprof.app`: name `tokprof`, bundle identifier `io.github.ignatov.tokprof`, an icon of its own, signed ad hoc. Releases from v0.1.5 carry it for Apple silicon and Intel; the [README](../../README.md#the-mac-app) says how to install one and open it the first time. From a clone, on macOS only:

```sh
crates/tokprof-desk/bundle.sh             # builds, signs ad hoc, checks and zips the app for this Mac
ditto crates/tokprof-desk/target/aarch64-apple-darwin/bundle/tokprof.app ~/Applications/tokprof.app
ln -sf ~/Applications/tokprof.app/Contents/MacOS/tokprof-desk ~/.local/bin/tokprof-desk
tokprof-desk                               # opens the app, every session, and gives the terminal back
tokprof-desk --project tokprof --since 7d  # the filters of tokprof ui
tokprof-desk --wait                        # stays in the terminal until the window closes
```

Started from a terminal, `tokprof-desk` checks its flags and that there are session logs, so its errors show there, then opens its bundle through Launch Services, or, when it is not in one, starts itself apart, writing to `~/Library/Logs/tokprof-desk.log`. A start without a terminal (Finder, the Dock, Spotlight, `open -a tokprof`) runs the window right away.

`app/icon.svg` is the icon's source; `app/icon.sh` renders it to `app/AppIcon.icns`, which is committed. `app/Info.plist` is the bundle's template, with the package's version written in by `bundle.sh`. The release workflow runs `bundle.sh` on its macOS jobs.

## A frame without a window

Rune renders headless with the same renderer, so the window can be checked without a person:

```sh
tokprof-desk --snap out.png --tree out.json             # the latest day, the list and the newest session
tokprof-desk --snap day.png --day 2026-11-03            # the lanes of that date
tokprof-desk --snap tools.png --tab tools               # another tab of the inspector
tokprof-desk --snap keys.png --scene inspector-keyboard # the inspector with the keyboard (or lanes-keyboard)
tokprof-desk --snap flame.png --scene flame-zoomed      # Flame zoomed into the costliest chain and its costliest turn
tokprof-desk --snap out.png --size 960x640 --theme light
tokprof-desk --snap ink.png --colors ink-meridian       # another theme, whatever --theme says
tokprof-desk --snap settings.png --scene settings       # the settings panel open
tokprof-desk --snap palette.png --scene palette         # the command palette open, with `to` typed
tokprof-desk --snap keys.png --scene keys               # the key help open
tokprof-desk --snap reports.png --scene reports         # Reports on the last 30 days, waited for up to 120 s
tokprof-desk --snap advice.png --scene reports-advice  # the same, with the top sessions of the first cause open
```

A frame without a window neither reads nor writes the saved settings: `--theme` sets its appearance and `--colors` its theme, else Merino Light or Merino Dark. Without a window it follows no logs: it waits up to 20 s for the X-ray of the newest session and for the day, then 0.5 s more for costs, as `tokprof ui --dump` waits. `--tree` writes every element with its text and place. The size is in points, 1400x900 by default; the window is no smaller than 960x640.

## How it is built

The sessions, their costs, X-rays and days come from `crates/tokprof-browse`, which `tokprof ui` uses too: the keys mean the same in both, and the window puts the browser in its mode with the day always beside the list. This crate draws them:

| File | What it holds |
|---|---|
| `src/main.rs` | The command line, the state of the window, the poll that takes in what the background work sent, and the keys |
| `src/view.rs` | The bar, the layout and the list |
| `src/day.rs` | The day's lanes |
| `src/inspector.rs` | The inspector's tabs, from the X-ray's data |
| `src/flame.rs` | The `Flame` tab: the frame tree of the X-ray's stacks, its layout, zoom and keys |
| `src/pal.rs` | The window's colors from a theme's roles, and the type |
| `src/themes.rs` | The color themes, read from `themes/themes.json` |
| `src/settings.rs` | The appearance and the themes, where they are kept, and the settings panel |
| `src/palette.rs` | The command palette: its entries, the fuzzy matcher, its keys and what a pick runs |
| `src/keys.rs` | The key help: the window's keys by where they work, and its card |
| `src/overlay.rs` | What the cards over the window share: the scrim, the card, keycaps |
| `src/changes.rs` | What to change in Reports: the savings, the ways back and the habits, their advice and the sessions behind them |
| `src/reports.rs` | Reports: the switch, the period, the cards and their keys, from the overview `tokprof-browse` reads on its reports worker |
| `src/ansi.rs` | The day's text without its color codes |
| `src/tests.rs` | The window driven headless through Rune's `Runtime` |

The browser's workers wake the window when they send. At rest it draws again only when a cache goes cold, every second while a warm row counts down on the screen, and every minute for the now line; the lanes and the inspector are built again only when what they show changed. In Reports it draws again only when the reports worker sends, at most every 250 ms while it reads, and reads nothing for reports while Day is shown.

## Themes

`themes/themes.json` holds the 41 color themes of Rex Beta 0.1.0 (build 1829), 14 light and 27 dark, with the ids and names Rex gives them; the themes are Rex's work. For each theme it keeps the roles the window draws with (canvas, surfaces, strokes, content, accent, danger, success, warning, info, terminal colors) as Rex draws them: Rex keeps them in Display P3, and the script converts them to sRGB, since the window draws in sRGB. To take the themes of another Rex build:

```sh
python3 crates/tokprof-desk/themes/import.py \
  "/Applications/Rex Beta.app/Contents/Resources/SuperUI_SuperDesignTokens.bundle/Contents/Resources/themes.json" \
  "Rex Beta 0.1.0 (1829)"
```

It stops when a theme lacks a role, so a changed format never writes a partial file. A saved theme id the new file no longer has gives the default theme.

## Rune

The window builds in a workspace of its own, with its own `Cargo.lock`, which the root workspace excludes: the root builds, CI and the release builds never fetch Rune. `Cargo.toml` pins Rune to a full rev of its `dev` branch, which is never rewritten.

To move the rev:
1. Render the frames above with the build you have.
2. Change the rev in `Cargo.toml`, build, and run the checks below.
3. Render the frames again and compare them.
4. Commit the move on its own.

## Checks

In this folder:

```sh
cargo fmt
cargo test
cargo clippy --all-targets -- -D warnings
```

The architecture test of the root workspace (`cargo test -p tokprof --test architecture`) reads this crate's manifest too: it may depend on `tokprof-core`, `tokprof-sources` and `tokprof-browse` only.
