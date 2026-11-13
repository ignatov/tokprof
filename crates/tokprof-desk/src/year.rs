//! Year: one calendar year in the template of Month and Quarter (desk-window, Year view). Under
//! the key numbers: the grid of the year's days with its metric pills, legend, numbers and month
//! bars; the punchcard beside the rhythm; by project, agents and models and what to change; the
//! records beside Year in review. All of it comes from the day store's sums of `Period::Year`
//! (see `tokprof_browse::period`): Year reads no log. The card of Year in review is drawn in
//! `month`, as the month's is, and computed here on a worker.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use rune::{Color, El, Key as RuneKey, KeyEvent, Tx, col, el, row, spacer, text};
use tokprof_browse::render_calendar::{amount, cut_amount, day_name, days_with, naive, weekday_plural, with_unit};
use tokprof_core::analysis::cache::Cause;
use tokprof_core::calendar::{Day, Metric, level};
use tokprof_core::fmt::{count, int, local_time, usd};
use tokprof_core::period::{Period, PeriodSums};
use tokprof_core::pool::Row as PoolRow;
use tokprof_core::rhythm;
use tokprof_core::savings::{Changes, SessionCost};
use tokprof_core::source::AgentId;
use tokprof_core::year::{MonthKind, Readiness, Year, months_of, weekdays};

use crate::overlay::{PILL, R_MARK, R_TILE};
use crate::pal::{self, Pal};
use crate::reports::{self, Hover, Mode, Part, Record, Target, card, cards_row, cursor_row, offset, usd_whole, widths};
use crate::{Desk, plain};

/// The grid's squares: the room between them, the least and the most a square takes.
const GAP: f32 = 4.0;
const MIN_SQUARE: f32 = 11.0;
const MAX_SQUARE: f32 = 44.0;
/// The room for the weekday labels left of the grid and the month labels above it.
const LABEL_W: f32 = 34.0;
const MONTH_H: f32 = 20.0;
/// The month bars under the grid: the room for the amount over a bar, the tallest bar, and the
/// lines under the baseline.
const BAR_TOP: f32 = 22.0;
const BAR_MAX: f32 = 56.0;
const BAR_UNDER: f32 = 34.0;
/// The head of the grid's card: the pills' row, the legend's, and the line of `--client`.
const PILLS_H: f32 = 28.0;
const LEGEND_H: f32 = 18.0;
const CLIENT_H: f32 = 16.0;
const CARD_GAP: f32 = 10.0;
const TIP_W: f32 = 340.0;
/// The days without logs that a quiet line names, in columns.
const QUIET_COLUMNS: usize = 15;
/// How far into the accent each level is, from the card's background.
const RAMP: [f32; 4] = [0.3, 0.55, 0.8, 1.0];
const WEEKDAYS: [&str; 7] = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
/// The models Agents and models lists before the rest.
const MODELS: usize = 6;

/// Year's own state: the metric, Year in review over the window, and the years computed for
/// their cards.
pub struct State {
    pub metric: Metric,
    pub card: Option<Card>,
    /// The frame shows only the card, for the image.
    pub alone: bool,
    /// The year `--year` names, for a frame without a window.
    pub wanted: Option<i32>,
    pub saving: Option<crate::month::Saved>,
    /// The years whose cards are computed, each with the store it came from, and those being
    /// computed.
    pub years: HashMap<i32, (usize, Arc<Year>)>,
    pub drawing: HashMap<i32, (usize, Slot)>,
}

impl Default for State {
    fn default() -> State {
        State { metric: Metric::Spend, card: None, alone: false, wanted: None, saving: None, years: HashMap::new(), drawing: HashMap::new() }
    }
}

/// Where a worker puts a year it computed.
pub type Slot = Arc<Mutex<Option<Year>>>;

/// Year in review over the window: the year it shows, the year ← or → asked for until it is
/// computed, and what the last save said.
pub struct Card {
    pub year: i32,
    pub next: Option<i32>,
    pub status: Option<String>,
}

// ---------------------------------------------------------------------------------------------
// The data.

/// The year's what to change, from the store's cards, as `Changes` so that the rows open their
/// sessions as Month's do.
pub fn changes_of(sums: &PeriodSums) -> Changes {
    let mut c = Changes::default();
    let Some(cards) = &sums.cards else { return c };
    let spend = sums.key.usd;
    for x in &cards.causes {
        c.saved.push(PoolRow { name: x.name.clone(), usd: x.usd, share: if spend > 0.0 { x.usd / spend } else { 0.0 }, ..PoolRow::default() });
        let sessions =
            x.sessions.iter().take(crate::changes::SESSIONS).map(|(a, m, v)| SessionCost { agent: AgentId::unknown(a), session: m.clone(), usd: *v });
        c.sessions.insert(x.name.clone(), sessions.collect());
        if let Some(cause) = Cause::ALL.into_iter().find(|k| k.name() == x.name) {
            let hint = tokprof_core::advice::miss_hint(cause);
            let mut chars = hint.chars();
            let hint = chars.next().map_or(String::new(), |f| f.to_uppercase().collect::<String>() + chars.as_str());
            c.hints.insert(x.name.clone(), format!("{hint}."));
        }
    }
    c
}

/// Keeps Year's what to change in step with the sums shown.
pub fn keep_changes(s: &mut Desk) {
    let Some(sums) = s.reports.sums().cloned() else { return };
    let at = Arc::as_ptr(&sums) as usize;
    if s.reports.year_changes.as_ref().is_none_or(|(a, _)| *a != at) {
        s.reports.year_changes = Some((at, changes_of(&sums)));
    }
}

/// The calendar's day the year was counted on: today while it runs, else the date of now.
fn today(sums: &PeriodSums, now: i64) -> i64 {
    sums.today.unwrap_or_else(|| rhythm::date_of(now, offset()))
}

/// Whether the year's card can be saved.
pub fn readiness(sums: &PeriodSums, now: i64) -> Readiness {
    let Period::Year(y) = sums.period else { return Readiness::default() };
    Readiness::of(y, sums.first_data, today(sums, now), sums.complete)
}

/// The year's extent in the bar: `logs since 6 Aug · 99 days` in the year the logs begin, `316 of
/// 365 days` in a later year in progress, nothing for a past year.
pub fn extent(sums: Option<&PeriodSums>, year: i32, now: i64) -> String {
    let Some(x) = sums.filter(|x| !x.complete) else { return String::new() };
    let r = readiness(x, now);
    let p = Period::Year(year);
    match r.logs_from {
        Some(f) => format!("logs since {} · {}", naive(f).format("%-d %b"), count(r.logs_days as u64, "day", "days")),
        None => format!("{} of {} days", today(x, now) - p.first_date() + 1, p.days()),
    }
}

/// The records of the year, as tiles: the biggest day and week, the longest streak, the most
/// commits in a day and the most sessions at once; no busiest hour, which the rhythm shows.
pub fn records(sums: &PeriodSums) -> Vec<Record> {
    let r = &sums.records;
    let rec = |label, value: String, sub: String, target| Record { label, value, sub, target };
    let mut out = Vec::new();
    if let Some((d, v)) = r.biggest_day {
        out.push(rec("Biggest day", usd_whole(v), reports::day_name(d), Some(Target::Day(d))));
    }
    if let Some(w) = r.biggest_week.and_then(|(m, _)| sums.weeks.iter().find(|w| w.monday == m)) {
        out.push(rec("Biggest week", usd_whole(w.amounts.usd), tokprof_core::week::dates(w.first, w.last, false), Some(Target::Week(w.monday))));
    }
    if let Some(t) = r.streak_text() {
        let (a, b) = t.split_once(", ").map_or((t.clone(), String::new()), |(a, b)| (a.to_string(), b.to_string()));
        out.push(rec("Longest streak", a, b, None));
    }
    if let Some((d, n)) = r.most_commits {
        out.push(rec("Most commits in a day", int(n), reports::day_name(d), Some(Target::Day(d))));
    }
    if let Some((n, ts)) = r.most_at_once {
        let date = rhythm::date_of(ts, offset());
        out.push(rec("Most at once", count(u64::from(n), "session", "sessions"), local_time(ts, "%a %-d %b %H:%M"), Some(Target::Day(date))));
    }
    out
}

/// What a date of the grid is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DateKind {
    /// Before the logs begin: a dashed outline.
    NoLogs,
    /// After the last day: a dot.
    ToCome,
    Day,
}

