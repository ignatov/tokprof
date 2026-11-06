# Proposal

## Why

`tokprof ui` is a terminal browser. A terminal cannot draw a day strip finer than one character per 10 minutes, cannot show a cost bar or a cooling cache except as colored characters, and takes no mouse. A native window can, and Rune (github.com/anomalyco/rune) is a Rust UI framework with its own GPU renderer that also renders headless to PNG. So the window can be checked without a person, as `tokprof ui --dump` is checked now.

## What Changes

- A new program, `tokprof-desk`, opens a native window that does what `tokprof ui` does: the list of sessions with their cost and next-request cell, the X-ray of the selected session, live updates from the agents' log directories, filter, sort and re-sort, the JFR and HTML exports, the handoff copy, and the day with steps through days.
- The window is laid out as a profiler, as the author chose from three HTML sketches on 2026-11-06: the day's lanes along the top (the user's sittings, one lane per session with its work, waits, messages, questions and stops, a tail as long as each session's cache stays warm, and a line at now), the sessions below them on the left, and an inspector of the selected session with tabs on the right. Everything is drawn as native controls in the system font, not as terminal text, and the window draws its own title bar with the day's name, so the window's buttons cover nothing.
- The list shows each session's title, and while its cache is warm, the time it stays warm, counting down.
- The inspector shows the X-ray's numbers as controls: a track of the session's requests on a time axis up to now, the next request's cost and the ways to continue with the cheapest marked, then tabs for requests, spend, work, waits, prompts, tools, context and cache.
- The window takes the same filters on its command line as `tokprof ui` (`--project`, `--session`, `--since`, `--until`) and the same keys, except that ← and → step the day, and it adds the mouse: a click on a row or a lane selects a session, a click on a tab shows it, the wheel scrolls, and a double click shows the inspector full size.
- `tokprof-desk --snap <png>` renders one frame without a window, after it waits for the X-ray of the newest session as `tokprof ui --dump` waits. `tokprof-desk --tree <json>` writes the element tree of that frame.
- `tokprof ui` stays and does not change. The code it shares with the window (the session list and its background work, the log watcher, the X-ray and day text) moves into a library used by both. `tokprof ui --dump` prints the same frame before and after the move.
- The window is built and installed by hand on macOS for now: `cargo install --path crates/tokprof-desk --root ~/.local`. The release archives, the release workflow and the Windows and Linux CI jobs do not include it while Rune is pre-release.

## Capabilities

### New Capabilities
- `desk-window`: the native window over the session browser: its layout, mouse and keys, the day strip drawn with shapes, and the headless frame.

### Modified Capabilities
None. session-browser, session-xray and work-rhythm keep their requirements; the window refers to them by name.

## Impact

- New crates: `crates/tokprof-browse` (the browser's model, moved out of `crates/tokprof/src/ui.rs`, `watch.rs`, `day.rs` and the X-ray and rhythm text renderers), and `crates/tokprof-desk` (the window).
- `crates/tokprof/tests/architecture.rs` learns the two packages and their edges: `tokprof-browse` on `tokprof-core`; `tokprof` and `tokprof-desk` on `tokprof-browse`, `tokprof-core` and `tokprof-sources`.
- A new git dependency, Rune, pinned to rev `781e2b6d87cd5f32d6f4365956fad4baaf24ee50` of its `dev` branch, only in `tokprof-desk`. It brings about 200 more crates (wgpu, Parley, Taffy, AccessKit); a clean release build of Rune's smallest example takes 1 m 30 s on the author's Mac, and its binary is 22.7 MB against 8.5 MB for `tokprof`. `cargo build -p tokprof` does not build Rune.
- Rune makes no network calls in what the window uses, so the window stays local only. The window is one more way to look; console first stays: `tokprof` and `tokprof ui` are unchanged.
- `tokprof-core` keeps as data what the X-ray and the list already print as text: the main conversation's requests (`XRay::points`), the kind and price of each way to continue in the carry-on advice, and a session's title and last request in its glance. No output of the CLI changes.

## How it is checked on the real logs

- `tokprof ui --dump 190x50` before and after the move to the shared library: the same lines, apart from costs that are still being computed.
- `tokprof-desk --snap` and `--tree` against `tokprof ui --dump 190x50` on the same moment: the same session count (832 on 2026-11-06, as the list title counts), the same total once every cost is known, and the same first 20 rows with their costs and next-request cells.
- The numbers of the inspector of a finished session (cost, context, next request, spend by component, prompts, tool rows) equal those of `tokprof scan` of it.
- The day of 2026-11-03: the same lanes and sessions as `tokprof day 2026-11-03`, and the same `sittings`, `answers` and `cache` lines.
- Time to the first frame with the list, and to the first X-ray, against `tokprof ui --dump`, which took 1.2 s on 2026-11-06; the full-corpus parse time is unchanged, about 8 s.

## Out of scope

- Builds for Windows and Linux, release archives, signing and a `.app` bundle.
- Views that `tokprof ui` does not have, beyond the request track and the cache tails: charts of spend over days, flame graphs in the window, the pool and the share files.
- Changes to `tokprof ui` beyond the move of its code.
- Settings, themes other than the system's light and dark, and a menu-bar or tray mode.
