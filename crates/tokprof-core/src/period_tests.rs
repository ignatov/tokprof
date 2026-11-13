use super::*;
use crate::daystore::Minute;

const HOUR_MS: i64 = 60 * MINUTE_MS;
const DAY_MS: i64 = 24 * HOUR_MS;

/// A zone one hour east of UTC, with no change of the clocks.
fn off(_: i64) -> i64 {
    HOUR_MS
}

fn date(year: i32, month: u32, day: i64) -> i64 {
    Ym { year, month }.first_date() + day - 1
}

/// Local time `h:m` on a date.
fn at(date: i64, h: i64, m: i64) -> i64 {
    rhythm::midnight(date, &off) + h * HOUR_MS + m * MINUTE_MS
}

fn entry(agent: &str, id: &str, project: &str) -> Entry {
    Entry { key: format!("/{agent}/{id}"), agent: agent.into(), session: id.into(), main: id.into(), project: project.into(), ..Entry::default() }
}

fn spend(ts: i64, usd: f64) -> Minute {
    Minute { minute: ts / MINUTE_MS, usd, requests: 1, ..Minute::default() }
}

fn sums(entries: &[Entry], period: Period, now: i64) -> PeriodSums {
    PeriodSums::of(entries, &Filters::default(), period, now, &off)
}

const OCT: Ym = Ym { year: 2026, month: 10 };
const NOV: Ym = Ym { year: 2026, month: 11 };
const Q4: Yq = Yq { year: 2026, q: 4 };

#[test]
fn periods_parse_name_and_step() {
    assert_eq!(Period::parse("2026-Q4"), Some(Period::Quarter(Q4)));
    assert_eq!(Period::parse("2026-q4"), Some(Period::Quarter(Q4)));
    assert_eq!(Period::parse("2026-11"), Some(Period::Month(NOV)));
    for bad in ["2026-Q5", "2026-q4x", "2026-Q0", "26-Q4", "2026Q4", "2026-Q", "+026-Q4"] {
        assert_eq!(Yq::parse(bad), None, "{bad}");
        assert_eq!(Period::parse(bad), None, "{bad}");
    }
    assert_eq!((Period::Month(NOV).name(), Period::Quarter(Q4).name()), ("November 2026".to_string(), "Q4 2026".to_string()));
    assert_eq!((Period::Month(OCT).previous().short_name(), Period::Quarter(Q4).previous().short_name()), ("September".to_string(), "Q3 2026".to_string()));
    assert_eq!(Period::Quarter(Q4).next(), Period::Quarter(Yq { year: 2027, q: 1 }));
    assert_eq!(Period::Quarter(Yq { year: 2027, q: 1 }).previous(), Period::Quarter(Q4));
    assert_eq!(Period::Month(NOV).quarter(), Period::Quarter(Q4));
    assert_eq!(Period::Quarter(Q4).months(), vec![OCT, NOV, Ym { year: 2026, month: 12 }]);
    assert_eq!((Period::Quarter(Q4).days(), Period::Month(NOV).days()), (92, 30));
    assert_eq!(Period::Quarter(Q4).key(), "2026-Q4");
    // From Quarter, Month shows the quarter's last month that has begun.
    let now = at(date(2026, 11, 12), 15, 0);
    assert_eq!(Period::Quarter(Q4).last_begun_month(now, &off), NOV);
    assert_eq!(Period::Quarter(Q4).current(now, &off), Period::Quarter(Q4));
    assert_eq!(Period::Month(OCT).current(now, &off), Period::Month(NOV));
}

