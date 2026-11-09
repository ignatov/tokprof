//! The Calendar view: the days of `tokprof calendar` as a grid of rounded squares, a column per
//! week, with the metric as pills, the legend, the numbers beside the grid and the punchcard
//! below, in the cards of Reports. The calendar is computed on a worker from the day store (see
//! `tokprof_browse::calendar`); a day under the pointer or the cursor shows all its numbers.

use rune::{Color, Cx, El, Key as RuneKey, KeyEvent, ScrollSpec, Tx, col, el, row, spacer, text};
use tokprof_browse::calendar::{Calendars, Computed};
use tokprof_browse::render_calendar::{amount, day_name, months, naive, weekday_plural};
use tokprof_core::calendar::{Calendar, Day, Metric, level, weekday};
use tokprof_core::fmt::{count, int, usd};
use tokprof_core::rhythm;

use crate::overlay::{PILL, R_GROUP};
use crate::pal::{self, Pal};
use crate::reports::{Mode, show};
use crate::{Desk, plain};

/// The key of the calendar's scroll viewport.
pub const PANE: &str = "calendar";
/// The grid's squares: the room between them, the least and the most a square takes.
const GAP: f32 = 4.0;
const MIN_SQUARE: f32 = 11.0;
const MAX_SQUARE: f32 = 44.0;
/// The room for the weekday labels left of the grid and the month labels above it.
const LABEL_W: f32 = 34.0;
const MONTH_H: f32 = 20.0;
/// The numbers beside the grid, and the least room the grid keeps for them to go beside it.
const NUMBERS_W: f32 = 330.0;
const TIP_W: f32 = 340.0;
/// The punchcard's squares.
const PUNCH: f32 = 22.0;
/// How far into the accent each level is, from the card's background.
const RAMP: [f32; 4] = [0.3, 0.55, 0.8, 1.0];
const WEEKDAYS: [&str; 7] = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];

/// The view's state: the metric, the cursor, the day under the pointer, and the calendar.
pub struct State {
    pub metric: Metric,
    /// The cursor's date; none for the last day, with no tooltip until a key moves it.
    pub cursor: Option<i64>,
    pub hover: Option<i64>,
    pub data: Calendars,
    /// The window filters by client, which the calendar cannot.
    pub client_ignored: bool,
    /// The window's filters, for the month in review.
    pub select: tokprof_core::query::Query,
    /// The worker's progress as the last poll saw it.
    seen: Option<(usize, usize)>,
}

impl State {
    pub fn new(data: Calendars) -> State {
        State { metric: Metric::Spend, cursor: None, hover: None, data, client_ignored: false, select: tokprof_core::query::Query::default(), seen: None }
    }

    /// The calendar, once computed.
    pub fn calendar(&self) -> Option<&Calendar> {
        self.data.get().and_then(|c| c.calendar.as_deref().ok())
    }
}

/// Takes in what the worker sent while Calendar is shown, asks for the calendar when it is
/// missing or out of date, and wakes the window when it is due. Returns whether the view changed.
pub fn poll(s: &mut Desk, tx: &mut Tx) -> bool {
    if s.reports.mode != Mode::Calendar {
        return false;
    }
    let changes = s.browser.changes();
    let arrived = s.calendar.data.tick(changes);
    if let Some(at) = s.calendar.data.next_due(changes) {
        tx.wake_after(at.saturating_duration_since(std::time::Instant::now()).max(std::time::Duration::from_millis(1)));
    }
    let progress = s.calendar.data.progress();
    let moved = progress != s.calendar.seen;
    s.calendar.seen = progress;
    arrived || moved
}

/// Picks a metric; the calendar is not computed again.
pub fn pick_metric(s: &mut Desk, m: Metric) {
    s.calendar.metric = m;
}

/// The metric after the one shown, or before it.
fn next_metric(m: Metric, back: bool) -> Metric {
    let n = Metric::ALL.len();
    let at = Metric::ALL.iter().position(|&x| x == m).unwrap_or(0);
    Metric::ALL[if back { (at + n - 1) % n } else { (at + 1) % n }]
}

/// The cursor's date, among the days shown.
pub fn cursor_date(s: &Desk) -> Option<i64> {
    let c = s.calendar.calendar()?;
    Some(s.calendar.cursor.unwrap_or(c.last).clamp(c.shown_from.min(c.last), c.last))
}

