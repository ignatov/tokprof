# Design

## Context

`tokprof ui` lives in the CLI crate: `crates/tokprof/src/ui.rs` (1,237 lines) holds both the browser's state and its ratatui drawing, `watch.rs` follows the log directories, `day.rs` builds a day, and `render_xray.rs` and `render_rhythm.rs` turn an X-ray and a day into ANSI text. The state is mostly frontend-neutral already. Only three things in it are ratatui's: the X-ray and day kept as `ratatui::Text` (converted from ANSI with `ansi-to-tui`), the selection and scroll kept in `ListState`, and the keys taken as crossterm `KeyEvent`s.

The architecture test (`crates/tokprof/tests/architecture.rs`) allows only the packages it knows, and only `tokprof-core` and `tokprof-sources` under `tokprof`.

Rune at rev `781e2b6d` builds and renders headless on the author's Mac: `cargo build --release -p hello` took 1 m 30 s from clean (203 crates), and `hello --snap` wrote the expected PNG. Rune gives every app `--snap`, `--tree`, `--size`, `--theme` and `--scale`, and passes other flags to the app through `rune::extra_args()`.

## Goals / Non-Goals

**Goals:**
- One browser model, two frontends. A fix to sorting, rows in place or live updates lands in both.
- The terminal browser behaves exactly as before; its `--dump` frame is the check.
- The window checks itself headless, the way `--dump` does for the terminal.

**Non-Goals:**
- No new analysis in `tokprof-core`, except what the window needs to draw what the text already shows as data: the strip's lanes, the main conversation's requests, and the prices of the ways to continue that the advice prints (see below).
- No Rune in `tokprof` or in CI.

## Decisions

### A library crate for the browser model: `tokprof-browse`

It takes from `crates/tokprof`: the state of `ui.rs` without drawing (entries, filter, sort, rows in place, the background workers for glances, X-rays and days, exports and handoff), `watch.rs`, `day.rs`, `clipboard.rs`, `paths.rs`, `render_sessions.rs`, and the X-ray and rhythm renderers with what they call: the tool rows and the failures line of `render_report.rs` (`render_tools.rs`) and the lines on the logs read of `main.rs` (`reading.rs`). It depends on `tokprof-core` only; the frontends hand it the `Catalog` and `Pricing` they built from `tokprof-sources`, as `main.rs` does now.

Its interface (`Browser<T>`):
- keys: `on_key(Key, Viewport)` takes a frontend-neutral key (a character, the arrows, PgUp, PgDn, Home, End, ⏎, esc, Backspace, and Ctrl+C as `Interrupt`) with the page of the list and the lines of the right pane, and does what session-browser says that key does. The keys' meaning lives in one place, so both frontends behave the same;
- `tick()` takes what the workers sent and asks for what is due (costs, the selected X-ray, the shown day); `settle()` waits as `ui --dump` does; `next_cold(now)` says when the next listed cache goes cold, so a frontend that sleeps until something happens knows when to redraw;
- read-only access to the rows' cells, the list title, the X-ray and day titles, what each pane shows (a text, a dim note while it is computed, or a note that there is nothing), the footer's keys, the filter and the status;
- `T` is the text as the frontend draws it: the browser hands each X-ray and day, as text with color codes, to the frontend's `show` once, when it arrives, and keeps the result.

The terminal turns crossterm keys into `Key`; the window turns Rune keys into `Key` and adds ⌘Q, ⌘W and the mouse.

The top row on screen moves into the model as a plain index, so Rows in place works the same in both: the terminal sets the page from its height, the window from the rows that fit in its pane.

*Alternatives.* (a) Make `tokprof` a library too and let the window depend on it: that drags clap, ratatui and every command into the window, and the architecture test would have to accept a CLI as a library. (b) Copy the model into the window: two copies of Rows in place, which took several rounds to get right, would drift. (c) Put the model in `tokprof-core`: the core stays analysis without threads, watchers or clipboard tools.

### The window crate: `tokprof-desk`

