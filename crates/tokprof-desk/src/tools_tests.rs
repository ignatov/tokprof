//! The `Tools` tab in the window: the cards, the tool table and the call log against the cells of
//! `tokprof scan --calls`, the filter by card, paging, the step to `Requests`, the keys and the
//! scene, on the Claude Code fixture and on X-rays made up from it.

use std::time::Instant;

use tokprof_browse::render_calls::{CallFilter, call_cells, calls_line, card_cells};
use tokprof_core::calllog::{CallRow, CallStatus};
use tokprof_core::model::{Class, ToolClass};
use tokprof_core::tools::{Group, ToolRow, ToolTable};

use super::*;
use crate::tools_tab::{self, CALLS_NOTE, Layout, card_key};

/// A row of `calls` calls, `failed` of them failed, whose counted calls took `durations` ms.
fn tool_row(class: ToolClass, mcp: Option<&str>, calls: u64, failed: u64, durations: Vec<u32>, tokens: f64) -> ToolRow {
    ToolRow {
        class: Class::Tool(class),
        calls,
        failed,
        time_ms: durations.iter().map(|&d| d as u64).sum(),
        durations,
        result_tokens: tokens,
        mcp: mcp.map(str::to_string),
        ..ToolRow::default()
    }
}

/// `n` durations of `ms`, then one that brings their sum to `sum`.
fn spread(n: usize, ms: u32, sum: u32) -> Vec<u32> {
    let mut v = vec![ms; n];
    v.push(sum - ms * n as u32);
    v
}

/// The groups of desk-window's scenarios: `shell` with 340 of 472 calls, 12 failed, 42 of 43
/// minutes; `files`; `mcp idea`; `subagents` that ran in the background; and `linear`, never
/// called.
fn groups() -> ToolTable {
    let mut t = ToolTable::default();
    t.rows.insert(("Bash".into(), None), tool_row(ToolClass::Execute, None, 340, 12, spread(299, 600, 2_520_000), 1_900_000.0));
    t.rows.insert(("Read".into(), None), tool_row(ToolClass::Read, None, 116, 0, spread(115, 100, 24_000), 80_000.0));
    t.rows.insert(("mcp__idea__find".into(), None), tool_row(ToolClass::Other, Some("idea"), 12, 1, spread(11, 1_200, 18_000), 9_000.0));
    t.rows.insert(("Agent".into(), None), tool_row(ToolClass::Subagent, None, 4, 0, Vec::new(), 2_000.0));
    t.servers.insert("linear".into());
    t
}

/// Call `i` of the main conversation: `shell` while `i < shell`, then `files`; made by request
/// `i % requests + 1`.
fn calls(n: usize, shell: usize, requests: usize) -> Vec<CallRow> {
    (0..n)
        .map(|i| {
            let (tool, group) = if i < shell { ("Bash", Group::Shell) } else { ("Read", Group::Files) };
            CallRow {
                start_ms: Some(1_794_000_000_000 + i as i64 * 1_000),
                request: Some((i % requests) as u32 + 1),
                tool: tool.into(),
                group,
                input: Some(format!("input {i}")),
                status: if i == 16 { CallStatus::Failed } else { CallStatus::Done },
                duration_ms: Some(400),
                timed: true,
                result_tokens: Some(1_000.0),
            }
        })
        .collect()
}

/// The window on the fixture's session, with the inspector's keyboard on `Tools`.
fn fixture_tools(size: (f32, f32)) -> Runtime<Desk> {
    let (catalog, select) = fixture();
    let mut browser = Browser::start(catalog, Pricing::defaults(), select, false, Arc::new(|| {}), frontend()).unwrap();
    browser.settle();
    let mut rt = runtime(with_selection(Desk::new(browser, false)), size);
    press(&mut rt, RuneKey::Tab);
    press(&mut rt, RuneKey::Text("7".into()));
    assert_eq!(rt.state.tab, Tab::Tools);
    rt
}

/// The same with the fixture's X-ray given these tools and calls.
fn made_up(tools: ToolTable, log: Vec<CallRow>, size: (f32, f32)) -> Runtime<Desk> {
    let mut rt = fixture_tools(size);
    let mut x = rt.state.browser.xray().cloned_x();
    x.tools = tools;
    x.call_log = log;
    let id = x.id.clone();
    rt.update(move |s, _| s.browser.put_xray(&id, x));
    settle(&mut rt);
    rt
}

