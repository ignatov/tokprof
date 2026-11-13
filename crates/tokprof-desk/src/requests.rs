//! The `Requests` tab: the request timeline of the main conversation (a stacked column per
//! request in the X-ray's five components, the context line, markers, the subagents' band), the
//! card of the selected request or of a range, and the table of the requests. The timeline and
//! the card stay above the table while it scrolls.

use rune::{Color, Cx, Drag, El, KeyEvent, Phase, Shape, Tx, canvas, col, el, row, spacer, text};
use tokprof_browse::browser::now_ms;
use tokprof_core::analysis::Component;
use tokprof_core::fmt::{count, human_duration, local_time, tokens, usd};
use tokprof_core::timeline::{self, MarkKind, RequestPoint};
use tokprof_core::xray::XRay;

use crate::Desk;
use crate::inspector::{self, Axis, Tab};
use crate::overlay::{PILL, R_GROUP, R_MARK};
use crate::pal::{self, Pal};

/// The key of the chart, whose width the columns are placed in.
pub const CHART: &str = "timeline-chart";
/// Requests whose columns fall within this many points share one.
pub const SLOT: f32 = 4.0;
/// A miss whose avoidable premium is at least this is red.
pub const RED_USD: f64 = 0.50;
/// The inspector's least height, in points, for the timeline to stay above the table.
pub const PIN_H: f32 = 760.0;
/// The rows the table holds at most.
pub const PAGE: usize = 300;
/// Entered groups the card lists.
const ENTERED_SHOWN: usize = 8;
const CHART_H: f32 = 124.0;
/// The room of the context scale at the chart's right.
const RIGHT_SCALE: f32 = 36.0;
/// A label keeps this clear of the label before it.
const LABEL_GAP: f32 = 8.0;
/// A drag shorter than this is a click.
const DRAG_MIN: f32 = 4.0;

/// The tab's state: for which session, the selected request, a range, the column under the
/// pointer, the unit, where a drag started and how wide the chart was laid out.
#[derive(Clone, Debug, Default)]
pub struct State {
    pub session: String,
    pub sel: Option<usize>,
    /// The range's anchor and its other end, as indices of requests.
    pub range: Option<(usize, usize)>,
    pub hover: Option<Hover>,
    pub tokens: bool,
    pub drag: Option<f32>,
    pub width: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Hover {
    /// The slot under the pointer.
    Slot(i64),
    /// The selected request's column, for a frame without a pointer.
    Selected,
}

impl State {
    /// What the tab draws from, for the inspector's memo.
    pub fn version(&self) -> impl std::hash::Hash {
        (self.session.clone(), self.sel, self.range, self.hover, self.tokens)
    }
}

/// The selected request and the range of the X-ray shown: the costliest request and no range
/// for a session the state is not about.
pub fn current(st: &State, x: &XRay) -> (usize, Option<(usize, usize)>) {
    let n = x.points.len();
    if st.session != x.id || n == 0 {
        return (costliest(&x.points), None);
    }
    let sel = st.sel.unwrap_or_else(|| costliest(&x.points)).min(n - 1);
    (sel, st.range.map(|(a, b)| (a.min(n - 1), b.min(n - 1))))
}

/// The costliest request, by dollars, or by tokens when nothing is priced.
pub fn costliest(points: &[RequestPoint]) -> usize {
    let priced = points.iter().any(|p| p.priced);
    let value = |p: &RequestPoint| if priced { p.usd } else { p.total(false) };
    points.iter().enumerate().max_by(|a, b| value(a.1).total_cmp(&value(b.1)).then(b.0.cmp(&a.0))).map_or(0, |(i, _)| i)
}

/// The rows of the table: all, or the 300 that end at the latest request, or the 300-row page
/// that holds the selected one.
pub fn page(n: usize, sel: usize) -> (usize, usize) {
    if n <= PAGE {
        return (0, n);
    }
    let lo = (n - PAGE).min(sel - sel % PAGE);
    (lo, lo + PAGE)
}

/// Requests that share a column: their indices, the sums of their parts and the last context.
#[derive(Clone, Debug, PartialEq)]
pub struct Column {
    pub slot: i64,
    pub first: usize,
    pub last: usize,
    pub parts: [f64; 5],
    pub context: u64,
}

impl Column {
    pub fn x(&self) -> f32 {
        self.slot as f32 * SLOT
    }

    pub fn total(&self) -> f64 {
        self.parts.iter().sum()
    }

