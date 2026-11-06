//! The day's lanes in the window: an hours row, the `You` lane with the sittings, a lane per
//! session with its work, waits, messages, questions and stops at the times of the log, a tail as
//! long as its cache stays warm, the now line, and under them the legend, the line about the time
//! under the pointer and the day's `sittings`, `answers` and `cache` lines.

use rune::{Color, Cx, El, ScrollSpec, col, el, row, spacer, text};
use tokprof_browse::browser::now_ms;
use tokprof_browse::render_rhythm::DayParts;
use tokprof_core::fmt::local_time;
use tokprof_core::rhythm::{Mark, StripLanes};

use crate::pal::{self, Pal};
use crate::view::{self, dot};
use crate::{Desk, ansi};

/// The width of the lane labels, the height of a lane and of the hours row, and of the bars,
/// marks and tails in a lane.
pub const LABEL_WIDTH: f32 = 236.0;
pub const LANE: f32 = 26.0;
const RULER: f32 = 26.0;
const BAR: f32 = 11.0;
const TICK: (f32, f32) = (2.0, 15.0);
const TAIL: f32 = 5.0;
/// Room right of the tracks, so that the last hour's label fits.
const RIGHT: f32 = 16.0;
/// The key of the hours row's scale, whose width every lane's track has.
pub const SCALE: &str = "scale";
const HOUR_MS: i64 = 3_600_000;
/// The height under the lanes: the legend with the pointer's line, and the day's lines.
const UNDER: f32 = 58.0;

/// Where the pointer rests over the lanes: the lane's session, `None` for the `You` lane, and how
/// far into the track, in points.
#[derive(Clone, Debug, PartialEq)]
pub struct Pointer {
    pub lane: Option<String>,
    pub x: f32,
}

/// The time the lanes show, in epoch ms.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Range {
    pub begin: i64,
    pub end: i64,
}

impl Range {
    /// Where a moment is on a track `width` points wide.
    pub fn x(&self, ts: i64, width: f32) -> f32 {
        ((ts - self.begin) as f64 / (self.end - self.begin).max(1) as f64 * width as f64) as f32
    }

    /// The moment at `x` points into such a track.
    pub fn ts(&self, x: f32, width: f32) -> i64 {
        self.begin + (x as f64 / width.max(1.0) as f64 * (self.end - self.begin) as f64).round() as i64
    }

    /// The left edge and the width of a span on such a track, cut to the range: at least 1 point
    /// wide, or `None` outside it.
    pub fn bar(&self, a: i64, b: i64, width: f32) -> Option<(f32, f32)> {
        if b < self.begin || a > self.end {
            return None;
        }
        let (x0, x1) = (self.x(a.max(self.begin), width), self.x(b.min(self.end), width));
        Some((x0, (x1 - x0).max(1.0)))
    }

    fn holds(&self, ts: i64) -> bool {
        ts >= self.begin && ts <= self.end
    }
}

/// The time the lanes show. `24 h` shows the strip's 24 hours; `Fit` the whole hours from the
/// day's first action to the end of its last tail or `now`, whichever is later, at least two
/// hours, within the strip's.
pub fn range(lanes: &StripLanes, tails: &[(i64, i64)], now: Option<i64>, fit: bool) -> Range {
    let day = Range { begin: lanes.begin, end: lanes.end };
    if !fit {
        return day;
    }
    let marks = lanes.lanes.iter().flat_map(|l| l.marks.iter().map(|m| (m.0, m.1)));
    let first = lanes.you.iter().copied().chain(marks.clone()).map(|m| m.0).min();
    let last = lanes.you.iter().copied().chain(marks).map(|m| m.1).chain(tails.iter().map(|t| t.1)).chain(now).max();
    let (Some(first), Some(last)) = (first, last) else { return day };
    let begin = lanes.hours.iter().map(|h| h.0).filter(|&h| h <= first).max().unwrap_or(day.begin);
    let end = lanes.hours.iter().map(|h| h.0).filter(|&h| h > last).min().unwrap_or(day.end);
    Range { begin, end: end.max(begin + 2 * HOUR_MS).min(day.end) }
}

/// Spans of one kind closer than a point merged into one, so that a lane takes at most as many
/// shapes as its track has points.
fn merged(mut spans: Vec<(i64, i64)>, point_ms: i64) -> Vec<(i64, i64)> {
    spans.sort_unstable();
    let mut out: Vec<(i64, i64)> = Vec::with_capacity(spans.len());
    for (a, b) in spans {
        match out.last_mut() {
            Some(last) if a < last.1 + point_ms => last.1 = last.1.max(b),
            _ => out.push((a, b)),
        }
    }
    out
}

