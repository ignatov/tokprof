//! The window driven headless: Rune's runtime with the same view, keys and clicks.

use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};

use rune::{ElementInfo, Env, Runtime, Shell};
use tokprof_browse::watch::Change;
use tokprof_core::pricing::Pricing;
use tokprof_core::source::{Locator, SessionRef};
use tokprof_core::xray::SessionRow;

use super::*;

/// Session `i` in `project`, last active `at` seconds after the epoch.
fn row(i: usize, project: &str, at: u64) -> SessionRow {
    SessionRow {
        file: SessionRef {
            source: "test",
            agent: AgentId::unknown("claude"),
            locator: Locator::File(format!("/x/{i}.jsonl").into()),
            session_id: format!("id{i}"),
            main_id: format!("id{i}"),
            subagent: false,
            size: 1 + at,
            modified_ns: at as i128 * 1_000_000_000,
            created_ms: None,
        },
        subagents: Vec::new(),
        project: project.to_string(),
        id: format!("id{i}"),
        session_id: format!("id{i}"),
        bytes: 1 + at,
        active_ms: at as i64 * 1000,
        client: None,
    }
}

/// `n` sessions that compute nothing, listed 0, 1, 2…; every third is in `~/src/air`.
fn desk_of(n: usize) -> Desk {
    let rows = (0..n).map(|i| row(i, if i % 3 == 1 { "~/src/air" } else { "~/src/app" }, 10_000 - i as u64)).collect();
    Desk::new(Browser::detached(Arc::new(Catalog::new(Vec::new(), PathBuf::new())), rows, Pricing::defaults(), frontend()), false)
}

fn runtime(desk: Desk, size: (f32, f32)) -> Runtime<Desk> {
    let mut rt = Runtime::new(desk, view::view, size, Env::new(true, 1.0));
    rt.poll = Some(poll);
    rt.on_key = Some(on_key);
    settle(&mut rt);
    rt
}

/// Runs frames until springs and scrolls have landed.
fn settle(rt: &mut Runtime<Desk>) {
    for _ in 0..90 {
        rt.frame(1.0 / 60.0);
    }
}

fn press(rt: &mut Runtime<Desk>, key: RuneKey) {
    rt.key(KeyEvent::from(key));
    settle(rt);
}

fn typed(rt: &mut Runtime<Desk>, text: &str) {
    for c in text.chars() {
        press(rt, RuneKey::Text(c.to_string()));
    }
}

fn command(rt: &mut Runtime<Desk>, c: char) {
    rt.key(KeyEvent { key: RuneKey::Command(c), mods: rune::Mods { cmd: true, ..rune::Mods::default() } });
    rt.frame(1.0 / 60.0);
}

/// The element keyed `key` inside the element keyed `outer`.
fn inside(rt: &Runtime<Desk>, outer: &str, key: &str) -> Option<ElementInfo> {
    let all = rt.elements();
    let row = all.iter().position(|e| e.key.as_deref() == Some(outer))?;
    let within = |mut at: Option<usize>| {
        while let Some(i) = at {
            if i == row {
                return true;
            }
            at = all[i].parent;
        }
        false
    };
    all.iter().find(|e| e.key.as_deref() == Some(key) && within(e.parent)).cloned()
}

fn text_in(rt: &Runtime<Desk>, outer: &str, key: &str) -> String {
    inside(rt, outer, key).and_then(|e| e.text).unwrap_or_default()
}

/// Every text on the screen.
fn texts(rt: &Runtime<Desk>) -> Vec<String> {
    rt.elements().into_iter().filter_map(|e| e.text).collect()
}

fn click(rt: &mut Runtime<Desk>, key: &str) {
    let (x, y) = rt.find(key).unwrap_or_else(|| panic!("no {key} on the screen"));
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    rt.frame(1.0 / 60.0);
}

#[test]
fn filters_select_as_tokprof_ui_does() {
    let cli = Cli::parse_from(["tokprof-desk", "--project", "~/src/app/", "--since", "2026-11-01", "--until", "2026-11-05", "--session", "e666"]);
    let q = query(&cli, Vec::new()).unwrap();
    assert_eq!(q.project.as_deref(), Some("~/src/app"), "a project as the projects are written");
    assert_eq!((q.since_ms, q.until_ms), (Some(parse_date("2026-11-01").unwrap()), Some(parse_date("2026-11-05").unwrap())));
    assert_eq!(q.session.as_deref(), Some("e666"));
    let q = query(&Cli::parse_from(["tokprof-desk", "--since", "7d"]), Vec::new()).unwrap();
    let week = now_ms() - q.since_ms.unwrap();
    assert!((7 * 86_400_000 - 1000..=7 * 86_400_000 + 1000).contains(&week), "7d is seven days back");
    assert!(query(&Cli::parse_from(["tokprof-desk", "--until", "11/05"]), Vec::new()).is_err());
}

