//! The Reports view: the report of the last 7, 30 or 90 days in place of the lanes, the list and
//! the inspector, in cards in the look of the settings panel. The numbers are those `tokprof
//! summary` and `tokprof pauses` print, from the same functions; the browser package reads them on
//! a worker (see `tokprof_browse::reports`).

use rune::{Color, Cx, El, Key as RuneKey, KeyEvent, ScrollSpec, Tx, col, el, row, spacer, text};
use tokprof_browse::reports::{DEFAULT_PERIOD, PERIODS, Progress, Report, Reports};
use tokprof_core::analysis::Component;
use tokprof_core::fmt::{int, local_time, tokens, usd};
use tokprof_core::overview::{DaySpend, Overview};
use tokprof_core::pauses::AgentPauses;
use tokprof_core::report::Spend;
use tokprof_core::rhythm;
use tokprof_core::savings::Changes;

use crate::overlay::{PILL, R_GROUP, R_MARK, R_ROW, R_TILE};
use crate::pal::{self, Pal};
use crate::view;
use crate::{Desk, plain};

/// The key of the report's scroll viewport.
pub const PANE: &str = "reports";
/// The rows of the project and origin cards before the rest.
pub const TOP: usize = 10;
/// The height of the day chart, and the width of its axis labels.
const CHART_H: f32 = 170.0;
const AXIS_W: f32 = 52.0;
/// The room above and under the day chart's lines, half a label's height.
const PLOT_PAD: f32 = 7.0;

/// What the window shows under the bar.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Mode {
    Day,
    Reports,
    Calendar,
}

/// The parts of the report that take the keyboard, in the order Tab moves through them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Part {
    Period,
    Saved,
    Ways,
    Habits,
    Days,
    Projects,
    Origins,
    Models,
    Returns,
    Sizes,
}

impl Part {
    pub const ORDER: [Part; 10] =
        [Part::Period, Part::Saved, Part::Ways, Part::Habits, Part::Days, Part::Projects, Part::Origins, Part::Models, Part::Returns, Part::Sizes];

    pub fn key(self) -> &'static str {
        match self {
            Part::Period => "rep-period",
            Part::Saved => "rep-saved",
            Part::Ways => "rep-ways",
            Part::Habits => "rep-habits",
            Part::Days => "rep-days",
            Part::Projects => "rep-projects",
            Part::Origins => "rep-origins",
            Part::Models => "rep-models",
            Part::Returns => "rep-returns",
            Part::Sizes => "rep-sizes",
        }
    }

    fn step(self, back: bool) -> Part {
        let n = Part::ORDER.len();
        let at = Part::ORDER.iter().position(|&p| p == self).unwrap_or(0);
        Part::ORDER[if back { (at + n - 1) % n } else { (at + 1) % n }]
    }
}

/// The view's state: the mode, the period, the part with the keyboard and its cursor, the day
/// under the pointer, and the reports.
pub struct State {
    pub mode: Mode,
    /// The period, in days.
    pub days: u32,
    pub part: Part,
    /// The day chart's cursor, as a local date; none, or a date the report no longer has, for
    /// the last date. A date stays the same day when the report is read again with other dates.
    pub day: Option<i64>,
    /// The row cursor of the part with the keyboard.
    pub row: usize,
    /// The date under the pointer in the day chart.
    pub hover: Option<i64>,
    pub data: Reports,
    /// The row of what to change whose sessions are open under it.
    pub open: Option<(Part, String)>,
    /// The worker's progress as the last poll saw it.
    seen: Option<Progress>,
}

impl State {
    pub fn new(data: Reports) -> State {
        State { mode: Mode::Day, days: DEFAULT_PERIOD, part: Part::Period, day: None, row: 0, hover: None, data, open: None, seen: None }
    }

    /// The report of the period shown, once read.
    pub fn report(&self) -> Option<&Report> {
        self.data.get(self.days)
    }

    fn overview(&self) -> Option<&Overview> {
        self.report().and_then(|r| r.overview.as_deref().ok())
    }

    /// What to change in the period shown, once read.
    pub fn changes(&self) -> Option<&Changes> {
        self.overview().map(|o| &o.changes)
    }
}

/// Takes in what the reports' worker sent while Reports is shown, asks for the shown period when
/// it is missing or out of date, and wakes the window when one is due. Returns whether the view
/// changed. While Day is shown it does nothing.
pub fn poll(s: &mut Desk, tx: &mut Tx) -> bool {
    if s.reports.mode != Mode::Reports {
        return false;
    }
    let (days, changes) = (s.reports.days, s.browser.changes());
    let arrived = s.reports.data.tick(days, changes);
    if let Some(at) = s.reports.data.next_due(days, changes) {
        tx.wake_after(at.saturating_duration_since(std::time::Instant::now()).max(std::time::Duration::from_millis(1)));
    }
    let progress = s.reports.data.progress();
    let moved = progress != s.reports.seen;
    s.reports.seen = progress;
    arrived || moved
}

