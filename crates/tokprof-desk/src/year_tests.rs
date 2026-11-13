//! Year in the window: the switch and the bar, the grid with its days without logs and to come,
//! the month bars, the cards, the records, the keys, Year in review and the frames, on a made-up
//! store with a fixed clock: Thursday 12 November 2026 at 15:00.

use tokprof_core::calendar::Metric;
use tokprof_core::daystore::{Entry, Minute, Store};
use tokprof_core::month::{Month, Ym};
use tokprof_core::period::Period;

use super::*;

const MIN: i64 = 60_000;
const HOUR: i64 = 3_600_000;

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

/// The store: `id0` in zebrafish with a request and two messages a day from `from` to 12 Nov,
/// 175 commits on Wed 11 Nov and $1,211 more on Sun 25 Oct; `id1` in heron every third day from
/// 5 Oct; an idle miss of zebrafish each day in October.
fn store_from(from: &str) -> Store {
    let mut z = entry("id0", "~/src/zebrafish");
    let mut h = entry("id1", "~/src/heron");
    for d in date(from)..=date("2026-11-12") {
        z.minutes.push(Minute {
            minute: at(d, 10, 0) / MIN,
            usd: 10.0,
            avoidable_usd: 1.0,
            requests: 1,
            commits: if d == date("2026-11-11") { 175 } else { 0 },
            ..Minute::default()
        });
        z.actions.push((at(d, 10, 0), true));
        z.actions.push((at(d, 12, 0), true));
        z.work.push((at(d, 10, 0) / HOUR, 0b1111));
    }
    z.minutes.push(Minute { minute: at(date("2026-10-25"), 11, 0) / MIN, usd: 1_211.0, requests: 1, ..Minute::default() });
    for d in date("2026-10-01")..=date("2026-10-31") {
        z.misses.push((at(d, 10, 0) / HOUR, "idle > TTL".into(), 1.0));
    }
    z.models.push((at(date("2026-10-25"), 11, 0) / HOUR, "claude-opus-5-5".into(), 1_211.0));
    for d in (date("2026-10-05")..=date("2026-11-12")).step_by(3) {
        h.minutes.push(Minute { minute: at(d, 14, 0) / MIN, usd: 20.0, requests: 1, ..Minute::default() });
    }
    z.minutes.sort_by_key(|m| m.minute);
    z.actions.sort_unstable();
    Store { entries: vec![z, h], ..Store::default() }
}

fn store() -> Store {
    store_from("2026-08-06")
}

/// A desk of seven listed sessions on `store` at the fixed clock.
fn year_desk_of(store: Store) -> Desk {
    let mut desk = desk_of(7);
    let cal = tokprof_core::calendar::Calendar::of(&store.entries, &tokprof_core::calendar::Filters::default(), now(), &rhythm::local_offset);
    desk.calendar.data.put_with(cal, store, 0);
    desk.reports.clock = Some(now());
    desk.week.clock = Some(now());
    desk
}

/// Sums the year shown and computes its card from the store at the fixed clock, as the workers
/// would.
fn fill(rt: &mut Runtime<Desk>) {
    settle(rt);
    rt.update(|s, _| {
        let key = s.reports.key();
        if s.reports.sums.get(&key).is_none() {
            let store = s.calendar.data.get().and_then(|c| c.store.clone()).expect("the store");
            let sums = tokprof_browse::period::sums_of(&store, &Query::default(), &key, now());
            s.reports.sums.put(key, sums);
        }
        year::keep_changes(s);
        let y = s.reports.shown_year();
        if !s.year.years.contains_key(&y) {
            year::settle_cards(s, &[y]);
        }
    });
    settle(rt);
}

fn in_year(store: Store, size: (f32, f32)) -> Runtime<Desk> {
    let mut rt = runtime(year_desk_of(store), size);
    command(&mut rt, '5');
    fill(&mut rt);
    rt
}

fn text_of(rt: &Runtime<Desk>, key: &str) -> String {
    rt.elements().into_iter().find(|e| e.key.as_deref() == Some(key)).and_then(|e| e.text).unwrap_or_default()
}

fn sums(rt: &Runtime<Desk>) -> Arc<tokprof_core::period::PeriodSums> {
    rt.state.reports.sums().cloned().expect("the year's sums")
}

