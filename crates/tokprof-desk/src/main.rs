//! `tokprof-desk`: the session browser of tokprof in a native window, laid out as a profiler. The
//! browser of `tokprof-browse` holds the sessions, their costs, X-rays and days, as it does for
//! `tokprof ui`; this draws them with Rune and passes the window's keys and clicks on.

mod ansi;
mod calendar;
mod changes;
mod charts;
mod day;
mod deltas;
mod details;
mod flame;
mod inspector;
mod keys;
mod menubar;
mod month;
mod overlay;
mod pal;
mod palette;
mod reports;
mod requests;
mod sessions;
mod settings;
mod themes;
mod view;
mod week;

use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use anyhow::{Result, bail};
use clap::Parser;
use rune::{App, AppSpec, Chrome, Key as RuneKey, KeyEvent, Tx, Waker};

use tokprof_browse::browser::{Browser, Frontend, Key, Sort, Viewport, Wake, now_ms};
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
    /// Only sessions whose project, or the folder they ran in, contains this text.
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
    /// With --snap or --tree: draw in this color theme, whatever --theme says (an id such as
    /// merino-light or ink-meridian).
    #[arg(long)]
    colors: Option<String>,
    /// With --snap or --tree: the calendar's metric (spend, agent, keyboard, messages, commits,
    /// avoidable), as `tokprof calendar --metric` takes it.
    #[arg(long)]
    metric: Option<String>,
    /// With --snap or --tree: the week that holds this date (YYYY-MM-DD), for the scene `week`;
    /// this week by default.
    #[arg(long)]
    week: Option<String>,
    /// With --snap or --tree: the month (YYYY-MM) of the scenes `month`, `month-advice`,
    /// `month-review` and `month-card`; the current month by default.
    #[arg(long)]
    month: Option<String>,
    /// With --snap or --tree: the quarter (YYYY-Qn) of the scene `quarter`; the current quarter by
    /// default.
    #[arg(long)]
    quarter: Option<String>,
    /// With --snap: draw only the month in review of --month, from the day store as it is, without
    /// listing or costing sessions; fail when the card cannot be drawn. Save image runs this.
    #[arg(long, hide = true)]
    card_image: bool,
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

/// The window's state: the browser and what the window keeps of its own.
pub struct Desk {
    pub browser: Book,
    /// The theme the window draws in, as the view last chose it; rows built later, as a table
    /// scrolls, read it.
    pub theme: Cell<&'static themes::Theme>,
    /// No session is selected in the window: Day shows the day summary under the lanes, and
    /// Sessions highlights no row. The browser keeps a current row all the same.
    pub none: bool,
    /// The user stepped the lanes to another day: once that day is computed, a selected session
    /// without a lane on it is deselected.
    pub stepped: bool,
    /// A session picked, whose lane is scrolled to once the day shown is computed: the pick may
    /// show another day, whose lanes are not laid out yet.
    pub reveal_lane: Option<String>,
    /// The place of the border between the lanes and the details, a share of the height under
    /// the bar, as dragged and saved; `None` for the room that Room for the lanes gives.
    pub lanes_share: Option<f32>,
    /// The lanes' height when a drag of the border started, and as last laid out.
    pub border_from: Option<f32>,
    pub lanes_h: f32,
    /// The day summary's cursor among its rows, and what it computed of the day store.
    pub day_cursor: usize,
    pub day_spend: RefCell<details::Memo>,
    /// How far the details pane is scrolled, in points, as its viewport last said.
    pub pane_offset: Rc<Cell<f32>>,
    /// The height of the details pane, and the window's size, from their layout.
    pub pane_height: f32,
    pub window: (f32, f32),
    /// When a listed cache goes cold next, as the last poll found it.
    pub next_cold: Option<i64>,
    /// When a wait on the screen next changes, as the last poll found it.
    pub next_wait: Option<i64>,
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
    /// The status the bar shows, the browser's or the window's own, and when it came.
    pub status: (String, Instant),
    /// The browser's status as the last poll saw it.
    pub browser_status: String,
    /// The appearance and the themes, and the store that keeps them: the app's settings file in a
    /// window, memory without one.
    pub settings: settings::Settings,
    pub prefs: rune::prefs::Prefs,
    /// The settings panel, the command palette and the key help, while one is open.
    pub panel: Option<settings::Panel>,
    pub palette: Option<palette::Palette>,
    pub help: bool,
    /// The minute of the last poll: the lanes' now line and the inspector's times move at a new
    /// one.
    pub minute: i64,
    /// The pane that has the keyboard.
    pub pane: Focus,
    /// The row of the inspector's tab the cursor is on, and how many rows the tab has, as the
    /// inspector last drew it.
    pub cursor: usize,
    pub tab_rows: Cell<usize>,
    /// The `Flame` tab's zoom, cursor and hover, the tree of the X-ray shown, and the width its
    /// graph was last laid out in.
    pub flame: flame::State,
    pub flame_memo: RefCell<flame::Memo>,
    /// The X-ray the inspector was last drawn from, held so that its address, the inspector's
    /// key, is not taken by a later X-ray.
    pub xray_seen: RefCell<Option<Arc<tokprof_core::xray::XRay>>>,
    pub flame_w: f32,
    /// The view under the bar, and the state of Month and Quarter.
    pub reports: reports::State,
    /// The state of Calendar.
    pub calendar: calendar::State,
    /// The month in review over Month, Quarter or Calendar.
    pub month: month::State,
    /// The cache countdown in the macOS menu bar.
    pub menu_bar: menubar::MenuBar,
    /// The `Requests` tab's selection, range, hover and unit.
    pub requests: requests::State,
    /// The Sessions screen.
    pub sessions: sessions::State,
    /// The state of Week.
    pub week: week::State,
    /// The week `--week` names, and the quarter `--quarter` names, for their scenes.
    pub week_wanted: Option<i64>,
    pub quarter_wanted: Option<tokprof_core::period::Yq>,
    /// When the clock next takes a session out of what the quick filters keep, as the last poll
    /// found it.
    pub next_filter: Option<i64>,
}

