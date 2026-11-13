//! A calendar week as the window shows it: its numbers from the day store, summed on a worker
//! the moment a week is asked for, and what only its logs give, read on another worker from the
//! logs around the week by Day's rule (see `day::logs_around`). Both are kept for each week and
//! filter text; the logs are read again after they changed, with the back-off of the reports.

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::Result;
use tokprof_core::agents::Agents;
use tokprof_core::calendar::{Filters, text_matches};
use tokprof_core::catalog::Catalog;
use tokprof_core::daystore::Store;
use tokprof_core::overview::{self, Overview};
use tokprof_core::pauses::ColdReturn;
use tokprof_core::pricing::Pricing;
use tokprof_core::query::Query;
use tokprof_core::rhythm::{self, Days, Lane, Offset, Timeline, View};
use tokprof_core::source::SessionRef;
use tokprof_core::week::{Week, Window};

use crate::browser::{Wake, now_ms};

const HOUR_MS: i64 = 3_600_000;
/// The least the week's logs wait before they are read again after they changed, as a report
/// waits.
const PAUSE: Duration = Duration::from_secs(20);
const WAKE_EVERY: Duration = Duration::from_millis(250);

/// A week and the window's filter text: what the store's sums and the read are kept by.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Key {
    /// The local date of the week's Monday, as days since 1970-01-01.
    pub monday: i64,
    /// The filter text, trimmed; `None` when there is none.
    pub text: Option<String>,
}

impl Key {
    /// The week of `monday` with the filter `text`; blank text is no filter.
    pub fn new(monday: i64, text: &str) -> Key {
        let t = text.trim();
        Key { monday, text: (!t.is_empty()).then(|| t.to_string()) }
    }
}

/// The store's filters of a week: the command line's, with the filter text.
pub fn filters(select: &Query, key: &Key) -> Filters {
    Filters { text: key.text.clone(), ..crate::calendar::filters(select) }
}

/// The week of `key` in the store, as of `now_ms`, in the machine's time zone. The store has no
/// client, so `--client` does not cut it.
pub fn week_of(store: &Store, select: &Query, key: &Key, now_ms: i64) -> Week {
    Week::of(&store.entries, &filters(select, key), key.monday, now_ms, &rhythm::local_offset)
}

/// The logs a week is read from, by the times of their lines: from 18:00 on the Sunday before the
/// week to 12:00 on the Monday after it, as `tokprof day` picks the logs of a day.
pub fn read_window(monday: i64, offset: Offset) -> (i64, i64) {
    (rhythm::midnight(monday, offset) - 6 * HOUR_MS, rhythm::midnight(monday + 7, offset) + 12 * HOUR_MS)
}

/// What the read counts: the week's window, to now for this week, cut by `--since` and `--until`.
pub fn counted(select: &Query, monday: i64, now_ms: i64, offset: Offset) -> (i64, i64) {
    let w = Window::of(monday, now_ms, offset);
    let from = select.since_ms.map_or(w.from_ms, |s| s.max(w.from_ms));
    (from, select.until_ms.map_or(w.to_ms, |u| u.min(w.to_ms)).max(from))
}

/// The main logs around the week by Day's rule, and the subagent logs of their sessions.
pub fn logs_of_week(catalog: &Catalog, select: &Query, monday: i64, offset: Offset) -> Vec<SessionRef> {
    let all = catalog.discover(&select.agents);
    let (subagents, mains): (Vec<SessionRef>, Vec<SessionRef>) = all.into_iter().partition(|f| f.subagent);
    let mut picked = crate::day::logs_around(catalog, select, read_window(monday, offset), Some(&mains));
    let sessions: HashSet<(String, String)> = picked.iter().map(|f| (f.agent.name().to_string(), f.session_id.clone())).collect();
    picked.extend(subagents.into_iter().filter(|f| select.holds(f) && sessions.contains(&(f.agent.name().to_string(), f.main_id.clone()))));
    picked
}

/// The waits of a session for the user: its id, its label and the spans.
pub type Waits = (String, String, Vec<(i64, i64)>);

