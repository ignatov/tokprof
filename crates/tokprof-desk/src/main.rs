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
        }
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

/// How a key scrolls the full-size inspector, in points: a line, or a page.
fn pane_scroll(e: &KeyEvent) -> Option<i32> {
    match &e.key {
        RuneKey::Up => Some(-1),
        RuneKey::Down => Some(1),
        RuneKey::Text(t) if t == "k" && !e.mods.cmd => Some(-1),
        RuneKey::Text(t) if t == "j" && !e.mods.cmd => Some(1),
        RuneKey::PageUp => Some(i32::MIN),
        RuneKey::PageDown => Some(i32::MAX),
        RuneKey::Text(t) if t == " " && !e.mods.cmd => Some(i32::MAX),
        _ => None,
    }
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
    // The full-size inspector scrolls in its own viewport, which the wheel moves too.
    if s.browser.zoomed()
        && !s.browser.editing()
        && let Some(lines) = pane_scroll(e)
    {
        match lines {
            i32::MIN => tx.scroll_page(view::PANE, -1),
            i32::MAX => tx.scroll_page(view::PANE, 1),
            n => tx.scroll_to_y(view::PANE, (s.pane_offset.get() + n as f32 * 20.0).max(0.0)),
        }
        return true;
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

/// The window's own title bar: the traffic lights where the bar's height centers them, and only
/// the bar's empty parts move the window.
const CHROME: Chrome = Chrome { full_size_content: true, traffic_lights: Some((18.0, 16.0)), drag_regions: true };

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    check_size(&args)?;
    // Rune takes its own flags; the rest are ours.
    let cli = Cli::try_parse_from(std::iter::once("tokprof-desk".to_string()).chain(rune::extra_args().iter().cloned())).unwrap_or_else(|e| e.exit());
    let desk = desk(&cli, tokprof_sources::catalog(), rune::is_headless(), Arc::new(wake))?;
    let spec = AppSpec {
        min_size: Some((960.0, 640.0)),
        setup: Some(setup),
        poll: Some(poll),
        on_key: Some(on_key),
        ..AppSpec::new("tokprof", (1400.0, 900.0), placeholder, view::view)
    };
    App::new(spec).chrome(CHROME).init(move || desk).run();
    Ok(())
}

/// `AppSpec` wants a state it can make; the window starts from the one `main` made instead.
fn placeholder() -> Desk {
    unreachable!("the window starts from the state main made")
}

#[cfg(test)]
mod tests;
