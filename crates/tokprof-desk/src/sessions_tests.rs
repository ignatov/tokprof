//! The Sessions screen in the window: the bar, the table and its columns, the days' headers, the
//! sort, the quick filters, the footer, Projects, the inspector, the keys and the scenes, on a few
//! made-up sessions.

use tokprof_browse::browser::QuickFilters;
use tokprof_core::daystore::{Entry, Minute, Store};
use tokprof_core::fmt::local_time;
use tokprof_core::xray::Glance;

use super::*;
use crate::sessions::{self, Col, Slot};

const MIN: i64 = 60_000;
const HOUR: i64 = 3_600_000;

fn today() -> i64 {
    rhythm::date_of(now_ms(), &rhythm::local_offset)
}

fn midnight(date: i64) -> i64 {
    rhythm::midnight(date, &rhythm::local_offset)
}

/// A time of today `k` minutes ago, or, so soon after midnight that it would be yesterday, `k`
/// seconds before the first minute, so that the order stays.
fn today_at(k: i64) -> i64 {
    let now = now_ms();
    let t = now - k * MIN;
    if t >= midnight(today()) { t } else { midnight(today()) + (120 - k) * 1000 }
}

/// The day the window finds a moment in before the store comes, and the day Day opens a selected
/// session on: the local date five hours earlier. Just after midnight that is yesterday.
fn early_day(ms: i64) -> i64 {
    rhythm::date_of(ms - 5 * HOUR, &rhythm::local_offset)
}

/// A time `days` days before today, at `hour`.
fn day_at(days: i64, hour: i64) -> i64 {
    midnight(today() - days) + hour * HOUR
}

/// Session `i` of `agent` in `project`, last active at `active` ms.
fn srow(i: usize, agent: &str, project: &str, active: i64) -> SessionRow {
    let mut r = row(i, project, 0);
    r.file.agent = AgentId::unknown(agent);
    r.active_ms = active;
    r.file.modified_ns = active as i128 * 1_000_000;
    r
}

