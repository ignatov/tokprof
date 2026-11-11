//! The session browser without its screen: the sessions on the left, the X-ray of the selected
//! one on the right, and the day view, as `tokprof ui` and `tokprof-desk` show them. Costs fill in
//! from a background pass over all logs; X-rays are computed on a worker thread, newest request
//! first, and kept. The list follows the log directories: new sessions show up, and a session that
//! is still being written is costed and X-rayed again as it grows. Rows move only when the user
//! sorts or filters; what arrives in the background fills in cells.
//!
//! A screen gives the browser its keys and the size of its panes, and draws what it reads back:
//! the rows' cells, the titles, the footer and the texts, which the screen turns from text with
//! color codes into what it draws once, when they arrive.

use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::{Duration, Instant};

use anyhow::{Result, bail};
use rayon::prelude::*;

use tokprof_core::catalog::Catalog;
use tokprof_core::fmt::{local_time, usd};
use tokprof_core::labels::display_path;
use tokprof_core::pricing::Pricing;
use tokprof_core::query::Query;
use tokprof_core::rhythm;
use tokprof_core::source::{Locator, SessionRef};
use tokprof_core::xray::{self, Glance, SessionRow, XRay};

use crate::render_rhythm::DayParts;
use crate::watch::{self, Change};

/// The least a day shown beside the list waits before it is computed again after the logs
/// changed.
const BESIDE_DAY_PAUSE: Duration = Duration::from_secs(20);

/// The order of the list.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sort {
    Time,
    Cost,
    /// What the next request would cost now.
    Next,
    /// The sessions that wait for the user, the longest wait first, then the others by time;
    /// only with the waiting option.
    Waiting,
}

impl Sort {
    pub const ALL: [Sort; 4] = [Sort::Time, Sort::Cost, Sort::Next, Sort::Waiting];

    pub fn name(self) -> &'static str {
        match self {
            Sort::Time => "time",
            Sort::Cost => "cost",
            Sort::Next => "next request",
            Sort::Waiting => "waiting",
        }
    }

    /// The order after this one, with or without the waiting option.
    fn following(self, waiting: bool) -> Sort {
        match self {
            Sort::Time => Sort::Cost,
            Sort::Cost => Sort::Next,
            Sort::Next if waiting => Sort::Waiting,
            Sort::Next | Sort::Waiting => Sort::Time,
        }
    }
}

/// A key as the browser takes it, from a terminal or a window.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    Char(char),
    Up,
    Down,
    Left,
    Right,
    PageUp,
    PageDown,
    Home,
    End,
    Enter,
    Esc,
    Backspace,
    /// Ctrl+C, which quits from anywhere, also while a filter is being typed.
    Interrupt,
}

/// How much the screen shows: the rows of the list that make a page, and the lines of the right
/// pane, which a full-screen X-ray or day scrolls through.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Viewport {
    pub page: usize,
    pub lines: usize,
}

/// What a screen does with the texts it shows. Each text arrives as text with color codes; the
/// screen turns it into what it draws once, when it arrives.
pub struct Frontend<T> {
    /// The text as the screen draws it.
    pub show: fn(String) -> T,
    /// The lines of a shown text, which scrolling stops at.
    pub lines: fn(&T) -> usize,
    /// The day's layout in a pane of this size.
    pub day_layout: fn((u16, u16)) -> rhythm::Layout,
}

/// Wakes the screen when the background work sent something, so that a screen that sleeps until
/// something happens draws it.
pub type Wake = Arc<dyn Fn() + Send + Sync>;

/// What the right pane shows.
pub enum Pane<'a, T> {
    Text(&'a T),
    /// The text is being computed: dim.
    Computing(String),
    /// There is nothing to show.
    Empty(&'static str),
}

/// The cells of a listed session.
pub struct Cells<'a> {
    /// The last activity, local `MM-DD HH:MM`.
    pub time: String,
    pub agent: &'a str,
    /// The agent's place in the registry, which picks its color.
    pub agent_place: usize,
    pub project: &'a str,
    /// The session's cost so far, or `…` while it is being costed.
    pub cost: String,
    /// The same in dollars, once known.
    pub usd: Option<f64>,
    /// The next-request cell, and whether the cache is cold.
    pub next: Option<(String, bool)>,
    /// The session's title, once its glance found one.
    pub title: Option<&'a str>,
    /// The last activity, in epoch ms.
    pub active_ms: i64,
    /// While the cache is warm, when it goes cold, in epoch ms.
    pub warm_until: Option<i64>,
    /// What the next request costs now.
    pub next_usd: Option<f64>,
}

enum Note {
    /// A session's glance, the version of its logs it was computed from, and how long it took.
    Glance(usize, u32, Glance, Duration),
    /// The same for an X-ray: its text, and the X-ray itself unless it failed.
    Report(usize, u32, String, Option<Arc<XRay>>, Duration),
    Change(Change),
    Status(String),
    /// A day view: the day asked for (`None` for the day of the latest action), the date it shows,
    /// the count of log changes and the screen size it was computed for, the local hour its strip
    /// starts at, its text and its parts, and how long it took.
    Day(Option<i64>, i64, u32, (u16, u16), i64, String, Option<Arc<DayParts>>, Duration),
}

/// Sends what the background work found, and wakes the screen.
#[derive(Clone)]
struct Notify {
    tx: Sender<Note>,
    wake: Wake,
}

impl Notify {
    /// Whether the browser still takes notes.
    fn send(&self, note: Note) -> bool {
        let sent = self.tx.send(note).is_ok();
        (self.wake)();
        sent
    }
}

/// A day the browser computed.
struct DayText<T> {
    /// The local date it shows.
    date: i64,
    text: T,
    /// The text above the strip, the strip and its lanes, and the text below; none for a day
    /// that failed.
    parts: Option<Arc<DayParts>>,
    /// The count of log changes it was computed after, and the screen size it was laid out for.
    changes: u32,
    size: (u16, u16),
}

/// What the day worker is asked: the day, the count of log changes, the screen size, and the local
/// hour the strip starts at once a first day set it.
type DayJob = (Option<i64>, u32, (u16, u16), Option<i64>);

/// A day job, with the main logs the browser knows when it follows the log directories, which the
/// day reads instead of listing the directories again.
type DayAsk = (DayJob, Option<Vec<SessionRef>>);

/// A listed session and what is known about it.
struct Entry<T> {
    row: SessionRow,
    /// Counts the changes of the session's logs. Glances and X-rays carry the version they were
    /// computed from; an older one is shown until a newer one is ready.
    version: u32,
    glance: Option<Glance>,
    glanced: u32,
    glancing: bool,
    /// When the session may be costed again: one that keeps changing waits as long as its last
    /// costing took, so that it takes at most half of a core.
    glance_after: Instant,
    report: Option<T>,
    /// The X-ray the report was drawn from, for a screen that draws it itself.
    xray: Option<Arc<XRay>>,
    reported: u32,
    /// The session's main log is gone.
    gone: bool,
}

impl<T> Entry<T> {
    fn new(row: SessionRow) -> Entry<T> {
        Entry { row, version: 1, glance: None, glanced: 0, glancing: false, glance_after: Instant::now(), report: None, xray: None, reported: 0, gone: false }
    }

    /// Whether this is the entry of the session with this id: the id its row names, or the one
    /// its log's file name gives, which the day's lanes may name it by.
    fn is(&self, session: &str) -> bool {
        let r = &self.row;
        r.session_id == session || r.file.session_id == session || r.file.main_id == session
    }
}

pub struct Browser<T> {
    catalog: Arc<Catalog>,
    frontend: Frontend<T>,
    entries: Vec<Entry<T>>,
    /// Entries by the session's main log: its file, or its row in a store that holds many.
    by_log: HashMap<Locator, usize>,
    pricing: Arc<Pricing>,
    /// The X-ray asked for last.
    pending: Option<(usize, u32)>,
    /// When the X-ray of a session that changed may be computed again, as for glances.
    report_after: Instant,
    visible: Vec<usize>,
    /// The selected row of the list.
    selected: Option<usize>,
    /// The first row on the screen.
    top: usize,
    /// The first line of the right pane on the screen.
    scroll: usize,
    zoom: bool,
    filter: String,
    editing: bool,
    sort: Sort,
    status: String,
    notes: Receiver<Note>,
    notify: Notify,
    xray_tx: Sender<(usize, u32, SessionRef)>,
    /// The day shown in the right pane instead of the X-ray: `Some(None)` is the day of the
    /// latest action, `Some(Some(date))` another day.
    day: Option<Option<i64>>,
    /// The days computed, by the day asked for.
    days: HashMap<Option<i64>, DayText<T>>,
    /// Counts the changes of the log directories, after which the current day is computed again.
    changes: u32,
    day_pending: Option<DayJob>,
    /// When the current day may be computed again, as for X-rays.
    day_after: Instant,
    day_tx: Sender<DayAsk>,
    /// The list holds every main log the day may read: the log directories are followed, and
    /// no filter leaves a session out.
    knows_all: bool,
    /// The size of the pane the day is drawn in, when it was last drawn.
    pane: (u16, u16),
    /// The local hour the first day shown starts its strip at; later days start theirs there too.
    day_axis: Option<i64>,
    /// Whether the browser costs sessions in the background; a detached one does not.
    costs: bool,
    /// Counts the times the list was put in place again after the logs changed.
    relisted: u32,
    /// The screen shows a day beside the list at all times, as the window does (see
    /// `keep_day_beside`).
    beside: bool,
    /// The waiting option: the list may be sorted by waiting, and shows waits then.
    waiting: bool,
    /// The temporary folders, whose sessions never wait.
    temp: Arc<Vec<String>>,
}

