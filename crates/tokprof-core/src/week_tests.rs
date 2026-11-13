use super::*;
use crate::daystore::{Minute, work_bits};

const CET: i64 = HOUR_MS;

fn off(_: i64) -> i64 {
    CET
}

/// Mon 2 Nov 2026 00:00 in a zone one hour east of UTC, and its date.
const MIDNIGHT: i64 = 1_793_574_000_000;
const NOV2: i64 = 20_759;

fn at(day: i64, h: i64, m: i64) -> i64 {
    MIDNIGHT + day * DAY_MS + h * HOUR_MS + m * MINUTE_MS
}

fn entry(id: &str) -> Entry {
    Entry { key: format!("/{id}"), agent: "claude".into(), session: id.into(), main: id.into(), project: "~/src/app".into(), ..Entry::default() }
}

fn spend(ts: i64, usd: f64) -> Minute {
    Minute { minute: ts / MINUTE_MS, usd, requests: 1, ..Minute::default() }
}

fn messages(times: &[i64]) -> Vec<(i64, bool)> {
    times.iter().map(|&t| (t, true)).collect()
}

fn week(entries: &[Entry], monday: i64, now: i64) -> Week {
    Week::of(entries, &Filters::default(), monday, now, &off)
}

#[test]
fn a_week_runs_from_monday_midnight_and_compares_with_the_same_point_of_last_week() {
    // Thursday 12 Nov 2026, 14:20.
    let now = at(10, 14, 20);
    let monday = monday_of(rhythm::date_of(now, &off));
    assert_eq!(monday, NOV2 + 7);
    let w = Window::of(monday, now, &off);
    assert_eq!((w.from_ms, w.to_ms, w.end_ms), (at(7, 0, 0), now, at(14, 0, 0)));
    assert!(!w.complete());
    assert_eq!(w.before, (MIDNIGHT, at(3, 14, 20)), "Mon 2 Nov 00:00 to Thu 5 Nov 14:20");
    assert_eq!(w.last_week, (MIDNIGHT, at(7, 0, 0)));
    // A full week compares with the whole week before.
    let full = Window::of(NOV2, now, &off);
    assert!(full.complete());
    assert_eq!((full.to_ms, full.before), (at(7, 0, 0), (at(-7, 0, 0), MIDNIGHT)));

    let mut a = entry("a");
    a.minutes = vec![spend(at(-14, 10, 0), 1.0), spend(at(1, 10, 0), 4.0), spend(at(4, 10, 0), 9.0), spend(at(8, 10, 0), 6.0)];
    let w = week(&[a], monday, now);
    assert_eq!(w.before.note(&off), "against 2–5 Nov, to 14:20");
    assert_eq!((w.totals.usd, w.before.sums().unwrap().usd), (6.0, 4.0), "Friday 6 Nov is after the same point");
    assert_eq!(w.last_week.as_ref().map(|s| s.usd), Some(13.0), "the last full week, beside it");
    assert_eq!(w.pace_days, None);
}

#[test]
fn a_week_across_a_change_of_the_clock_keeps_local_midnights() {
    // Summer time until 2026-10-25 01:00 UTC, then winter time.
    const SWITCH: i64 = MIDNIGHT - 190 * HOUR_MS;
    let dst = |ts: i64| if ts < SWITCH { 2 * HOUR_MS } else { HOUR_MS };
    let oct19 = NOV2 - 14;
    let w = Window::of(oct19, at(20, 0, 0), &dst);
    assert_eq!(w.end_ms - w.from_ms, 169 * HOUR_MS, "a week with an hour more");
    assert_eq!(local(w.from_ms, &dst, "%a %H:%M"), "Mon 00:00");
    assert_eq!(local(w.end_ms, &dst, "%a %-d %b %H:%M"), "Mon 26 Oct 00:00");
    // Thursday 29 Oct, 14:20 winter time, compares to Thursday 22 Oct, 14:20 summer time.
    let now = MIDNIGHT - 4 * DAY_MS + 14 * HOUR_MS + 20 * MINUTE_MS;
    let w = Window::of(NOV2 - 7, now, &dst);
    assert_eq!(local(w.before.1, &dst, "%a %-d %b %H:%M"), "Thu 22 Oct 14:20");
    assert_eq!(w.before.0, Window::of(oct19, now, &dst).from_ms);
}