/// Shows Day, Reports or Calendar.
pub fn show(s: &mut Desk, mode: Mode) {
    s.reports.mode = mode;
    s.reports.hover = None;
    s.calendar.hover = None;
}

/// Picks the period of `days` days.
pub fn pick_period(s: &mut Desk, days: u32, tx: &mut Tx) {
    if s.reports.days != days {
        s.reports.days = days;
        s.reports.day = None;
        s.reports.hover = None;
        s.reports.open = None;
        if s.reports.part != Part::Period {
            s.reports.row = 0;
        }
        tx.scroll_to_y(PANE, 0.0);
    }
}

/// ⌘1 shows Day, ⌘2 Reports and ⌘3 Calendar, whichever shows now.
pub fn switch_key(s: &mut Desk, e: &KeyEvent) -> bool {
    let mode = if e.is_command('1') {
        Mode::Day
    } else if e.is_command('2') {
        Mode::Reports
    } else if e.is_command('3') {
        Mode::Calendar
    } else {
        return false;
    };
    show(s, mode);
    true
}

/// The dates of a report's window, as days since 1970-01-01 in local time, first to last.
pub fn dates(r: &Report) -> Vec<i64> {
    let offset = &rhythm::local_offset;
    let (first, last) = (rhythm::date_of(r.since_ms, offset), rhythm::date_of((r.until_ms - 1).max(r.since_ms), offset));
    (first..=last).collect()
}

/// Opens the lanes of `date` in Day, as a click on its bar does.
pub fn open_day(s: &mut Desk, date: i64, tx: &mut Tx) {
    s.browser.show_day(date);
    show(s, Mode::Day);
    tx.scroll_to_y(view::PANE, 0.0);
}

/// Filters the list to `project` and shows Day, as a click on its bar does.
pub fn open_project(s: &mut Desk, project: &str, tx: &mut Tx) {
    s.browser.set_filter(project);
    s.cursor = 0;
    show(s, Mode::Day);
    tx.scroll_to_y(view::LIST, 0.0);
    if let Some(row) = s.browser.selected() {
        tx.scroll_into_view(s.browser.session(row).to_string());
    }
    tx.scroll_to_y(view::PANE, 0.0);
}

/// The rows a part's cursor moves through in the report.
fn rows_of(o: &Overview, part: Part) -> usize {
    let listed = |n: usize| n.min(TOP) + usize::from(n > TOP);
    match part {
        Part::Period => PERIODS.len(),
        Part::Days | Part::Saved | Part::Ways | Part::Habits => 0,
        Part::Projects => listed(o.spread.projects.len()),
        Part::Origins => listed(o.collector.work.by_origin.len()),
        Part::Models => o.collector.spend.models.len(),
        Part::Returns => o.pauses.agents.values().map(|a| a.return_rows().len()).sum(),
        Part::Sizes => o.pauses.agents.values().map(|a| a.size_rows().len()).sum(),
    }
}

