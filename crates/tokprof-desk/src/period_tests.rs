//! Month and Quarter in the window: the switch and the bar, the days of the month and spend by
//! day, the weeks of the quarter, by project, the cards that read the logs, the records and the
//! cards to share, the keys, the filter and the frames, on a made-up store with a fixed clock:
//! Thursday 12 November 2026 at 15:00.

use tokprof_browse::period::Key as PeriodKey;
use tokprof_core::daystore::{Entry, Minute, Store};
use tokprof_core::month::Ym;
use tokprof_core::period::{Period, Yq};
use tokprof_core::pool::Row as PoolRow;
use tokprof_core::savings::{Changes, SessionCost};
use tokprof_core::xray::Glance;

use super::*;

const MIN: i64 = 60_000;
const HOUR: i64 = 3_600_000;
const OCT: Ym = Ym { year: 2026, month: 10 };
const NOV: Ym = Ym { year: 2026, month: 11 };
const Q4: Yq = Yq { year: 2026, q: 4 };

fn date(s: &str) -> i64 {
    rhythm::date_of(parse_date(s).unwrap() + 12 * HOUR, &rhythm::local_offset)
}

fn at(d: i64, h: i64, m: i64) -> i64 {
    rhythm::midnight(d, &rhythm::local_offset) + h * HOUR + m * MIN
}

/// Thursday 12 November 2026, 15:00.
fn now() -> i64 {
    at(date("2026-11-12"), 15, 0)
}

fn entry(id: &str, project: &str) -> Entry {
    Entry { key: format!("/x/{id}.jsonl"), agent: "claude".into(), session: id.into(), main: id.into(), project: project.into(), ..Entry::default() }
}

fn spend(e: &mut Entry, ts: i64, usd: f64, avoidable: f64) {
    e.minutes.push(Minute { minute: ts / MIN, usd, avoidable_usd: avoidable, requests: 1, read_tokens: 900, prompt_tokens: 1000, ..Minute::default() });
}

/// The store: `id0` in zebrafish from 6 August, a request a day from 1 September; `id1` in heron
/// every third day from 5 October; 9 cold returns of zebrafish on 5 November.
fn store() -> Store {
    let mut z = entry("id0", "~/src/zebrafish");
    let mut h = entry("id1", "~/src/heron");
    spend(&mut z, at(date("2026-08-06"), 10, 0), 5.0, 0.0);
    for d in date("2026-09-01")..=date("2026-11-12") {
        spend(&mut z, at(d, 10, 0), 10.0, 1.0);
        z.actions.push((at(d, 10, 0), true));
        z.actions.push((at(d, 23, 30), true));
    }
    for d in (date("2026-10-05")..=date("2026-11-12")).step_by(3) {
        spend(&mut h, at(d, 14, 0), 20.0, 0.0);
    }
    let nov5 = date("2026-11-05");
    z.cold.extend((0..9).map(|i| at(nov5, 11, i)));
    z.misses.push((at(nov5, 11, 0) / HOUR, "idle > TTL".into(), 9.0));
    z.minutes.sort_by_key(|m| m.minute);
    z.actions.sort_unstable();
    Store { entries: vec![z, h], ..Store::default() }
}

/// What to change with a cause split over seven sessions.
fn changes() -> Changes {
    let saving = |name: &str, usd: f64| PoolRow { name: name.into(), usd, share: usd / 400.0, ..PoolRow::default() };
    let mut c = Changes {
        saved: vec![saving("idle > TTL", 10.0), saving("history edited", 2.0)],
        habits: vec![saving("prompts of 400k+", 20.0)],
        ..Changes::default()
    };
    let agent = tokprof_core::source::AgentId::unknown("claude");
    c.sessions.insert("idle > TTL".into(), (0..7).map(|i| SessionCost { agent: agent.clone(), session: format!("id{i}"), usd: 7.0 - i as f64 }).collect());
    c.hints.insert("idle > TTL".into(), "compact or wrap up a big session before a long break".into());
    c
}

/// A desk of seven listed sessions on the made-up store at the fixed clock.
fn period_desk() -> Desk {
    period_desk_of(store())
}

/// A desk of seven listed sessions on `store` at the fixed clock.
fn period_desk_of(store: Store) -> Desk {
    let mut desk = desk_of(7);
    desk.browser.put_glance("id2", Glance { title: Some("A python helper".into()), ..Glance::default() });
    let cal = tokprof_core::calendar::Calendar::of(&store.entries, &tokprof_core::calendar::Filters::default(), now(), &rhythm::local_offset);
    desk.calendar.data.put_with(cal, store, 0);
    desk.reports.clock = Some(now());
    desk.week.clock = Some(now());
    desk
}

