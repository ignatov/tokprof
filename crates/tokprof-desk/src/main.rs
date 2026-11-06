//! `tokprof-desk`: the session browser of tokprof in a native window. The browser of
//! `tokprof-browse` holds the sessions, their costs, X-rays and days, as it does for `tokprof ui`;
//! this draws them with Rune and passes the window's keys and clicks on.

mod ansi;
mod day;
mod view;

use std::cell::Cell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use anyhow::{Result, bail};
use clap::Parser;
use rune::{App, AppSpec, Key as RuneKey, KeyEvent, Tx, Waker};

use tokprof_browse::browser::{Browser, Frontend, Key, Viewport, Wake, now_ms};
use tokprof_browse::filters::{normalize_project, parse_date, parse_since};
use tokprof_browse::paths;
use tokprof_core::catalog::Catalog;
use tokprof_core::query::Query;
use tokprof_core::rhythm;
use tokprof_core::source::AgentId;

use crate::ansi::Styled;

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
    /// With --snap or --tree: show the day view of this date (YYYY-MM-DD).
    #[arg(long)]
    day: Option<String>,
}

pub type Book = Browser<Styled>;

/// The window's state: the browser and what the window keeps of its own.
pub struct Desk {
    pub browser: Book,
    /// Whether the window draws dark, as the system was when the view last ran; rows built later,
    /// as the list scrolls, read it.
    pub dark: Cell<bool>,
    /// How far the list and the right pane are scrolled, in points, as their viewports last said.
    pub list_offset: Rc<Cell<f32>>,
    pub pane_offset: Rc<Cell<f32>>,
    /// The heights of the list and the right pane, from their layout.
    pub list_height: f32,
    pub pane_height: f32,
    /// When a listed cache goes cold next, as the last poll found it.
    pub next_cold: Option<i64>,
    /// The browser's count of lists put in place again, as the last poll saw it.
    pub relisted: u32,
    /// Where the pointer rests over the day's strip.
    pub pointer: Option<day::Pointer>,
}

impl Desk {
    pub fn new(mut browser: Book) -> Desk {
        // The window lays the day out itself; the browser computes it at once.
        browser.set_pane(view::DAY_PANE);
        let relisted = browser.relisted();
        Desk {
            browser,
            dark: Cell::new(true),
            list_offset: Rc::default(),
            pane_offset: Rc::default(),
            list_height: 0.0,
            pane_height: 0.0,
            next_cold: None,
            relisted,
            pointer: None,
        }
    }

    /// Scrolls the list so that its first row on the screen is the browser's, keeping how far
    /// into that row it was scrolled.
    fn restore_list(&self, tx: &mut Tx) {
        let into = self.list_offset.get().rem_euclid(view::ROW);
        tx.scroll_to_y(view::LIST, self.browser.top() as f32 * view::ROW + into);
    }

    /// Selects the session with this id, as a click on its row does.
    pub fn select_session(&mut self, session: &str) {
        if let Some(row) = self.browser.row_of(session) {
            self.browser.select_row(row);
        }
    }

    /// The rows of the list a page moves by, and the lines of the right pane.
    fn viewport(&self) -> Viewport {
        Viewport { page: ((self.list_height / view::ROW) as usize).max(1), lines: (self.pane_height / view::LINE) as usize }
    }
}

/// How the window shows the browser's texts: without their color codes, with the ranges of each
/// style.
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
/// listed cache goes cold or held-back work is due.
fn poll(s: &mut Desk, _: f64, tx: &mut Tx) -> bool {
    // Where the list is scrolled to, unless the list was put in place since and the browser
    // knows better.
    if s.browser.relisted() == s.relisted {
        s.browser.set_top((s.list_offset.get() / view::ROW).max(0.0) as usize);
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
    arrived || cooled
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

/// How a key scrolls a full-size X-ray or day, in lines: by one, or by a page.
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
    // The full-size X-ray and day scroll in the window's own viewport, which the wheel moves too.
    if s.browser.zoomed()
        && !s.browser.editing()
        && let Some(lines) = pane_scroll(e)
    {
        match lines {
            i32::MIN => tx.scroll_page(view::PANE, -1),
            i32::MAX => tx.scroll_page(view::PANE, 1),
            n => tx.scroll_to_y(view::PANE, (s.pane_offset.get() + n as f32 * view::LINE).max(0.0)),
        }
        return true;
    }
    let Some(key) = key_of(e) else { return false };
    let (selected, zoomed, day) = (s.browser.selected(), s.browser.zoomed(), s.browser.showing_day());
    if !s.browser.on_key(key, s.viewport()) {
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
    if (s.browser.selected(), s.browser.zoomed(), s.browser.showing_day()) != (selected, zoomed, day) {
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
/// (`--snap`, `--tree`), it follows no logs and waits for the X-ray of the newest session, or for
/// the day `--day` names, as `tokprof ui --dump` waits.
fn desk(cli: &Cli, catalog: Catalog, headless: bool, wake: Wake) -> Result<Desk> {
    let catalog = Arc::new(catalog);
    let agents = match cli.agent.is_empty() {
        true => catalog.agents(),
        false => cli.agent.iter().map(|a| catalog.agent(a)).collect::<Result<Vec<_>>>()?,
    };
    let select = query(cli, agents)?;
    let pricing = paths::pricing(cli.prices.as_deref())?;
    let mut browser = Browser::start(catalog, pricing, select, !headless, wake, frontend())?;
    if headless {
        browser.settle();
        if let Some(date) = cli.day.as_deref() {
            let date = rhythm::date_of(parse_date(date)? + 12 * 3_600_000, &rhythm::local_offset);
            browser.show_day(date);
            browser.set_pane(view::DAY_PANE);
            browser.settle_day();
        }
    }
    Ok(Desk::new(browser))
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    check_size(&args)?;
    // Rune takes its own flags; the rest are ours.
    let cli = Cli::try_parse_from(std::iter::once("tokprof-desk".to_string()).chain(rune::extra_args().iter().cloned())).unwrap_or_else(|e| e.exit());
    let desk = desk(&cli, tokprof_sources::catalog(), rune::is_headless(), Arc::new(wake))?;
    let spec = AppSpec {
        min_size: Some((720.0, 400.0)),
        setup: Some(setup),
        poll: Some(poll),
        on_key: Some(on_key),
        ..AppSpec::new("tokprof", (1400.0, 900.0), placeholder, view::view)
    };
    App::new(spec).init(move || desk).run();
    Ok(())
}

/// `AppSpec` wants a state it can make; the window starts from the one `main` made instead.
fn placeholder() -> Desk {
    unreachable!("the window starts from the state main made")
}

#[cfg(test)]
mod tests;
