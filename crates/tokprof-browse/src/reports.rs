//! The reports over a period that the window shows: the last 7, 30 or 90 days of what the filters
//! select, read on a worker thread into an overview (see `tokprof_core::overview`), kept for each
//! period, and read again after the logs changed, with the back-off of the day beside the list.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use tokprof_core::catalog::Catalog;
use tokprof_core::overview::{self, Overview};
use tokprof_core::pricing::Pricing;
use tokprof_core::query::Query;

use crate::browser::{Wake, now_ms};

/// The periods, in days.
pub const PERIODS: [u32; 3] = [7, 30, 90];
/// The period shown first.
pub const DEFAULT_PERIOD: u32 = 30;
/// The least a report waits before it is read again after the logs changed, as the day beside
/// the list waits.
const PAUSE: Duration = Duration::from_secs(20);
/// How often the worker wakes the screen while it reads, for the count of logs.
const WAKE_EVERY: Duration = Duration::from_millis(250);
const DAY_MS: i64 = 86_400_000;

/// A report the worker read.
pub struct Report {
    /// The overview, or the error that stopped it.
    pub overview: Result<Arc<Overview>, String>,
    /// The window it counts: from `since_ms` to `until_ms`, the end being `--until` or the moment
    /// the reading started.
    pub since_ms: i64,
    pub until_ms: i64,
    /// The count of log changes it was read after, how long it took, and when it arrived.
    pub changes: u32,
    pub took: Duration,
    pub arrived: Instant,
}

impl Report {
    /// When it may be read again after the logs changed: four times as long as it took, at least
    /// `PAUSE`, after it arrived.
    fn due(&self) -> Instant {
        self.arrived + (self.took * 4).max(PAUSE)
    }
}

/// How far the worker is: the period it reads, and the logs read of all it reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Progress {
    pub days: u32,
    pub done: usize,
    pub total: usize,
}

/// The query of a period of `days` days at `now_ms`: the filters, with the period's start unless
/// `--since` starts later.
pub fn period_query(select: &Query, days: u32, now_ms: i64) -> Query {
    let start = now_ms - days as i64 * DAY_MS;
    Query { since_ms: Some(select.since_ms.map_or(start, |s| s.max(start))), ..select.clone() }
}

/// A job: the period, and the count of log changes it is read after.
type Job = (u32, u32);

struct Done {
    job: Job,
    overview: Result<Overview, String>,
    window: (i64, i64),
    took: Duration,
}

/// What the worker shares while it reads.
#[derive(Default)]
struct Shared {
    days: AtomicU32,
    done: AtomicUsize,
    total: AtomicUsize,
}

/// The reports and the worker that reads them.
pub struct Reports {
    jobs: Option<Sender<Job>>,
    notes: Receiver<Done>,
    kept: HashMap<u32, Report>,
    /// The job asked for last, until its period arrives.
    pending: Option<Job>,
    shared: Arc<Shared>,
}

impl Reports {
    /// Starts the worker over the logs of `catalog` that `select` selects. It reads nothing until
    /// a report is asked for.
    pub fn start(catalog: Arc<Catalog>, pricing: Pricing, select: Query, wake: Wake) -> Reports {
        let (jobs, rx) = mpsc::channel::<Job>();
        let (tx, notes) = mpsc::channel();
        let shared = Arc::new(Shared::default());
        {
            let shared = shared.clone();
            std::thread::spawn(move || {
                let mut last: Option<Job> = None;
                while let Ok(mut job) = rx.recv() {
                    while let Ok(newer) = rx.try_recv() {
                        job = newer;
                    }
                    // A period asked for again while it was read, after the same changes, was
                    // just sent: reading it twice in a row would only cost the time.
                    if last == Some(job) {
                        continue;
                    }
                    last = Some(job);
                    let started = Instant::now();
                    let now = now_ms();
                    let query = period_query(&select, job.0, now);
                    let window = (query.since_ms.unwrap_or(now), query.until_ms.map_or(now, |u| u.min(now)));
                    shared.days.store(job.0, Ordering::Relaxed);
                    shared.done.store(0, Ordering::Relaxed);
                    shared.total.store(0, Ordering::Relaxed);
                    let woke = Mutex::new(Instant::now());
                    let progress = |done: usize, total: usize| {
                        shared.done.store(done, Ordering::Relaxed);
                        shared.total.store(total, Ordering::Relaxed);
                        let mut at = woke.lock().unwrap_or_else(|e| e.into_inner());
                        if at.elapsed() >= WAKE_EVERY {
                            *at = Instant::now();
                            wake();
                        }
                    };
                    // A report that panicked fails as one that could not be read.
                    let overview =
                        crate::contain::run(|| overview::collect(&catalog, &query, &pricing, progress).map_err(|e| format!("{e:#}"))).and_then(|o| o);
                    if tx.send(Done { job, overview, window, took: started.elapsed() }).is_err() {
                        return;
                    }
                    wake();
                }
            });
        }
        Reports { jobs: Some(jobs), notes, kept: HashMap::new(), pending: None, shared }
    }