fn xray(rt: &Runtime<Desk>) -> tokprof_core::xray::XRay {
    rt.state.browser.xray().cloned_x()
}

fn element(rt: &Runtime<Desk>, outer: &str, key: &str) -> rune::ElementInfo {
    inside(rt, outer, key).unwrap_or_else(|| panic!("no {key} in {outer}"))
}

fn pal_of(rt: &Runtime<Desk>, dark: bool) -> pal::Pal {
    pal::pal(rt.state.settings.theme(dark))
}

#[test]
fn tools_holds_the_numbers_of_tokprof_scan_calls() {
    let rt = fixture_tools((1400.0, 1800.0));
    let x = xray(&rt);
    let cards = card_cells(&x.tools);
    assert!(cards.len() >= 4 && cards.last().unwrap().never_called, "the fixture's groups, `mcp linear` never called: {cards:?}");
    for c in &cards {
        let key = card_key(&c.name);
        assert_eq!(text_in(&rt, &key, "card-name"), c.name);
        if c.never_called {
            assert_eq!(text_in(&rt, &key, "card-never"), "never called");
            assert!(inside(&rt, &key, "card-calls-bar").is_none(), "no bars on {key}");
        } else {
            assert_eq!(text_in(&rt, &key, "card-calls"), c.calls);
            assert_eq!(text_in(&rt, &key, "card-time"), c.time);
            assert_eq!(text_in(&rt, &key, "card-tokens"), c.tokens);
        }
    }
    let shown = texts(&rt);
    assert!(shown.iter().any(|t| t == CALLS_NOTE) && shown.contains(&calls_line(&x)), "the lines over the call log");
    for row in call_cells(&x.call_log, &CallFilter::default()).iter().skip(1) {
        for cell in row.iter().filter(|c| !c.is_empty()) {
            assert!(shown.contains(cell), "{cell} of {row:?} on the screen");
        }
    }
}

#[test]
fn a_status_has_its_color() {
    let mut rt = fixture_tools((1400.0, 1800.0));
    for dark in [false, true] {
        rt.update(move |s, _| s.pick_appearance(if dark { settings::Appearance::Dark } else { settings::Appearance::Light }));
        settle(&mut rt);
        let p = pal_of(&rt, dark);
        let colors = |t: &str| rt.elements().into_iter().filter(|e| e.text.as_deref() == Some(t)).filter_map(|e| e.text_color).collect::<Vec<_>>();
        for (status, color) in
            [("failed", p.cold), ("running", p.warm), ("asked you", p.question), ("refused", p.ink2), ("denied", p.ink2), ("background", p.ink2)]
        {
            assert!(colors(status).contains(&color), "{status} in its color, dark {dark}");
        }
    }
}

#[test]
fn a_card_shows_its_group_s_numbers_and_shares() {
    let mut rt = made_up(groups(), calls(40, 30, 5), (1400.0, 1400.0));
    for dark in [false, true] {
        rt.update(move |s, _| s.pick_appearance(if dark { settings::Appearance::Dark } else { settings::Appearance::Light }));
        settle(&mut rt);
        let p = pal_of(&rt, dark);
        let shell = card_key("shell");
        assert_eq!(
            ["card-name", "card-calls", "card-failed", "card-time", "card-tokens"].map(|k| text_in(&rt, &shell, k)),
            ["shell", "340 calls", "12 failed", "42m · med 0.6s", "1.9M tok"]
        );
        assert_eq!(element(&rt, &shell, "card-failed").text_color, Some(p.cold), "failed in red");
        let share = |bar: &str| {
            let track = element(&rt, &shell, bar);
            let fill =
                rt.elements().into_iter().find(|e| e.key.as_deref() == Some("fill") && e.rect[1] == track.rect[1] && e.rect[0] == track.rect[0]).unwrap();
            (fill.rect[2] / track.rect[2], fill.fill)
        };
        let (calls, blue) = share("card-calls-bar");
        let (time, green) = share("card-time-bar");
        assert!((calls - 0.72).abs() < 0.01 && (time - 0.98).abs() < 0.01, "72% of the calls and 98% of the time: {calls} {time}");
        assert_eq!((blue, green), (p.bars, p.work), "the requests' blue and the work's green, 60% of the way on a dark theme");
        // No time to count, and a server never called.
        let agents = card_key("subagents");
        assert_eq!(text_in(&rt, &agents, "card-time"), "– · med –");
        assert!(inside(&rt, &agents, "card-time-bar").is_none() && inside(&rt, &agents, "card-calls-bar").is_some());
        let linear = card_key("mcp linear");
        assert_eq!(element(&rt, &linear, "card-name").text_color, Some(p.ink2));
        assert_eq!(element(&rt, &linear, "card-never").text_color, Some(p.ink3));
        assert!(inside(&rt, &linear, "card-calls-bar").is_none(), "no bars");
        let at = |k: &str| rt.presented(k).unwrap().0;
        assert!(at(&linear)[0] > at(&shell)[0], "the never-called card comes last");
    }
}

