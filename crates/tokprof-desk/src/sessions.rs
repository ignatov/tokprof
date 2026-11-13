//! The Sessions screen (desk-sessions): every listed session in one table, under a toolbar of
//! quick filters and over a footer that totals the filtered set, with a narrow inspector of the
//! selected session at the right. The table is the browser's list, in its order and with its
//! filters, as Day's list is; what is the screen's own is here: the mode, the folds of Projects,
//! the cursor on a project's row, the inspector being open, and the layout of the rows.

use std::borrow::Cow;
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::sync::Arc;

use rune::{Color, Cx, El, GrowthSpec, Key as RuneKey, KeyEvent, PopupMenu, ScrollSpec, Tx, col, el, row, spacer, text};
use tokprof_browse::browser::{Key, QuickFilters, Sort, now_ms};
use tokprof_core::calendar::SessionDays;
use tokprof_core::fmt::{human_duration, int, local_time, usd};
use tokprof_core::rhythm;

use crate::overlay::{self, PILL, R_ROW};
use crate::pal::{self, Pal};
use crate::reports::{self, Mode};
use crate::view::{self, dot, segment, segments};
use crate::{Desk, plain};

/// The keys of the table's and the inspector's scroll viewports.
pub const TABLE: &str = "sessions-table";
pub const PANE: &str = "sessions-pane";
/// The height of a row of the table, a day's header and a project's row alike.
pub const ROW: f32 = 28.0;
/// The width of the inspector.
pub const INSPECTOR_W: f32 = 360.0;
const TOOL_H: f32 = 44.0;
const HEAD_H: f32 = 28.0;
const FOOT_H: f32 = 34.0;
/// The gap between cells, and the table's padding at each side.
const GAP: f32 = 6.0;
const PAD: f32 = 10.0;
/// The least width of the title, which takes the room the other columns leave.
const TITLE_MIN: f32 = 100.0;
/// The room before the model, which follows the right-set `Next`.
const MODEL_PAD: f32 = 10.0;

/// A slot of the table: a day's header, a group's label in waiting order, a project's row, or a
/// session's row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Slot {
    Day(i64),
    Label(&'static str),
    Project(usize),
    Row(usize),
}

/// A project of Projects mode: its sessions' rows in the table's order and their sums.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Group {
    pub project: String,
    pub rows: Vec<usize>,
    pub cost: f64,
    pub avoidable: f64,
    pub cold: u64,
    pub last_ms: i64,
}

/// A day's header: how many rows are under it, and that day's spend in the day store of the
/// sessions the filters keep, while the store is not there yet `None`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Header {
    pub count: usize,
    pub spend: Option<f64>,
}

/// The table as the view last laid it out.
#[derive(Default)]
pub struct Layout {
    pub slots: Vec<Slot>,
    pub groups: Vec<Group>,
    pub headers: HashMap<i64, Header>,
    /// The `Last` and `Started` cells carry their day: the table is one list.
    pub flat: bool,
    /// Some time in the table is in another year than now, which its cells then name.
    pub years: bool,
    /// The columns shown, with their widths (0 for the title).
    pub cols: Vec<(Col, f32)>,
    /// The days the rows are grouped by, and the day of now among them.
    pub days: Days,
    pub today: i64,
}

/// The days of the table, as Day's days run (work-rhythm, Start of the day): by the day store's
/// days once the store is there, so that a header's date, its spend and `Open in Day` agree; before
/// that by the local date five hours earlier, as Day finds a selected session's day.
#[derive(Clone, Default)]
pub struct Days(Option<Arc<SessionDays>>);

impl Days {
    /// The date of the day the moment `ms` belongs to.
    pub fn date_of(&self, ms: i64) -> i64 {
        let offset = &rhythm::local_offset;
        match &self.0 {
            Some(days) => days.date_of(ms, offset),
            None => rhythm::date_of(ms - 5 * 3_600_000, offset),
        }
    }
}

/// The days of the table as they are now.
pub fn days(s: &Desk) -> Days {
    Days(session_days(s))
}

/// The screen's state.
#[derive(Default)]
pub struct State {
    /// `Projects` instead of `Sessions`.
    pub projects: bool,
    /// The projects unfolded in Projects mode; they all start folded.
    pub open: HashSet<String>,
    /// The project whose row the cursor is on, in Projects mode; otherwise the cursor is on the
    /// browser's selected row.
    pub cursor: Option<String>,
    /// The inspector shows the selected session; esc and its close button close it.
    pub inspector: bool,
    /// The inspector has the keyboard, not the table.
    pub keys_inspector: bool,
    /// How far the table and the inspector are scrolled, and the table's height.
    pub offset: Rc<Cell<f32>>,
    pub pane_offset: Rc<Cell<f32>>,
    pub height: f32,
    /// The first slot on the screen is a header, as the last poll found the table.
    pub separator_on_top: bool,
    /// The rows of the table, as the view last laid them out.
    pub layout: RefCell<Layout>,
    /// The day store's spend by day and session, with the store and the date it was made for.
    days: RefCell<Option<(usize, i64, Arc<SessionDays>)>>,
    /// The window's flags leave sessions out, so that a day's header counts the listed ones even
    /// with no filter of the screen's set.
    pub narrowed: bool,
}

/// The table's columns, in their order (desk-sessions, Table columns).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Col {
    Last,
    Agent,
    Project,
    Title,
    Started,
    Duration,
    Requests,
    Cost,
    Avoidable,
    Cold,
    Next,
    Model,
}

impl Col {
    pub const ALL: [Col; 12] = [
        Col::Last,
        Col::Agent,
        Col::Project,
        Col::Title,
        Col::Started,
        Col::Duration,
        Col::Requests,
        Col::Cost,
        Col::Avoidable,
        Col::Cold,
        Col::Next,
        Col::Model,
    ];

    /// The columns a narrow table leaves out, first to last.
    pub const DROPPED: [Col; 3] = [Col::Model, Col::Started, Col::Requests];

    pub fn title(self) -> &'static str {
        match self {
            Col::Last => "Last",
            Col::Agent => "",
            Col::Project => "Project",
            Col::Title => "Title",
            Col::Started => "Started",
            Col::Duration => "Duration",
            Col::Requests => "Requests",
            Col::Cost => "Cost",
            Col::Avoidable => "Avoidable",
            Col::Cold => "Cold",
            Col::Next => "Next",
            Col::Model => "Model",
        }
    }

    /// The name its elements are keyed by: `col-<name>` in a row, `head-<name>` in the header,
    /// `foot-<name>` in the footer.
    pub fn name(self) -> &'static str {
        match self {
            Col::Last => "last",
            Col::Agent => "agent",
            Col::Project => "project",
            Col::Title => "title",
            Col::Started => "started",
            Col::Duration => "duration",
            Col::Requests => "requests",
            Col::Cost => "cost",
            Col::Avoidable => "avoidable",
            Col::Cold => "cold",
            Col::Next => "next",
            Col::Model => "model",
        }
    }

    /// The order a click on its header sorts by.
    pub fn sort(self) -> Sort {
        Sort::COLUMNS[Col::ALL.iter().position(|&c| c == self).unwrap_or(0)]
    }

    /// Its cells are set right: times and numbers.
    fn right(self) -> bool {
        !matches!(self, Col::Agent | Col::Project | Col::Title | Col::Model)
    }

    /// Its width; 0 for the title, which takes the rest. `flat` for a table without day headers,
    /// whose times carry their day, `years` when some of them carry their year too, and
    /// `narrow` while the inspector takes its width.
    fn width(self, flat: bool, years: bool, narrow: bool) -> f32 {
        match self {
            Col::Last if !flat => 40.0,
            Col::Last | Col::Started if years => 104.0,
            Col::Last | Col::Started => 78.0,
            Col::Agent => 10.0,
            // `temporary folders`, the longest name the program gives, fits either way.
            Col::Project if narrow => 120.0,
            Col::Project => 132.0,
            Col::Title => 0.0,
            Col::Duration if narrow => 50.0,
            Col::Duration => 56.0,
            Col::Requests => 64.0,
            Col::Cost => 66.0,
            Col::Avoidable => 74.0,
            Col::Cold if narrow => 38.0,
            Col::Cold => 44.0,
            Col::Next => 68.0,
            Col::Model if narrow => 124.0,
            Col::Model => 140.0,
        }
    }
}

