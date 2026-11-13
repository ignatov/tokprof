//! What the window draws: its own title bar with the day's controls, and in Day the lanes of the
//! day across the window's width, the border that moves, and the details pane under them: the
//! day summary, or the inspector of the selected session (desk-window, Day layout A).

use rune::{Color, Cursor, Cx, Drag, El, Phase, col, el, row, spacer, text};
use tokprof_browse::browser::{Key, now_ms};
use tokprof_core::fmt::{local_time, usd};
use tokprof_core::rhythm;

use crate::Desk;
use crate::overlay::{self, PILL};
use crate::pal::{self, Pal};

/// The height of the title bar.
pub const BAR_H: f32 = 46.0;
/// The height of the heads of the panes, so that the lines under them meet.
pub const HEAD_H: f32 = 44.0;
/// The key of the details pane's scroll viewport, the inspector's or the day summary's.
pub const PANE: &str = "pane";
/// The height of the border between the lanes and the details, which a drag moves.
pub const BORDER_H: f32 = 6.0;
/// The least room the details pane keeps under a dragged border, and the shares of the height
/// under the bar the lanes take at least and at most.
pub const DETAILS_MIN: f32 = 200.0;
pub const SHARE_MIN: f32 = 0.25;
pub const SHARE_MAX: f32 = 0.75;
/// The size, in characters, the browser lays the day's text out for; the window draws the lanes
/// itself and keeps only the text around them.
pub const DAY_PANE: (u16, u16) = (200, 400);
/// The share of the window's height the lanes may take.
const LANES_SHARE: f32 = 0.45;
/// How long the bar shows a status message.
pub const STATUS_SECS: u64 = 6;

pub fn view(s: &Desk, cx: &mut Cx<Desk>) -> El {
    let theme = s.settings.theme(cx.system_dark());
    s.theme.set(theme);
    let p = pal::pal(theme);
    let height = window_size(s, cx).1;
    // The image of a month in review: the card alone, filling the frame.
    if s.month.alone
        && let Some(card) = &s.month.card
    {
        return col().key("root").w_full().h_full().fill(p.bg).child(crate::month::card_el(&card.month, &p, false));
    }
    let mut root = col()
        .key("root")
        .w_full()
        .h_full()
        .fill(p.bg)
        .on_layout(|s: &mut Desk, r| s.window = (r[2], r[3]))
        .child(bar(s, cx, &p))
        .child(el().w_full().h(1.0).fill(p.line));
    // The lanes and the inspector change when what they show does, and with the minute; the
    // list's countdowns tick every second, so the view runs every second while one is on the
    // screen and builds the two again only when they changed.
    let inspector = |cx: &Cx<Desk>| rune::memo("inspector", crate::inspector::version(s, cx, &theme.id), || crate::inspector::inspector(s, cx, &p));
    if s.reports.mode == crate::reports::Mode::Week {
        root = root.child(crate::week::view(s, cx, &p));
    } else if s.reports.mode.period() {
        root = root.child(crate::reports::view(s, cx, &p));
    } else if s.reports.mode == crate::reports::Mode::Sessions {
        root = root.child(crate::sessions::view(s, cx, &p));
    } else if s.reports.mode == crate::reports::Mode::Calendar {
        root = root.child(crate::calendar::view(s, cx, &p));
    } else if s.browser.zoomed() && s.current().is_some() {
        root = root.child(inspector(cx));
    } else {
        // The lanes take the room Room for the lanes gives them, up to it, or the dragged
        // border's place exactly.
        let fixed = s.lanes_share.map(|share| border_h(share, height - BAR_H - 1.0));
        let max_h = fixed.unwrap_or_else(|| lanes_max_h(height));
        let lanes = rune::memo("lanes", crate::day::version(s, cx, max_h, &theme.id), || crate::day::lanes(s, cx, &p, max_h));
        let lanes = col().key("lanes-block").w_full().fill(p.track).on_layout(|s: &mut Desk, r| s.lanes_h = r[3]).child(lanes);
        let lanes = match fixed {
            Some(h) => lanes.h(h),
            None => lanes,
        };
        let details = match s.current() {
            Some(_) => inspector(cx),
            None => rune::memo("day-summary", crate::details::version(s, cx, &theme.id), || crate::details::summary(s, cx, &p)),
        };
        let details = col().key("details").w_full().grow(1.0).min_h(0.0).child(details);
        root = root.child(lanes).child(border(&p)).child(details);
    }
    if let Some(panel) = s.panel {
        root = root.child(crate::settings::panel(s, panel, &p, height));
    }
    if let Some(palette) = &s.palette {
        root = root.child(crate::palette::view(s, palette, &p));
    }
    if s.help {
        root = root.child(crate::keys::view(&p, height));
    }
    if let Some(card) =
        s.month.card.as_ref().filter(|_| matches!(s.reports.mode, crate::reports::Mode::Calendar | crate::reports::Mode::Month | crate::reports::Mode::Quarter))
    {
        root = root.child(crate::month::view(card, &p));
    }
    root
}