/// The keys of Calendar: the cursor, the metric, ⏎ and esc; every other key does nothing.
pub fn on_key(s: &mut Desk, e: &KeyEvent, tx: &mut Tx) -> bool {
    if !plain(e) {
        return false;
    }
    let text = match &e.key {
        RuneKey::Text(t) => Some(t.as_str()),
        _ => None,
    };
    match (&e.key, text) {
        (RuneKey::Escape, _) => show(s, Mode::Day),
        (_, Some("m")) => pick_metric(s, next_metric(s.calendar.metric, false)),
        (_, Some("M")) => pick_metric(s, next_metric(s.calendar.metric, true)),
        (_, Some("r")) => crate::month::open(s, None),
        (RuneKey::Enter, _) => {
            if let Some(date) = cursor_date(s) {
                crate::reports::open_day(s, date, tx);
            }
        }
        _ => {
            let step: Option<i64> = match (&e.key, text) {
                (RuneKey::Up, _) | (_, Some("k")) => Some(-1),
                (RuneKey::Down, _) | (_, Some("j")) => Some(1),
                (RuneKey::Left, _) | (_, Some("h")) => Some(-7),
                (RuneKey::Right, _) | (_, Some("l")) => Some(7),
                (RuneKey::Home, _) => Some(i64::MIN / 2),
                (RuneKey::End, _) => Some(i64::MAX / 2),
                _ => None,
            };
            if let (Some(step), Some(at), Some(c)) = (step, cursor_date(s), s.calendar.calendar()) {
                let to = at.saturating_add(step).clamp(c.shown_from.min(c.last), c.last);
                s.calendar.cursor = Some(to);
                s.calendar.hover = None;
            }
        }
    }
    true
}

/// The calendar under the bar.
pub fn view(s: &Desk, cx: &Cx<Desk>, p: &Pal) -> El {
    let c = &s.calendar;
    let identity = c.data.get().map(|x| (x.arrived, x.calendar.as_ref().map(|o| std::sync::Arc::as_ptr(o) as usize).ok()));
    let width = s.window.0.round() as i64;
    let version = crate::view::hash_of((c.metric, c.cursor, c.hover, identity, c.data.reading(), c.seen, &s.theme.get().id, cx.system_dark(), width));
    col().key(PANE).w_full().grow(1.0).min_h(0.0).scroll(ScrollSpec { stick_bottom: false }).child(rune::memo("calendar-body", version, || body(s, p)))
}

fn body(s: &Desk, p: &Pal) -> El {
    let c = &s.calendar;
    let mut cards = col().w_full().px(24.0).pt(18.0).pb(30.0).gap(20.0).child(head(s, p));
    match c.data.get() {
        None => cards = cards.child(filling(s, p)),
        Some(Computed { calendar: Err(e), .. }) => cards = cards.child(note(format!("error: {e}"), p)),
        Some(Computed { calendar: Ok(cal), .. }) if cal.empty => cards = cards.child(note("No requests and no messages in this time.".into(), p)),
        Some(Computed { calendar: Ok(cal), .. }) => {
            cards = cards.child(crate::reports::card(None, "Days", Some("a click opens the day".into()), grid_card(s, cal, p), &s.reports, p));
            cards = cards.child(crate::reports::card(None, "By weekday and hour", Some("local time".into()), punchcard(cal, c.metric, p), &s.reports, p));
        }
    }
    cards
}

