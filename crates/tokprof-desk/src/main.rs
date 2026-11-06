//! `tokprof-desk`: the session browser of tokprof in a native window, laid out as a profiler. The
//! browser of `tokprof-browse` holds the sessions, their costs, X-rays and days, as it does for
//! `tokprof ui`; this draws them with Rune and passes the window's keys and clicks on.

mod ansi;
mod day;
mod inspector;
mod pal;
mod view;

use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use anyhow::{Result, bail};
use clap::Parser;
use rune::{App, AppSpec, Chrome, Key as RuneKey, KeyEvent, Tx, Waker};

use tokprof_browse::browser::{Browser, Frontend, Key, Viewport, Wake, now_ms};
use tokprof_browse::filters::{normalize_project, parse_date, parse_since};
use tokprof_browse::paths;
use tokprof_core::catalog::Catalog;
use tokprof_core::query::Query;
use tokprof_core::rhythm;
use tokprof_core::source::AgentId;

use crate::ansi::Styled;
use crate::inspector::Tab;

#[derive(Parser, Debug)]
#[command(
    name = "tokprof-desk",
    version,
    about = "The session browser of tokprof in a native window",
    after_help = "Rune's flags render without a window: --snap out.png, --tree out.json, --size WxH (default 1400x900), --theme light|dark."
)]
struct Cli {
    /// Agents to read, comma separated [default: every agent tokprof reads].
    #[arg(long, value_delimiter = ',')]
    agent: Vec<String>,
    /// Only sessions active on or after this date (YYYY-MM-DD) or this many days back (7d).
    #[arg(long)]
    since: Option<String>,
    /// Only sessions created before this date (YYYY-MM-DD).
    #[arg(long)]
    until: Option<String>,
    /// Only projects whose path contains this text.
    #[arg(long)]
    project: Option<String>,
    /// Only the session with this id prefix.
    #[arg(long)]
    session: Option<String>,
    /// Only sessions whose client, the program that ran the agent, contains this text.
    #[arg(long)]
    client: Option<String>,
    /// TOML price file applied over the built-in prices and ~/.config/tokprof/prices.toml.
    #[arg(long)]
    prices: Option<PathBuf>,
    /// With --snap or --tree: show the day of this date (YYYY-MM-DD) in the lanes.
    #[arg(long)]
    day: Option<String>,
    /// With --snap or --tree: show this tab of the inspector (summary, requests, spend, work,
    /// waits, prompts, tools, context, cache).
    #[arg(long)]
    tab: Option<String>,
    /// Stay in the terminal until the window closes, instead of starting it on its own.
    #[arg(long)]
    wait: bool,
}

/// Where the window runs when the command starts.
#[derive(Debug, PartialEq, Eq)]
enum Start {
    /// In this process: a frame without a window, `--wait`, or a start without a terminal, as
    /// Launch Services and the detached process start it.
    Here,
    /// The app bundle the binary is in, opened through Launch Services.
    Bundle(PathBuf),
    /// This binary again, as a process of its own.
    Detached,
}

/// How a start goes: from a terminal, the window starts on its own and the terminal comes back.
fn start_of(exe: &std::path::Path, headless: bool, wait: bool, terminal: bool) -> Start {
    if headless || wait || !terminal {
        return Start::Here;
    }
    match bundle_of(exe) {
        Some(app) => Start::Bundle(app),
        None => Start::Detached,
    }
}

/// The app bundle an executable is in, as `<name>.app/Contents/MacOS/<executable>`.
fn bundle_of(exe: &std::path::Path) -> Option<PathBuf> {
    let macos = exe.parent()?;
    let contents = macos.parent()?;
    let app = contents.parent()?;
    (macos.file_name()? == "MacOS" && contents.file_name()? == "Contents" && app.extension()? == "app").then(|| app.to_path_buf())
}

/// Where the window started on its own writes what it prints.
fn log_path() -> PathBuf {
    std::env::var_os("HOME").map_or_else(std::env::temp_dir, PathBuf::from).join("Library/Logs/tokprof-desk.log")
}

