//! Interactive session browser, `tokprof ui`: the sessions on the left, the X-ray of the
//! selected one on the right. Costs fill in from a background pass over all logs; X-rays are
//! computed on a worker thread, newest request first, and kept. The list follows the log
//! directories: new sessions show up, and a session that is still being written is costed and
//! X-rayed again as it grows.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
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

use crate::adapters::{self, SourceFile};
use crate::model::Agent;
use crate::pricing::Pricing;
use crate::report::usd;
use crate::watch::{self, Change};
use crate::xray::{self, SessionRow};

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
}

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
    xray_tx: Sender<(usize, u32, SourceFile)>,
    notes_tx: Sender<Note>,
}

pub fn run(agents: &[Agent], pricing: Pricing, dump: Option<(u16, u16)>) -> Result<()> {
    let (notes_tx, notes_rx) = mpsc::channel();
    // The log directories are watched from before the sessions are listed, so that no change
    // is missed.
    let following = match dump {
        Some(_) => Ok(()),
        None => {
            let tx = notes_tx.clone();
            watch::watch(adapters::roots(agents), move |change| tx.send(Note::Change(change)).is_ok())
        }
    };
    let rows = xray::rows(agents, usize::MAX);
    if rows.is_empty() {
        bail!("no session logs found");
    }
    let pricing = Arc::new(pricing);

    // X-rays, one at a time; a newer request replaces the ones still waiting.
    let (xray_tx, xray_rx) = mpsc::channel::<(usize, u32, SourceFile)>();
    {
        let (pricing, tx) = (pricing.clone(), notes_tx.clone());
        std::thread::spawn(move || {
            while let Ok(mut job) = xray_rx.recv() {
                while let Ok(newer) = xray_rx.try_recv() {
                    job = newer;
                }
                let (i, version, file) = job;
                let started = Instant::now();
                let text = xray::xray(&file, &pricing, &xray::Style::with_color(true)).unwrap_or_else(|e| format!("error: {e:#}"));
                if tx.send(Note::Report(i, version, text, started.elapsed())).is_err() {
                    return;
                }
            }
        });
    }

    let mut app = App::new(rows, pricing, xray_tx, notes_tx);
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
    fn new(rows: Vec<SessionRow>, pricing: Arc<Pricing>, xray_tx: Sender<(usize, u32, SourceFile)>, notes_tx: Sender<Note>) -> App {
        let by_path = rows.iter().enumerate().map(|(i, r)| (r.file.path.clone(), i)).collect();
        let mut app = App {
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
        };
        app.refilter();
        app
    }

    fn selected(&self) -> Option<usize> {
        self.list.selected().and_then(|s| self.visible.get(s).copied())
    }

    fn absorb(&mut self, notes: &Receiver<Note>) {
        let (mut costs_changed, mut listed) = (false, false);
        while let Ok(note) = notes.try_recv() {
            match note {
                Note::Glance(i, version, g, took) => {
                    let e = &mut self.entries[i];
                    e.glance = Some(g);
                    e.glanced = version;
                    e.glancing = false;
                    e.glance_after = Instant::now() + took;
                    costs_changed = true;
                }
                Note::Report(i, version, text, took) => {
                    let e = &mut self.entries[i];
                    e.report = Some(text.into_text().unwrap_or_else(|_| Text::raw(text)));
                    e.reported = version;
                    self.report_after = Instant::now() + took;
                }
                Note::Change(change) => listed |= self.apply(change),
                Note::Status(s) => self.status = s,
            }
        }
        if listed {
            // When the selected session is gone, the cursor stays where it was.
            let (kept, at) = (self.selected(), self.list.selected());
            self.refilter();
            if self.selected() != kept
                && let Some(at) = at
                && !self.visible.is_empty()
            {
                self.list.select(Some(at.min(self.visible.len() - 1)));
                self.scroll = 0;
            }
        } else if costs_changed && self.sort != Sort::Time {
            self.sort();
        }
    }

    /// Takes in a change of the log directories; returns whether the list changed.
    fn apply(&mut self, change: Change) -> bool {
        let mut changed = false;
        match change {
            Change::Session(row) => changed = self.upsert(row),
            Change::Gone(path) => {
                for e in self.entries.iter_mut().filter(|e| !e.gone && e.row.file.path.starts_with(&path)) {
                    e.gone = true;
                    changed = true;
                }
            }
            Change::All(rows) => {
                let found: HashSet<&PathBuf> = rows.iter().map(|r| &r.file.path).collect();
                for e in self.entries.iter_mut().filter(|e| !e.gone && !found.contains(&e.row.file.path)) {
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
            if let Some(&j) = self.by_path.get(&sub.path)
                && !self.entries[j].gone
            {
                self.entries[j].gone = true;
                changed = true;
            }
        }
        let Some(&i) = self.by_path.get(&row.file.path) else {
            self.by_path.insert(row.file.path.clone(), self.entries.len());
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
        let (pricing, tx) = (self.pricing.clone(), self.notes_tx.clone());
        std::thread::spawn(move || {
            due.into_par_iter().for_each_with(tx, |tx, (i, version, row)| {
                let started = Instant::now();
                let g = xray::glance(&row, &pricing);
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

    fn refilter(&mut self) {
        let keep = self.selected();
        let needle = self.filter.to_lowercase();
        self.visible = (0..self.entries.len())
            .filter(|&i| {
                let (e, r) = (&self.entries[i], &self.entries[i].row);
                !e.gone && (needle.is_empty() || format!("{} {} {}", r.file.agent.name(), r.project, r.id).to_lowercase().contains(&needle))
            })
            .collect();
        self.sort_keeping(keep);
    }

    fn sort(&mut self) {
        self.sort_keeping(self.selected());
    }

    fn sort_keeping(&mut self, keep: Option<usize>) {
        let now = now_ms();
        let key = |e: &Entry| match self.sort {
            Sort::Time => e.row.modified_ms() as f64,
            Sort::Cost => e.glance.map_or(-1.0, |g| g.cost),
            Sort::Next => e.glance.and_then(|g| g.next(now)).map_or(-1.0, |(usd, _)| usd),
        };
        let keys: Vec<f64> = self.entries.iter().map(key).collect();
        self.visible.sort_by(|&a, &b| keys[b].total_cmp(&keys[a]).then(a.cmp(&b)));
        let at = keep.and_then(|k| self.visible.iter().position(|&i| i == k));
        self.list.select(if self.visible.is_empty() { None } else { Some(at.unwrap_or(0)) });
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
            KeyCode::Esc if self.zoom => self.zoom = false,
            KeyCode::Esc if !self.filter.is_empty() => {
                self.filter.clear();
                self.refilter();
            }
            KeyCode::Esc => return false,
            KeyCode::Up | KeyCode::Char('k') if self.zoom => self.scroll = self.scroll.saturating_sub(1),
            KeyCode::Down | KeyCode::Char('j') if self.zoom => self.scroll = self.scroll.saturating_add(1),
            KeyCode::PageUp if self.zoom => self.scroll = self.scroll.saturating_sub(page),
            KeyCode::PageDown | KeyCode::Char(' ') if self.zoom => self.scroll = self.scroll.saturating_add(page),
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
            KeyCode::Char('f') => self.export("jfr", false),
            KeyCode::Char('o') => self.export("html", true),
            KeyCode::Char('c') => self.handoff(),
            _ => {}
        }
        true
    }

    /// Copies the line that starts a new session from the selected one.
    fn handoff(&mut self) {
        let Some(i) = self.selected() else { return };
        let row = &self.entries[i].row;
        self.status = match crate::handoff::copy(&crate::handoff::line(&row.file)) {
            Some(_) => format!("copied the handoff line for {}: paste it into a new session", row.id),
            None => format!("no clipboard tool (pbcopy, wl-copy, xclip); run: tokprof handoff {}", row.id),
        };
    }

    /// Writes the selected session's profile on a worker thread; HTML opens in the browser.
    fn export(&mut self, ext: &str, open: bool) {
        let Some(i) = self.selected() else { return };
        let row = self.entries[i].row.clone();
        let dir = crate::export::default_dir();
        let path = dir.join(format!("{}.{ext}", row.id));
        self.status = format!("writing {}…", adapters::display_path(&path.to_string_lossy()));
        let (pricing, tx) = (self.pricing.clone(), self.notes_tx.clone());
        std::thread::spawn(move || {
            let note = std::fs::create_dir_all(&dir)
                .map_err(anyhow::Error::from)
                .and_then(|_| xray::export(&row, &pricing, &path))
                .map(|written| {
                    if open {
                        let _ = std::process::Command::new("open").arg(&path).spawn();
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
    draw_report(frame, app, report);
    draw_footer(frame, app, footer);
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
            let agent_color = if r.file.agent == Agent::Codex { Color::Green } else { Color::Magenta };
            let g = app.entries[i].glance;
            let cost = g.map_or_else(|| "…".to_string(), |g| usd(g.cost));
            let next = match g.as_ref().and_then(|g| xray::next_cell(g, now)) {
                Some((cell, cold)) => Span::styled(format!("{cell:>12}"), Style::new().fg(if cold { Color::Red } else { Color::Green })),
                None => Span::raw(" ".repeat(12)),
            };
            ListItem::new(Line::from(vec![
                Span::raw(xray::local_time(r.modified_ms(), "%m-%d %H:%M")).dim(),
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
        " {} sessions · {} · by {}{} · next = one more request ",
        app.visible.len(),
        usd(known),
        app.sort.name(),
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
            " ↑↓ move · ⏎ zoom · / filter · s sort: time, cost, next · f JFR for IntelliJ · o HTML · c handoff · q quit"
        };
        let mut spans = vec![Span::raw(keys).dim()];
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
    use std::time::SystemTime;

    /// Session `i` in `project`, its log last written `at` seconds after the epoch.
    fn row(i: usize, project: &str, at: u64) -> SessionRow {
        let modified = SystemTime::UNIX_EPOCH + Duration::from_secs(at);
        SessionRow {
            file: SourceFile { agent: Agent::Claude, path: format!("/x/{i}.jsonl").into(), size: 1 + at, modified },
            subagents: Vec::new(),
            project: project.to_string(),
            id: format!("id{i}"),
            bytes: 1 + at,
        }
    }

    fn app(projects: &[&str]) -> App {
        let rows = projects.iter().enumerate().map(|(i, p)| row(i, p, 0)).collect();
        let (xray_tx, _) = mpsc::channel();
        let (notes_tx, _) = mpsc::channel();
        App::new(rows, Arc::new(Pricing::defaults()), xray_tx, notes_tx)
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

        take(&mut a, Change::Session(row(2, "~/src/c", 10)));
        assert_eq!((a.visible.clone(), a.selected()), (vec![2, 0, 1], Some(1)), "a new session goes on top; the cursor stays on its session");

        take(&mut a, Change::Session(row(0, "~/src/a", 0)));
        assert_eq!(a.entries[0].version, 1, "an unchanged session stays as it is");

        take(&mut a, Change::Session(row(1, "~/src/b", 20)));
        assert_eq!((a.visible.clone(), a.selected()), (vec![1, 2, 0], Some(1)), "a session that grew moves up");
        assert_eq!((a.entries[1].version, a.entries[1].reported), (2, 1));
        a.request_report();
        assert_eq!(a.pending, Some((1, 2)), "and is X-rayed again");

        press(&mut a, KeyCode::Down);
        take(&mut a, Change::Gone("/x/2.jsonl".into()));
        assert_eq!((a.visible.clone(), a.selected()), (vec![1, 0], Some(0)), "a gone session leaves the cursor where it was");

        take(&mut a, Change::All(vec![row(0, "~/src/a", 0), row(2, "~/src/c", 30)]));
        assert_eq!(a.visible, vec![2, 0], "a rescan drops the sessions that are gone and brings back the ones that are there");
        assert_eq!(a.entries[2].version, 2);

        let mut parent = row(0, "~/src/a", 40);
        parent.subagents.push(row(2, "~/src/c", 30).file);
        take(&mut a, Change::Session(parent));
        assert_eq!(a.visible, vec![0], "a log that turns out to be a subagent's leaves the list");
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
        press(&mut a, KeyCode::Down);
        assert_eq!((a.scroll, a.selected()), (1, Some(2)), "arrows scroll the report when zoomed");
        assert!(press(&mut a, KeyCode::Esc) && !a.zoom);
        assert!(!press(&mut a, KeyCode::Char('q')));
    }
}
