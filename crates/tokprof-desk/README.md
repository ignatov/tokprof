# tokprof-desk

The session browser of tokprof in a native window: what `tokprof ui` shows in the terminal, drawn with [Rune](https://github.com/anomalyco/rune), a Rust UI framework with its own GPU renderer.

- The list of sessions on the left: the last activity, the agent, the project, the cost so far with a bar as long as its share of the dearest session listed, and what one more request would cost now, green while the prompt cache is warm and red once it is cold. The cell turns red at the moment the cache goes cold.
- The X-ray of the selected session on the right: the text `tokprof scan` prints, in the system's monospaced font with its colors. Select text with the mouse and copy it with ⌘C.
- The day of your latest message with `d`: the strip drawn at the times of the log, a lane per session, the time under the pointer named under the strip, and a click on a lane to select its session.
- The keys of `tokprof ui` (see the key table in the [README](../../README.md#quick-start)), ⌘Q and ⌘W to quit, a click to select a session, a double click to show its X-ray full size, and the wheel to scroll.
- It follows the agents' log directories as `tokprof ui` does: new sessions join the list, sessions that grow are costed again, and the rows on the screen stay where they are.

## Install

macOS only for now: Rune is pre-release, so the window is in no release archive and in no CI job.

```sh
cargo install --path crates/tokprof-desk --root ~/.local
tokprof-desk                               # every session
tokprof-desk --project tokprof --since 7d  # the filters of tokprof ui
```

## A frame without a window

Rune renders headless with the same renderer, so the window can be checked without a person:

```sh
tokprof-desk --snap out.png --tree out.json             # the list and the X-ray of the newest session
tokprof-desk --snap day.png --day 2026-11-03            # the day view of that date
tokprof-desk --snap out.png --size 900x600 --theme light
```

Without a window it follows no logs: it waits up to 20 s for the X-ray of the newest session, or for the day `--day` names, then 0.5 s more for costs, as `tokprof ui --dump` waits. `--tree` writes every element with its text and place. The size is in points, 1400x900 by default.

## How it is built

The sessions, their costs, X-rays and days come from `crates/tokprof-browse`, which `tokprof ui` uses too: the keys mean the same in both. This crate draws them:

| File | What it holds |
|---|---|
| `src/main.rs` | The command line, the state of the window, the poll that takes in what the background work sent, and the keys |
| `src/view.rs` | The list, the X-ray pane and the footer |
| `src/day.rs` | The day view and its strip |
| `src/ansi.rs` | The X-ray's and the day's color codes as styled ranges |
| `src/tests.rs` | The window driven headless through Rune's `Runtime` |

The browser's workers wake the window when they send; nothing polls while the window rests.

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
