//! The reports of a calendar month or quarter that the window shows: what the filters and the
//! filter text select of the period's logs, read on a worker thread into an overview (see
//! `tokprof_core::overview`), kept for each period and filter text, and read again after the logs
//! changed, with the back-off of the day beside the list. A newer key asked for replaces the one
//! pending.
//!
//! The worker takes any key that says what to read (`Span`): a period with its filter text
//! (`period::Key`).

use std::collections::HashMap;
use std::hash::Hash;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use tokprof_core::catalog::Catalog;
use tokprof_core::fmt::int;
use tokprof_core::overview::{self, Overview};
use tokprof_core::pricing::Pricing;
use tokprof_core::query::Query;
use tokprof_core::rhythm;

use crate::browser::{Wake, now_ms};
use crate::period::Key;

/// The least a report waits before it is read again after the logs changed, as the day beside
/// the list waits.
const PAUSE: Duration = Duration::from_secs(20);
/// How often the worker wakes the screen while it reads, for the count of logs.
const WAKE_EVERY: Duration = Duration::from_millis(250);

/// What a report's key reads: the query of its window, and the filter text it keeps sessions by.
pub trait Span: Clone + Eq + Hash + Send + 'static {
    /// The query the worker reads at `now_ms`.
    fn query(&self, select: &Query, now_ms: i64) -> Query;
    /// The window's filter text, if any.
    fn text(&self) -> Option<&str> {
        None
    }
}

/// A calendar period with the filter text: its window cut by `--since` and `--until`.
impl Span for Key {
    fn query(&self, select: &Query, _now_ms: i64) -> Query {
        crate::period::query(select, self.period, &rhythm::local_offset)
    }

    fn text(&self) -> Option<&str> {
        self.text.as_deref()
    }
}

/// A report the worker read.
pub struct Report {
    /// The overview, or the error that stopped it.
    pub overview: Result<Arc<Overview>, String>,
    /// The window it counts: from `since_ms` to `until_ms`, the end being `--until`, the period's
    /// end, or the moment the reading started.
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

/// How far the worker is: the key it reads, and the logs read of all it reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Progress<K = Key> {
    pub key: K,
    pub done: usize,
    pub total: usize,
}

impl Progress<Key> {
    /// `reading the logs of October 2026… 120 of 2,302 logs`.
    pub fn text(&self) -> String {
        format!("reading the logs of {}… {} of {} logs", self.key.period.name(), int(self.done as u64), int(self.total as u64))
    }
}

/// A job: the key, and the count of log changes it is read after.
type Job<K> = (K, u32);

struct Done<K> {
    job: Job<K>,
    overview: Result<Overview, String>,
    window: (i64, i64),
    took: Duration,
}

/// What the worker shares while it reads.
struct Shared<K> {
    key: Mutex<Option<K>>,
    done: AtomicUsize,
    total: AtomicUsize,
}

impl<K> Default for Shared<K> {
    fn default() -> Shared<K> {
        Shared { key: Mutex::new(None), done: AtomicUsize::new(0), total: AtomicUsize::new(0) }
    }
}

/// The window a query counts at `now`: from its `--since` to its end, or to now.
fn window_of(query: &Query, now: i64) -> (i64, i64) {
    let since = query.since_ms.unwrap_or(now);
    (since, query.until_ms.map_or(now, |u| u.min(now)).max(since.min(now)))
}

/// The reports and the worker that reads them.
pub struct Reports<K: Span = Key> {
    jobs: Option<Sender<Job<K>>>,
    notes: Receiver<Done<K>>,
    kept: HashMap<K, Report>,
    /// The job asked for last, until its report arrives.
    pending: Option<Job<K>>,
    shared: Arc<Shared<K>>,
    /// The filters, for the window of a report that fails without a worker.
    select: Query,
}

impl<K: Span> Reports<K> {
    /// Starts the worker over the logs of `catalog` that `select` selects. It reads nothing until
    /// a report is asked for.
    pub fn start(catalog: Arc<Catalog>, pricing: Pricing, select: Query, wake: Wake) -> Reports<K> {
        let (jobs, rx) = mpsc::channel::<Job<K>>();
        let (tx, notes) = mpsc::channel();
        let shared: Arc<Shared<K>> = Arc::default();
        {
            let (shared, select) = (shared.clone(), select.clone());
            std::thread::spawn(move || {
                let mut last: Option<Job<K>> = None;
                while let Ok(mut job) = rx.recv() {
                    // Only the newest key asked for is read: the ones before it were left.
                    while let Ok(newer) = rx.try_recv() {
                        job = newer;
                    }
                    // A key asked for again while it was read, after the same changes, was just
                    // sent: reading it twice in a row would only cost the time.
                    if last.as_ref() == Some(&job) {
                        continue;
                    }
                    last = Some(job.clone());
                    let started = Instant::now();
                    let now = now_ms();
                    let query = job.0.query(&select, now);
                    let window = window_of(&query, now);
                    *shared.key.lock().unwrap_or_else(|e| e.into_inner()) = Some(job.0.clone());
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
                    let text = job.0.text();
                    let overview = crate::contain::run(|| overview::collect_text(&catalog, &query, text, &pricing, progress).map_err(|e| format!("{e:#}")))
                        .and_then(|o| o);
                    if tx.send(Done { job, overview, window, took: started.elapsed() }).is_err() {
                        return;
                    }
                    wake();
                }
            });
        }
        Reports { jobs: Some(jobs), notes, kept: HashMap::new(), pending: None, shared, select }
    }

