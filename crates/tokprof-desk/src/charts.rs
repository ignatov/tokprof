//! The charts of Month and Quarter: the days of the month as a grid and spend by day with the
//! month before by weekday (desk-window, Days of the month, Spend by day), and the weeks of the
//! quarter stacked by project with its months and its three trends, its Months card and its
//! project shifts (Weeks, Months and Trends of the quarter, Project shifts). All of it comes from
//! the day store.

use rune::{Color, El, Shape, Tx, canvas, col, el, row, text};
use tokprof_browse::render_calendar::cut_amount;
use tokprof_core::calendar::{Unit, level};
use tokprof_core::fmt::{count, int, local_time, tokens};
use tokprof_core::period::{NO_SHIFTS, Period, PeriodSums, WeekKind};
use tokprof_core::rhythm;

use crate::Desk;
use crate::overlay::{R_MARK, R_ROW};
use crate::pal::{self, Pal};
use crate::reports::{self, Hover, Part, card, cursor_row, noon, offset, usd_whole};

const WEEKDAYS: [&str; 7] = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
/// The grid's cells: their height and the room between them.
const CELL_H: f32 = 54.0;
const CELL_GAP: f32 = 4.0;
/// The cold-return ticks a cell draws before `+n`.
pub const TICKS: usize = 5;
/// The day chart and the weeks chart: the plot's height, the axis labels' width, the room above
/// and under the lines for the labels on them.
const CHART_H: f32 = 210.0;
const WEEKS_H: f32 = 190.0;
const AXIS_W: f32 = 46.0;
const PLOT_PAD: f32 = 8.0;
/// A trend's strip.
const TREND_H: f32 = 58.0;
/// The hues of the quarter's six projects, the same in every theme (see the design), checked for
/// contrast on Merino Light and Merino Dark.
const HUES: [&str; 6] = ["#2f6fd0", "#13958a", "#8a5cc9", "#b87a10", "#d4558f", "#6b8f23"];

/// The color of the quarter's project `i` of the six, mixed with the card as the calendar levels
/// are; `other` in the subtle ink.
pub fn project_color(i: Option<usize>, p: &Pal) -> Color {
    match i.and_then(|i| HUES.get(i)).and_then(|h| rune::color::hex(h)) {
        Some(c) => pal::mix_srgb(p.bar, c, if p.dark { 0.8 } else { 1.0 }),
        None => pal::mix_srgb(p.bar, p.ink3, 0.55),
    }
}

/// A round step for an axis up to `max`: 1, 2 or 5 times a power of ten, so that three to six
/// lines fit.
pub fn axis_step(max: f64) -> f64 {
    if max <= 0.0 {
        return 1.0;
    }
    let raw = max / 4.0;
    let power = 10f64.powf(raw.log10().floor());
    [1.0, 2.0, 5.0, 10.0].into_iter().map(|m| m * power).find(|s| *s >= raw).unwrap_or(10.0 * power)
}

/// An amount on an axis: `$0`, `$500`, `$1.5k`.
fn axis_label(v: f64) -> String {
    if v == 0.0 {
        "$0".into()
    } else if v >= 1000.0 {
        let k = v / 1000.0;
        if k.fract() == 0.0 { format!("${k:.0}k") } else { format!("${k:.1}k") }
    } else if v.fract() != 0.0 {
        format!("${v:.2}")
    } else {
        format!("${}", int(v as u64))
    }
}

/// `$5.1k`, `$412`.
fn short_usd(v: f64) -> String {
    if v >= 1000.0 { format!("${:.1}k", v / 1000.0) } else { usd_whole(v) }
}

/// The axis's lines and labels for a plot `h` high up to `top` in steps of `step`.
fn axis(top: f64, step: f64, h: f32, plot_w: f32, p: &Pal) -> (El, El) {
    let (mut labels, mut lines) = (el().w(AXIS_W).h(h + 2.0 * PLOT_PAD), el().w(plot_w).h(h + 2.0 * PLOT_PAD));
    let n = (top / step).round() as usize;
    for i in 0..=n {
        let y = PLOT_PAD + h - (i as f64 * step / top) as f32 * h;
        lines = lines.child(el().absolute(0.0, y.min(PLOT_PAD + h - 1.0)).w(plot_w).h(1.0).fill(p.row_line));
        labels = labels.child(
            row().absolute(0.0, y - 7.0).w(AXIS_W - 8.0).justify_end().child(text(axis_label(i as f64 * step)).font(pal::HOURS).color(p.ink3).single_line()),
        );
    }
    (labels, lines)
}

// ---------------------------------------------------------------------------------------------
// The days of the month.

/// The dates of the month that open: from the store's first day with data to today, or to its
/// last date.
pub fn openable_dates(sums: &PeriodSums) -> Vec<i64> {
    let first = sums.period.first_date().max(sums.first_data.unwrap_or(i64::MAX));
    let last = sums.today.unwrap_or(sums.period.last_date()).min(sums.period.last_date());
    (first..=last).collect()
}

/// The day cursor's date: the one it was moved to, else today, or the month's last date that
/// opens.
pub fn day_cursor(s: &Desk, sums: &PeriodSums) -> Option<i64> {
    let dates = openable_dates(sums);
    let (&first, &last) = (dates.first()?, dates.last()?);
    Some(s.reports.day.unwrap_or(last).clamp(first, last))
}

/// The date marked in the grid or the chart: under the pointer, else under the cursor of the
/// card with the keyboard.
fn marked(s: &Desk, sums: &PeriodSums, part: Part) -> Option<i64> {
    let hover = match (s.reports.hover, part) {
        (Some(Hover::Cell(d)), Part::Days) | (Some(Hover::Bar(d)), Part::Chart) => Some(d),
        _ => None,
    };
    hover.or_else(|| (s.reports.part == Some(part)).then(|| day_cursor(s, sums)).flatten())
}

/// What a date of the month is in the grid.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DateKind {
    NoLogs,
    ToCome,
    Day,
}

