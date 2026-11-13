//! The project a session's folder belongs to (session-logs, Project of a folder).
//!
//! The first of these rules that gives an answer places a folder: a group that the log's own
//! source names, the temporary folders, the repository that git's files on disk give, a
//! repository remembered from an earlier run, and else the folder itself. No rule reads the
//! layout of a path, a folder's name, a branch or a remote, and none runs git. The caller gives
//! the home directory, the temporary roots and the file of remembered repositories; this module
//! reads no environment.

use std::collections::{BTreeMap, HashMap};
use std::io::ErrorKind;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};

use serde::{Deserialize, Serialize};

use crate::source::LogSource;

/// The project of every folder at or below a temporary root.
pub const TEMPORARY: &str = "temporary folders";
/// The file of remembered repositories, in the cache folder next to the day store.
pub const REPOSITORIES_FILE: &str = "repositories-1.json";
/// The format of the file of remembered repositories, in its name and its header.
const REPOSITORIES_FORMAT: u32 = 1;

/// How a folder's project was placed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Placed {
    /// A group that the log's source names for its agent's folders.
    Group,
    /// The folder is at or below a temporary root.
    Temporary,
    /// The folder is a repository's root or below it. A submodule is a repository of its own.
    Repository,
    /// The folder is a linked worktree's root or below it, and the project is its repository.
    Worktree,
    /// The disk could not place the folder, and a root remembered from an earlier run did: a
    /// worktree's root when `worktree` is set, else a repository's.
    Remembered { worktree: bool },
    /// Nothing placed the folder, so it is its own project.
    Folder,
    /// Nothing placed the folder, so it is its own project, and it is gone from disk.
    NotFound,
}

/// The project of a folder, and how it was placed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Placement {
    /// A group's name, [`TEMPORARY`], or a path written with `~` for the home directory.
    pub project: String,
    pub placed: Placed,
}

impl Placement {
    /// The folder is in a linked worktree, as git's files say now or as was remembered.
    pub fn in_worktree(&self) -> bool {
        self.placed.in_worktree()
    }
}

impl Placed {
    /// The folder is in a linked worktree, as git's files say now or as was remembered.
    pub fn in_worktree(self) -> bool {
        matches!(self, Placed::Worktree | Placed::Remembered { worktree: true })
    }
}

/// What the X-ray's folder line says about the folder a session ran in (session-xray, Folder
/// line): its label and its text. `worktree` and the folder for a folder in a linked worktree,
/// else `folder` and the folder when it differs from the project, and `folder` and `not found`
/// for a folder that is its own project and is gone from disk. `None` when there is no such
/// line, also for a session whose project was not placed.
pub fn folder_line<'a>(project: &str, folder: &'a str, placed: Option<Placed>) -> Option<(&'static str, &'a str)> {
    let placed = placed?;
    if placed == Placed::NotFound {
        return Some(("folder", "not found"));
    }
    if folder.is_empty() || folder == project {
        return None;
    }
    Some((if placed.in_worktree() { "worktree" } else { "folder" }, folder))
}

/// The repository of a folder, as git's files on disk give it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Repository {
    /// The folder that holds the `.git` entry: a repository's root or a worktree's root.
    pub root: PathBuf,
    /// The repository: the root itself, or for a linked worktree its main working tree, or its
    /// git folder when that is a bare repository.
    pub project: PathBuf,
    /// The root is a linked worktree.
    pub worktree: bool,
}

/// The repository of a folder (session-logs, Repository of a folder): from the folder up to the
/// nearest existing folder with a `.git` entry, never reading one in the home directory or in a
/// folder that holds it. A `.git` folder makes its folder the root, and so does a `.git` file
/// whose git folder has no `commondir`, as a submodule's. A `.git` file whose git folder has a
/// `commondir` is a linked worktree's. A folder that is gone is placed from the nearest folder
/// above it that exists. `None` when no `.git` entry is found, or the nearest one, its git folder
/// or `commondir` cannot be read.
pub fn repository(folder: &Path, home: &Path) -> Option<Repository> {
    if !folder.has_root() {
        return None;
    }
    for dir in folder.ancestors() {
        if !home.as_os_str().is_empty() && home.starts_with(dir) {
            return None;
        }
        let dot = dir.join(".git");
        let meta = match std::fs::metadata(&dot) {
            Ok(meta) => meta,
            Err(e) if matches!(e.kind(), ErrorKind::NotFound | ErrorKind::NotADirectory) => continue,
            Err(_) => return None,
        };
        if meta.is_dir() {
            return Some(Repository { root: dir.to_path_buf(), project: dir.to_path_buf(), worktree: false });
        }
        return linked(dir, &dot);
    }
    None
}

