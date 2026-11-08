//! The Calendar view in the window: the switch, the grid's clicks, the cursor, the metric and
//! the tooltip, on a calendar of a few made-up days.

use tokprof_core::calendar::{Calendar, Filters, Metric};
use tokprof_core::daystore::{Entry, Minute};

use super::*;

const DAY_MS: i64 = 86_400_000;

/// A desk whose calendar holds requests and messages on the 20 days before `now`, two a day.
fn calendar_desk() -> (Desk, Calendar) {
    let now = 1_793_620_800_000; // 2026-11-02 12:00 UTC, a Monday
    let at = |d: i64, h: i64| now - d * DAY_MS + h * 3_600_000;
    let mut entry = Entry { key: "/a".into(), agent: "claude".into(), session: "s".into(), main: "s".into(), ..Entry::default() };
    for d in 0..20 {
        for h in [-2, 0] {
            entry.actions.push((at(d, h), true));
            entry.minutes.push(Minute { minute: at(d, h) / 60_000, usd: (d + 1) as f64, requests: 1, commits: u32::from(d == 3), ..Minute::default() });
        }
    }
    entry.actions.sort_unstable();
    entry.minutes.sort_by_key(|m| m.minute);
    let cal = Calendar::of(&[entry], &Filters::default(), now, &rhythm::local_offset);
    let mut desk = desk_of(3);
    desk.calendar.data.put(cal.clone(), 0);
    (desk, cal)
}

#[test]
fn command_3_and_the_switch_show_the_calendar_and_esc_goes_back() {
    let (desk, _) = calendar_desk();
    let mut rt = runtime(desk, (1400.0, 1400.0));
    command(&mut rt, '3');
    settle(&mut rt);
    assert_eq!(rt.state.reports.mode, reports::Mode::Calendar);
    assert!(rt.presented(calendar::PANE).is_some() && rt.presented("cal-grid").is_some());
    for gone in ["lanes", view::LIST, "inspector", "day-name", "today", "zoom", "filter"] {
        assert!(rt.presented(gone).is_none(), "{gone} is not shown in Calendar");
    }
    typed(&mut rt, "q/");
    assert_eq!(rt.shell_request, None, "q does not quit from Calendar");
    press(&mut rt, RuneKey::Escape);
    assert_eq!(rt.state.reports.mode, reports::Mode::Day);
    click(&mut rt, "mode-calendar");
    settle(&mut rt);
    assert_eq!(rt.state.reports.mode, reports::Mode::Calendar);
    command(&mut rt, '2');
    assert_eq!(rt.state.reports.mode, reports::Mode::Reports, "⌘2 from Calendar");
    command(&mut rt, '3');
    command(&mut rt, '1');
    assert_eq!(rt.state.reports.mode, reports::Mode::Day);
}

#[test]
fn a_click_on_a_day_and_enter_open_it_in_day() {
    let (desk, cal) = calendar_desk();
    let mut rt = runtime(desk, (1400.0, 1400.0));
    command(&mut rt, '3');
    settle(&mut rt);
    let date = cal.last - 3;
    click(&mut rt, &format!("cal-day-{date}"));
    settle(&mut rt);
    assert_eq!(rt.state.reports.mode, reports::Mode::Day);
    assert_eq!(rhythm::date_of(rt.state.browser.day_noon(), &rhythm::local_offset), date);
    // By keys: a week back from the last day, and ⏎.
    command(&mut rt, '3');
    settle(&mut rt);
    press(&mut rt, RuneKey::Left);
    assert_eq!(rt.state.calendar.cursor, Some(cal.last - 7));
    press(&mut rt, RuneKey::Enter);
    assert_eq!(rt.state.reports.mode, reports::Mode::Day);
    assert_eq!(rhythm::date_of(rt.state.browser.day_noon(), &rhythm::local_offset), cal.last - 7);
}

#[test]
fn the_cursor_stays_among_the_days_shown_and_shows_its_tooltip() {
    let (desk, cal) = calendar_desk();
    let mut rt = runtime(desk, (1400.0, 1400.0));
    command(&mut rt, '3');
    settle(&mut rt);
    assert!(rt.presented("cal-tip").is_none(), "no tooltip before a key moves the cursor");
    press(&mut rt, RuneKey::Down);
    assert_eq!(rt.state.calendar.cursor, Some(cal.last), "it never passes the last day");
    press(&mut rt, RuneKey::Up);
    assert_eq!(rt.state.calendar.cursor, Some(cal.last - 1));
    press(&mut rt, RuneKey::Home);
    assert_eq!(rt.state.calendar.cursor, Some(cal.shown_from));
    press(&mut rt, RuneKey::Left);
    assert_eq!(rt.state.calendar.cursor, Some(cal.shown_from), "nor the first");
    press(&mut rt, RuneKey::End);
    assert_eq!(rt.state.calendar.cursor, Some(cal.last));
    assert!(rt.presented("cal-tip").is_some());
    let tip = calendar::tooltip_lines(cal.day(cal.last).unwrap());
    assert_eq!(text_in(&rt, "cal-tip", "tip-0"), tip[0]);
    assert!(texts(&rt).contains(&tip[1]), "{tip:?}");
}

#[test]
fn m_cycles_the_metric_and_the_pills_pick_it() {
    let (desk, _) = calendar_desk();
    let mut rt = runtime(desk, (1400.0, 1400.0));
    command(&mut rt, '3');
    settle(&mut rt);
    assert_eq!(rt.state.calendar.metric, Metric::Spend);
    typed(&mut rt, "mm");
    assert_eq!(rt.state.calendar.metric, Metric::Keyboard);
    typed(&mut rt, "M");
    assert_eq!(rt.state.calendar.metric, Metric::Agent);
    click(&mut rt, "metric-messages");
    settle(&mut rt);
    assert_eq!(rt.state.calendar.metric, Metric::Messages);
    assert_eq!(text_in(&rt, "cal-total", "value"), "40", "two messages on each of 20 days");
}

#[test]
fn the_palette_offers_the_calendar_and_its_metrics() {
    let (desk, _) = calendar_desk();
    let mut rt = runtime(desk, (1400.0, 1400.0));
    let titles: Vec<String> = palette::entries(&rt.state, "calendar").into_iter().map(|e| e.title).collect();
    assert!(titles.contains(&"Calendar".to_string()) && titles.contains(&"Calendar: messages".to_string()), "{titles:?}");
    command(&mut rt, 'k');
    typed(&mut rt, "calendar: messages");
    press(&mut rt, RuneKey::Enter);
    assert_eq!((rt.state.reports.mode, rt.state.calendar.metric), (reports::Mode::Calendar, Metric::Messages));
    assert!(keys::GROUPS.iter().any(|(name, _)| *name == "Calendar"), "the key help has its group");
    settle(&mut rt);
}

#[test]
fn the_first_look_reads_the_logs_into_the_store() {
    let mut desk = desk_of(3);
    desk.calendar = calendar::State::new(tokprof_browse::calendar::Calendars::detached());
    let mut rt = runtime(desk, (1400.0, 900.0));
    command(&mut rt, '3');
    settle(&mut rt);
    assert_eq!(text_of(&rt, "cal-progress"), "reading the logs into the store…");
}