fn date_kind(sums: &PeriodSums, date: i64) -> DateKind {
    if sums.first_data.is_none_or(|f| date < f) {
        DateKind::NoLogs
    } else if sums.today.is_some_and(|t| date > t) {
        DateKind::ToCome
    } else {
        DateKind::Day
    }
}

/// The cell of a date: its spend, avoidable dollars and cold returns.
fn cell_of(sums: &PeriodSums, date: i64) -> (f64, f64, usize) {
    let i = (date - sums.period.first_date()) as usize;
    let a = sums.span.days.get(i).map(|d| (d.amounts.usd, d.amounts.avoidable_usd)).unwrap_or_default();
    (a.0, a.1, sums.cold_by_date.get(i).copied().unwrap_or(0))
}

/// The tooltip of a date of the grid: `Wed 11 Nov 2026 · $906 spent, $56 avoidable`, `6 cold
/// returns`, `click: open this day`.
pub fn cell_tip(sums: &PeriodSums, date: i64) -> Vec<String> {
    let (spend, avoidable, cold) = cell_of(sums, date);
    let so_far = if sums.today == Some(date) { " so far" } else { "" };
    let mut out = vec![format!("{} · {}{so_far} spent, {} avoidable", local_time(noon(date), "%a %-d %b %Y"), usd_whole(spend), usd_whole(avoidable))];
    if cold > 0 {
        out.push(count(cold as u64, "cold return", "cold returns"));
    }
    out.push("click: open this day".into());
    out
}

/// The ticks of `n` cold returns: up to five, and `+n` beyond.
pub fn ticks(n: usize) -> (usize, Option<String>) {
    (n.min(TICKS), (n > TICKS).then(|| format!("+{}", n - TICKS)))
}

/// The days of the month: a row per week from Monday to Sunday, each date with its spend at its
/// level, its cold returns, today outlined, and the legend of the cuts.
pub fn days_card(s: &Desk, sums: &PeriodSums, p: &Pal, w: f32) -> El {
    let Period::Month(ym) = sums.period else { return el() };
    let inner = w - 28.0;
    let cw = (inner - 6.0 * CELL_GAP) / 7.0;
    let first = sums.period.first_date();
    let lead = tokprof_core::calendar::weekday(first) as i64;
    let days = sums.period.days();
    let weeks = ((lead + days) as f32 / 7.0).ceil() as usize;
    let cuts = sums.span.cuts;
    let cursor = (s.reports.part == Some(Part::Days)).then(|| day_cursor(s, sums)).flatten();
    let tip = marked(s, sums, Part::Days).filter(|&d| date_kind(sums, d) == DateKind::Day);
    let head = row().w_full().gap(CELL_GAP).children(WEEKDAYS.iter().map(|d| row().w(cw).pl(4.0).child(text(*d).font(pal::HOURS).color(p.ink3).single_line())));
    let mut grid = el().key("mgrid").w(inner).h(weeks as f32 * (CELL_H + CELL_GAP));
    let mut tip_at = None;
    for slot in 0..(weeks * 7) as i64 {
        let date = first + slot - lead;
        if date < first || date >= first + days {
            continue;
        }
        let (x, y) = ((slot % 7) as f32 * (cw + CELL_GAP), (slot / 7) as f32 * (CELL_H + CELL_GAP));
        let kind = date_kind(sums, date);
        let (spend, _, cold) = cell_of(sums, date);
        let lvl = if kind == DateKind::Day { level(spend, cuts) } else { 0 };
        let dark_text = lvl >= 3;
        let ink = if dark_text { p.on_accent } else { p.ink };
        let mut c = el().key(format!("mday-{date}")).absolute(x, y).w(cw).h(CELL_H).radius(8.0);
        c = match kind {
            DateKind::Day if lvl == 0 && p.dark => c.border(1.0, p.row_line),
            DateKind::Day => c.fill(crate::calendar::level_color(lvl, p.accent, p)),
            _ => c.border(1.0, p.row_line),
        };
        let day_n = local_time(noon(date), "%-d");
        c = c.child(text(day_n).key("mday-n").font(pal::SMALL).color(if kind == DateKind::Day { ink } else { p.ink3 }).single_line().absolute(7.0, 5.0));
        match kind {
            DateKind::NoLogs => c = c.child(text("no logs").key("mday-nologs").font(pal::HOURS).color(p.ink3).single_line().absolute(7.0, CELL_H - 19.0)),
            DateKind::ToCome => {}
            DateKind::Day => {
                // `$1,221` in a narrow cell steps down a size, so that it is not cut at the edge.
                let amount = usd_whole(spend);
                let font = if rune::text::advance(&amount, pal::STRONG_NUM) <= cw - 7.0 - 5.0 { pal::STRONG_NUM } else { pal::SMALL_STRONG_NUM };
                c = c.child(text(amount).key("mday-spend").font(font).color(ink).single_line().absolute(7.0, CELL_H - 21.0));
                let (n, more) = ticks(cold);
                let tick = if dark_text { p.on_accent } else { p.cold };
                let mut marks = row().key("mday-ticks").gap(2.0).center_y();
                for _ in 0..n {
                    marks = marks.child(el().w(2.0).h(9.0).radius(1.0).fill(tick));
                }
                let more_w = more.as_ref().map_or(0.0, |m| rune::text::advance(m, pal::HOURS) + 2.0);
                if let Some(m) = more {
                    marks = marks.child(text(m).key("mday-more").font(pal::HOURS).color(tick).single_line().pl(2.0));
                }
                let marks_w = n as f32 * 4.0 - 2.0 + more_w;
                // Beside the date where they have room, else under it.
                if n > 0 {
                    let beside = cw - 7.0 - marks_w >= 27.0;
                    c = c.child(if beside { marks.absolute(cw - 7.0 - marks_w, 8.0) } else { marks.absolute(7.0, 21.0) });
                }
                c = c
                    .role("button")
                    .label(local_time(noon(date), "%a %-d %b"))
                    .on_hover_change(move |s: &mut Desk, inside, _| {
                        if inside {
                            s.reports.hover = Some(Hover::Cell(date));
                        } else if s.reports.hover == Some(Hover::Cell(date)) {
                            s.reports.hover = None;
                        }
                    })
                    .on_press_tx(move |s: &mut Desk, tx| reports::open_day(s, date, tx));
            }
        }
        if sums.today == Some(date) {
            c = c.outline(2.0, 1.5, p.accent);
        }
        if cursor == Some(date) {
            c = c.outline(2.5, 2.5, p.ink);
        }
        if tip == Some(date) {
            tip_at = Some((x, y));
        }
        grid = grid.child(c);
    }
    // The tooltip of the date marked, under its row, over the cells under it.
    if let (Some(date), Some((x, y))) = (tip, tip_at) {
        let tip_w = 250.0f32.min(inner);
        let lines = cell_tip(sums, date);
        let mut t = col().key("mday-tip").w(tip_w).padding(10.0).gap(3.0).radius(10.0).fill(p.panel).border(1.0, p.row_line).shadow(
            12.0,
            26.0,
            Color::BLACK.alpha(0.2),
        );
        for (i, l) in lines.into_iter().enumerate() {
            t = t.child(text(l).key(format!("tip-{i}")).w_full().wrap().font(if i == 0 { pal::STRONG } else { pal::SMALL_NUM }).color(if i == 0 {
                p.ink
            } else {
                p.ink2
            }));
        }
        let rows_h = weeks as f32 * (CELL_H + CELL_GAP);
        let ty = if y + CELL_H + 4.0 + 76.0 <= rows_h + 40.0 { y + CELL_H + 4.0 } else { (y - 80.0).max(0.0) };
        grid = grid.child(t.absolute(x.min(inner - tip_w).max(0.0), ty));
    }
    let swatch = |lvl: usize| crate::calendar::cell(lvl, p.accent, p, 11.0, 11.0, 3.0);
    let mut legend = row().key("mgrid-legend").w_full().flex_wrap().gaps(4.0, 6.0).center_y();
    legend = legend.child(text("less").font(pal::SMALL).color(p.ink2).single_line()).children((0..5).map(swatch));
    let more = match cuts {
        Some(c) => format!("more · cuts at {}, {}, {}", cut_amount(c[0], Unit::Usd), cut_amount(c[1], Unit::Usd), cut_amount(c[2], Unit::Usd)),
        None => "more".into(),
    };
    legend = legend.child(text(more).key("mgrid-cuts").font(pal::SMALL).color(p.ink2).single_line().pr(10.0));
    legend = legend.child(el().w(3.0).h(10.0).radius(1.0).fill(p.cold)).child(text("one cold return").font(pal::SMALL).color(p.ink2).single_line());
    let content = col().w_full().gap(6.0).child(head).child(grid).child(legend.pt(4.0));
    card(Some(Part::Days), &format!("Days of {}", ym.month_name()), Some("spend per day · a click opens the day".into()), content, &s.reports, p)
}

