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
    Desk::new(Browser::detached(Arc::new(Catalog::new(Vec::new(), PathBuf::new())), rows, Pricing::defaults(), frontend()))
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

/// The element keyed `key` inside the row of `session`.
fn in_row(rt: &Runtime<Desk>, session: &str, key: &str) -> Option<ElementInfo> {
    let all = rt.elements();
    let row = all.iter().position(|e| e.key.as_deref() == Some(session))?;
    let inside = |mut at: Option<usize>| {
        while let Some(i) = at {
            if i == row {
                return true;
            }
            at = all[i].parent;
        }
        false
    };
    all.iter().find(|e| e.key.as_deref() == Some(key) && inside(e.parent)).cloned()
}

fn cost_text(rt: &Runtime<Desk>, session: &str) -> String {
    in_row(rt, session, "usd").and_then(|e| e.text).unwrap_or_default()
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

#[test]
fn no_logs_fail_before_any_window_opens() {
    let dir = std::env::temp_dir().join(format!("tokprof-desk-empty-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let catalog = Catalog::new(vec![Box::new(tokprof_source_claude_code::ClaudeCode::at(dir.clone(), Path::new("/h")))], PathBuf::from("/h"));
    let err = desk(&Cli::parse_from(["tokprof-desk"]), catalog, true, Arc::new(|| {})).err().map(|e| e.to_string());
    assert_eq!(err.as_deref(), Some("no session logs found"));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_bad_size_fails_as_ui_dump_does() {
    let args = |a: &[&str]| a.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    assert!(check_size(&args(&["--snap", "a.png", "--size", "1400x900"])).is_ok());
    assert!(check_size(&args(&["--size=900x600"])).is_ok());
    for bad in [&["--size", "1400"][..], &["--size", "1400xabc"], &["--size=0x600"], &["--size"]] {
        let err = check_size(&args(bad)).unwrap_err().to_string();
        assert_eq!(err, "--size takes WIDTHxHEIGHT", "{bad:?}");
    }
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
    let mut rt = Runtime::new(Desk::new(browser), view::view, (1400.0, 900.0), Env::new(true, 1.0));
    rt.frame(0.0);
    assert_eq!(cost_text(&rt, &session), "…", "a frame that does not poll shows no cost");
    assert!(in_row(&rt, &session, "bar").is_none());
    rt.poll = Some(poll);
    rt.frame(1.0 / 60.0);
    let shown = cost_text(&rt, &session);
    assert_eq!(shown, rt.state.browser.cells(0, now_ms()).cost, "the next poll shows it");
    assert!(shown.starts_with('$'), "{shown}");
    assert!(in_row(&rt, &session, "bar").is_some(), "with its bar");
}

#[test]
fn the_x_ray_is_the_renderer_s_text_selectable_and_scrolls_full_size() {
    let (catalog, select) = fixture();
    let mut browser = Browser::start(catalog.clone(), Pricing::defaults(), select.clone(), false, Arc::new(|| {}), frontend()).unwrap();
    browser.settle();
    let mut rt = runtime(Desk::new(browser), (1400.0, 400.0));
    let file = tokprof_core::xray::rows(&catalog, &select, 1).remove(0).file;
    let x = tokprof_core::xray::xray(&catalog, &file, &Pricing::defaults(), now_ms()).unwrap();
    let colored = tokprof_browse::render_xray::text(&x, &tokprof_core::fmt::Style::with_color(true), &paths::profile_dir(), catalog.home());
    let pane = rt.elements().into_iter().find(|e| e.key.as_deref() == Some("pane-text")).unwrap();
    let shown = pane.text.clone().unwrap();
    assert!(colored.contains('\x1b'), "the renderer colors the X-ray");
    assert_eq!(shown, ansi::parse(&colored).text, "the window shows its text without the codes");

    // A drag across the `next` line selects it, and ⌘C copies it without color codes.
    let clipboard = rune::clipboard::memory();
    rt.use_clipboard(&clipboard);
    let line = shown.lines().position(|l| l.starts_with("next ")).unwrap();
    let y = pane.rect[1] + (line as f32 + 0.5) * view::LINE;
    rt.pointer_down(pane.rect[0] + 1.0, y);
    rt.pointer_move(pane.rect[0] + pane.rect[2] - 1.0, y);
    rt.pointer_up(pane.rect[0] + pane.rect[2] - 1.0, y);
    rt.frame(1.0 / 60.0);
    command(&mut rt, 'c');
    let copied = clipboard.text().map(|t| t.to_string()).unwrap_or_default();
    assert_eq!(copied.trim_end(), shown.lines().nth(line).unwrap().trim_end(), "⌘C copies the selected line");
    assert!(!copied.contains('\x1b'));

    press(&mut rt, RuneKey::Enter);
    assert!(rt.state.browser.zoomed(), "⏎ shows the X-ray full size");
    press(&mut rt, RuneKey::Down);
    assert_eq!(rt.state.pane_offset.get(), view::LINE, "↓ scrolls it by a line");
    press(&mut rt, RuneKey::PageDown);
    let paged = rt.state.pane_offset.get();
    assert!(paged > 10.0 * view::LINE, "PgDn by a page: {paged}");
    for _ in 0..20 {
        press(&mut rt, RuneKey::PageDown);
    }
    let end = rt.state.pane_offset.get();
    press(&mut rt, RuneKey::Text(" ".into()));
    assert_eq!(rt.state.pane_offset.get(), end, "the last page stays on the screen");
    press(&mut rt, RuneKey::Up);
    assert_eq!(rt.state.pane_offset.get(), end - view::LINE);
    assert_eq!(rt.state.browser.selected(), Some(0), "and the same session stays selected");
    press(&mut rt, RuneKey::Escape);
    assert!(!rt.state.browser.zoomed() && rt.shell_request.is_none(), "esc goes back to the list");
}

thread_local! {
    /// The day the test view draws: a day computed from logs needs logs written on that day.
    static DAY: std::cell::RefCell<Option<tokprof_browse::render_rhythm::DayParts>> = const { std::cell::RefCell::new(None) };
}

fn day_only(s: &Desk, cx: &mut rune::Cx<Desk>) -> rune::El {
    let t = rune::tok::theme(true);
    DAY.with(|d| rune::col().w_full().h_full().px(12.0).child(day::day_view(s, d.borrow().as_ref().unwrap(), cx, &t, true)))
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
    tokprof_browse::render_rhythm::DayParts { above: "tokprof · day\nday       …\n\n".into(), strip: Some((strip, lanes)), below: "\nsittings  1\n".into() }
}

#[test]
fn the_day_strip_draws_the_log_s_times_names_them_under_the_pointer_and_opens_a_lane() {
    let parts = synthetic_day();
    let lanes = parts.strip.clone().unwrap().1;
    DAY.with(|d| *d.borrow_mut() = Some(parts));
    let mut rt = Runtime::new(desk_of(10), day_only, (1400.0, 900.0), Env::new(true, 1.0));
    settle(&mut rt);
    let lane = lanes.lanes[0].clone();
    let track = rt.presented(&format!("track-{}", lane.id)).unwrap().0;
    let hours = rt.presented(day::HOURS).unwrap().0;
    assert_eq!((track[0], track[2]), (hours[0], hours[2]), "every lane's track lines up with the hours row");
    assert!(track[0] >= day::LABEL_WIDTH, "after the labels");

    // Each piece of work is a bar from its start to its end, at least a point wide.
    let (a, b, _) = *lane.marks.iter().find(|m| m.2 == rhythm::Mark::Work).expect("the session worked");
    let (x, w) = day::bar(a, b, lanes.begin, track[2]);
    let bars: Vec<[f32; 4]> = rt.elements().iter().filter(|e| e.key.as_deref() == Some("work")).map(|e| e.rect).collect();
    assert!(bars.iter().any(|r| (r[0] - (track[0] + x)).abs() < 0.5 && r[2] >= w - 0.5 && r[2] >= 1.0), "a bar at {x} wide {w}: {bars:?}");

    // The pointer over it names the time, the session and the work.
    let mid = track[0] + x + w / 2.0;
    rt.pointer_move(mid, track[1] + day::LANE / 2.0);
    settle(&mut rt);
    let line = rt.elements().into_iter().find(|e| e.key.as_deref() == Some("pointer-text")).and_then(|e| e.text).unwrap();
    assert!(line.ends_with(&format!(" · {} · agent working", lane.label.trim_end())), "{line}");
    assert_eq!(line.len() - line.trim_start_matches(|c: char| c.is_ascii_digit() || c == ':').len(), 5, "it starts with the time, HH:MM: {line}");

    // The message is a mark at its moment, and the you lane shows the sitting.
    let message = rt.elements().into_iter().find(|e| e.key.as_deref() == Some("message")).unwrap().rect;
    let x = day::x_of(lane.marks[1].0, lanes.begin, track[2]);
    assert!((message[0] + message[2] / 2.0 - (track[0] + x)).abs() < 0.5, "at 14:40: {message:?}");
    assert!(rt.presented("track-you").is_some() && rt.elements().iter().any(|e| e.key.as_deref() == Some("present")));
    assert!(rt.elements().iter().any(|e| e.key.as_deref() == Some("legend")));

    // A click on the lane selects its session in the list.
    rt.state.browser.show_day(0);
    click(&mut rt, &format!("lane-{}", lane.id));
    assert!(!rt.state.browser.showing_day() && !rt.state.browser.zoomed(), "back to the list with the X-ray");
    assert_eq!(rt.state.browser.selected(), rt.state.browser.row_of("id3"));
    assert_eq!(rt.state.browser.selected(), Some(3));
}

#[test]
fn bars_are_shares_of_the_dearest_session() {
    assert_eq!(view::bar_share(Some(30.0), Some(120.0)), Some(0.25), "a quarter as long as the dearest");
    assert_eq!(view::bar_share(Some(120.0), Some(120.0)), Some(1.0));
    assert_eq!(view::bar_share(Some(0.0), Some(0.0)), Some(0.0), "sessions that cost nothing");
    assert_eq!(view::bar_share(None, Some(120.0)), None, "a session still being costed has no bar");
    let rt = runtime(desk_of(3), (1400.0, 900.0));
    assert_eq!(cost_text(&rt, "id0"), "…");
    assert!(in_row(&rt, "id0", "bar").is_none(), "and no bar");
}

#[test]
fn the_list_keeps_its_width_and_the_x_ray_takes_the_rest() {
    for width in [1400.0, 900.0, 720.0] {
        let rt = runtime(desk_of(3), (width, 600.0));
        let list = rt.presented("list-pane").unwrap().0;
        let right = rt.presented("right").unwrap().0;
        assert_eq!(list[2], view::LIST_WIDTH, "at {width}");
        assert_eq!(right[2], width - view::LIST_WIDTH - 1.0, "at {width}: the rest, less the line between them");
    }
}

#[test]
fn the_titles_and_the_footer_read_as_in_the_terminal() {
    let rt = runtime(desk_of(30), (1400.0, 900.0));
    let text = |key: &str| rt.elements().into_iter().find(|e| e.key.as_deref() == Some(key)).and_then(|e| e.text);
    let title = rt.elements().into_iter().filter(|e| e.parent.is_some()).find_map(|e| e.text.filter(|t| t.contains(" sessions · "))).unwrap();
    assert!(title.starts_with("30 sessions · $0.00 · by time · costing 30…"), "{title}");
    assert_eq!(text("keys").as_deref(), Some(rt.state.browser.footer_keys().trim_start()));
    assert_eq!(text("pane-text").as_deref(), Some("computing the X-ray of id0 (0.0 MB)…"));
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
    assert!(rt.state.browser.list_title(now_ms()).contains("· by cost "), "s sorts by cost");
    press(&mut rt, RuneKey::Text("r".into()));
    assert!(rt.state.browser.list_title(now_ms()).contains("· by cost "), "r sorts again in the same order");

    typed(&mut rt, "/q");
    assert_eq!(rt.state.browser.filter(), "q", "q is typed into a filter");
    assert_eq!(rt.shell_request, None);
    press(&mut rt, RuneKey::Escape);
    assert!(!rt.state.browser.editing() && rt.state.browser.filter().is_empty(), "esc while typing clears the filter");
    press(&mut rt, RuneKey::Text("q".into()));
    assert_eq!(rt.shell_request, Some(Shell::Quit), "q quits");
}

#[test]
fn command_q_and_w_quit_and_esc_in_the_list_quits() {
    let mut rt = runtime(desk_of(3), (1400.0, 900.0));
    command(&mut rt, 'w');
    assert_eq!(rt.shell_request, Some(Shell::Close));
    let mut rt = runtime(desk_of(3), (1400.0, 900.0));
    command(&mut rt, 'q');
    assert_eq!(rt.shell_request, Some(Shell::Quit));
    let mut rt = runtime(desk_of(3), (1400.0, 900.0));
    press(&mut rt, RuneKey::Enter);
    press(&mut rt, RuneKey::Escape);
    assert_eq!(rt.shell_request, None, "esc in the full-size X-ray goes back to the list");
    press(&mut rt, RuneKey::Escape);
    assert_eq!(rt.shell_request, Some(Shell::Quit), "esc in the list with no filter quits");
}

#[test]
fn a_click_selects_a_session_a_double_click_zooms_and_the_wheel_scrolls() {
    let mut rt = runtime(desk_of(60), (1400.0, 900.0));
    click(&mut rt, "id4");
    assert_eq!(rt.state.browser.selected(), Some(4), "a click selects its row");
    assert_eq!(rt.state.browser.report_title(), " claude id4 · ~/src/air ");
    let (x, y) = rt.find("id4").unwrap();
    rt.frame(0.6);
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    rt.frame(0.05);
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    settle(&mut rt);
    assert!(rt.state.browser.zoomed(), "a double click shows its X-ray full size");
    assert!(rt.presented("list-pane").is_none(), "the list is not drawn");

    let mut rt = runtime(desk_of(60), (1400.0, 900.0));
    let (x, y) = rt.find("id3").unwrap();
    rt.wheel(x, y, -400.0);
    settle(&mut rt);
    assert!(rt.state.list_offset.get() > 300.0, "the wheel scrolls the list");
    assert_eq!(rt.state.browser.selected(), Some(0), "and leaves the selection where it was");
}

#[test]
fn rows_on_the_screen_stay_when_a_session_joins_above_them() {
    let mut rt = runtime(desk_of(80), (1400.0, 900.0));
    let (x, y) = rt.find("id3").unwrap();
    rt.wheel(x, y, -480.0);
    settle(&mut rt);
    let on_screen = ["id25", "id30", "id40"];
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
fn the_list_comes_back_where_it_was_after_the_full_size_x_ray() {
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
    assert_eq!(key(RuneKey::Tab), None, "Tab moves focus");
    assert_eq!(key(RuneKey::Command('c')), None, "⌘C copies the selected text");
    let ctrl_c = KeyEvent { key: RuneKey::Text("c".into()), mods: rune::Mods { ctrl: true, ..rune::Mods::default() } };
    assert_eq!(key_of(&ctrl_c), Some(Key::Interrupt));
}