/// Sums the period shown from the store at the fixed clock and puts the fixture's logs in as what
/// its logs gave, with `changes` as What to change, as the workers would.
fn fill(rt: &mut Runtime<Desk>) {
    // The filter text counts once a poll has taken it in.
    settle(rt);
    // Put in through an update, so that the window draws again, as a worker's wake makes it.
    rt.update(|s, _| {
        let key = s.reports.key();
        if s.reports.sums.get(&key).is_none() {
            let store = s.calendar.data.get().and_then(|c| c.store.clone()).expect("the store");
            let sums = tokprof_browse::period::sums_of(&store, &Query::default(), &key, now());
            s.reports.sums.put(key.clone(), sums);
        }
        if s.reports.data.get(&key).is_none() {
            let (catalog, select) = fixture();
            let mut o = tokprof_core::overview::collect(&catalog, &select, &Pricing::defaults(), |_, _| {}).unwrap();
            o.changes = changes();
            let window = key.period.window(now(), &rhythm::local_offset);
            s.reports.data.put(key, o, window, 0);
        }
    });
    settle(rt);
}

fn in_view(c: char, size: (f32, f32)) -> Runtime<Desk> {
    let mut rt = runtime(period_desk(), size);
    command(&mut rt, c);
    fill(&mut rt);
    rt
}

fn shown_date(rt: &Runtime<Desk>) -> i64 {
    rhythm::date_of(rt.state.browser.day_noon(), &rhythm::local_offset)
}

#[test]
fn the_switch_shows_month_and_quarter_and_steps_their_periods() {
    let mut rt = in_view('3', (1500.0, 1200.0));
    assert_eq!(rt.state.reports.mode, reports::Mode::Month, "⌘3 marks Month");
    assert!(inside(&rt, "mode-month", "mode-month").is_some() || rt.presented("mode-month").is_some());
    assert_eq!(text_of(&rt, "period-name"), "November 2026");
    assert_eq!(text_of(&rt, "period-span"), "1–12 Nov so far · vs 1–12 Oct");
    assert_eq!(text_of(&rt, "rep-title"), "November 2026 so far");
    assert_eq!(text_of(&rt, "rep-vs"), "against 1–12 Oct, to 15:00");
    for gone in ["lanes", "details", "day-name", "zoom", "rep-period"] {
        assert!(rt.presented(gone).is_none(), "{gone} is not shown in Month");
    }
    // › never past now; ‹ back to October, which compares with all of September.
    click(&mut rt, "period-on");
    fill(&mut rt);
    assert_eq!(text_of(&rt, "period-name"), "November 2026");
    click(&mut rt, "period-back");
    fill(&mut rt);
    assert_eq!(text_of(&rt, "period-name"), "October 2026");
    assert_eq!(text_of(&rt, "period-span"), "Thu 1 – Sat 31 Oct · vs September");
    // Month → Quarter → Month keeps October.
    command(&mut rt, '4');
    fill(&mut rt);
    assert_eq!(text_of(&rt, "period-name"), "Q4 2026");
    assert_eq!(text_of(&rt, "period-span"), "1 Oct – 12 Nov, 43 of 92 days");
    command(&mut rt, '3');
    fill(&mut rt);
    assert_eq!(text_of(&rt, "period-name"), "October 2026");
    // ‹ never before the store's first period: August.
    for _ in 0..4 {
        click(&mut rt, "period-back");
        fill(&mut rt);
    }
    assert_eq!(text_of(&rt, "period-name"), "August 2026");
    click(&mut rt, "today");
    fill(&mut rt);
    assert_eq!(text_of(&rt, "period-name"), "November 2026");
    // From Quarter alone, Month shows the quarter's last month that has begun.
    command(&mut rt, '4');
    click(&mut rt, "period-back");
    fill(&mut rt);
    assert_eq!(text_of(&rt, "period-name"), "Q3 2026");
    click(&mut rt, "period-back");
    fill(&mut rt);
    assert_eq!(text_of(&rt, "period-name"), "Q3 2026", "not before the quarter of the first day");
    command(&mut rt, '3');
    fill(&mut rt);
    assert_eq!(text_of(&rt, "period-name"), "September 2026");
    command(&mut rt, '5');
    assert_eq!(rt.state.reports.mode, reports::Mode::Calendar, "⌘5 shows Calendar");
}