// ---------------------------------------------------------------------------------------------
// Spend by day.

/// The line under spend by day about a date: `Tue 3 Nov · $412 · $381 useful, $31 avoidable · Tue
/// 6 Oct $260`, the date it faces in the month before by weekday; without the line through the
/// month before, no date it faces.
pub fn bar_line(sums: &PeriodSums, date: i64) -> String {
    let (spend, avoidable, _) = cell_of(sums, date);
    let so_far = if sums.today == Some(date) { " so far" } else { "" };
    let mut out = format!(
        "{} · {}{so_far} · {} useful, {} avoidable",
        local_time(noon(date), "%a %-d %b"),
        usd_whole(spend),
        usd_whole(spend - avoidable),
        usd_whole(avoidable)
    );
    if sums.before_cells.is_empty() {
        return out;
    }
    match (sums.facing(date), sums.before_at(date)) {
        (Some(f), Some(was)) => out += &format!(" · {} {}", local_time(noon(f), "%a %-d %b"), usd_whole(was)),
        (Some(f), None) => out += &format!(" · {} no logs", local_time(noon(f), "%a %-d %b")),
        (None, _) => out += &format!(" · no date of {} faces it", sums.period.previous().short_name()),
    }
    out
}

/// The line under spend by day on its bars and the key number: the night across the month's
/// first that lies in a cell of the month before.
pub fn gap_line(sums: &PeriodSums) -> Option<String> {
    let cells: f64 = sums.span.days.iter().map(|d| d.amounts.usd).sum();
    let gap = sums.key.usd - cells;
    let month = sums.period.short_name();
    if gap >= 0.5 {
        let night = sums.period.first_date() - 1;
        Some(format!("{} of {month} fell in the night of {}'s cell", usd_whole(gap), local_time(noon(night), "%-d %b")))
    } else if gap <= -0.5 {
        let last = sums.span.days.last().map_or(sums.period.last_date(), |d| d.date);
        Some(format!("{} of the bars fell after {month}, in the night after {}", usd_whole(-gap), local_time(noon(last), "%-d %b")))
    } else {
        None
    }
}

/// Spend by day: a bar per date, the useful part in the accent and the avoidable part on top in
/// the cold color, the month before's line by weekday, the now line, and the line about the date
/// marked.
/// The height of the grid of the days of the month, which spend by day beside it matches.
pub fn grid_h(sums: &PeriodSums) -> f32 {
    let lead = tokprof_core::calendar::weekday(sums.period.first_date()) as i64;
    let weeks = ((lead + sums.period.days()) as f32 / 7.0).ceil();
    weeks * (CELL_H + CELL_GAP)
}