#[test]
fn a_week_across_a_year_and_the_names_of_weeks() {
    let dec31 = NOV2 + 59;
    assert_eq!(weekday(dec31), 3, "a Thursday");
    let monday = monday_of(dec31);
    assert_eq!(monday_of(dec31 + 2), monday, "Saturday 2 Jan 2027 is in the same week");
    assert_eq!(name(monday), "28 Dec 2026 – 3 Jan 2027");
    let w = Window::of(monday, at(80, 0, 0), &off);
    assert_eq!(w.end_ms - w.from_ms, 7 * DAY_MS);
    assert_eq!(local(w.end_ms, &off, "%Y-%m-%d %H:%M"), "2027-01-04 00:00");
    assert_eq!(name(NOV2 + 7), "9–15 Nov 2026");
    assert_eq!(name(NOV2 - 7), "26 Oct – 1 Nov 2026");
    assert_eq!(dates(NOV2, NOV2 + 3, false), "2–5 Nov");
}

/// A week of a main session, its subagent, a session the rhythm leaves out, and a resumed
/// session with two logs.
fn four_logs() -> Vec<Entry> {
    let mut a = entry("a");
    a.actions = messages(&[at(0, 10, 0), at(0, 10, 5), at(0, 10, 10), at(0, 14, 0)]);
    a.minutes = vec![Minute { read_tokens: 900, prompt_tokens: 1000, avoidable_usd: 3.0, ..spend(at(0, 10, 1), 5.0) }, spend(at(-3, 10, 0), 50.0)];
    a.work = work_bits(&[(at(0, 10, 0), at(0, 10, 30))]);
    a.misses = vec![(at(0, 10, 0) / HOUR_MS, "idle > TTL".into(), 3.0)];
    a.cold = vec![at(0, 10, 1)];
    let mut sub = entry("agent-1");
    (sub.key, sub.main, sub.subagent) = ("/a/sub".into(), "a".into(), true);
    sub.minutes = vec![spend(at(0, 10, 2), 2.0)];
    sub.work = work_bits(&[(at(0, 16, 0), at(0, 16, 30))]);
    let mut quiet = entry("b");
    (quiet.left_out, quiet.project) = (true, "(temporary)".into());
    quiet.minutes = vec![spend(at(0, 11, 0), 1.0)];
    quiet.actions = messages(&[at(0, 11, 0)]);
    quiet.work = work_bits(&[(at(0, 11, 0), at(0, 11, 10))]);
    let resumed = |key: &str, work: (i64, i64), minute: Minute, actions: &[i64]| Entry {
        key: key.into(),
        agent: "codex".into(),
        session: "c".into(),
        main: "c".into(),
        project: "~/src/web".into(),
        minutes: vec![minute],
        work: work_bits(&[work]),
        actions: messages(actions),
        ..Entry::default()
    };
    let c1 = resumed("/c1", (at(0, 12, 0), at(0, 12, 20)), spend(at(0, 12, 0), 1.0), &[]);
    let c2 = resumed("/c2", (at(0, 12, 10), at(0, 12, 30)), spend(at(0, 12, 15), 1.0), &[at(0, 12, 10)]);
    vec![a, sub, quiet, c1, c2]
}

#[test]
fn the_store_sums_over_the_window() {
    let w = week(&four_logs(), NOV2, at(9, 12, 0));
    let t = &w.totals;
    assert_eq!((t.usd, t.avoidable_usd, t.requests), (10.0, 3.0, 5), "every log spends, the subagent's and the left-out one's too");
    assert_eq!(t.cache_hit(), Some(0.9));
    assert_eq!(t.agent_minutes, 60, "a's 30 minutes and the resumed session's 12:00 to 12:30 once; no subagent or left-out work");
    assert_eq!(t.session_minutes, 60, "the resumed session's two logs count its minutes once");
    assert_eq!(t.sittings.len(), 3);
    assert_eq!(t.keyboard_ms(), 12 * MINUTE_MS, "10 minutes and two of a minute");
    assert_eq!((t.messages.len(), t.typed_sessions, t.sessions), (5, 2, 3), "the left-out session types nothing; the resumed one is one session");
    assert_eq!((t.cold, t.causes.get("idle > TTL").copied()), (1, Some(3.0)));
    assert_eq!(t.sittings_line(), "sittings  3, 12m at the keyboard, 5 messages to 2 sessions; median sitting 1.0m, median break 2.0h");
    assert_eq!((t.logs, t.gone), (5, 0));
    // By project: spend and minutes each once per project.
    let p: Vec<(&str, f64, u64)> = w.projects.iter().map(|p| (p.name.as_str(), p.usd, p.agent_minutes)).collect();
    assert_eq!(p, [("~/src/app", 7.0, 30), ("~/src/web", 2.0, 30), ("(temporary)", 1.0, 0)]);
    assert!((w.projects[0].share - 0.7).abs() < 1e-9);
}