#[test]
fn the_palette_offers_the_periods_and_last_month_shows_october() {
    let mut rt = in_view('3', (1500.0, 1200.0));
    let titles: Vec<String> = palette::entries(&rt.state, "").into_iter().map(|e| e.title).collect();
    for t in ["Day", "Week", "Sessions", "Quarter", "This month", "Last month", "This quarter", "Last quarter", "Month in review"] {
        assert!(titles.contains(&t.to_string()), "{t} in {titles:?}");
    }
    run_from_palette(&mut rt, "last month", palette::Pick::Act(palette::Action::LastMonth));
    fill(&mut rt);
    assert_eq!((rt.state.reports.mode, rt.state.reports.shown_month()), (reports::Mode::Month, OCT));
    run_from_palette(&mut rt, "last quarter", palette::Pick::Act(palette::Action::LastQuarter));
    fill(&mut rt);
    assert_eq!((rt.state.reports.mode, rt.state.reports.shown_quarter()), (reports::Mode::Quarter, Q4.previous()));
    assert_eq!(palette::Action::Calendar.keys(), ["⌘5"]);
}

#[test]
fn the_days_of_the_month_open_their_dates_and_name_them() {
    let mut rt = in_view('3', (1500.0, 1600.0));
    let nov5 = date("2026-11-05");
    // 9 cold returns: five ticks and `+4`.
    assert_eq!(inside(&rt, &format!("mday-{nov5}"), "mday-ticks").map(|_| ()), Some(()));
    assert_eq!(text_in(&rt, &format!("mday-{nov5}"), "mday-more"), "+4");
    assert_eq!(charts::ticks(9), (5, Some("+4".into())));
    // The legend names the cuts, as the card of the month does.
    let cuts = rt.state.reports.sums().unwrap().span.cuts.expect("cuts");
    assert_eq!(text_of(&rt, "mgrid-cuts"), format!("more · cuts at ${}, ${}, ${}", cuts[0].round(), cuts[1].round(), cuts[2].round()));
    // The pointer over a date names it.
    let nov11 = date("2026-11-11");
    let (x, y) = rt.find(&format!("mday-{nov11}")).unwrap();
    rt.pointer_move(x, y);
    settle(&mut rt);
    assert_eq!(text_of(&rt, "tip-0"), "Wed 11 Nov 2026 · $10 spent, $1.00 avoidable");
    assert_eq!(text_of(&rt, "tip-1"), "click: open this day");
    // A date to come opens nothing; a date opens Day.
    let nov20 = date("2026-11-20");
    click(&mut rt, &format!("mday-{nov20}"));
    assert_eq!(rt.state.reports.mode, reports::Mode::Month, "a date to come opens nothing");
    click(&mut rt, &format!("mday-{nov5}"));
    settle(&mut rt);
    assert_eq!((rt.state.reports.mode, shown_date(&rt)), (reports::Mode::Day, nov5));
}

#[test]
fn spend_by_day_names_the_date_a_bar_faces_and_opens_it() {
    let mut rt = in_view('3', (1500.0, 1600.0));
    let nov3 = date("2026-11-03");
    let (x, y) = rt.find(&format!("mbar-hit-{nov3}")).unwrap();
    rt.pointer_move(x, y);
    settle(&mut rt);
    // Mon 2 Nov faces Mon 5 Oct, so Tue 3 Nov faces Tue 6 Oct.
    assert_eq!(text_of(&rt, "mbar-read"), "Tue 3 Nov · $10 · $9.00 useful, $1.00 avoidable · Tue 6 Oct $10");
    assert!(rt.presented(&format!("mbar-hit-{}", date("2026-11-20"))).is_none(), "a date to come has no bar");
    assert!(rt.presented("mbar-now").is_some(), "the now line");
    click(&mut rt, &format!("mbar-hit-{nov3}"));
    settle(&mut rt);
    assert_eq!((rt.state.reports.mode, shown_date(&rt)), (reports::Mode::Day, nov3));
}

/// The window on `store` in Month on `month`.
fn month_on(store: Store, month: Ym) -> Runtime<Desk> {
    let mut rt = runtime(period_desk_of(store), (1500.0, 1600.0));
    command(&mut rt, '3');
    rt.update(|s, _| s.reports.month = Some(month));
    fill(&mut rt);
    rt
}

/// The line under spend by day with the pointer over the bar of `day`.
fn bar_read(rt: &mut Runtime<Desk>, day: &str) -> String {
    let (x, y) = rt.find(&format!("mbar-hit-{}", date(day))).unwrap();
    rt.pointer_move(x, y);
    settle(rt);
    text_of(rt, "mbar-read")
}

