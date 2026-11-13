//! Day layout A in the window: the lanes' header, the details pane with the day summary while no
//! session is selected, the selection and its day, the keys and the focus, the border between the
//! lanes and the details, and the day store read while Day shows; on a made-up day.

use tokprof_browse::render_rhythm::DayParts;
use tokprof_core::daystore::{Entry, Minute, Store};
use tokprof_core::fmt::{local_time, usd};
use tokprof_core::pauses::{ColdReturn, Starter};
use tokprof_core::rhythm::{Mark, Strip, StripLane, StripLanes};
use tokprof_core::xray::Glance;

use super::*;
use crate::details;

const MIN: i64 = 60_000;
const HOUR: i64 = 3_600_000;

fn midnight(date: i64) -> i64 {
    rhythm::midnight(date, &rhythm::local_offset)
}

fn today() -> i64 {
    rhythm::date_of(now_ms(), &rhythm::local_offset)
}

/// The past day the tests show: three days ago.
fn shown() -> i64 {
    today() - 3
}

/// A time of the shown day, `h` hours after its midnight.
fn at(h: i64, m: i64) -> i64 {
    midnight(shown()) + h * HOUR + m * MIN
}

/// An entry of the day store: what `session` of `main` spent at each time.
fn spent(session: &str, main: &str, subagent: bool, project: &str, times: &[(i64, f64)]) -> Entry {
    Entry {
        key: format!("/x/{session}.jsonl"),
        agent: "claude".into(),
        session: session.into(),
        main: main.into(),
        subagent,
        project: project.into(),
        minutes: times.iter().map(|&(ts, usd)| Minute { minute: ts / MIN, usd, requests: 1, ..Minute::default() }).collect(),
        ..Entry::default()
    }
}

/// The day's lines as `tokprof day` prints them.
const BELOW: &str = "\nsittings  7, 1.8h at the keyboard, 34 messages to 16 sessions; median sitting 1.1m, median break 29m\n\
agents    8.8 agent-hours over 1.8h at the keyboard, some agent at work 5.9h; peak 4 at 11:54\n\
answers   the agent's turn took a median 44s; you answered in a median 4.4m\n\
queue     no answer while another session waited\n\
cache     2 cold returns, $4.60 in their first requests, $4.14 of it avoidable\n";

/// The shown day, from 05:00 to 05:00 the next morning: lanes of id0, id1 and id2, and two cold
/// returns, of id1 at 13:08 after 1.2h and of id2 at 16:00 after 3h.
fn day_parts(date: i64, latest: bool) -> DayParts {
    let begin = midnight(date) + 5 * HOUR;
    let h = |k: i64| begin + (k - 5) * HOUR;
    let lane = |i: usize, from: i64| StripLane {
        id: format!("id{i}"),
        label: format!("id{i} Session {i}"),
        marks: vec![(from, from + 30 * MIN, Mark::Work), (from + 40 * MIN, from + 40 * MIN, Mark::Message)],
    };
    let lanes = StripLanes {
        begin,
        end: begin + 24 * HOUR,
        hours: (0..24).map(|k| (begin + k * HOUR, (k + 5) % 24)).collect(),
        you: vec![(h(9), h(17))],
        lanes: vec![lane(0, h(10)), lane(1, h(12)), lane(2, h(15))],
        agents: Default::default(),
    };
    let returns = vec![
        ColdReturn { ts_ms: h(13) + 8 * MIN, pause_ms: 72 * MIN, usd: 2.40, premium_usd: 2.27, context: 0, starter: Starter::Typed, session: "id1".into() },
        ColdReturn { ts_ms: h(16), pause_ms: 3 * HOUR, usd: 2.20, premium_usd: 1.87, context: 0, starter: Starter::Typed, session: "id2".into() },
    ];
    DayParts {
        above: "tokprof · day\n".into(),
        strip: Some((Strip { lines: vec!["(the strip)".into()] }, lanes)),
        below: BELOW.into(),
        bounds: (begin, if latest { i64::MAX } else { begin + 24 * HOUR }),
        returns,
        priced: true,
    }
}

