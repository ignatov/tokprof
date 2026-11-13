//! The Week view: the seven days of a calendar week on the left, each a strip of its day on one
//! clock axis with the day's numbers, the totals and the week's rhythm lines under them; on the
//! right a column of the tiles against the week before, What to change, the line for next week
//! and By project (desk-window, Week view). The numbers come from the day store at once and the
//! rest from one background read of the week's logs (see `tokprof_browse::week`).

use std::sync::Arc;

use rune::{Color, Cx, El, Key as RuneKey, KeyEvent, ScrollSpec, Shape, Span, Tx, canvas, col, el, rich, row, spacer, text};
use tokprof_browse::browser::{Key, now_ms};
use tokprof_browse::week::{Key as WeekKey, Read, WeekLogs, Weeks};
use tokprof_core::compare::{self, Sense, Verdict};
use tokprof_core::fmt::{clip_end, count, int, local_time, usd};
use tokprof_core::rhythm::{self, Offset};
use tokprof_core::savings::{Changes, SessionCost};
use tokprof_core::week::{self as core_week, Row, RowKind, Sums, Week};

use crate::inspector::Tab;
use crate::overlay::{self, R_GROUP, R_MARK, R_ROW, R_TILE};
use crate::pal::{self, Pal};
use crate::reports::{Mode, show};
use crate::{Desk, Focus, plain};

/// The key of the week's scroll viewport.
pub const PANE: &str = "week";
/// The width of the column on the right.
pub const COLUMN_W: f32 = 340.0;
/// The room around the pane's content and between the days and the column.
const PAD: f32 = 20.0;
const GAP: f32 = 16.0;
/// A day's row: the weekday and date, the strip, the four numbers.
const LABEL_W: f32 = 62.0;
const NUM_W: [f32; 4] = [54.0, 44.0, 58.0, 46.0];
/// The room inside the card of the days.
const DAYS_PAD: f32 = 10.0;
const CELL_GAP: f32 = 8.0;
const ROW_H: f32 = 46.0;
/// The strip in a row, from its top, its bars and ticks, and the agents band under it.
const STRIP_TOP: f32 = 6.0;
const STRIP_H: f32 = 26.0;
const BAR_H: f32 = 14.0;
const BAND_GAP: f32 = 3.0;
const BAND_H: f32 = 6.0;
/// A cold-return tick: its line, and the width its click takes.
const TICK_W: f32 = 2.0;
const TICK_HIT: f32 = 8.0;
/// The rows of What to change from each of its two parts, and the sessions a row opens.
const CHANGES_SHOWN: usize = 3;
const SESSIONS: usize = crate::changes::SESSIONS;
const HOUR_MS: i64 = 3_600_000;
const DAY_MS: i64 = 86_400_000;

/// The parts of Week that take the keyboard, in the order Tab moves through them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Part {
    Days,
    Changes,
    Projects,
}

impl Part {
    pub const ORDER: [Part; 3] = [Part::Days, Part::Changes, Part::Projects];

    pub fn key(self) -> &'static str {
        match self {
            Part::Days => "week-days",
            Part::Changes => "week-changes",
            Part::Projects => "week-projects",
        }
    }

    fn step(self, back: bool) -> Part {
        let n = Part::ORDER.len();
        let at = Part::ORDER.iter().position(|&p| p == self).unwrap_or(0);
        Part::ORDER[if back { (at + n - 1) % n } else { (at + 1) % n }]
    }
}

/// The two parts of What to change.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Section {
    Saved,
    Habits,
}

/// A line of What to change the cursor moves through: a row, or a session of the row that is
/// open.
#[derive(Clone, Debug, PartialEq)]
pub enum Line {
    Row(Section, String),
    Session(Section, String, usize),
}

/// The view's state: the week shown, the cursor, the part with the keyboard, the pointer, and
/// the weeks computed.
pub struct State {
    /// The Monday of the week shown, as a local date; `None` for this week.
    pub monday: Option<i64>,
    /// The day the cursor rings; `None` for today in this week, Monday in another.
    pub cursor: Option<i64>,
    pub part: Part,
    /// The cursor among the lines of What to change or the rows of By project.
    pub row: usize,
    /// The row of What to change whose sessions are open under it.
    pub open: Option<(Section, String)>,
    /// The pointer over a strip: the row's date, and how far into the strip, in points.
    pub hover: Option<(i64, f32)>,
    pub data: Weeks,
    /// The filter text the week is counted with: the window's, once it is not being typed.
    pub text: String,
    /// The window filters by client, which the store cannot.
    pub client_ignored: bool,
    /// A clock for tests, in place of now.
    pub clock: Option<i64>,
    /// The store's and the read's progress as the last poll saw them.
    seen: (Progress, Progress),
}

impl State {
    pub fn new(data: Weeks) -> State {
        State {
            monday: None,
            cursor: None,
            part: Part::Days,
            row: 0,
            open: None,
            hover: None,
            data,
            text: String::new(),
            client_ignored: false,
            clock: None,
            seen: (None, None),
        }
    }
}

/// Logs read of all a worker reads, while it reads.
type Progress = Option<(usize, usize)>;

fn offset() -> Offset<'static> {
    &rhythm::local_offset
}

/// Now, or the tests' clock.
pub fn now(s: &Desk) -> i64 {
    s.week.clock.unwrap_or_else(now_ms)
}

/// The Monday of this week.
pub fn this_monday(s: &Desk) -> i64 {
    core_week::monday_of(rhythm::date_of(now(s), offset()))
}

/// The Monday of the week shown.
pub fn monday(s: &Desk) -> i64 {
    s.week.monday.unwrap_or_else(|| this_monday(s))
}

/// What the week shown is kept by: its Monday and the filter text.
pub fn key(s: &Desk) -> WeekKey {
    WeekKey::new(monday(s), &s.week.text)
}

/// The store's sums of the week shown, once computed.
pub fn week(s: &Desk) -> Option<&Arc<Week>> {
    s.week.data.week(&key(s))
}

/// The read of the week shown's logs, once it arrived.
fn read(s: &Desk) -> Option<&Read> {
    s.week.data.logs(&key(s))
}

/// What the week's logs gave, once read without an error.
pub fn logs(s: &Desk) -> Option<&WeekLogs> {
    read(s).and_then(|r| r.logs.as_deref().ok())
}

/// Week is entered: it opens on this week, with the keyboard on the days.
pub fn entered(s: &mut Desk) {
    s.week.monday = None;
    s.week.cursor = None;
    s.week.part = Part::Days;
    s.week.row = 0;
    s.week.open = None;
    s.week.hover = None;
    if !s.browser.editing() {
        s.week.text = s.browser.filter().to_string();
    }
}

/// Shows this week, with the cursor on today, as Today and `t` do.
pub fn this_week(s: &mut Desk, tx: &mut Tx) {
    s.week.monday = None;
    s.week.cursor = None;
    s.week.open = None;
    s.week.hover = None;
    tx.scroll_to_y(PANE, 0.0);
}

/// The Monday of the earliest week that can be shown: the one with the store's first day.
fn first_monday(s: &Desk) -> Option<i64> {
    week(s).and_then(|w| w.first_data).map(core_week::monday_of)
}

/// Steps the week by `delta` weeks, never past this week nor before the week of the store's
/// first day with data.
pub fn step(s: &mut Desk, delta: i64, tx: &mut Tx) {
    let this = this_monday(s);
    let mut to = (monday(s) + 7 * delta).min(this);
    if let Some(first) = first_monday(s) {
        to = to.max(first.min(this));
    }
    if to == monday(s) {
        return;
    }
    s.week.monday = (to != this).then_some(to);
    s.week.cursor = None;
    s.week.open = None;
    s.week.hover = None;
    if s.week.part != Part::Days {
        s.week.row = 0;
    }
    tx.scroll_to_y(PANE, 0.0);
}

/// Takes in what the workers sent while Week is shown: the store from the calendar's worker, the
/// week's sums and its read; asks for what is missing or out of date, and wakes the window when
/// one is due. Returns whether the view changed. While Week is not shown it does nothing.
pub fn poll(s: &mut Desk, tx: &mut Tx) -> bool {
    if s.reports.mode != Mode::Week {
        return false;
    }
    // The filter counts once it is not being typed.
    let mut changed = false;
    if !s.browser.editing() && s.week.text != s.browser.filter() {
        s.week.text = s.browser.filter().to_string();
        s.week.row = 0;
        s.week.open = None;
        changed = true;
    }
    let changes = s.browser.changes();
    let stored = s.calendar.data.tick(changes);
    let wake =
        |tx: &mut Tx, at: std::time::Instant| tx.wake_after(at.saturating_duration_since(std::time::Instant::now()).max(std::time::Duration::from_millis(1)));
    if let Some(at) = s.calendar.data.next_due(changes) {
        wake(tx, at);
    }
    let store = s.calendar.data.get().and_then(|c| c.store.clone());
    let key = key(s);
    let arrived = s.week.data.tick(&key, store.as_ref(), changes);
    if let Some(at) = s.week.data.next_due(&key, changes) {
        wake(tx, at);
    }
    let seen = (s.calendar.data.progress(), s.week.data.progress());
    let moved = seen != s.week.seen;
    s.week.seen = seen;
    changed || stored || arrived || moved
}