pub fn chart_card(s: &Desk, sums: &PeriodSums, p: &Pal, w: f32, grid_h: f32) -> El {
    // The plot as high as the grid beside it allows, under its legend and lines.
    let chart_h = (grid_h - 2.0 * PLOT_PAD - 30.0).clamp(CHART_H, 320.0);
    let first = sums.period.first_date();
    let n = sums.period.days() as usize;
    let inner = w - 28.0;
    let plot_w = (inner - AXIS_W - 28.0).max(100.0);
    let bw = plot_w / n as f32;
    let before = |d: i64| sums.before_at(d);
    let dates: Vec<i64> = (first..first + n as i64).collect();
    let max = dates
        .iter()
        .map(|&d| if date_kind(sums, d) == DateKind::Day { cell_of(sums, d).0 } else { 0.0 })
        .chain(dates.iter().filter_map(|&d| before(d)))
        .fold(0.0, f64::max);
    let step = axis_step(max);
    let top = (max / step).ceil().max(1.0) * step;
    let y = |v: f64| PLOT_PAD + chart_h - (v / top) as f32 * chart_h;
    let (labels, mut plot) = axis(top, step, chart_h, plot_w, p);
    let mark = marked(s, sums, Part::Chart);
    for (i, &date) in dates.iter().enumerate() {
        let x = i as f32 * bw;
        let kind = date_kind(sums, date);
        if mark == Some(date) {
            plot = plot.child(el().absolute(x, PLOT_PAD).w(bw).h(chart_h).radius(3.0).fill(p.sel));
        }
        if kind == DateKind::Day {
            let (spend, avoidable, _) = cell_of(sums, date);
            let pad = if bw > 8.0 { 1.5 } else { 0.5 };
            let (yu, yt) = (y(spend - avoidable), y(spend));
            let base = y(0.0);
            let mut bar = col()
                .key(format!("mbar-{date}"))
                .absolute(x + pad, yt)
                .w((bw - 2.0 * pad).max(1.0))
                .h((base - yt).max(0.0))
                .radius(R_MARK)
                .clip_rounded(R_MARK);
            if yu - yt >= 0.5 {
                bar = bar.child(el().w_full().h(yu - yt).fill(p.cold));
            }
            bar = bar.child(el().w_full().grow(1.0).fill(p.accent));
            plot = plot.child(bar);
        }
    }
    // The month before's line, by weekday; it breaks where a date faces none or a date before the
    // first log, and there is none without a comparison.
    let lined = !sums.before_cells.is_empty();
    let mut shape = Shape::new();
    let mut seg: Vec<[f32; 2]> = Vec::new();
    let mut last_point = None;
    for (i, &date) in dates.iter().enumerate() {
        match before(date) {
            Some(v) => {
                let pt = [i as f32 * bw + bw / 2.0, y(v)];
                seg.push(pt);
                last_point = Some(pt);
            }
            None if seg.len() > 1 => shape = shape.stroke(std::mem::take(&mut seg), 2.0, p.ink2.alpha(0.75)),
            None => seg.clear(),
        }
    }
    if seg.len() > 1 {
        shape = shape.stroke(seg, 2.0, p.ink2.alpha(0.75));
    }
    if lined {
        plot = plot.child(canvas(shape).key("mbar-before").absolute(0.0, 0.0).w(plot_w).h(chart_h + 2.0 * PLOT_PAD));
    }
    let prev = sums.period.previous().short_name();
    if let Some([x, yy]) = last_point {
        let short: String = prev.chars().take(3).collect();
        plot = plot.child(el().absolute(x - 3.5, yy - 3.5).w(7.0).h(7.0).radius(3.5).fill(p.ink2));
        plot = plot.child(text(short).key("mbar-before-label").font(pal::HOURS).color(p.ink2).single_line().absolute((x + 6.0).min(plot_w + 4.0), yy - 7.0));
    }
    // Now, and the dates to come.
    if let Some(t) = sums.today {
        let i = (t - first) as f32;
        let day_start = rhythm::midnight(t, offset());
        let frac = ((sums.to_ms - day_start) as f32 / 86_400_000.0).clamp(0.0, 1.0);
        let x = (i + frac) * bw;
        plot = plot.child(el().key("mbar-now").absolute(x - 0.75, PLOT_PAD - 4.0).w(1.5).h(chart_h + 4.0).fill(p.now));
        plot = plot.child(
            text(format!("now {}", local_time(sums.to_ms, "%H:%M")))
                .key("mbar-now-label")
                .font(pal::HOURS)
                .color(p.now)
                .single_line()
                .absolute(x + 4.0, PLOT_PAD - 4.0),
        );
        let last = sums.period.last_date();
        if t < last {
            let label = format!("{}–{} {} to come", local_time(noon(t + 1), "%-d"), local_time(noon(last), "%-d"), local_time(noon(last), "%b"));
            let lx = x + 6.0;
            if plot_w - lx > 90.0 {
                plot = plot.child(text(label).key("mbar-tocome").font(pal::HOURS).color(p.ink3).single_line().absolute(lx, PLOT_PAD + chart_h - 16.0));
            }
        }
    }
    // The hit areas over everything: a date that opens.
    for (i, &date) in dates.iter().enumerate() {
        if date_kind(sums, date) != DateKind::Day {
            continue;
        }
        plot = plot.child(
            el().key(format!("mbar-hit-{date}"))
                .absolute(i as f32 * bw, PLOT_PAD)
                .w(bw)
                .h(chart_h)
                .role("button")
                .label(local_time(noon(date), "%a %-d %b"))
                .on_hover_change(move |s: &mut Desk, inside, _| {
                    if inside {
                        s.reports.hover = Some(Hover::Bar(date));
                    } else if s.reports.hover == Some(Hover::Bar(date)) {
                        s.reports.hover = None;
                    }
                })
                .on_press_tx(move |s: &mut Desk, tx| reports::open_day(s, date, tx)),
        );
    }
    let mut dates_row = el().w(plot_w).h(14.0);
    for (i, &date) in dates.iter().enumerate() {
        let d = i + 1;
        if d == 1 || d % 5 == 0 || d == n {
            if d == n && n % 5 >= 1 && n % 5 <= 2 && d != 1 {
                // Too close to the label before it.
                if n % 5 == 1 {
                    continue;
                }
            }
            let x = i as f32 * bw + bw / 2.0 - 6.0;
            dates_row = dates_row.child(text(local_time(noon(date), "%-d")).font(pal::HOURS).color(p.ink3).single_line().absolute(x, 0.0));
        }
    }
    let chart = col().w_full().gap(2.0).child(row().w_full().child(labels).child(plot)).child(row().w_full().child(el().w(AXIS_W)).child(dates_row));
    let (line, color) = match mark {
        Some(d) => (bar_line(sums, d), p.ink2),
        None => ("point at a day to read it; a click opens the day".to_string(), p.ink3),
    };
    let swatch = |c: Color| el().w(10.0).h(10.0).radius(R_MARK).fill(c);
    let item = |e: El, t: String| row().gap(6.0).center_y().child(e).child(text(t).font(pal::SMALL).color(p.ink2).single_line());
    let mut legend = row()
        .key("mbar-legend")
        .w_full()
        .flex_wrap()
        .gaps(4.0, 14.0)
        .pl(AXIS_W)
        .child(item(swatch(p.accent), "useful".into()))
        .child(item(swatch(p.cold), "avoidable on cache misses".into()));
    if lined {
        legend = legend.child(item(el().w(14.0).h(2.0).radius(1.0).fill(p.ink2), format!("{prev}, by weekday")));
    }
    let mut content = col()
        .w_full()
        .gap(8.0)
        .child(chart)
        .child(legend)
        .child(text(line).key("mbar-read").w_full().pl(AXIS_W).font(pal::SMALL_NUM).color(color).single_line().truncate().min_w(0.0));
    if let Some(g) = gap_line(sums) {
        content = content.child(text(g).key("mbar-gap").w_full().pl(AXIS_W).wrap().font(pal::SMALL).color(p.ink3));
    }
    let caption = if lined { format!("useful and avoidable · line: {prev} by weekday") } else { "useful and avoidable".into() };
    card(Some(Part::Chart), "Spend by day", Some(caption), content, &s.reports, p)
}

