//! The inspector of the selected session: its X-ray drawn as controls, under tabs. The blocks
//! read the fields `render_xray.rs` reads and format them with the same helpers, so that a
//! number here is the number `tokprof scan` prints.

use rune::{Color, Cx, El, ScrollSpec, col, el, row, spacer, text};
use tokprof_browse::browser::{Pane, now_ms};
use tokprof_browse::render_tools::{failures_line, tool_cells};
use tokprof_core::advice::{Topic, Way};
use tokprof_core::analysis::Component;
use tokprof_core::busy;
use tokprof_core::fmt::{count, human_duration, local_time, tokens, usd};
use tokprof_core::model::BlockKind;
use tokprof_core::profile::Values;
use tokprof_core::report::Spend;
use tokprof_core::rhythm::duration;
use tokprof_core::tools::RowOrder;
use tokprof_core::xray::XRay;

use crate::Desk;
use crate::overlay::{self, PILL, R_GROUP, R_MARK, R_ROW};
use crate::pal::{self, Pal};
use crate::view::{self, dot};

/// The inspector's tabs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Tab {
    Summary,
    Requests,
    Spend,
    Work,
    Waits,
    Prompts,
    Tools,
    Context,
    Cache,
    Flame,
}

impl Tab {
    pub const ALL: [Tab; 10] = [Tab::Summary, Tab::Requests, Tab::Spend, Tab::Work, Tab::Waits, Tab::Prompts, Tab::Tools, Tab::Context, Tab::Cache, Tab::Flame];

    pub fn name(self) -> &'static str {
        match self {
            Tab::Summary => "Summary",
            Tab::Requests => "Requests",
            Tab::Spend => "Spend",
            Tab::Work => "Work",
            Tab::Waits => "Waits",
            Tab::Prompts => "Prompts",
            Tab::Tools => "Tools",
            Tab::Context => "Context",
            Tab::Cache => "Cache",
            Tab::Flame => "Flame",
        }
    }

    pub fn named(name: &str) -> Option<Tab> {
        Tab::ALL.into_iter().find(|t| t.name().eq_ignore_ascii_case(name))
    }
}

/// The key of the request track, whose width its marks are placed in.
pub const TRACK: &str = "req-track";
const TRACK_H: f32 = 62.0;

/// What the inspector shows, for `memo`: the X-ray, the tab, the selected session with its next
/// request and cache, the width of the request track, and the minute.
pub fn version(s: &Desk, cx: &Cx<Desk>, theme: &str) -> u64 {
    let b = &s.browser;
    let now = now_ms();
    let selected = b.selected().map(|r| (b.session(r).to_string(), b.cells(r, now).next_usd.map(f64::to_bits), b.cache_life(b.session(r))));
    let pending = match b.report() {
        Pane::Text(_) => String::new(),
        Pane::Computing(n) => n,
        Pane::Empty(n) => n.to_string(),
    };
    view::hash_of((
        (s.xray_key(), pending, s.tab, theme, selected),
        (
            b.zoomed(),
            cx.layout_size(TRACK).map(|sz| sz[0].to_bits()),
            cx.layout_size(view::PANE).map(|sz| sz[0].to_bits()),
            view::minute(),
            keyboard(s),
            s.cursor,
        ),
        (s.tab == Tab::Flame).then(|| crate::flame::version(s, cx)),
        (s.tab == Tab::Requests).then(|| {
            (
                s.requests.version(),
                cx.layout_size(crate::requests::CHART).map(|sz| sz[0].to_bits()),
                cx.layout_size("inspector").map(|sz| sz[1] >= crate::requests::PIN_H),
            )
        }),
    ))
}

/// Whether the inspector has the keyboard: as the pane that has it, or full size.
pub fn keyboard(s: &Desk) -> bool {
    s.pane == crate::Focus::Inspector || s.browser.zoomed()
}

thread_local! {
    /// The rows of the tab being drawn: how many are numbered so far, and the one the cursor is
    /// on while the inspector has the keyboard.
    static ROWS: std::cell::Cell<(usize, Option<usize>)> = const { std::cell::Cell::new((0, None)) };
}

/// Numbers a row of the tab being drawn (`row-0`, `row-1`, …), marks the cursor's, and lets a
/// click put the cursor on it.
fn numbered(line: El, p: &Pal) -> El {
    let (n, cursor) = ROWS.get();
    ROWS.set((n + 1, cursor));
    let line = line.key(format!("row-{n}")).radius(R_ROW).on_press(move |s: &mut Desk| {
        s.pane = crate::Focus::Inspector;
        s.cursor = n;
        if s.tab == Tab::Requests {
            crate::requests::row_pressed(s, n);
        }
    });
    if cursor == Some(n) { overlay::ring_at(line.fill(p.sel), true, 1.0, p) } else { line.hover(p.hover) }
}

/// Puts the cursor of the tab being drawn on row `n`, for a tab whose selection is its own.
pub(crate) fn cursor_row(n: Option<usize>) {
    let (count, _) = ROWS.get();
    ROWS.set((count, n));
}