pub fn date_kind(sums: &PeriodSums, date: i64) -> DateKind {
    if sums.first_data.is_none_or(|f| date < f) {
        DateKind::NoLogs
    } else if sums.today.is_some_and(|t| date > t) {
        DateKind::ToCome
    } else {
        DateKind::Day
    }
}

/// The grid's columns: a Monday per week, from the week of 1 January to the week of 31 December.
pub fn columns(sums: &PeriodSums) -> Vec<i64> {
    sums.period.mondays()
}

/// The side of a square in a card whose content is `inner` points wide, for `weeks` columns.
pub fn square(inner: f32, weeks: usize) -> f32 {
    ((inner - LABEL_W) / weeks as f32 - GAP).clamp(MIN_SQUARE, MAX_SQUARE)
}

/// A metric's amount in each month, January first, as `tokprof month` counts it; none for a
/// month without logs or not begun.
pub fn month_values(sums: &PeriodSums, m: Metric, now: i64) -> Vec<Option<f64>> {
    months_of(sums, today(sums, now)).iter().map(|r| r.month.map(|x| x.total.value(m))).collect()
}

/// The days the numbers count: from the day the logs begin to the last day.
fn counted_days(sums: &PeriodSums) -> Vec<&Day> {
    let c = &sums.cells;
    let from = sums.first_data.map_or(i64::MAX, |f| f.max(sums.period.first_date()));
    (from..=c.last).filter_map(|d| c.day(d)).collect()
}

/// The quiet line inside the days without logs: `Logs since 6 Aug. January to July have no logs,
/// so these days are unknown, not idle.`
pub fn quiet_line(sums: &PeriodSums) -> String {
    let Period::Year(y) = sums.period else { return String::new() };
    let Some(f) = sums.first_data else { return "No logs yet, so these days are unknown, not idle.".into() };
    if f > sums.period.last_date() {
        return format!("No logs in {y}: the logs begin {}, so these days are unknown, not idle.", naive(f).format("%-d %b %Y"));
    }
    let month = tokprof_core::month::Ym::of_date(f).month;
    let name = |m: u32| tokprof_core::month::Ym { year: y, month: m }.month_name();
    let before = match month {
        1 => String::new(),
        2 => " January has no logs, so these days are unknown, not idle.".to_string(),
        m => format!(" January to {} have no logs, so these days are unknown, not idle.", name(m - 1)),
    };
    format!("Logs since {}.{before}", naive(f).format("%-d %b"))
}

/// The label under the months without logs: `no logs from January to 5 August`.
pub fn no_logs_label(sums: &PeriodSums) -> String {
    let Period::Year(y) = sums.period else { return String::new() };
    match sums.first_data.filter(|&f| f <= sums.period.last_date()) {
        Some(f) => format!("no logs from January to {}", naive(f - 1).format("%-d %B")),
        None => format!("no logs in {y}"),
    }
}

// ---------------------------------------------------------------------------------------------
// The keys.

/// Picks a metric; nothing is counted again.
pub fn pick_metric(s: &mut Desk, m: Metric) {
    s.year.metric = m;
}

/// The metric after the one shown, or before it.
pub fn next_metric(m: Metric, back: bool) -> Metric {
    let n = Metric::ALL.len();
    let at = Metric::ALL.iter().position(|&x| x == m).unwrap_or(0);
    Metric::ALL[if back { (at + n - 1) % n } else { (at + 1) % n }]
}

/// The month the months' cursor is on: the one it was moved to, else this month or the last.
fn month_cursor(s: &Desk, sums: &PeriodSums) -> usize {
    let rows = months_of(sums, today(sums, s.reports.now()));
    let last = rows.iter().rposition(|r| r.kind != MonthKind::NotBegun).unwrap_or(0);
    if s.reports.part == Some(Part::Months) { s.reports.row.min(last) } else { last }
}

/// The keys of Year: `m`, ⇧M and `r` anywhere, the grid's day cursor and the months' month cursor
/// while they have the keyboard; the rest as Keys in Month and Quarter.
pub fn on_key(s: &mut Desk, e: &KeyEvent, tx: &mut Tx) -> bool {
    if s.browser.editing() || !plain(e) && e.key != RuneKey::Tab {
        return reports::on_key(s, e, tx);
    }
    let text = match &e.key {
        RuneKey::Text(t) => Some(t.as_str()),
        _ => None,
    };
    match text {
        Some("m") => {
            pick_metric(s, next_metric(s.year.metric, false));
            return true;
        }
        Some("M") => {
            pick_metric(s, next_metric(s.year.metric, true));
            return true;
        }
        Some("r") => {
            open(s, s.reports.shown_year());
            return true;
        }
        _ => {}
    }
    let Some(sums) = s.reports.sums().cloned() else { return reports::on_key(s, e, tx) };
    match s.reports.part {
        Some(Part::Grid) => {
            let dates = crate::charts::openable_dates(&sums);
            let (Some(&first), Some(&last)) = (dates.first(), dates.last()) else { return reports::on_key(s, e, tx) };
            let at = crate::charts::day_cursor(s, &sums).unwrap_or(last);
            let to = match (&e.key, text) {
                (RuneKey::Up, _) | (_, Some("k")) => Some(at - 1),
                (RuneKey::Down, _) | (_, Some("j")) => Some(at + 1),
                (RuneKey::Left, _) | (_, Some("h")) => Some(at - 7),
                (RuneKey::Right, _) | (_, Some("l")) => Some(at + 7),
                (RuneKey::Home, _) => Some(first),
                (RuneKey::End, _) => Some(last),
                _ => None,
            };
            if let Some(to) = to {
                s.reports.day = Some(to.clamp(first, last));
                s.reports.hover = None;
                return true;
            }
            if e.key == RuneKey::Enter {
                reports::open_day(s, at, tx);
                return true;
            }
        }
        Some(Part::Months) => {
            let rows = months_of(&sums, today(&sums, s.reports.now()));
            let last = rows.iter().rposition(|r| r.kind != MonthKind::NotBegun).unwrap_or(0);
            let at = month_cursor(s, &sums);
            let to = match (&e.key, text) {
                (RuneKey::Left, _) | (_, Some("h")) => Some(at.saturating_sub(1)),
                (RuneKey::Right, _) | (_, Some("l")) => Some((at + 1).min(last)),
                (RuneKey::Home, _) => Some(0),
                (RuneKey::End, _) => Some(last),
                _ => None,
            };
            if let Some(to) = to {
                s.reports.row = to;
                s.reports.hover = None;
                return true;
            }
            if e.key == RuneKey::Enter {
                if let Some(r) = rows.get(at).filter(|r| r.kind != MonthKind::NotBegun) {
                    reports::show_period(s, Period::Month(r.ym), tx);
                }
                return true;
            }
            if matches!(e.key, RuneKey::Up | RuneKey::Down) || matches!(text, Some("k" | "j")) {
                return true;
            }
        }
        _ => {}
    }
    let taken = reports::on_key(s, e, tx);
    // The months take the keyboard on this month.
    if e.key == RuneKey::Tab && s.reports.part == Some(Part::Months) {
        let rows = months_of(&sums, today(&sums, s.reports.now()));
        s.reports.row = rows.iter().rposition(|r| r.kind != MonthKind::NotBegun).unwrap_or(0);
    }
    taken
}

// ---------------------------------------------------------------------------------------------
// The pane.

/// What of Year's own state the pane shows, for the pane's memo.
pub fn version(s: &Desk) -> u64 {
    let years: Vec<(i32, usize)> = s.year.years.iter().map(|(y, (_, x))| (*y, Arc::as_ptr(x) as usize)).collect();
    let mut years = years;
    years.sort_unstable();
    crate::view::hash_of((s.year.metric, years, s.year.saving.is_some(), s.reports.year_changes.as_ref().map(|x| x.0)))
}