/// The head: the metric's pills, and what the store read or is reading again.
fn head(s: &Desk, p: &Pal) -> El {
    let c = &s.calendar;
    let pills = row().key("cal-metrics").gap(6.0).center_y().children(Metric::ALL.iter().map(|&m| {
        let chosen = m == c.metric;
        row()
            .key(format!("metric-{}", m.name()))
            .h(28.0)
            .px(14.0)
            .center_y()
            .radius(PILL)
            .fill(if chosen { p.accent } else { p.bar })
            .border(1.0, if chosen { p.accent } else { p.row_line })
            .hover(if chosen { p.accent.alpha(0.88) } else { p.hover })
            .role("radio")
            .checked(chosen)
            .label(m.title())
            .on_press(move |s: &mut Desk| pick_metric(s, m))
            .child(text(m.title()).font(pal::STRONG).color(if chosen { p.on_accent } else { p.ink2 }).single_line())
    }));
    let review = row()
        .key("month-review")
        .h(28.0)
        .px(14.0)
        .center_y()
        .radius(PILL)
        .fill(p.bar)
        .border(1.0, p.row_line)
        .hover(p.hover)
        .role("button")
        .label("Month in review")
        .on_press(|s: &mut Desk| crate::month::open(s, None))
        .child(text("Month in review").font(pal::STRONG).color(p.ink).single_line());
    let mut head = row().w_full().gap(14.0).center_y().child(pills).child(spacer().grow(1.0));
    if c.data.get().is_some_and(|x| x.store.is_some()) {
        head = head.child(review);
    }
    let aside = match c.data.get() {
        _ if c.data.reading() && c.data.get().is_some() => "updating…".to_string(),
        Some(x) if x.calendar.is_ok() => {
            let gone = if x.gone > 0 { format!(" · {} logs gone from disk", int(x.gone as u64)) } else { String::new() };
            format!("from the day store in {:.1} s{gone}", x.took.as_secs_f64())
        }
        _ => String::new(),
    };
    head = head.child(text(aside).key("cal-read").font(pal::SMALL).color(p.ink3).single_line());
    // What went wrong with the store: unreadable, reset, or not written.
    if let Some(w) = c.data.get().and_then(|x| x.warnings.first()) {
        head = head.child(
            text(w.trim_start_matches("warning: ").to_string()).key("cal-warning").font(pal::SMALL).color(p.cold).single_line().truncate().shrink().min_w(0.0),
        );
    }
    if c.client_ignored {
        head = head.child(text("--client is not applied: the store keeps no clients").font(pal::SMALL).color(p.cold).single_line());
    }
    head
}

/// The line while the store is filled, with how far the worker is.
fn filling(s: &Desk, p: &Pal) -> El {
    let line = match s.calendar.seen {
        Some((done, total)) if total > 0 => format!("reading the logs into the store… {} of {} logs", int(done as u64), int(total as u64)),
        _ => "reading the logs into the store…".to_string(),
    };
    let share = s.calendar.seen.filter(|g| g.1 > 0).map_or(0.0, |g| g.0 as f64 / g.1 as f64);
    col()
        .key("cal-filling")
        .w_full()
        .padding(18.0)
        .gap(12.0)
        .radius(R_GROUP)
        .fill(p.bar)
        .child(text(line).key("cal-progress").font(pal::BODY_NUM).color(p.ink2).single_line())
        .child(row().w(320.0).child(crate::overlay::meter(share, p.accent, p, 6.0)))
}

fn note(t: String, p: &Pal) -> El {
    text(t).key("cal-note").w_full().wrap().font(pal::BODY).color(p.ink2).selectable(true)
}

/// The color of a level: the subtle stroke for none, else the accent mixed into the card's
/// background.
pub fn level_color(lvl: usize, p: &Pal) -> Color {
    if lvl == 0 { p.row_line } else { pal::mix_srgb(p.bar, p.accent, RAMP[lvl - 1]) }
}

/// The side of a square for a grid of `weeks` in a card as wide as the window allows, with the
/// numbers beside it or not.
fn layout(window_w: f32, weeks: usize) -> (f32, bool) {
    let inner = window_w - 48.0 - 28.0;
    let fit = |room: f32| ((room - LABEL_W) / weeks as f32 - GAP).clamp(MIN_SQUARE, MAX_SQUARE);
    let beside = fit(inner - NUMBERS_W - 24.0);
    if beside >= 14.0 { (beside, true) } else { (fit(inner), false) }
}