#[test]
fn the_same_point_of_the_period_before() {
    // 12 Nov 15:00: 1 Oct 00:00 to 12 Oct 15:00.
    let now = at(date(2026, 11, 12), 15, 0);
    assert_eq!(Period::Month(NOV).before(now, &off), (at(date(2026, 10, 1), 0, 0), at(date(2026, 10, 12), 15, 0)));
    // 31 Mar 10:00: all of February, never past its end.
    let now = at(date(2027, 3, 31), 10, 0);
    assert_eq!(Period::Month(Ym { year: 2027, month: 3 }).before(now, &off), (at(date(2027, 2, 1), 0, 0), at(date(2027, 3, 1), 0, 0)));
    // Q4 after 42 d 15 h: from 1 Jul for 42 d 15 h, to 12 Aug 15:00.
    let now = at(date(2026, 10, 1), 0, 0) + 42 * DAY_MS + 15 * HOUR_MS;
    let before = Period::Quarter(Q4).before(now, &off);
    assert_eq!(before, (at(date(2026, 7, 1), 0, 0), at(date(2026, 7, 1), 0, 0) + 42 * DAY_MS + 15 * HOUR_MS));
    assert_eq!(before.1, at(date(2026, 8, 12), 15, 0));
    // A complete period compares with the whole period before.
    assert_eq!(Period::Month(OCT).before(now + 60 * DAY_MS, &off), Period::Month(Ym { year: 2026, month: 9 }).bounds(&off));
    // `tokprof month` uses the same rule.
    let mut e = entry("claude", "a", "~/src/app");
    e.minutes = vec![spend(at(date(2026, 8, 1), 10, 0), 1.0), spend(at(date(2026, 10, 3), 10, 0), 2.0), spend(at(date(2026, 11, 3), 10, 0), 3.0)];
    let now = at(date(2026, 11, 12), 15, 0);
    let m = Month::of(&[e], &Filters::default(), NOV, now, &off);
    assert_eq!(m.previous.and_then(|p| p.to_ms), Some(Period::Month(NOV).before(now, &off).1));
}

/// Every date of Q4 2026 to `last` with $1 at noon, and the store's first request on 1 Jul.
fn daily(last: i64) -> Entry {
    let mut e = entry("claude", "a", "~/src/app");
    e.minutes = std::iter::once(spend(at(date(2026, 7, 1), 12, 0), 1.0)).chain((date(2026, 10, 1)..=last).map(|d| spend(at(d, 12, 0), 1.0))).collect();
    e
}

#[test]
fn a_quarter_has_a_week_per_monday_that_touches_it() {
    let now = at(date(2027, 1, 20), 12, 0);
    let q4 = sums(&[daily(date(2026, 12, 31))], Period::Quarter(Q4), now);
    assert_eq!(q4.weeks.len(), 14);
    let (first, last) = (&q4.weeks[0], &q4.weeks[13]);
    assert_eq!((first.monday, first.first, first.last), (date(2026, 9, 28), date(2026, 10, 1), date(2026, 10, 4)), "Thu–Sun");
    assert_eq!((last.monday, last.first, last.last), (date(2026, 12, 28), date(2026, 12, 28), date(2026, 12, 31)));
    assert!(first.cut() && last.cut() && !q4.weeks[1].cut());
    // Q2 2024 starts on Monday 1 April.
    assert_eq!(Period::Quarter(Yq { year: 2024, q: 2 }).mondays().len(), 13);
    let q2 = sums(&[], Period::Quarter(Yq { year: 2024, q: 2 }), now);
    assert_eq!(q2.weeks.len(), 13);
    assert!(q2.weeks.iter().all(|w| w.kind == WeekKind::NoLogs));
}

#[test]
fn the_edge_weeks_count_only_their_dates_inside() {
    let mut e = entry("claude", "a", "~/src/app");
    // Tue 29 Sep is in the week of 28 Sep but before the quarter; 2 Oct inside it; 1 Jan after it.
    e.minutes = vec![
        spend(at(date(2026, 9, 29), 12, 0), 5.0),
        spend(at(date(2026, 10, 2), 12, 0), 2.0),
        spend(at(date(2026, 12, 30), 12, 0), 3.0),
        spend(at(date(2027, 1, 1), 12, 0), 7.0),
    ];
    let q = sums(&[e], Period::Quarter(Q4), at(date(2027, 1, 20), 12, 0));
    assert_eq!((q.weeks[0].amounts.usd, q.weeks[13].amounts.usd), (2.0, 3.0));
    assert_eq!(q.key.usd, 5.0, "the key numbers count the quarter's window");
    assert_eq!(q.projects[0].weeks[0], 2.0, "the projects by week count the same dates");
    assert_eq!(q.records.biggest_week, Some((date(2026, 12, 28), 3.0)));
}