/// The columns that fit a table `width` points wide, with their widths: `Model`, then
/// `Started`, then `Requests` left out while the rest and the title's least width do not fit.
pub fn fit(width: f32, flat: bool, years: bool, narrow: bool) -> Vec<(Col, f32)> {
    let mut shown: Vec<Col> = Col::ALL.to_vec();
    let need = |shown: &[Col]| {
        shown.iter().map(|c| if *c == Col::Title { TITLE_MIN } else { c.width(flat, years, narrow) }).sum::<f32>()
            + GAP * (shown.len() as f32 - 1.0)
            + 2.0 * PAD
    };
    for drop in Col::DROPPED {
        if need(&shown) <= width {
            break;
        }
        shown.retain(|&c| c != drop);
    }
    // Room the title does not need lets a long project keep more of its name; with too little
    // room even without those columns, the project gives up some of its own, down to a limit.
    let spare = (width - need(&shown) - SPARE_TITLE).clamp(0.0, PROJECT_MORE);
    let short = (need(&shown) - width).clamp(0.0, PROJECT_LESS);
    shown.into_iter().map(|c| (c, c.width(flat, years, narrow) + if c == Col::Project { spare - short } else { 0.0 })).collect()
}

/// The room the title keeps before the project takes more, the most it takes, and the most it
/// gives up in a window too narrow.
const SPARE_TITLE: f32 = 240.0;
const PROJECT_MORE: f32 = 48.0;
const PROJECT_LESS: f32 = 30.0;
/// Under this window width the agents' pills show their dots and counts without their names.
const COMPACT_W: f32 = 1240.0;

/// When Sessions shows: the table has the keyboard, the inspector shows the selected session,
/// and in Projects mode the cursor is on its project's row.
pub fn entered(s: &mut Desk) {
    let current = s.current();
    let project = current.map(|r| s.browser.cells(r, now_ms()).project.to_string());
    let st = &mut s.sessions;
    st.inspector = current.is_some();
    st.keys_inspector = false;
    st.cursor = None;
    if st.projects
        && let Some(project) = project
        && !st.open.contains(&project)
    {
        st.cursor = Some(project);
    }
}

/// Whether the inspector shows: it is open, and the cursor is on a session's row.
pub fn inspector_shown(s: &Desk) -> bool {
    s.sessions.inspector && s.sessions.cursor.is_none() && s.current().is_some()
}

/// Whether the table has the keyboard: no filter is being typed, and the inspector does not.
pub fn table_has_keys(s: &Desk) -> bool {
    !s.browser.editing() && !(s.sessions.keys_inspector && inspector_shown(s))
}

/// The day store's spend by day and session, made once per store and date.
fn session_days(s: &Desk) -> Option<Arc<SessionDays>> {
    let store = s.calendar.data.get()?.store.clone()?;
    let now = now_ms();
    let today = rhythm::date_of(now, &rhythm::local_offset);
    let key = Arc::as_ptr(&store) as usize;
    let mut kept = s.sessions.days.borrow_mut();
    if let Some((k, d, days)) = kept.as_ref()
        && *k == key
        && *d == today
    {
        return Some(days.clone());
    }
    let days = Arc::new(SessionDays::of(&store.entries, now, &rhythm::local_offset));
    *kept = Some((key, today, days.clone()));
    Some(days)
}

/// Lays the table out for the browser's list as it is now: in Projects mode a row per project
/// with its sessions under it while it is open; in waiting order the waiting sessions and the
/// others under their labels; in time order, newest first, the sessions under their days'
/// headers; else one list.
pub fn layout(s: &Desk) -> Layout {
    let b = &s.browser;
    let now = now_ms();
    let n = b.len();
    let year = year_start(now);
    let years = (0..n).any(|r| b.active_ms(r) < year || b.glance(r).and_then(|g| g.started_ms).is_some_and(|t| t < year));
    let days = days(s);
    let mut out = Layout { years, today: days.date_of(now), days, ..Layout::default() };
    if s.sessions.projects {
        out.flat = true;
        let mut at: HashMap<&str, usize> = HashMap::new();
        for r in 0..n {
            let c = b.cells(r, now);
            let i = *at.entry(c.project).or_insert_with(|| {
                out.groups.push(Group { project: c.project.to_string(), ..Group::default() });
                out.groups.len() - 1
            });
            let g = &mut out.groups[i];
            g.rows.push(r);
            g.last_ms = g.last_ms.max(c.active_ms);
            if let Some(gl) = b.glance(r) {
                g.cost += gl.cost;
                g.avoidable += gl.avoidable;
                g.cold += gl.cold_returns;
            }
        }
        out.groups.sort_by(|a, b| b.cost.total_cmp(&a.cost).then_with(|| b.last_ms.cmp(&a.last_ms)).then_with(|| a.project.cmp(&b.project)));
        for (i, g) in out.groups.iter().enumerate() {
            out.slots.push(Slot::Project(i));
            if s.sessions.open.contains(&g.project) {
                out.slots.extend(g.rows.iter().map(|&r| Slot::Row(r)));
            }
        }
        return out;
    }
    if b.sort_order() == Sort::Waiting {
        out.flat = false;
        let last = (0..n).rev().find(|&r| b.wait(r, now).is_some());
        out.slots.push(Slot::Label(if last.is_some() { "Waiting for you" } else { "Nobody waits for you" }));
        let others = last.map_or(0, |l| l + 1);
        for r in 0..n {
            if r == others {
                out.slots.push(Slot::Label("Others"));
            }
            out.slots.push(Slot::Row(r));
        }
        if others >= n && last.is_none() {
            out.slots.push(Slot::Label("Others"));
        }
        return out;
    }
    let by_day = b.sort_order() == Sort::Time && !b.sort_reversed();
    out.flat = !by_day;
    let mut last = None;
    for r in 0..n {
        if by_day {
            let date = out.days.date_of(b.active_ms(r));
            if last != Some(date) {
                out.slots.push(Slot::Day(date));
                last = Some(date);
            }
            out.headers.entry(date).or_default().count += 1;
        }
        out.slots.push(Slot::Row(r));
    }
    if by_day && let Some(days) = out.days.0.clone() {
        let unfiltered = b.quick_filters().is_empty() && b.filter().is_empty() && !s.sessions.narrowed;
        let keys = (!unfiltered).then(|| b.store_keys());
        for (date, h) in out.headers.iter_mut() {
            h.spend = Some(match &keys {
                None => days.total(*date),
                Some(keys) => days.spend(*date, |a, m| keys.contains(&(a, m))),
            });
        }
    }
    out
}

/// 00:00 local time on the first of January of the year of `now`.
fn year_start(now: i64) -> i64 {
    let year = local_time(now, "%Y");
    tokprof_browse::filters::parse_date(&format!("{year}-01-01")).unwrap_or(i64::MIN)
}

/// A time of the table: `HH:MM` under a day's header, else its day and time, `11 Nov 23:12`,
/// with the year when it is not this year.
pub fn when(ms: i64, flat: bool, now: i64) -> String {
    if !flat {
        return local_time(ms, "%H:%M");
    }
    if ms < year_start(now) { local_time(ms, "%-d %b %Y %H:%M") } else { local_time(ms, "%-d %b %H:%M") }
}

/// The `Started` cell: `HH:MM` on the day of the last activity, else its day and time.
pub fn started(start: i64, active: i64, now: i64, days: &Days) -> String {
    if days.date_of(start) == days.date_of(active) { local_time(start, "%H:%M") } else { when(start, true, now) }
}

/// The `Model` cell: the model of most of the requests, and how many others there are.
pub fn model_cell(model: Option<(&str, usize)>) -> String {
    match model {
        Some((m, 0)) => m.to_string(),
        Some((m, more)) => format!("{m} +{more}"),
        None => "—".to_string(),
    }
}

/// The `Sessions` button, apart from the switch, marked while Sessions shows.
pub fn bar_button(s: &Desk, p: &Pal) -> El {
    let on = s.reports.mode == Mode::Sessions;
    row()
        .key("sessions-button")
        .h(28.0)
        .px(12.0)
        .center_y()
        .radius(PILL)
        .fill(if on { p.accent } else { p.panel })
        .hover(if on { p.accent } else { p.hover })
        .role("button")
        .label("Sessions")
        .checked(on)
        .on_press_tx(|s: &mut Desk, tx| show(s, tx))
        .child(text("Sessions").font(pal::STRONG).color(if on { p.on_accent } else { p.ink }).single_line())
}