/// The rows of What to change: the first three of each part.
fn shown_rows(c: &Changes, sec: Section) -> &[tokprof_core::pool::Row] {
    let rows = match sec {
        Section::Saved => &c.saved,
        Section::Habits => &c.habits,
    };
    &rows[..rows.len().min(CHANGES_SHOWN)]
}

/// The lines of What to change, with the sessions of the row that is open under it.
pub fn lines(c: &Changes, open: Option<&(Section, String)>) -> Vec<Line> {
    let mut out = Vec::new();
    for sec in [Section::Saved, Section::Habits] {
        for r in shown_rows(c, sec) {
            out.push(Line::Row(sec, r.name.clone()));
            if open.is_some_and(|(s, n)| *s == sec && *n == r.name) {
                let n = c.sessions_of(&r.name).map_or(0, |l| l.len().min(SESSIONS));
                out.extend((0..n).map(|i| Line::Session(sec, r.name.clone(), i)));
            }
        }
    }
    out
}

/// What to change of the week shown, once read.
fn changes(s: &Desk) -> Option<&Changes> {
    logs(s).filter(|_| !week(s).is_some_and(|w| w.totals.all_gone())).map(|l| &l.overview.changes)
}

/// Opens or closes the sessions of a row; a row not split by session has none to open.
pub fn toggle(s: &mut Desk, sec: Section, name: &str) {
    let Some(c) = changes(s) else { return };
    let key = (sec, name.to_string());
    let open = if c.sessions_of(name).is_none_or(|l| l.is_empty()) {
        s.week.open.clone()
    } else if s.week.open.as_ref() == Some(&key) {
        None
    } else {
        Some(key)
    };
    let at = lines(c, open.as_ref()).iter().position(|l| *l == Line::Row(sec, name.to_string())).unwrap_or(0);
    s.week.open = open;
    s.week.part = Part::Changes;
    s.week.row = at;
}

/// What ⏎ does on the cursor's line of What to change.
fn enter_change(s: &mut Desk, tx: &mut Tx) {
    let Some(c) = changes(s) else { return };
    match lines(c, s.week.open.as_ref()).get(s.week.row).cloned() {
        Some(Line::Row(sec, name)) => toggle(s, sec, &name),
        Some(Line::Session(_, name, i)) => {
            if let Some(x) = c.sessions_of(&name).and_then(|l| l.get(i)).cloned() {
                crate::changes::open_session(s, &x, tx);
            }
        }
        None => {}
    }
}

/// Sets the window's filter to a project and stays on the week, as a click on it does.
pub fn filter_project(s: &mut Desk, project: &str, tx: &mut Tx) {
    s.browser.set_filter(project);
    s.week.text = project.to_string();
    s.week.open = None;
    tx.scroll_to_y(PANE, 0.0);
}

/// The days the cursor can open, in order.
fn openable(w: &Week) -> Vec<i64> {
    w.rows.iter().filter(|r| r.opens()).map(|r| r.date).collect()
}

/// The cursor's date: the one it was moved to, else today in this week and Monday in another;
/// within the days that open.
pub fn cursor_date(s: &Desk) -> Option<i64> {
    let w = week(s)?;
    let days = openable(w);
    let (&first, &last) = (days.first()?, days.last()?);
    let today = rhythm::date_of(now(s), offset());
    let default = if s.week.monday.is_none() { today } else { monday(s) };
    Some(s.week.cursor.unwrap_or(default).clamp(first, last))
}

/// Opens Day of `date`, as a click on its row does.
pub fn open_day(s: &mut Desk, date: i64, tx: &mut Tx) {
    crate::reports::open_day(s, date, tx);
}

/// Opens Day of `date` with the session of a cold return selected and its inspector on `Cache`,
/// as a click on its tick does; a session the list does not hold says so, and Day shows the date.
pub fn open_tick(s: &mut Desk, date: i64, session: &str, tx: &mut Tx) {
    let Some(row) = s.browser.row_of(session) else {
        let id: String = session.chars().take(6).collect();
        s.status = (format!("session {id} is not in the list"), std::time::Instant::now());
        open_day(s, date, tx);
        return;
    };
    let id = s.browser.session(row).to_string();
    s.none = false;
    s.browser.select_row(row);
    // The tick's day, whichever day the selection would show.
    s.browser.show_day(date);
    s.stepped = false;
    s.pane = Focus::Lanes;
    show(s, Mode::Day);
    s.show_tab(Tab::Cache, tx);
    s.reveal_lane = Some(id.clone());
    tx.scroll_into_view(format!("lane-{id}"));
}

/// The keys of Week: the cursor among the days, the week, the parts, the rows of What to change
/// and By project, `/`, `t` and esc; every other key does nothing.
pub fn on_key(s: &mut Desk, e: &KeyEvent, tx: &mut Tx) -> bool {
    // A filter being typed takes the keys.
    if s.browser.editing() {
        if let Some(key) = crate::key_of(e) {
            if !crate::browser_key(s, key, tx) {
                tx.quit();
            }
            return true;
        }
        return false;
    }
    if !plain(e) && e.key != RuneKey::Tab {
        return false;
    }
    let text = match &e.key {
        RuneKey::Text(t) => Some(t.as_str()),
        _ => None,
    };
    match (&e.key, text) {
        (RuneKey::Tab, _) => {
            s.week.part = s.week.part.step(e.mods.shift);
            s.week.row = 0;
            tx.scroll_into_view(s.week.part.key());
            return true;
        }
        (RuneKey::Escape, _) => {
            if s.week.part != Part::Days {
                s.week.part = Part::Days;
            } else if !s.browser.filter().is_empty() {
                s.browser.clear_filter();
            } else {
                show(s, Mode::Day);
            }
            return true;
        }
        (_, Some("/")) => {
            let view = s.viewport();
            s.browser.on_key(Key::Char('/'), view);
            return true;
        }
        (_, Some("t")) => {
            this_week(s, tx);
            return true;
        }
        (RuneKey::Left, _) | (_, Some("h" | "[")) => {
            step(s, -1, tx);
            return true;
        }
        (RuneKey::Right, _) | (_, Some("l" | "]")) => {
            step(s, 1, tx);
            return true;
        }
        _ => {}
    }
    let moved: Option<isize> = match (&e.key, text) {
        (RuneKey::Up, _) | (_, Some("k")) => Some(-1),
        (RuneKey::Down, _) | (_, Some("j")) => Some(1),
        (RuneKey::PageUp, _) => Some(-10),
        (RuneKey::PageDown, _) => Some(10),
        (RuneKey::Home, _) | (_, Some("g")) => Some(isize::MIN / 2),
        (RuneKey::End, _) | (_, Some("G")) => Some(isize::MAX / 2),
        _ => None,
    };
    let enter = e.key == RuneKey::Enter;
    match s.week.part {
        Part::Days => {
            let Some(w) = week(s) else { return true };
            let days = openable(w);
            let Some(at) = cursor_date(s).and_then(|d| days.iter().position(|&x| x == d)) else { return true };
            if let Some(step) = moved {
                let to = (at as isize).saturating_add(step).clamp(0, days.len() as isize - 1) as usize;
                s.week.cursor = Some(days[to]);
                tx.scroll_into_view(format!("week-day-{}", days[to]));
            } else if enter {
                open_day(s, days[at], tx);
            }
        }
        Part::Changes => {
            let n = changes(s).map_or(0, |c| lines(c, s.week.open.as_ref()).len());
            if let Some(step) = moved {
                if n > 0 {
                    s.week.row = (s.week.row as isize).saturating_add(step).clamp(0, n as isize - 1) as usize;
                    tx.scroll_into_view(format!("wchange-{}", s.week.row));
                }
            } else if enter {
                enter_change(s, tx);
            }
        }
        Part::Projects => {
            let Some(w) = week(s).cloned() else { return true };
            let (shown, more) = w.shown_projects();
            let n = shown.len() + usize::from(more > 0);
            if let Some(step) = moved {
                if n > 0 {
                    s.week.row = (s.week.row as isize).saturating_add(step).clamp(0, n as isize - 1) as usize;
                    tx.scroll_into_view(format!("wproj-{}", s.week.row));
                }
            } else if enter && let Some(p) = shown.get(s.week.row) {
                filter_project(s, &p.name, tx);
            }
        }
    }
    true
}

// ---------------------------------------------------------------------------------------------
// The bar.

/// The gap between the bar's controls.
const BAR_GAP: f32 = 10.0;