#[test]
fn spend_by_day_draws_the_month_before_only_when_compared() {
    // November compares with October: the line, its legend and its caption.
    let rt = in_view('3', (1500.0, 1600.0));
    assert!(rt.presented("mbar-before").is_some() && rt.presented("mbar-before-label").is_some());
    let all = texts(&rt);
    assert!(all.iter().any(|t| t == "October, by weekday"), "{all:?}");
    assert!(all.iter().any(|t| t == "useful and avoidable · line: October by weekday"));
    // October, whose logs start on 29 Sep: no comparison, so no line, no legend of it, no caption
    // of it, and a bar names no date of September.
    let mut z = entry("id0", "~/src/zebrafish");
    for d in date("2026-09-29")..=date("2026-11-12") {
        spend(&mut z, at(d, 10, 0), 10.0, 1.0);
    }
    let mut rt = month_on(Store { entries: vec![z], ..Store::default() }, OCT);
    assert_eq!(text_of(&rt, "rep-vs"), "no comparison with September: claude's logs start 29 Sep");
    assert!(rt.presented("mbar-before").is_none() && rt.presented("mbar-before-label").is_none());
    let all = texts(&rt);
    assert!(!all.iter().any(|t| t.contains("by weekday")), "{all:?}");
    assert!(all.iter().any(|t| t == "useful and avoidable"));
    assert_eq!(bar_read(&mut rt, "2026-10-06"), "Tue 6 Oct · $10 · $9.00 useful, $1.00 avoidable");
}

#[test]
fn spend_by_day_breaks_the_line_over_the_dates_before_the_logs() {
    // The logs start on 10 Oct and November has no request yet: November compares with October,
    // and its dates that face 4 to 9 Oct have no point.
    let mut h = entry("id1", "~/src/heron");
    for d in date("2026-10-10")..=date("2026-10-31") {
        spend(&mut h, at(d, 10, 0), 4.0, 0.0);
    }
    let mut rt = month_on(Store { entries: vec![h], ..Store::default() }, NOV);
    assert!(rt.state.reports.sums().unwrap().before.numbers().is_some());
    assert!(rt.presented("mbar-before").is_some());
    assert_eq!(bar_read(&mut rt, "2026-11-03"), "Tue 3 Nov · $0.00 · $0.00 useful, $0.00 avoidable · Tue 6 Oct no logs");
    assert_eq!(bar_read(&mut rt, "2026-11-07"), "Sat 7 Nov · $0.00 · $0.00 useful, $0.00 avoidable · Sat 10 Oct $4.00");
}

#[test]
fn a_four_digit_day_fits_its_cell_in_a_narrow_window() {
    let mut z = entry("id0", "~/src/zebrafish");
    for d in date("2026-10-01")..=date("2026-10-31") {
        spend(&mut z, at(d, 10, 0), if d == date("2026-10-25") { 1_221.0 } else { 10.0 }, 0.0);
    }
    for size in [(960.0, 640.0), (1500.0, 950.0)] {
        let mut rt = runtime(period_desk_of(Store { entries: vec![z.clone()], ..Store::default() }), size);
        command(&mut rt, '3');
        rt.update(|s, _| s.reports.month = Some(OCT));
        fill(&mut rt);
        let cell = format!("mday-{}", date("2026-10-25"));
        let outer = rt.elements().into_iter().find(|e| e.key.as_deref() == Some(cell.as_str())).unwrap().rect;
        let amount = inside(&rt, &cell, "mday-spend").unwrap();
        assert_eq!(amount.text.as_deref(), Some("$1,221"));
        assert!(amount.rect[0] + amount.rect[2] <= outer[0] + outer[2] - 1.0, "{size:?}: {:?} in {outer:?}", amount.rect);
    }
}

#[test]
fn the_rest_of_the_quarter_reads_other_and_its_count() {
    let projects = ["zebrafish", "heron", "otter", "lynx", "ibis", "newt", "vole", "wren"];
    let entries = projects
        .iter()
        .enumerate()
        .map(|(i, name)| {
            let mut e = entry(&format!("id{i}"), &format!("~/src/{name}"));
            spend(&mut e, at(date("2026-10-07"), 10, 0), 80.0 - i as f64, 0.0);
            e
        })
        .collect();
    let mut rt = runtime(period_desk_of(Store { entries, ..Store::default() }), (1500.0, 1600.0));
    command(&mut rt, '4');
    fill(&mut rt);
    assert_eq!(text_of(&rt, "qlegend-other"), "other, 2 projects");
}