#[test]
fn a_project_s_hours_count_each_minute_once_and_more_projects_fold() {
    let mut entries = Vec::new();
    for k in 0..10 {
        let mut e = entry(&format!("p{k}"));
        e.project = format!("~/src/p{k}");
        e.minutes = vec![spend(at(1, 10, 0), 10.0 - k as f64)];
        e.work = work_bits(&[(at(1, 10, 0), at(1, 11, 0))]);
        e.actions = messages(&[at(1, 10, 0)]);
        entries.push(e);
    }
    let w = week(&entries, NOV2, at(9, 12, 0));
    let (shown, more) = w.shown_projects();
    assert_eq!((shown.len(), more), (6, 4));
    assert_eq!(shown[0].agent_minutes, 60);
    assert!(w.projects.iter().all(|p| p.agent_minutes <= w.totals.agent_minutes), "no project has more hours than the tile");
    assert_eq!(w.totals.agent_minutes, 60);
}

/// Actions that put the day starts of 26 Oct – 9 Nov where the tests want them: Sunday 1 Nov's
/// day runs until after 04:21 on Monday, Monday's starts at 07:40, Tuesday's runs until 06:50 on
/// Wednesday, and each later day starts at 02:30.
fn nights() -> Entry {
    let mut e = entry("n");
    let mut times = vec![at(-2, 10, 0), at(-1, 10, 0), at(-1, 22, 0), at(0, 0, 30), at(0, 4, 21), at(0, 11, 0), at(0, 20, 0)];
    times.extend([at(1, 9, 0), at(1, 22, 0), at(2, 3, 40), at(2, 10, 0), at(2, 20, 0)]);
    for d in 3..=7 {
        times.extend([at(d, 9, 0), at(d, 20, 0)]);
    }
    e.actions = messages(&times);
    e
}

#[test]
fn rows_are_whole_calendar_days_and_a_line_names_what_lies_across_the_mondays() {
    let mut e = nights();
    // Mon 01:00 is Sunday 1 Nov's night; Wed 02:00 Tuesday's; Mon 9 Nov 01:00 Sunday 8 Nov's.
    e.minutes = vec![spend(at(0, 1, 0), 28.0), spend(at(2, 2, 0), 5.0), spend(at(2, 11, 0), 7.0), spend(at(7, 1, 0), 21.0)];
    e.work = work_bits(&[(at(2, 2, 0), at(2, 2, 30))]);
    let w = week(std::slice::from_ref(&e), NOV2, at(9, 12, 0));
    let usd: Vec<f64> = w.rows.iter().map(|r| r.cell.amounts.usd).collect();
    assert_eq!(usd, [0.0, 5.0, 7.0, 0.0, 0.0, 0.0, 21.0], "the night after Wednesday's midnight stays in Tuesday's row");
    assert_eq!(w.rows[2].cell.from_ms, at(2, 6, 50), "Wednesday's row starts at its day start");
    assert_eq!(w.totals.usd, 40.0, "the totals count Monday 00:00 to Monday 00:00");
    assert_eq!(
        w.gap.line(NOV2).as_deref(),
        Some("The days add up to $33: $28 of the week fell in the night of Sun 1 Nov, and $21 of Sun 8 Nov came after the week.")
    );
    // Sunday 1 Nov's row of the week before holds the night across Monday 00:00.
    let before = week(std::slice::from_ref(&e), NOV2 - 7, at(9, 12, 0));
    assert_eq!(before.rows[6].cell.amounts.usd, 28.0);
    // Tuesday's work until 02:30 on Wednesday is inside the axis: no mark.
    assert_eq!((w.axis.from_h, w.axis.to_h), (2, 31));
    assert_eq!(w.rows[1].work, vec![(at(2, 2, 0), at(2, 2, 30))]);
    assert!(w.rows.iter().all(|r| r.from_mark.is_none() && r.until_mark.is_none()));
}

#[test]
fn rows_that_add_up_have_no_line() {
    let mut e = nights();
    e.minutes = vec![spend(at(2, 11, 0), 7.0), spend(at(3, 12, 0), 3.0)];
    let w = week(&[e], NOV2, at(9, 12, 0));
    assert_eq!((w.gap.rows_usd, w.totals.usd), (10.0, 10.0));
    assert_eq!(w.gap.line(NOV2), None);
}