#[test]
fn command_5_and_the_switch_show_year_and_esc_shows_day() {
    let mut rt = in_year(store(), (1500.0, 1200.0));
    assert_eq!(rt.state.reports.mode, reports::Mode::Year, "⌘5 shows Year");
    assert!(rt.presented("mode-year").is_some() && rt.presented("mode-calendar").is_none(), "no Calendar in the switch");
    assert!(rt.presented(reports::PANE).is_some() && rt.presented("year-grid").is_some());
    for gone in ["lanes", "details", "day-name", "zoom"] {
        assert!(rt.presented(gone).is_none(), "{gone} is not shown in Year");
    }
    assert_eq!(text_of(&rt, "period-name"), "2026");
    assert_eq!(text_of(&rt, "period-span"), "logs since 6 Aug · 99 days");
    assert_eq!(text_of(&rt, "rep-title"), "2026 so far");
    assert_eq!(text_of(&rt, "rep-vs"), "no logs in 2025");
    typed(&mut rt, "q");
    assert_eq!(rt.shell_request, None, "q does not quit from Year");
    press(&mut rt, RuneKey::Escape);
    assert_eq!(rt.state.reports.mode, reports::Mode::Day, "esc shows Day");
    click(&mut rt, "mode-year");
    settle(&mut rt);
    assert_eq!(rt.state.reports.mode, reports::Mode::Year);
    command(&mut rt, '1');
    assert_eq!(rt.state.reports.mode, reports::Mode::Day);
    command(&mut rt, '5');
    command(&mut rt, '0');
    assert_eq!(rt.state.reports.mode, reports::Mode::Sessions, "⌘0 from Year shows Sessions");
}

#[test]
fn the_year_in_the_bar_never_goes_past_this_year_or_before_the_logs() {
    // Logs from June 2025: 2025 can be shown, 2024 cannot.
    let mut s = store();
    s.entries[1].minutes.insert(0, Minute { minute: at(date("2025-06-01"), 12, 0) / MIN, usd: 5.0, requests: 1, ..Minute::default() });
    let mut rt = in_year(s, (1500.0, 1200.0));
    click(&mut rt, "period-on");
    fill(&mut rt);
    assert_eq!(text_of(&rt, "period-name"), "2026", "never past this year");
    assert_eq!(text_of(&rt, "period-span"), "316 of 365 days", "a later year in progress");
    press(&mut rt, RuneKey::Left);
    fill(&mut rt);
    assert_eq!(text_of(&rt, "period-name"), "2025", "← steps the year while no card has the keyboard");
    assert_eq!(text_of(&rt, "period-span"), "", "nothing for a past year");
    press(&mut rt, RuneKey::Text("[".into()));
    fill(&mut rt);
    assert_eq!(text_of(&rt, "period-name"), "2025", "never before the year the logs begin");
    press(&mut rt, RuneKey::Text("]".into()));
    fill(&mut rt);
    assert_eq!(text_of(&rt, "period-name"), "2026");
    press(&mut rt, RuneKey::Left);
    typed(&mut rt, "t");
    fill(&mut rt);
    assert_eq!(text_of(&rt, "period-name"), "2026", "t shows this year");
    press(&mut rt, RuneKey::Left);
    click(&mut rt, "today");
    fill(&mut rt);
    assert_eq!(text_of(&rt, "period-name"), "2026", "Today shows this year");
}

#[test]
fn the_grid_has_53_or_54_columns_days_without_logs_and_days_to_come() {
    let rt = in_year(store(), (1500.0, 1200.0));
    let x = sums(&rt);
    assert_eq!(year::columns(&x).len(), 53);
    let y2040 = tokprof_browse::period::sums_of(&Store::default(), &Query::default(), &tokprof_browse::period::Key::new(Period::Year(2040), ""), now());
    assert_eq!(year::columns(&y2040).len(), 54, "a leap year that starts on a Sunday");
    assert!(rt.presented(&format!("year-nologs-{}", date("2026-08-05"))).is_some(), "5 Aug has no logs");
    assert!(rt.presented(&format!("year-day-{}", date("2026-08-06"))).is_some(), "the logs begin on 6 Aug");
    assert!(rt.presented(&format!("year-tocome-{}", date("2026-11-13"))).is_some(), "13 Nov is to come");
    assert!(rt.presented(&format!("year-day-{}", date("2026-11-13"))).is_none());
    assert_eq!(text_of(&rt, "year-quiet-text"), "Logs since 6 Aug. January to July have no logs, so these days are unknown, not idle.");
    assert_eq!(text_in(&rt, "year-active", "value"), "99 of 99", "never of the year's 316 days");
    for key in ["year-streaks", "year-busiest", "year-kept"] {
        assert!(rt.presented(key).is_some(), "{key} stands under the grid");
    }
    assert!(rt.presented("year-total").is_none(), "the total is the Spend tile's, not repeated under the grid");
    assert!(rt.presented("year-weekday").is_none(), "the busiest weekday is the Rhythm card's, not repeated under the grid");
    assert!(rt.presented("year-today").is_some());
}