/// Year's cards under the key numbers, in `w` points; two to a row in a narrow window.
pub fn cards(s: &Desk, sums: &PeriodSums, p: &Pal, w: f32, pair: bool) -> Vec<El> {
    let side = if pair { (w * 0.36).max(300.0) } else { 380.0 };
    vec![
        grid_card(s, sums, p, w),
        row()
            .w_full()
            .gap(reports::GAP)
            .items_stretch()
            .child(col().w(w - side - reports::GAP).child(punchcard(s, sums, p, w - side - reports::GAP)))
            .child(col().w(side).child(rhythm_card(s, sums, p, side))),
        cards_row(vec![reports::projects(s, sums, p), agents(s, sums, p), changes_card(s, sums, p)], if pair { 2 } else { 3 }, w),
        cards_row(vec![reports::records_card(s, sums, p, widths(w, 2)), share_card(s, sums, p, widths(w, 2))], 2, w),
    ]
}

/// Whether any date of the year has logs.
pub fn has_logs(sums: &PeriodSums) -> bool {
    sums.first_data.is_some_and(|f| f <= sums.period.last_date())
}

/// A year without logs: its grid of days without logs, whose quiet line says when they begin.
pub fn without_logs(s: &Desk, sums: &PeriodSums, p: &Pal, w: f32) -> Vec<El> {
    vec![grid_card(s, sums, p, w)]
}

/// The color of a level: the subtle stroke for none, else the metric's color mixed into the
/// card's background.
pub fn level_color(lvl: usize, top: Color, p: &Pal) -> Color {
    if lvl == 0 { p.row_line } else { pal::mix_srgb(p.bar, top, RAMP[lvl - 1]) }
}

/// The color a metric's levels go up to: the cold color for avoidable spend, else the accent.
pub fn metric_color(m: Metric, p: &Pal) -> Color {
    if m == Metric::Avoidable { p.cold } else { p.accent }
}

/// A cell of a level, `w` by `h` with corners of `r`: filled, or in a dark theme, at level none,
/// an empty cell with a hairline, so that the first level always differs from nothing.
pub fn cell(lvl: usize, top: Color, p: &Pal, w: f32, h: f32, r: f32) -> El {
    let e = el().w(w).h(h).radius(r);
    if lvl == 0 && p.dark { e.border(1.0, p.row_line) } else { e.fill(level_color(lvl, top, p)) }
}

/// A day before the logs begin: a dashed outline in the subtle stroke, unlike level none.
pub fn no_logs_cell(p: &Pal, w: f32, r: f32) -> El {
    el().w(w).h(w).radius(r).border(1.0, if p.dark { p.ink3.alpha(0.55) } else { p.ink3.alpha(0.45) }).border_dash(2.5, 2.0)
}

/// The metric's pills, as one segmented group.
fn pills(s: &Desk, p: &Pal) -> El {
    crate::view::segments(
        "year-metrics",
        Metric::ALL
            .iter()
            .map(|&m| crate::view::segment(format!("metric-{}", m.name()), m.title(), m == s.year.metric, p).on_press(move |s: &mut Desk| pick_metric(s, m)))
            .collect(),
        p.panel,
    )
}

/// `less`, the five squares and `more` with the cuts, then `no logs` and `to come`.
fn legend(cuts: Option<[f64; 3]>, m: Metric, p: &Pal) -> El {
    let top = metric_color(m, p);
    let sq = 11.0;
    let more = match cuts {
        Some(c) => format!("more · cuts at {}, {}, {}", cut_amount(c[0], m.unit()), cut_amount(c[1], m.unit()), cut_amount(c[2], m.unit())),
        None => "more".into(),
    };
    row()
        .key("year-legend")
        .h(LEGEND_H)
        .gap(4.0)
        .center_y()
        .child(text("less").font(pal::SMALL).color(p.ink2).single_line().pr(2.0))
        .children((0..5).map(|l| cell(l, top, p, sq, sq, 3.0)))
        .child(text(more).key("year-cuts").font(pal::SMALL_NUM).color(p.ink2).single_line().pl(2.0).pr(12.0))
        .child(no_logs_cell(p, sq, 3.0))
        .child(text("no logs").font(pal::SMALL).color(p.ink2).single_line().pr(10.0))
        .child(row().w(sq).h(sq).center().child(crate::view::dot(p.ink3.alpha(0.7), 3.0)))
        .child(text("to come").font(pal::SMALL).color(p.ink2).single_line())
}

/// The grid of the year with its pills, legend, month bars and numbers, and the tooltip of the
/// day or the month marked over all of it.
fn grid_card(s: &Desk, sums: &PeriodSums, p: &Pal, w: f32) -> El {
    let m = s.year.metric;
    let cal = &sums.cells;
    let cuts = cal.cuts(m);
    let top = metric_color(m, p);
    let inner = w - 28.0;
    let mondays = columns(sums);
    let weeks = mondays.len();
    let first_monday = mondays[0];
    let sq = square(inner, weeks);
    let step = sq + GAP;
    let grid_w = LABEL_W + weeks as f32 * step - GAP;
    let grid_h = MONTH_H + 7.0 * step - GAP;
    let (first, last) = (sums.period.first_date(), sums.period.last_date());
    let col_of = |date: i64| ((date - first_monday) / 7) as usize;
    let x_of = |c: usize| LABEL_W + c as f32 * step;
    let cursor = (s.reports.part == Some(Part::Grid)).then(|| crate::charts::day_cursor(s, sums)).flatten();
    let marked = match s.reports.hover {
        Some(Hover::Cell(d)) => Some(d),
        _ => cursor,
    }
    .filter(|&d| date_kind(sums, d) == DateKind::Day);
    let mut plot = el().key("year-grid").w(grid_w).h(grid_h + BAR_TOP + BAR_MAX + BAR_UNDER);
    for ym in sums.period.months() {
        plot = plot.child(text(ym.month_name()[..3].to_string()).font(pal::HOURS).color(p.ink3).single_line().absolute(x_of(col_of(ym.first_date())), 2.0));
    }
    for wd in [0, 2, 4] {
        plot = plot.child(text(WEEKDAYS[wd]).font(pal::HOURS).color(p.ink3).single_line().absolute(0.0, MONTH_H + wd as f32 * step + (sq - 12.0) / 2.0));
    }
    let r = (sq / 5.0).clamp(2.0, 8.0);
    for (c, &monday) in mondays.iter().enumerate() {
        for wd in 0..7 {
            let date = monday + wd;
            if date < first || date > last {
                continue;
            }
            let (x, y) = (x_of(c), MONTH_H + wd as f32 * step);
            match date_kind(sums, date) {
                DateKind::NoLogs => plot = plot.child(no_logs_cell(p, sq, r).key(format!("year-nologs-{date}")).absolute(x, y)),
                DateKind::ToCome => {
                    plot =
                        plot.child(row().key(format!("year-tocome-{date}")).absolute(x, y).w(sq).h(sq).center().child(crate::view::dot(p.ink3.alpha(0.7), 3.0)))
                }
                DateKind::Day => {
                    let lvl = cal.day(date).map_or(0, |d| level(d.amounts.value(m), cuts));
                    let mut sqr = cell(lvl, top, p, sq, sq, r)
                        .key(format!("year-day-{date}"))
                        .absolute(x, y)
                        .role("button")
                        .label(day_name(date))
                        .on_hover_change(move |s: &mut Desk, inside, _| {
                            if inside {
                                s.reports.hover = Some(Hover::Cell(date));
                            } else if s.reports.hover == Some(Hover::Cell(date)) {
                                s.reports.hover = None;
                            }
                        })
                        .on_press_tx(move |s: &mut Desk, tx| reports::open_day(s, date, tx));
                    if cursor == Some(date) {
                        sqr = sqr.outline(2.0, 1.5, p.accent);
                    }
                    plot = plot.child(sqr);
                    // Today: a dot in its square; rings are for the cursor.
                    if sums.today == Some(date) {
                        plot = plot.child(row().key("year-today").absolute(x, y).w(sq).h(sq).center().child(crate::view::dot(p.ink, 4.0)));
                    }
                }
            }
        }
    }
    // The quiet line in a tinted box inside the days without logs, when they fill 15 columns.
    let empty_cols = match sums.first_data {
        Some(f) if f <= last => col_of(f.max(first)),
        _ => weeks,
    };
    if empty_cols >= QUIET_COLUMNS {
        let line = quiet_line(sums);
        let run = (empty_cols as f32 - 1.0) * step - GAP;
        let want = rune::text::advance(&line, pal::SMALL) + 36.0;
        let (bw, lines) = if want <= run { (want, 1.0f32) } else { (run, 3.0) };
        let bh = (lines * 16.0 + 16.0).max(2.0 * step - GAP);
        let (bx, by) = (LABEL_W + (run - bw) / 2.0 + step / 2.0, MONTH_H + 3.5 * step - bh / 2.0);
        plot = plot.child(
            row()
                .key("year-quiet")
                .absolute(bx, by)
                .w(bw)
                .h(bh)
                .px(16.0)
                .center()
                .radius(10.0)
                .fill(pal::mix_srgb(p.bar, p.panel, 0.85))
                .border(1.0, p.row_line)
                .child(text(line).key("year-quiet-text").w_full().wrap().max_lines(3).align(rune::Align::Center).font(pal::SMALL).color(p.ink2)),
        );
    }
    plot = month_bars(s, sums, plot, p, (grid_h, step, first_monday));
    let mut top = PILLS_H + CARD_GAP + LEGEND_H + CARD_GAP;
    let mut content = col().w_full().gap(CARD_GAP);
    let read = if s.reports.sums.summing(&s.reports.key()) { "updating…" } else { "" };
    content = content.child(
        row()
            .w_full()
            .h(PILLS_H)
            .gap(14.0)
            .center_y()
            .child(pills(s, p))
            .child(spacer().grow(1.0))
            .child(text(read).key("year-read").font(pal::SMALL).color(p.ink3).single_line()),
    );
    content = content.child(legend(cuts, m, p));
    if s.reports.client_ignored {
        content = content
            .child(text("Year counts every client: the day store has no client").key("year-client").h(CLIENT_H).font(pal::SMALL).color(p.ink3).single_line());
        top += CLIENT_H + CARD_GAP;
    }
    if let Some(w) = s.calendar.data.get().and_then(|x| x.warnings.first()) {
        content = content.child(
            text(w.trim_start_matches("warning: ").to_string())
                .key("year-warning")
                .w_full()
                .h(CLIENT_H)
                .single_line()
                .truncate()
                .font(pal::SMALL)
                .color(p.cold),
        );
        top += CLIENT_H + CARD_GAP;
    }
    content = content.child(plot);
    if has_logs(sums) {
        content = content.child(numbers(sums, m, p, inner));
    }
    // The tooltip of the day marked, under the grid at its column, kept inside the card, over
    // the bars and the numbers: the card's last child.
    if let Some(date) = marked
        && let Some(d) = cal.day(date)
    {
        let x = x_of(col_of(date)).min(inner - TIP_W).max(0.0);
        content = content.child(tooltip(d, p).absolute(x, top + grid_h + 6.0));
    }
    card(Some(Part::Grid), "Days of the year", Some("a click opens the day, a month bar opens Month".into()), content, &s.reports, p)
}

