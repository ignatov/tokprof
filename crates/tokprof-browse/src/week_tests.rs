use super::*;
use std::path::{Path, PathBuf};
use tokprof_core::rhythm::Event;
use tokprof_core::week::GONE_ALL;

const MINUTE_MS: i64 = 60_000;
/// The week of 26 Oct 2026, which has ended whenever the tests run.
const MONDAY: i64 = 20_752;

fn offset() -> Offset<'static> {
    &rhythm::local_offset
}

/// A local time of the week: `day` days after its Monday, at `h:m`.
fn at(day: i64, h: i64, m: i64) -> i64 {
    rhythm::midnight(MONDAY + day, offset()) + h * HOUR_MS + m * MINUTE_MS
}

fn iso(ts: i64) -> String {
    chrono::DateTime::from_timestamp_millis(ts).unwrap().format("%Y-%m-%dT%H:%M:%SZ").to_string()
}

/// A Claude Code log of a session in `cwd` with a typed message at each of `turns` and an answer
/// that ends the turn a minute later.
fn log(id: &str, cwd: &str, turns: &[i64], sidechain: bool) -> String {
    let side = if sidechain { r#","isSidechain":true,"agentId":"a1""# } else { "" };
    let mut out = String::new();
    for (k, &t) in turns.iter().enumerate() {
        out += &format!(
            r#"{{"type":"user","sessionId":"{id}","cwd":"{cwd}","entrypoint":"cli","timestamp":"{}","message":{{"role":"user","content":"step {k}"}}{side}}}"#,
            iso(t)
        );
        out.push('\n');
        out += &format!(
            r#"{{"type":"assistant","sessionId":"{id}","cwd":"{cwd}","entrypoint":"cli","requestId":"r{k}","timestamp":"{}","message":{{"id":"m{k}","model":"claude-opus-5-5","stop_reason":"end_turn","content":[{{"type":"text","text":"done"}}],"usage":{{"input_tokens":12,"cache_read_input_tokens":20000,"cache_creation_input_tokens":300,"output_tokens":40}}}}{side}}}"#,
            iso(t + MINUTE_MS)
        );
        out.push('\n');
    }
    out
}

/// Writes a log and sets its modification time to its last line.
fn write(path: &Path, text: &str, last: i64) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
    let f = std::fs::File::options().write(true).open(path).unwrap();
    f.set_modified(std::time::UNIX_EPOCH + Duration::from_millis((last + MINUTE_MS) as u64)).unwrap();
}

const A: &str = "aaaa1111-0000-4000-8000-000000000001";
const B: &str = "bbbb2222-0000-4000-8000-000000000002";
const C: &str = "cccc3333-0000-4000-8000-000000000003";
const D: &str = "dddd4444-0000-4000-8000-000000000004";
const E: &str = "eeee5555-0000-4000-8000-000000000005";
const F: &str = "ffff6666-0000-4000-8000-000000000006";