#[test]
fn a_day_without_logs_has_no_tooltip_and_a_day_with_logs_has_one() {
    let mut rt = in_year(store(), (1500.0, 1200.0));
    rt.update(|s, _| s.reports.hover = Some(reports::Hover::Cell(date("2026-03-01"))));
    settle(&mut rt);
    assert!(rt.presented("year-tip").is_none(), "no tooltip on a day without logs");
    rt.update(|s, _| s.reports.hover = Some(reports::Hover::Cell(date("2026-12-01"))));
    settle(&mut rt);
    assert!(rt.presented("year-tip").is_none(), "nor on a day to come");
    rt.update(|s, _| s.reports.hover = Some(reports::Hover::Cell(date("2026-11-05"))));
    settle(&mut rt);
    assert_eq!(text_in(&rt, "year-tip", "tip-0"), "Thu 5 Nov 2026");
    assert_eq!(text_in(&rt, "year-tip", "tip-1"), "$10.00 spent, $1.00 of it avoidable");
    click(&mut rt, &format!("year-day-{}", date("2026-11-05")));
    settle(&mut rt);
    assert_eq!(rt.state.reports.mode, reports::Mode::Day);
    assert_eq!(rhythm::date_of(rt.state.browser.day_noon(), &rhythm::local_offset), date("2026-11-05"));
}

#[test]
fn the_month_bars_are_the_core_s_months_and_open_month() {
    let mut rt = in_year(store(), (1500.0, 1200.0));
    let x = sums(&rt);
    let store = store();
    for m in [Metric::Spend, Metric::Commits, Metric::Messages] {
        let bars = year::month_values(&x, m, now());
        for (i, ym) in Period::Year(2026).months().into_iter().enumerate() {
            let want = (ym.month >= 8 && ym.month <= 11)
                .then(|| Month::of(&store.entries, &tokprof_core::calendar::Filters::default(), ym, now(), &rhythm::local_offset).total.value(m));
            assert_eq!(bars[i], want, "{m:?} {ym:?}");
        }
    }
    assert_eq!(text_of(&rt, "year-month-amount-9"), "$1,701", "October");
    assert_eq!(text_of(&rt, "year-month-tag-7"), "from 6 Aug");
    assert_eq!(text_of(&rt, "year-month-tag-10"), "so far");
    assert_eq!(text_of(&rt, "year-month-tag-11"), "not begun");
    assert_eq!(text_of(&rt, "year-months-nologs"), "no logs from January to 5 August");
    assert!(rt.presented("year-month-6").is_none(), "July has no bar");
    click(&mut rt, "metric-commits");
    settle(&mut rt);
    assert_eq!(text_of(&rt, "year-month-amount-10"), "175", "the bars follow the pills");
    rt.update(|s, _| s.reports.hover = Some(reports::Hover::Month(9)));
    settle(&mut rt);
    assert_eq!(text_in(&rt, "year-month-tip", "tip-0"), "October 2026 · 0 commits");
    assert_eq!(text_in(&rt, "year-month-tip", "tip-1"), "click: open Month");
    click(&mut rt, "year-month-9");
    settle(&mut rt);
    assert_eq!((rt.state.reports.mode, rt.state.reports.shown_month()), (reports::Mode::Month, Ym { year: 2026, month: 10 }));
}