/// The bars of the months under the grid, each centered under its month's columns.
fn month_bars(s: &Desk, sums: &PeriodSums, mut plot: El, p: &Pal, (grid_h, step, first_monday): (f32, f32, i64)) -> El {
    let m = s.year.metric;
    let now = s.reports.now();
    let rows = months_of(sums, today(sums, now));
    let values = month_values(sums, m, now);
    let most = values.iter().flatten().copied().fold(0.0, f64::max);
    let base = grid_h + BAR_TOP + BAR_MAX + 6.0;
    let col_of = |date: i64| ((date - first_monday) / 7) as f32;
    let span = |ym: tokprof_core::month::Ym| {
        let (a, b) = (col_of(ym.first_date()), col_of(ym.next().first_date() - 1));
        (LABEL_W + a * step, LABEL_W + (b + 1.0) * step - GAP)
    };
    let top = metric_color(m, p);
    let cursor = (s.reports.part == Some(Part::Months)).then(|| month_cursor(s, sums));
    let hover = match s.reports.hover {
        Some(Hover::Month(i)) => Some(i),
        _ => None,
    };
    plot = plot.child(el().absolute(LABEL_W, base).w(span(rows[11].ym).1 - LABEL_W).h(1.0).fill(p.row_line));
    // One label under the months without logs.
    let empty: Vec<usize> = (0..12).filter(|&i| rows[i].kind == MonthKind::NoLogs).collect();
    if let (Some(&a), Some(&b), true) = (empty.first(), empty.last(), has_logs(sums)) {
        let (x0, x1) = (span(rows[a].ym).0, span(rows[b].ym).1);
        plot = plot.child(
            row()
                .absolute(x0, base - 18.0)
                .w(x1 - x0)
                .justify_center()
                .child(text(no_logs_label(sums)).key("year-months-nologs").font(pal::HOURS).color(p.ink3).single_line().truncate().min_w(0.0)),
        );
    }
    for (i, r) in rows.iter().enumerate() {
        let (x0, x1) = span(r.ym);
        let mw = x1 - x0;
        let cx = (x0 + x1) / 2.0;
        let bw = (mw * 0.55).min(44.0);
        let name = r.ym.month_name()[..3].to_string();
        let ink = if r.kind == MonthKind::NotBegun || r.kind == MonthKind::NoLogs { p.ink3 } else { p.ink2 };
        plot = plot.child(row().absolute(x0, base + 4.0).w(mw).justify_center().child(text(name).font(pal::HOURS).color(ink).single_line()));
        let tag = match (r.kind, r.from) {
            (MonthKind::NotBegun, _) => Some("not begun".to_string()),
            (_, Some(f)) => Some(format!("from {}", naive(f).format("%-d %b"))),
            (MonthKind::SoFar, None) => Some("so far".to_string()),
            _ => None,
        };
        if let Some(t) = tag {
            plot = plot.child(
                row()
                    .absolute(x0, base + 18.0)
                    .w(mw)
                    .justify_center()
                    .child(text(t).key(format!("year-month-tag-{i}")).font(pal::HOURS).color(p.ink3).single_line().truncate().min_w(0.0)),
            );
        }
        let Some(v) = values[i] else { continue };
        let h = if most > 0.0 { (v / most) as f32 * BAR_MAX } else { 0.0 };
        let shown = if v > 0.0 { h.max(2.0) } else { 0.0 };
        let ym = r.ym;
        let mut hit = el()
            .key(format!("year-month-{i}"))
            .absolute(x0, grid_h + 6.0)
            .w(mw)
            .h(base - grid_h - 6.0)
            .radius(R_MARK)
            .role("button")
            .label(ym.name())
            .on_hover_change(move |s: &mut Desk, inside, _| {
                if inside {
                    s.reports.hover = Some(Hover::Month(i));
                } else if s.reports.hover == Some(Hover::Month(i)) {
                    s.reports.hover = None;
                }
            })
            .on_press_tx(move |s: &mut Desk, tx| reports::show_period(s, Period::Month(ym), tx));
        if cursor == Some(i) {
            hit = hit.outline(2.0, 1.0, p.accent);
        }
        plot = plot.child(hit);
        plot = plot.child(el().key(format!("year-bar-{i}")).absolute(cx - bw / 2.0, base - shown).w(bw).h(shown).corners([3.0, 3.0, 0.0, 0.0]).fill(top));
        // An amount that does not fit over its month steps down a size.
        let label = month_amount(v, m);
        let font = if rune::text::advance(&label, pal::SMALL_STRONG_NUM) <= mw - 2.0 { pal::SMALL_STRONG_NUM } else { pal::HOURS };
        plot = plot.child(
            row()
                .absolute(x0 - 6.0, base - shown - 17.0)
                .w(mw + 12.0)
                .justify_center()
                .child(text(label).key(format!("year-month-amount-{i}")).font(font).color(p.ink).single_line()),
        );
    }
    // The month marked: its name, amount and what a click does, over the bottom of the grid.
    if let Some(i) = hover.or(cursor)
        && let Some(v) = values.get(i).copied().flatten()
    {
        let (x0, _) = span(rows[i].ym);
        let lines = [month_tip(rows[i].ym, v, m, rows[i].kind == MonthKind::SoFar), "click: open Month".to_string()];
        let mut t = col().key("year-month-tip").w(240.0).padding(10.0).gap(3.0).radius(10.0).fill(p.panel).border(1.0, p.row_line).shadow(
            12.0,
            26.0,
            Color::BLACK.alpha(0.2),
        );
        for (k, l) in lines.into_iter().enumerate() {
            t = t.child(text(l).key(format!("tip-{k}")).w_full().wrap().font(if k == 0 { pal::STRONG } else { pal::SMALL }).color(if k == 0 {
                p.ink
            } else {
                p.ink2
            }));
        }
        let max_x = span(rows[11].ym).1 - 240.0;
        plot = plot.child(t.absolute(x0.min(max_x).max(0.0), grid_h - 62.0));
    }
    plot
}

