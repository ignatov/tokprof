//! The project of a session placed from the folder it ran in (session-logs, Project of a folder),
//! with the program run on a home of its own that holds a repository, a worktree of it, a
//! subfolder, a temporary folder and a Codex chat folder.
//!
//! The home is made under Cargo's own scratch folder of the target directory, not under the
//! system's temporary folder: the program takes the temporary folders as one project, and on
//! Linux `/tmp` is always one. The run's temporary folder is `<home>/tmp`.

mod common;

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const SEP: char = std::path::MAIN_SEPARATOR;

/// The Claude Code fixture session, its id and the folder its lines name.
const CLAUDE_ID: &str = "5e5510f0-1111-4222-8333-944445555666";
const CLAUDE_CWD: &str = "/w/app";
/// The Codex Desktop chat fixture, and the folder its lines name.
const CHAT: &str = "rollout-2026-11-02T09-00-00-01a19a00-3333-7222-8333-944445555888.jsonl";
const CHAT_CWD: &str = "/h/Documents/Codex/2026-11-02/explain-the-error";

/// `~` and the parts of a path under it, with the platform's separator, as the program prints it.
fn tilde(parts: &[&str]) -> String {
    std::iter::once("~").chain(parts.iter().copied()).collect::<Vec<_>>().join(&SEP.to_string())
}

/// A new home with a repository at `~/src/zebrafish`, a worktree of it at
/// `~/lease/4/zebrafish` as `git worktree add` lays it out, and a subfolder
/// `~/src/zebrafish/plugins/okapi`.
fn home(name: &str) -> PathBuf {
    let home = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("tokprof-projects-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&home);
    let repo = home.join("src").join("zebrafish");
    std::fs::create_dir_all(repo.join(".git").join("objects")).unwrap();
    std::fs::create_dir_all(repo.join("plugins").join("okapi")).unwrap();
    let git = repo.join(".git").join("worktrees").join("zebrafish4");
    std::fs::create_dir_all(&git).unwrap();
    std::fs::write(git.join("commondir"), "../..\n").unwrap();
    let worktree = home.join("lease").join("4").join("zebrafish");
    std::fs::create_dir_all(&worktree).unwrap();
    // git writes `/` in the path, also on Windows.
    std::fs::write(worktree.join(".git"), format!("gitdir: {}\n", git.to_string_lossy().replace('\\', "/"))).unwrap();
    std::fs::create_dir_all(home.join("tmp")).unwrap();
    home
}

/// A path as the text inside a JSON string: a Windows path holds `\`, which JSON escapes.
fn json_text(path: &Path) -> String {
    let quoted = serde_json::to_string(&path.to_string_lossy()).unwrap();
    quoted[1..quoted.len() - 1].to_string()
}

/// The Claude Code fixture session, run in `cwd` under the id `id`.
fn claude_session(home: &Path, cwd: &Path, id: &str) {
    let fixture =
        common::root().join("sources").join("claude-code").join("tests").join("fixtures").join("projects").join("-w-app").join(format!("{CLAUDE_ID}.jsonl"));
    let text = std::fs::read_to_string(fixture).unwrap().replace(CLAUDE_CWD, &json_text(cwd)).replace(CLAUDE_ID, id);
    let dir = home.join(".claude").join("projects").join("-scratch");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join(format!("{id}.jsonl")), text).unwrap();
}

/// The Codex Desktop chat fixture, run in `~/Documents/Codex/2026-11-02/narwhal-chat`.
fn codex_chat(home: &Path) {
    let fixtures = common::root().join("sources").join("codex").join("tests").join("fixtures").join("sessions").join("2026").join("11").join("02");
    let cwd = home.join("Documents").join("Codex").join("2026-11-02").join("narwhal-chat");
    let text = std::fs::read_to_string(fixtures.join(CHAT)).unwrap().replace(CHAT_CWD, &json_text(&cwd));
    let dir = home.join(".codex").join("sessions").join("2026").join("11").join("02");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join(CHAT), text).unwrap();
}