// ---------------------------------------------------------------------------------------------
// The weeks of the quarter.

/// The week cursor's place: the one it was moved to, else the current week, or the last week
/// that opens.
pub fn week_cursor(s: &Desk, sums: &PeriodSums) -> Option<usize> {
    let open: Vec<usize> = (0..sums.weeks.len()).filter(|&i| sums.weeks[i].opens()).collect();
    let last = *open.last()?;
    Some(s.reports.week.filter(|w| open.contains(w)).unwrap_or(last))
}

/// The week marked: under the pointer, else under the cursor while the weeks have the keyboard.
fn marked_week(s: &Desk, sums: &PeriodSums) -> Option<usize> {
    match s.reports.hover {
        Some(Hover::Week(i)) => Some(i),
        _ => (s.reports.part == Some(Part::Weeks)).then(|| week_cursor(s, sums)).flatten(),
    }
}

/// `28 Sep`, and `Thu–Sun` for a week cut by the quarter.
fn week_labels(sums: &PeriodSums, i: usize) -> (String, Option<String>) {
    let w = &sums.weeks[i];
    let wd = |d: i64| WEEKDAYS[tokprof_core::calendar::weekday(d)];
    (local_time(noon(w.monday), "%-d %b"), w.cut().then(|| format!("{}–{}", wd(w.first), wd(w.last))))
}

/// A trend's value at a week, and how it reads.
fn trend_value(sums: &PeriodSums, i: usize, k: usize) -> Option<f64> {
    let t = sums.weeks[i].trend()?;
    match k {
        0 => t.read_share,
        1 => t.avoidable_share,
        _ => Some(t.average_context),
    }
}

fn trend_text(v: f64, k: usize) -> String {
    match k {
        0 | 1 => format!("{:.1}%", 100.0 * v),
        _ => tokens(v),
    }
}

const TRENDS: [&str; 3] = ["Read from the cache", "Avoidable share", "Average context"];

/// The line about the week marked: `19–25 Oct · $5,142 · zebrafish $2,000, … · read from the
/// cache 98.1%, avoidable 7.2%, context 312k · click: open this week`.
pub fn week_line(sums: &PeriodSums, i: usize) -> String {
    let w = &sums.weeks[i];
    let mut out = format!("{} · {}", tokprof_core::week::dates(w.first, w.last, false), usd_whole(w.amounts.usd));
    if w.kind == WeekKind::SoFar {
        out += " so far";
    }
    let names = crate::view::ProjectNames::new(sums.projects.iter().map(|x| x.name.as_str()));
    let mut parts: Vec<(String, f64)> = sums.projects.iter().filter(|x| x.weeks[i] >= 0.5).map(|x| (names.of(&x.name), x.weeks[i])).collect();
    parts.sort_by(|a, b| b.1.total_cmp(&a.1));
    if !parts.is_empty() {
        let shown: Vec<String> = parts.iter().take(4).map(|(n, v)| format!("{n} {}", usd_whole(*v))).collect();
        out += &format!(" · {}", shown.join(", "));
        if parts.len() > 4 {
            out += &format!(", {} more", parts.len() - 4);
        }
    }
    if let Some(t) = w.trend() {
        let mut tr = Vec::new();
        if let Some(r) = t.read_share {
            tr.push(format!("read from the cache {:.1}%", 100.0 * r));
        }
        if let Some(a) = t.avoidable_share {
            tr.push(format!("avoidable {:.1}%", 100.0 * a));
        }
        tr.push(format!("context {}", tokens(t.average_context)));
        out += &format!(" · {}", tr.join(", "));
    }
    out + " · click: open this week"
}