/// The keys of Reports: every key but those of the app (⌘Q, ⌘W, ⌘,) is its own.
pub fn on_key(s: &mut Desk, e: &KeyEvent, tx: &mut Tx) -> bool {
    if !plain(e) && e.key != RuneKey::Tab {
        return false;
    }
    let text = match &e.key {
        RuneKey::Text(t) => Some(t.as_str()),
        _ => None,
    };
    let r = &mut s.reports;
    match (&e.key, text) {
        (RuneKey::Tab, _) => {
            r.part = r.part.step(e.mods.shift);
            r.row = 0;
            r.day = None;
            tx.scroll_into_view(r.part.key());
            return true;
        }
        (RuneKey::Escape, _) => {
            show(s, Mode::Day);
            return true;
        }
        _ => {}
    }
    let step: Option<isize> = match (&e.key, text) {
        (RuneKey::Left, _) | (_, Some("h")) | (RuneKey::Up, _) | (_, Some("k")) => Some(-1),
        (RuneKey::Right, _) | (_, Some("l")) | (RuneKey::Down, _) | (_, Some("j")) => Some(1),
        (RuneKey::PageUp, _) => Some(-10),
        (RuneKey::PageDown, _) => Some(10),
        (RuneKey::Home, _) | (_, Some("g")) => Some(isize::MIN / 2),
        (RuneKey::End, _) | (_, Some("G")) => Some(isize::MAX / 2),
        _ => None,
    };
    let across = matches!(e.key, RuneKey::Left | RuneKey::Right) || matches!(text, Some("h" | "l"));
    let enter = e.key == RuneKey::Enter;
    match r.part {
        Part::Period => {
            if let Some(step) = step.filter(|_| across || matches!(e.key, RuneKey::Home | RuneKey::End)) {
                let at = PERIODS.iter().position(|&d| d == r.days).unwrap_or(1) as isize;
                let to = at.saturating_add(step).clamp(0, PERIODS.len() as isize - 1) as usize;
                pick_period(s, PERIODS[to], tx);
            }
        }
        Part::Days => {
            let Some(dates) = r.report().map(dates) else { return true };
            let last = dates.len().saturating_sub(1);
            let at = day_at(&dates, r.day);
            if let Some(step) = step.filter(|_| across || matches!(e.key, RuneKey::Home | RuneKey::End)) {
                r.day = dates.get((at as isize).saturating_add(step).clamp(0, last as isize) as usize).copied();
            } else if enter && let Some(&date) = dates.get(at) {
                open_day(s, date, tx);
            }
        }
        part @ (Part::Saved | Part::Ways | Part::Habits) => {
            let rows = crate::changes::line_count(s, part);
            if let Some(step) = step.filter(|_| !across)
                && rows > 0
            {
                let r = &mut s.reports;
                r.row = (r.row as isize).saturating_add(step).clamp(0, rows as isize - 1) as usize;
                tx.scroll_into_view(format!("{}-{}", part.key(), r.row));
            } else if enter {
                crate::changes::enter(s, part, tx);
            }
        }
        part => {
            let Some(o) = r.overview() else { return true };
            let rows = rows_of(o, part);
            if let Some(step) = step.filter(|_| !across)
                && rows > 0
            {
                r.row = (r.row as isize).saturating_add(step).clamp(0, rows as isize - 1) as usize;
                tx.scroll_into_view(format!("{}-{}", part.key(), r.row));
            } else if enter && part == Part::Projects {
                let project = o.spread.projects().get(r.row).filter(|_| r.row < TOP).map(|(p, _)| p.to_string());
                if let Some(project) = project {
                    open_project(s, &project, tx);
                }
            }
        }
    }
    true
}

/// The switch between Day, Reports and Calendar, at the start of the bar.
pub fn switch(s: &Desk, p: &Pal) -> El {
    let mode = s.reports.mode;
    view::segments(
        "mode",
        vec![
            view::segment("mode-day".into(), "Day", mode == Mode::Day, p).on_press(|s: &mut Desk| show(s, Mode::Day)),
            view::segment("mode-reports".into(), "Reports", mode == Mode::Reports, p).on_press(|s: &mut Desk| show(s, Mode::Reports)),
            view::segment("mode-calendar".into(), "Calendar", mode == Mode::Calendar, p).on_press(|s: &mut Desk| show(s, Mode::Calendar)),
        ],
        p.panel,
    )
}

/// The report under the bar: its head with the period, then the cards, or what is being read.
pub fn view(s: &Desk, cx: &Cx<Desk>, p: &Pal) -> El {
    let r = &s.reports;
    let report = r.report();
    let identity = report.map(|x| (x.arrived, x.overview.as_ref().map(|o| std::sync::Arc::as_ptr(o) as usize).ok()));
    let version =
        view::hash_of((r.days, r.part, r.day, r.row, r.hover, &r.open, identity, r.data.reading(r.days), r.seen, &s.theme.get().id, cx.system_dark()));
    // The memo takes its id as its key, so the viewport is outside it, keyed for the scrolls.
    col().key(PANE).w_full().grow(1.0).min_h(0.0).scroll(ScrollSpec { stick_bottom: false }).child(rune::memo("reports-body", version, || body(s, p)))
}

fn body(s: &Desk, p: &Pal) -> El {
    let r = &s.reports;
    let mut cards = col().w_full().px(24.0).pt(18.0).pb(30.0).gap(20.0).child(head(s, p));
    match r.report() {
        None => cards = cards.child(computing(s, p)),
        Some(Report { overview: Err(e), .. }) => cards = cards.child(note(format!("error: {e}"), p)),
        Some(report @ Report { overview: Ok(o), .. }) => {
            cards = cards
                .child(card(None, "Key numbers", None, key_numbers(o, p), r, p))
                .child(crate::changes::cards(s, &o.changes, p))
                .child(card(Some(Part::Days), "Spend by day", Some("a click opens the day".into()), day_chart(s, report, o, p), r, p))
                .child(
                    row()
                        .w_full()
                        .gap(20.0)
                        .items_start()
                        .child(card(Some(Part::Projects), "By project", Some("a click shows its sessions".into()), projects(s, o, p), r, p))
                        .child(card(Some(Part::Origins), "By origin", Some("the kind of work".into()), origins(s, o, p), r, p)),
                )
                .child(card(Some(Part::Models), "By model", Some("API-equivalent USD; tokens where no price is known".into()), models(s, o, p), r, p))
                .child(
                    row()
                        .w_full()
                        .gap(20.0)
                        .items_start()
                        .child(card(
                            Some(Part::Returns),
                            "Cold returns by pause",
                            Some("requests that missed after a long pause".into()),
                            returns(s, o, p),
                            r,
                            p,
                        ))
                        .child(card(Some(Part::Sizes), "Sessions by largest context", Some("the largest prompt of each".into()), sizes(s, o, p), r, p)),
                );
        }
    }
    cards
}