It is a binary with `rune` as a git dependency pinned to the full rev, in a workspace of its own: `crates/tokprof-desk/Cargo.toml` has an empty `[workspace]` table and its own `Cargo.lock`, the root workspace `exclude`s it, and it reaches `tokprof-browse`, `tokprof-core` and `tokprof-sources` by path. A member of the root workspace, even one left out of `default-members` or `--exclude`d, would make every root build resolve the whole workspace and so fetch Rune's repository (153 MB): CI on Linux, macOS and Windows and the release builds would fetch it on each cold cache. On its own, the root workspace, its CI, its release builds and `cargo test --workspace` stay as they are, and build no Rune. The window's checks run in its folder (`cargo test`, `cargo clippy --all-targets`, `cargo fmt`); AGENTS.md names them. The architecture test reads the window's manifest with `cargo metadata --no-deps`, which fetches nothing, so the window's dependency edges are checked in CI too. The window has a version of its own, since it is not released.

### Rune state, threads and redraws

The Rune state wraps the browser model. Its workers already send their results on a channel, and the browser takes a wake function when it starts. The window passes one that calls the Rune `Waker` once `AppSpec::setup` hands it over, so each note a worker sends runs `poll`, which runs `tick()`, and the view re-runs only when something arrived. `poll` also asks `tx.wake_after` for `next_cold(now)`, which turns a cell red the moment a cache goes cold without a timer ticking every frame. When nothing changes, the window draws nothing.

### The layout: a profiler

The author looked at three HTML sketches of the window on 2026-11-06 (a Mac list and detail, a dashboard, and a profiler) and chose the profiler: the first version drew the X-ray as the terminal's monospaced text, and in a window that read as a terminal. So the window is laid out as Instruments is: the day's lanes along the top, the sessions below them on the left, and an inspector of the selected session with tabs on the right. The day is always on screen, which a terminal could not afford, and it is where the window says most about the cache: when each session last asked, and how long its cache stays warm after.

The window draws its own title bar (`Chrome { full_size_content, traffic_lights: Some((18, 16)), drag_regions: true }`): the bar is a `window_drag` element 46 points high whose content starts at `Cx::titlebar_inset()`, so the traffic lights never cover the day's name, as they covered the first version's title.

### The list

A virtual row list (`virtual_rows`), since 832 sessions are listed on the author's machine, 28 points a row, with sticky separator rows naming the day in time order. A row shows the time, the agent's dot, the title (or the project until the glance brings the title), the cost and the next-request cell, in the system font with tabular figures. While a listed cache is warm, its cell counts down the seconds it stays warm, so the window asks Rune to wake it every second while a warm row is on screen, and only then; otherwise it sleeps until `next_cold`, as before.

Rows in place in the window: Rune's virtual rows keep the reader's first row on screen wherever a row comes, so after every relist the window scrolls the list to the first row the browser keeps there, as `tokprof ui` does.

### The inspector: the X-ray as data

The X-ray worker sends the `XRay` it computed (in an `Arc`) together with its text, and the browser keeps both; the terminal keeps drawing the text, and the window draws the `XRay`. The inspector's blocks read the same fields as `render_xray.rs` and format them with the same `tokprof_core::fmt` helpers, so a number in the window is the number in `tokprof scan`. That makes a second renderer of the X-ray, the cost the first version avoided; the window's tests compare the inspector's numbers with the text renderer's for the same `XRay`, so the two do not drift silently.

Two things the text shows only as words become data in the core:
- the requests of the main conversation (`XRay::points`: time, prompt tokens, cost), which `Summary`'s request track and the `Requests` tab draw;
- the ways to continue: each carry-on advice line keeps its kind (resume, handoff, compact) and its price next to its text, so `Summary` can put the three side by side and mark the cheapest without parsing a sentence.

### The lanes: shapes from the strip, a tail for the cache

The day worker returns the `rhythm::View`, the window and the axis together with the text, split at the strip into the text above it, the strip's lanes and the text below (`DayParts`). The window draws the lanes from `StripLanes`, where work, answers, questions, actions and sittings are spans in milliseconds; each becomes an element placed at `(t − start) / (end − start) × width` in its lane, at least 1 point wide, and spans of one kind closer than a point merge. `Fit` narrows `start` and `end` to whole hours around the day's actions, now and the tails; `24 h` uses the strip's own 24 hours.

A lane's tail runs from the session's last request to when its cache goes cold. Both times come from the session's glance, which knows the cache's TTL from the log; the glance also brings the session's title for the list. So `Glance` gains the last request's time and the title, and stops being `Copy`.