/// tokprof on the home, with `<home>/tmp` as the system's temporary folder.
fn tokprof(home: &Path) -> Command {
    let tmp = home.join("tmp");
    let mut c = common::tokprof(home);
    c.env("TMPDIR", &tmp).env("TMP", &tmp).env("TEMP", &tmp);
    c
}

fn run(home: &Path, args: &[&str]) -> String {
    let out: Output = tokprof(home).args(args).output().unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    assert!(out.status.success(), "{args:?}: {stdout}{}", String::from_utf8_lossy(&out.stderr));
    stdout
}

/// The `session` line of the X-ray and the line after it.
fn header(home: &Path, id: &str) -> (String, String) {
    let text = run(home, &["scan", id]);
    let mut lines = text.lines().skip(1);
    (lines.next().unwrap_or_default().to_string(), lines.next().unwrap_or_default().to_string())
}

/// The projects `tokprof sessions` lists with these arguments before it, by short id.
fn listed(home: &Path, args: &[&str]) -> Vec<(String, String)> {
    let args: Vec<&str> = args.iter().copied().chain(["sessions"]).collect();
    let text = run(home, &args);
    let mut rows: Vec<(String, String)> = text
        .lines()
        .skip(1)
        .filter_map(|l| {
            let mut words = l.split_whitespace().skip(3);
            Some((words.next()?.to_string(), words.nth(2)?.to_string()))
        })
        .collect();
    rows.sort();
    rows
}

#[test]
fn a_worktree_and_a_subfolder_are_in_the_repository() {
    let home = home("repository");
    let repo = tilde(&["src", "zebrafish"]);
    claude_session(&home, &home.join("lease").join("4").join("zebrafish"), CLAUDE_ID);
    claude_session(&home, &home.join("src").join("zebrafish").join("plugins").join("okapi"), "6f6621f1-1111-4222-8333-944445555666");
    claude_session(&home, &home.join("src").join("zebrafish"), "7a7732a2-1111-4222-8333-944445555666");
    claude_session(&home, &home.join("tmp").join("quokka-run"), "8b8843b3-1111-4222-8333-944445555666");

    let all = listed(&home, &[]);
    let temporary = "temporary".to_string();
    assert_eq!(
        all,
        [("5e5510f0".into(), repo.clone()), ("6f6621f1".into(), repo.clone()), ("7a7732a2".into(), repo.clone()), ("8b8843b3".into(), temporary)],
        "the project's words, `temporary folders` cut at its space"
    );
    let session = format!("session   claude  {repo}  ");
    let (first, folder) = header(&home, "5e5510f0");
    assert!(first.starts_with(&session), "{first}");
    assert_eq!(folder, format!("worktree  {}", tilde(&["lease", "4", "zebrafish"])));
    let (first, folder) = header(&home, "6f6621f1");
    assert!(first.starts_with(&session), "{first}");
    assert_eq!(folder, format!("folder    {}", tilde(&["src", "zebrafish", "plugins", "okapi"])));
    let (first, folder) = header(&home, "7a7732a2");
    assert!(first.starts_with(&session), "{first}");
    assert!(folder.starts_with("title "), "the repository's root has no folder line: {folder}");
    let (first, folder) = header(&home, "8b8843b3");
    assert!(first.starts_with("session   claude  temporary folders  "), "{first}");
    assert_eq!(folder, format!("folder    {}", tilde(&["tmp", "quokka-run"])));

    // `--project` matches the project or the folder; a path the shell expanded matches as typed.
    let ids = |args: &[&str]| listed(&home, args).into_iter().map(|(id, _)| id).collect::<Vec<_>>();
    let expanded = home.join("src").join("zebrafish");
    assert_eq!(ids(&["--project", expanded.to_str().unwrap()]), ["5e5510f0", "6f6621f1", "7a7732a2"]);
    assert_eq!(ids(&["--project", "lease"]), ["5e5510f0"]);
    assert_eq!(ids(&["--project", "temporary"]), ["8b8843b3"]);
    assert!(ids(&["--project", "~/src/other"]).is_empty());
    std::fs::remove_dir_all(&home).unwrap();
}

