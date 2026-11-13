//! Month and Quarter: one calendar month or quarter in place of the lanes, the list and the
//! inspector, in one scrolling pane of cards (desk-window, Report cards). The key numbers, the
//! days of the month, the weeks of the quarter, by project and the records come from the day
//! store at once (see `tokprof_browse::period`); by origin, by model, the pause tables, what to
//! change and the largest prompt read the period's logs in the background, as `tokprof summary`,
//! `tokprof pauses` and `tokprof share` count them (see `tokprof_browse::reports`). The charts of
//! the two periods are in `charts`.

use std::sync::Arc;

use rune::{Color, Cx, El, Key as RuneKey, KeyEvent, ScrollSpec, Span, TextStyle, Tx, col, el, rich, row, spacer, text};
use tokprof_browse::browser::{Key as BrowserKey, now_ms};
use tokprof_browse::period::{Key, Periods};
use tokprof_browse::reports::{Progress, Report, Reports};
use tokprof_core::analysis::Component;
use tokprof_core::fmt::{count, int, local_time, tokens, usd};
use tokprof_core::month::Ym;
use tokprof_core::overview::Overview;
use tokprof_core::pauses::AgentPauses;
use tokprof_core::period::{Period, PeriodSums, Yq};
use tokprof_core::rhythm::{self, Offset};
use tokprof_core::savings::Changes;

use crate::overlay::{self, PILL, R_GROUP, R_MARK, R_ROW, R_TILE};
use crate::pal::{self, Pal};
use crate::view;
use crate::{Desk, plain};

/// The key of the pane's scroll viewport.
pub const PANE: &str = "reports";
/// The rows of the project and origin cards before the rest.
pub const TOP: usize = 10;
/// The room around the pane's content, and between its cards.
pub const PAD: f32 = 20.0;
pub const GAP: f32 = 16.0;
/// Below this width a row of cards holds two of them.
pub const WIDE: f32 = 1180.0;
/// The number of a key number.
const VALUE: TextStyle = TextStyle::new(22.0, 700.0).tracking(-0.3).features("'tnum' 1");
const HEADLINE: TextStyle = TextStyle::new(20.0, 700.0).tracking(-0.3);
const RECORD: TextStyle = TextStyle::new(17.0, 700.0).tracking(-0.2).features("'tnum' 1");

/// What the window shows under the bar.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Mode {
    Day,
    /// A calendar week (see `week`).
    Week,
    /// A calendar month and a calendar quarter, in this pane.
    Month,
    Quarter,
    Calendar,
    /// The Sessions screen (see `sessions`), apart from the switch.
    Sessions,
}

impl Mode {
    /// Month or Quarter, the views of this pane.
    pub fn period(self) -> bool {
        matches!(self, Mode::Month | Mode::Quarter)
    }
}

/// The cards that take the keyboard.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Part {
    /// Month: the days of the month, and spend by day, each with a day cursor.
    Days,
    Chart,
    /// Quarter: the weeks of the quarter with its trends, with a week cursor; its months and its
    /// project shifts, with a row cursor.
    Weeks,
    Months,
    Shifts,
    Projects,
    Origins,
    Models,
    Returns,
    Sizes,
    Saved,
    Ways,
    Habits,
    Records,
}

/// The order Tab moves through Month's cards and Quarter's: the cards with days, weeks or rows,
/// then the savings, the ways back and the habits, then the records.
pub const MONTH_PARTS: [Part; 11] =
    [Part::Days, Part::Chart, Part::Projects, Part::Origins, Part::Models, Part::Returns, Part::Sizes, Part::Saved, Part::Ways, Part::Habits, Part::Records];
pub const QUARTER_PARTS: [Part; 9] =
    [Part::Weeks, Part::Months, Part::Shifts, Part::Projects, Part::Models, Part::Saved, Part::Ways, Part::Habits, Part::Records];

impl Part {
    pub fn key(self) -> &'static str {
        match self {
            Part::Days => "rep-days",
            Part::Chart => "rep-chart",
            Part::Weeks => "rep-weeks",
            Part::Months => "rep-months",
            Part::Shifts => "rep-shifts",
            Part::Projects => "rep-projects",
            Part::Origins => "rep-origins",
            Part::Models => "rep-models",
            Part::Returns => "rep-returns",
            Part::Sizes => "rep-sizes",
            Part::Saved => "rep-saved",
            Part::Ways => "rep-ways",
            Part::Habits => "rep-habits",
            Part::Records => "rep-records",
        }
    }
}

/// The part after `at` in `order`, or before it; from the page, the first or the last.
pub fn step_part(order: &[Part], at: Option<Part>, back: bool) -> Part {
    let n = order.len();
    match at.and_then(|p| order.iter().position(|&x| x == p)) {
        Some(i) => order[if back { (i + n - 1) % n } else { (i + 1) % n }],
        None if back => order[n - 1],
        None => order[0],
    }
}

/// What the pointer rests over.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Hover {
    /// A date of the days of the month, or of spend by day.
    Cell(i64),
    Bar(i64),
    /// A week of the quarter, by its place.
    Week(usize),
    /// A row of by model.
    Model(usize),
}

/// The view's state: the period shown, the card with the keyboard and its cursors, the pointer,
/// and the periods computed from the store and read from the logs.
pub struct State {
    pub mode: Mode,
    /// The month and the quarter shown; `None` for the current one.
    pub month: Option<Ym>,
    pub quarter: Option<Yq>,
    /// The card with the keyboard; none while the page has it.
    pub part: Option<Part>,
    /// The day cursor of the days and of spend by day, the week cursor of the weeks, and the row
    /// cursor of a card with rows.
    pub day: Option<i64>,
    pub week: Option<usize>,
    pub row: usize,
    pub hover: Option<Hover>,
    /// The row of what to change whose sessions are open under it.
    pub open: Option<(Part, String)>,
    /// What the period's logs gave, and its sums from the store.
    pub data: Reports,
    pub sums: Periods,
    /// The filter text the period is counted with: the window's, once it is not being typed.
    pub text: String,
    /// The window filters by client, which the store cannot.
    pub client_ignored: bool,
    /// A clock for tests, in place of now.
    pub clock: Option<i64>,
    /// An element to scroll into view at the next poll, as a frame's scene asks.
    pub reveal: Option<String>,
    /// The log read's and the store's progress as the last poll saw them.
    seen: (Option<Progress>, Option<(usize, usize)>, bool),
}

impl State {
    pub fn new(data: Reports, sums: Periods) -> State {
        State {
            mode: Mode::Day,
            month: None,
            quarter: None,
            part: None,
            day: None,
            week: None,
            row: 0,
            hover: None,
            open: None,
            data,
            sums,
            text: String::new(),
            client_ignored: false,
            clock: None,
            reveal: None,
            seen: (None, None, false),
        }
    }

    /// Now, or the tests' clock.
    pub fn now(&self) -> i64 {
        self.clock.unwrap_or_else(now_ms)
    }

    /// The current month and quarter.
    pub fn this_month(&self) -> Ym {
        Ym::of_date(rhythm::date_of(self.now(), offset()))
    }

    pub fn this_quarter(&self) -> Yq {
        Yq::of(self.this_month())
    }

    /// The month Month shows, and the quarter Quarter shows.
    pub fn shown_month(&self) -> Ym {
        self.month.unwrap_or_else(|| self.this_month())
    }

    pub fn shown_quarter(&self) -> Yq {
        self.quarter.unwrap_or_else(|| self.this_quarter())
    }

    /// The period shown: Quarter's in Quarter, else Month's.
    pub fn period(&self) -> Period {
        if self.mode == Mode::Quarter { Period::Quarter(self.shown_quarter()) } else { Period::Month(self.shown_month()) }
    }

    /// What the period shown is kept by: the period and the filter text.
    pub fn key(&self) -> Key {
        Key::new(self.period(), &self.text)
    }

    /// The store's sums of the period shown, once computed.
    pub fn sums(&self) -> Option<&Arc<PeriodSums>> {
        self.sums.get(&self.key())
    }

    /// The read of the period shown's logs, once it arrived.
    pub fn report(&self) -> Option<&Report> {
        self.data.get(&self.key())
    }

    /// What the period's logs gave, once read without an error.
    pub fn overview(&self) -> Option<&Overview> {
        self.report().and_then(|r| r.overview.as_deref().ok())
    }

    /// What to change in the period shown, once read.
    pub fn changes(&self) -> Option<&Changes> {
        self.overview().map(|o| &o.changes)
    }

    /// The order Tab moves through the cards.
    pub fn parts(&self) -> &'static [Part] {
        if self.mode == Mode::Quarter { &QUARTER_PARTS } else { &MONTH_PARTS }
    }
}

pub fn offset() -> Offset<'static> {
    &rhythm::local_offset
}