/// A month's amount over its bar: `$11,632`, `221h`, `486`.
pub fn month_amount(v: f64, m: Metric) -> String {
    match m {
        Metric::Spend | Metric::Avoidable => usd_whole(v),
        Metric::Agent | Metric::Keyboard => crate::reports::big_hours((v * 60_000.0) as i64),
        _ => int(v.round() as u64),
    }
}

/// `October 2026 · $11,632 spent`, `November 2026 · 128h of agent time so far`.
pub fn month_tip(ym: tokprof_core::month::Ym, v: f64, m: Metric, so_far: bool) -> String {
    let what = match m {
        Metric::Spend => format!("{} spent", usd_whole(v)),
        Metric::Avoidable => format!("{} avoidable", usd_whole(v)),
        Metric::Agent => format!("{} of agent time", month_amount(v, m)),
        Metric::Keyboard => format!("{} at the keyboard", month_amount(v, m)),
        _ => with_unit(v, m),
    };
    format!("{} · {what}{}", ym.name(), if so_far { " so far" } else { "" })
}

/// The numbers under the grid, in the metric's unit: the days with it of the days with logs,
/// streaks, the busiest day, and the days only the store keeps. The total is in the key numbers
/// and the busiest weekday in the rhythm, so neither is repeated here.
fn numbers(sums: &PeriodSums, m: Metric, p: &Pal, inner: f32) -> El {
    let cal = &sums.cells;
    let days = counted_days(sums);
    let active = days.iter().filter(|d| d.amounts.value(m) > 0.0).count();
    let (current, longest) = cal.streaks(m);
    let label = days_with(m);
    let mut head = label.chars();
    let title = head.next().map_or(String::new(), |c| c.to_uppercase().collect::<String>() + head.as_str());
    let n_days = |n: i64| count(n as u64, "day", "days");
    let busiest = cal.busiest(m);
    let kept = cal.only_kept();
    let per = if inner >= 720.0 { 4 } else { 2 };
    let tw = (inner - 16.0 * (per as f32 - 1.0)) / per as f32;
    let line = |key: &'static str, label: &str, value: String, sub: String| {
        col()
            .key(key)
            .w(tw)
            .gap(2.0)
            .child(text(label.to_string()).font(pal::SMALL_STRONG).color(p.ink3).single_line().truncate().min_w(0.0))
            .child(text(value).key("value").font(pal::DAY).color(p.ink).single_line().truncate().min_w(0.0))
            .child(text(sub).key("sub").w_full().wrap().max_lines(2).font(pal::SMALL).color(p.ink2))
    };
    let mut out = row().key("year-numbers").w_full().flex_wrap().gaps(14.0, 16.0).items_start();
    out = out
        .child(line("year-active", &title, format!("{active} of {}", days.len()), "of the days with logs".to_string()))
        .child(line(
            "year-streaks",
            &format!("Streaks of {label}"),
            current.map_or("no current streak".to_string(), |s| format!("{} now", n_days(s.days()))),
            longest.map_or(String::new(), |s| format!("longest {} ({} – {})", n_days(s.days()), day_name(s.from), day_name(s.to))),
        ))
        .child(line(
            "year-busiest",
            "Busiest day",
            busiest.map_or("none".to_string(), |d| day_name(d.date)),
            busiest.map_or(String::new(), |d| with_unit(d.amounts.value(m), m)),
        ))
        .child(line(
            "year-kept",
            "Kept by the store",
            if kept == 0 { "no day yet".to_string() } else { n_days(kept as i64) },
            if kept == 0 {
                "days stay here after the agents delete their logs".to_string()
            } else {
                "come only from the store: the agents deleted their logs".to_string()
            },
        ));
    if cal.untimed.0 > 0 {
        out = out.child(line(
            "year-untimed",
            "Without a time",
            format!("{} {}", int(cal.untimed.0), if cal.untimed.0 == 1 { "request" } else { "requests" }),
            format!("in no day: their logs give no time ({})", usd(cal.untimed.1)),
        ));
    }
    out
}