#[test]
fn weeks_to_come_the_current_week_and_weeks_without_requests() {
    // On Thu 12 Nov 15:00, with every date to 12 Nov and nothing in the week of 12 Oct.
    let now = at(date(2026, 11, 12), 15, 0);
    let mut e = daily(date(2026, 11, 12));
    e.minutes.retain(|m| !(date(2026, 10, 12)..=date(2026, 10, 18)).contains(&rhythm::date_of(m.minute * MINUTE_MS, &off)));
    e.minutes.iter_mut().filter(|m| rhythm::date_of(m.minute * MINUTE_MS, &off) == date(2026, 10, 5)).for_each(|m| {
        m.read_tokens = 98;
        m.prompt_tokens = 100;
    });
    let q = sums(&[e], Period::Quarter(Q4), now);
    let kind = |monday: i64| q.weeks.iter().find(|w| w.monday == monday).unwrap();
    assert_eq!(kind(date(2026, 11, 9)).kind, WeekKind::SoFar);
    assert_eq!(kind(date(2026, 11, 16)).kind, WeekKind::Future);
    assert!(!kind(date(2026, 11, 16)).opens());
    assert_eq!(kind(date(2026, 11, 2)).kind, WeekKind::Ended);
    // A week without requests has no point on the trends.
    assert_eq!(kind(date(2026, 10, 12)).trend(), None);
    let t = kind(date(2026, 10, 5)).trend().unwrap();
    assert_eq!((t.read_share, t.avoidable_share, t.average_context), (Some(0.98), Some(0.0), 100.0 / 7.0));
    assert_eq!(q.records.best_cache_week, Some((date(2026, 10, 5), 0.98)));
    // The streak since the week without requests is still going today.
    assert_eq!(q.records.streak, Some((Streak { from: date(2026, 10, 19), to: date(2026, 11, 12) }, true)));
    assert_eq!(q.records.streak_text().as_deref(), Some("25 days, since Mon 19 Oct, still going"));
    assert_eq!(q.months.len(), 2, "December has not begun");
}

#[test]
fn records_of_a_month() {
    let mut e = entry("claude", "a", "~/src/app");
    let d = |n| date(2026, 10, n);
    e.minutes = vec![
        spend(at(d(2), 12, 0), 4.0), // Fri
        spend(at(d(3), 12, 0), 0.5), // Sat: a weekend is not a weekday
        spend(at(d(6), 12, 0), 2.0), // Tue
        spend(at(d(7), 12, 0), 2.0), // Wed: as quiet as Tuesday, later
        spend(at(d(8), 12, 0), 9.0), // Thu
        spend(at(d(9), 12, 0), 9.0), // Fri: as big as Thursday, later
    ];
    e.cold = vec![at(d(6), 13, 0), at(d(8), 13, 0), at(d(8), 14, 0), at(d(9), 13, 0), at(d(9), 14, 0)];
    let m = sums(&[e], Period::Month(OCT), at(date(2026, 12, 1), 12, 0));
    let r = &m.records;
    assert_eq!(r.biggest_day, Some((d(8), 9.0)), "a tie goes to the earliest");
    assert_eq!(r.quietest_weekday, Some((d(6), 2.0)), "weekends and days without spend are skipped");
    assert_eq!(r.most_cold, Some((d(8), 2)));
    assert_eq!(m.cold_by_date[5], 1, "6 Oct");
    assert_eq!(r.streak, Some((Streak { from: d(6), to: d(9) }, false)));
    assert_eq!(r.streak_text().as_deref(), Some("4 days, Tue 6 Oct – Fri 9 Oct"));
    assert!(m.months.is_empty() && m.shifts.is_empty(), "a month has neither");
    assert_eq!(m.span.days.len(), 31);
}

#[test]
fn the_seven_key_numbers_and_the_most_at_once() {
    let d = date(2026, 10, 14);
    let bits = (1u64 << 10) - 1;
    let hour = at(d, 10, 0) / HOUR_MS;
    let mut a = entry("claude", "a", "~/src/app");
    a.minutes = vec![Minute { avoidable_usd: 1.0, saved_usd: 30.0, commits: 2, ..spend(at(d, 10, 0), 4.0) }];
    a.work = vec![(hour, bits)];
    a.actions = vec![(at(d, 9, 58), true)];
    a.cold = vec![at(d, 10, 0)];
    a.misses = vec![(hour, "idle > TTL".into(), 1.0)];
    let mut b = entry("codex", "b", "~/src/tool");
    b.minutes = vec![spend(at(d, 10, 5), 6.0)];
    b.work = vec![(hour, bits << 5)];
    // A resumed session's second log counts once at a minute.
    let mut a2 = entry("claude", "a", "~/src/app");
    a2.key = "/claude/a-2".into();
    a2.work = vec![(hour, bits)];
    let m = sums(&[a, b, a2], Period::Month(OCT), at(date(2026, 12, 1), 12, 0));
    let k = &m.key;
    assert_eq!((k.usd, k.avoidable_usd, k.saved_usd, k.commits, k.sessions, k.cold), (10.0, 1.0, 30.0, 2, 2, 1));
    assert_eq!(k.agent_minutes, 15, "each minute some agent worked, once");
    assert!(k.keyboard_ms > 0);
    assert_eq!(k.avoidable_share(), Some(0.1));
    assert_eq!(k.causes.get("idle > TTL"), Some(&1.0));
    assert_eq!(m.records.most_at_once, Some((2, at(d, 10, 5))), "two sessions from 10:05, the first such minute");
}