/// The color of a mark, as work-rhythm colors it.
pub fn mark_color(m: Mark, p: &Pal) -> Color {
    match m {
        Mark::Work => p.work,
        Mark::Wait => p.wait,
        Mark::Message => p.message,
        Mark::Question => p.question,
        Mark::Interrupt => p.stop,
        Mark::Present => p.you,
    }
}

fn mark_key(m: Mark) -> &'static str {
    match m {
        Mark::Work => "work",
        Mark::Wait => "wait",
        Mark::Message => "message",
        Mark::Question => "question",
        Mark::Interrupt => "interrupt",
        Mark::Present => "present",
    }
}

/// The line under the lanes about the time under the pointer: `14:32 · ecad11 Инфографика
/// прерываний · agent working`, the lane's label, and what it shows there, the cache's tail
/// included.
pub fn pointer_text(lanes: &StripLanes, lives: &[Option<(i64, i64)>], range: Range, pointer: &Pointer, width: f32) -> String {
    let ts = range.ts(pointer.x, width);
    let slack = ((range.end - range.begin) as f64 * 2.0 / width.max(1.0) as f64) as i64;
    let (name, index) = match &pointer.lane {
        None => ("you".to_string(), None),
        Some(id) => match lanes.lanes.iter().position(|l| &l.id == id) {
            Some(i) => (lanes.lanes[i].label.clone(), Some(i)),
            None => return String::new(),
        },
    };
    let warm = index.and_then(|i| lives.get(i).copied().flatten()).filter(|&(a, b)| ts >= a && ts < b).map(|_| "cache warm");
    let mark = lanes.at(index, ts, slack).map(Mark::name).or(warm);
    let mut out = format!("{} · {}", local_time(ts, "%H:%M"), name.trim_end());
    if let Some(m) = mark {
        out.push_str(" · ");
        out.push_str(m);
    }
    out
}

/// The day's summary in the bar: when its first action was, to now or to its last, its sittings
/// and its sessions with actions.
pub fn summary(s: &Desk) -> Option<String> {
    let parts = s.browser.day_parts()?;
    let (_, lanes) = parts.strip.as_ref()?;
    let marks = lanes.lanes.iter().flat_map(|l| l.marks.iter().map(|m| (m.0, m.1)));
    let first = lanes.you.iter().copied().chain(marks.clone()).map(|m| m.0).min()?;
    let last = lanes.you.iter().copied().chain(marks).map(|m| m.1).max()?;
    let to = if s.browser.day_is_latest() { "now".to_string() } else { local_time(last, "%H:%M") };
    let sittings = line_of(&parts.below, "sittings").and_then(|l| l.split(|c: char| !c.is_ascii_digit()).find(|n| !n.is_empty()).map(str::to_string));
    let sittings = sittings.unwrap_or_else(|| lanes.you.len().to_string());
    let plural = |n: &str, one: &str, many: &str| format!("{n} {}", if n == "1" { one } else { many });
    Some(format!(
        "{} to {to}, {}, {}",
        local_time(first, "%H:%M"),
        plural(&sittings, "sitting", "sittings"),
        plural(&lanes.lanes.len().to_string(), "session", "sessions")
    ))
}

/// The line of a day's text that starts with `label`, without its color codes or its label.
fn line_of(text: &str, label: &str) -> Option<String> {
    text.lines().map(|l| ansi::parse(l).text).find(|l| l.starts_with(label)).map(|l| l[label.len()..].trim().to_string())
}

/// What the lanes show, for `memo`: the day, the zoom, the size, the selection, the pointer,
/// each lane's cache and the minute.
pub fn version(s: &Desk, cx: &Cx<Desk>, max_h: f32, dark: bool) -> u64 {
    let b = &s.browser;
    let parts = b.day_parts();
    let lanes: Vec<_> =
        parts.and_then(|p| p.strip.as_ref()).map_or(Vec::new(), |(_, l)| l.lanes.iter().map(|l| (b.cache_life(&l.id), b.row_of(&l.id))).collect());
    let (known, pending) = b.known_total();
    view::hash_of((
        (parts.map(|p| p as *const DayParts as usize), b.day_noon(), b.day_is_latest(), s.fit, dark, max_h.to_bits()),
        (cx.layout_size(SCALE).map(|sz| sz[0].to_bits()), b.selected().map(|r| b.session(r)), s.pointer.as_ref().map(|p| (p.lane.clone(), p.x.to_bits()))),
        (view::minute(), lanes, known.to_bits(), pending, s.pane == crate::Focus::Lanes),
    ))
}

