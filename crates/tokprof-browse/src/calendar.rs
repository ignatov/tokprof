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

/// The least the calendar waits before it is computed again after the logs changed: each time
/// reads the grown logs whole and writes the store.
const PAUSE: Duration = Duration::from_secs(60);
const WAKE_EVERY: Duration = Duration::from_millis(250);

/// The store at `path`, brought up to date with the logs of `agents` and written back when an
/// entry changed. A store that exists but cannot be read is used in memory and never written; a
/// store that could not be parsed was kept aside; a write that fails leaves the old store. Each
/// says so in the refresh's warnings, which do not stop the calendar. `progress(done, total)`
/// follows the logs read.
pub fn update(
    catalog: &Catalog,
    agents: &[AgentId],
    pricing: &Pricing,
    path: &Path,
    progress: impl Fn(usize, usize) + Sync + Send,
) -> Result<(Store, Refreshed)> {
    let mut store = Store::load(path, now_ms());
    let mut refreshed = daystore::refresh(&mut store, catalog, agents, pricing, &crate::day::temp_folders(), progress)?;
    let shown = |p: &Path| display_path(&p.to_string_lossy(), None, catalog.home());
    if let Some(e) = &store.unreadable {
        refreshed.warnings.push(format!("warning: the day store {} could not be read ({e}); this run keeps it in memory and does not write it", shown(path)));
    } else if (refreshed.changed || store.broken.is_some())
        && let Err(e) = store.save()
    {
        refreshed.warnings.push(format!("warning: the day store could not be written: {e:#}"));
    }
    if let Some(aside) = &store.broken {
        refreshed.warnings.push(format!("warning: the day store could not be parsed; it was kept as {} and started anew", shown(aside)));
    }
    Ok((store, refreshed))
}

/// The prices of the window, read again when the user's price file or `--prices` changed, so
/// that the window and the command line make the same fingerprint.
pub struct PriceWatch {
    explicit: Option<std::path::PathBuf>,
    stamps: Vec<Option<std::time::SystemTime>>,
    pricing: Pricing,
}

impl PriceWatch {
    pub fn new(pricing: Pricing, explicit: Option<std::path::PathBuf>) -> PriceWatch {
        let mut w = PriceWatch { explicit, stamps: Vec::new(), pricing };
        w.stamps = w.stamps_now();
        w
    }

    fn stamps_now(&self) -> Vec<Option<std::time::SystemTime>> {
        std::iter::once(crate::paths::user_prices()).chain(self.explicit.clone()).map(|p| std::fs::metadata(p).and_then(|m| m.modified()).ok()).collect()
    }

    /// The prices, read again if a price file changed since; a file that fails to read keeps
    /// the prices before.
    pub fn current(&mut self) -> &Pricing {
        let now = self.stamps_now();
        if now != self.stamps {
            self.stamps = now;
            if let Ok(p) = crate::paths::pricing(self.explicit.as_deref()) {
                self.pricing = p;
            }
        }
        &self.pricing
    }
}

/// The calendar's filters from the command line's.
pub fn filters(select: &Query) -> Filters {
    Filters {
        agents: select.agents.iter().map(|a| a.name().to_string()).collect(),
        project: select.project.clone(),
        session: select.session.clone(),
        since_ms: select.since_ms,
        until_ms: select.until_ms,
        text: None,
    }
}

/// The calendar of the store's entries as of `now_ms`, in the machine's time zone.
pub fn compute(store: &Store, select: &Query, now_ms: i64) -> Calendar {
    Calendar::of(&store.entries, &filters(select), now_ms, &rhythm::local_offset)
}

/// The month `ym` of the store's entries that `select` keeps, whatever its time window, as of
/// `now_ms`, in the machine's time zone.
pub fn month(store: &Store, select: &Query, ym: tokprof_core::month::Ym, now_ms: i64) -> tokprof_core::month::Month {
    let f = Filters { since_ms: None, until_ms: None, ..filters(select) };
    tokprof_core::month::Month::of(&store.entries, &f, ym, now_ms, &rhythm::local_offset)
}