#[test]
fn pace_per_day_without_a_comparison() {
    // Q4 on 12 Nov with $15,586, the store from 1 October: 43 days.
    let mut e = entry("claude", "a", "~/src/app");
    e.minutes = vec![spend(at(date(2026, 10, 1), 10, 0), 15_000.0), spend(at(date(2026, 11, 12), 10, 0), 586.0)];
    let now = at(date(2026, 11, 12), 15, 0);
    let q = sums(std::slice::from_ref(&e), Period::Quarter(Q4), now);
    assert_eq!(q.before, Before::NoLogs(Period::Quarter(Q4).before(now, &off)));
    assert_eq!(q.before.note(Period::Quarter(Q4).previous(), &off), "no logs in Q3 2026");
    assert_eq!((q.pace_days, q.pace_usd(q.key.usd).as_deref()), (Some(43), Some("$362 a day")));
    // October, complete: 31 days; saved by the cache as a multiple of the spend.
    let mut e = entry("claude", "a", "~/src/app");
    e.minutes = vec![Minute { saved_usd: 87_329.0, ..spend(at(date(2026, 10, 1), 10, 0), 11_632.0) }];
    let m = sums(&[e], Period::Month(OCT), now);
    assert_eq!((m.pace_days, m.pace_usd(m.key.usd).as_deref()), (Some(31), Some("$375 a day")));
    assert_eq!(m.saved_multiple().as_deref(), Some("7.5× the spend"));
    assert_eq!(m.before.note(Period::Month(OCT).previous(), &off), "no logs in September 2026");
    // A first month counts from the store's first request, today whole.
    let mut e = entry("claude", "a", "~/src/app");
    e.minutes = vec![spend(at(date(2026, 11, 5), 10, 0), 80.0)];
    let n = sums(&[e], Period::Month(NOV), now);
    assert_eq!(n.pace_days, Some(8), "5 to 12 November");
}

/// Codex all along from 1 Jun; claude from `claude_from`; both spend in September and after.
fn two_agents(claude_from: i64, codex_from: i64) -> Vec<Entry> {
    let mut codex = entry("codex", "x", "~/src/tool");
    codex.minutes = vec![
        spend(at(codex_from, 10, 0), 1.0),
        spend(at(date(2026, 9, 15), 10, 0), 50.0),
        spend(at(date(2026, 10, 10), 10, 0), 50.0),
        spend(at(date(2026, 11, 3), 10, 0), 50.0),
    ];
    let mut claude = entry("claude", "c", "~/src/app");
    claude.minutes = vec![spend(at(claude_from, 10, 0), 9.0), spend(at(date(2026, 10, 4), 10, 0), 30.0), spend(at(date(2026, 11, 4), 10, 0), 30.0)];
    vec![codex, claude]
}

#[test]
fn an_agent_that_starts_in_the_period_before_makes_it_incomplete() {
    let now = at(date(2026, 11, 12), 15, 0);
    let oct = sums(&two_agents(date(2026, 9, 29), date(2026, 6, 1)), Period::Month(OCT), now);
    assert!(matches!(oct.before, Before::Incomplete { .. }) && oct.pace_days == Some(31));
    assert_eq!(oct.before.note(Period::Month(OCT).previous(), &off), "no comparison with September: claude's logs start 29 Sep");
    assert!(oct.gone_causes().is_empty(), "no cause is listed as gone without a comparison");
    // Two agents start late in Q3: both are named, as month-review names them.
    let q4 = sums(&two_agents(date(2026, 9, 29), date(2026, 8, 6)), Period::Quarter(Q4), now);
    assert_eq!(q4.before.note(Period::Quarter(Q4).previous(), &off), "no comparison with Q3 2026: claude's logs start 29 Sep, codex's logs start 6 Aug");
    // November against the same point of a complete October.
    let nov = sums(&two_agents(date(2026, 9, 1), date(2026, 6, 1)), Period::Month(NOV), now);
    let Before::Against { numbers, partial, .. } = &nov.before else { panic!("{:?}", nov.before) };
    assert!(*partial && nov.pace_days.is_none());
    assert_eq!((nov.key.usd, numbers.usd), (80.0, 80.0), "1–12 Oct holds 4 and 10 Oct");
    assert_eq!(nov.before.note(Period::Month(NOV).previous(), &off), "against 1–12 Oct, to 15:00");
}