/// The head: the period's pills, its dates, and what was read or is being read again.
fn head(s: &Desk, p: &Pal) -> El {
    let r = &s.reports;
    let on = r.part == Part::Period;
    let pills = row().key(Part::Period.key()).gap(6.0).center_y().children(PERIODS.iter().map(|&d| {
        let chosen = d == r.days;
        row()
            .key(format!("period-{d}"))
            .h(28.0)
            .px(14.0)
            .center_y()
            .radius(PILL)
            .fill(if chosen { p.accent } else { p.bar })
            .border(1.0, if chosen { p.accent } else { p.row_line })
            .hover(if chosen { p.accent.alpha(0.88) } else { p.hover })
            .role("radio")
            .checked(chosen)
            .label(format!("{d} days"))
            .on_press_tx(move |s: &mut Desk, tx| {
                s.reports.part = Part::Period;
                pick_period(s, d, tx);
            })
            .child(text(format!("{d} days")).font(pal::STRONG).color(if chosen { p.on_accent } else { p.ink2 }).single_line())
    }));
    let pills = el().radius(PILL).child(pills);
    let pills = if on { pills.outline(2.0, 3.0, p.accent) } else { pills };
    let mut head = row().w_full().gap(14.0).center_y().child(pills);
    if let Some(report) = r.report() {
        let dates = dates(report);
        let first = rhythm::midnight(dates[0], &rhythm::local_offset) + 12 * 3_600_000;
        let span = format!("{} – {}", local_time(first, "%-d %b"), local_time(report.until_ms, "%-d %b %Y"));
        head = head.child(text(span).key("rep-span").font(pal::DAY).color(p.ink).single_line());
        head = head.child(spacer().grow(1.0));
        let aside = if r.data.reading(r.days) {
            "updating…".to_string()
        } else {
            match &report.overview {
                Ok(o) => format!("read {} {} in {:.1} s", int(o.logs as u64), if o.logs == 1 { "log" } else { "logs" }, report.took.as_secs_f64()),
                Err(_) => String::new(),
            }
        };
        head = head.child(text(aside).key("rep-read").font(pal::SMALL).color(p.ink3).single_line());
    }
    head
}

/// The line while a period is read for the first time, with how far the worker is.
fn computing(s: &Desk, p: &Pal) -> El {
    let r = &s.reports;
    let line = match r.seen {
        Some(g) if g.days == r.days => {
            format!("computing the last {} days… {} of {} logs", r.days, int(g.done as u64), int(g.total as u64))
        }
        Some(g) => {
            format!("computing the last {} days, after the last {} days… {} of {} logs", r.days, g.days, int(g.done as u64), int(g.total as u64))
        }
        None => format!("computing the last {} days…", r.days),
    };
    let share = r.seen.filter(|g| g.total > 0).map_or(0.0, |g| g.done as f64 / g.total as f64);
    col()
        .key("rep-computing")
        .w_full()
        .padding(18.0)
        .gap(12.0)
        .radius(R_GROUP)
        .fill(p.bar)
        .child(text(line).key("rep-progress").font(pal::BODY_NUM).color(p.ink2).single_line())
        .child(row().w(320.0).child(crate::overlay::meter(share, p.accent, p, 6.0)))
}

fn note(t: String, p: &Pal) -> El {
    text(t).w_full().wrap().font(pal::BODY).color(p.ink2).selectable(true)
}

/// A card: its title and an aside over a soft group, ringed in the accent while its part has the
/// keyboard, as the settings panel draws its parts.
pub fn card(part: Option<Part>, title: &str, aside: Option<String>, content: El, r: &crate::reports::State, p: &Pal) -> El {
    let on = part.is_some_and(|x| x == r.part);
    let mut group = col().w_full().min_w(0.0).padding(14.0).radius(R_GROUP).fill(p.bar).child(content);
    if let Some(part) = part {
        group = group.key(part.key());
    }
    let group = if on { group.outline(2.0, 2.0, p.accent) } else { group };
    let mut top = row().w_full().gap(8.0).center_y().child(text(title.to_string()).font(pal::HEAD).color(if on { p.accent } else { p.ink2 }).single_line());
    if let Some(a) = aside {
        top = top.child(spacer().grow(1.0)).child(text(a).font(pal::SMALL).color(p.ink3).single_line().truncate().shrink().min_w(0.0));
    }
    col().grow(1.0).min_w(0.0).gap(10.0).child(top).child(group)
}