/// The repository of a root whose `.git` is a file: `gitdir: <git folder>`, and in that folder
/// `commondir` names the repository's git folder for a linked worktree. A relative path counts
/// from the folder of the file that holds it.
fn linked(root: &Path, dot: &Path) -> Option<Repository> {
    let text = read_git_file(dot)?;
    let gitdir = text.lines().find_map(|l| l.strip_prefix("gitdir:")).map(str::trim).filter(|g| !g.is_empty())?;
    let gitdir = normal(&root.join(gitdir));
    if !std::fs::metadata(&gitdir).ok()?.is_dir() {
        return None;
    }
    let common = match std::fs::read_to_string(gitdir.join("commondir")) {
        Ok(text) => text,
        Err(e) if e.kind() == ErrorKind::NotFound => {
            return Some(Repository { root: root.to_path_buf(), project: root.to_path_buf(), worktree: false });
        }
        Err(_) => return None,
    };
    let common = common.lines().next().map(str::trim).filter(|c| !c.is_empty())?;
    let common = normal(&gitdir.join(common));
    let project = if common.file_name().is_some_and(|n| n == ".git") { common.parent()?.to_path_buf() } else { common };
    Some(Repository { root: root.to_path_buf(), project, worktree: true })
}

fn read_git_file(path: &Path) -> Option<String> {
    #[cfg(test)]
    tests::count_read(path);
    std::fs::read_to_string(path).ok()
}

/// Whether a folder is at or below one of the temporary roots (session-logs, Temporary
/// folders). The caller gives the roots: the system's temporary folder as it is written and as
/// it resolves, and on Unix `/tmp` and `/private/tmp`.
pub fn temporary(folder: &Path, roots: &[String]) -> bool {
    roots.iter().filter(|r| !r.is_empty()).any(|r| folder.starts_with(normal(Path::new(r))))
}

/// The path with `.` and `..` taken out by its text alone, so that no link is resolved and the
/// path stays as the log and git's files wrote it.
fn normal(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in path.components() {
        match c {
            Component::CurDir => {}
            Component::ParentDir => match out.components().next_back() {
                Some(Component::Normal(_)) => {
                    out.pop();
                }
                Some(Component::RootDir | Component::Prefix(_)) => {}
                _ => out.push(c),
            },
            _ => out.push(c),
        }
    }
    out
}

/// A folder written with `~` for the home directory, as the sources write it, in full.
fn expand(folder: &str, home: &Path) -> PathBuf {
    match folder.strip_prefix('~') {
        Some(rest) if rest.is_empty() || rest.starts_with(['/', '\\']) => {
            let mut full = home.as_os_str().to_os_string();
            full.push(rest);
            normal(Path::new(&full))
        }
        _ => normal(Path::new(folder)),
    }
}

/// A path written with `~` for the home directory.
fn tilde(path: &Path, home: &Path) -> String {
    match path.strip_prefix(home) {
        Ok(rest) if !home.as_os_str().is_empty() => {
            if rest.as_os_str().is_empty() {
                "~".to_string()
            } else {
                format!("~{}{}", std::path::MAIN_SEPARATOR, rest.display())
            }
        }
        _ => path.to_string_lossy().to_string(),
    }
}

/// The repository roots and worktree roots found on disk, each with the project it gave, kept in
/// a file so that a folder keeps its project after it is gone (session-logs, Repositories
/// remembered). The file is read once, when a root is first needed. A file that cannot be read
/// or parsed counts as empty, and one that cannot be written stays as it is.
pub struct Remembered {
    path: PathBuf,
    roots: OnceLock<Mutex<BTreeMap<String, String>>>,
}