/// What the bar holds in Week: the week's name with the steps to the weeks before and after,
/// Today, and the week's summary as much of it as has room.
pub fn bar_controls(s: &Desk, cx: &Cx<Desk>, p: &Pal) -> Vec<El> {
    let m = monday(s);
    let this = this_monday(s);
    let first = first_monday(s);
    let back = first.is_none_or(|f| m > f);
    let mut name = row().gap(6.0).center_y().child(text(core_week::name(m)).key("week-name").font(pal::DAY).color(p.ink).single_line());
    let k = key(s);
    let again = (s.week.data.reading(&k) && read(s).is_some()) || (s.week.data.summing(&k) && week(s).is_some());
    if again {
        name = name.child(text("updating…").key("week-updating").font(pal::SMALL).color(p.ink3).single_line());
    }
    let mut out = vec![
        crate::view::step_button("week-back", "‹", "The week before", back, p).on_press_tx(|s: &mut Desk, tx| step(s, -1, tx)),
        name,
        crate::view::step_button("week-on", "›", "The week after", m < this, p).on_press_tx(|s: &mut Desk, tx| step(s, 1, tx)),
        crate::view::button("today", "Today", m < this, p.panel, p).on_press_tx(|s: &mut Desk, tx| this_week(s, tx)),
    ];
    // The room between Today and what the bar holds on its right; the summary is the first part
    // of the bar to give way, so that the filter keeps its width.
    let today = cx.layout_with("today", |r| (r[0] + r[2]).round());
    let right = if s.status_shown() { cx.layout_with("status", |r| r[0].round()) } else { cx.layout_with("filter", |r| r[0].round()) };
    let room = today.zip(right).map(|(a, b)| b - a - 3.0 * BAR_GAP);
    if let Some(t) = fitting_summary(s, room) {
        out.push(text(t).key("week-summary").font(pal::BODY).color(p.ink2).single_line().truncate().shrink().min_w(0.0));
    }
    out
}

/// The longest summary that fits in `room` points: `Mon–Thu so far · 39 sittings`, then
/// `Mon–Thu so far`, then none; all of it while the room is not known yet.
pub fn fitting_summary(s: &Desk, room: Option<f32>) -> Option<String> {
    let full = summary(s);
    let mut tries = vec![full.clone()];
    if let Some((days, _)) = full.split_once(" · ") {
        tries.push(days.to_string());
    }
    let Some(room) = room else { return Some(full).filter(|t| !t.is_empty()) };
    tries.into_iter().find(|t| !t.is_empty() && rune::text::advance(t, pal::BODY) <= room)
}

/// The week's summary in the bar: `Mon–Thu so far · 39 sittings` for this week, `82 sittings`
/// for another.
pub fn summary(s: &Desk) -> String {
    let Some(w) = week(s) else { return String::new() };
    let n = w.totals.sittings.len();
    let sittings = count(n as u64, "sitting", "sittings");
    if w.window.complete() {
        return sittings;
    }
    let today = rhythm::date_of(w.window.to_ms.max(w.window.from_ms), offset());
    let days = if today == w.window.monday { weekday(today).to_string() } else { format!("{}–{}", weekday(w.window.monday), weekday(today)) };
    format!("{days} so far · {sittings}")
}

fn weekday(date: i64) -> &'static str {
    ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"][tokprof_core::calendar::weekday(date) as usize]
}

// ---------------------------------------------------------------------------------------------
// The view.

/// Week under the bar: the notes, then the days and the column side by side, in one scrolling
/// pane.
pub fn view(s: &Desk, cx: &Cx<Desk>, p: &Pal) -> El {
    let w = &s.week;
    let k = key(s);
    let sums = week(s).map(|x| Arc::as_ptr(x) as usize);
    let logs = read(s).map(|r| (r.arrived, r.logs.as_ref().map(|l| Arc::as_ptr(l) as usize).ok()));
    let width = crate::view::window_size(s, cx).0.round() as i64;
    let store = s.calendar.data.get().map(|c| (c.arrived, c.store.as_ref().map(|x| Arc::as_ptr(x) as usize)));
    let version = crate::view::hash_of((
        (&k, sums, logs, store, w.data.reading(&k), w.data.summing(&k), w.seen),
        (w.cursor, w.part, w.row, &w.open, w.hover.map(|(d, x)| (d, x.to_bits())), w.client_ignored),
        (&s.theme.get().id, cx.system_dark(), width, s.browser.filter(), s.browser.relisted(), s.browser.len()),
    ));
    let window_w = width as f32;
    col().key(PANE).w_full().grow(1.0).min_h(0.0).scroll(ScrollSpec { stick_bottom: false }).child(rune::memo("week-body", version, || body(s, p, window_w)))
}

/// The room the days take in a window `window_w` wide, and the width of a strip.
pub fn layout(window_w: f32) -> (f32, f32) {
    let days = (window_w - 2.0 * PAD - GAP - COLUMN_W).max(320.0);
    let strip = days - 2.0 * DAYS_PAD - LABEL_W - NUM_W.iter().sum::<f32>() - CELL_GAP * NUM_W.len() as f32 - CELL_GAP;
    (days, strip.max(120.0))
}

fn body(s: &Desk, p: &Pal, window_w: f32) -> El {
    let (days_w, strip_w) = layout(window_w);
    let mut out = col().w_full().px(PAD).pt(14.0).pb(30.0).gap(12.0);
    for n in notes(s, p) {
        out = out.child(n);
    }
    let left = col().key("week-left").w(days_w).min_w(0.0).gap(14.0).children(days(s, strip_w, p));
    let right = col().key("week-column").w(COLUMN_W).gap(14.0).children(column(s, p));
    out.child(row().w_full().gap(GAP).items_start().child(left).child(right))
}

/// The quiet lines under the bar: `--client`, the filter, and the match by title only.
fn notes(s: &Desk, p: &Pal) -> Vec<El> {
    let mut out = Vec::new();
    let line = |key: &'static str, t: String, c: Color| text(t).key(key).w_full().wrap().font(pal::SMALL).color(c);
    if s.week.client_ignored {
        out.push(line("week-client", "Week counts every client: the day store has no client".into(), p.ink3));
    }
    let t = s.week.text.trim();
    if !t.is_empty() {
        // A project a click set is named as By project names it; the field keeps its path.
        let project = week(s).filter(|w| w.projects.iter().any(|x| x.name == t));
        let note = match project {
            Some(w) => {
                let name = crate::view::ProjectNames::new(w.projects.iter().map(|x| x.name.as_str())).of(t);
                format!("Project \"{name}\": Week counts the logs whose agent, project or session id holds its path.")
            }
            None => format!("Filter \"{t}\": Week counts the logs whose agent, project or session id holds it."),
        };
        out.push(line("week-filter", note, p.ink2));
        let n = s.browser.title_only(t);
        if n > 0 {
            out.push(line(
                "week-titles",
                format!(
                    "{} \"{t}\" only by title; Week counts agents, projects and session ids, not titles. Sessions (⌘0) lists them.",
                    if n == 1 { "1 session matches".to_string() } else { format!("{} sessions match", int(n as u64)) }
                ),
                p.ink3,
            ));
        }
    }
    out
}

/// The line while the store is filled, or the week is summed.
fn waiting_line(key: &'static str, t: String, share: Option<f64>, p: &Pal) -> El {
    let mut c = col()
        .key(key)
        .w_full()
        .padding(18.0)
        .gap(12.0)
        .radius(R_GROUP)
        .fill(p.bar)
        .child(text(t).key("week-progress").font(pal::BODY_NUM).color(p.ink2).single_line().truncate().min_w(0.0));
    if let Some(share) = share {
        c = c.child(row().w(320.0).child(overlay::meter(share, p.accent, p, 6.0)));
    }
    c
}

/// The left side: the days with their strips, the totals and the rhythm lines.
fn days(s: &Desk, strip_w: f32, p: &Pal) -> Vec<El> {
    let store = s.calendar.data.get().and_then(|c| c.store.as_ref());
    if store.is_none() {
        let progress = s.calendar.data.progress();
        let share = progress.filter(|g| g.1 > 0).map(|g| g.0 as f64 / g.1 as f64).unwrap_or(0.0);
        let t = match s.calendar.data.get().and_then(|c| c.calendar.as_ref().err()) {
            Some(e) => format!("error: {e}"),
            None => crate::details::reading(progress),
        };
        return vec![waiting_line("week-filling", t, Some(share), p)];
    }
    let Some(w) = week(s) else { return vec![waiting_line("week-summing", "counting the week…".into(), None, p)] };
    let t = s.week.text.trim();
    if !t.is_empty() && !w.totals.any() && w.rows.iter().all(|r| !r.cell.any()) {
        return vec![
            text(format!("Nothing in {} matches \"{t}\".", w.name()))
                .key("week-nothing")
                .w_full()
                .wrap()
                .font(pal::BODY)
                .color(p.ink2)
                .padding(18.0)
                .radius(R_GROUP)
                .fill(p.bar),
        ];
    }
    let on = s.week.part == Part::Days;
    let mut rows = col().w_full().gap(2.0).child(head(w, strip_w, p));
    let cursor = cursor_date(s);
    for r in &w.rows {
        rows = rows.child(day_row(s, w, r, strip_w, on && cursor == Some(r.date), p));
    }
    let pointer = pointer_text(s, w, strip_w);
    let (pointer, color) =
        if pointer.is_empty() { ("point at a strip to read it; a click opens the day, a red tick its session".to_string(), p.ink3) } else { (pointer, p.ink2) };
    rows = rows.child(total_row(w, strip_w, p)).child(total_lines(w, p));
    rows = rows.child(text(pointer).key("week-pointer").h(18.0).pl(LABEL_W + CELL_GAP).font(pal::SMALL_NUM).color(color).single_line().truncate().min_w(0.0));
    // The cursor's ring on a day says that the days have the keyboard.
    let days = col().key(Part::Days.key()).w_full().padding(DAYS_PAD).radius(R_GROUP).fill(p.bar).child(rows);
    vec![days, legend(p), rhythm_lines(s, w, p)]
}

