//! Interactive session browser, `tokprof ui`: the sessions on the left, the X-ray of the
//! selected one on the right. Costs fill in from a background pass over all logs; X-rays are
//! computed on a worker thread, newest request first, and kept. The list follows the log
//! directories: new sessions show up, and a session that is still being written is costed and
//! X-rayed again as it grows. Rows move only when the user sorts or filters; what arrives in the
//! background fills in cells.

use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::{Duration, Instant};

use ansi_to_tui::IntoText;
use anyhow::{Result, bail};
use rayon::prelude::*;
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style, Stylize};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, List, ListItem, ListState, Paragraph};
use ratatui::{Frame, Terminal};

use tokprof_core::catalog::Catalog;
use tokprof_core::query::Query;
use tokprof_core::labels::display_path;
use tokprof_core::source::SessionRef;
use tokprof_core::pricing::Pricing;
use tokprof_core::fmt::usd;
use tokprof_core::rhythm;
use tokprof_core::xray::{self, SessionRow};

use crate::watch::{self, Change};

const LIST_WIDTH: u16 = 74;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Sort {
    Time,
    Cost,
    /// What the next request would cost now.
    Next,
}

impl Sort {
    fn name(self) -> &'static str {
        match self {
            Sort::Time => "time",
            Sort::Cost => "cost",
            Sort::Next => "next request",
        }
    }

    fn following(self) -> Sort {
        match self {
            Sort::Time => Sort::Cost,
            Sort::Cost => Sort::Next,
            Sort::Next => Sort::Time,
        }
    }
}

enum Note {
    /// A session's glance, the version of its logs it was computed from, and how long it took.
    Glance(usize, u32, xray::Glance, Duration),
    /// The same for an X-ray.
    Report(usize, u32, String, Duration),
    Change(Change),
    Status(String),
    /// A day view: the day asked for (`None` for the day of the latest action), the date it shows,
    /// the count of log changes and the width it was computed for, the local hour its strip starts
    /// at, its text, and how long it took.
    Day(Option<i64>, i64, u32, u16, i64, String, Duration),
}

/// A day the browser computed.
struct DayText {
    /// The local date it shows.
    date: i64,
    text: Text<'static>,
    /// The count of log changes it was computed after, and the pane width it was laid out for.
    changes: u32,
    width: u16,
}

/// What the day worker is asked: the day, the count of log changes, the width, and the local hour
/// the strip starts at once a first day set it.
type DayJob = (Option<i64>, u32, u16, Option<i64>);

/// A listed session and what is known about it.
struct Entry {
    row: SessionRow,
    /// Counts the changes of the session's logs. Glances and X-rays carry the version they were
    /// computed from; an older one is shown until a newer one is ready.
    version: u32,
    glance: Option<xray::Glance>,
    glanced: u32,
    glancing: bool,
    /// When the session may be costed again: one that keeps changing waits as long as its last
    /// costing took, so that it takes at most half of a core.
    glance_after: Instant,
    report: Option<Text<'static>>,
    reported: u32,
    /// The session's main log is gone.
    gone: bool,
}

impl Entry {
    fn new(row: SessionRow) -> Entry {
        Entry {
            row,
            version: 1,
            glance: None,
            glanced: 0,
            glancing: false,
            glance_after: Instant::now(),
            report: None,
            reported: 0,
            gone: false,
        }
    }
}

struct App {
    catalog: Arc<Catalog>,
    entries: Vec<Entry>,
    /// Entries by the path of the session's main log.
    by_path: HashMap<PathBuf, usize>,
    pricing: Arc<Pricing>,
    /// The X-ray asked for last.
    pending: Option<(usize, u32)>,
    /// When the X-ray of a session that changed may be computed again, as for glances.
    report_after: Instant,
    visible: Vec<usize>,
    list: ListState,
    scroll: u16,
    zoom: bool,
    filter: String,
    editing: bool,
    sort: Sort,
    status: String,
    xray_tx: Sender<(usize, u32, SessionRef)>,
    notes_tx: Sender<Note>,
    /// The day shown in the right pane instead of the X-ray: `Some(None)` is the day of the
    /// latest action, `Some(Some(date))` another day.
    day: Option<Option<i64>>,
    /// The days computed, by the day asked for.
    days: HashMap<Option<i64>, DayText>,
    /// Counts the changes of the log directories, after which the current day is computed again.
    changes: u32,
    day_pending: Option<DayJob>,
    /// When the current day may be computed again, as for X-rays.
    day_after: Instant,
    day_tx: Sender<DayJob>,
    /// The width of the right pane when it was last drawn.
    pane_width: u16,
    /// The local hour the first day shown starts its strip at; later days start theirs there too.
    day_axis: Option<i64>,
}

/// The colors of the agent column, by the agent's place in the registry.
const AGENT_COLORS: [Color; 4] = [Color::Magenta, Color::Green, Color::Cyan, Color::Yellow];

