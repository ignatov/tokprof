# Design

## Context

Each log source sets the project from the folder its log names, with `~` for the home directory:

| source | field | seen in |
|---|---|---|
| Claude Code | the first `cwd` of `user` and `assistant` lines (`gitBranch` is there too, unused) | 2.1.319 |
| Codex | `cwd` of the `session_meta` payload, else of `turn_context` (`session_meta.payload.git` has `commit_hash` and `branch`, unused) | CLI 0.167.0; Codex Desktop 0.147.0-alpha.6.5 to 0.166.0-alpha.5.1 |
| pi | `cwd` of the `session` line that starts the log | session format 3 |
| OpenCode | `directory` of the `session` or `session_v2` row (its `project` table has a `worktree` column, unused) | 1.18.16 |

A source does it twice: in its quick look at the first records of a log, used by the session list, the `--project` filter and the X-ray's choice of session, and in the full read. Both go through the catalog that holds the sources. No log names the repository of a folder, so that answer has to come from the disk.

Git keeps a linked worktree as a folder whose `.git` is a file, `gitdir: <repo>/.git/worktrees/<name>`. That git folder holds `commondir`, usually `../..`, which names the repository's git folder. A submodule's `.git` file names `<super>/.git/modules/<name>`, which has no `commondir`. This layout has been the same since git 2.5. Git 2.48 can write both paths relative. Seen with git 2.50.1 on the author's machine.

The rhythm already treats as temporary the system's temporary folder, its resolved form, and `/tmp` and `/private/tmp` on Unix (work-rhythm, Sessions left out). The CLI computes these roots and passes them in.

Codex Desktop starts a chat that has no project in a new folder `~/Documents/Codex/<YYYY-MM-DD>/<slug of the first prompt>`. All 43 such rollouts in the store say `originator` `Codex Desktop`.

The day store re-reads every log that still exists when the fingerprint changes, and every build changes it. It keeps the entries of deleted logs as they are.

See proposal.md for the counts.

## Goals / Non-Goals

**Goals:**
- One order of rules for every agent, applied in one place after the source has read the folder.
- Read at most two small files per folder, and never run git.
- Keep a folder in its repository after the folder is deleted, for the CLI, `tokprof ui`, the window and the day store alike.

**Non-Goals:**
- Any rule from the shape of a path, except the one a source owns for its own agent's folders.
- Changing what a handoff note or a tool target is relative to: both stay relative to the session's own folder.

## Decisions

### 1. The order of the rules

The rules run in this order: source group, temporary folders, repository from disk, remembered repository, the folder itself (session-logs, Project of a folder).
- Source groups come first because only the source knows its agent's own folders.
- Temporary folders come before git, because a harness often runs `git init` in a temporary folder. Each such run would otherwise be a project of its own.
- The memory comes after the disk, so that git's answer for a folder as it is now always wins.

### 2. The repository from git's two files

Walk up from the folder to the nearest existing folder with a `.git` entry. Stop before the home directory and any folder that holds it.
- A `.git` folder: that folder is the root.
- A `.git` file whose git folder holds `commondir`: a linked worktree. The root is the folder above the git folder that `commondir` names, or that git folder itself when it is not named `.git` (a bare repository).
- A `.git` file without `commondir`: a submodule, which is its own root.
- Anything that cannot be read: no answer, and the next rule runs.

A gone folder is placed from the nearest folder above it that exists. So `~/src/app/out/run-17` falls into `~/src/app` after the run folder is removed.

The walk never reads a `.git` in the home directory or above it. A home kept in git (dotfiles) would otherwise fold `~/Documents/notes` and every other loose folder into `~`. Nothing on the author's disk has this today; a unit test covers it.

Alternatives:
- Known layouts, such as Branch Deck's pool (`~/.branch-deck/pool/<repo>-<hash>/<n>/<repo>`), `~/.air/worktrees/temporary/<repo>-<word>-<animal>` and `~/.codex/worktrees/<hex>/<repo>`. Rejected. They give a folder name, not a repository, so `~/src/app` and `~/work/app` would look the same, and they break when a tool changes its layout. That is a guess.
- `git rev-parse --show-toplevel --git-common-dir`. Rejected. It gives the same answer but needs git, and at about 5 ms per process it costs 2 s for 405 folders.
- Codex's `git` fields or a remote URL. Rejected. Only Codex logs them, and a remote joins separate clones, which is a different question.
- OpenCode's `project.worktree`. Rejected. It is one agent only, and it names the folder OpenCode first saw, not git's answer.
- Git's own records of worktrees, `<repo>/.git/worktrees/<name>/gitdir`, to place a folder that is gone. Measured: the 22 repositories found hold 48 records, and these place none of the 3 gone worktree folders, because the tools remove worktrees with prune. Left out.

### 3. A source names a group without the core naming the agent

`LogSource` gains a method with a default answer of none: `fn folder_group(&self, folder: &Path) -> Option<&'static str>`.
- It is a pure function of the folder's path, so the catalog can also ask it for a kept store entry whose log is gone. It asks the source of the entry's agent.
- The Codex source answers `codex chats` when the folder is `<home>/Documents/Codex/<YYYY-MM-DD>/<name>` or below it. The home directory is the one the source was made with.
- The other three sources keep the default.