#[test]
fn a_small_agent_is_named_and_a_late_project_keeps_the_comparison() {
    let now = at(date(2026, 11, 12), 15, 0);
    let mut entries = two_agents(date(2026, 9, 1), date(2026, 6, 1));
    // OpenCode from 5 Nov with 3% of November's spend so far.
    let mut o = entry("opencode", "o", "~/src/app");
    o.minutes = vec![spend(at(date(2026, 11, 5), 10, 0), 80.0 * 0.03 / 0.97)];
    entries.push(o);
    let nov = sums(&entries, Period::Month(NOV), now);
    assert!(nov.before.numbers().is_some());
    assert_eq!(nov.before.note(Period::Month(NOV).previous(), &off), "against 1–12 Oct, to 15:00; opencode only from 5 Nov, 3% of spend, not judged");
    // A project whose first request is on 5 Nov: the agents' history in the store is complete.
    let mut new = entry("claude", "n", "~/src/new");
    new.minutes = vec![spend(at(date(2026, 11, 5), 10, 0), 20.0)];
    let mut entries = two_agents(date(2026, 9, 1), date(2026, 6, 1));
    entries.push(new);
    for filters in [Filters { project: Some("~/src/new".into()), ..Filters::default() }, Filters { text: Some("src/NEW".into()), ..Filters::default() }] {
        let p = PeriodSums::of(&entries, &filters, Period::Month(NOV), now, &off);
        let Before::Against { numbers, .. } = &p.before else { panic!("{:?}", p.before) };
        assert_eq!((p.key.usd, numbers.usd), (20.0, 0.0), "compared with nothing of it in October");
    }
    // Without any log of the agents kept in October, there are no logs.
    let filters = Filters { agents: vec!["opencode".into()], ..Filters::default() };
    let p = PeriodSums::of(&entries, &filters, Period::Month(NOV), now, &off);
    assert_eq!(p.before.note(Period::Month(OCT), &off), "no logs in October 2026");
}

#[test]
fn causes_gone_since_the_period_before_and_within_the_window_s_filters() {
    let now = at(date(2026, 11, 12), 15, 0);
    let mut entries = two_agents(date(2026, 9, 1), date(2026, 6, 1));
    let hour = |d: i64| at(d, 10, 0) / HOUR_MS;
    entries[1].misses = vec![
        (hour(date(2026, 10, 4)), "compaction call".into(), 367.0),
        (hour(date(2026, 10, 4)), "idle > TTL".into(), 10.0),
        (hour(date(2026, 11, 4)), "idle > TTL".into(), 5.0),
    ];
    let nov = sums(&entries, Period::Month(NOV), now);
    assert_eq!(nov.gone_causes(), vec![("compaction call".to_string(), 367.0)]);
    // `--since` inside October: no comparison, and the period counts from it.
    let since = at(date(2026, 10, 9), 0, 0);
    let filters = Filters { since_ms: Some(since), ..Filters::default() };
    let oct = PeriodSums::of(&entries, &filters, Period::Month(OCT), now, &off);
    assert_eq!(oct.before, Before::Within("--since 2026-10-09".into()));
    assert_eq!(oct.before.note(Period::Month(OCT).previous(), &off), "within --since 2026-10-09");
    assert_eq!((oct.from_ms, oct.key.usd, oct.pace_days), (since, 50.0, Some(23)), "9 to 31 October");
}