/// The file as it is written: the format and the absolute paths of each root and its project.
#[derive(Default, Serialize, Deserialize)]
struct RootsFile {
    format: u32,
    roots: BTreeMap<String, String>,
}

impl Remembered {
    /// The roots kept in the file at `path`, such as `<cache folder>/repositories-1.json`.
    pub fn at(path: PathBuf) -> Remembered {
        Remembered { path, roots: OnceLock::new() }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    fn roots(&self) -> MutexGuard<'_, BTreeMap<String, String>> {
        self.roots.get_or_init(|| Mutex::new(read_roots(&self.path))).lock().unwrap_or_else(|e| e.into_inner())
    }

    /// The nearest remembered root at or above the folder, and its project.
    pub fn nearest(&self, folder: &Path) -> Option<(PathBuf, PathBuf)> {
        let roots = self.roots();
        folder.ancestors().find_map(|dir| roots.get(dir.to_str()?).map(|project| (dir.to_path_buf(), PathBuf::from(project))))
    }

    /// Keeps a root that the disk placed, with its project. A pair that is new or changed is
    /// written at once: the file is read again, the pair set in it, and a new file renamed over
    /// it. Two processes that write at once may lose one pair; it is learned again while its
    /// folder exists.
    pub fn learn(&self, root: &Path, project: &Path) {
        let (Some(root), Some(project)) = (root.to_str(), project.to_str()) else { return };
        let mut roots = self.roots();
        if roots.get(root).is_some_and(|p| p == project) {
            return;
        }
        roots.insert(root.to_string(), project.to_string());
        let mut on_disk = read_roots(&self.path);
        on_disk.insert(root.to_string(), project.to_string());
        // A cache folder that cannot be written keeps what it holds; the roots stay known
        // to this process.
        let _ = write_roots(&self.path, on_disk);
    }
}

fn read_roots(path: &Path) -> BTreeMap<String, String> {
    let Ok(bytes) = std::fs::read(path) else { return BTreeMap::new() };
    match serde_json::from_slice::<RootsFile>(&bytes) {
        Ok(f) if f.format == REPOSITORIES_FORMAT => f.roots,
        _ => BTreeMap::new(),
    }
}

/// Writes the roots to a new file next to `path` and renames it over the old one, so that a
/// reader sees one whole file or the other.
fn write_roots(path: &Path, roots: BTreeMap<String, String>) -> std::io::Result<()> {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let dir = path.parent().filter(|d| !d.as_os_str().is_empty()).unwrap_or(Path::new("."));
    std::fs::create_dir_all(dir)?;
    let name = path.file_name().map_or_else(|| REPOSITORIES_FILE.into(), |n| n.to_string_lossy());
    let tmp = dir.join(format!("{name}.{}.{}.tmp", std::process::id(), COUNTER.fetch_add(1, Ordering::Relaxed)));
    let bytes = serde_json::to_vec(&RootsFile { format: REPOSITORIES_FORMAT, roots })?;
    let written = (|| -> std::io::Result<()> {
        use std::io::Write as _;
        let mut f = std::fs::File::create(&tmp)?;
        f.write_all(&bytes)?;
        f.sync_all()
    })();
    let renamed = written.and_then(|()| std::fs::rename(&tmp, path));
    if renamed.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    renamed
}

/// What the rules place a folder with: the home directory, the temporary roots, and the
/// remembered repositories. Rules without temporary roots or remembered repositories place
/// folders from disk only.
pub struct Rules {
    pub home: PathBuf,
    pub temp_roots: Vec<String>,
    pub remembered: Option<Remembered>,
}

impl Rules {
    /// Rules that place folders from disk only.
    pub fn from_disk(home: PathBuf) -> Rules {
        Rules { home, temp_roots: Vec::new(), remembered: None }
    }