/// The lanes of the day the browser shows, at most `max_h` points high with what is under them.
pub fn lanes(s: &Desk, cx: &Cx<Desk>, p: &Pal, max_h: f32) -> El {
    match s.browser.day_parts() {
        Some(parts) => lanes_of(s, parts, cx, p, max_h),
        None => {
            let note = format!("computing the day of {}…", local_time(s.browser.day_noon(), "%Y-%m-%d"));
            frame(s, row().h(3.0 * LANE).px(16.0).center_y().child(text(note).key("lanes-note").font(pal::BODY).color(p.ink2)), p)
        }
    }
}

fn frame(s: &Desk, body: El, p: &Pal) -> El {
    col()
        .key("lanes")
        .w_full()
        .fill(p.track)
        .child(view::focus_line("lanes-focus", s.pane == crate::Focus::Lanes, p))
        .child(body)
        .child(el().w_full().h(1.0).fill(p.line))
}

/// The lanes of a day.
pub fn lanes_of(s: &Desk, parts: &DayParts, cx: &Cx<Desk>, p: &Pal, max_h: f32) -> El {
    let b = &s.browser;
    let frame = |body: El| frame(s, body, p);
    let Some((_, lanes)) = &parts.strip else {
        return frame(
            row().h(3.0 * LANE).px(16.0).center_y().child(text("nobody typed in a session this day").key("lanes-note").font(pal::BODY).color(p.ink2)),
        );
    };
    let width = cx.layout_size(SCALE).map_or(1000.0, |sz| sz[0]).max(120.0);
    let now = now_ms();
    let now_in = (b.day_is_latest() && now >= lanes.begin && now < lanes.end).then_some(now);
    let lives: Vec<Option<(i64, i64)>> = lanes.lanes.iter().map(|l| b.cache_life(&l.id)).collect();
    let tails: Vec<(i64, i64)> = lives.iter().flatten().copied().collect();
    let range = range(lanes, &tails, now_in, s.fit);
    let grid = Grid { range, width, now: now_in.filter(|&n| range.holds(n)) };
    let mut body = col().key("lanes-body").w_full().child(ruler(lanes, &grid, p)).child(you_lane(lanes, &grid, p));
    let selected = b.selected().map(|r| b.session(r).to_string());
    for (lane, life) in lanes.lanes.iter().zip(&lives) {
        body = body.child(session_lane(s, lanes, lane, *life, selected.as_deref(), &grid, p));
    }
    let scroller = col().key("lanes-scroll").w_full().max_h((max_h - UNDER).max(3.0 * LANE)).scroll(ScrollSpec { stick_bottom: false }).child(body);
    let pointer = s.pointer.as_ref().map(|ptr| pointer_text(lanes, &lives, range, ptr, width)).unwrap_or_default();
    // Under the tracks when they leave room; from the window's edge in a narrow one.
    let lead = if width >= 900.0 { LABEL_WIDTH + 14.0 } else { 14.0 };
    frame(col().w_full().pb(8.0).child(scroller).child(legend(pointer, lead, p)).child(day_lines(&parts.below, lead, p)))
}

/// What every track of the lanes shares: the time it shows, its width, and now when it is in it.
struct Grid {
    range: Range,
    width: f32,
    now: Option<i64>,
}