/// The panes of Day that take the keyboard: the lanes, and the details pane under them, which
/// shows the selected session's inspector or the day summary.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Focus {
    Lanes,
    Inspector,
}

impl Focus {
    /// The other pane, as Tab and Shift+Tab move to it.
    fn step(self) -> Focus {
        match self {
            Focus::Lanes => Focus::Inspector,
            Focus::Inspector => Focus::Lanes,
        }
    }
}

impl Desk {
    pub fn new(mut browser: Book, live: bool) -> Desk {
        // The window lays the day out itself; the browser computes it at once, beside the list.
        browser.keep_day_beside();
        browser.set_pane(view::DAY_PANE);
        let relisted = browser.relisted();
        let prefs = if live { rune::prefs::Prefs::open(settings::APP_ID) } else { rune::prefs::Prefs::memory() };
        let settings = settings::Settings::read(&prefs);
        let lanes_share = settings::lanes_share(&prefs);
        browser.set_waiting(settings.waiting);
        // The window's filter matches the sessions' titles too, in Day and in Sessions.
        browser.set_match_titles(true);
        // The window starts with no session selected: no X-ray is computed until one is.
        browser.hold_reports(true);
        Desk {
            none: true,
            stepped: false,
            reveal_lane: None,
            lanes_share,
            border_from: None,
            lanes_h: 0.0,
            day_cursor: 0,
            day_spend: RefCell::default(),
            settings,
            prefs,
            panel: None,
            palette: None,
            help: false,
            browser_status: String::new(),
            browser,
            theme: Cell::new(themes::default(true)),
            pane_offset: Rc::default(),
            pane_height: 0.0,
            window: (0.0, 0.0),
            next_cold: None,
            next_wait: None,
            relisted,
            pointer: None,
            tab: Tab::Summary,
            fit: true,
            live,
            status: (String::new(), Instant::now()),
            minute: 0,
            pane: Focus::Lanes,
            cursor: 0,
            tab_rows: Cell::new(0),
            flame: flame::State::default(),
            flame_memo: RefCell::default(),
            xray_seen: RefCell::default(),
            flame_w: 0.0,
            reports: reports::State::new(tokprof_browse::reports::Reports::detached(), tokprof_browse::period::Periods::detached()),
            calendar: calendar::State::new(tokprof_browse::calendar::Calendars::detached()),
            month: month::State::default(),
            menu_bar: menubar::MenuBar::default(),
            requests: requests::State::default(),
            sessions: sessions::State::default(),
            week: week::State::new(tokprof_browse::week::Weeks::detached()),
            week_wanted: None,
            quarter_wanted: None,
            next_filter: None,
        }
    }

    /// The desk with its settings in this store instead.
    pub fn with_prefs(mut self, prefs: rune::prefs::Prefs) -> Desk {
        self.settings = settings::Settings::read(&prefs);
        self.lanes_share = settings::lanes_share(&prefs);
        self.browser.set_waiting(self.settings.waiting);
        self.prefs = prefs;
        self
    }

    /// What identifies the X-ray shown, for a memo's version: its address, while the desk holds
    /// it, so that no later X-ray can have the same.
    pub fn xray_key(&self) -> Option<usize> {
        let x = self.browser.xray_shared();
        let key = x.as_ref().map(|x| Arc::as_ptr(x) as usize);
        *self.xray_seen.borrow_mut() = x;
        key
    }

    /// Shows tab `t` from its top, with the cursor on its first row.
    pub fn show_tab(&mut self, t: Tab, tx: &mut Tx) {
        self.tab = t;
        self.cursor = 0;
        tx.scroll_to_y(view::PANE, 0.0);
    }

    /// The session selected in the window, by its row: none while the window shows none.
    pub fn current(&self) -> Option<usize> {
        if self.none { None } else { self.browser.selected() }
    }

    /// Selects the session of a lane, as a click on the lane does: its lane scrolled to, and the
    /// inspector on the tab it shows, from its top.
    pub fn open_lane(&mut self, id: &str, tx: &mut Tx) {
        if self.browser.open_session(id) {
            self.none = false;
            self.cursor = 0;
            tx.scroll_into_view(format!("lane-{id}"));
            tx.scroll_to_y(view::PANE, 0.0);
        }
    }