fn component_color(c: Component, p: &Pal) -> Color {
    match c {
        Component::Read => p.read,
        Component::Write => p.write,
        Component::Rewrite => p.miss,
        Component::Output => p.out,
        _ => p.neutral,
    }
}

/// A key number: what it is, the number, and a line under it.
fn tile(key: &'static str, label: &str, value: String, sub: String, color: Color, p: &Pal) -> El {
    col()
        .key(key)
        .grow(1.0)
        .min_w(0.0)
        .px(14.0)
        .py(12.0)
        .gap(4.0)
        .radius(R_TILE)
        .fill(p.panel)
        .child(text(label.to_string()).font(pal::SMALL_STRONG).color(p.ink2).single_line())
        .child(text(value).key("value").font(pal::KPI).color(color).single_line())
        .child(text(sub).key("sub").w_full().wrap().max_lines(2).font(pal::SMALL).color(p.ink3))
}

fn key_numbers(o: &Overview, p: &Pal) -> El {
    let spend = o.spend_usd();
    let (avoidable, share) = o.avoidable();
    let (returns, premium) = o.cold_returns();
    let per_agent: Vec<String> = o.pauses.agents.iter().map(|(a, x)| format!("{} {}", a.name(), x.return_totals().0)).collect();
    row()
        .w_full()
        .gap(10.0)
        .child(tile("kpi-spend", "Spend", usd(spend), "API-equivalent".into(), p.ink, p))
        .child(tile("kpi-avoidable", "Avoidable on cache misses", usd(avoidable), format!("{share:.1}% of spend"), p.cold, p))
        .child(tile("kpi-returns", "Cold returns", int(returns as u64), format!("{} avoidable · {}", usd(premium), per_agent.join(", ")), p.ink, p))
        .child(tile("kpi-sessions", "Sessions", int(o.sessions() as u64), "with a request in the period".into(), p.ink, p))
        .child(tile("kpi-requests", "Requests", int(o.requests()), "counted".into(), p.ink, p))
}

/// A round step for an axis up to `max`: 1, 2 or 5 times a power of ten, so that three to six
/// lines fit.
fn axis_step(max: f64) -> f64 {
    if max <= 0.0 {
        return 1.0;
    }
    let raw = max / 4.0;
    let power = 10f64.powf(raw.log10().floor());
    [1.0, 2.0, 5.0, 10.0].into_iter().map(|m| m * power).find(|s| *s >= raw).unwrap_or(10.0 * power)
}

/// The place of the day cursor among `dates`: its date's, or the last date's when it has none or
/// a date `dates` does not hold.
pub fn day_at(dates: &[i64], day: Option<i64>) -> usize {
    day.and_then(|d| dates.iter().position(|&x| x == d)).unwrap_or(dates.len().saturating_sub(1))
}