impl<T> Browser<T> {
    /// Lists every session that `select` selects, starts the workers that compute X-rays and days,
    /// and costs every session in the background. With `follow`, it also follows the agents' log
    /// directories. Fails when no session is found.
    pub fn start(catalog: Arc<Catalog>, pricing: Pricing, select: Query, follow: bool, wake: Wake, frontend: Frontend<T>) -> Result<Browser<T>> {
        let (notes_tx, notes) = mpsc::channel();
        let notify = Notify { tx: notes_tx, wake };
        // Sessions that grow are read again and again: only what was appended is read.
        catalog.keep_logs();
        // The log directories are watched from before the sessions are listed, so that no change
        // is missed.
        let following = match follow {
            false => Ok(()),
            true => {
                let (notify, select) = (notify.clone(), select.clone());
                watch::watch(catalog.clone(), select.agents.clone(), move |change| {
                    // Only the sessions the filters select join the list.
                    let change = match change {
                        Change::Session(row) if !select.row(&row) => return true,
                        Change::All(rows) => Change::All(rows.into_iter().filter(|r| select.row(r)).collect()),
                        change => change,
                    };
                    notify.send(Note::Change(change))
                })
            }
        };
        let rows = xray::rows(&catalog, &select, usize::MAX);
        if rows.is_empty() {
            bail!("no session logs found");
        }
        let pricing = Arc::new(pricing);

        // Days, one at a time, as X-rays are.
        let (day_tx, day_rx) = mpsc::channel::<DayAsk>();
        {
            let (catalog, pricing, notify, select, layout) = (catalog.clone(), pricing.clone(), notify.clone(), select.clone(), frontend.day_layout);
            std::thread::spawn(move || {
                // A pool of its own, so that a day does not wait behind the costing of every session.
                let pool = crate::contain::pool(4);
                while let Ok(mut ask) = day_rx.recv() {
                    while let Ok(newer) = day_rx.try_recv() {
                        ask = newer;
                    }
                    let ((key, changes, size, axis), known) = ask;
                    let started = Instant::now();
                    let style = tokprof_core::fmt::Style::with_color(true);
                    let build = || crate::day::build(&catalog, &select, &pricing, key, now_ms(), layout(size), axis, &style, known.as_deref());
                    // A day that panicked fails as one that could not be read.
                    let built = match &pool {
                        Some(pool) => crate::contain::run_on(Some(pool), build),
                        None => crate::contain::run(build),
                    };
                    let (date, hour, text, parts) = match built.map_err(anyhow::Error::msg).and_then(|day| day) {
                        Ok(day) => (day.date, day.axis_hour, day.text, Some(Arc::new(day.parts))),
                        Err(e) => (key.unwrap_or_else(|| rhythm::date_of(now_ms(), &rhythm::local_offset)), axis.unwrap_or(0), format!("error: {e:#}"), None),
                    };
                    if !notify.send(Note::Day(key, date, changes, size, hour, text, parts, started.elapsed())) {
                        return;
                    }
                }
            });
        }

        // X-rays, one at a time; a newer request replaces the ones still waiting.
        let (xray_tx, xray_rx) = mpsc::channel::<(usize, u32, SessionRef)>();
        {
            let (catalog, pricing, notify) = (catalog.clone(), pricing.clone(), notify.clone());
            std::thread::spawn(move || {
                while let Ok(mut job) = xray_rx.recv() {
                    while let Ok(newer) = xray_rx.try_recv() {
                        job = newer;
                    }
                    let (i, version, file) = job;
                    let started = Instant::now();
                    // An X-ray that panicked fails as one that could not be computed.
                    let computed = crate::contain::run(|| {
                        xray::xray(&catalog, &file, &pricing, now_ms()).map(|x| {
                            let style = tokprof_core::fmt::Style::with_color(true);
                            (crate::render_xray::text(&x, &style, &crate::paths::profile_dir(), catalog.home()), Some(Arc::new(x)))
                        })
                    });
                    let (text, data) = match computed.map_err(anyhow::Error::msg).and_then(|x| x) {
                        Ok(x) => x,
                        Err(e) => (format!("error: {e:#}"), None),
                    };
                    if !notify.send(Note::Report(i, version, text, data, started.elapsed())) {
                        return;
                    }
                }
            });
        }

        let mut browser = Browser::new(catalog, rows, pricing, frontend, notes, notify, xray_tx, day_tx);
        match following {
            Err(e) => browser.status = format!("not following the logs: {e:#}"),
            Ok(()) => {
                let unfiltered = select.project.is_none() && select.session.is_none() && select.client.is_none();
                browser.knows_all = follow && unfiltered && select.since_ms.is_none() && select.until_ms.is_none();
            }
        }
        // Costs of every session, in the background.
        browser.request_glances();
        Ok(browser)
    }

    /// A browser of these rows that computes nothing in the background, for tests.
    pub fn detached(catalog: Arc<Catalog>, rows: Vec<SessionRow>, pricing: Pricing, frontend: Frontend<T>) -> Browser<T> {
        let (notes_tx, notes) = mpsc::channel();
        let (xray_tx, _) = mpsc::channel();
        let (day_tx, _) = mpsc::channel();
        let notify = Notify { tx: notes_tx, wake: Arc::new(|| {}) };
        let mut browser = Browser::new(catalog, rows, Arc::new(pricing), frontend, notes, notify, xray_tx, day_tx);
        browser.costs = false;
        browser
    }

    #[allow(clippy::too_many_arguments)]
    fn new(
        catalog: Arc<Catalog>,
        rows: Vec<SessionRow>,
        pricing: Arc<Pricing>,
        frontend: Frontend<T>,
        notes: Receiver<Note>,
        notify: Notify,
        xray_tx: Sender<(usize, u32, SessionRef)>,
        day_tx: Sender<DayAsk>,
    ) -> Browser<T> {
        let by_log = rows.iter().enumerate().map(|(i, r)| (r.file.locator.clone(), i)).collect();
        let mut browser = Browser {
            catalog,
            frontend,
            entries: rows.into_iter().map(Entry::new).collect(),
            by_log,
            pricing,
            pending: None,
            report_after: Instant::now(),
            visible: Vec::new(),
            selected: None,
            top: 0,
            scroll: 0,
            zoom: false,
            filter: String::new(),
            editing: false,
            sort: Sort::Time,
            status: String::new(),
            notes,
            notify,
            xray_tx,
            day: None,
            days: HashMap::new(),
            changes: 0,
            day_pending: None,
            day_after: Instant::now(),
            day_tx,
            knows_all: false,
            pane: (0, 0),
            day_axis: None,
            costs: true,
            relisted: 0,
            beside: false,
            waiting: false,
            temp: Arc::new(crate::day::temp_folders()),
        };
        browser.refilter();
        browser
    }

    /// Takes in what the background work sent, and asks for what is due: costs of sessions that
    /// changed, the selected session's X-ray, and the shown day. Returns whether anything
    /// arrived.
    pub fn tick(&mut self) -> bool {
        let arrived = self.absorb();
        self.request_glances();
        self.request_report();
        self.request_day();
        arrived
    }

    /// When `tick` would next ask for something it holds back now: the cost of a session that
    /// keeps changing, the X-ray of the selected one, or the current day, each of which waits as
    /// long as its last computation took. A screen that sleeps until something happens wakes
    /// then.
    pub fn next_due(&self) -> Option<Instant> {
        let now = Instant::now();
        let glance = self.entries.iter().filter(|e| !e.gone && !e.glancing && e.glanced < e.version && e.glance_after > now).map(|e| e.glance_after).min();
        let report = self.selected_entry().filter(|&i| {
            let e = &self.entries[i];
            e.reported != e.version && self.pending != Some((i, e.version)) && e.report.is_some() && now < self.report_after
        });
        let day = self.day.filter(|key| key.is_none() && self.days.get(key).is_some_and(|d| d.changes != self.changes) && now < self.day_after);
        [glance, report.map(|_| self.report_after), day.map(|_| self.day_after)].into_iter().flatten().min()
    }

    /// Waits up to 20 s for the X-ray of the selected session, then 0.5 s more for costs, for a
    /// screen that draws one frame and exits.
    pub fn settle(&mut self) {
        let deadline = Instant::now() + Duration::from_secs(20);
        self.request_report();
        while Instant::now() < deadline && self.selected_entry().is_some_and(|i| self.entries[i].report.is_none()) {
            std::thread::sleep(Duration::from_millis(50));
            self.absorb();
        }
        std::thread::sleep(Duration::from_millis(500));
        self.absorb();
    }

    /// Shows the day of `date` full size, as `d` and the arrows reach it; in the window's mode,
    /// beside the list, where the date of the latest action is that day, as Today shows it.
    pub fn show_day(&mut self, date: i64) {
        let latest = self.days.get(&None).map(|d| d.date);
        self.day = Some(if self.beside && latest == Some(date) { None } else { Some(date) });
        self.zoom = !self.beside;
        self.scroll = 0;
    }

    /// Puts the browser in the window's mode: a day is shown beside the list at all times, the
    /// day of the latest action to start with. Selecting a session shows its day, ← and → (and
    /// `h`, `l`, `[`, `]`) step the day instead of opening and closing the X-ray, `d` shows the
    /// selected session's day, ⏎ shows the X-ray full size and esc ends that.
    pub fn keep_day_beside(&mut self) {
        self.beside = true;
        self.day = Some(None);
        self.zoom = false;
    }

    /// In the window's mode, shows the selected session's day, even after the arrows went
    /// elsewhere, as `d` does; also while a filter is being typed, which it leaves as it is.
    pub fn show_selected_day(&mut self) {
        if self.beside {
            self.day = Some(None);
            self.follow_selection();
        }
    }

    /// Steps the day by `delta` days, as the buttons beside its name do.
    pub fn step_day_by(&mut self, delta: i64) {
        self.step_day(delta);
    }

    /// Clears the filter, typed or being typed, as a field's clear button does.
    pub fn clear_filter(&mut self) {
        self.editing = false;
        if !self.filter.is_empty() {
            self.filter.clear();
            self.refilter();
        }
    }

    /// Sets the filter to `text`, as typing it and ⏎ would; in the window's mode the day follows
    /// the session it leaves selected.
    pub fn set_filter(&mut self, text: &str) {
        self.editing = false;
        self.filter = text.to_string();
        self.refilter();
        self.follow_selection();
    }

    /// Counts the changes of the log directories the browser took in, for work that is done
    /// again after them.
    pub fn changes(&self) -> u32 {
        self.changes
    }

    /// Shows the day of the latest action again, as Today does.
    pub fn show_latest_day(&mut self) {
        if self.day.is_some() {
            self.day = Some(None);
            self.scroll = 0;
        }
    }

    /// Whether the day shown is the day of the latest action.
    pub fn day_is_latest(&self) -> bool {
        self.day == Some(None)
    }

    /// The noon of the date the day shown shows, or will once it is computed.
    pub fn day_noon(&self) -> i64 {
        self.shown_day().1
    }

    /// In the window's mode, shows the day of the selected session unless the day shown has its
    /// lane: the local date of its last activity five hours earlier, since a day runs from the
    /// middle of one night to the middle of the next.
    fn follow_selection(&mut self) {
        let Some(i) = self.selected_entry().filter(|_| self.beside) else { return };
        let shown = self.day_parts().and_then(|p| p.strip.as_ref()).is_some_and(|(_, lanes)| lanes.lanes.iter().any(|l| self.entries[i].is(&l.id)));
        if shown {
            return;
        }
        let date = rhythm::date_of(self.entries[i].row.active_ms - 5 * 3_600_000, &rhythm::local_offset);
        let latest = self.days.get(&None).map(|d| d.date);
        self.day = Some(if latest == Some(date) { None } else { Some(date) });
        self.scroll = 0;
    }

    /// Waits up to 20 s for the day shown, for a screen that draws one frame and exits. The day
    /// is laid out for the pane set last.
    pub fn settle_day(&mut self) {
        let deadline = Instant::now() + Duration::from_secs(20);
        self.request_day();
        while Instant::now() < deadline && self.day.is_some_and(|key| !self.days.contains_key(&key)) {
            std::thread::sleep(Duration::from_millis(50));
            self.absorb();
        }
    }

    /// The selected session's entry.
    fn selected_entry(&self) -> Option<usize> {
        self.selected.and_then(|s| self.visible.get(s).copied())
    }

    /// Sets the selected row; without one, the screen starts at the top again.
    fn select(&mut self, row: Option<usize>) {
        self.selected = row;
        if row.is_none() {
            self.top = 0;
        }
    }

    /// Takes in what the background work sent; returns whether anything arrived. It fills in
    /// cells and never moves a row that stays listed: rows move only when the user sorts or
    /// filters.
    fn absorb(&mut self) -> bool {
        let (mut listed, mut arrived) = (false, false);
        while let Ok(note) = self.notes.try_recv() {
            arrived = true;
            match note {
                Note::Glance(i, version, g, took) => {
                    let e = &mut self.entries[i];
                    e.glance = Some(g);
                    e.glanced = version;
                    e.glancing = false;
                    e.glance_after = Instant::now() + took;
                }
                Note::Report(i, version, text, data, took) => {
                    let e = &mut self.entries[i];
                    e.report = Some((self.frontend.show)(text));
                    e.xray = data;
                    e.reported = version;
                    self.report_after = Instant::now() + took;
                }
                Note::Change(change) => {
                    self.changes += 1;
                    listed |= self.apply(change);
                }
                Note::Status(s) => self.status = s,
                Note::Day(key, date, changes, size, hour, text, parts, took) => {
                    let text = (self.frontend.show)(text);
                    self.days.insert(key, DayText { date, text, parts, changes, size });
                    self.day_axis.get_or_insert(hour);
                    if self.day_pending.is_some_and(|p| (p.0, p.1, p.2) == (key, changes, size)) {
                        self.day_pending = None;
                    }
                    // Beside the list, the day is on the screen all the time and reads every log of
                    // the day, so it waits longer: the lanes need no second-by-second freshness.
                    let pause = if self.beside { (took * 4).max(BESIDE_DAY_PAUSE) } else { took };
                    self.day_after = Instant::now() + pause;
                }
            }
        }
        if listed {
            self.relist();
        }
        arrived
    }