    pub fn holds(&self, i: usize) -> bool {
        (self.first..=self.last).contains(&i)
    }
}

/// The columns of the requests at `xs` of their times, in dollars or tokens.
pub fn columns(points: &[RequestPoint], xs: impl Fn(i64) -> f32, dollars: bool) -> Vec<Column> {
    let mut out: Vec<Column> = Vec::new();
    for (i, p) in points.iter().enumerate() {
        let slot = (xs(p.ts_ms) / SLOT).floor() as i64;
        let parts = p.parts.map(|v| if dollars { v.usd } else { v.tokens });
        match out.last_mut() {
            Some(c) if c.slot == slot => {
                c.last = i;
                c.parts.iter_mut().zip(parts).for_each(|(a, b)| *a += b);
                c.context = p.prompt_tokens;
            }
            _ => out.push(Column { slot, first: i, last: i, parts, context: p.prompt_tokens }),
        }
    }
    out
}

/// The costliest request of a column.
fn costliest_in(points: &[RequestPoint], c: &Column) -> usize {
    c.first + costliest(&points[c.first..=c.last])
}

/// The column at `x` points, or the nearest within a slot.
pub fn column_at(cols: &[Column], x: f32) -> Option<usize> {
    let slot = (x / SLOT).floor() as i64;
    cols.iter().enumerate().filter(|(_, c)| (c.slot - slot).abs() <= 1).min_by_key(|(_, c)| (c.slot - slot).abs()).map(|(i, _)| i)
}

/// The axis and the cache's times of the X-ray shown: as Summary's track draws them.
struct Frame {
    axis: Axis,
    last: i64,
    cold_at: i64,
    now: i64,
    next: Option<f64>,
}

fn frame(s: &Desk, x: &XRay) -> Option<Frame> {
    let first = x.points.first()?.ts_ms;
    let now = now_ms();
    let session = s.browser.selected().map(|r| s.browser.session(r).to_string()).unwrap_or_default();
    let (last, cold_at) = s.browser.cache_life(&session).unwrap_or_else(|| {
        let last = x.points.last().map_or(first, |q| q.ts_ms);
        (last, x.context.as_ref().map_or(last, |c| last.saturating_add(c.cache.ttl_ms)))
    });
    let next = s.browser.selected().and_then(|r| s.browser.cells(r, now).next_usd);
    Some(Frame { axis: Axis::of(first, last, cold_at, now), last, cold_at, now, next })
}

fn width(s: &Desk, cx: Option<&Cx<Desk>>) -> f32 {
    cx.and_then(|cx| cx.layout_size(CHART).map(|sz| sz[0])).unwrap_or(if s.requests.width > 0.0 { s.requests.width } else { 640.0 }).max(100.0)
}

/// The width the time axis takes in a chart `w` wide: the right scale keeps the rest.
fn plot(w: f32) -> f32 {
    w - RIGHT_SCALE
}

/// Whether the parts are dollars.
fn dollars(s: &Desk, x: &XRay) -> bool {
    x.priced && !s.requests.tokens
}

/// The part's color, as the Spend card draws it.
fn part_color(i: usize, p: &Pal) -> Color {
    inspector::component_color(Component::ALL[i], p)
}

fn amount(v: f64, dollars: bool) -> String {
    if dollars { usd(v) } else { tokens(v) }
}

/// The tab: the timeline and the card, which stay above the table, and the table.
pub fn tab(s: &Desk, x: &XRay, cx: &Cx<Desk>, p: &Pal) -> (Option<El>, Vec<El>) {
    if x.points.is_empty() {
        return (None, vec![text("no requests").font(pal::BODY).color(p.ink2)]);
    }
    let (sel, range) = current(&s.requests, x);
    let top = col().w_full().gap(12.0).children(chart(s, x, cx, sel, range, p)).child(card(x, sel, range, dollars(s, x), p));
    (Some(top), vec![table(s, x, sel, p)])
}

/// The timeline card.
fn chart(s: &Desk, x: &XRay, cx: &Cx<Desk>, sel: usize, range: Option<(usize, usize)>, p: &Pal) -> Option<El> {
    let f = frame(s, x)?;
    let w = width(s, Some(cx));
    let xs = |ts: i64| f.axis.at(ts) * plot(w);
    let dollars = dollars(s, x);
    let cols = columns(&x.points, xs, dollars);
    let next = f.next.filter(|_| dollars);
    // The requests set the scale; a next request taller than it is cut, with a mark.
    let top = cols.iter().map(Column::total).fold(0.0, f64::max).max(1e-9);
    let window = x.context.as_ref().and_then(|c| c.window_tokens).filter(|&w| w > 0);
    let ctx_top = window.map_or_else(|| x.points.iter().map(|q| q.prompt_tokens).max().unwrap_or(1).max(1) as f64, |w| w as f64);
    let h = |v: f64| (v / top) as f32 * CHART_H;

    // The columns and the context line, in one canvas.
    let mut shape = Shape::new();
    for c in &cols {
        let mut y = CHART_H;
        for (i, v) in c.parts.iter().enumerate() {
            let ph = h(*v);
            if ph <= 0.0 {
                continue;
            }
            let x0 = c.x();
            shape = shape.fill(vec![[x0, y - ph], [x0 + SLOT - 1.0, y - ph], [x0 + SLOT - 1.0, y], [x0, y]], part_color(i, p));
            y -= ph;
        }
    }
    let line: Vec<[f32; 2]> = cols.iter().map(|c| [c.x() + (SLOT - 1.0) / 2.0, CHART_H - (c.context as f64 / ctx_top).min(1.0) as f32 * CHART_H]).collect();
    shape = shape.stroke(line, 1.5, p.ink2.alpha(0.8));
    let sel_col = cols.iter().position(|c| c.holds(sel));
    if let Some(c) = sel_col.map(|i| &cols[i]) {
        let (x0, y0) = (c.x() - 1.5, (CHART_H - h(c.total()) - 2.0).max(0.0));
        shape = shape.stroke_with(
            vec![[x0, y0], [x0 + SLOT + 2.0, y0], [x0 + SLOT + 2.0, CHART_H + 2.0], [x0, CHART_H + 2.0]],
            true,
            1.5,
            rune::LineCap::Butt,
            p.accent,
        );
    }

    let mut chart = el().key(CHART).w_full().h(CHART_H + 2.0);
    let cold = f.now >= f.cold_at;
    // A range, shaded.
    if let Some((a, b)) = range {
        let (lo, hi) = (a.min(b), a.max(b));
        let x0 = cols.iter().find(|c| c.holds(lo)).map_or(0.0, Column::x);
        let x1 = cols.iter().find(|c| c.holds(hi)).map_or(w, |c| c.x() + SLOT);
        chart = chart.child(
            el().key("range")
                .absolute(x0 - 1.0, 0.0)
                .w((x1 - x0 + 2.0).max(2.0))
                .h(CHART_H)
                .radius(R_MARK)
                .fill(p.accent.alpha(0.12))
                .border(1.0, p.accent.alpha(0.5)),
        );
    }
    chart = chart.child(canvas(shape).key("columns").absolute(0.0, 0.0).w(w).h(CHART_H + 2.0));
    // The miss and compaction lines.
    // Each label with its weight: a miss's avoidable dollars; a compaction weighs least.
    let mut labels: Vec<(f32, String, Color, f64)> = Vec::new();
    for (i, q) in x.points.iter().enumerate() {
        let Some(m) = q.miss.as_ref().filter(|m| !m.cause.expected()) else { continue };
        let col = cols.iter().find(|c| c.holds(i));
        let at = col.map_or(xs(q.ts_ms), |c| c.x() + (SLOT - 1.0) / 2.0);
        let color = if red(m) { p.cold } else { p.write };
        // The line stands over the column only, so that the column's height reads.
        let bar_top = CHART_H - col.map_or(0.0, |c| h(c.total()));
        let line_h = (bar_top - 3.0).max(0.0);
        if line_h > 0.0 {
            chart = chart.child(el().key(format!("miss-{i}")).absolute(at - 0.5, 0.0).w(1.0).h(line_h).fill(color.alpha(0.8)));
        }
        labels.push((at, m.cause.name().to_string(), color, m.premium_usd.unwrap_or(0.0)));
    }
    for (i, m) in x.marks.iter().enumerate() {
        if let MarkKind::Compaction(kind) = &m.kind {
            let at = xs(m.ts_ms);
            chart = chart.child(el().key(format!("compaction-{i}")).absolute(at - 0.5, 0.0).w(1.0).h(CHART_H).border(1.0, p.ink2).border_dash(3.0, 3.0));
            labels.push((at, kind.as_ref().map_or_else(|| "compacted".to_string(), |k| format!("compacted ({k})")), p.ink2, -1.0));
        }
    }
    // The next request, dashed, at now, with its cost over it; cut at the chart's top with a mark
    // when it is taller than the requests' scale.
    if let Some(n) = next {
        let c = if cold { p.cold } else { p.warm };
        let nh = h(n).clamp(2.0, CHART_H);
        let x0 = (xs(f.now) - 7.0).min(plot(w) - 14.0);
        if h(n) > CHART_H {
            chart = chart.child(text("▲").key("next-clipped").font(pal::HOURS).color(c).single_line().absolute(x0 + 3.0, 1.0));
        }
        let lw = pal::text_w(&usd(n), pal::SMALL_STRONG) + 4.0;
        chart = chart.child(
            text(usd(n)).key("next-value").font(pal::SMALL_STRONG).color(c).single_line().absolute((x0 + 7.0 - lw).max(0.0), (CHART_H - nh - 16.0).max(14.0)),
        );
        chart = chart.child(
            el().key("next-request")
                .absolute(x0, CHART_H - nh)
                .w(14.0)
                .h(nh)
                .radius(3.0)
                .fill(if cold { p.cold_bg } else { c.alpha(0.1) })
                .border(1.4, c)
                .border_dash(3.0, 2.5),
        );
    }
    // The scales: dollars or tokens at the left, the context at the right.
    let scale = |t: String, right: bool, y: f32| {
        let label = text(t).font(pal::HOURS).color(p.ink3).single_line();
        if right { row().absolute(w - RIGHT_SCALE, y).w(RIGHT_SCALE).justify_end().child(label) } else { row().absolute(2.0, y).child(label) }
    };
    chart = chart
        .child(scale(amount(top, dollars), false, 0.0).key("scale-left"))
        .child(scale(if window.is_some() { tokens(ctx_top) } else { "largest".into() }, true, 0.0).key("scale-right"))
        .child(scale("0".into(), true, CHART_H - 12.0));
    // The card of the column under the pointer.
    let hovered = match s.requests.hover.filter(|_| s.requests.session == x.id) {
        Some(Hover::Slot(slot)) => column_at(&cols, slot as f32 * SLOT + SLOT / 2.0),
        Some(Hover::Selected) => sel_col,
        None => None,
    };
    // It goes over the whole card, after the rest, so that nothing draws over it.
    let hover = hovered.map(|i| &cols[i]).map(|c| {
        let at = if c.x() + 310.0 < w { c.x() + 10.0 } else { (c.x() - 300.0).max(0.0) };
        hover_card(x, c, dollars, p).absolute(14.0 + at, 14.0 + 16.0 + 12.0 + 12.0)
    });
    let chart = chart
        .on_layout(|s: &mut Desk, r| s.requests.width = r[2])
        .on_pointer_move_local(|s: &mut Desk, at, _| {
            if let Some(id) = s.browser.xray().map(|x| x.id.clone()) {
                adopt(s, &id);
            }
            s.requests.hover = Some(Hover::Slot((at.x / SLOT).floor() as i64));
        })
        .on_hover_change(|s: &mut Desk, inside, _| {
            if !inside {
                s.requests.hover = None;
            }
        })
        .on_drag(|s: &mut Desk, d: &mut Drag, tx| drag(s, d, tx));

    // The labels of the lines, the costliest miss first and compactions last, each where it
    // keeps clear of every label placed.
    let mut label_row = el().key("marker-labels").w_full().h(16.0);
    for (x0, t, color) in place_labels(labels, w) {
        label_row = label_row.child(text(t).font(pal::SMALL).color(color).single_line().absolute(x0, 1.0));
    }
    // The small marks: typed messages, stops, failure streaks and commits.
    let mut marks = el().key("event-marks").w_full().h(12.0);
    for (i, m) in x.marks.iter().enumerate() {
        let at = xs(m.ts_ms);
        match m.kind {
            MarkKind::Typed => marks = marks.child(el().key(format!("typed-{i}")).absolute(at - 2.5, 4.0).w(5.0).h(5.0).radius(2.5).fill(p.message)),
            MarkKind::Interrupt => marks = marks.child(el().key(format!("stop-{i}")).absolute(at - 2.5, 4.0).w(5.0).h(5.0).radius(1.0).fill(p.stop)),
            MarkKind::Compaction(_) => {}
        }
    }
    // The commits on their own row just above the axis, as diamonds.
    let mut commits = el().key("commit-marks").w_full().h(11.0);
    for (i, q) in x.points.iter().enumerate() {
        let at = xs(q.ts_ms);
        if q.commits > 0 {
            commits = commits.child(text("◆").key(format!("commit-{i}")).font(pal::HOURS).color(p.ink).single_line().absolute(at - 3.5, -1.0));
        }
        if q.streak {
            marks = marks.child(text("▲").key(format!("streak-{i}")).font(pal::HOURS).color(p.cold).single_line().absolute(at - 4.0, -1.0));
        }
    }
    // When the subagents ran.
    let mut band = el().key("subagent-band").w_full().h(8.0);
    for (i, b) in x.subagent_spans.iter().enumerate() {
        let x0 = xs(b.first_ms);
        band = band.child(
            el().key(format!("subagent-{i}"))
                .absolute(x0, 2.0)
                .w((xs(b.last_ms) - x0).max(3.0))
                .h(4.0)
                .radius(2.0)
                .fill(p.agents[0].alpha(0.7))
                .help(b.task.clone()),
        );
    }
    // The times under the axis, as Summary gives them.
    // The cache's band along the axis, and the times under it.
    let mut axis_row = el().w_full().h(30.0 + inspector::BAND_H).child(inspector::cache_band(&f.axis, plot(w), p).absolute(0.0, 0.0));
    let first = x.points[0].ts_ms;
    // In the order of their claim to room: the start, the last request, then the cache.
    let times = vec![
        (xs(first), local_time(first, "%H:%M"), "start"),
        (xs(f.last), local_time(f.last, "%H:%M"), "last request"),
        (xs(f.cold_at), local_time(f.cold_at, "%H:%M"), if cold { "cache cold" } else { "cold then" }),
    ];
    axis_row = axis_row.children(inspector::axis_times(times, w - inspector::now_w(f.now, f.last), p));
    axis_row = axis_row.child(inspector::now_el(f.now, f.last, w, p));
    let body = col()
        .w_full()
        .child(label_row)
        .child(marks)
        .child(chart)
        .child(band)
        .child(commits)
        .child(el().w_full().h(1.0).fill(p.line))
        .child(axis_row)
        .child(legend(x, p));
    let aside = format!("{} of the main conversation, {} to now", count(x.points.len() as u64, "request", "requests"), local_time(first, "%H:%M"));
    let head = row()
        .w_full()
        .gap(8.0)
        .center_y()
        .child(text("Request timeline").font(pal::HEAD).color(p.ink2).single_line())
        .child(spacer().grow(1.0))
        .child(text(aside).key("aside").font(pal::SMALL).color(p.ink3).single_line().truncate().shrink().min_w(0.0))
        .children(x.priced.then(|| unit_pill(s, p)));
    Some(
        col()
            .key("request-timeline")
            .w_full()
            .gap(10.0)
            .child(head)
            .child(col().w_full().padding(14.0).gap(6.0).radius(R_GROUP).fill(p.bar).child(body).children(hover)),
    )
}

/// Where the labels of the lines go: by weight, heaviest first, each over its line or left of
/// the chart's end, where it keeps `LABEL_GAP` clear of every label placed; a label without room
/// is left out. Each label is its place, its text, its color and its weight.
pub fn place_labels(mut labels: Vec<(f32, String, Color, f64)>, w: f32) -> Vec<(f32, String, Color)> {
    labels.sort_by(|a, b| b.3.total_cmp(&a.3).then(a.0.total_cmp(&b.0)));
    let mut placed: Vec<(f32, f32)> = Vec::new();
    let mut out = Vec::new();
    for (at, t, color, _) in labels {
        let tw = pal::text_w(&t, pal::SMALL) + 4.0;
        let x0 = (at - 2.0).min(w - tw).max(0.0);
        if placed.iter().all(|&(a, b)| x0 + tw + LABEL_GAP <= a || x0 >= b + LABEL_GAP) {
            placed.push((x0, x0 + tw));
            out.push((x0, t, color));
        }
    }
    out
}

/// Whether a miss is red: its avoidable premium is at least `RED_USD`.
pub fn red(m: &timeline::PointMiss) -> bool {
    m.premium_usd.is_some_and(|u| u >= RED_USD)
}

/// The `$` / `tokens` pill.
fn unit_pill(s: &Desk, p: &Pal) -> El {
    let seg = |label: &'static str, on: bool, tokens: bool| {
        row()
            .key(format!("unit-{label}"))
            .h(22.0)
            .px(9.0)
            .center_y()
            .radius(PILL)
            .fill(if on { p.accent } else { Color::TRANSPARENT })
            .hover(if on { p.accent } else { p.hover })
            .role("button")
            .checked(on)
            .label(label)
            .on_press(move |s: &mut Desk| s.requests.tokens = tokens)
            .child(text(label).font(pal::SMALL_STRONG).color(if on { p.on_accent } else { p.ink2 }).single_line())
    };
    row().key("unit").gap(1.0).padding(2.0).radius(PILL).fill(p.panel).child(seg("$", !s.requests.tokens, false)).child(seg("tokens", s.requests.tokens, true))
}