/// Eight sessions, id0 to id7, last active on the shown day, id7 without a lane (a program ran
/// it). The store: id0 spent $28 that day and $5 more in its subagent, and $90 the day before;
/// id3 spent $2 at 03:00, before the day's start, which counts for the day before; id1, id2 and
/// id4 to id7 spent less that day.
fn day_desk() -> Desk {
    let rows = (0..8).map(|i| row(i, if i % 2 == 0 { "~/src/app" } else { "~/src/tool" }, (at(20, 0) / 1000 - i as i64) as u64)).collect();
    let mut desk = Desk::new(Browser::detached(Arc::new(Catalog::new(Vec::new(), PathBuf::new())), rows, Pricing::defaults(), frontend()), false);
    for i in 0..8 {
        let g = Glance { cost: 10.0 + i as f64, title: Some(format!("Session {i}")), cold_at_ms: now_ms() - HOUR, next_cold: Some(0.5), ..Glance::default() };
        desk.browser.put_glance(&format!("id{i}"), g);
    }
    let entries = vec![
        spent("id0", "id0", false, "~/src/app", &[(at(10, 5), 20.0), (at(14, 30), 8.0), (at(-8, 0), 90.0)]),
        spent("id0-sub", "id0", true, "~/src/app", &[(at(14, 40), 5.0)]),
        spent("id1", "id1", false, "~/src/tool", &[(at(12, 10), 12.0)]),
        spent("id2", "id2", false, "~/src/app", &[(at(15, 10), 6.0)]),
        spent("id3", "id3", false, "~/src/tool", &[(at(3, 0), 2.0), (at(11, 0), 4.0)]),
        spent("id4", "id4", false, "~/src/app", &[(at(11, 30), 3.0)]),
        spent("id5", "id5", false, "~/src/tool", &[(at(16, 0), 1.5)]),
        spent("id6", "id6", false, "~/src/app", &[(at(17, 0), 1.0)]),
    ];
    desk.calendar.data.put_with(tokprof_core::calendar::Calendar::default(), Store { entries, ..Store::default() }, 0);
    desk.browser.put_day(Some(shown()), shown(), day_parts(shown(), false));
    desk.browser.show_day(shown());
    desk
}

fn text_of(rt: &Runtime<Desk>, key: &str) -> String {
    rt.elements().into_iter().find(|e| e.key.as_deref() == Some(key)).and_then(|e| e.text).unwrap_or_default()
}

fn card(rt: &Runtime<Desk>, card: &str, key: &str) -> String {
    text_in(rt, card, key)
}

fn dollars(t: &str) -> f64 {
    t.trim_start_matches('$').replace(',', "").parse().unwrap_or_else(|_| panic!("{t} is not dollars"))
}

#[test]
fn the_day_summary_shows_the_cards_of_a_past_day() {
    let mut rt = runtime(day_desk(), (1400.0, 900.0));
    assert_eq!(rt.state.current(), None, "no session selected");
    assert_eq!(text_of(&rt, "day-title"), "The day");
    assert_eq!(text_of(&rt, "day-date"), local_time(at(12, 0), "%A, %-d %B"));
    // Spend: the day's requests within its bounds, the subagent's included, the 03:00 request not.
    assert_eq!(card(&rt, "card-spend", "card-value"), "$60.50", "28 + 5 + 12 + 6 + 4 + 3 + 1.50 + 1");
    assert_eq!(card(&rt, "card-spend", "spend-before"), "vs $92.00 the day before", "the whole day before, with 03:00 of this date");
    let bar = rt.elements().into_iter().find(|e| e.key.as_deref() == Some("hour-14")).expect("the bar of 14:00");
    let help = rt.elements().into_iter().find(|e| e.help.as_deref().is_some_and(|h| h.starts_with("14:00–15:00"))).and_then(|e| e.help);
    assert_eq!(help.as_deref(), Some("14:00–15:00 · $13.00"), "the hour's spend under the pointer");
    assert!(bar.rect[3] > 0.0);
    assert!(rt.elements().iter().any(|e| e.key.as_deref() == Some("hour-13")), "an hour without a request still has its place");
    // The rhythm cards: the number large, the rest of the line under it.
    assert_eq!(card(&rt, "card-sittings", "card-value"), "7");
    assert_eq!(card(&rt, "card-sittings", "card-text"), "1.8h at the keyboard, 34 messages to 16 sessions; median sitting 1.1m, median break 29m");
    assert_eq!(card(&rt, "card-agents", "card-value"), "8.8");
    assert_eq!(card(&rt, "card-answers", "card-value"), "4.4m");
    assert_eq!(card(&rt, "card-queue", "card-value"), "—");
    assert_eq!(card(&rt, "card-queue", "card-text"), "no answer while another session waited");
    assert_eq!(card(&rt, "card-cache", "card-value"), "2 cold returns");
    let p = pal::pal(rt.state.theme.get());
    assert_eq!(inside(&rt, "card-cache", "card-value").and_then(|e| e.text_color), Some(p.cold), "in the theme's red");
    assert_eq!(card(&rt, "card-cache", "card-text"), "$4.60 in their first requests, $4.14 of it avoidable");
    // Six cards in a row at 1400 points, of one height.
    let cards: Vec<[f32; 4]> =
        ["card-spend", "card-sittings", "card-agents", "card-answers", "card-queue", "card-cache"].map(|k| rt.presented(k).unwrap().0).to_vec();
    assert!(cards.iter().all(|c| c[1] == cards[0][1] && c[3] == cards[0][3]), "{cards:?}");
    // No Waiting for you now on a past day.
    assert!(rt.presented("day-waiting").is_none());
    // The lanes' header names the day by its date, with the Spend card's dollars.
    let caption = text_of(&rt, "day-caption");
    assert!(caption.starts_with(&format!("{}: 09:00 to ", local_time(at(12, 0), "%a %-d %b"))), "{caption}");
    assert!(caption.ends_with(", 1 sitting, 3 in the lanes, $60.50") || caption.ends_with(", 7 sittings, 3 in the lanes, $60.50"), "{caption}");
    assert!(rt.presented("zoom-fit").is_some() && rt.presented("zoom-day").is_some(), "with the zoom");
    // A filter: the lanes it keeps, of all of them.
    rt.update(|s, _| s.browser.set_filter("tool"));
    settle(&mut rt);
    let caption = text_of(&rt, "day-caption");
    assert!(caption.contains(", 1 of 3 in the lanes, "), "{caption}");
}