/// Logs around the week of 26 Oct: A on Wednesday with a subagent, and F on Thursday in another
/// project; B on the Sunday evening before and E on the Monday morning after, inside the logs
/// read but outside the week's window; C two weeks before and D on the Tuesday after, outside.
fn logs(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("tokprof-week-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    let zebrafish = d.join("-w-src-zebrafish");
    let heron = d.join("-w-src-heron");
    let main = |dir: &Path, id: &str, cwd: &str, turns: &[i64]| write(&dir.join(format!("{id}.jsonl")), &log(id, cwd, turns, false), *turns.last().unwrap());
    main(&zebrafish, A, "/w/src/zebrafish", &[at(2, 10, 0), at(2, 10, 10), at(2, 10, 50)]);
    let sub = zebrafish.join(A).join("subagents");
    write(&sub.join("agent-a1.jsonl"), &log(A, "/w/src/zebrafish", &[at(2, 10, 1)], true), at(2, 10, 1));
    std::fs::write(sub.join("agent-a1.meta.json"), r#"{"agentType": "Explore", "description": "Look", "toolUseId": "toolu_1"}"#).unwrap();
    main(&heron, F, "/w/src/heron", &[at(3, 15, 0)]);
    main(&zebrafish, B, "/w/src/zebrafish", &[at(-1, 21, 0)]);
    main(&zebrafish, E, "/w/src/zebrafish", &[at(7, 9, 0)]);
    main(&zebrafish, C, "/w/src/zebrafish", &[at(-14, 10, 0)]);
    main(&zebrafish, D, "/w/src/zebrafish", &[at(8, 10, 0)]);
    d
}

fn catalog_at(dir: &Path) -> Arc<Catalog> {
    Arc::new(Catalog::new(vec![Box::new(tokprof_source_claude_code::ClaudeCode::at(dir.to_path_buf(), Path::new("/w")))], PathBuf::from("/w")))
}

fn ids(files: &[SessionRef]) -> Vec<String> {
    let mut v: Vec<String> = files.iter().map(|f| format!("{}{}", &f.main_id[..4], if f.subagent { " sub" } else { "" })).collect();
    v.sort();
    v
}

#[test]
fn the_read_takes_only_the_logs_around_the_week() {
    let d = logs("around");
    let catalog = catalog_at(&d);
    let select = Query { agents: catalog.agents(), ..Query::default() };
    let files = logs_of_week(&catalog, &select, MONDAY, offset());
    assert_eq!(ids(&files), ["aaaa", "aaaa sub", "bbbb", "eeee", "ffff"], "from Sunday 18:00 before to Monday 12:00 after, with A's subagent");
    let w = read(&catalog, &select, &Pricing::defaults(), &Key::new(MONDAY, ""), now_ms(), |_, _| {}).unwrap();
    assert_eq!((w.files, w.window), (5, (at(0, 0, 0), at(7, 0, 0))));
    // The requests of B and E lie outside the week's window.
    assert_eq!(w.overview.requests(), 5, "A's three, its subagent's one and F's one");
    assert_eq!(w.overview.sessions(), 2);
    assert!(w.sittings.starts_with("sittings  3, "), "A's two and F's, not B's or E's: {}", w.sittings);
    assert!(w.sittings.contains(" 4 messages to 2 sessions;"), "{}", w.sittings);
    assert!(w.answers.starts_with("answers   ") && w.queue.starts_with("queue     ") && w.cache.starts_with("cache     "));
    assert!(w.read.starts_with("read claude 5 files"), "{}", w.read);
    // A answered its own turn's end after 9 minutes: a wait, while no agent worked.
    assert_eq!(w.waits.len(), 1);
    assert_eq!(w.waits[0].2[0], (at(2, 10, 1), at(2, 10, 10)));
    assert_eq!(w.waiting_at(at(2, 10, 5)), [&A[..6]]);
    assert_eq!(w.yellow[0], (at(2, 10, 1), at(2, 10, 10)));
    assert_eq!(w.peak.map(|p| p.0), Some(1));
}

/// A lane of a session: its events at minutes after `base`.
fn timeline(id: &str, base: i64, events: &[(i64, Event)]) -> Timeline {
    let agent = tokprof_core::AgentId::new(tokprof_core::AgentInfo::named("claude"));
    Timeline { agent, id: id.into(), title: None, left_out: false, events: events.iter().map(|&(m, e)| (base + m * MINUTE_MS, e)).collect() }
}

#[test]
fn yellow_is_a_wait_while_no_agent_works() {
    // a: works from 14:00 to 16:00, then waits for the user until 16:30. b works from 16:10 to
    // 16:20.
    let base = 1_793_577_600_000 + 14 * HOUR_MS;
    let a = timeline(
        "a",
        base,
        &[(0, Event::Typed), (1, Event::Request), (120, Event::TurnEnd), (150, Event::Typed), (151, Event::Request), (152, Event::TurnEnd)],
    );
    let b = timeline("b", base, &[(130, Event::Typed), (131, Event::Request), (140, Event::TurnEnd)]);
    let view = View::new(vec![a.clone()], base + 24 * HOUR_MS, &|_| 0);
    let pairs: Vec<(&Timeline, &Lane)> = view.timelines.iter().zip(&view.lanes).collect();
    assert_eq!(yellow(&pairs, &view.days), [(base + 120 * MINUTE_MS, base + 150 * MINUTE_MS)], "a wait that ends at the next message");
    let view = View::new(vec![a, b], base + 24 * HOUR_MS, &|_| 0);
    let pairs: Vec<(&Timeline, &Lane)> = view.timelines.iter().zip(&view.lanes).collect();
    let at = |m: i64| base + m * MINUTE_MS;
    assert_eq!(yellow(&pairs, &view.days), [(at(120), at(130)), (at(140), at(150))], "not while b works");
}

#[test]
fn the_filter_text_keeps_a_project_a_session_and_its_subagents_or_nothing() {
    let d = logs("text");
    let catalog = catalog_at(&d);
    let select = Query { agents: catalog.agents(), ..Query::default() };
    let pricing = Pricing::defaults();
    let read_with = |text: &str| read(&catalog, &select, &pricing, &Key::new(MONDAY, text), now_ms(), |_, _| {}).unwrap();
    let project = read_with("ZebraF");
    assert_eq!((project.overview.requests(), project.overview.sessions()), (4, 1), "A and its subagent, not F in ~/src/heron");
    let session = read_with(&A[..8].to_uppercase());
    assert_eq!(session.overview.requests(), 4, "a session id keeps the session and its subagents");
    let none = read_with("zzz");
    assert_eq!((none.overview.requests(), none.sittings.as_str()), (0, ""));
    assert_eq!(Key::new(MONDAY, "  "), Key { monday: MONDAY, text: None });
    // The store's sums keep the same logs.
    let path = d.join("store").join(tokprof_core::daystore::FILE_NAME);
    let (store, _) = crate::calendar::update(&catalog, &select.agents, &pricing, &path, |_, _| {}).unwrap();
    let requests = |text: &str| week_of(&store, &select, &Key::new(MONDAY, text), now_ms()).totals.requests;
    assert_eq!((requests(""), requests("ZebraF"), requests(&A[..8]), requests("zzz")), (5, 4, 4, 0));
}

#[test]
fn a_week_whose_logs_are_gone_says_what_does_not_count_them() {
    let d = logs("gone");
    let catalog = catalog_at(&d);
    let select = Query { agents: catalog.agents(), ..Query::default() };
    let pricing = Pricing::defaults();
    let path = d.join("store").join(tokprof_core::daystore::FILE_NAME);
    crate::calendar::update(&catalog, &select.agents, &pricing, &path, |_, _| {}).unwrap();
    // The subagent's log goes after the store kept it.
    std::fs::remove_file(d.join("-w-src-zebrafish").join(A).join("subagents/agent-a1.jsonl")).unwrap();
    let (store, r) = crate::calendar::update(&catalog, &select.agents, &pricing, &path, |_, _| {}).unwrap();
    assert_eq!(r.gone, 1);
    let key = Key::new(MONDAY, "");
    let week = week_of(&store, &select, &key, now_ms());
    assert_eq!((week.totals.gone, week.totals.logs, week.totals.requests), (1, 3, 5), "the store still counts it");
    assert_eq!(week.totals.gone_line().as_deref(), Some("1 log of this week is gone: the waits, answers and What to change do not count them"));
    assert_eq!(read(&catalog, &select, &pricing, &key, now_ms(), |_, _| {}).unwrap().overview.requests(), 4, "the read does not");
    // Every log of the week gone.
    for id in [A, F] {
        let dir = if id == F { "-w-src-heron" } else { "-w-src-zebrafish" };
        std::fs::remove_file(d.join(dir).join(format!("{id}.jsonl"))).unwrap();
    }
    let (store, _) = crate::calendar::update(&catalog, &select.agents, &pricing, &path, |_, _| {}).unwrap();
    let week = week_of(&store, &select, &key, now_ms());
    assert!(week.totals.all_gone());
    assert_eq!(week.totals.gone_line().as_deref(), Some(GONE_ALL));
    assert_eq!(week.rows[2].work.len(), 1, "the strips keep the store's work");
}

#[test]
fn a_week_is_read_once_kept_and_read_again_only_after_changes_and_its_pause() {
    let d = logs("worker");
    let catalog = catalog_at(&d);
    let select = Query { agents: catalog.agents(), ..Query::default() };
    let pricing = Pricing::defaults();
    let path = d.join("store").join(tokprof_core::daystore::FILE_NAME);
    let store = Arc::new(crate::calendar::update(&catalog, &select.agents, &pricing, &path, |_, _| {}).unwrap().0);
    let woken = Arc::new(AtomicUsize::new(0));
    let counted = woken.clone();
    let mut w = Weeks::start(
        catalog,
        pricing,
        select,
        Arc::new(move || {
            counted.fetch_add(1, Ordering::SeqCst);
        }),
    );
    let key = Key::new(MONDAY, "");
    assert!(w.logs(&key).is_none() && w.week(&key).is_none() && w.progress().is_none(), "nothing is read before it is asked for");
    w.settle(&key, Some(&store), 0, Duration::from_secs(20));
    assert!(w.logs(&key).is_some_and(|r| r.logs.is_ok()), "the logs arrived");
    assert_eq!(w.week(&key).map(|wk| wk.totals.requests), Some(5), "and the store's sums");
    assert!(woken.load(Ordering::SeqCst) >= 1);
    assert!(!w.reading(&key) && !w.summing(&key) && w.progress().is_none());
    // The same store and logs: nothing is asked for again.
    w.tick(&key, Some(&store), 0);
    assert!(!w.reading(&key) && !w.summing(&key));
    assert_eq!(w.next_due(&key, 0), None);
    // The logs changed: the read waits for its pause, at least 20 s after it arrived.
    w.tick(&key, Some(&store), 1);
    assert!(!w.reading(&key), "it waits for its pause");
    assert!(w.next_due(&key, 1).expect("due after its pause") >= w.logs(&key).unwrap().arrived + PAUSE);
    // A newer store is summed again.
    let newer = Arc::new((*store).clone());
    w.tick(&key, Some(&newer), 1);
    assert!(w.summing(&key));
    // Another week is read on its own, and the first one is kept.
    let before = Key::new(MONDAY - 7, "");
    w.settle(&before, Some(&newer), 1, Duration::from_secs(20));
    assert!(w.logs(&before).is_some() && w.logs(&key).is_some(), "a week stepped away from is kept");
    assert_eq!(w.logs(&before).and_then(|r| r.logs.as_ref().ok()).map(|l| l.overview.requests()), Some(1), "C's");
}

#[test]
fn a_newer_week_replaces_the_one_asked_for_before() {
    let (jobs, queue) = mpsc::channel();
    let mut w = Weeks { jobs: Some(jobs), ..Weeks::detached() };
    let (a, b) = (Key::new(MONDAY, ""), Key::new(MONDAY - 7, ""));
    w.tick(&a, None, 0);
    w.tick(&b, None, 0);
    assert!(w.reading(&b) && !w.reading(&a), "the newer key is the one waited for");
    // The worker takes the newest job of its queue.
    let mut newest = queue.recv().unwrap();
    while let Ok(n) = queue.try_recv() {
        newest = n;
    }
    assert_eq!(newest, (b.clone(), 0));
    // The same week with another filter is another read.
    let filtered = Key::new(MONDAY - 7, "heron");
    w.tick(&filtered, None, 0);
    assert!(w.reading(&filtered) && !w.reading(&b));
}

#[test]
fn detached_weeks_read_nothing() {
    let mut w = Weeks::detached();
    let key = Key::new(MONDAY, "");
    w.tick(&key, None, 0);
    assert!(!w.reading(&key) && w.logs(&key).is_none() && w.next_due(&key, 3).is_none());
}