/// What the week's logs give: the overview of `tokprof summary`, `pauses` and `share` over the
/// week's window, and the rhythm of its main sessions.
pub struct WeekLogs {
    /// The summary, the pauses and What to change, as Reports has them.
    pub overview: Overview,
    /// The window counted.
    pub window: (i64, i64),
    /// The logs read, with their size in bytes.
    pub files: usize,
    pub bytes: u64,
    /// The line on what was read, as other commands write it, and the warnings.
    pub read: String,
    pub warnings: Vec<String>,
    /// The rhythm lines of the window, as `tokprof pauses` prints them; empty without a message.
    pub sittings: String,
    pub answers: String,
    pub queue: String,
    /// The `cache` line, from the cold returns of the window.
    pub cache: String,
    /// The cold returns of the window in time order, each with its session.
    pub returns: Vec<ColdReturn>,
    /// Whether any request read had a price.
    pub priced: bool,
    /// The rhythm's advice over the window, cold returns first.
    pub advice: Vec<String>,
    /// The highest number of agents at work and the first moment of it, as Day counts them.
    pub peak: Option<(u32, i64)>,
    /// Each kept session with a wait for the user in the read: its id, its label (the first 6
    /// characters of the id and its title) and the waits, from the end of the agent's turn to the
    /// message that answered it, by the Day strip's rule.
    pub waits: Vec<Waits>,
    /// Where some session waited for the user while no agent worked: the strips' yellow.
    pub yellow: Vec<(i64, i64)>,
}

impl WeekLogs {
    /// The sessions that waited for the user at `ts`, by their labels.
    pub fn waiting_at(&self, ts: i64) -> Vec<&str> {
        self.waits.iter().filter(|(_, _, w)| w.iter().any(|&(a, b)| a <= ts && ts < b)).map(|(_, label, _)| label.as_str()).collect()
    }
}

/// Reads the logs of the week of `key` into what only they give, as of `now_ms`, calling
/// `progress(done, total)` before the first log and after each. The filter text keeps the logs
/// whose agent, project, session id or main session id holds it, as the store's sums do.
pub fn read(catalog: &Catalog, select: &Query, pricing: &Pricing, key: &Key, now_ms: i64, progress: impl Fn(usize, usize) + Sync + Send) -> Result<WeekLogs> {
    let started = Instant::now();
    let offset: Offset = &rhythm::local_offset;
    let window = counted(select, key.monday, now_ms, offset);
    let files = logs_of_week(catalog, select, key.monday, offset);
    let query = Query { since_ms: Some(window.0), until_ms: Some(window.1), ..select.clone() };
    let keep =
        |s: &tokprof_core::Session, f: &SessionRef| key.text.as_deref().is_none_or(|t| text_matches(t, f.agent.name(), &s.project, &f.session_id, &f.main_id));
    let temp = crate::day::temp_folders();
    let (overview, timelines) = overview::collect_logs(catalog, &files, &query, pricing, keep, Some(&temp), progress)?;
    let read = crate::reading::sources(catalog, &files, started.elapsed().as_secs_f64());
    let warnings = crate::reading::warnings(&overview.unreadable);
    // The days as `tokprof pauses --until` finds them: as of the window's end.
    let view = View::new(timelines, window.1, offset);
    let summary = view.summary(window);
    // As the pause report's rhythm section, no lines without a message.
    let lines = if summary.sittings.is_empty() { Vec::new() } else { summary.lines() };
    let line = |head: &str| lines.iter().find(|l| l.starts_with(head)).cloned().unwrap_or_default();
    let mut returns: Vec<ColdReturn> =
        overview.pauses.agents.values().flat_map(|a| a.returns.iter().cloned()).filter(|r| r.ts_ms >= window.0 && r.ts_ms < window.1).collect();
    returns.sort_by(|a, b| a.ts_ms.cmp(&b.ts_ms).then_with(|| a.session.cmp(&b.session)));
    let priced = overview.pauses.agents.values().any(|a| a.priced);
    let advice = summary.advice(&rhythm::Returns::of(&returns, priced), &view.instruction_files(window));
    let kept: Vec<(&Timeline, &Lane)> = view.timelines.iter().zip(&view.lanes).filter(|(t, _)| !t.left_out).collect();
    let peak = Agents::of(&kept, &summary.sittings, summary.keyboard_ms(), window).peak;
    let waits = waits(&kept, &view.days);
    let yellow = yellow(&kept, &view.days);
    Ok(WeekLogs {
        window,
        files: files.len(),
        bytes: files.iter().map(|f| f.size).sum(),
        read,
        warnings,
        sittings: line("sittings  "),
        answers: line("answers   "),
        queue: line("queue     "),
        cache: crate::render_rhythm::cache_line(&returns, priced),
        returns,
        priced,
        advice,
        peak,
        waits,
        yellow,
        overview,
    })
}