#[test]
fn the_key_numbers_the_rhythm_agents_and_what_to_change() {
    let mut rt = in_year(store(), (1500.0, 2400.0));
    let x = sums(&rt);
    assert_eq!(text_in(&rt, "kpi-spend", "value"), reports::usd_whole(x.key.usd));
    assert_eq!(text_in(&rt, "kpi-spend", "change"), format!("{} a day", reports::usd_whole(x.key.usd / 99.0)), "the pace over the 99 days of logs");
    assert_eq!(text_in(&rt, "year-rhythm-2", "record-value"), "Sunday", "the spend of 25 Oct makes Sundays the busiest");
    assert_eq!(text_in(&rt, "year-agent-0", "label"), "claude");
    assert_eq!(text_in(&rt, "year-model-0", "label"), "claude-opus-5-5");
    // What to change from the store: idle > TTL with its months, and its session behind it.
    assert!(texts(&rt).contains(&"Oct $31".to_string()), "{:?}", texts(&rt));
    click(&mut rt, "rep-saved-0");
    settle(&mut rt);
    assert_eq!(rt.state.reports.open, Some((reports::Part::Saved, "idle > TTL".into())));
    click(&mut rt, "rep-saved-1");
    settle(&mut rt);
    assert_eq!(rt.state.reports.mode, reports::Mode::Day, "a session behind a row opens in Day");
    assert_eq!(rt.state.current().map(|r| rt.state.browser.session(r).to_string()), Some("id0".to_string()));
}

#[test]
fn a_project_click_keeps_the_year_and_filters_every_card() {
    let mut rt = in_year(store(), (1500.0, 2400.0));
    let all = sums(&rt).key.usd;
    click(&mut rt, "rep-projects-1");
    fill(&mut rt);
    assert_eq!(rt.state.browser.filter(), "~/src/heron");
    assert_eq!((rt.state.reports.mode, rt.state.reports.shown_year()), (reports::Mode::Year, 2026));
    let x = sums(&rt);
    assert!(x.key.usd < all && x.key.usd > 0.0);
    assert_eq!(text_in(&rt, "kpi-spend", "value"), reports::usd_whole(x.key.usd));
    assert_eq!(x.cards.as_ref().map(|c| c.agents.len()), Some(1));
    assert!(x.cards.as_ref().is_some_and(|c| c.causes.is_empty()), "heron has no miss");
    assert_eq!(text_in(&rt, "year-active", "value"), "13 of 99", "the days before heron's first request still have logs");
    press(&mut rt, RuneKey::Escape);
    assert!(rt.state.browser.filter().is_empty(), "esc clears the filter first");
    assert_eq!(rt.state.reports.mode, reports::Mode::Year);
}

#[test]
fn the_records_open_their_day_and_week() {
    let mut rt = in_year(store(), (1500.0, 2400.0));
    let rec = reports::records(&rt.state);
    let labels: Vec<&str> = rec.iter().map(|r| r.label).collect();
    assert_eq!(labels, ["Biggest day", "Biggest week", "Longest streak", "Most commits in a day", "Most at once"]);
    assert_eq!((rec[0].value.as_str(), rec[0].sub.as_str()), ("$1,221", "Sun 25 Oct"));
    assert_eq!((rec[2].value.as_str(), rec[2].sub.as_str()), ("99 days", "since Thu 6 Aug, still going"));
    assert_eq!((rec[3].value.as_str(), rec[3].sub.as_str()), ("175", "Wed 11 Nov"));
    assert!(!rec.iter().any(|r| r.label.contains("hour")), "the busiest hour is in the rhythm only");
    click(&mut rt, "rep-records-1");
    settle(&mut rt);
    assert_eq!(rt.state.reports.mode, reports::Mode::Week);
    assert_eq!(rt.state.week.monday, Some(date("2026-10-19")));
    command(&mut rt, '5');
    fill(&mut rt);
    click(&mut rt, "rep-records-3");
    settle(&mut rt);
    assert_eq!((rt.state.reports.mode, rhythm::date_of(rt.state.browser.day_noon(), &rhythm::local_offset)), (reports::Mode::Day, date("2026-11-11")));
}