/// The spend by day: a column per date with its components stacked, the axis, the dates under
/// them, and a line that names the day under the pointer or the cursor.
fn day_chart(s: &Desk, report: &Report, o: &Overview, p: &Pal) -> El {
    let r = &s.reports;
    let dates = dates(report);
    let spends: Vec<DaySpend> = dates.iter().map(|d| o.spread.days.get(d).copied().unwrap_or_default()).collect();
    let max = spends.iter().map(DaySpend::usd).fold(0.0, f64::max);
    let step = axis_step(max);
    let top = (max / step).ceil().max(1.0) * step;
    let cursor = (r.part == Part::Days).then(|| day_at(&dates, r.day));
    let marked = r.hover.and_then(|h| dates.iter().position(|&d| d == h)).or(cursor);
    // The axis's labels on the left, and the plot with its lines and the columns over them.
    // The plot has room above and under its lines for the labels centered on them.
    let (mut axis, mut plot) = (el().w(AXIS_W).h(CHART_H + 2.0 * PLOT_PAD), el().grow(1.0).min_w(0.0).h(CHART_H + 2.0 * PLOT_PAD));
    let lines = (top / step).round() as usize;
    for i in 0..=lines {
        let y = PLOT_PAD + CHART_H - (i as f64 * step / top) as f32 * CHART_H;
        plot = plot.child(el().absolute(0.0, y.min(PLOT_PAD + CHART_H - 1.0)).w_full().h(1.0).fill(p.row_line));
        axis = axis.child(
            row().absolute(0.0, y - 7.0).w(AXIS_W - 8.0).justify_end().child(text(axis_label(i as f64 * step)).font(pal::HOURS).color(p.ink3).single_line()),
        );
    }
    let columns = row().absolute(0.0, PLOT_PAD).w_full().h(CHART_H).gap(if dates.len() > 45 { 1.0 } else { 3.0 }).children(
        dates.iter().zip(&spends).enumerate().map(|(i, (&date, d))| {
            let mut stack = col().w_full().justify_end().radius(R_MARK).clip_rounded(R_MARK);
            for c in Component::ALL.iter().rev() {
                let v = d.usd[Component::ALL.iter().position(|x| x == c).unwrap_or(0)];
                let h = (v / top) as f32 * CHART_H;
                if h >= 0.5 {
                    stack = stack.child(el().w_full().h(h).fill(component_color(*c, p)));
                }
            }
            let column = col()
                .key(format!("day-{date}"))
                .grow(1.0)
                .min_w(0.0)
                .h_full()
                .justify_end()
                .radius(4.0)
                .role("button")
                .label(local_time(noon(date), "%a %-d %b"))
                .on_hover_change(move |s: &mut Desk, inside, _| {
                    if inside {
                        s.reports.hover = Some(date);
                    } else if s.reports.hover == Some(date) {
                        s.reports.hover = None;
                    }
                })
                .on_press_tx(move |s: &mut Desk, tx| open_day(s, date, tx))
                .child(stack);
            if marked == Some(i) { column.fill(p.sel) } else { column.hover(p.hover) }
        }),
    );
    let grid = row().w_full().h(CHART_H + 2.0 * PLOT_PAD).child(axis).child(plot.child(columns));
    // A date under every column that has room, and the month at its first.
    let every = match dates.len() {
        0..=16 => 1,
        17..=40 => 2,
        _ => 7,
    };
    let month = |i: usize| i == 0 || local_time(noon(dates[i]), "%-d") == "1";
    let labels = row().w_full().pl(AXIS_W).gap(if dates.len() > 45 { 1.0 } else { 3.0 }).children(dates.iter().enumerate().map(|(i, &date)| {
        let at = noon(date);
        let first = month(i);
        // A month's label is wider: the regular label beside it gives way.
        let beside = every > 1 && ((i > 0 && month(i - 1)) || (i + 1 < dates.len() && month(i + 1)));
        let shown = first || ((dates.len() - 1 - i).is_multiple_of(every) && !beside);
        let label = if !shown {
            String::new()
        } else if first {
            local_time(at, "%-d %b")
        } else {
            local_time(at, "%-d")
        };
        row().grow(1.0).min_w(0.0).justify_center().child(text(label).font(pal::HOURS).color(p.ink3).single_line())
    }));
    col().w_full().gap(8.0).child(grid).child(labels).child(readout(marked.map(|i| (dates[i], spends[i])), &spends, p))
}

/// An amount on the axis: whole dollars, with cents only below a dollar.
fn axis_label(v: f64) -> String {
    if v == 0.0 {
        "$0".into()
    } else if v < 1.0 || v.fract() != 0.0 {
        format!("${v:.2}")
    } else {
        format!("${}", int(v as u64))
    }
}

fn noon(date: i64) -> i64 {
    rhythm::midnight(date, &rhythm::local_offset) + 12 * 3_600_000
}

/// The line under the day chart: the day marked, its spend, its requests and each component's
/// amount, or the period's when no day is marked.
fn readout(day: Option<(i64, DaySpend)>, all: &[DaySpend], p: &Pal) -> El {
    let (title, d) = match day {
        Some((date, d)) => (local_time(noon(date), "%a %-d %b"), d),
        None => {
            let mut total = DaySpend::default();
            for d in all {
                for (a, b) in total.usd.iter_mut().zip(d.usd) {
                    *a += b;
                }
                total.requests += d.requests;
            }
            (format!("{} days", all.len()), total)
        }
    };
    let head = format!("{title} · {} · {} {}", usd(d.usd()), int(d.requests), if d.requests == 1 { "request" } else { "requests" });
    let chips = Component::ALL.iter().enumerate().map(|(i, c)| {
        row()
            .gap(6.0)
            .center_y()
            .child(el().w(10.0).h(10.0).radius(R_MARK).fill(component_color(*c, p)))
            .child(text(format!("{} {}", c.name(), usd(d.usd[i]))).font(pal::SMALL_NUM).color(p.ink2).single_line())
    });
    row()
        .key("rep-readout")
        .w_full()
        .pl(AXIS_W)
        .flex_wrap()
        .gaps(6.0, 16.0)
        .center_y()
        .child(text(head).key("rep-readout-head").font(pal::STRONG_NUM).color(p.ink).single_line())
        .children(chips)
}

/// Marks the row `i` of a part when its cursor is on it, and keys it for the keys to scroll to.
pub fn cursor_row(line: El, part: Part, i: usize, s: &Desk, p: &Pal) -> El {
    let r = &s.reports;
    let line = line.key(format!("{}-{i}", part.key())).radius(R_ROW);
    if r.part == part && r.row == i { line.fill(p.sel) } else { line.hover(p.hover) }
}