/// The head of the days: `Day`, the hour labels over the strips, and the heads of the numbers.
fn head(w: &Week, strip_w: f32, p: &Pal) -> El {
    let axis = w.axis;
    let per_hour = strip_w / axis.hours().max(1) as f32;
    let every = if per_hour >= 24.0 {
        1
    } else if per_hour >= 12.0 {
        2
    } else {
        3
    };
    let mut hours = el().key("week-hours").w(strip_w).h(18.0);
    for (h, label) in axis.labels() {
        if (h - axis.from_h) % every != 0 {
            continue;
        }
        let x = (h - axis.from_h) as f32 * per_hour;
        hours = hours.child(text(label).key(format!("hour-{h}")).font(pal::HOURS).color(p.ink3).single_line().absolute(x - 6.0, 2.0));
    }
    let mut out = row()
        .key("week-head")
        .w_full()
        .h(18.0)
        .gap(CELL_GAP)
        .center_y()
        .child(row().w(LABEL_W).child(text("Day").font(pal::SMALL_STRONG).color(p.ink2).single_line()));
    out = out.child(hours);
    for (i, h) in ["Spend", "Agent", "Keyboard", "Cold"].into_iter().enumerate() {
        out = out.child(row().w(NUM_W[i]).justify_end().child(text(h).font(pal::SMALL_STRONG).color(p.ink2).single_line()));
    }
    out
}

/// The part of a strip `[a, b)` of the row of `date` covers on the axis, in points, cut to it:
/// at least 1 point wide, or `None` outside it.
fn span_x(w: &Week, date: i64, (a, b): (i64, i64), strip_w: f32) -> Option<(f32, f32)> {
    let (x0, x1) = (w.axis.x(date, a, offset()), w.axis.x(date, b, offset()));
    if x1 <= 0.0 || x0 >= 1.0 || b <= a {
        return None;
    }
    let (x0, x1) = (x0.clamp(0.0, 1.0) as f32 * strip_w, x1.clamp(0.0, 1.0) as f32 * strip_w);
    Some((x0, (x1 - x0).max(1.0)))
}

/// Spans cut to a row's span, merged where closer than a point.
fn clipped(spans: impl Iterator<Item = (i64, i64)>, (from, to): (i64, i64), point_ms: i64) -> Vec<(i64, i64)> {
    let mut v: Vec<(i64, i64)> = spans.map(|(a, b)| (a.max(from), b.min(to))).filter(|(a, b)| b > a).collect();
    v.sort_unstable();
    let mut out: Vec<(i64, i64)> = Vec::new();
    for (a, b) in v {
        match out.last_mut() {
            Some(last) if a < last.1 + point_ms => last.1 = last.1.max(b),
            _ => out.push((a, b)),
        }
    }
    out
}

/// The strip's yellow of a row: where some session waited and no agent worked, from the read.
pub fn yellow_of(logs: Option<&WeekLogs>, r: &Row) -> Vec<(i64, i64)> {
    logs.map(|l| clipped(l.yellow.iter().copied(), r.span, 0)).unwrap_or_default()
}

/// Hours as the rows count them: `12.5h`, `13m` under an hour, `0h` for none.
pub fn hours(ms: i64) -> String {
    match ms {
        0 => "0h".to_string(),
        ms if ms < HOUR_MS => format!("{}m", (ms as f64 / 60_000.0).round().max(1.0) as i64),
        ms => format!("{:.1}h", ms as f64 / HOUR_MS as f64),
    }
}

/// A day's row: its weekday and date, its strip with the agents band, and its numbers.
fn day_row(s: &Desk, w: &Week, r: &Row, strip_w: f32, ringed: bool, p: &Pal) -> El {
    let date = r.date;
    let today = !w.window.complete() && rhythm::date_of(w.window.to_ms, offset()) == date;
    let ink = match r.kind {
        RowKind::Day if today => p.accent,
        RowKind::Day => p.ink,
        _ => p.ink3,
    };
    let noon = rhythm::midnight(date, offset()) + 12 * HOUR_MS;
    let label = col()
        .w(LABEL_W)
        .pl(6.0)
        .gap(1.0)
        .child(text(weekday(date)).key("day-weekday").font(pal::STRONG).color(ink).single_line())
        .child(text(local_time(noon, "%-d %b")).key("day-date").font(pal::SMALL).color(if today { p.accent } else { p.ink3 }).single_line());
    let mut line = row().key(format!("week-day-{date}")).w_full().h(ROW_H).gap(CELL_GAP).center_y().radius(R_ROW).child(label);
    match r.kind {
        RowKind::NoLogs => {
            return line.child(text("no logs").key("day-nologs").font(pal::SMALL).color(p.ink3).single_line().grow(1.0));
        }
        RowKind::Future => {
            let strip = el().key("strip").w(strip_w).h(ROW_H).child(el().absolute(0.0, STRIP_TOP).w(strip_w).h(STRIP_H).radius(4.0).border(1.0, p.row_line));
            return line.child(strip);
        }
        RowKind::Day => {}
    }
    line = line.child(strip(s, w, r, strip_w, today, p));
    let a = &r.cell.amounts;
    let cells = [
        (usd(a.usd), p.ink, "day-spend"),
        (hours(a.agent_minutes as i64 * 60_000), p.ink, "day-agent"),
        (hours(a.keyboard_ms), p.ink, "day-keyboard"),
        (format!("{} cold", int(r.cold as u64)), if r.cold > 0 { p.cold } else { p.ink3 }, "day-cold"),
    ];
    for (i, (t, c, k)) in cells.into_iter().enumerate() {
        line = line.child(row().w(NUM_W[i]).justify_end().child(text(t).key(k).font(pal::STRONG_NUM).color(c).single_line()));
    }
    let line =
        line.role("button").label(format!("{} {}", weekday(date), local_time(noon, "%-d %b"))).on_press_tx(move |s: &mut Desk, tx| open_day(s, date, tx));
    let line = if ringed { line.fill(p.sel) } else { line.hover(p.hover) };
    overlay::ring_at(line, ringed, 0.0, p)
}