#[test]
fn the_keyboard_in_year() {
    let mut rt = in_year(store(), (1500.0, 2400.0));
    // m and ⇧M pick the metric anywhere.
    typed(&mut rt, "mm");
    assert_eq!(rt.state.year.metric, Metric::Keyboard);
    typed(&mut rt, "M");
    assert_eq!(rt.state.year.metric, Metric::Agent);
    // Tab: the grid, the months, By project, What to change, the records.
    let mut parts = Vec::new();
    for _ in 0..5 {
        press(&mut rt, RuneKey::Tab);
        parts.push(rt.state.reports.part);
    }
    use reports::Part::*;
    assert_eq!(parts, [Some(Grid), Some(Months), Some(Projects), Some(Saved), Some(Records)]);
    // The grid: the last day, a week back and ⏎ opens it.
    press(&mut rt, RuneKey::Tab);
    assert_eq!(rt.state.reports.part, Some(Grid));
    press(&mut rt, RuneKey::Down);
    assert_eq!(rt.state.reports.day, Some(date("2026-11-12")), "never past the last day");
    press(&mut rt, RuneKey::Home);
    press(&mut rt, RuneKey::Up);
    press(&mut rt, RuneKey::Up);
    assert_eq!(rt.state.reports.day, Some(date("2026-08-06")), "nor into the days without logs");
    press(&mut rt, RuneKey::End);
    press(&mut rt, RuneKey::Left);
    assert!(rt.presented("year-tip").is_some(), "the cursor's day shows its tooltip");
    assert_eq!(rt.state.reports.mode, reports::Mode::Year, "← moves the cursor, not the year");
    press(&mut rt, RuneKey::Enter);
    assert_eq!((rt.state.reports.mode, rhythm::date_of(rt.state.browser.day_noon(), &rhythm::local_offset)), (reports::Mode::Day, date("2026-11-05")));
    // The months: on this month, ← and ⏎ open the month before.
    command(&mut rt, '5');
    fill(&mut rt);
    press(&mut rt, RuneKey::Tab);
    press(&mut rt, RuneKey::Tab);
    assert_eq!((rt.state.reports.part, rt.state.reports.row), (Some(Months), 10));
    press(&mut rt, RuneKey::Left);
    press(&mut rt, RuneKey::Enter);
    assert_eq!((rt.state.reports.mode, rt.state.reports.shown_month()), (reports::Mode::Month, Ym { year: 2026, month: 10 }));
    // esc steps back: the page, then Day.
    command(&mut rt, '5');
    fill(&mut rt);
    press(&mut rt, RuneKey::Tab);
    press(&mut rt, RuneKey::Escape);
    assert_eq!((rt.state.reports.mode, rt.state.reports.part), (reports::Mode::Year, None));
    press(&mut rt, RuneKey::Escape);
    assert_eq!(rt.state.reports.mode, reports::Mode::Day);
}

#[test]
fn the_key_help_and_the_palette_have_year() {
    let mut rt = in_year(store(), (1400.0, 900.0));
    let groups: Vec<&str> = keys::GROUPS.iter().map(|g| g.0).collect();
    assert!(groups.contains(&"Year") && !groups.contains(&"Calendar") && !groups.contains(&"Reports"));
    let of: Vec<&str> = keys::GROUPS.iter().find(|g| g.0 == "Year").unwrap().1.iter().flat_map(|r| r.keys.iter().copied()).collect();
    for k in ["↑", "↓", "←", "→", "Home", "End", "m", "⇧M", "r", "s", "⏎", "t", "Tab", "esc", "[", "]"] {
        assert!(of.contains(&k), "Year names {k}");
    }
    press(&mut rt, RuneKey::Text("?".into()));
    assert!(rt.state.help, "? opens the key help in Year");
    press(&mut rt, RuneKey::Escape);
    let titles: Vec<String> = palette::entries(&rt.state, "year").into_iter().map(|e| e.title).collect();
    for t in ["Year: messages", "Year: commits", "Year in review"] {
        assert!(titles.contains(&t.to_string()), "{t} in {titles:?}");
    }
    assert_eq!(palette::Action::Year.keys(), ["⌘5"]);
    run_from_palette(&mut rt, "year: messages", palette::Pick::Act(palette::Action::YearOf(Metric::Messages)));
    assert_eq!((rt.state.reports.mode, rt.state.year.metric), (reports::Mode::Year, Metric::Messages));
    command(&mut rt, '1');
    let titles: Vec<String> = palette::entries(&rt.state, "").into_iter().map(|e| e.title).collect();
    assert!(titles.contains(&"Year".to_string()), "{titles:?}");
}