/// What the colors and marks are.
fn legend(x: &XRay, p: &Pal) -> El {
    let swatch = |c: Color, t: &str| {
        row().gap(5.0).center_y().child(el().w(9.0).h(9.0).radius(R_MARK).fill(c)).child(text(t.to_string()).font(pal::SMALL).color(p.ink2).single_line())
    };
    let line = |c: Color, t: &str, w: f32| {
        row().gap(5.0).center_y().child(el().w(w).h(10.0).fill(c)).child(text(t.to_string()).font(pal::SMALL).color(p.ink2).single_line())
    };
    let parts: Vec<usize> = (0..5).filter(|&i| x.points.iter().any(|q| q.parts[i].tokens > 0.0)).collect();
    let mut out = row()
        .key("timeline-legend")
        .w_full()
        .pt(4.0)
        .gap(12.0)
        .flex_wrap()
        .children(parts.into_iter().map(|i| swatch(part_color(i, p), Component::ALL[i].name())));
    out = out.child(row().gap(5.0).center_y().child(el().w(14.0).h(1.5).fill(p.ink2)).child(text("context").font(pal::SMALL).color(p.ink2).single_line()));
    out = out.child(line(p.cold, &format!("miss ≥ {}", usd(RED_USD)), 1.5)).child(line(p.write, "miss", 1.5));
    out = out.child(
        row()
            .gap(5.0)
            .center_y()
            .child(el().w(1.0).h(10.0).border(1.0, p.ink2).border_dash(2.0, 2.0))
            .child(text("compaction").font(pal::SMALL).color(p.ink2).single_line()),
    );
    let has = |k: fn(&MarkKind) -> bool| x.marks.iter().any(|m| k(&m.kind));
    if has(|k| matches!(k, MarkKind::Typed)) {
        out =
            out.child(row().gap(5.0).center_y().child(crate::view::dot(p.message, 5.0)).child(text("you typed").font(pal::SMALL).color(p.ink2).single_line()));
    }
    if has(|k| matches!(k, MarkKind::Interrupt)) {
        out = out.child(
            row()
                .gap(5.0)
                .center_y()
                .child(el().w(5.0).h(5.0).radius(1.0).fill(p.stop))
                .child(text("you stopped it").font(pal::SMALL).color(p.ink2).single_line()),
        );
    }
    if x.points.iter().any(|q| q.streak) {
        out = out.child(
            row()
                .gap(5.0)
                .center_y()
                .child(text("▲").font(pal::HOURS).color(p.cold).single_line())
                .child(text("3 failed calls in a row").font(pal::SMALL).color(p.ink2).single_line()),
        );
    }
    if x.points.iter().any(|q| q.commits > 0) {
        out = out.child(
            row()
                .gap(5.0)
                .center_y()
                .child(text("◆").font(pal::HOURS).color(p.ink).single_line())
                .child(text("commit").font(pal::SMALL).color(p.ink2).single_line()),
        );
    }
    // The time after the last request: the cache's band and the next request.
    out = out.child(
        row()
            .gap(5.0)
            .center_y()
            .child(row().child(el().w(9.0).h(5.0).radius(1.5).fill(p.warm.alpha(0.75))).child(el().w(9.0).h(5.0).radius(1.5).fill(p.cold.alpha(0.75))))
            .child(text("cache warm, then cold, after the last request").font(pal::SMALL).color(p.ink2).single_line()),
    );
    if x.priced {
        out = out.child(
            row()
                .gap(5.0)
                .center_y()
                .child(el().w(9.0).h(10.0).radius(2.0).border(1.2, p.ink2).border_dash(2.0, 2.0))
                .child(text("next request, estimated").font(pal::SMALL).color(p.ink2).single_line()),
        );
    }
    if !x.subagent_spans.is_empty() {
        out = out.child(
            row()
                .gap(5.0)
                .center_y()
                .child(el().w(14.0).h(4.0).radius(2.0).fill(p.agents[0].alpha(0.7)))
                .child(text("subagent").font(pal::SMALL).color(p.ink2).single_line()),
        );
    }
    out
}