impl Grid {
    /// The hours in the range, with their local hour, and whether there is room for the half
    /// hours.
    fn hours<'a>(&self, lanes: &'a StripLanes) -> impl Iterator<Item = (i64, i64)> + 'a {
        let range = self.range;
        lanes.hours.iter().copied().filter(move |h| range.holds(h.0))
    }

    fn halves(&self) -> bool {
        self.width / ((self.range.end - self.range.begin) as f32 / HOUR_MS as f32) >= 60.0
    }

    /// A lane's track: the hour lines, the time after now shaded, the now line, and `marks`.
    fn track(&self, key: String, lanes: &StripLanes, marks: Vec<El>, p: &Pal) -> El {
        let mut t = el().key(key).grow(1.0).min_w(0.0).h(LANE).mr(RIGHT);
        for (h, _) in self.hours(lanes) {
            let x = self.range.x(h, self.width);
            t = t.child(el().absolute(x, 0.0).w(1.0).h(LANE).fill(p.hour));
            if self.halves() {
                t = t.child(el().absolute(self.range.x(h + HOUR_MS / 2, self.width), LANE * 0.3).w(1.0).h(LANE * 0.4).fill(p.hour));
            }
        }
        if let Some(now) = self.now {
            let x = self.range.x(now, self.width);
            t = t.child(el().absolute(x, 0.0).w((self.width - x).max(0.0)).h(LANE).fill(p.later));
        }
        t = t.children(marks);
        if let Some(now) = self.now {
            t = t.child(el().key("now").absolute(self.range.x(now, self.width) - 0.75, 0.0).w(1.5).h(LANE).fill(p.now));
        }
        t
    }

    fn bar(&self, key: &'static str, a: i64, b: i64, h: f32, c: Color) -> Option<El> {
        let (x, w) = self.range.bar(a, b, self.width)?;
        Some(el().key(key).absolute(x, (LANE - h) / 2.0).w(w).h(h).radius(2.0).fill(c))
    }

    fn tick(&self, key: &'static str, ts: i64, c: Color) -> Option<El> {
        self.range
            .holds(ts)
            .then(|| el().key(key).absolute(self.range.x(ts, self.width) - TICK.0 / 2.0, (LANE - TICK.1) / 2.0).w(TICK.0).h(TICK.1).radius(1.0).fill(c))
    }

    /// The cache's tail: warm from the last request until it goes cold, fading out; solid up to
    /// now while it is still warm, and a mark where it went cold once it is.
    fn tail(&self, (last, cold_at): (i64, i64), p: &Pal) -> Vec<El> {
        let fade = |a: i64, b: i64, key: &'static str| {
            self.range.bar(a, b, self.width).map(|(x, w)| {
                el().key(key)
                    .absolute(x, (LANE - TAIL) / 2.0)
                    .w(w)
                    .h(TAIL)
                    .linear_gradient(std::f32::consts::FRAC_PI_2, [(0.0, p.tail), (1.0, p.tail.alpha(0.0))])
            })
        };
        match self.now {
            Some(now) if cold_at > now => {
                let solid = self.range.bar(last, now, self.width).map(|(x, w)| el().key("tail").absolute(x, (LANE - TAIL) / 2.0).w(w).h(TAIL).fill(p.tail));
                solid.into_iter().chain(fade(now, cold_at, "tail-ahead")).collect()
            }
            _ => {
                let end = self
                    .range
                    .holds(cold_at)
                    .then(|| el().key("cold-at").absolute(self.range.x(cold_at, self.width) - 0.75, (LANE - 9.0) / 2.0).w(1.5).h(9.0).fill(p.work.alpha(0.5)));
                fade(last, cold_at, "tail").into_iter().chain(end).collect()
            }
        }
    }
}

fn ruler(lanes: &StripLanes, g: &Grid, p: &Pal) -> El {
    let per_hour = g.width / ((g.range.end - g.range.begin) as f32 / HOUR_MS as f32);
    let every = if per_hour >= 44.0 {
        1
    } else if per_hour >= 22.0 {
        2
    } else {
        3
    };
    let mut scale = el().key(SCALE).grow(1.0).min_w(0.0).h(RULER).mr(RIGHT);
    let now_x = g.now.map(|n| g.range.x(n, g.width));
    for (h, hour) in g.hours(lanes) {
        let x = g.range.x(h, g.width);
        // The now label covers the hours next to it.
        if hour % every == 0 && now_x.is_none_or(|n| (x - n).abs() > 40.0) {
            scale = scale.child(text(format!("{hour:02}:00")).key(format!("hour-{hour}")).font(pal::HOURS).color(p.ink3).single_line().absolute(x - 14.0, 7.0));
        }
    }
    if let Some(now) = g.now {
        let x = g.range.x(now, g.width);
        scale = scale.child(
            row()
                .key("now-label")
                .absolute(x - 19.0, 4.0)
                .h(17.0)
                .px(5.0)
                .center_y()
                .radius(4.0)
                .fill(p.now)
                .child(text(local_time(now, "%H:%M")).font(pal::SMALL_STRONG).color(p.on_accent).single_line()),
        );
    }
    row()
        .key("hours")
        .w_full()
        .h(RULER)
        .fill(p.track)
        .sticky_top(0.0)
        .child(row().w(LABEL_WIDTH).h(RULER).px(14.0).center_y().child(text("Lanes, most actions first").font(pal::SMALL).color(p.ink3).single_line()))
        .child(scale)
}