/// Shows Sessions with the selected row on the screen, as ⌘0 and the button do.
pub fn show(s: &mut Desk, tx: &mut Tx) {
    reports::show(s, Mode::Sessions);
    reveal(s, tx);
}

/// Scrolls the table to the cursor's row.
fn reveal(s: &Desk, tx: &mut Tx) {
    match &s.sessions.cursor {
        Some(p) => tx.scroll_into_view(format!("proj-{p}")),
        None => {
            if let Some(r) = s.current() {
                tx.scroll_into_view(s.browser.session(r).to_string());
            }
        }
    }
}

/// What the bar holds in Sessions in place of the day: `All sessions`, since when and how many,
/// and Today.
pub fn bar_controls(s: &Desk, p: &Pal) -> Vec<El> {
    let (n, first) = s.browser.all_sessions();
    let mut since = format!("{} {}", int(n as u64), if n == 1 { "session" } else { "sessions" });
    if let Some(first) = first {
        let fmt = if first < year_start(now_ms()) { "%-d %b %Y" } else { "%-d %b" };
        since = format!("since {} · {since}", local_time(first, fmt));
    }
    vec![
        text("All sessions").key("all-sessions").font(pal::DAY).color(p.ink).single_line().pl(4.0),
        text(since).key("all-since").font(pal::BODY).color(p.ink2).single_line().truncate().shrink().min_w(0.0),
        view::button("today", "Today", true, p.panel, p).on_press_tx(|s: &mut Desk, tx| today(s, tx)),
    ]
}

/// Today and `t`: time order, newest first, scrolled to its top.
pub fn today(s: &mut Desk, tx: &mut Tx) {
    if s.browser.sort_order() != Sort::Time || s.browser.sort_reversed() {
        s.browser.set_sort(Sort::Time, false);
    }
    tx.scroll_to_y(TABLE, 0.0);
}

/// The Sessions screen under the bar: the toolbar, the table with its footer, and the inspector.
pub fn view(s: &Desk, cx: &Cx<Desk>, p: &Pal) -> El {
    let shown = inspector_shown(s);
    let window_w = view::window_size(s, cx).0;
    let width = window_w - if shown { INSPECTOR_W + 1.0 } else { 0.0 };
    let mut layout = layout(s);
    layout.cols = fit(width, layout.flat, layout.years, shown);
    *s.sessions.layout.borrow_mut() = layout;
    let table = table_box(s, p);
    let mut body = row().key("sessions-body").w_full().grow(1.0).min_h(0.0).child(table);
    if shown {
        let theme = s.theme.get().id.clone();
        let version = view::hash_of((
            crate::inspector::version(s, cx, &theme),
            s.sessions.keys_inspector,
            glance_version(s),
            cx.layout_size(PANE).map(|sz| sz[0].to_bits()),
        ));
        body = body.child(el().w(1.0).h_full().fill(p.line)).child(rune::memo("sessions-inspector", version, || inspector(s, cx, p)));
    }
    col().key("sessions").w_full().grow(1.0).min_h(0.0).child(toolbar(s, p)).child(el().w_full().h(1.0).fill(p.line)).child(body)
}

/// What the inspector shows of the selected session's glance, for its memo.
fn glance_version(s: &Desk) -> Option<(u64, u64, u64)> {
    let g = s.current().and_then(|r| s.browser.glance(r))?;
    Some((g.cost.to_bits(), g.avoidable.to_bits(), g.cold_returns))
}

/// A pill of the toolbar: its label, the count it keeps, and its dot for an agent.
fn pill(key: String, label: &str, count: Option<usize>, agent: Option<Color>, on: bool, p: &Pal) -> El {
    pill_named(key, label, true, count, agent, on, p)
}

/// A pill whose label shows, or only its dot and count, the label in its help.
fn pill_named(key: String, label: &str, named: bool, count: Option<usize>, agent: Option<Color>, on: bool, p: &Pal) -> El {
    let mut pill = row()
        .key(key)
        .h(24.0)
        .px(10.0)
        .gap(5.0)
        .center_y()
        .radius(PILL)
        .fill(if on { p.accent } else { p.panel })
        .border(1.0, if on { p.accent } else { p.row_line })
        .hover(if on { p.accent.alpha(0.88) } else { p.hover })
        .role("checkbox")
        .checked(on)
        .label(label.to_string());
    if let Some(c) = agent {
        pill = pill.child(dot(c, 7.0));
    }
    if !named {
        pill = pill.help(label.to_string());
    } else {
        pill = pill.child(
            text(label.to_string()).key("pill-label").font(if on { pal::STRONG } else { pal::BODY }).color(if on { p.on_accent } else { p.ink }).single_line(),
        );
    }
    if let Some(n) = count {
        pill = pill.child(text(int(n as u64)).key("pill-count").font(pal::SMALL_NUM).color(if on { p.on_accent.alpha(0.8) } else { p.ink3 }).single_line());
    }
    pill
}

fn vsep(p: &Pal) -> El {
    el().w(1.0).h(18.0).fill(p.line)
}

/// Sets the quick filters as `change` changes them, and keeps the cursor's row on the screen.
fn quick(s: &mut Desk, tx: &mut Tx, change: impl FnOnce(&mut QuickFilters)) {
    let mut q = s.browser.quick_filters().clone();
    change(&mut q);
    s.browser.set_quick_filters(q);
    after_filter(s, tx);
}

/// After the set changed: a project row the cursor was on may be gone, and the selected row is
/// shown.
fn after_filter(s: &mut Desk, tx: &mut Tx) {
    if let Some(p) = &s.sessions.cursor
        && !(0..s.browser.len()).any(|r| s.browser.cells(r, now_ms()).project == p)
    {
        s.sessions.cursor = None;
    }
    tx.scroll_to_y(TABLE, 0.0);
    reveal(s, tx);
}

/// The toolbar: the mode, the quick filters with their counts, the project picker, `Waiting
/// first` with the setting on, and how the table is laid out.
fn toolbar(s: &Desk, p: &Pal) -> El {
    let b = &s.browser;
    let q = b.quick_filters();
    let now = now_ms();
    let st = &s.sessions;
    let count = |f: QuickFilters| Some(b.quick_count(&f, now));
    let mode = segments(
        "sessions-mode",
        vec![
            segment("mode-sessions".into(), "Sessions", !st.projects, p).on_press_tx(|s: &mut Desk, tx| set_projects(s, false, tx)),
            segment("mode-projects".into(), "Projects", st.projects, p).on_press_tx(|s: &mut Desk, tx| set_projects(s, true, tx)),
        ],
        p.panel,
    );
    let mut bar = row()
        .key("sessions-toolbar")
        .w_full()
        .h(TOOL_H)
        .px(12.0)
        .gap(8.0)
        .center_y()
        .child(mode)
        .child(vsep(p))
        .child(pill("pill-all".into(), "All", None, None, q.is_empty(), p).on_press_tx(|s: &mut Desk, tx| quick(s, tx, |q| *q = QuickFilters::default())))
        .child(
            pill("pill-running".into(), "Running now", count(QuickFilters { running: true, ..Default::default() }), None, q.running, p)
                .on_press_tx(|s: &mut Desk, tx| quick(s, tx, |q| q.running = !q.running)),
        )
        .child(
            pill("pill-warm".into(), "Warm", count(QuickFilters { warm: true, ..Default::default() }), None, q.warm, p)
                .on_press_tx(|s: &mut Desk, tx| quick(s, tx, |q| q.warm = !q.warm)),
        )
        .child(
            pill("pill-week".into(), "This week", count(QuickFilters { this_week: true, ..Default::default() }), None, q.this_week, p)
                .on_press_tx(|s: &mut Desk, tx| quick(s, tx, |q| q.this_week = !q.this_week)),
        )
        .child(vsep(p));
    let compact = s.window.0 > 0.0 && s.window.0 < COMPACT_W;
    for (agent, _) in b.agents() {
        let on = q.agents.contains(&agent);
        let place = s.browser.agent_place(&agent).unwrap_or(0);
        let n = count(QuickFilters { agents: vec![agent.clone()], ..Default::default() });
        let name = agent.clone();
        let pill = pill_named(format!("pill-agent-{agent}"), &agent, !compact, n, Some(p.agents[place % p.agents.len()]), on, p);
        bar = bar.child(pill.on_press_tx(move |s: &mut Desk, tx| {
            let name = name.clone();
            quick(s, tx, move |q| match q.agents.iter().position(|a| *a == name) {
                Some(i) => {
                    q.agents.remove(i);
                }
                None => q.agents.push(name),
            })
        }));
    }
    bar = bar.child(vsep(p)).child(picker(s, p));
    if b.waiting() {
        let on = b.sort_order() == Sort::Waiting;
        bar = bar.child(pill("waiting-first".into(), "Waiting first", None, None, on, p).on_press_tx(move |s: &mut Desk, tx| {
            s.browser.sort_by(if on { Sort::Time } else { Sort::Waiting });
            tx.scroll_to_y(TABLE, 0.0);
        }));
    }
    let layout = st.layout.borrow();
    let what = if st.projects {
        "grouped by project".to_string()
    } else if b.sort_order() == Sort::Waiting {
        "waiting first".to_string()
    } else if !layout.flat {
        "grouped by day".to_string()
    } else {
        format!("one list, by {}", b.sort_order().name())
    };
    bar.child(spacer().grow(1.0)).child(text(what).key("sessions-note").font(pal::SMALL).color(p.ink3).single_line().truncate().shrink().min_w(0.0))
}