struct G {
    cost: f64,
    requests: u64,
    started: i64,
    avoidable: f64,
    cold: u64,
    models: Vec<(&'static str, u64)>,
    title: &'static str,
    /// While warm, when it goes cold; else it went cold an hour ago.
    warm_until: Option<i64>,
    working: Option<i64>,
}

fn glance(g: G) -> Glance {
    let now = now_ms();
    Glance {
        cost: g.cost,
        requests: g.requests,
        started_ms: Some(g.started),
        avoidable: g.avoidable,
        cold_returns: g.cold,
        models: g.models.into_iter().map(|(m, n)| (m.to_string(), n)).collect(),
        title: Some(g.title.to_string()),
        next_warm: Some(0.12),
        next_cold: Some(2.10),
        cold_at_ms: g.warm_until.unwrap_or(now - HOUR),
        last_ms: now - 2 * MIN,
        working_last: g.working,
        ..Glance::default()
    }
}

/// An entry of the day store: what session `main` of `agent` spent at each time.
fn spent(agent: &str, main: &str, at: &[(i64, f64)]) -> Entry {
    Entry {
        key: format!("/x/{main}.jsonl"),
        agent: agent.into(),
        session: main.into(),
        main: main.into(),
        minutes: at.iter().map(|&(ts, usd)| Minute { minute: ts / MIN, usd, requests: 1, ..Minute::default() }).collect(),
        ..Entry::default()
    }
}

/// Five sessions: two today, two yesterday, one two days ago that is still being costed.
/// - id0, claude in ~/src/app, runs now, warm, $118 of which $90 spent yesterday and $28 today;
/// - id1, codex in ~/src/tool, warm, $2.23 today, $1.79 avoidable and a cold return;
/// - id2, claude in ~/src/app, cold, $212.40 yesterday, $14.80 avoidable and 2 cold returns;
/// - id3, codex in ~/src/tool, cold, $38.20 yesterday, its title holds `Python`;
/// - id4, claude in ~/src/app, not costed yet.
fn table_desk(store: bool) -> Desk {
    let now = now_ms();
    let rows = vec![
        srow(0, "claude", "~/src/app", today_at(2)),
        srow(1, "codex", "~/src/tool", today_at(10)),
        srow(2, "claude", "~/src/app", day_at(1, 22)),
        srow(3, "codex", "~/src/tool", day_at(1, 10)),
        srow(4, "claude", "~/src/app", day_at(2, 12)),
    ];
    let mut desk = Desk::new(Browser::detached(Arc::new(Catalog::new(Vec::new(), PathBuf::new())), rows, Pricing::defaults(), frontend()), false);
    let b = &mut desk.browser;
    b.put_glance(
        "id0",
        glance(G {
            cost: 118.0,
            requests: 1606,
            started: day_at(1, 15),
            avoidable: 0.0,
            cold: 0,
            models: vec![("claude-opus-5-5", 1465), ("claude-fable-5-2", 141)],
            title: "Python chat bugs",
            warm_until: Some(now + 20 * MIN),
            working: Some(now - MIN),
        }),
    );
    b.put_glance(
        "id1",
        glance(G {
            cost: 2.23,
            requests: 64,
            started: today_at(70),
            avoidable: 1.79,
            cold: 1,
            models: vec![("gpt-5.6-codex", 64)],
            title: "Skip old logs",
            warm_until: Some(now + 5 * MIN),
            working: None,
        }),
    );
    b.put_glance(
        "id2",
        glance(G {
            cost: 212.40,
            requests: 2914,
            started: day_at(1, 17),
            avoidable: 14.80,
            cold: 2,
            models: vec![("claude-opus-5-5", 2914)],
            title: "Profiler order",
            warm_until: None,
            working: None,
        }),
    );
    b.put_glance(
        "id3",
        glance(G {
            cost: 38.20,
            requests: 402,
            started: day_at(1, 8),
            avoidable: 1.20,
            cold: 0,
            models: vec![("gpt-5.6-codex", 402)],
            title: "Python review",
            warm_until: None,
            working: None,
        }),
    );
    if store {
        let entries = vec![
            spent("claude", "id0", &[(day_at(1, 16), 90.0), (today_at(3), 28.0)]),
            spent("codex", "id1", &[(today_at(11), 2.23)]),
            spent("claude", "id2", &[(day_at(1, 21), 212.40)]),
            spent("codex", "id3", &[(day_at(1, 9), 38.20)]),
        ];
        desk.calendar.data.put_with(tokprof_core::calendar::Calendar::default(), Store { entries, ..Store::default() }, 0);
    }
    desk
}

/// The window in Sessions on `desk`, `size` points.
fn in_sessions(desk: Desk, size: (f32, f32)) -> Runtime<Desk> {
    let mut rt = runtime(desk, size);
    command(&mut rt, '0');
    settle(&mut rt);
    rt
}

fn text_of(rt: &Runtime<Desk>, key: &str) -> String {
    rt.elements().into_iter().find(|e| e.key.as_deref() == Some(key)).and_then(|e| e.text).unwrap_or_default()
}

/// The ids of the table's rows, in their order.
fn order(rt: &Runtime<Desk>) -> Vec<String> {
    (0..rt.state.browser.len()).map(|r| rt.state.browser.session(r).to_string()).collect()
}

fn header(rt: &Runtime<Desk>, days_ago: i64) -> String {
    text_in(rt, &format!("sday-{}", today() - days_ago), "day-total")
}

#[test]
fn the_button_command_0_and_the_palette_show_sessions_from_every_screen() {
    for from in [reports::Mode::Day, reports::Mode::Month, reports::Mode::Calendar] {
        let mut rt = runtime(table_desk(true), (1400.0, 900.0));
        reports::show(&mut rt.state, from);
        settle(&mut rt);
        click(&mut rt, "sessions-button");
        settle(&mut rt);
        assert_eq!(rt.state.reports.mode, reports::Mode::Sessions, "from {from:?}");
        for shown in ["sessions-toolbar", sessions::TABLE, "sessions-foot", "all-sessions", "all-since", "today", "filter", "live"] {
            assert!(rt.presented(shown).is_some() || shown == "live", "{shown} in Sessions");
        }
        for gone in ["lanes", "details", "day-name", "zoom", "day-back"] {
            assert!(rt.presented(gone).is_none(), "{gone} is not shown in Sessions");
        }
        assert_eq!(text_of(&rt, "all-sessions"), "All sessions");
        let since = text_of(&rt, "all-since");
        assert!(since.starts_with("since ") && since.ends_with(" · 5 sessions"), "{since}");
        command(&mut rt, '1');
        settle(&mut rt);
        assert_eq!(rt.state.reports.mode, reports::Mode::Day, "⌘1 shows Day");
        reports::show(&mut rt.state, from);
        settle(&mut rt);
        command(&mut rt, '0');
        settle(&mut rt);
        assert_eq!(rt.state.reports.mode, reports::Mode::Sessions, "⌘0 from {from:?}");
    }
    // The palette's `Sessions`, and the selection kept in Day.
    let mut rt = runtime(table_desk(true), (1400.0, 900.0));
    run_from_palette(&mut rt, "sessions", palette::Pick::Act(palette::Action::Sessions));
    settle(&mut rt);
    assert_eq!(rt.state.reports.mode, reports::Mode::Sessions);
    click(&mut rt, "id2");
    command(&mut rt, '1');
    settle(&mut rt);
    let selected = rt.state.browser.selected().map(|r| rt.state.browser.session(r).to_string());
    assert_eq!(selected.as_deref(), Some("id2"), "the session selected in Sessions stays selected in Day");
}

#[test]
fn every_column_shows_at_1400_and_model_started_and_requests_go_at_960() {
    let rt = in_sessions(with_selection(table_desk(true)), (1400.0, 900.0));
    assert!(sessions::inspector_shown(&rt.state), "the inspector is open");
    for c in Col::ALL.into_iter().filter(|&c| c != Col::Agent) {
        assert!(rt.presented(&format!("head-{}", c.name())).is_some(), "{} at 1400 points", c.title());
    }
    let rt = in_sessions(with_selection(table_desk(true)), (960.0, 640.0));
    for c in [Col::Model, Col::Started, Col::Requests] {
        assert!(rt.presented(&format!("head-{}", c.name())).is_none(), "{} is left out at 960 points", c.title());
    }
    for c in [Col::Last, Col::Project, Col::Title, Col::Duration, Col::Cost, Col::Avoidable, Col::Cold, Col::Next] {
        assert!(rt.presented(&format!("head-{}", c.name())).is_some(), "{} stays at 960 points", c.title());
    }
    let title = inside(&rt, "id0", "col-title").expect("the title");
    assert!(title.rect[2] >= 60.0, "the title keeps the room that is left: {:?}", title.rect);
    assert_eq!(sessions::fit(1400.0 - 361.0, false, false, true).len(), 12);
}

#[test]
fn the_cells_read_their_values_and_dots_while_being_costed() {
    let rt = in_sessions(table_desk(true), (1400.0, 900.0));
    let b = &rt.state.browser;
    let now = now_ms();
    let cell = |id: &str, c: &str| text_in(&rt, id, c);
    assert_eq!(cell("id0", "col-model"), "claude-opus-5-5 +1", "the model of most requests, and how many others");
    assert_eq!(cell("id0", "col-requests"), "1,606");
    assert_eq!(cell("id0", "col-cost"), "$118");
    assert_eq!((cell("id0", "col-avoidable"), cell("id0", "col-cold")), ("—".into(), "—".into()), "nothing to avoid");
    assert_eq!((cell("id2", "col-avoidable"), cell("id2", "col-cold")), ("$14.80".into(), "2".into()));
    let p = pal::pal(rt.state.theme.get());
    assert_eq!(inside(&rt, "id2", "col-avoidable").and_then(|e| e.text_color), Some(p.cold), "in the theme's red");
    assert_eq!(inside(&rt, "id0", "col-avoidable").and_then(|e| e.text_color), Some(p.ink3), "in the dim ink");
    for c in ["col-requests", "col-cost", "col-avoidable", "col-cold", "col-model", "next-text"] {
        assert_eq!(cell("id4", c), "…", "{c} while being costed");
    }
    // Under a day's header, the last activity's time; started the day before, its day and time.
    let r0 = b.row_of("id0").unwrap();
    assert_eq!(cell("id0", "col-last"), local_time(b.active_ms(r0), "%H:%M"));
    let start = b.glance(r0).unwrap().started_ms.unwrap();
    assert_eq!(cell("id0", "col-started"), sessions::when(start, true, now));
    assert_eq!(cell("id0", "col-duration"), tokprof_core::fmt::human_duration((b.active_ms(r0) - start) as f64 / 1000.0));
    assert_eq!(cell("id2", "col-started"), "17:00", "on the date of its last activity");
    // The warm countdown, in green.
    let next = cell("id0", "next-text");
    assert!(next.starts_with("19:") || next.starts_with("20:"), "{next}");
    assert_eq!(inside(&rt, "id0", "next-text").and_then(|e| e.text_color), Some(p.warm));
}

#[test]
fn the_countdown_wakes_the_window_only_while_a_warm_row_is_on_the_screen() {
    let rt = in_sessions(table_desk(true), (1400.0, 900.0));
    let now = now_ms();
    assert!(sessions::counting(&rt.state, now), "a warm row counts down on the screen");
    assert!(!sessions::counting(&rt.state, now + 30 * MIN), "nothing is warm half an hour later");
    let mut desk = table_desk(true);
    desk.browser.set_quick_filters(QuickFilters { agents: vec!["codex".into()], ..QuickFilters::default() });
    desk.browser.put_glance("id1", Glance { cost: 2.23, cold_at_ms: now - MIN, next_cold: Some(0.5), ..Glance::default() });
    let rt = in_sessions(desk, (1400.0, 900.0));
    assert!(!sessions::counting(&rt.state, now), "only cold rows: the window sleeps");
}

#[test]
fn days_headers_count_the_day_s_spend_of_what_the_filters_keep() {
    let mut rt = in_sessions(table_desk(true), (1400.0, 900.0));
    assert_eq!(header(&rt, 0), "2 sessions · $30.23 spent", "$28 of id0 and $2.23 of id1");
    assert_eq!(text_in(&rt, "id0", "col-cost"), "$118", "a row keeps its whole-life cost: a header is not the sum of its rows");
    assert_eq!(header(&rt, 1), "2 sessions · $341 spent", "yesterday holds id0's other $90, with $212.40 and $38.20");
    assert_eq!(text_in(&rt, &format!("sday-{}", today()), "day-label"), "Today");
    // A project picked: the spend of its sessions on each day, wherever they are listed.
    rt.update(|s, _| s.browser.set_quick_filters(QuickFilters { project: Some("~/src/app".into()), ..QuickFilters::default() }));
    settle(&mut rt);
    assert_eq!(header(&rt, 0), "1 session · $28.00 spent");
    assert_eq!(header(&rt, 1), "1 session · $302 spent", "id0's $90 included");
    // Warm and codex: id1 alone; it leaves when its cache goes cold.
    rt.update(|s, _| s.browser.set_quick_filters(QuickFilters { warm: true, agents: vec!["codex".into()], ..QuickFilters::default() }));
    settle(&mut rt);
    assert_eq!(order(&rt), ["id1"]);
    assert_eq!(header(&rt, 0), "1 session · $2.23 spent");
    rt.update(|s, _| s.browser.put_glance("id1", Glance { cost: 2.23, cold_at_ms: now_ms() - 1, next_cold: Some(0.5), ..Glance::default() }));
    settle(&mut rt);
    assert!(order(&rt).is_empty(), "a session that went cold leaves the set");
    assert!(rt.presented(&format!("sday-{}", today())).is_none());
    // A filter that keeps two sessions by their titles only.
    rt.update(|s, _| s.browser.set_quick_filters(QuickFilters::default()));
    rt.update(|s, _| s.browser.set_filter("python"));
    settle(&mut rt);
    assert_eq!(order(&rt), ["id0", "id3"]);
    assert_eq!(header(&rt, 0), "1 session · $28.00 spent");
    assert_eq!(header(&rt, 1), "1 session · $128 spent", "id0's $90 and id3's $38.20");
}

#[test]
fn headers_wait_for_the_store_go_in_other_orders_and_open_their_day() {
    // Before the store, the rows go under the days of the local dates five hours earlier: after
    // 05:00 id0 and id1 are today's, and just after midnight they are yesterday's with id2 and id3.
    let rt = in_sessions(table_desk(false), (1400.0, 900.0));
    let current = early_day(now_ms());
    let n = (0..rt.state.browser.len()).filter(|&r| early_day(rt.state.browser.active_ms(r)) == current).count();
    assert!(n >= 1, "the newest session is on the current day");
    let want = format!("{n} session{} · … spent", if n == 1 { "" } else { "s" });
    assert_eq!(text_in(&rt, &format!("sday-{current}"), "day-total"), want, "the store is not there yet");
    let mut rt = in_sessions(table_desk(true), (1400.0, 900.0));
    rt.update(|s, _| s.browser.sort_by(Sort::Cost));
    settle(&mut rt);
    assert!(rt.elements().iter().all(|e| e.key.as_deref() != Some("day-total")), "no header sorted by cost");
    let r0 = rt.state.browser.row_of("id0").unwrap();
    assert_eq!(text_in(&rt, "id0", "col-last"), sessions::when(rt.state.browser.active_ms(r0), true, now_ms()), "each time with its day");
    rt.update(|s, _| s.browser.set_sort(Sort::Time, true));
    settle(&mut rt);
    assert!(rt.elements().iter().all(|e| e.key.as_deref() != Some("day-total")), "nor oldest first");
    rt.update(|s, _| s.browser.set_sort(Sort::Time, false));
    settle(&mut rt);
    rt.update(|s, _| s.select_session("id0"));
    settle(&mut rt);
    click(&mut rt, &format!("sday-{}", today() - 1));
    settle(&mut rt);
    assert_eq!(rt.state.reports.mode, reports::Mode::Day, "a header opens its day");
    assert_eq!(rhythm::date_of(rt.state.browser.day_noon(), &rhythm::local_offset), today() - 1);
    assert_eq!(rt.state.current(), None, "with no session selected");
}

#[test]
fn a_header_click_sorts_by_its_column_and_another_reverses_it() {
    let mut rt = in_sessions(table_desk(true), (1400.0, 900.0));
    click(&mut rt, "head-avoidable");
    settle(&mut rt);
    assert_eq!(order(&rt), ["id2", "id1", "id3", "id0", "id4"], "the most avoidable first, the one being costed last");
    assert_eq!(text_in(&rt, "head-avoidable", "head-text"), "Avoidable ↓");
    click(&mut rt, "head-avoidable");
    settle(&mut rt);
    assert_eq!(order(&rt), ["id0", "id3", "id1", "id2", "id4"], "the fewest first, the one being costed still last");
    assert_eq!(text_in(&rt, "head-avoidable", "head-text"), "Avoidable ↑");
    click(&mut rt, "head-title");
    settle(&mut rt);
    assert_eq!(text_in(&rt, "head-title", "head-text"), "Title ↑", "text sorts from A first");
    // Back to time order, newest first, with its headers.
    rt.update(|s, _| s.browser.set_sort(Sort::Time, true));
    click(&mut rt, "head-last");
    settle(&mut rt);
    assert_eq!((rt.state.browser.sort_order(), rt.state.browser.sort_reversed()), (Sort::Time, false));
    assert!(rt.presented(&format!("sday-{}", today())).is_some());
    // A cost that arrives leaves the rows where they are, until `r`.
    click(&mut rt, "head-cost");
    settle(&mut rt);
    assert_eq!(order(&rt), ["id2", "id0", "id3", "id1", "id4"]);
    rt.update(|s, _| s.browser.put_glance("id4", Glance { cost: 900.0, cold_at_ms: i64::MAX, ..Glance::default() }));
    settle(&mut rt);
    assert_eq!(order(&rt), ["id2", "id0", "id3", "id1", "id4"], "rows in place");
    assert_eq!(text_in(&rt, "id4", "col-cost"), "$900");
    press(&mut rt, RuneKey::Text("r".into()));
    assert_eq!(order(&rt)[0], "id4", "r sorts again");
}

#[test]
fn pills_combine_count_and_all_clears_them() {
    let mut rt = in_sessions(table_desk(true), (1400.0, 900.0));
    let count = |rt: &Runtime<Desk>, pill: &str| text_in(rt, pill, "pill-count");
    assert_eq!(count(&rt, "pill-running"), "1");
    assert_eq!(count(&rt, "pill-warm"), "2");
    assert_eq!((count(&rt, "pill-agent-claude"), count(&rt, "pill-agent-codex")), ("3".into(), "2".into()));
    let week = rt.state.browser.quick_count(&QuickFilters { this_week: true, ..QuickFilters::default() }, now_ms());
    assert_eq!(count(&rt, "pill-week"), week.to_string());
    click(&mut rt, "pill-warm");
    click(&mut rt, "pill-agent-codex");
    settle(&mut rt);
    assert_eq!(order(&rt), ["id1"], "warm and codex: the warm Codex session");
    assert_eq!(text_of(&rt, "foot-title"), "1 session · warm · codex");
    click(&mut rt, "pill-agent-claude");
    settle(&mut rt);
    assert_eq!(order(&rt), ["id0", "id1"], "two agents keep either, still warm");
    click(&mut rt, "pill-all");
    settle(&mut rt);
    assert_eq!(rt.state.browser.len(), 5);
    assert!(rt.state.browser.quick_filters().is_empty());
    // The counts follow the bar's filter.
    rt.update(|s, _| s.browser.set_filter("python"));
    settle(&mut rt);
    assert_eq!(count(&rt, "pill-agent-codex"), "1");
    rt.update(|s, _| s.browser.clear_filter());
    // The project picker.
    click(&mut rt, "project-picker");
    let menu = rt.popup_menu().cloned().expect("the picker opens its menu");
    assert_eq!(menu.find("any").map(|i| i.title().to_string()).as_deref(), Some("Project: any"));
    assert_eq!(menu.find("p:~/src/app").map(|i| i.title().to_string()).as_deref(), Some("app · 3"), "by its name, as the lanes label it");
    rt.resolve_popup(Some("p:~/src/tool")).unwrap();
    settle(&mut rt);
    assert_eq!(order(&rt), ["id1", "id3"]);
    assert_eq!(text_in(&rt, "project-picker", "picker-label"), "tool", "the project's name, as its cells read");
    // `Waiting first` with the setting on.
    assert!(rt.presented("waiting-first").is_none());
    rt.update(|s, _| s.pick_waiting(true));
    settle(&mut rt);
    click(&mut rt, "waiting-first");
    settle(&mut rt);
    assert_eq!(rt.state.browser.sort_order(), Sort::Waiting);
    assert!(texts(&rt).iter().any(|t| t == "Nobody waits for you"));
}

#[test]
fn the_footer_totals_the_whole_set_not_the_rows_on_the_screen() {
    let rows = (0..65).map(|i| srow(i, "claude", "~/src/app", today_at(i as i64))).collect();
    let mut desk = Desk::new(Browser::detached(Arc::new(Catalog::new(Vec::new(), PathBuf::new())), rows, Pricing::defaults(), frontend()), false);
    for i in 0..60 {
        let g = Glance { cost: 1.5, avoidable: 0.25, cold_returns: 1, cold_at_ms: i64::MAX, ..Glance::default() };
        desk.browser.put_glance(&format!("id{i}"), g);
    }
    let rt = in_sessions(desk, (1400.0, 640.0));
    let shown = (0..65).filter(|i| rt.presented(&format!("id{i}")).is_some_and(|(r, _)| r[1] < 640.0)).count();
    assert!(shown < 30, "{shown} rows on the screen");
    assert_eq!(text_of(&rt, "foot-title"), "65 sessions · all time · costing 5…");
    assert_eq!((text_of(&rt, "foot-cost"), text_of(&rt, "foot-avoidable"), text_of(&rt, "foot-cold")), ("$90.00".into(), "$15.00".into(), "60".into()));
    assert_eq!(text_of(&rt, "foot-share"), "16.7%");
    // The wheel scrolls the table and leaves the selection where it was.
    let mut rt = rt;
    let (x, y) = rt.find("id3").unwrap();
    rt.wheel(x, y, -400.0);
    settle(&mut rt);
    assert!(rt.state.sessions.offset.get() > 300.0, "the wheel scrolls the table");
    assert_eq!(rt.state.browser.selected(), Some(0));
}

#[test]
fn projects_fold_unfold_by_click_and_keys_with_their_subtotals() {
    let mut rt = in_sessions(table_desk(true), (1400.0, 900.0));
    click(&mut rt, "mode-projects");
    settle(&mut rt);
    let layout = sessions::layout(&rt.state);
    assert_eq!(layout.groups.iter().map(|g| g.project.as_str()).collect::<Vec<_>>(), ["~/src/app", "~/src/tool"], "the costliest first");
    let app = "proj-~/src/app";
    assert_eq!(text_in(&rt, app, "project-sessions"), "3 sessions");
    assert_eq!(text_in(&rt, app, "col-cost"), "$330", "$118 and $212.40; the one being costed adds nothing yet");
    assert_eq!(text_in(&rt, app, "project-share"), "89% of spend");
    assert_eq!((text_in(&rt, app, "col-avoidable"), text_in(&rt, app, "col-cold")), ("$14.80".into(), "2".into()));
    assert_eq!(text_in(&rt, "proj-~/src/tool", "col-cost"), "$40.43");
    assert!(rt.presented("id0").is_none(), "groups start folded");
    click(&mut rt, app);
    settle(&mut rt);
    assert!(["id0", "id2", "id4"].iter().all(|id| rt.presented(id).is_some()), "a click unfolds it");
    let r2 = rt.state.browser.row_of("id2").unwrap();
    assert_eq!(text_in(&rt, "id2", "col-last"), sessions::when(rt.state.browser.active_ms(r2), true, now_ms()), "each time with its day");
    click(&mut rt, app);
    settle(&mut rt);
    assert!(rt.presented("id0").is_none(), "another folds it");
    // By keys: → unfolds the cursor's project, ← folds it with the cursor on its row.
    assert_eq!(rt.state.sessions.cursor.as_deref(), Some("~/src/app"));
    press(&mut rt, RuneKey::Right);
    assert!(rt.presented("id0").is_some());
    press(&mut rt, RuneKey::Down);
    assert_eq!(rt.state.sessions.cursor, None, "↓ goes into its sessions");
    press(&mut rt, RuneKey::Left);
    assert!(rt.presented("id0").is_none());
    assert_eq!(rt.state.sessions.cursor.as_deref(), Some("~/src/app"));
    press(&mut rt, RuneKey::Enter);
    assert!(rt.presented("id0").is_some(), "⏎ on a project's row unfolds it");
    // A filter in Projects.
    click(&mut rt, "pill-agent-codex");
    settle(&mut rt);
    assert_eq!(sessions::layout(&rt.state).groups.len(), 1, "only the projects with sessions kept");
}

#[test]
fn the_inspector_shows_day_s_summary_in_one_column() {
    let (catalog, select) = fixture();
    let mut browser = Browser::start(catalog, Pricing::defaults(), select, false, Arc::new(|| {}), frontend()).unwrap();
    browser.settle();
    let mut rt = runtime(with_selection(Desk::new(browser, false)), (1400.0, 900.0));
    let numbers = |rt: &Runtime<Desk>, pane: &str| ["spend-total", "next-cost", "way-cost", "next-value"].map(|k| text_in(rt, pane, k)).to_vec();
    let day = numbers(&rt, "inspector");
    assert!(!day[0].is_empty(), "Day's Summary has its spend");
    let session = rt.state.browser.session(0).to_string();
    command(&mut rt, '0');
    settle(&mut rt);
    assert!(rt.presented("sessions-inspector").is_some());
    assert_eq!(numbers(&rt, "sessions-inspector"), day, "the same numbers as Day's Summary");
    let w = rt.presented("sessions-inspector").unwrap().0[2];
    assert!((w - sessions::INSPECTOR_W).abs() < 0.5, "{w} points wide");
    for k in ["open-in-day", "handoff", "inspector-close", "cache-line", "in-day-note", "header"] {
        assert!(inside(&rt, "sessions-inspector", k).is_some(), "{k} in the inspector");
    }
    assert_eq!(text_in(&rt, "sessions-inspector", "in-day-note"), "Requests, Spend, Work, Waits, Prompts, Tools, Context, Cache and Flame open in Day.");
    assert!(inside(&rt, "sessions-inspector", "tabs").is_none(), "no other tab");
    click(&mut rt, "inspector-close");
    settle(&mut rt);
    assert!(rt.presented("sessions-inspector").is_none(), "closed");
    assert_eq!(rt.state.browser.selected(), Some(0), "the row stays selected");
    click(&mut rt, &session);
    settle(&mut rt);
    assert!(rt.presented("sessions-inspector").is_some(), "a click on a row opens it");
    click(&mut rt, "open-in-day");
    settle(&mut rt);
    assert_eq!(rt.state.reports.mode, reports::Mode::Day);
    assert_eq!(rt.state.browser.selected().map(|r| rt.state.browser.session(r).to_string()), Some(session));
    assert_eq!(rt.state.tab, inspector::Tab::Summary);
}

#[test]
fn keys_move_open_step_back_sort_and_quit() {
    let mut rt = in_sessions(table_desk(true), (1400.0, 900.0));
    let selected = |rt: &Runtime<Desk>| rt.state.current().map(|r| rt.state.browser.session(r).to_string()).unwrap_or_default();
    assert_eq!(selected(&rt), "", "none selected to start with");
    assert!(!sessions::inspector_shown(&rt.state), "and no inspector");
    press(&mut rt, RuneKey::Down);
    assert_eq!(selected(&rt), "id0", "↓ selects the first row");
    press(&mut rt, RuneKey::Down);
    assert_eq!(selected(&rt), "id1");
    press(&mut rt, RuneKey::Text("j".into()));
    assert_eq!(selected(&rt), "id2", "past yesterday's header");
    assert!(sessions::inspector_shown(&rt.state), "the inspector shows it");
    press(&mut rt, RuneKey::Up);
    assert_eq!(selected(&rt), "id1");
    press(&mut rt, RuneKey::End);
    assert_eq!(selected(&rt), "id4");
    press(&mut rt, RuneKey::Home);
    assert_eq!(selected(&rt), "id0");
    press(&mut rt, RuneKey::PageDown);
    assert_eq!(selected(&rt), "id4");
    press(&mut rt, RuneKey::PageUp);
    assert_eq!(selected(&rt), "id0");
    // Tab to the inspector and back.
    press(&mut rt, RuneKey::Tab);
    assert!(rt.state.sessions.keys_inspector);
    press(&mut rt, RuneKey::Down);
    assert_eq!(selected(&rt), "id0", "↓ scrolls the inspector");
    press(&mut rt, RuneKey::Tab);
    assert!(!rt.state.sessions.keys_inspector);
    // esc: the inspector, then the filter; never the window.
    rt.update(|s, _| s.browser.set_filter("i"));
    settle(&mut rt);
    press(&mut rt, RuneKey::Escape);
    assert!(!sessions::inspector_shown(&rt.state) && rt.state.browser.filter() == "i", "the first esc closes the inspector");
    press(&mut rt, RuneKey::Escape);
    assert!(rt.state.browser.filter().is_empty(), "the second clears the filter");
    press(&mut rt, RuneKey::Escape);
    assert_eq!(rt.shell_request, None, "esc never quits");
    // `/` types a filter, which matches titles.
    typed(&mut rt, "/skip");
    assert!(rt.state.browser.editing());
    assert_eq!(order(&rt), ["id1"]);
    press(&mut rt, RuneKey::Escape);
    assert!(!rt.state.browser.editing() && rt.state.browser.filter().is_empty());
    // s, r and t.
    press(&mut rt, RuneKey::Text("s".into()));
    assert_eq!(rt.state.browser.sort_order(), Sort::Cost, "s cycles the sort");
    assert_eq!(text_in(&rt, "head-cost", "head-text"), "Cost ↓", "and the header marks it");
    press(&mut rt, RuneKey::Text("r".into()));
    assert_eq!(rt.state.browser.sort_order(), Sort::Cost);
    press(&mut rt, RuneKey::Text("t".into()));
    assert_eq!((rt.state.browser.sort_order(), rt.state.browser.sort_reversed()), (Sort::Time, false), "t: time order, newest first");
    assert!(rt.state.sessions.offset.get() < 1.0, "at its top");
    assert!(rt.presented(&format!("sday-{}", today())).is_some_and(|(r, _)| r[1] < 200.0), "under Today");
    rt.update(|s, _| s.browser.sort_by(Sort::Cost));
    click(&mut rt, "today");
    settle(&mut rt);
    assert_eq!(rt.state.browser.sort_order(), Sort::Time, "Today does the same");
    // c, f and o act on the selected session; the made-up session's profile fails to be written,
    // in contained work that says nothing on stderr.
    tokprof_browse::contain::install_hook();
    press(&mut rt, RuneKey::Text("c".into()));
    assert!(rt.state.browser.status().contains("handoff"), "{}", rt.state.browser.status());
    let wrote = |rt: &Runtime<Desk>, ext: &str| {
        let status = rt.state.browser.status();
        (status.starts_with("writing") && status.contains(&format!("id1.{ext}"))) || status.starts_with("export failed")
    };
    press(&mut rt, RuneKey::Text("f".into()));
    assert!(wrote(&rt, "jfr"), "{}", rt.state.browser.status());
    std::thread::sleep(Duration::from_millis(200));
    press(&mut rt, RuneKey::Text("o".into()));
    assert!(wrote(&rt, "html"), "{}", rt.state.browser.status());
    // ⏎ opens the session in Day.
    press(&mut rt, RuneKey::Down);
    press(&mut rt, RuneKey::Enter);
    assert_eq!(rt.state.reports.mode, reports::Mode::Day);
    assert_eq!(selected(&rt), "id2");
    // A double click does too.
    let mut rt = in_sessions(table_desk(true), (1400.0, 900.0));
    let (x, y) = rt.find("id2").unwrap();
    rt.frame(0.6);
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    rt.frame(0.05);
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    settle(&mut rt);
    assert_eq!(rt.state.reports.mode, reports::Mode::Day, "a double click opens it in Day");
    assert_eq!(selected(&rt), "id2");
    // q quits.
    let mut rt = in_sessions(table_desk(true), (1400.0, 900.0));
    press(&mut rt, RuneKey::Text("q".into()));
    assert_eq!(rt.shell_request, Some(Shell::Quit));
}

#[test]
fn the_scenes_draw_sessions_projects_cost_and_waiting() {
    let scene = |name: &str| {
        let mut desk = table_desk(true);
        (SCENES.iter().find(|(n, _)| *n == name).unwrap_or_else(|| panic!("the {name} scene")).1)(&mut desk);
        runtime(desk, (1400.0, 900.0))
    };
    let rt = scene("sessions");
    assert_eq!(rt.state.reports.mode, reports::Mode::Sessions);
    assert_eq!(rt.state.browser.selected(), Some(0), "the newest session selected");
    assert!(rt.presented("sessions-inspector").is_some(), "with its inspector");
    let all = texts(&rt);
    for t in ["All", "Running now", "Warm", "This week", "claude", "codex", "Project: any", "Today", "2 sessions · $30.23 spent", "Total"] {
        assert!(all.iter().any(|x| x == t), "{t:?} in {all:?}");
    }
    assert_eq!(text_of(&rt, "foot-title"), "5 sessions · all time · costing 1…");
    assert_eq!((text_of(&rt, "foot-cost"), text_of(&rt, "foot-avoidable"), text_of(&rt, "foot-cold")), ("$371".into(), "$17.79".into(), "3".into()));

    let rt = scene("sessions-projects");
    assert!(rt.state.sessions.projects && rt.state.sessions.open.is_empty(), "every project folded");
    assert!(rt.presented("sessions-inspector").is_none());
    let rows: Vec<String> = rt.elements().into_iter().filter_map(|e| e.key).filter(|k| k.starts_with("proj-")).collect();
    assert_eq!(rows, ["proj-~/src/app", "proj-~/src/tool"], "one row per project, the costliest first");

    let rt = scene("sessions-cost");
    assert_eq!(rt.state.browser.sort_order(), Sort::Cost);
    assert!(rt.presented("sessions-inspector").is_none(), "no inspector");
    assert!(rt.elements().iter().all(|e| e.key.as_deref() != Some("day-total")));
    assert_eq!(order(&rt), ["id2", "id0", "id3", "id1", "id4"]);

    let rt = scene("waiting");
    assert_eq!((rt.state.reports.mode, rt.state.browser.sort_order()), (reports::Mode::Sessions, Sort::Waiting), "waiting is drawn in Sessions");
}

/// The local date the lanes show.
fn lanes_date(rt: &Runtime<Desk>) -> i64 {
    rhythm::date_of(rt.state.browser.day_noon(), &rhythm::local_offset)
}

fn selected_id(rt: &Runtime<Desk>) -> Option<String> {
    rt.state.current().map(|r| rt.state.browser.session(r).to_string())
}

#[test]
fn a_session_picked_in_the_palette_opens_on_its_day_or_is_selected_in_sessions() {
    // In Day: the session's day, with that session selected and its Summary from the top.
    let mut rt = runtime(table_desk(true), (1400.0, 900.0));
    rt.update(|s, tx| s.show_tab(Tab::Tools, tx));
    run_from_palette(&mut rt, "profiler order", palette::Pick::Session("id2".into()));
    settle(&mut rt);
    assert_eq!(rt.state.reports.mode, reports::Mode::Day);
    assert_eq!(selected_id(&rt).as_deref(), Some("id2"));
    assert_eq!(lanes_date(&rt), today() - 1, "yesterday, the day of its last activity");
    assert_eq!(rt.state.tab, Tab::Summary, "from its Summary");
    assert!(rt.presented("back-to-day").is_some(), "the details pane shows its inspector");

    // From Reports, a session opens in Day too.
    command(&mut rt, '3');
    settle(&mut rt);
    run_from_palette(&mut rt, "skip old logs", palette::Pick::Session("id1".into()));
    settle(&mut rt);
    assert_eq!((rt.state.reports.mode, selected_id(&rt).as_deref()), (reports::Mode::Day, Some("id1")));
    let r1 = rt.state.browser.row_of("id1").unwrap();
    assert_eq!(lanes_date(&rt), early_day(rt.state.browser.active_ms(r1)), "today, or yesterday's night just after midnight");

    // In Sessions it is selected there, scrolled to, with its Summary in the inspector.
    command(&mut rt, '0');
    settle(&mut rt);
    run_from_palette(&mut rt, "python review", palette::Pick::Session("id3".into()));
    settle(&mut rt);
    assert_eq!(rt.state.reports.mode, reports::Mode::Sessions, "Sessions stays");
    assert_eq!(selected_id(&rt).as_deref(), Some("id3"));
    assert!(sessions::inspector_shown(&rt.state) && rt.presented("id3").is_some(), "its row and its inspector");

    // The palette names the days as the table's headers do.
    for days_ago in [0, 1, 2] {
        let label = text_in(&rt, &format!("sday-{}", today() - days_ago), "day-label");
        let want = if days_ago == 0 { palette::Pick::Day(None) } else { palette::Pick::Day(Some(today() - days_ago)) };
        let entry = palette::entries(&rt.state, "").into_iter().find(|e| e.pick == want).expect("the day is in the palette");
        assert_eq!(entry.title, label, "{days_ago} days ago");
    }

    // In Projects, its project unfolds so that its row shows.
    rt.update(|s, tx| sessions::set_projects(s, true, tx));
    settle(&mut rt);
    assert!(rt.state.sessions.open.is_empty() && rt.presented("id0").is_none(), "folded");
    run_from_palette(&mut rt, "python chat", palette::Pick::Session("id0".into()));
    settle(&mut rt);
    assert!(rt.state.sessions.open.contains("~/src/app"), "{:?}", rt.state.sessions.open);
    assert_eq!(selected_id(&rt).as_deref(), Some("id0"));
    assert!(rt.presented("id0").is_some() && sessions::inspector_shown(&rt.state));
}

#[test]
fn sort_by_in_the_palette_shows_sessions_sorted_as_a_header_click_sorts() {
    // The test's calendar is empty: Calendar is drawn without the store.
    for from in [reports::Mode::Day, reports::Mode::Month, reports::Mode::Calendar] {
        let mut rt = runtime(table_desk(from != reports::Mode::Calendar), (1400.0, 900.0));
        reports::show(&mut rt.state, from);
        settle(&mut rt);
        run_from_palette(&mut rt, "sort by cost", palette::Pick::Act(palette::Action::SortCost));
        settle(&mut rt);
        assert_eq!(rt.state.reports.mode, reports::Mode::Sessions, "from {from:?}");
        assert_eq!((rt.state.browser.sort_order(), rt.state.browser.sort_reversed()), (Sort::Cost, false));
        assert_eq!(text_in(&rt, "head-cost", "head-text"), "Cost ↓");
        let by_palette = order(&rt);
        let mut clicked = in_sessions(table_desk(true), (1400.0, 900.0));
        click(&mut clicked, "head-cost");
        settle(&mut clicked);
        assert_eq!(by_palette, order(&clicked), "the order a click on `Cost` gives");
    }
    // Sort again shows Sessions too.
    let mut rt = runtime(table_desk(true), (1400.0, 900.0));
    run_from_palette(&mut rt, "sort again", palette::Pick::Act(palette::Action::Resort));
    settle(&mut rt);
    assert_eq!(rt.state.reports.mode, reports::Mode::Sessions);
}

#[test]
fn back_to_the_lanes_and_the_full_size_inspector_are_offered_when_they_do_something() {
    let offered = |rt: &Runtime<Desk>, a| palette::entries(&rt.state, "").iter().any(|e| e.pick == palette::Pick::Act(a));
    let (full, back) = (palette::Action::FullSize, palette::Action::BackToLanes);
    let mut rt = runtime(table_desk(true), (1400.0, 900.0));
    assert!(!offered(&rt, full) && !offered(&rt, back), "no session selected");
    rt.update(|s, tx| {
        s.pick("id0", tx);
    });
    settle(&mut rt);
    assert!(offered(&rt, full) && !offered(&rt, back));
    run_from_palette(&mut rt, "full-size", palette::Pick::Act(full));
    settle(&mut rt);
    assert!(rt.state.browser.zoomed());
    assert!(!offered(&rt, full) && offered(&rt, back));
    let entry = palette::entries(&rt.state, "").into_iter().find(|e| e.pick == palette::Pick::Act(back)).unwrap();
    assert_eq!((entry.title.as_str(), entry.keys), ("Back to the lanes", &["esc"][..]));
    run_from_palette(&mut rt, "back to the lanes", palette::Pick::Act(back));
    settle(&mut rt);
    assert!(!rt.state.browser.zoomed() && rt.presented("lanes").is_some(), "the lanes are back");
    assert_eq!(selected_id(&rt).as_deref(), Some("id0"), "with the session selected");
}

#[test]
fn a_pick_in_the_menu_bar_opens_the_session_in_day_on_its_day() {
    for from in [reports::Mode::Month, reports::Mode::Sessions, reports::Mode::Calendar, reports::Mode::Day] {
        let mut rt = runtime(table_desk(from != reports::Mode::Calendar), (1400.0, 900.0));
        rt.update(|s, tx| {
            if from == reports::Mode::Sessions {
                sessions::show(s, tx);
            } else {
                reports::show(s, from);
            }
            s.pick("id1", tx);
            s.show_tab(Tab::Cache, tx);
            s.palette = Some(palette::Palette::default());
        });
        settle(&mut rt);
        rt.update(|s, tx| menubar::pick(s, menubar::Pick::Session("id2".into()), tx));
        settle(&mut rt);
        assert_eq!(rt.state.reports.mode, reports::Mode::Day, "from {from:?}");
        assert_eq!(selected_id(&rt).as_deref(), Some("id2"));
        assert_eq!(lanes_date(&rt), today() - 1, "on yesterday, its day");
        assert_eq!(rt.state.tab, Tab::Summary, "its Summary from the top");
        assert!(rt.state.palette.is_none() && !rt.state.browser.zoomed());
    }
}

/// Two sessions around a night: id0 in ~/src/app from 22:00 two days ago to 02:00 yesterday,
/// then nobody acts until 09:00, when id1 starts in the temporary folders. The night's middle,
/// 05:30, starts yesterday.
fn night_desk() -> Desk {
    let rows = vec![srow(1, "claude", "temporary folders", day_at(1, 10)), srow(0, "claude", "~/src/app", day_at(1, 2))];
    let mut desk = Desk::new(Browser::detached(Arc::new(Catalog::new(Vec::new(), PathBuf::new())), rows, Pricing::defaults(), frontend()), false);
    let g = |cost: f64, started: i64, title: &'static str| {
        glance(G { cost, requests: 10, started, avoidable: 0.0, cold: 0, models: vec![("claude-opus-5-5", 10)], title, warm_until: None, working: None })
    };
    desk.browser.put_glance("id0", g(50.0, day_at(2, 22), "Late fix"));
    desk.browser.put_glance("id1", g(5.0, day_at(1, 9), "Morning check"));
    let acted = |e: Entry, at: &[i64]| Entry { actions: at.iter().map(|&t| (t, true)).collect(), ..e };
    let entries = vec![
        acted(
            spent("claude", "id0", &[(day_at(2, 23), 40.0), (day_at(1, 1) + 30 * MIN, 10.0)]),
            &[day_at(2, 22), day_at(2, 23) + 30 * MIN, day_at(1, 1), day_at(1, 2)],
        ),
        acted(spent("claude", "id1", &[(day_at(1, 9) + 30 * MIN, 5.0)]), &[day_at(1, 9), day_at(1, 10)]),
    ];
    desk.calendar.data.put_with(tokprof_core::calendar::Calendar::default(), Store { entries, ..Store::default() }, 0);
    desk
}