/// A strip: the sittings, the waits, the work, the cold returns, the hour lines, the edge marks,
/// the now line and the agents band under it.
fn strip(s: &Desk, w: &Week, r: &Row, strip_w: f32, today: bool, p: &Pal) -> El {
    let date = r.date;
    let axis_ms = (w.axis.hours().max(1) * HOUR_MS) as f64;
    let point_ms = (axis_ms / strip_w.max(1.0) as f64) as i64;
    let mid = STRIP_TOP + (STRIP_H - BAR_H) / 2.0;
    let mut e = el().key("strip").w(strip_w).h(ROW_H).child(el().key("strip-bg").absolute(0.0, STRIP_TOP).w(strip_w).h(STRIP_H).radius(4.0).fill(p.track));
    // The hour lines, every third hour.
    for h in (w.axis.from_h..=w.axis.to_h).filter(|h| h % 3 == 0) {
        let x = (h - w.axis.from_h) as f32 / w.axis.hours().max(1) as f32 * strip_w;
        if x > 0.5 && x < strip_w - 0.5 {
            e = e.child(el().absolute(x, STRIP_TOP).w(1.0).h(STRIP_H).fill(p.hour));
        }
    }
    for (i, (x, wd)) in clipped(r.sittings.iter().copied(), r.span, point_ms).into_iter().filter_map(|sp| span_x(w, date, sp, strip_w)).enumerate() {
        e = e.child(el().key(format!("present-{i}")).absolute(x, STRIP_TOP).w(wd).h(STRIP_H).fill(p.you));
    }
    let yellow = clipped(yellow_of(logs(s), r).into_iter(), r.span, point_ms);
    for (i, (x, wd)) in yellow.into_iter().filter_map(|sp| span_x(w, date, sp, strip_w)).enumerate() {
        e = e.child(el().key(format!("wait-{i}")).absolute(x, mid).w(wd).h(BAR_H).radius(2.0).fill(p.wait).border(1.0, p.wait_edge));
    }
    for (i, (x, wd)) in clipped(r.work.iter().copied(), r.span, point_ms).into_iter().filter_map(|sp| span_x(w, date, sp, strip_w)).enumerate() {
        e = e.child(el().key(format!("work-{i}")).absolute(x, mid).w(wd).h(BAR_H).radius(2.0).fill(p.work));
    }
    // The time after now, shaded, and the now line.
    if today {
        let x = w.axis.x(date, w.window.to_ms, offset()).clamp(0.0, 1.0) as f32 * strip_w;
        e = e.child(el().key("later").absolute(x, STRIP_TOP).w((strip_w - x).max(0.0)).h(STRIP_H).fill(p.later));
        e = e.child(el().key("now").absolute(x - 0.75, STRIP_TOP - 3.0).w(1.5).h(STRIP_H + 6.0).fill(p.now));
        // The time of now in a pill beside the line, inside the strip, as Day's now label.
        let label = local_time(w.window.to_ms, "%H:%M");
        let pill_w = 40.0;
        let px = if x + 4.0 + pill_w <= strip_w { x + 4.0 } else { (x - 4.0 - pill_w).max(0.0) };
        e = e.child(
            row()
                .key("now-label")
                .absolute(px, STRIP_TOP + (STRIP_H - 16.0) / 2.0)
                .w(pill_w)
                .h(16.0)
                .center()
                .radius(4.0)
                .fill(p.now)
                .child(text(label).font(pal::SMALL_STRONG).color(p.on_accent).single_line()),
        );
    }
    // The edge marks of a day longer than the axis.
    let mark = |key: &'static str, x: f32, t: String| el().key(key).absolute(x, STRIP_TOP).w(4.0).h(STRIP_H).radius(1.0).fill(p.ink2).role("note").label(t);
    if let Some(t) = r.from_text(offset()) {
        e = e.child(mark("mark-from", 0.0, t));
    }
    if let Some(t) = r.until_text(offset()) {
        e = e.child(mark("mark-until", strip_w - 4.0, t));
    }
    e = e.child(band(w, r, strip_w, p));
    // The ticks last, over everything, each opening its session on `Cache`.
    for (i, t) in r.ticks.iter().enumerate() {
        let x = w.axis.x(date, t.ts_ms, offset());
        if !(0.0..=1.0).contains(&x) {
            continue;
        }
        let x = x as f32 * strip_w;
        let session = t.session.clone();
        e = e.child(
            el().key(format!("tick-{i}"))
                .absolute(x - TICK_HIT / 2.0, STRIP_TOP - 3.0)
                .w(TICK_HIT)
                .h(STRIP_H + 6.0)
                .role("button")
                .label(format!("cold return {}", t.session.chars().take(6).collect::<String>()))
                .on_press_tx(move |s: &mut Desk, tx| open_tick(s, date, &session, tx))
                .child(el().absolute((TICK_HIT - TICK_W) / 2.0, 0.0).w(TICK_W).h(STRIP_H + 6.0).radius(1.0).fill(p.cold)),
        );
    }
    e.on_pointer_move_local(move |s: &mut Desk, at, _| s.week.hover = Some((date, at.x))).on_hover_change(move |s: &mut Desk, inside, _| {
        if !inside && s.week.hover.is_some_and(|h| h.0 == date) {
            s.week.hover = None;
        }
    })
}

/// The agents band under a strip: a stepped area as high in each stretch as the most sessions at
/// work in it, the week's highest number or 4 its full height.
fn band(w: &Week, r: &Row, strip_w: f32, p: &Pal) -> El {
    let top = w.band_top.max(1) as f32;
    let y0 = STRIP_TOP + STRIP_H + BAND_GAP;
    let foot = y0 + BAND_H;
    let mut shape = Shape::new();
    // The most at work in each point of the strip.
    let n = strip_w.ceil().max(1.0) as usize;
    let mut most = vec![0u32; n];
    for (i, &(ts, c)) in r.band.iter().enumerate() {
        let end = r.band.get(i + 1).map_or(ts, |s| s.0);
        if c == 0 || end <= ts {
            continue;
        }
        let Some((x, wd)) = span_x(w, r.date, (ts, end), strip_w) else { continue };
        let (from, to) = (x.floor() as usize, ((x + wd).ceil() as usize).min(n));
        for m in most.iter_mut().take(to).skip(from) {
            *m = (*m).max(c);
        }
    }
    let mut k = 0;
    while k < n {
        let c = most[k];
        let mut j = k + 1;
        while j < n && most[j] == c {
            j += 1;
        }
        if c > 0 {
            let y = foot - BAND_H * (c as f32 / top).min(1.0);
            shape = shape.fill(vec![[k as f32, y - y0], [j as f32, y - y0], [j as f32, BAND_H], [k as f32, BAND_H]], p.work.alpha(0.85));
        }
        k = j;
    }
    let base = el().absolute(0.0, foot - 1.0).w(strip_w).h(1.0).fill(p.row_line);
    el().key("band").absolute(0.0, 0.0).w(strip_w).h(ROW_H).child(base).child(canvas(shape).key("band-area").absolute(0.0, y0).w(strip_w).h(BAND_H))
}

/// The moment under the pointer, from its place on the strip of `date`.
fn pointer_ts(w: &Week, date: i64, x: f32, strip_w: f32) -> i64 {
    let hours = w.axis.from_h as f64 + (x / strip_w.max(1.0)).clamp(0.0, 1.0) as f64 * w.axis.hours() as f64;
    let local = date * DAY_MS + (hours * HOUR_MS as f64).round() as i64;
    local - rhythm::local_offset(local - rhythm::local_offset(local))
}

/// The line under the days about the moment under the pointer: `Tue 3 Nov · 16:36 · 5 agents at
/// work · you were there`, the waits and a cold return near it; the date and the time alone when
/// nothing is there.
pub fn pointer_text(s: &Desk, w: &Week, strip_w: f32) -> String {
    let Some((date, x)) = s.week.hover else { return String::new() };
    let Some(r) = w.rows.iter().find(|r| r.date == date && r.kind == RowKind::Day) else { return String::new() };
    let ts = pointer_ts(w, date, x, strip_w);
    let noon = rhythm::midnight(date, offset()) + 12 * HOUR_MS;
    let mut out = format!("{} · {}", local_time(noon, "%a %-d %b"), local_time(ts, "%H:%M"));
    let inside = ts >= r.span.0 && ts < r.span.1;
    if inside {
        let at = r.band.iter().rev().find(|b| b.0 <= ts).map_or(0, |b| b.1);
        if at > 0 {
            out += &format!(" · {} at work", count(at as u64, "agent", "agents"));
        }
        if r.sittings.iter().any(|&(a, b)| a <= ts && ts < b.max(a + 60_000)) {
            out += " · you were there";
        }
        if let Some(l) = logs(s) {
            let n = l.waiting_at(ts).len();
            if n > 0 {
                out += &format!(" · waiting for your answer: {}", count(n as u64, "session", "sessions"));
            }
        }
    }
    // A cold return within a few points of the pointer.
    let near = (TICK_HIT as f64 / strip_w.max(1.0) as f64 * (w.axis.hours() * HOUR_MS) as f64) as i64;
    if let Some(t) = r.ticks.iter().filter(|t| (t.ts_ms - ts).abs() <= near).min_by_key(|t| (t.ts_ms - ts).abs()) {
        out += &format!(" · cold return: {}", session_label(s, &t.session));
    }
    if x <= 6.0
        && let Some(t) = r.from_text(offset())
    {
        out += &format!(" · {t}");
    }
    if x >= strip_w - 6.0
        && let Some(t) = r.until_text(offset())
    {
        out += &format!(" · {t}");
    }
    out
}

/// A session as the list names it: its id's first 6 characters and its title, cut.
fn session_label(s: &Desk, session: &str) -> String {
    let id: String = session.chars().take(6).collect();
    let title = s.browser.row_of(session).and_then(|r| s.browser.cells(r, now_ms()).title.map(str::to_string));
    match title {
        Some(t) => format!("{id} {}", clip_end(&t, 16)),
        None => id,
    }
}

/// What the strips' colors are.
fn legend(p: &Pal) -> El {
    let swatch = |c: Color| el().w(10.0).h(10.0).radius(R_MARK).fill(c);
    let item = |e: El, t: &str| row().gap(6.0).center_y().child(e).child(text(t.to_string()).font(pal::SMALL).color(p.ink2).single_line());
    row()
        .key("week-legend")
        .w_full()
        .flex_wrap()
        .gaps(4.0, 14.0)
        .pl(LABEL_W + 10.0 + CELL_GAP)
        .child(item(swatch(p.work), "agent working"))
        .child(item(swatch(p.wait), "waiting for your answer, no agent working"))
        .child(item(swatch(p.you), "you were there"))
        .child(item(el().w(2.0).h(12.0).radius(1.0).fill(p.cold), "cold return"))
        .child(item(el().w(10.0).h(6.0).radius(1.0).fill(p.work.alpha(0.85)), "agents at work"))
}