/// The grid, its labels and legend, the numbers, and the tooltip of the day marked.
fn grid_card(s: &Desk, cal: &Calendar, p: &Pal) -> El {
    let m = s.calendar.metric;
    let cuts = cal.cuts(m);
    let weeks = cal.weeks();
    let window_w = if s.window.0 > 0.0 { s.window.0 } else { crate::view::WINDOW.0 };
    let (sq, beside) = layout(window_w, weeks);
    let step = sq + GAP;
    let grid_w = LABEL_W + weeks as f32 * step;
    let grid_h = MONTH_H + 7.0 * step;
    let cursor = cursor_date(s);
    let mut grid = el().key("cal-grid").w(grid_w).h(grid_h);
    for (at, name) in months(cal, 1) {
        grid = grid.child(text(name).font(pal::HOURS).color(p.ink3).single_line().absolute(LABEL_W + at as f32 * step, 2.0));
    }
    for wd in [0, 2, 4] {
        grid = grid.child(text(WEEKDAYS[wd]).font(pal::HOURS).color(p.ink3).single_line().absolute(0.0, MONTH_H + wd as f32 * step + (sq - 12.0) / 2.0));
    }
    for w in 0..weeks {
        for wd in 0..7 {
            let date = cal.first_monday + 7 * w as i64 + wd as i64;
            if date > cal.last {
                break;
            }
            let d = cal.day(date);
            let lvl = if date < cal.shown_from { 0 } else { d.map_or(0, |d| level(d.amounts.value(m), cuts)) };
            let mut square = el()
                .key(format!("cal-day-{date}"))
                .absolute(LABEL_W + w as f32 * step, MONTH_H + wd as f32 * step)
                .w(sq)
                .h(sq)
                .radius((sq / 5.0).clamp(2.0, 8.0))
                .fill(level_color(lvl, p))
                .role("button")
                .label(day_name(date))
                .on_hover_change(move |s: &mut Desk, inside, _| {
                    if inside {
                        s.calendar.hover = Some(date);
                    } else if s.calendar.hover == Some(date) {
                        s.calendar.hover = None;
                    }
                })
                .on_press_tx(move |s: &mut Desk, tx| crate::reports::open_day(s, date, tx));
            if cursor == Some(date) && s.calendar.cursor.is_some() {
                square = square.outline(2.0, 1.5, p.accent);
            } else if date == cal.last {
                square = square.border(1.5, p.ink);
            }
            grid = grid.child(square);
        }
    }
    let left = col().gap(10.0).child(grid).child(legend(cuts, m, p, sq.min(16.0)));
    let numbers = numbers(cal, m, p);
    let content = if beside { row().w_full().gap(24.0).items_start().child(left).child(numbers) } else { col().w_full().gap(18.0).child(left).child(numbers) };
    // The day marked, under the pointer or under a cursor a key moved, gets its tooltip over
    // everything in the card; the grid starts at the card's corner, so its places hold here.
    let mut card = el().w_full().child(content);
    let marked = s.calendar.hover.or(s.calendar.cursor.and(cursor));
    if let Some(date) = marked
        && let Some(d) = cal.day(date)
    {
        let w = ((date - cal.first_monday) / 7) as f32;
        let wd = weekday(date) as f32;
        let right = LABEL_W + (w + 1.0) * step + 6.0;
        let x = if right + TIP_W <= window_w - 100.0 { right } else { (LABEL_W + w * step - TIP_W - 6.0).max(0.0) };
        card = card.child(tooltip(d, p).absolute(x, (MONTH_H + wd * step - 8.0).max(0.0)));
    }
    card
}

/// `less`, the five squares and `more`, with the cuts.
fn legend(cuts: Option<[f64; 3]>, m: Metric, p: &Pal, sq: f32) -> El {
    let square = |lvl: usize| el().w(sq).h(sq).radius(3.0).fill(level_color(lvl, p));
    let scale = row()
        .gap(4.0)
        .center_y()
        .child(text("less").font(pal::SMALL).color(p.ink3).single_line().pr(4.0))
        .children((0..5).map(square))
        .child(text("more").font(pal::SMALL).color(p.ink3).single_line().pl(4.0));
    let mut l = col().key("cal-legend").pl(LABEL_W).gap(8.0).child(scale);
    // The cuts go on a line of their own, so that a narrow grid keeps them under it.
    if let Some(c) = cuts {
        let cut = |lvl: usize, t: String| row().gap(5.0).center_y().child(square(lvl)).child(text(t).font(pal::SMALL_NUM).color(p.ink2).single_line());
        l = l.child(
            row()
                .key("cal-cuts")
                .gap(12.0)
                .center_y()
                .child(cut(1, format!("up to {}", amount(c[0], m.unit()))))
                .child(cut(2, format!("up to {}", amount(c[1], m.unit()))))
                .child(cut(3, format!("up to {}", amount(c[2], m.unit()))))
                .child(cut(4, "more".into())),
        );
    }
    l
}

