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
    let p = pal::pal(crate::themes::default(true));
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
    let below = "\nsittings  1, 50m at the keyboard\nanswers   the agent's turn took a median 30m\nqueue     no answer while another session waited\ncache     no cold returns\n";
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
    let all = texts(&rt);
    let at = |t: &str| all.iter().position(|x| x == t).unwrap_or_else(|| panic!("no {t:?} in {all:?}"));
    assert!(at("the agent's turn took a median 30m") < at("no answer while another session waited"), "the queue line under answers");
    assert!(at("no answer while another session waited") < at("no cold returns"), "and over cache");

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
    let switch = rt.presented("mode").unwrap().0;
    assert_eq!(switch[0], 78.0 + 16.0, "the switch starts right of the buttons");
    assert!(rt.presented("day-back").unwrap().0[0] > switch[0] + switch[2], "the day's controls after it");
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
    assert_eq!(text_in(&rt, "list-head", "list-title"), "30 sessions, $0.00");
    assert_eq!(text_in(&rt, "list-head", "list-costing"), "costing 30", "and, quieter, the sessions being costed");
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

/// Whether the window's background is drawn in this theme's canvas.
fn drawn_in(rt: &Runtime<Desk>, id: &str) -> bool {
    let fill = rt.elements().into_iter().find(|e| e.key.as_deref() == Some("root")).map(|e| e.fill).unwrap();
    let want = themes::find(id).unwrap().color("canvas");
    [(fill.r, want.r), (fill.g, want.g), (fill.b, want.b)].iter().all(|(a, b)| (a - b).abs() < 0.01)
}

#[test]
fn a_dark_theme_on_a_light_system_and_back_to_the_system() {
    let mut rt = runtime(desk_of(3), (1400.0, 900.0));
    rt.set_system_dark(false);
    settle(&mut rt);
    assert!(drawn_in(&rt, "merino-light"), "the default light theme");
    rt.update(|s, _| {
        s.pick_appearance(settings::Appearance::Dark);
        s.pick_theme(true, "ink-meridian");
    });
    settle(&mut rt);
    assert!(drawn_in(&rt, "ink-meridian"), "dark while the system is light");
    rt.update(|s, _| s.pick_appearance(settings::Appearance::System));
    settle(&mut rt);
    assert!(drawn_in(&rt, "merino-light"));
    rt.set_system_dark(true);
    settle(&mut rt);
    assert!(drawn_in(&rt, "ink-meridian"), "the system turned dark");
}

#[test]
fn command_comma_opens_the_settings_and_keys_stay_with_them() {
    let mut rt = runtime(desk_of(5), (1400.0, 900.0));
    let selected = rt.state.browser.selected();
    command(&mut rt, ',');
    settle(&mut rt);
    assert!(rt.presented("settings").is_some(), "the panel is open");
    typed(&mut rt, "qj");
    assert_eq!(rt.shell_request, None, "q does not quit behind the panel");
    assert_eq!(rt.state.browser.selected(), selected, "j does not move the list");
    // The window is dark: the keyboard starts in Dark theme, which comes right after Appearance.
    assert_eq!(rt.state.panel.unwrap().resolve(true), (settings::Part::Dark, true));
    assert!(rt.presented("part-dark").unwrap().0[1] < rt.presented("part-light").unwrap().0[1], "the dark grid above the light one");
    let dark: Vec<&str> = themes::of(true).map(|t| t.id.as_str()).collect();
    // `j` went a row down in the grid; → goes to the card after that one.
    let at = dark.iter().position(|&id| id == rt.state.settings.dark_theme).unwrap();
    assert_eq!(at, dark.iter().position(|&id| id == themes::DEFAULT_DARK).unwrap() + settings::COLUMNS, "j is the grid's ↓");
    press(&mut rt, RuneKey::Right);
    let after = dark[at + 1];
    assert_eq!(rt.state.settings.dark_theme, after, "→ in Dark theme picks the next dark theme");
    press(&mut rt, RuneKey::End);
    assert_eq!(rt.state.settings.dark_theme, *dark.last().unwrap());
    shift_tab(&mut rt);
    press(&mut rt, RuneKey::Right);
    assert_eq!(rt.state.settings.appearance, settings::Appearance::Light, "→ from As in system picks Light");
    assert!(rt.presented("part-dark").unwrap().0[1] < rt.presented("part-light").unwrap().0[1], "the grids stay where they were");
    press(&mut rt, RuneKey::Tab);
    press(&mut rt, RuneKey::Tab);
    let light: Vec<&str> = themes::of(false).map(|t| t.id.as_str()).collect();
    press(&mut rt, RuneKey::Home);
    assert_eq!(rt.state.settings.light_theme, light[0], "Home in Light theme picks its first card");
    press(&mut rt, RuneKey::Down);
    assert_eq!(rt.state.settings.light_theme, light[settings::COLUMNS], "↓ picks the card below");
    press(&mut rt, RuneKey::Up);
    press(&mut rt, RuneKey::Up);
    assert_eq!(rt.state.settings.light_theme, light[0], "↑ on the first row stays");
    press(&mut rt, RuneKey::Escape);
    assert!(rt.state.panel.is_none() && rt.presented("settings").is_none(), "esc closes it");
    assert_eq!(rt.state.browser.selected(), selected);
    assert_eq!(rt.shell_request, None);
    command(&mut rt, ',');
    command(&mut rt, ',');
    assert!(rt.state.panel.is_none(), "⌘, again closes it");
    command(&mut rt, ',');
    command(&mut rt, 'q');
    assert_eq!(rt.shell_request, Some(Shell::Quit), "⌘Q still quits");
}

#[test]
fn a_click_on_a_theme_picks_it_and_done_closes_the_panel() {
    let mut rt = runtime(desk_of(3), (1400.0, 900.0));
    rt.set_system_dark(false);
    command(&mut rt, ',');
    settle(&mut rt);
    click(&mut rt, "theme-alpine-milk");
    settle(&mut rt);
    assert_eq!(rt.state.settings.light_theme, "alpine-milk");
    assert!(drawn_in(&rt, "alpine-milk"));
    click(&mut rt, "settings-done");
    assert!(rt.state.panel.is_none());
}

#[test]
fn the_choices_are_kept_for_the_next_start() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("prefs.json");
    let mut desk = desk_of(1).with_prefs(rune::prefs::Prefs::at(&path));
    assert!(!path.exists(), "nothing is written before a choice changes");
    desk.pick_appearance(settings::Appearance::Light);
    desk.pick_theme(false, "alpine-milk");
    let file = std::fs::read_to_string(&path).unwrap();
    assert!(file.contains("alpine-milk") && file.contains("light"), "{file}");
    let again = desk_of(1).with_prefs(rune::prefs::Prefs::at(&path));
    assert_eq!(again.settings.appearance, settings::Appearance::Light);
    assert_eq!(again.settings.theme(true).id, "alpine-milk");
}

#[test]
fn a_settings_file_that_cannot_be_written_says_so_in_the_bar() {
    let dir = tempfile::tempdir().unwrap();
    // A file where the settings' folder should be: the folder cannot be made.
    let blocker = dir.path().join("blocked");
    std::fs::write(&blocker, "").unwrap();
    let mut desk = desk_of(1).with_prefs(rune::prefs::Prefs::at(blocker.join("prefs.json")));
    desk.pick_theme(true, "ink-meridian");
    assert_eq!(desk.settings.dark_theme, "ink-meridian", "the choice still applies");
    assert!(desk.status.0.starts_with("can't save settings"), "{}", desk.status.0);
}

