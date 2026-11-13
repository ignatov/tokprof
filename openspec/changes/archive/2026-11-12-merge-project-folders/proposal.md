# Proposal

## Why

tokprof names a session's project after the folder the agent ran in. So every git worktree, every subfolder of a repository, every temporary folder of a harness run and every Codex Desktop chat is a project of its own. On the author's logs that gives 391 project paths over all logs and 116 in 9–12 Nov 2026. Grouping by project barely groups. The coming desk changes group by project a lot (add-sessions-screen, add-week-view, add-month-quarter-views, add-year-view). So this small change lands before them and places each folder in the project it belongs to.

## What Changes

- **A project is placed from the folder**, by the first rule that gives an answer:
  1. **A group the log's own source names.** The Codex source names Codex Desktop chat folders, `~/Documents/Codex/<YYYY-MM-DD>/<name>`, as one project, `codex chats`.
  2. **Temporary folders.** A folder under the system's temporary folder (`$TMPDIR`, such as `/var/folders/…/T`, and its resolved form `/private/var/folders/…/T`) or under `/tmp` or `/private/tmp` has the project `temporary folders`. These are the roots the rhythm already leaves out.
  3. **The repository.** git's own files say where the repository's root is: the nearest `.git` folder above the session's folder, or, for a linked worktree, the main working tree that its `.git` file and `commondir` name. Subfolders fold into the root. Named copies that git says are worktrees, such as `~/src/heron-2`, fold into `~/src/heron`.
  4. **A remembered repository.** Each repository root and worktree root tokprof finds is written to `repositories-1.json` in its cache folder. A worktree deleted later keeps its sessions in its repository.
  5. **Else the folder itself**, as today.
- **Never guess.** No rule reads the layout of a path (worktree's pool, `~/.air/worktrees`), a folder's name, a branch or a remote. A `.git` in the home directory or above is never read, so a home kept in git does not swallow other folders. A folder that is gone and was never seen keeps its own path.
- **The session's own folder stays known.** Every view that names or groups by project shows the project. The X-ray header (`tokprof`, `tokprof scan`, the `tokprof ui` pane) adds a line after `session` when the folder differs: `worktree <folder>` for a worktree, else `folder <folder>`. A folder that is its own project and is gone from disk reads `folder    not found`, in the X-ray only. The window's inspector shows the `worktree` or `folder` line under the project.
- **`--project` matches either.** It keeps a session when its project or its own folder contains the text. `--project ~/src/zebrafish` takes in its worktrees and subfolders; `--project branch-deck` still finds the sessions of worktree's pool; `--project temporary` finds the temporary folders.
- **The day store keeps both.** An entry names its project and, when it differs, the session's folder. The format stays `2`. The first run of the new build reads every log on disk again, as after any build. Entries of deleted logs are placed again on the next refresh.
- **Where it shows.** No view changes for the merge itself: `tokprof sessions`, the X-ray, `tokprof misses`, the `project` frame of `tokprof flame` and the profiles, `tokprof calendar --project`, `tokprof month --project`, the window's list, palette and Reports' `By project` card, and the four desk changes take the new projects.
- **Privacy is as before.** `tokprof share` and the month card still name no project or folder.

Checked on the author's logs (measured 2026-11-12, tokprof 0.1.6, `days-2.json` with 2,347 logs, the folders on disk that day):

| window | project paths now | projects after |
|---|---|---|
| all logs (6 Aug – 12 Nov) | 391 | 35 |
| 2–8 Nov 2026 | 18 | 10 |
| 9–12 Nov 2026 | 116 | 8 |
| October 2026 | 230 | 16 |

- By rule, over 405 distinct folders (a folder seen by two agents counts twice): 298 temporary, 32 Codex chats, 24 in worktrees, 22 in a repository (its root or a subfolder), 13 in no repository, 3 gone and never seen.
- The 24 worktree paths fold into 6 repositories: `~/src/zebrafish` (3 worktree leases), `~/src/heron` (`heron-2`, `heron-3`, 5 leases and 5 of their subfolders), `~/src/sample-lang` (2 Codex worktrees, 1 Air worktree, 1 `.openidea` worktree), `~/src/stilt` (3 Air worktrees), `~/src/tern` (2) and `~/src/sample-demo` (1). They hold $3,298 of $24,603 spent (13.4%).
- After the merge the top projects over all logs are `~/src/heron` $11,721, `~/src/osprey` $3,915, `~/src/wren` $2,315, `~/src/plover` $2,221 and `~/src/zebrafish` $1,776. `codex chats` holds $46 and `temporary folders` $13.
- 3 folders stay apart and show `folder    not found`: `~/.air/worktrees/temporary/tool-agent-royal-squid`, `~/.codex/worktrees/0dca/sample-lang` and `~/.codex/worktrees/997c/sample-lang`, all with under $1. `…/5/heron/out/sample-demo` is a repository of its own inside a lease and stays apart, as it should.
- Placing all 405 folders took 42 ms in a Python script. The Rust code must add under 1% to the 7.1 s that `tokprof summary` takes over all logs.

The implementation re-runs these counts with the new build: the distinct projects of `tokprof sessions --limit 100000` for all logs and for each window, `Total priced` of `tokprof summary` (it must not change), and the read time of its `read …` line.

Out of scope:
- merging separate clones of one remote;
- the handoff note: it keeps naming the folder the session ran in, since the next session must start there;
- the `/` filter of `tokprof ui`, which keeps matching the agent, the project and the id;
- groups that other sources could name, such as chat folders of other agents;
- worktrees and subfolders listed under their project in the Sessions screen's Projects mode and the `By project` cards. This is a later idea for add-sessions-screen.

Decisions the author made on 2026-11-12: temporary folders are one project; named copies that are worktrees merge; subfolders fold into the root and show as a line like a worktree; Codex chat folders are one project; a folder that is not found is marked in the X-ray only; worktrees under a project in the desk views wait for a later change.

Open questions for the author:
1. **Codex's chat folder.** The rule takes `~/Documents/Codex` as Codex Desktop's folder, as seen in every chat rollout from 0.147.0-alpha.6.5 to 0.166.0-alpha.5.1. If Codex Desktop lets the user move it, chats elsewhere stay their own projects. Is that fine until it happens?
2. **The name `codex chats`.** Other agents' chat folders could get their own groups later. Is `codex chats` the right name, or `Codex chats` like the client name `Codex Desktop`?

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `session-logs`: the project of a session is placed from its folder by source groups (Codex chat folders), temporary folders, the repository from git's files and remembered repositories; `--project` also matches the session's own folder.
- `activity-calendar`: an entry names the session's folder when it differs from the project, and kept entries are placed again on refresh.
- `cost-attribution`: the `project` dimension is the session's project as session-logs defines it.
- `session-xray`: the header names the folder a session ran in, and a folder not found.
- `desk-window`: the inspector names the folder a session ran in.

## Impact

- The project is placed in one shared place after a log source has read the folder. A source can name a group for a folder of its own agent; only the Codex source does, so the shared code names no agent.
- A new file `repositories-1.json` in the cache folder, next to `days-2.json`.
- The day store's entry gains an optional field; the store format stays `2`.
- Golden outputs change where a fixture runs in a subfolder, a worktree or a temporary folder; the X-ray header gains a line for such sessions.
- README: what a project is, the rules, and the `worktree` and `folder` lines.
- No new dependency. No network.
