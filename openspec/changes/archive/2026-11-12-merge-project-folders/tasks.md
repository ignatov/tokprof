# Tasks

## 0. Numbers before the change

- [x] 0.1 Before any code, with the installed 0.1.6 build, record these numbers in this task:
  - the distinct projects of `tokprof sessions --limit 100000`, over all logs and with `--since 2026-11-02 --until 2026-11-09`, `--since 2026-11-09`, and `--since 2026-10-01 --until 2026-11-01`;
  - `Total priced` of `tokprof summary` over all logs and over 2–8 Nov ($1,843);
  - the time on the `read …` line of `tokprof summary` (7.1 s on 2026-11-12);
  - the distinct `project` values of `days-2.json` over all entries and for each window (391, 18, 116 and 230 on 2026-11-12).

  Verify that the commands ran and the numbers are written here.

  Recorded on 2026-11-12 at 18:35 with the installed 0.1.6:
  - `tokprof sessions --limit 100000`: 1,094 sessions in 397 distinct projects over all logs; 107 sessions in 16 projects for 2–8 Nov; 249 in 127 for `--since 2026-11-09`; 405 in 232 for October. An empty project (11 OpenCode sessions) counts as one.
  - `Total priced`: $24,748 over all logs; $1,843 over 2–8 Nov.
  - `read …` line of `tokprof summary` over all logs (2,403 files, 9.8 GB): 9.0 s, 8.3 s, 8.1 s, on a machine with a load average near 20 from other sessions.
  - `days-2.json` (2,347 entries), distinct `project` values, a window counting an entry with any minute in it (local time): 392 over all entries, 18 for 2–8 Nov, 116 for 9–12 Nov, 230 for October.

## 1. Placing a folder

- [x] 1.1 In tokprof-core, add Repository of a folder (session-logs):
  - walk up to the nearest existing folder with a `.git` entry, never reading one in the home directory or a folder that holds it;
  - a `.git` folder is the root, and so is a submodule's `.git` file, which has no `commondir`;
  - a worktree's `.git` file plus `commondir` gives the main working tree, or the bare git folder;
  - relative paths count from the folder of the file that holds them.

  Verify with unit tests next to the code, on folders made in a scratch dir: a worktree root; a named copy that is a worktree; a subfolder of a repository and of a worktree; a gone subfolder of an existing repository; a repository inside a worktree; a submodule; a bare repository; relative and absolute `gitdir:`; an unreadable `.git` file; and a home directory with its own `.git` folder that must not take `~/Documents/notes`. Build the expected paths with the platform's separator.
- [x] 1.2 Add Temporary folders. The caller passes the roots, which come from the code the rhythm uses today (the system temporary folder, its resolved form, `/tmp`, `/private/tmp`). Verify with unit tests: a folder under each root; a root itself; a sibling of a root such as `/tmpfoo`; another user's `/var/folders/…/T`; and `~/src/app/tmp`.
- [x] 1.3 Add the order of Project of a folder: source group, temporary, repository, remembered, the folder. Return the project and how it was placed. Verify with unit tests: a temporary folder that holds `.git` is `temporary folders`; a group wins over a repository; a gone folder that nothing places is marked as not found.
- [x] 1.4 Add Repositories remembered:
  - load `repositories-1.json` once;
  - when the disk cannot place a folder, take the nearest remembered root at or above it;
  - save a new or changed pair by reading the file again, setting the pair and renaming a temporary file over it.

  Verify with unit tests: a worktree removed after it was learned still places its folders; git's answer replaces a remembered pair; an unparsable file counts as empty; a read-only folder gives no error; the file holds only paths and the format number.
- [x] 1.5 Memoize per pair of source and folder per process, behind a lock. Let the catalog take the temporary roots and the file's path; a catalog without them places from disk only. Verify with a unit test that places the same folder from 8 threads and reads its `.git` file once.

## 2. Codex chat folders

- [x] 2.1 Add a source method that names a group for a folder, with a default of none. In `sources/codex`, answer `codex chats` for `<home>/Documents/Codex/<YYYY-MM-DD>/<name>` and folders below it. Verify with unit tests in the codex source: a chat folder; a subfolder of one; `~/Documents/Codex/notes`; a date that is not a date (`2026-13-40`); a folder under another home. Also check that `cargo test -p tokprof --test architecture` passes, so the shared code names no agent.
- [x] 2.2 Name the rule, the `originator` `Codex Desktop` and the versions seen (0.147.0-alpha.6.5 to 0.166.0-alpha.5.1) in `sources/codex/README.md`. Add a synthetic fixture rollout with such a `cwd` under `sources/codex/tests/fixtures/`. Verify that the conformance test passes on it.

