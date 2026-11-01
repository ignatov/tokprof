# Proposal

## Why

Small faults in what tokprof prints and writes, found while writing the baseline specs. Each one is minor, but together they make the tool feel careless:
- `tokprof flame --stack foo` fails, yet first creates `~/tokprof`.
- `-o x.HTML` writes folded stacks, because extensions are compared case-sensitively.
- `unknown component "foo"` does not list the valid names, while the unknown dimension error does.
- A JFR whose `--stack` dimensions are all taken by `--threads` is written with 0 samples and no message.
- `wrote 1 frames`, `1 threads`, and in the X-ray `1 misses`, `1 subagents`, `1 requests`.
- In `tokprof ui`, the export fails as a whole when one subagent log cannot be parsed, though `tokprof flame` skips it. `o` opens the page only with macOS's `open`.
- `tokprof ui --dump 160xabc` fails with `invalid digit found in string`.
- The full-screen X-ray in `tokprof ui` scrolls past its last line.
- The handoff note counts `[Request interrupted by user]` as a prompt; it shows up in the notes of 3f951de0 and dcc54952.

## What Changes

- The profile directory is created only after the options are checked.
- Output extensions match ignoring case.
- The unknown component error lists `read, write, rewrite, uncached, output`.
- A JFR with no dimension left below its threads is an error.
- Counts of 1 take the singular in every message: `flame`'s line, the X-ray, `sessions` and the ui.
- The ui export leaves unreadable subagent logs out and says how many; `o` uses `xdg-open` outside macOS.
- `--dump` with a bad size says `--dump takes WIDTHxHEIGHT`.
- Scrolling the full-screen X-ray stops at its last page.
- The note leaves `[Request interrupted …]` lines out of its prompts.

Out of scope:
- The `next` line's share for a session that cost $0.00 so far (`= 181% of the session's $0.00`). That line belongs to the open change `detect-idle-compaction`.
- The wording of the advice items, which belongs to the open change `compact-xray-advice`.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `profile-export`: the profile directory, extensions, the unknown component error, threads with no stack below them, singular counts in the summary line.
- `session-browser`: the ui export with unreadable subagent logs, opening the page on Linux, the `--dump` error, scrolling the full-screen X-ray.
- `handoff`: interruption markers are not prompts.

## How it is checked on the real logs

- `tokprof flame --stack foo` with `TOKPROF_DIR` set to a fresh directory fails and leaves no directory.
- `tokprof --session 3f951de0 flame -o /tmp/x.HTML` writes an HTML page.
- `tokprof handoff 3f951de0 --note` has no `[Request interrupted` prompt.
- The X-ray of a session with one miss and one subagent reads `1 miss` and `1 subagent`.

## Impact

`src/main.rs` (directory creation, extensions, errors), `src/export.rs`, `src/jfr.rs`, `src/ui.rs` (export, open, `--dump`, scrolling), `src/handoff.rs` (prompts), `src/xray.rs` (plural words only, outside the lines the open changes rewrite).