pub fn run(catalog: Arc<Catalog>, pricing: Pricing, dump: Option<(u16, u16)>, select: Query) -> Result<()> {
    let (notes_tx, notes_rx) = mpsc::channel();
    // The log directories are watched from before the sessions are listed, so that no change
    // is missed.
    let following = match dump {
        Some(_) => Ok(()),
        None => {
            let (tx, select) = (notes_tx.clone(), select.clone());
            watch::watch(catalog.clone(), select.agents.clone(), move |change| {
                // Only the sessions the filters select join the list.
                let change = match change {
                    Change::Session(row) if !select.row(&row) => return true,
                    Change::All(rows) => Change::All(rows.into_iter().filter(|r| select.row(r)).collect()),
                    change => change,
                };
                tx.send(Note::Change(change)).is_ok()
            })
        }
    };
    let rows = xray::rows(&catalog, &select, usize::MAX);
    if rows.is_empty() {
        bail!("no session logs found");
    }
    let pricing = Arc::new(pricing);

    // Days, one at a time, as X-rays are.
    let (day_tx, day_rx) = mpsc::channel::<DayJob>();
    {
        let (catalog, pricing, tx, select) = (catalog.clone(), pricing.clone(), notes_tx.clone(), select.clone());
        std::thread::spawn(move || {
            // A pool of its own, so that a day does not wait behind the costing of every session.
            let pool = rayon::ThreadPoolBuilder::new().num_threads(4).build().ok();
            while let Ok(mut job) = day_rx.recv() {
                while let Ok(newer) = day_rx.try_recv() {
                    job = newer;
                }
                let (key, changes, width, axis) = job;
                let started = Instant::now();
                let style = tokprof_core::fmt::Style::with_color(true);
                let build = || crate::day::build(&catalog, &select, &pricing, key, now_ms(), strip_columns(width), axis, &style);
                let built = match &pool {
                    Some(pool) => pool.install(build),
                    None => build(),
                };
                let (date, hour, text) = match built {
                    Ok(day) => (day.date, day.axis_hour, day.text),
                    Err(e) => (key.unwrap_or_else(|| rhythm::date_of(now_ms(), &rhythm::local_offset)), axis.unwrap_or(0), format!("error: {e:#}")),
                };
                if tx.send(Note::Day(key, date, changes, width, hour, text, started.elapsed())).is_err() {
                    return;
                }
            }
        });
    }

    // X-rays, one at a time; a newer request replaces the ones still waiting.
    let (xray_tx, xray_rx) = mpsc::channel::<(usize, u32, SessionRef)>();
    {
        let (catalog, pricing, tx) = (catalog.clone(), pricing.clone(), notes_tx.clone());
        std::thread::spawn(move || {
            while let Ok(mut job) = xray_rx.recv() {
                while let Ok(newer) = xray_rx.try_recv() {
                    job = newer;
                }
                let (i, version, file) = job;
                let started = Instant::now();
                let text = xray::xray(&catalog, &file, &pricing, now_ms()).map(|x| crate::render_xray::text(&x, &tokprof_core::fmt::Style::with_color(true), &crate::paths::profile_dir(), catalog.home())).unwrap_or_else(|e| format!("error: {e:#}"));
                if tx.send(Note::Report(i, version, text, started.elapsed())).is_err() {
                    return;
                }
            }
        });
    }

    let mut app = App::new(catalog, rows, pricing, xray_tx, notes_tx, day_tx);
    if let Err(e) = following {
        app.status = format!("not following the logs: {e:#}");
    }
    // Costs of every session, in the background.
    app.request_glances();

    if let Some((width, height)) = dump {
        return dump_frame(&mut app, &notes_rx, width, height);
    }
    let mut terminal = ratatui::init();
    let result = event_loop(&mut terminal, &mut app, &notes_rx);
    ratatui::restore();
    result
}

fn event_loop(terminal: &mut ratatui::DefaultTerminal, app: &mut App, notes: &Receiver<Note>) -> Result<()> {
    loop {
        app.absorb(notes);
        app.request_glances();
        app.request_report();
        app.request_day();
        terminal.draw(|f| draw(f, app))?;
        if event::poll(Duration::from_millis(100))?
            && let Event::Key(key) = event::read()?
                && key.kind == KeyEventKind::Press && !app.on_key(key, terminal.size()?.height) {
                    return Ok(());
                }
    }
}

/// Renders one frame without a terminal, for checking the layout.
fn dump_frame(app: &mut App, notes: &Receiver<Note>, width: u16, height: u16) -> Result<()> {
    let deadline = Instant::now() + Duration::from_secs(20);
    app.request_report();
    while Instant::now() < deadline && app.selected().is_some_and(|i| app.entries[i].report.is_none()) {
        std::thread::sleep(Duration::from_millis(50));
        app.absorb(notes);
    }
    std::thread::sleep(Duration::from_millis(500));
    app.absorb(notes);
    let mut terminal = Terminal::new(ratatui::backend::TestBackend::new(width, height))?;
    terminal.draw(|f| draw(f, app))?;
    let buffer = terminal.backend().buffer();
    for y in 0..height {
        let line: String = (0..width).map(|x| buffer[(x, y)].symbol()).collect();
        println!("{}", line.trim_end());
    }
    Ok(())
}

impl App {
    fn new(catalog: Arc<Catalog>, rows: Vec<SessionRow>, pricing: Arc<Pricing>, xray_tx: Sender<(usize, u32, SessionRef)>, notes_tx: Sender<Note>, day_tx: Sender<DayJob>) -> App {
        let by_path = rows.iter().enumerate().map(|(i, r)| (r.file.path().to_path_buf(), i)).collect();
        let mut app = App {
            catalog,
            entries: rows.into_iter().map(Entry::new).collect(),
            by_path,
            pricing,
            pending: None,
            report_after: Instant::now(),
            visible: Vec::new(),
            list: ListState::default(),
            scroll: 0,
            zoom: false,
            filter: String::new(),
            editing: false,
            sort: Sort::Time,
            status: String::new(),
            xray_tx,
            notes_tx,
            day: None,
            days: HashMap::new(),
            changes: 0,
            day_pending: None,
            day_after: Instant::now(),
            day_tx,
            pane_width: 0,
            day_axis: None,
        };
        app.refilter();
        app
    }