/// The card over a column: its request or requests, their parts and context, and the cause of
/// a miss.
fn hover_card(x: &XRay, c: &Column, dollars: bool, p: &Pal) -> El {
    let pts = &x.points[c.first..=c.last];
    let one = pts.len() == 1;
    let q = &pts[0];
    let head = if one {
        let mut t = format!("#{} · {} · {}", c.first + 1, local_time(q.ts_ms, "%m-%d %H:%M:%S"), q.model);
        if let Some(e) = &q.effort {
            t += &format!(" · effort {e}");
        }
        t
    } else {
        format!(
            "{} · {} → {}",
            count(pts.len() as u64, "request", "requests"),
            local_time(q.ts_ms, "%H:%M:%S"),
            local_time(pts[pts.len() - 1].ts_ms, "%H:%M:%S")
        )
    };
    let mut body = col().w(280.0).gap(3.0).child(text(head).key("hover-head").font(pal::SMALL_STRONG).color(p.ink).single_line().truncate());
    for i in 0..5 {
        let (t, u): (f64, f64) = pts.iter().map(|q| (q.parts[i].tokens, q.parts[i].usd)).fold((0.0, 0.0), |a, b| (a.0 + b.0, a.1 + b.1));
        if t <= 0.0 {
            continue;
        }
        body = body.child(
            row()
                .w_full()
                .gap(6.0)
                .center_y()
                .child(el().w(8.0).h(8.0).radius(2.0).fill(part_color(i, p)))
                .child(text(Component::ALL[i].name()).font(pal::SMALL).color(p.ink2).single_line().grow(1.0).min_w(0.0))
                .child(text(tokens(t)).font(pal::SMALL_NUM).color(p.ink2).single_line())
                .children(x.priced.then(|| row().w(52.0).justify_end().child(text(usd(u)).font(pal::SMALL_NUM).color(p.ink).single_line()))),
        );
    }
    let total = if x.priced { usd(pts.iter().map(|q| q.usd).sum()) } else { format!("{} tok", tokens(c.parts.iter().sum::<f64>())) };
    let _ = dollars;
    body =
        body.child(text(format!("cost {total} · context {}", tokens(c.context as f64))).key("hover-total").font(pal::SMALL_STRONG).color(p.ink).single_line());
    for q in pts {
        if let Some(m) = &q.miss {
            let mut t = format!("{}: {}", if m.cause.expected() { "rebuilt" } else { "missed" }, m.text());
            if let Some(a) = m.premium_usd.filter(|_| !m.cause.expected()) {
                t += &format!(" · {} avoidable", usd(a));
            }
            body = body.child(text(t).key("hover-miss").font(pal::SMALL).color(if red(m) { p.cold } else { p.ink2 }).wrap());
        }
    }
    let mut notes = Vec::new();
    let streaks = pts.iter().filter(|q| q.streak).count();
    let commits: u32 = pts.iter().map(|q| q.commits).sum();
    if streaks > 0 {
        notes.push("3 failed calls in a row".to_string());
    }
    if commits > 0 {
        notes.push(count(commits as u64, "commit", "commits"));
    }
    let (a, b) = (q.ts_ms, pts[pts.len() - 1].ts_ms);
    let typed = x.marks.iter().filter(|m| m.kind == MarkKind::Typed && m.ts_ms <= b && m.ts_ms > a - 60_000).count();
    if typed > 0 {
        notes.push(if typed == 1 { "you typed before it".to_string() } else { format!("you typed {typed} times") });
    }
    if !notes.is_empty() {
        body = body.child(text(notes.join(" · ")).font(pal::SMALL).color(p.ink2).single_line().truncate());
    }
    col().key("hover-card").padding(10.0).radius(10.0).fill(p.panel).border(1.0, p.line).shadow(2.0, 8.0, Color::BLACK.alpha(0.18)).child(body)
}