#[test]
fn three_cards_in_a_row_in_a_narrow_window() {
    let rt = runtime(day_desk(), (1100.0, 700.0));
    let (spend, answers) = (rt.presented("card-spend").unwrap().0, rt.presented("card-answers").unwrap().0);
    assert_eq!(answers[0], spend[0], "the fourth card starts the second row");
    assert!(answers[1] > spend[1]);
}

#[test]
fn the_top_sessions_and_the_rest_add_up_to_the_spend_card_and_the_returns_to_the_cache_card() {
    let rt = runtime(day_desk(), (1400.0, 900.0));
    let spends: Vec<f64> = (0..details::TOP).map(|i| dollars(&text_in(&rt, &format!("top-{i}"), "top-spend"))).collect();
    assert_eq!(text_in(&rt, "top-0", "top-spend"), "$33.00", "id0's $28 and its subagent's $5 that day, not its $90 the day before");
    assert_eq!(text_in(&rt, "top-0", "title"), "Session 0");
    assert!(spends.windows(2).all(|w| w[0] >= w[1]), "most first: {spends:?}");
    assert_eq!(text_of(&rt, "top-more-text"), "2 more sessions");
    let rest = dollars(&text_of(&rt, "top-more-spend"));
    let total = dollars(&card(&rt, "card-spend", "card-value"));
    assert!((spends.iter().sum::<f64>() + rest - total).abs() < 0.005, "{spends:?} + {rest} = {total}");
    // The bars: each as long as its share of the first.
    let bar = |i: usize| inside(&rt, &format!("top-{i}"), "fill").unwrap().rect[2];
    assert!((bar(1) / bar(0) - (12.0 / 33.0) as f32).abs() < 0.02, "{} of {}", bar(1), bar(0));
    // The cold returns, in time order, add up to the Cache card.
    assert_eq!(text_in(&rt, "return-0", "return-time"), local_time(at(13, 8), "%H:%M"));
    assert_eq!(text_in(&rt, "return-0", "return-pause"), "after 1.2h");
    assert_eq!((text_in(&rt, "return-0", "return-cost"), text_in(&rt, "return-0", "return-avoidable")), ("$2.40".into(), "$2.27".into()));
    let sum = |key: &str| (0..2).map(|i| dollars(&text_in(&rt, &format!("return-{i}"), key))).sum::<f64>();
    let text = card(&rt, "card-cache", "card-text");
    assert_eq!(text, format!("{} in their first requests, {} of it avoidable", usd(sum("return-cost")), usd(sum("return-avoidable"))));
    assert!(rt.presented("return-2").is_none(), "as many rows as the card counts");
}

