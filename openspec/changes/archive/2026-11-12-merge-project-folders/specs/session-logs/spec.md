## MODIFIED Requirements

### Requirement: Session id and project
Each log SHALL carry a session id and a folder read from the log, as Claude Code and Codex session ids, Pi session id and project and OpenCode sessions say; the project those requirements name is this folder. The session's project is placed from the folder (see Project of a folder). The folder and a project that is a path are written with `~` for the home directory.

#### Scenario: Claude Code subagent transcript
- **WHEN** a subagent transcript's lines carry the parent's `sessionId`
- **THEN** its requests are reported under the parent session's id

#### Scenario: Codex subagent thread
- **WHEN** a Codex subagent rollout of session `M` is read
- **THEN** its requests are reported under `M`'s id, not under its own thread id
- **AND** `tokprof misses` lists its misses with `M`'s short id and the thread's chain, such as `subagent:review`

#### Scenario: Codex thread whose parent is not on disk
- **WHEN** a Codex rollout names the parent thread `P`, and no rollout of `P` is found
- **THEN** its requests are reported under `P`

#### Scenario: No id in the log
- **WHEN** a Claude Code transcript has no `sessionId`, or a Codex rollout has no `session_meta` id
- **THEN** the id comes from the file name: the transcript's name without `.jsonl`, or the 36-character thread id at the end of the rollout's name

#### Scenario: Project under the home directory
- **WHEN** the first `cwd` is `/Users/me/src/app` and the home directory is `/Users/me`
- **THEN** the project is `~/src/app`

#### Scenario: A session in a worktree
- **WHEN** the first `cwd` is `/Users/me/.branch-deck/pool/app-c821e1d7/4/app`, a worktree of `/Users/me/src/app`
- **THEN** the session's folder is `~/.branch-deck/pool/app-c821e1d7/4/app` and its project is `~/src/app`

### Requirement: Project filter
`--project` SHALL keep only the logs whose project or folder (see Session id and project), written with `~` for the home directory, contains the given text. A value that starts with the home directory is read with `~` in its place, and a trailing `/` is dropped, so a path the shell expanded matches as typed.

#### Scenario: Part of the path
- **WHEN** `--project billing` is given
- **THEN** the sessions of `~/src/billing` are kept

#### Scenario: Absolute path under the home directory
- **WHEN** `--project /Users/me/src/app` is given, as a shell expands an unquoted `~/src/app`, and the home directory is `/Users/me`
- **THEN** it is read as `~/src/app` and the sessions of `~/src/app` match

#### Scenario: Trailing slash
- **WHEN** `--project ~/src/billing/` is given
- **THEN** it matches the sessions of `~/src/billing`, as `--project ~/src/billing` does

#### Scenario: A repository with worktrees and subfolders
- **WHEN** `--project ~/src/app` is given, and sessions ran in `~/src/app`, in `~/src/app/plugins/web` and in the worktree `~/.branch-deck/pool/app-c821e1d7/4/app`
- **THEN** the sessions of all three are kept

#### Scenario: Text of the folder
- **WHEN** `--project branch-deck` is given
- **THEN** the sessions whose folder is `~/.branch-deck/pool/app-c821e1d7/4/app` are kept, though their project is `~/src/app`

#### Scenario: Temporary folders
- **WHEN** `--project temporary` is given
- **THEN** the sessions whose project is `temporary folders` are kept

## ADDED Requirements

### Requirement: Project of a folder
A session's project SHALL be placed from its folder by the first of these rules that gives an answer: a group that the log's own source names for the folder (see Codex chat folders); `temporary folders` (see Temporary folders); the repository of the folder (see Repository of a folder); a remembered repository (see Repositories remembered); else the folder itself. No other rule is tried, such as the layout of a path, a folder's name, a branch or a remote.

#### Scenario: A folder of no repository
- **WHEN** a session ran in `~/.blume/harnessrunsqdfpv5`, which is in no repository, no temporary folder and no group
- **THEN** its project is `~/.blume/harnessrunsqdfpv5`

#### Scenario: A temporary folder inside a repository
- **WHEN** a session ran in `/tmp/demo`, and `/tmp/demo` holds a `.git` folder
- **THEN** its project is `temporary folders`, since that rule comes first

### Requirement: Temporary folders
A folder at or below a temporary root SHALL have the project `temporary folders`. The roots are the system's temporary folder as the platform names it to tokprof (on macOS `$TMPDIR`, such as `/var/folders/19/nlk_…/T`), the same folder with its links resolved (`/private/var/folders/19/nlk_…/T`), and on macOS and Linux also `/tmp` and `/private/tmp`. These are the roots whose sessions the rhythm leaves out (see work-rhythm). No other folder is temporary.

#### Scenario: A harness run in the temporary folder
- **WHEN** `$TMPDIR` is `/var/folders/19/nlk_x/T/`, and a session ran in `/private/var/folders/19/nlk_x/T/cmsg-acp-3fa2`
- **THEN** its project is `temporary folders`, and its folder is `/private/var/folders/19/nlk_x/T/cmsg-acp-3fa2`

#### Scenario: A folder under /tmp
- **WHEN** a session ran in `/private/tmp/ttl-check-run`
- **THEN** its project is `temporary folders`

#### Scenario: Another user's temporary folder
- **WHEN** a session ran in `/var/folders/7k/other_y/T/run`, which is not under `$TMPDIR` or its resolved form
- **THEN** it is not a temporary folder, and the next rules place it

#### Scenario: A folder named tmp elsewhere
- **WHEN** a session ran in `~/src/app/tmp`
- **THEN** it is not a temporary folder