    /// The project of a folder written with `~` for the home directory (session-logs, Project of
    /// a folder), by the first rule that gives one: the group that `group` names for the folder
    /// in full, the temporary folders, the repository on disk, a remembered repository, and else
    /// the folder itself. A root the disk places is remembered. A folder that is not a full path,
    /// such as an empty one, is only asked for its group.
    pub fn place(&self, folder: &str, group: impl FnOnce(&Path) -> Option<&'static str>) -> Placement {
        let path = expand(folder, &self.home);
        if let Some(group) = group(&path) {
            return Placement { project: group.to_string(), placed: Placed::Group };
        }
        let itself = |placed| Placement { project: folder.to_string(), placed };
        if !path.has_root() {
            return itself(Placed::Folder);
        }
        if temporary(&path, &self.temp_roots) {
            return Placement { project: TEMPORARY.to_string(), placed: Placed::Temporary };
        }
        if let Some(repo) = repository(&path, &self.home) {
            if let Some(remembered) = &self.remembered {
                remembered.learn(&repo.root, &repo.project);
            }
            let placed = if repo.worktree { Placed::Worktree } else { Placed::Repository };
            return Placement { project: tilde(&repo.project, &self.home), placed };
        }
        if let Some((root, project)) = self.remembered.as_ref().and_then(|r| r.nearest(&path)) {
            return Placement { project: tilde(&project, &self.home), placed: Placed::Remembered { worktree: root != project } };
        }
        itself(if path.exists() { Placed::Folder } else { Placed::NotFound })
    }
}

/// The placement of each pair of source name and folder asked for so far, which the first
/// thread to ask fills in.
type Placements = HashMap<(String, String), Arc<OnceLock<Placement>>>;

/// The rules, applied to each pair of source and folder once per process.
pub struct Placer {
    rules: Rules,
    placed: Mutex<Placements>,
}

impl Placer {
    pub fn new(rules: Rules) -> Placer {
        Placer { rules, placed: Mutex::new(HashMap::new()) }
    }

    pub fn rules(&self) -> &Rules {
        &self.rules
    }