/// The waits of each session, as the Day strip draws them: from an end of the agent's turn to the
/// message that answered it, in the same day.
fn waits(lanes: &[(&Timeline, &Lane)], days: &Days) -> Vec<Waits> {
    lanes
        .iter()
        .filter_map(|(t, l)| {
            let spans: Vec<(i64, i64)> = l.answers.iter().filter(|(end, msg)| msg > end && days.day_of(*end) == days.day_of(*msg)).copied().collect();
            let id6: String = t.id.chars().take(6).collect();
            (!spans.is_empty()).then(|| (t.id.clone(), format!("{id6} {}", t.title.as_deref().unwrap_or("")).trim_end().to_string(), spans))
        })
        .collect()
}

/// Where some session waited for the user, as the Day strip counts a wait, while no agent worked.
pub fn yellow(lanes: &[(&Timeline, &Lane)], days: &Days) -> Vec<(i64, i64)> {
    let wait = union(waits(lanes, days).into_iter().flat_map(|(_, _, w)| w));
    let work = union(lanes.iter().flat_map(|(_, l)| l.work.iter().copied()));
    let mut out = Vec::new();
    for (a, b) in wait {
        let mut from = a;
        for &(x, y) in work.iter().filter(|(x, y)| *y > a && *x < b) {
            if x > from {
                out.push((from, x));
            }
            from = from.max(y);
        }
        if b > from {
            out.push((from, b));
        }
    }
    out
}

/// Spans merged where they overlap, in time order.
fn union(spans: impl Iterator<Item = (i64, i64)>) -> Vec<(i64, i64)> {
    let mut v: Vec<(i64, i64)> = spans.filter(|(a, b)| b > a).collect();
    v.sort_unstable();
    let mut out: Vec<(i64, i64)> = Vec::new();
    for (a, b) in v {
        match out.last_mut() {
            Some(last) if a <= last.1 => last.1 = last.1.max(b),
            _ => out.push((a, b)),
        }
    }
    out
}

/// The store's sums of a week, as the worker computed them from one store.
pub struct Summed {
    pub week: Arc<Week>,
    /// The store they come from, by its address.
    store: usize,
}

/// A week's logs as the worker read them.
pub struct Read {
    /// What they give, or the error that stopped the read.
    pub logs: Result<Arc<WeekLogs>, String>,
    /// The count of log changes it was read after, how long it took, and when it arrived.
    pub changes: u32,
    pub took: Duration,
    pub arrived: Instant,
}

impl Read {
    /// When it may be read again after the logs changed: four times as long as it took, at least
    /// `PAUSE`, after it arrived.
    fn due(&self) -> Instant {
        self.arrived + (self.took * 4).max(PAUSE)
    }
}

/// A read asked for: the week and filter, and the count of log changes.
type Job = (Key, u32);
/// Sums asked for: the week and filter, the store, and its address.
type SumJob = (Key, Arc<Store>, usize);

struct ReadDone {
    job: Job,
    logs: Result<WeekLogs, String>,
    took: Duration,
}

struct SumDone {
    key: Key,
    store: usize,
    week: Week,
}

#[derive(Default)]
struct Shared {
    done: AtomicUsize,
    total: AtomicUsize,
}

