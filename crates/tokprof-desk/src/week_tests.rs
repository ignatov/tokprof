//! Week in the window: the switch, the bar, the days with their strips, the totals and lines,
//! the column, the clicks and keys, the progress and the filter, on a made-up store with a fixed
//! clock: Thursday 12 November 2026 at 14:20.

use tokprof_browse::week::{Key as WeekKey, WeekLogs};
use tokprof_core::daystore::{Entry, Minute, Store};
use tokprof_core::pool::Row as PoolRow;
use tokprof_core::savings::{Changes, SessionCost};
use tokprof_core::xray::Glance;

use super::*;

const MIN: i64 = 60_000;
const HOUR: i64 = 3_600_000;

fn date(s: &str) -> i64 {
    rhythm::date_of(parse_date(s).unwrap() + 12 * HOUR, &rhythm::local_offset)
}

fn at(d: i64, h: i64, m: i64) -> i64 {
    rhythm::midnight(d, &rhythm::local_offset) + h * HOUR + m * MIN
}

/// Thursday 12 November 2026, 14:20.
fn now() -> i64 {
    at(date("2026-11-12"), 14, 20)
}

fn entry(id: &str, project: &str) -> Entry {
    Entry { key: format!("/x/{id}.jsonl"), agent: "claude".into(), session: id.into(), main: id.into(), project: project.into(), ..Entry::default() }
}

/// The agent worked in every minute of `[from, to)`.
fn work(e: &mut Entry, from: i64, to: i64) {
    let mut m = from / MIN;
    while m * MIN < to {
        let (h, b) = (m.div_euclid(60), m.rem_euclid(60));
        match e.work.iter_mut().find(|x| x.0 == h) {
            Some(x) => x.1 |= 1 << b,
            None => e.work.push((h, 1 << b)),
        }
        m += 1;
    }
    e.work.sort_unstable();
}

fn spend(e: &mut Entry, ts: i64, usd: f64, avoidable: f64) {
    e.minutes.push(Minute { minute: ts / MIN, usd, avoidable_usd: avoidable, requests: 1, read_tokens: 900, prompt_tokens: 1000, ..Minute::default() });
    e.minutes.sort_by_key(|m| m.minute);
}

/// Messages every 10 minutes in `[from, to)`.
fn messages(e: &mut Entry, from: i64, to: i64) {
    let mut t = from;
    while t < to {
        e.actions.push((t, true));
        t += 10 * MIN;
    }
    e.actions.sort_unstable();
}

/// The store: `id0` in `~/src/app` and `id1` in `~/src/air`, last week and this week so far.
fn store() -> Store {
    let (mon, tue, thu) = (date("2026-11-09"), date("2026-11-10"), date("2026-11-12"));
    let (mon0, tue0, thu0, fri0) = (date("2026-11-02"), date("2026-11-03"), date("2026-11-05"), date("2026-11-06"));
    let mut a = entry("id0", "~/src/app");
    let mut b = entry("id1", "~/src/air");
    // Last week: Monday and Thursday morning before 14:20, and Friday after the same point.
    work(&mut a, at(mon0, 9, 0), at(mon0, 10, 0));
    spend(&mut a, at(mon0, 9, 0), 100.0, 40.0);
    messages(&mut a, at(mon0, 9, 0), at(mon0, 9, 30));
    spend(&mut a, at(thu0, 10, 0), 50.0, 0.0);
    spend(&mut a, at(fri0, 10, 0), 300.0, 0.0);
    a.cold.extend([at(tue0, 10, 0), at(tue0, 11, 0)]);
    spend(&mut a, at(tue0, 10, 0), 0.0, 0.0);
    // This week.
    work(&mut a, at(mon, 9, 0), at(mon, 11, 0));
    spend(&mut a, at(mon, 9, 0), 100.0, 10.0);
    messages(&mut a, at(mon, 9, 0), at(mon, 10, 30));
    work(&mut a, at(tue, 13, 0), at(tue, 16, 0));
    spend(&mut a, at(tue, 13, 0), 200.0, 0.0);
    messages(&mut a, at(tue, 13, 0), at(tue, 14, 0));
    work(&mut b, at(tue, 14, 0), at(tue, 15, 0));
    spend(&mut b, at(tue, 13, 42), 5.0, 4.0);
    b.cold.push(at(tue, 13, 42));
    messages(&mut b, at(tue, 13, 42), at(tue, 13, 43));
    work(&mut a, at(thu, 10, 0), at(thu, 11, 0));
    spend(&mut a, at(thu, 10, 0), 50.0, 0.0);
    messages(&mut a, at(thu, 10, 0), at(thu, 10, 20));
    a.misses.push((at(mon, 9, 0) / HOUR, "idle > TTL".into(), 10.0));
    // The week before last, so that the comparison is complete.
    spend(&mut a, at(date("2026-10-30"), 10, 0), 10.0, 0.0);
    // A message late every evening, so that each night has its pause.
    for d in date("2026-10-30")..date("2026-11-12") {
        a.actions.push((at(d, 23, 0), true));
    }
    a.actions.sort_unstable();
    Store { entries: vec![a, b], ..Store::default() }
}