fn label(head: String, rest: String, agent: Option<Color>, p: &Pal) -> El {
    let mut l = row().w(LABEL_WIDTH).h(LANE).pl(14.0).pr(10.0).gap(7.0).center_y();
    if let Some(c) = agent {
        l = l.child(dot(c, 7.0));
    }
    l.child(text(head).font(pal::STRONG).color(p.ink).single_line())
        .child(text(rest).font(pal::BODY).color(p.ink2).single_line().truncate().shrink().min_w(0.0))
}

fn hover(track: El, lane: Option<String>) -> El {
    track.on_pointer_move_local(move |s: &mut Desk, at, _| s.pointer = Some(Pointer { lane: lane.clone(), x: at.x })).on_hover_change(
        |s: &mut Desk, inside, _| {
            if !inside {
                s.pointer = None;
            }
        },
    )
}

fn you_lane(lanes: &StripLanes, g: &Grid, p: &Pal) -> El {
    let marks =
        lanes.you.iter().enumerate().filter_map(|(i, &(a, b))| g.bar("present", a, b, BAR - 1.0, p.you).map(|e| e.key(format!("present-{i}")))).collect();
    let track = hover(g.track("track-you".into(), lanes, marks, p), None);
    row().key("lane-you").w_full().h(LANE).child(label("You".into(), "at the keyboard".into(), None, p)).child(track)
}

#[allow(clippy::too_many_arguments)]
fn session_lane(
    s: &Desk,
    lanes: &StripLanes,
    lane: &tokprof_core::rhythm::StripLane,
    life: Option<(i64, i64)>,
    selected: Option<&str>,
    g: &Grid,
    p: &Pal,
) -> El {
    let b = &s.browser;
    let point_ms = ((g.range.end - g.range.begin) as f64 / g.width.max(1.0) as f64) as i64;
    let spans = |m: Mark| merged(lane.marks.iter().filter(|x| x.2 == m).map(|&(a, b, _)| (a, b)).collect(), point_ms);
    let mut marks: Vec<El> = life.map(|l| g.tail(l, p)).unwrap_or_default();
    // Siblings need keys of their own: a mark's kind and its place among its kind.
    for m in [Mark::Wait, Mark::Work] {
        marks.extend(
            spans(m)
                .into_iter()
                .enumerate()
                .filter_map(|(i, (a, b))| g.bar(mark_key(m), a, b, BAR, mark_color(m, p)).map(|e| e.key(format!("{}-{i}", mark_key(m))))),
        );
    }
    for m in [Mark::Question, Mark::Message, Mark::Interrupt] {
        let mut at: Vec<i64> = lane.marks.iter().filter(|x| x.2 == m).map(|x| x.0).collect();
        at.sort_unstable();
        at.dedup_by(|b, a| *b - *a < point_ms);
        marks.extend(at.into_iter().enumerate().filter_map(|(i, ts)| g.tick(mark_key(m), ts, mark_color(m, p)).map(|e| e.key(format!("{}-{i}", mark_key(m))))));
    }
    // The lane's session as the list shows it, when it is listed.
    let (head, rest, agent) = match b.row_of(&lane.id) {
        Some(r) => {
            let c = b.cells(r, now_ms());
            let project = c.project.rsplit(['/', '\\']).next().unwrap_or(c.project).to_string();
            (project, c.title.unwrap_or(c.project).to_string(), Some(p.agents[c.agent_place % p.agents.len()]))
        }
        None => (lane.label.split_whitespace().next().unwrap_or("").to_string(), lane.label.split_once(' ').map_or(String::new(), |x| x.1.to_string()), None),
    };
    let is_selected = selected.is_some_and(|id| b.row_of(&lane.id).is_some_and(|r| b.session(r) == id));
    let id = lane.id.clone();
    row()
        .key(format!("lane-{}", lane.id))
        .w_full()
        .h(LANE)
        .fill(if is_selected { p.sel } else { Color::TRANSPARENT })
        .hover(if is_selected { p.sel } else { p.hover })
        .role("button")
        .label(lane.label.clone())
        .on_press_tx(move |s: &mut Desk, tx| {
            s.pane = crate::Focus::Lanes;
            s.open_lane(&id, tx);
        })
        .child(label(head, rest, agent, p))
        .child(hover(g.track(format!("track-{}", lane.id), lanes, marks, p), Some(lane.id.clone())))
}