#[test]
fn a_click_on_a_row_of_the_summary_selects_its_session_and_back_to_the_day() {
    let mut rt = runtime(day_desk(), (1400.0, 900.0));
    rt.update(|s, tx| s.show_tab(inspector::Tab::Cache, tx));
    click(&mut rt, "top-1");
    settle(&mut rt);
    let s = &rt.state;
    assert_eq!(s.current().map(|r| s.browser.session(r).to_string()).as_deref(), Some("id1"));
    assert_eq!(s.tab, inspector::Tab::Summary, "its Summary from the top");
    assert!(inside(&rt, "lane-id1", "lane-band").is_some(), "its lane highlighted");
    assert!(rt.presented("day-summary").is_none() && rt.presented("back-to-day").is_some());
    // Another lane keeps the tab shown.
    click(&mut rt, "tab-Cache");
    click(&mut rt, "lane-id2");
    settle(&mut rt);
    assert_eq!((rt.state.current(), rt.state.tab), (rt.state.browser.row_of("id2"), inspector::Tab::Cache));
    // `‹ The day`: no lane highlighted, the day summary.
    click(&mut rt, "back-to-day");
    settle(&mut rt);
    assert_eq!(rt.state.current(), None);
    assert!(rt.presented("day-summary").is_some() && inside(&rt, "lane-id2", "lane-band").is_none());
    // A cold return's row selects its session too.
    click(&mut rt, "return-1");
    settle(&mut rt);
    assert_eq!(rt.state.current(), rt.state.browser.row_of("id2"));
}

#[test]
fn the_current_day_lists_who_waits_now_and_compares_with_yesterday_by_the_clock() {
    let now = now_ms();
    let mut desk = day_desk();
    desk.browser.put_day(None, today(), DayParts { bounds: (now - 2 * HOUR, i64::MAX), ..day_parts(today(), true) });
    desk.browser.show_latest_day();
    desk.browser.put_glance(
        "id5",
        Glance { title: Some("Session 5".into()), waiting_since: Some(now - 4 * MIN - 10_000), cold_at_ms: now - HOUR, ..Glance::default() },
    );
    desk.browser
        .put_glance("id6", Glance { title: Some("Session 6".into()), waiting_since: Some(now - 40 * MIN), cold_at_ms: now - HOUR, ..Glance::default() });
    let mut rt = runtime(desk, (1400.0, 900.0));
    assert!(rt.state.browser.day_is_latest());
    assert_eq!(text_in(&rt, "waiting-0", "title"), "Session 6", "the longest wait first");
    assert_eq!(text_in(&rt, "waiting-1", "waiting-since"), format!("since {}, 4m", local_time(now - 4 * MIN - 10_000, "%H:%M")));
    // A wait that reaches the limit leaves the list.
    rt.update(|s, _| s.browser.put_glance("id6", Glance { waiting_since: Some(now - rhythm::WAITING_LIMIT_MS), cold_at_ms: now - HOUR, ..Glance::default() }));
    settle(&mut rt);
    assert_eq!(text_in(&rt, "waiting-0", "title"), "Session 5");
    assert!(rt.presented("waiting-1").is_none());
    // The lanes' header names the current day by its weekday, to now.
    let caption = text_of(&rt, "day-caption");
    assert!(caption.starts_with(&format!("{}: ", local_time(now, "%A"))) && caption.contains(" to now, "), "{caption}");
    // Nobody waits.
    rt.update(|s, _| s.browser.put_glance("id5", Glance { cold_at_ms: now - HOUR, ..Glance::default() }));
    settle(&mut rt);
    assert_eq!(text_in(&rt, "day-waiting", "waiting-none"), "Nobody waits for you");
}