fn summed(store: &Store, monday: i64, text: &str, now: i64) -> tokprof_core::week::Week {
    tokprof_browse::week::week_of(store, &Query::default(), &WeekKey::new(monday, text), now)
}

/// What the week's logs give, with `changes` as What to change and a cold return of `id1`.
fn logs_with(changes: Changes) -> WeekLogs {
    let (catalog, select) = fixture();
    let mut overview = tokprof_core::overview::collect(&catalog, &select, &Pricing::defaults(), |_, _| {}).unwrap();
    overview.changes = changes;
    let tue = date("2026-11-10");
    let ret = tokprof_core::pauses::ColdReturn {
        ts_ms: at(tue, 13, 42),
        pause_ms: 2 * HOUR,
        usd: 5.0,
        premium_usd: 4.0,
        context: 100_000,
        starter: tokprof_core::pauses::Starter::Typed,
        session: Arc::from("id1"),
    };
    WeekLogs {
        overview,
        window: (at(date("2026-11-09"), 0, 0), now()),
        files: 2,
        bytes: 10,
        read: String::new(),
        warnings: Vec::new(),
        sittings: String::new(),
        answers: "answers   the agent's turn took a median 1.2m; you answered in a median 3.9m".into(),
        queue: "queue     when several sessions waited, you answered the one that waited least first in 50% of 4 answers (40% by chance)".into(),
        cache: "cache     1 cold return, $5.00 in its first request, $4.00 of it avoidable".into(),
        returns: vec![ret],
        priced: true,
        advice: vec!["1 return after the cache expired cost $4.00 that a warm cache would have saved; its break was 2.0h. Come back sooner.".into()],
        peak: Some((2, at(tue, 14, 0))),
        waits: vec![("id0".into(), "id0 Ship it".into(), vec![(at(tue, 16, 0), at(tue, 16, 30))])],
        yellow: vec![(at(tue, 16, 0), at(tue, 16, 30))],
    }
}

fn saving(name: &str, usd: f64) -> PoolRow {
    PoolRow { name: name.into(), usd, share: usd / 355.0, ..PoolRow::default() }
}

fn changes() -> Changes {
    let mut c = Changes {
        saved: vec![saving("idle > TTL", 10.0), saving("history edited", 2.0), saving("model switch", 1.0), saving("tools changed", 0.5)],
        habits: vec![saving("effort max or xhigh", 300.0), saving("prompts of 400k+", 200.0), saving("single lookups", 100.0), saving("long sessions", 50.0)],
        ..Changes::default()
    };
    let agent = tokprof_core::source::AgentId::unknown("claude");
    c.sessions.insert(
        "idle > TTL".into(),
        vec![SessionCost { agent: agent.clone(), session: "id0".into(), usd: 8.0 }, SessionCost { agent, session: "id1".into(), usd: 2.0 }],
    );
    c.hints.insert("idle > TTL".into(), "compact or wrap up a big session before a long break".into());
    c
}

/// A desk on the made-up store at the fixed clock, its week summed; with `read`, its logs read.
fn week_desk(read: bool) -> Desk {
    let mut desk = desk_of(3);
    desk.browser.put_glance("id1", Glance { title: Some("Speed up the build".into()), ..Glance::default() });
    let store = store();
    let mut weeks = tokprof_browse::week::Weeks::detached();
    for monday in [date("2026-11-09"), date("2026-11-02"), date("2026-10-26")] {
        weeks.put(WeekKey::new(monday, ""), summed(&store, monday, "", now()));
    }
    if read {
        weeks.put_logs(WeekKey::new(date("2026-11-09"), ""), logs_with(changes()), 0);
    }
    let cal = tokprof_core::calendar::Calendar::of(&store.entries, &tokprof_core::calendar::Filters::default(), now(), &rhythm::local_offset);
    desk.calendar.data.put_with(cal, store, 0);
    desk.week = week::State::new(weeks);
    desk.week.clock = Some(now());
    desk
}

fn in_week(desk: Desk, size: (f32, f32)) -> Runtime<Desk> {
    let mut rt = runtime(desk, size);
    command(&mut rt, '2');
    settle(&mut rt);
    rt
}

fn rect(rt: &Runtime<Desk>, outer: &str, key: &str) -> [f32; 4] {
    inside(rt, outer, key).unwrap_or_else(|| panic!("no {key} in {outer}")).rect
}