#[test]
fn the_cards_wrap_to_the_inspector_s_width() {
    // A 520 pt inspector has 484 pt inside its padding: 3 cards of 156 pt with 8 pt between them.
    assert_eq!(tools_tab::per_row(520.0 - 36.0), 3);
    assert_eq!(tools_tab::per_row(483.0), 2);
    let mut t = groups();
    t.rows.insert(("WebSearch".into(), None), tool_row(ToolClass::Fetch, None, 5, 0, vec![12_000], 0.0));
    t.rows.insert(("Monitor".into(), None), tool_row(ToolClass::Other, None, 3, 0, vec![100], 0.0));
    let rt = made_up(t, calls(10, 5, 5), (1000.0, 900.0));
    let pane = rt.presented(view::PANE).unwrap().0[2];
    let per = tools_tab::per_row(pane - 36.0);
    let first = |row: &str| rt.elements().into_iter().filter(|e| e.parent.is_some_and(|p| rt.elements()[p].key.as_deref() == Some(row))).count();
    assert_eq!(first("cards-0"), per, "a full first row in {pane} pt");
    assert_eq!(first("cards-1"), 7 - per, "the rest on the next");
    let (a, b) = (rt.presented(&card_key("shell")).unwrap().0, rt.presented(&card_key("files")).unwrap().0);
    assert_eq!((a[2], b[0] - a[0] - a[2]), (tools_tab::CARD_W, tools_tab::CARD_GAP));
}

#[test]
fn a_click_on_a_card_filters_both_tables_and_a_second_clears_it() {
    let mut rt = made_up(groups(), calls(412, 280, 5), (1400.0, 900.0));
    click(&mut rt, &card_key("shell"));
    settle(&mut rt);
    assert_eq!(rt.state.tools.group, Some(Group::Shell));
    assert!(ringed(&rt).contains(&card_key("shell")), "ringed in the accent: {:?}", ringed(&rt));
    assert_eq!(text_in(&rt, "calls", "calls-showing"), "showing 280 of 412 calls");
    let l = Layout::of(&rt.state.tools, &xray(&rt));
    assert_eq!((l.tool_rows(), l.calls.len()), (1, 280), "the Bash row and the shell calls");
    assert!(!texts(&rt).iter().any(|t| t == "Read"), "no files row in either table");
    // Another card, then the chosen one again.
    click(&mut rt, &card_key("files"));
    settle(&mut rt);
    assert_eq!(rt.state.tools.group, Some(Group::Files));
    click(&mut rt, &card_key("files"));
    settle(&mut rt);
    assert_eq!(rt.state.tools.group, None);
    assert!(inside(&rt, "calls", "calls-showing").is_none());
    // A never-called card cannot be chosen.
    click(&mut rt, &card_key("mcp linear"));
    settle(&mut rt);
    assert_eq!(rt.state.tools.group, None);
    assert_eq!(Layout::of(&rt.state.tools, &xray(&rt)).calls.len(), 412);
}

#[test]
fn a_new_session_opens_with_no_card_and_the_cursor_on_the_first_row() {
    let mut rt = made_up(groups(), calls(40, 30, 5), (1400.0, 900.0));
    rt.update(|s, _| s.tools = tools_tab::State { session: "another".into(), group: Some(Group::Shell), cursor: 7 });
    settle(&mut rt);
    let l = Layout::of(&rt.state.tools, &xray(&rt));
    assert_eq!((l.group, l.cursor), (None, 0), "a state of another session is ignored");
    assert_eq!(ringed(&rt), ["row-0"]);
    press(&mut rt, RuneKey::Down);
    assert_eq!((rt.state.tools.session.clone(), rt.state.tools.cursor), (xray(&rt).id, 1), "the state is now this session's");
}