#[test]
fn a_project_click_sets_the_filter_and_stays_on_the_period() {
    let mut rt = in_view('3', (1500.0, 2400.0));
    let at = rt.state.reports.sums().unwrap().projects.iter().position(|p| p.name == "~/src/zebrafish").unwrap();
    assert_eq!(text_in(&rt, &format!("rep-projects-{at}"), "label"), "zebrafish", "the project's name");
    click(&mut rt, &format!("rep-projects-{at}"));
    fill(&mut rt);
    assert_eq!((rt.state.reports.mode, rt.state.reports.shown_month()), (reports::Mode::Month, NOV));
    assert_eq!(rt.state.browser.filter(), "~/src/zebrafish");
    assert_eq!(text_of(&rt, "rep-title"), "November 2026 so far · ~/src/zebrafish");
    // Every card counts only the project.
    let sums = rt.state.reports.sums().unwrap();
    assert_eq!(sums.projects.len(), 1);
    // Day's lanes and the Sessions table keep the same filter.
    command(&mut rt, '1');
    assert_eq!(rt.state.browser.filter(), "~/src/zebrafish");
    command(&mut rt, '0');
    assert_eq!(rt.state.browser.filter(), "~/src/zebrafish");
}

#[test]
fn the_cards_that_read_the_logs_wait_for_them_then_show_their_rows() {
    let mut rt = runtime(period_desk(), (1500.0, 2600.0));
    command(&mut rt, '3');
    // The store's cards at once, the log cards reading.
    let key = rt.state.reports.key();
    let store = rt.state.calendar.data.get().and_then(|c| c.store.clone()).unwrap();
    rt.update(|s, _| s.reports.sums.put(key.clone(), tokprof_browse::period::sums_of(&store, &Query::default(), &key, now())));
    settle(&mut rt);
    assert!(rt.presented("kpi-spend").is_some() && rt.presented("mgrid").is_some());
    assert!(texts(&rt).iter().any(|t| t == "reading the logs of November 2026…"), "{:?}", texts(&rt));
    // An agent without cold returns says so.
    let (catalog, select) = fixture();
    let mut o = tokprof_core::overview::collect(&catalog, &select, &Pricing::defaults(), |_, _| {}).unwrap();
    o.changes = changes();
    for a in o.pauses.agents.values_mut() {
        a.returns.clear();
    }
    rt.update(|s, _| s.reports.data.put(key, o, (0, now()), 0));
    settle(&mut rt);
    let shown = texts(&rt);
    assert!(shown.iter().any(|t| t == "No cold returns."), "{:?}", shown.iter().filter(|t| t.contains("cold") || t.contains("request")).collect::<Vec<_>>());
    assert!(rt.presented("rep-models-0").is_some() && rt.presented("rep-origins-0").is_some());
}

#[test]
fn a_cause_opens_its_top_five_sessions_and_a_session_opens_in_day() {
    let mut rt = in_view('3', (1500.0, 3200.0));
    assert!(texts(&rt).iter().any(|t| t == "compact or wrap up a big session before a long break"), "the cause's advice");
    let part = reports::Part::Saved;
    click(&mut rt, &format!("{}-0", part.key()));
    settle(&mut rt);
    assert_eq!(rt.state.reports.open, Some((part, "idle > TTL".into())));
    let c = rt.state.reports.changes().unwrap().clone();
    assert_eq!(changes::lines(&c, part, rt.state.reports.open.as_ref()).len(), 1 + 5 + 1, "five of its seven sessions, then the next row");
    assert_eq!(text_in(&rt, &format!("{}-6", part.key()), "label"), "history edited");
    click(&mut rt, &format!("{}-2", part.key()));
    settle(&mut rt);
    assert_eq!(rt.state.reports.mode, reports::Mode::Day);
    assert_eq!(rt.state.current().map(|r| rt.state.browser.session(r).to_string()).as_deref(), Some("id1"));
}

#[test]
fn month_in_review_opens_from_month_and_keeps_the_keys() {
    let mut rt = in_view('3', (1500.0, 3200.0));
    assert_eq!(text_of(&rt, "review-ready"), "November's card is ready on Tue 1 Dec.");
    typed(&mut rt, "r");
    assert_eq!(rt.state.month.card.as_ref().and_then(|c| c.month.ym), Some(OCT), "r on November opens October");
    // The keys stay with the card.
    typed(&mut rt, "qm");
    assert!(rt.state.month.card.is_some() && rt.shell_request.is_none());
    press(&mut rt, RuneKey::Escape);
    assert!(rt.state.month.card.is_none());
    // September's own card.
    rt.update(|s, _| s.reports.month = Some(Ym { year: 2026, month: 9 }));
    fill(&mut rt);
    click(&mut rt, "review-open");
    assert_eq!(rt.state.month.card.as_ref().and_then(|c| c.month.ym), Some(Ym { year: 2026, month: 9 }));
}