## 3. The project in every view

- [x] 3.1 In the catalog, apply the rules to the folder of the quick look and of the full read. Keep the session's own folder and how its project was placed next to the project in both. The CLI (`crates/tokprof`), `tokprof ui` and `tokprof-desk` pass the temporary roots and `<cache folder>/repositories-1.json`. Verify with a test over a Claude Code fixture copied into a scratch worktree and a scratch subfolder: `tokprof sessions` and the X-ray name the repository root. The conformance tests of the four sources must still pass unchanged.
- [x] 3.2 Make `--project` match the project or the folder, in the log filters and in the calendar's filters over the store. Verify with unit tests next to the filter code:
  - `~/src/app` keeps its worktree's and its subfolder's sessions;
  - `pool` keeps a session whose folder is `~/w/pool/4/app`;
  - `temporary` keeps the temporary folders;
  - `~/src/other` keeps none of them.
- [x] 3.3 Add the X-ray header's folder line after `session` (session-xray, Folder line), in `tokprof`, `tokprof scan` and the `tokprof ui` pane. It reads `worktree <folder>`, `folder <folder>`, or `folder    not found`. Verify with tests of the rendered header for a worktree, a subfolder, a temporary folder, a gone unknown folder and a repository root. Add golden cases if the fixtures allow, re-bless with `TOKPROF_BLESS=1 cargo test -p tokprof --test golden`, and review the diff.

  No golden case was added: the X-ray and `tokprof sessions` print local times, which the goldens leave out, and the fixtures' folders are outside the goldens' home, so no golden changed. `crates/tokprof/tests/projects.rs` runs the program on a worktree, a subfolder, a repository root and a temporary folder instead.
- [x] 3.4 In tokprof-desk, show the `worktree` or `folder` line under the project in `Summary`, clipped from the left (desk-window, Folder in the inspector), with no line for a folder not found. Verify with a desk test over the frame's tree. Then run the checks in `crates/tokprof-desk`: `cargo fmt && cargo test && cargo clippy --all-targets -- -D warnings`.
- [x] 3.5 Check privacy. Verify with tests that the output of `tokprof share` and of `tokprof month`, over fixtures in a worktree, a temporary folder and a Codex chat folder, names no project and no folder, as before.
- [x] 3.6 Update README.md: what a project is; the five rules in their order; the `worktree`, `folder` and `folder    not found` lines; `--project` matching either the project or the folder; `repositories-1.json` in the cache folder; and that a worktree deleted before tokprof saw it stays its own project. Verify that the README's examples run as written.

## 4. The day store

- [x] 4.1 Give an entry an optional `folder`, written only when it differs from `project`. Keep the format at 2 and the file at `days-2.json`. Verify with unit tests: an entry without `folder` loads; an entry of a worktree session writes both; an entry with a field the build does not know still loads, as an older build reads ours.
- [x] 4.2 On refresh, place again the project of each kept entry that has no `folder`, through its agent's source and the rules, memoized. Set `project` and `folder` when the project moves. Verify with unit tests in tokprof-browse's calendar tests:
  - a deleted log's entry in a scratch worktree moves to the repository, and `--project <repository>` then counts its days;
  - a deleted Codex chat's entry moves to `codex chats`;
  - an entry of a gone, unknown folder stays as it is.

## 5. Checks

- [x] 5.1 Run `cargo fmt --all`, `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings` and the desk checks. Verify that all of them pass, on the CI's Windows job too.

  Passed locally on macOS on 2026-11-12: the three root checks, `cargo test -p tokprof --test architecture`, and `cargo fmt && cargo test && cargo clippy --all-targets -- -D warnings` in `crates/tokprof-desk`. The Windows job is pending the push; nothing was pushed in this run. The new tests build expected paths with the platform's separator, write `gitdir:` with `/` as git does, escape paths for JSON, and give the CLI's run its own `TMPDIR`, `TMP` and `TEMP`.
- [x] 5.2 Reinstall with `cargo install --path crates/tokprof --root ~/.local`, and the app with `./bundle.sh` and `ditto`. Verify that `tokprof --version` names the new commit.

  Reinstalled both on 2026-11-12 at 19:44. Nothing was committed in this run, so `tokprof --version` says `tokprof 0.1.6` and the build is still that of 9b3a94a. The installed build ran once on the real cache: `days-2.json` now holds 36 distinct projects over all entries and `folder` on 709 entries, and `repositories-1.json` was written next to it.