/// The card of the selected request, or of the range.
fn card(x: &XRay, sel: usize, range: Option<(usize, usize)>, dollars: bool, p: &Pal) -> El {
    match range {
        Some((a, b)) if a != b => range_card(x, a.min(b), a.max(b), dollars, p),
        _ => request_card(x, sel, dollars, p),
    }
}

/// A part's rows under a stacked bar.
fn split(parts: &[(usize, f64, f64)], priced: bool, p: &Pal) -> El {
    let total: f64 = parts.iter().map(|x| if priced { x.2 } else { x.1 }).sum::<f64>().max(1e-12);
    let bar = row().w_full().h(8.0).gap(2.0).children(
        parts
            .iter()
            .filter(|x| (if priced { x.2 } else { x.1 }) > 0.0)
            .map(|(i, t, u)| el().h(8.0).radius(R_MARK).grow((if priced { *u } else { *t } / total) as f32).min_w(2.0).fill(part_color(*i, p))),
    );
    let rows = col().w_full().gap(3.0).children(parts.iter().filter(|x| x.1 > 0.0).map(|(i, t, u)| {
        row()
            .w_full()
            .gap(8.0)
            .center_y()
            .child(el().w(9.0).h(9.0).radius(R_MARK).fill(part_color(*i, p)))
            .child(text(Component::ALL[*i].name()).font(pal::BODY).color(p.ink).single_line().grow(1.0).min_w(0.0))
            .child(row().w(56.0).justify_end().child(text(tokens(*t)).font(pal::BODY_NUM).color(p.ink2).single_line()))
            .children(priced.then(|| row().w(60.0).justify_end().child(text(usd(*u)).font(pal::STRONG_NUM).color(p.ink).single_line())))
    }));
    col().w_full().gap(8.0).child(bar).child(rows)
}

fn card_box(key: &str, title: String, aside: String, body: El, p: &Pal) -> El {
    let head = row()
        .w_full()
        .gap(8.0)
        .center_y()
        .child(text(title).key("card-title").font(pal::HEAD).color(p.ink2).single_line())
        .child(spacer().grow(1.0))
        .child(text(aside).key("card-aside").font(pal::SMALL).color(p.ink3).single_line().truncate().shrink().min_w(0.0));
    col().key(key.to_string()).w_full().gap(10.0).child(head).child(col().w_full().padding(14.0).gap(10.0).radius(R_GROUP).fill(p.bar).child(body))
}

fn request_card(x: &XRay, i: usize, _dollars: bool, p: &Pal) -> El {
    let q = &x.points[i];
    let mut aside = format!("{} · {}", local_time(q.ts_ms, "%m-%d %H:%M:%S"), q.model);
    if let Some(e) = &q.effort {
        aside += &format!(" · effort {e}");
    }
    if q.advisor {
        aside += " · advisor";
    }
    let missed_red = q.miss.as_ref().is_some_and(|m| !m.cause.expected() && red(m));
    // One lead sentence, red for a costly miss, and the rest of the account in plain ink.
    let sentence = timeline::sentence(q);
    let (lead, rest) = match sentence.split_once(". ") {
        Some((a, b)) => (format!("{a}."), b.to_string()),
        None => (sentence, String::new()),
    };
    let why = col()
        .w_full()
        .gap(2.0)
        .child(text(lead).key("why").font(pal::STRONG).color(if missed_red { p.cold } else { p.ink }).wrap().selectable(true))
        .children((!rest.is_empty()).then(|| text(rest).key("why-rest").font(pal::BODY).color(p.ink2).wrap().selectable(true)));
    let cost = if q.priced { usd(q.usd) } else { format!("{} tok", tokens(q.total(false))) };
    let parts: Vec<(usize, f64, f64)> = (0..5).map(|k| (k, q.parts[k].tokens, q.parts[k].usd)).collect();
    let left = col()
        .grow(1.0)
        .w(0.0)
        .min_w(0.0)
        .gap(8.0)
        .child(
            row()
                .gap(8.0)
                .center_y()
                .child(text(cost).key("request-cost").font(pal::KPI).color(p.ink).single_line())
                .child(text(format!("context {}", tokens(q.prompt_tokens as f64))).font(pal::SMALL).color(p.ink2).single_line()),
        )
        .child(split(&parts, q.priced, p));
    let entered_head = if q.first { "What its prompt holds" } else { "What entered since the previous request" };
    let mut right = col().grow(1.0).w(0.0).min_w(0.0).gap(4.0).child(text(entered_head).font(pal::SMALL_STRONG).color(p.ink2).single_line());
    match &q.entered {
        None => right = right.child(text("unknown: the log does not show this prompt").key("entered-unknown").font(pal::BODY).color(p.ink2)),
        Some(e) if e.is_empty() => right = right.child(text("nothing new").font(pal::BODY).color(p.ink2)),
        Some(e) => {
            let (top, more, more_tokens) = e.top(ENTERED_SHOWN);
            for (k, g) in top.iter().enumerate() {
                let what = [Some(g.kind.name().to_string()), g.tool.clone(), g.target.clone()].into_iter().flatten().collect::<Vec<_>>().join("  ");
                right = right.child(
                    row()
                        .key(format!("entered-{k}"))
                        .w_full()
                        .gap(8.0)
                        .center_y()
                        .child(row().w(52.0).justify_end().child(text(tokens(g.tokens)).font(pal::BODY_NUM).color(p.ink).single_line()))
                        .child(text(what).font(pal::BODY).color(p.ink).single_line().truncate_middle().grow(1.0).min_w(0.0)),
                );
            }
            if more > 0 {
                right = right.child(text(format!("{more} more, {}", tokens(more_tokens))).font(pal::SMALL).color(p.ink2).single_line());
            }
        }
    }
    let mut changes: Vec<(String, bool)> = q.changes.iter().map(|c| (change_text(c), false)).collect();
    if let Some(g) = timeline::gap_text(q) {
        // The lead sentence already says it in red; here it stays in ink.
        changes.push((g, false));
    }
    if !changes.is_empty() {
        right = right.child(el().h(4.0)).child(text("What changed since the previous request").font(pal::SMALL_STRONG).color(p.ink2).single_line());
        for (k, (t, hot)) in changes.into_iter().enumerate() {
            right = right.child(text(t).key(format!("change-{k}")).font(pal::BODY).color(if hot { p.cold } else { p.ink }).wrap());
        }
    }
    let body = col().w_full().gap(12.0).child(why).child(row().w_full().gap(18.0).items_start().child(left).child(right));
    card_box("request-card", format!("Request {} of {}", i + 1, x.points.len()), aside, body, p)
}

/// A change in the card's words: tools with their names.
fn change_text(c: &timeline::Change) -> String {
    match c {
        timeline::Change::Tools { added, removed } => {
            let mut t = c.text();
            let names: Vec<String> = added.iter().map(|n| format!("+{n}")).chain(removed.iter().map(|n| format!("−{n}"))).take(6).collect();
            if !names.is_empty() {
                t += &format!(" ({})", names.join(", "));
            }
            t
        }
        _ => c.text(),
    }
}