/// Starts the window on its own with these arguments, and returns.
fn start_apart(start: &Start, exe: &std::path::Path, args: &[String]) -> Result<()> {
    use std::os::unix::process::CommandExt;
    use std::process::{Command, Stdio};
    match start {
        Start::Bundle(app) => {
            let status = Command::new("/usr/bin/open").arg("-a").arg(app).arg("--args").args(args).status()?;
            if !status.success() {
                bail!("could not open {}: {status}", app.display());
            }
        }
        Start::Detached => {
            let path = log_path();
            if let Some(dir) = path.parent() {
                std::fs::create_dir_all(dir)?;
            }
            let log = std::fs::OpenOptions::new().create(true).append(true).open(&path)?;
            let mut child = Command::new(exe);
            child.args(args).stdin(Stdio::null()).stdout(log.try_clone()?).stderr(log);
            // A session of its own: the terminal closing does not end it.
            // SAFETY: setsid is async-signal-safe, and nothing else runs between fork and exec.
            unsafe {
                child.pre_exec(|| {
                    libc::setsid();
                    Ok(())
                });
            }
            child.spawn()?;
        }
        Start::Here => {}
    }
    Ok(())
}

pub type Book = Browser<Styled>;

/// A slot of the list: a separator before the sessions of a day (its local date), or a session's
/// row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Slot {
    Day(i64),
    Row(usize),
}

/// The window's state: the browser and what the window keeps of its own.
pub struct Desk {
    pub browser: Book,
    /// Whether the window draws dark, as the system was when the view last ran; rows built later,
    /// as the list scrolls, read it.
    pub dark: Cell<bool>,
    /// The list's slots as the view last laid them out, for the rows it builds as it scrolls.
    pub slots: RefCell<Vec<Slot>>,
    /// How far the list and the inspector are scrolled, in points, as their viewports last said.
    pub list_offset: Rc<Cell<f32>>,
    pub pane_offset: Rc<Cell<f32>>,
    /// The heights of the list, the inspector and the window, from their layout.
    pub list_height: f32,
    pub pane_height: f32,
    pub window_h: f32,
    /// When a listed cache goes cold next, as the last poll found it.
    pub next_cold: Option<i64>,
    /// The browser's count of lists put in place again, as the last poll saw it.
    pub relisted: u32,
    /// Where the pointer rests over the lanes.
    pub pointer: Option<day::Pointer>,
    /// The inspector's tab.
    pub tab: Tab,
    /// The lanes show the hours of the day's actions (`Fit`), or its 24 hours.
    pub fit: bool,
    /// The window follows the logs.
    pub live: bool,
    /// The browser's latest status, and when it came.
    pub status: (String, Instant),
    /// The first slot on the screen is a day's separator, as the last poll found the list.
    pub separator_on_top: bool,
    /// The minute of the last poll: the lanes' now line and the inspector's times move at a new
    /// one.
    pub minute: i64,
    /// The pane that has the keyboard.
    pub pane: Focus,
    /// The row of the inspector's tab the cursor is on, and how many rows the tab has, as the
    /// inspector last drew it.
    pub cursor: usize,
    pub tab_rows: Cell<usize>,
}

/// The panes that take the keyboard, in the order Tab moves through them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Focus {
    Lanes,
    List,
    Inspector,
}

impl Focus {
    const ORDER: [Focus; 3] = [Focus::Lanes, Focus::List, Focus::Inspector];

    /// The next pane, or the one before.
    fn step(self, back: bool) -> Focus {
        let at = Focus::ORDER.iter().position(|&f| f == self).unwrap_or(1);
        Focus::ORDER[if back { (at + 2) % 3 } else { (at + 1) % 3 }]
    }
}