pub fn inspector(s: &Desk, cx: &Cx<Desk>, p: &Pal) -> El {
    ROWS.set((0, keyboard(s).then_some(s.cursor)));
    // The pane's padding and a card's.
    let pane_w = cx.layout_size(view::PANE).map(|sz| sz[0]).unwrap_or_else(|| {
        let list = if s.browser.zoomed() { 0.0 } else { view::LIST_WIDTH + 1.0 };
        view::window_size(s, cx).0 - list
    });
    ROOM.set(pane_w - 36.0 - 28.0);
    // What stays above the scrolling pane: the request timeline and its card.
    let mut pinned = None;
    let body = match s.browser.xray() {
        Some(x) => match s.tab {
            Tab::Summary => summary(s, x, cx, p),
            Tab::Requests => {
                let (top, mut rest) = crate::requests::tab(s, x, cx, p);
                // The timeline stays above the table where the inspector is tall enough.
                let tall = cx.layout_size("inspector").is_some_and(|sz| sz[1] >= crate::requests::PIN_H);
                match top {
                    Some(t) if tall => pinned = Some(t),
                    Some(t) => rest.insert(0, t),
                    None => {}
                }
                rest
            }
            Tab::Spend => spend_tab(x, p),
            Tab::Work => work(x, p),
            Tab::Waits => waits(x, p),
            Tab::Prompts => prompts(x, p),
            Tab::Tools => tools(x, p),
            Tab::Context => context(x, p),
            Tab::Cache => cache(x, p),
            Tab::Flame => crate::flame::tab(s, x, cx, p),
        },
        None => match s.browser.report() {
            Pane::Text(t) => vec![note(t.text.clone(), p)],
            Pane::Computing(n) => vec![note(n, p)],
            Pane::Empty(n) => {
                let mut head = n.to_string();
                if let Some(c) = head.get(..1) {
                    head = c.to_uppercase() + &head[1..];
                }
                vec![overlay::empty("pane-empty", head, None, p)]
            }
        },
    };
    // Summary's ways are rows of a box, not of a list the cursor moves through.
    s.tab_rows.set(if s.tab == Tab::Summary { 0 } else { ROWS.get().0 });
    // The chosen tab is a pill; while the inspector has the keyboard and its tab has no rows for a
    // cursor, the ring marks it instead.
    let ring_tab = keyboard(s) && s.tab_rows.get() == 0 && s.tab != Tab::Flame;
    let (pad, digits) = tab_fit(pane_w);
    let tabs = row().key("tabs").w_full().h(view::HEAD_H).px(10.0).gap(1.0).center_y().children(Tab::ALL.into_iter().enumerate().map(|(i, t)| {
        let on = s.tab == t;
        let tab = row()
            .key(format!("tab-{}", t.name()))
            .h(28.0)
            // Where the digits fit, a tab keeps their room without the keyboard too, around its
            // name, so that the tabs stay where they are when the keyboard comes and goes.
            .px(if digits && !keyboard(s) { pad + (DIGIT_W + 5.0) / 2.0 } else { pad })
            .gap(5.0)
            .center_y()
            .radius(PILL)
            .fill(if on { p.sel } else { Color::TRANSPARENT })
            .hover(if on { p.sel } else { p.hover })
            .role("tab")
            .checked(on)
            .label(t.name())
            .on_press_tx(move |s: &mut Desk, tx| {
                s.show_tab(t, tx);
                s.pane = crate::Focus::Inspector;
            })
            .child(text(t.name()).font(pal::HEAD).color(if on { if keyboard(s) { p.accent } else { p.ink } } else { p.ink2 }).single_line())
            .children((digits && keyboard(s)).then(|| overlay::keycap_on(&((i + 1) % 10).to_string(), p.panel, p).key("tab-digit")));
        overlay::ring(tab, on && ring_tab, p)
    }));
    let offset = s.pane_offset.clone();
    let pane = col()
        .key(view::PANE)
        .w_full()
        .grow(1.0)
        .min_h(0.0)
        .scroll(ScrollSpec { stick_bottom: false })
        .on_scroll_offset(move |at| offset.set(at.offset[1]))
        .on_layout(|s: &mut Desk, r| s.pane_height = r[3])
        .child(col().key("pane-body").w_full().px(18.0).pt(14.0).pb(26.0).gap(14.0).children(body));
    let pinned = pinned.map(|t| col().key("pinned").w_full().px(18.0).pt(14.0).child(t));
    col().key("inspector").grow(1.0).h_full().min_w(0.0).min_h(0.0).child(tabs).child(el().w_full().h(1.0).fill(p.line)).children(pinned).child(pane)
}

fn note(s: impl Into<String>, p: &Pal) -> El {
    text(s.into()).w_full().wrap().font(pal::BODY).color(p.ink2).selectable(true)
}

/// The width of a tab's digit as a keycap.
const DIGIT_W: f32 = 20.0;

/// The padding of the inspector's tabs in a pane `pane_w` wide, so that all ten stay on one line,
/// and whether there is room for the digits that pick them; they show as keycaps while the
/// inspector has the keyboard.
pub fn tab_fit(pane_w: f32) -> (f32, bool) {
    let labels: f32 = Tab::ALL.iter().map(|t| pal::text_w(t.name(), pal::HEAD)).sum();
    // The row's padding and the gaps between the tabs, and some slack for the estimate.
    let room = pane_w - 20.0 - 9.0 - 24.0 - labels;
    let digits_w = 10.0 * (DIGIT_W + 5.0);
    if room - digits_w >= 2.0 * 10.0 * 8.0 {
        return (8.0, true);
    }
    ((room / 20.0).floor().clamp(3.0, 8.0), false)
}

/// A card: its title and a note at its right over a soft group, as Reports draws its cards.
fn boxed(key: &str, title: &str, aside: Option<String>, body: El, p: &Pal) -> El {
    let mut head = row().w_full().gap(8.0).center_y().child(text(title.to_string()).font(pal::HEAD).color(p.ink2).single_line());
    if let Some(a) = aside {
        head = head.child(spacer().grow(1.0)).child(text(a).key("aside").font(pal::SMALL).color(p.ink3).single_line().truncate().shrink().min_w(0.0));
    }
    let group = col().w_full().min_w(0.0).grow(1.0).padding(14.0).gap(9.0).radius(R_GROUP).fill(p.bar).child(body);
    col().key(key.to_string()).w_full().min_w(0.0).gap(10.0).child(head).child(group)
}

/// Rows of a bar chart: a label, a bar as long as its share of the largest, the share, the value
/// and a note.
pub(crate) fn bar_rows(rows: &[(String, f64, String, String)], color: Color, p: &Pal) -> El {
    let max = rows.iter().map(|r| r.1).fold(0.0, f64::max).max(1e-9);
    // A note goes under its label, so that every bar has the same track and the numbers end at
    // the card's edge.
    col().w_full().gap(4.0).children(rows.iter().enumerate().map(|(i, (label, share, value, note))| {
        let mut head = col().w(190.0).gap(1.0).child(text(label.clone()).font(pal::BODY).color(p.ink).single_line().truncate().w(190.0));
        if !note.is_empty() {
            head = head.child(text(note.clone()).key("note").font(pal::SMALL).color(p.ink3).single_line().truncate().w(190.0));
        }
        let line = row()
            .w_full()
            .min_h(20.0)
            .py(if note.is_empty() { 0.0 } else { 2.0 })
            .gap(10.0)
            .center_y()
            .child(head)
            .child(crate::overlay::meter(share / max, color, p, 6.0).key(format!("bar-{i}")))
            .child(row().w(50.0).justify_end().child(text(format!("{:.1}%", 100.0 * share)).font(pal::BODY_NUM).color(p.ink2).single_line()))
            .child(row().w(64.0).justify_end().child(text(value.clone()).font(pal::STRONG_NUM).color(p.ink).single_line()));
        numbered(line, p)
    }))
}