/// Takes in what the workers sent while Month or Quarter is shown: the store from the calendar's
/// worker, the period's sums and its log read, and the images of the cards; asks for what is
/// missing or out of date, and wakes the window when one is due. Returns whether the view
/// changed. While neither is shown it does nothing.
pub fn poll(s: &mut Desk, tx: &mut Tx) -> bool {
    if !s.reports.mode.period() {
        return false;
    }
    // The filter counts once it is not being typed.
    let mut changed = false;
    if !s.browser.editing() && s.reports.text != s.browser.filter() {
        s.reports.text = s.browser.filter().to_string();
        s.reports.row = 0;
        s.reports.open = None;
        changed = true;
    }
    let changes = s.browser.changes();
    let wake =
        |tx: &mut Tx, at: std::time::Instant| tx.wake_after(at.saturating_duration_since(std::time::Instant::now()).max(std::time::Duration::from_millis(1)));
    let stored = s.calendar.data.tick(changes);
    if let Some(at) = s.calendar.data.next_due(changes) {
        wake(tx, at);
    }
    let store = s.calendar.data.get().and_then(|c| c.store.clone());
    let key = s.reports.key();
    let summed = s.reports.sums.tick(&key, store.as_ref());
    let arrived = s.reports.data.tick(&key, changes);
    if let Some(at) = s.reports.data.next_due(&key, changes) {
        wake(tx, at);
    }
    if let Some(key) = s.reports.reveal.take() {
        tx.scroll_into_view(key);
        changed = true;
    }
    let review = review_months(s);
    let images = crate::month::want_images(s, &review);
    let seen = (s.reports.data.progress(), s.calendar.data.progress(), s.reports.sums.summing(&key));
    let moved = seen != s.reports.seen;
    s.reports.seen = seen;
    changed || stored || summed || arrived || moved || images
}

/// Shows Day, Week, Month, Quarter, Calendar or Sessions. Month opens on the current month, and
/// Quarter on the current quarter; Quarter from Month shows the month's quarter, and Month from
/// Quarter the month it showed when that is in the quarter, else the quarter's last month that
/// has begun.
pub fn show(s: &mut Desk, mode: Mode) {
    let from = s.reports.mode;
    if mode == Mode::Sessions && from != Mode::Sessions {
        crate::sessions::entered(s);
    }
    if mode == Mode::Week && from != Mode::Week {
        crate::week::entered(s);
    }
    let r = &mut s.reports;
    if mode != from {
        match (from, mode) {
            (Mode::Month, Mode::Quarter) => {
                let q = Yq::of(r.shown_month());
                r.quarter = (q != r.this_quarter()).then_some(q);
            }
            (Mode::Quarter, Mode::Month) => {
                let q = Period::Quarter(r.shown_quarter());
                let m = if q.months().contains(&r.shown_month()) { r.shown_month() } else { q.last_begun_month(r.now(), offset()) };
                r.month = (m != r.this_month()).then_some(m);
            }
            (_, Mode::Month) => r.month = None,
            (_, Mode::Quarter) => {
                r.quarter = None;
                r.month = None;
            }
            _ => {}
        }
        if mode.period() {
            reset(r);
            if !s.browser.editing() {
                s.reports.text = s.browser.filter().to_string();
            }
        }
    }
    s.reports.mode = mode;
    s.reports.hover = None;
    s.calendar.hover = None;
}

/// The cursors and the open row of a period left behind.
fn reset(r: &mut State) {
    r.part = None;
    r.day = None;
    r.week = None;
    r.row = 0;
    r.hover = None;
    r.open = None;
}

/// Shows `period` in its view, as a click on a month and the palette's entries do.
pub fn show_period(s: &mut Desk, period: Period, tx: &mut Tx) {
    match period {
        Period::Month(ym) => {
            show(s, Mode::Month);
            s.reports.month = (ym != s.reports.this_month()).then_some(ym);
        }
        Period::Quarter(yq) => {
            show(s, Mode::Quarter);
            s.reports.quarter = (yq != s.reports.this_quarter()).then_some(yq);
        }
    }
    reset(&mut s.reports);
    tx.scroll_to_y(PANE, 0.0);
}

/// The first period that can be shown: the one of the store's first day with data.
fn first_period(s: &Desk) -> Option<Period> {
    let first = s.reports.sums()?.first_data?;
    Some(if s.reports.mode == Mode::Quarter { Period::quarter_of(first) } else { Period::month_of(first) })
}

/// Whether the period shown is the current one.
pub fn is_current(s: &Desk) -> bool {
    match s.reports.mode {
        Mode::Quarter => s.reports.shown_quarter() == s.reports.this_quarter(),
        _ => s.reports.shown_month() == s.reports.this_month(),
    }
}

/// Whether ‹ can step back: not before the period of the store's first day.
fn can_go_back(s: &Desk) -> bool {
    first_period(s).is_none_or(|f| s.reports.period().first_date() > f.first_date())
}

/// Steps the period by one, back or on, never past the current one nor before the period of the
/// store's first day.
pub fn step(s: &mut Desk, back: bool, tx: &mut Tx) {
    if back && !can_go_back(s) || !back && is_current(s) {
        return;
    }
    let to = if back { s.reports.period().previous() } else { s.reports.period().next() };
    match to {
        Period::Month(ym) => s.reports.month = (ym != s.reports.this_month()).then_some(ym),
        Period::Quarter(yq) => s.reports.quarter = (yq != s.reports.this_quarter()).then_some(yq),
    }
    let part = s.reports.part;
    reset(&mut s.reports);
    s.reports.part = part;
    tx.scroll_to_y(PANE, 0.0);
}

/// Shows the current period, as Today and `t` do.
pub fn this_period(s: &mut Desk, tx: &mut Tx) {
    match s.reports.mode {
        Mode::Quarter => s.reports.quarter = None,
        _ => s.reports.month = None,
    }
    reset(&mut s.reports);
    tx.scroll_to_y(PANE, 0.0);
}

/// ⌘1 shows Day, ⌘2 Week, ⌘3 Month, ⌘4 Quarter, ⌘5 Calendar and ⌘0 Sessions, whichever shows
/// now.
pub fn switch_key(s: &mut Desk, e: &KeyEvent) -> bool {
    let mode = [('0', Mode::Sessions), ('1', Mode::Day), ('2', Mode::Week), ('3', Mode::Month), ('4', Mode::Quarter), ('5', Mode::Calendar)]
        .into_iter()
        .find(|(c, _)| e.is_command(*c))
        .map(|(_, m)| m);
    let Some(mode) = mode else { return false };
    show(s, mode);
    true
}

/// Opens the lanes of `date` in Day, as a click on a day does.
pub fn open_day(s: &mut Desk, date: i64, tx: &mut Tx) {
    s.browser.show_day(date);
    // The selection stays where its session has a lane that day.
    s.stepped = true;
    show(s, Mode::Day);
    tx.scroll_to_y(view::PANE, 0.0);
}

/// Opens Week on the week of its Monday `monday`, as a click on a week of the quarter does.
pub fn open_week(s: &mut Desk, monday: i64, tx: &mut Tx) {
    show(s, Mode::Week);
    s.week.monday = (monday != crate::week::this_monday(s)).then_some(monday);
    tx.scroll_to_y(crate::week::PANE, 0.0);
}

/// Sets the window's filter to a project and stays on the period, as a click on it does.
pub fn filter_project(s: &mut Desk, project: &str, tx: &mut Tx) {
    s.browser.set_filter(project);
    s.reports.text = project.to_string();
    s.reports.open = None;
    s.reports.row = 0;
    tx.scroll_to_y(PANE, 0.0);
}

/// The months whose cards Month and Quarter show as images: Month's card, and in Quarter the
/// last complete month of the quarter.
pub fn review_months(s: &Desk) -> Vec<Ym> {
    match s.reports.mode {
        Mode::Month => vec![review_month(s)],
        Mode::Quarter => vec![quarter_image(s)],
        _ => Vec::new(),
    }
}

/// The month Month's card opens: the month shown when it is complete, else the last complete
/// month.
pub fn review_month(s: &Desk) -> Ym {
    let shown = s.reports.shown_month();
    if shown < s.reports.this_month() { shown } else { s.reports.this_month().previous() }
}

/// The month whose image Quarter shows: its last complete month, else the month before it.
pub fn quarter_image(s: &Desk) -> Ym {
    let this = s.reports.this_month();
    let months = Period::Quarter(s.reports.shown_quarter()).months();
    months.iter().rev().copied().find(|m| *m < this).unwrap_or_else(|| months[0].previous())
}