    fn selected(&self) -> Option<usize> {
        self.list.selected().and_then(|s| self.visible.get(s).copied())
    }

    /// Takes in what the background work sent. It fills in cells and never moves a row that
    /// stays listed: rows move only when the user sorts or filters.
    fn absorb(&mut self, notes: &Receiver<Note>) {
        let mut listed = false;
        while let Ok(note) = notes.try_recv() {
            match note {
                Note::Glance(i, version, g, took) => {
                    let e = &mut self.entries[i];
                    e.glance = Some(g);
                    e.glanced = version;
                    e.glancing = false;
                    e.glance_after = Instant::now() + took;
                }
                Note::Report(i, version, text, took) => {
                    let e = &mut self.entries[i];
                    e.report = Some(text.into_text().unwrap_or_else(|_| Text::raw(text)));
                    e.reported = version;
                    self.report_after = Instant::now() + took;
                }
                Note::Change(change) => {
                    self.changes += 1;
                    listed |= self.apply(change);
                }
                Note::Status(s) => self.status = s,
                Note::Day(key, date, changes, width, hour, text, took) => {
                    let text = text.into_text().unwrap_or_else(|_| Text::raw(text));
                    self.days.insert(key, DayText { date, text, changes, width });
                    self.day_axis.get_or_insert(hour);
                    if self.day_pending.is_some_and(|p| (p.0, p.1, p.2) == (key, changes, width)) {
                        self.day_pending = None;
                    }
                    self.day_after = Instant::now() + took;
                }
            }
        }
        if listed {
            self.relist();
        }
    }