#[test]
fn year_in_review_opens_steps_and_keeps_the_keys() {
    let mut s = store();
    s.entries[1].minutes.insert(0, Minute { minute: at(date("2025-06-01"), 12, 0) / MIN, usd: 5.0, requests: 1, ..Minute::default() });
    let mut rt = in_year(s, (1500.0, 2400.0));
    assert!(rt.presented("year-image").is_some() && rt.presented("year-open").is_some());
    typed(&mut rt, "r");
    assert_eq!(rt.state.year.card.as_ref().map(|c| c.year), Some(2026));
    assert!(rt.presented("year-card").is_some() && rt.presented("year-headline").is_some());
    typed(&mut rt, "qm");
    assert_eq!(rt.shell_request, None, "q stays with the card");
    assert_eq!(rt.state.year.metric, Metric::Spend, "m does not reach Year");
    press(&mut rt, RuneKey::Right);
    assert_eq!(rt.state.year.card.as_ref().and_then(|c| c.next), None, "never past this year");
    press(&mut rt, RuneKey::Left);
    for _ in 0..300 {
        if rt.state.year.card.as_ref().is_some_and(|c| c.year == 2025) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
        rt.frame(1.0 / 60.0);
    }
    assert_eq!(rt.state.year.card.as_ref().map(|c| c.year), Some(2025), "2025 arrives from the worker");
    press(&mut rt, RuneKey::Left);
    assert_eq!(rt.state.year.card.as_ref().and_then(|c| c.next), None, "never before the year the logs begin");
    press(&mut rt, RuneKey::Escape);
    assert!(rt.state.year.card.is_none() && rt.state.reports.mode == reports::Mode::Year);
    click(&mut rt, "year-open");
    settle(&mut rt);
    assert!(rt.state.year.card.is_some());
}

#[test]
fn a_card_that_is_not_ready_is_dimmed_and_not_saved() {
    let mut s = store_from("2026-10-10");
    s.entries[1].minutes.retain(|m| m.minute * MIN >= at(date("2026-10-10"), 0, 0));
    let mut rt = in_year(s, (1500.0, 2400.0));
    let ready = year::readiness(&sums(&rt), now());
    assert!(!ready.ready);
    assert_eq!(ready.ready_on_text().as_deref(), Some("Fri 1 Jan 2027"));
    assert!(text_of(&rt, "year-review-ready").contains("2026 has 34 days of logs, from 10 Oct, so its card is ready on Fri 1 Jan 2027"));
    assert!(rt.presented("year-lockband").is_some(), "the preview is dimmed under its band");
    typed(&mut rt, "r");
    typed(&mut rt, "s");
    assert!(rt.state.year.saving.is_none(), "no file is written");
    assert_eq!(rt.state.year.card.as_ref().and_then(|c| c.status.clone()).as_deref(), Some("the card of 2026 is ready on Fri 1 Jan 2027"));
}

#[test]
fn a_client_filter_says_year_counts_every_client() {
    let mut rt = in_year(store(), (1500.0, 1200.0));
    rt.update(|s, _| s.reports.client_ignored = true);
    settle(&mut rt);
    assert_eq!(text_of(&rt, "year-client"), "Year counts every client: the day store has no client");
    assert!(rt.presented("rep-client").is_none(), "said once, under the pills");
}

#[test]
fn year_draws_no_frames_at_rest() {
    let mut rt = in_year(store(), (1500.0, 1200.0));
    settle(&mut rt);
    assert!(!rt.update_if(|s, tx| poll(s, 0.0, tx)), "no more frames while nothing changes");
    assert!(!rt.animating);
}