#[test]
fn the_weeks_of_the_quarter_open_week_month_and_set_the_filter() {
    let mut rt = in_view('4', (1500.0, 1800.0));
    let sums = rt.state.reports.sums().unwrap().clone();
    assert_eq!(sums.weeks.len(), 14);
    assert_eq!(text_of(&rt, "qweek-cut-0"), "Thu–Sun");
    // A column opens Week on its week.
    click(&mut rt, "qweek-hit-3");
    settle(&mut rt);
    assert_eq!((rt.state.reports.mode, week::monday(&rt.state)), (reports::Mode::Week, sums.weeks[3].monday));
    // A trend point opens Week too.
    command(&mut rt, '4');
    fill(&mut rt);
    click(&mut rt, "qtrend-hit-0-5");
    settle(&mut rt);
    assert_eq!((rt.state.reports.mode, week::monday(&rt.state)), (reports::Mode::Week, sums.weeks[5].monday));
    // A month's name under the weeks, and its row in Months, open Month.
    command(&mut rt, '4');
    fill(&mut rt);
    click(&mut rt, "qmonth-1");
    settle(&mut rt);
    assert_eq!((rt.state.reports.mode, rt.state.reports.shown_month(), rt.state.reports.week), (reports::Mode::Month, NOV, None));
    command(&mut rt, '4');
    fill(&mut rt);
    click(&mut rt, "rep-months-0");
    settle(&mut rt);
    assert_eq!((rt.state.reports.mode, rt.state.reports.shown_month()), (reports::Mode::Month, OCT));
    // A project in the legend sets the filter and stays on the quarter.
    command(&mut rt, '4');
    fill(&mut rt);
    let j = sums.stacked().0.iter().position(|p| p.name == "~/src/heron").unwrap();
    click(&mut rt, &format!("qlegend-{j}"));
    fill(&mut rt);
    assert_eq!((rt.state.reports.mode, rt.state.browser.filter()), (reports::Mode::Quarter, "~/src/heron"));
}

#[test]
fn the_quarter_shares_the_cards_of_its_months() {
    let mut rt = in_view('4', (1500.0, 2600.0));
    let label = |rt: &Runtime<Desk>, i: usize| rt.elements().into_iter().find(|e| e.key.as_deref() == Some(&format!("share-{i}"))).and_then(|e| e.label);
    assert_eq!(label(&rt, 0).as_deref(), Some("Open October"));
    assert_eq!(label(&rt, 1).as_deref(), Some("November so far"));
    assert_eq!(label(&rt, 2).as_deref(), Some("December"));
    click(&mut rt, "share-2");
    assert!(rt.state.month.card.is_none(), "December is disabled in November");
    click(&mut rt, "share-0");
    assert_eq!(rt.state.month.card.as_ref().and_then(|c| c.month.ym), Some(OCT));
    // The records of the quarter: the largest prompt from the logs.
    assert!(texts(&rt).iter().any(|t| t == "Largest prompt"));
}

#[test]
fn keys_step_the_period_move_the_cursors_and_step_back_with_esc() {
    let mut rt = in_view('3', (1500.0, 1200.0));
    // The page has the keyboard: ← steps the month, and back.
    press(&mut rt, RuneKey::Left);
    fill(&mut rt);
    assert_eq!(rt.state.reports.shown_month(), OCT);
    press(&mut rt, RuneKey::Text("]".into()));
    fill(&mut rt);
    assert_eq!(rt.state.reports.shown_month(), NOV);
    // Tab gives the days the keyboard, the cursor on today; ← twice and ⏎ open 10 November.
    press(&mut rt, RuneKey::Tab);
    assert_eq!(rt.state.reports.part, Some(reports::Part::Days));
    press(&mut rt, RuneKey::Left);
    press(&mut rt, RuneKey::Left);
    press(&mut rt, RuneKey::Enter);
    assert_eq!((rt.state.reports.mode, shown_date(&rt)), (reports::Mode::Day, date("2026-11-10")));
    // q does nothing, ⌘0 shows Sessions, `t` the current period.
    command(&mut rt, '3');
    fill(&mut rt);
    typed(&mut rt, "qsd");
    assert!(rt.shell_request.is_none() && rt.state.reports.mode == reports::Mode::Month);
    // Esc in steps: by project with the keyboard and a filter set.
    rt.update(|s, _| {
        s.browser.set_filter("zebra");
        s.reports.part = Some(reports::Part::Projects);
    });
    fill(&mut rt);
    press(&mut rt, RuneKey::Escape);
    assert_eq!((rt.state.reports.part, rt.state.browser.filter()), (None, "zebra"));
    press(&mut rt, RuneKey::Escape);
    assert_eq!(rt.state.browser.filter(), "");
    press(&mut rt, RuneKey::Escape);
    assert_eq!(rt.state.reports.mode, reports::Mode::Day);
    command(&mut rt, '4');
    command(&mut rt, '0');
    assert_eq!(rt.state.reports.mode, reports::Mode::Sessions, "⌘0 from Quarter");
    // Ctrl+C quits from Month, and `?` opens the key help.
    command(&mut rt, '3');
    press(&mut rt, RuneKey::Text("?".into()));
    assert!(rt.state.help);
    press(&mut rt, RuneKey::Escape);
    rt.key(KeyEvent { key: RuneKey::Text("c".into()), mods: rune::Mods { ctrl: true, ..rune::Mods::default() } });
    rt.frame(1.0 / 60.0);
    assert!(rt.shell_request.is_some());
}