fn legend(pointer: String, lead: f32, p: &Pal) -> El {
    let mut line = row().key("legend").w_full().h(24.0).pl(lead).pr(RIGHT).gap(14.0).center_y();
    for m in [Mark::Work, Mark::Wait, Mark::Message, Mark::Question, Mark::Interrupt, Mark::Present] {
        let swatch = match m {
            Mark::Work | Mark::Wait | Mark::Present => el().w(10.0).h(10.0).radius(2.0).fill(mark_color(m, p)),
            _ => el().w(2.0).h(12.0).radius(1.0).fill(mark_color(m, p)),
        };
        line = line.child(row().gap(6.0).center_y().child(swatch).child(text(m.name()).font(pal::SMALL).color(p.ink2).single_line()));
    }
    let tail = el().w(14.0).h(TAIL).linear_gradient(std::f32::consts::FRAC_PI_2, [(0.0, p.tail), (1.0, p.tail.alpha(0.0))]);
    line = line.child(row().gap(6.0).center_y().child(tail).child(text("cache warm").font(pal::SMALL).color(p.ink2).single_line()));
    line.child(spacer().grow(1.0)).child(text(pointer).key("pointer-text").font(pal::SMALL_NUM).color(p.ink2).single_line().truncate().shrink().min_w(0.0))
}