/// The window's height: as the root was last laid out, else as the window was opened (`--size`
/// in a frame without a window).
pub fn window_size(s: &Desk, cx: &Cx<Desk>) -> (f32, f32) {
    match cx.layout_size("root").filter(|sz| sz[1] > 0.0) {
        Some(sz) => (sz[0], sz[1]),
        None if s.window.1 > 0.0 => s.window,
        None => WINDOW,
    }
}

/// The height of the lanes, their header and what is under them, for a border dragged to `share`
/// of the `room` under the bar: from 25% to 75% of it, the details pane keeping 200 points.
pub fn border_h(share: f32, room: f32) -> f32 {
    (share.clamp(SHARE_MIN, SHARE_MAX) * room).min(room - DETAILS_MIN - BORDER_H).max(0.0)
}

/// The border between the lanes and the details: a drag moves it and saves its place when it
/// ends, a double click gives back the room Room for the lanes gives.
fn border(p: &Pal) -> El {
    row()
        .key("border")
        .w_full()
        .h(BORDER_H)
        .center()
        .fill(p.bg)
        .hover(p.hover)
        .cursor(Cursor::ResizeRow)
        .role("separator")
        .label("The border between the lanes and the details")
        .on_drag(|s: &mut Desk, d: &mut Drag, _| drag_border(s, d))
        .on_double_press(|s: &mut Desk| s.save_lanes_share(None))
        .child(el().key("border-grip").w(36.0).h(3.0).radius(1.5).fill(p.line))
}

/// A press, drag or release on the border: the lanes take the height they had and what the
/// pointer moved, within the places the drag allows; the place is saved when the drag ends.
pub fn drag_border(s: &mut Desk, d: &Drag) {
    let room = s.window.1 - BAR_H - 1.0;
    match d.phase {
        Phase::Down => s.border_from = Some(s.lanes_h),
        Phase::Move => {
            if let Some(from) = s.border_from
                && room > 0.0
                && d.dy != 0.0
            {
                let h = border_h((from + d.dy) / room, room);
                s.lanes_share = Some(h / room);
            }
        }
        Phase::Up => {
            let Some(from) = s.border_from.take() else { return };
            if d.cancelled() {
                s.lanes_share = crate::settings::lanes_share(&s.prefs);
                return;
            }
            if d.dy != 0.0 && room > 0.0 {
                let share = border_h((from + d.dy) / room, room) / room;
                s.save_lanes_share(Some(share));
            }
        }
    }
}

/// The most the lanes take with what is under them, in a window `height` points high.
pub fn lanes_max_h(height: f32) -> f32 {
    // 45% of the room under the bar, or more, up to 55%, to show 8 session lanes at 900 points
    // high and 3 at 640, and in between as many as a straight line between them gives.
    let room = height - BAR_H - 1.0;
    let sessions = (3.0 + (height - 640.0) / 260.0 * 5.0).floor().clamp(3.0, 8.0) as usize;
    (room * LANES_SHARE).max(crate::day::height_for(sessions)).min(room * 0.55)
}

/// The window's size when it opens, in points.
pub const WINDOW: (f32, f32) = (1400.0, 900.0);

/// A hash of what a part of the view shows, for `memo`.
pub fn hash_of(parts: impl std::hash::Hash) -> u64 {
    use std::hash::{BuildHasher, BuildHasherDefault};
    BuildHasherDefault::<std::collections::hash_map::DefaultHasher>::default().hash_one(parts)
}

/// The minute of now, which the parts that show time are drawn again at.
pub fn minute() -> i64 {
    now_ms() / 60_000
}

/// A control in the bar: a label in a pill that does something when pressed, on `fill`.
pub fn button(key: &'static str, label: &str, enabled: bool, fill: Color, p: &Pal) -> El {
    row()
        .key(key)
        .h(28.0)
        .px(12.0)
        .gap(6.0)
        .center_y()
        .radius(PILL)
        .fill(fill)
        .hover(p.hover)
        .role("button")
        .label(label.to_string())
        .disabled(!enabled)
        .child(text(label.to_string()).font(pal::STRONG).color(if enabled { p.ink } else { p.ink3 }).single_line())
}

