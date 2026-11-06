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
}

impl Tab {
    pub const ALL: [Tab; 9] = [Tab::Summary, Tab::Requests, Tab::Spend, Tab::Work, Tab::Waits, Tab::Prompts, Tab::Tools, Tab::Context, Tab::Cache];

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
        (b.xray().map(|x| x as *const XRay as usize), pending, s.tab, theme, selected),
        (b.zoomed(), cx.layout_size(TRACK).map(|sz| sz[0].to_bits()), view::minute(), keyboard(s), s.cursor),
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
    let line = line.key(format!("row-{n}")).radius(5.0).on_press(move |s: &mut Desk| {
        s.pane = crate::Focus::Inspector;
        s.cursor = n;
    });
    if cursor == Some(n) { line.fill(p.sel) } else { line.hover(p.hover) }
}

pub fn inspector(s: &Desk, cx: &Cx<Desk>, p: &Pal) -> El {
    let tabs = row().key("tabs").w_full().h(38.0).px(12.0).children(Tab::ALL.into_iter().map(|t| {
        let on = s.tab == t;
        col()
            .key(format!("tab-{}", t.name()))
            .h(38.0)
            .px(7.0)
            .justify_center()
            .role("tab")
            .checked(on)
            .label(t.name())
            .hover(p.hover)
            .on_press_tx(move |s: &mut Desk, tx| {
                s.show_tab(t, tx);
                s.pane = crate::Focus::Inspector;
            })
            .child(spacer().grow(1.0))
            .child(text(t.name()).font(pal::HEAD).color(if on { p.ink } else { p.ink2 }).single_line())
            .child(spacer().grow(1.0))
            .child(el().w_full().h(2.0).radius(1.0).fill(if on { p.accent } else { Color::TRANSPARENT }))
    }));
    ROWS.set((0, keyboard(s).then_some(s.cursor)));
    let body = match s.browser.xray() {
        Some(x) => match s.tab {
            Tab::Summary => summary(s, x, cx, p),
            Tab::Requests => requests(s, x, cx, p),
            Tab::Spend => spend_tab(x, p),
            Tab::Work => work(x, p),
            Tab::Waits => waits(x, p),
            Tab::Prompts => prompts(x, p),
            Tab::Tools => tools(x, p),
            Tab::Context => context(x, p),
            Tab::Cache => cache(x, p),
        },
        None => match s.browser.report() {
            Pane::Text(t) => vec![note(t.text.clone(), p)],
            Pane::Computing(n) => vec![note(n, p)],
            Pane::Empty(n) => vec![note(n, p)],
        },
    };
    // Summary's ways are rows of a box, not of a list the cursor moves through.
    s.tab_rows.set(if s.tab == Tab::Summary { 0 } else { ROWS.get().0 });
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
    col()
        .key("inspector")
        .grow(1.0)
        .h_full()
        .min_w(0.0)
        .min_h(0.0)
        .child(view::focus_line("inspector-focus", keyboard(s) && !s.browser.zoomed(), p))
        .child(tabs)
        .child(el().w_full().h(1.0).fill(p.line))
        .child(pane)
}

fn note(s: impl Into<String>, p: &Pal) -> El {
    text(s.into()).w_full().wrap().font(pal::BODY).color(p.ink2).selectable(true)
}

/// A box with a header, and a note at the header's right.
fn boxed(key: &str, title: &str, aside: Option<String>, body: El, p: &Pal) -> El {
    let mut head = row().w_full().gap(8.0).center_y().child(text(title.to_string()).font(pal::HEAD).color(p.ink2).single_line());
    if let Some(a) = aside {
        head = head.child(spacer().grow(1.0)).child(text(a).key("aside").font(pal::SMALL).color(p.ink3).single_line().truncate().shrink().min_w(0.0));
    }
    col().key(key.to_string()).w_full().min_w(0.0).px(13.0).pt(11.0).pb(12.0).gap(9.0).radius(9.0).fill(p.panel).border(1.0, p.line).child(head).child(body)
}

/// A bar `share` of the way along its track.
fn meter(share: f64, color: Color, p: &Pal, h: f32) -> El {
    let share = share.clamp(0.0, 1.0) as f32;
    row()
        .grow(1.0)
        .min_w(0.0)
        .h(h)
        .radius(h / 2.0)
        .fill(p.hover)
        .child(el().h(h).radius(h / 2.0).fill(color).grow(share.max(1e-4)).min_w(if share > 0.0 { 2.0 } else { 0.0 }))
        .child(el().h(h).grow((1.0 - share).max(1e-4)).min_w(0.0))
}