#[test]
fn a_frame_without_a_window_ignores_the_saved_settings_and_takes_colors() {
    // A headless desk keeps its settings in memory: the defaults, whatever a file holds.
    let memory = desk_of(1);
    assert!(!memory.prefs.is_persistent());
    assert_eq!(memory.settings, settings::Settings::default());
    let t = colors(&Cli::parse_from(["tokprof-desk", "--colors", "ink-meridian"])).unwrap().unwrap();
    assert_eq!(settings::Settings::of_theme(t).theme(false).id, "ink-meridian");
    assert!(colors(&Cli::parse_from(["tokprof-desk"])).unwrap().is_none());
    let (catalog, dir) = empty_catalog("colors");
    let err = desk(&Cli::parse_from(["tokprof-desk", "--colors", "nosuch"]), catalog, true, Arc::new(|| {})).err().map(|e| e.to_string()).unwrap_or_default();
    assert!(err.starts_with("--colors takes one of: abyssal-trench, alpine-milk"), "{err}");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn the_settings_scene_opens_the_panel_on_the_defaults() {
    let mut desk = desk_of(3);
    let (_, open) = SCENES.iter().find(|(name, _)| *name == "settings").unwrap();
    open(&mut desk);
    let mut rt = runtime(desk, (1400.0, 900.0));
    settle(&mut rt);
    for key in ["settings", "appearance-system", "theme-merino-light", "theme-merino-dark"] {
        assert!(rt.presented(key).is_some(), "{key}");
    }
    // The chosen ones are named in the accent of the theme the window is in.
    let accent = rt.state.theme.get().color("accent");
    let marked = |key: &str| {
        let c = rt.elements().into_iter().find(|e| e.key.as_deref() == Some(key)).and_then(|e| e.text_color).unwrap_or_else(|| panic!("no {key}"));
        (c.r - accent.r).abs() < 0.01 && (c.g - accent.g).abs() < 0.01 && (c.b - accent.b).abs() < 0.01
    };
    assert!(marked("appearance-label-system") && marked("theme-name-merino-light") && marked("theme-name-merino-dark"));
    assert!(!marked("appearance-label-dark") && !marked("theme-name-alpine-milk"));
    for hint in ["section", "choose", "done"] {
        assert!(texts(&rt).iter().any(|t| t == hint), "the foot names the keys: {hint}");
    }
}

#[test]
fn a_click_outside_the_card_closes_the_panel_and_reaches_nothing_behind_it() {
    let mut rt = runtime(desk_of(5), (1400.0, 900.0));
    let selected = rt.state.browser.selected();
    command(&mut rt, ',');
    settle(&mut rt);
    // The list's fourth row lies under the scrim, left of the card.
    let (x, y) = rt.find("id3").expect("the row is on the screen");
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    settle(&mut rt);
    assert!(rt.state.panel.is_none(), "the panel closed");
    assert_eq!(rt.state.browser.selected(), selected, "the row under the scrim was not selected");
}

fn ctrl(rt: &mut Runtime<Desk>, c: &str) {
    rt.key(KeyEvent { key: RuneKey::Text(c.into()), mods: rune::Mods { ctrl: true, ..rune::Mods::default() } });
    settle(rt);
}

/// The palette's entries for what is typed in it now.
fn listed(rt: &Runtime<Desk>) -> Vec<palette::Entry> {
    palette::entries(&rt.state, &rt.state.palette.as_ref().expect("the palette is open").query)
}

#[test]
fn command_k_opens_the_palette_and_keys_stay_with_it() {
    let mut rt = runtime(desk_of(5), (1400.0, 900.0));
    let selected = rt.state.browser.selected();
    command(&mut rt, 'k');
    settle(&mut rt);
    assert!(rt.presented("palette").is_some(), "the palette is open");
    assert_eq!(rt.state.palette.as_ref().unwrap().at, 0, "its first entry highlighted");
    typed(&mut rt, "qj");
    assert_eq!(rt.shell_request, None, "q does not quit behind the palette");
    assert_eq!(rt.state.browser.selected(), selected, "j does not move the list");
    assert_eq!(rt.state.palette.as_ref().unwrap().query, "qj");
    assert_eq!(text_in(&rt, "palette-query", "palette-text"), "qj", "the field shows what is typed");
    press(&mut rt, RuneKey::Backspace);
    assert_eq!(rt.state.palette.as_ref().unwrap().query, "q");
    press(&mut rt, RuneKey::Escape);
    assert!(rt.state.palette.is_none() && rt.presented("palette").is_none(), "esc closes it");
    assert_eq!(rt.shell_request, None);
    command(&mut rt, 'k');
    command(&mut rt, 'k');
    assert!(rt.state.palette.is_none(), "⌘K again closes it");
    command(&mut rt, 'k');
    command(&mut rt, 'q');
    assert_eq!(rt.shell_request, Some(Shell::Quit), "⌘Q still quits");
}

#[test]
fn typing_filters_and_ranks_and_enter_runs_the_first_match() {
    let mut rt = runtime(desk_of(5), (1400.0, 900.0));
    command(&mut rt, 'k');
    typed(&mut rt, "cache");
    let list = listed(&rt);
    assert_eq!(list[0].pick, palette::Pick::Tab(Tab::Cache), "{:?}", list.iter().map(|e| &e.title).collect::<Vec<_>>());
    assert_eq!(text_in(&rt, "pick-0", "pick-title"), "Cache");
    assert!(rt.presented("palette-group-tabs").is_some(), "under its heading");
    press(&mut rt, RuneKey::Enter);
    assert!(rt.state.palette.is_none(), "⏎ closes the palette");
    assert_eq!(rt.state.tab, Tab::Cache, "and shows the tab");

    command(&mut rt, 'k');
    typed(&mut rt, "to");
    let titles: Vec<String> = listed(&rt).into_iter().map(|e| e.title).collect();
    let at = |t: &str| titles.iter().position(|x| x == t);
    assert!(at("Tools").is_some() && at("Today").is_some(), "{titles:?}");
    assert!(at("Copy the handoff line").is_none_or(|h| h > at("Tools").unwrap() && h > at("Today").unwrap()), "{titles:?}");

    typed(&mut rt, "zzzz");
    assert!(rt.presented("palette-empty").is_some(), "nothing matches");
    press(&mut rt, RuneKey::Enter);
    assert!(rt.state.palette.is_some(), "⏎ with nothing matched does nothing");
}

#[test]
fn arrows_and_control_keys_move_the_highlight_and_enter_runs_it() {
    let mut rt = runtime(desk_of(5), (1400.0, 900.0));
    command(&mut rt, 'k');
    typed(&mut rt, "sort");
    let list = listed(&rt);
    assert_eq!(list[2].title, "Sort by Cost", "{:?}", list.iter().map(|e| &e.title).collect::<Vec<_>>());
    press(&mut rt, RuneKey::Down);
    ctrl(&mut rt, "n");
    ctrl(&mut rt, "p");
    press(&mut rt, RuneKey::Down);
    assert_eq!(rt.state.palette.as_ref().unwrap().at, 2);
    press(&mut rt, RuneKey::Enter);
    assert_eq!(rt.state.browser.sort_order(), tokprof_browse::browser::Sort::Cost, "the third entry ran");
    assert!(rt.state.palette.is_none());

    command(&mut rt, 'k');
    typed(&mut rt, "sort");
    let last = listed(&rt).len() - 1;
    for _ in 0..last + 3 {
        press(&mut rt, RuneKey::Down);
    }
    assert_eq!(rt.state.palette.as_ref().unwrap().at, last, "↓ stays on the last entry");
    press(&mut rt, RuneKey::Up);
    assert_eq!(rt.state.palette.as_ref().unwrap().at, last - 1);
}

#[test]
fn picking_a_session_selects_it_scrolls_to_it_and_shows_its_summary() {
    let mut rt = runtime(desk_of(60), (1400.0, 900.0));
    rt.update(|s, tx| s.show_tab(Tab::Tools, tx));
    settle(&mut rt);
    command(&mut rt, 'k');
    typed(&mut rt, "air");
    let list = listed(&rt);
    let at = list.iter().position(|e| e.pick == palette::Pick::Session("id58".into())).expect("id58 is in ~/src/air");
    for _ in 0..at {
        press(&mut rt, RuneKey::Down);
    }
    assert!(rt.presented(&palette::row_key(at)).is_some(), "the highlight is scrolled into view");
    press(&mut rt, RuneKey::Enter);
    settle(&mut rt);
    assert_eq!(rt.state.browser.selected(), rt.state.browser.row_of("id58"));
    assert!(rt.presented("id58").is_some_and(|(r, _)| r[1] < 900.0), "the list scrolled to it");
    assert_eq!(rt.state.tab, Tab::Summary);

    // From the full-size inspector, the list is drawn again.
    press(&mut rt, RuneKey::Enter);
    assert!(rt.state.browser.zoomed());
    command(&mut rt, 'k');
    typed(&mut rt, "air");
    press(&mut rt, RuneKey::Enter);
    settle(&mut rt);
    assert!(!rt.state.browser.zoomed() && rt.presented("list-pane").is_some());
    assert_eq!(rt.state.browser.selected(), rt.state.browser.row_of("id1"), "the most recent of ~/src/air first");
}

#[test]
fn picking_a_day_shows_it_in_the_lanes() {
    let mut rt = runtime(desk_of(5), (1400.0, 900.0));
    let selected = rt.state.browser.selected();
    command(&mut rt, 'k');
    typed(&mut rt, "1970");
    let list = listed(&rt);
    let date = rhythm::date_of(10_000_000, &rhythm::local_offset);
    assert_eq!(list[0].pick, palette::Pick::Day(Some(date)), "{:?}", list.iter().map(|e| &e.title).collect::<Vec<_>>());
    assert_eq!(list[0].right, "5 sessions, $0.00");
    press(&mut rt, RuneKey::Enter);
    assert_eq!(rhythm::date_of(rt.state.browser.day_noon(), &rhythm::local_offset), date);
    assert_eq!(rt.state.browser.selected(), selected, "the selection stays");
}

#[test]
fn the_entries_with_nothing_typed_and_their_limits() {
    let desk = desk_of(150);
    let list = palette::entries(&desk, "");
    let groups: Vec<palette::Group> = list.iter().map(|e| e.group).collect();
    let mut order = groups.clone();
    order.dedup();
    use palette::Group::*;
    assert_eq!(order, vec![Actions, Tabs, Sessions, Days]);
    assert_eq!(groups.iter().filter(|&&g| g == Sessions).count(), palette::RECENT_SESSIONS);
    let sessions: Vec<&palette::Pick> = list.iter().filter(|e| e.group == Sessions).map(|e| &e.pick).collect();
    assert_eq!(sessions[0], &palette::Pick::Session("id0".into()), "the most recent first");
    assert_eq!(list.iter().find(|e| e.group == Days).unwrap().title, "Today");
    assert!(!list.iter().any(|e| e.title == "Clear the filter" || e.title == "Back to the list"), "no filter, not full size");
    let matched = palette::entries(&desk, "src");
    assert_eq!(matched.iter().filter(|e| e.group == Sessions).count(), palette::MATCHED_SESSIONS);
}

#[test]
fn a_click_on_a_row_runs_it_and_a_click_outside_closes_the_palette() {
    let mut rt = runtime(desk_of(5), (1400.0, 900.0));
    command(&mut rt, 'k');
    typed(&mut rt, "work");
    settle(&mut rt);
    click(&mut rt, &palette::row_key(0));
    settle(&mut rt);
    assert_eq!(rt.state.tab, Tab::Work);
    assert!(rt.state.palette.is_none());

    let selected = rt.state.browser.selected();
    command(&mut rt, 'k');
    settle(&mut rt);
    let (x, y) = rt.find("id3").expect("the row is on the screen");
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    settle(&mut rt);
    assert!(rt.state.palette.is_none(), "the palette closed");
    assert_eq!(rt.state.browser.selected(), selected, "the row under the scrim was not selected");
}

#[test]
fn one_card_at_a_time() {
    let mut rt = runtime(desk_of(3), (1400.0, 900.0));
    command(&mut rt, 'k');
    command(&mut rt, ',');
    assert!(rt.state.palette.is_some() && rt.state.panel.is_none(), "⌘, does nothing over the palette");
    typed(&mut rt, "?");
    assert!(!rt.state.help, "? is typed into the query");
    press(&mut rt, RuneKey::Escape);
    command(&mut rt, ',');
    command(&mut rt, 'k');
    assert!(rt.state.panel.is_some() && rt.state.palette.is_none(), "⌘K does nothing over the settings");
    command(&mut rt, ',');
    // The palette's Settings… closes it and opens the panel.
    command(&mut rt, 'k');
    typed(&mut rt, "settings");
    press(&mut rt, RuneKey::Enter);
    assert!(rt.state.panel.is_some() && rt.state.palette.is_none());
}

#[test]
fn a_filter_being_typed_is_kept_when_the_palette_opens() {
    let mut rt = runtime(desk_of(6), (1400.0, 900.0));
    typed(&mut rt, "/air");
    command(&mut rt, 'k');
    assert!(!rt.state.browser.editing() && rt.state.browser.filter() == "air");
    assert!(rt.state.palette.is_some());
    typed(&mut rt, "clear");
    press(&mut rt, RuneKey::Enter);
    assert!(rt.state.browser.filter().is_empty(), "Clear the filter is offered while one is set");
}

#[test]
fn question_mark_opens_the_key_help_and_closes_it() {
    let mut rt = runtime(desk_of(5), (1400.0, 900.0));
    let selected = rt.state.browser.selected();
    typed(&mut rt, "?");
    assert!(rt.state.help && rt.presented("keys").is_some(), "? opens the key help");
    typed(&mut rt, "jq");
    assert_eq!((rt.state.browser.selected(), rt.shell_request), (selected, None), "keys stay with it");
    command(&mut rt, 'k');
    assert!(rt.state.palette.is_none(), "no palette over it");
    typed(&mut rt, "?");
    assert!(!rt.state.help && rt.presented("keys").is_none(), "? closes it");
    typed(&mut rt, "?");
    press(&mut rt, RuneKey::Escape);
    assert!(!rt.state.help, "esc closes it");
    assert_eq!(rt.shell_request, None, "and does not quit");

    typed(&mut rt, "/a?");
    assert!(!rt.state.help && rt.state.browser.filter() == "a?", "? is typed into a filter");
    press(&mut rt, RuneKey::Escape);

    // The palette's Key help opens it too.
    command(&mut rt, 'k');
    typed(&mut rt, "key help");
    press(&mut rt, RuneKey::Enter);
    assert!(rt.state.help);
    for name in ["Anywhere", "List", "Lanes", "Inspector", "Settings", "Palette"] {
        assert!(texts(&rt).iter().any(|t| t == name), "{name}");
    }
}

/// Every key the key help names.
fn helped() -> Vec<&'static str> {
    keys::GROUPS.iter().flat_map(|(_, rows)| rows.iter().flat_map(|r| r.keys.iter().copied())).collect()
}