/// The project picker: `Project: any`, or the project picked; a click opens the projects of the
/// listed sessions, the most sessions first, each with its count.
fn picker(s: &Desk, p: &Pal) -> El {
    let picked = s.browser.quick_filters().project.clone();
    let on = picked.is_some();
    let label = picked.as_deref().map_or_else(|| "Project: any".to_string(), |p| view::project_name(p).to_string());
    row()
        .key("project-picker")
        .h(24.0)
        .pl(10.0)
        .pr(8.0)
        .gap(5.0)
        .center_y()
        .max_w(220.0)
        .radius(PILL)
        .fill(if on { p.accent } else { p.panel })
        .border(1.0, if on { p.accent } else { p.row_line })
        .hover(if on { p.accent.alpha(0.88) } else { p.hover })
        .role("button")
        .label(picked.clone().unwrap_or_else(|| label.clone()))
        .on_press_tx(|s: &mut Desk, tx| {
            let menu = project_menu(s);
            tx.popup_menu(menu, "project-picker", |s: &mut Desk, picked, tx| {
                if let Some(id) = picked {
                    pick_project(s, id, tx);
                }
            });
        })
        .child(
            text(label)
                .key("picker-label")
                .font(if on { pal::STRONG } else { pal::BODY })
                .color(if on { p.on_accent } else { p.ink })
                .single_line()
                .truncate()
                .shrink()
                .min_w(0.0),
        )
        .child(text("▾").font(pal::SMALL).color(if on { p.on_accent } else { p.ink3 }).single_line())
}

/// The picker's menu: `any`, then each project by its name with its count, by its path where
/// two projects have one name; its ids are `any` and `p:<project>`.
pub fn project_menu(s: &Desk) -> PopupMenu {
    let selected = s.browser.quick_filters().project.as_ref().map_or_else(|| "any".to_string(), |p| format!("p:{p}"));
    let projects = s.browser.projects();
    let mut names: HashMap<&str, usize> = HashMap::new();
    for (p, _) in &projects {
        *names.entry(view::project_name(p)).or_default() += 1;
    }
    let shown = |p: &str| if names.get(view::project_name(p)).copied().unwrap_or(0) > 1 { p.to_string() } else { view::project_name(p).to_string() };
    let options = std::iter::once(("any".to_string(), "Project: any".to_string()))
        .chain(projects.iter().map(|(p, n)| (format!("p:{p}"), format!("{} · {}", shown(p), int(*n as u64)))));
    PopupMenu::picker(options, &selected)
}

/// Picks `any` or `p:<project>` in the picker.
pub fn pick_project(s: &mut Desk, id: &str, tx: &mut Tx) {
    let project = id.strip_prefix("p:").map(str::to_string);
    quick(s, tx, |q| q.project = project);
}

/// Switches between `Sessions` and `Projects`; the folds stay as they were.
pub fn set_projects(s: &mut Desk, on: bool, tx: &mut Tx) {
    if s.sessions.projects == on {
        return;
    }
    s.sessions.projects = on;
    s.sessions.cursor = None;
    if on && let Some(r) = s.current() {
        let project = s.browser.cells(r, now_ms()).project.to_string();
        if !s.sessions.open.contains(&project) {
            s.sessions.cursor = Some(project);
        }
    }
    tx.scroll_to_y(TABLE, 0.0);
    reveal(s, tx);
}

/// Folds or unfolds a project, with the cursor on its row.
pub fn toggle(s: &mut Desk, project: &str, tx: &mut Tx) {
    if !s.sessions.open.remove(project) {
        s.sessions.open.insert(project.to_string());
    }
    s.sessions.cursor = Some(project.to_string());
    tx.scroll_into_view(format!("proj-{project}"));
}

/// The table with its header row on top and its footer under it.
fn table_box(s: &Desk, p: &Pal) -> El {
    let layout = s.sessions.layout.borrow();
    let count = layout.slots.len();
    let offset = s.sessions.offset.clone();
    let rows = col().key("sessions-rows").w_full().growth(GrowthSpec::default()).virtual_rows(count, ROW, slot_key, slot_el);
    let table = col()
        .key(TABLE)
        .w_full()
        .grow(1.0)
        .min_h(0.0)
        .scroll(ScrollSpec { stick_bottom: false })
        .on_scroll_offset(move |at| offset.set(at.offset[1]))
        .on_layout(|s: &mut Desk, r| s.sessions.height = r[3])
        .child(rows);
    let b = &s.browser;
    let none = b.is_empty().then(|| {
        let what = if b.filter().is_empty() { "No session is kept by these filters".to_string() } else { format!("No session matches “{}”", b.filter()) };
        let hint =
            if b.filter().is_empty() { overlay::hint(&[], "click All to see every session", p) } else { overlay::hint(&["esc"], "clears the filter", p) };
        col().w_full().padding(12.0).child(overlay::empty("sessions-empty", what, Some(hint), p))
    });
    col()
        .key("sessions-box")
        .grow(1.0)
        .h_full()
        .min_w(0.0)
        .min_h(0.0)
        .child(header_row(s, &layout.cols, p))
        .child(el().w_full().h(1.0).fill(p.line))
        .children(none)
        .child(table)
        .child(el().w_full().h(1.0).fill(p.line))
        .child(footer(s, &layout.cols, p))
}

/// A cell of a column: as wide as the column, its content set right for times and numbers.
fn cell(c: Col, w: f32, content: El) -> El {
    let cell = if w == 0.0 { row().grow(1.0).min_w(0.0) } else { row().w(w).min_w(w) };
    let cell = cell.h_full().center_y();
    if c.right() { cell.justify_end().child(content) } else { cell.child(content) }
}

/// The header row: a title per column, a click sorts by it, and the sorted one is marked with
/// `↓` or `↑`.
fn header_row(s: &Desk, cols: &[(Col, f32)], p: &Pal) -> El {
    let b = &s.browser;
    let (sort, reversed) = (b.sort_order(), b.sort_reversed());
    let keys = table_has_keys(s);
    row().key("sessions-head").w_full().h(HEAD_H).px(PAD).gap(GAP).center_y().children(cols.iter().map(|&(c, w)| {
        if c == Col::Agent {
            return cell(c, w, el());
        }
        let on = c.sort() == sort;
        // Down for the greatest, the newest or Z first; up the other way.
        let down = sort.is_text() == reversed;
        let label = if on { format!("{} {}", c.title(), if down { "↓" } else { "↑" }) } else { c.title().to_string() };
        let color = if on { if keys { p.accent } else { p.ink } } else { p.ink3 };
        let head = row()
            .key(format!("head-{}", c.name()))
            .h(22.0)
            .center_y()
            .role("button")
            .label(format!("Sort by {}", c.title()))
            .on_press_tx(move |s: &mut Desk, tx| {
                s.browser.sort_by_column(c.sort());
                tx.scroll_to_y(TABLE, 0.0);
                reveal(s, tx);
            })
            .child(text(label).key("head-text").font(pal::SMALL_STRONG).color(color).single_line().truncate().min_w(0.0));
        cell(c, w, if c == Col::Model { head.pl(MODEL_PAD) } else { head })
    }))
}

