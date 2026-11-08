//! The calendar as both programs get it: the day store brought up to date with the logs, the
//! line on what that read, and, for the window, a worker that does it in the background and does
//! it again after the logs changed, with the back-off of the reports.

use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::Result;
use tokprof_core::calendar::{Calendar, Filters};
use tokprof_core::catalog::Catalog;
use tokprof_core::daystore::{self, Refreshed, Store};
use tokprof_core::labels::display_path;
use tokprof_core::pricing::Pricing;
use tokprof_core::query::Query;
use tokprof_core::rhythm;
use tokprof_core::source::AgentId;

use crate::browser::{Wake, now_ms};

/// The least the calendar waits before it is computed again after the logs changed.
const PAUSE: Duration = Duration::from_secs(20);
const WAKE_EVERY: Duration = Duration::from_millis(250);

/// The store at `path`, brought up to date with the logs of `agents` and written back when it
/// changed. `progress(done, total)` follows the logs read.
pub fn update(
    catalog: &Catalog,
    agents: &[AgentId],
    pricing: &Pricing,
    path: &Path,
    progress: impl Fn(usize, usize) + Sync + Send,
) -> Result<(Store, Refreshed)> {
    let mut store = Store::load(path);
    let refreshed = daystore::refresh(&mut store, catalog, agents, pricing, &crate::day::temp_folders(), progress)?;
    if refreshed.changed || store.broken.is_some() {
        store.save()?;
    }
    Ok((store, refreshed))
}

/// The calendar's filters from the command line's.
pub fn filters(select: &Query) -> Filters {
    Filters {
        agents: select.agents.iter().map(|a| a.name().to_string()).collect(),
        project: select.project.clone(),
        session: select.session.clone(),
        since_ms: select.since_ms,
        until_ms: select.until_ms,
    }
}

/// The calendar of the store's entries as of `now_ms`, in the machine's time zone.
pub fn compute(store: &Store, select: &Query, now_ms: i64) -> Calendar {
    Calendar::of(&store.entries, &filters(select), now_ms, &rhythm::local_offset)
}

/// The line on what the refresh read: the logs read again as other commands name them, the logs
/// taken from the store, and those that are gone.
pub fn read_line(catalog: &Catalog, r: &Refreshed, secs: f64, path: &Path) -> String {
    let read = if r.read.is_empty() { format!("read no log in {secs:.1}s") } else { crate::reading::sources(catalog, &r.read, secs) };
    let gone = if r.gone > 0 { format!(", {} gone from disk", tokprof_core::fmt::int(r.gone as u64)) } else { String::new() };
    format!(
        "{read}; {} from the store{gone} ({})",
        tokprof_core::fmt::count(r.from_store as u64, "log", "logs"),
        display_path(&path.to_string_lossy(), None, catalog.home())
    )
}

/// A calendar the worker computed.
pub struct Computed {
    pub calendar: Result<Arc<Calendar>, String>,
    /// The logs whose days come from the store alone.
    pub gone: usize,
    pub changes: u32,
    pub took: Duration,
    pub arrived: Instant,
}

impl Computed {
    fn due(&self) -> Instant {
        self.arrived + (self.took * 4).max(PAUSE)
    }
}

struct Done {
    changes: u32,
    calendar: Result<Calendar, String>,
    gone: usize,
    took: Duration,
}

#[derive(Default)]
struct Shared {
    done: AtomicUsize,
    total: AtomicUsize,
}

/// The calendar and the worker that computes it.
pub struct Calendars {
    jobs: Option<Sender<u32>>,
    notes: Receiver<Done>,
    kept: Option<Computed>,
    pending: Option<u32>,
    shared: Arc<Shared>,
}

impl Calendars {
    /// Starts the worker over the logs `select` selects, with the store at `path`. It reads
    /// nothing until the calendar is asked for.
    pub fn start(catalog: Arc<Catalog>, pricing: Pricing, select: Query, path: std::path::PathBuf, wake: Wake) -> Calendars {
        let (jobs, rx) = mpsc::channel::<u32>();
        let (tx, notes) = mpsc::channel();
        let shared = Arc::new(Shared::default());
        {
            let shared = shared.clone();
            std::thread::spawn(move || {
                while let Ok(mut changes) = rx.recv() {
                    while let Ok(newer) = rx.try_recv() {
                        changes = newer;
                    }
                    let started = Instant::now();
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
                    let (calendar, gone) = match update(&catalog, &select.agents, &pricing, &path, progress) {
                        Ok((store, r)) => (Ok(compute(&store, &select, now_ms())), r.gone),
                        Err(e) => (Err(format!("{e:#}")), 0),
                    };
                    if tx.send(Done { changes, calendar, gone, took: started.elapsed() }).is_err() {
                        return;
                    }
                    wake();
                }
            });
        }
        Calendars { jobs: Some(jobs), notes, kept: None, pending: None, shared }
    }

    /// A calendar that computes nothing, for tests: it holds what `put` gives it.
    pub fn detached() -> Calendars {
        let (_, notes) = mpsc::channel();
        Calendars { jobs: None, notes, kept: None, pending: None, shared: Arc::default() }
    }