    /// Takes in what the log directories changed without moving the rows that stay listed: a
    /// session that is gone takes its row with it, and a new one is put before the first row that
    /// the current order puts after it. Rows that come or go above the screen move the screen
    /// with them, so the rows on it stay where they are. When the selected session is gone, the
    /// cursor stays on its row.
    fn relist(&mut self) {
        let (kept, at) = (self.selected(), self.list.selected());
        let first = self.list.offset();
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
        *self.list.offset_mut() = offset;
        match kept.and_then(|k| self.visible.iter().position(|&i| i == k)) {
            Some(row) => self.list.select(Some(row)),
            None if self.visible.is_empty() => self.list.select(None),
            None => {
                self.list.select(Some(at.unwrap_or(0).min(self.visible.len() - 1)));
                self.scroll = 0;
            }
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
            Change::All(rows) => {
                let found: HashSet<&Path> = rows.iter().map(|r| r.file.path()).collect();
                for e in self.entries.iter_mut().filter(|e| !e.gone && !found.contains(e.row.file.path())) {
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
            if let Some(&j) = self.by_path.get(sub.path())
                && !self.entries[j].gone
            {
                self.entries[j].gone = true;
                changed = true;
            }
        }
        let Some(&i) = self.by_path.get(row.file.path()) else {
            self.by_path.insert(row.file.path().to_path_buf(), self.entries.len());
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
        let (catalog, pricing, tx) = (self.catalog.clone(), self.pricing.clone(), self.notes_tx.clone());
        std::thread::spawn(move || {
            due.into_par_iter().for_each_with(tx, |tx, (i, version, row)| {
                let started = Instant::now();
                let g = xray::glance(&catalog, &row, &pricing, now_ms());
                let _ = tx.send(Note::Glance(i, version, g, started.elapsed()));
            });
        });
    }

    fn request_report(&mut self) {
        let Some(i) = self.selected() else { return };
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
        let width = self.pane_width;
        if width == 0 || self.day_pending.is_some_and(|p| (p.0, p.1, p.2) == (key, self.changes, width)) {
            return;
        }
        if let Some(d) = self.days.get(&key)
            && d.width == width
            && (key.is_some() || d.changes == self.changes || Instant::now() < self.day_after)
        {
            return;
        }
        let job = (key, self.changes, width, self.day_axis);
        self.day_pending = Some(job);
        let _ = self.day_tx.send(job);
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
        let keep = self.selected();
        self.visible = (0..self.entries.len()).filter(|&i| self.shows(i)).collect();
        self.sort_keeping(keep);
    }

    fn sort(&mut self) {
        self.sort_keeping(self.selected());
    }

    /// The value of each entry that the current order sorts by, at `now`; an unknown one is -1.
    fn keys(&self, now: i64) -> Vec<f64> {
        let key = |e: &Entry| match self.sort {
            Sort::Time => e.row.active_ms as f64,
            Sort::Cost => e.glance.map_or(-1.0, |g| g.cost),
            Sort::Next => e.glance.and_then(|g| g.next(now)).map_or(-1.0, |(usd, _)| usd),
        };
        self.entries.iter().map(key).collect()
    }

    fn sort_keeping(&mut self, keep: Option<usize>) {
        let keys = self.keys(now_ms());
        self.visible.sort_by(|&a, &b| order(&keys, a, b));
        let at = keep.and_then(|k| self.visible.iter().position(|&i| i == k));
        self.list.select(if self.visible.is_empty() { None } else { Some(at.unwrap_or(0)) });
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
        let at = self.list.selected().unwrap_or(0) as isize + delta;
        self.list.select(Some(at.clamp(0, self.visible.len() as isize - 1) as usize));
        self.scroll = 0;
    }

    /// Handles a key; returns false to quit.
    fn on_key(&mut self, key: KeyEvent, height: u16) -> bool {
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            return false;
        }
        if self.editing {
            match key.code {
                KeyCode::Char(c) => self.filter.push(c),
                KeyCode::Backspace => {
                    self.filter.pop();
                }
                KeyCode::Enter => self.editing = false,
                KeyCode::Esc => {
                    self.filter.clear();
                    self.editing = false;
                }
                _ => return true,
            }
            self.refilter();
            return true;
        }
        let page = height.saturating_sub(4).max(1);
        match key.code {
            KeyCode::Char('q') => return false,
            // The day takes the whole screen; these go back to the list with the X-ray.
            KeyCode::Char('d') | KeyCode::Esc | KeyCode::Left | KeyCode::Char('h') | KeyCode::Backspace if self.day.is_some() => {
                self.day = None;
                self.zoom = false;
                self.scroll = 0;
            }
            KeyCode::Esc if self.zoom => self.zoom = false,
            KeyCode::Esc if !self.filter.is_empty() => {
                self.filter.clear();
                self.refilter();
            }
            KeyCode::Esc => return false,
            KeyCode::Up | KeyCode::Char('k') if self.zoom => self.scroll = self.scroll.saturating_sub(1),
            KeyCode::Down | KeyCode::Char('j') if self.zoom => self.scroll = self.scroll.saturating_add(1).min(self.max_scroll(height)),
            KeyCode::PageUp if self.zoom => self.scroll = self.scroll.saturating_sub(page),
            KeyCode::PageDown | KeyCode::Char(' ') if self.zoom => self.scroll = self.scroll.saturating_add(page).min(self.max_scroll(height)),
            KeyCode::Up | KeyCode::Char('k') => self.move_by(-1),
            KeyCode::Down | KeyCode::Char('j') => self.move_by(1),
            KeyCode::PageUp => self.move_by(-(page as isize)),
            KeyCode::PageDown => self.move_by(page as isize),
            KeyCode::Home | KeyCode::Char('g') => self.move_by(isize::MIN / 2),
            KeyCode::End | KeyCode::Char('G') => self.move_by(isize::MAX / 2),
            KeyCode::Enter | KeyCode::Right | KeyCode::Char('l') => self.zoom = true,
            KeyCode::Left | KeyCode::Char('h') | KeyCode::Backspace => self.zoom = false,
            KeyCode::Char('/') => self.editing = true,
            KeyCode::Char('s') => {
                self.sort = self.sort.following();
                self.sort();
            }
            KeyCode::Char('r') => self.sort(),
            KeyCode::Char('f') => self.export("jfr", false),
            KeyCode::Char('o') => self.export("html", true),
            KeyCode::Char('c') => self.handoff(),
            KeyCode::Char('d') => {
                self.day = Some(None);
                self.zoom = true;
                self.scroll = 0;
            }
            KeyCode::Char('[') => self.step_day(-1),
            KeyCode::Char(']') => self.step_day(1),
            _ => {}
        }
        true
    }

    /// Copies the line that starts a new session from the selected one.
    fn handoff(&mut self) {
        let Some(i) = self.selected() else { return };
        let row = &self.entries[i].row;
        self.status = match crate::clipboard::copy(&tokprof_core::handoff::line(&self.catalog, &row.file)) {
            Some(_) => format!("copied the handoff line for {}: paste it into a new session", row.id),
            None => format!("no clipboard tool (pbcopy, wl-copy, xclip); run: tokprof handoff {}", row.id),
        };
    }

    /// Writes the selected session's profile on a worker thread; HTML opens in the browser.
    /// How far the full-screen X-ray scrolls: until its last line is at the bottom of the pane,
    /// which loses a footer line and two border lines from the terminal's height.
    fn max_scroll(&self, height: u16) -> u16 {
        let lines = match self.day {
            Some(key) => self.days.get(&key).map_or(0, |d| d.text.lines.len()),
            None => self.selected().and_then(|i| self.entries[i].report.as_ref()).map_or(0, |t| t.lines.len()),
        };
        lines.saturating_sub(height.saturating_sub(3) as usize).min(u16::MAX as usize) as u16
    }

    fn export(&mut self, ext: &str, open: bool) {
        let Some(i) = self.selected() else { return };
        let row = self.entries[i].row.clone();
        let dir = crate::paths::profile_dir();
        let path = dir.join(format!("{}.{ext}", row.id));
        self.status = format!("writing {}…", display_path(&path.to_string_lossy(), None, self.catalog.home()));
        let (catalog, pricing, tx) = (self.catalog.clone(), self.pricing.clone(), self.notes_tx.clone());
        std::thread::spawn(move || {
            let note = std::fs::create_dir_all(&dir)
                .map_err(anyhow::Error::from)
                .and_then(|_| xray::export(&catalog, &row, &pricing, &path))
                .map(|written| {
                    if open {
                        let opener = if cfg!(target_os = "macos") { "open" } else { "xdg-open" };
                        let _ = std::process::Command::new(opener).arg(&path).spawn();
                    }
                    format!("wrote {written}")
                })
                .unwrap_or_else(|e| format!("export failed: {e:#}"));
            let _ = tx.send(Note::Status(note));
        });
    }
}

fn draw(frame: &mut Frame, app: &mut App) {
    let [body, footer] = Layout::vertical([Constraint::Fill(1), Constraint::Length(1)]).areas(frame.area());
    let report = if app.zoom {
        body
    } else {
        let [left, right] = Layout::horizontal([Constraint::Length(LIST_WIDTH), Constraint::Fill(1)]).areas(body);
        draw_list(frame, app, left);
        right
    };
    app.pane_width = report.width;
    if app.day.is_some() {
        draw_day(frame, app, report);
    } else {
        draw_report(frame, app, report);
    }
    draw_footer(frame, app, footer);
}

/// The columns a day strip may take in a pane this wide: less the borders and the lane labels.
fn strip_columns(width: u16) -> i64 {
    (width as i64 - 23).clamp(12, rhythm::STRIP_COLUMNS)
}

fn draw_day(frame: &mut Frame, app: &App, area: Rect) {
    let key = app.day.flatten();
    let shown = app.day.and_then(|k| app.days.get(&k));
    let offset = &rhythm::local_offset;
    let date = shown.map(|d| d.date).or(key).unwrap_or_else(|| rhythm::date_of(now_ms(), offset));
    let noon = rhythm::midnight(date, offset) + 12 * 3_600_000;
    let updating = if shown.is_some() && app.day_pending.is_some_and(|p| Some(p.0) == app.day) { " · updating…" } else { "" };
    let title = format!(" day · {}{updating} ", tokprof_core::fmt::local_time(noon, "%a %Y-%m-%d"));
    let text = shown.map(|d| d.text.clone()).unwrap_or_else(|| Text::raw(format!("computing the day of {}…", tokprof_core::fmt::local_time(noon, "%Y-%m-%d"))).dim());
    frame.render_widget(Paragraph::new(text).block(Block::bordered().title(title)).scroll((app.scroll, 0)), area);
}

fn draw_list(frame: &mut Frame, app: &mut App, area: Rect) {
    // date 11, agent 6, cost 9, next 12, gaps 4, highlight 2, borders 2
    let project_width = area.width.saturating_sub(46) as usize;
    let now = now_ms();
    let items: Vec<ListItem> = app
        .visible
        .iter()
        .map(|&i| {
            let r = &app.entries[i].row;
            let agent_color = AGENT_COLORS[app.catalog.order(&r.file.agent).0 % AGENT_COLORS.len()];
            let g = app.entries[i].glance;
            let cost = g.map_or_else(|| "…".to_string(), |g| usd(g.cost));
            let next = match g.as_ref().and_then(|g| crate::render_sessions::next_cell(g, now)) {
                Some((cell, cold)) => Span::styled(format!("{cell:>12}"), Style::new().fg(if cold { Color::Red } else { Color::Green })),
                None => Span::raw(" ".repeat(12)),
            };
            ListItem::new(Line::from(vec![
                Span::raw(tokprof_core::fmt::local_time(r.active_ms, "%m-%d %H:%M")).dim(),
                Span::raw(" "),
                Span::styled(format!("{:<6}", r.file.agent.name()), Style::new().fg(agent_color)),
                Span::raw(" "),
                Span::raw(format!("{:<width$}", clip_left(&r.project, project_width), width = project_width)),
                Span::raw(" "),
                Span::raw(format!("{cost:>9}")).bold(),
                next,
            ]))
        })
        .collect();
    let known: f64 = app.visible.iter().filter_map(|&i| app.entries[i].glance.map(|g| g.cost)).sum();
    let pending = app.visible.iter().filter(|&&i| app.entries[i].glance.is_none()).count();
    let title = format!(
        " {} sessions · {} · by {}{}{} · next = one more request ",
        app.visible.len(),
        usd(known),
        app.sort.name(),
        if app.out_of_date(now) { ", r re-sorts" } else { "" },
        if pending > 0 { format!(" · costing {pending}…") } else { String::new() }
    );
    let list = List::new(items)
        .block(Block::bordered().title(title))
        .highlight_style(Style::new().add_modifier(Modifier::REVERSED))
        .highlight_symbol("▸ ");
    frame.render_stateful_widget(list, area, &mut app.list);
}

fn draw_report(frame: &mut Frame, app: &App, area: Rect) {
    let (title, text) = match app.selected() {
        Some(i) => {
            let (e, r) = (&app.entries[i], &app.entries[i].row);
            let text = e.report.clone().unwrap_or_else(|| {
                Text::raw(format!("computing the X-ray of {} ({:.1} MB)…", r.id, r.bytes as f64 / 1e6)).dim()
            });
            // The session grew since this X-ray; a new one follows.
            let updating = if e.report.is_some() && e.reported < e.version { " · updating…" } else { "" };
            (format!(" {} {} · {}{updating} ", r.file.agent.name(), r.id, r.project), text)
        }
        None if app.filter.is_empty() => (" X-ray ".to_string(), Text::raw("no sessions")),
        None => (" X-ray ".to_string(), Text::raw("no session matches the filter")),
    };
    frame.render_widget(Paragraph::new(text).block(Block::bordered().title(title)).scroll((app.scroll, 0)), area);
}

fn draw_footer(frame: &mut Frame, app: &App, area: Rect) {
    let line = if app.editing {
        Line::from(vec![Span::raw(" filter: ").dim(), Span::raw(format!("{}▌", app.filter)), Span::raw("   ⏎ done · esc clear").dim()])
    } else {
        let keys = if app.zoom {
            " ↑↓ PgUp PgDn scroll · ← back · f JFR · o HTML · c handoff · q quit"
        } else {
            " ↑↓ move · ⏎ zoom · / filter · s sort: time, cost, next · r re-sort · f JFR for IntelliJ · o HTML · c handoff · q quit"
        };
        let days = if app.day.is_some() { " · d day · [ ] days" } else { " · d day" };
        let mut spans = vec![Span::raw(format!("{keys}{days}")).dim()];
        if !app.filter.is_empty() {
            spans.push(Span::raw(format!("   filter: {}", app.filter)));
        }
        if !app.status.is_empty() {
            spans.push(Span::raw(format!("   {}", app.status)).fg(Color::Yellow));
        }
        Line::from(spans)
    };
    frame.render_widget(Paragraph::new(line), area);
}

/// The order of the list: the larger value first, then the session listed first.
fn order(keys: &[f64], a: usize, b: usize) -> Ordering {
    keys[b].total_cmp(&keys[a]).then(a.cmp(&b))
}

fn now_ms() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis() as i64)
}