/// The line on what the refresh read: the logs read again as other commands name them, the logs
/// taken from the store, and those that are gone.
pub fn read_line(catalog: &Catalog, r: &Refreshed, secs: f64, path: &Path) -> String {
    let read = if r.read.is_empty() { format!("read no log in {secs:.1}s") } else { crate::reading::sources(catalog, &r.read, secs) };
    let gone = if r.gone > 0 { format!(", {} gone from disk", tokprof_core::fmt::int(r.gone as u64)) } else { String::new() };
    format!(
        "{read}; {} {} from the store{gone} ({})",
        tokprof_core::fmt::int(r.from_store as u64),
        if r.from_store == 1 { "log" } else { "logs" },
        display_path(&path.to_string_lossy(), None, catalog.home())
    )
}

/// A calendar the worker computed.
pub struct Computed {
    pub calendar: Result<Arc<Calendar>, String>,
    /// The store it was computed from, for the month in review.
    pub store: Option<Arc<Store>>,
    /// What went wrong with the store: unreadable, reset, or not written.
    pub warnings: Vec<String>,
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
    store: Option<Arc<Store>>,
    warnings: Vec<String>,
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
    pub fn start(
        catalog: Arc<Catalog>,
        pricing: Pricing,
        prices: Option<std::path::PathBuf>,
        select: Query,
        path: std::path::PathBuf,
        wake: Wake,
    ) -> Calendars {
        let mut watch = PriceWatch::new(pricing, prices);
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
                    // A calendar that panicked fails as one whose store could not be brought up to date.
                    let pricing = watch.current();
                    let computed = crate::contain::run(|| {
                        update(&catalog, &select.agents, pricing, &path, progress).map(|(store, r)| (compute(&store, &select, now_ms()), r, store))
                    });
                    let (calendar, gone, store, warnings) = match computed.map_err(anyhow::Error::msg).and_then(|c| c) {
                        Ok((calendar, r, store)) => (Ok(calendar), r.gone, Some(Arc::new(store)), r.warnings),
                        Err(e) => (Err(format!("{e:#}")), 0, None, Vec::new()),
                    };
                    if tx.send(Done { changes, calendar, store, gone, warnings, took: started.elapsed() }).is_err() {
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
        self.kept = Some(Computed {
            calendar: Ok(Arc::new(calendar)),
            store: None,
            warnings: Vec::new(),
            gone: 0,
            changes,
            took: Duration::ZERO,
            arrived: Instant::now(),
        });
    }

    /// `put`, with the store it came from.
    pub fn put_with(&mut self, calendar: Calendar, store: Store, changes: u32) {
        self.put(calendar, changes);
        if let Some(c) = &mut self.kept {
            c.store = Some(Arc::new(store));
        }
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
            self.kept = Some(Computed {
                calendar: d.calendar.map(Arc::new),
                store: d.store,
                warnings: d.warnings,
                gone: d.gone,
                changes: d.changes,
                took: d.took,
                arrived: Instant::now(),
            });
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
        // A worker that is gone computes nothing more: the calendar fails instead of waiting.
        if jobs.send(changes).is_err() {
            self.pending = None;
            self.kept = Some(Computed {
                calendar: Err("the worker that computes the calendar stopped".to_string()),
                store: None,
                warnings: Vec::new(),
                gone: 0,
                changes,
                took: Duration::ZERO,
                arrived: Instant::now(),
            });
        }
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

    /// The calendar computed last, to change, as a test sets what the worker would have sent.
    pub fn get_mut(&mut self) -> Option<&mut Computed> {
        self.kept.as_mut()
    }

    /// Whether a worker computes it; a detached one holds only what it is given.
    pub fn computes(&self) -> bool {
        self.jobs.is_some()
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

    /// The entry of a deleted log of `agent`, whose session ran in `project`, with one request on
    /// 2026-09-01 at 10:00 UTC.
    fn kept(agent: &str, key: &Path, project: &str) -> daystore::Entry {
        let minute = 1_788_256_800_000 / 60_000;
        daystore::Entry {
            key: key.to_string_lossy().to_string(),
            agent: agent.into(),
            session: format!("s-{project}"),
            main: format!("s-{project}"),
            project: project.into(),
            minutes: vec![daystore::Minute { minute, usd: 2.5, requests: 1, ..daystore::Minute::default() }],
            ..daystore::Entry::default()
        }
    }

    #[test]
    fn kept_entries_are_placed_again() {
        let dir = scratch("placed-again");
        let home = dir.join("home");
        // A repository at `~/src/app`, and a worktree of it at `~/lease/2/app`.
        let git = home.join("src").join("app").join(".git").join("worktrees").join("app2");
        std::fs::create_dir_all(&git).unwrap();
        std::fs::write(git.join("commondir"), "../..\n").unwrap();
        let worktree = home.join("lease").join("2").join("app");
        std::fs::create_dir_all(&worktree).unwrap();
        std::fs::write(worktree.join(".git"), format!("gitdir: {}\n", git.to_string_lossy().replace('\\', "/"))).unwrap();
        let sources: Vec<Box<dyn tokprof_core::source::LogSource>> = vec![
            Box::new(tokprof_source_claude_code::ClaudeCode::at(dir.join("projects"), &home)),
            Box::new(tokprof_source_codex::Codex::at(dir.join("codex"), &home)),
        ];
        let catalog = Catalog::new(sources, home.clone());
        let path = dir.join(daystore::FILE_NAME);
        let logs = dir.join("deleted");
        let entries = vec![
            kept("claude", &logs.join("a.jsonl"), "~/lease/2/app"),
            kept("codex", &logs.join("b.jsonl"), "~/Documents/Codex/2026-09-01/f"),
            kept("codex", &logs.join("c.jsonl"), "~/.codex/worktrees/0dca/app"),
        ];
        Store { path: path.clone(), entries, ..Store::default() }.save().unwrap();
        let agents = catalog.agents();
        let (store, r) = update(&catalog, &agents, &Pricing::defaults(), &path, |_, _| {}).unwrap();
        assert_eq!((r.gone, r.changed), (3, true));
        let placed: Vec<(&str, &str)> = store.entries.iter().map(|e| (e.project.as_str(), e.folder.as_str())).collect();
        let sep = std::path::MAIN_SEPARATOR;
        let app = format!("~{sep}src{sep}app");
        assert_eq!(
            placed,
            [(app.as_str(), "~/lease/2/app"), ("codex chats", "~/Documents/Codex/2026-09-01/f"), ("~/.codex/worktrees/0dca/app", "")],
            "a deleted log's worktree moves to its repository, a deleted Codex chat to the chats, and a gone unknown folder stays"
        );
        // The store keeps them so, and `--project` counts the moved entry's day.
        let loaded = Store::load(&path, 0);
        assert_eq!(loaded.entries.iter().map(|e| (e.project.as_str(), e.folder.as_str())).collect::<Vec<_>>(), placed);
        let select = |project: &str| Query { agents: agents.clone(), project: Some(project.into()), ..Query::default() };
        let now = 1_800_000_000_000;
        let repository = compute(&store, &select(&app), now);
        assert_eq!((repository.active(Metric::Spend).0, repository.total(Metric::Spend)), (1, 2.5), "one day with spend");
        assert_eq!(compute(&store, &select("lease"), now).total(Metric::Spend), 2.5, "the folder's text too");
        assert_eq!(compute(&store, &select("codex chats"), now).total(Metric::Spend), 2.5);
        // Placed once, they are not placed again.
        let (_, again) = update(&catalog, &agents, &Pricing::defaults(), &path, |_, _| {}).unwrap();
        assert!(!again.changed);
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
        let mut c = Calendars::start(catalog, Pricing::defaults(), None, select, path, Arc::new(|| {}));
        assert!(c.get().is_none() && c.progress().is_none(), "nothing before it is asked for");
        c.settle(0, Duration::from_secs(20));
        assert!(c.get().is_some_and(|x| x.calendar.is_ok()));
        c.tick(0);
        assert!(!c.reading() && c.next_due(0).is_none());
        c.tick(1);
        assert!(!c.reading(), "it waits for its pause");
        assert!(c.next_due(1).unwrap() >= c.get().unwrap().arrived + PAUSE);
    }

    #[cfg(unix)]
    #[test]
    fn a_moved_log_that_cannot_be_read_keeps_its_old_entry() {
        let dir = scratch("moved-unreadable");
        let logs = dir.join("projects");
        copy_dir(&fixtures(), &logs);
        let path = dir.join(daystore::FILE_NAME);
        let pricing = Pricing::defaults();
        let catalog = catalog_at(&logs);
        let agents = catalog.agents();
        let (first, _) = update(&catalog, &agents, &pricing, &path, |_, _| {}).unwrap();
        let one = PathBuf::from(&first.entries.iter().find(|e| !e.subagent && !e.key.contains("subagents")).unwrap().key);
        let moved = one.parent().unwrap().parent().unwrap().join("moved-project");
        std::fs::create_dir_all(&moved).unwrap();
        let to = moved.join(one.file_name().unwrap());
        std::fs::rename(&one, &to).unwrap();
        // The log at the new place cannot be read: the old entry stays, as the only record.
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&to, std::fs::Permissions::from_mode(0o000)).unwrap();
        let (store, r) = update(&catalog, &agents, &pricing, &path, |_, _| {}).unwrap();
        std::fs::set_permissions(&to, std::fs::Permissions::from_mode(0o644)).unwrap();
        if r.unreadable.is_empty() {
            return; // root reads anything; the rule only shows for a user the file shuts out
        }
        assert!(store.entries.iter().any(|e| e.key == one.to_string_lossy()), "the old entry stays");
        assert_eq!(store.entries.len(), first.entries.len());
    }

    #[test]
    fn a_store_that_cannot_be_written_still_gives_the_calendar() {
        let dir = scratch("unwritable");
        // The store's path is a folder: the rename over it fails.
        let path = dir.join(daystore::FILE_NAME);
        std::fs::create_dir_all(path.join("x")).unwrap();
        let catalog = catalog_at(&fixtures());
        let (store, r) = update(&catalog, &catalog.agents(), &Pricing::defaults(), &path, |_, _| {}).unwrap();
        assert!(!store.entries.is_empty());
        assert!(r.warnings.iter().any(|w| w.starts_with("warning: the day store")), "{:?}", r.warnings);
    }

    #[test]
    fn the_window_reads_the_prices_again_when_their_file_changes() {
        let dir = scratch("prices");
        let file = dir.join("prices.toml");
        std::fs::write(&file, "[models.\"claude-opus-5-5\"]\ninput = 1.0\noutput = 2.0\n").unwrap();
        let first = crate::paths::pricing(Some(&file)).unwrap();
        let mut watch = PriceWatch::new(first, Some(file.clone()));
        let before = daystore::fingerprint(watch.current());
        std::thread::sleep(Duration::from_millis(20));
        std::fs::write(&file, "[models.\"claude-opus-5-5\"]\ninput = 3.0\noutput = 2.0\n").unwrap();
        let t = std::time::SystemTime::now() + Duration::from_secs(5);
        std::fs::File::options().write(true).open(&file).unwrap().set_modified(t).unwrap();
        assert_ne!(daystore::fingerprint(watch.current()), before);
    }

    #[test]
    fn a_detached_calendar_computes_nothing() {
        let mut c = Calendars::detached();
        c.tick(0);
        assert!(!c.reading() && c.get().is_none() && c.next_due(1).is_none());
        c.put(Calendar::default(), 0);
        assert!(c.get().is_some());
    }

    #[test]
    fn a_calendar_that_panicked_shows_the_error_and_the_worker_goes_on() {
        use crate::contain::testing::{self, BOOM};
        let (catalog, armed, dir) = testing::catalog("calendar");
        let select = Query { agents: catalog.agents(), ..Query::default() };
        let mut c = Calendars::start(catalog, Pricing::defaults(), None, select, dir.join(daystore::FILE_NAME), Arc::new(|| {}));
        c.settle(0, Duration::from_secs(20));
        let failed = c.get().expect("the calendar arrived");
        assert_eq!(failed.calendar.as_ref().err(), Some(&format!("tokprof panicked: {BOOM}")));
        assert!(!c.reading());
        // The next calendar, once its pause passed, is computed by the same worker.
        armed.store(false, Ordering::SeqCst);
        c.kept = None;
        c.settle(1, Duration::from_secs(20));
        assert!(c.get().is_some_and(|x| x.calendar.is_ok()));
    }

    #[test]
    fn a_worker_that_is_gone_fails_the_calendar_instead_of_waiting() {
        let (jobs, rx) = mpsc::channel();
        drop(rx);
        let mut c = Calendars { jobs: Some(jobs), ..Calendars::detached() };
        c.tick(0);
        assert!(!c.reading());
        assert_eq!(c.get().and_then(|x| x.calendar.as_ref().err()).map(String::as_str), Some("the worker that computes the calendar stopped"));
    }
}