/// Runs a scene on the made-up desk at the fixed clock, the year filled in first.
fn scene(name: &str, size: (f32, f32), dark: bool) -> Runtime<Desk> {
    let mut desk = year_desk_of(store());
    desk.year.wanted = Some(2026);
    let mut rt = Runtime::new(desk, view::view, size, Env::new(dark, 1.0));
    rt.poll = Some(poll);
    rt.on_key = Some(on_key);
    rt.update(|s, _| reports::show(s, reports::Mode::Year));
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
fn the_frames_of_year() {
    for dark in [false, true] {
        for size in [(1500.0, 950.0), (1100.0, 700.0), (960.0, 640.0)] {
            let rt = scene("year", size, dark);
            assert_eq!(text_of(&rt, "period-name"), "2026");
            assert!(rt.presented("rep-band").is_some() && rt.presented("year-grid").is_some(), "{size:?}");
            assert_eq!(cut_off(&rt, size.0), Vec::<String>::new(), "Year at {size:?}");
            let rt = scene("year-day", size, dark);
            assert_eq!(rt.state.reports.day, Some(date("2026-11-05")));
            assert!(rt.presented("year-tip").is_some());
            let rt = scene("year-review", size, dark);
            assert!(rt.presented("year-card").is_some() && rt.presented("year-card-save").is_some());
            let rt = scene("year-card", (540.0, 675.0), dark);
            assert!(rt.state.year.alone && rt.presented("year-headline").is_some());
            assert_eq!(cut_off(&rt, 540.0), Vec::<String>::new());
        }
    }
    let names: Vec<&str> = SCENES.iter().map(|s| s.0).collect();
    for gone in ["calendar", "calendar-day"] {
        assert!(!names.contains(&gone), "{gone} is gone");
    }
}

#[test]
fn the_flags_of_the_year_frames() {
    let err = |args: &[&str]| {
        let cli = Cli::parse_from(std::iter::once("tokprof-desk").chain(args.iter().copied()));
        year_of(&cli).err().map(|e| e.to_string()).unwrap_or_default()
    };
    assert_eq!(err(&["--year", "26"]), "--year takes YYYY");
    assert_eq!(err(&["--year", "2026-11"]), "--year takes YYYY");
    assert_eq!(err(&["--year", "2099"]), "2099 has not begun");
    assert_eq!(err(&["--year", "2026"]), "");
}

#[test]
fn the_year_card_image_fails_for_a_year_without_logs() {
    let dir = std::env::temp_dir().join(format!("tokprof-year-image-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let mut s = store();
    s.path = dir.join("days-2.json");
    s.save().unwrap();
    let cli = crate::Cli::try_parse_from(["tokprof-desk", "--card-image", "--year", "2024"]).unwrap();
    let catalog = Arc::new(Catalog::new(Vec::new(), PathBuf::new()));
    let err = crate::card_image(&cli, catalog.clone(), Pricing::defaults(), Query::default(), &s.path).err().map(|e| e.to_string());
    assert_eq!(err.as_deref(), Some("could not draw the year card: 2024 has no logs"));
    let cli = crate::Cli::try_parse_from(["tokprof-desk", "--card-image", "--year", "2026"]).unwrap();
    let desk = crate::card_image(&cli, catalog, Pricing::defaults(), Query::default(), &s.path).unwrap();
    assert!(desk.year.alone && year::card_year(&desk).is_some() && desk.browser.is_empty());
}

#[test]
fn a_save_that_fails_or_hangs_says_so_and_ends() {
    let mut fails = std::process::Command::new("sh");
    fails.args(["-c", "echo first >&2; echo could not draw the year card >&2; exit 3"]).stderr(std::process::Stdio::piped());
    assert_eq!(month::run_for(fails, std::time::Duration::from_secs(5)), Err("could not draw the year card".to_string()));
    let mut hangs = std::process::Command::new("sleep");
    hangs.arg("5").stderr(std::process::Stdio::piped());
    let started = std::time::Instant::now();
    let err = month::run_for(hangs, std::time::Duration::from_millis(200)).unwrap_err();
    assert!(err.contains("was stopped") && started.elapsed() < std::time::Duration::from_secs(3), "{err}");
}

#[test]
fn a_day_without_activity_is_an_empty_cell_in_a_dark_theme_and_no_logs_is_dashed() {
    let dark = crate::pal::pal(crate::themes::find("merino-dark").unwrap());
    assert!(dark.dark);
    assert_ne!(year::level_color(0, dark.accent, &dark), year::level_color(1, dark.accent, &dark));
    for id in ["merino-dark", "neon-arcade", "cathode-amber"] {
        assert!(crate::pal::pal(crate::themes::find(id).unwrap()).dark, "{id} is dark, so its empty days are outlined");
    }
}

#[test]
fn a_days_tooltip_names_every_metric() {
    let d = tokprof_core::calendar::Day {
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
        typed_sessions: 5,
        from_ms: 1_794_000_000_000,
        to_ms: 1_794_000_000_000 + 26 * 3_600_000,
        live: false,
        gone: true,
    };
    let lines = year::tooltip_lines(&d);
    assert_eq!(lines[..4], ["Thu 5 Nov 2026", "$214 spent, $31.00 of it avoidable", "1,820 requests in 8 sessions, 5 you typed in", "agents worked 6.2h"]);
    assert_eq!(
        lines[5..],
        ["2.5h at the keyboard in 14 sittings, 47 messages", "3 commits, 1 push, no pull request", "only from the store: its logs are gone"]
    );
    assert!(lines[4].starts_with("the day ran 26.0h"));
}