#[test]
fn the_filter_text_keeps_a_project_in_the_period_s_sums() {
    let mut z = entry("claude", "z", "~/src/zebrafish");
    z.minutes = vec![spend(at(date(2026, 10, 3), 10, 0), 7.0)];
    let mut other = entry("claude", "o", "~/src/app");
    other.minutes = vec![spend(at(date(2026, 10, 4), 10, 0), 5.0)];
    let filters = Filters { text: Some("ZebraF".into()), ..Filters::default() };
    let oct = PeriodSums::of(&[z, other], &filters, Period::Month(OCT), at(date(2026, 12, 1), 0, 0), &off);
    assert_eq!(oct.key.usd, 7.0);
    assert_eq!(oct.projects.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(), ["~/src/zebrafish"]);
    assert_eq!(oct.span.days[2].amounts.usd, 7.0);
}

/// A quarter's store: `base` spends `usd(week)` in each week from 1 Oct, at noon on the week's
/// first date inside Q4, and claude has data from 1 Jul.
fn weekly(project: &str, agent: &str, usd: impl Fn(usize) -> f64, weeks: usize) -> Entry {
    let mut e = entry(agent, project, project);
    let mondays = Period::Quarter(Q4).mondays();
    e.minutes = (0..weeks).filter(|&w| usd(w) > 0.0).map(|w| spend(at(mondays[w].max(date(2026, 10, 1)) + 1, 12, 0), usd(w))).collect();
    e
}

fn history() -> Entry {
    let mut e = entry("claude", "old", "~/src/old");
    e.minutes = vec![spend(at(date(2026, 7, 1), 12, 0), 1.0)];
    e
}

/// Thu 12 Nov 2026 15:00: six ended weeks, from the week of 28 Sep to the week of 2 Nov.
fn nov12() -> i64 {
    at(date(2026, 11, 12), 15, 0)
}

#[test]
fn a_project_that_started() {
    let base = weekly("base", "claude", |_| 100.0, 7);
    let swing = weekly("swing-rune", "claude", |w| if w == 5 { 12.0 } else { 0.0 }, 7);
    let q = sums(&[history(), base, swing], Period::Quarter(Q4), nov12());
    assert_eq!(q.shifts.len(), 1, "{:?}", q.shifts);
    let s = &q.shifts[0];
    assert_eq!((s.project.as_str(), s.kind, s.usd), ("swing-rune", ShiftKind::Started, 12.0));
    assert_eq!(s.detail(), "week of 2 Nov");
}

#[test]
fn logs_that_start_late_start_no_project() {
    // pi's first request in the store is in the project's first week: too late to tell.
    let base = weekly("base", "claude", |_| 100.0, 7);
    let late = weekly("late", "pi", |w| if w >= 3 { 12.0 } else { 0.0 }, 7);
    let q = sums(&[history(), base.clone(), late.clone()], Period::Quarter(Q4), nov12());
    assert!(q.shifts.iter().all(|s| s.kind != ShiftKind::Started), "{:?}", q.shifts);
    // With pi's data from 28 days before the week of 19 Oct, it started.
    let mut pi = entry("pi", "p", "~/src/pi");
    pi.minutes = vec![spend(at(date(2026, 9, 21), 12, 0), 1.0)];
    let q = sums(&[history(), base, late, pi], Period::Quarter(Q4), nov12());
    assert_eq!(q.shifts.iter().find(|s| s.project == "late").map(|s| (s.kind, s.detail())), Some((ShiftKind::Started, "week of 19 Oct".to_string())));
}

#[test]
fn a_project_that_paused() {
    // 5% in the weeks of 5, 12 and 19 Oct, none on 26 Oct and 2 Nov, back on 9 Nov; it had
    // requests in September, so it did not start.
    let base = weekly("base", "claude", |_| 95.0, 7);
    let mut pod = weekly("airpods-motion", "claude", |w| if (1..=3).contains(&w) || w == 6 { 5.0 } else { 0.0 }, 7);
    pod.minutes.push(spend(at(date(2026, 9, 10), 12, 0), 1.0));
    let q = sums(&[history(), base, pod], Period::Quarter(Q4), nov12());
    let s = q.shifts.iter().find(|s| s.project == "airpods-motion").expect("a pause");
    assert_eq!((s.kind, s.detail()), (ShiftKind::Paused, "26 Oct – 8 Nov".to_string()));
}