/// The weeks of the quarter: a column per week stacked by its six projects and `other`, the
/// months bracketed under them, the three trends, the legend and the line about the week marked.
pub fn weeks_card(s: &Desk, sums: &PeriodSums, p: &Pal, w: f32) -> El {
    let n = sums.weeks.len();
    let inner = w - 28.0;
    let plot_w = (inner - AXIS_W).max(200.0);
    let cw = plot_w / n as f32;
    let bar_w = (cw * 0.62).min(36.0);
    let (stacked, other) = sums.stacked();
    let max = sums.weeks.iter().map(|x| x.amounts.usd).fold(0.0, f64::max);
    let step = axis_step(max);
    let top = (max / step).ceil().max(1.0) * step;
    let y = |v: f64| PLOT_PAD + WEEKS_H - (v / top) as f32 * WEEKS_H;
    let (labels, mut plot) = axis(top, step, WEEKS_H, plot_w, p);
    let mark = marked_week(s, sums);
    for (i, wk) in sums.weeks.iter().enumerate() {
        let x0 = i as f32 * cw;
        let x = x0 + (cw - bar_w) / 2.0;
        if mark == Some(i) {
            plot = plot.child(el().absolute(x0, PLOT_PAD).w(cw).h(WEEKS_H).radius(4.0).fill(p.sel));
        }
        match wk.kind {
            WeekKind::NoLogs => {
                plot = plot.child(
                    text("no logs").key(format!("qweek-nologs-{i}")).font(pal::HOURS).color(p.ink3).single_line().absolute(x0 + cw / 2.0 - 17.0, y(0.0) - 16.0),
                );
            }
            WeekKind::Future => {
                plot = plot.child(el().key(format!("qweek-future-{i}")).absolute(x, y(0.0) - 1.0).w(bar_w).h(1.0).fill(p.ink3));
            }
            WeekKind::SoFar | WeekKind::Ended => {
                let mut stack = col()
                    .key(format!("qweek-{i}"))
                    .absolute(x, y(wk.amounts.usd))
                    .w(bar_w)
                    .h((y(0.0) - y(wk.amounts.usd)).max(0.0))
                    .radius(4.0)
                    .clip_rounded(4.0);
                // From the top: other, then the sixth project down to the first.
                let mut parts: Vec<(f64, Color)> = stacked.iter().enumerate().map(|(j, x)| (x.weeks[i], project_color(Some(j), p))).collect();
                parts.push((other[i], project_color(None, p)));
                for (v, c) in parts.into_iter().rev() {
                    let h = (v / top) as f32 * WEEKS_H;
                    if h >= 0.5 {
                        stack = stack.child(el().w_full().h(h).fill(c));
                    }
                }
                plot = plot.child(stack);
                let vy = y(wk.amounts.usd) - 15.0;
                let label = short_usd(wk.amounts.usd);
                plot = plot.child(
                    row()
                        .absolute(x0, vy)
                        .w(cw)
                        .justify_center()
                        .child(text(label).key(format!("qweek-value-{i}")).font(pal::SMALL_STRONG).color(p.ink).single_line()),
                );
                if wk.kind == WeekKind::SoFar {
                    plot = plot.child(
                        row()
                            .absolute(x0, vy - 13.0)
                            .w(cw)
                            .justify_center()
                            .child(text("so far").key("qweek-sofar").font(pal::HOURS).color(p.ink3).single_line()),
                    );
                }
            }
        }
    }
    // The hit areas over the columns: a week that opens.
    for (i, wk) in sums.weeks.iter().enumerate() {
        if wk.opens() {
            plot = plot.child(week_hit(i, wk.monday, i as f32 * cw, PLOT_PAD, cw, WEEKS_H, format!("qweek-hit-{i}"), sums));
        }
    }
    let mut under = el().w(plot_w).h(28.0);
    // In narrow columns a week reads its day, and its month only where the month changes.
    let short = cw < 52.0;
    for i in 0..n {
        let (mut a, b) = week_labels(sums, i);
        if short && i > 0 && local_time(noon(sums.weeks[i].monday), "%b") == local_time(noon(sums.weeks[i - 1].monday), "%b") {
            a = local_time(noon(sums.weeks[i].monday), "%-d");
        }
        let x0 = i as f32 * cw;
        under = under.child(row().absolute(x0, 0.0).w(cw).justify_center().child(text(a).font(pal::HOURS).color(p.ink3).single_line()));
        if let Some(b) = b {
            under = under.child(
                row().absolute(x0, 13.0).w(cw).justify_center().child(text(b).key(format!("qweek-cut-{i}")).font(pal::HOURS).color(p.ink3).single_line()),
            );
        }
    }
    let chart = col()
        .w_full()
        .child(row().w_full().child(labels).child(plot))
        .child(row().w_full().child(el().w(AXIS_W)).child(under))
        .child(row().w_full().child(el().w(AXIS_W)).child(brackets(s, sums, p, plot_w, cw)))
        .child(row().w_full().child(el().w(AXIS_W)).child(trends(s, sums, p, plot_w, cw, mark)));
    // The legend: the six projects, a click sets the filter; and `other`.
    let names = crate::view::ProjectNames::new(sums.projects.iter().map(|x| x.name.as_str()));
    let mut legend = row().key("qweeks-legend").w_full().pl(AXIS_W).flex_wrap().gaps(4.0, 12.0);
    for (j, x) in stacked.iter().enumerate() {
        let name = x.name.clone();
        legend = legend.child(
            row()
                .key(format!("qlegend-{j}"))
                .gap(5.0)
                .px(4.0)
                .h(20.0)
                .center_y()
                .radius(R_ROW)
                .hover(p.hover)
                .role("button")
                .label(x.name.clone())
                .on_press_tx(move |s: &mut Desk, tx| reports::filter_project(s, &name, tx))
                .child(el().w(10.0).h(10.0).radius(R_MARK).fill(project_color(Some(j), p)))
                .child(text(names.of(&x.name)).font(pal::SMALL).color(p.ink2).single_line()),
        );
    }
    if other.iter().any(|&v| v > 0.0) {
        legend = legend.child(
            row().gap(5.0).px(4.0).h(20.0).center_y().child(el().w(10.0).h(10.0).radius(R_MARK).fill(project_color(None, p))).child(
                text(format!("other, {}", count((sums.projects.len() - stacked.len()) as u64, "project", "projects")))
                    .key("qlegend-other")
                    .font(pal::SMALL)
                    .color(p.ink2)
                    .single_line(),
            ),
        );
    }
    let (line, color) = match mark {
        Some(i) => (week_line(sums, i), p.ink2),
        None => ("point at a week to read it; a click opens it in Week".to_string(), p.ink3),
    };
    let content = col()
        .w_full()
        .gap(8.0)
        .child(chart)
        .child(legend)
        .child(text(line).key("qweeks-read").w_full().pl(AXIS_W).wrap().max_lines(2).font(pal::SMALL_NUM).color(color));
    card(Some(Part::Weeks), "Weeks of the quarter", Some("spend by project · a click opens the week".into()), content, &s.reports, p)
}