    /// Takes in what the log directories changed without moving the rows that stay listed: a
    /// session that is gone takes its row with it, and a new one is put before the first row that
    /// the current order puts after it. Rows that come or go above the screen move the screen
    /// with them, so the rows on it stay where they are. When the selected session is gone, the
    /// cursor stays on its row.
    fn relist(&mut self) {
        self.relisted += 1;
        let (kept, at) = (self.selected_entry(), self.selected);
        let first = self.top;
        let mut offset = first;
        let listed: HashSet<usize> = self.visible.iter().copied().collect();
        let mut rows = Vec::with_capacity(self.visible.len());
        for (row, &i) in self.visible.iter().enumerate() {
            if self.shows(i) {
                rows.push(i);
            } else if row < first {
                offset -= 1;
            }
        }
        let keys = self.keys(now_ms());
        for i in (0..self.entries.len()).filter(|&i| !listed.contains(&i) && self.shows(i)) {
            let place = rows.iter().position(|&j| order(&keys, i, j) == Ordering::Less).unwrap_or(rows.len());
            if place < offset {
                offset += 1;
            }
            rows.insert(place, i);
        }
        self.visible = rows;
        self.top = offset;
        match kept.and_then(|k| self.visible.iter().position(|&i| i == k)) {
            Some(row) => self.select(Some(row)),
            None if self.visible.is_empty() => self.select(None),
            None => {
                self.select(Some(at.unwrap_or(0).min(self.visible.len() - 1)));
                self.scroll = 0;
            }
        }
    }

    /// Takes in a change of the log directories as the watcher sends it: the rows that stay
    /// listed keep their places (see `relist`).
    pub fn apply_change(&mut self, change: Change) {
        self.changes += 1;
        if self.apply(change) {
            self.relist();
        }
    }

    /// Takes in a change of the log directories; returns whether the list changed.
    fn apply(&mut self, change: Change) -> bool {
        let mut changed = false;
        match change {
            Change::Session(row) => changed = self.upsert(*row),
            Change::Gone(path) => {
                for e in self.entries.iter_mut().filter(|e| !e.gone && e.row.file.path().starts_with(&path)) {
                    e.gone = true;
                    changed = true;
                }
            }
            Change::Held { store, logs } => {
                let held: HashSet<&Locator> = logs.iter().collect();
                let of_store = |l: &Locator| matches!(l, Locator::Row { db, .. } if *db == store);
                for e in self.entries.iter_mut().filter(|e| !e.gone && of_store(&e.row.file.locator) && !held.contains(&e.row.file.locator)) {
                    e.gone = true;
                    changed = true;
                }
            }
            Change::All(rows) => {
                let found: HashSet<&Locator> = rows.iter().map(|r| &r.file.locator).collect();
                for e in self.entries.iter_mut().filter(|e| !e.gone && !found.contains(&e.row.file.locator)) {
                    e.gone = true;
                    changed = true;
                }
                for row in rows {
                    changed |= self.upsert(row);
                }
            }
        }
        changed
    }

    /// Adds a session, or takes in the new state of its logs; returns whether anything changed.
    fn upsert(&mut self, row: SessionRow) -> bool {
        // A Codex rollout read before its first line was written looked like a session of its
        // own; it is listed with the session that spawned it.
        let mut changed = false;
        for sub in &row.subagents {
            if let Some(&j) = self.by_log.get(&sub.locator)
                && !self.entries[j].gone
            {
                self.entries[j].gone = true;
                changed = true;
            }
        }
        let Some(&i) = self.by_log.get(&row.file.locator) else {
            self.by_log.insert(row.file.locator.clone(), self.entries.len());
            self.entries.push(Entry::new(row));
            return true;
        };
        let e = &mut self.entries[i];
        if !e.gone && e.row.file == row.file && e.row.subagents == row.subagents {
            return changed;
        }
        e.row = row;
        e.version += 1;
        e.gone = false;
        true
    }

    /// Costs the sessions that are new or changed since their last glance, on a worker thread.
    fn request_glances(&mut self) {
        if !self.costs {
            return;
        }
        let now = Instant::now();
        let due: Vec<(usize, u32, SessionRow)> = self
            .entries
            .iter_mut()
            .enumerate()
            .filter(|(_, e)| !e.gone && !e.glancing && e.glanced < e.version && e.glance_after <= now)
            .map(|(i, e)| {
                e.glancing = true;
                (i, e.version, e.row.clone())
            })
            .collect();
        if due.is_empty() {
            return;
        }
        let (catalog, pricing, notify, temp) = (self.catalog.clone(), self.pricing.clone(), self.notify.clone(), self.temp.clone());
        std::thread::spawn(move || {
            let _ = crate::contain::run(|| {
                due.into_par_iter().for_each_with(notify, |notify, (i, version, row)| {
                    let started = Instant::now();
                    // A glance that panicked counts as one whose logs could not be read, and the
                    // others of the batch go on.
                    let g = crate::contain::run(|| xray::glance(&catalog, &row, &pricing, now_ms(), &temp))
                        .unwrap_or_else(|_| Glance { cold_at_ms: i64::MAX, ..Glance::default() });
                    notify.send(Note::Glance(i, version, g, started.elapsed()));
                });
            });
        });
    }

    fn request_report(&mut self) {
        let Some(i) = self.selected_entry() else { return };
        let e = &self.entries[i];
        if e.reported == e.version || self.pending == Some((i, e.version)) {
            return;
        }
        if e.report.is_some() && Instant::now() < self.report_after {
            return;
        }
        self.pending = Some((i, e.version));
        let _ = self.xray_tx.send((i, e.version, e.row.file.clone()));
    }

    /// Asks for the shown day when it has no view for the pane's width yet, or when it is the
    /// current day and a log changed since, once a pause as long as its last computation passed.
    fn request_day(&mut self) {
        let Some(key) = self.day else { return };
        let size = self.pane;
        if size.0 == 0 || self.day_pending.is_some_and(|p| (p.0, p.1, p.2) == (key, self.changes, size)) {
            return;
        }
        if let Some(d) = self.days.get(&key)
            && d.size == size
            && (key.is_some() || d.changes == self.changes || Instant::now() < self.day_after)
        {
            return;
        }
        let job = (key, self.changes, size, self.day_axis);
        self.day_pending = Some(job);
        let _ = self.day_tx.send((job, self.known_logs()));
    }

    /// The main logs of the listed sessions and of those a filter hides, when they are every main
    /// log the day may read, largest first as the log directories list them.
    fn known_logs(&self) -> Option<Vec<SessionRef>> {
        if !self.knows_all {
            return None;
        }
        let mut logs: Vec<SessionRef> = self.entries.iter().filter(|e| !e.gone).map(|e| e.row.file.clone()).collect();
        logs.sort_by(|a, b| b.size.cmp(&a.size).then_with(|| a.path().cmp(b.path())));
        Some(logs)
    }

    /// Steps the shown day by `delta` days, never past the day of the latest action.
    fn step_day(&mut self, delta: i64) {
        let Some(key) = self.day else { return };
        let latest = self.days.get(&None).map(|d| d.date);
        let Some(current) = key.or(latest) else { return };
        let next = current + delta;
        self.day = Some(match latest {
            Some(l) if next >= l => None,
            None if delta > 0 => key,
            _ => Some(next),
        });
        self.scroll = 0;
    }

    /// Whether entry `i` is listed: its main log is there and it matches the filter.
    fn shows(&self, i: usize) -> bool {
        let (e, r) = (&self.entries[i], &self.entries[i].row);
        !e.gone && (self.filter.is_empty() || format!("{} {} {}", r.file.agent.name(), r.project, r.id).to_lowercase().contains(&self.filter.to_lowercase()))
    }

    fn refilter(&mut self) {
        let keep = self.selected_entry();
        self.visible = (0..self.entries.len()).filter(|&i| self.shows(i)).collect();
        self.sort_keeping(keep);
    }

    /// Sorts the list again in its order. The list then shows from its first row, as far as
    /// the selection allows, so that the top of the new order is on the screen.
    fn sort(&mut self) {
        self.sort_keeping(self.selected_entry());
        self.top = 0;
    }

    /// The value of each entry that the current order sorts by, at `now`; an unknown one is -1.
    fn keys(&self, now: i64) -> Vec<f64> {
        let key = |e: &Entry<T>| match self.sort {
            Sort::Time => e.row.active_ms as f64,
            Sort::Cost => e.glance.as_ref().map_or(-1.0, |g| g.cost),
            Sort::Next => e.glance.as_ref().and_then(|g| g.next(now)).map_or(-1.0, |(usd, _)| usd),
            // A wait sorts above every time, the oldest first; the others follow by time.
            Sort::Waiting => match &e.glance {
                None => -1.0,
                Some(g) => match g.waiting_since.filter(|&w| rhythm::waits_at(w, now)) {
                    Some(since) => 2.0 * now as f64 - since as f64,
                    None => e.row.active_ms as f64,
                },
            },
        };
        self.entries.iter().map(key).collect()
    }

    fn sort_keeping(&mut self, keep: Option<usize>) {
        let keys = self.keys(now_ms());
        self.visible.sort_by(|&a, &b| order(&keys, a, b));
        let at = keep.and_then(|k| self.visible.iter().position(|&i| i == k));
        self.select(if self.visible.is_empty() { None } else { Some(at.unwrap_or(0)) });
    }

    /// Whether sorting again at `now` would change the order of the list.
    fn out_of_date(&self, now: i64) -> bool {
        let keys = self.keys(now);
        self.visible.windows(2).any(|w| order(&keys, w[0], w[1]) == Ordering::Greater)
    }

    fn move_by(&mut self, delta: isize) {
        if self.visible.is_empty() {
            return;
        }
        let at = self.selected.unwrap_or(0) as isize + delta;
        self.select(Some(at.clamp(0, self.visible.len() as isize - 1) as usize));
        self.scroll = 0;
    }

    /// Handles a key; returns false to quit.
    pub fn on_key(&mut self, key: Key, view: Viewport) -> bool {
        let (before, typing) = (self.selected_entry().map(|i| self.entries[i].row.session_id.clone()), self.editing);
        let go_on = match self.beside_key(key) {
            Some(go_on) => go_on,
            None => self.key(key, view),
        };
        // The day follows a session the user moved to, not the first one a filter being typed
        // happens to leave.
        if !typing && self.selected_entry().map(|i| &self.entries[i].row.session_id) != before.as_ref() {
            self.follow_selection();
        }
        go_on
    }

    /// The keys that mean something else in the window's mode, where the day is always beside
    /// the list; `None` for the others and outside that mode.
    fn beside_key(&mut self, key: Key) -> Option<bool> {
        if !self.beside || self.editing || key == Key::Interrupt {
            return None;
        }
        match key {
            Key::Left | Key::Char('h') | Key::Char('[') => self.step_day(-1),
            Key::Right | Key::Char('l') | Key::Char(']') => self.step_day(1),
            Key::Char('d') => self.show_selected_day(),
            Key::Enter => self.zoom = true,
            Key::Backspace => self.zoom = false,
            Key::Esc if self.zoom => self.zoom = false,
            Key::Esc if !self.filter.is_empty() => {
                self.filter.clear();
                self.refilter();
            }
            Key::Esc => return Some(false),
            _ => return None,
        }
        Some(true)
    }