/// The key of slot `i`: a session's id, which names its row while rows come and go.
fn slot_key(s: &Desk, i: usize) -> Cow<'_, str> {
    let layout = s.sessions.layout.borrow();
    match layout.slots.get(i) {
        Some(Slot::Row(r)) if *r < s.browser.len() => Cow::Borrowed(s.browser.session(*r)),
        Some(Slot::Day(d)) => Cow::Owned(format!("sday-{d}")),
        Some(Slot::Label(l)) => Cow::Owned(format!("label-{l}")),
        Some(Slot::Project(g)) => Cow::Owned(format!("proj-{}", layout.groups.get(*g).map_or("", |g| g.project.as_str()))),
        _ => Cow::Borrowed("gone"),
    }
}

fn slot_el(s: &Desk, i: usize) -> El {
    let p = pal::pal(s.theme.get());
    let layout = s.sessions.layout.borrow();
    match layout.slots.get(i).copied() {
        Some(Slot::Row(r)) if r < s.browser.len() => session_row(s, r, &layout, &p),
        Some(Slot::Day(date)) => day_header(date, layout.today, layout.headers.get(&date).copied().unwrap_or_default(), &p),
        Some(Slot::Label(label)) => {
            row().w_full().h(ROW).px(PAD + 2.0).center_y().fill(p.bg).child(text(label).key("group-label").font(pal::SMALL_STRONG).color(p.ink2).single_line())
        }
        Some(Slot::Project(g)) => match layout.groups.get(g) {
            Some(group) => project_row(s, group, &layout, &p),
            None => el().h(ROW),
        },
        _ => el().h(ROW),
    }
}

/// A day's header: its name, its date beside the names that are not one, and its count and
/// spend; a click shows the day in Day.
fn day_header(date: i64, today: i64, h: Header, p: &Pal) -> El {
    let label = view::day_label(date, today);
    let noon = rhythm::midnight(date, &rhythm::local_offset) + 12 * 3_600_000;
    let near = today - date < 7;
    let spend = h.spend.map_or_else(|| "…".to_string(), usd);
    let total = format!("{} {} · {spend} spent", int(h.count as u64), if h.count == 1 { "session" } else { "sessions" });
    row()
        .key("day-row")
        .w_full()
        .h(ROW)
        .px(PAD + 2.0)
        .gap(10.0)
        .center_y()
        .fill(p.bg)
        .hover(p.hover)
        .role("button")
        .label(format!("Show {label} in Day"))
        .on_press_tx(move |s: &mut Desk, tx| {
            // That date's lanes with no session selected.
            reports::open_day(s, date, tx);
            s.deselect(tx);
        })
        .child(text(label).key("day-label").font(pal::SMALL_STRONG).color(p.ink2).single_line())
        .children(near.then(|| text(local_time(noon, "%a %-d %b")).key("day-date").font(pal::SMALL).color(p.ink3).single_line()))
        .child(spacer().grow(1.0))
        .child(text(total).key("day-total").font(pal::SMALL_NUM).color(p.ink3).single_line())
}

/// A project's cell: its name as the lanes label it, cut at its end where it does not fit.
fn project_text(project: &str, p: &Pal) -> El {
    t("col-project", view::project_name(project), pal::STRONG, p.ink)
}

/// A text of a cell: in `font` and `color`, cut at its end where it does not fit.
fn t(key: &'static str, s: impl Into<String>, font: rune::TextStyle, color: Color) -> El {
    text(s.into()).key(key).font(font).color(color).single_line().truncate().min_w(0.0)
}

/// A session's row. Its handlers find the session by its id: the row may hold another one by
/// the time it is pressed.
fn session_row(s: &Desk, r: usize, layout: &Layout, p: &Pal) -> El {
    let b = &s.browser;
    let now = now_ms();
    let c = b.cells(r, now);
    let g = b.glance(r);
    let selected = s.current() == Some(r) && s.sessions.cursor.is_none();
    let running = g.is_some_and(|g| g.runs_at(now));
    let pending = |v: Option<String>| v.unwrap_or_else(|| "…".to_string());
    let name = c.title.map_or_else(|| c.project.to_string(), str::to_string);
    let agent = p.agents[c.agent_place % p.agents.len()];
    let start = g.and_then(|gl| gl.started_ms);
    let cells = layout.cols.iter().map(|&(col, w)| {
        let content = match col {
            Col::Last => match b.wait(r, now) {
                Some(wait) => t("col-last", rhythm::duration(wait), pal::SMALL_NUM, p.wait),
                None if running => t("col-last", when(c.active_ms, layout.flat, now), pal::SMALL_STRONG, p.warm),
                None => t("col-last", when(c.active_ms, layout.flat, now), pal::SMALL_NUM, p.ink2),
            },
            Col::Agent => dot(agent, 7.0).key("col-agent"),
            Col::Project => project_text(c.project, p),
            Col::Title => t("col-title", name.clone(), pal::BODY, if c.title.is_some() { p.ink } else { p.ink2 }),
            Col::Started => t("col-started", start.map_or_else(|| "…".to_string(), |st| started(st, c.active_ms, now, &layout.days)), pal::SMALL_NUM, p.ink3),
            Col::Duration => {
                t("col-duration", start.map_or_else(|| "…".to_string(), |st| human_duration((c.active_ms - st).max(0) as f64 / 1000.0)), pal::BODY_NUM, p.ink)
            }
            Col::Requests => t("col-requests", pending(g.map(|g| int(g.requests))), pal::BODY_NUM, p.ink),
            Col::Cost => t("col-cost", c.cost.clone(), pal::STRONG_NUM, p.ink),
            Col::Avoidable => match g {
                None => t("col-avoidable", "…", pal::BODY_NUM, p.ink3),
                Some(g) if g.avoidable >= 0.005 => t("col-avoidable", usd(g.avoidable), pal::BODY_NUM, p.cold),
                Some(_) => t("col-avoidable", "—", pal::BODY_NUM, p.ink3),
            },
            Col::Cold => match g {
                None => t("col-cold", "…", pal::BODY_NUM, p.ink3),
                Some(g) if g.cold_returns > 0 => t("col-cold", int(g.cold_returns), pal::BODY_NUM, p.cold),
                Some(_) => t("col-cold", "—", pal::BODY_NUM, p.ink3),
            },
            Col::Next => view::next_cell(&c, now, p),
            Col::Model => t("col-model", pending(g.map(|g| model_cell(g.model()))), pal::SMALL, p.ink2).pl(MODEL_PAD),
        };
        cell(col, w, content)
    });
    let inner = row()
        .key("cells")
        .w_full()
        .h_full()
        .px(PAD - 6.0)
        .gap(GAP)
        .center_y()
        .radius(R_ROW)
        .fill(if selected { p.sel } else { Color::TRANSPARENT })
        .hover(if selected { p.sel } else { p.hover })
        .children(cells);
    let inner = overlay::ring_at(inner, selected && table_has_keys(s), 1.0, p);
    let (press, open) = (b.session(r).to_string(), b.session(r).to_string());
    row()
        .w_full()
        .h(ROW)
        .px(6.0)
        .py(1.0)
        .role("option")
        .label(format!("{} {} {} {} {}", when(c.active_ms, layout.flat, now), c.agent, c.project, name, c.cost))
        .checked(selected)
        .help_when_truncated(format!("{name}\n{}", c.project))
        .on_press_tx(move |s: &mut Desk, tx| select(s, &press, tx))
        .on_double_press_tx(move |s: &mut Desk, tx| open_in_day(s, &open, tx))
        .child(inner)
}