impl Desk {
    pub fn new(mut browser: Book, live: bool) -> Desk {
        // The window lays the day out itself; the browser computes it at once, beside the list.
        browser.keep_day_beside();
        browser.set_pane(view::DAY_PANE);
        let relisted = browser.relisted();
        Desk {
            browser,
            dark: Cell::new(true),
            slots: RefCell::default(),
            list_offset: Rc::default(),
            pane_offset: Rc::default(),
            list_height: 0.0,
            pane_height: 0.0,
            window_h: 0.0,
            next_cold: None,
            relisted,
            pointer: None,
            tab: Tab::Summary,
            fit: true,
            live,
            status: (String::new(), Instant::now()),
            separator_on_top: false,
            minute: 0,
            pane: Focus::List,
            cursor: 0,
            tab_rows: Cell::new(0),
        }
    }

    /// Shows tab `t` from its top, with the cursor on its first row.
    pub fn show_tab(&mut self, t: Tab, tx: &mut Tx) {
        self.tab = t;
        self.cursor = 0;
        tx.scroll_to_y(view::PANE, 0.0);
    }

    /// Selects the session of a lane, as a click on the lane does: in the list, scrolled to, and
    /// in the inspector from its top.
    pub fn open_lane(&mut self, id: &str, tx: &mut Tx) {
        if self.browser.open_session(id)
            && let Some(row) = self.browser.selected()
        {
            self.cursor = 0;
            tx.scroll_into_view(self.browser.session(row).to_string());
            tx.scroll_into_view(format!("lane-{id}"));
            tx.scroll_to_y(view::PANE, 0.0);
        }
    }

    /// The lanes of the day shown whose sessions are listed, in their order.
    fn listed_lanes(&self) -> Vec<String> {
        let Some((_, lanes)) = self.browser.day_parts().and_then(|p| p.strip.as_ref()) else { return Vec::new() };
        lanes.lanes.iter().filter(|l| self.browser.row_of(&l.id).is_some()).map(|l| l.id.clone()).collect()
    }

    /// The keys of the lanes while they have the keyboard: the lane above or below, the first or
    /// the last; the rest go on to the browser.
    fn lanes_key(&mut self, e: &KeyEvent, tx: &mut Tx) -> bool {
        let lanes = self.listed_lanes();
        let selected = self.browser.selected();
        let at = selected.and_then(|r| lanes.iter().position(|id| self.browser.row_of(id) == Some(r)));
        let Some(step) = lane_step(e) else { return false };
        // The keys that move through the lanes stay with them, lanes or none.
        if let Some(to) = lane_to(lanes.len(), at, step) {
            let id = lanes[to].clone();
            self.open_lane(&id, tx);
        }
        true
    }

    /// The keys of the inspector while it has the keyboard: the tab before or after or of a
    /// digit, and the cursor through the tab's rows, or the inspector scrolled where the tab has
    /// none; the rest go on to the browser.
    fn inspector_key(&mut self, e: &KeyEvent, tx: &mut Tx) -> bool {
        let at = Tab::ALL.iter().position(|&t| t == self.tab).unwrap_or(0);
        let text = match &e.key {
            RuneKey::Text(t) if plain(e) => Some(t.as_str()),
            _ => None,
        };
        match (&e.key, text) {
            (RuneKey::Left, _) | (_, Some("h")) => {
                self.show_tab(Tab::ALL[at.saturating_sub(1)], tx);
                return true;
            }
            (RuneKey::Right, _) | (_, Some("l")) => {
                self.show_tab(Tab::ALL[(at + 1).min(Tab::ALL.len() - 1)], tx);
                return true;
            }
            (_, Some(d)) if d.len() == 1 && ("1"..="9").contains(&d) => {
                self.show_tab(Tab::ALL[d.parse::<usize>().unwrap_or(1) - 1], tx);
                return true;
            }
            _ => {}
        }
        let rows = self.tab_rows.get();
        let step: Option<isize> = match (&e.key, text) {
            (RuneKey::Up, _) | (_, Some("k")) => Some(-1),
            (RuneKey::Down, _) | (_, Some("j")) => Some(1),
            (RuneKey::PageUp, _) => Some(-10),
            (RuneKey::PageDown, _) => Some(10),
            (RuneKey::Home, _) | (_, Some("g")) => Some(isize::MIN / 2),
            (RuneKey::End, _) | (_, Some("G")) => Some(isize::MAX / 2),
            _ => None,
        };
        if rows > 0
            && let Some(step) = step
        {
            self.cursor = (self.cursor as isize).saturating_add(step).clamp(0, rows as isize - 1) as usize;
            tx.scroll_into_view(format!("row-{}", self.cursor));
            return true;
        }
        // A tab without rows scrolls: a line, a page, or to its ends.
        match step {
            Some(-1) => tx.scroll_to_y(view::PANE, (self.pane_offset.get() - 20.0).max(0.0)),
            Some(1) => tx.scroll_to_y(view::PANE, self.pane_offset.get() + 20.0),
            Some(-10) => tx.scroll_page(view::PANE, -1),
            Some(10) => tx.scroll_page(view::PANE, 1),
            Some(s) if s < 0 => tx.scroll_to_y(view::PANE, 0.0),
            Some(_) => tx.scroll_page(view::PANE, 1000),
            None if text == Some(" ") => tx.scroll_page(view::PANE, 1),
            None => return false,
        }
        true
    }