fn range_card(x: &XRay, lo: usize, hi: usize, _dollars: bool, p: &Pal) -> El {
    let pts = &x.points[lo..=hi];
    let (a, b) = (pts[0].ts_ms, pts[pts.len() - 1].ts_ms);
    let parts: Vec<(usize, f64, f64)> = (0..5).map(|k| (k, pts.iter().map(|q| q.parts[k].tokens).sum(), pts.iter().map(|q| q.parts[k].usd).sum())).collect();
    let cost = if x.priced { usd(pts.iter().map(|q| q.usd).sum()) } else { format!("{} tok", tokens(pts.iter().map(|q| q.total(false)).sum())) };
    let misses: Vec<&timeline::PointMiss> = pts.iter().filter_map(|q| q.miss.as_ref()).filter(|m| !m.cause.expected()).collect();
    let avoidable: f64 = misses.iter().filter_map(|m| m.premium_usd).sum();
    let compactions = x.marks.iter().filter(|m| matches!(m.kind, MarkKind::Compaction(_)) && m.ts_ms >= a && m.ts_ms <= b).count();
    let typed = x.marks.iter().filter(|m| m.kind == MarkKind::Typed && m.ts_ms >= a && m.ts_ms <= b).count();
    let mut facts = vec![count(pts.len() as u64, "request", "requests")];
    facts.push(if misses.is_empty() {
        "no misses".to_string()
    } else {
        format!("{} · {} avoidable", count(misses.len() as u64, "miss", "misses"), usd(avoidable))
    });
    if compactions > 0 {
        facts.push(count(compactions as u64, "compaction", "compactions"));
    }
    facts.push(format!("{} you typed", count(typed as u64, "message", "messages")));
    let left = col()
        .grow(1.0)
        .w(0.0)
        .min_w(0.0)
        .gap(8.0)
        .child(text(cost).key("range-cost").font(pal::KPI).color(p.ink).single_line())
        .child(split(&parts, x.priced, p));
    let right = col()
        .grow(1.0)
        .w(0.0)
        .min_w(0.0)
        .gap(5.0)
        .children(facts.into_iter().enumerate().map(|(k, f)| text(f).key(format!("range-fact-{k}")).font(pal::BODY).color(p.ink).single_line()));
    let body = row().w_full().gap(18.0).items_start().child(left).child(right);
    let aside = format!("{} → {} ({})", local_time(a, "%H:%M"), local_time(b, "%H:%M"), human_duration((b - a) as f64 / 1000.0));
    card_box("range-card", format!("Requests {}–{}", lo + 1, hi + 1), aside, body, p)
}

/// The table of the requests: the page that holds the selected one.
fn table(s: &Desk, x: &XRay, sel: usize, p: &Pal) -> El {
    let n = x.points.len();
    let (lo, hi) = page(n, sel);
    inspector::cursor_row(inspector::keyboard(s).then_some(sel - lo));
    let rows = x.points[lo..hi].iter().enumerate().map(|(k, q)| {
        let i = lo + k;
        let u = &q.usage;
        let missed = q.miss.as_ref().is_some_and(|m| !m.cause.expected());
        let cost = if q.priced { usd(q.usd) } else { format!("{} tok", tokens(q.total(false))) };
        vec![
            ((i + 1).to_string(), None),
            (local_time(q.ts_ms, "%m-%d %H:%M:%S"), None),
            (q.gap_ms.map_or(String::new(), |g| human_duration(g as f64 / 1000.0)), q.past_ttl().then_some(p.cold)),
            (tokens(q.prompt_tokens as f64), None),
            (tokens(u.cache_read as f64), None),
            (tokens(u.cache_write() as f64), None),
            (tokens(u.uncached as f64), None),
            (tokens(u.output as f64), None),
            (cost, None),
            (timeline::why(q, false), missed.then_some(if q.miss.as_ref().is_some_and(red) { p.cold } else { p.write })),
        ]
    });
    let cols = [
        inspector::Column("#", 40.0, true),
        inspector::Column("sent", 108.0, false),
        inspector::Column("after", 56.0, true),
        inspector::Column("in context", 72.0, true),
        inspector::Column("read", 60.0, true),
        inspector::Column("write", 60.0, true),
        inspector::Column("uncached", 64.0, true),
        inspector::Column("output", 56.0, true),
        inspector::Column("cost", 60.0, true),
        inspector::Column("why", 0.0, false),
    ];
    let mut body = col().w_full().gap(8.0);
    if lo > 0 {
        body = body.child(text(format!("and {lo} earlier requests")).key("earlier").font(pal::BODY).color(p.ink2));
    }
    body = body.child(inspector::table_dropping(&cols, rows.collect(), &["uncached", "after", "read", "output", "in context"], p));
    if hi < n {
        body = body.child(text(format!("and {} later requests", n - hi)).key("later").font(pal::BODY).color(p.ink2));
    }
    let aside = "an after longer than the cache lives is in red".to_string();
    let head = row()
        .w_full()
        .gap(8.0)
        .center_y()
        .child(text("Requests").font(pal::HEAD).color(p.ink2).single_line())
        .child(spacer().grow(1.0))
        .child(text(aside).font(pal::SMALL).color(p.ink3).single_line().truncate().shrink().min_w(0.0));
    col().key("requests").w_full().gap(10.0).child(head).child(col().w_full().padding(14.0).radius(R_GROUP).fill(p.bar).child(body))
}

/// Makes the state about session `id`, from the costliest request, when it is about another.
fn adopt(s: &mut Desk, id: &str) {
    if s.requests.session != id {
        s.requests = State { session: id.to_string(), width: s.requests.width, tokens: s.requests.tokens, ..State::default() };
    }
}

/// Selects request `i` and keeps its row on the screen.
fn select(s: &mut Desk, i: usize, tx: &mut Tx) {
    let Some(n) = s.browser.xray().map(|x| x.points.len()) else { return };
    s.requests.sel = Some(i);
    s.requests.range = None;
    let (lo, _) = page(n, i);
    tx.scroll_into_view(format!("row-{}", i - lo));
}

/// A click on a row of the table selects its request.
pub fn row_pressed(s: &mut Desk, row: usize) {
    let Some((id, n, sel)) = s.browser.xray().map(|x| (x.id.clone(), x.points.len(), current(&s.requests, x).0)) else { return };
    adopt(s, &id);
    let (lo, hi) = page(n, sel);
    if lo + row < hi {
        s.requests.sel = Some(lo + row);
        s.requests.range = None;
    }
}

/// A press, drag or release on the chart: a click selects the column's costliest request, a
/// drag of `DRAG_MIN` or more selects the range it covers.
fn drag(s: &mut Desk, d: &mut Drag, tx: &mut Tx) {
    let Some(x) = s.browser.xray_shared() else { return };
    adopt(s, &x.id);
    let Some(f) = frame(s, &x) else { return };
    let w = width(s, None);
    let cols = columns(&x.points, |ts| f.axis.at(ts) * plot(w), dollars(s, &x));
    s.pane = crate::Focus::Inspector;
    match d.phase {
        Phase::Down => s.requests.drag = Some(d.x),
        Phase::Move => {
            let Some(start) = s.requests.drag else { return };
            if (d.x - start).abs() >= DRAG_MIN {
                let (a, b) = (start.min(d.x), start.max(d.x));
                let inside: Vec<&Column> = cols.iter().filter(|c| c.x() + SLOT >= a && c.x() <= b).collect();
                if let (Some(first), Some(last)) = (inside.first(), inside.last()) {
                    let (lo, hi) = (first.first, last.last);
                    s.requests.range = Some(if d.x >= start { (lo, hi) } else { (hi, lo) });
                }
            }
        }
        Phase::Up => {
            let start = s.requests.drag.take();
            if d.cancelled() {
                return;
            }
            if start.is_some_and(|st| (d.x - st).abs() < DRAG_MIN)
                && let Some(c) = column_at(&cols, d.x)
            {
                let i = costliest_in(&x.points, &cols[c]);
                select(s, i, tx);
            }
        }
    }
}