#[test]
fn a_session_last_active_at_two_lists_under_the_day_before_and_opens_there() {
    let mut rt = in_sessions(night_desk(), (1400.0, 900.0));
    let slots = rt.state.sessions.layout.borrow().slots.clone();
    let (before, yesterday) = (today() - 2, today() - 1);
    assert_eq!(slots, [Slot::Day(yesterday), Slot::Row(0), Slot::Day(before), Slot::Row(1)], "02:00 belongs to the day before");
    assert_eq!(header(&rt, 2), "1 session · $50.00 spent", "the day before's spend, its 01:30 included");
    assert_eq!(header(&rt, 1), "1 session · $5.00 spent");
    assert_eq!(
        (text_in(&rt, "id0", "col-last"), text_in(&rt, "id0", "col-started")),
        ("02:00".into(), "22:00".into()),
        "it started on the day it is listed under"
    );
    // The palette's days count it there too.
    let entry = palette::entries(&rt.state, "").into_iter().find(|e| e.pick == palette::Pick::Day(Some(before))).expect("the day before is in the palette");
    assert!(entry.right.starts_with("1 session,"), "{}", entry.right);
    // Open in Day shows the day of its header.
    rt.update(|s, tx| sessions::open_in_day(s, "id0", tx));
    settle(&mut rt);
    assert_eq!((rt.state.reports.mode, selected_id(&rt).as_deref()), (reports::Mode::Day, Some("id0")));
    assert_eq!(lanes_date(&rt), before);
}

#[test]
fn the_project_cell_names_the_project_as_the_lanes_do() {
    let mut rt = in_sessions(night_desk(), (1400.0, 900.0));
    assert_eq!(text_in(&rt, "id0", "col-project"), "app", "the last folder, not the path");
    assert_eq!(text_in(&rt, "id1", "col-project"), "temporary folders");
    rt.update(|s, tx| sessions::set_projects(s, true, tx));
    settle(&mut rt);
    assert_eq!(text_in(&rt, "proj-~/src/app", "col-project"), "app", "and so does a project's row");
    assert_eq!(text_in(&rt, "proj-temporary folders", "col-project"), "temporary folders");
}