The line under the lanes names what is under the pointer. It comes from the same spans and the tails, found by the pointer's time in the lane under it.

### The model in the window's mode

The terminal opens the day full screen and closes it again; the window always shows one. `Browser::keep_day_beside()` puts the model in the window's mode: the day is always computed for the lanes (the latest day by default), selecting a session shows its day, ← and → step the day instead of opening and closing the X-ray, `d` shows the selected session's day, ⏎ shows the inspector full size and esc ends that. The keys' meanings stay in the model, and the terminal, which never calls it, keeps its own.

A session's day is the local date of its last activity five hours earlier, since work-rhythm's day runs from the middle of one night to the middle of the next, around five in the morning on the author's logs. When the day that comes back has no lane of the session, the lanes stay on it.

### The keyboard in the window's panes

The author asked on 2026-11-06 to move through every list of the window from the keyboard. The window keeps which pane has the keyboard (the lanes, the list or the inspector) and the inspector's row cursor itself: the model knows the list and the days, not the window's panes, tabs or tables. The window's key handler sends the keys of the lanes and the inspector to the window's own actions and every other key to the model, so the list's keys stay the model's.

Rune's focus is opt-in, and the window opts no element in: every key reaches the window's handler, and a click on a control never takes the arrows away. The inspector numbers its rows as it draws them (`row-0`, `row-1`, …), across the tables and bar charts of a tab in their order, so the cursor is a number and `Tx::scroll_into_view` finds its row.

### Headless frames

Before `rune::run`, `main` reads `--snap`, `--day` and `--size` itself. A bad `--size` fails with our message before Rune sees it. With `--snap`, it does not watch the logs, and it builds the model synchronously: it lists the sessions, waits up to 20 s for the X-ray of the newest one (or for the day given by `--day`), and 0.5 s more for costs, the same waits as `ui --dump`. Then it hands Rune a state that is already filled. Rune renders the frame on its virtual clock and writes the PNG and the tree.

### Log fields and agent versions

The window reads nothing new from the logs. It shows what session-logs, session-xray and work-rhythm already read, so it depends on the same fields and the same Claude Code and Codex versions as they do (see the sources' READMEs).

### Parse time

There is no change to parsing. The window costs every session in the background, as `tokprof ui` does: about 8 s of CPU on the author's corpus, spread over the cores. `tokprof ui --dump` took 1.2 s wall time on 2026-11-06. The move to `tokprof-browse` changes no work, only where the code lives.

## Risks / Trade-offs

- [Rune is pre-release and changes fast: about 1,560 commits in the last 30 days, and the authors do not take pull requests] → Pin a full rev. Move it by hand, in a commit of its own that does nothing else, after `tokprof-desk --snap` still renders the same frames. The window is a leaf crate: if Rune breaks, the CLI is not affected.
- [The move breaks the terminal browser in a way the tests miss] → Move first and build the window second. Compare `tokprof ui --dump 190x50` before and after the move on the same logs with the cost workers given time to finish. Move `ui.rs`'s 1,237 lines and their tests as they are, without rewriting them.
- [Rows in place behaves differently with a pixel scroll offset than with a terminal's row offset] → Unit-test the model with the top row index. Check the window with a Rune script that adds a session while the list is scrolled, then `--tree` the rows on screen before and after.
- [Build time and size: `cargo test --workspace` on the author's Mac now builds Rune too, and `target/` grows] → It is a one-time 1.5 min, incremental after. If it gets in the way, AGENTS.md can drop `--workspace` for the CLI checks and run `-p tokprof-desk` on its own.
- [Several sessions edit this repository at once, and the move touches `ui.rs`, which they may be editing] → Do the move in one commit that moves code and changes nothing else, so a parallel edit rebases onto it as a move.
- [A text renderer change shows up in both frontends] → That is intended. The window checks it through `--tree`, which holds the X-ray text.

## Migration Plan

1. Add `tokprof-browse` and move the model into it. `tokprof ui` must stay the same. Commit.
2. Add `tokprof-desk` with the list and X-ray, then the day strip, then headless frames. Commit each step.
3. Rollback: removing `crates/tokprof-desk` and its workspace line undoes step 2. Step 1 stays, since it is a refactor with no change in behavior.

## Open Questions

- The window's icon and its name in the Dock while it is not a `.app` bundle. This can wait for the bundle change.