/// The numbers of the CLI: total, active days, streaks, the busiest day and weekday, and the
/// days that come only from the store.
fn numbers(cal: &Calendar, m: Metric, p: &Pal) -> El {
    let unit = m.unit();
    let shown = cal.shown();
    let line = |key: &'static str, label: &str, value: String, sub: String| {
        col()
            .key(key)
            .w_full()
            .gap(2.0)
            .child(text(label.to_string()).font(pal::SMALL_STRONG).color(p.ink3).single_line())
            .child(text(value).key("value").font(pal::DAY).color(p.ink).single_line())
            .child(text(sub).key("sub").w_full().wrap().font(pal::SMALL).color(p.ink2))
    };
    let span = match (shown.first(), shown.last()) {
        (Some(a), Some(b)) => {
            format!("{} – {} {}, {}", day_name(a.date), day_name(b.date), naive(b.date).format("%Y"), count(shown.len() as u64, "day", "days"))
        }
        _ => String::new(),
    };
    let (active, all) = cal.active();
    let (current, longest) = cal.streaks();
    let days = |n: i64| count(n as u64, "day", "days");
    let busiest = cal.busiest(m);
    let weekday = cal.busiest_weekday(m);
    let kept = cal.only_kept();
    col()
        .key("cal-numbers")
        .w(NUMBERS_W)
        .gap(14.0)
        .child(line("cal-total", "Total", amount(cal.total(m), unit), span))
        .child(line("cal-active", "Active days", format!("{active} of {all}"), "days with a request".into()))
        .child(line(
            "cal-streaks",
            "Streaks",
            current.map_or("no current streak".to_string(), |s| format!("{} now", days(s.days()))),
            longest.map_or(String::new(), |s| format!("longest {} ({} – {})", days(s.days()), day_name(s.from), day_name(s.to))),
        ))
        .child(line(
            "cal-busiest",
            "Busiest day",
            busiest.map_or("none".to_string(), |d| day_name(d.date)),
            busiest.map_or(String::new(), |d| amount(d.amounts.value(m), unit)),
        ))
        .child(line(
            "cal-weekday",
            "Busiest weekday",
            weekday.map_or("none".to_string(), |(w, _)| weekday_plural(w).to_string()),
            weekday.map_or(String::new(), |(_, v)| amount(v, unit)),
        ))
        .child(line(
            "cal-kept",
            "Kept by the store",
            if kept == 0 { "no day yet".to_string() } else { days(kept as i64) },
            if kept == 0 {
                "days stay here after the agents delete their logs".to_string()
            } else {
                "come only from the store: the agents deleted their logs".to_string()
            },
        ))
        .children((cal.untimed.0 > 0).then(|| {
            line(
                "cal-untimed",
                "Without a time",
                format!("{} {}", int(cal.untimed.0), if cal.untimed.0 == 1 { "request" } else { "requests" }),
                format!("in no day: their logs give no time ({})", usd(cal.untimed.1)),
            )
        }))
}

/// Every metric of a day, in a small card.
pub fn tooltip_lines(d: &Day) -> Vec<String> {
    let a = &d.amounts;
    let mut lines = vec![
        naive(d.date).format("%a %-d %b %Y").to_string(),
        format!("{} spent, {} of it avoidable", usd(a.usd), usd(a.avoidable_usd)),
        format!("{} in {}", num(a.requests, "request", "requests"), num(d.sessions as u64, "session", "sessions")),
        if a.agent_minutes > 0 { format!("agents worked {}", rhythm::duration(a.agent_minutes as i64 * 60_000)) } else { "no agent worked".to_string() },
        if a.sittings > 0 {
            format!(
                "{} at the keyboard in {}, {}",
                rhythm::duration(a.keyboard_ms),
                count(a.sittings as u64, "sitting", "sittings"),
                count(a.messages, "message", "messages")
            )
        } else {
            "you did not sit at the agents".to_string()
        },
        format!(
            "{}, {}, {}",
            counted(a.commits, "commit", "commits"),
            counted(a.pushes, "push", "pushes"),
            counted(a.pull_requests, "pull request", "pull requests")
        ),
    ];
    if d.only_kept() {
        lines.push("only from the store: its logs are gone".into());
    } else if d.gone {
        lines.push("partly from the store: some of its logs are gone".into());
    }
    lines
}

/// `1,820 requests`: a count with its thousands apart and its word.
fn num(n: u64, one: &str, many: &str) -> String {
    format!("{} {}", int(n), if n == 1 { one } else { many })
}