/// A column of a table: its title, its width (0 for the one that takes the rest), and whether
/// its cells are numbers, set right.
pub(crate) struct Column(pub &'static str, pub f32, pub bool);

fn table(cols: &[Column], rows: Vec<Vec<(String, Option<Color>)>>, p: &Pal) -> El {
    table_dropping(cols, rows, &[], p)
}

/// The columns that fit in `room` points: those named in `drop` left out, first to last, while
/// the fixed columns, the gaps and the text column's least width are more than the room.
pub fn fitting(cols: &[(f32, &str)], drop: &[&str], room: f32) -> Vec<bool> {
    let mut keep = vec![true; cols.len()];
    let need = |keep: &[bool]| {
        let shown: Vec<_> = cols.iter().zip(keep).filter(|(_, k)| **k).map(|(c, _)| c.0).collect();
        shown.iter().map(|&w| if w == 0.0 { TEXT_MIN } else { w }).sum::<f32>() + 12.0 * (shown.len() as f32 - 1.0).max(0.0)
    };
    for name in drop {
        if need(&keep) <= room {
            break;
        }
        if let Some(i) = cols.iter().position(|c| c.1 == *name) {
            keep[i] = false;
        }
    }
    keep
}

/// The least width of a table's text column.
const TEXT_MIN: f32 = 120.0;

thread_local! {
    /// The width a card's content has in the inspector, as it was last laid out.
    static ROOM: std::cell::Cell<f32> = const { std::cell::Cell::new(f32::MAX) };
}

/// A table whose columns named in `drop` are left out, first to last, when the card is too narrow.
pub(crate) fn table_dropping(cols: &[Column], rows: Vec<Vec<(String, Option<Color>)>>, drop: &[&str], p: &Pal) -> El {
    let keep = fitting(&cols.iter().map(|c| (c.1, c.0)).collect::<Vec<_>>(), drop, ROOM.get());
    let cols: Vec<&Column> = cols.iter().zip(&keep).filter(|(_, k)| **k).map(|(c, _)| c).collect();
    let rows: Vec<Vec<(String, Option<Color>)>> = rows.into_iter().map(|r| r.into_iter().zip(&keep).filter(|(_, k)| **k).map(|(c, _)| c).collect()).collect();
    let cell = |t: String, c: &Column, color: Color, font| {
        // A path keeps its start and its end.
        let path = !c.2 && t.contains('/');
        let t = text(t).font(font).color(color).single_line().min_w(0.0);
        let t = if path { t.truncate_middle() } else { t.truncate() };
        match (c.1, c.2) {
            (0.0, _) => row().grow(1.0).min_w(TEXT_MIN).child(t),
            (w, true) => row().w(w).justify_end().child(t),
            (w, false) => row().w(w).child(t),
        }
    };
    let head = row().w_full().h(22.0).gap(12.0).center_y().children(cols.iter().map(|c| cell(c.0.to_string(), c, p.ink2, pal::SMALL_STRONG)));
    let mut out = col().w_full().child(head).child(el().w_full().h(1.0).fill(p.line));
    for r in rows {
        let line = row().w_full().h(24.0).gap(12.0).center_y().children(r.into_iter().zip(cols.iter().copied()).map(|((t, color), c)| {
            let font = if c.2 { pal::BODY_NUM } else { pal::BODY };
            cell(t, c, color.unwrap_or(p.ink), font)
        }));
        out = out.child(numbered(line, p)).child(el().w_full().h(1.0).fill(p.row_line));
    }
    out
}

fn plain(t: impl Into<String>) -> (String, Option<Color>) {
    (t.into(), None)
}

/// An amount as the X-ray shows it: dollars when priced, tokens when not.
fn amount(x: &XRay, v: &Values) -> String {
    if x.priced { usd(v.usd) } else { format!("{} tok", tokens(v.tokens)) }
}

fn spend_amount(x: &XRay, s: &Spend) -> String {
    if x.priced { usd(s.usd) } else { format!("{} tok", tokens(s.tokens)) }
}

/// The spend by component, as the X-ray's spend block counts it: each component with its share
/// and amount, those that read 0.0% left out.
pub fn components(x: &XRay) -> Vec<(Component, f64, String)> {
    let value = |v: &Values| if x.priced { v.usd } else { v.tokens };
    let total: f64 = x.components.iter().map(value).sum();
    Component::ALL.iter().zip(&x.components).map(|(c, v)| (*c, value(v) / total.max(1e-9), amount(x, v))).filter(|(_, share, _)| *share >= 0.0005).collect()
}

pub(crate) fn component_color(c: Component, p: &Pal) -> Color {
    match c {
        Component::Read => p.read,
        Component::Write => p.write,
        Component::Rewrite => p.miss,
        Component::Output => p.out,
        _ => p.neutral,
    }
}

fn total(x: &XRay) -> String {
    let t: f64 = x.components.iter().map(|v| if x.priced { v.usd } else { v.tokens }).sum();
    if x.priced { usd(t) } else { format!("{} tokens", tokens(t)) }
}

fn spend_box(x: &XRay, p: &Pal) -> El {
    let parts = components(x);
    let bar = row().w_full().h(8.0).gap(2.0).children(parts.iter().map(|(c, share, _)| {
        el().h(8.0).radius(R_MARK).grow(*share as f32).min_w(2.0).fill(component_color(*c, p)).help(format!("{}: {:.1}%", c.name(), 100.0 * share))
    }));
    let legend = col().w_full().gap(5.0).children(parts.iter().map(|(c, share, v)| {
        row()
            .w_full()
            .gap(8.0)
            .center_y()
            .child(el().w(10.0).h(10.0).radius(R_MARK).fill(component_color(*c, p)))
            .child(text(c.name()).font(pal::BODY).color(p.ink).single_line().grow(1.0).min_w(0.0))
            .child(text(format!("{:.1}%", 100.0 * share)).font(pal::BODY_NUM).color(p.ink2).single_line())
            .child(row().w(56.0).justify_end().child(text(v.clone()).font(pal::STRONG_NUM).color(p.ink).single_line()))
    }));
    let head = text(total(x)).key("spend-total").font(pal::KPI).color(p.ink).single_line();
    boxed(
        "spend",
        "Spend",
        Some(if x.priced { "API-equivalent".into() } else { "no price".into() }),
        col().w_full().gap(9.0).child(head).child(bar).child(legend),
        p,
    )
}

/// The header: the title, what the session is, and the handoff.
fn header(s: &Desk, x: &XRay, p: &Pal) -> El {
    let agent = s.browser.selected().map(|r| s.browser.cells(r, now_ms()).agent_place).map_or(p.ink2, |place| p.agents[place % p.agents.len()]);
    let mut facts = row()
        .key("facts")
        .w_full()
        .gap(16.0)
        .center_y()
        .child(row().gap(6.0).center_y().child(dot(agent, 7.0)).child(text(x.agent.name().to_string()).font(pal::STRONG).color(agent).single_line()))
        .child(
            text(x.project.clone())
                .font(pal::STRONG)
                .color(p.ink)
                .single_line()
                .truncate_head()
                .min_w(pal::text_w(&x.project, pal::STRONG).min(320.0))
                .max_w(320.0),
        )
        .child(text(x.short_id.clone()).font(pal::BODY_NUM).color(p.ink2).single_line().selectable(true));
    if !x.models.is_empty() {
        let models = x.models.iter().map(|(m, n)| format!("{m} ×{n}")).collect::<Vec<_>>().join(", ");
        facts = facts.child(text(models).font(pal::BODY).color(p.ink2).single_line().truncate().shrink().min_w(0.0));
    }
    let mut what = count(x.requests, "request", "requests");
    if let Some(r) = &x.rhythm {
        what += &format!(" in {}, {} of work", count(r.turns as u64, "turn", "turns"), duration(r.work_ms));
    }
    if let Some((a, b)) = x.span_ms {
        what = format!("{} to {}, {}; {what}", local_time(a, "%m-%d %H:%M"), local_time(b, "%H:%M"), human_duration((b - a) as f64 / 1000.0));
    }
    let title = x.title.clone().unwrap_or_else(|| "Untitled session".to_string());
    row()
        .key("header")
        .w_full()
        .gap(14.0)
        .child(
            col()
                .grow(1.0)
                .min_w(0.0)
                .gap(5.0)
                .child(text(title).key("title").w_full().wrap().max_lines(2).font(pal::TITLE).color(p.ink).selectable(true))
                .child(facts)
                .child(text(what).key("what").font(pal::BODY).color(p.ink2).single_line().truncate().selectable(true)),
        )
        .child(view::button("handoff", "Copy handoff line", true, p.bar, p).pr(5.0).child(overlay::keycap_on("c", p.panel, p)).on_press(|s: &mut Desk| {
            let view = s.viewport();
            s.browser.on_key(tokprof_browse::browser::Key::Char('c'), view);
        }))
}

/// The time axis of the request track: from the first request to now. When the time after the
/// last request would take more than `TAIL` of the axis, the axis breaks at the last request and
/// that time gets `TAIL`: the warm part of it the first half and the cold part the second, or all
/// of it while the cache is still warm.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Axis {
    pub begin: i64,
    pub last: i64,
    pub cold_at: i64,
    pub now: i64,
}