    /// The slot of the list that row `row` sits in.
    fn slot_of(&self, row: usize) -> usize {
        self.slots.borrow().iter().position(|s| *s == Slot::Row(row)).unwrap_or(row)
    }

    /// The row in slot `slot`, or the next row after a separator.
    fn row_at(&self, slot: usize) -> usize {
        let slots = self.slots.borrow();
        slots.iter().skip(slot).find_map(|s| if let Slot::Row(r) = s { Some(*r) } else { None }).unwrap_or(0)
    }

    /// Scrolls the list so that its first row on the screen is the browser's, keeping how far
    /// into that row it was scrolled, and the separator above it when it was the first on the
    /// screen. The slots are laid out again for the list as it is now.
    fn restore_list(&self, tx: &mut Tx) {
        *self.slots.borrow_mut() = view::slots(self);
        let into = self.list_offset.get().rem_euclid(view::ROW);
        let mut slot = self.slot_of(self.browser.top());
        if self.separator_on_top && slot > 0 && matches!(self.slots.borrow().get(slot - 1), Some(Slot::Day(_))) {
            slot -= 1;
        }
        tx.scroll_to_y(view::LIST, slot as f32 * view::ROW + into);
    }

    /// Selects the session with this id, as a click on its row does; the inspector shows its
    /// top.
    pub fn select_session(&mut self, session: &str) {
        if let Some(row) = self.browser.row_of(session) {
            self.browser.select_row(row);
            self.cursor = 0;
        }
    }

    /// The rows of the list a page moves by, and the lines of the inspector.
    fn viewport(&self) -> Viewport {
        Viewport { page: ((self.list_height / view::ROW) as usize).max(1), lines: (self.pane_height / 20.0) as usize }
    }

    /// Whether the bar shows the browser's status: for some seconds after it came.
    pub fn status_shown(&self) -> bool {
        !self.status.0.is_empty() && self.status.1.elapsed() < Duration::from_secs(view::STATUS_SECS)
    }
}

/// How the window keeps the browser's texts: without their color codes, with the ranges of each
/// style. It draws the X-ray from the X-ray itself; the texts serve the day's lines and errors.
pub fn frontend() -> Frontend<Styled> {
    Frontend { show, lines: |s| s.lines, day_layout }
}

fn show(text: String) -> Styled {
    ansi::parse(&text)
}

/// The day's text layout: the window shows every lane, as wide as a wide terminal shows them.
fn day_layout(_: (u16, u16)) -> rhythm::Layout {
    rhythm::Layout { label: 32, columns: 144, rows: 2 * 200, space: false }
}

/// The waker Rune gives the window once it runs; the browser's workers call it after they send.
static WAKER: OnceLock<Waker> = OnceLock::new();

fn wake() {
    if let Some(w) = WAKER.get() {
        w();
    }
}

fn setup(_: &mut Desk, waker: Waker) {
    let _ = WAKER.set(waker);
}