#[test]
fn the_day_before_counts_up_to_the_same_time_of_day() {
    let now = 1_794_500_000_000; // a Thursday in November 2026, in the afternoon in Europe
    let date = rhythm::date_of(now, &rhythm::local_offset);
    let by = midnight(date - 1) + (now - midnight(date));
    let entries = vec![spent("a", "a", false, "~/src/app", &[(by - HOUR, 431.0), (by + HOUR, 1000.0), (now - HOUR, 146.0)])];
    let d = details::compute(&entries, |_| true, (midnight(date) + 5 * HOUR, i64::MAX), date, Some(now), now);
    assert_eq!(d.day.usd, 146.0);
    assert_eq!(d.before, Some((431.0, Some(by))), "yesterday up to the same time of day");
    let d = details::compute(&entries, |_| true, (midnight(date) + 5 * HOUR, i64::MAX), date, None, now);
    assert_eq!(d.before.map(|b| b.0), Some(1431.0), "a past day compares with the whole day before");
    let none = vec![spent("a", "a", false, "~/src/app", &[(now - HOUR, 146.0)])];
    assert_eq!(details::compute(&none, |_| true, (midnight(date) + 5 * HOUR, i64::MAX), date, Some(now), now).before, None, "no day before, no comparison");
}

#[test]
fn keys_move_the_summary_s_cursor_through_its_lists_and_enter_selects() {
    let mut rt = runtime(day_desk(), (1400.0, 900.0));
    press(&mut rt, RuneKey::Tab);
    assert_eq!(rt.state.pane, Focus::Inspector, "Tab gives the day summary the keyboard");
    assert_eq!(text_color_of(&rt, "day-title"), Some(pal::pal(rt.state.theme.get()).accent), "its title in the accent");
    assert_eq!(ringed(&rt), ["top-0"], "the cursor on the first top session");
    for _ in 0..4 {
        press(&mut rt, RuneKey::Text("j".into()));
    }
    assert_eq!(ringed(&rt), ["top-4"], "the last top session");
    press(&mut rt, RuneKey::Down);
    assert_eq!(ringed(&rt), ["return-0"], "then the right column");
    press(&mut rt, RuneKey::End);
    assert_eq!(ringed(&rt), ["return-1"]);
    press(&mut rt, RuneKey::Home);
    assert_eq!(ringed(&rt), ["top-0"]);
    for _ in 0..5 {
        press(&mut rt, RuneKey::Down);
    }
    press(&mut rt, RuneKey::Enter);
    assert_eq!(rt.state.current(), rt.state.browser.row_of("id1"), "⏎ selects the first cold return's session");
    assert!(rt.presented("back-to-day").is_some(), "shown in the inspector");
    // esc gives the keyboard back to the lanes, then deselects; never quits.
    press(&mut rt, RuneKey::Escape);
    assert_eq!((rt.state.pane, rt.state.current()), (Focus::Lanes, rt.state.browser.row_of("id1")));
    press(&mut rt, RuneKey::Escape);
    assert_eq!(rt.state.current(), None);
    press(&mut rt, RuneKey::Escape);
    assert_eq!(rt.shell_request, None);
}

fn text_color_of(rt: &Runtime<Desk>, key: &str) -> Option<rune::Color> {
    rt.elements().into_iter().find(|e| e.key.as_deref() == Some(key)).and_then(|e| e.text_color)
}

#[test]
fn the_lanes_select_from_none_step_days_and_t_shows_the_latest() {
    let mut desk = day_desk();
    // The day before has id1's lane only.
    let mut before = day_parts(shown() - 1, false);
    if let Some((_, lanes)) = &mut before.strip {
        lanes.lanes.retain(|l| l.id == "id1");
    }
    desk.browser.put_day(Some(shown() - 1), shown() - 1, before);
    desk.browser.put_day(None, today(), DayParts { bounds: (now_ms() - HOUR, i64::MAX), ..day_parts(today(), true) });
    let mut rt = runtime(desk, (1400.0, 900.0));
    assert_eq!(rt.state.pane, Focus::Lanes);
    press(&mut rt, RuneKey::Down);
    assert_eq!(rt.state.current(), rt.state.browser.row_of("id0"), "↓ with none selected: the first lane");
    rt.update(|s, tx| s.deselect(tx));
    press(&mut rt, RuneKey::Up);
    assert_eq!(rt.state.current(), rt.state.browser.row_of("id2"), "↑: the last");
    // ← to the day before: id2 has no lane there, so none is selected.
    press(&mut rt, RuneKey::Left);
    assert_eq!(rhythm::date_of(rt.state.browser.day_noon(), &rhythm::local_offset), shown() - 1);
    assert_eq!(rt.state.current(), None, "a session without a lane that day is not kept");
    assert!(rt.presented("day-summary").is_some());
    // → back, id1 selected; ← keeps it, its lane is there.
    press(&mut rt, RuneKey::Right);
    rt.update(|s, tx| s.open_lane("id1", tx));
    press(&mut rt, RuneKey::Left);
    assert_eq!(rt.state.current(), rt.state.browser.row_of("id1"), "kept: it has a lane that day");
    // t: the day of the latest action, as Today.
    press(&mut rt, RuneKey::Text("t".into()));
    assert!(rt.state.browser.day_is_latest());
    // d does nothing now.
    rt.update(|s, _| s.browser.show_day(shown()));
    settle(&mut rt);
    press(&mut rt, RuneKey::Text("d".into()));
    assert!(!rt.state.browser.day_is_latest(), "d is gone");
}

