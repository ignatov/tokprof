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
    let cal = Calendar::of(std::slice::from_ref(&entry), &Filters::default(), now, &rhythm::local_offset);
    let mut desk = desk_of(3);
    let store = tokprof_core::daystore::Store { entries: vec![entry], ..Default::default() };
    desk.calendar.data.put_with(cal.clone(), store, 0);
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
    assert_eq!(text_in(&rt, "cal-total", "value"), "40 messages", "two messages on each of 20 days");
    // The days and streaks follow the metric: commits on one day only.
    click(&mut rt, "metric-commits");
    settle(&mut rt);
    assert_eq!(text_in(&rt, "cal-active", "value"), format!("1 of {}", rt.state.calendar.calendar().unwrap().shown().len()));
    assert_eq!(text_in(&rt, "cal-total", "value"), "2 commits");
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

#[test]
fn r_opens_the_month_in_review_of_the_cursor_and_its_keys_stay_with_it() {
    let (desk, cal) = calendar_desk();
    let mut rt = runtime(desk, (1400.0, 900.0));
    command(&mut rt, '3');
    settle(&mut rt);
    assert!(rt.presented("month-review").is_some(), "the button by the metric pills");
    typed(&mut rt, "r");
    let ym = tokprof_core::month::Ym::of_date(cal.last);
    assert_eq!(rt.state.month.card.as_ref().and_then(|c| c.month.ym), Some(ym));
    assert!(rt.presented("month-card").is_some() && rt.presented("month-save").is_some());
    let shown = texts(&rt);
    assert!(shown.contains(&ym.name()), "{shown:?}");
    for private in ["~/", "/a", "s1"] {
        assert!(!shown.iter().any(|t| t.contains(private) && t.contains("month")), "{private}");
    }
    // Keys stay with the card: q does nothing, ← shows the month before, esc closes it.
    typed(&mut rt, "qm");
    assert_eq!(rt.shell_request, None);
    assert_eq!(rt.state.calendar.metric, Metric::Spend, "m does not reach the calendar");
    press(&mut rt, RuneKey::Left);
    assert_eq!(rt.state.month.card.as_ref().and_then(|c| c.month.ym), Some(ym.previous()));
    press(&mut rt, RuneKey::Right);
    press(&mut rt, RuneKey::Right);
    assert_eq!(rt.state.month.card.as_ref().and_then(|c| c.month.ym), Some(ym), "never past this month");
    press(&mut rt, RuneKey::Escape);
    assert!(rt.state.month.card.is_none() && rt.state.reports.mode == reports::Mode::Calendar);
    // A click on the button opens it too, and the palette offers it.
    click(&mut rt, "month-review");
    settle(&mut rt);
    assert!(rt.state.month.card.is_some());
    press(&mut rt, RuneKey::Escape);
    let titles: Vec<String> = palette::entries(&rt.state, "month").into_iter().map(|e| e.title).collect();
    assert!(titles.contains(&"Month in review".to_string()), "{titles:?}");
}

#[test]
fn a_store_that_was_reset_says_so_in_the_window() {
    let (mut desk, _) = calendar_desk();
    let warning = "warning: the day store could not be parsed; it was kept as ~/Library/Caches/tokprof/days-2.json.broken-20261109T010000Z and started anew";
    desk.calendar.data.get_mut().unwrap().warnings.push(warning.into());
    let mut rt = runtime(desk, (1400.0, 900.0));
    command(&mut rt, '3');
    settle(&mut rt);
    assert_eq!(text_of(&rt, "cal-warning"), warning.trim_start_matches("warning: "));
}

#[test]
fn a_day_without_activity_is_an_empty_cell_in_a_dark_theme() {
    let dark = crate::pal::pal(crate::themes::find("merino-dark").unwrap());
    let light = crate::pal::pal(crate::themes::find("merino-light").unwrap());
    assert!(dark.dark && !light.dark);
    // Level none has no fill in dark, so the first level always differs from nothing.
    let none = calendar::level_color(0, dark.accent, &dark);
    let one = calendar::level_color(1, dark.accent, &dark);
    assert_ne!(none, one);
    for id in ["merino-dark", "neon-arcade", "cathode-amber"] {
        let p = crate::pal::pal(crate::themes::find(id).unwrap());
        assert!(p.dark, "{id} is dark, so its empty days are outlined");
    }
}