    /// What a key does outside the window's mode, and in it for the keys that mean the same.
    fn key(&mut self, key: Key, view: Viewport) -> bool {
        if key == Key::Interrupt {
            return false;
        }
        if self.editing {
            match key {
                Key::Char(c) => self.filter.push(c),
                Key::Backspace => {
                    self.filter.pop();
                }
                Key::Enter => self.editing = false,
                Key::Esc => {
                    self.filter.clear();
                    self.editing = false;
                }
                _ => return true,
            }
            self.refilter();
            return true;
        }
        let page = view.page.max(1);
        match key {
            Key::Char('q') => return false,
            // The day takes the whole screen; these go back to the list with the X-ray, and the
            // arrows step through days.
            Key::Char('d') | Key::Esc | Key::Backspace if self.day.is_some() => {
                self.day = None;
                self.zoom = false;
                self.scroll = 0;
            }
            Key::Left | Key::Char('h') | Key::Char('[') if self.day.is_some() => self.step_day(-1),
            Key::Right | Key::Char('l') | Key::Char(']') if self.day.is_some() => self.step_day(1),
            Key::Esc if self.zoom => self.zoom = false,
            Key::Esc if !self.filter.is_empty() => {
                self.filter.clear();
                self.refilter();
            }
            Key::Esc => return false,
            Key::Up | Key::Char('k') if self.zoom => self.scroll = self.scroll.saturating_sub(1),
            Key::Down | Key::Char('j') if self.zoom => self.scroll = self.scroll.saturating_add(1).min(self.max_scroll(view.lines)),
            Key::PageUp if self.zoom => self.scroll = self.scroll.saturating_sub(page),
            Key::PageDown | Key::Char(' ') if self.zoom => self.scroll = self.scroll.saturating_add(page).min(self.max_scroll(view.lines)),
            Key::Up | Key::Char('k') => self.move_by(-1),
            Key::Down | Key::Char('j') => self.move_by(1),
            Key::PageUp => self.move_by(-(page as isize)),
            Key::PageDown => self.move_by(page as isize),
            Key::Home | Key::Char('g') => self.move_by(isize::MIN / 2),
            Key::End | Key::Char('G') => self.move_by(isize::MAX / 2),
            Key::Enter | Key::Right | Key::Char('l') => self.zoom = true,
            Key::Left | Key::Char('h') | Key::Backspace => self.zoom = false,
            Key::Char('/') => self.editing = true,
            Key::Char('s') => {
                self.sort = self.sort.following(self.waiting);
                self.sort();
            }
            Key::Char('r') => self.sort(),
            Key::Char('f') => self.export("jfr", false),
            Key::Char('o') => self.export("html", true),
            Key::Char('c') => self.handoff(),
            Key::Char('d') => {
                self.day = Some(None);
                self.zoom = true;
                self.scroll = 0;
            }
            _ => {}
        }
        true
    }

    /// Copies the line that starts a new session from the selected one.
    fn handoff(&mut self) {
        let Some(i) = self.selected_entry() else { return };
        let row = &self.entries[i].row;
        self.status = match crate::clipboard::copy(&tokprof_core::handoff::line(&self.catalog, &row.file)) {
            Some(_) => format!("copied the handoff line for {}: paste it into a new session", row.id),
            None => format!("no clipboard tool ({}); run: tokprof handoff {}", crate::clipboard::TOOLS, row.id),
        };
    }

    /// How far the full-screen X-ray or day scrolls: until its last line is at the bottom of a
    /// pane of `shown` lines.
    fn max_scroll(&self, shown: usize) -> usize {
        let lines = match self.day {
            Some(key) => self.days.get(&key).map_or(0, |d| (self.frontend.lines)(&d.text)),
            None => self.selected_entry().and_then(|i| self.entries[i].report.as_ref()).map_or(0, |t| (self.frontend.lines)(t)),
        };
        lines.saturating_sub(shown)
    }

    /// Writes the selected session's profile on a worker thread; HTML opens in the browser.
    fn export(&mut self, ext: &str, open: bool) {
        let Some(i) = self.selected_entry() else { return };
        let row = self.entries[i].row.clone();
        let dir = crate::paths::profile_dir();
        let path = dir.join(format!("{}.{ext}", row.id));
        self.status = format!("writing {}…", display_path(&path.to_string_lossy(), None, self.catalog.home()));
        let (catalog, pricing, notify) = (self.catalog.clone(), self.pricing.clone(), self.notify.clone());
        std::thread::spawn(move || {
            // An export that panicked fails as one that could not be written.
            let written = crate::contain::run_on(None, || {
                std::fs::create_dir_all(&dir).map_err(anyhow::Error::from).and_then(|_| xray::export(&catalog, &row, &pricing, &path))
            });
            let note = written
                .map_err(anyhow::Error::msg)
                .and_then(|w| w)
                .map(|written| {
                    if open {
                        let (program, target) = opener(&path);
                        let _ = std::process::Command::new(program).arg(target).spawn();
                    }
                    format!("wrote {written}")
                })
                .unwrap_or_else(|e| format!("export failed: {e:#}"));
            notify.send(Note::Status(note));
        });
    }

    /// The count of listed sessions.
    pub fn len(&self) -> usize {
        self.visible.len()
    }

    pub fn is_empty(&self) -> bool {
        self.visible.is_empty()
    }