#[test]
fn the_cursor_runs_through_both_tables_and_the_call_log_pages() {
    let mut rt = made_up(groups(), calls(1_250, 600, 5), (1400.0, 900.0));
    let t = Layout::of(&rt.state.tools, &xray(&rt)).tool_rows();
    assert_eq!(t, 4);
    // From the last row of the tool table to the first call, on the screen.
    for _ in 0..t {
        press(&mut rt, RuneKey::Down);
    }
    assert_eq!(rt.state.tools.cursor, t);
    assert!(rt.presented(&format!("row-{t}")).is_some(), "the first call is on the screen");
    // The cursor on the 10th call: calls 1 to 300 and the note of the others.
    rt.update(move |s, _| s.tools.cursor = t + 9);
    settle(&mut rt);
    assert_eq!(text_in(&rt, "calls", "calls-later"), "and 950 later calls");
    assert!(inside(&rt, "calls", "calls-earlier").is_none());
    press(&mut rt, RuneKey::End);
    assert_eq!(rt.state.tools.cursor, t + 1_249);
    assert_eq!(text_in(&rt, "calls", "calls-earlier"), "and 950 earlier calls");
    let l = Layout::of(&rt.state.tools, &xray(&rt));
    assert_eq!((l.page(), l.drawn(l.cursor)), ((950, 1_250), t + 299));
    assert_eq!(l.joined(t + 299), Some(t + 1_249));
    assert_eq!(l.joined(t + 300), None, "past the page");
    assert!(rt.presented(&format!("row-{}", t + 299)).is_some(), "the last call is on the screen");
}

#[test]
fn enter_on_a_call_shows_its_request_and_tools_comes_back_as_it_was() {
    let mut rt = made_up(groups(), calls(40, 30, 5), (1400.0, 900.0));
    assert!(xray(&rt).points.len() >= 5, "the fixture has 5 requests");
    // ⏎ on a row of the tool table does nothing.
    press(&mut rt, RuneKey::Enter);
    assert_eq!((rt.state.tab, rt.state.browser.zoomed()), (Tab::Tools, false));
    press(&mut rt, RuneKey::Right);
    assert_eq!(rt.state.tools.group, Some(Group::Shell), "→ chooses the first card");
    // Call 17 of the shell calls, made by request 2 (16 % 5 + 1).
    let t = Layout::of(&rt.state.tools, &xray(&rt)).tool_rows();
    for _ in 0..t + 16 {
        press(&mut rt, RuneKey::Down);
    }
    let l = Layout::of(&rt.state.tools, &xray(&rt));
    assert_eq!(l.call_at(l.cursor), Some(16));
    press(&mut rt, RuneKey::Enter);
    assert_eq!(rt.state.tab, Tab::Requests);
    assert_eq!(rt.state.requests.sel, Some(1), "request 2 selected");
    let title = rt.elements().into_iter().find(|e| e.key.as_deref() == Some("card-title")).and_then(|e| e.text).unwrap_or_default();
    assert!(title.starts_with("Request 2 of "), "{title}");
    press(&mut rt, RuneKey::Text("7".into()));
    assert_eq!((rt.state.tab, rt.state.tools.group.clone(), rt.state.tools.cursor), (Tab::Tools, Some(Group::Shell), t + 16), "as it was");
    assert_eq!(ringed(&rt), [card_key("shell"), format!("row-{}", t + 16)]);
}

#[test]
fn a_double_click_on_a_call_shows_its_request() {
    let mut rt = made_up(groups(), calls(40, 30, 5), (1400.0, 1400.0));
    let t = Layout::of(&rt.state.tools, &xray(&rt)).tool_rows();
    // The 3rd call, made by request 3.
    let (x, y) = rt.find(&format!("row-{}", t + 2)).unwrap();
    rt.frame(0.6);
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    rt.frame(0.05);
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    settle(&mut rt);
    assert_eq!((rt.state.tab, rt.state.requests.sel), (Tab::Requests, Some(2)));
    assert_eq!(rt.state.tools.cursor, t + 2, "the click put the cursor on it");
}