/// Before every frame: takes in what the background work sent, and asks to be woken when a
/// listed cache goes cold, when held-back work is due, every second while a warm row counts down
/// on the screen, and when the status message is due to go.
fn poll(s: &mut Desk, _: f64, tx: &mut Tx) -> bool {
    // Where the list is scrolled to, unless the list was put in place since and the browser
    // knows better.
    if s.browser.relisted() == s.relisted {
        let slot = (s.list_offset.get() / view::ROW).max(0.0) as usize;
        let top = s.row_at(slot);
        s.separator_on_top = matches!(s.slots.borrow().get(slot), Some(Slot::Day(_)));
        s.browser.set_top(top);
    }
    let arrived = s.browser.tick();
    // Rune would keep the first row on the screen there whatever came or went; the browser keeps
    // the rows on the screen in place instead, as `tokprof ui` does, so the list scrolls to its
    // first row.
    if s.browser.relisted() != s.relisted {
        s.relisted = s.browser.relisted();
        s.restore_list(tx);
    }
    let now = now_ms();
    // A cache that went cold since turns its cell red now.
    let cooled = s.next_cold.is_some_and(|at| now >= at);
    s.next_cold = s.browser.next_cold(now);
    if let Some(at) = s.next_cold {
        tx.wake_after(Duration::from_millis((at - now).max(1) as u64));
    }
    if let Some(at) = s.browser.next_due() {
        tx.wake_after(at.saturating_duration_since(Instant::now()).max(Duration::from_millis(1)));
    }
    // A countdown on the screen moves every second; the parts that show time move every minute.
    let (top, page) = (s.browser.top(), s.viewport().page + 2);
    let counting = (top..(top + page).min(s.browser.len())).any(|r| s.browser.cells(r, now).warm_until.is_some());
    if counting {
        tx.wake_after(Duration::from_millis((1000 - now.rem_euclid(1000)) as u64 + 5));
    } else {
        tx.wake_after(Duration::from_millis((60_000 - now.rem_euclid(60_000)) as u64 + 5));
    }
    let minute = now / 60_000;
    let turned = s.minute != minute;
    s.minute = minute;
    if s.browser.status() != s.status.0 {
        s.status = (s.browser.status().to_string(), Instant::now());
    }
    if s.status_shown() {
        tx.wake_after(Duration::from_secs(view::STATUS_SECS).saturating_sub(s.status.1.elapsed()) + Duration::from_millis(5));
    }
    arrived || cooled || counting || turned
}

/// The browser's key for a key of the window, when it takes one.
fn key_of(e: &KeyEvent) -> Option<Key> {
    Some(match &e.key {
        RuneKey::Text(t) if e.mods.ctrl && t.eq_ignore_ascii_case("c") => Key::Interrupt,
        RuneKey::Text(t) if !e.mods.cmd && !e.mods.ctrl => {
            let mut chars = t.chars();
            let c = chars.next()?;
            if chars.next().is_some() {
                return None;
            }
            Key::Char(c)
        }
        RuneKey::Up => Key::Up,
        RuneKey::Down => Key::Down,
        RuneKey::Left => Key::Left,
        RuneKey::Right => Key::Right,
        RuneKey::PageUp => Key::PageUp,
        RuneKey::PageDown => Key::PageDown,
        RuneKey::Home => Key::Home,
        RuneKey::End => Key::End,
        RuneKey::Enter => Key::Enter,
        RuneKey::Escape => Key::Esc,
        RuneKey::Backspace => Key::Backspace,
        _ => return None,
    })
}

/// A key without Command, Control or Option.
fn plain(e: &KeyEvent) -> bool {
    !(e.mods.cmd || e.mods.ctrl || e.mods.alt)
}

/// How a key moves through the lanes: a lane up or down, or to the first or the last.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LaneStep {
    Up,
    Down,
    First,
    Last,
}

fn lane_step(e: &KeyEvent) -> Option<LaneStep> {
    Some(match &e.key {
        RuneKey::Up => LaneStep::Up,
        RuneKey::Down => LaneStep::Down,
        RuneKey::Text(t) if t == "k" && plain(e) => LaneStep::Up,
        RuneKey::Text(t) if t == "j" && plain(e) => LaneStep::Down,
        RuneKey::Home => LaneStep::First,
        RuneKey::End => LaneStep::Last,
        _ => return None,
    })
}