/// The keys of Month and Quarter: Tab through the cards, the period with ← → `[` `]` and `t`, the
/// cursors, ⏎, `/`, `r` in Month and esc; every other key does nothing.
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
    let part = s.reports.part;
    match (&e.key, text) {
        (RuneKey::Tab, _) => {
            let to = step_part(s.reports.parts(), part, e.mods.shift);
            let r = &mut s.reports;
            r.part = Some(to);
            r.row = 0;
            r.hover = None;
            tx.scroll_into_view(to.key());
            return true;
        }
        (RuneKey::Escape, _) => {
            if part.is_some() {
                s.reports.part = None;
            } else if !s.browser.filter().is_empty() {
                s.browser.clear_filter();
            } else {
                show(s, Mode::Day);
            }
            return true;
        }
        (_, Some("/")) => {
            let view = s.viewport();
            s.browser.on_key(BrowserKey::Char('/'), view);
            return true;
        }
        (_, Some("t")) => {
            this_period(s, tx);
            return true;
        }
        (_, Some("r")) if s.reports.mode == Mode::Month => {
            let ym = review_month(s);
            crate::month::open(s, Some(ym));
            return true;
        }
        (_, Some("[")) => {
            step(s, true, tx);
            return true;
        }
        (_, Some("]")) => {
            step(s, false, tx);
            return true;
        }
        _ => {}
    }
    let across: Option<i64> = match (&e.key, text) {
        (RuneKey::Left, _) | (_, Some("h")) => Some(-1),
        (RuneKey::Right, _) | (_, Some("l")) => Some(1),
        _ => None,
    };
    let Some(part) = part else {
        if let Some(d) = across {
            step(s, d < 0, tx);
        }
        return true;
    };
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
    match part {
        Part::Days | Part::Chart => {
            let Some(sums) = s.reports.sums().cloned() else { return true };
            let dates = crate::charts::openable_dates(&sums);
            let Some(at) = crate::charts::day_cursor(s, &sums).and_then(|d| dates.iter().position(|&x| x == d)) else { return true };
            let to = match (across, moved) {
                (Some(d), _) => Some(at as isize + d as isize),
                (_, Some(m)) if m.abs() > 1 => Some(at as isize + m.signum() * isize::MAX / 4),
                // ↑ ↓ move a week in the grid.
                (_, Some(m)) if part == Part::Days => Some(at as isize + 7 * m),
                _ => None,
            };
            if let Some(to) = to {
                s.reports.day = Some(dates[to.clamp(0, dates.len() as isize - 1) as usize]);
                s.reports.hover = None;
            } else if enter {
                open_day(s, dates[at], tx);
            }
        }
        Part::Weeks => {
            let Some(sums) = s.reports.sums().cloned() else { return true };
            let open: Vec<usize> = (0..sums.weeks.len()).filter(|&i| sums.weeks[i].opens()).collect();
            let Some(at) = crate::charts::week_cursor(s, &sums).and_then(|w| open.iter().position(|&x| x == w)) else { return true };
            let to = match (across, moved) {
                (Some(d), _) => Some(at as isize + d as isize),
                (_, Some(m)) if m.abs() > 1 => Some(at as isize + m.signum() * isize::MAX / 4),
                _ => None,
            };
            if let Some(to) = to {
                s.reports.week = Some(open[to.clamp(0, open.len() as isize - 1) as usize]);
                s.reports.hover = None;
            } else if enter {
                open_week(s, sums.weeks[open[at]].monday, tx);
            }
        }
        part @ (Part::Saved | Part::Ways | Part::Habits) => {
            let rows = crate::changes::line_count(s, part);
            if let Some(m) = moved {
                if rows > 0 {
                    let r = &mut s.reports;
                    r.row = (r.row as isize).saturating_add(m).clamp(0, rows as isize - 1) as usize;
                    tx.scroll_into_view(format!("{}-{}", part.key(), r.row));
                }
            } else if enter {
                crate::changes::enter(s, part, tx);
            }
        }
        part => {
            let rows = rows_of(s, part);
            if let Some(m) = moved {
                if rows > 0 {
                    let r = &mut s.reports;
                    r.row = (r.row as isize).saturating_add(m).clamp(0, rows as isize - 1) as usize;
                    tx.scroll_into_view(format!("{}-{}", part.key(), r.row));
                }
            } else if enter {
                enter_row(s, part, tx);
            }
        }
    }
    true
}

/// The rows a card's cursor moves through.
fn rows_of(s: &Desk, part: Part) -> usize {
    let listed = |n: usize| n.min(TOP) + usize::from(n > TOP);
    let sums = s.reports.sums();
    let o = s.reports.overview();
    match part {
        Part::Projects => sums.map_or(0, |x| listed(x.projects.len())),
        Part::Origins => o.map_or(0, |o| listed(o.collector.work.by_origin.len())),
        Part::Models => o.map_or(0, |o| o.collector.spend.models.len()),
        Part::Returns => o.map_or(0, |o| o.pauses.agents.values().map(|a| a.return_rows().len()).sum()),
        Part::Sizes => o.map_or(0, |o| o.pauses.agents.values().map(|a| a.size_rows().len()).sum()),
        Part::Months => sums.map_or(0, |x| x.period.months().len()),
        Part::Shifts => sums.map_or(0, |x| x.shifts.len()),
        Part::Records => records(s).len(),
        _ => 0,
    }
}

/// What ⏎ does on a row: a project sets the filter, a month opens Month, a shift sets the filter,
/// a record opens its day or week.
fn enter_row(s: &mut Desk, part: Part, tx: &mut Tx) {
    let row = s.reports.row;
    let Some(sums) = s.reports.sums().cloned() else { return };
    match part {
        Part::Projects => {
            if let Some(p) = sums.projects.get(row).filter(|_| row < TOP) {
                filter_project(s, &p.name.clone(), tx);
            }
        }
        Part::Months => {
            if let Some(&ym) = sums.period.months().get(row)
                && rhythm::midnight(ym.first_date(), offset()) <= s.reports.now()
            {
                show_period(s, Period::Month(ym), tx);
            }
        }
        Part::Shifts => {
            if let Some(x) = sums.shifts.get(row) {
                filter_project(s, &x.project.clone(), tx);
            }
        }
        Part::Records => match records(s).get(row).and_then(|r| r.target) {
            Some(Target::Day(d)) => open_day(s, d, tx),
            Some(Target::Week(m)) => open_week(s, m, tx),
            None => {}
        },
        _ => {}
    }
}

// ---------------------------------------------------------------------------------------------
// The bar.

/// The gap between the bar's controls.
const BAR_GAP: f32 = 10.0;

/// What the bar holds in Month and Quarter: the period's name with the steps to the periods before
/// and after, Today, and the span text as much of it as has room.
pub fn bar_controls(s: &Desk, cx: &Cx<Desk>, p: &Pal) -> Vec<El> {
    let period = s.reports.period();
    let current = is_current(s);
    let mut name = row().gap(6.0).center_y().child(text(period.name()).key("period-name").font(pal::DAY).color(p.ink).single_line());
    let k = s.reports.key();
    let again = (s.reports.data.reading(&k) && s.reports.report().is_some()) || (s.reports.sums.summing(&k) && s.reports.sums().is_some());
    if again {
        name = name.child(text("updating…").key("period-updating").font(pal::SMALL).color(p.ink3).single_line());
    }
    let mut out = vec![
        view::step_button("period-back", "‹", "The period before", can_go_back(s), p).on_press_tx(|s: &mut Desk, tx| step(s, true, tx)),
        name,
        view::step_button("period-on", "›", "The period after", !current, p).on_press_tx(|s: &mut Desk, tx| step(s, false, tx)),
        view::button("today", "Today", !current, p.panel, p).on_press_tx(|s: &mut Desk, tx| this_period(s, tx)),
    ];
    let today = cx.layout_with("today", |r| (r[0] + r[2]).round());
    let right = if s.status_shown() { cx.layout_with("status", |r| r[0].round()) } else { cx.layout_with("filter", |r| r[0].round()) };
    let room = today.zip(right).map(|(a, b)| b - a - 3.0 * BAR_GAP);
    if let Some(t) = fitting_span(s, room) {
        out.push(text(t).key("period-span").font(pal::BODY).color(p.ink2).single_line().truncate().shrink().min_w(0.0));
    }
    out
}

/// The longest span text that fits in `room` points: all of it, then the part before ` · `, then
/// none; all of it while the room is not known yet.
pub fn fitting_span(s: &Desk, room: Option<f32>) -> Option<String> {
    let full = span_text(s);
    let mut tries = vec![full.clone()];
    if let Some((first, _)) = full.split_once(" · ") {
        tries.push(first.to_string());
    }
    let Some(room) = room else { return Some(full).filter(|t| !t.is_empty()) };
    tries.into_iter().find(|t| !t.is_empty() && rune::text::advance(t, pal::BODY) <= room)
}