/// Every metric of a day, as its tooltip reads them.
pub fn tooltip_lines(d: &Day) -> Vec<String> {
    let a = &d.amounts;
    let mut lines = vec![
        naive(d.date).format("%a %-d %b %Y").to_string(),
        format!("{} spent, {} of it avoidable", usd(a.usd), usd(a.avoidable_usd)),
        format!(
            "{} in {}, {} you typed in",
            num(a.requests, "request", "requests"),
            num(d.sessions as u64, "session", "sessions"),
            int(d.typed_sessions as u64)
        ),
        if a.agent_minutes > 0 { format!("agents worked {}", rhythm::duration(a.agent_minutes as i64 * 60_000)) } else { "no agent worked".to_string() },
    ];
    // A day runs from one night's middle to the next, so it can be longer than 24 hours.
    let span = d.to_ms - d.from_ms;
    if span > 24 * 3_600_000 + 30 * 60_000 {
        let at = |ts: i64| local_time(ts, "%a %H:%M");
        lines.push(format!("the day ran {:.1}h, from {} to {}", span as f64 / 3_600_000.0, at(d.from_ms), at(d.to_ms)));
    }
    lines.extend([
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
    ]);
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
    let mut card =
        col().key("year-tip").w(TIP_W).padding(12.0).gap(4.0).radius(12.0).fill(p.panel).border(1.0, p.row_line).shadow(14.0, 30.0, Color::BLACK.alpha(0.22));
    for (i, l) in tooltip_lines(d).into_iter().enumerate() {
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

/// The weekday × hour punchcard of the picked metric in the grid's levels, the night hours in
/// the question color.
fn punchcard(s: &Desk, sums: &PeriodSums, p: &Pal, w: f32) -> El {
    let m = s.year.metric;
    let cal = &sums.cells;
    let cuts = cal.punch_cuts(m);
    let inner = w - 28.0;
    // The 24 hours take the card's width; a cell is at most 30 points high.
    let side = ((inner - LABEL_W) / 24.0 - 3.0).max(12.0);
    let high = side.min(30.0);
    let step = side + 3.0;
    let down = high + 3.0;
    let night = |h: usize| !(5..22).contains(&h);
    let mut grid = el().key("year-punch").w(LABEL_W + 24.0 * step).h(MONTH_H + 7.0 * down);
    for h in (0..24).step_by(3) {
        grid = grid.child(text(format!("{h:02}")).font(pal::HOURS).color(p.ink3).single_line().absolute(LABEL_W + h as f32 * step, 2.0));
    }
    for (wd, name) in WEEKDAYS.iter().enumerate() {
        grid = grid.child(text(*name).font(pal::HOURS).color(p.ink3).single_line().absolute(0.0, MONTH_H + wd as f32 * down + (high - 12.0) / 2.0));
        for h in 0..24 {
            let v = cal.punch.get(wd).map_or(0.0, |x| x[h].value(m));
            let top = if night(h) { p.question } else { metric_color(m, p) };
            grid = grid.child(
                cell(level(v, cuts), top, p, side, high, 4.0)
                    .key(format!("punch-{wd}-{h}"))
                    .absolute(LABEL_W + h as f32 * step, MONTH_H + wd as f32 * down)
                    .label(format!("{} {h:02}:00, {}", WEEKDAYS[wd], amount(v, m.unit()))),
            );
        }
    }
    let swatch = |c: Color| el().w(10.0).h(10.0).radius(3.0).fill(c);
    let legend = row()
        .key("year-punch-legend")
        .w_full()
        .flex_wrap()
        .gaps(4.0, 6.0)
        .center_y()
        .child(text("less").font(pal::SMALL).color(p.ink2).single_line())
        .children((0..5).map(|l| cell(l, metric_color(m, p), p, 10.0, 10.0, 3.0)))
        .child(text("more").font(pal::SMALL).color(p.ink2).single_line().pr(10.0))
        .child(swatch(p.question))
        .child(text("at night, 22:00–05:00").font(pal::SMALL).color(p.ink2).single_line());
    let content = col().w_full().gap(10.0).child(grid).child(legend);
    card(None, "When the agents work", Some(format!("{} by weekday and hour, local time", m.title().to_lowercase())), content, &s.reports, p)
}

/// The rhythm of the year: the busiest hour, the night share, the busiest weekday and the weekend
/// share, which the pills do not change.
fn rhythm_card(s: &Desk, sums: &PeriodSums, p: &Pal, w: f32) -> El {
    let span = &sums.span;
    let (weekday, weekend) = weekdays(&span.days);
    let tiles: Vec<(&str, String, String)> = vec![
        (
            "Busiest hour",
            span.busiest_hour.map_or("–".into(), |(h, _)| format!("{h:02}:00–{:02}:00", (h + 1) % 24)),
            span.busiest_hour.map_or("no agent time".into(), |(_, x)| format!("{:.0}% of agent time", 100.0 * x)),
        ),
        ("Night share", span.night_share.map_or("–".into(), |x| format!("{:.0}%", 100.0 * x)), "of agent time 22:00–05:00".into()),
        (
            "Busiest weekday",
            weekday.map_or("–".into(), |(d, _)| weekday_name(d).to_string()),
            weekday.map_or("no spend".into(), |(d, v)| format!("{} on {}", usd_whole(v), weekday_plural(d))),
        ),
        ("Weekend share", weekend.map_or("–".into(), |x| format!("{:.0}%", 100.0 * x)), "of spend, Saturdays and Sundays".into()),
    ];
    let tw = (w - 28.0 - 10.0) / 2.0;
    let mut grid = row().key("year-rhythm").w_full().flex_wrap().gaps(10.0, 10.0);
    for (i, (label, value, sub)) in tiles.into_iter().enumerate() {
        grid = grid.child(
            col()
                .key(format!("year-rhythm-{i}"))
                .w(tw)
                .px(12.0)
                .py(10.0)
                .gap(1.0)
                .radius(R_TILE)
                .fill(p.panel)
                .child(text(label).key("record-label").font(pal::SMALL).color(p.ink3).single_line())
                .child(text(value).key("record-value").font(pal::DAY).color(p.ink).single_line().truncate().min_w(0.0))
                .child(text(sub).key("record-sub").w_full().wrap().max_lines(2).font(pal::SMALL).color(p.ink2)),
        );
    }
    card(None, "Rhythm", Some("agent time and spend over the year".into()), grid, &s.reports, p)
}

fn weekday_name(w: usize) -> &'static str {
    ["Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday", "Sunday"][w % 7]
}

/// Agents and models from the store: a bar per agent, a row per model with its agent.
fn agents(s: &Desk, sums: &PeriodSums, p: &Pal) -> El {
    let Some(c) = &sums.cards else { return card(None, "Agents and models", None, el(), &s.reports, p) };
    let total: f64 = c.agents.iter().map(|x| x.1).sum::<f64>().max(1e-9);
    let first = c.agents.first().map_or(1e-9, |x| x.1.max(1e-9));
    let mut out = col().w_full().gap(2.0);
    if c.agents.is_empty() {
        out = out.child(text("No spend in this year.").key("agents-none").font(pal::BODY).color(p.ink2).single_line());
    }
    let color = |agent: &str| s.browser.agent_place(agent).map_or(p.ink3, |i| p.agents[i % p.agents.len()]);
    for (i, (agent, v)) in c.agents.iter().enumerate() {
        let line = reports::bar_line(agent.clone(), true, v / first, color(agent), v / total, usd_whole(*v), p);
        out = out.child(line.key(format!("year-agent-{i}")));
    }
    if !c.models.is_empty() {
        out = out.child(el().w_full().h(1.0).my(6.0).fill(p.row_line));
    }
    let all: f64 = c.models.iter().map(|x| x.2).sum::<f64>().max(1e-9);
    for (i, (agent, model, v)) in c.models.iter().take(MODELS).enumerate() {
        out = out.child(
            row()
                .key(format!("year-model-{i}"))
                .w_full()
                .h(24.0)
                .px(6.0)
                .gap(8.0)
                .center_y()
                .child(crate::view::dot(color(agent), 7.0))
                .child(text(model.clone()).key("label").font(pal::BODY).color(p.ink).single_line().truncate().shrink().min_w(0.0))
                .child(text(agent.clone()).font(pal::SMALL).color(p.ink3).single_line())
                .child(spacer().grow(1.0))
                .child(
                    row().w(48.0).justify_end().child(text(format!("{:.1}%", 100.0 * v / all)).key("share").font(pal::SMALL_NUM).color(p.ink2).single_line()),
                )
                .child(row().w(64.0).justify_end().child(text(usd_whole(*v)).key("amount").font(pal::STRONG_NUM).color(p.ink).single_line())),
        );
    }
    if c.models.len() > MODELS {
        let rest: f64 = c.models[MODELS..].iter().map(|x| x.2).sum();
        out = out.child(
            row()
                .key("year-models-more")
                .w_full()
                .h(24.0)
                .px(6.0)
                .gap(8.0)
                .center_y()
                .child(text(format!("{} more models", c.models.len() - MODELS)).font(pal::BODY).color(p.ink2).single_line())
                .child(spacer().grow(1.0))
                .child(row().w(64.0).justify_end().child(text(usd_whole(rest)).font(pal::STRONG_NUM).color(p.ink2).single_line())),
        );
    }
    card(None, "Agents and models", Some("from the store, API-equivalent USD".into()), out, &s.reports, p)
}

/// What to change from the store: a row per miss cause with its dollars, its share, its months
/// and the advice; a row opens its top sessions.
fn changes_card(s: &Desk, sums: &PeriodSums, p: &Pal) -> El {
    let (Some(c), Some(cards)) = (s.reports.changes(), &sums.cards) else {
        return card(Some(Part::Saved), "What to change", None, el(), &s.reports, p);
    };
    let total: f64 = c.saved.iter().map(|x| x.usd).sum();
    let first = c.saved.first().map_or(1e-9, |x| x.usd.max(1e-9));
    let mut out = col().w_full().gap(2.0);
    if c.saved.is_empty() {
        out = out.child(text("No miss could have been avoided this year.").key("year-changes-none").w_full().wrap().font(pal::BODY).color(p.ink2));
    }
    let lines = crate::changes::lines(c, Part::Saved, s.reports.open.as_ref());
    for r in &c.saved {
        let at = lines.iter().position(|l| *l == crate::changes::Line::Row(r.name.clone())).unwrap_or(0);
        let open = s.reports.open.as_ref().is_some_and(|(q, n)| *q == Part::Saved && *n == r.name);
        let split = c.sessions_of(&r.name).is_some_and(|l| !l.is_empty());
        let chevron = if !split {
            " "
        } else if open {
            "▾"
        } else {
            "▸"
        };
        let name = r.name.clone();
        let line = row()
            .w_full()
            .h(26.0)
            .px(6.0)
            .gap(8.0)
            .center_y()
            .child(row().w(12.0).child(text(chevron).font(pal::SMALL).color(p.ink3).single_line()))
            .child(text(r.name.clone()).key("label").font(pal::BODY).color(p.ink).single_line().truncate().w(130.0))
            .child(crate::overlay::meter(r.usd / first, p.accent.alpha(0.75), p, 6.0))
            .child(row().w(46.0).justify_end().child(text(format!("{:.1}%", 100.0 * r.share)).font(pal::SMALL_NUM).color(p.ink2).single_line()))
            .child(row().w(64.0).justify_end().child(text(usd_whole(r.usd)).key("amount").font(pal::STRONG_NUM).color(p.accent).single_line()));
        let mut line = cursor_row(line, Part::Saved, at, s, p).role("button").label(r.name.clone());
        if split {
            line = line.on_press(move |s: &mut Desk| crate::changes::toggle(s, Part::Saved, &name));
        }
        let months = cards
            .causes
            .iter()
            .find(|x| x.name == r.name)
            .map(|x| x.months.iter().map(|(ym, v)| format!("{} {}", &ym.month_name()[..3], usd_whole(*v))).collect::<Vec<_>>().join(" · "));
        let mut block = col().w_full().gap(1.0).child(line);
        // In a row with the room around them, so that two lines are measured as two.
        if let Some(m) = months.filter(|m| !m.is_empty()) {
            block = block.child(row().w_full().pl(26.0).pr(6.0).child(text(m).key("months").w_full().wrap().font(pal::SMALL_NUM).color(p.ink2)));
        }
        if let Some(h) = c.hint(&r.name) {
            block = block.child(row().w_full().pl(26.0).pr(6.0).pb(4.0).child(text(h.to_string()).key("hint").w_full().wrap().font(pal::SMALL).color(p.ink3)));
        }
        if open && let Some(list) = c.sessions_of(&r.name) {
            for (i, x) in list.iter().take(crate::changes::SESSIONS).enumerate() {
                block = block.child(crate::changes::session_line(s, Part::Saved, at + 1 + i, x, p));
            }
        }
        out = out.child(block);
    }
    let share = if sums.key.usd > 0.0 { format!(", {:.1}% of spend", 100.0 * total / sums.key.usd) } else { String::new() };
    card(Some(Part::Saved), "What to change", Some(format!("{} avoidable this year{share}", usd_whole(total))), out, &s.reports, p)
}

/// Year in review in the pane: the card small, dimmed while it is not ready, with Open and Save.
fn share_card(s: &Desk, sums: &PeriodSums, p: &Pal, w: f32) -> El {
    let Period::Year(y) = sums.period else { return el() };
    let now = s.reports.now();
    let ready = readiness(sums, now);
    let scale = if w >= 620.0 { 0.5 } else { 0.4 };
    let (iw, ih) = (crate::month::CARD.0 * scale, crate::month::CARD.1 * scale);
    let mut frame = el().key("year-image").w(iw).h(ih).radius(10.0).clip_rounded(10.0).fill(p.bg).border(1.0, p.row_line);
    frame = match s.year.years.get(&y) {
        Some((_, year)) => {
            let card = crate::month::year_card_el(year, p, false);
            let (dx, dy) = (-(crate::month::CARD.0 - iw) / 2.0, -(crate::month::CARD.1 - ih) / 2.0);
            frame.child(card.scale(scale).absolute(dx, dy))
        }
        None => frame.child(text("drawing the card…").font(pal::SMALL).color(p.ink3).single_line().absolute(12.0, 12.0)),
    };
    let headline = if sums.complete { format!("{y} with agents") } else { format!("{y} with agents, so far") };
    let mut side = col().grow(1.0).min_w(0.0).gap(8.0).child(text(headline).key("year-review-name").font(pal::DAY).color(p.ink).single_line());
    let line = if ready.ready {
        "The card of the year, as tokprof year prints it, to post: no project names, no titles.".to_string()
    } else if let Some(on) = ready.ready_on_text() {
        format!(
            "Not ready yet: the card opens once the year has 90 days of logs, or when it is over. {}, so its card is ready on {on}. Until then the preview shows the numbers so far, dimmed.",
            ready.logs_text(y)
        )
    } else {
        format!("{y} has no logs, so it has no card.")
    };
    side = side.child(text(line).key("year-review-ready").w_full().wrap().font(pal::SMALL).color(p.ink2));
    let has_logs = ready.ready_on.is_some();
    side = side.child(
        row()
            .gap(8.0)
            .flex_wrap()
            .child(
                reports::share_button("year-open".into(), if ready.ready { "Open card".into() } else { "Open preview".into() }, true, has_logs, p)
                    .on_press(move |s: &mut Desk| open(s, y)),
            )
            .child(reports::share_button("year-save".into(), "Save image".into(), false, ready.ready, p).on_press(move |s: &mut Desk| save_year(s, y))),
    );
    side = side.child(text("r opens the card; in it, s saves a 1080×1350 image to ~/Downloads").w_full().wrap().font(pal::SMALL).color(p.ink3));
    let content = if w >= 520.0 { row().w_full().gap(16.0).items_start().child(frame).child(side) } else { col().w_full().gap(12.0).child(frame).child(side) };
    let aside = if ready.ready { if sums.complete { "the year" } else { "so far" } } else { "not ready yet" };
    card(None, "Year in review", Some(aside.into()), content, &s.reports, p)
}

// ---------------------------------------------------------------------------------------------
// Year in review.

/// Asks for the cards of the shown year and of the open card's years from the store as it is
/// now, on a worker, and takes in those computed. Returns whether one arrived.
pub fn want_cards(s: &mut Desk) -> bool {
    let mut arrived = false;
    let done: Vec<i32> = s.year.drawing.iter().filter(|(_, (_, slot))| slot.lock().map(|m| m.is_some()).unwrap_or(true)).map(|(y, _)| *y).collect();
    for y in done {
        if let Some((at, slot)) = s.year.drawing.remove(&y)
            && let Some(m) = slot.lock().unwrap_or_else(|e| e.into_inner()).take()
        {
            s.year.years.insert(y, (at, Arc::new(m)));
            arrived = true;
        }
    }
    if let Some(c) = &mut s.year.card
        && let Some(n) = c.next.filter(|n| s.year.years.contains_key(n))
    {
        c.year = n;
        c.next = None;
        arrived = true;
    }
    let Some(store) = s.calendar.data.get().and_then(|c| c.store.clone()) else { return arrived };
    let at = Arc::as_ptr(&store) as usize;
    let mut wanted = vec![s.reports.shown_year()];
    if let Some(c) = &s.year.card {
        wanted.extend([Some(c.year), c.next].into_iter().flatten());
    }
    let now = s.reports.now();
    for y in wanted {
        let fresh = s.year.years.get(&y).is_some_and(|(a, _)| *a == at) || s.year.drawing.get(&y).is_some_and(|(a, _)| *a == at);
        if fresh {
            continue;
        }
        let slot: Arc<Mutex<Option<Year>>> = Arc::default();
        s.year.drawing.insert(y, (at, slot.clone()));
        let (store, select) = (store.clone(), s.calendar.select.clone());
        std::thread::spawn(move || {
            if let Ok(year) = tokprof_browse::contain::run_on(None, || tokprof_browse::calendar::year(&store, &select, y, now)) {
                *slot.lock().unwrap_or_else(|e| e.into_inner()) = Some(year);
            }
            crate::wake();
        });
    }
    arrived
}

/// Computes the cards of `years` at once, for a frame without a window.
pub fn settle_cards(s: &mut Desk, years: &[i32]) {
    let Some(store) = s.calendar.data.get().and_then(|c| c.store.clone()) else { return };
    let at = Arc::as_ptr(&store) as usize;
    for &y in years {
        let year = tokprof_browse::calendar::year(&store, &s.calendar.select, y, s.reports.now());
        s.year.years.insert(y, (at, Arc::new(year)));
    }
}

/// The year the logs begin in, as the store's sums say it.
fn first_year(s: &Desk) -> Option<i32> {
    s.reports.sums().and_then(|x| x.first_data).map(|f| tokprof_core::month::Ym::of_date(f).year)
}

/// Opens Year in review of `year` over the window.
pub fn open(s: &mut Desk, year: i32) {
    s.year.card = Some(Card { year, next: None, status: None });
    if s.year.years.contains_key(&year) {
        return;
    }
    // Shown at once when the store is in; else as soon as the worker sends it.
    if let Some(store) = s.calendar.data.get().and_then(|c| c.store.clone()) {
        let at = Arc::as_ptr(&store) as usize;
        let y = tokprof_browse::calendar::year(&store, &s.calendar.select, year, s.reports.now());
        s.year.years.insert(year, (at, Arc::new(y)));
    }
}

/// The year the open card shows, once computed.
pub fn card_year(s: &Desk) -> Option<&Year> {
    let c = s.year.card.as_ref()?;
    s.year.years.get(&c.year).map(|(_, y)| y.as_ref())
}

/// The keys while the card is open: ← and → the year before and after, never past this year nor
/// before the year the logs begin; `s` saves the image; esc closes it.
pub fn card_key(s: &mut Desk, e: &KeyEvent, _: &mut Tx) -> bool {
    if e.is_command('s') {
        save(s);
        return true;
    }
    if !plain(e) {
        return false;
    }
    match &e.key {
        RuneKey::Escape => s.year.card = None,
        RuneKey::Left | RuneKey::Right => {
            let this = s.reports.this_year();
            let first = first_year(s).unwrap_or(this);
            if let Some(c) = &mut s.year.card {
                let from = c.next.unwrap_or(c.year);
                let to = if e.key == RuneKey::Left { from - 1 } else { from + 1 };
                if (first..=this).contains(&to) {
                    c.next = Some(to);
                    c.status = None;
                }
            }
        }
        RuneKey::Text(t) if t == "s" => save(s),
        _ => {}
    }
    true
}

/// Saves the open card's image.
pub fn save(s: &mut Desk) {
    let Some(y) = s.year.card.as_ref().map(|c| c.year) else { return };
    save_year(s, y);
}

/// Draws the card of `year` as an image into `~/Downloads/tokprof-<YYYY>.png`, by this program
/// run again without a window, as Save image does for a month; a card that is not ready is not
/// saved, and says when it will be.
pub fn save_year(s: &mut Desk, year: i32) {
    if s.year.saving.is_some() {
        return;
    }
    let ready = s.year.years.get(&year).map(|(_, y)| y.ready);
    let line = match ready {
        Some(r) if r.ready => None,
        Some(r) => Some(match r.ready_on_text() {
            Some(on) => format!("the card of {year} is ready on {on}"),
            None => format!("{year} has no logs, so it has no card"),
        }),
        None => Some(format!("the card of {year} is still being drawn")),
    };
    if let Some(line) = line {
        set_status(s, line);
        return;
    }
    let path = crate::month::free_path(&image_path(&tokprof_browse::paths::home(), year));
    let Ok(exe) = std::env::current_exe() else { return };
    let mut args: Vec<String> =
        vec!["--snap".into(), path.to_string_lossy().to_string(), "--scene".into(), "year-card".into(), "--year".into(), format!("{year}")];
    args.extend(["--size".into(), format!("{}x{}", crate::month::CARD.0, crate::month::CARD.1), "--colors".into(), s.theme.get().id.clone()]);
    args.extend(s.month.args.iter().cloned());
    args.push("--card-image".into());
    let saved: crate::month::Saved = Arc::default();
    s.year.saving = Some(saved.clone());
    if let Some(c) = &mut s.year.card {
        c.status = Some("saving the image…".into());
    }
    std::thread::spawn(move || {
        let mut cmd = std::process::Command::new(exe);
        cmd.args(&args).stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::piped());
        let result = tokprof_browse::contain::run_on(None, || crate::month::run_for(cmd, crate::month::SAVE_LIMIT))
            .and_then(|r| r)
            .and_then(|_| if path.exists() { Ok(path) } else { Err("the image was not written".to_string()) });
        *saved.lock().unwrap_or_else(|e| e.into_inner()) = Some(result);
        crate::wake();
    });
}