/// The lane a step goes to among `count`, from the selected session's lane `at` (none when its
/// session has no lane that day, which goes to the first).
fn lane_to(count: usize, at: Option<usize>, step: LaneStep) -> Option<usize> {
    let last = count.checked_sub(1)?;
    Some(match (step, at) {
        (LaneStep::First, _) | (_, None) => 0,
        (LaneStep::Last, _) => last,
        (LaneStep::Up, Some(i)) => i.saturating_sub(1),
        (LaneStep::Down, Some(i)) => (i + 1).min(last),
    })
}

fn on_key(s: &mut Desk, e: &KeyEvent, tx: &mut Tx) -> bool {
    if e.is_command('q') {
        tx.quit();
        return true;
    }
    if e.is_command('w') {
        tx.close_window();
        return true;
    }
    // The pane with the keyboard takes its keys first; a filter being typed takes them all.
    if !s.browser.editing() && plain(e) {
        let zoomed = s.browser.zoomed();
        match e.key {
            RuneKey::Tab if !zoomed => {
                s.pane = s.pane.step(e.mods.shift);
                return true;
            }
            RuneKey::Escape if !zoomed && s.pane != Focus::List => {
                s.pane = Focus::List;
                return true;
            }
            _ => {}
        }
        // The full-size inspector has the keyboard, whichever pane had it.
        let pane = if zoomed { Focus::Inspector } else { s.pane };
        let taken = match pane {
            Focus::Inspector => s.inspector_key(e, tx),
            Focus::Lanes => s.lanes_key(e, tx),
            Focus::List => false,
        };
        if taken {
            return true;
        }
    }
    let Some(key) = key_of(e) else { return false };
    let (selected, zoomed) = (s.browser.selected(), s.browser.zoomed());
    let view = s.viewport();
    if !s.browser.on_key(key, view) {
        tx.quit();
        return true;
    }
    // The list is drawn again after a full-size view, where it was.
    if zoomed && !s.browser.zoomed() {
        s.restore_list(tx);
    }
    if s.browser.selected() != selected
        && let Some(row) = s.browser.selected()
    {
        tx.scroll_into_view(s.browser.session(row).to_string());
    }
    if (s.browser.selected(), s.browser.zoomed()) != (selected, zoomed) {
        s.cursor = 0;
        tx.scroll_to_y(view::PANE, 0.0);
    }
    true
}

/// `--size`, checked before Rune reads it, so that it fails as `tokprof ui --dump` does.
fn check_size(args: &[String]) -> Result<()> {
    let mut args = args.iter();
    while let Some(a) = args.next() {
        let value = match a.strip_prefix("--size=") {
            Some(v) => Some(v),
            None if a == "--size" => Some(args.next().map_or("", String::as_str)),
            None => None,
        };
        if let Some(v) = value {
            let ok = v.split_once('x').is_some_and(|(w, h)| [w, h].iter().all(|n| n.parse::<f32>().is_ok_and(|n| n > 0.0)));
            if !ok {
                bail!("--size takes WIDTHxHEIGHT");
            }
        }
    }
    Ok(())
}

/// The sessions the filters select, as `tokprof ui` selects them.
fn query(cli: &Cli, agents: Vec<AgentId>) -> Result<Query> {
    Ok(Query {
        agents,
        project: cli.project.as_deref().map(normalize_project),
        session: cli.session.clone(),
        since_ms: cli.since.as_deref().map(parse_since).transpose()?,
        until_ms: cli.until.as_deref().map(parse_date).transpose()?,
        client: cli.client.clone(),
    })
}