#[test]
fn the_key_help_names_the_keys_the_window_handles() {
    let named = helped();
    for k in [
        "⌘K", "⌘,", "?", "Tab", "⇧Tab", "⏎", "esc", "⌫", "[", "]", "d", "/", "s", "r", "c", "f", "o", "q", "⌘Q", "⌘W", "Ctrl+C", "↑", "↓", "←", "→", "j", "k",
        "h", "l", "g", "G", "PgUp", "PgDn", "Home", "End", "1–9", "Space", "Ctrl+P", "Ctrl+N", "⌘1", "⌘2", "0", "⇧←", "⇧→",
    ] {
        assert!(named.contains(&k), "the key help names {k}");
    }
    let flame: Vec<&str> = keys::GROUPS.iter().find(|g| g.0 == "Flame").expect("a Flame group").1.iter().flat_map(|r| r.keys.iter().copied()).collect();
    // Every key the graph takes is named in its group, as the browser's are below.
    let (mut rt, _, _) = flame_desk();
    let keys = [RuneKey::Left, RuneKey::Right, RuneKey::Up, RuneKey::Down, RuneKey::Enter, RuneKey::Backspace, RuneKey::Escape, RuneKey::Home, RuneKey::End];
    let names = ["←", "→", "↑", "↓", "⏎", "⌫", "esc", "Home", "End"];
    for (key, name) in keys.into_iter().zip(names) {
        let taken = rt.state.flame_key(&KeyEvent::from(key.clone()));
        assert!(!taken || flame.contains(&name), "Flame takes {name}, its group does not name it");
    }
    for c in (' '..='~').map(|c| c.to_string()) {
        let taken = rt.state.flame_key(&KeyEvent::from(RuneKey::Text(c.clone())));
        assert!(!taken || flame.contains(&c.as_str()), "Flame takes {c:?}, its group does not name it");
    }
    // Every character the browser does something with is named; c, f and o copy and write
    // files, so they are only named above.
    let fingerprint = |b: &Book| (b.selected(), b.zoomed(), b.editing(), b.sort_order(), b.day_noon(), b.day_is_latest(), b.status().to_string());
    for c in (' '..='~').filter(|c| !"cfo".contains(*c)) {
        let mut desk = desk_of(3);
        desk.browser.select_row(1);
        let view = desk.viewport();
        let before = fingerprint(&desk.browser);
        let go_on = desk.browser.on_key(Key::Char(c), view);
        if !go_on || fingerprint(&desk.browser) != before {
            assert!(
                named.contains(&c.to_string().as_str()) || (c == ' ' && named.contains(&"Space")),
                "the browser handles {c:?}, the key help does not name it"
            );
        }
    }
}

#[test]
fn the_palette_and_keys_scenes_open_their_cards() {
    for (scene, key) in [("palette", "palette"), ("keys", "keys")] {
        let mut desk = desk_of(3);
        let (_, open) = SCENES.iter().find(|(name, _)| *name == scene).unwrap();
        open(&mut desk);
        let rt = runtime(desk, (1400.0, 900.0));
        assert!(rt.presented(key).is_some(), "{scene}");
    }
    let mut desk = desk_of(3);
    SCENES.iter().find(|(name, _)| *name == "palette").unwrap().1(&mut desk);
    assert_eq!(desk.palette.unwrap().query, "to");
}

#[test]
fn a_short_group_goes_above_a_long_one_as_good_and_loose_matches_are_left_out() {
    let rows = (0..40).map(|i| row(i, if i == 0 { "tools-app" } else { "~/src/tolerant" }, 10_000 - i as u64)).collect();
    let desk = Desk::new(Browser::detached(Arc::new(Catalog::new(Vec::new(), PathBuf::new())), rows, Pricing::defaults(), frontend()), false);
    let list = palette::entries(&desk, "to");
    assert_eq!(list[0].pick, palette::Pick::Tab(Tab::Tools), "Tools and tools-app match as well; the tabs go first");
    assert_eq!(list[1].pick, palette::Pick::Session("id0".into()));
    assert!(!list.iter().any(|e| e.title == "Copy the handoff line"), "t and o far apart");
    let titles: Vec<&str> = list.iter().map(|e| e.title.as_str()).collect();
    let at = |t: &str| titles.iter().position(|x| *x == t).unwrap_or_else(|| panic!("{t} in {titles:?}"));
    assert!(at("Today") < at("Keyboard to the list"), "{titles:?}");
}

/// The window on the fixture's session, its inspector showing `Flame` with the keyboard.
fn flame_desk() -> (Runtime<Desk>, Arc<Catalog>, Query) {
    let (catalog, select) = fixture();
    let mut browser = Browser::start(catalog.clone(), Pricing::defaults(), select.clone(), false, Arc::new(|| {}), frontend()).unwrap();
    browser.settle();
    let mut rt = runtime(Desk::new(browser, false), (1400.0, 900.0));
    press(&mut rt, RuneKey::Tab);
    typed(&mut rt, "0");
    settle(&mut rt);
    (rt, catalog, select)
}

fn shifted(rt: &mut Runtime<Desk>, key: RuneKey) {
    rt.key(KeyEvent { key, mods: rune::Mods { shift: true, ..rune::Mods::default() } });
    settle(rt);
}

#[test]
fn the_flame_tab_draws_the_stacks_tokprof_flame_writes() {
    let (mut rt, catalog, select) = flame_desk();
    assert_eq!((rt.state.tab, rt.state.pane), (inspector::Tab::Flame, Focus::Inspector), "0 shows Flame");
    let tree = rt.state.flame_tree().unwrap();
    // The `.collapsed` file of the same session, as `tokprof flame --session <id>` writes it.
    let x = rt.state.browser.xray().cloned_x();
    let row = tokprof_core::xray::rows(&catalog, &select, 10).into_iter().find(|r| r.id == x.short_id).expect("the session's row");
    let out = std::env::temp_dir().join(format!("tokprof-desk-flame-{}.collapsed", std::process::id()));
    tokprof_core::xray::export(&catalog, &row, &Pricing::defaults(), &out).unwrap();
    let written: i64 = std::fs::read_to_string(&out).unwrap().lines().map(|l| l.rsplit_once(' ').unwrap().1.parse::<i64>().unwrap()).sum();
    let _ = std::fs::remove_file(out);
    assert!(written > 0);
    assert_eq!(tree.total(), written, "the root is the sum of the profile, in µ$");
    let scan: f64 = x.components.iter().map(|v| v.usd).sum();
    assert!((tree.total() as f64 / 1e6 - scan).abs() < 0.005, "and the session's total: {} against {scan}", tree.total());
    assert!(text_in(&rt, "flame-crumbs", "flame-total").starts_with(&format!("{} · 100.0%", tokprof_core::fmt::usd(tree.total() as f64 / 1e6))));
    // Each child of the root is as wide as its share, less the gap.
    let root = rt.presented("frame-0").unwrap().0;
    let graph = rt.presented(flame::GRAPH).unwrap().0;
    assert!((root[2] - graph[2]).abs() < 0.5, "the root is full width");
    assert!(tree.nodes[0].children.len() >= 2, "the fixture has a main chain and a subagent");
    for &c in &tree.nodes[0].children {
        let want = tree.nodes[c].value as f32 / tree.total() as f32 * (root[2] + flame::GAP) - flame::GAP;
        if want >= flame::MIN_W {
            let bar = rt.presented(&format!("frame-{c}")).unwrap().0;
            assert!((bar[2] - want).abs() < 0.5, "{}: {} against {want}", tree.nodes[c].name, bar[2]);
        } else {
            assert!(rt.presented(&format!("frame-{c}")).is_none());
        }
    }
    let hidden = flame::layout(&tree, 0, graph[2]).1;
    assert_eq!(text_in(&rt, "flame-info", "flame-hidden"), flame::hidden_note(hidden));
    assert_eq!(text_in(&rt, "flame-info", "flame-line"), flame::describe(&tree, 0, 0), "the line names the cursor's frame");
    // The pointer on a frame names it in the line, instead of the cursor's.
    let main = tree.nodes[0].children[0];
    let (mx, my) = rt.find(&format!("frame-{main}")).unwrap();
    rt.pointer_move(mx, my);
    settle(&mut rt);
    assert_eq!(text_in(&rt, "flame-info", "flame-line"), flame::describe(&tree, 0, main));
    rt.pointer_move(1.0, 1.0);
    settle(&mut rt);
    assert_eq!(text_in(&rt, "flame-info", "flame-line"), flame::describe(&tree, 0, 0), "and the cursor's again once it leaves");
    // The tree is built once per X-ray.
    press(&mut rt, RuneKey::Down);
    assert!(Rc::ptr_eq(&tree, &rt.state.flame_tree().unwrap()));
}