#[test]
fn a_worktree_removed_after_a_run_stays_in_its_repository() {
    let home = home("removed");
    let worktree = home.join("lease").join("4").join("zebrafish");
    claude_session(&home, &worktree, CLAUDE_ID);
    let (first, folder) = header(&home, "5e5510f0");
    let repo = tilde(&["src", "zebrafish"]);
    assert!(first.starts_with(&format!("session   claude  {repo}  ")), "{first}");
    let remembered = std::fs::read_to_string(home.join("cache").join("repositories-1.json")).unwrap();
    assert!(remembered.contains(&json_text(&worktree)), "{remembered}");

    // The worktree goes, as a tool removes it; git's record of it is left.
    std::fs::remove_dir_all(&worktree).unwrap();
    assert_eq!(header(&home, "5e5510f0"), (first, folder));
    assert_eq!(listed(&home, &[]), [("5e5510f0".to_string(), repo)]);

    // Without the file of remembered repositories, the folder is its own project, not found.
    std::fs::remove_file(home.join("cache").join("repositories-1.json")).unwrap();
    let (first, folder) = header(&home, "5e5510f0");
    assert!(first.starts_with(&format!("session   claude  {}  ", tilde(&["lease", "4", "zebrafish"]))), "{first}");
    assert_eq!(folder, "folder    not found");
    std::fs::remove_dir_all(&home).unwrap();
}

#[test]
fn the_day_store_names_the_folder_where_it_differs_from_the_project() {
    let home = home("store");
    claude_session(&home, &home.join("lease").join("4").join("zebrafish"), CLAUDE_ID);
    claude_session(&home, &home.join("src").join("zebrafish"), "7a7732a2-1111-4222-8333-944445555666");
    run(&home, &["calendar"]);
    let store: serde_json::Value = serde_json::from_slice(&std::fs::read(home.join("cache").join("days-2.json")).unwrap()).unwrap();
    assert_eq!(store["format"], 2);
    let entry = |id: &str| store["logs"].as_array().unwrap().iter().find(|e| e["session"] == id).cloned().unwrap();
    let worktree = entry(CLAUDE_ID);
    assert_eq!(
        (worktree["project"].as_str(), worktree["folder"].as_str()),
        (Some(&*tilde(&["src", "zebrafish"])), Some(&*tilde(&["lease", "4", "zebrafish"])))
    );
    let root = entry("7a7732a2-1111-4222-8333-944445555666");
    assert_eq!(root["project"].as_str(), Some(&*tilde(&["src", "zebrafish"])));
    assert!(root.get("folder").is_none(), "no folder at the repository's root: {root}");
    std::fs::remove_dir_all(&home).unwrap();
}

#[test]
fn share_and_the_month_card_name_no_project_and_no_folder() {
    let home = home("private");
    claude_session(&home, &home.join("lease").join("4").join("zebrafish"), CLAUDE_ID);
    claude_session(&home, &home.join("tmp").join("quokka-run"), "8b8843b3-1111-4222-8333-944445555666");
    codex_chat(&home);
    // The three sessions are placed as the rules say.
    let projects: Vec<String> = listed(&home, &[]).into_iter().map(|(_, p)| p).collect();
    assert_eq!(projects, ["codex", &tilde(&["src", "zebrafish"]), "temporary"], "the chat, the worktree, the temporary folder: {projects:?}");

    let file = home.join("out").join("share.json");
    let mut shown = run(&home, &["share", "-o", file.to_str().unwrap()]);
    shown += &std::fs::read_to_string(&file).unwrap();
    for month in ["2026-10", "2026-11"] {
        shown += &run(&home, &["month", month]);
    }
    assert!(shown.contains("Nothing was sent anywhere") && shown.contains("October 2026") && shown.contains("November 2026"), "{shown}");
    for word in ["zebrafish", "lease", "quokka", "narwhal", "Documents", "temporary folders", "codex chats"] {
        assert!(!shown.contains(word), "{word:?} in:\n{shown}");
    }
    assert!(!shown.contains(&*home.to_string_lossy()) && !shown.contains(&json_text(&home)), "the home in:\n{shown}");
    std::fs::remove_dir_all(&home).unwrap();
}