#[test]
fn the_arrows_choose_cards_and_esc_clears_the_card_first() {
    let mut rt = made_up(groups(), calls(40, 30, 5), (1400.0, 900.0));
    let order: Vec<Group> = card_cells(&xray(&rt).tools).into_iter().filter(|c| !c.never_called).map(|c| c.group).collect();
    press(&mut rt, RuneKey::Left);
    assert_eq!(rt.state.tools.group.as_ref(), order.last(), "← from none chooses the last card that can be chosen");
    press(&mut rt, RuneKey::Escape);
    press(&mut rt, RuneKey::Text("l".into()));
    assert_eq!(rt.state.tools.group.as_ref(), order.first(), "l (→) from none chooses the first");
    press(&mut rt, RuneKey::Right);
    assert_eq!(rt.state.tools.group.as_ref(), order.get(1), "→ the next");
    assert_eq!(rt.state.tab, Tab::Tools);
    press(&mut rt, RuneKey::Escape);
    assert_eq!((rt.state.tools.group.clone(), rt.state.pane), (None, Focus::Inspector), "esc clears the card and keeps the keyboard");
    press(&mut rt, RuneKey::Escape);
    assert_eq!(rt.state.pane, Focus::Lanes, "a second esc gives it back");
    // The key help names them.
    let inspector: Vec<&str> = keys::GROUPS.iter().find(|g| g.0 == "Inspector").unwrap().1.iter().map(|r| r.what).collect();
    for what in ["in Tools, the group card before, after", "in Tools, the request of the call", "in Tools, clear the chosen card"] {
        assert!(inspector.contains(&what), "{what}");
    }
}

#[test]
fn a_session_without_tool_calls_says_so() {
    let rt = made_up(ToolTable::default(), Vec::new(), (1400.0, 900.0));
    assert!(texts(&rt).iter().any(|t| t == "No tool calls."));
    assert!(rt.presented("groups").is_none() && rt.presented("calls").is_none());
}

#[test]
fn the_tools_group_scene_chooses_the_first_card_and_the_first_call() {
    let (catalog, select) = fixture();
    let mut browser = Browser::start(catalog, Pricing::defaults(), select, false, Arc::new(|| {}), frontend()).unwrap();
    browser.settle();
    let mut desk = Desk::new(browser, false);
    let scene = SCENES.iter().find(|s| s.0 == "tools-group").expect("the scene");
    (scene.1)(&mut desk);
    let rt = runtime(desk, (1400.0, 1400.0));
    let x = xray(&rt);
    let first = card_cells(&x.tools).into_iter().next().unwrap();
    assert_eq!((rt.state.tab, rt.state.pane, rt.state.tools.group.clone()), (Tab::Tools, Focus::Inspector, Some(first.group.clone())));
    let l = Layout::of(&rt.state.tools, &x);
    assert_eq!(l.cursor, l.tool_rows(), "the cursor on the first call");
    assert!(l.calls.iter().all(|&i| x.call_log[i].group == first.group), "the call log filtered");
    assert_eq!(ringed(&rt), [card_key(&first.name), format!("row-{}", l.tool_rows())]);
    assert!(text_in(&rt, "calls", "calls-showing").starts_with(&format!("showing {} of ", l.calls.len())));
}

#[test]
fn tools_draws_three_thousand_calls_in_time() {
    let mut rt = fixture_tools((1400.0, 900.0));
    let mut x = xray(&rt);
    x.tools = groups();
    x.call_log = calls(3_000, 2_000, 5);
    let id = x.id.clone();
    rt.update(|s, tx| s.show_tab(Tab::Summary, tx));
    settle(&mut rt);
    let started = Instant::now();
    rt.update(move |s, tx| {
        s.browser.put_xray(&id, x);
        s.show_tab(Tab::Tools, tx);
    });
    rt.frame(1.0 / 60.0);
    let took = started.elapsed();
    eprintln!("Tools with 3,000 calls: the cells and the first frame took {took:?}");
    assert!(rt.presented("calls").is_some());
    // The goal is for the app, an optimized build (`cargo test --release`): about 26 ms on an M-series
    // Mac. A test build is about 15 times slower, and slower still beside the other tests, so it
    // only guards against a cost that grows faster than the calls.
    let limit = if cfg!(debug_assertions) { 5_000 } else { 100 };
    assert!(took.as_millis() < limit, "{took:?}");
}