#[test]
fn projects_that_grew_and_fell_between_the_halves() {
    // Six ended weeks: air has 12% of the first three weeks' spend and 6% of the last three.
    let base = weekly("base", "claude", |w| if w < 3 { 88.0 } else { 94.0 }, 7);
    let mut air = weekly("air", "claude", |w| if w < 3 { 12.0 } else { 6.0 }, 7);
    air.minutes.push(spend(at(date(2026, 9, 10), 12, 0), 1.0));
    let mut base = base;
    base.minutes.push(spend(at(date(2026, 9, 10), 12, 0), 1.0));
    let q = sums(&[history(), base.clone(), air.clone()], Period::Quarter(Q4), nov12());
    let rows: Vec<(&str, ShiftKind, String)> = q.shifts.iter().map(|s| (s.project.as_str(), s.kind, s.detail())).collect();
    assert_eq!(rows, [("base", ShiftKind::Grew, "88% → 94%".to_string()), ("air", ShiftKind::Fell, "12% → 6%".to_string())], "grew before fell");
    // Three ended weeks are too few to judge halves by.
    let q = sums(&[history(), base, air], Period::Quarter(Q4), at(date(2026, 10, 21), 12, 0));
    assert!(q.shifts.is_empty(), "{:?}", q.shifts);
}

#[test]
fn an_odd_number_of_weeks_leaves_the_middle_one_out() {
    // Seven ended weeks on 17 Nov; air's spike in the middle week, 19 Oct, does not count.
    let mut base = weekly("base", "claude", |_| 90.0, 8);
    base.minutes.push(spend(at(date(2026, 9, 10), 12, 0), 1.0));
    let mut air = weekly("air", "claude", |w| if w == 3 { 500.0 } else { 10.0 }, 8);
    air.minutes.push(spend(at(date(2026, 9, 10), 12, 0), 1.0));
    let q = sums(&[history(), base, air], Period::Quarter(Q4), at(date(2026, 11, 17), 12, 0));
    assert!(q.shifts.is_empty(), "the halves are weeks 1–3 and 5–7: {:?}", q.shifts);
    assert_eq!(q.weeks.iter().filter(|w| w.kind == WeekKind::Ended).count(), 7);
}

#[test]
fn nothing_moved_and_at_most_five_rows_in_kind_order() {
    let mut base = weekly("base", "claude", |_| 100.0, 7);
    base.minutes.push(spend(at(date(2026, 9, 10), 12, 0), 1.0));
    let q = sums(&[history(), base.clone()], Period::Quarter(Q4), nov12());
    assert!(q.shifts.is_empty());
    assert_eq!(NO_SHIFTS, "No project started, paused or moved 5 points this quarter.");
    // Six new projects of different sizes and a falling base: the five started rows show, the
    // costliest first.
    let mut entries = vec![history(), base];
    for (i, usd) in [10.0, 20.0, 30.0, 40.0, 50.0, 60.0].into_iter().enumerate() {
        entries.push(weekly(&format!("new{i}"), "claude", move |w| if w >= 4 { usd } else { 0.0 }, 7));
    }
    let q = sums(&entries, Period::Quarter(Q4), nov12());
    let rows: Vec<(&str, ShiftKind)> = q.shifts.iter().map(|s| (s.project.as_str(), s.kind)).collect();
    assert_eq!(
        rows,
        [("new5", ShiftKind::Started), ("new4", ShiftKind::Started), ("new3", ShiftKind::Started), ("new2", ShiftKind::Started), ("new1", ShiftKind::Started)]
    );
}

#[test]
fn the_months_of_a_quarter_and_their_pace() {
    let now = nov12();
    let mut e = history();
    e.minutes.extend([spend(at(date(2026, 10, 10), 12, 0), 11_632.0), spend(at(date(2026, 11, 10), 12, 0), 3_954.0)]);
    let q = sums(&[e], Period::Quarter(Q4), now);
    assert_eq!(q.months.iter().map(|m| (m.ym, m.total.usd)).collect::<Vec<_>>(), [(Some(OCT), 11_632.0), (Some(NOV), 3_954.0)]);
    assert_eq!(q.months_line(now, &off).as_deref(), Some("November runs $330 a day against October's $375 (−12%)"));
    let (shown, other) = q.stacked();
    assert_eq!((shown.len(), other.iter().sum::<f64>()), (1, 0.0));
}

