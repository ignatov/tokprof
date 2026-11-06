//! The day view in the window: the text above the strip, the strip drawn with shapes at the times
//! of the log, the line about the time under the pointer, the legend, and the text below.

use rune::tok::Theme;
use rune::{Color, Cx, El, TextStyle, col, el, rich, row, text};
use tokprof_browse::render_rhythm::DayParts;
use tokprof_core::fmt::local_time;
use tokprof_core::rhythm::{Mark, StripLanes};

use crate::Desk;
use crate::ansi;
use crate::view::{self, ansi_color};

/// The width of the lane labels, the height of a lane, and of the bars and moments in it.
pub const LABEL_WIDTH: f32 = 240.0;
pub const LANE: f32 = 18.0;
const BAR: f32 = 12.0;
const MOMENT: f32 = 3.0;
/// The key of the hours row's track, whose width every lane's track has.
pub const HOURS: &str = "hours-track";
const DAY_MS: f64 = 86_400_000.0;
const LABEL: TextStyle = TextStyle::new(12.0, 400.0);

/// Where the pointer rests over the strip: the lane's session, `None` for the `you` lane, and how
/// far into the track, in points.
#[derive(Clone, Debug, PartialEq)]
pub struct Pointer {
    pub lane: Option<String>,
    pub x: f32,
}

/// Where a moment of the day is on a track `width` points wide that starts at `begin`.
pub fn x_of(ts: i64, begin: i64, width: f32) -> f32 {
    ((ts - begin) as f64 / DAY_MS * width as f64) as f32
}

/// The moment at `x` points into such a track.
pub fn ts_at(x: f32, begin: i64, width: f32) -> i64 {
    begin + (x as f64 / width.max(1.0) as f64 * DAY_MS).round() as i64
}

/// The left edge and the width of a span of the day on such a track: at least 1 point wide.
pub fn bar(a: i64, b: i64, begin: i64, width: f32) -> (f32, f32) {
    let (x0, x1) = (x_of(a, begin, width), x_of(b, begin, width));
    (x0, (x1 - x0).max(1.0))
}

/// Spans of one kind closer than a point merged into one, so that a lane takes at most as many
/// shapes as the track has points.
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

/// The color of a mark, as the strip in a terminal colors it.
pub fn mark_color(m: Mark, dark: bool, t: &Theme) -> Color {
    match m {
        Mark::Work => ansi_color(2, dark, t),
        Mark::Wait => ansi_color(3, dark, t).alpha(0.55),
        Mark::Message => ansi_color(6, dark, t),
        Mark::Question => ansi_color(5, dark, t),
        Mark::Interrupt => ansi_color(1, dark, t),
        Mark::Present => ansi_color(4, dark, t),
    }
}

/// A text with color codes as the window shows it.
fn text_block(s: &str, t: &Theme, dark: bool) -> El {
    let styled = ansi::parse(s.trim_end_matches('\n'));
    rich(styled.text.clone(), view::spans(&styled, dark, t)).font(view::MONO).color(t.text).selectable(true)
}

/// The day as the window draws it.
pub fn day_view(s: &Desk, parts: &DayParts, cx: &mut Cx<Desk>, t: &Theme, dark: bool) -> El {
    let mut body = col().key("day").w_full().gap(8.0).child(text_block(&parts.above, t, dark).key("above"));
    if let Some((_, lanes)) = &parts.strip {
        let width = cx.layout_size(HOURS).map_or(960.0, |s| s[0]).max(120.0);
        body = body.child(strip(s, lanes, width, t, dark)).child(pointer_line(s, lanes, width, t)).child(legend(t, dark));
    }
    if !parts.below.trim().is_empty() {
        body = body.child(text_block(&parts.below, t, dark).key("below"));
    }
    body
}