#[test]
fn command_2_and_the_switch_show_week_on_this_week_and_esc_goes_back() {
    let mut rt = runtime(week_desk(true), (1400.0, 1600.0));
    command(&mut rt, '2');
    settle(&mut rt);
    assert_eq!(rt.state.reports.mode, reports::Mode::Week);
    assert_eq!(text_of(&rt, "week-name"), "9–15 Nov 2026", "this week");
    assert!(inside(&rt, "mode-week", "mode-week").is_some() || rt.presented("mode-week").is_some());
    for gone in ["lanes", "details", "day-name", "zoom", "rep-period"] {
        assert!(rt.presented(gone).is_none(), "{gone} is not shown in Week");
    }
    assert!(rt.presented("filter").is_some() && rt.presented("week-column").is_some(), "the filter and the column");
    typed(&mut rt, "qsdr");
    assert_eq!(rt.shell_request, None, "q does not quit from Week");
    assert_eq!(rt.state.reports.mode, reports::Mode::Week);
    press(&mut rt, RuneKey::Escape);
    assert_eq!(rt.state.reports.mode, reports::Mode::Day, "esc with no filter shows Day");
    click(&mut rt, "mode-week");
    settle(&mut rt);
    assert_eq!(rt.state.reports.mode, reports::Mode::Week);
    command(&mut rt, '3');
    assert_eq!(rt.state.reports.mode, reports::Mode::Month, "⌘3 shows Month");
    assert_eq!(rt.state.reports.month, None, "on the current month");
    command(&mut rt, '4');
    assert_eq!(rt.state.reports.mode, reports::Mode::Quarter);
    command(&mut rt, '5');
    assert_eq!(rt.state.reports.mode, reports::Mode::Year);
    command(&mut rt, '1');
    assert_eq!(rt.state.reports.mode, reports::Mode::Day);
    // The palette offers Week and the week before it.
    let titles: Vec<String> = palette::entries(&rt.state, "").into_iter().map(|e| e.title).collect();
    assert!(titles.contains(&"Week".to_string()) && titles.contains(&"Week: last week".to_string()), "{titles:?}");
    run_from_palette(&mut rt, "week: last", palette::Pick::Act(palette::Action::WeekLast));
    settle(&mut rt);
    assert_eq!((rt.state.reports.mode, text_of(&rt, "week-name").as_str()), (reports::Mode::Week, "2–8 Nov 2026"));
    let keys = palette::Action::Week.keys();
    assert_eq!(keys, ["⌘2"]);
    assert_eq!(palette::Action::Month.keys(), ["⌘3"]);
    assert_eq!(palette::Action::Quarter.keys(), ["⌘4"]);
    assert_eq!(palette::Action::Year.keys(), ["⌘5"]);
}

#[test]
fn the_column_keeps_its_width_in_a_narrow_window() {
    let rt = in_week(week_desk(true), (960.0, 900.0));
    let column = rt.presented("week-column").unwrap().0;
    assert!((column[2] - week::COLUMN_W).abs() < 0.5, "{column:?}");
    let left = rt.presented("week-left").unwrap().0;
    assert!(left[0] + left[2] <= column[0], "the days take the rest: {left:?} {column:?}");
}

#[test]
fn the_bar_names_the_week_steps_it_never_past_this_week_nor_before_the_logs() {
    let mut rt = in_week(week_desk(true), (1400.0, 1600.0));
    let n = rt.state.week.data.week(&WeekKey::new(date("2026-11-09"), "")).unwrap().totals.sittings.len();
    assert_eq!(text_of(&rt, "week-summary"), format!("Mon–Thu so far · {n} sittings"));
    click(&mut rt, "week-on");
    settle(&mut rt);
    assert_eq!(text_of(&rt, "week-name"), "9–15 Nov 2026", "never past this week");
    click(&mut rt, "week-back");
    settle(&mut rt);
    assert_eq!(text_of(&rt, "week-name"), "2–8 Nov 2026");
    let n = rt.state.week.data.week(&WeekKey::new(date("2026-11-02"), "")).unwrap().totals.sittings.len();
    assert_eq!(text_of(&rt, "week-summary"), format!("{n} sittings"), "another week names only its sittings");
    click(&mut rt, "week-back");
    settle(&mut rt);
    assert_eq!(text_of(&rt, "week-name"), "26 Oct – 1 Nov 2026", "the week of the store's first day");
    click(&mut rt, "week-back");
    settle(&mut rt);
    assert_eq!(text_of(&rt, "week-name"), "26 Oct – 1 Nov 2026", "not before it");
    click(&mut rt, "today");
    settle(&mut rt);
    assert_eq!(text_of(&rt, "week-name"), "9–15 Nov 2026");
    // Across months and across years.
    rt.update(|s, _| s.week.monday = Some(date("2026-10-26")));
    settle(&mut rt);
    assert_eq!(text_of(&rt, "week-name"), "26 Oct – 1 Nov 2026");
    rt.update(|s, _| {
        s.week.clock = Some(at(date("2027-01-06"), 12, 0));
        s.week.monday = Some(date("2026-12-28"));
    });
    settle(&mut rt);
    assert_eq!(text_of(&rt, "week-name"), "28 Dec 2026 – 3 Jan 2027");
}