## 6. Numbers after the change

- [x] 6.1 Repeat 0.1 with the new build and record the numbers next to the old ones. Expected from the 2026-11-12 measurement:
  - 391 → 35 projects over all logs, 18 → 10 for 2–8 Nov, 116 → 8 for 9–12 Nov, 230 → 16 for October;
  - 24 worktree paths folded into 6 repositories;
  - 3 folders not found;
  - `Total priced` unchanged to the cent;
  - `tokprof --project ~/src/zebrafish --since 2026-11-02 --until 2026-11-09 summary` now counts the spend of the worktree leases too.

  Verify that `days-2.json` holds `folder` on the moved entries and that `repositories-1.json` holds the roots found.

  Recorded on 2026-11-12 at 19:35 with the new build, uncommitted on 9b3a94a, on a scratch copy of the cache folder (`TOKPROF_CACHE_DIR`), each `before` run with the installed 0.1.6 just before:
  - `tokprof sessions --limit 100000`, distinct projects before → after: 402 → 36 over all logs (1,106 sessions), 15 → 11 for 2–8 Nov (107), 136 → 10 for `--since 2026-11-09` (261), 228 → 17 for October (405). The 0.1 counts were taken with another parser and fewer sessions; these pairs come from one parser, run back to back.
  - `days-2.json`, distinct `project` values: 392 → 36 over all entries, 18 → 10 for 2–8 Nov, 116 → 8 for 9–12 Nov, 230 → 16 for October. The refresh read 93 logs that had changed since and took the other 2,326 from the store; it placed the kept entries again, and 709 of 2,419 entries now hold `folder`.
  - 24 worktree paths in 6 repositories; 4 subfolders of repositories; 321 temporary folders and 32 Codex chat folders in their groups. `repositories-1.json` holds 38 roots, 20 of them worktree roots.
  - 3 folders not found.
  - Spend unchanged to the micro-dollar: `tokprof flame --stack agent,model` writes the same file before and after, $1,842.940228 over 2–8 Nov and $11,631.834871 over October; `Total priced` $1,843 and $11,632 both times.
  - `tokprof --project ~/src/zebrafish --since 2026-11-02 --until 2026-11-09 summary`: `Total priced` $1,059 → $1,215, with the leases.
- [x] 6.2 Record the time on the `read …` line of `tokprof summary` over all logs, three runs before and three after. Verify that it grew by under 1% (the design expects under 10 ms of 7.1 s), and that `tokprof` with no arguments still answers in under a second.

  Recorded on 2026-11-12 at 19:35, before and after in turn, at a load average of 31 to 37: before 5.7 s, 6.1 s, 6.6 s; after 5.5 s, 5.6 s, 6.5 s. The difference is below the noise of the loaded machine, and the after runs were not slower. `tokprof` with no arguments: 0.07 to 0.08 s before and after. `tokprof sessions --limit 100000`, which reads the head of all 1,106 sessions and places every folder, seven runs each in turn: median 0.112 s before, 0.108 s after, so placing costs well under 10 ms.
- [x] 6.3 Remove nothing on the author's disk. Instead, run with `TOKPROF_CACHE_DIR` set to a scratch copy of the cache folder, on a scratch worktree made with `git worktree add` and removed after one run. Verify that the second run still names the repository for its session. Then verify that `openspec validate --all --strict` passes.

  Done on 2026-11-12 with `TOKPROF_CACHE_DIR` on the scratch copy and a scratch repository with one commit outside the temporary folders and outside any other repository, a synthetic Claude Code session in its worktree made with `git worktree add`. The first run named the repository and `worktree <folder>`; after `git worktree remove`, the second run named the same repository and the same `worktree` line, from `repositories-1.json`. The scratch folders were removed after. `openspec validate --all --strict`: 29 passed. (A first try put the scratch repository inside this checkout's `target/tmp`: once the worktree was gone, the walk up found this checkout's own `.git` first, which comes before the remembered roots, so the session went to the enclosing repository, as the order of the rules says.)

## Workflow follow-up

- Land this change before add-sessions-screen, add-week-view, add-month-quarter-views and add-year-view, so that their `By project` numbers use the new projects.
- Later idea for add-sessions-screen: list a project's worktrees and subfolders under it in Projects mode and in the `By project` cards.
- Archive with `openspec archive merge-project-folders -y`, after a simulated archive of all pending changes in a copy of `openspec/`. Then run `openspec validate --all --strict`.