    /// The cells of row `row` of the list at `now`.
    pub fn cells(&self, row: usize, now: i64) -> Cells<'_> {
        let r = &self.entries[self.visible[row]].row;
        let g = self.entries[self.visible[row]].glance.as_ref();
        let time = match self.wait_of(self.visible[row], now) {
            Some(wait) => format!("waits {}", rhythm::duration(wait)),
            None => local_time(r.active_ms, "%m-%d %H:%M"),
        };
        Cells {
            time,
            agent: r.file.agent.name(),
            agent_place: self.catalog.order(&r.file.agent).0,
            project: &r.project,
            cost: g.map_or_else(|| "…".to_string(), |g| usd(g.cost)),
            usd: g.map(|g| g.cost),
            next: g.and_then(|g| crate::render_sessions::next_cell(g, now)),
            title: g.and_then(|g| g.title.as_deref()),
            active_ms: r.active_ms,
            warm_until: g.and_then(|g| g.next(now)).filter(|(_, cold)| !cold).and(g.map(|g| g.cold_at_ms)),
            next_usd: g.and_then(|g| g.next(now)).map(|(usd, _)| usd),
        }
    }

    /// The session id of row `row`, which names its row while rows come and go.
    pub fn session(&self, row: usize) -> &str {
        &self.entries[self.visible[row]].row.session_id
    }

    /// The row of the session with this id.
    /// The row of the session with this id: the id its row names, or the one its log's file name
    /// gives, which the day's lanes may name it by.
    pub fn row_of(&self, session: &str) -> Option<usize> {
        self.visible.iter().position(|&i| self.entries[i].is(session))
    }

    /// The row of the session that `agent` shows by this short id, as the reports name sessions.
    pub fn row_of_short(&self, agent: &tokprof_core::source::AgentId, short: &str) -> Option<usize> {
        self.visible.iter().position(|&i| {
            let r = &self.entries[i].row;
            r.file.agent == *agent && (r.id == short || tokprof_core::model::short_id(agent, &r.session_id) == short)
        })
    }

    /// When the session with this id sent its main conversation's latest request and when its
    /// cache goes or went cold, in epoch ms, once its glance knows; listed or not.
    pub fn cache_life(&self, session: &str) -> Option<(i64, i64)> {
        let g = self.entries.iter().find(|e| !e.gone && e.is(session))?.glance.as_ref()?;
        (g.last_ms > 0 && g.cold_at_ms != i64::MAX).then_some((g.last_ms, g.cold_at_ms))
    }

    /// When the session of row `row` was last active, in epoch ms.
    pub fn active_ms(&self, row: usize) -> i64 {
        self.entries[self.visible[row]].row.active_ms
    }

    /// The total of the costs known of the listed sessions, and how many are still being costed.
    pub fn known_total(&self) -> (f64, usize) {
        let known = self.visible.iter().filter_map(|&i| self.entries[i].glance.as_ref().map(|g| g.cost)).sum();
        (known, self.visible.iter().filter(|&&i| self.entries[i].glance.is_none()).count())
    }

    /// How long the session of entry `i` has waited for the user at `now`, while the list is
    /// sorted by waiting and the wait is within the waiting limit.
    fn wait_of(&self, i: usize, now: i64) -> Option<i64> {
        if self.sort != Sort::Waiting {
            return None;
        }
        let since = self.entries[i].glance.as_ref()?.waiting_since?;
        rhythm::waits_at(since, now).then_some((now - since).max(0))
    }

    /// How long each listed session that waits for the user has waited at `now`, while the list
    /// is sorted by waiting, by row.
    pub fn wait(&self, row: usize, now: i64) -> Option<i64> {
        self.wait_of(self.visible[row], now)
    }

    /// How many listed sessions wait for the user at `now`.
    pub fn waiting_count(&self, now: i64) -> usize {
        self.visible.iter().filter(|&&i| self.entries[i].glance.as_ref().and_then(|g| g.waiting_since).is_some_and(|w| rhythm::waits_at(w, now))).count()
    }

    /// The waiting option: whether the list may be sorted by waiting. Turning it off while the
    /// list is sorted by waiting sorts it by time.
    pub fn set_waiting(&mut self, on: bool) {
        self.waiting = on;
        if !on && self.sort == Sort::Waiting {
            self.sort_by(Sort::Time);
        }
    }

    pub fn waiting(&self) -> bool {
        self.waiting
    }

    /// The orders `s` cycles through.
    pub fn sorts(&self) -> &'static [Sort] {
        if self.waiting { &Sort::ALL } else { &Sort::ALL[..3] }
    }

    /// When the wait a waiting row shows next changes its text or passes the waiting limit, after
    /// `now`, while the list is sorted by waiting.
    pub fn next_wait_change(&self, now: i64) -> Option<i64> {
        if self.sort != Sort::Waiting {
            return None;
        }
        self.visible
            .iter()
            .filter_map(|&i| self.entries[i].glance.as_ref()?.waiting_since)
            .filter(|&since| rhythm::waits_at(since, now))
            .map(|since| {
                let wait = (now - since).max(0);
                // The span reads in seconds, tenths of minutes, minutes, then tenths of hours.
                let step = match wait {
                    w if w < 60_000 => 1_000,
                    w if w < 600_000 => 6_000,
                    w if w < 3_600_000 => 60_000,
                    _ => 360_000,
                };
                (since + (wait / step + 1) * step).min(since + rhythm::WAITING_LIMIT_MS)
            })
            .min()
    }

    /// Takes this glance for the session with this id, as the costing would give it; for the
    /// tests of a screen, whose browser costs nothing itself.
    #[doc(hidden)]
    pub fn put_glance(&mut self, session: &str, glance: Glance) {
        if let Some(e) = self.entries.iter_mut().find(|e| e.is(session)) {
            e.glance = Some(glance);
        }
    }

    /// The list's order.
    pub fn sort_order(&self) -> Sort {
        self.sort
    }

    /// Orders the list by `sort`, as `s` does until it gets there; the selection stays.
    pub fn sort_by(&mut self, sort: Sort) {
        self.sort = sort;
        self.sort();
    }

    /// The X-ray of the selected session as computed last, for a screen that draws it itself.
    pub fn xray(&self) -> Option<&XRay> {
        self.selected_entry().and_then(|i| self.entries[i].xray.as_deref())
    }

    /// The same X-ray, shared: a screen that keys what it built from it by the X-ray can hold
    /// this, so that no later X-ray takes its place in memory and its key.
    pub fn xray_shared(&self) -> Option<Arc<XRay>> {
        self.selected_entry().and_then(|i| self.entries[i].xray.clone())
    }

    /// Selects row `row`, as a click on it does; its X-ray shows from its top.
    pub fn select_row(&mut self, row: usize) {
        if row < self.visible.len() {
            self.select(Some(row));
            self.scroll = 0;
            self.follow_selection();
        }
    }

    /// Shows the selected session's X-ray full size, as ⏎ does.
    pub fn zoom_in(&mut self) {
        self.zoom = true;
    }

    /// The dearest of the costs known of the listed sessions.
    pub fn dearest(&self) -> Option<f64> {
        self.visible.iter().filter_map(|&i| self.entries[i].glance.as_ref().map(|g| g.cost)).max_by(f64::total_cmp)
    }

    /// The selected row of the list.
    pub fn selected(&self) -> Option<usize> {
        self.selected
    }

    /// The first row on the screen.
    pub fn top(&self) -> usize {
        self.top
    }

    /// Counts the times the list was put in place again after the logs changed; a screen whose
    /// own scrolling would keep other rows on it puts the first row back on `top` then.
    pub fn relisted(&self) -> u32 {
        self.relisted
    }

    /// Takes in the first row the screen shows after it drew the list.
    pub fn set_top(&mut self, top: usize) {
        self.top = top;
    }

    /// The first line of the right pane on the screen.
    pub fn scroll(&self) -> usize {
        self.scroll
    }

    /// Whether the right pane takes the whole screen.
    pub fn zoomed(&self) -> bool {
        self.zoom
    }

    /// Whether the right pane shows the day instead of the X-ray.
    pub fn showing_day(&self) -> bool {
        self.day.is_some()
    }

    /// Takes in the size of the pane the right pane was drawn in, which the day is laid out for.
    pub fn set_pane(&mut self, size: (u16, u16)) {
        self.pane = size;
    }

    /// The size of the pane the right pane was last drawn in.
    pub fn pane(&self) -> (u16, u16) {
        self.pane
    }

    /// Whether a filter is being typed.
    pub fn editing(&self) -> bool {
        self.editing
    }

    pub fn filter(&self) -> &str {
        &self.filter
    }

    /// The latest status message.
    pub fn status(&self) -> &str {
        &self.status
    }

    /// The list's title, like ` 756 sessions · $2,065 · by time · costing 396… · next = one more
    /// request `, where the amount is the total of the costs known so far.
    pub fn list_title(&self, now: i64) -> String {
        let known: f64 = self.visible.iter().filter_map(|&i| self.entries[i].glance.as_ref().map(|g| g.cost)).sum();
        let pending = self.visible.iter().filter(|&&i| self.entries[i].glance.is_none()).count();
        let waits = match (self.sort, self.waiting_count(now)) {
            (Sort::Waiting, 0) => ", none waits".to_string(),
            (Sort::Waiting, n) => format!(", {n} wait"),
            _ => String::new(),
        };
        format!(
            " {} sessions · {} · by {}{waits}{}{} · next = one more request ",
            self.visible.len(),
            usd(known),
            self.sort.name(),
            if self.out_of_date(now) { ", r re-sorts" } else { "" },
            if pending > 0 { format!(" · costing {pending}…") } else { String::new() }
        )
    }

    /// The X-ray pane's title: ` <agent> <id> · <project> `, with ` · updating…` while a newer
    /// X-ray is being computed.
    pub fn report_title(&self) -> String {
        match self.selected_entry() {
            Some(i) => {
                let (e, r) = (&self.entries[i], &self.entries[i].row);
                // The session grew since this X-ray; a new one follows.
                let updating = if e.report.is_some() && e.reported < e.version { " · updating…" } else { "" };
                format!(" {} {} · {}{updating} ", r.file.agent.name(), r.id, r.project)
            }
            None => " X-ray ".to_string(),
        }
    }

    /// What the X-ray pane shows.
    pub fn report(&self) -> Pane<'_, T> {
        match self.selected_entry() {
            Some(i) => {
                let (e, r) = (&self.entries[i], &self.entries[i].row);
                match &e.report {
                    Some(text) => Pane::Text(text),
                    None => Pane::Computing(format!("computing the X-ray of {} ({:.1} MB)…", r.id, r.bytes as f64 / 1e6)),
                }
            }
            None if self.filter.is_empty() => Pane::Empty("no sessions"),
            None => Pane::Empty("no session matches the filter"),
        }
    }

    /// The day shown, if computed, and the noon of the date it shows or will show.
    fn shown_day(&self) -> (Option<&DayText<T>>, i64) {
        let key = self.day.flatten();
        let shown = self.day.and_then(|k| self.days.get(&k));
        let offset = &rhythm::local_offset;
        let date = shown.map(|d| d.date).or(key).unwrap_or_else(|| rhythm::date_of(now_ms(), offset));
        (shown, rhythm::midnight(date, offset) + 12 * 3_600_000)
    }

    /// The day pane's title, like ` day · Tue 2026-11-03 `, with ` · updating…` while a newer
    /// view of the day is being computed.
    pub fn day_title(&self) -> String {
        let (shown, noon) = self.shown_day();
        let updating = if shown.is_some() && self.day_pending.is_some_and(|p| Some(p.0) == self.day) { " · updating…" } else { "" };
        format!(" day · {}{updating} ", local_time(noon, "%a %Y-%m-%d"))
    }

    /// The parts of the day shown, once it is computed: the text above its strip, the strip with
    /// its lanes, and the text below.
    pub fn day_parts(&self) -> Option<&DayParts> {
        self.shown_day().0.and_then(|d| d.parts.as_deref())
    }

    /// Selects the session with this id and goes back to the list with its X-ray, as a click on
    /// its lane in the day does. Returns whether it is listed.
    pub fn open_session(&mut self, session: &str) -> bool {
        let Some(row) = self.row_of(session) else { return false };
        if !self.beside {
            self.day = None;
        }
        self.zoom = false;
        self.select_row(row);
        true
    }

    /// What the day pane shows.
    pub fn day_pane(&self) -> Pane<'_, T> {
        match self.shown_day() {
            (Some(d), _) => Pane::Text(&d.text),
            (None, noon) => Pane::Computing(format!("computing the day of {}…", local_time(noon, "%Y-%m-%d"))),
        }
    }

    /// The keys the footer names in the view shown.
    pub fn footer_keys(&self) -> &'static str {
        if self.day.is_some() {
            " ↑↓ PgUp PgDn scroll · ← → days · esc back · f JFR · o HTML · c handoff · q quit"
        } else if self.zoom {
            " ↑↓ PgUp PgDn scroll · ← back · f JFR · o HTML · c handoff · q quit · d day"
        } else {
            if self.waiting {
                " ↑↓ move · ⏎ zoom · / filter · s sort: time, cost, next, waiting · r re-sort · f JFR for IntelliJ · o HTML · c handoff · q quit · d day"
            } else {
                " ↑↓ move · ⏎ zoom · / filter · s sort: time, cost, next · r re-sort · f JFR for IntelliJ · o HTML · c handoff · q quit · d day"
            }
        }
    }

    /// When the next-request cell of a listed session turns to the cold cost after `now`: the
    /// earliest time a listed session's prompt cache goes cold.
    pub fn next_cold(&self, now: i64) -> Option<i64> {
        self.visible
            .iter()
            .filter_map(|&i| self.entries[i].glance.as_ref())
            .filter(|g| g.next_warm.is_some() || g.next_cold.is_some())
            .map(|g| g.cold_at_ms)
            .filter(|&at| at > now && at != i64::MAX)
            .min()
    }
}

/// The command that opens a file with the program registered for it, with the path as it takes
/// it: `open` on macOS, `explorer` on Windows, which misreads `/` in a path, and `xdg-open`
/// elsewhere.
fn opener(path: &Path) -> (&'static str, PathBuf) {
    if cfg!(target_os = "macos") {
        ("open", path.to_path_buf())
    } else if cfg!(windows) {
        ("explorer", PathBuf::from(path.to_string_lossy().replace('/', "\\")))
    } else {
        ("xdg-open", path.to_path_buf())
    }
}

/// The order of the list: the larger value first, then the session listed first.
fn order(keys: &[f64], a: usize, b: usize) -> Ordering {
    keys[b].total_cmp(&keys[a]).then(a.cmp(&b))
}