/// The keys of the tab while the inspector has the keyboard: ← → and ↑ ↓ step, PgUp PgDn by
/// ten, Home End to the ends, Shift+↑ ↓ extend a range, `t` the unit. Shift+← → go on, to
/// change the tab.
pub fn key(s: &mut Desk, e: &KeyEvent, tx: &mut Tx) -> bool {
    use rune::Key as K;
    if e.mods.cmd || e.mods.ctrl || e.mods.alt {
        return false;
    }
    let Some((id, n, sel, range)) = s.browser.xray().filter(|x| !x.points.is_empty()).map(|x| {
        let (sel, range) = current(&s.requests, x);
        (x.id.clone(), x.points.len(), sel, range)
    }) else {
        return false;
    };
    let text = match &e.key {
        K::Text(t) => Some(t.as_str()),
        _ => None,
    };
    let shift = e.mods.shift;
    if text == Some("t") {
        s.requests.tokens = !s.requests.tokens;
        return true;
    }
    let step: isize = match (&e.key, text) {
        (K::Left | K::Right, _) if shift => return false,
        (K::Up, _) if shift => {
            extend(s, &id, sel, range, -1, n, tx);
            return true;
        }
        (K::Down, _) if shift => {
            extend(s, &id, sel, range, 1, n, tx);
            return true;
        }
        (K::Left | K::Up, _) | (_, Some("h" | "k")) => -1,
        (K::Right | K::Down, _) | (_, Some("l" | "j")) => 1,
        (K::PageUp, _) => -10,
        (K::PageDown, _) => 10,
        (K::Home, _) | (_, Some("g")) => isize::MIN / 2,
        (K::End, _) | (_, Some("G")) => isize::MAX / 2,
        _ => return false,
    };
    adopt(s, &id);
    let to = (sel as isize).saturating_add(step).clamp(0, n as isize - 1) as usize;
    select(s, to, tx);
    true
}

/// Moves the range's free end by one request, starting it at the selected request.
fn extend(s: &mut Desk, id: &str, sel: usize, range: Option<(usize, usize)>, step: isize, n: usize, tx: &mut Tx) {
    adopt(s, id);
    let (anchor, end) = range.unwrap_or((sel, sel));
    let end = (end as isize + step).clamp(0, n as isize - 1) as usize;
    s.requests.sel = Some(sel);
    s.requests.range = Some((anchor, end));
    let (lo, _) = page(n, end);
    if page(n, sel).0 == lo {
        tx.scroll_into_view(format!("row-{}", end - lo));
    }
}

/// esc with a range clears it; true when there was one.
pub fn clear_range(s: &mut Desk) -> bool {
    if s.tab != Tab::Requests || s.requests.range.is_none() {
        return false;
    }
    s.requests.range = None;
    true
}

/// For `--scene requests-hover`: `Requests` with the keyboard and the pointer's card over the
/// costliest request.
pub fn scene_hover(s: &mut Desk) {
    s.pane = crate::Focus::Inspector;
    s.tab = Tab::Requests;
    if let Some(id) = s.browser.xray().map(|x| x.id.clone()) {
        adopt(s, &id);
        s.requests.hover = Some(Hover::Selected);
    }
}