/// The hit area of a week: the pointer marks it, a click opens it in Week.
#[allow(clippy::too_many_arguments)]
fn week_hit(i: usize, monday: i64, x: f32, y: f32, w: f32, h: f32, key: String, sums: &PeriodSums) -> El {
    let (a, _) = week_labels(sums, i);
    el().key(key)
        .absolute(x, y)
        .w(w)
        .h(h)
        .role("button")
        .label(format!("week of {a}"))
        .on_hover_change(move |s: &mut Desk, inside, _| {
            if inside {
                s.reports.hover = Some(Hover::Week(i));
            } else if s.reports.hover == Some(Hover::Week(i)) {
                s.reports.hover = None;
            }
        })
        .on_press_tx(move |s: &mut Desk, tx| reports::open_week(s, monday, tx))
}

/// The place of a date under the weeks, in points from the first week's left.
fn date_x(sums: &PeriodSums, date: i64, cw: f32) -> f32 {
    let first_monday = sums.weeks.first().map_or(date, |w| w.monday);
    let i = (date - first_monday) / 7;
    let wd = tokprof_core::calendar::weekday(date) as f32;
    i as f32 * cw + (wd + 0.5) / 7.0 * cw
}

/// What a month of the quarter reads under the weeks: `$11,632 · $375 a day`, `$3,954 so far ·
/// $330 a day`, or `has not begun`.
pub fn month_text(s: &Desk, sums: &PeriodSums, i: usize) -> String {
    let Some(m) = sums.months.get(i) else { return "has not begun".into() };
    let now = s.reports.now();
    let pace = sums.month_days(m, now, offset()).map(|d| format!(" · {} a day", usd_whole(m.total.usd / d as f64))).unwrap_or_default();
    let so_far = if m.complete { "" } else { " so far" };
    format!("{}{so_far}{pace}", usd_whole(m.total.usd))
}

/// The months of the quarter bracketed under its weeks, each with its name, its spend and its
/// pace; a click on a name opens Month on it.
fn brackets(s: &Desk, sums: &PeriodSums, p: &Pal, plot_w: f32, cw: f32) -> El {
    let mut out = el().key("qmonths-under").w(plot_w).h(48.0);
    for (i, ym) in sums.period.months().into_iter().enumerate() {
        let (a, b) = (date_x(sums, ym.first_date(), cw), date_x(sums, ym.next().first_date() - 1, cw));
        let begun = rhythm::midnight(ym.first_date(), offset()) <= s.reports.now();
        let line = p.ink3;
        out = out
            .child(el().absolute(a, 4.0).w(1.0).h(6.0).fill(line))
            .child(el().absolute(b, 4.0).w(1.0).h(6.0).fill(line))
            .child(el().absolute(a, 9.0).w((b - a).max(1.0)).h(1.0).fill(line));
        let mut name = row()
            .key(format!("qmonth-{i}"))
            .absolute(a, 14.0)
            .w((b - a).max(40.0))
            .justify_center()
            .radius(R_ROW)
            .child(text(ym.month_name()).font(pal::STRONG).color(if begun { p.ink } else { p.ink3 }).single_line());
        if begun {
            name = name.hover(p.hover).role("button").label(ym.name()).on_press_tx(move |s: &mut Desk, tx| reports::show_period(s, Period::Month(ym), tx));
        }
        out = out.child(name).child(
            row()
                .absolute(a, 31.0)
                .w((b - a).max(40.0))
                .justify_center()
                .child(text(month_text(s, sums, i)).key(format!("qmonth-text-{i}")).font(pal::HOURS).color(p.ink2).single_line()),
        );
    }
    out
}

/// The three trends, a strip each: a point per week with requests, broken where a week has none,
/// with the first and the last value named.
fn trends(s: &Desk, sums: &PeriodSums, p: &Pal, plot_w: f32, cw: f32, mark: Option<usize>) -> El {
    let _ = s;
    let n = sums.weeks.len();
    let mut out = col().key("qtrends").w(plot_w).gap(6.0).pt(6.0);
    for (k, name) in TRENDS.iter().enumerate() {
        let values: Vec<Option<f64>> = (0..n).map(|i| trend_value(sums, i, k)).collect();
        let known: Vec<f64> = values.iter().flatten().copied().collect();
        let (lo, hi) = known.iter().fold((f64::MAX, f64::MIN), |(a, b), &v| (a.min(v), b.max(v)));
        let pad = ((hi - lo) * 0.25).max(if k == 2 { 1.0 } else { 0.002 });
        let (lo, hi) = (lo - pad, hi + pad);
        let plot_h = TREND_H - 20.0;
        let yv = |v: f64| 18.0 + plot_h * (1.0 - ((v - lo) / (hi - lo).max(1e-9)) as f32);
        let first = known.first().map(|&v| trend_text(v, k));
        let last = known.last().map(|&v| trend_text(v, k));
        let head = match (first, last) {
            (Some(a), Some(b)) if known.len() > 1 => format!("{name} · {a} → {b}"),
            (Some(a), _) => format!("{name} · {a}"),
            _ => format!("{name} · no week with requests"),
        };
        let mut strip = el().key(format!("qtrend-{k}")).w(plot_w).h(TREND_H);
        strip = strip.child(el().absolute(0.0, TREND_H - 1.0).w(plot_w).h(1.0).fill(p.row_line));
        strip = strip.child(text(head).key("qtrend-head").font(pal::SMALL).color(p.ink2).single_line().absolute(2.0, 0.0));
        let mut shape = Shape::new();
        let mut seg: Vec<[f32; 2]> = Vec::new();
        for (i, v) in values.iter().enumerate() {
            match v {
                Some(v) => seg.push([i as f32 * cw + cw / 2.0, yv(*v)]),
                None if seg.len() > 1 => shape = shape.stroke(std::mem::take(&mut seg), 2.0, p.ink),
                None => seg.clear(),
            }
        }
        if seg.len() > 1 {
            shape = shape.stroke(seg, 2.0, p.ink);
        }
        // A lone week between breaks shows as its point.
        for (i, v) in values.iter().enumerate() {
            let alone = v.is_some() && (i == 0 || values[i - 1].is_none()) && (i + 1 == n || values[i + 1].is_none());
            if let (true, Some(v)) = (alone, v) {
                let (x, yy) = (i as f32 * cw + cw / 2.0, yv(*v));
                strip = strip.child(el().absolute(x - 2.5, yy - 2.5).w(5.0).h(5.0).radius(2.5).fill(p.ink));
            }
        }
        strip = strip.child(canvas(shape).absolute(0.0, 0.0).w(plot_w).h(TREND_H));
        if let Some(i) = mark
            && let Some(v) = values.get(i).copied().flatten()
        {
            let (x, yy) = (i as f32 * cw + cw / 2.0, yv(v));
            strip = strip.child(el().key("qtrend-mark").absolute(x - 4.0, yy - 4.0).w(8.0).h(8.0).radius(4.0).fill(p.ink).border(2.0, p.bar));
            let lx = if x + 60.0 < plot_w { x + 8.0 } else { x - 56.0 };
            strip = strip.child(
                text(trend_text(v, k))
                    .key("qtrend-value")
                    .font(pal::SMALL_STRONG)
                    .color(p.ink)
                    .single_line()
                    .absolute(lx, (yy - 7.0).clamp(12.0, TREND_H - 14.0)),
            );
        }
        for (i, wk) in sums.weeks.iter().enumerate() {
            if wk.opens() && values[i].is_some() {
                strip = strip.child(week_hit(i, wk.monday, i as f32 * cw, 14.0, cw, TREND_H - 14.0, format!("qtrend-hit-{k}-{i}"), sums));
            }
        }
        out = out.child(strip);
    }
    out
}