#[test]
fn spend_by_day_faces_the_month_before_by_weekday() {
    let dec = Ym { year: 2026, month: 12 };
    assert_eq!((weekday_shift(NOV), weekday_shift(dec)), (28, 35));
    // November against October: 1 Nov faces Sun 4 Oct, 29 and 30 Nov face nothing.
    let mut e = entry("claude", "a", "~/src/zebrafish");
    e.minutes = vec![spend(at(date(2026, 10, 4), 10, 0), 7.0), spend(at(date(2026, 10, 31), 10, 0), 9.0)];
    let s = sums(&[e.clone()], Period::Month(NOV), at(date(2026, 11, 12), 15, 0));
    assert!(s.before.numbers().is_some());
    assert_eq!(s.before_cells.len(), 28, "October from its first log, 4 Oct");
    assert_eq!(s.facing(date(2026, 11, 1)), Some(date(2026, 10, 4)));
    assert_eq!(s.facing(date(2026, 11, 2)), Some(date(2026, 10, 5)), "Mon 2 Nov faces Mon 5 Oct");
    assert_eq!(s.facing(date(2026, 11, 28)), Some(date(2026, 10, 31)));
    assert_eq!((s.facing(date(2026, 11, 29)), s.facing(date(2026, 11, 30))), (None, None));
    assert_eq!(s.before_at(date(2026, 11, 1)), Some(7.0));
    assert_eq!(s.before_at(date(2026, 11, 2)), Some(0.0), "a date with logs and no spend is a zero");
    assert_eq!(s.before_at(date(2026, 11, 29)), None);
    // December against November: 1 to 5 Dec face nothing, 6 Dec faces Sun 1 Nov.
    let s = sums(&[e], Period::Month(dec), at(date(2026, 12, 20), 15, 0));
    assert!((1..=5).all(|d| s.facing(date(2026, 12, d)).is_none()));
    assert_eq!(s.facing(date(2026, 12, 6)), Some(date(2026, 11, 1)));
    // A quarter has no such line.
    assert!(sums(&[], Period::Quarter(Q4), at(date(2026, 11, 12), 15, 0)).before_cells.is_empty());
}

#[test]
fn spend_by_day_draws_no_line_without_a_comparison() {
    // October against September, whose logs start on 29 Sep: no comparison, so no line, though
    // the dates still face their dates of September.
    let mut e = entry("claude", "a", "~/src/zebrafish");
    e.minutes = vec![spend(at(date(2026, 9, 29), 10, 0), 300.0), spend(at(date(2026, 9, 30), 10, 0), 400.0), spend(at(date(2026, 10, 5), 10, 0), 50.0)];
    let s = sums(&[e], Period::Month(OCT), at(date(2026, 11, 12), 15, 0));
    assert!(matches!(s.before, Before::Incomplete { .. }), "{:?}", s.before);
    assert!(s.before_cells.is_empty());
    assert_eq!(s.facing(date(2026, 10, 28)), Some(date(2026, 9, 30)));
    assert!((1..=31).all(|d| s.before_at(date(2026, 10, d)).is_none()));
    // No logs at all in the month before: no line either.
    let mut e = entry("claude", "a", "~/src/heron");
    e.minutes = vec![spend(at(date(2026, 11, 3), 10, 0), 5.0)];
    let s = sums(&[e], Period::Month(NOV), at(date(2026, 11, 12), 15, 0));
    assert!(matches!(s.before, Before::NoLogs(_)));
    assert!(s.before_cells.is_empty());
}

#[test]
fn spend_by_day_breaks_its_line_before_the_first_log() {
    // October is compared, but its logs start on 10 Oct: 1 to 6 Nov face 4 to 9 Oct and have no
    // point, not a zero; 7 Nov faces Sat 10 Oct.
    let mut e = entry("codex", "a", "~/src/heron");
    e.minutes = vec![spend(at(date(2026, 10, 10), 10, 0), 6.0), spend(at(date(2026, 10, 12), 10, 0), 8.0)];
    let s = sums(&[e], Period::Month(NOV), at(date(2026, 11, 12), 15, 0));
    assert!(s.before.numbers().is_some(), "{:?}", s.before);
    assert_eq!(s.before_cells.first().map(|c| c.0), Some(date(2026, 10, 10)));
    assert!((1..=6).all(|d| s.before_at(date(2026, 11, d)).is_none()));
    assert_eq!(s.facing(date(2026, 11, 3)), Some(date(2026, 10, 6)));
    assert_eq!(s.before_at(date(2026, 11, 7)), Some(6.0));
    assert_eq!(s.before_at(date(2026, 11, 8)), Some(0.0));
}