    pub fn put(&mut self, calendar: Calendar, changes: u32) {
        self.kept = Some(Computed { calendar: Ok(Arc::new(calendar)), gone: 0, changes, took: Duration::ZERO, arrived: Instant::now() });
    }

    /// Takes in what the worker computed, and asks for the calendar when there is none, or when
    /// the logs changed since (`changes` counts them) and its pause has passed. Returns whether
    /// a calendar arrived.
    pub fn tick(&mut self, changes: u32) -> bool {
        let arrived = self.absorb();
        self.request(changes);
        arrived
    }

    fn absorb(&mut self) -> bool {
        let mut arrived = false;
        while let Ok(d) = self.notes.try_recv() {
            arrived = true;
            if self.pending == Some(d.changes) {
                self.pending = None;
            }
            self.kept = Some(Computed { calendar: d.calendar.map(Arc::new), gone: d.gone, changes: d.changes, took: d.took, arrived: Instant::now() });
        }
        arrived
    }

    fn request(&mut self, changes: u32) {
        let Some(jobs) = &self.jobs else { return };
        if self.pending.is_some() {
            return;
        }
        if let Some(c) = &self.kept
            && (c.changes == changes || Instant::now() < c.due())
        {
            return;
        }
        self.pending = Some(changes);
        let _ = jobs.send(changes);
    }

    /// When the calendar is due to be computed again, while it is out of date.
    pub fn next_due(&self, changes: u32) -> Option<Instant> {
        if self.jobs.is_none() || self.pending.is_some() {
            return None;
        }
        self.kept.as_ref().filter(|c| c.changes != changes).map(Computed::due)
    }

    pub fn get(&self) -> Option<&Computed> {
        self.kept.as_ref()
    }

    /// Whether it is being computed, a first time or again.
    pub fn reading(&self) -> bool {
        self.pending.is_some()
    }

    /// The logs read into the store of all to read, while it is being computed.
    pub fn progress(&self) -> Option<(usize, usize)> {
        self.pending?;
        Some((self.shared.done.load(Ordering::Relaxed), self.shared.total.load(Ordering::Relaxed)))
    }