#[test]
fn days_before_the_first_data_read_no_logs_and_days_after_now_are_empty() {
    // The first data on Wednesday; it is Thursday 14:20.
    let mut e = entry("a");
    e.actions = messages(&[at(2, 9, 0), at(2, 9, 30), at(3, 9, 0)]);
    e.minutes = vec![spend(at(2, 9, 10), 2.0)];
    e.cold = vec![at(2, 9, 10), at(3, 9, 1)];
    let w = week(&[e], NOV2, at(3, 14, 20));
    let kinds: Vec<RowKind> = w.rows.iter().map(|r| r.kind).collect();
    use RowKind::*;
    assert_eq!(kinds, [NoLogs, NoLogs, Day, Day, Future, Future, Future]);
    assert_eq!(w.rows[3].cell.to_ms, at(3, 14, 20), "today ends now");
    assert_eq!((w.rows[2].cold, w.rows[2].ticks[0].session.as_str()), (1, "a"));
    assert_eq!((w.rows[3].cold, w.rows[3].cell.amounts.usd), (1, 0.0), "a day with data before it and none of its own spent $0.00");
    assert!(!w.rows[0].opens() && !w.rows[5].opens() && w.rows[2].opens());
    assert_eq!(w.rows[2].sittings, vec![(at(2, 9, 0), at(2, 9, 0)), (at(2, 9, 30), at(2, 9, 30))]);
}

#[test]
fn the_axis_runs_from_the_earliest_start_to_the_latest_end() {
    // 2–8 Nov: the days start between 03:41 and 05:49, and the latest ends at 06:50 the next
    // morning.
    let spans = [(NOV2, at(0, 3, 41), at(1, 5, 49)), (NOV2 + 1, at(1, 5, 49), at(2, 4, 0)), (NOV2 + 6, at(6, 4, 30), at(7, 6, 50))];
    let axis = Axis::of(spans.into_iter(), &off);
    assert_eq!((axis.from_h, axis.to_h, axis.hours()), (3, 31, 28), "03:00 to 07:00 the next morning");
    let labels = axis.labels();
    assert_eq!((labels.first().map(|l| l.1.as_str()), labels.last().map(|l| l.1.as_str())), (Some("03"), Some("06")));
    assert_eq!(axis.x(NOV2 + 1, at(1, 13, 42), &off), axis.x(NOV2 + 2, at(2, 13, 42), &off), "13:42 at one place in every row");
    assert_eq!(Axis::default().hours(), 24);
}

#[test]
fn a_day_longer_than_the_axis_is_cut_with_marks() {
    // 5–11 Oct: Saturday's day has no night pause and runs from Fri 19:29 to Sun 11:56.
    let oct5 = -28;
    let d = |day: i64, h: i64, m: i64| at(oct5 + day, h, m);
    let mut times = Vec::new();
    for day in 0..4 {
        times.extend([d(day, 9, 0), d(day, 20, 0)]);
    }
    times.extend([d(4, 9, 0), d(4, 19, 0)]);
    let mut t = d(4, 19, 58);
    while t <= d(6, 11, 26) {
        times.push(t);
        t += 10 * MINUTE_MS;
    }
    times.extend([d(6, 11, 26), d(6, 12, 26), d(6, 20, 0), d(7, 9, 0)]);
    let mut e = entry("a");
    e.actions = messages(&times);
    let w = week(&[e], NOV2 - 28, at(9, 12, 0));
    assert_eq!(w.rows[5].cell.from_ms, d(4, 19, 29));
    assert_eq!(w.rows[5].cell.to_ms, d(6, 11, 56));
    assert_eq!((w.axis.from_h, w.axis.to_h), (0, 30), "00:00 to 06:00 the next morning, the 30 hours allowed");
    let sat = &w.rows[5];
    assert_eq!((sat.from_text(&off).as_deref(), sat.until_text(&off).as_deref()), (Some("from Fri 19:29"), Some("until Sun 11:56")));
    assert!(w.rows.iter().enumerate().all(|(i, r)| i == 5 || (r.from_mark.is_none() && r.until_mark.is_none())), "only Saturday is cut");
}