// ---------------------------------------------------------------------------------------------
// Totals and comparisons.

/// A change against the week before, in runs of their colors.
fn runs(key: &'static str, parts: Vec<(String, Color)>) -> El {
    let joined: String = parts.iter().map(|x| x.0.as_str()).collect();
    let mut spans = Vec::new();
    let mut at = 0;
    for (t, c) in &parts {
        spans.push(Span::new(at, at + t.len()).color(*c));
        at += t.len();
    }
    rich(joined, spans).key(key).w_full().font(pal::SMALL_NUM)
}

fn verdict_color(v: Verdict, p: &Pal) -> Color {
    match v {
        Verdict::Better => p.warm,
        Verdict::Worse => p.cold,
        Verdict::Neutral => p.ink2,
    }
}

/// A change of hours: `↑ 5.7h (+19%)`, `= 0h`.
pub fn hours_delta(now_ms: i64, was_ms: i64) -> String {
    let (now, was) = (now_ms as f64 / HOUR_MS as f64, was_ms as f64 / HOUR_MS as f64);
    let d = now - was;
    if d.abs() < 0.05 {
        return "= 0h".to_string();
    }
    let arrow = if d > 0.0 { "↑" } else { "↓" };
    let percent = if was.abs() > 1e-9 { format!(" ({}{:.0}%)", if d > 0.0 { "+" } else { "−" }, (100.0 * d / was).abs()) } else { String::new() };
    format!("{arrow} {:.1}h{percent}", d.abs())
}

/// What the tiles show of a number: its change against the week before, or its
/// pace per day, or nothing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Num {
    Spend,
    Agent,
    Keyboard,
    Avoidable,
    CacheHit,
    Cold,
}

/// The line under a number: its change against the week before in its colors, or its pace per
/// day when the week has no comparison.
fn under(w: &Week, n: Num, p: &Pal) -> Option<Vec<(String, Color)>> {
    let t = &w.totals;
    if let Some(b) = w.before.sums() {
        let money = |now: f64, was: f64, sense: Sense| (compare::delta(now, was, true), verdict_color(compare::verdict(now, was, sense, true), p));
        return Some(match n {
            Num::Spend => vec![money(t.usd, b.usd, Sense::Neutral)],
            Num::Agent => vec![(hours_delta(t.agent_minutes as i64 * 60_000, b.agent_minutes as i64 * 60_000), p.ink2)],
            Num::Keyboard => vec![(hours_delta(t.keyboard_ms(), b.keyboard_ms()), p.ink2)],
            Num::Avoidable => {
                let (now, was) = (100.0 * t.avoidable_share().unwrap_or(0.0), 100.0 * b.avoidable_share().unwrap_or(0.0));
                let share = verdict_color(compare::verdict((now * 10.0).round(), (was * 10.0).round(), Sense::LowerIsBetter, false), p);
                vec![money(t.avoidable_usd, b.avoidable_usd, Sense::LowerIsBetter), (" · ".into(), p.ink3), (compare::points(now, was), share)]
            }
            Num::CacheHit => {
                let (now, was) = (100.0 * t.cache_hit()?, 100.0 * b.cache_hit()?);
                // A higher hit is better.
                let v = compare::verdict(-(now * 10.0).round(), -(was * 10.0).round(), Sense::LowerIsBetter, false);
                vec![(compare::points(now, was), verdict_color(v, p))]
            }
            Num::Cold => vec![(
                compare::delta(t.cold as f64, b.cold as f64, false),
                verdict_color(compare::verdict(t.cold as f64, b.cold as f64, Sense::LowerIsBetter, false), p),
            )],
        });
    }
    let pace = match n {
        Num::Spend => w.pace_usd(t.usd)?,
        Num::Agent => format!("{} a day", hours((w.pace(t.agent_minutes as f64 * 60_000.0)?) as i64)),
        Num::Keyboard => format!("{} a day", hours(w.pace(t.keyboard_ms() as f64)? as i64)),
        Num::Avoidable => w.pace_usd(t.avoidable_usd)?,
        Num::Cold => format!("{:.1} a day", w.pace(t.cold as f64)?),
        // A share has no pace.
        Num::CacheHit => return None,
    };
    Some(vec![(pace, p.ink2)])
}

/// The notes of the comparison: what the week compares with, or why it does not.
pub fn before_note(w: &Week) -> String {
    w.before.note(offset())
}

/// The week's label in the footer of the days: `Week`, or `Mon–Thu so far` for a week in
/// progress.
pub fn total_label(w: &Week) -> String {
    if w.window.complete() {
        return "Week".to_string();
    }
    let today = rhythm::date_of(w.window.to_ms.max(w.window.from_ms), offset());
    if today == w.window.monday { format!("{} so far", weekday(today)) } else { format!("{}–{} so far", weekday(w.window.monday), weekday(today)) }
}

/// The footer of the days: the week's numbers over its window under the days' numbers, with no
/// changes; the tiles have them.
fn total_row(w: &Week, strip_w: f32, p: &Pal) -> El {
    let t = &w.totals;
    let cells = [
        (usd(t.usd), p.ink, "total-spend"),
        (hours(t.agent_minutes as i64 * 60_000), p.ink, "total-agent"),
        (hours(t.keyboard_ms()), p.ink, "total-keyboard"),
        (format!("{} cold", int(t.cold as u64)), if t.cold > 0 { p.cold } else { p.ink3 }, "total-cold"),
    ];
    let mut line = row()
        .key("week-totals")
        .w_full()
        .h(28.0)
        .gap(CELL_GAP)
        .center_y()
        .child(text(total_label(w)).key("total-label").w(LABEL_W + CELL_GAP + strip_w).pl(6.0).font(pal::STRONG).color(p.ink).single_line().truncate());
    for (i, (t, c, k)) in cells.into_iter().enumerate() {
        line = line.child(row().w(NUM_W[i]).justify_end().child(text(t).key(k).font(pal::STRONG_NUM).color(c).single_line()));
    }
    col().w_full().child(el().w_full().h(1.0).fill(p.row_line)).child(line)
}

/// The lines under the footer that the tiles do not hold: what the week compares with, the last
/// full week for a week in progress, and the line on rows and totals.
fn total_lines(w: &Week, p: &Pal) -> El {
    let line = |key: &'static str, t: String, font, c: Color| text(t).key(key).w_full().wrap().font(font).color(c);
    let mut c = col().key("week-total-lines").w_full().pl(LABEL_W + CELL_GAP).gap(3.0).child(line("total-note", before_note(w), pal::SMALL, p.ink3));
    if let Some(l) = &w.last_week {
        let t = format!(
            "last week {} · {} agent · {} at the keyboard · {}",
            usd(l.usd),
            hours(l.agent_minutes as i64 * 60_000),
            hours(l.keyboard_ms()),
            count(l.cold as u64, "cold return", "cold returns")
        );
        c = c.child(line("total-last", t, pal::SMALL_NUM, p.ink2));
    }
    if let Some(t) = w.gap.line(w.window.monday) {
        c = c.child(line("total-gap", t, pal::SMALL, p.ink2));
    }
    c
}

/// The text of a line that the week's logs give, while they are being read or when they are not.
fn read_state(s: &Desk, w: &Week) -> Result<(), String> {
    if w.totals.all_gone() {
        return Err(core_week::GONE_ALL.to_string());
    }
    match read(s).map(|r| &r.logs) {
        Some(Ok(_)) => Ok(()),
        Some(Err(e)) => Err(format!("error: {e}")),
        None => Err(reading_line(s)),
    }
}

/// `reading the week's logs… 12 of 162 logs`.
pub fn reading_line(s: &Desk) -> String {
    match s.week.data.progress() {
        Some((done, total)) if total > 0 => format!("reading the week's logs… {} of {} logs", int(done as u64), int(total as u64)),
        _ => "reading the week's logs…".to_string(),
    }
}

/// A rhythm line cut into its label and its text: `sittings  82, …` gives `sittings` and `82, …`.
fn split_line(line: &str) -> (String, String) {
    match line.split_once(' ') {
        Some((head, rest)) => (head.to_string(), rest.trim_start().to_string()),
        None => (line.to_string(), String::new()),
    }
}