    /// Selects a session as the palette, the menu bar, Sessions and the day summary do: on its
    /// day (desk-window, The selected session's day), its `Summary` from the top, the full-size
    /// inspector left as it is. Returns whether the session is listed.
    pub fn pick(&mut self, id: &str, tx: &mut Tx) -> bool {
        let Some(row) = self.browser.row_of(id) else { return false };
        self.none = false;
        self.browser.select_row(row);
        self.show_tab(Tab::Summary, tx);
        tx.scroll_into_view(format!("lane-{id}"));
        self.reveal_lane = Some(id.to_string());
        true
    }

    /// No session selected: the details pane shows the day summary from its top, as esc and
    /// `‹ The day` do.
    pub fn deselect(&mut self, tx: &mut Tx) {
        if self.browser.zoomed() {
            self.browser.on_key(Key::Backspace, self.viewport());
        }
        self.none = true;
        self.day_cursor = 0;
        tx.scroll_to_y(view::PANE, 0.0);
    }

    /// Steps the lanes' day by `delta` days, as the buttons beside its name and ← → do.
    pub fn step_day(&mut self, delta: i64) {
        self.browser.step_day_by(delta);
        self.stepped = true;
    }

    /// Shows the day of the latest action, as Today and `t` do.
    pub fn show_latest(&mut self) {
        self.browser.show_latest_day();
        self.stepped = true;
    }