    /// Asks for the calendar and waits up to `limit` for it, for a frame without a window.
    pub fn settle(&mut self, changes: u32, limit: Duration) {
        let deadline = Instant::now() + limit;
        self.tick(changes);
        while Instant::now() < deadline && self.kept.is_none() {
            std::thread::sleep(Duration::from_millis(50));
            self.absorb();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use tokprof_core::analysis;
    use tokprof_core::calendar::Metric;
    use tokprof_core::report::Collector;
    use tokprof_core::scan::{OnUnreadable, report_logs, scan};

    fn fixtures() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../sources/claude-code/tests/fixtures/projects")
    }

    fn catalog_at(dir: &Path) -> Arc<Catalog> {
        Arc::new(Catalog::new(vec![Box::new(tokprof_source_claude_code::ClaudeCode::at(dir.to_path_buf(), Path::new("/w")))], PathBuf::from("/w")))
    }

    fn scratch(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("tokprof-calendar-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn copy_dir(from: &Path, to: &Path) {
        std::fs::create_dir_all(to).unwrap();
        for e in std::fs::read_dir(from).unwrap().flatten() {
            let p = e.path();
            if p.is_dir() {
                copy_dir(&p, &to.join(e.file_name()));
            } else {
                std::fs::copy(&p, to.join(e.file_name())).unwrap();
            }
        }
    }

    #[test]
    fn the_store_sums_to_the_summary() {
        let catalog = catalog_at(&fixtures());
        let select = Query { agents: catalog.agents(), ..Query::default() };
        let pricing = Pricing::defaults();
        let path = scratch("sum").join(daystore::FILE_NAME);
        let (store, r) = update(&catalog, &select.agents, &pricing, &path, |_, _| {}).unwrap();
        assert!(!r.read.is_empty() && r.from_store == 0 && path.exists());
        let files = report_logs(&catalog, &select);
        let (collector, _) = scan(
            &catalog,
            &files,
            &select,
            OnUnreadable::Skip,
            || Collector::new(None, None),
            |c, s, _| analysis::analyze(s, &pricing, &select.window(), c),
            |mut a, b| {
                a.merge(b);
                a
            },
        )
        .unwrap();
        let usd: f64 = store.entries.iter().flat_map(|e| &e.minutes).map(|m| m.usd).sum();
        let avoidable: f64 = store.entries.iter().flat_map(|e| &e.minutes).map(|m| m.avoidable_usd).sum();
        let requests: u64 = store.entries.iter().flat_map(|e| &e.minutes).map(|m| m.requests as u64).sum();
        assert!(collector.spend.priced_usd() > 0.0);
        assert!((usd - collector.spend.priced_usd()).abs() < 1e-9, "{usd} vs {}", collector.spend.priced_usd());
        assert!((avoidable - collector.avoidable_usd()).abs() < 1e-9);
        assert_eq!(requests, collector.spend.models.values().map(|m| m.requests).sum::<u64>());
        // The calendar over all time counts the same, whenever the requests were.
        let cal = compute(&store, &select, 1_900_000_000_000);
        assert!(cal.weeks() <= 53);
        assert!(cal.total(Metric::Spend) <= usd + 1e-9);
    }

    #[test]
    fn unchanged_logs_are_not_read_and_gone_ones_stay() {
        let dir = scratch("refresh");
        let logs = dir.join("projects");
        copy_dir(&fixtures(), &logs);
        let path = dir.join(daystore::FILE_NAME);
        let pricing = Pricing::defaults();
        let catalog = catalog_at(&logs);
        let agents = catalog.agents();
        let (first, r1) = update(&catalog, &agents, &pricing, &path, |_, _| {}).unwrap();
        let n = first.entries.len();
        assert_eq!((r1.read.len(), r1.from_store, r1.gone), (n, 0, 0));
        // Again: nothing is read and nothing written.
        let written = std::fs::metadata(&path).unwrap().modified().unwrap();
        let (_, r2) = update(&catalog, &agents, &pricing, &path, |_, _| {}).unwrap();
        assert_eq!((r2.read.len(), r2.from_store, r2.changed), (0, n, false));
        assert_eq!(std::fs::metadata(&path).unwrap().modified().unwrap(), written);
        // A log grows: only it is read again.
        let one = first.entries.iter().find(|e| !e.subagent).unwrap().key.clone();
        let mut f = std::fs::OpenOptions::new().append(true).open(&one).unwrap();
        std::io::Write::write_all(&mut f, b"\n").unwrap();
        drop(f);
        let (_, r3) = update(&catalog, &agents, &pricing, &path, |_, _| {}).unwrap();
        assert_eq!((r3.read.len(), r3.from_store), (1, n - 1));
        // New prices: every log is read again.
        let mut other = Pricing::defaults();
        other.add_layer("test", None, "[models.\"claude-opus-5-5\"]\ninput = 1.0\noutput = 2.0\n").unwrap();
        let (_, r4) = update(&catalog, &agents, &other, &path, |_, _| {}).unwrap();
        assert_eq!(r4.read.len(), n);
        // The log is deleted: its entry stays, flagged gone.
        std::fs::remove_file(&one).unwrap();
        let (store, r5) = update(&catalog, &agents, &other, &path, |_, _| {}).unwrap();
        assert_eq!((store.entries.len(), r5.gone), (n, 1));
        assert!(store.entries.iter().find(|e| e.key == one).unwrap().gone);
        let line = read_line(&catalog, &r5, 0.3, &path);
        assert!(line.starts_with("read no log in 0.3s; ") && line.contains(", 1 gone from disk ("), "{line}");
    }

    #[test]
    fn a_moved_log_counts_once() {
        let dir = scratch("moved");
        let logs = dir.join("projects");
        copy_dir(&fixtures(), &logs);
        let path = dir.join(daystore::FILE_NAME);
        let pricing = Pricing::defaults();
        let catalog = catalog_at(&logs);
        let agents = catalog.agents();
        let (first, _) = update(&catalog, &agents, &pricing, &path, |_, _| {}).unwrap();
        let n = first.entries.len();
        let one = PathBuf::from(&first.entries.iter().find(|e| !e.subagent && !e.key.contains("subagents")).unwrap().key);
        let project = one.parent().unwrap();
        let moved = project.parent().unwrap().join("moved-project");
        std::fs::create_dir_all(&moved).unwrap();
        std::fs::rename(&one, moved.join(one.file_name().unwrap())).unwrap();
        let (store, r) = update(&catalog, &agents, &pricing, &path, |_, _| {}).unwrap();
        assert_eq!((store.entries.len(), r.gone), (n, 0), "the old path's entry is dropped");
    }

    #[test]
    fn the_worker_computes_once_and_again_only_after_changes_and_its_pause() {
        let catalog = catalog_at(&fixtures());
        let select = Query { agents: catalog.agents(), ..Query::default() };
        let path = scratch("worker").join(daystore::FILE_NAME);
        let mut c = Calendars::start(catalog, Pricing::defaults(), select, path, Arc::new(|| {}));
        assert!(c.get().is_none() && c.progress().is_none(), "nothing before it is asked for");
        c.settle(0, Duration::from_secs(20));
        assert!(c.get().is_some_and(|x| x.calendar.is_ok()));
        c.tick(0);
        assert!(!c.reading() && c.next_due(0).is_none());
        c.tick(1);
        assert!(!c.reading(), "it waits for its pause");
        assert!(c.next_due(1).unwrap() >= c.get().unwrap().arrived + PAUSE);
    }

    #[test]
    fn a_detached_calendar_computes_nothing() {
        let mut c = Calendars::detached();
        c.tick(0);
        assert!(!c.reading() && c.get().is_none() && c.next_due(1).is_none());
        c.put(Calendar::default(), 0);
        assert!(c.get().is_some());
    }
}