fn strip(s: &Desk, lanes: &StripLanes, width: f32, t: &Theme, dark: bool) -> El {
    let point_ms = (DAY_MS / width as f64) as i64;
    let mut hours = el().key(HOURS).grow(1.0).min_w(0.0).h(LANE);
    // A label at every hour that has room for it, else at every other hour.
    let every = if width / 24.0 >= 22.0 { 1 } else { 2 };
    for (k, &(h, hour)) in lanes.hours.iter().enumerate() {
        let x = x_of(h, lanes.begin, width);
        hours = hours.child(el().absolute(x, 0.0).w(1.0).h(LANE).fill(t.separator));
        if k % every == 0 {
            hours = hours.child(text(format!("{hour:02}")).font(view::MONO).color(t.text_secondary).absolute(x + 3.0, 1.0));
        }
    }
    let hours_row = row().key("hours").w_full().h(LANE).fill(t.bg).sticky_top(0.0).child(el().w(LABEL_WIDTH).h(LANE)).child(hours);
    let mut out = col().key("strip").w_full().gap(4.0).child(hours_row);
    let track = |key: String, spans: &[(Vec<(i64, i64)>, Mark)], moments: &[(Vec<i64>, Mark)], lane: Option<String>| {
        let mut track = el().key(key).grow(1.0).min_w(0.0).h(LANE).radius(3.0).fill(t.hover);
        for &(h, _) in &lanes.hours {
            track = track.child(el().absolute(x_of(h, lanes.begin, width), 0.0).w(1.0).h(LANE).fill(t.separator));
        }
        for (spans, m) in spans {
            for &(a, b) in &merged(spans.clone(), point_ms) {
                let (x, w) = bar(a, b, lanes.begin, width);
                track = track.child(el().key(mark_key(*m)).absolute(x, (LANE - BAR) / 2.0).w(w).h(BAR).radius(2.0).fill(mark_color(*m, dark, t)));
            }
        }
        for (moments, m) in moments {
            let mut at: Vec<i64> = moments.clone();
            at.sort_unstable();
            at.dedup_by(|b, a| *b - *a < point_ms);
            for ts in at {
                let x = x_of(ts, lanes.begin, width) - MOMENT / 2.0;
                track = track.child(el().key(mark_key(*m)).absolute(x, 1.0).w(MOMENT).h(LANE - 2.0).radius(1.5).fill(mark_color(*m, dark, t)));
            }
        }
        let here = lane.clone();
        track.on_pointer_move_local(move |s: &mut Desk, p, _| s.pointer = Some(Pointer { lane: here.clone(), x: p.x })).on_hover_change(
            |s: &mut Desk, inside, _| {
                if !inside {
                    s.pointer = None;
                }
            },
        )
    };
    let you = track("track-you".into(), &[(lanes.you.clone(), Mark::Present)], &[], None);
    out = out.child(row().key("lane-you").w_full().h(LANE).child(label("you".into(), t)).child(you));
    for lane in &lanes.lanes {
        let of = |m: Mark| lane.marks.iter().filter(|x| x.2 == m).map(|&(a, b, _)| (a, b)).collect::<Vec<_>>();
        let at = |m: Mark| lane.marks.iter().filter(|x| x.2 == m).map(|&(a, _, _)| a).collect::<Vec<_>>();
        let spans = [(of(Mark::Wait), Mark::Wait), (of(Mark::Work), Mark::Work)];
        let moments = [(at(Mark::Question), Mark::Question), (at(Mark::Message), Mark::Message), (at(Mark::Interrupt), Mark::Interrupt)];
        let id = lane.id.clone();
        let selected = s.browser.selected().map(|r| s.browser.session(r)) == Some(lane.id.as_str());
        out = out.child(
            row()
                .key(format!("lane-{}", lane.id))
                .w_full()
                .h(LANE)
                .radius(3.0)
                .fill(if selected { t.control_off } else { Color::TRANSPARENT })
                .hover(t.hover)
                .role("button")
                .label(lane.label.clone())
                .on_press_tx(move |s: &mut Desk, tx| {
                    if s.browser.open_session(&id)
                        && let Some(row) = s.browser.selected()
                    {
                        tx.scroll_into_view(s.browser.session(row).to_string());
                        tx.scroll_to_y(view::PANE, 0.0);
                    }
                })
                .child(label(lane.label.clone(), t))
                .child(track(format!("track-{}", lane.id), &spans, &moments, Some(lane.id.clone()))),
        );
    }
    out
}