/// Rows of a bar chart: a label, a bar as long as its share of the first, its share of all, and
/// its amount; the rows after `TOP` as one.
fn bars(s: &Desk, part: Part, rows: Vec<(String, Spend)>, priced: bool, color: Color, p: &Pal) -> Vec<El> {
    let key = |x: &Spend| if priced { x.usd } else { x.tokens };
    let total: f64 = rows.iter().map(|(_, x)| key(x)).sum();
    let first = rows.first().map_or(0.0, |(_, x)| key(x)).max(1e-9);
    let mut shown: Vec<(String, Spend, bool)> = rows.iter().take(TOP).map(|(l, x)| (l.clone(), *x, true)).collect();
    if rows.len() > TOP {
        let mut rest = Spend::default();
        rows.iter().skip(TOP).for_each(|(_, x)| rest.add(*x));
        shown.push((format!("{} more", rows.len() - TOP), rest, false));
    }
    shown
        .into_iter()
        .enumerate()
        .map(|(i, (label, x, drill))| {
            let amount = if priced { usd(x.usd) } else { format!("{} tok", tokens(x.tokens)) };
            let line = row()
                .w_full()
                .h(26.0)
                .px(6.0)
                .gap(10.0)
                .center_y()
                .child(text(label.clone()).key("label").font(pal::BODY).color(if drill { p.ink } else { p.ink2 }).single_line().truncate().w(200.0))
                .child(crate::overlay::meter(key(&x) / first, if drill { color } else { p.neutral }, p, 6.0))
                .child(
                    row()
                        .w(50.0)
                        .justify_end()
                        .child(text(format!("{:.1}%", 100.0 * key(&x) / total.max(1e-9))).key("share").font(pal::BODY_NUM).color(p.ink2).single_line()),
                )
                .child(row().w(66.0).justify_end().child(text(amount).key("amount").font(pal::STRONG_NUM).color(p.ink).single_line()));
            let line = cursor_row(line, part, i, s, p);
            if part == Part::Projects && drill {
                line.role("button").label(label.clone()).on_press_tx(move |s: &mut Desk, tx| open_project(s, &label, tx))
            } else {
                line
            }
        })
        .collect()
}

fn projects(s: &Desk, o: &Overview, p: &Pal) -> El {
    let rows = o.spread.projects().into_iter().map(|(name, x)| (name.to_string(), x)).collect();
    col().w_full().gap(2.0).children(bars(s, Part::Projects, rows, o.priced(), p.bars, p))
}

fn origins(s: &Desk, o: &Overview, p: &Pal) -> El {
    let origins = o.collector.work.origins();
    let priced = origins.iter().any(|(_, x)| x.usd > 0.0);
    let rows = origins.into_iter().map(|(name, x)| (name.to_string(), x)).collect();
    col().w_full().gap(2.0).children(bars(s, Part::Origins, rows, priced, p.out, p))
}