    /// Reports that read nothing, for tests: they hold what `put` gives them.
    pub fn detached() -> Reports<K> {
        let (_, notes) = mpsc::channel();
        Reports { jobs: None, notes, kept: HashMap::new(), pending: None, shared: Arc::default(), select: Query::default() }
    }

    /// Keeps `overview` as the report of a key over a window, read after `changes`.
    pub fn put(&mut self, key: K, overview: Overview, window: (i64, i64), changes: u32) {
        let report =
            Report { overview: Ok(Arc::new(overview)), since_ms: window.0, until_ms: window.1, changes, took: Duration::ZERO, arrived: Instant::now() };
        self.kept.insert(key, report);
    }

    /// Takes in what the worker read, and asks for the report of `key` when it has none, or when
    /// the logs changed since its report (`changes` counts them) and its pause has passed. A key
    /// asked for replaces the one pending. Returns whether a report arrived.
    pub fn tick(&mut self, key: &K, changes: u32) -> bool {
        let arrived = self.absorb();
        self.request(key, changes);
        arrived
    }

    fn absorb(&mut self) -> bool {
        let mut arrived = false;
        while let Ok(d) = self.notes.try_recv() {
            arrived = true;
            // Only the report asked for ends the wait: an older one of the same key, read before
            // the logs changed again, or one of a key left since, leaves it reading.
            if self.pending.as_ref() == Some(&d.job) {
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

    fn request(&mut self, key: &K, changes: u32) {
        let Some(jobs) = &self.jobs else { return };
        if self.pending.as_ref().is_some_and(|p| &p.0 == key) {
            return;
        }
        if let Some(r) = self.kept.get(key)
            && (r.changes == changes || Instant::now() < r.due())
        {
            return;
        }
        self.pending = Some((key.clone(), changes));
        // A worker that is gone reads nothing more: the report fails instead of waiting.
        if jobs.send((key.clone(), changes)).is_err() {
            self.pending = None;
            let now = now_ms();
            let (since_ms, until_ms) = window_of(&key.query(&self.select, now), now);
            let overview = Err("the worker that reads the reports stopped".to_string());
            self.kept.insert(key.clone(), Report { overview, since_ms, until_ms, changes, took: Duration::ZERO, arrived: Instant::now() });
        }
    }

    /// When the report of `key` is due to be read again, while it is out of date and waits for
    /// its pause; a screen that sleeps until something happens wakes then.
    pub fn next_due(&self, key: &K, changes: u32) -> Option<Instant> {
        if self.jobs.is_none() || self.reading(key) {
            return None;
        }
        self.kept.get(key).filter(|r| r.changes != changes).map(Report::due)
    }

    /// The report of a key, once read.
    pub fn get(&self, key: &K) -> Option<&Report> {
        self.kept.get(key)
    }

    /// Whether the key is being read, a first time or again.
    pub fn reading(&self, key: &K) -> bool {
        self.pending.as_ref().is_some_and(|p| &p.0 == key)
    }

    /// How far the worker is, while anything is asked for: the key it reads now, which may be
    /// one left for a newer one that waits for it to end.
    pub fn progress(&self) -> Option<Progress<K>> {
        let pending = self.pending.as_ref()?;
        let s = &self.shared;
        let key = s.key.lock().unwrap_or_else(|e| e.into_inner()).clone().unwrap_or_else(|| pending.0.clone());
        Some(Progress { key, done: s.done.load(Ordering::Relaxed), total: s.total.load(Ordering::Relaxed) })
    }

    /// Asks for `key` and waits up to `limit` for it, for a screen that draws one frame and exits.
    pub fn settle(&mut self, key: &K, changes: u32, limit: Duration) {
        let deadline = Instant::now() + limit;
        self.tick(key, changes);
        while Instant::now() < deadline && self.get(key).is_none() {
            std::thread::sleep(Duration::from_millis(50));
            self.absorb();
        }
    }
}

#[cfg(test)]
#[path = "reports_tests.rs"]
mod tests;