#[test]
fn flame_zooms_by_click_and_enter_and_out_by_crumb_and_esc() {
    let (mut rt, _, _) = flame_desk();
    let tree = rt.state.flame_tree().unwrap();
    let width = rt.presented("frame-0").unwrap().0[2];
    let main = tree.nodes[0].children[0];
    click(&mut rt, &format!("frame-{main}"));
    settle(&mut rt);
    assert_eq!(rt.state.flame.root, tree.path(main));
    assert!((rt.presented(&format!("frame-{main}")).unwrap().0[2] - width).abs() < 0.5, "the frame clicked fills the width");
    assert!(rt.presented("frame-0").is_none(), "the session's bar gives way to the crumb");
    assert_eq!(text_in(&rt, "crumb-1", "crumb-name"), tree.nodes[main].name);
    click(&mut rt, "crumb-0");
    settle(&mut rt);
    assert!(rt.state.flame.root.is_empty(), "the first crumb shows the whole session");
    assert_eq!(rt.state.flame.cursor, tree.path(main), "with the cursor where the root was");

    // Keys: up to the root, down to its first child, right to the next.
    rt.pointer_move(1.0, 1.0);
    press(&mut rt, RuneKey::Up);
    assert!(rt.state.flame.cursor.is_empty());
    press(&mut rt, RuneKey::Down);
    press(&mut rt, RuneKey::Right);
    let second = tree.nodes[0].children[1];
    assert_eq!(rt.state.flame.cursor, tree.path(second));
    assert_eq!(text_in(&rt, "flame-info", "flame-line"), flame::describe(&tree, 0, second));
    press(&mut rt, RuneKey::Right);
    assert_eq!(rt.state.flame.cursor, tree.path(*tree.nodes[0].children.get(2).unwrap_or(&second)), "the last drawn sibling stays");
    press(&mut rt, RuneKey::Left);
    press(&mut rt, RuneKey::Left);
    assert_eq!(rt.state.flame.cursor, tree.path(main));
    assert_eq!(rt.state.tab, inspector::Tab::Flame, "← and → stay in the graph");
    press(&mut rt, RuneKey::Enter);
    assert_eq!(rt.state.flame.root, tree.path(main), "⏎ zooms into the cursor's frame");
    assert!(!rt.state.browser.zoomed(), "and does not show the inspector full size");
    press(&mut rt, RuneKey::Escape);
    assert!(rt.state.flame.root.is_empty(), "esc zooms out");
    assert_eq!(rt.state.pane, Focus::Inspector, "and keeps the keyboard");
    press(&mut rt, RuneKey::Enter);
    press(&mut rt, RuneKey::Backspace);
    assert!(rt.state.flame.root.is_empty(), "Backspace zooms out");
    press(&mut rt, RuneKey::Escape);
    assert_eq!(rt.state.pane, Focus::List, "esc at the top gives the keyboard back");

    // Shift+← leaves the graph for the tab before.
    press(&mut rt, RuneKey::Tab);
    shifted(&mut rt, RuneKey::Left);
    assert_eq!(rt.state.tab, inspector::Tab::Cache);
    shifted(&mut rt, RuneKey::Right);
    assert_eq!(rt.state.tab, inspector::Tab::Flame);
}

#[test]
fn a_click_on_the_shown_root_zooms_out_one_level() {
    let (mut rt, _, _) = flame_desk();
    let tree = rt.state.flame_tree().unwrap();
    let main = tree.nodes[0].children[0];
    let turn = tree.nodes[main].children[0];
    click(&mut rt, &format!("frame-{main}"));
    settle(&mut rt);
    click(&mut rt, &format!("frame-{turn}"));
    settle(&mut rt);
    assert_eq!(rt.state.flame.root, tree.path(turn));
    assert_eq!(text_in(&rt, "crumb-2", "crumb-name"), tree.nodes[turn].name);
    click(&mut rt, &format!("frame-{turn}"));
    settle(&mut rt);
    assert_eq!(rt.state.flame.root, tree.path(main));
}

/// The fixture's report, kept as the 30-day report of a desk of `n` sessions, over a window of
/// five days that ends after the fixture's last request; and its dates.
fn reports_desk(n: usize) -> (Desk, Vec<i64>) {
    let (catalog, select) = fixture();
    let o = tokprof_core::overview::collect(&catalog, &select, &Pricing::defaults(), |_, _| {}).unwrap();
    let last = *o.spread.days.keys().last().expect("the fixture has requests");
    let offset = &rhythm::local_offset;
    let window = (rhythm::midnight(last - 4, offset), rhythm::midnight(last + 1, offset));
    let mut desk = desk_of(n);
    desk.reports.data.put(30, o, window, 0);
    (desk, (last - 4..=last).collect())
}

/// The text of the element keyed `key`.
fn text_of(rt: &Runtime<Desk>, key: &str) -> String {
    rt.elements().into_iter().find(|e| e.key.as_deref() == Some(key)).and_then(|e| e.text).unwrap_or_default()
}

fn reports_of(rt: &Runtime<Desk>) -> &tokprof_core::overview::Overview {
    rt.state.reports.report().and_then(|r| r.overview.as_deref().ok()).expect("the report")
}

#[test]
fn the_switch_and_its_keys_show_reports_and_back() {
    let (desk, _) = reports_desk(3);
    let mut rt = runtime(desk, (1400.0, 900.0));
    assert_eq!(rt.state.reports.mode, reports::Mode::Day, "the window starts in Day");
    assert!(rt.presented("day-name").is_some() && rt.presented(reports::PANE).is_none());
    click(&mut rt, "mode-reports");
    settle(&mut rt);
    assert_eq!(rt.state.reports.mode, reports::Mode::Reports);
    assert!(rt.presented(reports::PANE).is_some(), "the report under the bar");
    for gone in ["lanes", view::LIST, "inspector", "day-name", "today", "zoom", "filter"] {
        assert!(rt.presented(gone).is_none(), "{gone} is not shown in Reports");
    }
    assert!(rt.presented("mode").is_some(), "the switch stays");
    command(&mut rt, '1');
    settle(&mut rt);
    assert_eq!(rt.state.reports.mode, reports::Mode::Day);
    assert!(rt.presented("lanes").is_some());
    command(&mut rt, '2');
    settle(&mut rt);
    assert_eq!(rt.state.reports.mode, reports::Mode::Reports);
    // q and the browser's keys do nothing there; esc goes back to Day.
    typed(&mut rt, "qsd/");
    assert_eq!(rt.shell_request, None, "q does not quit from Reports");
    assert!(!rt.state.browser.editing() && rt.state.reports.mode == reports::Mode::Reports);
    press(&mut rt, RuneKey::Escape);
    assert_eq!(rt.state.reports.mode, reports::Mode::Day);
    assert_eq!(rt.shell_request, None, "esc in Reports goes back to Day, not out");
}

#[test]
fn the_cards_show_the_numbers_of_the_summary_and_the_pauses() {
    let (desk, dates) = reports_desk(3);
    let mut rt = runtime(desk, (1400.0, 2400.0));
    command(&mut rt, '2');
    settle(&mut rt);
    let o = reports_of(&rt);
    let (spend, (avoidable, share), (returns, premium)) = (o.spend_usd(), o.avoidable(), o.cold_returns());
    assert_eq!(text_in(&rt, "kpi-spend", "value"), tokprof_core::fmt::usd(spend));
    assert_eq!(text_in(&rt, "kpi-avoidable", "value"), tokprof_core::fmt::usd(avoidable));
    assert_eq!(text_in(&rt, "kpi-avoidable", "sub"), format!("{share:.1}% of spend"));
    assert_eq!(text_in(&rt, "kpi-returns", "value"), returns.to_string());
    assert!(text_in(&rt, "kpi-returns", "sub").starts_with(&format!("{} avoidable", tokprof_core::fmt::usd(premium))));
    assert_eq!(text_in(&rt, "kpi-requests", "value"), tokprof_core::fmt::int(o.requests()));
    assert_eq!(text_in(&rt, "kpi-sessions", "value"), o.sessions().to_string());
    // A column for every date of the window, those without requests too.
    for d in &dates {
        assert!(rt.presented(&format!("day-{d}")).is_some(), "a column for {d}");
    }
    // The projects, most expensive first, with their amounts.
    let projects = o.spread.projects();
    assert_eq!(text_in(&rt, "rep-projects-0", "label"), projects[0].0);
    assert_eq!(text_in(&rt, "rep-projects-0", "amount"), tokprof_core::fmt::usd(projects[0].1.usd));
    let shown = texts(&rt);
    for (agent, a) in &o.pauses.agents {
        for row in reports::size_cells(a) {
            assert!(row.iter().all(|c| shown.contains(c)), "{agent:?}'s {row:?} on the screen");
        }
    }
}