/// A column of a table: its title, its width (0 for the one that takes the rest), and whether
/// its cells are numbers, set right.
struct Column(&'static str, f32, bool);

fn table(s: &Desk, part: Part, first: usize, cols: &[Column], rows: Vec<Vec<String>>, p: &Pal) -> El {
    let cell = |t: String, c: &Column, color: Color, font| {
        let t = text(t).font(font).color(color).single_line().truncate().min_w(0.0);
        match (c.1, c.2) {
            (0.0, _) => row().grow(1.0).min_w(0.0).child(t),
            (w, true) => row().w(w).justify_end().child(t),
            (w, false) => row().w(w).child(t),
        }
    };
    let head = row().w_full().h(22.0).px(6.0).gap(12.0).center_y().children(cols.iter().map(|c| cell(c.0.to_string(), c, p.ink2, pal::SMALL_STRONG)));
    let mut out = col().w_full().child(head).child(el().w_full().h(1.0).fill(p.row_line));
    for (i, r) in rows.into_iter().enumerate() {
        let line = row().w_full().h(26.0).px(6.0).gap(12.0).center_y().children(r.into_iter().zip(cols).map(|(t, c)| {
            let font = if c.2 { pal::BODY_NUM } else { pal::BODY };
            cell(t, c, p.ink, font)
        }));
        out = out.child(cursor_row(line, part, first + i, s, p));
    }
    out
}

fn models(s: &Desk, o: &Overview, p: &Pal) -> El {
    let mut cols = vec![Column("agent", 56.0, false), Column("model", 0.0, false), Column("requests", 70.0, true), Column("prompt known", 86.0, true)];
    cols.extend([
        Column("cache read", 80.0, true),
        Column("cache write", 80.0, true),
        Column("rewrite after miss", 120.0, true),
        Column("uncached input", 100.0, true),
        Column("output", 70.0, true),
        Column("total", 80.0, true),
    ]);
    let rows = o
        .collector
        .spend
        .by_cost()
        .into_iter()
        .map(|((agent, model), m)| {
            let mut row =
                vec![agent.name().to_string(), model.clone(), int(m.requests), format!("{:.0}%", 100.0 * m.with_prompt as f64 / m.requests.max(1) as f64)];
            row.extend(m.by_component.iter().map(|v| if m.priced { usd(v.usd) } else { format!("{} tok", tokens(v.tokens)) }));
            row.push(if m.priced { usd(m.usd()) } else { format!("{} tok", tokens(m.tokens())) });
            row
        })
        .collect();
    table(s, Part::Models, 0, &cols, rows, p)
}

/// An agent's part of a pause card: its name over its table or a line.
fn agent_part(agent: &tokprof_core::source::AgentId, content: El, p: &Pal) -> El {
    col().w_full().gap(6.0).child(text(format!("{} ({})", agent.info().title, agent.name())).font(pal::SMALL_STRONG).color(p.ink2).single_line()).child(content)
}

fn returns(s: &Desk, o: &Overview, p: &Pal) -> El {
    let cols = [
        Column("pause", 0.0, false),
        Column("returns", 60.0, true),
        Column("first request", 90.0, true),
        Column("avoidable", 76.0, true),
        Column("median context", 104.0, true),
    ];
    let mut out = col().w_full().gap(16.0);
    let mut first = 0;
    for (agent, a) in &o.pauses.agents {
        let rows: Vec<Vec<String>> = a
            .return_rows()
            .iter()
            .map(|r| {
                vec![r.name.to_string(), r.count.to_string(), usd(r.usd), usd(r.premium_usd), r.median_context.map_or_else(String::new, |t| tokens(t as f64))]
            })
            .collect();
        let n = rows.len();
        let content = if n == 0 {
            text("No cold returns.").font(pal::BODY).color(p.ink2).single_line()
        } else {
            let (count, usd_first, premium) = a.return_totals();
            col().w_full().gap(6.0).child(table(s, Part::Returns, first, &cols, rows, p)).child(
                text(format!(
                    "{}: {} in first requests, {} of it avoidable.",
                    tokprof_core::fmt::count(count as u64, "cold return", "cold returns"),
                    usd(usd_first),
                    usd(premium)
                ))
                .font(pal::SMALL)
                .color(p.ink2)
                .single_line(),
            )
        };
        first += n;
        out = out.child(agent_part(agent, content, p));
    }
    if o.pauses.agents.is_empty() {
        out = out.child(text("No requests in these logs.").font(pal::BODY).color(p.ink2).single_line());
    }
    out
}

/// The rows of an agent's largest-context table, as `tokprof pauses` prints them.
pub fn size_cells(a: &AgentPauses) -> Vec<Vec<String>> {
    let (spend_usd, spend_tokens) = a.spend();
    let total = if a.priced { spend_usd } else { spend_tokens };
    a.size_rows()
        .iter()
        .map(|r| {
            let part = if a.priced { r.usd } else { r.tokens };
            vec![
                r.name.to_string(),
                r.count.to_string(),
                if a.priced { usd(r.usd) } else { format!("{} tok", tokens(r.tokens)) },
                format!("{:.1}%", 100.0 * part / total.max(1e-9)),
                r.cold_returns.to_string(),
            ]
        })
        .collect()
}

fn sizes(s: &Desk, o: &Overview, p: &Pal) -> El {
    let cols = [
        Column("largest context", 0.0, false),
        Column("sessions", 64.0, true),
        Column("spend", 70.0, true),
        Column("share", 56.0, true),
        Column("cold returns", 84.0, true),
    ];
    let mut out = col().w_full().gap(16.0);
    let mut first = 0;
    for (agent, a) in &o.pauses.agents {
        let rows = size_cells(a);
        let n = rows.len();
        out = out.child(agent_part(agent, table(s, Part::Sizes, first, &cols, rows, p), p));
        first += n;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn axis_steps_are_round_and_fit_three_to_six_lines() {
        for (max, step) in [(0.0, 1.0), (9.0, 5.0), (412.0, 200.0), (1_000.0, 500.0), (37.0, 10.0), (1.2, 0.5)] {
            assert_eq!(axis_step(max), step, "{max}");
            let lines = (max / step).ceil();
            assert!(max == 0.0 || (1.0..=6.0).contains(&lines), "{max}: {lines} lines");
        }
    }

    #[test]
    fn tab_goes_through_the_parts_and_back() {
        let mut p = Part::Period;
        for _ in 0..Part::ORDER.len() {
            p = p.step(false);
        }
        assert_eq!(p, Part::Period);
        assert_eq!(Part::Period.step(true), Part::Sizes);
    }
}