/// A project's row: its newest session's last activity, its fold, its name, its sessions with
/// its share of the set's cost, and its sums.
fn project_row(s: &Desk, g: &Group, layout: &Layout, p: &Pal) -> El {
    let now = now_ms();
    let open = s.sessions.open.contains(&g.project);
    let on = s.sessions.cursor.as_deref() == Some(g.project.as_str());
    let total: f64 = layout.groups.iter().map(|g| g.cost).sum();
    let max = layout.groups.iter().map(|g| g.cost).fold(0.0, f64::max).max(1e-9);
    let n = g.rows.len();
    let cells = layout.cols.iter().map(|&(col, w)| {
        let content = match col {
            Col::Last => t("col-last", when(g.last_ms, true, now), pal::SMALL_NUM, p.ink2),
            Col::Agent => t("fold", if open { "▾" } else { "▸" }, pal::SMALL, p.ink2),
            Col::Project => project_text(&g.project, p).help(g.project.clone()),
            Col::Title => row()
                .w_full()
                .gap(8.0)
                .center_y()
                .child(t("project-sessions", format!("{} {}", int(n as u64), if n == 1 { "session" } else { "sessions" }), pal::BODY_NUM, p.ink2))
                .child(row().grow(1.0).max_w(160.0).min_w(20.0).child(overlay::meter(g.cost / max, p.bars, p, 6.0).key("project-bar")))
                .child(t("project-share", format!("{} of spend", percent(g.cost / total.max(1e-9))), pal::SMALL_NUM, p.ink2)),
            Col::Cost => t("col-cost", usd(g.cost), pal::STRONG_NUM, p.ink),
            Col::Avoidable if g.avoidable >= 0.005 => t("col-avoidable", usd(g.avoidable), pal::BODY_NUM, p.cold),
            Col::Avoidable => t("col-avoidable", "—", pal::BODY_NUM, p.ink3),
            Col::Cold if g.cold > 0 => t("col-cold", int(g.cold), pal::BODY_NUM, p.cold),
            Col::Cold => t("col-cold", "—", pal::BODY_NUM, p.ink3),
            _ => el(),
        };
        cell(col, w, content)
    });
    let inner = row()
        .key("cells")
        .w_full()
        .h_full()
        .px(PAD - 6.0)
        .gap(GAP)
        .center_y()
        .radius(R_ROW)
        .fill(if on { p.sel } else { p.bar })
        .hover(p.hover)
        .children(cells);
    let inner = overlay::ring_at(inner, on && table_has_keys(s), 1.0, p);
    let project = g.project.clone();
    row()
        .w_full()
        .h(ROW)
        .px(6.0)
        .py(1.0)
        .role("treeitem")
        .label(format!("{} {} sessions {}", g.project, n, usd(g.cost)))
        .checked(open)
        .on_press_tx(move |s: &mut Desk, tx| {
            s.sessions.keys_inspector = false;
            toggle(s, &project, tx);
        })
        .child(inner)
}

/// A share as a whole percent, `<1%` for a share above none that rounds to none.
pub fn percent(share: f64) -> String {
    if share > 0.0 && share < 0.005 { "<1%".to_string() } else { format!("{:.0}%", 100.0 * share) }
}

/// The footer: the count of the filtered set and what keeps it, and the sums of its whole-life
/// costs, avoidable dollars and cold returns, with the avoidable share; `costing <n>…` while some
/// are being costed.
fn footer(s: &Desk, cols: &[(Col, f32)], p: &Pal) -> El {
    let t_ = totals(s);
    let share = if t_.cost > 0.0 { format!("{:.1}%", 100.0 * t_.avoidable / t_.cost) } else { "—".to_string() };
    let mut what = format!("{} {} · {}", int(t_.count as u64), if t_.count == 1 { "session" } else { "sessions" }, kept_by(s));
    if t_.pending > 0 {
        what += &format!(" · costing {}…", int(t_.pending as u64));
    }
    row().key("sessions-foot").w_full().h(FOOT_H).px(PAD).gap(GAP).center_y().fill(p.bar).children(
        cols.iter().filter(|c| !matches!(c.0, Col::Last | Col::Agent | Col::Project)).map(|&(c, w)| {
            let content = match c {
                // `Total` and the set take the room of the columns left of the numbers.
                Col::Title => row().w_full().gap(10.0).center_y().child(text("Total").key("foot-label").font(pal::BODY).color(p.ink3).single_line()).child(t(
                    "foot-title",
                    what.clone(),
                    pal::STRONG,
                    p.ink,
                )),
                Col::Cost => t("foot-cost", usd(t_.cost), pal::STRONG_NUM, p.ink),
                Col::Avoidable => t("foot-avoidable", usd(t_.avoidable), pal::STRONG_NUM, if t_.avoidable >= 0.005 { p.cold } else { p.ink3 }),
                Col::Cold => t("foot-cold", int(t_.cold), pal::STRONG_NUM, if t_.cold > 0 { p.cold } else { p.ink3 }),
                Col::Next => t("foot-share", share.clone(), pal::BODY_NUM, p.ink2).help("the avoidable share of the cost"),
                Col::Model => t("foot-model", "avoidable share", pal::SMALL, p.ink3).pl(MODEL_PAD),
                _ => el(),
            };
            cell(c, w, content)
        }),
    )
}

/// The sums of the filtered set.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Totals {
    pub count: usize,
    pub cost: f64,
    pub avoidable: f64,
    pub cold: u64,
    /// The sessions still being costed, whose numbers are not in the sums yet.
    pub pending: usize,
}

/// The sums of every session the filters keep, on the screen or not.
pub fn totals(s: &Desk) -> Totals {
    let b = &s.browser;
    let mut t = Totals { count: b.len(), ..Totals::default() };
    for r in 0..b.len() {
        match b.glance(r) {
            Some(g) => {
                t.cost += g.cost;
                t.avoidable += g.avoidable;
                t.cold += g.cold_returns;
            }
            None => t.pending += 1,
        }
    }
    t
}

/// What keeps the set: `all time`, the pills' names, the project, and the bar's filter.
pub fn kept_by(s: &Desk) -> String {
    let b = &s.browser;
    let q = b.quick_filters();
    let mut parts: Vec<String> = Vec::new();
    if q.running {
        parts.push("running now".into());
    }
    if q.warm {
        parts.push("warm".into());
    }
    if q.this_week {
        parts.push("this week".into());
    }
    if !q.agents.is_empty() {
        parts.push(q.agents.join(" or "));
    }
    if let Some(p) = &q.project {
        parts.push(p.clone());
    }
    if !b.filter().is_empty() {
        parts.push(format!("“{}”", b.filter()));
    }
    if parts.is_empty() { "all time".to_string() } else { parts.join(" · ") }
}

/// The inspector: `Open in Day`, `Copy handoff line` and close on top, the selected session's
/// Summary in one column under them.
fn inspector(s: &Desk, cx: &Cx<Desk>, p: &Pal) -> El {
    let keys = s.sessions.keys_inspector && !s.browser.editing();
    let open = row()
        .key("open-in-day")
        .h(28.0)
        .pl(12.0)
        .pr(5.0)
        .gap(6.0)
        .center_y()
        .radius(PILL)
        .fill(p.accent)
        .hover(p.accent.alpha(0.88))
        .role("button")
        .label("Open in Day")
        .on_press_tx(|s: &mut Desk, tx| {
            if let Some(r) = s.current() {
                let id = s.browser.session(r).to_string();
                open_in_day(s, &id, tx);
            }
        })
        .child(text("Open in Day").font(pal::STRONG).color(p.on_accent).single_line())
        .child(overlay::keycap_on("⏎", p.accent, p));
    let handoff = view::button("handoff", "Copy handoff line", true, p.bar, p).pr(5.0).child(overlay::keycap_on("c", p.panel, p)).on_press(|s: &mut Desk| {
        let view = s.viewport();
        s.browser.on_key(Key::Char('c'), view);
    });
    let close = row()
        .key("inspector-close")
        .h(24.0)
        .px(4.0)
        .center()
        .radius(PILL)
        .hover(p.hover)
        .role("button")
        .label("Close the inspector")
        .on_press(|s: &mut Desk| {
            s.sessions.inspector = false;
            s.sessions.keys_inspector = false;
        })
        .child(overlay::keycap_on("esc", p.panel, p));
    let head =
        row().key("inspector-head").w_full().h(view::HEAD_H).px(12.0).gap(8.0).center_y().child(open).child(handoff).child(spacer().grow(1.0)).child(close);
    let body = match s.browser.xray() {
        Some(x) => crate::inspector::summary_column(s, x, cx, p),
        None => vec![match s.browser.report() {
            tokprof_browse::browser::Pane::Text(t) => text(t.text.clone()).w_full().wrap().font(pal::BODY).color(p.ink2),
            tokprof_browse::browser::Pane::Computing(n) => text(n).key("computing").w_full().wrap().font(pal::BODY).color(p.ink2),
            tokprof_browse::browser::Pane::Empty(n) => text(n).w_full().wrap().font(pal::BODY).color(p.ink2),
        }],
    };
    let offset = s.sessions.pane_offset.clone();
    let pane = col()
        .key(PANE)
        .w_full()
        .grow(1.0)
        .min_h(0.0)
        .scroll(ScrollSpec { stick_bottom: false })
        .on_scroll_offset(move |at| offset.set(at.offset[1]))
        .child(col().key("sessions-pane-body").w_full().px(14.0).pt(12.0).pb(24.0).gap(12.0).children(body));
    let pane = overlay::ring_at(pane, keys, -2.0, p);
    col()
        .key("sessions-inspector")
        .w(INSPECTOR_W)
        .h_full()
        .min_h(0.0)
        .on_press(|s: &mut Desk| s.sessions.keys_inspector = true)
        .child(head)
        .child(el().w_full().h(1.0).fill(p.line))
        .child(pane)
}

