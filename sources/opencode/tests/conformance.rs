//! The source passes the checks every log source must pass, over its fixtures, and reads them as
//! the session-logs spec says. The fixtures are SQL text; the stores are built from it in a
//! temporary data directory.

use std::path::{Path, PathBuf};

use tokprof_core::Pricing;
use tokprof_core::model::{CompactionTrigger, Session};
use tokprof_core::source::{Claim, LogSource, Touched};
use tokprof_source_opencode::OpenCode;

/// A data directory with the fixture stores, and a copy of `opencode.db` as `opencode-next.db`,
/// which 2.x imports and tokprof leaves out.
fn data_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("tokprof-opencode-{name}-{}", std::process::id())).join("opencode");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    for entry in std::fs::read_dir(&fixtures).unwrap().flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        let Some(store) = name.strip_suffix(".sql") else { continue };
        let db = rusqlite::Connection::open(dir.join(store)).unwrap();
        db.execute_batch(&std::fs::read_to_string(entry.path()).unwrap()).unwrap();
    }
    std::fs::copy(dir.join("opencode.db"), dir.join("opencode-next.db")).unwrap();
    dir
}

fn sessions(source: &OpenCode) -> Vec<(String, bool, Session)> {
    let mut out: Vec<(String, bool, Session)> = source
        .discover()
        .iter()
        .map(|log| {
            let mut d = source.decoder(log);
            source.open(log).unwrap().for_each(&mut |r| d.feed(r)).unwrap();
            (log.session_id.clone(), log.subagent, d.finish())
        })
        .collect();
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

#[test]
fn the_fixtures_conform() {
    tokprof_core::testkit::conform(&OpenCode::at(data_dir("conform"), Path::new("/h"))).unwrap();
}

/// Each store's sessions, each request once: the 1.x store's session and its subagent; the
/// session 2.x copied and went on with; and the fork, without its copy.
#[test]
fn the_fixtures_count_each_call_once() {
    let source = OpenCode::at(data_dir("count"), Path::new("/h"));
    let s = sessions(&source);
    let counts: Vec<(&str, &str, bool, &str, usize)> =
        s.iter().map(|(id, sub, s)| (&id[id.len() - 1..], s.id.as_str(), *sub, s.project.as_str(), s.chains[0].requests.len())).collect();
    assert_eq!(
        counts,
        [
            ("a", "ses_f00000000001aaaaaaaaaaaaaa", false, "~/src/app", 4),
            ("b", "ses_f00000000001aaaaaaaaaaaaaa", true, "~/src/app", 2),
            ("c", "ses_f00000000003cccccccccccccc", false, "~/src/api", 5),
            ("d", "ses_f00000000004dddddddddddddd", false, "~/src/api", 1),
        ]
    );
    let marks = |s: &Session| s.chains[0].requests.iter().map(|r| (r.compaction_call, r.compacted_before)).collect::<Vec<_>>();
    assert_eq!(marks(&s[0].2), [(false, false), (false, false), (true, false), (false, true)]);
    assert_eq!(marks(&s[2].2), [(false, false), (true, false), (false, true), (true, false), (false, true)]);
    let kinds = |s: &Session| s.compactions.iter().map(|c| c.kind.clone()).collect::<Vec<_>>();
    assert_eq!((kinds(&s[0].2), kinds(&s[2].2)), (vec![CompactionTrigger::Auto], vec![CompactionTrigger::Manual, CompactionTrigger::Auto]));
    let first = s[0].2.chains[0].requests[0].usage;
    assert_eq!((first.uncached, first.write_short, first.output, first.reasoning), (3000, 2000, 120, 20));
    assert_eq!((s[1].2.chains[0].name.as_str(), s[1].2.task.as_deref()), ("subagent: List files", Some("List files")));
    assert_eq!(s[0].2.title.as_deref(), Some("Add a mul function"));
}

/// OpenCode's own cost is within 1.5% of tokprof's, for every session.
#[test]
fn tokprof_s_dollars_agree_with_opencode_s_own() {
    let pricing = Pricing::defaults();
    for (id, _, s) in sessions(&OpenCode::at(data_dir("dollars"), Path::new("/h"))) {
        let ours: f64 = s.chains[0].requests.iter().map(|r| pricing.get(&r.model, r.usage.prompt(), r.ts_ms).unwrap().cost(&r.usage)).sum();
        let theirs = s.reported_usd.unwrap();
        assert!((ours - theirs).abs() <= 0.015 * theirs, "{id}: tokprof ${ours:.6}, OpenCode ${theirs:.6}");
    }
}

/// The main sessions of a store, the subagents under them, a store as a target, and what a
/// change of a store or its journal touches.
#[test]
fn stores_as_logs() {
    let dir = data_dir("stores");
    let source = OpenCode::at(dir.clone(), Path::new("/h"));
    std::fs::write(dir.join("opencode-broken.db"), "not a store").unwrap();
    let logs = source.discover();
    assert_eq!(logs.len(), 4, "opencode-next.db is left out, and a file that is not a store adds nothing");
    assert_eq!(source.claim(&dir.join("opencode-broken.db")), Claim::No);
    let main = logs.iter().find(|l| l.session_id.ends_with('a')).unwrap();
    let subs = source.subagents(main);
    assert_eq!(subs.iter().map(|s| s.session_id.as_str()).collect::<Vec<_>>(), ["ses_f00000000002bbbbbbbbbbbbbb"]);
    assert_eq!(source.head(main).project, "~/src/app");
    assert!(source.last_active(main).is_some_and(|t| t <= main.modified_ms()));

    let store = dir.join("opencode.db");
    assert_eq!(source.claim(&store), Claim::Yes);
    assert_eq!(source.claim(&dir.join("notes.txt")), Claim::No);
    let latest = source.session_at(&store).unwrap();
    assert_eq!(latest.session_id, "ses_f00000000004dddddddddddddd", "the fork was written last");
    assert_eq!(source.polled(&latest), [store.clone(), dir.join("opencode.db-wal")]);
    // The fixtures were written in 2026-11; none is recent now.
    match source.touched(&dir.join("opencode.db-wal"), false) {
        Touched::Store { db, changed, held } => {
            assert_eq!((db, changed.len(), held.len()), (store.clone(), 0, 2), "the fixtures were written in 2026-11; none lately");
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(source.touched(&dir.join("snapshot/abc"), true), Touched::Other);
    assert_eq!(source.touched(&dir.join("opencode.db-shm"), false), Touched::Other, "reading a store writes its index");
    assert_eq!(source.touched(&dir.join("opencode-broken.db"), false), Touched::Other, "a store that cannot be read tells nothing");

    let beta = dir.join("opencode-beta.db");
    std::fs::remove_file(&beta).unwrap();
    assert_eq!(source.touched(&beta, false), Touched::Store { db: beta.clone(), changed: Vec::new(), held: Vec::new() }, "a store that is gone holds nothing");
    assert_eq!(source.discover().len(), 2);

    let main = OpenCode::at(data_dir("activity"), Path::new("/h")).discover().into_iter().find(|l| l.session_id.ends_with('a')).unwrap();
    let source = OpenCode::at(main.path().parent().unwrap().to_path_buf(), Path::new("/h"));
    let a = source.activity(&main).unwrap();
    assert_eq!(
        (a.cwd.as_deref(), a.prompts.len(), a.last_text.as_deref()),
        (Some("/h/src/app"), 2, Some("Added sub.")),
        "the compaction asked for has no text"
    );
}

/// A main session a store holds that it did not hold before, such as one 2.x imported with its
/// old times, is changed for the ui though it was not written lately.
#[test]
fn a_session_new_to_the_store_is_changed() {
    let dir = data_dir("fresh");
    let source = OpenCode::at(dir.clone(), Path::new("/h"));
    assert_eq!(source.discover().len(), 4);
    let store = dir.join("opencode.db");
    let db = rusqlite::Connection::open(&store).unwrap();
    db.execute_batch(
        "INSERT INTO session_v2 (id, project_id, slug, directory, title, version, time_created, time_updated)
         VALUES ('ses_f00000000005eeeeeeeeeeeeee', 'global', 's', '/h/src/old', 'Imported', '2.2.4', 1000, 2000);",
    )
    .unwrap();
    drop(db);
    match source.touched(&store, false) {
        Touched::Store { changed, held, .. } => {
            assert_eq!(changed.iter().map(|r| r.session_id.as_str()).collect::<Vec<_>>(), ["ses_f00000000005eeeeeeeeeeeeee"]);
            assert_eq!(held.len(), 3);
        }
        other => panic!("{other:?}"),
    }
}

/// A data directory with the stores of `tests/conform`, of the paths `tests/fixtures` lacks, kept
/// apart from it, since other tests read it.
fn paths_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("tokprof-opencode-paths-{name}-{}", std::process::id())).join("opencode");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    for entry in std::fs::read_dir(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/conform")).unwrap().flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        let Some(store) = name.strip_suffix(".sql") else { continue };
        rusqlite::Connection::open(dir.join(store)).unwrap().execute_batch(&std::fs::read_to_string(entry.path()).unwrap()).unwrap();
    }
    dir
}

#[test]
fn the_path_fixtures_conform() {
    tokprof_core::testkit::conform(&OpenCode::at(paths_dir("conform"), Path::new("/h"))).unwrap();
}

/// A 2.x store without the 1.x tables: a router's model, a failed compaction that is a call, a
/// finished one without a call, one not finished, a message of another type, an aborted reply,
/// and subagents two deep, one without a title. A 1.x store without the part table, whose
/// replies count their own tokens.
#[test]
fn the_path_fixtures_hold_each_path() {
    use tokprof_core::api::ApiId;
    let s = sessions(&OpenCode::at(paths_dir("count"), Path::new("/h")));
    let summary: Vec<(&str, bool, &str, usize)> =
        s.iter().map(|(id, sub, s)| (&id[id.len() - 1..], *sub, s.chains[0].name.as_str(), s.chains[0].requests.len())).collect();
    assert_eq!(summary, [("e", false, "main", 3), ("e", true, "subagent:explore", 1), ("e", true, "subagent: Check one file", 1), ("g", false, "main", 3)]);
    let main = &s[0].2;
    let r = &main.chains[0].requests;
    let got: Vec<(&str, ApiId, bool, bool)> = r.iter().map(|r| (r.model.as_str(), r.api, r.compaction_call, r.compacted_before)).collect();
    assert_eq!(
        got,
        [
            ("claude-sonnet-4-5", ApiId::Anthropic, false, false),
            ("claude-sonnet-4-5", ApiId::Anthropic, true, false),
            ("claude-sonnet-4-5", ApiId::Anthropic, false, true)
        ]
    );
    assert_eq!(main.compactions.iter().map(|c| c.kind.clone()).collect::<Vec<_>>(), [CompactionTrigger::Auto], "only a finished compaction is one");
    assert_eq!((s[2].2.id.as_str(), s[2].2.task.as_deref()), (main.id.as_str(), Some("Check one file")), "a subagent's subagent counts under the main session");
    let old = &s[3].2.chains[0].requests;
    assert_eq!(
        old.iter().map(|r| (r.usage.output, r.usage.reasoning, r.compaction_call)).collect::<Vec<_>>(),
        [(75, 15, false), (200, 0, true), (20, 0, false)]
    );
}

/// OpenCode's own cost is within 1.5% of tokprof's for these stores too.
#[test]
fn tokprof_s_dollars_agree_with_opencode_s_own_on_every_path() {
    let pricing = Pricing::defaults();
    for (id, _, s) in sessions(&OpenCode::at(paths_dir("dollars"), Path::new("/h"))) {
        let ours: f64 = s.chains[0].requests.iter().map(|r| pricing.get(&r.model, r.usage.prompt(), r.ts_ms).unwrap().cost(&r.usage)).sum();
        let theirs = s.reported_usd.unwrap();
        assert!((ours - theirs).abs() <= 0.015 * theirs, "{id}: tokprof ${ours:.6}, OpenCode ${theirs:.6}");
    }
}