fn clip_left(s: &str, width: usize) -> String {
    let n = s.chars().count();
    if n <= width {
        return s.to_string();
    }
    let tail: String = s.chars().skip(n + 1 - width).collect();
    format!("…{tail}")
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

    fn app_of(rows: Vec<SessionRow>) -> App {
        let (xray_tx, _) = mpsc::channel();
        let (notes_tx, _) = mpsc::channel();
        let (day_tx, _) = mpsc::channel();
        App::new(Arc::new(Catalog::new(Vec::new(), PathBuf::new())), rows, Arc::new(Pricing::defaults()), xray_tx, notes_tx, day_tx)
    }

    fn app(projects: &[&str]) -> App {
        app_of(projects.iter().enumerate().map(|(i, p)| row(i, p, 0)).collect())
    }

    /// 30 sessions, session `i` last active `1000 - i` seconds after the epoch: listed 0, 1, 2…
    fn thirty() -> App {
        app_of((0..30).map(|i| row(i, "~/p", 1000 - i as u64)).collect())
    }

    /// The first and the last line of the ui drawn 160 columns wide.
    fn title_and_footer(app: &mut App) -> (String, String) {
        let mut t = Terminal::new(ratatui::backend::TestBackend::new(160, 20)).unwrap();
        t.draw(|f| draw(f, app)).unwrap();
        let b = t.backend().buffer();
        let line = |y| (0..160).map(|x| b[(x, y)].symbol()).collect::<String>();
        (line(0), line(19))
    }

    fn press(app: &mut App, code: KeyCode) -> bool {
        app.on_key(KeyEvent::new(code, KeyModifiers::NONE), 30)
    }

    fn take(app: &mut App, change: Change) {
        let (tx, rx) = mpsc::channel();
        tx.send(Note::Change(change)).unwrap();
        app.absorb(&rx);
    }

    #[test]
    fn the_list_follows_the_logs() {
        let mut a = app(&["~/src/a", "~/src/b"]);
        press(&mut a, KeyCode::Down);
        a.entries[1].report = Some(Text::raw("X-ray of b"));
        a.entries[1].reported = 1;

        take(&mut a, Change::Session(Box::new(row(2, "~/src/c", 10))));
        assert_eq!((a.visible.clone(), a.selected()), (vec![2, 0, 1], Some(1)), "a new session goes on top; the cursor stays on its session");

        take(&mut a, Change::Session(Box::new(row(0, "~/src/a", 0))));
        assert_eq!(a.entries[0].version, 1, "an unchanged session stays as it is");

        take(&mut a, Change::Session(Box::new(row(1, "~/src/b", 20))));
        assert_eq!((a.visible.clone(), a.selected()), (vec![2, 0, 1], Some(1)), "a session that grew keeps its row");
        assert_eq!(a.entries[1].row.active_ms, 20_000, "with its new time");
        assert_eq!((a.entries[1].version, a.entries[1].reported), (2, 1));
        a.request_report();
        assert_eq!(a.pending, Some((1, 2)), "and is X-rayed again");

        press(&mut a, KeyCode::Home);
        take(&mut a, Change::Gone("/x/2.jsonl".into()));
        assert_eq!((a.visible.clone(), a.selected()), (vec![0, 1], Some(0)), "a gone session leaves the cursor where it was");

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

        press(&mut a, KeyCode::Char('s'));
        let by_cost = a.visible.clone();
        let (tx, rx) = mpsc::channel();
        for i in [29, 5, 17] {
            let g = xray::Glance { cost: i as f64, ..xray::Glance::default() };
            tx.send(Note::Glance(i, a.entries[i].version, g, Duration::ZERO)).unwrap();
        }
        a.absorb(&rx);
        assert_eq!(a.visible, by_cost, "costs that arrive in cost order fill in their cells and move no row");
        assert_eq!(a.entries[29].glance.map(|g| g.cost), Some(29.0));
    }

    #[test]
    fn rows_on_the_screen_stay_when_rows_come_or_go_above_it() {
        let mut a = thirty();
        *a.list.offset_mut() = 10;
        a.list.select(Some(15));
        let screen = |a: &App| a.visible[a.list.offset()..a.list.offset() + 10].to_vec();
        let shown = screen(&a);

        take(&mut a, Change::Session(Box::new(row(30, "~/p", 5000))));
        assert_eq!(a.visible[0], 30, "a new session goes on top in time order");
        assert_eq!((screen(&a), a.selected()), (shown.clone(), Some(15)), "the rows on the screen stay, and so does the selection");

        take(&mut a, Change::Gone("/x/3.jsonl".into()));
        assert_eq!((screen(&a), a.selected()), (shown.clone(), Some(15)), "a session that leaves above the screen moves nothing on it");

        take(&mut a, Change::Gone("/x/12.jsonl".into()));
        assert_eq!(screen(&a)[..2], shown[..2], "a session that leaves on the screen keeps the rows above it");
        assert_eq!(screen(&a)[2..9], shown[3..10], "and pulls the rows below it up by one");
        assert_eq!(a.selected(), Some(15));
    }

    #[test]
    fn r_re_sorts_an_order_out_of_date() {
        let mut a = thirty();
        press(&mut a, KeyCode::Char('j'));
        let (title, footer) = title_and_footer(&mut a);
        assert!(title.contains("· by time · ") && !title.contains("r re-sorts"), "{title}");
        assert!(footer.contains("s sort: time, cost, next · r re-sort"), "{footer}");

        take(&mut a, Change::Session(Box::new(row(12, "~/p", 3000))));
        assert_eq!(a.visible[12], 12);
        let (title, _) = title_and_footer(&mut a);
        assert!(title.contains("· by time, r re-sorts · "), "a request in a session down the list puts the order out of date: {title}");

        press(&mut a, KeyCode::Char('r'));
        assert_eq!((a.visible[0], a.selected()), (12, Some(1)), "r puts that session on top and keeps the selection");
        let (title, _) = title_and_footer(&mut a);
        assert!(!title.contains("r re-sorts"), "{title}");

        // By next request, a cache that goes cold changes the order with no new note.
        let now = now_ms();
        for (i, e) in a.entries.iter_mut().enumerate() {
            let (warm, cold, cold_at) = if i == 7 { (0.1, 5.0, now + 60_000) } else { (1.0, 1.0, i64::MAX) };
            e.glance = Some(xray::Glance { cost: 10.0, next_warm: Some(warm), next_cold: Some(cold), cold_at_ms: cold_at });
        }
        press(&mut a, KeyCode::Char('s'));
        press(&mut a, KeyCode::Char('s'));
        assert_eq!(*a.visible.last().unwrap(), 7, "the cheapest next request goes last while the cache is warm");
        assert!(!a.out_of_date(now));
        assert!(a.out_of_date(now + 120_000), "once the cache is cold, the order is out of date");
        assert_eq!(*a.visible.last().unwrap(), 7, "and the row stays where it is");
    }

    #[test]
    fn keys_move_filter_sort_and_quit() {
        let mut a = app(&["~/src/a", "~/src/beta", "~/src/b2"]);
        assert_eq!(a.selected(), Some(0));
        press(&mut a, KeyCode::Down);
        press(&mut a, KeyCode::Down);
        press(&mut a, KeyCode::Down);
        assert_eq!(a.selected(), Some(2), "stops at the last row");

        for c in "/b".chars() {
            press(&mut a, KeyCode::Char(c));
        }
        assert!(a.editing);
        assert_eq!(a.visible, vec![1, 2]);
        assert_eq!(a.selected(), Some(2), "keeps the selected session when it still matches");
        press(&mut a, KeyCode::Enter);
        press(&mut a, KeyCode::Esc);
        assert!(a.filter.is_empty() && a.visible.len() == 3);

        let glance = |cost, next| xray::Glance { cost, next_warm: Some(next), next_cold: Some(next), cold_at_ms: i64::MAX };
        for (e, g) in a.entries.iter_mut().zip([Some(glance(1.0, 9.0)), Some(glance(5.0, 0.1)), None]) {
            e.glance = g;
        }
        press(&mut a, KeyCode::Char('s'));
        assert_eq!(a.visible, vec![1, 0, 2], "most expensive first, unknown last");
        press(&mut a, KeyCode::Char('s'));
        assert_eq!(a.visible, vec![0, 1, 2], "dearest next request first");
        press(&mut a, KeyCode::Char('s'));
        press(&mut a, KeyCode::Char('s'));

        press(&mut a, KeyCode::Enter);
        assert!(a.zoom);
        // 40 lines in a pane of 27 (30 rows less the footer and the borders) scroll by 13.
        a.entries[2].report = Some(Text::raw(vec!["line"; 40].join("\n")));
        press(&mut a, KeyCode::Down);
        assert_eq!((a.scroll, a.selected()), (1, Some(2)), "arrows scroll the report when zoomed");
        for _ in 0..3 {
            press(&mut a, KeyCode::PageDown);
        }
        assert_eq!(a.scroll, 13, "scrolling stops at the last page");
        assert!(press(&mut a, KeyCode::Esc) && !a.zoom);
        assert!(!press(&mut a, KeyCode::Char('q')));
    }

    /// The day view the worker would send for `key`, showing `date`.
    fn day_note(app: &mut App, key: Option<i64>, date: i64, changes: u32, text: &str) {
        let (tx, rx) = mpsc::channel();
        tx.send(Note::Day(key, date, changes, app.pane_width, 4, text.to_string(), Duration::ZERO)).unwrap();
        app.absorb(&rx);
    }

    /// The right pane's title and first line, drawn 160 columns wide.
    fn pane(app: &mut App) -> (String, String) {
        let mut t = Terminal::new(ratatui::backend::TestBackend::new(160, 20)).unwrap();
        t.draw(|f| draw(f, app)).unwrap();
        let b = t.backend().buffer();
        let from = if app.zoom { 0 } else { LIST_WIDTH };
        let line = |y| (from..160).map(|x| b[(x, y)].symbol()).collect::<String>();
        (line(0), line(1))
    }

    fn today() -> i64 {
        rhythm::date_of(now_ms(), &rhythm::local_offset)
    }

    fn label(date: i64, format: &str) -> String {
        tokprof_core::fmt::local_time(rhythm::midnight(date, &rhythm::local_offset) + 12 * 3_600_000, format)
    }

    #[test]
    fn d_shows_the_day_and_d_again_the_xray() {
        let mut a = app(&["~/src/a"]);
        press(&mut a, KeyCode::Char('d'));
        let (title, body) = pane(&mut a);
        assert!(title.contains(&format!(" day · {} ", label(today(), "%a %Y-%m-%d"))), "{title}");
        assert!(body.contains(&format!("computing the day of {}…", label(today(), "%Y-%m-%d"))), "{body}");
        a.request_day();
        assert!(a.zoom, "the day takes the whole screen");
        assert_eq!(a.day_pending, Some((None, 0, 160, None)), "the day is asked for at the screen's width");
        day_note(&mut a, None, today() - 1, 0, "tokprof · day\nday       …");
        let (title, body) = pane(&mut a);
        assert!(title.contains(&format!(" day · {} ", label(today() - 1, "%a %Y-%m-%d"))), "the latest action's day: {title}");
        assert!(body.contains("tokprof · day"), "{body}");
        assert_eq!(a.day_axis, Some(4), "the first day sets the hour strips start at");
        press(&mut a, KeyCode::Char('d'));
        assert!(a.day.is_none() && !a.zoom, "d goes back to the list with the X-ray");
        assert!(!pane(&mut a).0.contains(" day · "));
    }

    #[test]
    fn brackets_step_through_days_but_not_past_the_latest() {
        let mut a = app(&["~/src/a"]);
        press(&mut a, KeyCode::Char('['));
        assert_eq!(a.day, None, "the brackets need the day view");
        press(&mut a, KeyCode::Char('d'));
        press(&mut a, KeyCode::Char('['));
        assert_eq!(a.day, Some(None), "the latest day is not known before it is computed");
        pane(&mut a);
        day_note(&mut a, None, 100, 0, "latest");
        press(&mut a, KeyCode::Char('['));
        press(&mut a, KeyCode::Char('['));
        press(&mut a, KeyCode::Char(']'));
        assert_eq!(a.day, Some(Some(99)));
        press(&mut a, KeyCode::Char(']'));
        assert_eq!(a.day, Some(None), "back on the latest day");
        press(&mut a, KeyCode::Char(']'));
        assert_eq!(a.day, Some(None), "] on the latest day does nothing");
    }

    #[test]
    fn the_day_goes_full_screen_and_the_footer_names_its_keys() {
        let mut a = app(&["~/src/a"]);
        assert!(title_and_footer(&mut a).1.contains(" · q quit · d day"), "the list's footer");
        press(&mut a, KeyCode::Char('d'));
        pane(&mut a);
        day_note(&mut a, None, 100, 0, &vec!["line"; 40].join("\n"));
        let footer = title_and_footer(&mut a).1;
        assert!(footer.contains(" · d day · [ ] days"), "{footer}");
        press(&mut a, KeyCode::Enter);
        assert!(a.zoom);
        let (title, _) = pane(&mut a);
        assert!(title.starts_with("┌ day · "), "full screen: {title}");
        press(&mut a, KeyCode::Down);
        assert_eq!(a.scroll, 1, "the full-screen day scrolls");
        press(&mut a, KeyCode::Char('['));
        assert_eq!((a.day, a.zoom, a.scroll), (Some(Some(99)), true, 0), "[ works full screen");
        pane(&mut a);
        a.request_day();
        assert_eq!(a.day_pending, Some((Some(99), 0, 160, Some(4))), "the day before starts its strip at the first day's hour");
        for key in [KeyCode::Esc, KeyCode::Left, KeyCode::Char('h'), KeyCode::Backspace] {
            if a.day.is_none() {
                press(&mut a, KeyCode::Char('d'));
            }
            assert!(a.day.is_some() && a.zoom);
            assert!(press(&mut a, key), "{key:?} does not quit");
            assert_eq!((a.day, a.zoom), (None, false), "{key:?} goes back to the list");
        }
    }

    #[test]
    fn the_current_day_follows_the_logs_and_a_past_day_stays() {
        let mut a = app(&["~/src/a"]);
        press(&mut a, KeyCode::Char('d'));
        pane(&mut a);
        day_note(&mut a, None, 100, 0, "latest");
        a.request_day();
        assert_eq!(a.day_pending, None, "fresh: nothing to ask");
        take(&mut a, Change::Session(Box::new(row(0, "~/src/a", 5))));
        a.request_day();
        assert_eq!(a.day_pending, Some((None, 1, 160, Some(4))), "a log changed: computed again");
        assert!(pane(&mut a).0.contains(" · updating…"), "the old view shows until the new one is ready");
        day_note(&mut a, None, 100, 1, "latest again");
        assert!(!pane(&mut a).0.contains("updating"));
        // A past day, once computed, shows at once though logs changed since.
        press(&mut a, KeyCode::Char('['));
        pane(&mut a);
        day_note(&mut a, Some(99), 99, 1, "the day before");
        take(&mut a, Change::Session(Box::new(row(0, "~/src/a", 6))));
        press(&mut a, KeyCode::Char(']'));
        press(&mut a, KeyCode::Char('['));
        a.request_day();
        assert_eq!(a.day_pending, None, "the past day is not asked for again");
        assert!(pane(&mut a).1.contains("the day before"), "and shows at once");
        press(&mut a, KeyCode::Char(']'));
        a.request_day();
        assert_eq!(a.day_pending, Some((None, 2, 160, Some(4))), "the current day is, since logs changed");
    }
}