/// Rows of a bar chart: a label, a bar as long as its share of the largest, the share, the value
/// and a note.
fn bar_rows(rows: &[(String, f64, String, String)], color: Color, p: &Pal) -> El {
    let max = rows.iter().map(|r| r.1).fold(0.0, f64::max).max(1e-9);
    col().w_full().gap(4.0).children(rows.iter().map(|(label, share, value, note)| {
        let mut line = row()
            .w_full()
            .h(20.0)
            .gap(10.0)
            .center_y()
            .child(text(label.clone()).font(pal::BODY).color(p.ink).single_line().truncate().w(190.0))
            .child(meter(share / max, color, p, 6.0))
            .child(row().w(50.0).justify_end().child(text(format!("{:.1}%", 100.0 * share)).font(pal::BODY_NUM).color(p.ink2).single_line()))
            .child(row().w(64.0).justify_end().child(text(value.clone()).font(pal::STRONG_NUM).color(p.ink).single_line()));
        if !note.is_empty() {
            line = line.child(text(note.clone()).font(pal::SMALL).color(p.ink3).single_line().truncate().w(150.0));
        }
        numbered(line, p)
    }))
}

/// A column of a table: its title, its width (0 for the one that takes the rest), and whether
/// its cells are numbers, set right.
struct Column(&'static str, f32, bool);

fn table(cols: &[Column], rows: Vec<Vec<(String, Option<Color>)>>, p: &Pal) -> El {
    let cell = |t: String, c: &Column, color: Color, font| {
        let t = text(t).font(font).color(color).single_line().truncate().min_w(0.0);
        match (c.1, c.2) {
            (0.0, _) => row().grow(1.0).min_w(0.0).child(t),
            (w, true) => row().w(w).justify_end().child(t),
            (w, false) => row().w(w).child(t),
        }
    };
    let head = row().w_full().h(22.0).gap(12.0).center_y().children(cols.iter().map(|c| cell(c.0.to_string(), c, p.ink2, pal::SMALL_STRONG)));
    let mut out = col().w_full().child(head).child(el().w_full().h(1.0).fill(p.line));
    for r in rows {
        let line = row().w_full().h(24.0).gap(12.0).center_y().children(r.into_iter().zip(cols).map(|((t, color), c)| {
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

fn component_color(c: Component, p: &Pal) -> Color {
    match c {
        Component::Read => p.read,
        Component::Write => p.write,
        Component::Rewrite => p.cold,
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
        el().h(8.0).radius(2.0).grow(*share as f32).min_w(2.0).fill(component_color(*c, p)).help(format!("{}: {:.1}%", c.name(), 100.0 * share))
    }));
    let legend = col().w_full().gap(5.0).children(parts.iter().map(|(c, share, v)| {
        row()
            .w_full()
            .gap(8.0)
            .center_y()
            .child(el().w(9.0).h(9.0).radius(2.0).fill(component_color(*c, p)))
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
        .child(text(x.project.clone()).font(pal::STRONG).color(p.ink).single_line().truncate_head().shrink().min_w(0.0))
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
        .child(view::button("handoff", "Copy handoff line", true, p).on_press(|s: &mut Desk| {
            let view = s.viewport();
            s.browser.on_key(tokprof_browse::browser::Key::Char('c'), view);
        }))
}

/// The time axis of the request track: from the first request to now. When the session went
/// cold long ago, the axis breaks after its cache went cold and keeps its last part for now.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Axis {
    pub begin: i64,
    /// Where the axis breaks, `now` when it does not.
    pub cut: i64,
    pub now: i64,
}

const TEN_MIN: i64 = 600_000;

impl Axis {
    pub fn of(first: i64, cold_at: i64, now: i64) -> Axis {
        let begin = first - first.rem_euclid(TEN_MIN);
        let life = cold_at.max(first + TEN_MIN) - begin;
        if now - begin <= 3 * life.max(3_600_000) { Axis { begin, cut: now, now } } else { Axis { begin, cut: begin + life + life / 6, now } }
    }

    pub fn broken(&self) -> bool {
        self.cut < self.now
    }

    /// Where a moment is along the axis, from 0 to 1; after a break, the time between the break
    /// and now is left out.
    pub fn at(&self, ts: i64) -> f32 {
        // Now sits a little before the end, so that the next request fits.
        if !self.broken() {
            return 0.97 * ((ts - self.begin) as f64 / (self.now - self.begin).max(1) as f64).clamp(0.0, 1.0) as f32;
        }
        if ts <= self.cut { 0.8 * ((ts - self.begin) as f64 / (self.cut - self.begin).max(1) as f64).clamp(0.0, 1.0) as f32 } else { 0.97 }
    }
}

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
    let axis = Axis::of(first, cold_at, now);
    let width = cx.layout_size(TRACK).map_or(640.0, |sz| sz[0]).max(100.0);
    let xs = |ts: i64| axis.at(ts) * width;
    let top = x.points.iter().map(|q| q.usd).fold(0.0, f64::max).max(next.unwrap_or(0.0)).max(1e-9);
    let height = |usd: f64| ((usd / top) as f32 * tall).max(2.0);
    let cold = now >= cold_at;
    let mut track = el().key(TRACK).w_full().h(tall + 18.0);
    // The time the cache stayed warm, and the time it has been cold.
    let warm_end = cold_at.min(now);
    track = track.child(el().key("warm-span").absolute(xs(last), 18.0).w((xs(warm_end) - xs(last)).max(1.0)).h(tall).fill(p.tail.alpha(0.45)));
    if cold {
        let x0 = xs(cold_at);
        track = track.child(el().key("cold-span").absolute(x0, 18.0).w((xs(now) - x0).max(1.0)).h(tall).fill(p.cold.alpha(0.07)));
    }
    if axis.broken() {
        let x0 = 0.8 * width;
        track = track.child(text("…").font(pal::BODY).color(p.ink3).absolute(x0 + 0.075 * width, 18.0 + tall / 2.0 - 8.0));
    }
    for (i, q) in x.points.iter().enumerate() {
        let h = height(q.usd);
        track = track.child(el().key(format!("request-{i}")).absolute(xs(q.ts_ms) - 1.5, 18.0 + tall - h).w(3.0).h(h).radius(1.5).fill(p.bars).help(format!(
            "{}: {}, {} in context",
            local_time(q.ts_ms, "%H:%M:%S"),
            usd(q.usd),
            tokens(q.prompt_tokens as f64)
        )));
    }
    if let Some(n) = next {
        let h = height(n);
        let c = if cold { p.cold } else { p.warm };
        let x0 = xs(now);
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
    }
    if let Some(dear) = x.points.iter().max_by(|a, b| a.usd.total_cmp(&b.usd)) {
        track = track
            .child(text(usd(dear.usd)).font(pal::SMALL_STRONG).color(p.ink).single_line().absolute(xs(dear.ts_ms) - 14.0, (tall - height(dear.usd)).max(0.0)));
    }
    // The times under the axis, each where there is room.
    let mut marks: Vec<(f32, String, &'static str)> =
        vec![(xs(first), local_time(first, "%H:%M"), "start"), (xs(last), local_time(last, "%H:%M"), "last request")];
    marks.push((xs(cold_at), local_time(cold_at, "%H:%M"), if cold { "cache cold" } else { "cold then" }));
    let mut axis_row = el().w_full().h(30.0);
    let mut taken = f32::MIN;
    for (x0, t, what) in marks {
        if x0 - taken >= 84.0 && x0 <= width - 150.0 {
            axis_row = axis_row.child(
                col()
                    .absolute(x0, 3.0)
                    .child(text(t).font(pal::SMALL_STRONG).color(p.ink).single_line())
                    .child(text(what).font(pal::SMALL).color(p.ink2).single_line()),
            );
            taken = x0;
        }
    }
    axis_row = axis_row.child(
        col()
            .absolute(width - 120.0, 3.0)
            .w(120.0)
            .items_end()
            .child(text(local_time(now, "%H:%M")).font(pal::SMALL_STRONG).color(p.ink).single_line())
            .child(text("now, next request").font(pal::SMALL).color(p.ink2).single_line()),
    );
    let since = format!("{} to now", local_time(axis.begin, "%H:%M"));
    let body = col().w_full().child(track).child(el().w_full().h(1.0).fill(p.line)).child(axis_row);
    Some(boxed("requests-track", "Requests and the cache", Some(since), body, p))
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
            row().w_full().gap(8.0).center_y().child(meter(share, p.neutral, p, 6.0)).child(
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
                .radius(7.0)
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
            .children([next_box(s, x, p), ways_box(x, p), spend_box(x, p)].map(|b| col().grow(1.0).min_w(0.0).child(b))),
    );
    out.extend(tips(x, p));
    out
}

fn requests(s: &Desk, x: &XRay, cx: &Cx<Desk>, p: &Pal) -> Vec<El> {
    let mut out: Vec<El> = request_track(s, x, cx, p, 120.0).into_iter().collect();
    let ttl = x.context.as_ref().map_or(i64::MAX, |c| c.cache.ttl_ms);
    let shown = x.points.len().saturating_sub(300);
    let rows = x.points.iter().enumerate().skip(shown).map(|(i, q)| {
        let gap = i.checked_sub(1).map(|j| q.ts_ms - x.points[j].ts_ms);
        let after = gap.map_or(String::new(), |g| human_duration(g as f64 / 1000.0));
        let cold = gap.is_some_and(|g| g >= ttl);
        vec![
            plain((i + 1).to_string()),
            plain(local_time(q.ts_ms, "%m-%d %H:%M:%S")),
            (after, cold.then_some(p.cold)),
            plain(tokens(q.prompt_tokens as f64)),
            plain(usd(q.usd)),
        ]
    });
    let cols =
        [Column("#", 40.0, true), Column("sent", 120.0, false), Column("after", 70.0, true), Column("in context", 90.0, true), Column("cost", 70.0, true)];
    let mut body = col().w_full().gap(8.0).child(table(&cols, rows.collect(), p));
    if shown > 0 {
        body = body.child(note(format!("and {shown} earlier requests"), p));
    }
    let aside = format!("{} of the main conversation; an `after` longer than the cache lives is in red", count(x.points.len() as u64, "request", "requests"));
    out.push(boxed("requests", "Requests", Some(aside), body, p));
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
            table(&[Column("cost", 70.0, true), Column("task", 0.0, false), Column("origin", 180.0, false)], rows, p),
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
    vec![boxed("prompts", "Prompts", Some("what each message you typed cost, subagents included".into()), table(&cols, rows, p), p)]
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
    let mut body = col().w_full().gap(8.0).child(table(&cols, rows, p));
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
                        let what = [b.tool.as_deref(), b.target.as_deref()].into_iter().flatten().collect::<Vec<_>>().join("  ");
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
                let what = [tool.as_deref(), target.as_deref()].into_iter().flatten().collect::<Vec<_>>().join("  ");
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
    if x.misses.is_empty() {
        return vec![boxed("misses", "Cache misses", None, text("No misses").font(pal::STRONG).color(p.warm), p)];
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
    let body = col().w_full().gap(8.0).child(text(head).w_full().wrap().font(pal::STRONG).color(color).selectable(true)).child(table(&cols, rows, p));
    vec![boxed("misses", "Cache misses", None, body, p)]
}

#[cfg(test)]
mod tests {
    use super::*;

    const HOUR: i64 = 3_600_000;

    #[test]
    fn the_axis_runs_to_now_and_breaks_after_a_long_cold_time() {
        // A session from 10:27 to 11:09, cold at 12:09, now 14:33: one axis to now.
        let (first, cold_at, now) = (10 * HOUR + 27 * 60_000, 12 * HOUR + 9 * 60_000, 14 * HOUR + 33 * 60_000);
        let a = Axis::of(first, cold_at, now);
        assert!(!a.broken() && a.begin == 10 * HOUR + 20 * 60_000, "{a:?}");
        assert_eq!(a.at(now), 0.97, "now a little before the end");
        assert!((a.at(cold_at) - 0.97 * 109.0 / 253.0).abs() < 1e-4, "12:09 is 109 minutes of 253 along");
        // Three days later the axis breaks: the session keeps 80% of the track, now its end.
        let later = Axis::of(first, cold_at, now + 72 * HOUR);
        assert!(later.broken() && later.at(cold_at) < 0.8 && later.at(now + 72 * HOUR) > 0.9, "{later:?}");
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
    fn tabs_are_named_as_the_x_ray_blocks() {
        assert_eq!(Tab::named("prompts"), Some(Tab::Prompts));
        assert_eq!(Tab::named("nothing"), None);
        assert_eq!(Tab::ALL.map(Tab::name).join(" "), "Summary Requests Spend Work Waits Prompts Tools Context Cache");
    }
}