/// The weeks and the two workers that compute them: one sums the store, one reads the logs.
/// Each takes one job at a time, the newest asked for first.
pub struct Weeks {
    jobs: Option<Sender<Job>>,
    sum_jobs: Option<Sender<SumJob>>,
    notes: Receiver<ReadDone>,
    sums: Receiver<SumDone>,
    kept: HashMap<Key, Read>,
    summed: HashMap<Key, Summed>,
    /// The read asked for last, until it arrives, and the same for the sums.
    pending: Option<Job>,
    pending_sum: Option<(Key, usize)>,
    shared: Arc<Shared>,
}

impl Weeks {
    /// Starts the workers over the logs of `catalog` that `select` selects. They compute nothing
    /// until a week is asked for.
    pub fn start(catalog: Arc<Catalog>, pricing: Pricing, select: Query, wake: Wake) -> Weeks {
        let (jobs, rx) = mpsc::channel::<Job>();
        let (tx, notes) = mpsc::channel();
        let (sum_jobs, sum_rx) = mpsc::channel::<SumJob>();
        let (sum_tx, sums) = mpsc::channel();
        let shared = Arc::new(Shared::default());
        {
            let (select, wake) = (select.clone(), wake.clone());
            std::thread::spawn(move || {
                while let Ok(mut job) = sum_rx.recv() {
                    while let Ok(newer) = sum_rx.try_recv() {
                        job = newer;
                    }
                    let (key, store, at) = job;
                    let week = crate::contain::run(|| week_of(&store, &select, &key, now_ms()));
                    // A sum that panicked leaves the week without its numbers; the next store
                    // tries again.
                    let Ok(week) = week else { continue };
                    if sum_tx.send(SumDone { key, store: at, week }).is_err() {
                        return;
                    }
                    wake();
                }
            });
        }
        {
            let shared = shared.clone();
            std::thread::spawn(move || {
                let mut last: Option<Job> = None;
                while let Ok(mut job) = rx.recv() {
                    while let Ok(newer) = rx.try_recv() {
                        job = newer;
                    }
                    // A week asked for again while it was read, after the same changes, was
                    // just sent.
                    if last.as_ref() == Some(&job) {
                        continue;
                    }
                    last = Some(job.clone());
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
                    let logs =
                        crate::contain::run(|| read(&catalog, &select, &pricing, &job.0, now_ms(), progress).map_err(|e| format!("{e:#}"))).and_then(|r| r);
                    if tx.send(ReadDone { job, logs, took: started.elapsed() }).is_err() {
                        return;
                    }
                    wake();
                }
            });
        }
        Weeks {
            jobs: Some(jobs),
            sum_jobs: Some(sum_jobs),
            notes,
            sums,
            kept: HashMap::new(),
            summed: HashMap::new(),
            pending: None,
            pending_sum: None,
            shared,
        }
    }

    /// Weeks that compute nothing, for tests: they hold what `put` and `put_logs` give them.
    pub fn detached() -> Weeks {
        let (_, notes) = mpsc::channel();
        let (_, sums) = mpsc::channel();
        Weeks {
            jobs: None,
            sum_jobs: None,
            notes,
            sums,
            kept: HashMap::new(),
            summed: HashMap::new(),
            pending: None,
            pending_sum: None,
            shared: Arc::default(),
        }
    }

    /// Keeps `week` as the store's sums of `key`.
    pub fn put(&mut self, key: Key, week: Week) {
        self.summed.insert(key, Summed { week: Arc::new(week), store: 0 });
    }

    /// Keeps `logs` as the read of `key` after `changes`.
    pub fn put_logs(&mut self, key: Key, logs: WeekLogs, changes: u32) {
        self.kept.insert(key, Read { logs: Ok(Arc::new(logs)), changes, took: Duration::ZERO, arrived: Instant::now() });
    }