/// Selects a session's row, as a click does: the inspector opens on its Summary from the top, and
/// the table has the keyboard.
pub fn select(s: &mut Desk, id: &str, tx: &mut Tx) {
    s.select_session(id);
    let st = &mut s.sessions;
    st.cursor = None;
    st.inspector = true;
    st.keys_inspector = false;
    tx.scroll_to_y(PANE, 0.0);
}

/// Selects a listed session's row as the palette does while Sessions shows: its project unfolded
/// in Projects mode, the row scrolled to, and its Summary in the inspector.
pub fn pick(s: &mut Desk, id: &str, tx: &mut Tx) {
    let Some(r) = s.browser.row_of(id) else { return };
    if s.sessions.projects {
        let project = s.browser.cells(r, now_ms()).project.to_string();
        s.sessions.open.insert(project);
    }
    select(s, id, tx);
    tx.scroll_into_view(id.to_string());
}

/// After the order changed, as a header click changes it: the table from its top, with the
/// cursor's row on the screen.
pub fn sorted(s: &Desk, tx: &mut Tx) {
    tx.scroll_to_y(TABLE, 0.0);
    reveal(s, tx);
}

/// Shows Day on the day the session's row is listed under, with that session selected and its
/// Summary from the top (desk-sessions, Opening a session in Day).
pub fn open_in_day(s: &mut Desk, id: &str, tx: &mut Tx) {
    // The day of its last activity as the table's headers have it, taken before the store or the
    // list can change.
    let date = s.browser.row_of(id).map(|r| days(s).date_of(s.browser.active_ms(r)));
    reports::show(s, Mode::Day);
    if s.browser.zoomed() {
        s.browser.on_key(Key::Backspace, s.viewport());
    }
    if s.pick(id, tx) {
        s.pane = crate::Focus::Lanes;
        // That day even when the day shown has its lane; without a lane on it, the lanes' header
        // says so (see `day::no_lane`).
        match date {
            Some(date) => s.browser.show_day(date),
            None => s.browser.show_selected_day(),
        }
    }
}

/// Lays the rows out again for the list as it is now, keeping the columns the view fitted.
fn relayout(s: &Desk) {
    let cols = std::mem::take(&mut s.sessions.layout.borrow_mut().cols);
    let fresh = layout(s);
    *s.sessions.layout.borrow_mut() = Layout { cols, ..fresh };
}

/// The slots the cursor stops at: the sessions' rows and the projects' rows.
fn stops(layout: &Layout) -> Vec<usize> {
    layout.slots.iter().enumerate().filter(|(_, s)| matches!(s, Slot::Row(_) | Slot::Project(_))).map(|(i, _)| i).collect()
}

/// The slot the cursor is on.
fn cursor_slot(s: &Desk, layout: &Layout) -> Option<usize> {
    match &s.sessions.cursor {
        Some(p) => layout.slots.iter().position(|x| matches!(x, Slot::Project(g) if layout.groups.get(*g).is_some_and(|g| g.project == *p))),
        None => {
            let r = s.current()?;
            layout.slots.iter().position(|x| *x == Slot::Row(r))
        }
    }
}

/// Moves the cursor to the stop `step` stops from where it is, past the days' headers.
fn move_by(s: &mut Desk, step: isize, tx: &mut Tx) {
    relayout(s);
    let target = {
        let layout = s.sessions.layout.borrow();
        let stops = stops(&layout);
        if stops.is_empty() {
            return;
        }
        let at = cursor_slot(s, &layout).and_then(|slot| stops.iter().position(|&x| x == slot));
        let to = match at {
            Some(i) => (i as isize).saturating_add(step).clamp(0, stops.len() as isize - 1) as usize,
            None if step < 0 => stops.len() - 1,
            None => 0,
        };
        layout.slots[stops[to]]
    };
    go_to(s, target, tx);
}

/// Puts the cursor on a slot's row and shows it.
fn go_to(s: &mut Desk, slot: Slot, tx: &mut Tx) {
    match slot {
        Slot::Row(r) => {
            let id = s.browser.session(r).to_string();
            select(s, &id, tx);
            tx.scroll_into_view(id);
        }
        Slot::Project(g) => {
            let project = s.sessions.layout.borrow().groups.get(g).map(|g| g.project.clone());
            if let Some(project) = project {
                tx.scroll_into_view(format!("proj-{project}"));
                s.sessions.cursor = Some(project);
            }
        }
        _ => {}
    }
}

/// The project the cursor is on or in.
fn cursor_project(s: &Desk) -> Option<String> {
    s.sessions.cursor.clone().or_else(|| s.current().map(|r| s.browser.cells(r, now_ms()).project.to_string()))
}

/// The rows a page moves by.
fn page(s: &Desk) -> isize {
    ((s.sessions.height / ROW) as isize - 1).max(1)
}

/// The keys of Sessions. A filter being typed takes every key; the table, or the inspector, has
/// the others.
pub fn on_key(s: &mut Desk, e: &KeyEvent, tx: &mut Tx) -> bool {
    let Some(key) = crate::key_of(e) else {
        // Tab between the table and the inspector.
        if e.key == RuneKey::Tab && !s.browser.editing() && !e.mods.cmd && !e.mods.ctrl {
            if inspector_shown(s) {
                s.sessions.keys_inspector = !s.sessions.keys_inspector;
            }
            return true;
        }
        return false;
    };
    if s.browser.editing() || key == Key::Interrupt {
        let go_on = crate::browser_key(s, key, tx);
        if !go_on {
            tx.quit();
        }
        after_filter(s, tx);
        return true;
    }
    if !plain(e) {
        return false;
    }
    let inspector = s.sessions.keys_inspector && inspector_shown(s);
    match key {
        Key::Esc if inspector => s.sessions.keys_inspector = false,
        Key::Esc if inspector_shown(s) => s.sessions.inspector = false,
        Key::Esc if !s.browser.filter().is_empty() => {
            s.browser.clear_filter();
            after_filter(s, tx);
        }
        Key::Esc => {}
        Key::Up | Key::Char('k') if inspector => tx.scroll_to_y(PANE, (s.sessions.pane_offset.get() - 20.0).max(0.0)),
        Key::Down | Key::Char('j') if inspector => tx.scroll_to_y(PANE, s.sessions.pane_offset.get() + 20.0),
        Key::PageUp if inspector => tx.scroll_page(PANE, -1),
        Key::PageDown | Key::Char(' ') if inspector => tx.scroll_page(PANE, 1),
        Key::Home | Key::Char('g') if inspector => tx.scroll_to_y(PANE, 0.0),
        Key::End | Key::Char('G') if inspector => tx.scroll_page(PANE, 1000),
        Key::Up | Key::Char('k') => move_by(s, -1, tx),
        Key::Down | Key::Char('j') => move_by(s, 1, tx),
        Key::PageUp => move_by(s, -page(s), tx),
        Key::PageDown => move_by(s, page(s), tx),
        Key::Home | Key::Char('g') => move_by(s, isize::MIN / 2, tx),
        Key::End | Key::Char('G') => move_by(s, isize::MAX / 2, tx),
        Key::Enter => match &s.sessions.cursor {
            Some(p) => {
                let p = p.clone();
                toggle(s, &p, tx);
            }
            None => {
                if let Some(r) = s.current() {
                    let id = s.browser.session(r).to_string();
                    open_in_day(s, &id, tx);
                }
            }
        },
        Key::Right | Key::Char('l') if s.sessions.projects => {
            if let Some(p) = s.sessions.cursor.clone().filter(|p| !s.sessions.open.contains(p)) {
                toggle(s, &p, tx);
            }
        }
        Key::Left | Key::Char('h') if s.sessions.projects => {
            if let Some(p) = cursor_project(s) {
                s.sessions.open.remove(&p);
                tx.scroll_into_view(format!("proj-{p}"));
                s.sessions.cursor = Some(p);
            }
        }
        Key::Char('t') => today(s, tx),
        Key::Char('s' | 'r') => {
            crate::browser_key(s, key, tx);
            tx.scroll_to_y(TABLE, 0.0);
            reveal(s, tx);
        }
        Key::Char('/' | 'f' | 'o' | 'c') => {
            crate::browser_key(s, key, tx);
        }
        Key::Char('q') => tx.quit(),
        _ => {}
    }
    true
}