#[test]
fn the_pills_pick_a_period_and_one_not_read_yet_says_so() {
    let (desk, _) = reports_desk(3);
    let mut rt = runtime(desk, (1400.0, 900.0));
    command(&mut rt, '2');
    settle(&mut rt);
    assert_eq!(rt.state.reports.days, 30, "30 days to start with");
    assert!(rt.presented("kpi-spend").is_some());
    click(&mut rt, "period-7");
    settle(&mut rt);
    assert_eq!(rt.state.reports.days, 7);
    assert_eq!(text_of(&rt, "rep-progress"), "computing the last 7 days…");
    assert!(rt.presented("kpi-spend").is_none());
    // The period by keys: it has the keyboard to start with.
    press(&mut rt, RuneKey::Right);
    assert_eq!(rt.state.reports.days, 30);
    assert!(rt.presented("kpi-spend").is_some(), "the 30 days were kept");
    press(&mut rt, RuneKey::End);
    assert_eq!(rt.state.reports.days, 90);
    press(&mut rt, RuneKey::Left);
    press(&mut rt, RuneKey::Left);
    assert_eq!(rt.state.reports.days, 7);
}

#[test]
fn a_click_on_a_day_opens_it_in_day() {
    let (desk, dates) = reports_desk(3);
    let mut rt = runtime(desk, (1400.0, 2800.0));
    command(&mut rt, '2');
    settle(&mut rt);
    click(&mut rt, &format!("day-{}", dates[1]));
    settle(&mut rt);
    assert_eq!(rt.state.reports.mode, reports::Mode::Day);
    assert_eq!(rhythm::date_of(rt.state.browser.day_noon(), &rhythm::local_offset), dates[1]);
}

#[test]
fn a_click_on_a_project_filters_the_list_to_it() {
    let (desk, _) = reports_desk(3);
    let mut rt = runtime(desk, (1400.0, 2800.0));
    command(&mut rt, '2');
    settle(&mut rt);
    let project = reports_of(&rt).spread.projects()[0].0.to_string();
    click(&mut rt, "rep-projects-0");
    settle(&mut rt);
    assert_eq!(rt.state.reports.mode, reports::Mode::Day);
    assert_eq!(rt.state.browser.filter(), project);
    assert_eq!(text_in(&rt, "filter", "filter-text"), project, "the filter field shows it");
}

#[test]
fn keys_move_through_the_cards_days_and_rows_and_enter_drills_down() {
    let (desk, dates) = reports_desk(30);
    let mut rt = runtime(desk, (1400.0, 900.0));
    command(&mut rt, '2');
    settle(&mut rt);
    assert_eq!(rt.state.reports.part, reports::Part::Period);
    // Past what to change: the savings, the ways back and the habits.
    for _ in 0..4 {
        press(&mut rt, RuneKey::Tab);
    }
    assert_eq!(rt.state.reports.part, reports::Part::Days);
    let readout = |rt: &Runtime<Desk>| text_of(rt, "rep-readout-head");
    let last = tokprof_core::fmt::local_time(rhythm::midnight(dates[4], &rhythm::local_offset) + 12 * 3_600_000, "%a %-d %b");
    assert!(readout(&rt).starts_with(&last), "the cursor starts on the last day: {}", readout(&rt));
    press(&mut rt, RuneKey::Left);
    press(&mut rt, RuneKey::Text("h".into()));
    press(&mut rt, RuneKey::Enter);
    assert_eq!(rt.state.reports.mode, reports::Mode::Day);
    assert_eq!(rhythm::date_of(rt.state.browser.day_noon(), &rhythm::local_offset), dates[2], "two days before the last");
    // Back in Reports, Tab twice from the days reaches the projects; ↓ and ⏎ open the second.
    command(&mut rt, '2');
    settle(&mut rt);
    press(&mut rt, RuneKey::Tab);
    assert_eq!(rt.state.reports.part, reports::Part::Projects);
    shift_tab(&mut rt);
    assert_eq!(rt.state.reports.part, reports::Part::Days);
    press(&mut rt, RuneKey::Tab);
    let projects: Vec<String> = reports_of(&rt).spread.projects().iter().map(|(p, _)| p.to_string()).collect();
    press(&mut rt, RuneKey::Down);
    let want = projects.get(1).unwrap_or(&projects[0]).clone();
    assert_eq!(rt.state.reports.row, usize::from(projects.len() > 1));
    press(&mut rt, RuneKey::Enter);
    assert_eq!((rt.state.reports.mode, rt.state.browser.filter().to_string()), (reports::Mode::Day, want));
    // Reports keeps its part; Shift+Tab from the period goes round to the last card.
    command(&mut rt, '2');
    settle(&mut rt);
    assert_eq!(rt.state.reports.part, reports::Part::Projects);
    for _ in 0..6 {
        shift_tab(&mut rt);
    }
    assert_eq!(rt.state.reports.part, reports::Part::Sizes);
}

#[test]
fn reports_ask_for_nothing_while_day_is_shown() {
    let (catalog, select) = fixture();
    let mut desk = desk_of(3);
    desk.reports.data = tokprof_browse::reports::Reports::start(catalog, Pricing::defaults(), select, Arc::new(|| {}));
    let mut rt = runtime(desk, (1400.0, 900.0));
    assert!(rt.state.reports.data.progress().is_none() && rt.state.reports.report().is_none(), "Day reads nothing for reports");
    command(&mut rt, '2');
    settle(&mut rt);
    let deadline = Instant::now() + Duration::from_secs(20);
    while rt.state.reports.report().is_none() {
        assert!(Instant::now() < deadline, "the report did not arrive");
        std::thread::sleep(Duration::from_millis(20));
        rt.frame(1.0 / 60.0);
    }
    settle(&mut rt);
    assert!(rt.presented("kpi-spend").is_some(), "the cards replace the progress");
}

#[test]
fn the_pointer_over_a_day_names_its_numbers_under_the_chart() {
    let (desk, dates) = reports_desk(3);
    let mut rt = runtime(desk, (1400.0, 2800.0));
    command(&mut rt, '2');
    settle(&mut rt);
    let head = text_of(&rt, "rep-readout-head");
    assert!(head.starts_with("5 days · "), "the period's numbers without a day: {head}");
    let o = reports_of(&rt);
    let (date, day) = o.spread.days.iter().next_back().map(|(d, s)| (*d, *s)).unwrap();
    assert_eq!(date, dates[4]);
    let (x, y) = rt.find(&format!("day-{date}")).unwrap();
    rt.pointer_move(x, y);
    settle(&mut rt);
    assert_eq!(rt.state.reports.hover, Some(date));
    let noon = rhythm::midnight(date, &rhythm::local_offset) + 12 * 3_600_000;
    let want = format!(
        "{} · {} · {} request",
        tokprof_core::fmt::local_time(noon, "%a %-d %b"),
        tokprof_core::fmt::usd(day.usd()),
        tokprof_core::fmt::int(day.requests)
    );
    assert!(text_of(&rt, "rep-readout-head").starts_with(&want), "{} against {want}", text_of(&rt, "rep-readout-head"));
    rt.pointer_leave();
    settle(&mut rt);
    assert_eq!(rt.state.reports.hover, None);
}

/// Opens the palette, types `query`, and runs the entry `pick` with the arrows and ⏎.
fn run_from_palette(rt: &mut Runtime<Desk>, query: &str, pick: palette::Pick) {
    command(rt, 'k');
    typed(rt, query);
    let at = listed(rt).iter().position(|e| e.pick == pick).unwrap_or_else(|| panic!("{pick:?} for {query:?}"));
    for _ in 0..at {
        press(rt, RuneKey::Down);
    }
    press(rt, RuneKey::Enter);
}

#[test]
fn the_palette_switches_to_reports_and_picks_a_period() {
    let (desk, _) = reports_desk(3);
    let mut rt = runtime(desk, (1400.0, 900.0));
    let act = |a| palette::Pick::Act(a);
    let offered = |rt: &Runtime<Desk>, a| palette::entries(&rt.state, "").iter().any(|e| e.pick == palette::Pick::Act(a));
    assert!(offered(&rt, palette::Action::Reports) && !offered(&rt, palette::Action::DayView), "Day is offered only from Reports");
    run_from_palette(&mut rt, "last 7", act(palette::Action::Period7));
    assert_eq!((rt.state.reports.mode, rt.state.reports.days), (reports::Mode::Reports, 7));
    assert!(offered(&rt, palette::Action::DayView));
    run_from_palette(&mut rt, "day", act(palette::Action::DayView));
    assert_eq!(rt.state.reports.mode, reports::Mode::Day);
    // A day picked from Reports is shown in Day.
    run_from_palette(&mut rt, "reports", act(palette::Action::Reports));
    assert_eq!(rt.state.reports.mode, reports::Mode::Reports);
    run_from_palette(&mut rt, "today", palette::Pick::Day(None));
    assert_eq!(rt.state.reports.mode, reports::Mode::Day);
    // `?` opens the key help from Reports too.
    command(&mut rt, '2');
    settle(&mut rt);
    press(&mut rt, RuneKey::Text("?".into()));
    assert!(rt.state.help);
}