#[test]
fn the_days_show_their_strips_numbers_and_the_now_line() {
    let rt = in_week(week_desk(true), (1400.0, 1600.0));
    let w = rt.state.week.data.week(&WeekKey::new(date("2026-11-09"), "")).unwrap().clone();
    let (mon, tue, thu, fri) = (date("2026-11-09"), date("2026-11-10"), date("2026-11-12"), date("2026-11-13"));
    let row = |d: i64| format!("week-day-{d}");
    assert_eq!(text_in(&rt, &row(tue), "day-spend"), "$205");
    assert_eq!(text_in(&rt, &row(tue), "day-agent"), "3.0h", "13:00 to 16:00, the minutes of two sessions once");
    assert_eq!(text_in(&rt, &row(tue), "day-cold"), "1 cold");
    let cell = &w.rows[1].cell.amounts;
    assert_eq!(text_in(&rt, &row(tue), "day-keyboard"), week::hours(cell.keyboard_ms));
    assert_eq!(text_in(&rt, &row(mon), "day-spend"), "$100");
    assert_eq!(text_in(&rt, &row(date("2026-11-11")), "day-spend"), "$0.00", "a day without activity reads zero");
    // The days after now have no numbers.
    assert!(inside(&rt, &row(fri), "day-spend").is_none() && inside(&rt, &row(fri), "work-0").is_none());
    // The bars stand where the axis puts their times.
    let strip = rect(&rt, &row(tue), "strip-bg");
    let bar = rect(&rt, &row(tue), "work-0");
    let want = w.axis.x(tue, at(tue, 13, 0), &rhythm::local_offset) as f32;
    assert!(((bar[0] - strip[0]) / strip[2] - want).abs() < 0.005, "work starts at 13:00: {bar:?} in {strip:?}, {want}");
    let end = w.axis.x(tue, at(tue, 16, 0), &rhythm::local_offset) as f32;
    assert!(((bar[0] + bar[2] - strip[0]) / strip[2] - end).abs() < 0.005, "and ends at 16:00");
    // The yellow of the read, after the work.
    let wait = rect(&rt, &row(tue), "wait-0");
    assert!(wait[0] >= bar[0] + bar[2] - 1.0, "the wait after the work: {wait:?}");
    // The tick of the cold return, at 13:42.
    let tick = rect(&rt, &row(tue), "tick-0");
    let want = w.axis.x(tue, at(tue, 13, 42), &rhythm::local_offset) as f32;
    assert!(((tick[0] + tick[2] / 2.0 - strip[0]) / strip[2] - want).abs() < 0.005);
    // Today has the now line, at 14:20.
    let now_line = rect(&rt, &row(thu), "now");
    let want = w.axis.x(thu, now(), &rhythm::local_offset) as f32;
    assert!(((now_line[0] + now_line[2] / 2.0 - strip[0]) / strip[2] - want).abs() < 0.005);
    assert!(inside(&rt, &row(tue), "now").is_none(), "only today has it");
    // The hour labels: one row of them, over the strips.
    assert!(texts(&rt).iter().any(|t| *t == format!("{:02}", w.axis.from_h.rem_euclid(24))));
}

#[test]
fn the_totals_are_the_footer_of_the_days_and_the_tiles_compare_with_the_same_point_of_last_week() {
    let rt = in_week(week_desk(true), (1400.0, 1600.0));
    let w = rt.state.week.data.week(&WeekKey::new(date("2026-11-09"), "")).unwrap().clone();
    // The footer: the week's numbers under the days' own, with no changes.
    assert_eq!(text_in(&rt, "week-totals", "total-label"), "Mon–Thu so far");
    assert_eq!(text_in(&rt, "week-totals", "total-spend"), "$355");
    assert_eq!(text_in(&rt, "week-totals", "total-agent"), week::hours(w.totals.agent_minutes as i64 * 60_000));
    assert_eq!(text_in(&rt, "week-totals", "total-keyboard"), week::hours(w.totals.keyboard_ms()));
    assert_eq!(text_in(&rt, "week-totals", "total-cold"), "1 cold");
    assert!(inside(&rt, "week-totals", "change").is_none(), "the changes live in the tiles");
    let tue = format!("week-day-{}", date("2026-11-10"));
    for (total, day) in [("total-spend", "day-spend"), ("total-agent", "day-agent"), ("total-keyboard", "day-keyboard"), ("total-cold", "day-cold")] {
        let (a, b) = (rect(&rt, "week-totals", total), rect(&rt, &tue, day));
        assert!((a[0] + a[2] - b[0] - b[2]).abs() < 0.5, "{total} ends where {day} ends: {a:?} {b:?}");
        assert!(a[1] > b[1], "{total} under the days");
    }
    // The changes: in the tiles only.
    assert_eq!(text_in(&rt, "tile-spend", "change"), "↑ $205 (+137%)", "against $150 to Thu 14:20");
    assert_eq!(text_in(&rt, "tile-cold", "change"), "↓ 1 (−50%)");
    // Under the footer, the lines the tiles do not hold.
    assert_eq!(text_of(&rt, "total-note"), "against 2–5 Nov, to 14:20");
    assert!(text_of(&rt, "total-last").starts_with("last week $450 · "), "{}", text_of(&rt, "total-last"));
    // The rhythm lines: from the store at once, from the read when it came.
    assert_eq!(text_in(&rt, "line-sittings", "line-text"), w.totals.sittings_line().split_once(' ').unwrap().1.trim_start());
    assert!(text_in(&rt, "line-agents", "line-text").ends_with("peak 2 on Tue at 14:00"), "{}", text_in(&rt, "line-agents", "line-text"));
    assert_eq!(text_in(&rt, "line-answers", "line-text"), "the agent's turn took a median 1.2m; you answered in a median 3.9m");
    assert_eq!(text_in(&rt, "line-cache", "line-text"), "1 cold return, $5.00 in its first request, $4.00 of it avoidable");
    assert!(text_in(&rt, "line-night", "line-text").starts_with("no agent worked"));
    assert!(text_in(&rt, "line-advice", "line-text").starts_with("▸ 1 return after the cache expired"));
}