/// A step in the bar: a sign in a circle.
pub fn step_button(key: &'static str, sign: &str, label: &str, enabled: bool, p: &Pal) -> El {
    row()
        .key(key)
        .w(28.0)
        .h(28.0)
        .center()
        .radius(PILL)
        .fill(p.panel)
        .hover(p.hover)
        .role("button")
        .label(label.to_string())
        .disabled(!enabled)
        .child(text(sign.to_string()).font(pal::DAY).color(if enabled { p.ink } else { p.ink3 }).single_line())
}

/// One segment of a segmented control: a pill, the chosen one in the accent.
pub fn segment(key: String, label: &str, on: bool, p: &Pal) -> El {
    row()
        .key(key)
        .h(24.0)
        .px(12.0)
        .center_y()
        .radius(PILL)
        .fill(if on { p.accent } else { Color::TRANSPARENT })
        .hover(if on { p.accent } else { p.hover })
        .role("radio")
        .checked(on)
        .label(label.to_string())
        .child(text(label.to_string()).font(pal::STRONG).color(if on { p.on_accent } else { p.ink2 }).single_line())
}

/// Segments in a pill of `fill`: the bar's on the raised surface, the others on the bar's color.
pub fn segments(key: &'static str, parts: Vec<El>, fill: Color) -> El {
    row().key(key).h(28.0).px(2.0).gap(2.0).center_y().radius(PILL).fill(fill).role("radiogroup").children(parts)
}

/// A dot of a color, as a status or an agent shows.
pub fn dot(c: Color, size: f32) -> El {
    el().w(size).h(size).radius(size / 2.0).fill(c)
}

/// The window's own title bar: the room for the window's buttons, the switch and `Sessions`, the
/// day's name with the steps to the days before and after, Today, the status, the filter and the
/// live mark. Dragging it where it has no control moves the window.
fn bar(s: &Desk, cx: &Cx<Desk>, p: &Pal) -> El {
    let b = &s.browser;
    let inset = cx.titlebar_inset();
    let latest = b.day_is_latest();
    let mut bar = row()
        .key("bar")
        .w_full()
        .h(BAR_H)
        .fill(p.bar)
        .window_drag()
        .pl(if inset > 0.0 { inset + 16.0 } else { 16.0 })
        .pr(14.0)
        .gap(10.0)
        .center_y()
        .child(crate::reports::switch(s, p))
        .child(crate::sessions::bar_button(s, p));
    // Calendar has no day and no filter: the bar keeps the switch, the status and the live mark.
    let day = s.reports.mode == crate::reports::Mode::Day;
    let sessions = s.reports.mode == crate::reports::Mode::Sessions;
    let week = s.reports.mode == crate::reports::Mode::Week;
    let period = s.reports.mode.period();
    if sessions {
        bar = bar.children(crate::sessions::bar_controls(s, p));
    }
    if week {
        bar = bar.children(crate::week::bar_controls(s, cx, p));
    }
    if period {
        bar = bar.children(crate::reports::bar_controls(s, cx, p));
    }
    if day {
        bar = bar
            .child(step_button("day-back", "‹", "The day before", true, p).on_press(|s: &mut Desk| s.step_day(-1)))
            .child(text(local_time(b.day_noon(), "%A, %-d %B")).key("day-name").font(pal::DAY).color(p.ink).single_line())
            .child(step_button("day-on", "›", "The day after", !latest, p).on_press(|s: &mut Desk| s.step_day(1)))
            .child(button("today", "Today", !latest, p.panel, p).on_press(|s: &mut Desk| s.show_latest()));
    }
    bar = bar.child(spacer().grow(1.0));
    if s.status_shown() {
        bar = bar.child(text(s.status.0.clone()).key("status").font(pal::BODY).color(p.ink2).single_line().truncate().shrink().min_w(0.0));
    }
    if day || sessions || week || period {
        bar = bar.child(filter_field(s, p));
    }
    if s.live {
        bar = bar
            .child(row().key("live").gap(5.0).center_y().child(dot(p.warm_dot, 7.0)).child(text("Live").font(pal::SMALL_STRONG).color(p.warm).single_line()));
    }
    bar
}

