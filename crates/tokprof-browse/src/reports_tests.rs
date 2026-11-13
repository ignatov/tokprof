use super::*;
use std::path::{Path, PathBuf};
use tokprof_core::month::Ym;
use tokprof_core::period::{Period, Yq};

const OCT: Period = Period::Month(Ym { year: 2026, month: 10 });
const NOV: Period = Period::Month(Ym { year: 2026, month: 11 });

fn key(period: Period, text: &str) -> Key {
    Key::new(period, text)
}

/// Reports whose worker is the test: it reads the jobs sent and sends what it likes.
fn by_hand() -> (Reports<Key>, Sender<Done<Key>>, Receiver<Job<Key>>) {
    let (tx, notes) = mpsc::channel();
    let (jobs, queue) = mpsc::channel();
    let r = Reports { jobs: Some(jobs), notes, kept: HashMap::new(), pending: None, shared: Arc::default(), select: Query::default() };
    (r, tx, queue)
}

fn done(job: Job<Key>) -> Done<Key> {
    Done { job, overview: Err("empty".into()), window: (0, 1), took: Duration::ZERO }
}

#[test]
fn only_the_report_asked_for_ends_the_wait() {
    let (mut r, tx, _queue) = by_hand();
    let (oct, nov) = (key(OCT, ""), key(NOV, ""));
    r.pending = Some((nov.clone(), 2));
    tx.send(done((oct.clone(), 2))).unwrap();
    assert!(r.tick(&nov, 2), "October arrived");
    assert!(r.reading(&nov) && r.get(&oct).is_some(), "another period leaves November reading");
    tx.send(done((nov.clone(), 1))).unwrap();
    r.tick(&nov, 2);
    assert!(r.reading(&nov) && r.get(&nov).is_some_and(|x| x.changes == 1), "a report from before the last change leaves it reading");
    tx.send(done((nov.clone(), 2))).unwrap();
    r.tick(&nov, 2);
    assert!(!r.reading(&nov) && r.progress().is_none(), "the report asked for ends it");
}

#[test]
fn a_newer_key_replaces_the_one_pending() {
    let (mut r, tx, queue) = by_hand();
    let (oct, zebra) = (key(OCT, ""), key(OCT, "zebrafish"));
    r.tick(&oct, 0);
    assert!(r.reading(&oct));
    // The filter typed and ⏎: the same period with a text is another key, and replaces it.
    r.tick(&zebra, 0);
    assert!(r.reading(&zebra) && !r.reading(&oct), "only the last key is waited for");
    assert_eq!(r.progress().map(|p| p.key), Some(zebra.clone()), "before the worker starts it, the key asked for");
    assert_eq!(queue.try_iter().map(|j| j.0).collect::<Vec<_>>(), [oct.clone(), zebra.clone()], "the worker takes the newest of what waits");
    // The key left arrives first: it is kept, and the newer one still reads.
    tx.send(done((oct.clone(), 0))).unwrap();
    r.tick(&zebra, 0);
    assert!(r.get(&oct).is_some() && r.reading(&zebra));
    // Back to the key left: it shows at once and asks for nothing.
    r.tick(&oct, 0);
    assert!(!r.reading(&oct) && r.reading(&zebra));
    assert_eq!(queue.try_iter().count(), 0);
}

#[test]
fn the_progress_names_the_period() {
    let p = Progress { key: key(Period::Quarter(Yq { year: 2026, q: 4 }), ""), done: 120, total: 2302 };
    assert_eq!(p.text(), "reading the logs of Q4 2026… 120 of 2,302 logs");
}

fn fixture() -> (Arc<Catalog>, Query) {
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../sources/claude-code/tests/fixtures/projects");
    let catalog = Arc::new(Catalog::new(vec![Box::new(tokprof_source_claude_code::ClaudeCode::at(fixtures, Path::new("/w")))], PathBuf::from("/w")));
    let select = Query { agents: catalog.agents(), ..Query::default() };
    (catalog, select)
}

#[test]
fn a_report_is_read_once_kept_and_read_again_only_after_changes_and_its_pause() {
    let (catalog, select) = fixture();
    let woken = Arc::new(AtomicUsize::new(0));
    let counted = woken.clone();
    let mut r: Reports<Key> = Reports::start(
        catalog,
        Pricing::defaults(),
        select,
        Arc::new(move || {
            counted.fetch_add(1, Ordering::SeqCst);
        }),
    );
    // The fixture's session ran on 30 October 2026.
    let oct = key(OCT, "");
    assert!(r.get(&oct).is_none() && r.progress().is_none(), "nothing is read before it is asked for");
    r.settle(&oct, 0, Duration::from_secs(20));
    let report = r.get(&oct).expect("the report arrived");
    let overview = report.overview.as_ref().expect("read");
    assert!(overview.requests() > 0, "October holds the fixture's requests");
    assert_eq!((report.since_ms, report.until_ms), OCT.bounds(&rhythm::local_offset), "a complete month counts all of it");
    assert!(woken.load(Ordering::SeqCst) >= 1, "the worker woke the screen when it sent");
    assert!(r.progress().is_none() && !r.reading(&oct));
    // The same logs: nothing is asked for again, and nothing is due.
    r.tick(&oct, 0);
    assert!(!r.reading(&oct));
    assert_eq!(r.next_due(&oct, 0), None);
    // The logs changed: the report waits for its pause, at least 20 s after it arrived.
    r.tick(&oct, 1);
    assert!(!r.reading(&oct), "it waits for its pause");
    let due = r.next_due(&oct, 1).expect("due after its pause");
    assert!(due >= r.get(&oct).unwrap().arrived + PAUSE);
    // Another period is read on its own, and holds none of October's requests.
    let nov = key(NOV, "");
    r.tick(&nov, 1);
    assert!(r.reading(&nov) && r.progress().is_some(), "the worker counts its logs while it reads");
    r.settle(&nov, 1, Duration::from_secs(20));
    assert_eq!(r.get(&nov).and_then(|x| x.overview.as_ref().ok()).map(|o| o.requests()), Some(0));
}