#[test]
fn the_column_shows_the_tiles_what_to_change_next_week_and_by_project() {
    let rt = in_week(week_desk(false), (1400.0, 1600.0));
    assert_eq!(text_in(&rt, "tile-spend", "value"), "$355");
    assert_eq!(text_in(&rt, "tile-spend", "change"), "↑ $205 (+137%)");
    assert_eq!(text_in(&rt, "tile-avoidable", "value"), "$14.00");
    assert_eq!(text_in(&rt, "tile-cold", "value"), "1");
    assert_eq!(text_in(&rt, "tile-cold", "small"), "…", "the cold returns' dollars wait for the read");
    assert_eq!(text_in(&rt, "tile-cold", "change"), "↓ 1 (−50%)");
    assert!(text_of(&rt, "changes-state").starts_with("reading the week's logs…"));
    assert!(text_in(&rt, "line-answers", "line-text").starts_with("reading the week's logs…"), "the lines that need the read wait for it");
    assert!(rt.presented("wait-0").is_none(), "no yellow before the read");
    // The read arrives.
    let mut rt = rt;
    rt.update(|s, _| s.week.data.put_logs(WeekKey::new(date("2026-11-09"), ""), logs_with(changes()), 0));
    settle(&mut rt);
    assert_eq!(text_in(&rt, "tile-cold", "small"), "$4.00");
    // Three rows of each part, with their advice.
    let labels: Vec<String> = (0..6).map(|i| text_in(&rt, &format!("wchange-{i}"), "label")).collect();
    assert_eq!(labels, ["idle > TTL", "history edited", "model switch", "effort max or xhigh", "prompts of 400k+", "single lookups"]);
    assert!(texts(&rt).iter().any(|t| t == "compact or wrap up a big session before a long break"));
    assert!(texts(&rt).iter().any(|t| t == "the rows overlap"));
    assert_eq!(text_of(&rt, "next-line"), "$10.00 went to idle > TTL. Compact or wrap up a big session before a long break.");
    assert_eq!(text_in(&rt, "wproj-0", "label"), "app", "a project by its name");
    assert_eq!(text_in(&rt, "wproj-0", "amount"), "$350");
    assert_eq!(text_in(&rt, "wproj-1", "label"), "air");
    assert!(texts(&rt).iter().any(|t| t == "agent hours, each minute once"));
    // A row's sessions open under it, and a session opens in Day.
    click(&mut rt, "wchange-0");
    settle(&mut rt);
    assert_eq!(rt.state.week.open, Some((week::Section::Saved, "idle > TTL".to_string())));
    assert!(inside(&rt, "wchange-1", "session-title").is_some(), "its first session under it");
    click(&mut rt, "wchange-2");
    settle(&mut rt);
    assert_eq!(rt.state.reports.mode, reports::Mode::Day);
    assert_eq!(rt.state.current().map(|r| rt.state.browser.session(r).to_string()).as_deref(), Some("id1"));
}

#[test]
fn more_than_six_projects_fold_into_one_row() {
    let mut desk = week_desk(true);
    let mon = date("2026-11-09");
    let mut store = store();
    for i in 0..7 {
        let mut e = entry(&format!("p{i}"), &format!("~/src/p{i}"));
        spend(&mut e, at(mon, 12, i), 1.0 + i as f64, 0.0);
        store.entries.push(e);
    }
    desk.week.data.put(WeekKey::new(mon, ""), summed(&store, mon, "", now()));
    let rt = in_week(desk, (1400.0, 1800.0));
    assert_eq!(text_in(&rt, "wproj-6", "label"), "3 more", "9 projects: 6 and the rest");
}

#[test]
fn projects_read_their_names_and_their_paths_where_two_share_a_name() {
    let mut desk = week_desk(true);
    let mon = date("2026-11-09");
    let mut store = store();
    for (i, project) in ["~/work/app", "temporary folders", "codex chats"].into_iter().enumerate() {
        let mut e = entry(&format!("q{i}"), project);
        spend(&mut e, at(mon, 12, i as i64), 50.0 - i as f64, 0.0);
        store.entries.push(e);
    }
    desk.week.data.put(WeekKey::new(mon, ""), summed(&store, mon, "", now()));
    let rt = in_week(desk, (1400.0, 1800.0));
    let labels: Vec<String> = (0..5).map(|i| text_in(&rt, &format!("wproj-{i}"), "label")).collect();
    assert_eq!(labels, ["~/src/app", "~/work/app", "temporary folders", "codex chats", "air"], "two `app`s by their paths, the rest by name");
}

