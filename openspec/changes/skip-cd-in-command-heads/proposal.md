# Proposal

## Why

A shell row is named after the command head: the first words of the command, after a leading `cd <dir> &&`. A command that starts with `cd <dir>; ` keeps the `cd` in its head, so every such command of one folder makes one row, such as `bash cd /w/app;`, whatever it runs. In the saved Copilot logs at least 1,477 of 12,230 `bash` calls start that way and fall into one row; Claude Code transcripts hold about 9,900 such commands against about 189,000 with `&&`. Codex commands seen start with no `cd`.

## What Changes

- The command head skips a leading `cd <dir>` up to the first `&&` or `;`, whichever comes first. `cd /w; ./gradlew test` gives `gradlew test`, and `cd /w; git status && git diff` gives `git status`, where it gave `git diff` before.
- Where it shows: the rows of `tokprof tools`, the X-ray's `tools` block and the window's Tools tab; the programs of a share file, which follow the head.

## Capabilities

### New Capabilities
None.

### Modified Capabilities
- `tool-breakdown`: changes `Rows by tool and command`, which now says how a leading `cd` is skipped.

## How it is checked

- `tokprof --agent copilot tools` on the saved Copilot logs: no row named `bash cd …` is left, and the calls of the old `bash cd /home/runner/…;` row go to the rows of what the commands ran.
- `tokprof tools` on the author's corpus: the calls and the spend stay the same; only some Claude Code rows change, such as `Bash for`.
- The goldens do not change: no fixture command starts with `cd <dir>;`.

## Out of scope

- Other ways to set the folder, such as `pushd`, `(cd <dir> && …)` or `git -C`.

## Impact

`labels::command_head` in `tokprof-core` and its unit test. No change to the share file's format.