/// The filter: what is typed, or what it filters while nothing is; a click starts typing.
fn filter_field(s: &Desk, p: &Pal) -> El {
    let b = &s.browser;
    let editing = b.editing();
    let empty = b.filter().is_empty();
    let mut field = row()
        .key("filter")
        .w(210.0)
        .shrink()
        .min_w(150.0)
        .h(28.0)
        .pl(12.0)
        .pr(5.0)
        .gap(1.0)
        .center_y()
        .radius(PILL)
        .fill(p.panel)
        .role("searchbox")
        .label("Filter")
        .on_press(|s: &mut Desk| {
            if !s.browser.editing() {
                let view = s.viewport();
                s.browser.on_key(Key::Char('/'), view);
            }
        });
    field = overlay::ring(field, editing, p);
    // The placeholder, or what is typed; while typing, it ends at the caret, its start cut when long.
    if !(empty && editing) {
        // The filter of Week, Month and Quarter has no titles: the day store holds none.
        let store = s.reports.mode == crate::reports::Mode::Week || s.reports.mode.period();
        let hint = if store { "Filter by agent, project or id" } else { "Filter by project, id or title" };
        let (shown, color) = if empty { (hint, p.ink3) } else { (b.filter(), p.ink) };
        let t = text(shown.to_string()).key("filter-text").font(pal::BODY).color(color).single_line().min_w(0.0);
        field = field.child(if editing { t.truncate_head().shrink() } else { t.truncate().grow(1.0) });
    }
    if editing {
        field = field.child(el().key("caret").w(1.5).h(15.0).radius(0.75).fill(p.ink)).child(spacer().grow(1.0));
    }
    if empty && !editing {
        field = field.child(overlay::keycap_on("/", p.bar, p).key("filter-key"));
    }
    if !empty {
        let clear = row()
            .key("filter-clear")
            .w(18.0)
            .h(18.0)
            .radius(9.0)
            .center()
            .fill(p.hover)
            .role("button")
            .label("Clear the filter")
            .on_press(|s: &mut Desk| s.browser.clear_filter())
            .child(text("×").font(pal::SMALL_STRONG).color(p.ink2).single_line());
        field = field.child(clear);
    }
    field
}

/// A project's name as the lanes label it: the last folder of its path (`app` of `~/src/app`), or
/// a group's name as it is (`temporary folders`). The full path is in the inspector.
pub fn project_name(project: &str) -> &str {
    project.trim_end_matches(['/', '\\']).rsplit(['/', '\\']).next().filter(|n| !n.is_empty()).unwrap_or(project)
}

/// The names of a list of projects, as the project picker shows them: each by its name (see
/// [`project_name`]), by its path where two projects of the list have one name.
pub struct ProjectNames(std::collections::HashMap<String, usize>);

impl ProjectNames {
    pub fn new<'a>(all: impl IntoIterator<Item = &'a str>) -> Self {
        let mut names = std::collections::HashMap::new();
        let mut seen = std::collections::HashSet::new();
        for p in all {
            if seen.insert(p) {
                *names.entry(project_name(p).to_string()).or_default() += 1;
            }
        }
        Self(names)
    }

    /// The name `project` is shown by: its path when it shares its name, else its name.
    pub fn of(&self, project: &str) -> String {
        let name = project_name(project);
        if self.0.get(name).copied().unwrap_or(0) > 1 { project.to_string() } else { name.to_string() }
    }
}

/// A day's name in the list: Today, Yesterday, the weekday within the week, the date before.
pub fn day_label(date: i64, today: i64) -> String {
    let noon = rhythm::midnight(date, &rhythm::local_offset) + 12 * 3_600_000;
    match today - date {
        0 => "Today".to_string(),
        1 => "Yesterday".to_string(),
        2..=6 => local_time(noon, "%A"),
        _ => local_time(noon, "%a %-d %b %Y"),
    }
}

/// The time a cache stays warm, as the list counts it down: `24:47`, `1:02:03` past an hour.
pub fn countdown(ms: i64) -> String {
    let secs = (ms.max(0) + 999) / 1000;
    let (h, m, s) = (secs / 3600, secs / 60 % 60, secs % 60);
    if h > 0 { format!("{h}:{m:02}:{s:02}") } else { format!("{m}:{s:02}") }
}

/// The next-request cell of a row at `now`: `…` while it is being costed, the countdown in green
/// while its cache is warm, the next request's cost in red once it is cold, `none` without one.
pub fn next_cell(c: &tokprof_browse::browser::Cells, now: i64, p: &Pal) -> El {
    match (c.warm_until, c.next_usd) {
        // Still being costed.
        _ if c.usd.is_none() => row().key("next").child(text("…").key("next-text").font(pal::SMALL).color(p.ink3).single_line()),
        (Some(until), _) => row()
            .key("next")
            .gap(5.0)
            .center_y()
            .child(dot(p.warm_dot, 7.0))
            .child(text(countdown(until - now)).key("next-text").font(pal::SMALL_NUM).color(p.warm).single_line()),
        (None, Some(usd_next)) => row()
            .key("next")
            .gap(5.0)
            .center_y()
            .child(dot(p.cold_dot, 7.0))
            .child(text(usd(usd_next)).key("next-text").font(pal::SMALL_NUM).color(p.cold).single_line()),
        (None, None) => row().key("next").child(text("none").key("next-text").font(pal::SMALL).color(p.ink3).single_line()),
    }
}