#[test]
fn the_bar_s_summary_gives_way_before_the_filter() {
    let width = |rt: &Runtime<Desk>, key: &str| rt.presented(key).map(|r| r.0[2]);
    let full = in_week(week_desk(true), (1500.0, 900.0));
    let n = full.state.week.data.week(&WeekKey::new(date("2026-11-09"), "")).unwrap().totals.sittings.len();
    assert_eq!(text_of(&full, "week-summary"), format!("Mon–Thu so far · {n} sittings"));
    for w in [1100.0, 1000.0, 960.0] {
        let rt = in_week(week_desk(true), (w, 700.0));
        assert_eq!(width(&rt, "filter"), Some(210.0), "the filter keeps its width at {w}");
        if let Some(r) = rt.presented("week-summary") {
            let t = text_of(&rt, "week-summary");
            assert!(t == format!("Mon–Thu so far · {n} sittings") || t == "Mon–Thu so far", "{t} at {w}");
            assert!(rune::text::advance(&t, crate::pal::BODY) <= r.0[2] + 0.5, "{t} is not cut at {w}: {:?}", r.0);
        }
    }
}

#[test]
fn clicks_open_a_day_a_tick_s_session_on_cache_and_filter_by_a_project() {
    let (mon, tue, fri) = (date("2026-11-09"), date("2026-11-10"), date("2026-11-13"));
    let lanes_date = |rt: &Runtime<Desk>| rhythm::date_of(rt.state.browser.day_noon(), &rhythm::local_offset);
    let mut rt = in_week(week_desk(true), (1400.0, 1600.0));
    // A day after now opens nothing.
    click(&mut rt, &format!("week-day-{fri}"));
    settle(&mut rt);
    assert_eq!(rt.state.reports.mode, reports::Mode::Week);
    click(&mut rt, &format!("week-day-{mon}"));
    settle(&mut rt);
    assert_eq!((rt.state.reports.mode, lanes_date(&rt)), (reports::Mode::Day, mon));
    // A tick: Day of its date with its session selected, on Cache.
    command(&mut rt, '2');
    settle(&mut rt);
    let (x, y) = rt.find("tick-0").unwrap();
    rt.pointer_move(x, y);
    settle(&mut rt);
    let line = text_of(&rt, "week-pointer");
    assert!(line.starts_with("Tue 10 Nov · 13:4"), "{line}");
    assert!(line.contains("1 agent at work") && line.contains("cold return: id1 Speed up the"), "{line}");
    click(&mut rt, "tick-0");
    settle(&mut rt);
    assert_eq!(rt.state.reports.mode, reports::Mode::Day);
    assert_eq!(rt.state.current().map(|r| rt.state.browser.session(r).to_string()).as_deref(), Some("id1"));
    assert_eq!((rt.state.tab, lanes_date(&rt)), (inspector::Tab::Cache, tue));
    // A project sets the window's filter and stays on the week.
    command(&mut rt, '2');
    settle(&mut rt);
    click(&mut rt, "wproj-1");
    settle(&mut rt);
    assert_eq!(rt.state.reports.mode, reports::Mode::Week);
    assert_eq!((rt.state.browser.filter(), rt.state.week.text.as_str()), ("~/src/air", "~/src/air"));
    assert_eq!(text_of(&rt, "week-name"), "9–15 Nov 2026");
    // Day's lanes and Sessions keep it.
    command(&mut rt, '1');
    assert_eq!(rt.state.browser.filter(), "~/src/air");
    command(&mut rt, '0');
    settle(&mut rt);
    let listed: Vec<String> = (0..rt.state.browser.len()).map(|r| rt.state.browser.session(r).to_string()).collect();
    assert_eq!(listed, ["id1"], "the table keeps what the filter keeps");
}

#[test]
fn a_tick_of_a_session_not_listed_says_so_and_shows_the_day() {
    let mut rt = in_week(week_desk(true), (1400.0, 1600.0));
    rt.update(|s, tx| week::open_tick(s, date("2026-11-10"), "zz9999", tx));
    settle(&mut rt);
    assert_eq!(rt.state.reports.mode, reports::Mode::Day);
    assert_eq!(rt.state.status.0, "session zz9999 is not in the list");
}