#[test]
fn the_weeks_take_the_keyboard_and_enter_opens_the_week() {
    let mut rt = in_view('4', (1500.0, 1600.0));
    press(&mut rt, RuneKey::Tab);
    assert_eq!(rt.state.reports.part, Some(reports::Part::Weeks));
    let sums = rt.state.reports.sums().unwrap().clone();
    press(&mut rt, RuneKey::Left);
    press(&mut rt, RuneKey::Enter);
    assert_eq!((rt.state.reports.mode, week::monday(&rt.state)), (reports::Mode::Week, sums.weeks[5].monday), "the week before the current one");
}

#[test]
fn the_filter_counts_the_period_and_says_what_it_matches() {
    let mut rt = in_view('3', (1500.0, 1200.0));
    rt.update(|s, _| s.browser.set_filter("zzz"));
    fill(&mut rt);
    assert_eq!(text_of(&rt, "rep-nothing"), "Nothing in November 2026 matches \"zzz\".");
    rt.update(|s, _| s.browser.set_filter("python"));
    fill(&mut rt);
    assert_eq!(
        text_of(&rt, "rep-titles"),
        "1 session matches \"python\" only by title; Month counts agents, projects and session ids, not titles. Sessions (⌘0) lists them."
    );
    // `/` starts the filter, esc gives it up.
    rt.update(|s, _| s.browser.clear_filter());
    typed(&mut rt, "/");
    assert!(rt.state.browser.editing());
    press(&mut rt, RuneKey::Escape);
    assert!(!rt.state.browser.editing() && rt.state.reports.mode == reports::Mode::Month);
    // With --client, the store's numbers say they count every client.
    rt.update(|s, _| s.reports.client_ignored = true);
    settle(&mut rt);
    assert_eq!(text_of(&rt, "rep-client"), "the key numbers and charts count every client: the day store has no client");
}

#[test]
fn the_flags_of_the_frames_fail_as_they_should() {
    let err = |args: &[&str]| {
        let cli = Cli::parse_from(std::iter::once("tokprof-desk").chain(args.iter().copied()));
        match (month_of(&cli), quarter_of(&cli)) {
            (Err(e), _) | (_, Err(e)) => e.to_string(),
            _ => String::new(),
        }
    };
    assert_eq!(err(&["--quarter", "2026-Q5"]), "--quarter takes YYYY-Qn");
    assert_eq!(err(&["--quarter", "2026-q4x"]), "--quarter takes YYYY-Qn");
    assert_eq!(err(&["--month", "october"]), "--month takes YYYY-MM");
    assert_eq!(err(&["--month", "2099-05"]), "May 2099 has not begun");
    assert_eq!(err(&["--quarter", "2099-Q2"]), "Q2 2099 has not begun");
    assert_eq!(err(&["--quarter", "2026-Q4", "--month", "2026-10"]), "");
    let names: Vec<&str> = SCENES.iter().map(|s| s.0).collect();
    for gone in ["reports", "reports-advice", "reports-week"] {
        assert!(!names.contains(&gone), "{gone} is gone");
    }
}

/// Runs a scene on the made-up desk at the fixed clock, its period filled in first.
fn scene(name: &str, month: Option<Ym>, size: (f32, f32), dark: bool) -> Runtime<Desk> {
    let mut desk = period_desk();
    desk.month.wanted = month;
    desk.quarter_wanted = Some(Q4);
    let mut rt = Runtime::new(desk, view::view, size, Env::new(dark, 1.0));
    rt.poll = Some(poll);
    rt.on_key = Some(on_key);
    // The scene waits for the workers; here they are filled in before it runs.
    rt.update(|s, _| {
        reports::show(s, if name == "quarter" { reports::Mode::Quarter } else { reports::Mode::Month });
        if let Some(m) = month {
            s.reports.month = Some(m);
        }
    });
    fill(&mut rt);
    let run = SCENES.iter().find(|s| s.0 == name).unwrap_or_else(|| panic!("no scene {name}")).1;
    rt.update(|s, _| run(s));
    settle(&mut rt);
    rt
}