    /// Takes in what the workers sent, and asks for the week of `key`: its sums when `store` is
    /// newer than the one they came from, and its read when it has none, or when the logs changed
    /// since (`changes` counts them) and its pause has passed. A key asked for replaces the one
    /// asked for before it. Returns whether anything arrived.
    pub fn tick(&mut self, key: &Key, store: Option<&Arc<Store>>, changes: u32) -> bool {
        let arrived = self.absorb();
        if let Some(store) = store {
            self.request_sum(key, store);
        }
        self.request(key, changes);
        arrived
    }

    fn absorb(&mut self) -> bool {
        let mut arrived = false;
        while let Ok(d) = self.sums.try_recv() {
            arrived = true;
            if self.pending_sum.as_ref().is_some_and(|(k, s)| *k == d.key && *s == d.store) {
                self.pending_sum = None;
            }
            self.summed.insert(d.key, Summed { week: Arc::new(d.week), store: d.store });
        }
        while let Ok(d) = self.notes.try_recv() {
            arrived = true;
            // Only the read asked for ends the wait.
            if self.pending.as_ref() == Some(&d.job) {
                self.pending = None;
            }
            let read = Read { logs: d.logs.map(Arc::new), changes: d.job.1, took: d.took, arrived: Instant::now() };
            self.kept.insert(d.job.0, read);
        }
        arrived
    }

    fn request_sum(&mut self, key: &Key, store: &Arc<Store>) {
        let Some(jobs) = &self.sum_jobs else { return };
        let at = Arc::as_ptr(store) as usize;
        if self.summed.get(key).is_some_and(|s| s.store == at) || self.pending_sum.as_ref().is_some_and(|(k, s)| k == key && *s == at) {
            return;
        }
        self.pending_sum = Some((key.clone(), at));
        if jobs.send((key.clone(), store.clone(), at)).is_err() {
            self.pending_sum = None;
        }
    }

    fn request(&mut self, key: &Key, changes: u32) {
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
        // A worker that is gone reads nothing more: the read fails instead of waiting.
        if jobs.send((key.clone(), changes)).is_err() {
            self.pending = None;
            let read = Read { logs: Err("the worker that reads the week's logs stopped".to_string()), changes, took: Duration::ZERO, arrived: Instant::now() };
            self.kept.insert(key.clone(), read);
        }
    }

    /// The store's sums of a week, once computed; from an older store while a newer one is summed.
    pub fn week(&self, key: &Key) -> Option<&Arc<Week>> {
        self.summed.get(key).map(|s| &s.week)
    }

    /// The read of a week's logs, once it arrived.
    pub fn logs(&self, key: &Key) -> Option<&Read> {
        self.kept.get(key)
    }

    /// Whether the week's logs are being read, a first time or again.
    pub fn reading(&self, key: &Key) -> bool {
        self.pending.as_ref().is_some_and(|p| &p.0 == key)
    }

    /// Whether the week's sums are being computed from a newer store.
    pub fn summing(&self, key: &Key) -> bool {
        self.pending_sum.as_ref().is_some_and(|(k, _)| k == key)
    }

    /// The logs read of all the read takes, while a read is asked for.
    pub fn progress(&self) -> Option<(usize, usize)> {
        self.pending.as_ref()?;
        Some((self.shared.done.load(Ordering::Relaxed), self.shared.total.load(Ordering::Relaxed)))
    }

    /// When the read of `key` is due again, while it is out of date and waits for its pause.
    pub fn next_due(&self, key: &Key, changes: u32) -> Option<Instant> {
        if self.jobs.is_none() || self.reading(key) {
            return None;
        }
        self.kept.get(key).filter(|r| r.changes != changes).map(Read::due)
    }

    /// Asks for the week and waits up to `limit` for its sums and its read, for a frame without
    /// a window.
    pub fn settle(&mut self, key: &Key, store: Option<&Arc<Store>>, changes: u32, limit: Duration) {
        let deadline = Instant::now() + limit;
        self.tick(key, store, changes);
        while Instant::now() < deadline && (self.logs(key).is_none() || (store.is_some() && self.week(key).is_none())) {
            std::thread::sleep(Duration::from_millis(50));
            self.absorb();
        }
    }
}

#[cfg(test)]
#[path = "week_tests.rs"]
mod tests;