/// For `--scene requests-range`: a range from the costliest request over the 20 before it.
pub fn scene_range(s: &mut Desk) {
    s.pane = crate::Focus::Inspector;
    s.tab = Tab::Requests;
    if let Some((id, sel)) = s.browser.xray().map(|x| (x.id.clone(), costliest(&x.points))) {
        adopt(s, &id);
        s.requests.sel = Some(sel);
        s.requests.range = Some((sel, sel.saturating_sub(20)));
    }
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};
    use std::sync::Arc;

    use rune::{Env, Key as RuneKey, KeyEvent, Runtime};
    use tokprof_browse::browser::Browser;
    use tokprof_core::catalog::Catalog;
    use tokprof_core::model::Usage;
    use tokprof_core::pricing::Pricing;
    use tokprof_core::profile::Values;
    use tokprof_core::query::Query;

    use super::*;

    fn point(ts_ms: i64, usd: f64) -> RequestPoint {
        RequestPoint {
            ts_ms,
            prompt_tokens: 1_000,
            usd,
            model: "m".into(),
            effort: None,
            advisor: false,
            usage: Usage::default(),
            parts: [Values { tokens: 1.0, usd, avoidable: 0.0 }, Values::default(), Values::default(), Values::default(), Values::default()],
            priced: true,
            first: false,
            prev_prompt: None,
            gap_ms: None,
            ttl: None,
            miss: None,
            entered: None,
            changes: Vec::new(),
            failed: 0,
            streak: false,
            commits: 0,
        }
    }

    #[test]
    fn requests_within_one_slot_share_a_column_with_their_sums() {
        let points = [point(0, 1.0), point(1, 2.0), point(5, 4.0), point(9, 8.0)];
        // A point of time per millisecond: 0 and 1 share the first slot, 5 is the second.
        let cols = columns(&points, |ts| ts as f32, true);
        assert_eq!(cols.iter().map(|c| (c.slot, c.first, c.last, c.total())).collect::<Vec<_>>(), [(0, 0, 1, 3.0), (1, 2, 2, 4.0), (2, 3, 3, 8.0)]);
        assert_eq!(costliest_in(&points, &cols[0]), 1);
        assert_eq!(column_at(&cols, 6.0), Some(1));
        assert_eq!(column_at(&cols, 40.0), None);
    }

    #[test]
    fn the_table_holds_the_page_of_the_selected_request() {
        assert_eq!(page(100, 50), (0, 100));
        assert_eq!(page(622, 9), (0, 300));
        assert_eq!(page(622, 450), (300, 600));
        assert_eq!(page(622, 610), (322, 622));
        assert_eq!(costliest(&[point(0, 1.0), point(1, 3.0), point(2, 2.0)]), 1);
    }

    #[test]
    fn a_miss_is_red_from_fifty_cents_avoidable() {
        let m =
            |u: f64| timeline::PointMiss { cause: tokprof_core::analysis::cache::Cause::IdleTtl, detail: String::new(), rewritten: 1, premium_usd: Some(u) };
        assert!(red(&m(0.50)) && !red(&m(0.49)));
    }

    fn fixture() -> (Arc<Catalog>, Query) {
        let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../sources/claude-code/tests/fixtures/projects");
        let catalog = Arc::new(Catalog::new(vec![Box::new(tokprof_source_claude_code::ClaudeCode::at(fixtures, Path::new("/w")))], PathBuf::from("/w")));
        let select = Query { agents: catalog.agents(), project: None, session: None, since_ms: None, until_ms: None, client: None };
        (catalog, select)
    }

    fn settle(rt: &mut Runtime<Desk>) {
        for _ in 0..90 {
            rt.frame(1.0 / 60.0);
        }
    }

    fn key(rt: &mut Runtime<Desk>, key: RuneKey, shift: bool) {
        rt.key(KeyEvent { key, mods: rune::Mods { shift, ..rune::Mods::default() } });
        settle(rt);
    }

    fn texts(rt: &Runtime<Desk>) -> Vec<String> {
        rt.elements().into_iter().filter_map(|e| e.text).collect()
    }

    /// The window on the fixture's session, `Requests` shown with the inspector's keyboard.
    fn requests_desk() -> Runtime<Desk> {
        let (catalog, select) = fixture();
        let mut browser = Browser::start(catalog, Pricing::defaults(), select, false, Arc::new(|| {}), crate::frontend()).unwrap();
        browser.settle();
        let mut rt = Runtime::new(
            {
                let mut d = Desk::new(browser, false);
                d.none = false;
                d.browser.hold_reports(false);
                d
            },
            crate::view::view,
            (1400.0, 1300.0),
            Env::new(true, 1.0),
        );
        rt.poll = Some(crate::poll);
        rt.on_key = Some(crate::on_key);
        settle(&mut rt);
        key(&mut rt, RuneKey::Tab, false);
        key(&mut rt, RuneKey::Text("2".into()), false);
        rt
    }

    fn title(rt: &Runtime<Desk>) -> String {
        rt.elements().into_iter().find(|e| e.key.as_deref() == Some("card-title")).and_then(|e| e.text).unwrap_or_default()
    }

    #[test]
    fn the_tab_opens_on_the_costliest_request_and_the_arrows_step() {
        let mut rt = requests_desk();
        assert_eq!(rt.state.tab, Tab::Requests);
        let x = rt.state.browser.xray().expect("the fixture's X-ray").clone();
        assert!(x.points.len() >= 2, "the fixture has requests");
        let first = costliest(&x.points);
        assert_eq!(title(&rt), format!("Request {} of {}", first + 1, x.points.len()));
        // The parts of every column add up to its cost, as `tokprof scan` counts it.
        for q in &x.points {
            assert!((q.total(true) - q.usd).abs() < 1e-9);
        }
        // The why comes first, in plain words.
        let text_of = |k: &str| rt.elements().into_iter().find(|e| e.key.as_deref() == Some(k)).and_then(|e| e.text);
        let why = text_of("why").unwrap();
        let whole = match text_of("why-rest") {
            Some(rest) => format!("{why} {rest}"),
            None => why,
        };
        assert_eq!(whole, timeline::sentence(&x.points[first]));
        let back = if first == 0 { RuneKey::Right } else { RuneKey::Left };
        key(&mut rt, back.clone(), false);
        let to = if first == 0 { 1 } else { first - 1 };
        assert_eq!(title(&rt), format!("Request {} of {}", to + 1, x.points.len()));
        assert_eq!(rt.state.tab, Tab::Requests);
        // The table's marked row is the selected request.
        assert!(texts(&rt).contains(&(to + 1).to_string()));
        // `t` switches the unit, and Shift+→ the tab.
        key(&mut rt, RuneKey::Text("t".into()), false);
        assert!(rt.state.requests.tokens);
        key(&mut rt, RuneKey::Right, true);
        assert_eq!(rt.state.tab, Tab::Spend);
    }

    #[test]
    fn shift_and_the_vertical_arrows_total_a_range_that_esc_clears() {
        let mut rt = requests_desk();
        let n = rt.state.browser.xray().unwrap().points.len();
        let sel = costliest(&rt.state.browser.xray().unwrap().points);
        let step = if sel + 1 < n { RuneKey::Down } else { RuneKey::Up };
        key(&mut rt, step, true);
        assert!(title(&rt).starts_with("Requests "), "{}", title(&rt));
        assert!(texts(&rt).iter().any(|t| t == "2 requests"));
        key(&mut rt, RuneKey::Escape, false);
        assert!(title(&rt).starts_with("Request "));
        assert_eq!(rt.state.pane, crate::Focus::Inspector);
    }

    #[test]
    fn the_pointer_shows_its_column_and_a_drag_selects_a_range() {
        let mut rt = requests_desk();
        let ([x0, y0, w, h], _) = rt.presented(CHART).expect("the chart");
        let x = rt.state.browser.xray().unwrap().clone();
        let f = frame(&rt.state, &x).unwrap();
        let cols = columns(&x.points, |ts| f.axis.at(ts) * plot(w), true);
        let c = &cols[0];
        rt.pointer_move(x0 + c.x() + 1.0, y0 + h - 2.0);
        settle(&mut rt);
        assert!(texts(&rt).iter().any(|t| t.starts_with("cost ")), "the hover card");
        // A click selects the column's costliest request.
        rt.pointer_down(x0 + c.x() + 1.0, y0 + h - 2.0);
        rt.pointer_up(x0 + c.x() + 1.0, y0 + h - 2.0);
        settle(&mut rt);
        assert_eq!(title(&rt), format!("Request {} of {}", costliest_in(&x.points, c) + 1, x.points.len()));
        // A drag over every column totals them.
        if cols.len() > 1 {
            let last = &cols[cols.len() - 1];
            rt.pointer_down(x0 + c.x(), y0 + h / 2.0);
            rt.pointer_move(x0 + (c.x() + last.x()) / 2.0, y0 + h / 2.0);
            rt.pointer_move(x0 + last.x() + 2.0, y0 + h / 2.0);
            rt.pointer_up(x0 + last.x() + 2.0, y0 + h / 2.0);
            settle(&mut rt);
            assert_eq!(title(&rt), format!("Requests 1–{}", x.points.len()));
        }
    }

    #[test]
    fn the_costliest_miss_keeps_its_label_and_a_compaction_gives_way() {
        let c = Color::BLACK;
        let labels = vec![(100.0, "compacted".to_string(), c, -1.0), (130.0, "tools changed".to_string(), c, 0.12), (160.0, "idle > TTL".to_string(), c, 6.73)];
        let kept: Vec<String> = place_labels(labels, 900.0).into_iter().map(|l| l.1).collect();
        assert_eq!(kept, ["idle > TTL"], "the dearest first; the others collide with it");
        let apart = vec![(100.0, "compacted".to_string(), c, -1.0), (400.0, "idle > TTL".to_string(), c, 6.73)];
        assert_eq!(place_labels(apart, 900.0).len(), 2, "labels with room all show");
    }

    #[test]
    fn now_carries_its_day_when_the_last_request_was_on_another() {
        let day = 86_400_000;
        let last = 1_794_000_000_000;
        assert_eq!(inspector::now_label(last + 600_000, last).1, "now, next request");
        let (time, what) = inspector::now_label(last + 5 * day + 9 * 3_600_000, last);
        assert_eq!(what, "now, 5.4d later");
        assert_eq!(time, local_time(last + 5 * day + 9 * 3_600_000, "%a %-d %b %H:%M"));
    }

    #[test]
    fn the_last_request_keeps_its_label_before_the_cold_cache() {
        // An axis 953 pt wide whose last request sits just before the break, as at 1400 pt.
        let p = crate::pal::pal(crate::themes::default(false));
        let marks = vec![(0.0, "11:21".to_string(), "start"), (816.0, "18:27".to_string(), "last request"), (850.0, "19:27".to_string(), "cold then")];
        let labels = inspector::axis_times(marks, 953.0 - 110.0, &p);
        assert_eq!(labels.len(), 2, "start and the last request; the cold time gives way");
    }
}