#[test]
fn keys_move_the_cursor_step_weeks_and_esc_steps_back() {
    let lanes_date = |rt: &Runtime<Desk>| rhythm::date_of(rt.state.browser.day_noon(), &rhythm::local_offset);
    let mut rt = in_week(week_desk(true), (1400.0, 1600.0));
    let thu = date("2026-11-12");
    assert_eq!(week::cursor_date(&rt.state), Some(thu), "today in this week");
    press(&mut rt, RuneKey::Down);
    assert_eq!(week::cursor_date(&rt.state), Some(thu), "never after now");
    press(&mut rt, RuneKey::Up);
    press(&mut rt, RuneKey::Text("k".into()));
    press(&mut rt, RuneKey::Enter);
    assert_eq!((rt.state.reports.mode, lanes_date(&rt)), (reports::Mode::Day, date("2026-11-10")));
    command(&mut rt, '2');
    settle(&mut rt);
    press(&mut rt, RuneKey::Home);
    assert_eq!(week::cursor_date(&rt.state), Some(date("2026-11-09")));
    press(&mut rt, RuneKey::End);
    assert_eq!(week::cursor_date(&rt.state), Some(thu));
    // ← steps back a week with the cursor on its Monday; `t` comes back to today.
    press(&mut rt, RuneKey::Left);
    assert_eq!(text_of(&rt, "week-name"), "2–8 Nov 2026");
    assert_eq!(week::cursor_date(&rt.state), Some(date("2026-11-02")));
    typed(&mut rt, "t");
    assert_eq!((text_of(&rt, "week-name").as_str(), week::cursor_date(&rt.state)), ("9–15 Nov 2026", Some(thu)));
    press(&mut rt, RuneKey::Text("[".into()));
    press(&mut rt, RuneKey::Text("]".into()));
    assert_eq!(text_of(&rt, "week-name"), "9–15 Nov 2026");
    // Tab goes to What to change and By project; esc in steps: the days, the filter, Day.
    press(&mut rt, RuneKey::Tab);
    assert_eq!(rt.state.week.part, week::Part::Changes);
    press(&mut rt, RuneKey::Down);
    assert_eq!(rt.state.week.row, 1);
    press(&mut rt, RuneKey::Tab);
    assert_eq!(rt.state.week.part, week::Part::Projects);
    rt.update(|s, _| s.browser.set_filter("app"));
    settle(&mut rt);
    press(&mut rt, RuneKey::Escape);
    assert_eq!((rt.state.week.part, rt.state.browser.filter()), (week::Part::Days, "app"));
    press(&mut rt, RuneKey::Escape);
    assert_eq!((rt.state.reports.mode, rt.state.browser.filter()), (reports::Mode::Week, ""));
    press(&mut rt, RuneKey::Escape);
    assert_eq!(rt.state.reports.mode, reports::Mode::Day);
    // `/` starts the filter in Week; ⏎ keeps it and Week counts it.
    command(&mut rt, '2');
    settle(&mut rt);
    typed(&mut rt, "/air");
    assert!(rt.state.browser.editing());
    assert_eq!(rt.state.week.text, "", "typing starts nothing");
    press(&mut rt, RuneKey::Enter);
    assert_eq!((rt.state.browser.filter(), rt.state.week.text.as_str()), ("air", "air"));
    // By project by keys: ⏎ filters by it.
    rt.update(|s, _| s.browser.clear_filter());
    settle(&mut rt);
    rt.update(|s, _| {
        s.week.part = week::Part::Projects;
        s.week.row = 0;
    });
    press(&mut rt, RuneKey::Enter);
    assert_eq!(rt.state.browser.filter(), "~/src/app");
}

#[test]
fn the_progress_shows_while_the_store_is_filled_and_the_client_line() {
    let mut desk = week_desk(false);
    desk.calendar.data = tokprof_browse::calendar::Calendars::detached();
    desk.week.client_ignored = true;
    let mut rt = in_week(desk, (1400.0, 1600.0));
    assert_eq!(text_of(&rt, "week-progress"), "reading the logs into the store…");
    assert_eq!(text_of(&rt, "week-client"), "Week counts every client: the day store has no client");
    rt.update(|s, _| s.calendar.data.put_with(tokprof_core::calendar::Calendar::default(), store(), 0));
    settle(&mut rt);
    assert!(rt.presented("week-progress").is_none());
    assert_eq!(text_in(&rt, "tile-spend", "value"), "$355");
    // At rest: no poll asks for a frame.
    assert!(!rt.update_if(|s, tx| poll(s, 0.0, tx)), "Week draws no more frames while nothing changes");
    assert!(!rt.animating);
}

#[test]
fn the_week_s_logs_are_read_on_the_worker_only_while_week_shows() {
    let (catalog, select) = fixture();
    let mut desk = week_desk(false);
    desk.week.data = tokprof_browse::week::Weeks::start(catalog, Pricing::defaults(), select, Arc::new(|| {}));
    let mut rt = runtime(desk, (1400.0, 1600.0));
    assert!(rt.state.week.data.progress().is_none(), "Day reads nothing for Week");
    command(&mut rt, '2');
    rt.frame(1.0 / 60.0);
    let key = week::key(&rt.state);
    assert!(rt.state.week.data.reading(&key), "Week asks for its read");
    let deadline = Instant::now() + Duration::from_secs(30);
    while rt.state.week.data.logs(&key).is_none() {
        assert!(Instant::now() < deadline, "the read did not arrive");
        std::thread::sleep(Duration::from_millis(20));
        rt.frame(1.0 / 60.0);
    }
    settle(&mut rt);
    assert!(!text_of(&rt, "changes-state").starts_with("reading the week's logs…"));
}