/// The week's rhythm lines and the advice on cold returns, with the line on gone logs.
fn rhythm_lines(s: &Desk, w: &Week, p: &Pal) -> El {
    let t = &w.totals;
    let state = read_state(s, w);
    let l = logs(s).filter(|_| state.is_ok());
    let from_read = |line: Option<&String>, label: &str| -> (String, bool) {
        match (line, &state) {
            (Some(x), Ok(())) if !x.is_empty() => (split_line(x).1, false),
            (_, Ok(())) => (format!("no {label} in this week"), true),
            (_, Err(e)) => (e.clone(), true),
        }
    };
    let mut rows: Vec<(&'static str, String, bool)> = Vec::new();
    rows.push(("sittings", if t.sittings.is_empty() { "nobody typed this week".into() } else { split_line(&t.sittings_line()).1 }, false));
    rows.push(("agents", split_line(&t.agents_line(l.and_then(|l| l.peak), offset())).1, false));
    let (answers, quiet) = from_read(l.map(|l| &l.answers), "answers");
    rows.push(("answers", answers, quiet));
    let (queue, quiet) = from_read(l.map(|l| &l.queue), "queue");
    rows.push(("queue", queue, quiet));
    rows.push(("night", split_line(&t.night_line()).1, false));
    let cache = match l {
        Some(l) => split_line(&l.cache).1,
        None => format!("{}; {}", count(t.cold as u64, "cold return", "cold returns"), state.clone().err().unwrap_or_default()),
    };
    rows.push(("cache", cache, false));
    let advice = l.and_then(|l| l.advice.iter().find(|a| a.contains("after the cache expired")).cloned());
    let mut c = col().key("week-rhythm").w_full().padding(12.0).gap(4.0).radius(R_GROUP).fill(p.bar);
    for (name, body, quiet) in rows {
        let body_el = if name == "cache" {
            let mut spans = Vec::new();
            let mut at = 0;
            for (run, hot) in crate::day::cold_runs(&body) {
                if hot {
                    spans.push(Span::new(at, at + run.len()).color(p.cold).weight(650.0));
                }
                at += run.len();
            }
            rich(body.clone(), spans).key("line-text").grow(1.0).min_w(0.0).font(pal::BODY).color(p.ink2).selectable(true)
        } else {
            text(body).key("line-text").grow(1.0).min_w(0.0).wrap().font(pal::BODY).color(if quiet { p.ink3 } else { p.ink2 }).selectable(true)
        };
        c = c.child(
            row()
                .key(format!("line-{name}"))
                .w_full()
                .gap(12.0)
                .items_start()
                .child(text(name).key("line-label").w(64.0).font(pal::STRONG).color(p.ink).single_line())
                .child(body_el),
        );
    }
    if let Some(a) = advice {
        c = c.child(
            row()
                .key("line-advice")
                .w_full()
                .gap(12.0)
                .items_start()
                .child(text("advice").key("line-label").w(64.0).font(pal::STRONG).color(p.ink).single_line())
                .child(text(format!("▸ {a}")).key("line-text").grow(1.0).min_w(0.0).wrap().font(pal::BODY).color(p.ink).selectable(true)),
        );
    }
    if let Some(g) = t.gone_line().filter(|_| !t.all_gone()) {
        c = c.child(text(g).key("line-gone").w_full().wrap().pt(4.0).font(pal::SMALL).color(p.ink3));
    }
    if let Some(l) = l {
        let took = read(s).map_or(0.0, |r| r.took.as_secs_f64());
        let line = format!("read {} in {took:.1} s", count(l.files as u64, "log", "logs"));
        c = c.child(text(line).key("line-read").w_full().pt(4.0).font(pal::SMALL).color(p.ink3).single_line());
    }
    c
}

// ---------------------------------------------------------------------------------------------
// The column.

/// A card of the column: its title and an aside over a soft group, ringed while its part has
/// the keyboard.
fn card(key: &'static str, title: &str, aside: Option<String>, content: El, on: bool, p: &Pal) -> El {
    let group = col().key(key).w_full().min_w(0.0).padding(12.0).gap(6.0).radius(R_GROUP).fill(p.bar).child(content);
    let group = overlay::ring(group, on, p);
    let mut top = row().w_full().gap(8.0).center_y().child(text(title.to_string()).font(pal::HEAD).color(if on { p.accent } else { p.ink2 }).single_line());
    if let Some(a) = aside {
        top = top.child(spacer().grow(1.0)).child(text(a).font(pal::SMALL).color(p.ink3).single_line().truncate().shrink().min_w(0.0));
    }
    col().w_full().gap(8.0).child(top).child(group)
}

fn column(s: &Desk, p: &Pal) -> Vec<El> {
    let Some(w) = week(s) else { return Vec::new() };
    vec![
        tiles(s, w, p),
        card(Part::Changes.key(), "What to change", None, what_to_change(s, w, p), s.week.part == Part::Changes, p),
        card("week-next", "Next week", None, next_week(&w.totals, p), false, p),
        card(Part::Projects.key(), "By project", Some("agent hours, each minute once".into()), projects(s, w, p), s.week.part == Part::Projects, p),
    ]
}

/// A tile: what it is, the number with a smaller part beside it, and the line under it.
fn tile(key: &'static str, label: &str, value: String, small: Option<String>, color: Color, under: Option<Vec<(String, Color)>>, p: &Pal) -> El {
    let mut v = row().gap(5.0).items_end().child(text(value).key("value").font(pal::CARD).color(color).single_line());
    if let Some(sm) = small {
        v = v.child(text(sm).key("small").font(pal::SMALL_STRONG).color(p.ink2).single_line().pb(2.0));
    }
    let mut c = col()
        .key(key)
        .grow(1.0)
        .min_w(0.0)
        .px(10.0)
        .py(8.0)
        .gap(2.0)
        .radius(R_TILE)
        .fill(p.panel)
        .child(text(label.to_string()).font(pal::SMALL).color(p.ink2).single_line().truncate().min_w(0.0));
    c = c.child(v);
    if let Some(parts) = under {
        c = c.child(runs("change", parts));
    }
    c
}

/// The six tiles, two in a row, and the note of the comparison.
fn tiles(s: &Desk, w: &Week, p: &Pal) -> El {
    let t = &w.totals;
    let premium = logs(s).filter(|_| !t.all_gone()).map(|l| l.returns.iter().map(|r| r.premium_usd).sum::<f64>());
    let reading = read(s).is_none() && !t.all_gone();
    let cold_small = match premium {
        Some(v) => Some(usd(v)),
        None if reading => Some("…".to_string()),
        None => None,
    };
    let all = [
        tile("tile-spend", "Spend", usd(t.usd), None, p.ink, under(w, Num::Spend, p), p),
        tile("tile-agent", "Agent hours", hours(t.agent_minutes as i64 * 60_000), None, p.ink, under(w, Num::Agent, p), p),
        tile("tile-keyboard", "At the keyboard", hours(t.keyboard_ms()), None, p.ink, under(w, Num::Keyboard, p), p),
        tile(
            "tile-avoidable",
            "Avoidable on misses",
            usd(t.avoidable_usd),
            t.avoidable_share().map(|x| format!("{:.1}%", 100.0 * x)),
            p.cold,
            under(w, Num::Avoidable, p),
            p,
        ),
        tile(
            "tile-hit",
            "Cache hit, prompt tokens",
            t.cache_hit().map_or("–".into(), |h| format!("{:.1}%", 100.0 * h)),
            None,
            p.ink,
            under(w, Num::CacheHit, p),
            p,
        ),
        tile("tile-cold", "Cold returns", int(t.cold as u64), cold_small, if t.cold > 0 { p.cold } else { p.ink }, under(w, Num::Cold, p), p),
    ];
    let mut out = col().key("week-tiles").w_full().gap(8.0);
    let mut it = all.into_iter();
    while let (Some(a), b) = (it.next(), it.next()) {
        out = out.child(row().w_full().gap(8.0).items_stretch().child(a).children(b));
    }
    out.child(text(before_note(w)).key("tiles-note").w_full().wrap().font(pal::SMALL).color(p.ink3))
}

/// What to change: the three costliest savings and habits with their advice and sessions.
fn what_to_change(s: &Desk, w: &Week, p: &Pal) -> El {
    let c = match (read_state(s, w), changes(s)) {
        (Ok(()), Some(c)) => c,
        (Err(e), _) => return text(e).key("changes-state").w_full().wrap().font(pal::BODY).color(p.ink2),
        _ => return text(reading_line(s)).key("changes-state").w_full().wrap().font(pal::BODY).color(p.ink2),
    };
    let lines = lines(c, s.week.open.as_ref());
    let on = s.week.part == Part::Changes;
    let mut out = col().w_full().gap(2.0);
    let mut at = 0;
    for (sec, title, aside, empty) in [
        (Section::Saved, "Could have been saved", None, "No cache misses that could have been saved."),
        (Section::Habits, "Habits that cost the most", Some("the rows overlap"), "No habits to show."),
    ] {
        let mut head = row()
            .w_full()
            .pt(if sec == Section::Habits { 8.0 } else { 0.0 })
            .gap(8.0)
            .center_y()
            .child(text(title).font(pal::SMALL_STRONG).color(p.ink2).single_line());
        if let Some(a) = aside {
            head = head.child(spacer().grow(1.0)).child(text(a).font(pal::SMALL).color(p.ink3).single_line());
        }
        out = out.child(head);
        let rows = shown_rows(c, sec);
        if rows.is_empty() {
            out = out.child(text(empty).font(pal::SMALL).color(p.ink3).single_line().px(6.0));
        }
        let first = rows.first().map_or(0.0, |r| r.usd).max(1e-9);
        for r in rows {
            let ringed = on && s.week.row == at;
            out = out.child(change_row(c, sec, r, first, at, ringed, &s.week.open, p));
            at += 1;
            if s.week.open.as_ref().is_some_and(|(x, n)| *x == sec && *n == r.name) {
                for (i, x) in c.sessions_of(&r.name).unwrap_or(&[]).iter().take(SESSIONS).enumerate() {
                    let ringed = on && s.week.row == at;
                    out = out.child(session_line(s, at, x, ringed, p));
                    let _ = i;
                    at += 1;
                }
            }
        }
    }
    debug_assert_eq!(at, lines.len());
    out
}

#[allow(clippy::too_many_arguments)]
fn change_row(c: &Changes, sec: Section, r: &tokprof_core::pool::Row, first: f64, at: usize, ringed: bool, open: &Option<(Section, String)>, p: &Pal) -> El {
    let split = c.sessions_of(&r.name).is_some_and(|l| !l.is_empty());
    let is_open = open.as_ref().is_some_and(|(x, n)| *x == sec && *n == r.name);
    let chevron = match (split, is_open) {
        (false, _) => " ",
        (true, true) => "▾",
        (true, false) => "▸",
    };
    let name = r.name.clone();
    let line = row()
        .w_full()
        .h(24.0)
        .px(4.0)
        .gap(6.0)
        .center_y()
        .child(row().w(10.0).child(text(chevron).font(pal::SMALL).color(p.ink3).single_line()))
        .child(text(r.name.clone()).key("label").font(pal::BODY).color(p.ink).single_line().truncate().grow(1.0).shrink().min_w(0.0))
        .child(row().w(48.0).child(overlay::meter(r.usd / first, p.accent.alpha(0.75), p, 5.0)))
        .child(row().w(40.0).justify_end().child(text(format!("{:.1}%", 100.0 * r.share)).key("share").font(pal::SMALL_NUM).color(p.ink2).single_line()))
        .child(row().w(54.0).justify_end().child(text(usd(r.usd)).key("amount").font(pal::STRONG_NUM).color(p.accent).single_line()));
    let line = line.key(format!("wchange-{at}")).radius(R_ROW).role("button").label(r.name.clone());
    let line = if ringed { line.fill(p.sel) } else { line.hover(p.hover) };
    let line = line.on_press(move |s: &mut Desk| {
        toggle(s, sec, &name);
    });
    let mut out = col().w_full().gap(1.0).child(line);
    if let Some(h) = c.hint(&r.name).filter(|_| !r.name.starts_with("tool hint: ")) {
        out = out
            .child(row().w_full().pl(20.0).pr(4.0).pb(3.0).child(text(h.to_string()).key("hint").grow(1.0).min_w(0.0).wrap().font(pal::SMALL).color(p.ink3)));
    }
    out
}

/// A session behind a row: its title, or its agent and id, and its dollars; a click opens it in
/// Day.
fn session_line(s: &Desk, at: usize, x: &SessionCost, ringed: bool, p: &Pal) -> El {
    let b = &s.browser;
    let listed = b.row_of_short(&x.agent, &x.session);
    let title = match listed {
        Some(row) => {
            let cells = b.cells(row, now_ms());
            cells.title.map_or_else(|| format!("{} {}", cells.agent, x.session), str::to_string)
        }
        None => format!("{} {} · not in the list", x.agent.name(), x.session),
    };
    let target = x.clone();
    let line = row()
        .key(format!("wchange-{at}"))
        .w_full()
        .h(22.0)
        .pl(20.0)
        .pr(4.0)
        .gap(8.0)
        .center_y()
        .radius(R_ROW)
        .child(el().w(5.0).h(5.0).radius(R_MARK).fill(p.accent.alpha(0.6)))
        .child(
            text(title)
                .key("session-title")
                .font(pal::SMALL)
                .color(if listed.is_some() { p.ink } else { p.ink3 })
                .single_line()
                .truncate()
                .grow(1.0)
                .shrink()
                .min_w(0.0),
        )
        .child(row().w(54.0).justify_end().child(text(usd(x.usd)).font(pal::SMALL_NUM).color(p.ink).single_line()))
        .role("button")
        .label(x.session.clone())
        .on_press_tx(move |s: &mut Desk, tx| crate::changes::open_session(s, &target, tx));
    if ringed { line.fill(p.sel) } else { line.hover(p.hover) }
}

/// The line for next week, and the other avoidable causes under it.
fn next_week(t: &Sums, p: &Pal) -> El {
    let lines = t.next_week();
    let mut c = col().w_full().gap(4.0);
    for (i, l) in lines.into_iter().enumerate() {
        let (font, color) = if i == 0 { (pal::STRONG, p.ink) } else { (pal::SMALL, p.ink2) };
        c = c.child(text(l).key(if i == 0 { "next-line" } else { "next-other" }).w_full().wrap().font(font).color(color).selectable(true));
    }
    c
}

/// By project: the six with the most spend, a bar as long as its share of the first, its
/// dollars, share and agent hours, and the rest as one row.
fn projects(s: &Desk, w: &Week, p: &Pal) -> El {
    let (shown, more) = w.shown_projects();
    if shown.is_empty() {
        return text("No spend this week.").key("projects-none").font(pal::BODY).color(p.ink2).single_line();
    }
    let on = s.week.part == Part::Projects;
    let first = shown[0].usd.max(1e-9);
    let names = crate::view::ProjectNames::new(w.projects.iter().map(|x| x.name.as_str()));
    let mut out = col().w_full().gap(2.0);
    for (i, x) in shown.iter().enumerate() {
        let name = x.name.clone();
        let ringed = on && s.week.row == i;
        let line = row()
            .key(format!("wproj-{i}"))
            .w_full()
            .h(26.0)
            .px(4.0)
            .gap(6.0)
            .center_y()
            .radius(R_ROW)
            .child(text(names.of(&x.name)).key("label").font(pal::BODY).color(p.ink).single_line().truncate().w(108.0))
            .child(overlay::meter(x.usd / first, p.bars, p, 5.0))
            .child(row().w(50.0).justify_end().child(text(usd(x.usd)).key("amount").font(pal::STRONG_NUM).color(p.ink).single_line()))
            .child(row().w(34.0).justify_end().child(text(format!("{:.0}%", 100.0 * x.share)).key("share").font(pal::SMALL_NUM).color(p.ink2).single_line()))
            .child(
                row().w(40.0).justify_end().child(text(hours(x.agent_minutes as i64 * 60_000)).key("hours").font(pal::SMALL_NUM).color(p.ink2).single_line()),
            )
            .role("button")
            .label(x.name.clone())
            .on_press_tx(move |s: &mut Desk, tx| filter_project(s, &name, tx));
        out = out.child(if ringed { line.fill(p.sel) } else { line.hover(p.hover) });
    }
    if more > 0 {
        let ringed = on && s.week.row == shown.len();
        let line = row()
            .key(format!("wproj-{}", shown.len()))
            .w_full()
            .h(24.0)
            .px(4.0)
            .center_y()
            .radius(R_ROW)
            .child(text(format!("{} more", int(more as u64))).key("label").font(pal::SMALL).color(p.ink3).single_line());
        out = out.child(if ringed { line.fill(p.sel) } else { line });
    }
    out
}

/// The scene: Week on the week of `date`, or this week, with the store and the week's logs
/// waited for.
pub fn scene(s: &mut Desk, date: Option<i64>) {
    show(s, Mode::Week);
    if let Some(d) = date {
        let m = core_week::monday_of(d);
        s.week.monday = (m != this_monday(s)).then_some(m);
    }
    let changes = s.browser.changes();
    s.calendar.data.settle(changes, std::time::Duration::from_secs(120));
    let store = s.calendar.data.get().and_then(|c| c.store.clone());
    let key = key(s);
    s.week.data.settle(&key, store.as_ref(), changes, std::time::Duration::from_secs(120));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hours_read_as_the_rows_count_them() {
        assert_eq!(hours(0), "0h");
        assert_eq!(hours(13 * 60_000), "13m");
        assert_eq!(hours(45 * HOUR_MS / 2), "22.5h");
        assert_eq!(hours_delta(36 * HOUR_MS, 30 * HOUR_MS), "↑ 6.0h (+20%)");
        assert_eq!(hours_delta(HOUR_MS, HOUR_MS), "= 0h");
    }

    #[test]
    fn tab_goes_through_the_parts_and_back() {
        assert_eq!(Part::Days.step(false), Part::Changes);
        assert_eq!(Part::Projects.step(false), Part::Days);
        assert_eq!(Part::Days.step(true), Part::Projects);
    }

    #[test]
    fn a_strip_at_960_points_keeps_the_column() {
        let (days, strip) = layout(960.0);
        assert_eq!(days, 960.0 - 2.0 * PAD - GAP - COLUMN_W);
        assert!(strip >= 230.0, "{strip}");
        let (_, wide) = layout(1500.0);
        assert!(wide >= 770.0, "{wide}");
    }
}