#[test]
fn a_session_without_a_lane_shows_its_inspector_and_the_header_says_so() {
    let mut rt = runtime(day_desk(), (1400.0, 900.0));
    rt.update(|s, tx| {
        s.pick("id7", tx);
    });
    settle(&mut rt);
    assert_eq!(rt.state.current(), rt.state.browser.row_of("id7"));
    assert!(rt.presented("back-to-day").is_some(), "its inspector");
    assert!(rt.elements().iter().all(|e| e.key.as_deref() != Some("lane-band")), "no lane highlighted");
    assert_eq!(text_of(&rt, "no-lane"), "no lane: nobody typed in id7 this day");
}

#[test]
fn opening_from_sessions_shows_the_session_on_its_day_with_or_without_its_lane() {
    let mut rt = runtime(day_desk(), (1400.0, 900.0));
    rt.update(|s, _| reports::show(s, reports::Mode::Sessions));
    settle(&mut rt);
    // A session a program ran: its day, no lane highlighted, its inspector, and the header says so.
    rt.update(|s, tx| sessions::open_in_day(s, "id7", tx));
    settle(&mut rt);
    assert_eq!(rt.state.reports.mode, reports::Mode::Day);
    assert_eq!(rhythm::date_of(rt.state.browser.day_noon(), &rhythm::local_offset), shown());
    assert_eq!((rt.state.current(), rt.state.tab), (rt.state.browser.row_of("id7"), inspector::Tab::Summary));
    assert!(rt.presented("back-to-day").is_some() && rt.presented("day-summary").is_none(), "its inspector in the details pane");
    assert!(rt.elements().iter().all(|e| e.key.as_deref() != Some("lane-band")), "no lane highlighted");
    assert_eq!(text_of(&rt, "no-lane"), "no lane: nobody typed in id7 this day");
    // A session with a lane that day: its lane highlighted, its Summary from the top.
    click(&mut rt, "tab-Cache");
    rt.update(|s, _| reports::show(s, reports::Mode::Sessions));
    rt.update(|s, tx| sessions::open_in_day(s, "id1", tx));
    settle(&mut rt);
    assert_eq!((rt.state.current(), rt.state.tab), (rt.state.browser.row_of("id1"), inspector::Tab::Summary));
    assert!(inside(&rt, "lane-id1", "lane-band").is_some());
    assert!(rt.presented("no-lane").is_none());
}

#[test]
fn a_frame_comes_when_the_store_s_progress_moves_and_none_at_rest() {
    assert_eq!(details::reading(Some((120, 1108))), "reading the logs into the store… 120 of 1108 logs");
    assert_eq!(details::reading(Some((0, 0))), "reading the logs into the store…", "before the logs are counted");
    let mut rt = runtime(day_desk(), (1400.0, 900.0));
    // The progress the last poll saw is not the worker's now (none: the store was read).
    rt.update(|s, _| s.calendar.seen = Some((3, 10)));
    assert!(rt.update_if(|s, tx| poll(s, 0.0, tx)), "the count moved: a frame");
    assert!(!rt.update_if(|s, tx| poll(s, 0.0, tx)), "then none");
}

