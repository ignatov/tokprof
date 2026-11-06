//! What the window draws: its own title bar with the day's controls, the day's lanes under it,
//! and under them the list of sessions on the left and the inspector of the selected one.

use std::borrow::Cow;

use rune::{Color, Cx, El, GrowthSpec, ScrollSpec, col, el, row, spacer, text};
use tokprof_browse::browser::{Key, Sort, now_ms};
use tokprof_core::fmt::{local_time, usd};
use tokprof_core::rhythm;

use crate::overlay::{self, PILL, R_ROW};
use crate::pal::{self, Pal};
use crate::{Desk, Slot};

/// The height of the title bar, of a row of the list, and the width of the list.
pub const BAR_H: f32 = 46.0;
pub const ROW: f32 = 28.0;
pub const LIST_WIDTH: f32 = 380.0;
/// The keys of the list's and the inspector's scroll viewports.
pub const LIST: &str = "list";
pub const PANE: &str = "pane";
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
    *s.slots.borrow_mut() = slots(s);
    let height = window_size(s, cx).1;
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
    if s.reports.mode == crate::reports::Mode::Reports {
        root = root.child(crate::reports::view(s, cx, &p));
    } else if s.browser.zoomed() {
        root = root.child(inspector(cx));
    } else {
        let max_h = lanes_max_h(height);
        let lanes = rune::memo("lanes", crate::day::version(s, cx, max_h, &theme.id), || crate::day::lanes(s, cx, &p, max_h));
        let body = row().key("body").w_full().grow(1.0).min_h(0.0).child(list_pane(s, &p)).child(el().w(1.0).h_full().fill(p.line)).child(inspector(cx));
        root = root.child(lanes).child(body);
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

/// The most the lanes take with what is under them, in a window `height` points high.
pub fn lanes_max_h(height: f32) -> f32 {
    (height - BAR_H) * LANES_SHARE
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

/// The window's own title bar: the room for the window's buttons, the day's name with the steps
/// to the days before and after, Today, the day's summary, the status, the lanes' zoom, the
/// filter and the live mark. Dragging it where it has no control moves the window.
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
        .child(crate::reports::switch(s, p));
    // Reports has no day: the bar keeps the switch, the status and the live mark.
    let day = s.reports.mode == crate::reports::Mode::Day;
    if day {
        bar = bar
            .child(step_button("day-back", "‹", "The day before", true, p).on_press(|s: &mut Desk| s.browser.step_day_by(-1)))
            .child(text(local_time(b.day_noon(), "%A, %-d %B")).key("day-name").font(pal::DAY).color(p.ink).single_line())
            .child(step_button("day-on", "›", "The day after", !latest, p).on_press(|s: &mut Desk| s.browser.step_day_by(1)))
            .child(button("today", "Today", !latest, p.panel, p).on_press(|s: &mut Desk| s.browser.show_latest_day()));
    }
    if let Some(summary) = crate::day::summary(s).filter(|_| day) {
        bar = bar.child(text(summary).key("day-summary").font(pal::BODY).color(p.ink2).single_line().truncate().shrink().min_w(0.0));
    }
    bar = bar.child(spacer().grow(1.0));
    if s.status_shown() {
        bar = bar.child(text(s.status.0.clone()).key("status").font(pal::BODY).color(p.ink2).single_line().truncate().shrink().min_w(0.0));
    }
    let zoom = segments(
        "zoom",
        vec![
            segment("zoom-fit".into(), "Fit", s.fit, p).on_press(|s: &mut Desk| s.fit = true),
            segment("zoom-day".into(), "24 h", !s.fit, p).on_press(|s: &mut Desk| s.fit = false),
        ],
        p.panel,
    );
    if day {
        bar = bar.child(zoom).child(filter_field(s, p));
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
    let mut field =
        row().key("filter").w(210.0).h(28.0).pl(12.0).pr(5.0).gap(1.0).center_y().radius(PILL).fill(p.panel).role("searchbox").label("Filter").on_press(
            |s: &mut Desk| {
                if !s.browser.editing() {
                    let view = s.viewport();
                    s.browser.on_key(Key::Char('/'), view);
                }
            },
        );
    field = overlay::ring(field, editing, p);
    // The placeholder, or what is typed; while typing, it ends at the caret, its start cut when long.
    if !(empty && editing) {
        let (shown, color) = if empty { ("Filter by project or id", p.ink3) } else { (b.filter(), p.ink) };
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

/// The slots of the list: a row per listed session and, in time order, a separator row before
/// the first session of each day.
pub fn slots(s: &Desk) -> Vec<Slot> {
    let b = &s.browser;
    let mut out = Vec::with_capacity(b.len() + 16);
    let by_time = b.sort_order() == Sort::Time;
    let offset = &rhythm::local_offset;
    let mut last = None;
    for r in 0..b.len() {
        if by_time {
            let date = rhythm::date_of(b.active_ms(r), offset);
            if last != Some(date) {
                out.push(Slot::Day(date));
                last = Some(date);
            }
        }
        out.push(Slot::Row(r));
    }
    out
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

fn list_pane(s: &Desk, p: &Pal) -> El {
    let b = &s.browser;
    let offset = s.list_offset.clone();
    let count = s.slots.borrow().len();
    let rows = col().key("rows").w_full().growth(GrowthSpec::default()).virtual_rows(
        count,
        ROW,
        |s: &Desk, i| match s.slots.borrow().get(i) {
            Some(Slot::Row(r)) => Cow::Borrowed(s.browser.session(*r)),
            Some(Slot::Day(d)) => Cow::Owned(format!("day-{d}")),
            None => Cow::Borrowed("gone"),
        },
        list_row,
    );
    // Where the rows on the screen go when sessions come and go is the browser's: the poll scrolls
    // the list to the first row it keeps there.
    let list = col()
        .key(LIST)
        .w_full()
        .grow(1.0)
        .min_h(0.0)
        .scroll(ScrollSpec { stick_bottom: false })
        .on_scroll_offset(move |at| offset.set(at.offset[1]))
        .on_layout(|s: &mut Desk, r| s.list_height = r[3])
        .child(rows);
    let (total, pending) = b.known_total();
    let n = b.len() as u64;
    let none = n == 0 && !b.filter().is_empty();
    let mut title = format!("{} {}, {}", tokprof_core::fmt::int(n), if n == 1 { "session" } else { "sessions" }, usd(total));
    if pending > 0 {
        title += &format!(", costing {pending}");
    }
    let sorts = Sort::ALL.iter().map(|&sort| {
        let label = match sort {
            Sort::Time => "Time",
            Sort::Cost => "Cost",
            Sort::Next => "Next",
        };
        segment(format!("sort-{label}"), label, b.sort_order() == sort, p).on_press(move |s: &mut Desk| s.browser.sort_by(sort))
    });
    let head = row()
        .key("list-head")
        .w_full()
        .h(34.0)
        .px(12.0)
        .gap(8.0)
        .center_y()
        .child(
            text(title)
                .key("list-title")
                .font(pal::SMALL_STRONG)
                .color(if list_has_keys(s) { p.accent } else { p.ink2 })
                .single_line()
                .truncate()
                .grow(1.0)
                .min_w(0.0),
        )
        .child(segments("sort", sorts.collect(), p.bar));
    col()
        .key("list-pane")
        .w(LIST_WIDTH)
        .h_full()
        .min_h(0.0)
        .child(head)
        .child(el().w_full().h(1.0).fill(p.line))
        .children(none.then(|| {
            let what = format!("No session matches “{}”", b.filter());
            col().w_full().padding(12.0).child(overlay::empty("list-empty", what, Some(overlay::hint(&["esc"], "clears the filter", p)), p))
        }))
        .child(list)
}

/// The time a cache stays warm, as the list counts it down: `24:47`, `1:02:03` past an hour.
pub fn countdown(ms: i64) -> String {
    let secs = (ms.max(0) + 999) / 1000;
    let (h, m, s) = (secs / 3600, secs / 60 % 60, secs % 60);
    if h > 0 { format!("{h}:{m:02}:{s:02}") } else { format!("{m}:{s:02}") }
}

/// Slot `i` of the list. A row's handlers find the session by its id: the row may hold another
/// one by the time it is pressed.
fn list_row(s: &Desk, i: usize) -> El {
    let p = pal::pal(s.theme.get());
    let slot = s.slots.borrow().get(i).copied();
    let r = match slot {
        Some(Slot::Row(r)) => r,
        Some(Slot::Day(date)) => {
            let today = rhythm::date_of(now_ms(), &rhythm::local_offset);
            return row()
                .w_full()
                .h(ROW)
                .px(18.0)
                .pt(10.0)
                .center_y()
                .child(text(day_label(date, today)).key("day-label").font(pal::SMALL_STRONG).color(p.ink3).single_line());
        }
        None => return el().h(ROW),
    };
    let b = &s.browser;
    let now = now_ms();
    let c = b.cells(r, now);
    let selected = b.selected() == Some(r);
    let (ink, ink2) = (p.ink, p.ink2);
    let (press, open) = (b.session(r).to_string(), b.session(r).to_string());
    let next = match (c.warm_until, c.next_usd) {
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
    };
    let name = c.title.map_or_else(|| c.project.to_string(), str::to_string);
    let agent = p.agents[c.agent_place % p.agents.len()];
    // The selected row is a rounded row inset in the pane, ringed while the list has the keyboard.
    let inner = row()
        .key("cells")
        .w_full()
        .h_full()
        .px(6.0)
        .gap(8.0)
        .center_y()
        .radius(R_ROW)
        .fill(if selected { p.sel } else { Color::TRANSPARENT })
        .hover(if selected { p.sel } else { p.hover })
        .child(text(local_time(c.active_ms, "%H:%M")).key("time").font(pal::SMALL_NUM).color(ink2).single_line().w(36.0))
        .child(dot(agent, 7.0).key("agent"))
        .child(text(name.clone()).key("title").font(pal::BODY).color(ink).single_line().truncate().grow(1.0).shrink().min_w(0.0))
        .child(row().w(58.0).justify_end().child(text(c.cost.to_string()).key("cost").font(pal::STRONG_NUM).color(ink).single_line()))
        .child(row().w(68.0).justify_end().child(next));
    let inner = overlay::ring_at(inner, selected && list_has_keys(s), 1.0, &p);
    row()
        .w_full()
        .h(ROW)
        .px(6.0)
        .py(1.0)
        .role("option")
        .label(format!("{} {} {} {}", local_time(c.active_ms, "%H:%M"), c.agent, name, c.cost))
        .checked(selected)
        .help_when_truncated(format!("{name}\n{}", c.project))
        .on_press_tx(move |s: &mut Desk, tx| {
            s.select_session(&press);
            s.pane = crate::Focus::List;
            tx.scroll_to_y(PANE, 0.0);
        })
        .on_double_press(move |s: &mut Desk| {
            s.select_session(&open);
            s.browser.zoom_in();
        })
        .child(inner)
}

/// Whether the list has the keyboard: as the pane that has it, while no filter is being typed.
pub fn list_has_keys(s: &Desk) -> bool {
    s.pane == crate::Focus::List && !s.browser.editing() && !s.browser.zoomed()
}