#[test]
fn the_agents_band_counts_each_session_once_with_a_peak_of_six() {
    let mut entries = vec![nights()];
    for k in 0..6 {
        let mut e = entry(&format!("w{k}"));
        let mut spans = vec![(at(2, 14, 0), at(2, 14, 30))];
        if k < 3 {
            spans.push((at(3, 14, 0), at(3, 14, 10)));
        }
        e.work = work_bits(&spans);
        entries.push(e);
    }
    // A second log of w0, as after a resume: the same session, not a seventh.
    let mut again = entries[1].clone();
    again.key = "/w0-again".into();
    entries.push(again);
    let w = week(&entries, NOV2, at(9, 12, 0));
    assert_eq!((w.rows[2].peak(), w.rows[3].peak(), w.band_top), (6, 3, 6));
    assert_eq!(w.rows[2].band, vec![(at(2, 14, 0), 6), (at(2, 14, 30), 0)]);
    assert_eq!(w.rows[2].work, vec![(at(2, 14, 0), at(2, 14, 30))]);
    assert_eq!(week(&entries[..1], NOV2, at(9, 12, 0)).band_top, BAND_MIN, "at least 4");
}

#[test]
fn a_first_week_gives_the_pace_per_day() {
    // 2–8 Mar 2026, the first request on Wednesday 4 Mar, $700 in the week.
    let mar2 = NOV2 - 245;
    assert_eq!(weekday(mar2), 0);
    let mut e = entry("a");
    let day = |d: i64, h: i64| rhythm::midnight(mar2 + d, &off) + h * HOUR_MS;
    e.minutes = vec![spend(day(2, 10), 300.0), spend(day(4, 10), 400.0)];
    e.actions = messages(&[day(2, 10), day(4, 10)]);
    let w = Week::of(&[e], &Filters::default(), mar2, at(9, 12, 0), &off);
    assert_eq!(w.before.note(&off), "no logs in 23 Feb – 1 Mar");
    assert_eq!(w.before.sums(), None);
    assert_eq!(w.pace_days, Some(5), "from 4 Mar");
    assert_eq!(w.pace_usd(w.totals.usd).as_deref(), Some("$140 a day"));
}

#[test]
fn within_since_counts_from_it_and_compares_with_nothing() {
    let mut e = entry("a");
    e.minutes = vec![spend(at(-5, 10, 0), 9.0), spend(at(1, 10, 0), 4.0), spend(at(3, 10, 0), 6.0)];
    let f = Filters { since_ms: Some(at(2, 0, 0)), ..Filters::default() };
    let w = Week::of(&[e], &f, NOV2, at(9, 12, 0), &off);
    assert_eq!((w.from_ms, w.totals.usd), (at(2, 0, 0), 6.0), "the week counts from 4 November");
    assert_eq!(w.before.note(&off), "within --since 2026-11-04");
    assert_eq!(w.pace_days, Some(5));
}

#[test]
fn an_agent_whose_logs_start_in_the_week_before_leaves_no_comparison() {
    // 28 Sep – 4 Oct: codex all along, claude from Tue 29 Sep with most of the spend.
    let sep28 = NOV2 - 35;
    let codex =
        Entry { agent: "codex".into(), minutes: vec![spend(at(-60, 10, 0), 1.0), spend(at(-40, 10, 0), 1.0), spend(at(-34, 10, 0), 1.0)], ..entry("x") };
    let claude = Entry { minutes: vec![spend(at(-34, 11, 0), 50.0)], ..entry("c") };
    let w = Week::of(&[codex.clone(), claude], &Filters::default(), sep28, at(9, 12, 0), &off);
    assert_eq!(w.before.note(&off), "no comparison with 21–27 Sep: claude's logs start 29 Sep");
    assert_eq!(w.pace_days, Some(7));
    // A small agent is named, and not judged.
    let mut opencode = entry("o");
    opencode.agent = "opencode".into();
    opencode.minutes = vec![spend(at(-4, 10, 0), 1.0), spend(at(2, 10, 0), 3.0)];
    let mut claude = entry("c");
    claude.minutes = vec![spend(at(-30, 10, 0), 50.0), spend(at(-5, 10, 0), 40.0), spend(at(1, 10, 0), 97.0)];
    let w = week(&[claude, opencode], NOV2, at(9, 12, 0));
    assert_eq!(w.before.note(&off), "against 26 Oct – 1 Nov; opencode only from 29 Oct, 3% of spend, not judged");
    assert_eq!(w.before.sums().map(|s| s.usd), Some(41.0));
    assert_eq!(w.pace_days, None);
}