The shared code passes the group name through as the project and never compares it with a literal, so `crates/tokprof/tests/architecture.rs` keeps passing: the word `codex` appears only under `sources/codex`. The source's README names the rule and the versions seen.

Alternative: a regex list in the core configured by the CLI. Rejected. It would put Codex's layout outside its source.

### 4. Apply it in the catalog, memoized per folder

The catalog applies the rules to the folder of the quick look and of the full read, so every command, `tokprof ui` and the window see the same project. The sources still read the folder as before. The quick look and the full read keep the session's own folder next to the project, plus how the project was placed (group, temporary, repository, worktree, remembered, the folder itself, the folder not found). The X-ray uses this for its `worktree` and `folder` lines.

Each distinct pair of source and folder is placed once per process and kept in a map behind a lock. The conformance test kit runs a source without the catalog, so it keeps comparing the source's own folder in the quick look and in the full read.

The library reads no environment. The CLI and the window give the catalog two things: the temporary roots, from the code that the rhythm uses today, and the path of `repositories-1.json`, in the same cache folder as `days-2.json`. A catalog made without them (tests, `--scene` frames) places folders from disk only.

### 5. Remember roots in their own small file

`repositories-1.json` holds the format number and pairs of root and project, as absolute paths. A pair is either a repository root with itself, or a worktree root with its repository.
- The file is read once, when the catalog first needs it.
- A pair that is new or different is saved like this: read the file again, set the pair, write a temporary file and rename it over the old one, as the day store does. Two processes that learn pairs at once may lose one; the next run learns it again while the folder exists.
- A folder that the disk cannot place takes the project of the nearest remembered root at or above it.

Alternatives:
- Keep the pairs only in the day store. Rejected. `summary`, `flame`, `sessions` and the X-ray read logs, not the store, so after a worktree is deleted they would disagree with the calendar.
- Keep the pairs inside `days-2.json`. Rejected. The X-ray would have to load a 5 MB file to answer in under a second.

### 6. The day store keeps its format

An entry gains an optional `folder`, written only when it differs from `project`. An older entry has none and reads as a session that ran in its project. A new format, `days-3.json`, would start a new store and lose the days of deleted logs that `days-2.json` keeps (activity-calendar, Another format). So the format stays `2`.
- Logs still on disk: the first run of the new build reads them again because the fingerprint changed, as after every build.
- Entries of deleted logs: on each refresh, an entry without a `folder` is placed again by its agent's source and the rules, memoized. That costs one lookup per distinct project path.
- An older build that reads the file ignores `folder` and re-reads the logs on disk, so their entries go back to paths.

### 7. What shows the folder

The project replaces the path everywhere it was shown, so no view changes its own code for the merge. The folder shows in two places:
- the X-ray header's `worktree` or `folder` line, and `folder    not found` (session-xray);
- a line under the project in the window's `Summary` (desk-window), without the not-found mark.

The `/` filter of `tokprof ui` and the handoff note do not change. A project such as `temporary folders` is not a path. Code that treats the project as a path must not join it onto the disk; today only the profile's default file name uses it, and it takes the `--project` text, not the project.

## Effect on parse time

The rules run once per distinct pair of source and folder, not per log or per request. Over all logs that is 405 pairs.
- Most pairs end at a string compare: the 298 temporary and 32 chat folders.
- The other 75 walk up with about 8 `stat` calls each. The 24 worktrees also read two small files.

A Python script placed all 405 in 42 ms, single-threaded. The Rust code should take under 10 ms, against 7.1 s for `tokprof summary` over all logs (9.7 GB, 2,347 logs): under 0.2%. The X-ray places one folder and reads `repositories-1.json` once, a few KB. Task 6.2 measures it.

## Risks / Trade-offs

- [A root reused later for another repository] → git's answer for the folder as it is now wins, and the pair is updated. Branch Deck's pool keeps a slot per repository, so it does not happen there.
- [Sessions in subfolders of a monorepo lose their own line in By project] → the folder stays in the X-ray, the inspector and `--project`. Sub-lines in the desk views are a later idea for add-sessions-screen.
- [A worktree deleted before tokprof ran once] → it stays its own project and shows `folder    not found`. 3 such folders exist today, with under $1 of spend.
- [Projects change between runs while a folder appears or goes] → only until its root is remembered; after that they stay the same.
- [Codex Desktop moves its chat folder] → chats elsewhere stay their own projects (proposal, open question 1).
- [Folders on a slow or network disk] → one walk per folder per process, stopping at the first `.git`.
- [Windows] → `gitdir:` holds `C:/…` with forward slashes, the temporary root is `%TEMP%`, and printed paths use `\`. Tests compare paths built with the platform's separator.
- [Privacy] → `repositories-1.json` holds paths only, like `days-2.json`, and stays local. `tokprof share` and the month card never name a project, so the merge cannot leak through them. A test checks that no folder or project appears in their output.

## Migration Plan

1. Ship with the next build. Its first run re-reads the logs on disk, as every build does, and writes `repositories-1.json`.
2. Rollback: an older build ignores `folder` and `repositories-1.json`. Its first run re-reads the logs on disk and gives them path projects again. Entries of deleted logs keep the new project.