#[test]
fn a_filter_that_matches_nothing_a_match_by_title_only_and_a_project_filter() {
    let mon = date("2026-11-09");
    let mut desk = week_desk(true);
    for i in 0..3 {
        desk.browser.put_glance(&format!("id{i}"), Glance { title: Some(format!("Port the Python tool {i}")), ..Glance::default() });
    }
    let store = store();
    for text in ["zzz", "python", "~/src/air"] {
        desk.week.data.put(WeekKey::new(mon, text), summed(&store, mon, text, now()));
    }
    let mut rt = in_week(desk, (1400.0, 1600.0));
    rt.update(|s, _| s.browser.set_filter("zzz"));
    settle(&mut rt);
    assert_eq!(text_of(&rt, "week-nothing"), "Nothing in 9–15 Nov 2026 matches \"zzz\".");
    rt.update(|s, _| s.browser.set_filter("python"));
    settle(&mut rt);
    assert_eq!(
        text_of(&rt, "week-titles"),
        "3 sessions match \"python\" only by title; Week counts agents, projects and session ids, not titles. Sessions (⌘0) lists them."
    );
    assert_eq!(rt.state.browser.len(), 3, "Day's lanes and Sessions keep them");
    rt.update(|s, _| s.browser.set_filter("~/src/air"));
    settle(&mut rt);
    assert_eq!(text_of(&rt, "week-filter"), "Project \"air\": Week counts the logs whose agent, project or session id holds its path.");
    assert_eq!(text_in(&rt, "tile-spend", "value"), "$5.00", "only the project");
    assert_eq!(text_in(&rt, "wproj-0", "label"), "air");
    assert!(inside(&rt, "week-column", "wproj-1").is_none());
}

#[test]
fn a_first_week_gives_the_pace_per_day_and_no_logs_before_it() {
    let (mon, wed) = (date("2026-03-02"), date("2026-03-04"));
    let mut e = entry("id0", "~/src/app");
    for d in 0..5 {
        spend(&mut e, at(wed + d, 10, 0), 140.0, 0.0);
        messages(&mut e, at(wed + d, 10, 0), at(wed + d, 10, 10));
    }
    let store = Store { entries: vec![e], ..Store::default() };
    let clock = at(date("2026-03-12"), 12, 0);
    let mut desk = desk_of(3);
    let mut weeks = tokprof_browse::week::Weeks::detached();
    weeks.put(WeekKey::new(mon, ""), summed(&store, mon, "", clock));
    desk.calendar.data.put_with(tokprof_core::calendar::Calendar::default(), store, 0);
    desk.week = week::State::new(weeks);
    desk.week.clock = Some(clock);
    let mut rt = in_week(desk, (1400.0, 1600.0));
    rt.update(|s, _| s.week.monday = Some(mon));
    settle(&mut rt);
    assert_eq!(text_in(&rt, "tile-spend", "value"), "$700");
    assert_eq!(text_in(&rt, "tile-spend", "change"), "$140 a day");
    assert_eq!(text_in(&rt, "week-totals", "total-spend"), "$700", "the footer has no pace");
    assert!(inside(&rt, "week-totals", "change").is_none());
    assert_eq!(text_of(&rt, "tiles-note"), "no logs in 23 Feb – 1 Mar");
    assert_eq!(text_in(&rt, &format!("week-day-{mon}"), "day-nologs"), "no logs");
    assert_eq!(text_in(&rt, &format!("week-day-{}", mon + 1), "day-nologs"), "no logs");
    assert_eq!(text_in(&rt, &format!("week-day-{wed}"), "day-spend"), "$140");
}

#[test]
fn the_week_scene_and_its_flags() {
    let cli = |a: &[&str]| Cli::parse_from(std::iter::once("tokprof-desk").chain(a.iter().copied()));
    assert_eq!(crate::week_of(&cli(&["--week", "4.11"])).unwrap_err().to_string(), "--week takes YYYY-MM-DD");
    assert_eq!(crate::week_of(&cli(&["--week", "2999-05-03"])).unwrap_err().to_string(), "the week of 2999-05-03 has not begun");
    assert_eq!(crate::week_of(&cli(&["--week", "2026-11-04"])).unwrap(), Some(date("2026-11-04")));
    // The frame of a made-up week: the store and the read already there.
    let mut desk = week_desk(true);
    desk.week_wanted = Some(date("2026-11-10"));
    let scene = SCENES.iter().find(|(name, _)| *name == "week").expect("the week scene");
    (scene.1)(&mut desk);
    assert!(SCENES.iter().all(|(name, _)| *name != "reports-week"), "Reports has no week");
    let rt = runtime(desk, (1500.0, 950.0));
    assert_eq!(rt.state.reports.mode, reports::Mode::Week);
    assert_eq!(text_of(&rt, "week-name"), "9–15 Nov 2026");
    assert_eq!(text_in(&rt, "tile-spend", "value"), "$355");
}