/// The dates of the period shown and what it compares with: `1–12 Nov so far · vs 1–12 Oct`,
/// `Thu 1 – Sat 31 Oct · vs September`, `1 Oct – 12 Nov, 43 of 92 days`.
pub fn span_text(s: &Desk) -> String {
    let r = &s.reports;
    let period = r.period();
    let now = r.now();
    let (from, to) = match r.sums() {
        Some(x) => (x.from_ms, x.to_ms),
        None => period.window(now, offset()),
    };
    let first = rhythm::date_of(from, offset());
    let last = rhythm::date_of((to - 1).max(from), offset());
    let complete = period.complete(now, offset());
    match period {
        Period::Month(_) if complete => {
            let day = |d: i64| local_time(noon(d), "%a %-d");
            format!("{} – {} {} · vs {}", day(first), day(last), local_time(noon(last), "%b"), period.previous().short_name())
        }
        Period::Month(_) => {
            let (a, b) = period.before(now, offset());
            let was = tokprof_core::week::dates(rhythm::date_of(a, offset()), rhythm::date_of((b - 1).max(a), offset()), false);
            format!("{} so far · vs {was}", tokprof_core::week::dates(first, last, false))
        }
        Period::Quarter(_) => {
            let span = format!("{} – {}", local_time(noon(first), "%-d %b"), local_time(noon(last), "%-d %b"));
            if complete {
                format!("{span}, {} days · vs {}", period.days(), period.previous().name())
            } else {
                format!("{span}, {} of {} days", last - period.first_date() + 1, period.days())
            }
        }
    }
}

pub fn noon(date: i64) -> i64 {
    rhythm::midnight(date, offset()) + 12 * 3_600_000
}

/// The switch between the views, at the start of the bar.
pub fn switch(s: &Desk, p: &Pal) -> El {
    let mode = s.reports.mode;
    let seg = |key: &str, label: &str, m: Mode| view::segment(key.into(), label, mode == m, p).on_press(move |s: &mut Desk| show(s, m));
    view::segments(
        "mode",
        vec![
            seg("mode-day", "Day", Mode::Day),
            seg("mode-week", "Week", Mode::Week),
            seg("mode-month", "Month", Mode::Month),
            seg("mode-quarter", "Quarter", Mode::Quarter),
            seg("mode-calendar", "Calendar", Mode::Calendar),
        ],
        p.panel,
    )
}

// ---------------------------------------------------------------------------------------------
// The pane.

/// Month or Quarter under the bar, in one scrolling pane.
pub fn view(s: &Desk, cx: &Cx<Desk>, p: &Pal) -> El {
    let r = &s.reports;
    let k = r.key();
    let sums = r.sums().map(|x| Arc::as_ptr(x) as usize);
    let logs = r.report().map(|x| (x.arrived, x.overview.as_ref().map(|o| Arc::as_ptr(o) as usize).ok()));
    let store = s.calendar.data.get().map(|c| (c.arrived, c.store.as_ref().map(|x| Arc::as_ptr(x) as usize)));
    let width = view::window_size(s, cx).0.round() as i64;
    let images = crate::month::images_version(s);
    let version = view::hash_of((
        (&k, r.mode, sums, logs, store, r.data.reading(&k), r.sums.summing(&k), &r.seen, images),
        (r.part, r.day, r.week, r.row, r.hover, &r.open, r.client_ignored),
        (&s.theme.get().id, cx.system_dark(), width, s.browser.filter(), s.browser.relisted(), s.browser.len()),
    ));
    let window_w = width as f32;
    // The memo takes its id as its key, so the viewport is outside it, keyed for the scrolls.
    col().key(PANE).w_full().grow(1.0).min_h(0.0).scroll(ScrollSpec { stick_bottom: false }).child(rune::memo("reports-body", version, || body(s, p, window_w)))
}

/// The width of the pane's content in a window `window_w` wide.
pub fn content_w(window_w: f32) -> f32 {
    (window_w - 2.0 * PAD).max(600.0)
}

/// The widths of `n` cards side by side in `w` points.
pub fn widths(w: f32, n: usize) -> f32 {
    (w - GAP * (n as f32 - 1.0)) / n as f32
}

fn body(s: &Desk, p: &Pal, window_w: f32) -> El {
    let w = content_w(window_w);
    let mut out = col().w_full().px(PAD).pt(16.0).pb(30.0).gap(GAP);
    out = out.child(band_head(s, p));
    for n in notes(s, p) {
        out = out.child(n);
    }
    let store = s.calendar.data.get().and_then(|c| c.store.as_ref());
    if store.is_none() {
        let progress = s.calendar.data.progress();
        let share = progress.filter(|g| g.1 > 0).map(|g| g.0 as f64 / g.1 as f64).unwrap_or(0.0);
        let t = match s.calendar.data.get().and_then(|c| c.calendar.as_ref().err()) {
            Some(e) => format!("error: {e}"),
            None => crate::details::reading(progress),
        };
        return out.child(waiting_line("rep-filling", t, Some(share), p));
    }
    let Some(sums) = s.reports.sums().cloned() else {
        return out.child(waiting_line("rep-summing", format!("counting {}…", s.reports.period().name()), None, p));
    };
    let t = s.reports.text.trim();
    if !t.is_empty() && !sums.key.any() && sums.projects.is_empty() {
        return out.child(
            text(format!("Nothing in {} matches \"{t}\".", sums.period.name()))
                .key("rep-nothing")
                .w_full()
                .wrap()
                .font(pal::BODY)
                .color(p.ink2)
                .padding(18.0)
                .radius(R_GROUP)
                .fill(p.bar),
        );
    }
    out = out.child(band(s, &sums, p, w));
    let pair = window_w < WIDE;
    if s.reports.mode == Mode::Quarter {
        let side = if pair { (w * 0.36).max(300.0) } else { 340.0 };
        out = out.child(
            row()
                .w_full()
                .gap(GAP)
                .items_start()
                .child(col().w(w - side - GAP).child(crate::charts::weeks_card(s, &sums, p, w - side - GAP)))
                .child(col().w(side).gap(GAP).child(crate::charts::months_card(s, &sums, p)).child(crate::charts::shifts_card(s, &sums, p))),
        );
        out = out.child(cards_row(vec![projects(s, &sums, p), models(s, p)], 2, w));
        out = out.child(changes(s, p, pair));
        out = out.child(cards_row(vec![records_card(s, &sums, p, widths(w, 2)), share_card(s, p, widths(w, 2))], 2, w));
    } else {
        let grid_w = (w * 0.4).clamp(400.0, 560.0);
        out = out.child(
            row()
                .w_full()
                .gap(GAP)
                .items_stretch()
                .child(col().w(grid_w).child(crate::charts::days_card(s, &sums, p, grid_w)))
                .child(col().w(w - grid_w - GAP).child(crate::charts::chart_card(s, &sums, p, w - grid_w - GAP, crate::charts::grid_h(&sums)))),
        );
        out = out.child(cards_row(vec![projects(s, &sums, p), origins(s, p), models(s, p)], if pair { 2 } else { 3 }, w));
        out = out.child(cards_row(vec![returns(s, p), sizes(s, p)], 2, w));
        out = out.child(changes(s, p, pair));
        out = out.child(cards_row(vec![records_card(s, &sums, p, widths(w, 2)), review_card(s, p, widths(w, 2))], 2, w));
    }
    out
}

/// Cards in rows of `per` side by side in `w` points, each as wide as the others; a last card
/// alone takes the whole row.
pub fn cards_row(cards: Vec<El>, per: usize, w: f32) -> El {
    let mut out = col().w_full().gap(GAP);
    let n = cards.len();
    let mut it = cards.into_iter().enumerate().peekable();
    while let Some(&(i, _)) = it.peek() {
        let left = n - i;
        let k = if left < per && left == 1 { 1 } else { per.min(left) };
        let cw = widths(w, if k == 1 { 1 } else { per });
        let mut line = row().w_full().gap(GAP).items_stretch();
        for _ in 0..k {
            if let Some((_, c)) = it.next() {
                line = line.child(col().w(cw).child(c));
            }
        }
        out = out.child(line);
    }
    out
}

/// The title of the band: the period, `so far` while it runs, and the filter; and the note of
/// what it compares with.
fn band_head(s: &Desk, p: &Pal) -> El {
    let r = &s.reports;
    let period = r.period();
    let mut title = period.name();
    if is_current(s) {
        title += " so far";
    }
    let t = r.text.trim();
    if !t.is_empty() {
        title += &format!(" · {t}");
    }
    let note = r.sums().map(|x| x.before.note(period.previous(), offset())).unwrap_or_default();
    row()
        .key("rep-head")
        .w_full()
        .gap(16.0)
        .items_end()
        .child(text(title).key("rep-title").font(HEADLINE).color(p.ink).single_line().truncate().shrink().min_w(0.0))
        .child(spacer().grow(1.0))
        .child(text(note).key("rep-vs").font(pal::BODY).color(p.ink2).single_line().truncate().shrink().min_w(0.0).pb(2.0))
}

/// The quiet lines under the title: `--client`, the filter, and the match by title only.
fn notes(s: &Desk, p: &Pal) -> Vec<El> {
    let mut out = Vec::new();
    let line = |key: &'static str, t: String, c: Color| text(t).key(key).w_full().wrap().font(pal::SMALL).color(c);
    if s.reports.client_ignored {
        out.push(line("rep-client", "the key numbers and charts count every client: the day store has no client".into(), p.ink3));
    }
    let t = s.reports.text.trim();
    if !t.is_empty() {
        let view_name = if s.reports.mode == Mode::Quarter { "Quarter" } else { "Month" };
        let n = s.browser.title_only(t);
        if n > 0 {
            out.push(line(
                "rep-titles",
                format!(
                    "{} \"{t}\" only by title; {view_name} counts agents, projects and session ids, not titles. Sessions (⌘0) lists them.",
                    if n == 1 { "1 session matches".to_string() } else { format!("{} sessions match", int(n as u64)) }
                ),
                p.ink3,
            ));
        }
    }
    out
}