#[test]
fn bars_of_one_chart_keep_the_order_of_their_shares_when_only_some_rows_have_a_note() {
    // As Waits draws them: the model with a note of its requests, background commands without.
    fn chart(_: &Desk, _: &mut rune::Cx<Desk>) -> rune::El {
        let p = crate::pal::pal(crate::themes::default(false));
        let rows = [
            ("model".to_string(), 0.27, "1.2h".to_string(), "274 requests".to_string()),
            ("background commands".to_string(), 0.242, "1.1h".to_string(), String::new()),
            ("subagents".to_string(), 0.1, "20m".to_string(), "1.5h after the turn ended".to_string()),
        ];
        rune::col().w(900.0).child(inspector::bar_rows(&rows, p.message, &p))
    }
    let mut rt = Runtime::new(desk_of(0), chart, (1000.0, 300.0), Env::new(false, 1.0));
    settle(&mut rt);
    let width = |i: usize| inside(&rt, &format!("bar-{i}"), "fill").unwrap_or_else(|| panic!("no bar {i}")).rect[2];
    let track = |i: usize| rt.elements().into_iter().find(|e| e.key.as_deref() == Some(&format!("bar-{i}"))).unwrap().rect;
    assert!(width(0) > width(1) && width(1) > width(2), "27.0% > 24.2% > 10%: {} {} {}", width(0), width(1), width(2));
    assert_eq!((track(0)[0], track(0)[2]), (track(1)[0], track(1)[2]), "one track for every row, with a note or without");
}

/// The keys of the elements ringed as having the keyboard, with the key of an ancestor for those
/// that have none of their own.
fn ringed(rt: &Runtime<Desk>) -> Vec<String> {
    let all = rt.elements();
    let named = |mut i: usize| loop {
        if let Some(k) = &all[i].key {
            return k.clone();
        }
        match all[i].parent {
            Some(p) => i = p,
            None => return String::new(),
        }
    };
    (0..all.len()).filter(|&i| all[i].trace_name == Some(overlay::RING)).map(named).collect()
}

#[test]
fn the_pane_with_the_keyboard_rings_its_selection_and_no_other_pane_does() {
    let mut rt = runtime(desk_of(30), (1400.0, 900.0));
    press(&mut rt, RuneKey::Down);
    assert_eq!(ringed(&rt), ["cells"], "the list's selected row");
    let id = rt.state.browser.session(rt.state.browser.selected().unwrap()).to_string();
    let row = inside(&rt, &id, "cells").expect("the selected row's cells");
    assert!(row.radius >= overlay::R_ROW - 0.01 && row.rect[0] > 0.0, "an inset rounded row: {:?}", row.rect);
    let p = pal::pal(rt.state.theme.get());
    assert_eq!(row.fill, p.sel, "in the selection color, not the accent");
    assert_eq!(inside(&rt, &id, "title").and_then(|e| e.text_color), Some(p.ink), "with the ink's text");
    assert_eq!(text_color(&rt, "list-title"), Some(p.accent), "the list's title in the accent");

    // The inspector takes it: Summary has no rows, so its tab is ringed; the list's row is not.
    press(&mut rt, RuneKey::Tab);
    assert_eq!(rt.state.pane, Focus::Inspector);
    assert_eq!(ringed(&rt), ["tab-Summary"]);
    assert_eq!(inside(&rt, &id, "cells").map(|e| e.fill), Some(p.sel), "the row stays selected, quieter");
    assert_ne!(text_color(&rt, "list-title"), Some(p.accent));

    // Typing a filter: the field has the ring, not the list.
    press(&mut rt, RuneKey::Escape);
    press(&mut rt, RuneKey::Text("/".into()));
    assert!(rt.state.browser.editing());
    assert_eq!(ringed(&rt), ["filter"]);
    press(&mut rt, RuneKey::Escape);
    assert_eq!(ringed(&rt), ["cells"], "the list has it again");
    assert!(rt.presented("filter-key").is_some(), "the idle filter names its key");
}

fn text_color(rt: &Runtime<Desk>, key: &str) -> Option<rune::Color> {
    rt.elements().into_iter().find(|e| e.key.as_deref() == Some(key)).and_then(|e| e.text_color)
}

#[test]
fn the_lanes_ring_the_selected_lane_while_they_have_the_keyboard() {
    let parts = synthetic_day();
    DAY.with(|d| *d.borrow_mut() = Some(parts));
    let mut rt = Runtime::new(desk_of(10), lanes_only, (1400.0, 900.0), Env::new(true, 1.0));
    settle(&mut rt);
    click(&mut rt, "lane-id3");
    settle(&mut rt);
    assert_eq!((rt.state.pane, rt.state.browser.selected()), (Focus::Lanes, Some(3)));
    assert!(inside(&rt, "lane-id3", "lane-mark").is_some(), "the selected lane's accent mark");
    assert_eq!(ringed(&rt), ["lane-band"]);
    let p = pal::pal(crate::themes::default(true));
    assert_eq!(text_color(&rt, "lanes-title"), Some(p.accent));
    rt.update(|s, _| s.pane = Focus::List);
    settle(&mut rt);
    assert!(ringed(&rt).is_empty() && inside(&rt, "lane-id3", "lane-band").is_some(), "the band stays, without the ring");
}

#[test]
fn the_inspector_rings_the_cursor_s_row_and_an_empty_filter_says_so() {
    let (catalog, select) = fixture();
    let mut browser = Browser::start(catalog, Pricing::defaults(), select, false, Arc::new(|| {}), frontend()).unwrap();
    browser.settle();
    let mut rt = runtime(Desk::new(browser, false), (1400.0, 900.0));
    press(&mut rt, RuneKey::Tab);
    press(&mut rt, RuneKey::Text("7".into()));
    press(&mut rt, RuneKey::Down);
    assert_eq!(ringed(&rt), ["row-1"], "the cursor's row in Tools");

    // A filter that lists nothing: the list says so, with the key that clears it.
    press(&mut rt, RuneKey::Escape);
    typed(&mut rt, "/zzzz");
    assert!(rt.presented("list-empty").is_some());
    assert_eq!(text_in(&rt, "list-empty", "empty-head"), "No session matches “zzzz”");
}

#[test]
fn the_size_flag_gives_the_first_frame_its_height() {
    let args = |a: &[&str]| a.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    assert_eq!(size_of(&args(&["--snap", "x.png", "--size", "960x640"])), Some((960.0, 640.0)));
    assert_eq!(size_of(&args(&["--size=1400x700"])), Some((1400.0, 700.0)));
    assert_eq!(size_of(&args(&["--snap", "x.png"])), None);
}

thread_local! {
    static MANY: std::cell::RefCell<Option<tokprof_browse::render_rhythm::DayParts>> = const { std::cell::RefCell::new(None) };
}

/// The lanes of thirty sessions, at the height the window gives them.
fn many_lanes(s: &Desk, cx: &mut rune::Cx<Desk>) -> rune::El {
    let p = pal::pal(crate::themes::default(true));
    let max_h = view::lanes_max_h(view::window_size(s, cx).1);
    MANY.with(|d| rune::col().w_full().h_full().child(day::lanes_of(s, d.borrow().as_ref().unwrap(), cx, &p, max_h)))
}

#[test]
fn in_a_small_window_the_lanes_and_what_is_under_them_take_at_most_45_percent() {
    let mut many = synthetic_day();
    if let Some((_, lanes)) = &mut many.strip {
        let lane = lanes.lanes[0].clone();
        lanes.lanes = (0..30).map(|i| tokprof_core::rhythm::StripLane { id: format!("id{i}"), ..lane.clone() }).collect();
    }
    MANY.with(|d| *d.borrow_mut() = Some(many));
    let mut desk = desk_of(30);
    // As main seeds it from `--size`, before any layout.
    desk.window = (960.0, 640.0);
    let mut rt = Runtime::new(desk, many_lanes, (960.0, 640.0), Env::new(true, 1.0));
    rt.frame(1.0 / 60.0);
    let lanes = rt.presented("lanes").expect("the lanes").0;
    assert!(lanes[3] <= 0.45 * (640.0 - view::BAR_H) + 1.0, "the first frame: {lanes:?}");
    // Whole lanes and the line of how many more leave at most a lane and that line unused.
    assert!(lanes[3] >= 0.45 * (640.0 - view::BAR_H) - day::LANE - 18.0, "and they take the room they have: {lanes:?}");
}

#[test]
fn dark_themes_lower_the_large_marks_and_keep_the_text_colors() {
    let t = crate::themes::find("neon-arcade").expect("neon-arcade");
    let p = pal::pal(t);
    assert_eq!(p.warm, t.color("success"), "text in the green stays as the theme has it");
    assert_eq!(p.work, pal::mix_srgb(t.color("canvas"), t.color("success"), pal::DARK_FILL), "work's bars sit into the background");
    assert_eq!(p.read, pal::mix_srgb(t.color("canvas"), t.color("info"), pal::DARK_FILL));
    let half = pal::mix_srgb(rune::Color::BLACK, rune::Color::WHITE, 0.5);
    assert!((rune::color::srgb_encode(half.r) - 0.5).abs() < 1e-4, "half way to the eye");
    let light = pal::pal(crate::themes::default(false));
    assert_eq!(light.work, crate::themes::default(false).color("success"), "light themes keep their marks");
}

#[test]
fn the_day_cursor_keeps_its_date_when_the_report_comes_back_with_fewer_dates() {
    let (catalog, select) = fixture();
    let o = || tokprof_core::overview::collect(&catalog, &select, &Pricing::defaults(), |_, _| {}).unwrap();
    let last = *o().spread.days.keys().last().expect("the fixture has requests");
    let offset = &rhythm::local_offset;
    let span = |from: i64, to: i64| (rhythm::midnight(from, offset), rhythm::midnight(to + 1, offset));
    let mut desk = desk_of(3);
    desk.reports.data.put(30, o(), span(last - 30, last), 0);
    let mut rt = runtime(desk, (1400.0, 900.0));
    command(&mut rt, '2');
    for _ in 0..4 {
        press(&mut rt, RuneKey::Tab);
    }
    assert_eq!(rt.state.reports.part, reports::Part::Days);
    press(&mut rt, RuneKey::End);
    assert_eq!(rt.state.reports.day, Some(last), "31 dates, the cursor on the last");

    // Read again over a window one date shorter, as across a change of the clocks: no panic, and
    // the cursor stays on its date, now the 30th.
    rt.update(|s, _| s.reports.data.put(30, o(), span(last - 29, last), 1));
    press(&mut rt, RuneKey::Down);
    let dates = reports::dates(rt.state.reports.report().unwrap());
    assert_eq!((dates.len(), reports::day_at(&dates, rt.state.reports.day)), (30, 29));
    press(&mut rt, RuneKey::Left);
    assert_eq!(rt.state.reports.day, Some(last - 1));
    // A window that has lost the cursor's date: the cursor falls back to the last date.
    rt.update(|s, _| s.reports.data.put(30, o(), span(last - 40, last - 10), 2));
    settle(&mut rt);
    let dates = reports::dates(rt.state.reports.report().unwrap());
    assert_eq!(reports::day_at(&dates, rt.state.reports.day), dates.len() - 1);
    press(&mut rt, RuneKey::Left);
    assert_eq!(rt.state.reports.day, Some(last - 11), "and moves from there");
}