/// The day's `sittings`, `answers` and `cache` lines, each after its label; the cache line in
/// the cold color when the day had cold returns.
fn day_lines(below: &str, lead: f32, p: &Pal) -> El {
    let mut line = row().key("day-lines").w_full().h(22.0).pl(lead).pr(RIGHT).gap(24.0).center_y();
    for label in ["sittings", "answers", "cache"] {
        if let Some(rest) = line_of(below, label) {
            let color = if label == "cache" && rest.contains("cold return") && !rest.starts_with("no ") { p.cold } else { p.ink2 };
            line = line.child(
                row()
                    .key(format!("day-{label}"))
                    .gap(5.0)
                    .center_y()
                    .shrink()
                    .min_w(0.0)
                    .child(text(label).font(pal::SMALL_STRONG).color(p.ink).single_line())
                    .child(text(rest).font(pal::SMALL).color(color).single_line().truncate().shrink().min_w(0.0).selectable(true)),
            );
        }
    }
    line
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokprof_core::rhythm::StripLane;

    const MIN: i64 = 60_000;

    /// A day from 04:00, as 0, with an hour mark at every hour.
    fn lanes() -> StripLanes {
        StripLanes {
            begin: 0,
            end: 24 * HOUR_MS,
            hours: (0..24).map(|k| (k * HOUR_MS, (k + 4) % 24)).collect(),
            you: vec![(4 * HOUR_MS + 50 * MIN, 7 * HOUR_MS + 20 * MIN)],
            lanes: vec![StripLane {
                id: "ecad11b9".into(),
                label: "ecad11 Инфографика прерываний".into(),
                marks: vec![(10 * HOUR_MS, 10 * HOUR_MS + 30 * MIN, Mark::Work), (10 * HOUR_MS + 40 * MIN, 10 * HOUR_MS + 40 * MIN, Mark::Message)],
            }],
        }
    }

    #[test]
    fn a_short_piece_of_work_is_a_bar_at_its_time_at_least_a_point_wide() {
        let day = Range { begin: 0, end: 24 * HOUR_MS };
        // 14:32:10 to 14:32:50 on a day from 04:00, on a track of 1152 points, 48 to an hour.
        let (a, b) = (10 * HOUR_MS + 32 * MIN + 10_000, 10 * HOUR_MS + 32 * MIN + 50_000);
        let (x, w) = day.bar(a, b, 1152.0).unwrap();
        assert!((x - (10.0 * 48.0 + 32.0 * 0.8 + 10.0 / 75.0)).abs() < 0.01, "10 h 32 m 10 s into the day: {x}");
        assert_eq!(w, 1.0, "40 seconds are half a point, drawn as one");
        assert_eq!(day.bar(0, 2 * HOUR_MS, 1152.0).map(|b| b.1), Some(96.0), "two hours are 96 points");
        assert_eq!(day.ts(day.x(5 * HOUR_MS, 1152.0), 1152.0), 5 * HOUR_MS, "the pointer reads the time back");
        let fit = Range { begin: 4 * HOUR_MS, end: 11 * HOUR_MS };
        assert_eq!(fit.bar(0, HOUR_MS, 700.0), None, "outside the range");
        assert_eq!(fit.bar(3 * HOUR_MS, 5 * HOUR_MS, 700.0), Some((0.0, 100.0)), "cut to the range");
    }

    #[test]
    fn a_tail_crosses_now_while_the_cache_is_warm() {
        // The last request at 13:58, the cache an hour, now 14:33: on a day from 04:00.
        let (last, now) = (9 * HOUR_MS + 58 * MIN, 10 * HOUR_MS + 33 * MIN);
        let g = Grid { range: Range { begin: 0, end: 24 * HOUR_MS }, width: 1440.0, now: Some(now) };
        let p = crate::pal::pal(false);
        let warm = g.tail((last, last + HOUR_MS), &p);
        assert_eq!(warm.len(), 2, "solid to now, fading after it");
        let cold = Grid { now: Some(last + 2 * HOUR_MS), ..g }.tail((last, last + HOUR_MS), &p);
        assert_eq!(cold.len(), 2, "faded, and the mark where it went cold");
    }

    #[test]
    fn fit_shows_the_whole_hours_from_the_first_action_to_the_last_tail_or_now() {
        let l = lanes();
        // The first action at 08:50 (4 h 50 m), a tail to 14:58 (10 h 58 m), now 14:33.
        let tail = (9 * HOUR_MS + 58 * MIN, 10 * HOUR_MS + 58 * MIN);
        let r = range(&l, &[tail], Some(10 * HOUR_MS + 33 * MIN), true);
        assert_eq!((r.begin, r.end), (4 * HOUR_MS, 11 * HOUR_MS), "08:00 to 15:00");
        assert_eq!(range(&l, &[tail], None, false), Range { begin: 0, end: 24 * HOUR_MS }, "24 h is the strip's day");
        let early = StripLanes { you: vec![(HOUR_MS, HOUR_MS + MIN)], lanes: Vec::new(), ..lanes() };
        assert_eq!(range(&early, &[], None, true), Range { begin: HOUR_MS, end: 3 * HOUR_MS }, "at least two hours");
    }

    #[test]
    fn the_pointer_line_names_the_time_the_lane_and_what_it_shows_the_tail_included() {
        let width = 1152.0;
        let day = Range { begin: 0, end: 24 * HOUR_MS };
        let lives = [Some((10 * HOUR_MS + 30 * MIN, 11 * HOUR_MS + 30 * MIN))];
        let at = |lane: Option<&str>, ts: i64| pointer_text(&lanes(), &lives, day, &Pointer { lane: lane.map(String::from), x: day.x(ts, width) }, width);
        let hhmm = |ts: i64| local_time(ts, "%H:%M");
        let work = 10 * HOUR_MS + 15 * MIN;
        assert_eq!(at(Some("ecad11b9"), work), format!("{} · ecad11 Инфографика прерываний · agent working", hhmm(work)));
        let message = 10 * HOUR_MS + 40 * MIN;
        assert_eq!(at(Some("ecad11b9"), message), format!("{} · ecad11 Инфографика прерываний · your message", hhmm(message)));
        let warm = 11 * HOUR_MS;
        assert_eq!(at(Some("ecad11b9"), warm), format!("{} · ecad11 Инфографика прерываний · cache warm", hhmm(warm)));
        assert_eq!(at(Some("ecad11b9"), 2 * HOUR_MS), format!("{} · ecad11 Инфографика прерываний", hhmm(2 * HOUR_MS)), "nothing there");
        assert_eq!(at(None, 6 * HOUR_MS), format!("{} · you · you were there", hhmm(6 * HOUR_MS)));
        assert_eq!(at(Some("gone"), work), "", "a lane of another day");
    }

    #[test]
    fn spans_closer_than_a_point_merge() {
        assert_eq!(merged(vec![(10, 20), (0, 5), (24, 30), (100, 110)], 5), vec![(0, 5), (10, 30), (100, 110)]);
        assert_eq!(merged(vec![], 5), vec![]);
    }
}