/// The line while the store is filled, or the period is summed.
fn waiting_line(key: &'static str, t: String, share: Option<f64>, p: &Pal) -> El {
    let mut c = col()
        .key(key)
        .w_full()
        .padding(18.0)
        .gap(12.0)
        .radius(R_GROUP)
        .fill(p.bar)
        .child(text(t).key("rep-progress").font(pal::BODY_NUM).color(p.ink2).single_line().truncate().min_w(0.0));
    if let Some(share) = share {
        c = c.child(row().w(320.0).child(overlay::meter(share, p.accent, p, 6.0)));
    }
    c
}

/// A card: its title and an aside over a soft group, ringed in the accent while its part has the
/// keyboard, as the settings panel draws its parts.
pub fn card(part: Option<Part>, title: &str, aside: Option<String>, content: El, r: &State, p: &Pal) -> El {
    let on = part.is_some_and(|x| Some(x) == r.part);
    let mut group = col().w_full().grow(1.0).min_w(0.0).padding(14.0).radius(R_GROUP).fill(p.bar).child(content);
    if let Some(part) = part {
        group = group.key(part.key());
    }
    let group = if on { group.outline(2.0, 2.0, p.accent) } else { group };
    let mut top = row().w_full().gap(8.0).center_y().child(text(title.to_string()).font(pal::HEAD).color(if on { p.accent } else { p.ink2 }).single_line());
    if let Some(a) = aside {
        top = top.child(spacer().grow(1.0)).child(text(a).font(pal::SMALL).color(p.ink3).single_line().truncate().shrink().min_w(0.0));
    }
    col().grow(1.0).min_w(0.0).gap(8.0).child(top).child(group)
}

/// What a card that reads the logs shows while they are read, or why it cannot show them; none
/// once they are read.
pub fn log_state(s: &Desk, p: &Pal) -> Option<El> {
    match s.reports.report().map(|r| &r.overview) {
        Some(Ok(_)) => None,
        Some(Err(e)) => Some(text(format!("error: {e}")).key("rep-error").w_full().wrap().font(pal::BODY).color(p.ink2)),
        None => Some(text(reading_line(s)).key("rep-reading").w_full().wrap().font(pal::BODY_NUM).color(p.ink2)),
    }
}

/// `reading the logs of November 2026… 120 of 2,302 logs`; with the logs of another period
/// before it, that one's count and then this one.
pub fn reading_line(s: &Desk) -> String {
    let period = s.reports.period();
    match s.reports.data.progress() {
        Some(g) if g.key.period == period => g.text(),
        Some(g) => format!("{}, then {}", g.text(), period.name()),
        None => format!("reading the logs of {}…", period.name()),
    }
}

// ---------------------------------------------------------------------------------------------
// The key numbers.

/// A key number: what it is, the number, a line under it and its comparison or pace.
#[allow(clippy::too_many_arguments)]
fn tile(key: &'static str, w: f32, label: &str, value: String, color: Color, sub: Option<String>, under: Vec<Vec<(String, Color)>>, p: &Pal) -> El {
    let mut c = col()
        .key(key)
        .w(w)
        .px(14.0)
        .py(12.0)
        .gap(3.0)
        .radius(R_TILE)
        .fill(p.panel)
        .child(text(label.to_string()).key("label").font(pal::SMALL).color(p.ink2).single_line().truncate().min_w(0.0))
        .child(text(value).key("value").font(VALUE).color(color).single_line());
    if let Some(sub) = sub {
        c = c.child(text(sub).key("sub").w_full().wrap().max_lines(2).font(pal::SMALL).color(p.ink3));
    }
    for (i, parts) in under.into_iter().enumerate() {
        c = c.child(runs(if i == 0 { "change" } else { "change-2" }, parts));
    }
    c
}

/// A line in runs of their colors.
pub fn runs(key: &'static str, parts: Vec<(String, Color)>) -> El {
    let joined: String = parts.iter().map(|x| x.0.as_str()).collect();
    let mut spans = Vec::new();
    let mut at = 0;
    for (t, c) in &parts {
        spans.push(Span::new(at, at + t.len()).color(*c));
        at += t.len();
    }
    rich(joined, spans).key(key).w_full().wrap().font(pal::SMALL_NUM)
}

/// Hours as a key number counts them: `221h`, `7.5h` under ten hours.
pub fn big_hours(ms: i64) -> String {
    let h = ms as f64 / 3_600_000.0;
    if h >= 10.0 { format!("{}h", int(h.round() as u64)) } else { crate::week::hours(ms) }
}

/// The band of the seven key numbers.
fn band(s: &Desk, sums: &PeriodSums, p: &Pal, w: f32) -> El {
    let k = &sums.key;
    let lines = crate::deltas::key_lines(sums, p);
    let mut lines = lines.into_iter();
    let mut next = || lines.next().unwrap_or_default();
    let share = k.avoidable_share().map(|x| format!("{:.1}% of spend · ", 100.0 * x)).unwrap_or_default();
    // Seven in a row where they fit, else four over three.
    let per = if w >= 1000.0 { 7 } else { 4 };
    let tw = (w - 8.0 * (per as f32 - 1.0)) / per as f32;
    let tile = |key, label, value, color, sub, under, p| tile(key, tw, label, value, color, sub, under, p);
    let tiles = vec![
        tile("kpi-spend", "Spend", usd_whole(k.usd), p.ink, None, next(), p),
        tile("kpi-agent", "Agent hours", big_hours(k.agent_minutes as i64 * 60_000), p.ink, None, next(), p),
        tile("kpi-keyboard", "At the keyboard", big_hours(k.keyboard_ms), p.ink, None, next(), p),
        tile("kpi-sessions", "Sessions", int(k.sessions as u64), p.ink, None, next(), p),
        tile(
            "kpi-avoidable",
            "Avoidable on misses",
            usd_whole(k.avoidable_usd),
            p.cold,
            Some(format!("{share}{}", count(k.cold as u64, "cold return", "cold returns"))),
            next(),
            p,
        ),
        tile("kpi-saved", "Saved by the cache", usd_whole(k.saved_usd), p.ink, None, next(), p),
        tile("kpi-commits", "Commits", int(k.commits), p.ink, None, next(), p),
    ];
    let _ = s;
    let mut out = col().key("rep-band").w_full().gap(8.0);
    let mut it = tiles.into_iter().peekable();
    while it.peek().is_some() {
        let mut line = row().w_full().gap(8.0).items_stretch();
        for _ in 0..per {
            match it.next() {
                Some(t) => line = line.child(t),
                None => line = line.child(el().w(tw)),
            }
        }
        out = out.child(line);
    }
    out
}

/// Whole dollars: `$11,632`; cents under $10.
pub fn usd_whole(v: f64) -> String {
    if v.abs() < 10.0 { usd(v) } else { tokprof_core::week::dollars(v) }
}

// ---------------------------------------------------------------------------------------------
// Rows and tables.

/// Marks the row `i` of a part when its cursor is on it, and keys it for the keys to scroll to.
pub fn cursor_row(line: El, part: Part, i: usize, s: &Desk, p: &Pal) -> El {
    let r = &s.reports;
    let line = line.key(format!("{}-{i}", part.key())).radius(R_ROW);
    if r.part == Some(part) && r.row == i { line.fill(p.sel) } else { line.hover(p.hover) }
}

/// A row of a bar list: its name, a bar as long as its share of the first, its share of all and
/// its dollars.
fn bar_line(name: String, strong: bool, share_of_first: f64, color: Color, share: f64, amount: String, p: &Pal) -> El {
    row()
        .w_full()
        .h(24.0)
        .px(6.0)
        .gap(10.0)
        .center_y()
        .child(text(name).key("label").font(pal::BODY).color(if strong { p.ink } else { p.ink2 }).single_line().truncate().w(150.0))
        .child(overlay::meter(share_of_first, if strong { color } else { p.neutral }, p, 6.0))
        .child(row().w(48.0).justify_end().child(text(format!("{:.1}%", 100.0 * share)).key("share").font(pal::SMALL_NUM).color(p.ink2).single_line()))
        .child(row().w(64.0).justify_end().child(text(amount).key("amount").font(pal::STRONG_NUM).color(p.ink).single_line()))
}