/// The window's browser over the sessions of `catalog` that `cli` selects. Without a window
/// (`--snap`, `--tree`), it follows no logs and waits for the X-ray of the newest session and for
/// the day, the one `--day` names or the latest, as `tokprof ui --dump` waits.
fn desk(cli: &Cli, catalog: Catalog, headless: bool, wake: Wake) -> Result<Desk> {
    let catalog = Arc::new(catalog);
    let (select, pricing, tab) = checked(cli, &catalog)?;
    let browser = Browser::start(catalog, pricing, select, !headless, wake, frontend())?;
    let mut desk = Desk::new(browser, !headless);
    desk.tab = tab;
    if headless {
        desk.browser.settle();
        if let Some(date) = cli.day.as_deref() {
            let date = rhythm::date_of(parse_date(date)? + 12 * 3_600_000, &rhythm::local_offset);
            desk.browser.show_day(date);
        }
        desk.browser.settle_day();
    }
    Ok(desk)
}

/// What the flags select, the prices and the tab, checked as the window will use them.
fn checked(cli: &Cli, catalog: &Catalog) -> Result<(Query, tokprof_core::pricing::Pricing, Tab)> {
    let agents = match cli.agent.is_empty() {
        true => catalog.agents(),
        false => cli.agent.iter().map(|a| catalog.agent(a)).collect::<Result<Vec<_>>>()?,
    };
    let select = query(cli, agents)?;
    let pricing = paths::pricing(cli.prices.as_deref())?;
    let tab = match cli.tab.as_deref() {
        Some(name) => {
            Tab::named(name).ok_or_else(|| anyhow::anyhow!("--tab takes one of: summary, requests, spend, work, waits, prompts, tools, context, cache"))?
        }
        None => Tab::Summary,
    };
    Ok((select, pricing, tab))
}

/// What a start from a terminal checks before the window starts on its own, so that its errors
/// show there: the flags, and that there are session logs.
fn check_before_start(cli: &Cli, catalog: &Catalog) -> Result<()> {
    let (select, _, _) = checked(cli, catalog)?;
    if tokprof_core::xray::rows(catalog, &select, 1).is_empty() {
        bail!("no session logs found");
    }
    Ok(())
}

/// The window's own title bar: the traffic lights where the bar's height centers them, and only
/// the bar's empty parts move the window.
const CHROME: Chrome = Chrome { full_size_content: true, traffic_lights: Some((18.0, 16.0)), drag_regions: true };

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    check_size(&args)?;
    // Rune takes its own flags; the rest are ours.
    let cli = Cli::try_parse_from(std::iter::once("tokprof-desk".to_string()).chain(rune::extra_args().iter().cloned())).unwrap_or_else(|e| e.exit());
    // From a terminal the window starts on its own, after the checks, and the terminal comes back.
    let exe = std::env::current_exe().and_then(|e| e.canonicalize())?;
    let start = start_of(&exe, rune::is_headless(), cli.wait, std::io::IsTerminal::is_terminal(&std::io::stdin()));
    if start != Start::Here {
        check_before_start(&cli, &tokprof_sources::catalog())?;
        return start_apart(&start, &exe, &args);
    }
    let desk = desk(&cli, tokprof_sources::catalog(), rune::is_headless(), Arc::new(wake))?;
    let spec = AppSpec {
        min_size: Some((960.0, 640.0)),
        // The system's font, as a Mac app's text is; Inter where there is none.
        families: ("system-ui, Inter", "ui-monospace"),
        scenes: SCENES,
        setup: Some(setup),
        poll: Some(poll),
        on_key: Some(on_key),
        ..AppSpec::new("tokprof", (1400.0, 900.0), placeholder, view::view)
    };
    App::new(spec).chrome(CHROME).init(move || desk).run();
    Ok(())
}

/// A named state a frame without a window can start from (`--scene`).
type Scene = (&'static str, fn(&mut Desk));

/// The scenes: a pane with the keyboard.
const SCENES: &[Scene] = &[
    ("lanes-keyboard", |s| s.pane = Focus::Lanes),
    ("inspector-keyboard", |s| {
        s.pane = Focus::Inspector;
        s.tab = Tab::Tools;
        s.cursor = 2;
    }),
];

/// `AppSpec` wants a state it can make; the window starts from the one `main` made instead.
fn placeholder() -> Desk {
    unreachable!("the window starts from the state main made")
}

#[cfg(test)]
mod tests;