const TEN_MIN: i64 = 600_000;
/// The most of the axis the time after the last request takes.
pub const TAIL: f32 = 0.08;
/// Where now sits, a little before the end, so that the next request fits.
const END: f32 = 0.97;

impl Axis {
    pub fn of(first: i64, last: i64, cold_at: i64, now: i64) -> Axis {
        let begin = first - first.rem_euclid(TEN_MIN);
        let now = now.max(first);
        let last = last.clamp(first, now);
        Axis { begin, last, cold_at, now }
    }

    pub fn broken(&self) -> bool {
        END * (self.now - self.last) as f32 > TAIL * (self.now - self.begin).max(1) as f32
    }

    /// Where the axis breaks, from 0 to 1: at the last request.
    pub fn break_at(&self) -> f32 {
        END - TAIL
    }

    /// Where a moment is along the axis, from 0 to 1.
    pub fn at(&self, ts: i64) -> f32 {
        let share = |a: i64, b: i64| ((ts - a) as f64 / (b - a).max(1) as f64).clamp(0.0, 1.0) as f32;
        if !self.broken() {
            return END * share(self.begin, self.now);
        }
        let work = END - TAIL;
        if ts <= self.last {
            return work * share(self.begin, self.last);
        }
        if self.cold_at > self.last && self.cold_at < self.now {
            return if ts <= self.cold_at {
                work + TAIL / 2.0 * share(self.last, self.cold_at)
            } else {
                work + TAIL / 2.0 + TAIL / 2.0 * share(self.cold_at, self.now)
            };
        }
        work + TAIL * share(self.last, self.now)
    }
}

/// The band along the axis from the last request to now: green while the cache stayed warm, red
/// once it went cold, with a break mark where the axis breaks. `width` is the axis's width.
pub fn cache_band(axis: &Axis, width: f32, p: &Pal) -> El {
    let xs = |ts: i64| axis.at(ts) * width;
    let warm_end = axis.cold_at.min(axis.now);
    let mut band = el().key("cache-band").w_full().h(BAND_H);
    band = band.child(
        el().key("warm-span").absolute(xs(axis.last), 1.0).w((xs(warm_end) - xs(axis.last)).max(1.0)).h(BAND_H - 2.0).radius(2.0).fill(p.warm.alpha(0.75)),
    );
    if axis.now >= axis.cold_at {
        let x0 = xs(axis.cold_at);
        band = band.child(el().key("cold-span").absolute(x0, 1.0).w((xs(axis.now) - x0).max(1.0)).h(BAND_H - 2.0).radius(2.0).fill(p.cold.alpha(0.6)));
    }
    if axis.broken() {
        band = band.child(text("//").key("axis-break").font(pal::HOURS).color(p.ink2).single_line().absolute(axis.break_at() * width - 5.0, -6.0));
    }
    band
}