/// Texts that run past the window's right edge.
fn cut_off(rt: &Runtime<Desk>, width: f32) -> Vec<String> {
    rt.elements().into_iter().filter(|e| e.text.is_some() && e.rect[0] + e.rect[2] > width + 0.5 && e.rect[2] > 0.0).filter_map(|e| e.text).collect()
}

#[test]
fn the_frames_of_month_and_quarter() {
    for dark in [false, true] {
        for size in [(1500.0, 950.0), (960.0, 640.0)] {
            let rt = scene("month", Some(OCT), size, dark);
            assert_eq!(text_of(&rt, "period-name"), "October 2026");
            assert!(rt.presented("rep-band").is_some() && rt.presented("mgrid").is_some(), "{size:?}");
            assert_eq!(cut_off(&rt, size.0), Vec::<String>::new(), "October at {size:?}");
            let rt = scene("month", None, size, dark);
            assert_eq!(text_of(&rt, "rep-title"), "November 2026 so far");
            assert_eq!(cut_off(&rt, size.0), Vec::<String>::new(), "November at {size:?}");
            let rt = scene("quarter", None, size, dark);
            assert_eq!(text_of(&rt, "period-name"), "Q4 2026");
            assert!(rt.presented("qweek-hit-0").is_some());
            assert_eq!(cut_off(&rt, size.0), Vec::<String>::new(), "Q4 at {size:?}");
        }
    }
    let rt = scene("month-advice", Some(OCT), (1500.0, 950.0), false);
    assert_eq!(rt.state.reports.open, Some((reports::Part::Saved, "idle > TTL".into())), "the first cause with sessions is open");
    let rt = scene("month-review", Some(NOV), (1500.0, 950.0), true);
    assert_eq!((rt.state.reports.mode, rt.state.month.card.as_ref().and_then(|c| c.month.ym)), (reports::Mode::Month, Some(OCT)));
    assert!(rt.presented("month-card").is_some());
}

#[test]
fn the_key_help_has_the_keys_of_a_period() {
    let mut rt = in_view('4', (1400.0, 900.0));
    press(&mut rt, RuneKey::Text("?".into()));
    assert!(rt.state.help);
    let of: Vec<&str> = keys::GROUPS.iter().find(|g| g.0 == "Month and Quarter").unwrap().1.iter().flat_map(|r| r.keys.iter().copied()).collect();
    for k in ["←", "→", "[", "]", "t", "Tab", "⏎", "/", "r", "esc"] {
        assert!(of.contains(&k), "Month and Quarter names {k}");
    }
    let _ = PeriodKey::new(Period::Month(NOV), "");
}

#[test]
fn month_and_quarter_draw_no_frames_at_rest() {
    for c in ['3', '4'] {
        let mut rt = in_view(c, (1500.0, 1200.0));
        // The card's image is drawn on a worker; once it is in, nothing changes.
        let deadline = Instant::now() + Duration::from_secs(20);
        while rt.state.month.images.is_empty() || !rt.state.month.drawing.is_empty() {
            assert!(Instant::now() < deadline, "the image did not arrive");
            std::thread::sleep(Duration::from_millis(20));
            rt.frame(1.0 / 60.0);
        }
        settle(&mut rt);
        assert!(rt.presented("rep-image").is_some());
        assert!(!rt.update_if(|s, tx| poll(s, 0.0, tx)), "no more frames while nothing changes in {c}");
        assert!(!rt.animating);
    }
}

#[test]
fn the_period_s_logs_are_read_only_while_month_or_quarter_shows() {
    let (catalog, select) = fixture();
    let mut desk = period_desk();
    desk.reports.data = tokprof_browse::reports::Reports::start(catalog, Pricing::defaults(), select, Arc::new(|| {}));
    let mut rt = runtime(desk, (1400.0, 900.0));
    assert!(rt.state.reports.data.progress().is_none() && rt.state.reports.report().is_none(), "Day reads nothing for them");
    command(&mut rt, '3');
    let deadline = Instant::now() + Duration::from_secs(20);
    while rt.state.reports.report().is_none() {
        assert!(Instant::now() < deadline, "the period's logs were not read");
        std::thread::sleep(Duration::from_millis(20));
        rt.frame(1.0 / 60.0);
    }
    assert_eq!(rt.state.reports.report().map(|r| r.since_ms), Some(Period::Month(NOV).bounds(&rhythm::local_offset).0), "the calendar month's window");
}