/// The row of the first session in slot `slot` or after it.
pub fn row_at(s: &Desk, slot: usize) -> Option<usize> {
    let layout = s.sessions.layout.borrow();
    layout.slots.iter().skip(slot).find_map(|x| if let Slot::Row(r) = x { Some(*r) } else { None })
}

/// Tells the browser which row is first on the screen, as the table is scrolled.
pub fn sync_top(s: &mut Desk) {
    let slot = (s.sessions.offset.get() / ROW).max(0.0) as usize;
    let top = row_at(s, slot);
    s.sessions.separator_on_top = matches!(s.sessions.layout.borrow().slots.get(slot), Some(Slot::Day(_) | Slot::Label(_)));
    if let Some(top) = top {
        s.browser.set_top(top);
    }
}

/// Scrolls the table so that its first row on the screen is the browser's, keeping how far into
/// that row it was scrolled and the header above it, as the list in Day does.
pub fn restore(s: &Desk, tx: &mut Tx) {
    relayout(s);
    let layout = s.sessions.layout.borrow();
    let into = s.sessions.offset.get().rem_euclid(ROW);
    let top = s.browser.top();
    let Some(mut slot) = layout.slots.iter().position(|x| *x == Slot::Row(top)) else { return };
    if s.sessions.separator_on_top && slot > 0 && matches!(layout.slots.get(slot - 1), Some(Slot::Day(_) | Slot::Label(_))) {
        slot -= 1;
    }
    tx.scroll_to_y(TABLE, slot as f32 * ROW + into);
}

/// Whether a warm row on the screen counts down at `now`.
pub fn counting(s: &Desk, now: i64) -> bool {
    let layout = s.sessions.layout.borrow();
    let first = (s.sessions.offset.get() / ROW).max(0.0) as usize;
    let shown = (s.sessions.height / ROW) as usize + 2;
    layout.slots.iter().skip(first).take(shown).any(|x| matches!(x, Slot::Row(r) if *r < s.browser.len() && s.browser.cells(*r, now).warm_until.is_some()))
}

/// Waits up to `limit` for every listed session to be costed, for a frame without a window.
pub fn settle_costs(s: &mut Desk, limit: std::time::Duration) {
    let deadline = std::time::Instant::now() + limit;
    loop {
        s.browser.tick();
        if s.browser.known_total().1 == 0 || std::time::Instant::now() >= deadline {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
}

/// While Sessions or Day shows: takes in the day store the calendar's worker brings up to date,
/// for the days' headers and the day summary, and asks for it again when the logs changed and its
/// pause passed; on a past day in Day, the logs changing asks for nothing. Returns whether it
/// arrived.
pub fn poll(s: &mut Desk, tx: &mut Tx) -> bool {
    let changes = match s.reports.mode {
        Mode::Sessions => s.browser.changes(),
        Mode::Day if s.browser.day_is_latest() => s.browser.changes(),
        Mode::Day => s.calendar.data.get().map_or(s.browser.changes(), |c| c.changes),
        _ => return false,
    };
    let stored = s.calendar.data.tick(changes);
    if let Some(at) = s.calendar.data.next_due(changes) {
        tx.wake_after(at.saturating_duration_since(std::time::Instant::now()).max(std::time::Duration::from_millis(1)));
    }
    // While the store is being filled, the Spend card and the top sessions of Day count
    // the logs read: a frame when the count moves, none at rest.
    let progress = s.calendar.data.progress();
    let moved = progress != s.calendar.seen;
    s.calendar.seen = progress;
    stored || moved
}

/// A scene's Sessions: shown, every listed session costed and the day store read, waiting up to
/// 120 s for each.
pub fn scene(s: &mut Desk) {
    reports::show(s, Mode::Sessions);
    if s.browser.costs() {
        settle_costs(s, std::time::Duration::from_secs(120));
    }
    if s.calendar.data.computes() {
        let changes = s.browser.changes();
        s.calendar.data.settle(changes, std::time::Duration::from_secs(120));
    }
}

/// The scene of Projects: every project folded, the cursor on the costliest.
pub fn scene_projects(s: &mut Desk) {
    s.sessions.projects = true;
    s.sessions.open.clear();
    scene(s);
    s.sessions.cursor = layout(s).groups.first().map(|g| g.project.clone());
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Local time on a date of 2026, as `YYYY-MM-DD`, at `h:m`.
    fn at(date: &str, h: i64, m: i64) -> i64 {
        tokprof_browse::filters::parse_date(date).unwrap() + h * 3_600_000 + m * 60_000
    }

    #[test]
    fn times_carry_their_day_in_one_list_and_their_year_when_it_is_not_this_one() {
        let now = at("2026-11-12", 14, 20);
        assert_eq!(when(at("2026-11-11", 23, 12), false, now), "23:12", "under a day's header");
        assert_eq!(when(at("2026-11-11", 23, 12), true, now), "11 Nov 23:12", "in one list");
        assert_eq!(when(at("2025-12-30", 9, 5), true, now), "30 Dec 2025 09:05");
    }

    #[test]
    fn a_long_session_starts_on_its_own_day() {
        let now = at("2026-11-12", 14, 20);
        let (start, last) = (at("2026-11-09", 15, 20), at("2026-11-11", 22, 48));
        let days = Days::default();
        assert_eq!(started(start, last, now, &days), "9 Nov 15:20");
        assert_eq!(started(at("2026-11-11", 8, 0), last, now, &days), "08:00", "on the day of the last activity");
        assert_eq!(started(at("2026-11-10", 23, 0), at("2026-11-11", 2, 0), now, &days), "23:00", "02:00 belongs to the day before");
        assert_eq!(human_duration((last - start) as f64 / 1000.0), "2.3d", "session-xray's span of 55.5 hours");
    }

    #[test]
    fn the_model_cell_names_the_others_by_count() {
        assert_eq!(model_cell(Some(("claude-opus-5-5", 1))), "claude-opus-5-5 +1");
        assert_eq!(model_cell(Some(("gpt-5.6-codex", 0))), "gpt-5.6-codex");
        assert_eq!(model_cell(None), "—");
    }

    #[test]
    fn narrow_tables_leave_out_model_then_started_then_requests() {
        let names = |w: f32, narrow: bool| fit(w, false, false, narrow).into_iter().map(|(c, _)| c).collect::<Vec<_>>();
        assert_eq!(names(1400.0 - 361.0, true).len(), 12, "every column at 1400 points with the inspector");
        let at_960 = names(960.0 - 361.0, true);
        for c in Col::DROPPED {
            assert!(!at_960.contains(&c), "{c:?} is left out at 960 points");
        }
        assert!(at_960.contains(&Col::Avoidable) && at_960.contains(&Col::Title));
        let project = |w: f32, narrow: bool| fit(w, false, false, narrow).into_iter().find(|c| c.0 == Col::Project).unwrap().1;
        let room = pal::text_w("temporary folders", pal::STRONG);
        assert!(project(1400.0 - 361.0, true) >= room && project(1400.0, false) >= room, "`temporary folders` fits at 1400 points");
        assert!(project(960.0 - 361.0, true) < room, "a narrow window cuts it at its end");
        let mid = names(850.0, true);
        assert!(!mid.contains(&Col::Model) && mid.contains(&Col::Requests), "the model goes first: {mid:?}");
        let cols = fit(2000.0, false, false, false);
        let project = cols.iter().find(|c| c.0 == Col::Project).unwrap().1;
        assert_eq!(project, Col::Project.width(false, false, false) + PROJECT_MORE, "spare room goes to the project, up to a limit");
    }

    #[test]
    fn shares_round_to_whole_percents() {
        assert_eq!(percent(0.461), "46%");
        assert_eq!(percent(0.001), "<1%");
        assert_eq!(percent(0.0), "0%");
    }

    #[test]
    fn every_column_sorts_by_its_own_order() {
        assert_eq!(Col::ALL.map(Col::sort), Sort::COLUMNS);
    }
}