### Requirement: Repository of a folder
Walking up from the folder to the nearest existing folder with a `.git` entry, never reading one in the home directory or a folder that holds it, the project SHALL be: that folder, when `.git` is a folder or a file whose git folder has no `commondir`; for a worktree, whose `.git` file's `gitdir:` names a git folder with `commondir`, the folder above the git folder that `commondir` names, or that git folder when it is not named `.git`. Relative paths are read from the file's folder.

#### Scenario: The worktree's root
- **WHEN** `~/.branch-deck/pool/app-c821e1d7/4/app/.git` reads `gitdir: /Users/me/src/app/.git/worktrees/app3`, and that folder's `commondir` reads `../..`
- **THEN** a session in `~/.branch-deck/pool/app-c821e1d7/4/app` has the project `~/src/app`

#### Scenario: A named copy that is a worktree
- **WHEN** `~/src/app-2/.git` reads `gitdir: /Users/me/src/app/.git/worktrees/app-2`
- **THEN** a session in `~/src/app-2` has the project `~/src/app`

#### Scenario: A subfolder of a repository
- **WHEN** a session ran in `~/src/app/plugins/web`, and `~/src/app/.git` is a folder
- **THEN** its project is `~/src/app`, and its folder `~/src/app/plugins/web`

#### Scenario: A subfolder of a worktree
- **WHEN** a session ran in `~/.branch-deck/pool/app-c821e1d7/4/app/plugins/web`
- **THEN** its project is `~/src/app`

#### Scenario: A folder that is gone inside a repository that exists
- **WHEN** a session ran in `~/src/app/out/run-17`, that folder is gone, and `~/src/app` exists
- **THEN** its project is `~/src/app`

#### Scenario: A repository inside a worktree
- **WHEN** a session ran in `~/.branch-deck/pool/app-c821e1d7/5/app/out/demo`, which holds a `.git` folder of its own
- **THEN** its project is `~/.branch-deck/pool/app-c821e1d7/5/app/out/demo`

#### Scenario: A submodule
- **WHEN** a session ran in `~/src/app/lib/sub`, and `~/src/app/lib/.git` is a file whose `gitdir:` names `/Users/me/src/app/.git/modules/lib`, which has no `commondir` file
- **THEN** its project is `~/src/app/lib`

#### Scenario: A bare repository
- **WHEN** a worktree's `commondir` names `/Users/me/src/app.git`, a git folder that is not named `.git`
- **THEN** its sessions have the project `~/src/app.git`

#### Scenario: The home directory in git
- **WHEN** `~/.git` is a folder, and a session ran in `~/Documents/notes`, which is in no other repository
- **THEN** its project is `~/Documents/notes`, not `~`

#### Scenario: Nothing to read
- **WHEN** the `.git` file, its git folder or `commondir` cannot be read
- **THEN** this rule gives no answer

### Requirement: Repositories remembered
tokprof SHALL keep each repository root and each worktree root it finds, with the project it gave, in `repositories-1.json` in its cache folder (see activity-calendar, Day store). The file holds those paths and no other text. A folder that the rule of Repository of a folder cannot place, at or below a remembered root, takes the project of the nearest such root. A file that cannot be read counts as empty, and one that cannot be written stays as it is; neither makes a command fail.

#### Scenario: A worktree deleted later
- **WHEN** a session ran in `~/.air/worktrees/temporary/app-shiny-raven`, tokprof found it to be a worktree of `~/src/app` on 2026-11-12, and the worktree was removed the day after
- **THEN** the session's project is still `~/src/app` on 2026-11-20

#### Scenario: A worktree never seen
- **WHEN** a session ran in `~/.codex/worktrees/0dca/app`, removed before any tokprof run found it
- **THEN** its project is `~/.codex/worktrees/0dca/app`

#### Scenario: A root that git places elsewhere now
- **WHEN** `repositories-1.json` holds `~/w/1/app` with `~/src/app`, and `~/w/1/app` is now a worktree of `~/src/other`
- **THEN** the project is `~/src/other`, and the file holds `~/src/other` for `~/w/1/app` from then on

#### Scenario: A cache folder that cannot be written
- **WHEN** the cache folder is read-only
- **THEN** each command prints what it would print otherwise, with the repositories on disk placed, and none of the commands fails

#### Scenario: Another cache folder
- **WHEN** `$TOKPROF_CACHE_DIR` is `/data/cache`
- **THEN** the roots are kept in `/data/cache/repositories-1.json`

### Requirement: Codex chat folders
A Codex log whose folder is `<home>/Documents/Codex/<YYYY-MM-DD>/<name>`, or a folder below it, where `<YYYY-MM-DD>` is a date, SHALL have the project `codex chats`. Codex Desktop makes such a folder for each chat that has no project; its rollouts were seen with `originator` `Codex Desktop` and `cli_version` 0.147.0-alpha.6.5 to 0.166.0-alpha.5.1. The rule belongs to Codex logs only.

#### Scenario: A chat folder
- **WHEN** a Codex rollout's `cwd` is `/Users/me/Documents/Codex/2026-11-02/cltk`
- **THEN** its project is `codex chats`, and its folder `~/Documents/Codex/2026-11-02/cltk`

#### Scenario: Another agent in the same folder
- **WHEN** a Claude Code transcript's first `cwd` is `/Users/me/Documents/Codex/2026-11-02/cltk`
- **THEN** the rule does not apply, and the next rules place it

#### Scenario: Not a date
- **WHEN** a Codex rollout's `cwd` is `/Users/me/Documents/Codex/notes`
- **THEN** the rule does not apply