pub fn now_ms() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis() as i64)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Session `i` in `project`, its log last written `at` seconds after the epoch.
    fn row(i: usize, project: &str, at: u64) -> SessionRow {
        SessionRow {
            file: SessionRef {
                source: "test",
                agent: tokprof_core::source::AgentId::unknown("claude"),
                locator: tokprof_core::source::Locator::File(format!("/x/{i}.jsonl").into()),
                session_id: format!("id{i}"),
                main_id: format!("id{i}"),
                subagent: false,
                size: 1 + at,
                modified_ns: at as i128 * 1_000_000_000,
                created_ms: None,
            },
            subagents: Vec::new(),
            project: project.to_string(),
            id: format!("id{i}"),
            session_id: format!("id{i}"),
            bytes: 1 + at,
            active_ms: at as i64 * 1000,
            client: None,
        }
    }

    /// Session `i`'s row, its log last written `written` seconds after the epoch and the session
    /// last active `active` seconds after it, as when bookkeeping is written to an idle session.
    fn resumed(i: usize, project: &str, written: u64, active: u64) -> SessionRow {
        SessionRow { active_ms: active as i64 * 1000, ..row(i, project, written) }
    }

    /// Texts kept as they arrive.
    fn plain() -> Frontend<String> {
        Frontend { show: |s| s, lines: |t| t.lines().count(), day_layout: |_| rhythm::Layout::TERMINAL }
    }

    fn app_of(rows: Vec<SessionRow>) -> Browser<String> {
        Browser::detached(Arc::new(Catalog::new(Vec::new(), PathBuf::new())), rows, Pricing::defaults(), plain())
    }

    fn app(projects: &[&str]) -> Browser<String> {
        app_of(projects.iter().enumerate().map(|(i, p)| row(i, p, 0)).collect())
    }

    /// 30 sessions, session `i` last active `1000 - i` seconds after the epoch: listed 0, 1, 2…
    fn thirty() -> Browser<String> {
        app_of((0..30).map(|i| row(i, "~/p", 1000 - i as u64)).collect())
    }

    /// A key on a screen 30 rows high: pages of 26 rows, and a right pane of 27 lines.
    fn press(app: &mut Browser<String>, key: Key) -> bool {
        app.on_key(key, Viewport { page: 26, lines: 27 })
    }

    fn take(app: &mut Browser<String>, change: Change) {
        app.notify.tx.send(Note::Change(change)).unwrap();
        app.absorb();
    }

    #[test]
    fn a_page_opens_with_the_platform_s_opener() {
        let (program, path) = opener(Path::new("C:/Users/me/tokprof/a.html"));
        if cfg!(target_os = "macos") {
            assert_eq!((program, path), ("open", PathBuf::from("C:/Users/me/tokprof/a.html")));
        } else if cfg!(windows) {
            assert_eq!((program, path), ("explorer", PathBuf::from(r"C:\Users\me\tokprof\a.html")));
        } else {
            assert_eq!((program, path), ("xdg-open", PathBuf::from("C:/Users/me/tokprof/a.html")));
        }
    }

    /// Session `i` of a store that holds many, last written `at` seconds after the epoch.
    fn stored(i: usize, at: u64) -> SessionRow {
        let mut r = row(i, "~/p", at);
        r.file.locator = Locator::Row { db: "/x/store.db".into(), id: format!("id{i}") };
        r
    }

    #[test]
    fn sessions_of_one_store_are_rows_of_their_own() {
        let mut a = app_of(vec![stored(0, 0), stored(1, 5)]);
        assert_eq!(a.entries.len(), 2);
        take(&mut a, Change::Session(Box::new(stored(0, 10))));
        assert_eq!((a.entries.len(), a.entries[0].version, a.entries[1].version), (2, 2, 1), "only the session that changed");
        take(&mut a, Change::Session(Box::new(stored(2, 20))));
        assert_eq!(a.entries.len(), 3, "a new session of the store joins the list");
        let held = |ids: &[usize]| ids.iter().map(|i| Locator::Row { db: "/x/store.db".into(), id: format!("id{i}") }).collect();
        take(&mut a, Change::Held { store: "/x/other.db".into(), logs: Vec::new() });
        assert!(a.entries.iter().all(|e| !e.gone), "another store's sessions stay");
        take(&mut a, Change::Held { store: "/x/store.db".into(), logs: held(&[0, 1, 2]) });
        assert!(a.entries.iter().all(|e| !e.gone));
        take(&mut a, Change::Held { store: "/x/store.db".into(), logs: held(&[0, 2]) });
        assert_eq!(a.entries.iter().map(|e| e.gone).collect::<Vec<_>>(), [false, true, false], "a session deleted from its store leaves");
        take(&mut a, Change::All(vec![stored(0, 10), stored(2, 20)]));
        assert_eq!(a.entries.iter().map(|e| e.gone).collect::<Vec<_>>(), [false, true, false], "a rescan leaves out the session no longer there");
    }

    #[test]
    fn the_list_follows_the_logs() {
        let mut a = app(&["~/src/a", "~/src/b"]);
        press(&mut a, Key::Down);
        a.entries[1].report = Some("X-ray of b".to_string());
        a.entries[1].reported = 1;

        take(&mut a, Change::Session(Box::new(row(2, "~/src/c", 10))));
        assert_eq!((a.visible.clone(), a.selected_entry()), (vec![2, 0, 1], Some(1)), "a new session goes on top; the cursor stays on its session");

        take(&mut a, Change::Session(Box::new(row(0, "~/src/a", 0))));
        assert_eq!(a.entries[0].version, 1, "an unchanged session stays as it is");

        take(&mut a, Change::Session(Box::new(row(1, "~/src/b", 20))));
        assert_eq!((a.visible.clone(), a.selected_entry()), (vec![2, 0, 1], Some(1)), "a session that grew keeps its row");
        assert_eq!(a.entries[1].row.active_ms, 20_000, "with its new time");
        assert_eq!((a.entries[1].version, a.entries[1].reported), (2, 1));
        a.request_report();
        assert_eq!(a.pending, Some((1, 2)), "and is X-rayed again");

        press(&mut a, Key::Home);
        take(&mut a, Change::Gone("/x/2.jsonl".into()));
        assert_eq!((a.visible.clone(), a.selected_entry()), (vec![0, 1], Some(0)), "a gone session leaves the cursor where it was");

        take(&mut a, Change::All(vec![row(0, "~/src/a", 0), row(2, "~/src/c", 30)]));
        assert_eq!(a.visible, vec![2, 0], "a rescan drops the sessions that are gone and brings back the ones that are there");
        assert_eq!(a.entries[2].version, 2);

        let mut parent = row(0, "~/src/a", 40);
        parent.subagents.push(row(2, "~/src/c", 30).file);
        take(&mut a, Change::Session(Box::new(parent)));
        assert_eq!(a.visible, vec![0], "a log that turns out to be a subagent's leaves the list");
    }

    #[test]
    fn rows_stay_in_place() {
        let mut a = thirty();
        let listed: Vec<usize> = (0..30).collect();
        assert_eq!(a.visible, listed);

        take(&mut a, Change::Session(Box::new(resumed(12, "~/p", 2000, 1000 - 12))));
        assert_eq!(a.visible, listed, "a change that keeps the last activity moves nothing");
        assert_eq!(a.entries[12].version, 2, "the session is costed again");
        assert!(!a.out_of_date(now_ms()), "and the order still holds");

        take(&mut a, Change::Session(Box::new(row(12, "~/p", 3000))));
        assert_eq!(a.visible, listed, "a session that works keeps its row");
        assert_eq!(a.entries[12].row.active_ms, 3_000_000, "and its time updates");

        let before = a.visible.clone();
        let rows: Vec<SessionRow> = a.entries.iter().map(|e| e.row.clone()).collect();
        take(&mut a, Change::All(rows));
        assert_eq!(a.visible, before, "a rescan after dropped events keeps the listed rows in place");

        press(&mut a, Key::Char('s'));
        let by_cost = a.visible.clone();
        for i in [29, 5, 17] {
            let g = xray::Glance { cost: i as f64, ..xray::Glance::default() };
            a.notify.tx.send(Note::Glance(i, a.entries[i].version, g, Duration::ZERO)).unwrap();
        }
        a.absorb();
        assert_eq!(a.visible, by_cost, "costs that arrive in cost order fill in their cells and move no row");
        assert_eq!(a.entries[29].glance.as_ref().map(|g| g.cost), Some(29.0));
    }

    #[test]
    fn rows_on_the_screen_stay_when_rows_come_or_go_above_it() {
        let mut a = thirty();
        a.set_top(10);
        a.select(Some(15));
        let screen = |a: &Browser<String>| a.visible[a.top()..a.top() + 10].to_vec();
        let shown = screen(&a);

        take(&mut a, Change::Session(Box::new(row(30, "~/p", 5000))));
        assert_eq!(a.visible[0], 30, "a new session goes on top in time order");
        assert_eq!((screen(&a), a.selected_entry()), (shown.clone(), Some(15)), "the rows on the screen stay, and so does the selection");
        assert_eq!(a.top(), 11, "the screen moves down by the row that came above it");

        take(&mut a, Change::Gone("/x/3.jsonl".into()));
        assert_eq!((screen(&a), a.selected_entry()), (shown.clone(), Some(15)), "a session that leaves above the screen moves nothing on it");
        assert_eq!(a.top(), 10, "the screen moves up by the row that left above it");

        take(&mut a, Change::Gone("/x/12.jsonl".into()));
        assert_eq!(screen(&a)[..2], shown[..2], "a session that leaves on the screen keeps the rows above it");
        assert_eq!(screen(&a)[2..9], shown[3..10], "and pulls the rows below it up by one");
        assert_eq!(a.selected_entry(), Some(15));
    }

    #[test]
    fn a_click_selects_a_row_by_its_session_and_a_double_click_zooms() {
        let mut a = thirty();
        let row = a.row_of("id7").unwrap();
        assert_eq!(a.session(row), "id7");
        a.scroll = 5;
        a.select_row(row);
        assert_eq!((a.selected(), a.selected_entry(), a.scroll), (Some(row), Some(7), 0), "its X-ray shows from its top");
        a.select_row(99);
        assert_eq!(a.selected(), Some(row), "a row that is not listed selects nothing");
        a.zoom_in();
        assert!(a.zoomed());
        assert_eq!(a.row_of("nope"), None);

        a.show_day(100);
        assert!(a.showing_day() && a.zoomed());
        assert!(a.open_session("id9"), "a click on a lane of the day");
        assert_eq!((a.showing_day(), a.zoomed(), a.selected_entry()), (false, false, Some(9)), "goes back to the list with that session selected");
        assert!(!a.open_session("nope"));
    }

    #[test]
    fn notes_say_what_arrived_and_next_due_when_held_back_work_is_due() {
        let mut a = app(&["~/src/a", "~/src/b"]);
        // `tick` returns what `absorb` does; these sessions cannot be costed without a source.
        assert!(!a.absorb(), "nothing arrived");
        a.entries[0].glanced = 1;
        a.entries[1].glanced = 1;
        assert_eq!(a.next_due(), None, "nothing is held back");
        assert_eq!(a.dearest(), None, "no cost is known yet");
        // Session 1 was costed and took 60 s; it changed since, so its next costing waits.
        let later = Instant::now() + Duration::from_secs(60);
        a.entries[1].glance = Some(xray::Glance { cost: 3.0, ..xray::Glance::default() });
        a.entries[1].glanced = 1;
        a.entries[1].version = 2;
        a.entries[1].glance_after = later;
        assert_eq!(a.next_due(), Some(later), "the costing held back is due then");
        a.entries[0].glance = Some(xray::Glance { cost: 12.0, ..xray::Glance::default() });
        assert_eq!(a.dearest(), Some(12.0));
        take(&mut a, Change::Gone("/x/1.jsonl".into()));
        assert_eq!(a.next_due(), None, "a session that left holds nothing back");
        a.notify.tx.send(Note::Status("hi".into())).unwrap();
        assert!(a.absorb(), "a note arrived");
    }

    /// Sessions 0..5 last active 1000 - i s after the epoch, costed, with sessions 1, 3 and 4
    /// waiting since 40m, 16m and 2m before `now`.
    fn waiting_app(now: i64) -> Browser<String> {
        let mut a = app_of((0..5).map(|i| row(i, "~/p", 1000 - i as u64)).collect());
        for (i, e) in a.entries.iter_mut().enumerate() {
            let since = match i {
                1 => Some(now - 40 * 60_000),
                3 => Some(now - 16 * 60_000),
                4 => Some(now - 2 * 60_000),
                _ => None,
            };
            e.glance = Some(xray::Glance { cost: 1.0, waiting_since: since, cold_at_ms: i64::MAX, ..xray::Glance::default() });
        }
        a
    }

    fn sessions(a: &Browser<String>) -> Vec<&str> {
        (0..a.len()).map(|r| a.session(r)).collect()
    }

    #[test]
    fn without_the_waiting_option_s_cycles_three_orders() {
        let mut a = waiting_app(now_ms());
        assert_eq!(a.sorts(), [Sort::Time, Sort::Cost, Sort::Next]);
        for _ in 0..3 {
            press(&mut a, Key::Char('s'));
        }
        assert_eq!(a.sort_order(), Sort::Time);
        assert!(a.footer_keys().contains("s sort: time, cost, next · "), "{}", a.footer_keys());
        assert!((0..a.len()).all(|r| !a.cells(r, now_ms()).time.starts_with("waits")), "no row shows a wait");
        a.set_waiting(true);
        assert!(a.footer_keys().contains("s sort: time, cost, next, waiting · "), "{}", a.footer_keys());
        assert_eq!(a.sort_order(), Sort::Time, "the list still starts in time order");
    }

    #[test]
    fn by_waiting_the_longest_wait_comes_first() {
        let now = now_ms();
        let mut a = waiting_app(now);
        a.set_waiting(true);
        for _ in 0..2 {
            press(&mut a, Key::Char('s'));
        }
        a.top = 3;
        press(&mut a, Key::Char('s'));
        assert_eq!(a.top(), 0, "after a sort the list shows from its first row");
        assert_eq!(a.sort_order(), Sort::Waiting);
        assert_eq!(sessions(&a), ["id1", "id3", "id4", "id0", "id2"], "the waits, the longest first, then the others newest first");
        assert!(a.list_title(now).contains("· by waiting, 3 wait · "), "{}", a.list_title(now));
        assert_eq!(a.cells(0, now).time, "waits 40m");
        assert_eq!(a.cells(2, now).time, "waits 2.0m");
        assert!(a.cells(3, now).time.contains(':'), "a row that does not wait shows its time");
        assert_eq!(a.wait(1, now), Some(16 * 60_000));
        assert_eq!(a.next_wait_change(now), Some(now + 6_000), "the 2.0m wait reads 2.1m in 6 s");
        press(&mut a, Key::Char('s'));
        assert_eq!(a.sort_order(), Sort::Time, "back to time after four presses");
        assert!((0..a.len()).all(|r| !a.cells(r, now).time.starts_with("waits")), "no wait by time");
        assert_eq!(a.next_wait_change(now), None);
    }

    #[test]
    fn an_answered_wait_keeps_its_row() {
        let now = now_ms();
        let mut a = waiting_app(now);
        a.set_waiting(true);
        a.sort_by(Sort::Waiting);
        // The user typed in session 3; once it is costed again it no longer waits.
        a.entries[3].glance.as_mut().unwrap().waiting_since = None;
        assert_eq!(sessions(&a)[1], "id3", "the row stays where it is");
        assert!(a.cells(1, now).time.contains(':'));
        assert!(a.list_title(now).contains("· by waiting, 2 wait, r re-sorts · "), "{}", a.list_title(now));
    }

    #[test]
    fn waits_pass_the_limit_and_none_waits() {
        let now = now_ms();
        let mut a = waiting_app(now);
        a.set_waiting(true);
        a.sort_by(Sort::Waiting);
        let later = now + 2 * 3_600_000 - 40 * 60_000;
        assert_eq!(a.waiting_count(later), 2, "the 40m wait passed the 2-hour limit");
        assert!(a.cells(0, later).time.contains(':'));
        for e in &mut a.entries {
            e.glance.as_mut().unwrap().waiting_since = None;
        }
        a.sort_by(Sort::Waiting);
        assert!(a.list_title(now).contains("· by waiting, none waits · "), "{}", a.list_title(now));
        assert_eq!(sessions(&a), ["id0", "id1", "id2", "id3", "id4"], "time order");
        a.set_waiting(false);
        assert_eq!(a.sort_order(), Sort::Time, "turning the option off sorts by time");
    }

    #[test]
    fn r_re_sorts_an_order_out_of_date() {
        let mut a = thirty();
        press(&mut a, Key::Char('j'));
        let title = a.list_title(now_ms());
        assert!(title.contains("· by time · ") && !title.contains("r re-sorts"), "{title}");
        assert!(a.footer_keys().contains("s sort: time, cost, next · r re-sort"), "{}", a.footer_keys());

        take(&mut a, Change::Session(Box::new(row(12, "~/p", 3000))));
        assert_eq!(a.visible[12], 12);
        let title = a.list_title(now_ms());
        assert!(title.contains("· by time, r re-sorts · "), "a request in a session down the list puts the order out of date: {title}");

        press(&mut a, Key::Char('r'));
        assert_eq!((a.visible[0], a.selected_entry()), (12, Some(1)), "r puts that session on top and keeps the selection");
        let title = a.list_title(now_ms());
        assert!(!title.contains("r re-sorts"), "{title}");

        // By next request, a cache that goes cold changes the order with no new note.
        let now = now_ms();
        for (i, e) in a.entries.iter_mut().enumerate() {
            let (warm, cold, cold_at) = if i == 7 { (0.1, 5.0, now + 60_000) } else { (1.0, 1.0, i64::MAX) };
            e.glance = Some(xray::Glance { cost: 10.0, next_warm: Some(warm), next_cold: Some(cold), cold_at_ms: cold_at, ..Default::default() });
        }
        press(&mut a, Key::Char('s'));
        press(&mut a, Key::Char('s'));
        assert_eq!(*a.visible.last().unwrap(), 7, "the cheapest next request goes last while the cache is warm");
        assert!(!a.out_of_date(now));
        assert!(a.out_of_date(now + 120_000), "once the cache is cold, the order is out of date");
        assert_eq!(*a.visible.last().unwrap(), 7, "and the row stays where it is");
    }

    #[test]
    fn keys_move_filter_sort_and_quit() {
        let mut a = app(&["~/src/a", "~/src/beta", "~/src/b2"]);
        assert_eq!(a.selected_entry(), Some(0));
        press(&mut a, Key::Down);
        press(&mut a, Key::Down);
        press(&mut a, Key::Down);
        assert_eq!(a.selected_entry(), Some(2), "stops at the last row");

        for c in "/b".chars() {
            press(&mut a, Key::Char(c));
        }
        assert!(a.editing);
        assert_eq!(a.visible, vec![1, 2]);
        assert_eq!(a.selected_entry(), Some(2), "keeps the selected session when it still matches");
        press(&mut a, Key::Enter);
        press(&mut a, Key::Esc);
        assert!(a.filter.is_empty() && a.visible.len() == 3);

        let glance = |cost, next| xray::Glance { cost, next_warm: Some(next), next_cold: Some(next), cold_at_ms: i64::MAX, ..Default::default() };
        for (e, g) in a.entries.iter_mut().zip([Some(glance(1.0, 9.0)), Some(glance(5.0, 0.1)), None]) {
            e.glance = g;
        }
        press(&mut a, Key::Char('s'));
        assert_eq!(a.visible, vec![1, 0, 2], "most expensive first, unknown last");
        press(&mut a, Key::Char('s'));
        assert_eq!(a.visible, vec![0, 1, 2], "dearest next request first");
        press(&mut a, Key::Char('s'));
        press(&mut a, Key::Char('s'));

        press(&mut a, Key::Enter);
        assert!(a.zoom);
        // 40 lines in a pane of 27 (30 rows less the footer and the borders) scroll by 13.
        a.entries[2].report = Some(vec!["line"; 40].join("\n"));
        press(&mut a, Key::Down);
        assert_eq!((a.scroll, a.selected_entry()), (1, Some(2)), "arrows scroll the report when zoomed");
        for _ in 0..3 {
            press(&mut a, Key::PageDown);
        }
        assert_eq!(a.scroll, 13, "scrolling stops at the last page");
        assert!(press(&mut a, Key::Esc) && !a.zoom);
        assert!(!press(&mut a, Key::Char('q')));
    }

    #[test]
    fn ctrl_c_quits_while_a_filter_is_typed_and_q_is_typed_into_it() {
        let mut a = app(&["~/src/a"]);
        press(&mut a, Key::Char('/'));
        assert!(press(&mut a, Key::Char('q')), "q is typed into the filter");
        assert_eq!(a.filter, "q");
        assert!(!press(&mut a, Key::Interrupt), "Ctrl+C quits");
    }

    #[test]
    fn the_next_cold_cache_is_the_first_of_the_listed_sessions() {
        let mut a = app(&["~/src/a", "~/src/b", "~/src/c", "~/src/d"]);
        let now = 1_000_000;
        let glance = |cold_at_ms| Some(xray::Glance { cost: 10.0, next_warm: Some(0.1), next_cold: Some(2.0), cold_at_ms, ..Default::default() });
        a.entries[0].glance = glance(now + 300_000);
        a.entries[1].glance = glance(now + 60_000);
        a.entries[2].glance = glance(now - 1);
        a.entries[3].glance = Some(xray::Glance { cost: 10.0, next_warm: None, next_cold: None, cold_at_ms: now + 10_000, ..Default::default() });
        assert_eq!(a.next_cold(now), Some(now + 60_000), "a cache already cold and a session without a forecast change no cell");
        let row = a.visible.iter().position(|&i| i == 1).unwrap();
        assert_eq!(a.cells(row, now + 59_999).next, Some(("$0.10   1%".to_string(), false)), "warm until then");
        assert_eq!(a.cells(row, now + 60_000).next, Some(("$2.00  20%".to_string(), true)), "cold from that moment");

        for c in "/src/a".chars() {
            press(&mut a, Key::Char(c));
        }
        assert_eq!(a.next_cold(now), Some(now + 300_000), "only the listed sessions count");
        assert_eq!(a.next_cold(now + 300_000), None, "and none goes cold after the last");
    }

    /// The day view the worker would send for `key`, showing `date`.
    fn day_note(app: &mut Browser<String>, key: Option<i64>, date: i64, changes: u32, text: &str) {
        app.notify.tx.send(Note::Day(key, date, changes, app.pane, 4, text.to_string(), None, Duration::ZERO)).unwrap();
        app.absorb();
    }

    /// The day pane's text, or what it says while the day is being computed.
    fn day_body(app: &Browser<String>) -> String {
        match app.day_pane() {
            Pane::Text(t) => t.clone(),
            Pane::Computing(t) => t,
            Pane::Empty(t) => t.to_string(),
        }
    }

    /// The full-screen pane of a screen 160 columns by 20 rows: all of it but the footer.
    const FULL: (u16, u16) = (160, 19);

    fn today() -> i64 {
        rhythm::date_of(now_ms(), &rhythm::local_offset)
    }

    fn label(date: i64, format: &str) -> String {
        local_time(rhythm::midnight(date, &rhythm::local_offset) + 12 * 3_600_000, format)
    }

    #[test]
    fn d_shows_the_day_and_d_again_the_xray() {
        let mut a = app(&["~/src/a"]);
        press(&mut a, Key::Char('d'));
        a.set_pane(FULL);
        let title = a.day_title();
        assert!(title.contains(&format!(" day · {} ", label(today(), "%a %Y-%m-%d"))), "{title}");
        assert!(matches!(a.day_pane(), Pane::Computing(_)));
        assert!(day_body(&a).contains(&format!("computing the day of {}…", label(today(), "%Y-%m-%d"))), "{}", day_body(&a));
        a.request_day();
        assert!(a.zoom, "the day takes the whole screen");
        assert_eq!(a.day_pending, Some((None, 0, FULL, None)), "the day is asked for at the screen's width");
        day_note(&mut a, None, today() - 1, 0, "tokprof · day\nday       …");
        let title = a.day_title();
        assert!(title.contains(&format!(" day · {} ", label(today() - 1, "%a %Y-%m-%d"))), "the latest action's day: {title}");
        assert!(day_body(&a).contains("tokprof · day"), "{}", day_body(&a));
        assert_eq!(a.day_axis, Some(4), "the first day sets the hour strips start at");
        press(&mut a, Key::Char('d'));
        assert!(a.day.is_none() && !a.zoom, "d goes back to the list with the X-ray");
        assert!(!a.showing_day());
    }

    #[test]
    fn arrows_step_through_days_but_not_past_the_latest() {
        let mut a = app(&["~/src/a"]);
        press(&mut a, Key::Char('['));
        assert_eq!(a.day, None, "the brackets need the day view");
        press(&mut a, Key::Char('d'));
        press(&mut a, Key::Left);
        assert_eq!(a.day, Some(None), "the latest day is not known before it is computed");
        a.set_pane(FULL);
        day_note(&mut a, None, 100, 0, "latest");
        press(&mut a, Key::Left);
        press(&mut a, Key::Left);
        press(&mut a, Key::Right);
        assert_eq!(a.day, Some(Some(99)));
        press(&mut a, Key::Right);
        assert_eq!(a.day, Some(None), "back on the latest day");
        press(&mut a, Key::Right);
        assert_eq!(a.day, Some(None), "→ on the latest day does nothing");
        for (back, on) in [(Key::Char('h'), Key::Char('l')), (Key::Char('['), Key::Char(']'))] {
            press(&mut a, back);
            assert_eq!(a.day, Some(Some(99)), "{back:?} steps back");
            press(&mut a, on);
            assert_eq!(a.day, Some(None), "{on:?} steps on");
        }
    }

    #[test]
    fn the_day_goes_full_screen_and_the_footer_names_its_keys() {
        let mut a = app(&["~/src/a"]);
        assert!(a.footer_keys().ends_with(" · q quit · d day"), "the list's footer");
        press(&mut a, Key::Char('d'));
        a.set_pane(FULL);
        day_note(&mut a, None, 100, 0, &vec!["line"; 40].join("\n"));
        assert_eq!(a.footer_keys(), " ↑↓ PgUp PgDn scroll · ← → days · esc back · f JFR · o HTML · c handoff · q quit");
        press(&mut a, Key::Enter);
        assert!(a.zoom && a.showing_day(), "full screen");
        press(&mut a, Key::Down);
        assert_eq!(a.scroll, 1, "the full-screen day scrolls");
        press(&mut a, Key::Left);
        assert_eq!((a.day, a.zoom, a.scroll), (Some(Some(99)), true, 0), "← steps to the day before, full screen");
        a.request_day();
        assert_eq!(a.day_pending, Some((Some(99), 0, FULL, Some(4))), "the day before starts its strip at the first day's hour");
        for key in [Key::Esc, Key::Char('d'), Key::Backspace] {
            if a.day.is_none() {
                press(&mut a, Key::Char('d'));
            }
            assert!(a.day.is_some() && a.zoom);
            assert!(press(&mut a, key), "{key:?} does not quit");
            assert_eq!((a.day, a.zoom), (None, false), "{key:?} goes back to the list");
        }
    }

    #[test]
    fn the_current_day_follows_the_logs_and_a_past_day_stays() {
        let mut a = app(&["~/src/a"]);
        press(&mut a, Key::Char('d'));
        a.set_pane(FULL);
        day_note(&mut a, None, 100, 0, "latest");
        a.request_day();
        assert_eq!(a.day_pending, None, "fresh: nothing to ask");
        take(&mut a, Change::Session(Box::new(row(0, "~/src/a", 5))));
        a.request_day();
        assert_eq!(a.day_pending, Some((None, 1, FULL, Some(4))), "a log changed: computed again");
        assert!(a.day_title().contains(" · updating…"), "the old view shows until the new one is ready");
        day_note(&mut a, None, 100, 1, "latest again");
        assert!(!a.day_title().contains("updating"));
        // A past day, once computed, shows at once though logs changed since.
        press(&mut a, Key::Char('['));
        day_note(&mut a, Some(99), 99, 1, "the day before");
        take(&mut a, Change::Session(Box::new(row(0, "~/src/a", 6))));
        press(&mut a, Key::Char(']'));
        press(&mut a, Key::Char('['));
        a.request_day();
        assert_eq!(a.day_pending, None, "the past day is not asked for again");
        assert!(day_body(&a).contains("the day before"), "and shows at once");
        press(&mut a, Key::Char(']'));
        a.request_day();
        assert_eq!(a.day_pending, Some((None, 2, FULL, Some(4))), "the current day is, since logs changed");
    }

    /// Seconds after the epoch at `hour` local time on `date`.
    fn at(date: i64, hour: i64) -> u64 {
        ((rhythm::midnight(date, &rhythm::local_offset) + hour * 3_600_000) / 1000) as u64
    }

    #[test]
    fn the_window_s_mode_keeps_a_day_beside_the_list_and_the_arrows_step_it() {
        let mut a = thirty();
        a.keep_day_beside();
        assert_eq!(a.day, Some(None), "the latest day from the start");
        assert!(!a.zoom, "beside the list, not over it");
        a.set_pane(FULL);
        day_note(&mut a, None, 100, 0, "latest");
        assert!(a.day_is_latest());
        press(&mut a, Key::Left);
        assert_eq!(a.day, Some(Some(99)), "← steps the day, not out of the X-ray");
        assert!(!a.day_is_latest());
        press(&mut a, Key::Right);
        assert_eq!(a.day, Some(None));
        for (back, on) in [(Key::Char('h'), Key::Char('l')), (Key::Char('['), Key::Char(']'))] {
            press(&mut a, back);
            assert_eq!(a.day, Some(Some(99)), "{back:?} steps back");
            press(&mut a, on);
            assert_eq!(a.day, Some(None), "{on:?} steps on");
        }
        a.step_day_by(-3);
        assert_eq!(a.day, Some(Some(97)), "the buttons step the day too");
        a.show_latest_day();
        assert!(a.day_is_latest(), "Today goes back to the latest day");
        press(&mut a, Key::Enter);
        assert!(a.zoom, "⏎ shows the X-ray full size");
        press(&mut a, Key::Char('j'));
        assert_eq!(a.selected(), Some(0), "full size, j scrolls instead of moving");
        assert!(press(&mut a, Key::Esc) && !a.zoom, "esc ends full size");
        assert_eq!(a.day, Some(None), "and the day stays");
        press(&mut a, Key::Char('/'));
        press(&mut a, Key::Char('q'));
        assert_eq!(a.filter(), "q", "q is typed into a filter");
        press(&mut a, Key::Enter);
        assert!(!a.zoom, "⏎ ends typing the filter");
        assert!(press(&mut a, Key::Esc) && a.filter().is_empty(), "esc clears the filter first");
        assert!(!press(&mut a, Key::Esc), "then quits");
    }

    #[test]
    fn selecting_a_session_shows_its_day() {
        let (now, offset) = (today(), &rhythm::local_offset);
        let latest = rhythm::date_of(now_ms(), offset);
        // Session 0 was active this afternoon, session 1 two days ago at 16:00, session 2 three
        // days ago at 01:00, which belongs to the day before by the five-hour rule.
        let mut a = app_of(vec![row(0, "~/a", at(now, 13)), row(1, "~/b", at(now - 2, 16)), row(2, "~/c", at(now - 3, 1))]);
        a.keep_day_beside();
        a.set_pane(FULL);
        day_note(&mut a, None, latest, 0, "latest");
        press(&mut a, Key::Char('j'));
        assert_eq!(a.day, Some(Some(now - 2)), "the day of its last activity");
        a.select_row(2);
        assert_eq!(a.day, Some(Some(now - 4)), "a night's work belongs to the day before");
        press(&mut a, Key::Left);
        assert_eq!(a.day, Some(Some(now - 5)));
        press(&mut a, Key::Char('d'));
        assert_eq!(a.day, Some(Some(now - 4)), "d shows the selected session's day again");
        press(&mut a, Key::Char('g'));
        assert_eq!(a.day, Some(None), "the latest day for a session of it");
        assert!(a.open_session("id1") && a.day == Some(Some(now - 2)) && a.selected() == Some(1), "a lane's click keeps the day beside");

        // A filter being typed moves the selection to what it leaves, and the day stays.
        press(&mut a, Key::Char('g'));
        assert_eq!(a.day, Some(None));
        for c in "/~/c".chars() {
            press(&mut a, Key::Char(c));
        }
        assert_eq!((a.selected().map(|r| a.session(r)), a.day), (Some("id2"), Some(None)), "the filter leaves only ~/c, and the day stays");
        press(&mut a, Key::Enter);
        assert_eq!(a.day, Some(None), "and stays once it is typed");
        press(&mut a, Key::Char('d'));
        assert_eq!(a.day, Some(Some(now - 4)), "d shows the selected session's day");
    }

    #[test]
    fn the_terminal_keeps_its_keys() {
        let mut a = thirty();
        press(&mut a, Key::Right);
        assert!(a.zoom && a.day.is_none(), "→ opens the X-ray full screen");
        press(&mut a, Key::Left);
        assert!(!a.zoom);
        a.select_row(5);
        assert_eq!(a.day, None, "a selection shows no day");
    }

    #[test]
    fn the_glance_gives_the_row_its_title_and_the_lane_its_cache_life() {
        let mut a = app(&["~/a", "~/b"]);
        let (last, cold_at) = (1_000_000, 4_600_000);
        a.entries[0].glance = Some(xray::Glance {
            cost: 1.0,
            next_warm: Some(0.1),
            next_cold: Some(2.0),
            cold_at_ms: cold_at,
            last_ms: last,
            title: Some("Fix the export".into()),
            waiting_since: None,
        });
        a.entries[1].glance = Some(xray::Glance { cost: 2.0, cold_at_ms: i64::MAX, ..Default::default() });
        assert_eq!(a.cache_life("id0"), Some((last, cold_at)));
        assert_eq!(a.cache_life("id1"), None, "no request, no cache");
        assert_eq!(a.cache_life("nobody"), None);
        let row = a.row_of("id0").unwrap();
        let warm = a.cells(row, cold_at - 1);
        assert_eq!((warm.title, warm.warm_until, warm.next_usd), (Some("Fix the export"), Some(cold_at), Some(0.1)));
        let cold = a.cells(row, cold_at);
        assert_eq!((cold.warm_until, cold.next_usd), (None, Some(2.0)), "cold: no countdown, the cold cost");
        assert_eq!(a.cells(a.row_of("id1").unwrap(), 0).title, None);
    }

    #[test]
    fn the_x_ray_arrives_with_its_text() {
        let mut a = app(&["~/a"]);
        assert!(a.xray().is_none());
        let version = a.entries[0].version;
        let x = Arc::new(crate::render_xray::tests::xray(Vec::new()));
        a.notify.tx.send(Note::Report(0, version, "text".into(), Some(x), Duration::ZERO)).unwrap();
        a.absorb();
        assert_eq!(a.xray().map(|x| x.project.as_str()), Some("~/p"));
        assert!(matches!(a.report(), Pane::Text(t) if t == "text"), "the terminal keeps the text");
        a.notify.tx.send(Note::Report(0, version + 1, "error: gone".into(), None, Duration::ZERO)).unwrap();
        a.absorb();
        assert!(a.xray().is_none(), "a failed X-ray has no data");
    }

    /// Ticks the browser until `done`, at most 20 s.
    fn tick_until(app: &mut Browser<String>, done: impl Fn(&Browser<String>) -> bool) {
        let deadline = Instant::now() + Duration::from_secs(20);
        while !done(app) && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(20));
            app.tick();
        }
    }

    fn shown(app: &Browser<String>) -> Option<String> {
        match app.report() {
            Pane::Text(t) => Some(t.clone()),
            _ => None,
        }
    }

    #[test]
    fn a_session_whose_decoder_panics_shows_the_error_and_the_workers_go_on() {
        use crate::contain::testing::{self, BOOM, BROKEN, HEALTHY};
        let (catalog, _, _) = testing::catalog("browser");
        let select = Query { agents: catalog.agents(), ..Query::default() };
        let mut app = Browser::start(catalog, Pricing::defaults(), select, false, Arc::new(|| {}), plain()).unwrap();
        assert!(app.open_session(BROKEN));
        tick_until(&mut app, |a| shown(a).is_some());
        assert_eq!(shown(&app).as_deref(), Some(format!("error: tokprof panicked: {BOOM}").as_str()));
        assert!(app.xray().is_none());
        // Its row leaves the costing: its logs count as unreadable, as a log that fails to parse.
        let costed = |a: &Browser<String>, id: &str| a.row_of(id).and_then(|row| a.cells(row, now_ms()).usd);
        tick_until(&mut app, |a| costed(a, BROKEN).is_some() && costed(a, HEALTHY).is_some());
        assert_eq!(costed(&app, BROKEN), Some(0.0));
        assert!(costed(&app, HEALTHY).is_some_and(|usd| usd > 0.0), "the rest of the batch was costed");
        // The worker survived: the X-ray of a healthy session still arrives.
        assert!(app.open_session(HEALTHY));
        tick_until(&mut app, |a| shown(a).is_some());
        let text = shown(&app).expect("the healthy X-ray arrived");
        assert!(!text.starts_with("error"), "{text}");
        assert!(app.xray().is_some());
    }
}