/// Where the card of `year` is saved: `~/Downloads/tokprof-2026.png`, before `-2`, `-3`.
pub fn image_path(home: &std::path::Path, year: i32) -> std::path::PathBuf {
    home.join("Downloads").join(format!("tokprof-{year:04}.png"))
}

fn set_status(s: &mut Desk, line: String) {
    s.status = (line.clone(), std::time::Instant::now());
    if let Some(c) = &mut s.year.card {
        c.status = Some(line);
    }
}

/// Takes in a finished save. Returns whether the card changed.
pub fn poll(s: &mut Desk) -> bool {
    let Some(saved) = &s.year.saving else { return false };
    let Some(result) = saved.lock().unwrap_or_else(|e| e.into_inner()).take() else { return false };
    s.year.saving = None;
    let line = match result {
        Ok(path) => format!("saved {}", tokprof_core::labels::display_path(&path.to_string_lossy(), None, &tokprof_browse::paths::home())),
        Err(e) => format!("could not save the image: {e}"),
    };
    set_status(s, line);
    true
}

/// The card over the window, with its buttons under it.
pub fn view(s: &Desk, p: &Pal) -> El {
    let Some(c) = &s.year.card else { return el() };
    let ready = card_year(s).map(|y| y.ready);
    let button = |key: &'static str, label: &str, primary: bool, enabled: bool| {
        row()
            .key(key)
            .h(32.0)
            .px(16.0)
            .center_y()
            .radius(PILL)
            .fill(if primary && enabled { p.accent } else { p.panel })
            .border(1.0, if primary && enabled { p.accent } else { p.row_line })
            .hover(if primary { p.accent.alpha(0.88) } else { p.hover })
            .role("button")
            .label(label.to_string())
            .disabled(!enabled)
            .child(
                text(label.to_string())
                    .font(pal::STRONG)
                    .color(if primary && enabled {
                        p.on_accent
                    } else if enabled {
                        p.ink
                    } else {
                        p.ink3
                    })
                    .single_line(),
            )
    };
    let status = c.status.clone().unwrap_or_else(|| "← → another year · s saves the image · esc closes".into());
    let buttons = row()
        .key("year-buttons")
        .w(crate::month::CARD.0)
        .gap(10.0)
        .center_y()
        .child(button("year-card-save", "Save image", true, ready.is_some_and(|r| r.ready)).on_press(save))
        .child(button("year-card-close", "Close", false, true).on_press(|s: &mut Desk| s.year.card = None))
        .child(spacer().grow(1.0))
        .child(text(status).key("year-status").font(pal::SMALL).color(p.ink2).single_line().truncate().shrink().min_w(0.0));
    let body = match card_year(s) {
        Some(y) => crate::month::year_card_el(y, p, true),
        None => col()
            .key("year-card")
            .w(crate::month::CARD.0)
            .h(crate::month::CARD.1)
            .radius(crate::overlay::R_CARD)
            .fill(p.bg)
            .center()
            .child(text("drawing the card…").font(pal::BODY).color(p.ink3).single_line()),
    };
    // In a window too low for the card and its buttons, both are drawn smaller.
    let height = if s.window.1 > 0.0 { s.window.1 } else { crate::view::WINDOW.1 };
    let fit = ((height - 40.0) / (crate::month::CARD.1 + 46.0)).min(1.0);
    crate::overlay::scrim("year-scrim", p, |s| s.year.card = None).center().child(
        col()
            .key("year-overlay")
            .gap(14.0)
            .items_center()
            .scale(fit)
            .child(body.shadow(30.0, 70.0, Color::BLACK.alpha(0.3)).on_press(|_: &mut Desk| {}))
            .child(buttons),
    )
}