/// The Months card: a row per month of the quarter with a bar of its spend and its days, and the
/// line that compares the current month's pace with the month before's.
pub fn months_card(s: &Desk, sums: &PeriodSums, p: &Pal) -> El {
    let max = sums.months.iter().map(|m| m.total.usd).fold(0.0, f64::max).max(1e-9);
    let now = s.reports.now();
    let mut out = col().w_full().gap(2.0);
    for (i, ym) in sums.period.months().into_iter().enumerate() {
        let m = sums.months.get(i);
        let days = match m {
            Some(m) if m.complete => count(ym.next().first_date().saturating_sub(ym.first_date()) as u64, "day", "days"),
            Some(m) => {
                let ran = sums.month_days(m, now, offset()).unwrap_or(0);
                format!("{ran} of {} days", ym.next().first_date() - ym.first_date())
            }
            None => "has not begun".into(),
        };
        let line = row()
            .w_full()
            .h(26.0)
            .px(6.0)
            .gap(10.0)
            .center_y()
            .child(text(ym.month_name()).key("label").font(pal::BODY).color(if m.is_some() { p.ink } else { p.ink3 }).single_line().w(76.0))
            .child(crate::overlay::meter(m.map_or(0.0, |m| m.total.usd / max), p.accent, p, 6.0))
            .child(row().w(84.0).justify_end().child(text(days).key("days").font(pal::SMALL_NUM).color(p.ink2).single_line()));
        let mut line = cursor_row(line, Part::Months, i, s, p);
        if m.is_some() {
            line = line.role("button").label(ym.name()).on_press_tx(move |s: &mut Desk, tx| reports::show_period(s, Period::Month(ym), tx));
        }
        out = out.child(line);
    }
    if let Some(l) = sums.months_line(now, offset()) {
        out = out.child(text(l).key("months-line").w_full().wrap().pt(6.0).px(6.0).font(pal::SMALL_NUM).color(p.ink2));
    }
    card(Some(Part::Months), "Months", None, out, &s.reports, p)
}

/// Project shifts: how the quarter's projects moved over its ended weeks; a click sets the filter.
pub fn shifts_card(s: &Desk, sums: &PeriodSums, p: &Pal) -> El {
    let names = crate::view::ProjectNames::new(sums.projects.iter().map(|x| x.name.as_str()));
    let mut out = col().w_full().gap(4.0);
    if sums.shifts.is_empty() {
        out = out.child(text(NO_SHIFTS).key("shifts-none").w_full().wrap().font(pal::BODY).color(p.ink2));
    }
    for (i, x) in sums.shifts.iter().enumerate() {
        let project = x.project.clone();
        let line = col()
            .w_full()
            .px(8.0)
            .py(5.0)
            .gap(1.0)
            .child(
                row()
                    .w_full()
                    .gap(8.0)
                    .center_y()
                    .child(
                        text(format!("{} {}", names.of(&x.project), x.kind.word()))
                            .key("label")
                            .font(pal::STRONG)
                            .color(p.ink)
                            .single_line()
                            .truncate()
                            .shrink()
                            .min_w(0.0),
                    )
                    .child(rune::spacer().grow(1.0))
                    .child(text(x.detail()).key("detail").font(pal::SMALL_NUM).color(p.ink2).single_line()),
            )
            .child(text(format!("{} in the quarter", usd_whole(x.usd))).key("sub").font(pal::SMALL_NUM).color(p.ink3).single_line());
        let line = cursor_row(line, Part::Shifts, i, s, p)
            .role("button")
            .label(x.project.clone())
            .on_press_tx(move |s: &mut Desk, tx: &mut Tx| reports::filter_project(s, &project, tx));
        out = out.child(line);
    }
    card(Some(Part::Shifts), "Project shifts", Some("over the ended weeks".into()), out, &s.reports, p)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nine_cold_returns_read_five_ticks_and_four_more() {
        assert_eq!(ticks(9), (5, Some("+4".to_string())));
        assert_eq!(ticks(5), (5, None));
        assert_eq!(ticks(0), (0, None));
    }

    #[test]
    fn axis_steps_are_round_and_their_labels_short() {
        for (max, step) in [(0.0, 1.0), (9.0, 5.0), (412.0, 200.0), (1_221.0, 500.0), (5_142.0, 2_000.0)] {
            assert_eq!(axis_step(max), step, "{max}");
        }
        assert_eq!((axis_label(0.0), axis_label(500.0), axis_label(1_500.0), axis_label(2_000.0)), ("$0".into(), "$500".into(), "$1.5k".into(), "$2k".into()));
    }
}