/// The times under an axis, each where there is room: from its place on, or, near the end, up to
/// it, keeping clear of now's label at the end.
pub fn axis_times(marks: Vec<(f32, String, &'static str)>, now_at: f32, p: &Pal) -> Vec<El> {
    // The labels in the order given, the first first: each right of its moment, else left of
    // it, where it keeps 12 pt clear of every label placed and of now's label from `now_at`.
    let mut placed: Vec<(f32, f32)> = vec![(now_at, f32::MAX)];
    let mut out = Vec::new();
    for (x0, t, what) in marks {
        let w = pal::text_w(&t, pal::SMALL_STRONG).max(pal::text_w(what, pal::SMALL)) + 4.0;
        let label =
            |c: El| c.child(text(t.clone()).font(pal::SMALL_STRONG).color(p.ink).single_line()).child(text(what).font(pal::SMALL).color(p.ink2).single_line());
        let clear = |a: f32, b: f32| a >= 0.0 && placed.iter().all(|&(pa, pb)| b + 12.0 <= pa || a >= pb + 12.0);
        if clear(x0, x0 + w) {
            out.push(label(col().absolute(x0, 3.0 + BAND_H)));
            placed.push((x0, x0 + w));
        } else if clear(x0 - 6.0 - w, x0 - 6.0) {
            out.push(label(col().absolute(x0 - 6.0 - w, 3.0 + BAND_H).w(w).items_end()));
            placed.push((x0 - 6.0 - w, x0 - 6.0));
        }
    }
    out
}

/// About how wide now's label is, in points.
pub fn now_w(now: i64, last: i64) -> f32 {
    let (time, what) = now_label(now, last);
    pal::text_w(&time, pal::SMALL_STRONG).max(pal::text_w(&what, pal::SMALL)) + 4.0
}

/// Now's label: its time and `now, next request`, or, when the last request was on another
/// day, now's day and time and how long after the last request it is.
pub fn now_label(now: i64, last: i64) -> (String, String) {
    if local_time(now, "%Y-%m-%d") == local_time(last, "%Y-%m-%d") {
        return (local_time(now, "%H:%M"), "now, next request".to_string());
    }
    let gap = (now - last).max(0) as f64 / 1000.0;
    let later = if gap >= 86_400.0 { format!("{:.1}d later", gap / 86_400.0) } else { format!("{} later", human_duration(gap)) };
    (local_time(now, "%a %-d %b %H:%M"), format!("now, {later}"))
}

/// Now's label at the right end of an axis `width` wide.
pub fn now_el(now: i64, last: i64, width: f32, p: &Pal) -> El {
    let (time, what) = now_label(now, last);
    let w = now_w(now, last);
    col()
        .key("now-label")
        .absolute(width - w, 3.0 + BAND_H)
        .w(w)
        .items_end()
        .child(text(time).font(pal::SMALL_STRONG).color(p.ink).single_line())
        .child(text(what).font(pal::SMALL).color(p.ink2).single_line())
}

/// The height of the cache band under an axis.
pub const BAND_H: f32 = 7.0;

/// The track of the session's requests on a time axis up to now: a bar per request as tall as
/// its cost, the time the cache stayed warm, the time it has been cold, and the next request.
fn request_track(s: &Desk, x: &XRay, cx: &Cx<Desk>, p: &Pal, tall: f32) -> Option<El> {
    let first = x.points.first()?.ts_ms;
    let now = now_ms();
    let session = s.browser.selected().map(|r| s.browser.session(r).to_string()).unwrap_or_default();
    let (last, cold_at) = s.browser.cache_life(&session).unwrap_or_else(|| {
        let last = x.points.last().map_or(first, |q| q.ts_ms);
        (last, x.context.as_ref().map_or(last, |c| last + c.cache.ttl_ms))
    });
    let next = s.browser.selected().and_then(|r| s.browser.cells(r, now).next_usd);
    let axis = Axis::of(first, last, cold_at, now);
    let width = cx.layout_size(TRACK).map_or(640.0, |sz| sz[0]).max(100.0);
    let xs = |ts: i64| axis.at(ts) * width;
    // The requests set the scale; a next request taller than it is cut, with a mark.
    let top = x.points.iter().map(|q| q.usd).fold(0.0, f64::max).max(1e-9);
    let height = |usd: f64| ((usd / top) as f32 * tall).max(2.0);
    let cold = now >= cold_at;
    let mut track = el().key(TRACK).w_full().h(tall + 18.0);
    for (i, q) in x.points.iter().enumerate() {
        let h = height(q.usd);
        track = track.child(el().key(format!("request-{i}")).absolute(xs(q.ts_ms) - 1.5, 18.0 + tall - h).w(3.0).h(h).radius(1.5).fill(p.bars).help(format!(
            "{}: {}, {} in context",
            local_time(q.ts_ms, "%H:%M:%S"),
            usd(q.usd),
            tokens(q.prompt_tokens as f64)
        )));
    }
    // Where the next request's label sits, so that the largest request's label keeps clear of it.
    let mut next_label: Option<(f32, f32)> = None;
    if let Some(n) = next {
        let h = height(n).min(tall);
        let c = if cold { p.cold } else { p.warm };
        let x0 = xs(now);
        if height(n) > tall {
            track = track.child(text("▲").key("next-clipped").font(pal::HOURS).color(c).single_line().absolute(x0 - 4.0, 19.0));
        }
        track = track
            .child(
                el().key("next-request")
                    .absolute(x0 - 7.0, 18.0 + tall - h)
                    .w(14.0)
                    .h(h)
                    .radius(3.0)
                    .fill(if cold { p.cold_bg } else { c.alpha(0.1) })
                    .border(1.4, c)
                    .border_dash(3.0, 2.5),
            )
            .child(text(usd(n)).key("next-value").font(pal::SMALL_STRONG).color(c).single_line().absolute(x0 - 40.0, 0.0));
        next_label = Some((x0 - 40.0, label_w(&usd(n))));
    }
    if let Some(dear) = x.points.iter().max_by(|a, b| a.usd.total_cmp(&b.usd)) {
        let t = usd(dear.usd);
        let y = (tall - height(dear.usd)).max(0.0);
        if let Some(at) = dear_label_x(xs(dear.ts_ms), label_w(&t), y, next_label) {
            track = track.child(text(t).key("dear-value").font(pal::SMALL_STRONG).color(p.ink).single_line().absolute(at, y));
        }
    }
    // The times under the axis, each where there is room.
    let mut marks: Vec<(f32, String, &'static str)> =
        vec![(xs(first), local_time(first, "%H:%M"), "start"), (xs(last), local_time(last, "%H:%M"), "last request")];
    marks.push((xs(cold_at), local_time(cold_at, "%H:%M"), if cold { "cache cold" } else { "cold then" }));
    // The cache's band along the axis, and the times under it.
    let mut axis_row = el().w_full().h(30.0 + BAND_H).child(cache_band(&axis, width, p).absolute(0.0, 0.0));
    axis_row = axis_row.children(axis_times(marks, width - now_w(now, last), p));
    axis_row = axis_row.child(now_el(now, last, width, p));
    let since = format!("{} to now", local_time(first, "%H:%M"));
    let body = col().w_full().child(track).child(el().w_full().h(1.0).fill(p.line)).child(axis_row);
    Some(boxed("requests-track", "Requests and the cache", Some(since), body, p))
}

/// About how wide a label of the request track is, in points.
fn label_w(t: &str) -> f32 {
    t.chars().count() as f32 * 7.2 + 4.0
}

/// Where the largest request's label goes over its bar at `bar_x`, `w` wide and `y` from the top:
/// over the bar, else left of it when it would cover the next request's label (its left edge and
/// width), else nowhere.
pub fn dear_label_x(bar_x: f32, w: f32, y: f32, next: Option<(f32, f32)>) -> Option<f32> {
    let clear = |x: f32| next.is_none_or(|(nx, nw)| y >= 16.0 || x + w + 4.0 <= nx || x >= nx + nw + 4.0);
    let over = bar_x - 14.0;
    let left = bar_x - 4.0 - w;
    [over, left].into_iter().find(|&x| x >= 0.0 && clear(x))
}

/// The next request's cost and the cache's state, live: from the glance, as the list shows them.
fn next_box(s: &Desk, x: &XRay, p: &Pal) -> El {
    let now = now_ms();
    let session = s.browser.selected().map(|r| s.browser.session(r).to_string()).unwrap_or_default();
    let next = s.browser.selected().and_then(|r| s.browser.cells(r, now).next_usd);
    let Some(c) = &x.context else {
        return boxed("next", "Next request", None, note("The log does not say how big the prompt is.", p), p);
    };
    if let Some(t) = &c.trailing {
        let body = col()
            .w_full()
            .gap(6.0)
            .children(t.next_usd.map(|n| text(usd(n)).font(pal::KPI).color(p.ink).single_line()))
            .child(note(format!("Compacted at {}; the next request re-builds ~{}.", local_time(t.at_ms, "%H:%M"), tokens(t.estimate_tokens)), p));
        return boxed("next", "Next request", None, body, p);
    }
    let (cold, aside) = match s.browser.cache_life(&session) {
        Some((_, cold_at)) if now >= cold_at => (true, format!("cold for {}", human_duration((now - cold_at) as f64 / 1000.0))),
        Some((_, cold_at)) => (false, format!("warm, cold in {} min", (cold_at - now + 59_999) / 60_000)),
        None if c.cache.cold() => (true, format!("cold for {}", human_duration((c.cache.idle_ms - c.cache.ttl_ms) as f64 / 1000.0))),
        None => (false, format!("warm, cold in {}", human_duration((c.cache.ttl_ms - c.cache.idle_ms) as f64 / 1000.0))),
    };
    let next = next.or(if cold { c.cache.next_cold_usd } else { c.cache.next_warm_usd });
    let share = |n: f64| {
        if c.spend_usd < 0.005 { String::new() } else { format!(": {:.0}% of the {} this session has cost", 100.0 * n / c.spend_usd, usd(c.spend_usd)) }
    };
    let line = match (cold, next, c.cache.next_warm_usd, c.cache.next_cold_usd) {
        (true, Some(n), _, _) => format!("It writes all {} tokens of the context into the cache again{}.", tokens(c.size_tokens as f64), share(n)),
        (false, _, Some(w), Some(k)) => format!("≈ {} a request while warm, ≈ {} once it is cold{}.", usd(w), usd(k), share(k)),
        _ => String::new(),
    };
    let mut body = col().w_full().gap(7.0);
    if let Some(n) = next {
        body = body.child(text(usd(n)).key("next-cost").font(pal::KPI).color(if cold { p.cold } else { p.warm }).single_line());
    }
    if !line.is_empty() {
        body = body.child(note(line, p));
    }
    if let Some(w) = c.window_tokens.filter(|&w| w > 0) {
        let share = c.size_tokens as f64 / w as f64;
        body = body.child(
            row().w_full().gap(8.0).center_y().child(crate::overlay::meter(share, p.neutral, p, 6.0)).child(
                text(format!("{}{} of {}", if c.approx { "~" } else { "" }, tokens(c.size_tokens as f64), tokens(w as f64)))
                    .font(pal::SMALL_NUM)
                    .color(p.ink2)
                    .single_line(),
            ),
        );
    }
    boxed("next", "Next request", Some(aside), body, p)
}

/// The ways to carry on that the advice prices, each at the price its first line gives, cheapest
/// first: the box marks the first when there is more than one.
pub fn ways(advice: &[tokprof_core::advice::Advice]) -> Vec<(Way, f64)> {
    let mut out: Vec<(Way, f64)> = Vec::new();
    for (w, usd) in advice.iter().flat_map(|a| a.ways.iter().copied()) {
        if !out.iter().any(|o| o.0 == w) {
            out.push((w, usd));
        }
    }
    out.sort_by(|a, b| a.1.total_cmp(&b.1));
    out
}

fn way_name(w: Way) -> (&'static str, &'static str) {
    match w {
        Way::Resume => ("Resume here", "the next request in this session"),
        Way::Handoff => ("New session from a handoff", "to pick up the work"),
        Way::Compact => ("/compact", "the compaction call"),
    }
}

fn ways_box(x: &XRay, p: &Pal) -> El {
    let ways = ways(&x.advice);
    if ways.is_empty() {
        return boxed("ways", "Ways to continue", None, note("Resuming is the way: the context is small enough.", p), p);
    }
    let marked = ways.len() > 1;
    let body =
        col().w_full().gap(4.0).children(ways.iter().enumerate().map(|(i, (w, cost))| {
            let best = marked && i == 0;
            let (name, what) = way_name(*w);
            row()
                .key(format!("way-{i}"))
                .w_full()
                .px(8.0)
                .py(6.0)
                .gap(10.0)
                .center_y()
                .radius(R_ROW)
                .fill(if best { p.sel } else { Color::TRANSPARENT })
                .child(
                    col().grow(1.0).min_w(0.0).gap(1.0).child(text(name).font(pal::STRONG).color(p.ink).single_line()).child(
                        text(if best { format!("{what}, the cheapest") } else { what.to_string() }).font(pal::SMALL).color(p.ink2).single_line().truncate(),
                    ),
                )
                .child(text(usd(*cost)).key("way-cost").font(pal::STRONG_NUM).color(if best { p.warm } else { p.ink }).single_line())
        }));
    boxed("ways", "Ways to continue", None, body, p)
}

/// The advice that is not one of the ways: the tips.
fn tips(x: &XRay, p: &Pal) -> Option<El> {
    let tips: Vec<&str> = x.advice.iter().filter(|a| a.topic != Topic::CarryOn || a.ways.is_empty()).map(|a| a.text.as_str()).collect();
    if tips.is_empty() {
        return None;
    }
    let body = col().w_full().gap(7.0).children(tips.into_iter().map(|t| {
        row()
            .w_full()
            .gap(8.0)
            .child(text("▸").font(pal::BODY).color(p.accent).single_line())
            .child(text(t.replace('`', "")).font(pal::BODY).color(p.ink).wrap().grow(1.0).min_w(0.0).selectable(true))
    }));
    Some(boxed("tips", "Tips", None, body, p))
}

fn summary(s: &Desk, x: &XRay, cx: &Cx<Desk>, p: &Pal) -> Vec<El> {
    let mut out = vec![header(s, x, p)];
    if x.requests == 0 {
        out.push(note("no requests", p));
        return out;
    }
    out.extend(request_track(s, x, cx, p, TRACK_H));
    out.push(
        row()
            .key("summary-boxes")
            .w_full()
            .gap(12.0)
            .items_stretch()
            .children([next_box(s, x, p), ways_box(x, p), spend_box(x, p)].map(|b| col().grow(1.0).min_w(0.0).child(b.grow(1.0)))),
    );
    out.extend(tips(x, p));
    out
}

fn spend_tab(x: &XRay, p: &Pal) -> Vec<El> {
    let mut out = vec![spend_box(x, p)];
    if x.chains.len() > 1 {
        let total: f64 = x.chains.iter().map(|(_, v)| if x.priced { v.usd } else { v.tokens }).sum();
        let rows: Vec<_> =
            x.chains.iter().map(|(n, v)| (n.clone(), (if x.priced { v.usd } else { v.tokens }) / total.max(1e-9), amount(x, v), String::new())).collect();
        out.push(boxed("chains", "By chain", Some("the main conversation and its subagents".into()), bar_rows(&rows, p.bars, p), p));
    }
    out
}

fn work(x: &XRay, p: &Pal) -> Vec<El> {
    let key = |s: &Spend| if x.priced { s.usd } else { s.tokens };
    let mut out = Vec::new();
    let origins = x.work.origins();
    let total: f64 = origins.iter().map(|(_, s)| key(s)).sum();
    let rows: Vec<_> = origins.iter().map(|(o, s)| (o.to_string(), key(s) / total.max(1e-9), spend_amount(x, s), String::new())).collect();
    out.push(boxed("origins", "By origin", Some("the kind of work each request was part of".into()), bar_rows(&rows, p.work, p), p));
    let tasks = x.work.tasks();
    if tasks.len() > 1 {
        let rows = tasks.iter().take(12).map(|(t, o, s)| vec![plain(spend_amount(x, s)), plain(t.to_string()), (o.to_string(), Some(p.ink2))]).collect();
        out.push(boxed(
            "tasks",
            "By task",
            Some("the session title, and what each subagent was asked to do".into()),
            table(&[Column("cost", 70.0, true), Column("task", 0.0, false), Column("origin", if ROOM.get() >= 900.0 { 270.0 } else { 240.0 }, false)], rows, p),
            p,
        ));
    }
    let activities = x.work.activities();
    let total: f64 = activities.iter().map(|(_, t)| key(&t.spend)).sum();
    let rows: Vec<_> = activities.iter().map(|(a, t)| (a.to_string(), key(&t.spend) / total.max(1e-9), spend_amount(x, &t.spend), String::new())).collect();
    out.push(boxed("activities", "By activity", Some("what the agent was doing before each request".into()), bar_rows(&rows, p.write, p), p));
    out
}

fn waits(x: &XRay, p: &Pal) -> Vec<El> {
    let b = &x.busy;
    let total = b.total_ms();
    if total < busy::WAIT_MS {
        return vec![note("The agent was busy for less than a second.", p)];
    }
    let rows: Vec<_> = b
        .sorted()
        .into_iter()
        .map(|(kind, ms, after)| {
            let note = match kind {
                busy::MODEL => count(b.main_requests, "request", "requests"),
                busy::API_RETRIES => count(b.retried, "request", "requests"),
                busy::SUBAGENTS if after > 0 => format!("{} after the turn ended", duration(after)),
                _ => String::new(),
            };
            (kind.to_string(), ms as f64 / total as f64, duration(ms), note)
        })
        .collect();
    let mut body = col().w_full().gap(10.0).child(bar_rows(&rows, p.message, p));
    let slow = match (b.slow, b.longest) {
        (0, _) | (_, None) => "No request took a minute or more.".to_string(),
        (n, Some((ms, output))) => format!(
            "{} a minute or more, {} of them in the main conversation; the longest took {} and wrote {} tokens.",
            if n == 1 { "1 request took".to_string() } else { format!("{} requests took", tokprof_core::fmt::int(n)) },
            tokprof_core::fmt::int(b.slow_main),
            duration(ms),
            tokens(output as f64)
        ),
    };
    body = body.child(note(slow, p));
    if !b.waits.is_empty() {
        let waits: Vec<String> = b.waits.iter().map(|w| format!("{} for {} at {}", duration(w.ms), w.kind, local_time(w.start_ms, "%m-%d %H:%M"))).collect();
        body = body.child(note(format!("Longest waits: {}.", waits.join(", ")), p));
    }
    vec![boxed("waits", "Waits", Some(format!("busy {}: what the agent waited on in its turns", duration(total))), body, p)]
}

fn prompts(x: &XRay, p: &Pal) -> Vec<El> {
    if x.prompts.is_empty() {
        return vec![note("Fewer than two typed messages: nothing to compare.", p)];
    }
    let rows = x
        .prompts
        .iter()
        .map(|q| {
            let acts = q.activities.iter().take(2).map(|(a, s)| format!("{a} {}", spend_amount(x, s))).collect::<Vec<_>>().join(", ");
            vec![
                plain(q.number.to_string()),
                plain(spend_amount(x, &q.spend)),
                plain(q.spend.requests.to_string()),
                plain(q.label.clone()),
                (acts, Some(p.ink2)),
            ]
        })
        .collect();
    let cols = [
        Column("#", 30.0, true),
        Column("cost", 70.0, true),
        Column("requests", 64.0, true),
        Column("you typed", 0.0, false),
        Column("it paid for", 260.0, false),
    ];
    vec![boxed("prompts", "Prompts", Some("what each message you typed cost, subagents included".into()), table_dropping(&cols, rows, &["it paid for"], p), p)]
}

fn tools(x: &XRay, p: &Pal) -> Vec<El> {
    if !x.tools.rows.values().any(|r| r.calls > 0) {
        return vec![note("No tool calls.", p)];
    }
    let cells = tool_cells(&x.tools, RowOrder::Rent, 24, x.tools.failed > 0);
    let widths = [0.0, 50.0, 80.0, 80.0, 70.0, 70.0, 56.0, 50.0, 70.0];
    let names: Vec<&'static str> = vec!["tool", "calls", "avg result", "avg input", "rent", "trips", "single", "failed", "after"];
    let n = cells.rows.first().map_or(0, Vec::len);
    let mut cols: Vec<Column> = (0..n).map(|i| Column(names[i], widths[i], i > 0)).collect();
    cols.push(Column("hint", 230.0, false));
    let rows = cells
        .rows
        .iter()
        .skip(1)
        .zip(cells.hints.iter().skip(1))
        .map(|(r, h)| r.iter().cloned().map(plain).chain([(h.clone(), Some(p.ink2))]).collect())
        .collect();
    let mut body = col().w_full().gap(8.0).child(table_dropping(&cols, rows, &["hint", "after", "failed", "single", "avg input", "trips"], p));
    if cells.more > 0 {
        body = body.child(note(format!("{} more", cells.more), p));
    }
    body = body.child(note(cells.spend.clone(), p));
    if let Some(f) = failures_line(&x.tools) {
        body = body.child(text(f).w_full().wrap().font(pal::BODY).color(p.cold).selectable(true));
    }
    vec![boxed("tools", "Tools", Some("calls, sizes, rent and trips by tool".into()), body, p)]
}

fn context(x: &XRay, p: &Pal) -> Vec<El> {
    let mut out = Vec::new();
    if let Some(c) = &x.composition {
        let aside = format!("{} tokens, as of {}", tokens(c.total_tokens), local_time(c.as_of_ms, "%H:%M"));
        if !c.known {
            out.push(boxed("held", "In context", Some(aside), note(format!("{} logs do not show what the prompt holds yet.", x.agent.name()), p), p));
        } else {
            let rows: Vec<_> = c.by_kind.iter().map(|(k, t)| (k.name().to_string(), t / c.total_tokens.max(1e-9), tokens(*t), String::new())).collect();
            let mut body = col().w_full().gap(10.0).child(bar_rows(&rows, p.neutral, p));
            if !c.largest.is_empty() {
                let rows = c
                    .largest
                    .iter()
                    .take(8)
                    .map(|b| {
                        let what = [b.tool.as_deref(), b.target.as_deref()].into_iter().flatten().collect::<Vec<_>>().join(" ");
                        let when = if matches!(b.kind, BlockKind::Tools | BlockKind::System) { "setup".to_string() } else { format!("turn {}", b.turn) };
                        vec![plain(tokens(b.tokens)), plain(b.kind.name()), plain(what), (when, Some(p.ink2))]
                    })
                    .collect();
                body = body.child(table(
                    &[Column("tokens", 70.0, true), Column("kind", 120.0, false), Column("largest blocks", 0.0, false), Column("when", 80.0, false)],
                    rows,
                    p,
                ));
            }
            out.push(boxed("held", "In context", Some(aside), body, p));
        }
    }
    let mut rows: Vec<_> = x.content.iter().collect();
    let key = |v: &Values| if x.priced { v.usd } else { v.tokens };
    rows.sort_by(|a, b| key(&b.1).total_cmp(&key(&a.1)));
    if !rows.is_empty() {
        let rows = rows
            .into_iter()
            .take(10)
            .map(|((kind, tool, target), v)| {
                let what = [tool.as_deref(), target.as_deref()].into_iter().flatten().collect::<Vec<_>>().join(" ");
                vec![plain(amount(x, v)), plain(kind.name()), plain(what)]
            })
            .collect();
        out.push(boxed(
            "costly",
            "Cost to keep",
            Some("what cost the most to keep in the context, with every request that re-read it".into()),
            table(&[Column("cost", 70.0, true), Column("kind", 120.0, false), Column("source", 0.0, false)], rows, p),
            p,
        ));
    }
    if let Some(v) = &x.setup {
        let show = |c: &Values| if v.priced { usd(c.usd) } else { format!("{} tok", tokens(c.tokens)) };
        let rows = v.rows.iter().take(10).map(|(name, cost)| vec![plain(name.clone()), plain(show(&cost.all))]).collect();
        let aside = format!("what every request carries before the work: {}, {:.0}% of the session", show(&v.total.all), 100.0 * v.share);
        out.push(boxed("setup", "Setup", Some(aside), table(&[Column("source", 0.0, false), Column("cost", 90.0, true)], rows, p), p));
    }
    if out.is_empty() {
        out.push(note("The log does not say what the context holds.", p));
    }
    out
}

fn cache(x: &XRay, p: &Pal) -> Vec<El> {
    let misses: Vec<_> = x.misses.iter().filter(|m| !m.cause.expected()).collect();
    if x.misses.is_empty() || misses.is_empty() {
        // Only rebuilds the session asked for, as a compaction or a change of model does.
        let mut causes: Vec<&str> = x.misses.iter().map(|m| m.cause.name()).collect();
        causes.sort_unstable();
        causes.dedup();
        let line = match x.misses.len() {
            0 => "every request after the first read its prefix from the cache".to_string(),
            n => format!("{} ({})", count(n as u64, "expected rebuild", "expected rebuilds"), causes.join(", ")),
        };
        return vec![overlay::empty("misses", "No cache misses".into(), Some(overlay::empty_line(&line, p)), p)];
    }
    let rewritten: u64 = misses.iter().map(|m| m.rewritten).sum();
    let premium: f64 = misses.iter().filter_map(|m| m.premium_usd).sum();
    let expected = x.misses.len() - misses.len();
    let mut head = format!("{}, re-processed {} tokens", count(misses.len() as u64, "miss", "misses"), tokens(rewritten as f64));
    if x.priced {
        head += &format!(", {} avoidable", usd(premium));
    }
    if expected > 0 {
        head += &format!(", {}", count(expected as u64, "expected rebuild", "expected rebuilds"));
    }
    let mut top = misses.clone();
    top.sort_by(|a, b| b.premium_usd.unwrap_or(0.0).total_cmp(&a.premium_usd.unwrap_or(0.0)).then(b.rewritten.cmp(&a.rewritten)));
    let rows = top
        .into_iter()
        .take(12)
        .map(|m| {
            let cause = if m.detail.is_empty() { m.cause.name().to_string() } else { format!("{}: {}", m.cause.name(), m.detail) };
            vec![
                plain(local_time(m.ts_ms, "%m-%d %H:%M")),
                plain(cause),
                plain(tokens(m.rewritten as f64)),
                plain(m.premium_usd.map_or("-".to_string(), usd)),
                (m.chain.clone(), Some(p.ink2)),
            ]
        })
        .collect();
    let cols = [
        Column("when", 90.0, false),
        Column("cause", 0.0, false),
        Column("re-processed", 90.0, true),
        Column("avoidable", 80.0, true),
        Column("chain", 120.0, false),
    ];
    let color = if misses.is_empty() { p.ink } else { p.cold };
    let body = col().w_full().gap(8.0).child(text(head).w_full().wrap().font(pal::STRONG).color(color).selectable(true)).child(table_dropping(
        &cols,
        rows,
        &["chain"],
        p,
    ));
    vec![boxed("misses", "Cache misses", None, body, p)]
}

#[cfg(test)]
mod tests {
    use super::*;

    const HOUR: i64 = 3_600_000;

    #[test]
    fn the_axis_runs_to_now_and_the_time_after_the_last_request_takes_at_most_eight_percent() {
        // A session from 10:27 to 11:09, cold at 12:09, now 11:12: one axis to now.
        let (first, last, cold_at) = (10 * HOUR + 27 * 60_000, 11 * HOUR + 9 * 60_000, 12 * HOUR + 9 * 60_000);
        let now = 11 * HOUR + 12 * 60_000;
        let a = Axis::of(first, last, cold_at, now);
        assert!(!a.broken() && a.begin == 10 * HOUR + 20 * 60_000, "{a:?}");
        assert_eq!(a.at(now), 0.97, "now a little before the end");
        assert!((a.at(last) - 0.97 * 49.0 / 52.0).abs() < 1e-4, "11:09 is 49 minutes of 52 along");
        // At 14:33 the time after the last request would take most of the axis: it breaks, and
        // the warm hour and the cold time after it share the last 8%.
        let later = Axis::of(first, last, cold_at, 14 * HOUR + 33 * 60_000);
        assert!(later.broken(), "{later:?}");
        assert!((later.at(last) - 0.89).abs() < 1e-4 && (later.at(cold_at) - 0.93).abs() < 1e-4 && (later.at(later.now) - 0.97).abs() < 1e-4);
        assert!((later.at(10 * HOUR + 48 * 60_000) - 0.89 * 28.0 / 49.0).abs() < 1e-4, "the work keeps 89%");
        // Three days later, the same.
        let days = Axis::of(first, last, cold_at, 14 * HOUR + 72 * HOUR);
        assert!(days.broken() && (days.at(cold_at) - 0.93).abs() < 1e-4);
    }

    #[test]
    fn the_ways_come_cheapest_first_each_at_its_first_price() {
        use tokprof_core::advice::Advice;
        let line = |topic: Topic, ways: Vec<(Way, f64)>| Advice { topic, text: String::new(), ways };
        let advice = [
            line(Topic::CarryOn, vec![(Way::Resume, 0.84)]),
            line(Topic::CarryOn, vec![(Way::Handoff, 0.52)]),
            line(Topic::CarryOn, vec![(Way::Compact, 1.01)]),
            line(Topic::CarryOn, vec![(Way::Handoff, 9.0)]),
            line(Topic::Reminders, Vec::new()),
        ];
        assert_eq!(ways(&advice), [(Way::Handoff, 0.52), (Way::Resume, 0.84), (Way::Compact, 1.01)], "the handoff is the cheapest, marked first");
        assert!(ways(&advice[4..]).is_empty(), "advice on other things prices no way");
    }

    #[test]
    fn the_largest_request_s_label_keeps_clear_of_the_next_one_s() {
        // Far from the next request's label: over its bar.
        assert_eq!(dear_label_x(100.0, 40.0, 0.0, Some((600.0, 40.0))), Some(86.0));
        // Right under it: left of its bar, or nowhere when that too would cover it.
        assert_eq!(dear_label_x(610.0, 40.0, 0.0, Some((630.0, 40.0))), Some(566.0));
        assert_eq!(dear_label_x(640.0, 40.0, 0.0, Some((600.0, 40.0))), None);
        // Lower down, where the other label is not.
        assert_eq!(dear_label_x(640.0, 40.0, 30.0, Some((600.0, 40.0))), Some(626.0));
    }

    #[test]
    fn narrow_tables_leave_out_their_least_columns_first() {
        let cols = [(0.0, "tool"), (50.0, "calls"), (70.0, "failed"), (70.0, "after"), (230.0, "hint")];
        let drop = ["hint", "after", "failed"];
        assert_eq!(fitting(&cols, &drop, 900.0), [true; 5], "room for all");
        // 120 + 50 + 70 + 70 + 230 and four gaps of 12 are 588.
        assert_eq!(fitting(&cols, &drop, 587.0), [true, true, true, true, false], "the hint goes first");
        assert_eq!(fitting(&cols, &drop, 300.0), [true, true, true, false, false], "then after");
        assert_eq!(fitting(&cols, &drop, 250.0), [true, true, false, false, false], "then failed");
        assert_eq!(fitting(&cols, &drop, 10.0), [true, true, false, false, false], "the others stay");
    }

    #[test]
    fn the_tabs_stay_on_one_line_and_show_their_digits_where_there_is_room() {
        assert_eq!(tab_fit(1019.0), (8.0, true), "the inspector of a 1400-point window");
        let (pad, digits) = tab_fit(579.0);
        assert!(!digits && pad < 8.0, "at 960 the tabs come closer and show no digits: {pad}");
        let labels: f32 = Tab::ALL.iter().map(|t| pal::text_w(t.name(), pal::HEAD)).sum();
        assert!(labels + 20.0 * pad + 9.0 + 20.0 <= 579.0, "all ten fit");
    }

    #[test]
    fn tabs_are_named_as_the_x_ray_blocks() {
        assert_eq!(Tab::named("prompts"), Some(Tab::Prompts));
        assert_eq!(Tab::named("nothing"), None);
        assert_eq!(Tab::ALL.map(Tab::name).join(" "), "Summary Requests Spend Work Waits Prompts Tools Context Cache Flame");
        assert_eq!(Tab::named("flame"), Some(Tab::Flame));
    }
}