#[test]
fn no_x_ray_is_asked_while_no_session_is_selected() {
    let (catalog, select) = fixture();
    let browser = Browser::start(catalog, Pricing::defaults(), select, false, Arc::new(|| {}), frontend()).unwrap();
    let mut rt = runtime(Desk::new(browser, false), (1400.0, 900.0));
    std::thread::sleep(Duration::from_millis(300));
    settle(&mut rt);
    assert!(rt.state.browser.xray().is_none(), "none selected: no X-ray");
    assert!(matches!(rt.state.browser.report(), tokprof_browse::browser::Pane::Computing(_)), "nor one being computed");
    let id = rt.state.browser.session(0).to_string();
    rt.update(|s, tx| {
        s.pick(&id, tx);
    });
    let deadline = Instant::now() + Duration::from_secs(20);
    while rt.state.browser.xray().is_none() {
        assert!(Instant::now() < deadline, "the X-ray did not come");
        std::thread::sleep(Duration::from_millis(20));
        rt.frame(1.0 / 60.0);
    }
}

#[test]
fn a_double_click_on_a_lane_shows_its_inspector_full_size() {
    let mut rt = runtime(day_desk(), (1400.0, 900.0));
    let (x, y) = rt.find("lane-id1").unwrap();
    rt.frame(0.6);
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    rt.frame(0.05);
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    settle(&mut rt);
    assert_eq!(rt.state.current(), rt.state.browser.row_of("id1"));
    assert!(rt.state.browser.zoomed() && rt.presented("lanes").is_none(), "full size, until esc");
    press(&mut rt, RuneKey::Escape);
    assert!(!rt.state.browser.zoomed() && rt.presented("lanes").is_some());
}

/// Drags the border by `dy` points, and lets go.
fn drag_border(rt: &mut Runtime<Desk>, dy: f32) {
    let (x, y) = rt.find("border").expect("the border");
    rt.pointer_down(x, y);
    for k in 1..=10 {
        rt.pointer_move(x, y + dy * k as f32 / 10.0);
        rt.frame(1.0 / 60.0);
    }
    rt.pointer_up(x, y + dy);
    settle(rt);
}

/// Thirty session lanes on the shown day.
fn busy_desk() -> Desk {
    let mut desk = day_desk();
    let mut parts = day_parts(shown(), false);
    if let Some((_, lanes)) = &mut parts.strip {
        let lane = lanes.lanes[0].clone();
        lanes.lanes = (0..30).map(|i| StripLane { id: format!("id{}", i % 8), ..lane.clone() }).collect();
    }
    desk.browser.put_day(Some(shown()), shown(), parts);
    desk
}

fn whole_lanes(rt: &Runtime<Desk>) -> usize {
    let scroll = rt.presented("lanes-scroll").unwrap().0;
    rt.elements()
        .iter()
        .filter(|e| {
            e.key.as_deref().is_some_and(|k| k.starts_with("lane-id")) && e.rect[1] >= scroll[1] && e.rect[1] + e.rect[3] <= scroll[1] + scroll[3] + 0.5
        })
        .count()
}

#[test]
fn the_border_moves_shows_more_lanes_is_saved_and_a_double_click_restores_it() {
    let path = std::env::temp_dir().join(format!("tokprof-desk-border-{}.json", std::process::id()));
    let _ = std::fs::remove_file(&path);
    let mut rt = runtime(busy_desk().with_prefs(rune::prefs::Prefs::at(&path)), (1400.0, 900.0));
    let room = 900.0 - view::BAR_H - 1.0;
    let before = whole_lanes(&rt);
    let lanes_h = rt.presented("lanes-block").unwrap().0[3];
    drag_border(&mut rt, 0.7 * room - lanes_h);
    let share = rt.state.lanes_share.expect("a place");
    assert!((share - 0.7).abs() < 0.01, "{share}");
    let block = rt.presented("lanes-block").unwrap().0;
    assert!((block[3] - 0.7 * room).abs() < 2.0, "the lanes take 70%: {block:?}");
    assert!(whole_lanes(&rt) > before, "more lanes show whole: {} after {before}", whole_lanes(&rt));
    let details = rt.presented("details").unwrap().0;
    assert!(details[3] >= view::DETAILS_MIN, "the details pane takes the rest: {details:?}");
    let saved = std::fs::read_to_string(&path).unwrap();
    assert!(saved.contains("\"lanes_share\": 0.7"), "{saved}");
    // The next start puts it there.
    let rt2 = runtime(busy_desk().with_prefs(rune::prefs::Prefs::at(&path)), (1400.0, 900.0));
    assert!((rt2.presented("lanes-block").unwrap().0[3] - 0.7 * room).abs() < 2.0);
    // A double click gives back the room the window gives and takes the key out.
    let (x, y) = rt.find("border").unwrap();
    rt.frame(0.6);
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    rt.frame(0.05);
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    settle(&mut rt);
    assert_eq!(rt.state.lanes_share, None);
    assert!((rt.presented("lanes-block").unwrap().0[3] - lanes_h).abs() < 1.0, "back to the default");
    assert!(!std::fs::read_to_string(&path).unwrap().contains("lanes_share"));
    let _ = std::fs::remove_file(&path);
}