fn label(s: String, t: &Theme) -> El {
    row().w(LABEL_WIDTH).h(LANE).pr(8.0).center_y().child(text(s).font(LABEL).color(t.text).single_line().truncate().min_w(0.0))
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

/// The line about the time under the pointer: `14:32 · ecad11 Инфографика прерываний · agent
/// working`, the session's label, and what its lane shows there, when it shows something.
pub fn pointer_text(lanes: &StripLanes, pointer: &Pointer, width: f32) -> String {
    let ts = ts_at(pointer.x, lanes.begin, width);
    let slack = (2.0 * DAY_MS / width.max(1.0) as f64) as i64;
    let (name, index) = match &pointer.lane {
        None => ("you".to_string(), None),
        Some(id) => match lanes.lanes.iter().position(|l| &l.id == id) {
            Some(i) => (lanes.lanes[i].label.clone(), Some(i)),
            None => return String::new(),
        },
    };
    let mark = if index.is_none() && pointer.lane.is_some() { None } else { lanes.at(index, ts, slack) };
    let mut out = format!("{} · {}", local_time(ts, "%H:%M"), name.trim_end());
    if let Some(m) = mark {
        out.push_str(" · ");
        out.push_str(m.name());
    }
    out
}

fn pointer_line(s: &Desk, lanes: &StripLanes, width: f32, t: &Theme) -> El {
    let line = s.pointer.as_ref().map(|p| pointer_text(lanes, p, width)).unwrap_or_default();
    row()
        .key("pointer")
        .w_full()
        .h(LANE)
        .child(el().w(LABEL_WIDTH))
        .child(text(line).key("pointer-text").font(view::MONO).color(t.text_secondary).single_line())
}

fn legend(t: &Theme, dark: bool) -> El {
    let mut line = row().key("legend").w_full().h(LANE).gap(16.0).center_y().child(el().w(LABEL_WIDTH - 16.0));
    for m in [Mark::Work, Mark::Wait, Mark::Message, Mark::Question, Mark::Interrupt, Mark::Present] {
        let swatch = match m {
            Mark::Work | Mark::Wait | Mark::Present => el().w(14.0).h(BAR).radius(2.0),
            _ => el().w(MOMENT).h(LANE - 2.0).radius(1.5),
        };
        line = line.child(
            row().gap(6.0).center_y().child(swatch.fill(mark_color(m, dark, t))).child(text(m.name()).font(LABEL).color(t.text_secondary).single_line()),
        );
    }
    line
}

#[cfg(test)]
mod tests {
    use super::*;

    const HOUR: i64 = 3_600_000;

    #[test]
    fn a_short_piece_of_work_is_a_bar_at_its_time_at_least_a_point_wide() {
        // A day from 04:00 on a track of 1152 points, 48 to an hour.
        let begin = 0;
        let (a, b) = (14 * HOUR + 32 * 60_000 + 10_000, 14 * HOUR + 32 * 60_000 + 50_000);
        let (x, w) = bar(a - 4 * HOUR + begin, b - 4 * HOUR + begin, begin, 1152.0);
        assert!((x - (10.0 * 48.0 + 32.0 * 0.8 + 10.0 / 75.0)).abs() < 0.01, "at 14:32:10, 10 h 32 m into the day: {x}");
        assert_eq!(w, 1.0, "40 seconds are half a point, drawn as one");
        let (_, w) = bar(0, 2 * HOUR, 0, 1152.0);
        assert_eq!(w, 96.0, "two hours are 96 points");
        assert_eq!(ts_at(x_of(5 * HOUR, 0, 1152.0), 0, 1152.0), 5 * HOUR, "the pointer reads the time back");
    }

    #[test]
    fn spans_closer_than_a_point_merge() {
        assert_eq!(merged(vec![(10, 20), (0, 5), (24, 30), (100, 110)], 5), vec![(0, 5), (10, 30), (100, 110)]);
        assert_eq!(merged(vec![], 5), vec![]);
    }

    fn lanes() -> StripLanes {
        use tokprof_core::rhythm::StripLane;
        StripLanes {
            begin: 0,
            end: 24 * HOUR,
            hours: (0..24).map(|k| (k * HOUR, (k + 4) % 24)).collect(),
            you: vec![(10 * HOUR, 11 * HOUR)],
            lanes: vec![StripLane {
                id: "ecad11b9".into(),
                label: "ecad11 Инфографика прерываний".into(),
                marks: vec![(10 * HOUR, 10 * HOUR + 30 * 60_000, Mark::Work), (10 * HOUR + 40 * 60_000, 10 * HOUR + 40 * 60_000, Mark::Message)],
            }],
        }
    }

    #[test]
    fn the_pointer_line_names_the_time_the_session_and_what_its_lane_shows() {
        let width = 1152.0;
        let at = |lane: Option<&str>, ts: i64| pointer_text(&lanes(), &Pointer { lane: lane.map(String::from), x: x_of(ts, 0, width) }, width);
        let hhmm = |ts: i64| local_time(ts, "%H:%M");
        let work = 10 * HOUR + 15 * 60_000;
        assert_eq!(at(Some("ecad11b9"), work), format!("{} · ecad11 Инфографика прерываний · agent working", hhmm(work)));
        let message = 10 * HOUR + 40 * 60_000;
        assert_eq!(at(Some("ecad11b9"), message), format!("{} · ecad11 Инфографика прерываний · your message", hhmm(message)));
        assert_eq!(at(Some("ecad11b9"), 2 * HOUR), format!("{} · ecad11 Инфографика прерываний", hhmm(2 * HOUR)), "nothing there");
        assert_eq!(at(None, 10 * HOUR + 30 * 60_000), format!("{} · you · you were there", hhmm(10 * HOUR + 30 * 60_000)));
        assert_eq!(at(Some("gone"), work), "", "a lane of another day");
    }
}