fn empty_catalog(name: &str) -> (Catalog, PathBuf) {
    let dir = std::env::temp_dir().join(format!("tokprof-desk-{name}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    (Catalog::new(vec![Box::new(tokprof_source_claude_code::ClaudeCode::at(dir.clone(), Path::new("/h")))], PathBuf::from("/h")), dir)
}

#[test]
fn no_logs_fail_before_any_window_opens() {
    let (catalog, dir) = empty_catalog("empty");
    let err = desk(&Cli::parse_from(["tokprof-desk"]), catalog, true, Arc::new(|| {})).err().map(|e| e.to_string());
    assert_eq!(err.as_deref(), Some("no session logs found"));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_bad_size_or_tab_fails_before_any_window_opens() {
    let args = |a: &[&str]| a.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    assert!(check_size(&args(&["--snap", "a.png", "--size", "1400x900"])).is_ok());
    assert!(check_size(&args(&["--size=900x600"])).is_ok());
    for bad in [&["--size", "1400"][..], &["--size", "1400xabc"], &["--size=0x600"], &["--size"]] {
        let err = check_size(&args(bad)).unwrap_err().to_string();
        assert_eq!(err, "--size takes WIDTHxHEIGHT", "{bad:?}");
    }
    let (catalog, dir) = empty_catalog("tab");
    let err = desk(&Cli::parse_from(["tokprof-desk", "--tab", "nope"]), catalog, true, Arc::new(|| {})).err().map(|e| e.to_string()).unwrap_or_default();
    assert!(err.starts_with("--tab takes one of: summary, requests"), "{err}");
    let _ = std::fs::remove_dir_all(dir);
}

/// The catalog of the Claude Code source's test logs: one session with a subagent.
fn fixture() -> (Arc<Catalog>, Query) {
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../sources/claude-code/tests/fixtures/projects");
    let catalog = Arc::new(Catalog::new(vec![Box::new(tokprof_source_claude_code::ClaudeCode::at(fixtures, Path::new("/w")))], PathBuf::from("/w")));
    let select = Query { agents: catalog.agents(), project: None, session: None, since_ms: None, until_ms: None, client: None };
    (catalog, select)
}

#[test]
fn a_cost_from_the_worker_shows_after_one_poll() {
    let (catalog, select) = fixture();
    let woken = Arc::new(AtomicUsize::new(0));
    let counted = woken.clone();
    let wake: Wake = Arc::new(move || {
        counted.fetch_add(1, Ordering::SeqCst);
    });
    // The browser costs its sessions as soon as it starts; the costing wakes the window when it
    // sends. Until then nothing else runs: the X-ray is asked for by the first poll.
    let browser = Browser::start(catalog, Pricing::defaults(), select, false, wake, frontend()).unwrap();
    let session = browser.session(0).to_string();
    let deadline = Instant::now() + Duration::from_secs(20);
    while woken.load(Ordering::SeqCst) == 0 {
        assert!(Instant::now() < deadline, "the costing did not wake the window");
        std::thread::sleep(Duration::from_millis(10));
    }
    let mut rt = Runtime::new(Desk::new(browser, false), view::view, (1400.0, 900.0), Env::new(true, 1.0));
    rt.frame(0.0);
    assert_eq!(text_in(&rt, &session, "cost"), "…", "a frame that does not poll shows no cost");
    assert_eq!(text_in(&rt, &session, "next-text"), "…", "nor a next request");
    rt.poll = Some(poll);
    rt.frame(1.0 / 60.0);
    let shown = text_in(&rt, &session, "cost");
    assert_eq!(shown, rt.state.browser.cells(0, now_ms()).cost, "the next poll shows it");
    assert!(shown.starts_with('$'), "{shown}");
}

#[test]
fn the_inspector_shows_the_numbers_of_tokprof_scan() {
    let (catalog, select) = fixture();
    let mut browser = Browser::start(catalog.clone(), Pricing::defaults(), select.clone(), false, Arc::new(|| {}), frontend()).unwrap();
    browser.settle();
    let mut rt = runtime(Desk::new(browser, false), (1400.0, 900.0));
    let x = rt.state.browser.xray().cloned_x();
    let scan = tokprof_browse::render_xray::text(&x, &tokprof_core::fmt::Style::with_color(false), &paths::profile_dir(), catalog.home());
    // The total spend and every component's amount, as the spend block prints them.
    let total = text_in(&rt, "spend", "spend-total");
    assert!(scan.contains(&format!("spend     {total} (API-equivalent)")), "{total} in:\n{scan}");
    for (c, share, amount) in inspector::components(&x) {
        let line = scan.lines().find(|l| l.trim_start().starts_with(c.name())).unwrap_or_else(|| panic!("no {} line", c.name()));
        assert!(line.contains(&format!("{:5.1}%", 100.0 * share)) && line.trim_end().ends_with(&amount), "{line} against {share} {amount}");
        assert!(texts(&rt).contains(&amount), "{amount} on the screen");
    }
    assert_eq!(text_in(&rt, "header", "title"), x.title.clone().unwrap_or_else(|| "Untitled session".into()));

    // The tools tab holds the rows of the X-ray's tool table.
    click(&mut rt, "tab-Tools");
    settle(&mut rt);
    assert_eq!(rt.state.tab, inspector::Tab::Tools);
    let cells = tokprof_browse::render_tools::tool_cells(&x.tools, tokprof_core::tools::RowOrder::Rent, 8, false);
    let shown = texts(&rt);
    for row in cells.rows.iter().skip(1) {
        for cell in row.iter().filter(|c| !c.is_empty()) {
            assert!(shown.contains(cell), "{cell} of {row:?} on the screen");
        }
    }
    // The prompts tab, each typed message with its cost.
    click(&mut rt, "tab-Prompts");
    settle(&mut rt);
    let shown = texts(&rt);
    for p in &x.prompts {
        assert!(shown.contains(&tokprof_core::fmt::usd(p.spend.usd)) && shown.contains(&p.label), "prompt {} on the screen", p.number);
    }
}

/// The X-ray the inspector shows, for a test to compare.
trait ClonedX {
    fn cloned_x(self) -> tokprof_core::xray::XRay;
}

impl ClonedX for Option<&tokprof_core::xray::XRay> {
    fn cloned_x(self) -> tokprof_core::xray::XRay {
        self.expect("the fixture's X-ray").clone()
    }
}

thread_local! {
    /// The day the test view draws: a day computed from logs needs logs written on that day.
    static DAY: std::cell::RefCell<Option<tokprof_browse::render_rhythm::DayParts>> = const { std::cell::RefCell::new(None) };
}

fn lanes_only(s: &Desk, cx: &mut rune::Cx<Desk>) -> rune::El {
    let p = pal::pal(true);
    DAY.with(|d| rune::col().w_full().h_full().child(day::lanes_of(s, d.borrow().as_ref().unwrap(), cx, &p, 600.0)))
}

/// A day from 04:00 local with one session that worked from 14:00 to 14:30 and got a message at
/// 14:40, while the user sat from 13:55 to 14:45.
fn synthetic_day() -> tokprof_browse::render_rhythm::DayParts {
    use tokprof_core::rhythm::{Mark, StripLane, StripLanes};
    const HOUR: i64 = 3_600_000;
    let begin = parse_date("2026-11-03").unwrap() + 4 * HOUR;
    let at = |h: i64, m: i64| begin + (h - 4) * HOUR + m * 60_000;
    let lanes = StripLanes {
        begin,
        end: begin + 24 * HOUR,
        hours: (0..24).map(|k| (begin + k * HOUR, (k + 4) % 24)).collect(),
        you: vec![(at(13, 55), at(14, 45))],
        lanes: vec![StripLane {
            id: "id3".into(),
            label: "id3 Инфографика прерываний".into(),
            marks: vec![(at(14, 0), at(14, 30), Mark::Work), (at(14, 40), at(14, 40), Mark::Message)],
        }],
    };
    let strip = tokprof_core::rhythm::Strip { lines: vec!["(the strip in characters)".into()] };
    let below = "\nsittings  1, 50m at the keyboard\nanswers   the agent's turn took a median 30m\ncache     no cold returns\n";
    tokprof_browse::render_rhythm::DayParts { above: "tokprof · day\nday       …\n\n".into(), strip: Some((strip, lanes)), below: below.into() }
}

#[test]
fn the_lanes_draw_the_log_s_times_name_them_under_the_pointer_and_select_a_lane() {
    let parts = synthetic_day();
    let lanes = parts.strip.clone().unwrap().1;
    DAY.with(|d| *d.borrow_mut() = Some(parts));
    let mut rt = Runtime::new(desk_of(10), lanes_only, (1400.0, 900.0), Env::new(true, 1.0));
    settle(&mut rt);
    let lane = lanes.lanes[0].clone();
    let track = rt.presented(&format!("track-{}", lane.id)).unwrap().0;
    let scale = rt.presented(day::SCALE).unwrap().0;
    assert_eq!((track[0], track[2]), (scale[0], scale[2]), "every lane's track lines up with the hours row");
    assert!(track[0] >= day::LABEL_WIDTH, "after the labels");

    // Fit shows 13:00 to 15:00: the sitting starts at 13:55 and ends at 14:45, two hours at least.
    let range = day::range(&lanes, &[], None, true);
    assert_eq!(range.end - range.begin, 2 * 3_600_000);
    // Each piece of work is a bar from its start to its end, at least a point wide.
    let (a, b, _) = *lane.marks.iter().find(|m| m.2 == rhythm::Mark::Work).expect("the session worked");
    let (x, w) = range.bar(a, b, track[2]).unwrap();
    let bar = inside(&rt, &format!("track-{}", lane.id), "work-0").expect("the work bar").rect;
    assert!((bar[0] - (track[0] + x)).abs() < 0.5 && (bar[2] - w).abs() < 0.5, "a bar at {x} wide {w}: {bar:?}");

    // The pointer over it names the time, the session and the work.
    rt.pointer_move(track[0] + x + w / 2.0, track[1] + day::LANE / 2.0);
    settle(&mut rt);
    let line = rt.elements().into_iter().find(|e| e.key.as_deref() == Some("pointer-text")).and_then(|e| e.text).unwrap();
    assert!(line.ends_with(&format!(" · {} · agent working", lane.label.trim_end())), "{line}");
    assert_eq!(line.len() - line.trim_start_matches(|c: char| c.is_ascii_digit() || c == ':').len(), 5, "it starts with the time, HH:MM: {line}");

    // The message is a mark at its moment, and the You lane shows the sitting.
    let message = inside(&rt, &format!("track-{}", lane.id), "message-0").unwrap().rect;
    let at = range.x(lane.marks[1].0, track[2]);
    assert!((message[0] + message[2] / 2.0 - (track[0] + at)).abs() < 0.5, "at 14:40: {message:?}");
    assert!(inside(&rt, "track-you", "present-0").is_some());
    assert!(rt.elements().iter().any(|e| e.key.as_deref() == Some("legend")));
    assert_eq!(text_in(&rt, "day-cache", ""), "", "the day's lines under the lanes");
    assert!(texts(&rt).iter().any(|t| t == "no cold returns"), "{:?}", texts(&rt));

    // A click on the lane selects its session in the list, with the day beside it.
    click(&mut rt, &format!("lane-{}", lane.id));
    assert_eq!(rt.state.browser.selected(), rt.state.browser.row_of("id3"));
    assert_eq!(rt.state.browser.selected(), Some(3));
    assert!(!rt.state.browser.zoomed());

    // A filter that leaves the session out leaves its lane out, and says so.
    rt.update(|s, _| {
        let view = s.viewport();
        for c in "/air".chars() {
            s.browser.on_key(Key::Char(c), view);
        }
        s.browser.on_key(Key::Enter, view);
    });
    settle(&mut rt);
    assert!(rt.presented("lane-id3").is_none(), "id3 is in ~/src/app, not ~/src/air");
    assert!(rt.presented("lanes-none").is_some(), "no session of this day matches");
    assert!(rt.presented("lane-you").is_some(), "your own lane stays");
    rt.update(|s, _| s.browser.clear_filter());
    settle(&mut rt);
    assert!(rt.presented("lane-id3").is_some() && rt.presented("lanes-none").is_none(), "back with the filter cleared");
}

#[test]
fn a_countdown_reads_minutes_and_seconds_rounded_up() {
    assert_eq!(view::countdown(24 * 60_000 + 47_000), "24:47");
    assert_eq!(view::countdown(24 * 60_000 + 46_001), "24:47", "a part of a second still counts");
    assert_eq!(view::countdown(59_000), "0:59");
    assert_eq!(view::countdown(3_723_000), "1:02:03");
    assert_eq!(view::countdown(-5), "0:00");
}

#[test]
fn the_list_keeps_its_width_and_the_inspector_takes_the_rest() {
    for width in [1400.0, 960.0] {
        let rt = runtime(desk_of(3), (width, 640.0));
        let list = rt.presented("list-pane").unwrap().0;
        let inspector = rt.presented("inspector").unwrap().0;
        assert_eq!(list[2], view::LIST_WIDTH, "at {width}");
        assert_eq!(inspector[2], width - view::LIST_WIDTH - 1.0, "at {width}: the rest, less the line between them");
    }
}

#[test]
fn nothing_of_the_window_s_own_lies_under_the_traffic_lights() {
    let mut rt = Runtime::new(desk_of(3), view::view, (1400.0, 900.0), Env::new(true, 1.0));
    rt.set_titlebar_inset(78.0);
    settle(&mut rt);
    let lights = CHROME.traffic_light_rects().unwrap();
    let all = rt.elements();
    let overlaps = |r: [f32; 4], l: [f32; 4]| r[0] < l[0] + l[2] && l[0] < r[0] + r[2] && r[1] < l[1] + l[3] && l[1] < r[1] + r[3];
    // Only the bar and what holds it are under the buttons.
    let holders = ["root", "bar"];
    for e in all.iter().filter(|e| e.rect[2] > 0.0 && e.rect[3] > 0.0) {
        if lights.iter().any(|&l| overlaps(e.rect, l)) {
            assert!(e.key.as_deref().is_some_and(|k| holders.contains(&k)) || e.parent.is_none(), "{:?} at {:?} is under the traffic lights", e.key, e.rect);
        }
    }
    let back = rt.presented("day-back").unwrap().0;
    assert_eq!(back[0], 78.0 + 16.0, "the day's controls start right of the buttons");
}

#[test]
fn the_bar_names_the_day_and_its_buttons_step_it() {
    let mut rt = runtime(desk_of(3), (1400.0, 900.0));
    let today = tokprof_core::fmt::local_time(rt.state.browser.day_noon(), "%A, %-d %B");
    assert_eq!(text_in(&rt, "bar", "day-name"), today);
    assert!(rt.state.browser.day_is_latest());
    click(&mut rt, "day-back");
    settle(&mut rt);
    // The latest day is not known in a browser that computes nothing, so the step goes nowhere
    // yet; the keys and the buttons share the model's step either way.
    rt.update(|s, _| s.browser.step_day_by(-1));
    click(&mut rt, "today");
    settle(&mut rt);
    assert!(rt.state.browser.day_is_latest(), "Today goes back to the latest day");
    click(&mut rt, "filter");
    settle(&mut rt);
    assert!(rt.state.browser.editing(), "a click on the filter starts typing it");
    assert!(inside(&rt, "bar", "caret").is_some() && inside(&rt, "bar", "filter-text").is_none(), "an empty field shows the caret alone");
    typed(&mut rt, "air");
    assert_eq!(text_in(&rt, "bar", "filter-text"), "air", "the text, and a thin caret after it");
    assert!(inside(&rt, "bar", "caret").is_some_and(|c| c.rect[2] < 2.0));
    assert_eq!(rt.state.browser.len(), 1);
    press(&mut rt, RuneKey::Enter);
    assert!(inside(&rt, "bar", "caret").is_none(), "no caret once the filter is typed");
    click(&mut rt, "filter-clear");
    settle(&mut rt);
    assert_eq!((rt.state.browser.filter(), rt.state.browser.len(), rt.state.browser.editing()), ("", 3, false), "× clears the filter");
    click(&mut rt, "zoom-day");
    settle(&mut rt);
    assert!(!rt.state.fit, "24 h");
}

#[test]
fn the_list_title_counts_the_sessions_and_the_sorts_switch() {
    let mut rt = runtime(desk_of(30), (1400.0, 900.0));
    assert_eq!(text_in(&rt, "list-head", "list-title"), "30 sessions, $0.00, costing 30");
    assert!(texts(&rt).iter().any(|t| t == "Thu 1 Jan 1970"), "a separator names the day: {:?}", &texts(&rt)[..20]);
    click(&mut rt, "sort-Cost");
    settle(&mut rt);
    assert_eq!(rt.state.browser.sort_order(), tokprof_browse::browser::Sort::Cost);
    assert!(!texts(&rt).iter().any(|t| t == "Thu 1 Jan 1970"), "no separators but in time order");
}

#[test]
fn keys_move_filter_sort_and_quit() {
    let mut rt = runtime(desk_of(30), (1400.0, 900.0));
    let selected = |rt: &Runtime<Desk>| rt.state.browser.selected();
    press(&mut rt, RuneKey::Text("j".into()));
    assert_eq!(selected(&rt), Some(1));
    press(&mut rt, RuneKey::Text("G".into()));
    assert_eq!(selected(&rt), Some(29), "G goes to the last session");
    assert!(rt.presented("id29").is_some_and(|(r, _)| r[1] < 900.0), "and the list scrolls to show it");
    press(&mut rt, RuneKey::Text("g".into()));
    assert_eq!(selected(&rt), Some(0), "g to the first");

    typed(&mut rt, "/air");
    assert!(rt.state.browser.editing());
    assert_eq!(rt.state.browser.len(), 10, "every third session is in ~/src/air");
    press(&mut rt, RuneKey::Enter);
    assert!(!rt.state.browser.editing() && rt.state.browser.filter() == "air", "⏎ keeps the filter");
    press(&mut rt, RuneKey::Escape);
    assert_eq!(rt.state.browser.len(), 30, "esc clears it");

    press(&mut rt, RuneKey::Text("s".into()));
    assert_eq!(rt.state.browser.sort_order(), tokprof_browse::browser::Sort::Cost, "s sorts by cost");
    press(&mut rt, RuneKey::Text("r".into()));
    assert_eq!(rt.state.browser.sort_order(), tokprof_browse::browser::Sort::Cost, "r sorts again in the same order");

    typed(&mut rt, "/q");
    assert_eq!(rt.state.browser.filter(), "q", "q is typed into a filter");
    assert_eq!(rt.shell_request, None);
    press(&mut rt, RuneKey::Escape);
    assert!(!rt.state.browser.editing() && rt.state.browser.filter().is_empty(), "esc while typing clears the filter");
    press(&mut rt, RuneKey::Text("q".into()));
    assert_eq!(rt.shell_request, Some(Shell::Quit), "q quits");
}

#[test]
fn command_q_and_w_quit_and_esc_quits_but_from_full_size() {
    let mut rt = runtime(desk_of(3), (1400.0, 900.0));
    command(&mut rt, 'w');
    assert_eq!(rt.shell_request, Some(Shell::Close));
    let mut rt = runtime(desk_of(3), (1400.0, 900.0));
    command(&mut rt, 'q');
    assert_eq!(rt.shell_request, Some(Shell::Quit));
    let mut rt = runtime(desk_of(3), (1400.0, 900.0));
    press(&mut rt, RuneKey::Enter);
    assert!(rt.state.browser.zoomed() && rt.presented("lanes").is_none(), "⏎ shows the inspector full size");
    press(&mut rt, RuneKey::Escape);
    assert_eq!(rt.shell_request, None, "esc in the full-size inspector goes back");
    assert!(rt.presented("lanes").is_some());
    press(&mut rt, RuneKey::Escape);
    assert_eq!(rt.shell_request, Some(Shell::Quit), "esc with no filter quits");
}

#[test]
fn a_click_selects_a_session_a_double_click_zooms_and_the_wheel_scrolls() {
    let mut rt = runtime(desk_of(60), (1400.0, 900.0));
    click(&mut rt, "id4");
    assert_eq!(rt.state.browser.selected(), Some(4), "a click selects its row");
    let (x, y) = rt.find("id4").unwrap();
    rt.frame(0.6);
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    rt.frame(0.05);
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    settle(&mut rt);
    assert!(rt.state.browser.zoomed(), "a double click shows its inspector full size");
    assert!(rt.presented("list-pane").is_none(), "the list is not drawn");

    let mut rt = runtime(desk_of(60), (1400.0, 900.0));
    let (x, y) = rt.find("id3").unwrap();
    rt.wheel(x, y, -400.0);
    settle(&mut rt);
    assert!(rt.state.list_offset.get() > 300.0, "the wheel scrolls the list");
    assert_eq!(rt.state.browser.selected(), Some(0), "and leaves the selection where it was");
}

#[test]
fn a_click_on_a_tab_shows_it() {
    let mut rt = runtime(desk_of(3), (1400.0, 900.0));
    for t in inspector::Tab::ALL.iter().rev() {
        click(&mut rt, &format!("tab-{}", t.name()));
        settle(&mut rt);
        assert_eq!(rt.state.tab, *t);
    }
}

#[test]
fn rows_on_the_screen_stay_when_a_session_joins_above_them() {
    let mut rt = runtime(desk_of(80), (1400.0, 900.0));
    let (x, y) = rt.find("id3").unwrap();
    rt.wheel(x, y, -480.0);
    settle(&mut rt);
    let on_screen = ["id25", "id30", "id33"];
    let before: Vec<[f32; 4]> = on_screen.iter().map(|k| rt.presented(k).unwrap().0).collect();
    rt.update(|s, _| s.browser.apply_change(Change::Session(Box::new(row(80, "~/src/new", 20_000)))));
    settle(&mut rt);
    assert_eq!(rt.state.browser.session(0), "id80", "the new session is first in time order");
    let after: Vec<[f32; 4]> = on_screen.iter().map(|k| rt.presented(k).unwrap().0).collect();
    assert_eq!(after, before, "the rows on the screen stay where they are");

    // At the top of the list, a new session is on the screen, and the rows below it move down.
    let mut rt = runtime(desk_of(80), (1400.0, 900.0));
    let first = rt.presented("id0").unwrap().0;
    rt.update(|s, _| s.browser.apply_change(Change::Session(Box::new(row(80, "~/src/new", 20_000)))));
    settle(&mut rt);
    let moved = rt.presented("id0").unwrap().0;
    assert_eq!(moved[1], first[1] + view::ROW, "the rows below it move down by one");
    assert_eq!(rt.state.browser.selected().map(|r| rt.state.browser.session(r).to_string()), Some("id0".into()), "the selection stays on its session");
}

#[test]
fn the_list_comes_back_where_it_was_after_the_full_size_inspector() {
    let mut rt = runtime(desk_of(80), (1400.0, 900.0));
    let (x, y) = rt.find("id3").unwrap();
    rt.wheel(x, y, -480.0);
    settle(&mut rt);
    click(&mut rt, "id30");
    let row = rt.presented("id30").unwrap().0;
    press(&mut rt, RuneKey::Enter);
    assert!(rt.presented("list-pane").is_none());
    press(&mut rt, RuneKey::Escape);
    assert_eq!(rt.presented("id30").map(|p| p.0), Some(row), "the selected row is where it was");
}

#[test]
fn window_keys_are_the_browser_s_keys() {
    let key = |k: RuneKey| key_of(&KeyEvent::from(k));
    assert_eq!(key(RuneKey::Text("j".into())), Some(Key::Char('j')));
    assert_eq!(key(RuneKey::Text("G".into())), Some(Key::Char('G')));
    assert_eq!(key(RuneKey::Escape), Some(Key::Esc));
    assert_eq!(key(RuneKey::Enter), Some(Key::Enter));
    assert_eq!(key(RuneKey::Left), Some(Key::Left), "← steps the day, in the browser's window mode");
    assert_eq!(key(RuneKey::Tab), None, "Tab moves the keyboard between the panes, not the browser");
    assert_eq!(key(RuneKey::Command('c')), None, "⌘C copies the selected text");
    let ctrl_c = KeyEvent { key: RuneKey::Text("c".into()), mods: rune::Mods { ctrl: true, ..rune::Mods::default() } };
    assert_eq!(key_of(&ctrl_c), Some(Key::Interrupt));
}

fn shift_tab(rt: &mut Runtime<Desk>) {
    rt.key(KeyEvent { key: RuneKey::Tab, mods: rune::Mods { shift: true, ..rune::Mods::default() } });
    settle(rt);
}

#[test]
fn tab_moves_the_keyboard_between_the_panes_and_esc_brings_it_back() {
    let mut rt = runtime(desk_of(30), (1400.0, 900.0));
    assert_eq!(rt.state.pane, Focus::List, "the list has the keyboard to start with");
    press(&mut rt, RuneKey::Tab);
    assert_eq!(rt.state.pane, Focus::Inspector, "Tab: lanes, list, inspector");
    press(&mut rt, RuneKey::Text("j".into()));
    press(&mut rt, RuneKey::Down);
    assert_eq!(rt.state.browser.selected(), Some(0), "the list's keys are the list's");
    press(&mut rt, RuneKey::Tab);
    assert_eq!(rt.state.pane, Focus::Lanes, "and round again");
    shift_tab(&mut rt);
    assert_eq!(rt.state.pane, Focus::Inspector, "Shift+Tab back");
    press(&mut rt, RuneKey::Escape);
    assert_eq!((rt.state.pane, rt.shell_request), (Focus::List, None), "esc gives the keyboard back to the list");
    press(&mut rt, RuneKey::Down);
    assert_eq!(rt.state.browser.selected(), Some(1), "and ↓ moves in the list again");

    // A click gives the pane it lands in the keyboard.
    click(&mut rt, "tab-Tools");
    assert_eq!((rt.state.pane, rt.state.tab), (Focus::Inspector, inspector::Tab::Tools));
    click(&mut rt, "id4");
    assert_eq!(rt.state.pane, Focus::List);
}

#[test]
fn the_lanes_step_to_the_lane_above_below_first_and_last() {
    let to = |at, step| lane_to(3, at, step);
    assert_eq!(to(Some(1), LaneStep::Down), Some(2));
    assert_eq!(to(Some(2), LaneStep::Down), Some(2), "the last stays");
    assert_eq!(to(Some(1), LaneStep::Up), Some(0));
    assert_eq!(to(Some(0), LaneStep::Up), Some(0), "the first stays");
    assert_eq!(to(Some(1), LaneStep::First), Some(0));
    assert_eq!(to(Some(0), LaneStep::Last), Some(2));
    assert_eq!(to(None, LaneStep::Down), Some(0), "from a session without a lane that day, the first");
    assert_eq!(lane_to(0, None, LaneStep::Down), None, "no lanes, nowhere to go");
    let step = |k: RuneKey| lane_step(&KeyEvent::from(k));
    assert_eq!((step(RuneKey::Text("j".into())), step(RuneKey::Text("k".into()))), (Some(LaneStep::Down), Some(LaneStep::Up)));
    assert_eq!((step(RuneKey::Home), step(RuneKey::End), step(RuneKey::Left)), (Some(LaneStep::First), Some(LaneStep::Last), None), "← steps the day");

    // Without a day yet, the lanes keep the keys that move through them.
    let mut rt = runtime(desk_of(30), (1400.0, 900.0));
    rt.update(|s, _| s.pane = Focus::Lanes);
    press(&mut rt, RuneKey::Down);
    assert_eq!(rt.state.browser.selected(), Some(0), "↓ with the lanes' keyboard leaves the list alone");
}

#[test]
fn the_inspector_s_keys_switch_tabs_and_move_the_cursor_through_rows() {
    let (catalog, select) = fixture();
    let mut browser = Browser::start(catalog, Pricing::defaults(), select, false, Arc::new(|| {}), frontend()).unwrap();
    browser.settle();
    let mut rt = runtime(Desk::new(browser, false), (1400.0, 900.0));
    press(&mut rt, RuneKey::Tab);
    press(&mut rt, RuneKey::Text("7".into()));
    assert_eq!(rt.state.tab, inspector::Tab::Tools, "7 shows the seventh tab");
    let rows = rt.state.tab_rows.get();
    assert!(rows >= 2, "the fixture's tools: {rows}");
    press(&mut rt, RuneKey::Down);
    press(&mut rt, RuneKey::Text("j".into()));
    assert_eq!(rt.state.cursor, 2.min(rows - 1), "↓ and j move the cursor");
    assert!(rt.presented(&format!("row-{}", rt.state.cursor)).is_some(), "its row is on the screen");
    press(&mut rt, RuneKey::End);
    assert_eq!(rt.state.cursor, rows - 1, "End to the last row");
    press(&mut rt, RuneKey::Text("k".into()));
    assert_eq!(rt.state.cursor, rows - 2);
    press(&mut rt, RuneKey::Home);
    assert_eq!(rt.state.cursor, 0, "Home to the first");
    press(&mut rt, RuneKey::Right);
    assert_eq!((rt.state.tab, rt.state.cursor), (inspector::Tab::Context, 0), "→ the next tab, from its first row");
    press(&mut rt, RuneKey::Text("h".into()));
    assert_eq!(rt.state.tab, inspector::Tab::Tools, "h the tab before");
    press(&mut rt, RuneKey::Left);
    press(&mut rt, RuneKey::Left);
    assert_eq!(rt.state.tab, inspector::Tab::Waits);
    assert_eq!(rt.state.browser.selected(), Some(0), "the session stays selected");
    press(&mut rt, RuneKey::Text("1".into()));
    assert_eq!((rt.state.tab, rt.state.tab_rows.get()), (inspector::Tab::Summary, 0), "Summary has no rows to move through");
    press(&mut rt, RuneKey::Down);
    assert_eq!(rt.state.cursor, 0, "↓ scrolls it instead");
}

#[test]
fn a_start_from_a_terminal_hands_the_window_over_and_any_other_runs_here() {
    let plain = Path::new("/Users/me/.local/bin/tokprof-desk");
    let bundled = Path::new("/Applications/tokprof.app/Contents/MacOS/tokprof-desk");
    assert_eq!(start_of(bundled, false, false, true), Start::Bundle(PathBuf::from("/Applications/tokprof.app")), "a bundle opens through Launch Services");
    assert_eq!(start_of(plain, false, false, true), Start::Detached, "a bare binary starts itself apart");
    assert_eq!(start_of(bundled, false, false, false), Start::Here, "Launch Services and the detached process start it without a terminal");
    assert_eq!(start_of(plain, true, false, true), Start::Here, "a frame without a window stays");
    assert_eq!(start_of(plain, false, true, true), Start::Here, "--wait stays");
    assert_eq!(bundle_of(Path::new("/x/tokprof.app/Contents/Resources/tokprof-desk")), None);
    assert_eq!(bundle_of(Path::new("/x/Contents/MacOS/tokprof-desk")), None, "not in a .app");
    assert!(Cli::parse_from(["tokprof-desk", "--wait"]).wait);
}

#[test]
fn a_start_from_a_terminal_fails_there_without_logs() {
    let (catalog, dir) = empty_catalog("start");
    let err = check_before_start(&Cli::parse_from(["tokprof-desk"]), &catalog).err().map(|e| e.to_string());
    assert_eq!(err.as_deref(), Some("no session logs found"));
    let (catalog, _) = fixture();
    assert!(check_before_start(&Cli::parse_from(["tokprof-desk"]), &catalog).is_ok(), "the fixture has a session");
    let _ = std::fs::remove_dir_all(dir);
}