fn iso(ts: i64) -> String {
    chrono::DateTime::from_timestamp_millis(ts).unwrap().format("%Y-%m-%dT%H:%M:%SZ").to_string()
}

/// A Claude Code log of one request in `cwd` at `ts`.
fn log(id: &str, cwd: &str, ts: i64) -> String {
    format!(
        r#"{{"type":"user","sessionId":"{id}","cwd":"{cwd}","entrypoint":"cli","timestamp":"{}","message":{{"role":"user","content":"go"}}}}
{{"type":"assistant","sessionId":"{id}","cwd":"{cwd}","entrypoint":"cli","requestId":"r1","timestamp":"{}","message":{{"id":"m1","model":"claude-opus-5-5","stop_reason":"end_turn","content":[{{"type":"text","text":"done"}}],"usage":{{"input_tokens":12,"cache_read_input_tokens":20000,"cache_creation_input_tokens":300,"output_tokens":40}}}}}}
"#,
        iso(ts),
        iso(ts + 60_000)
    )
}

#[test]
fn the_filter_text_keeps_the_logs_of_a_project() {
    let dir = std::env::temp_dir().join(format!("tokprof-reports-text-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let ts = rhythm::midnight(Ym { year: 2026, month: 10 }.first_date() + 13, &rhythm::local_offset) + 10 * 3_600_000;
    for (folder, id, cwd) in [
        ("-w-src-zebrafish", "aaaa1111-0000-4000-8000-000000000001", "/w/src/zebrafish"),
        ("-w-src-heron", "bbbb2222-0000-4000-8000-000000000002", "/w/src/heron"),
    ] {
        std::fs::create_dir_all(dir.join(folder)).unwrap();
        std::fs::write(dir.join(folder).join(format!("{id}.jsonl")), log(id, cwd, ts)).unwrap();
    }
    let catalog = Arc::new(Catalog::new(vec![Box::new(tokprof_source_claude_code::ClaudeCode::at(dir.clone(), Path::new("/w")))], PathBuf::from("/w")));
    let select = Query { agents: catalog.agents(), ..Query::default() };
    let mut r: Reports<Key> = Reports::start(catalog, Pricing::defaults(), select, Arc::new(|| {}));
    let projects = |r: &Reports<Key>, k: &Key| {
        let o = r.get(k).and_then(|x| x.overview.as_ref().ok()).expect("read");
        let mut p: Vec<String> = o.spread.projects().into_iter().map(|(name, _)| name.to_string()).collect();
        p.sort();
        (o.requests(), p)
    };
    let (all, zebra, none) = (key(OCT, ""), key(OCT, "ZebraF"), key(OCT, "zzz"));
    for k in [&all, &zebra, &none] {
        r.settle(k, 0, Duration::from_secs(20));
    }
    assert_eq!(projects(&r, &all), (2, vec!["~/src/heron".to_string(), "~/src/zebrafish".to_string()]));
    assert_eq!(projects(&r, &zebra), (1, vec!["~/src/zebrafish".to_string()]), "the text matches the project, ignoring case");
    assert_eq!(projects(&r, &none).0, 0);
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn detached_reports_read_nothing() {
    let mut r: Reports<Key> = Reports::detached();
    let oct = key(OCT, "");
    r.tick(&oct, 0);
    assert!(!r.reading(&oct) && r.get(&oct).is_none() && r.next_due(&oct, 3).is_none());
}

#[test]
fn a_report_that_panicked_shows_the_error_and_the_worker_goes_on() {
    use crate::contain::testing::{self, BOOM};
    let (catalog, armed, _) = testing::catalog("reports");
    let select = Query { agents: catalog.agents(), ..Query::default() };
    let mut r: Reports<Key> = Reports::start(catalog, Pricing::defaults(), select, Arc::new(|| {}));
    // The panic comes from reading the log, whatever its dates: the period of today reads it.
    let now = key(Period::month_of(rhythm::date_of(now_ms(), &rhythm::local_offset)), "");
    r.settle(&now, 0, Duration::from_secs(20));
    let failed = r.get(&now).expect("the report arrived");
    assert_eq!(failed.overview.as_ref().err(), Some(&format!("tokprof panicked: {BOOM}")));
    assert!(!r.reading(&now));
    // The next report, once its pause passed, is read by the same worker.
    armed.store(false, Ordering::SeqCst);
    r.kept.clear();
    r.settle(&now, 1, Duration::from_secs(20));
    assert!(r.get(&now).is_some_and(|x| x.overview.is_ok()));
}

#[test]
fn a_worker_that_is_gone_fails_the_report_instead_of_waiting() {
    let (jobs, rx) = mpsc::channel();
    drop(rx);
    let mut r = Reports { jobs: Some(jobs), ..Reports::detached() };
    let oct = key(OCT, "");
    r.tick(&oct, 0);
    assert!(!r.reading(&oct));
    let failed = r.get(&oct).expect("failed");
    assert_eq!(failed.overview.as_ref().err().map(String::as_str), Some("the worker that reads the reports stopped"));
    assert_eq!((failed.since_ms, failed.until_ms), OCT.bounds(&rhythm::local_offset), "the window of the period");
}