// ---------------------------------------------------------------------------------------------
// The scenes.

/// The scene: Year on `--year`, or this year, with the store, the sums and the card waited for.
pub fn scene(s: &mut Desk) {
    reports::show(s, Mode::Year);
    let this = s.reports.this_year();
    s.reports.year = s.year.wanted.filter(|&y| y != this);
    let limit = std::time::Duration::from_secs(120);
    let changes = s.browser.changes();
    s.calendar.data.settle(changes, limit);
    let store = s.calendar.data.get().and_then(|c| c.store.clone());
    let key = s.reports.key();
    s.reports.sums.settle(&key, store.as_ref(), limit);
    keep_changes(s);
    let y = s.reports.shown_year();
    settle_cards(s, &[y]);
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
    fn the_squares_fit_the_card_s_width() {
        // A window of 1400 and one of 960: 53 columns fill the card, between 11 and 44 points.
        let wide = square(crate::reports::content_w(1400.0) - 28.0, 53);
        let narrow = square(crate::reports::content_w(960.0) - 28.0, 53);
        assert!((20.0..=26.0).contains(&wide), "{wide}");
        assert!((MIN_SQUARE..=14.0).contains(&narrow), "{narrow}");
        assert_eq!(square(4000.0, 53), MAX_SQUARE);
    }

    #[test]
    fn the_amounts_of_the_months() {
        let oct = tokprof_core::month::Ym { year: 2026, month: 10 };
        assert_eq!(month_tip(oct, 11_631.8, Metric::Spend, false), "October 2026 · $11,632 spent");
        assert_eq!(month_tip(oct, 486.0, Metric::Commits, false), "October 2026 · 486 commits");
        assert_eq!(month_amount(221.0 * 60.0, Metric::Agent), "221h");
    }

    #[test]
    fn a_saved_year_card_is_named_by_its_year_and_never_overwrites() {
        let dir = std::env::temp_dir().join(format!("tokprof-year-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("Downloads")).unwrap();
        let path = image_path(&dir, 2026);
        assert_eq!(path, dir.join("Downloads").join("tokprof-2026.png"));
        std::fs::write(&path, b"x").unwrap();
        assert_eq!(crate::month::free_path(&path), dir.join("Downloads").join("tokprof-2026-2.png"));
        std::fs::write(dir.join("Downloads").join("tokprof-2026-2.png"), b"x").unwrap();
        assert_eq!(crate::month::free_path(&path), dir.join("Downloads").join("tokprof-2026-3.png"));
    }
}