/// By project, from the store: the ten with the most spend and the rest as one row; a click sets
/// the filter and stays on the period.
fn projects(s: &Desk, sums: &PeriodSums, p: &Pal) -> El {
    let rows = &sums.projects;
    let content = if rows.is_empty() {
        text("No spend in this period.").key("projects-none").font(pal::BODY).color(p.ink2).single_line()
    } else {
        let total: f64 = rows.iter().map(|x| x.usd).sum::<f64>().max(1e-9);
        let first = rows[0].usd.max(1e-9);
        let names = view::ProjectNames::new(rows.iter().map(|x| x.name.as_str()));
        let mut out = col().w_full().gap(2.0);
        for (i, x) in rows.iter().take(TOP).enumerate() {
            let name = x.name.clone();
            let line = bar_line(names.of(&x.name), true, x.usd / first, p.bars, x.usd / total, usd_whole(x.usd), p);
            let line = cursor_row(line, Part::Projects, i, s, p)
                .role("button")
                .label(x.name.clone())
                .on_press_tx(move |s: &mut Desk, tx| filter_project(s, &name, tx));
            out = out.child(line);
        }
        if rows.len() > TOP {
            let rest: f64 = rows[TOP..].iter().map(|x| x.usd).sum();
            let line = bar_line(format!("{} more", rows.len() - TOP), false, rest / first, p.bars, rest / total, usd_whole(rest), p);
            out = out.child(cursor_row(line, Part::Projects, TOP, s, p));
        }
        out
    };
    let aside = "a click sets the filter";
    card(Some(Part::Projects), "By project", Some(aside.into()), content, &s.reports, p)
}

/// By origin, from the logs: who asked for the requests.
fn origins(s: &Desk, p: &Pal) -> El {
    let content = match (log_state(s, p), s.reports.overview()) {
        (None, Some(o)) => {
            let rows = o.collector.work.origins();
            let priced = rows.iter().any(|(_, x)| x.usd > 0.0);
            let key = |x: &tokprof_core::report::Spend| if priced { x.usd } else { x.tokens };
            let total: f64 = rows.iter().map(|(_, x)| key(x)).sum::<f64>().max(1e-9);
            let first = rows.first().map_or(0.0, |(_, x)| key(x)).max(1e-9);
            let amount = |x: f64| if priced { usd_whole(x) } else { format!("{} tok", tokens(x)) };
            let mut out = col().w_full().gap(2.0);
            if rows.is_empty() {
                out = out.child(text("No requests in these logs.").font(pal::BODY).color(p.ink2).single_line());
            }
            for (i, (name, x)) in rows.iter().take(TOP).enumerate() {
                out = out.child(cursor_row(bar_line(name.to_string(), true, key(x) / first, p.out, key(x) / total, amount(key(x)), p), Part::Origins, i, s, p));
            }
            if rows.len() > TOP {
                let rest: f64 = rows[TOP..].iter().map(|(_, x)| key(x)).sum();
                out = out.child(cursor_row(
                    bar_line(format!("{} more", rows.len() - TOP), false, rest / first, p.out, rest / total, amount(rest), p),
                    Part::Origins,
                    TOP,
                    s,
                    p,
                ));
            }
            out
        }
        _ => col().w_full().child(log_state(s, p).unwrap_or_else(el)),
    };
    card(Some(Part::Origins), "By origin", Some("who asked for the request".into()), content, &s.reports, p)
}

pub fn component_color(c: Component, p: &Pal) -> Color {
    match c {
        Component::Read => p.read,
        Component::Write => p.write,
        Component::Rewrite => p.miss,
        Component::Output => p.out,
        _ => p.neutral,
    }
}

/// By model, from the logs: a row per model of the summary's table, in its order, with a bar of
/// its five components and its total; the pointer over a row names each component.
fn models(s: &Desk, p: &Pal) -> El {
    let Some(o) = s.reports.overview().filter(|_| log_state(s, p).is_none()) else {
        return card(Some(Part::Models), "By model", None, log_state(s, p).unwrap_or_else(el), &s.reports, p);
    };
    let models = o.collector.spend.by_cost();
    let head = row()
        .w_full()
        .h(20.0)
        .px(6.0)
        .gap(10.0)
        .center_y()
        .child(row().grow(1.0).min_w(0.0).child(text("model").font(pal::SMALL_STRONG).color(p.ink2).single_line()))
        .child(row().w(64.0).justify_end().child(text("requests").font(pal::SMALL_STRONG).color(p.ink2).single_line()))
        .child(row().w(96.0).child(text("parts").font(pal::SMALL_STRONG).color(p.ink2).single_line()))
        .child(row().w(64.0).justify_end().child(text("total").font(pal::SMALL_STRONG).color(p.ink2).single_line()));
    let mut out = col().w_full().gap(2.0).child(head).child(el().w_full().h(1.0).fill(p.row_line));
    let marked = match (s.reports.hover, s.reports.part) {
        (Some(Hover::Model(i)), _) => Some(i),
        (_, Some(Part::Models)) => Some(s.reports.row),
        _ => None,
    };
    for (i, ((agent, model), m)) in models.iter().enumerate() {
        let total = if m.priced { m.usd() } else { m.tokens() };
        let mut parts = row().w(96.0).h(8.0).gap(1.0).radius(R_MARK).clip_rounded(R_MARK);
        for (c, v) in Component::ALL.iter().zip(&m.by_component) {
            let part = if m.priced { v.usd } else { v.tokens };
            let share = (part / total.max(1e-9)) as f32;
            if share > 0.002 {
                parts = parts.child(el().w(96.0 * share).h(8.0).fill(component_color(*c, p)));
            }
        }
        let color = s.browser.agent_place(agent.name()).map_or(p.ink3, |i| p.agents[i % p.agents.len()]);
        let line = row()
            .w_full()
            .h(24.0)
            .px(6.0)
            .gap(10.0)
            .center_y()
            .child(
                row()
                    .grow(1.0)
                    .min_w(0.0)
                    .gap(6.0)
                    .center_y()
                    .child(view::dot(color, 7.0))
                    .child(text(model.clone()).key("label").font(pal::BODY).color(p.ink).single_line().truncate().shrink().min_w(0.0)),
            )
            .child(row().w(64.0).justify_end().child(text(int(m.requests)).key("requests").font(pal::BODY_NUM).color(p.ink2).single_line()))
            .child(parts)
            .child(
                row().w(64.0).justify_end().child(
                    text(if m.priced { usd_whole(m.usd()) } else { format!("{} tok", tokens(m.tokens())) })
                        .key("amount")
                        .font(pal::STRONG_NUM)
                        .color(p.ink)
                        .single_line(),
                ),
            )
            .on_hover_change(move |s: &mut Desk, inside, _| {
                if inside {
                    s.reports.hover = Some(Hover::Model(i));
                } else if s.reports.hover == Some(Hover::Model(i)) {
                    s.reports.hover = None;
                }
            });
        out = out.child(cursor_row(line, Part::Models, i, s, p));
    }
    // The model marked, by its parts, or what the colors are.
    let line = match marked.and_then(|i| models.get(i)) {
        Some(((agent, model), m)) => {
            let money = |v: &tokprof_core::profile::Values| if m.priced { usd(v.usd) } else { format!("{} tok", tokens(v.tokens)) };
            let parts: Vec<String> = Component::ALL.iter().zip(&m.by_component).map(|(c, v)| format!("{} {}", c.name(), money(v))).collect();
            let known = 100.0 * m.with_prompt as f64 / m.requests.max(1) as f64;
            text(format!("{} · {model}: {}; prompt known in {known:.0}%", agent.name(), parts.join(", ")))
                .key("models-read")
                .w_full()
                .wrap()
                .pt(6.0)
                .font(pal::SMALL_NUM)
                .color(p.ink2)
        }
        None => {
            let swatch = |c: Component| {
                row()
                    .gap(5.0)
                    .center_y()
                    .child(el().w(9.0).h(9.0).radius(R_MARK).fill(component_color(c, p)))
                    .child(text(c.name()).font(pal::SMALL).color(p.ink2).single_line())
            };
            row().key("models-legend").w_full().pt(6.0).flex_wrap().gaps(4.0, 12.0).children(Component::ALL.iter().map(|c| swatch(*c)))
        }
    };
    out = out.child(line);
    card(Some(Part::Models), "By model", Some("API-equivalent USD".into()), out, &s.reports, p)
}