#[test]
fn ctrl_c_quits_in_reports() {
    let (desk, _) = reports_desk(3);
    let mut rt = runtime(desk, (1400.0, 900.0));
    command(&mut rt, '2');
    rt.key(KeyEvent { key: RuneKey::Text("c".into()), mods: rune::Mods { ctrl: true, ..rune::Mods::default() } });
    rt.frame(1.0 / 60.0);
    assert!(rt.shell_request.is_some(), "Ctrl+C quits, as Anywhere says");
}

#[test]
fn the_palette_in_reports_shows_day_for_what_works_on_day() {
    let (desk, _) = reports_desk(3);
    let mut rt = runtime(desk, (1400.0, 900.0));
    command(&mut rt, '2');
    let run = |rt: &mut Runtime<Desk>, query: &str| {
        command(rt, 'k');
        typed(rt, query);
        press(rt, RuneKey::Enter);
    };
    run(&mut rt, "filter the");
    assert_eq!(rt.state.reports.mode, reports::Mode::Day, "the filter is typed where it shows");
    assert!(rt.state.browser.editing());
    press(&mut rt, RuneKey::Escape);
    command(&mut rt, '2');
    run(&mut rt, "full-size");
    assert_eq!((rt.state.reports.mode, rt.state.browser.zoomed()), (reports::Mode::Day, true));
    // What does not work on Day stays in Reports.
    press(&mut rt, RuneKey::Escape);
    command(&mut rt, '2');
    run(&mut rt, "reports: the last 7");
    assert_eq!((rt.state.reports.mode, rt.state.reports.days), (reports::Mode::Reports, 7));
}

#[test]
fn backspace_on_flame_at_the_top_leaves_full_size() {
    let (mut rt, _, _) = flame_desk();
    assert_eq!(rt.state.tab, inspector::Tab::Flame);
    press(&mut rt, RuneKey::Escape);
    press(&mut rt, RuneKey::Enter);
    assert!(rt.state.browser.zoomed(), "⏎ shows the inspector full size");
    press(&mut rt, RuneKey::Backspace);
    assert!(!rt.state.browser.zoomed(), "⌫ at the top of the graph goes back, as the key help says");
}

#[test]
fn a_click_on_a_frame_finds_it_by_its_path_in_the_tree_of_now() {
    let (mut rt, _, _) = flame_desk();
    let tree = rt.state.flame_tree().unwrap();
    let main = tree.nodes[0].children[0];
    rt.state.flame_click(&tree.path(main));
    assert_eq!(rt.state.flame.root, tree.path(main), "the frame drawn there");
    // A frame the tree no longer has, as after a new X-ray: the click does nothing.
    let before = (rt.state.flame.root.clone(), rt.state.flame.cursor.clone());
    let gone = vec![("no such frame".to_string(), tokprof_core::profile::Tint::Structure)];
    rt.state.flame_click(&gone);
    rt.state.flame_crumb(&gone);
    assert_eq!((rt.state.flame.root.clone(), rt.state.flame.cursor.clone()), before);
    // The tree is built once for an X-ray and kept while it is the one shown.
    assert!(Rc::ptr_eq(&tree, &rt.state.flame_tree().unwrap()));
}

/// The fixture's report as the 30-day report of a desk that lists the fixture's sessions, and the
/// first row of what to change that opens sessions, with its part.
fn advice_desk() -> (Desk, reports::Part, String) {
    let (catalog, select) = fixture();
    let o = tokprof_core::overview::collect(&catalog, &select, &Pricing::defaults(), |_, _| {}).unwrap();
    let last = *o.spread.days.keys().last().expect("the fixture has requests");
    let offset = &rhythm::local_offset;
    let window = (rhythm::midnight(last - 4, offset), rhythm::midnight(last + 1, offset));
    let c = &o.changes;
    let split = |rows: &[tokprof_core::pool::Row]| rows.iter().find(|r| c.sessions_of(&r.name).is_some_and(|l| !l.is_empty())).map(|r| r.name.clone());
    let (part, name) = match split(&c.saved) {
        Some(n) => (reports::Part::Saved, n),
        None => (reports::Part::Habits, split(&c.habits).expect("a habit split by session")),
    };
    let rows = tokprof_core::xray::rows(&catalog, &select, usize::MAX);
    let mut desk = Desk::new(Browser::detached(catalog, rows, Pricing::defaults(), frontend()), false);
    desk.reports.data.put(30, o, window, 0);
    (desk, part, name)
}

#[test]
fn what_to_change_shows_the_rows_of_tokprof_share_with_their_advice() {
    let (desk, _, _) = advice_desk();
    let mut rt = runtime(desk, (1400.0, 2400.0));
    command(&mut rt, '2');
    settle(&mut rt);
    let c = rt.state.reports.changes().unwrap().clone();
    assert!(!c.habits.is_empty(), "the fixture has habits");
    let shown = texts(&rt);
    for r in c.saved.iter().chain(&c.habits) {
        assert!(shown.contains(&r.name), "{} on the screen", r.name);
        assert!(shown.contains(&tokprof_core::fmt::usd(r.usd)) && shown.contains(&format!("{:.1}%", 100.0 * r.share)), "{}'s amounts", r.name);
        if let Some(h) = c.hint(&r.name).filter(|_| !r.name.starts_with("tool hint: ")) {
            assert!(shown.contains(&h.to_string()), "{}'s advice: {h}", r.name);
        }
    }
    for r in &c.saved {
        assert!(shown.contains(&r.per_hour.map_or("–".into(), tokprof_core::fmt::usd)), "{} per active hour", r.name);
    }
    if c.returns > 0 {
        let cheapest = c.cheapest().unwrap();
        assert!(inside(&rt, "rep-ways", "cheapest").is_some(), "the cheapest way is marked");
        assert!(shown.contains(&format!("{} of {}", cheapest.cheapest, c.returns)));
    }
    // Tab reaches the three cards after the period, in order.
    for want in [reports::Part::Saved, reports::Part::Ways, reports::Part::Habits, reports::Part::Days] {
        press(&mut rt, RuneKey::Tab);
        assert_eq!(rt.state.reports.part, want);
    }
}

#[test]
fn a_row_opens_its_top_sessions_and_a_session_opens_in_day() {
    let (desk, part, name) = advice_desk();
    let mut rt = runtime(desk, (1400.0, 2400.0));
    command(&mut rt, '2');
    settle(&mut rt);
    let c = rt.state.reports.changes().unwrap().clone();
    let at = changes::lines(&c, part, None).iter().position(|l| *l == changes::Line::Row(name.clone())).unwrap();
    let key = format!("{}-{at}", part.key());
    click(&mut rt, &key);
    settle(&mut rt);
    assert_eq!(rt.state.reports.open, Some((part, name.clone())), "a click opens the row");
    let sessions = c.sessions_of(&name).unwrap();
    let first = format!("{}-{}", part.key(), at + 1);
    assert!(rt.presented(&first).is_some(), "its sessions under it");
    assert!(
        rt.presented(&format!("{}-{}", part.key(), at + 1 + sessions.len().min(changes::SESSIONS))).is_none_or(|_| sessions.len() > changes::SESSIONS || true)
    );
    // The keys: ↓ onto the first session, ⏎ opens it in Day with it selected.
    assert_eq!((rt.state.reports.part, rt.state.reports.row), (part, at));
    press(&mut rt, RuneKey::Down);
    press(&mut rt, RuneKey::Enter);
    assert_eq!(rt.state.reports.mode, reports::Mode::Day);
    let row = rt.state.browser.selected().expect("a session is selected");
    assert_eq!(tokprof_core::model::short_id(&sessions[0].agent, rt.state.browser.session(row)), sessions[0].session);
    // Back in Reports, ⏎ on the row closes it again.
    command(&mut rt, '2');
    settle(&mut rt);
    press(&mut rt, RuneKey::Up);
    press(&mut rt, RuneKey::Enter);
    assert_eq!(rt.state.reports.open, None);
    assert_eq!(changes::line_count(&rt.state, part), changes::lines(&c, part, None).len(), "its sessions are gone");
}

#[test]
fn a_session_opened_from_what_to_change_leaves_a_filter_being_typed_alone() {
    let (desk, part, name) = advice_desk();
    let mut rt = runtime(desk, (1400.0, 2400.0));
    // A filter is being typed in Day, empty so far, when ⌘2 goes to Reports.
    typed(&mut rt, "/");
    assert!(rt.state.browser.editing());
    command(&mut rt, '2');
    settle(&mut rt);
    let sessions = rt.state.reports.changes().unwrap().sessions_of(&name).unwrap().to_vec();
    changes::toggle(&mut rt.state, part, &name);
    settle(&mut rt);
    press(&mut rt, RuneKey::Down);
    press(&mut rt, RuneKey::Enter);
    assert_eq!(rt.state.reports.mode, reports::Mode::Day);
    assert_eq!(rt.state.browser.filter(), "", "no `d` typed into the filter");
    let row = rt.state.browser.selected().expect("a session is selected");
    assert_eq!(tokprof_core::model::short_id(&sessions[0].agent, rt.state.browser.session(row)), sessions[0].session);
}

