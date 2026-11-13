# tokprof-desk

The session browser of tokprof in a native window laid out as a profiler, drawn with [Rune](https://github.com/anomalyco/rune), a Rust UI framework with its own GPU renderer. It shows what `tokprof ui` shows, as controls in the system font rather than terminal text.

- **The bar** at the top is the window's own: the switch `Day | Reports | Calendar` (⌘1, ⌘2, ⌘3) and, apart from it, `Sessions` (⌘0). In Day it holds the day's name with the days before and after, Today, the filter and `Live`; in Sessions `All sessions` with the date of the first log and the count of listed sessions, Today and the filter; then the status, right of the window's buttons.
- **The day's lanes** along the top of Day: a `You` lane with your sittings, `Agents at work` (how many sessions' agents worked at each moment, with the peak, and under the pointer the sessions working then), then a lane per session labeled with its project's name and title, with its work, its waits for your answer, your messages, its questions and your stops, at the times of the log. After each session's last request a green tail fades out over the time its cache stays warm, and a red line marks now. A header over the lanes gives the day's caption, such as `Thursday: 09:24 to now, 7 sittings, 11 in the lanes, $146`, and the zoom: `Fit` shows the hours of the day's actions, `24 h` the whole day. The line under the lanes names the time under the pointer and what the lane shows there, and a click on a lane selects its session.
- **The details pane** under the lanes, across the whole width. With no session selected it shows the day: cards for spend (by hour, against the day before), sittings, agents, answers, queue and cache, with the words of `tokprof day`; then the top sessions of the day by that day's spend, the day's cold returns, and, on the current day, who waits for you now. The window starts that way. A selected session shows its inspector instead, under `‹ The day` (esc) and the tabs. The border between the lanes and the details can be dragged, between 25% and 75%; the window keeps its place, and a double click puts it back.
- **The inspector** of the selected session, under tabs. `Summary` has the session's requests on a time axis up to now, with the time the cache stayed warm and has been cold and the next request at now; the next request's cost; the ways to continue (resume, a new session from a handoff, `/compact`) with the cheapest marked; the spend; and the advice. `Requests` is the request timeline: a column per request stacked by cost component, the context against its window, lines at cache misses and compactions, marks where you typed, stopped the agent, three tool calls failed in a row or the agent committed; a click or ← → shows why a request cost what it did, and a drag the total of a range. `Spend`, `Work`, `Waits`, `Prompts`, `Tools`, `Context` and `Cache` hold the X-ray's blocks of those names, with the numbers `tokprof scan` prints.
- **`Flame`**, the last tab, draws the session's spend as an icicle graph, the stacks and dollars `tokprof flame --session <id>` writes: the session on top, then its chains, turns, content kinds, tools, targets and components, each a rounded bar as wide as its share, the largest first. Structure is grey, content green, and the components in the colors of `Spend` (uncached in magenta). Frames under 3 pt are left out, and the line under the graph says how many; it also names the frame under the pointer, or under the cursor, with its path under the shown root, dollars, and shares of the session and of its parent. A click on a frame zooms into it, a click on the shown root or on a crumb of the path above the graph zooms out.
- **Sessions** (⌘0, or the `Sessions` button) shows every listed session in one table: last activity, agent, project, title, started, duration, requests, cost, avoidable, cold returns, the next request and the model. The project is named as the lanes name it; the inspector keeps the path. In time order the rows sit under day headers with the count and that day's spend from the day store, such as `Yesterday` and `52 sessions · $818 spent`; a row goes under the day its last activity belongs to as Day's days run, so a session last active at 02:00 sits under the day before. A click on a day header shows that day in Day. A click on a column header sorts by it, a second click the other way; sorted by anything but time, newest first, the table is one list. The toolbar's quick filters combine: `All`, `Running now`, `Warm`, `This week`, a pill per agent and a project picker. The footer totals the whole filtered set. `Projects` folds the table into a row per project with its subtotals. The selected session's `Summary` shows in a narrow inspector on the right, and ⏎, a double click or `Open in Day` show it in Day on the day of its row.
- **Reports** (⌘2) shows the last 7, 30 or 90 days within the filters the window was started with: the key numbers (spend, avoidable on cache misses, cold returns, sessions, requests), each against the period of the same length before; the spend by day stacked by cost component, the top 10 projects and origins, the models, the cold returns by pause and the sessions by largest context, with the numbers `tokprof summary --since <N>d` and `tokprof pauses --since <N>d` print. A click on a day opens it in Day; one on a project opens Sessions with that project picked. The report is read in the background, with the count of logs read on the screen; each period's is kept, and read again after the logs change at most every 4 times as long as it took, at least 20 s. Tab moves between the period and the cards, ← → pick the period or a day, ↑↓ move through a card's rows, ⏎ opens the day or the project, and esc or ⌘1 go back to Day.
- **What to change**, in Reports under the key numbers, holds the rows `tokprof share` prints for the period: what could have been saved by cause, with dollars per active hour; the ways to come back after a cold break with the cheapest marked; and the habits that cost the most. Under each row is what tokprof's advice says about it. A click or ⏎ on a cause or a habit opens its top 5 sessions under it (the cache lifetimes and the tool hints are not split by session), and a click or ⏎ on a session shows it in Day.
- **Calendar** (⌘3) shows the days of `tokprof calendar` as a grid with the metric as pills, the numbers beside it and the punchcard below; `Month in review` (or `r`) opens the card of the cursor's month.
- **Keys** are those of `tokprof ui` (see the key table in the [README](../../README.md#quick-start)), except that ← and → step the day and `t` goes to the day of the latest action, as Today does; ⏎ shows the inspector full size. esc steps back and never quits: from the full-size inspector, then the selection, then the filter. `q` quits in Day and Sessions, ⌘Q and ⌘W everywhere. A click selects a session, a double click shows its inspector full size, and the wheel scrolls.
- **Every pane takes the keyboard.** Tab and Shift+Tab move it between the lanes and the details pane in Day, and between the table and its inspector in Sessions. The pane that has it rings its selection in the accent (the lane, the row, the inspector's cursor row, or the chosen tab where a tab has no rows) and draws its title in the accent; the others keep their selection without the ring. In the lanes, ↑↓ (`k`, `j`), Home and End select the session of another lane. In the day summary, ↑↓ move through its rows and ⏎ selects a row's session. In the inspector, ← → (`h`, `l`), Shift+← → and 1–9 switch tabs, 0 shows `Flame`, and ↑↓, PgUp, PgDn, Home and End move a cursor through the tab's rows; in `Summary` they scroll. In `Flame`, ← → (`h`, `l`) move a ringed cursor to the frame before or after, ↓ (`j`) to the first child and ↑ (`k`) to the parent, ⏎ zooms into it, esc and Backspace zoom out, and Shift+← → leave for another tab. In Sessions, ↑↓, PgUp, PgDn, Home and End move through the rows, ⏎ opens the session in Day or folds a project, → and ← unfold and fold it, and `t` goes back to time order from the top.
- It follows the agents' log directories as `tokprof ui` does: new sessions join the table and the lanes, sessions that grow are costed again, and the rows on the screen stay where they are.
- **Settings** (`Settings…` in the app menu, ⌘,) open a panel over the window with the choice Rex offers: `Appearance` (`As in system`, `Light` or `Dark`), a `Light theme` and a `Dark theme`, Merino Light and Merino Dark by default. The appearances are tiles with a small window in each, and the themes grids of cards, each a picture of the theme's background with a prompt and dots of its colors; a pick applies at once. Tab moves between the three, ← → pick the appearance, ← → ↑ ↓, Home and End move through a grid, and the panel's foot names these keys; esc, ⏎, ⌘, again, `Done` or a click outside close the panel. The choices are kept in `~/Library/Application Support/io.github.ignatov.tokprof/prefs.json` (`appearance`, `light_theme`, `dark_theme`, `menu_bar`). The window's close, minimize and zoom buttons keep the system's appearance.
- **The menu bar countdown** (`Menu bar countdown` in the settings, last, after both theme grids: `Off` by default, `On`) puts a status item in the macOS menu bar, seen over any app. It counts the listed sessions whose cache is warm and whose next request would cost $1.00 or more once it is cold. Its title is `❄` and the countdown of the one that goes cold first, `❄ 12:40`; in its last 5 minutes the cold price joins it, `❄ 4:12 · $7.19`; with none, `❄` alone. Its menu lists those sessions, newest first, at most 8, with the title, countdown and cold price; a pick brings the window forward in Day on the session's day, with the session selected. `Open tokprof` and `Settings…` follow. When the first to go cold reaches 5:00, the Dock icon bounces once (no notification). The title moves every second while it counts, also with the window hidden, and holds off App Nap only then; closing the window quits, and the item goes with it.

- **The command palette** (`Command Palette…` in the app menu, ⌘K) opens a card near the top of the window with a query field. What is typed picks, ignoring case, the entries whose names hold its letters in order, the tightest first (at the start, at word starts, in a run), with the matched letters in the accent; a day only by runs of letters that each start a word, so `to` finds Today but not Thu 8 Oct: `Actions` (what the window does from a key, each with its key), `Tabs`, `Sessions` (title, project and cost; a pick opens the session in Day, or selects it in Sessions while Sessions shows) and `Days` (`Today` and the days of the listed sessions, named and grouped as the Sessions table's day headers; a pick shows the day in the lanes). With nothing typed it shows the actions and tabs, the 20 most recent sessions, `Today` and the 14 newest days. ↑ ↓ (Ctrl+P, Ctrl+N) move, ⏎ or a click runs an entry, esc, ⌘K or a click outside close it, and no other key reaches the window behind it.
- **The key help** (`?`, where no filter is being typed) lists every key of the window as keycaps, grouped by where it works: `Anywhere`, `Lanes`, `The day`, `Palette`, `Settings`, `Sessions`, `Inspector`, `Flame`, `Requests`, `Reports` and `Calendar`. esc or `?` close it. Only one of the settings, the palette and the key help is open at a time.

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
tokprof-desk --snap out.png --tree out.json             # the latest day and its summary
tokprof-desk --snap s.png --scene sessions              # Sessions, the newest session selected, after every session is costed
tokprof-desk --snap p.png --scene sessions-projects     # Sessions in Projects, every project folded (sessions-cost: sorted by cost)
tokprof-desk --snap day.png --day 2026-11-03            # the lanes of that date
tokprof-desk --snap tools.png --tab tools               # the newest session on its day, on that tab of the inspector
tokprof-desk --snap keys.png --scene inspector-keyboard # the inspector with the keyboard (or lanes-keyboard)
tokprof-desk --snap flame.png --scene flame-zoomed      # Flame zoomed into the costliest chain and its costliest turn
tokprof-desk --snap out.png --size 960x640 --theme light
tokprof-desk --snap ink.png --colors ink-meridian       # another theme, whatever --theme says
tokprof-desk --snap settings.png --scene settings       # the settings panel open
tokprof-desk --snap palette.png --scene palette         # the command palette open, with `to` typed
tokprof-desk --snap keys.png --scene keys               # the key help open
tokprof-desk --snap reports.png --scene reports         # Reports on the last 30 days, waited for up to 120 s
tokprof-desk --snap advice.png --scene reports-advice  # the same, with the top sessions of the first cause open
tokprof-desk --list-scenes                              # every scene
```

A frame without a window neither reads nor writes the saved settings: `--theme` sets its appearance and `--colors` its theme, else Merino Light or Merino Dark. Without a window it follows no logs: it waits up to 20 s for the X-ray of the newest session and for the day, then 0.5 s more for costs, as `tokprof ui --dump` waits; the default frame also waits for the day store, and `--scene sessions` up to 120 s for every session's cost. `--tree` writes every element with its text and place. The size is in points, 1400x900 by default; the window is no smaller than 960x640.

## How it is built

The sessions, their costs, X-rays and days come from `crates/tokprof-browse`, which `tokprof ui` uses too: the keys mean the same in both, and the window puts the browser in its mode with a day always shown. This crate draws them:

| File | What it holds |
|---|---|
| `src/main.rs` | The command line, the state of the window, the poll that takes in what the background work sent, and the keys |
| `src/view.rs` | The bar, the layout, and what the screens share |
| `src/day.rs` | The day's lanes and their header |
| `src/details.rs` | The day summary of Day's details pane: the cards, the top sessions, the cold returns and who waits |
| `src/sessions.rs` | The Sessions screen: the table, its day headers, sorts, quick filters, footer, Projects and the narrow inspector |
| `src/inspector.rs` | The inspector's tabs, from the X-ray's data |
| `src/requests.rs` | The `Requests` tab: the request timeline, the card of a request or a range, and the table |
| `src/flame.rs` | The `Flame` tab: the frame tree of the X-ray's stacks, its layout, zoom and keys |
| `src/pal.rs` | The window's colors from a theme's roles, and the type |
| `src/themes.rs` | The color themes, read from `themes/themes.json` |
| `src/settings.rs` | The appearance and the themes, where they are kept, and the settings panel |
| `src/menubar.rs` | The menu bar countdown: which sessions count, its title, menu and nudge, and its AppKit status item |
| `src/palette.rs` | The command palette: its entries, the fuzzy matcher, its keys and what a pick runs |
| `src/keys.rs` | The key help: the window's keys by where they work, and its card |
| `src/overlay.rs` | What the cards over the window share: the scrim, the card, keycaps |
| `src/changes.rs` | What to change in Reports: the savings, the ways back and the habits, their advice and the sessions behind them |
| `src/reports.rs` | Reports: the switch, the period, the cards and their keys, from the overview `tokprof-browse` reads on its reports worker |
| `src/deltas.rs` | Reports against the period before, from the day store |
| `src/calendar.rs` | Calendar: the grid, the metric, the numbers beside it and the punchcard |
| `src/month.rs` | The month in review card, and its image |
| `src/ansi.rs` | The day's text without its color codes |
| `src/tests.rs` | The window driven headless through Rune's `Runtime`, with `details_tests.rs`, `sessions_tests.rs` and `calendar_tests.rs` |

The browser's workers wake the window when they send. At rest it draws again only when a cache goes cold, every second while a warm row counts down on the screen or the menu bar counts down, and every minute for the now line; the lanes and the inspector are built again only when what they show changed. In Reports it draws again only when the reports worker sends, at most every 250 ms while it reads, and reads nothing for reports while Day is shown. The day store is refreshed on a worker, at most every 20 s and 4 times as long as it took, while Day or Sessions needs it.

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