/// A column of a table: its title, its width (0 for the one that takes the rest), and whether
/// its cells are numbers, set right.
struct Column(&'static str, f32, bool);

fn table(s: &Desk, part: Part, first: usize, cols: &[Column], rows: Vec<Vec<String>>, p: &Pal) -> El {
    let cell = |t: String, c: &Column, color: Color, font| {
        let t = text(t).font(font).color(color).single_line().truncate().min_w(0.0);
        match (c.1, c.2) {
            (0.0, _) => row().grow(1.0).min_w(0.0).child(t),
            (w, true) => row().w(w).justify_end().child(t),
            (w, false) => row().w(w).child(t),
        }
    };
    let head = row().w_full().h(20.0).px(6.0).gap(8.0).center_y().children(cols.iter().map(|c| cell(c.0.to_string(), c, p.ink2, pal::SMALL_STRONG)));
    let mut out = col().w_full().child(head).child(el().w_full().h(1.0).fill(p.row_line));
    for (i, r) in rows.into_iter().enumerate() {
        let line = row().w_full().h(24.0).px(6.0).gap(8.0).center_y().children(r.into_iter().zip(cols).map(|(t, c)| {
            let font = if c.2 { pal::BODY_NUM } else { pal::BODY };
            cell(t, c, p.ink, font)
        }));
        out = out.child(cursor_row(line, part, first + i, s, p));
    }
    out
}

/// An agent's part of a pause card: its name over its table or a line.
fn agent_part(agent: &tokprof_core::source::AgentId, content: El, p: &Pal) -> El {
    col().w_full().gap(6.0).child(text(format!("{} ({})", agent.info().title, agent.name())).font(pal::SMALL_STRONG).color(p.ink2).single_line()).child(content)
}

/// Cold returns by pause, per agent, as `tokprof pauses` prints them.
fn returns(s: &Desk, p: &Pal) -> El {
    let Some(o) = s.reports.overview().filter(|_| log_state(s, p).is_none()) else {
        return card(Some(Part::Returns), "Cold returns by pause", None, log_state(s, p).unwrap_or_else(el), &s.reports, p);
    };
    let cols = [
        Column("pause", 0.0, false),
        Column("returns", 52.0, true),
        Column("first req.", 70.0, true),
        Column("avoidable", 70.0, true),
        Column("median ctx", 72.0, true),
    ];
    let mut out = col().w_full().gap(14.0);
    let mut first = 0;
    for (agent, a) in &o.pauses.agents {
        let rows: Vec<Vec<String>> = a
            .return_rows()
            .iter()
            .map(|r| {
                vec![r.name.to_string(), r.count.to_string(), usd(r.usd), usd(r.premium_usd), r.median_context.map_or_else(String::new, |t| tokens(t as f64))]
            })
            .collect();
        let n = rows.len();
        let content = if n == 0 {
            text("No cold returns.").key("no-returns").font(pal::BODY).color(p.ink2).single_line()
        } else {
            let (count_, usd_first, premium) = a.return_totals();
            col().w_full().gap(6.0).child(table(s, Part::Returns, first, &cols, rows, p)).child(
                text(format!(
                    "{}: {} in first requests, {} of it avoidable.",
                    count(count_ as u64, "cold return", "cold returns"),
                    usd(usd_first),
                    usd(premium)
                ))
                .key("returns-total")
                .w_full()
                .wrap()
                .font(pal::SMALL)
                .color(p.ink2),
            )
        };
        first += n;
        out = out.child(agent_part(agent, content, p));
    }
    if o.pauses.agents.is_empty() {
        out = out.child(text("No requests in these logs.").font(pal::BODY).color(p.ink2).single_line());
    }
    card(Some(Part::Returns), "Cold returns by pause", Some("the first request after a long pause".into()), out, &s.reports, p)
}

/// The rows of an agent's largest-context table, as `tokprof pauses` prints them.
pub fn size_cells(a: &AgentPauses) -> Vec<Vec<String>> {
    let (spend_usd, spend_tokens) = a.spend();
    let total = if a.priced { spend_usd } else { spend_tokens };
    a.size_rows()
        .iter()
        .map(|r| {
            let part = if a.priced { r.usd } else { r.tokens };
            vec![
                r.name.to_string(),
                r.count.to_string(),
                if a.priced { usd_whole(r.usd) } else { format!("{} tok", tokens(r.tokens)) },
                format!("{:.1}%", 100.0 * part / total.max(1e-9)),
                r.cold_returns.to_string(),
            ]
        })
        .collect()
}

/// Sessions by largest context, per agent.
fn sizes(s: &Desk, p: &Pal) -> El {
    let Some(o) = s.reports.overview().filter(|_| log_state(s, p).is_none()) else {
        return card(Some(Part::Sizes), "Sessions by largest context", None, log_state(s, p).unwrap_or_else(el), &s.reports, p);
    };
    let cols = [
        Column("largest context", 0.0, false),
        Column("sessions", 58.0, true),
        Column("spend", 66.0, true),
        Column("share", 50.0, true),
        Column("cold ret.", 60.0, true),
    ];
    let mut out = col().w_full().gap(14.0);
    let mut first = 0;
    for (agent, a) in &o.pauses.agents {
        let rows = size_cells(a);
        let n = rows.len();
        out = out.child(agent_part(agent, table(s, Part::Sizes, first, &cols, rows, p), p));
        first += n;
    }
    card(Some(Part::Sizes), "Sessions by largest context", Some("the largest prompt of each".into()), out, &s.reports, p)
}

/// What to change, from the logs.
fn changes(s: &Desk, p: &Pal, narrow: bool) -> El {
    match (log_state(s, p), s.reports.changes()) {
        (None, Some(c)) => crate::changes::cards(s, c, p, narrow),
        (state, _) => card(Some(Part::Saved), "What to change", None, state.unwrap_or_else(el), &s.reports, p),
    }
}

// ---------------------------------------------------------------------------------------------
// Records and the cards to share.

/// What a record opens.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    Day(i64),
    Week(i64),
}

/// A record: what it is, its number, the line under it, and what it opens.
#[derive(Clone, Debug, PartialEq)]
pub struct Record {
    pub label: &'static str,
    pub value: String,
    pub sub: String,
    pub target: Option<Target>,
}

fn day_name(date: i64) -> String {
    local_time(noon(date), "%a %-d %b")
}

/// The records of the period shown: the month's six, or the quarter's.
pub fn records(s: &Desk) -> Vec<Record> {
    let Some(sums) = s.reports.sums() else { return Vec::new() };
    let r = &sums.records;
    let rec = |label, value: String, sub: String, target| Record { label, value, sub, target };
    let mut out = Vec::new();
    if let Some((d, v)) = r.biggest_day {
        out.push(rec("Biggest day", usd_whole(v), day_name(d), Some(Target::Day(d))));
    }
    let streak = r.streak_text().map(|t| match t.split_once(", ") {
        Some((a, b)) => (a.to_string(), b.to_string()),
        None => (t, String::new()),
    });
    if s.reports.mode == Mode::Quarter {
        if let Some((m, v)) = r.biggest_week {
            out.push(rec("Biggest week", usd_whole(v), tokprof_core::week::dates(m, m + 6, false), Some(Target::Week(m))));
        }
        if let Some((a, b)) = streak {
            out.push(rec("Longest streak", a, b, None));
        }
        if let Some((n, ts)) = r.most_at_once {
            let date = rhythm::date_of(ts, offset());
            out.push(rec("Most at once", count(n as u64, "session", "sessions"), local_time(ts, "%a %-d %b %H:%M"), Some(Target::Day(date))));
        }
        if let Some((m, share)) = r.best_cache_week {
            out.push(rec(
                "Best cache week",
                format!("{:.1}%", 100.0 * share),
                format!("read from cache, week of {}", local_time(noon(m), "%-d %b")),
                Some(Target::Week(m)),
            ));
        }
        let largest = match (s.reports.overview(), log_state(s, &crate::pal::pal(s.theme.get()))) {
            (Some(o), None) => match &o.pauses.largest {
                Some(l) => {
                    let date = rhythm::date_of(l.ts_ms, offset());
                    let project = view::project_name(&l.project).to_string();
                    rec("Largest prompt", format!("{} tok", tokens(l.tokens as f64)), format!("{project}, {}", day_name(date)), Some(Target::Day(date)))
                }
                None => rec("Largest prompt", "–".into(), "no prompt in these logs".into(), None),
            },
            _ => rec("Largest prompt", "…".into(), "reading the logs…".into(), None),
        };
        out.push(largest);
    } else {
        if let Some((d, v)) = r.quietest_weekday {
            out.push(rec("Quietest weekday", usd_whole(v), day_name(d), Some(Target::Day(d))));
        }
        if let Some((d, n)) = r.most_cold {
            let i = (d - sums.period.first_date()) as usize;
            let avoidable = sums.span.days.get(i).map_or(0.0, |x| x.amounts.avoidable_usd);
            out.push(rec("Most cold returns", int(n as u64), format!("{}, {} avoidable that day", day_name(d), usd_whole(avoidable)), Some(Target::Day(d))));
        }
        if let Some((a, b)) = streak {
            out.push(rec("Streak", a, b, None));
        }
        if let Some((h, share)) = r.busiest_hour {
            out.push(rec("Busiest hour", format!("{h:02}:00–{:02}:00", (h + 1) % 24), format!("{:.0}% of agent time", 100.0 * share), None));
        }
        if let Some(n) = r.night_share {
            out.push(rec("Night share", format!("{:.0}%", 100.0 * n), "of agent time 22:00–05:00".into(), None));
        }
    }
    out
}