/// `3 commits`, `1 push`, `no pull request`.
fn counted(n: u64, one: &str, many: &str) -> String {
    if n == 0 { format!("no {one}") } else { num(n, one, many) }
}

fn tooltip(d: &Day, p: &Pal) -> El {
    let lines = tooltip_lines(d);
    let mut card =
        col().key("cal-tip").w(TIP_W).padding(12.0).gap(4.0).radius(12.0).fill(p.panel).border(1.0, p.row_line).shadow(14.0, 30.0, Color::BLACK.alpha(0.22));
    for (i, l) in lines.into_iter().enumerate() {
        let font = if i == 0 { pal::STRONG } else { pal::SMALL_NUM };
        let color = if i == 0 {
            p.ink
        } else if l.starts_with("only from") || l.starts_with("partly from") {
            p.accent
        } else {
            p.ink2
        };
        card = card.child(text(l).key(format!("tip-{i}")).w_full().wrap().font(font).color(color));
    }
    card
}

/// The weekday × hour punchcard, with its legend.
fn punchcard(cal: &Calendar, m: Metric, p: &Pal) -> El {
    let cuts = cal.punch_cuts(m);
    let step = PUNCH + 3.0;
    let mut grid = el().key("cal-punch").w(LABEL_W + 24.0 * step).h(MONTH_H + 7.0 * step);
    for h in (0..24).step_by(3) {
        grid = grid.child(text(format!("{h:02}")).font(pal::HOURS).color(p.ink3).single_line().absolute(LABEL_W + h as f32 * step, 2.0));
    }
    for (wd, name) in WEEKDAYS.iter().enumerate() {
        grid = grid.child(text(*name).font(pal::HOURS).color(p.ink3).single_line().absolute(0.0, MONTH_H + wd as f32 * step + (PUNCH - 12.0) / 2.0));
        for h in 0..24 {
            let lvl = level(cal.punch[wd][h].value(m), cuts);
            grid = grid.child(
                el().key(format!("punch-{wd}-{h}"))
                    .absolute(LABEL_W + h as f32 * step, MONTH_H + wd as f32 * step)
                    .w(PUNCH)
                    .h(PUNCH)
                    .radius(4.0)
                    .fill(level_color(lvl, p))
                    .label(format!("{} {h:02}:00, {}", WEEKDAYS[wd], amount(cal.punch[wd][h].value(m), m.unit()))),
            );
        }
    }
    col().gap(10.0).child(grid).child(legend(cuts, m, p, 14.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metrics_cycle_both_ways() {
        assert_eq!(next_metric(Metric::Spend, false), Metric::Agent);
        assert_eq!(next_metric(Metric::Avoidable, false), Metric::Spend);
        assert_eq!(next_metric(Metric::Spend, true), Metric::Avoidable);
    }

    #[test]
    fn squares_grow_for_few_weeks_and_the_numbers_go_under_a_wide_grid() {
        let (few, beside) = layout(1400.0, 14);
        assert_eq!((few, beside), (MAX_SQUARE, true));
        // A year in a window of 1400: the numbers go under it, and its squares stay large.
        let (year, beside) = layout(1400.0, 53);
        assert!(!beside && (18.0..=22.0).contains(&year), "{year}");
        assert!(layout(1800.0, 53).1, "a wider window has room for both");
        let (narrow, beside) = layout(960.0, 53);
        assert!(!beside && narrow >= MIN_SQUARE, "{narrow}");
    }

    #[test]
    fn a_days_tooltip_names_every_metric() {
        let d = Day {
            date: 20_762,
            amounts: tokprof_core::calendar::Amounts {
                usd: 214.0,
                avoidable_usd: 31.0,
                requests: 1820,
                agent_minutes: 372,
                keyboard_ms: 9_000_000,
                sittings: 14,
                messages: 47,
                commits: 3,
                pushes: 1,
                pull_requests: 0,
                ..Default::default()
            },
            sessions: 8,
            live: false,
            gone: true,
        };
        assert_eq!(
            tooltip_lines(&d),
            [
                "Thu 5 Nov 2026",
                "$214 spent, $31.00 of it avoidable",
                "1,820 requests in 8 sessions",
                "agents worked 6.2h",
                "2.5h at the keyboard in 14 sittings, 47 messages",
                "3 commits, 1 push, no pull request",
                "only from the store: its logs are gone",
            ]
        );
    }
}