    /// Reports that read nothing, for tests: they hold what `put` gives them.
    pub fn detached() -> Reports {
        let (_, notes) = mpsc::channel();
        Reports { jobs: None, notes, kept: HashMap::new(), pending: None, shared: Arc::default() }
    }

    /// Keeps `overview` as the report of a period over a window, read after `changes`.
    pub fn put(&mut self, days: u32, overview: Overview, window: (i64, i64), changes: u32) {
        let report =
            Report { overview: Ok(Arc::new(overview)), since_ms: window.0, until_ms: window.1, changes, took: Duration::ZERO, arrived: Instant::now() };
        self.kept.insert(days, report);
    }

    /// Takes in what the worker read, and asks for the period `days` when it has no report, or
    /// when the logs changed since its report (`changes` counts them) and its pause has passed.
    /// Returns whether a report arrived.
    pub fn tick(&mut self, days: u32, changes: u32) -> bool {
        let arrived = self.absorb();
        self.request(days, changes);
        arrived
    }

    fn absorb(&mut self) -> bool {
        let mut arrived = false;
        while let Ok(d) = self.notes.try_recv() {
            arrived = true;
            // Only the report asked for ends the wait: an older one of the same period, read
            // before the logs changed again, leaves it reading.
            if self.pending == Some(d.job) {
                self.pending = None;
            }
            let report = Report {
                overview: d.overview.map(Arc::new),
                since_ms: d.window.0,
                until_ms: d.window.1,
                changes: d.job.1,
                took: d.took,
                arrived: Instant::now(),
            };
            self.kept.insert(d.job.0, report);
        }
        arrived
    }

    fn request(&mut self, days: u32, changes: u32) {
        let Some(jobs) = &self.jobs else { return };
        if self.pending.is_some_and(|p| p.0 == days) {
            return;
        }
        if let Some(r) = self.kept.get(&days)
            && (r.changes == changes || Instant::now() < r.due())
        {
            return;
        }
        self.pending = Some((days, changes));
        // A worker that is gone reads nothing more: the report fails instead of waiting.
        if jobs.send((days, changes)).is_err() {
            self.pending = None;
            let now = now_ms();
            let overview = Err("the worker that reads the reports stopped".to_string());
            let report = Report { overview, since_ms: now - days as i64 * DAY_MS, until_ms: now, changes, took: Duration::ZERO, arrived: Instant::now() };
            self.kept.insert(days, report);
        }
    }

    /// When the report of `days` is due to be read again, while it is out of date and waits for
    /// its pause; a screen that sleeps until something happens wakes then.
    pub fn next_due(&self, days: u32, changes: u32) -> Option<Instant> {
        if self.jobs.is_none() || self.pending.is_some_and(|p| p.0 == days) {
            return None;
        }
        self.kept.get(&days).filter(|r| r.changes != changes).map(Report::due)
    }

    /// The report of a period, once read.
    pub fn get(&self, days: u32) -> Option<&Report> {
        self.kept.get(&days)
    }

    /// Whether the period `days` is being read, a first time or again.
    pub fn reading(&self, days: u32) -> bool {
        self.pending.is_some_and(|p| p.0 == days)
    }

    /// How far the worker is, while anything is asked for.
    pub fn progress(&self) -> Option<Progress> {
        self.pending?;
        let s = &self.shared;
        Some(Progress { days: s.days.load(Ordering::Relaxed), done: s.done.load(Ordering::Relaxed), total: s.total.load(Ordering::Relaxed) })
    }