/// The records in tiles, three in a row.
fn records_card(s: &Desk, sums: &PeriodSums, p: &Pal, w: f32) -> El {
    let list = records(s);
    let per = if w >= 560.0 { 3 } else { 2 };
    let tile_w = (w - 28.0 - 10.0 * (per as f32 - 1.0)) / per as f32;
    let on = s.reports.part == Some(Part::Records);
    let mut grid = row().w_full().flex_wrap().gaps(10.0, 10.0);
    for (i, r) in list.iter().enumerate() {
        let target = r.target;
        let mut t = col()
            .key(format!("{}-{i}", Part::Records.key()))
            .w(tile_w)
            .px(12.0)
            .py(10.0)
            .gap(1.0)
            .radius(R_TILE)
            .fill(if on && s.reports.row == i { p.sel } else { p.panel })
            .child(text(r.label).key("record-label").font(pal::SMALL).color(p.ink3).single_line())
            .child(text(r.value.clone()).key("record-value").font(RECORD).color(p.ink).single_line().truncate().min_w(0.0))
            .child(text(r.sub.clone()).key("record-sub").w_full().wrap().max_lines(2).font(pal::SMALL).color(p.ink2));
        if let Some(target) = target {
            t = t.hover(p.hover).role("button").label(format!("{} {}", r.label, r.sub)).on_press_tx(move |s: &mut Desk, tx| match target {
                Target::Day(d) => open_day(s, d, tx),
                Target::Week(m) => open_week(s, m, tx),
            });
        }
        grid = grid.child(t);
    }
    let title = if sums.period.months().len() > 1 { "Records of the quarter" } else { "Records of the month" };
    let aside = (!sums.complete).then(|| "so far".to_string());
    card(Some(Part::Records), title, aside, grid, &s.reports, p)
}

/// A button of a card to share.
fn share_button(key: String, label: String, primary: bool, enabled: bool, p: &Pal) -> El {
    row()
        .key(key)
        .h(28.0)
        .px(12.0)
        .center_y()
        .radius(PILL)
        .fill(if primary && enabled { p.accent } else { p.panel })
        .border(1.0, if primary && enabled { p.accent } else { p.row_line })
        .hover(if primary { p.accent.alpha(0.88) } else { p.hover })
        .role("button")
        .label(label.clone())
        .disabled(!enabled)
        .child(
            text(label)
                .font(pal::STRONG)
                .color(if primary && enabled {
                    p.on_accent
                } else if enabled {
                    p.ink
                } else {
                    p.ink3
                })
                .single_line(),
        )
}

/// The image of a month's card at `scale`, or its place while it is computed.
fn image(s: &Desk, ym: Ym, scale: f32, p: &Pal) -> El {
    let (w, h) = (crate::month::CARD.0 * scale, crate::month::CARD.1 * scale);
    let frame = el().key("rep-image").w(w).h(h).radius(10.0).clip_rounded(10.0).fill(p.bg).border(1.0, p.row_line);
    match crate::month::image_of(s, ym) {
        Some(m) => {
            let card = crate::month::card_el(m, p, false);
            let (dx, dy) = (-(crate::month::CARD.0 - w) / 2.0, -(crate::month::CARD.1 - h) / 2.0);
            frame.child(card.scale(scale).absolute(dx, dy))
        }
        None => frame.child(text("drawing the card…").font(pal::SMALL).color(p.ink3).single_line().absolute(12.0, 12.0)),
    }
}

/// Month in review in Month: the image of the card `r` opens, its name, Open card and Save image.
fn review_card(s: &Desk, p: &Pal, w: f32) -> El {
    let ym = review_month(s);
    let shown = s.reports.shown_month();
    let current = shown == s.reports.this_month();
    let scale = if w >= 620.0 { 0.5 } else { 0.4 };
    let mut side = col().grow(1.0).min_w(0.0).gap(8.0).child(text(ym.name()).key("review-name").font(pal::DAY).color(p.ink).single_line()).child(
        text("The card of a complete month, as tokprof month prints it, to post: no project names, no titles.").w_full().wrap().font(pal::SMALL).color(p.ink2),
    );
    if current {
        let next = shown.next().first_date();
        side = side.child(
            text(format!("{}'s card is ready on {}.", shown.month_name(), local_time(noon(next), "%a %-d %b")))
                .key("review-ready")
                .w_full()
                .wrap()
                .font(pal::SMALL)
                .color(p.ink2),
        );
    }
    side = side.child(
        row()
            .gap(8.0)
            .flex_wrap()
            .child(share_button("review-open".into(), "Open card".into(), true, true, p).on_press(move |s: &mut Desk| crate::month::open(s, Some(ym))))
            .child(share_button("review-save".into(), "Save image".into(), false, true, p).on_press(move |s: &mut Desk| crate::month::save_month(s, ym))),
    );
    side = side.child(text("r opens the card; in it, s saves a 1080×1350 image to ~/Downloads").w_full().wrap().font(pal::SMALL).color(p.ink3));
    let content = if w >= 520.0 {
        row().w_full().gap(16.0).items_start().child(image(s, ym, scale, p)).child(side)
    } else {
        col().w_full().gap(12.0).child(image(s, ym, scale, p)).child(side)
    };
    card(None, "Month in review", Some(if current { "the last complete month".into() } else { "this month".into() }), content, &s.reports, p)
}

/// Quarter's cards to share: no card of its own; the image of its last complete month, and a
/// button per month.
fn share_card(s: &Desk, p: &Pal, w: f32) -> El {
    let ym = quarter_image(s);
    let this = s.reports.this_month();
    let scale = if w >= 620.0 { 0.5 } else { 0.4 };
    let mut buttons = row().key("share-buttons").gap(8.0).flex_wrap();
    for (i, m) in Period::Quarter(s.reports.shown_quarter()).months().into_iter().enumerate() {
        let (label, enabled) = if m < this {
            (format!("Open {}", m.month_name()), true)
        } else if m == this {
            (format!("{} so far", m.month_name()), true)
        } else {
            (m.month_name(), false)
        };
        let mut b = share_button(format!("share-{i}"), label, m == ym, enabled, p);
        if enabled {
            b = b.on_press(move |s: &mut Desk| crate::month::open(s, Some(m)));
        }
        buttons = buttons.child(b);
    }
    let side = col()
        .grow(1.0)
        .min_w(0.0)
        .gap(8.0)
        .child(text("The quarter shares its months").font(pal::DAY).color(p.ink).single_line())
        .child(
            text("A quarter card would repeat its months, so the quarter opens the cards of its months: a complete month, the current one so far.")
                .w_full()
                .wrap()
                .font(pal::SMALL)
                .color(p.ink2),
        )
        .child(buttons);
    let content = if w >= 520.0 {
        row().w_full().gap(16.0).items_start().child(image(s, ym, scale, p)).child(side)
    } else {
        col().w_full().gap(12.0).child(image(s, ym, scale, p)).child(side)
    };
    card(None, "Cards to share", Some("no card of its own".into()), content, &s.reports, p)
}

// ---------------------------------------------------------------------------------------------
// The scenes.

/// The scene: Month on `ym` or Quarter on `yq`, the current one without, with the store, the
/// sums, the log read and the card's image waited for.
pub fn scene(s: &mut Desk, period: Option<Period>, quarter: bool) {
    show(s, if quarter { Mode::Quarter } else { Mode::Month });
    match period {
        Some(Period::Month(ym)) if !quarter => s.reports.month = (ym != s.reports.this_month()).then_some(ym),
        Some(Period::Quarter(yq)) if quarter => s.reports.quarter = (yq != s.reports.this_quarter()).then_some(yq),
        _ => {}
    }
    let limit = std::time::Duration::from_secs(120);
    let changes = s.browser.changes();
    s.calendar.data.settle(changes, limit);
    let store = s.calendar.data.get().and_then(|c| c.store.clone());
    let key = s.reports.key();
    s.reports.sums.settle(&key, store.as_ref(), limit);
    s.reports.data.settle(&key, changes, limit);
    let review = review_months(s);
    crate::month::settle_images(s, &review);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tab_goes_through_the_cards_from_the_page_and_back() {
        assert_eq!(step_part(&MONTH_PARTS, None, false), Part::Days);
        assert_eq!(step_part(&MONTH_PARTS, None, true), Part::Records);
        assert_eq!(step_part(&MONTH_PARTS, Some(Part::Records), false), Part::Days);
        assert_eq!(step_part(&QUARTER_PARTS, Some(Part::Weeks), true), Part::Records);
        // The savings, the ways back and the habits come after the cards with rows, before the
        // records.
        let at = |p: Part| MONTH_PARTS.iter().position(|&x| x == p).unwrap();
        assert!(at(Part::Sizes) < at(Part::Saved) && at(Part::Habits) < at(Part::Records));
    }

    #[test]
    fn hours_and_dollars_of_the_key_numbers() {
        assert_eq!(big_hours(221 * 3_600_000 + 24 * 60_000), "221h");
        assert_eq!(big_hours(7 * 3_600_000 + 30 * 60_000), "7.5h");
        assert_eq!((usd_whole(11_631.83), usd_whole(4.5)), ("$11,632".to_string(), "$4.50".to_string()));
    }
}