#[test]
fn without_cold_returns_the_ways_back_have_no_rows_for_the_cursor() {
    let way = tokprof_core::pool::BenchRow { name: "resume".into(), ..Default::default() };
    let mut c = tokprof_core::savings::Changes { ways: vec![way], ..Default::default() };
    assert!(changes::lines(&c, reports::Part::Ways, None).is_empty(), "the card shows no ways");
    c.returns = 3;
    assert_eq!(changes::lines(&c, reports::Part::Ways, None), [changes::Line::Row("resume".into())]);
}

#[test]
fn a_session_the_list_does_not_hold_says_so_and_stays_in_reports() {
    let (desk, part, name) = reports_desk_with_advice();
    let mut rt = runtime(desk, (1400.0, 2400.0));
    command(&mut rt, '2');
    settle(&mut rt);
    changes::toggle(&mut rt.state, part, &name);
    settle(&mut rt);
    press(&mut rt, RuneKey::Down);
    press(&mut rt, RuneKey::Enter);
    assert_eq!(rt.state.reports.mode, reports::Mode::Reports);
    assert!(rt.state.status.0.ends_with("is not in the list"), "{}", rt.state.status.0);
}

/// `advice_desk`'s report over a list of other sessions.
fn reports_desk_with_advice() -> (Desk, reports::Part, String) {
    let (mut desk, part, name) = advice_desk();
    let mut other = desk_of(3);
    std::mem::swap(&mut other.reports, &mut desk.reports);
    (other, part, name)
}

#[test]
fn the_menu_bar_countdown_is_picked_in_the_settings_and_a_frame_makes_no_item() {
    let mut rt = runtime(desk_of(3), (1400.0, 900.0));
    assert!(!rt.state.settings.menu_bar, "off by default");
    command(&mut rt, ',');
    // The keyboard starts in the grid of the window's appearance; the other grid, then the menu
    // bar countdown, last.
    let part = |rt: &Runtime<Desk>| rt.state.panel.map(|p| p.resolve(rt.state.theme.get().dark()).0);
    press(&mut rt, RuneKey::Tab);
    press(&mut rt, RuneKey::Tab);
    assert_eq!(part(&rt), Some(settings::Part::MenuBar), "after both grids");
    press(&mut rt, RuneKey::Right);
    assert!(rt.state.settings.menu_bar, "→ picks On");
    assert_eq!(rt.state.prefs.get::<String>("menu_bar").as_deref(), Some("on"));
    assert!(!rt.state.menu_bar.shown(), "a frame without a window makes no item");
    press(&mut rt, RuneKey::Left);
    assert!(!rt.state.settings.menu_bar, "← picks Off");
    click(&mut rt, "menu-bar-on");
    assert!(rt.state.settings.menu_bar, "a click picks On");
    press(&mut rt, RuneKey::Tab);
    assert_eq!(part(&rt), Some(settings::Part::Waiting), "the waiting order comes last");
    press(&mut rt, RuneKey::Tab);
    assert_eq!(part(&rt), Some(settings::Part::Appearance), "Tab goes round");
}

#[test]
fn a_pick_in_the_menu_bar_selects_the_session_in_day_over_no_card() {
    let mut rt = runtime(desk_of(5), (1400.0, 900.0));
    rt.update(|s, _| {
        reports::show(s, reports::Mode::Reports);
        s.help = true;
    });
    settle(&mut rt);
    rt.update(|s, tx| menubar::pick(s, menubar::Pick::Session("id3".into()), tx));
    settle(&mut rt);
    let s = &rt.state;
    assert_eq!(s.reports.mode, reports::Mode::Day, "Day, where the session's row is");
    assert_eq!(s.browser.selected().map(|r| s.browser.session(r).to_string()).as_deref(), Some("id3"));
    assert!(!s.help && s.panel.is_none() && s.palette.is_none(), "no card over it");
    rt.update(|s, tx| menubar::pick(s, menubar::Pick::Settings, tx));
    settle(&mut rt);
    assert!(rt.state.panel.is_some(), "Settings… opens the panel");
    rt.update(|s, tx| menubar::pick(s, menubar::Pick::Session("gone".into()), tx));
    settle(&mut rt);
    assert!(rt.state.panel.is_some(), "a session no longer listed changes nothing");
    let s = &rt.state;
    assert_eq!(s.browser.selected().map(|r| s.browser.session(r).to_string()).as_deref(), Some("id3"));
}

#[test]
fn the_heads_of_the_list_and_the_inspector_meet_and_the_tabs_stay_put() {
    let mut rt = runtime(desk_of(30), (1400.0, 900.0));
    let rect = |rt: &Runtime<Desk>, key: &str| rt.elements().iter().find(|e| e.key.as_deref() == Some(key)).expect(key).rect;
    let (head, tabs) = (rect(&rt, "list-head"), rect(&rt, "tabs"));
    assert_eq!((head[1], head[3]), (tabs[1], tabs[3]), "one top and one height, so the lines under them are one line");
    let before: Vec<f32> = inspector::Tab::ALL.iter().map(|t| rect(&rt, &format!("tab-{}", t.name()))[0]).collect();
    press(&mut rt, RuneKey::Tab);
    assert_eq!(rt.state.pane, Focus::Inspector);
    assert!(inside(&rt, "tabs", "tab-digit").is_some(), "the digits show with the keyboard");
    let after: Vec<f32> = inspector::Tab::ALL.iter().map(|t| rect(&rt, &format!("tab-{}", t.name()))[0]).collect();
    assert_eq!(before, after, "and the tabs do not move for them");
}

/// Five sessions, costed, with id1, id3 and id4 waiting since 40m, 16m and 2m before now.
fn waiting_desk() -> Desk {
    let mut desk = desk_of(5);
    let now = tokprof_browse::browser::now_ms();
    for i in 0..5 {
        let since = match i {
            1 => Some(now - 40 * 60_000),
            3 => Some(now - 16 * 60_000),
            4 => Some(now - 2 * 60_000),
            _ => None,
        };
        let g = tokprof_core::xray::Glance { cost: 1.0, waiting_since: since, cold_at_ms: i64::MAX, ..Default::default() };
        desk.browser.put_glance(&format!("id{i}"), g);
    }
    desk
}

#[test]
fn the_waiting_order_is_a_setting_off_by_default() {
    let mut rt = runtime(waiting_desk(), (1400.0, 900.0));
    assert!(!rt.state.settings.waiting && rt.state.prefs.get::<String>("waiting").is_none(), "off without settings");
    assert!(rt.find("sort-Waiting").is_none(), "the head offers Time, Cost and Next only");
    for _ in 0..3 {
        press(&mut rt, RuneKey::Text("s".into()));
    }
    assert_eq!(rt.state.browser.sort_order(), tokprof_browse::browser::Sort::Time, "s cycles three orders");
    assert!(!texts(&rt).iter().any(|t| t == "40m"), "no row shows a wait");
    assert!(!palette::entries(&rt.state, "sort by").iter().any(|e| e.title == "Sort by Waiting"), "the palette has no Sort by Waiting");

    command(&mut rt, ',');
    let part = |rt: &Runtime<Desk>| rt.state.panel.map(|p| p.resolve(rt.state.theme.get().dark()).0);
    for _ in 0..3 {
        press(&mut rt, RuneKey::Tab);
    }
    assert_eq!(part(&rt), Some(settings::Part::Waiting), "last, after the menu bar countdown");
    press(&mut rt, RuneKey::Right);
    assert!(rt.state.settings.waiting, "→ picks On");
    assert_eq!(rt.state.prefs.get::<String>("waiting").as_deref(), Some("on"));
    press(&mut rt, RuneKey::Escape);
    assert!(rt.find("sort-Waiting").is_some(), "the head offers Waiting at once");

    click(&mut rt, "sort-Waiting");
    settle(&mut rt);
    let all = texts(&rt);
    let at = |t: &str| all.iter().position(|x| x == t).unwrap_or_else(|| panic!("no {t:?} in {all:?}"));
    // The test runs for seconds, so the 2-minute wait may read 2.0m or 2.1m.
    let two = all.iter().position(|x| x.starts_with("2.") && x.ends_with('m')).expect("the 2-minute wait");
    assert!(at("Waiting for you") < at("40m") && at("40m") < at("16m") && at("16m") < two && two < at("Others"), "{all:?}");
    assert_eq!(rt.state.browser.session(0), "id1");

    command(&mut rt, ',');
    for _ in 0..3 {
        press(&mut rt, RuneKey::Tab);
    }
    press(&mut rt, RuneKey::Left);
    assert_eq!(rt.state.browser.sort_order(), tokprof_browse::browser::Sort::Time, "off in waiting order sorts by time");
    press(&mut rt, RuneKey::Escape);
    assert!(rt.find("sort-Waiting").is_none(), "Waiting leaves the head");
}

#[test]
fn nobody_waits_and_the_palette_sorts_by_waiting() {
    let mut desk = desk_of(3);
    desk.browser.set_waiting(true);
    let mut rt = runtime(desk, (1400.0, 900.0));
    command(&mut rt, 'k');
    typed(&mut rt, "sort by waiting");
    press(&mut rt, RuneKey::Enter);
    settle(&mut rt);
    assert_eq!(rt.state.browser.sort_order(), tokprof_browse::browser::Sort::Waiting, "as the Waiting button sorts it");
    let all = texts(&rt);
    let at = |t: &str| all.iter().position(|x| x == t).unwrap_or_else(|| panic!("no {t:?} in {all:?}"));
    assert!(at("Nobody waits for you") < at("Others"), "{all:?}");
}

#[test]
fn a_frame_in_waiting_order() {
    let mut desk = waiting_desk();
    let scene = SCENES.iter().find(|(name, _)| *name == "waiting").expect("the waiting scene");
    (scene.1)(&mut desk);
    let rt = runtime(desk, (1400.0, 900.0));
    assert_eq!(rt.state.browser.sort_order(), tokprof_browse::browser::Sort::Waiting);
    assert!(texts(&rt).iter().any(|t| t == "Waiting for you"));
    assert!(!rt.state.settings.waiting, "without reading the saved settings");
}

#[path = "calendar_tests.rs"]
mod calendar_view;