#[test]
fn a_saved_place_that_does_not_fit_goes_to_the_nearest_one_and_a_frame_ignores_it() {
    let room = 900.0 - view::BAR_H - 1.0;
    assert_eq!(view::border_h(0.9, room), 0.75 * room, "75% at most");
    assert_eq!(view::border_h(0.1, room), 0.25 * room, "25% at least");
    let low = 500.0 - view::BAR_H - 1.0;
    assert_eq!(view::border_h(0.75, low), low - view::DETAILS_MIN - view::BORDER_H, "the details keep 200 points");
    let mut prefs = rune::prefs::Prefs::memory();
    prefs.set("lanes_share", &0.9).unwrap();
    let rt = runtime(busy_desk().with_prefs(prefs), (1400.0, 900.0));
    assert!((rt.presented("lanes-block").unwrap().0[3] - 0.75 * room).abs() < 1.0);
    // A frame without a window reads no settings file: its desk keeps its settings in memory.
    let headless = desk_of(1);
    assert!(!headless.prefs.is_persistent() && headless.lanes_share.is_none());
}

#[test]
fn the_store_is_read_while_day_shows_with_its_progress_then_the_cards_and_no_frames_at_rest() {
    let mut desk = day_desk();
    desk.calendar = calendar::State::new(tokprof_browse::calendar::Calendars::detached());
    let mut rt = runtime(desk, (1400.0, 900.0));
    assert_eq!(text_in(&rt, "card-spend", "spend-reading"), "reading the logs into the store…");
    assert_eq!(text_in(&rt, "day-top", "top-reading"), "reading the logs into the store…");
    assert_eq!(text_in(&rt, "card-sittings", "card-value"), "7", "the rhythm cards show at once");
    // The store arrives.
    let store = day_desk().calendar.data.get().unwrap().store.clone().unwrap();
    rt.update(|s, _| s.calendar.data.put_with(tokprof_core::calendar::Calendar::default(), (*store).clone(), 0));
    settle(&mut rt);
    assert_eq!(text_in(&rt, "card-spend", "card-value"), "$60.50");
    assert!(rt.presented("spend-reading").is_none());
    // At rest: no poll asks for a frame.
    assert!(!rt.update_if(|s, tx| poll(s, 0.0, tx)), "the summary draws no more frames while nothing changes");
    assert!(!rt.animating);
    // Stepping away and back shows it at once, from the same store.
    rt.update(|s, _| s.step_day(-1));
    settle(&mut rt);
    rt.update(|s, _| s.browser.show_day(shown()));
    settle(&mut rt);
    assert_eq!(text_in(&rt, "card-spend", "card-value"), "$60.50");
}

#[test]
fn day_asks_the_worker_for_the_store_and_shows_its_cards_once_it_is_filled() {
    let (catalog, select) = fixture();
    let dir = std::env::temp_dir().join(format!("tokprof-desk-day-store-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let mut desk = day_desk();
    desk.calendar.data = tokprof_browse::calendar::Calendars::start(catalog, Pricing::defaults(), None, select, dir.join("days.json"), Arc::new(|| {}));
    let mut rt = runtime(desk, (1400.0, 900.0));
    let deadline = Instant::now() + Duration::from_secs(30);
    while rt.state.calendar.data.get().is_none() {
        assert!(Instant::now() < deadline, "the store was not read");
        std::thread::sleep(Duration::from_millis(20));
        rt.frame(1.0 / 60.0);
    }
    settle(&mut rt);
    assert_eq!(text_in(&rt, "card-spend", "card-value"), "$0.00", "the fixture's requests are not on this day");
    assert_eq!(text_in(&rt, "day-top", "top-none"), "No spend this day.");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_client_filter_says_the_store_counts_every_client() {
    let mut desk = day_desk();
    desk.calendar.client_ignored = true;
    let rt = runtime(desk, (1400.0, 900.0));
    assert_eq!(text_in(&rt, "card-spend", "spend-client"), "every client: the day store has no client");
}