    /// Whether the selected session has a lane on the day shown.
    pub fn has_lane(&self) -> bool {
        let Some(r) = self.current() else { return false };
        let Some((_, lanes)) = self.browser.day_parts().and_then(|p| p.strip.as_ref()) else { return false };
        lanes.lanes.iter().any(|l| self.browser.row_of(&l.id) == Some(r))
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
        let selected = self.current();
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
    /// none; the rest go on to the browser. In `Flame`, the graph takes its keys first, and
    /// Shift+← and → switch tabs.
    fn inspector_key(&mut self, e: &KeyEvent, tx: &mut Tx) -> bool {
        if self.tab == Tab::Flame && self.flame_key(e) {
            return true;
        }
        if self.tab == Tab::Requests && requests::key(self, e, tx) {
            return true;
        }
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
            (_, Some("0")) => {
                self.show_tab(Tab::Flame, tx);
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

    /// Selects the session with this id, as a click on its row does; the inspector shows its
    /// top.
    pub fn select_session(&mut self, session: &str) {
        if let Some(row) = self.browser.row_of(session) {
            self.browser.select_row(row);
            self.none = false;
            self.cursor = 0;
        }
    }

    /// The rows of the Sessions table a page moves by, and the lines of the inspector.
    fn viewport(&self) -> Viewport {
        Viewport { page: ((self.sessions.height / sessions::ROW) as usize).max(1), lines: (self.pane_height / 20.0) as usize }
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
/// on the screen or in the menu bar, and when the status message is due to go.
fn poll(s: &mut Desk, _: f64, tx: &mut Tx) -> bool {
    // Where the list is scrolled to, unless the list was put in place since and the browser
    // knows better.
    let in_sessions = s.reports.mode == reports::Mode::Sessions;
    if s.browser.relisted() == s.relisted && in_sessions {
        sessions::sync_top(s);
    }
    // No X-ray while the window shows no session.
    s.browser.hold_reports(s.none);
    let arrived = s.browser.tick() | s.flame_dive();
    // A day the user stepped to keeps the selection only when its session has a lane there.
    let mut dropped = false;
    if s.stepped && s.browser.day_parts().is_some() {
        s.stepped = false;
        if !s.none && !s.has_lane() {
            s.deselect(tx);
            dropped = true;
        }
    }
    // The lane of a session picked on another day, once that day's lanes are laid out.
    let mut revealed = false;
    if s.reports.mode == reports::Mode::Day
        && let Some(id) = s.reveal_lane.clone()
        && let Some(parts) = s.browser.day_parts()
    {
        if parts.strip.as_ref().is_some_and(|(_, lanes)| lanes.lanes.iter().any(|l| l.id == id)) {
            tx.scroll_into_view(format!("lane-{id}"));
            revealed = true;
        }
        s.reveal_lane = None;
    }
    // Rune would keep the first row on the screen there whatever came or went; the browser keeps
    // the rows on the screen in place instead, as `tokprof ui` does, so the list scrolls to its
    // first row.
    let relisted = s.browser.relisted() != s.relisted;
    if relisted {
        s.relisted = s.browser.relisted();
        if in_sessions {
            sessions::restore(s, tx);
        }
    }
    let now = now_ms();
    // A cache that went cold since turns its cell red now.
    let cooled = s.next_cold.is_some_and(|at| now >= at);
    s.next_cold = s.browser.next_cold(now);
    if let Some(at) = s.next_cold {
        tx.wake_after(Duration::from_millis((at - now).max(1) as u64));
    }
    // A wait on the screen grows, and passes the waiting limit.
    let waited = s.next_wait.is_some_and(|at| now >= at);
    s.next_wait = s.browser.next_wait_change(now).into_iter().chain(details::next_wait(s, now)).min();
    if let Some(at) = s.next_wait {
        tx.wake_after(Duration::from_millis((at - now).max(1) as u64));
    }
    // The clock takes a session out of the quick filters: its agent fell silent, or the week
    // ended; a cache that goes cold is the cold one above.
    let filtered = s.next_filter.is_some_and(|at| now >= at);
    s.next_filter = s.browser.next_filter_change(now);
    if let Some(at) = s.next_filter {
        tx.wake_after(Duration::from_millis((at - now).max(1) as u64));
    }
    if let Some(at) = s.browser.next_due() {
        tx.wake_after(at.saturating_duration_since(Instant::now()).max(Duration::from_millis(1)));
    }
    // A countdown on the screen moves every second; the parts that show time move every minute.
    // Week, Month, Quarter and Calendar show neither.
    let day = matches!(s.reports.mode, reports::Mode::Day | reports::Mode::Sessions);
    let counting = match s.reports.mode {
        reports::Mode::Sessions => sessions::counting(s, now),
        reports::Mode::Day => details::counting(s, now),
        _ => false,
    };
    if counting {
        tx.wake_after(Duration::from_millis((1000 - now.rem_euclid(1000)) as u64 + 5));
    } else if day {
        tx.wake_after(Duration::from_millis((60_000 - now.rem_euclid(60_000)) as u64 + 5));
    }
    let minute = now / 60_000;
    let turned = day && s.minute != minute;
    s.minute = minute;
    if s.browser.status() != s.browser_status {
        s.browser_status = s.browser.status().to_string();
        s.status = (s.browser_status.clone(), Instant::now());
    }
    if s.status_shown() {
        tx.wake_after(Duration::from_secs(view::STATUS_SECS).saturating_sub(s.status.1.elapsed()) + Duration::from_millis(5));
    }
    let reported = reports::poll(s, tx) | calendar::poll(s, tx) | month::poll(s) | sessions::poll(s, tx) | week::poll(s, tx);
    // The menu bar counts down whether the window shows or not; it draws nothing in the window.
    menubar::poll(s, now, tx);
    arrived || relisted || cooled || waited || filtered || counting || turned || reported || dropped || revealed
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

/// The lane a step goes to among `count`, from the selected session's lane `at`: with none
/// (no session selected, or one without a lane that day), ↓ goes to the first and ↑ to the last.
fn lane_to(count: usize, at: Option<usize>, step: LaneStep) -> Option<usize> {
    let last = count.checked_sub(1)?;
    Some(match (step, at) {
        (LaneStep::First, _) | (LaneStep::Down, None) => 0,
        (LaneStep::Last, _) | (LaneStep::Up, None) => last,
        (LaneStep::Up, Some(i)) => i.saturating_sub(1),
        (LaneStep::Down, Some(i)) => (i + 1).min(last),
    })
}

fn on_key(s: &mut Desk, e: &KeyEvent, tx: &mut Tx) -> bool {
    if e.is_command('q') {
        tx.quit();
        return true;
    }
    // The month in review takes every other key while it is open.
    if s.month.card.is_some() && matches!(s.reports.mode, reports::Mode::Calendar | reports::Mode::Month | reports::Mode::Quarter) {
        return month::on_key(s, e, tx);
    }
    // ⌘K and ⌘, open and close the palette and the settings, as their menu items do where the
    // menu does not take them first; while a card is open, every other key is its own.
    if e.is_command('k') {
        s.toggle_palette();
        return true;
    }
    if e.is_command(',') {
        s.toggle_settings();
        return true;
    }
    if s.panel.is_some() {
        return s.settings_key(e, tx);
    }
    if s.palette.is_some() {
        return s.palette_key(e, tx);
    }
    if s.help {
        return s.help_key(e, tx);
    }
    if e.is_command('w') {
        tx.close_window();
        return true;
    }
    // ⌘1 shows Day, ⌘2 Week, ⌘3 Month, ⌘4 Quarter, ⌘5 Calendar and ⌘0 Sessions.
    if reports::switch_key(s, e) {
        if s.reports.mode == reports::Mode::Sessions {
            sessions::show(s, tx);
        }
        return true;
    }
    if s.reports.mode.period() {
        // `?` opens the key help there too, where it is not typed into a filter.
        if !s.browser.editing() && matches!(&e.key, RuneKey::Text(t) if t == "?") && !e.mods.cmd && !e.mods.ctrl {
            s.open_help();
            return true;
        }
        // Ctrl+C quits from anywhere, as the key help says.
        if key_of(e) == Some(Key::Interrupt) {
            tx.quit();
            return true;
        }
        return reports::on_key(s, e, tx);
    }
    if s.reports.mode == reports::Mode::Week {
        if !s.browser.editing() && matches!(&e.key, RuneKey::Text(t) if t == "?") && !e.mods.cmd && !e.mods.ctrl {
            s.open_help();
            return true;
        }
        if key_of(e) == Some(Key::Interrupt) {
            tx.quit();
            return true;
        }
        return week::on_key(s, e, tx);
    }
    if s.reports.mode == reports::Mode::Sessions {
        if !s.browser.editing() && matches!(&e.key, RuneKey::Text(t) if t == "?") && !e.mods.cmd && !e.mods.ctrl {
            s.open_help();
            return true;
        }
        return sessions::on_key(s, e, tx);
    }
    if s.reports.mode == reports::Mode::Calendar {
        if matches!(&e.key, RuneKey::Text(t) if t == "?") && !e.mods.cmd && !e.mods.ctrl {
            s.open_help();
            return true;
        }
        if key_of(e) == Some(Key::Interrupt) {
            tx.quit();
            return true;
        }
        return calendar::on_key(s, e, tx);
    }
    // The pane with the keyboard takes its keys first; a filter being typed takes them all.
    if !s.browser.editing() && plain(e) {
        let zoomed = s.browser.zoomed();
        match e.key {
            RuneKey::Tab if !zoomed => {
                s.pane = s.pane.step();
                return true;
            }
            // In a zoomed flame graph, esc zooms out before it gives the keyboard back.
            RuneKey::Escape if s.tab == Tab::Flame && inspector::keyboard(s) && s.flame_out() => return true,
            RuneKey::Escape if inspector::keyboard(s) && requests::clear_range(s) => return true,
            RuneKey::Escape => {
                step_back(s, tx);
                return true;
            }
            _ => {}
        }
        // The full-size inspector has the keyboard, whichever pane had it.
        let pane = if zoomed { Focus::Inspector } else { s.pane };
        let taken = match pane {
            Focus::Inspector if zoomed || s.current().is_some() => s.inspector_key(e, tx),
            Focus::Inspector => details::on_key(s, e, tx),
            Focus::Lanes => s.lanes_key(e, tx),
        };
        if taken {
            return true;
        }
        match &e.key {
            // `t` shows the day of the latest action, as Today does; `d` is gone, since the
            // selection is always shown on its day.
            RuneKey::Text(t) if t == "t" => {
                s.show_latest();
                return true;
            }
            RuneKey::Text(t) if t == "d" => return true,
            // ⏎ shows the selected session full size; with none selected there is nothing to show.
            RuneKey::Enter if s.current().is_none() => return true,
            RuneKey::Left | RuneKey::Right => s.stepped = true,
            RuneKey::Text(t) if ["h", "l", "[", "]"].contains(&t.as_str()) => s.stepped = true,
            _ => {}
        }
    }
    // `?` opens the key help, where it is not typed into a filter.
    if !s.browser.editing() && matches!(&e.key, RuneKey::Text(t) if t == "?") && !e.mods.cmd && !e.mods.ctrl {
        s.open_help();
        return true;
    }
    let Some(key) = key_of(e) else { return false };
    if !browser_key(s, key, tx) {
        tx.quit();
    }
    true
}

/// esc in Day steps back, and never quits: the full-size inspector closes, then the details pane
/// gives the keyboard back to the lanes, then the session is deselected, then the filter is
/// cleared.
fn step_back(s: &mut Desk, tx: &mut Tx) {
    if s.browser.zoomed() {
        browser_key(s, Key::Backspace, tx);
    } else if s.pane == Focus::Inspector {
        s.pane = Focus::Lanes;
    } else if s.current().is_some() {
        s.deselect(tx);
    } else if !s.browser.filter().is_empty() {
        s.browser.clear_filter();
    }
}

/// Passes a key to the browser, and keeps the inspector in step with what it did: from its top
/// for another session or after the full-size view. Returns false where the browser quits.
fn browser_key(s: &mut Desk, key: Key, tx: &mut Tx) -> bool {
    let (selected, zoomed) = (s.browser.selected(), s.browser.zoomed());
    let view = s.viewport();
    if !s.browser.on_key(key, view) {
        return false;
    }
    if (s.browser.selected(), s.browser.zoomed()) != (selected, zoomed) {
        s.cursor = 0;
        tx.scroll_to_y(view::PANE, 0.0);
    }
    true
}

/// The values of `--size` among `args`.
fn sizes(args: &[String]) -> Vec<&str> {
    let mut out = Vec::new();
    let mut args = args.iter();
    while let Some(a) = args.next() {
        match a.strip_prefix("--size=") {
            Some(v) => out.push(v),
            None if a == "--size" => out.push(args.next().map_or("", String::as_str)),
            None => {}
        }
    }
    out
}

fn parse_size(v: &str) -> Option<(f32, f32)> {
    let (w, h) = v.split_once('x')?;
    let (w, h) = (w.parse::<f32>().ok()?, h.parse::<f32>().ok()?);
    (w > 0.0 && h > 0.0).then_some((w, h))
}

/// `--size`, checked before Rune reads it, so that it fails as `tokprof ui --dump` does.
fn check_size(args: &[String]) -> Result<()> {
    if sizes(args).into_iter().any(|v| parse_size(v).is_none()) {
        bail!("--size takes WIDTHxHEIGHT");
    }
    Ok(())
}

/// The size `--size` asks for, the last one given.
pub fn size_of(args: &[String]) -> Option<(f32, f32)> {
    sizes(args).last().and_then(|v| parse_size(v))
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
    if cli.card_image {
        return card_image(cli, catalog, pricing, select, &paths::day_store());
    }
    let reports = tokprof_browse::reports::Reports::start(catalog.clone(), pricing.clone(), select.clone(), wake.clone());
    let periods = tokprof_browse::period::Periods::start(select.clone(), wake.clone());
    let weeks = tokprof_browse::week::Weeks::start(catalog.clone(), pricing.clone(), select.clone(), wake.clone());
    let desk_select = select.clone();
    let calendars =
        tokprof_browse::calendar::Calendars::start(catalog.clone(), pricing.clone(), cli.prices.clone(), select.clone(), paths::day_store(), wake.clone());
    let client_ignored = select.client.is_some();
    let browser = Browser::start(catalog, pricing, select, !headless, wake, frontend())?;
    let mut desk = Desk::new(browser, !headless);
    desk.tab = tab;
    desk.reports.data = reports;
    desk.reports.sums = periods;
    desk.reports.client_ignored = client_ignored;
    desk.calendar.data = calendars;
    desk.calendar.client_ignored = client_ignored;
    desk.week.data = weeks;
    desk.week.client_ignored = client_ignored;
    desk.week_wanted = week_of(cli)?;
    desk.sessions.narrowed = !cli.agent.is_empty() || [&cli.project, &cli.session, &cli.client, &cli.since, &cli.until].iter().any(|f| f.is_some());
    desk.calendar.metric = metric(cli)?.unwrap_or(tokprof_core::calendar::Metric::Spend);
    desk.calendar.select = Query { since_ms: None, until_ms: None, ..desk_select.clone() };
    desk.month.wanted = month_of(cli)?;
    desk.quarter_wanted = quarter_of(cli)?;
    desk.month.args = filter_args(cli);
    if headless {
        if let Some(t) = colors(cli)? {
            desk.settings = settings::Settings::of_theme(t);
        }
        // The image of a month in review needs no session, no X-ray and no day.
        if std::env::args().any(|a| a == "month-card") {
            return Ok(desk);
        }
        // `--tab` shows that tab of the newest session, selected on its day; else the frame shows
        // the day summary, with no session selected.
        // The day of the latest action first, so that the newest session's day is known as it.
        desk.browser.settle_day();
        if cli.tab.is_some() {
            select_newest(&mut desk);
        }
        desk.browser.hold_reports(desk.none);
        desk.browser.settle();
        if let Some(date) = cli.day.as_deref() {
            let date = rhythm::date_of(parse_date(date)? + 12 * 3_600_000, &rhythm::local_offset);
            desk.browser.show_day(date);
        }
        desk.browser.settle_day();
        // The day summary's spend, and the lanes' caption's, come from the day store.
        let changes = desk.browser.changes();
        desk.calendar.data.settle(changes, Duration::from_secs(120));
    }
    Ok(desk)
}

/// Selects the newest listed session on its day, for a frame of its inspector.
fn select_newest(s: &mut Desk) {
    if !s.browser.is_empty() {
        s.none = false;
        s.browser.select_row(0);
        s.browser.show_selected_day();
    }
}

/// A scene of the inspector: the newest session selected on its day when none is, and its X-ray
/// and day waited for, as a frame of `--tab` waits for them.
fn scene_session(s: &mut Desk) {
    if s.none {
        select_newest(s);
        s.browser.hold_reports(false);
        s.browser.settle();
        s.browser.settle_day();
    }
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
        Some(name) => Tab::named(name)
            .ok_or_else(|| anyhow::anyhow!("--tab takes one of: summary, requests, spend, work, waits, prompts, tools, context, cache, flame"))?,
        None => Tab::Summary,
    };
    colors(cli)?;
    metric(cli)?;
    month_of(cli)?;
    quarter_of(cli)?;
    week_of(cli)?;
    Ok((select, pricing, tab))
}

/// The local date `--week` names, if it names one: a date of a week that has begun.
fn week_of(cli: &Cli) -> Result<Option<i64>> {
    let Some(d) = cli.week.as_deref() else { return Ok(None) };
    let ok = d.len() == 10 && d.as_bytes()[4] == b'-' && d.as_bytes()[7] == b'-';
    let Some(ms) = ok.then(|| parse_date(d).ok()).flatten() else { bail!("--week takes YYYY-MM-DD") };
    let date = rhythm::date_of(ms + 12 * 3_600_000, &rhythm::local_offset);
    let monday = tokprof_core::week::monday_of(date);
    if rhythm::midnight(monday, &rhythm::local_offset) > now_ms() {
        bail!("the week of {d} has not begun");
    }
    Ok(Some(date))
}

/// The month `--month` names, if it names one: a month that has begun.
fn month_of(cli: &Cli) -> Result<Option<tokprof_core::month::Ym>> {
    let Some(m) = cli.month.as_deref() else { return Ok(None) };
    let Some(ym) = tokprof_core::month::Ym::parse(m) else { bail!("--month takes YYYY-MM") };
    begun(tokprof_core::period::Period::Month(ym))?;
    Ok(Some(ym))
}

/// The quarter `--quarter` names, if it names one: a quarter that has begun.
fn quarter_of(cli: &Cli) -> Result<Option<tokprof_core::period::Yq>> {
    let Some(q) = cli.quarter.as_deref() else { return Ok(None) };
    let Some(yq) = tokprof_core::period::Yq::parse(q) else { bail!("--quarter takes YYYY-Qn") };
    begun(tokprof_core::period::Period::Quarter(yq))?;
    Ok(Some(yq))
}

/// A period of `--month` or `--quarter` must have begun: `May 2027 has not begun`.
fn begun(period: tokprof_core::period::Period) -> Result<()> {
    if !period.begun(now_ms(), &rhythm::local_offset) {
        bail!("{} has not begun", period.name());
    }
    Ok(())
}

/// The desk of `--card-image`: no sessions, only the month in review of `--month` from the store
/// as it is, and an error when the card cannot be drawn, so that no other picture is saved as it.
fn card_image(cli: &Cli, catalog: Arc<Catalog>, pricing: tokprof_core::pricing::Pricing, select: Query, store: &std::path::Path) -> Result<Desk> {
    let ym = month_of(cli)?.ok_or_else(|| anyhow::anyhow!("--card-image needs --month"))?;
    let store = tokprof_core::daystore::Store::load(store, now_ms());
    if let Some(e) = &store.unreadable {
        bail!("could not draw the month card: the day store cannot be read ({e})");
    }
    let month = tokprof_browse::calendar::month(&store, &Query { since_ms: None, until_ms: None, ..select }, ym, now_ms());
    if store.entries.is_empty() {
        bail!("could not draw the month card: the day store is empty");
    }
    let browser = Browser::detached(catalog, Vec::new(), pricing, frontend());
    let mut desk = Desk::new(browser, false);
    if let Some(t) = colors(cli)? {
        desk.settings = settings::Settings::of_theme(t);
    }
    desk.month.card = Some(month::Card { month, status: None });
    desk.month.alone = true;
    desk.reports.mode = reports::Mode::Month;
    Ok(desk)
}

/// The filters of the command line, as the program that draws an image takes them again.
fn filter_args(cli: &Cli) -> Vec<String> {
    let mut out = Vec::new();
    if !cli.agent.is_empty() {
        out.extend(["--agent".to_string(), cli.agent.join(",")]);
    }
    for (flag, value) in [("--project", &cli.project), ("--session", &cli.session)] {
        if let Some(v) = value {
            out.extend([flag.to_string(), v.clone()]);
        }
    }
    if let Some(p) = &cli.prices {
        out.extend(["--prices".to_string(), p.to_string_lossy().to_string()]);
    }
    out
}

/// The metric `--metric` names, if it names one.
fn metric(cli: &Cli) -> Result<Option<tokprof_core::calendar::Metric>> {
    let Some(name) = cli.metric.as_deref() else { return Ok(None) };
    match tokprof_core::calendar::Metric::parse(name) {
        Some(m) => Ok(Some(m)),
        None => bail!("--metric takes one of: {}", tokprof_core::calendar::Metric::NAMES),
    }
}

/// The theme `--colors` names, if it names one.
fn colors(cli: &Cli) -> Result<Option<&'static themes::Theme>> {
    let Some(id) = cli.colors.as_deref() else { return Ok(None) };
    match themes::find(id) {
        Some(t) => Ok(Some(t)),
        None => bail!("--colors takes one of: {}", themes::all().iter().map(|t| t.id.as_str()).collect::<Vec<_>>().join(", ")),
    }
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
    // A job of the background work that panics shows as its error, and says nothing on stderr.
    tokprof_browse::contain::install_hook();
    let mut desk = desk(&cli, paths::placing(tokprof_sources::catalog()), rune::is_headless(), Arc::new(wake))?;
    // The first frame is laid out for the window's size before the root's layout says it.
    desk.window = size_of(&args).unwrap_or(view::WINDOW);
    let spec = AppSpec {
        min_size: Some((960.0, 640.0)),
        // The system's font, as a Mac app's text is; Inter where there is none.
        families: ("system-ui, Inter", "ui-monospace"),
        scenes: SCENES,
        setup: Some(setup),
        poll: Some(poll),
        on_key: Some(on_key),
        ..AppSpec::new("tokprof", view::WINDOW, placeholder, view::view)
    };
    let settings = rune::MenuItem::new("Settings…", |s: &mut Desk, _| s.toggle_settings()).id("settings");
    let settings = match "cmd+,".parse::<rune::Shortcut>() {
        Ok(chord) => settings.key_equivalent(chord),
        Err(_) => settings,
    };
    let palette = rune::MenuItem::new("Command Palette…", |s: &mut Desk, _| s.toggle_palette()).id("palette");
    let palette = match "cmd+k".parse::<rune::Shortcut>() {
        Ok(chord) => palette.key_equivalent(chord),
        Err(_) => palette,
    };
    let app = App::new(spec).chrome(CHROME).menu_item(palette).menu_item(settings);
    // The menu bar's picks reach the window through the running app.
    desk.menu_bar.handle = Some(app.handle());
    app.init(move || desk).run();
    Ok(())
}

/// A named state a frame without a window can start from (`--scene`).
type Scene = (&'static str, fn(&mut Desk));

/// The scenes: a pane with the keyboard, the flame graph zoomed two levels in, a filter being typed
/// that matches nothing, the settings open, the palette open with a query, the key help, Week,
/// Month and Quarter waited for, Month with the sessions of its first saving open and with its
/// card, and Calendar.
const SCENES: &[Scene] = &[
    ("settings", |s| s.panel = Some(settings::Panel::default())),
    ("waiting", |s| {
        s.browser.set_waiting(true);
        s.browser.sort_by(Sort::Waiting);
        sessions::scene(s);
    }),
    // Sessions in time order with the newest session selected and its inspector, in Projects with
    // every project folded, and sorted by cost without the inspector.
    ("sessions", |s| {
        select_newest(s);
        sessions::scene(s);
    }),
    ("sessions-projects", sessions::scene_projects),
    ("sessions-cost", |s| {
        s.browser.sort_by(Sort::Cost);
        sessions::scene(s);
        s.sessions.inspector = false;
    }),
    ("palette", |s| s.palette = Some(palette::Palette { query: "to".into(), at: 0 })),
    ("keys", |s| s.help = true),
    // Week on the week of --week, or this week, with the store and the week's logs waited for.
    ("week", |s| {
        let date = s.week_wanted;
        week::scene(s, date);
    }),
    ("calendar", |s| {
        reports::show(s, reports::Mode::Calendar);
        let changes = s.browser.changes();
        s.calendar.data.settle(changes, Duration::from_secs(120));
    }),
    // Month on the month of --month and Quarter on the quarter of --quarter, or the current
    // ones, with the store, the sums, the period's logs and the card's image waited for.
    ("month", |s| {
        let ym = s.month.wanted.map(tokprof_core::period::Period::Month);
        reports::scene(s, ym, false);
    }),
    ("quarter", |s| {
        let yq = s.quarter_wanted.map(tokprof_core::period::Period::Quarter);
        reports::scene(s, yq, true);
    }),
    // Month with the sessions of the first cause that has any open, with the keyboard on it.
    ("month-advice", |s| {
        let ym = s.month.wanted.map(tokprof_core::period::Period::Month);
        reports::scene(s, ym, false);
        let first = s.reports.changes().and_then(|c| c.saved.iter().find(|r| c.sessions_of(&r.name).is_some_and(|l| !l.is_empty())).map(|r| r.name.clone()));
        if let Some(name) = first {
            changes::toggle(s, reports::Part::Saved, &name);
            // Its last session on the screen, so that the row and its sessions all show.
            let n = s.reports.changes().and_then(|c| c.sessions_of(&name)).map_or(0, |l| l.len().min(changes::SESSIONS));
            s.reports.reveal = Some(format!("{}-{}", reports::Part::Saved.key(), s.reports.row + n));
        }
    }),
    // Month with its card open, as `r` opens it there.
    ("month-review", |s| {
        let ym = s.month.wanted.map(tokprof_core::period::Period::Month);
        reports::scene(s, ym, false);
        let open = reports::review_month(s);
        month::open(s, Some(open));
    }),
    ("month-card", |s| {
        // Drawn from the store already, with --card-image.
        if s.month.alone {
            return;
        }
        reports::show(s, reports::Mode::Month);
        let changes = s.browser.changes();
        s.calendar.data.settle(changes, Duration::from_secs(120));
        let ym = month::month_to_open(s, s.month.wanted);
        month::open(s, Some(ym));
        s.month.alone = true;
        // Never a picture of something else under the card's name.
        if s.month.card.is_none() {
            eprintln!("could not draw the month card: the day store gave no month");
            std::process::exit(2);
        }
    }),
    ("calendar-day", |s| {
        reports::show(s, reports::Mode::Calendar);
        let changes = s.browser.changes();
        s.calendar.data.settle(changes, Duration::from_secs(120));
        // The cursor a week back, with its day's tooltip.
        s.calendar.cursor = s.calendar.calendar().map(|c| c.last - 7);
    }),
    ("lanes-keyboard", |s| s.pane = Focus::Lanes),
    ("inspector-keyboard", |s| {
        scene_session(s);
        s.pane = Focus::Inspector;
        s.tab = Tab::Tools;
        s.cursor = 2;
    }),
    ("requests-hover", |s| {
        scene_session(s);
        requests::scene_hover(s);
    }),
    ("requests-range", |s| {
        scene_session(s);
        requests::scene_range(s);
    }),
    ("flame-zoomed", |s| {
        scene_session(s);
        s.pane = Focus::Inspector;
        s.tab = Tab::Flame;
        s.flame.dive = 2;
    }),
    ("filter-typing", |s| {
        let view = s.viewport();
        for c in "/zzzz".chars() {
            s.browser.on_key(Key::Char(c), view);
        }
    }),
];

/// `AppSpec` wants a state it can make; the window starts from the one `main` made instead.
fn placeholder() -> Desk {
    unreachable!("the window starts from the state main made")
}

#[cfg(test)]
mod tests;