#[test]
fn the_night_line_names_the_night_with_the_most() {
    let mut e = nights();
    e.work = work_bits(&[(at(1, 23, 0), at(1, 23, 30)), (at(2, 1, 0), at(2, 1, 6)), (at(3, 23, 0), at(3, 23, 12)), (at(2, 10, 0), at(2, 11, 12))]);
    let w = week(&[e], NOV2, at(9, 12, 0));
    assert_eq!((w.totals.agent_minutes, w.totals.night_minutes), (120, 48));
    assert_eq!(w.totals.night_line(), "night     40% of agent time between 22:00 and 05:00, 0.8h; 0.6h of it on Tuesday night");
    assert_eq!(week(&[nights()], NOV2, at(9, 12, 0)).totals.night_line(), "night     no agent worked between 22:00 and 05:00");
}

#[test]
fn the_agents_line_of_a_week_names_the_peak_s_weekday() {
    let mut a = nights();
    a.work = work_bits(&[(at(1, 9, 0), at(1, 10, 0))]);
    let mut b = entry("b");
    b.work = work_bits(&[(at(1, 9, 0), at(1, 9, 30))]);
    let w = week(&[a, b], NOV2, at(9, 12, 0));
    let line = w.totals.agents_line(Some((5, at(1, 17, 42))), &off);
    assert!(line.starts_with("agents    1.5 agent-hours over "), "{line}");
    assert!(line.contains(", some agent at work 1.0h"), "{line}");
    assert!(line.ends_with("; peak 5 on Tue at 17:42"), "{line}");
    assert_eq!(Sums::default().agents_line(None, &off), "agents    no agent worked");
}

#[test]
fn the_line_for_next_week_names_the_costliest_cause() {
    let s = Sums { causes: [("idle > TTL".to_string(), 88.57), ("history edited".to_string(), 1.32)].into_iter().collect(), ..Sums::default() };
    assert_eq!(
        s.next_week(),
        ["$88.57 went to idle > TTL. Compact or wrap up a big session before a long break.", "The other avoidable causes: history edited $1.32."]
    );
    assert_eq!(Sums::default().next_week(), ["No avoidable misses this week."]);
}

#[test]
fn the_line_on_gone_logs() {
    let mut s = Sums { logs: 10, gone: 3, ..Sums::default() };
    assert_eq!(s.gone_line().as_deref(), Some("3 logs of this week are gone: the waits, answers and What to change do not count them"));
    s.gone = 10;
    assert_eq!(s.gone_line().as_deref(), Some(GONE_ALL));
    assert!(s.all_gone());
    assert_eq!(Sums { logs: 4, ..Sums::default() }.gone_line(), None);
}

#[test]
fn the_filter_text_keeps_agents_projects_and_session_ids_in_any_case() {
    let mut zebra = entry("51417f00-aaaa");
    zebra.project = "~/src/zebrafish".into();
    let mut sub = entry("agent-9");
    (sub.main, sub.subagent, sub.project) = ("51417f00-aaaa".into(), true, "~/src/zebrafish".into());
    let mut heron = entry("77aa");
    (heron.agent, heron.project) = ("codex".into(), "~/src/heron".into());
    let all = [zebra, sub, heron];
    let kept = |text: &str| {
        let f = Filters { text: Some(text.into()), ..Filters::default() };
        all.iter().filter(|e| f.keeps(e)).map(|e| e.session.as_str()).collect::<Vec<_>>()
    };
    assert_eq!(kept("ZebraF"), ["51417f00-aaaa", "agent-9"]);
    assert_eq!(kept("51417F"), ["51417f00-aaaa", "agent-9"], "a session id keeps its subagents");
    assert_eq!(kept("CODEX"), ["77aa"]);
    assert!(kept("zzz").is_empty());
}

#[test]
fn a_project_new_this_week_compares_with_nothing_spent_not_with_no_logs() {
    // zebrafish starts on Tue 3 Nov; heron ran in the week before.
    let mut heron = Entry { project: "~/src/heron".into(), ..entry("h") };
    heron.minutes = vec![spend(at(-30, 10, 0), 5.0), spend(at(-5, 10, 0), 8.0)];
    let mut zebra = Entry { project: "~/src/zebrafish".into(), ..entry("z") };
    zebra.minutes = vec![spend(at(1, 10, 0), 12.0)];
    let f = Filters { text: Some("zebrafish".into()), ..Filters::default() };
    let w = Week::of(&[heron, zebra], &f, NOV2, at(9, 12, 0), &off);
    assert_eq!(w.before.note(&off), "against 26 Oct – 1 Nov");
    assert_eq!((w.totals.usd, w.before.sums().map(|s| s.usd)), (12.0, Some(0.0)), "growth from nothing");
    assert_eq!(w.pace_days, None);
}