    /// The project of a folder of a log of `source`, which names the folder's group; a folder
    /// of no source has none. Threads that ask for one pair at once wait for one answer.
    pub fn place(&self, source: Option<&dyn LogSource>, folder: &str) -> Placement {
        let key = (source.map_or("", |s| s.name()).to_string(), folder.to_string());
        let cell = {
            let mut placed = self.placed.lock().unwrap_or_else(|e| e.into_inner());
            placed.entry(key).or_default().clone()
        };
        cell.get_or_init(|| self.rules.place(folder, |path| source.and_then(|s| s.folder_group(path)))).clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The `.git` files read so far, by every test of this process.
    static GIT_FILE_READS: Mutex<Vec<PathBuf>> = Mutex::new(Vec::new());

    pub(super) fn count_read(path: &Path) {
        GIT_FILE_READS.lock().unwrap_or_else(|e| e.into_inner()).push(path.to_path_buf());
    }

    fn reads_of(path: &Path) -> usize {
        GIT_FILE_READS.lock().unwrap_or_else(|e| e.into_inner()).iter().filter(|p| *p == path).count()
    }

    /// A new empty folder for one test, with a home directory in it.
    fn scratch(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("tokprof-place-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("home")).unwrap();
        d
    }

    fn mkdir(path: &Path) -> &Path {
        std::fs::create_dir_all(path).unwrap();
        path
    }

    fn write(path: &Path, text: &str) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }

    /// `gitdir: <path>`, with `/` as git writes it, also on Windows.
    fn gitdir(path: &Path) -> String {
        format!("gitdir: {}\n", path.to_string_lossy().replace('\\', "/"))
    }

    /// A repository at `<home>/src/app` and a linked worktree of it at `<home>/pool/4/app`, as
    /// `git worktree add` lays them out.
    fn repo_with_worktree(home: &Path) -> (PathBuf, PathBuf) {
        let repo = home.join("src").join("app");
        mkdir(&repo.join(".git").join("objects"));
        let worktree = home.join("pool").join("4").join("app");
        let git = repo.join(".git").join("worktrees").join("app3");
        write(&git.join("commondir"), "../..\n");
        write(&worktree.join(".git"), &gitdir(&git));
        (repo, worktree)
    }

    fn repo(root: &Path, project: &Path, worktree: bool) -> Option<Repository> {
        Some(Repository { root: root.to_path_buf(), project: project.to_path_buf(), worktree })
    }

    #[test]
    fn a_worktree_belongs_to_its_main_working_tree() {
        let d = scratch("worktree");
        let home = d.join("home");
        let (app, worktree) = repo_with_worktree(&home);
        assert_eq!(repository(&worktree, &home), repo(&worktree, &app, true), "the worktree's root");
        let web = mkdir(&worktree.join("plugins").join("web")).to_path_buf();
        assert_eq!(repository(&web, &home), repo(&worktree, &app, true), "a subfolder of the worktree");
        // A named copy that git says is a worktree.
        let copy = home.join("src").join("app-2");
        let git = app.join(".git").join("worktrees").join("app-2");
        write(&git.join("commondir"), "../..\n");
        write(&copy.join(".git"), &gitdir(&git));
        assert_eq!(repository(&copy, &home), repo(&copy, &app, true));
    }

    #[test]
    fn a_subfolder_belongs_to_the_root_of_its_repository() {
        let d = scratch("subfolder");
        let home = d.join("home");
        let (app, _) = repo_with_worktree(&home);
        assert_eq!(repository(&app, &home), repo(&app, &app, false), "the root itself");
        let web = mkdir(&app.join("plugins").join("web")).to_path_buf();
        assert_eq!(repository(&web, &home), repo(&app, &app, false));
        let gone = app.join("out").join("run-17");
        assert!(!gone.exists());
        assert_eq!(repository(&gone, &home), repo(&app, &app, false), "a gone subfolder of a repository that exists");
    }

    #[test]
    fn a_repository_inside_a_worktree_is_its_own() {
        let d = scratch("nested");
        let home = d.join("home");
        let (_, worktree) = repo_with_worktree(&home);
        let demo = worktree.join("out").join("demo");
        mkdir(&demo.join(".git"));
        assert_eq!(repository(&demo, &home), repo(&demo, &demo, false));
    }

    #[test]
    fn a_submodule_is_its_own_root() {
        let d = scratch("submodule");
        let home = d.join("home");
        let (app, _) = repo_with_worktree(&home);
        let lib = app.join("lib");
        mkdir(&app.join(".git").join("modules").join("lib"));
        // A relative `gitdir:`, as git writes it for a submodule.
        write(&lib.join(".git"), "gitdir: ../.git/modules/lib\n");
        let sub = mkdir(&lib.join("sub")).to_path_buf();
        assert_eq!(repository(&sub, &home), repo(&lib, &lib, false));
    }

    #[test]
    fn a_worktree_of_a_bare_repository_belongs_to_its_git_folder() {
        let d = scratch("bare");
        let home = d.join("home");
        let bare = home.join("src").join("app.git");
        let git = bare.join("worktrees").join("x");
        write(&git.join("commondir"), "../..\n");
        let worktree = home.join("w").join("x");
        write(&worktree.join(".git"), &gitdir(&git));
        assert_eq!(repository(&worktree, &home), repo(&worktree, &bare, true));
    }

    #[test]
    fn a_relative_gitdir_counts_from_the_folder_of_its_file() {
        let d = scratch("relative");
        let home = d.join("home");
        let (app, _) = repo_with_worktree(&home);
        let git = app.join(".git").join("worktrees").join("rel");
        // An absolute `commondir`, and a relative `gitdir:` as git 2.48 can write it.
        write(&git.join("commondir"), &format!("{}\n", app.join(".git").display()));
        let worktree = home.join("pool").join("5").join("app");
        write(&worktree.join(".git"), "gitdir: ../../../src/app/.git/worktrees/rel\n");
        assert_eq!(repository(&worktree, &home), repo(&worktree, &app, true));
    }

    #[test]
    fn a_git_file_that_cannot_be_read_gives_no_answer() {
        let d = scratch("unreadable");
        let home = d.join("home");
        let (app, _) = repo_with_worktree(&home);
        let inner = app.join("inner");
        // No `gitdir:` line: the walk stops at the nearest `.git`, and does not go on to `app`.
        write(&inner.join(".git"), "not a git file\n");
        assert_eq!(repository(&inner, &home), None);
        write(&inner.join(".git"), &gitdir(&app.join(".git").join("worktrees").join("gone")));
        assert_eq!(repository(&inner, &home), None, "a git folder that is gone");
        let git = app.join(".git").join("worktrees").join("odd");
        mkdir(&git.join("commondir"));
        write(&inner.join(".git"), &gitdir(&git));
        assert_eq!(repository(&inner, &home), None, "a commondir that is not a file");
    }

    #[cfg(unix)]
    #[test]
    fn a_git_file_shut_to_the_user_gives_no_answer() {
        use std::os::unix::fs::PermissionsExt;
        let d = scratch("shut");
        let home = d.join("home");
        let (_, worktree) = repo_with_worktree(&home);
        let dot = worktree.join(".git");
        std::fs::set_permissions(&dot, std::fs::Permissions::from_mode(0o000)).unwrap();
        // Root reads anything; the rule only shows for a user the file shuts out.
        if std::fs::read(&dot).is_err() {
            assert_eq!(repository(&worktree, &home), None);
        }
        std::fs::set_permissions(&dot, std::fs::Permissions::from_mode(0o644)).unwrap();
    }

    #[test]
    fn a_home_kept_in_git_takes_no_folder() {
        let d = scratch("home-git");
        let home = d.join("home");
        mkdir(&home.join(".git"));
        let notes = mkdir(&home.join("Documents").join("notes")).to_path_buf();
        assert_eq!(repository(&notes, &home), None);
        assert_eq!(repository(&home, &home), None, "nor the home directory itself");
        // A `.git` above the home directory is not read either.
        mkdir(&d.join(".git"));
        let _ = std::fs::remove_dir_all(home.join(".git"));
        assert_eq!(repository(&notes, &home), None);
    }

    fn roots() -> Vec<String> {
        ["/var/folders/19/nlk_x/T/", "/private/var/folders/19/nlk_x/T", "/tmp", "/private/tmp"].map(String::from).to_vec()
    }

    #[test]
    fn a_folder_under_a_temporary_root_is_temporary() {
        let roots = roots();
        for folder in ["/var/folders/19/nlk_x/T/run", "/private/var/folders/19/nlk_x/T/cmsg-acp-3fa2", "/tmp/demo", "/private/tmp/ttl-check-run/x"] {
            assert!(temporary(Path::new(folder), &roots), "{folder}");
        }
        assert!(temporary(Path::new("/tmp"), &roots) && temporary(Path::new("/var/folders/19/nlk_x/T"), &roots), "a root itself");
        assert!(!temporary(Path::new("/tmpfoo"), &roots), "a sibling of a root");
        assert!(!temporary(Path::new("/var/folders/7k/other_y/T/run"), &roots), "another user's temporary folder");
        assert!(!temporary(Path::new("/Users/me/src/app/tmp"), &roots), "a folder named tmp elsewhere");
        assert!(!temporary(Path::new("/Users/me"), &[String::new()]), "an empty root holds nothing");
    }

    #[test]
    fn the_first_rule_that_answers_places_the_folder() {
        let d = scratch("order");
        let home = d.join("home");
        let (_, worktree) = repo_with_worktree(&home);
        let temp = mkdir(&d.join("tmp")).to_path_buf();
        let rules = Rules { home: home.clone(), temp_roots: vec![temp.to_string_lossy().to_string()], remembered: None };
        let none = |_: &Path| None;
        let sep = std::path::MAIN_SEPARATOR;
        let app = format!("~{sep}src{sep}app");

        // A temporary folder that holds `.git`.
        let demo = mkdir(&temp.join("demo")).to_path_buf();
        mkdir(&demo.join(".git"));
        let demo = demo.to_string_lossy().to_string();
        assert_eq!(rules.place(&demo, none), Placement { project: TEMPORARY.into(), placed: Placed::Temporary });

        // A group wins over a repository.
        let lease = "~/pool/4/app";
        let chats = |path: &Path| (path == worktree).then_some("chats");
        assert_eq!(rules.place(lease, chats), Placement { project: "chats".into(), placed: Placed::Group });
        assert_eq!(rules.place(lease, none), Placement { project: app.clone(), placed: Placed::Worktree });
        assert!(rules.place(lease, none).in_worktree());
        assert_eq!(rules.place("~/src/app/plugins", none), Placement { project: app, placed: Placed::Repository });

        // A folder that nothing places is its own project, and one that is gone is marked.
        mkdir(&home.join("loose"));
        assert_eq!(rules.place("~/loose", none), Placement { project: "~/loose".into(), placed: Placed::Folder });
        assert_eq!(rules.place("~/gone/run", none), Placement { project: "~/gone/run".into(), placed: Placed::NotFound });
        assert_eq!(rules.place("", none), Placement { project: String::new(), placed: Placed::Folder }, "no folder");
    }

    fn remembering(d: &Path) -> Rules {
        Rules { home: d.join("home"), temp_roots: Vec::new(), remembered: Some(Remembered::at(d.join("cache").join(REPOSITORIES_FILE))) }
    }

    #[test]
    fn a_worktree_removed_after_it_was_learned_still_places_its_folders() {
        let d = scratch("removed");
        let (_, worktree) = repo_with_worktree(&d.join("home"));
        mkdir(&worktree.join("plugins").join("web"));
        let sep = std::path::MAIN_SEPARATOR;
        let app = format!("~{sep}src{sep}app");
        let none = |_: &Path| None;
        assert_eq!(remembering(&d).place("~/pool/4/app/plugins/web", none).placed, Placed::Worktree);
        std::fs::remove_dir_all(&worktree).unwrap();
        // A later run reads the file again.
        let later = remembering(&d);
        assert_eq!(later.place("~/pool/4/app/plugins/web", none), Placement { project: app.clone(), placed: Placed::Remembered { worktree: true } });
        assert_eq!(later.place("~/pool/4/app", none), Placement { project: app, placed: Placed::Remembered { worktree: true } });
        assert!(later.place("~/pool/4/app", none).in_worktree());
        assert_eq!(later.place("~/pool/5/app", none).placed, Placed::NotFound, "a worktree never seen");
    }

    #[test]
    fn gits_answer_replaces_a_remembered_pair() {
        let d = scratch("replaced");
        let home = d.join("home");
        let (app, worktree) = repo_with_worktree(&home);
        let file = d.join("cache").join(REPOSITORIES_FILE);
        let other = home.join("src").join("other");
        let held = RootsFile { format: REPOSITORIES_FORMAT, roots: [(worktree.to_string_lossy().to_string(), other.to_string_lossy().to_string())].into() };
        write(&file, &serde_json::to_string(&held).unwrap());
        let rules = remembering(&d);
        assert_eq!(rules.remembered.as_ref().unwrap().nearest(&worktree), Some((worktree.clone(), other)));
        assert_eq!(rules.place("~/pool/4/app", |_| None).placed, Placed::Worktree);
        assert_eq!(read_roots(&file).get(worktree.to_str().unwrap()).map(PathBuf::from), Some(app.clone()), "the file holds git's answer");
        assert_eq!(rules.remembered.as_ref().unwrap().nearest(&worktree), Some((worktree, app)));
    }

    #[test]
    fn a_file_that_cannot_be_parsed_counts_as_empty() {
        let d = scratch("unparsable");
        let file = d.join("cache").join(REPOSITORIES_FILE);
        write(&file, "{not json");
        let (app, _) = repo_with_worktree(&d.join("home"));
        let rules = remembering(&d);
        assert_eq!(rules.remembered.as_ref().unwrap().nearest(&app), None);
        assert_eq!(rules.place("~/src/app", |_| None).placed, Placed::Repository);
        assert_eq!(read_roots(&file).len(), 1, "written anew with the root learned");
        // A file of another format counts as empty too.
        write(&file, &format!("{{\"format\":2,\"roots\":{{{:?}:{:?}}}}}", app.to_string_lossy(), app.to_string_lossy()));
        assert!(read_roots(&file).is_empty());
    }

    #[test]
    fn a_cache_folder_that_cannot_be_written_gives_no_error() {
        let d = scratch("unwritable");
        let (_, worktree) = repo_with_worktree(&d.join("home"));
        // The cache folder is a file, so nothing can be written under it.
        write(&d.join("cache"), "a file");
        let rules = remembering(&d);
        assert_eq!(rules.place("~/pool/4/app", |_| None).placed, Placed::Worktree);
        assert_eq!(rules.remembered.as_ref().unwrap().nearest(&worktree).map(|(root, _)| root), Some(worktree), "known to this process");
        assert_eq!(std::fs::read_to_string(d.join("cache")).unwrap(), "a file");
    }

    #[cfg(unix)]
    #[test]
    fn a_read_only_cache_folder_gives_no_error() {
        use std::os::unix::fs::PermissionsExt;
        let d = scratch("read-only");
        repo_with_worktree(&d.join("home"));
        let cache = mkdir(&d.join("cache")).to_path_buf();
        std::fs::set_permissions(&cache, std::fs::Permissions::from_mode(0o555)).unwrap();
        // Root writes anything; the rule only shows for a user the folder shuts out.
        let shut = std::fs::write(cache.join("probe"), "").is_err();
        let rules = remembering(&d);
        assert_eq!(rules.place("~/pool/4/app", |_| None).placed, Placed::Worktree);
        let left = std::fs::read_dir(&cache).unwrap().count();
        std::fs::set_permissions(&cache, std::fs::Permissions::from_mode(0o755)).unwrap();
        if shut {
            assert_eq!(left, 0, "nothing written, not even a temporary file");
        }
    }

    #[test]
    fn the_file_holds_only_paths_and_its_format() {
        let d = scratch("contents");
        let home = d.join("home");
        let (app, worktree) = repo_with_worktree(&home);
        let rules = remembering(&d);
        for folder in ["~/pool/4/app", "~/src/app/plugins", "~/src/app"] {
            rules.place(folder, |_| None);
        }
        let file = d.join("cache").join(REPOSITORIES_FILE);
        let value: serde_json::Value = serde_json::from_slice(&std::fs::read(&file).unwrap()).unwrap();
        let object = value.as_object().unwrap();
        assert_eq!(object.keys().collect::<Vec<_>>(), ["format", "roots"]);
        assert_eq!(object["format"], 1);
        let roots: Vec<(PathBuf, PathBuf)> =
            object["roots"].as_object().unwrap().iter().map(|(root, project)| (PathBuf::from(root), PathBuf::from(project.as_str().unwrap()))).collect();
        let mut want = vec![(app.clone(), app.clone()), (worktree, app)];
        want.sort();
        assert_eq!(roots, want);
        let left: Vec<_> = std::fs::read_dir(d.join("cache")).unwrap().flatten().map(|e| e.file_name()).collect();
        assert_eq!(left.len(), 1, "no temporary file stays: {left:?}");
    }

    #[test]
    fn the_folder_line_names_a_worktree_a_folder_or_a_folder_not_found() {
        let line = |project, folder, placed| folder_line(project, folder, Some(placed));
        assert_eq!(line("~/src/app", "~/pool/4/app", Placed::Worktree), Some(("worktree", "~/pool/4/app")));
        assert_eq!(line("~/src/app", "~/pool/4/app", Placed::Remembered { worktree: true }), Some(("worktree", "~/pool/4/app")));
        assert_eq!(line("~/src/app", "~/src/app/plugins/web", Placed::Repository), Some(("folder", "~/src/app/plugins/web")));
        assert_eq!(line(TEMPORARY, "/private/tmp/run", Placed::Temporary), Some(("folder", "/private/tmp/run")));
        assert_eq!(line("~/w/0dca/app", "~/w/0dca/app", Placed::NotFound), Some(("folder", "not found")));
        assert_eq!(line("~/src/app", "~/src/app", Placed::Repository), None, "a repository's root");
        assert_eq!(line("~/loose", "~/loose", Placed::Folder), None, "a folder that is its own project");
        assert_eq!(folder_line("~/src/app", "~/pool/4/app", None), None, "not placed");
    }

    #[test]
    fn a_folder_is_placed_once_per_process() {
        let d = scratch("once");
        let (_, worktree) = repo_with_worktree(&d.join("home"));
        let placer = Placer::new(remembering(&d));
        let placements: Vec<Placement> = std::thread::scope(|s| {
            let threads: Vec<_> = (0..8).map(|_| s.spawn(|| placer.place(None, "~/pool/4/app"))).collect();
            threads.into_iter().map(|t| t.join().unwrap()).collect()
        });
        assert!(placements.iter().all(|p| p.placed == Placed::Worktree && *p == placements[0]));
        assert_eq!(reads_of(&worktree.join(".git")), 1);
        placer.place(None, "~/pool/4/app");
        assert_eq!(reads_of(&worktree.join(".git")), 1, "asked again");
    }
}