    /// Asks for the period `days` and waits up to `limit` for it, for a screen that draws one
    /// frame and exits.
    pub fn settle(&mut self, days: u32, changes: u32, limit: Duration) {
        let deadline = Instant::now() + limit;
        self.tick(days, changes);
        while Instant::now() < deadline && self.get(days).is_none() {
            std::thread::sleep(Duration::from_millis(50));
            self.absorb();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::{Path, PathBuf};

    #[test]
    fn only_the_report_asked_for_ends_the_wait() {
        let (tx, notes) = mpsc::channel();
        let (jobs, _queue) = mpsc::channel();
        let mut r = Reports { jobs: Some(jobs), notes, kept: HashMap::new(), pending: Some((7, 2)), shared: Arc::default() };
        let done = |job: Job| Done { job, overview: Err("empty".into()), window: (0, 1), took: Duration::ZERO };
        tx.send(done((30, 2))).unwrap();
        assert!(r.tick(7, 2), "the 30 days arrived");
        assert!(r.reading(7) && r.get(30).is_some(), "another period leaves 7 days reading");
        tx.send(done((7, 1))).unwrap();
        r.tick(7, 2);
        assert!(r.reading(7) && r.get(7).is_some_and(|x| x.changes == 1), "a report from before the last change leaves it reading");
        tx.send(done((7, 2))).unwrap();
        r.tick(7, 2);
        assert!(!r.reading(7) && r.progress().is_none(), "the report asked for ends it");
    }

    #[test]
    fn a_period_starts_its_days_back_unless_the_filter_starts_later() {
        let now = 100 * DAY_MS;
        let q = period_query(&Query { project: Some("app".into()), until_ms: Some(now), ..Query::default() }, 7, now);
        assert_eq!(q.since_ms, Some(93 * DAY_MS));
        assert_eq!((q.project.as_deref(), q.until_ms), (Some("app"), Some(now)), "the other filters stay");
        let q = period_query(&Query { since_ms: Some(98 * DAY_MS), ..Query::default() }, 30, now);
        assert_eq!(q.since_ms, Some(98 * DAY_MS), "a later --since is kept");
        let q = period_query(&Query { since_ms: Some(10 * DAY_MS), ..Query::default() }, 30, now);
        assert_eq!(q.since_ms, Some(70 * DAY_MS));
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
        let mut r = Reports::start(
            catalog,
            Pricing::defaults(),
            select,
            Arc::new(move || {
                counted.fetch_add(1, Ordering::SeqCst);
            }),
        );
        assert!(r.get(30).is_none() && r.progress().is_none(), "nothing is read before it is asked for");
        r.settle(30, 0, Duration::from_secs(20));
        let report = r.get(30).expect("the report arrived");
        assert!(report.overview.is_ok());
        assert_eq!(report.until_ms - report.since_ms, 30 * DAY_MS);
        assert!(woken.load(Ordering::SeqCst) >= 1, "the worker woke the screen when it sent");
        assert!(r.progress().is_none() && !r.reading(30));
        // The same logs: nothing is asked for again, and nothing is due.
        r.tick(30, 0);
        assert!(!r.reading(30));
        assert_eq!(r.next_due(30, 0), None);
        // The logs changed: the report waits for its pause, at least 20 s after it arrived.
        r.tick(30, 1);
        assert!(!r.reading(30), "it waits for its pause");
        let due = r.next_due(30, 1).expect("due after its pause");
        assert!(due >= r.get(30).unwrap().arrived + PAUSE);
        // Another period is read on its own.
        r.tick(7, 1);
        assert!(r.reading(7) && r.progress().is_some(), "the worker counts its logs while it reads");
    }

    #[test]
    fn detached_reports_read_nothing() {
        let mut r = Reports::detached();
        r.tick(30, 0);
        assert!(!r.reading(30) && r.get(30).is_none() && r.next_due(30, 3).is_none());
    }

    #[test]
    fn a_report_that_panicked_shows_the_error_and_the_worker_goes_on() {
        use crate::contain::testing::{self, BOOM};
        let (catalog, armed, _) = testing::catalog("reports");
        let select = Query { agents: catalog.agents(), ..Query::default() };
        let mut r = Reports::start(catalog, Pricing::defaults(), select, Arc::new(|| {}));
        r.settle(30, 0, Duration::from_secs(20));
        let failed = r.get(30).expect("the report arrived");
        assert_eq!(failed.overview.as_ref().err(), Some(&format!("tokprof panicked: {BOOM}")));
        assert!(!r.reading(30));
        // The next report, once its pause passed, is read by the same worker.
        armed.store(false, Ordering::SeqCst);
        r.kept.clear();
        r.settle(30, 1, Duration::from_secs(20));
        assert!(r.get(30).is_some_and(|x| x.overview.is_ok()));
    }

    #[test]
    fn a_worker_that_is_gone_fails_the_report_instead_of_waiting() {
        let (jobs, rx) = mpsc::channel();
        drop(rx);
        let mut r = Reports { jobs: Some(jobs), ..Reports::detached() };
        r.tick(30, 0);
        assert!(!r.reading(30));
        assert_eq!(r.get(30).and_then(|x| x.overview.as_ref().err()).map(String::as_str), Some("the worker that reads the reports stopped"));
    }
}
