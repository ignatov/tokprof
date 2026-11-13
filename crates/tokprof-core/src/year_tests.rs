use super::*;
use crate::calendar::Streak;
use crate::daystore::Minute;
use crate::period::Before;

const HOUR_MS: i64 = 60 * MINUTE_MS;

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

fn entry(id: &str, project: &str) -> Entry {
    Entry { key: format!("/claude/{id}"), agent: "claude".into(), session: id.into(), main: id.into(), project: project.into(), ..Entry::default() }
}

fn spend(ts: i64, usd: f64) -> Minute {
    Minute { minute: ts / MINUTE_MS, usd, requests: 1, ..Minute::default() }
}

/// 12 Nov 2026 at 14:20, the day the proposal's numbers were measured.
fn nov12() -> i64 {
    at(date(2026, 11, 12), 14, 20)
}

fn year(entries: &[Entry], y: i32, now: i64) -> Year {
    Year::of(entries, &Filters::default(), y, now, &off)
}

/// A request at 10:00 on each date of `from..=to`, with a tenth of it avoidable.
fn daily(id: &str, project: &str, from: i64, to: i64, usd: f64) -> Entry {
    let mut e = entry(id, project);
    e.minutes = (from..=to).map(|d| Minute { avoidable_usd: usd / 10.0, ..spend(at(d, 10, 0), usd) }).collect();
    e
}

#[test]
fn a_year_is_a_period() {
    let y = Period::Year(2026);
    assert_eq!((y.name(), y.short_name(), y.key()), ("2026".to_string(), "2026".to_string(), "2026".to_string()));
    assert_eq!((y.previous(), y.next()), (Period::Year(2025), Period::Year(2027)));
    assert_eq!((y.days(), Period::Year(2028).days()), (365, 366));
    assert_eq!(y.months().len(), 12);
    assert_eq!(y.current(nov12(), &off), y);
    assert_eq!(Period::Month(Ym { year: 2026, month: 3 }).current(nov12(), &off), Period::Month(Ym { year: 2026, month: 11 }));
    assert_eq!(Period::year_of(date(2027, 1, 1)), Period::Year(2027));
    // From the week of 1 January to the week of 31 December: 53 columns, 54 in a leap year that
    // starts on a Sunday.
    assert_eq!((y.mondays().len(), Period::Year(2040).mondays().len()), (53, 54));
}

#[test]
fn months_add_up_to_the_year() {
    let a = daily("a", "~/src/heron", date(2026, 8, 6), date(2026, 11, 12), 10.0);
    let b = daily("b", "~/src/zebrafish", date(2026, 10, 1), date(2026, 10, 31), 3.0);
    let entries = [a, b];
    let y = year(&entries, 2026, nov12());
    let months: f64 = y.sums.months.iter().map(|m| m.total.usd).sum();
    assert!((months - y.sums.key.usd).abs() < 1e-9 && (y.sums.key.usd - (99.0 * 10.0 + 31.0 * 3.0)).abs() < 1e-9);
    // Each bar is the month of `tokprof month`.
    for m in &y.sums.months {
        let alone = Month::of(&entries, &Filters::default(), m.ym.unwrap(), nov12(), &off);
        assert_eq!((m.total.usd, m.total.requests, m.sessions), (alone.total.usd, alone.total.requests, alone.sessions));
    }
    let rows = y.months();
    let kinds: Vec<MonthKind> = rows.iter().map(|r| r.kind).collect();
    assert_eq!(&kinds[..7], &[MonthKind::NoLogs; 7]);
    assert_eq!(&kinds[7..], &[MonthKind::Ended, MonthKind::Ended, MonthKind::Ended, MonthKind::SoFar, MonthKind::NotBegun]);
    assert_eq!(rows[7].from, Some(date(2026, 8, 6)), "August counts from 6 August");
    assert!(rows[6].month.is_none() && rows[11].month.is_none() && rows[9].month.is_some());
}

#[test]
fn a_night_across_the_new_year_counts_in_the_new_year() {
    let mut e = entry("a", "~/src/heron");
    e.minutes = vec![spend(at(date(2026, 12, 31), 23, 0), 1.0), spend(at(date(2027, 1, 1), 1, 0), 2.0)];
    let now = at(date(2027, 1, 10), 12, 0);
    let (old, new) = (year(std::slice::from_ref(&e), 2026, now), year(&[e], 2027, now));
    assert_eq!((old.sums.key.usd, new.sums.key.usd), (1.0, 2.0));
    assert_eq!(new.sums.months[0].total.usd, 2.0, "as month-review counts a night across the first of a month");
}

#[test]
fn a_year_in_progress_compares_with_the_same_stretch() {
    // 1 Mar 2028 10:00: 60 days and 10 hours from 1 Jan, so 1 Jan 2027 00:00 to 2 Mar 2027 10:00.
    let now = at(date(2028, 3, 1), 10, 0);
    assert_eq!(Period::Year(2028).before(now, &off), (at(date(2027, 1, 1), 0, 0), at(date(2027, 3, 2), 10, 0)));
    let e = daily("a", "~/src/heron", date(2026, 12, 1), date(2028, 3, 1), 1.0);
    let y = year(&[e], 2028, now);
    let Before::Against { window, partial, numbers, .. } = &y.sums.before else { panic!("{:?}", y.sums.before) };
    assert!(*partial && *window == (at(date(2027, 1, 1), 0, 0), at(date(2027, 3, 2), 10, 0)));
    assert_eq!(numbers.usd, 60.0, "1 Jan to 1 Mar 2027: the request of 2 Mar at 10:00 is at the end, outside");
    assert!(y.sums.pace_days.is_none());
}

#[test]
fn no_year_before_gives_the_pace_and_a_note() {
    let e = daily("a", "~/src/heron", date(2026, 8, 6), date(2026, 11, 12), 10.0);
    let y = year(&[e], 2026, nov12());
    assert_eq!(y.sums.before, Before::NoLogs(Period::Year(2026).before(nov12(), &off)));
    assert_eq!(y.sums.before.note(Period::Year(2025), &off), "no logs in 2025");
    assert_eq!(y.sums.pace_days, Some(99));
    assert_eq!(y.sums.pace_usd(y.sums.key.usd).as_deref(), Some("$10 a day"));
}

#[test]
fn an_incomplete_year_before_names_the_agent() {
    let codex = Entry { agent: "codex".into(), key: "/codex/c".into(), ..daily("c", "~/src/heron", date(2025, 6, 1), date(2027, 2, 1), 1.0) };
    let claude = daily("a", "~/src/heron", date(2026, 9, 29), date(2027, 2, 1), 5.0);
    let y = year(&[codex, claude], 2027, at(date(2027, 2, 1), 12, 0));
    assert!(matches!(y.sums.before, Before::Incomplete { .. }));
    assert_eq!(y.sums.before.note(Period::Year(2026), &off), "no comparison with 2026: claude's logs start 29 Sep");
}

#[test]
fn a_project_that_started_later_keeps_the_days_before_as_days_with_logs() {
    let heron = daily("a", "~/src/heron", date(2026, 8, 6), date(2026, 11, 12), 10.0);
    let zebrafish = daily("b", "~/src/zebrafish", date(2026, 11, 1), date(2026, 11, 12), 1.0);
    let only = Filters { project: Some("zebrafish".into()), ..Filters::default() };
    let y = Year::of(&[heron, zebrafish], &only, 2026, nov12(), &off);
    assert_eq!(y.sums.first_data, Some(date(2026, 8, 6)), "the logs begin with the agent's history, whatever the project");
    assert!(y.has_logs(date(2026, 8, 10)) && !y.has_logs(date(2026, 8, 5)));
    assert_eq!(y.days_without_logs(), date(2026, 8, 6) - date(2026, 1, 1));
    // The active days and the pace count over the days with logs only, not the year's 316 so far.
    assert_eq!((y.active_days, y.ready.logs_days, y.sums.pace_days), (12, 99, Some(99)));
    assert_eq!(y.sums.key.usd, 12.0);
    assert_eq!(y.months()[8].kind, MonthKind::Ended, "September has logs and nothing of the project in it");
}

#[test]
fn records_of_the_year() {
    let mut a = daily("a", "~/src/heron", date(2026, 9, 26), date(2026, 11, 12), 10.0);
    a.minutes.push(Minute { commits: 175, ..spend(at(date(2026, 11, 11), 11, 0), 1.0) });
    a.minutes.push(spend(at(date(2026, 10, 25), 12, 0), 1_211.0));
    a.minutes.push(spend(at(date(2026, 8, 6), 12, 0), 5.0));
    let y = year(&[a], 2026, nov12());
    let r = &y.sums.records;
    assert_eq!(r.biggest_day, Some((date(2026, 10, 25), 1_221.0)));
    assert_eq!(r.most_commits, Some((date(2026, 11, 11), 175)));
    assert_eq!(r.streak_text().as_deref(), Some("48 days, since Sat 26 Sep, still going"));
    // The week of Mon 19 Oct holds the biggest day.
    let w = y.biggest_week().unwrap();
    assert_eq!((w.monday, w.amounts.usd), (date(2026, 10, 19), 70.0 + 1_211.0));
}

#[test]
fn the_week_of_28_december_splits_between_two_years() {
    let mut e = entry("a", "~/src/heron");
    e.minutes = vec![
        spend(at(date(2026, 6, 1), 10, 0), 1.0),
        spend(at(date(2026, 12, 29), 10, 0), 1_100.0),
        spend(at(date(2027, 1, 2), 10, 0), 900.0),
        spend(at(date(2027, 1, 20), 10, 0), 50.0),
    ];
    let now = at(date(2027, 2, 1), 10, 0);
    let (old, new) = (year(std::slice::from_ref(&e), 2026, now), year(&[e], 2027, now));
    assert_eq!(old.sums.records.biggest_week, Some((date(2026, 12, 28), 1_100.0)));
    assert_eq!(new.sums.records.biggest_week, Some((date(2026, 12, 28), 900.0)));
    let w = old.biggest_week().unwrap();
    assert_eq!((w.first, w.last), (date(2026, 12, 28), date(2026, 12, 31)), "cut at the year's end");
    assert_eq!(new.biggest_week().map(|w| (w.first, w.last)), Some((date(2027, 1, 1), date(2027, 1, 3))));
}

#[test]
fn most_sessions_at_once_counts_kept_main_sessions_by_the_minute() {
    let ten = at(date(2026, 11, 1), 10, 0) / MINUTE_MS;
    let (hour, bit) = (ten / 60, 1u64 << (ten % 60));
    let worked =
        |id: &str, main: &str| Entry { main: main.into(), work: vec![(hour, bit)], ..daily(id, "~/src/heron", date(2026, 11, 1), date(2026, 11, 1), 1.0) };
    // One session from 10:00:05 to 10:00:20, another from 10:00:40 to 10:00:50: both in 10:00.
    let a = worked("a", "a");
    let b = worked("b", "b");
    // A resumed log of the first session, a subagent and a session the rhythm leaves out.
    let resumed = Entry { key: "/claude/a2".into(), ..worked("a2", "a") };
    let sub = Entry { subagent: true, ..worked("s", "s") };
    let out = Entry { left_out: true, ..worked("o", "o") };
    // A later minute with two as well: the earliest minute wins.
    let later = |id: &str| Entry { work: vec![(hour + 2, 1)], ..worked(id, id) };
    let y = year(&[a, b, resumed, sub, out, later("c"), later("d")], 2026, nov12());
    assert_eq!(y.sums.records.most_at_once, Some((2, ten * MINUTE_MS)));
}

#[test]
fn the_rhythm_of_the_weekdays() {
    let mut e = entry("a", "~/src/heron");
    // Wed 4 Nov, Sat 7 Nov, Sun 8 Nov, Wed 11 Nov.
    e.minutes = vec![
        spend(at(date(2026, 11, 4), 10, 0), 3.0),
        spend(at(date(2026, 11, 7), 10, 0), 1.0),
        spend(at(date(2026, 11, 8), 10, 0), 2.0),
        spend(at(date(2026, 11, 11), 10, 0), 2.0),
    ];
    let y = year(&[e], 2026, nov12());
    assert_eq!(y.busiest_weekday, Some((2, 5.0)), "Wednesdays, $5");
    assert_eq!(y.weekend_share, Some(3.0 / 8.0));
    // Requests without a price: no busiest weekday and no weekend share.
    let mut free = entry("b", "~/src/heron");
    free.minutes = vec![Minute { usd: 0.0, unpriced: 1, ..spend(at(date(2026, 11, 4), 10, 0), 0.0) }];
    let y = year(&[free], 2026, nov12());
    assert!(y.any() && y.busiest_weekday.is_none() && y.weekend_share.is_none());
}

#[test]
fn when_the_card_is_ready() {
    // Logs from 10 Oct: 34 days on 12 Nov, and the 90th day would be 7 Jan, so 1 Jan comes first.
    let r = Readiness::of(2026, Some(date(2026, 10, 10)), date(2026, 11, 12), false);
    assert!(!r.ready);
    assert_eq!((r.logs_days, r.ready_on), (34, Some(date(2027, 1, 1))));
    assert_eq!(format!("ready on {}: {}", r.ready_on_text().unwrap(), r.logs_text(2026)), "ready on Fri 1 Jan 2027: 2026 has 34 days of logs, from 10 Oct");
    // Logs from 6 Aug: 99 days, ready since its 90th day, 3 Nov.
    let r = Readiness::of(2026, Some(date(2026, 8, 6)), date(2026, 11, 12), false);
    assert!(r.ready);
    assert_eq!((r.logs_days, r.ready_on), (99, Some(date(2026, 11, 3))));
    // A year that is over is ready with 12 days of logs.
    let r = Readiness::of(2026, Some(date(2026, 12, 20)), date(2027, 1, 5), true);
    assert!(r.ready && r.logs_days == 12);
    assert_eq!(r.since_text().as_deref(), Some("logs since 20 Dec"));
    // A year with no logs is never ready, over or not.
    for (year, first, complete) in [(2026, None, false), (2025, Some(date(2026, 8, 6)), true)] {
        assert_eq!(Readiness::of(year, first, date(2026, 11, 12), complete), Readiness::default());
    }
    // Logs from before the year: every day of it has logs.
    let r = Readiness::of(2027, Some(date(2026, 8, 6)), date(2027, 4, 1), false);
    assert_eq!((r.ready, r.logs_days, r.logs_from, r.logs_text(2027)), (true, 91, None, "2027 has 91 days of logs".to_string()));
}

#[test]
fn a_night_after_midnight_counts_to_the_day_before() {
    // The user typed until 02:30 on 13 Nov, in the night of 12 Nov: the calendar's day is still
    // the 12th, so 2026 has 99 days of logs at 02:50, as `tokprof calendar` counts them.
    let mut e = daily("a", "~/src/heron", date(2026, 8, 6), date(2026, 11, 12), 10.0);
    let night: Vec<i64> = (0..30).map(|i| at(date(2026, 11, 12), 21, 0) + i * 11 * MINUTE_MS).collect();
    e.actions = night.iter().map(|&t| (t, true)).collect();
    let now = at(date(2026, 11, 13), 2, 50);
    let y = year(&[e], 2026, now);
    assert_eq!(y.today, date(2026, 11, 12));
    assert_eq!((y.ready.logs_days, y.sums.pace_days), (99, Some(99)));
}

#[test]
fn a_year_takes_marathon_from_100_days() {
    // A 48-day run, a tenth avoidable: no rule of the six holds for the year.
    let e = daily("a", "~/src/heron", date(2026, 9, 26), date(2026, 11, 12), 10.0);
    let y = year(std::slice::from_ref(&e), 2026, nov12());
    assert_eq!(y.sums.span.longest.map(|s| s.days()), Some(48));
    assert_eq!((y.title.clone(), y.sums.span.title.clone()), (None, None));
    // While October alone, with its 31 days in a row, is a month's Marathon.
    let oct = Month::of(&[e], &Filters::default(), Ym { year: 2026, month: 10 }, nov12(), &off);
    assert_eq!(oct.title.map(|t| t.name), Some("Marathon"));
    // A 104-day run is.
    let e = daily("a", "~/src/heron", date(2027, 1, 1), date(2027, 4, 14), 10.0);
    let y = year(&[e], 2027, at(date(2027, 6, 1), 9, 0));
    assert_eq!(y.title, Some(Title { name: "Marathon", rule: "104 days in a row with the agents (title from 100)".into() }));
    assert_eq!(y.sums.span.title, y.title);
    assert!(title_over(&Month { longest: Some(Streak { from: 0, to: 98 }), ..Month::default() }, MARATHON_DAYS).is_none(), "99 days");
}

#[test]
fn the_default_year() {
    let e = daily("a", "~/src/heron", date(2026, 8, 6), date(2026, 12, 30), 1.0);
    let f = Filters::default();
    // On 1 Jan 2027 at 09:00, before any request of 2027: the year just ended.
    assert_eq!(default_year(std::slice::from_ref(&e), &f, at(date(2027, 1, 1), 9, 0), &off), 2026);
    // The first year of use prints itself.
    assert_eq!(default_year(std::slice::from_ref(&e), &f, nov12(), &off), 2026);
    // Once 2027 holds a request.
    let mut later = e.clone();
    later.minutes.push(spend(at(date(2027, 1, 1), 9, 30), 1.0));
    assert_eq!(default_year(&[later], &f, at(date(2027, 1, 1), 10, 0), &off), 2027);
    // A message counts too, and with nothing at all, this year.
    let mut typed = entry("b", "~/src/heron");
    typed.actions = vec![(at(date(2027, 1, 1), 9, 0), true)];
    assert_eq!(default_year(&[e, typed], &f, at(date(2027, 1, 1), 9, 30), &off), 2027);
    assert_eq!(default_year(&[], &f, at(date(2027, 1, 1), 9, 0), &off), 2027);
}

#[test]
fn the_store_cards_of_a_year_count_agents_models_and_causes_by_month() {
    let mut a = daily("a", "~/src/heron", date(2026, 10, 1), date(2026, 10, 10), 10.0);
    a.models = vec![
        (at(date(2026, 10, 2), 10, 0) / HOUR_MS, "claude-opus-5-5".into(), 60.0),
        (at(date(2026, 10, 3), 10, 0) / HOUR_MS, "claude-fable-5-2".into(), 40.0),
    ];
    a.misses = vec![
        (at(date(2026, 10, 2), 10, 0) / HOUR_MS, "idle > TTL".into(), 5.0),
        (at(date(2026, 11, 2), 10, 0) / HOUR_MS, "idle > TTL".into(), 2.0),
        (at(date(2025, 12, 31), 10, 0) / HOUR_MS, "idle > TTL".into(), 50.0),
    ];
    let mut b = entry("b", "~/src/zebrafish");
    b.agent = "codex".into();
    b.minutes = vec![spend(at(date(2026, 9, 5), 9, 0), 30.0)];
    b.misses = vec![(at(date(2026, 9, 5), 9, 0) / HOUR_MS, "model switch".into(), 1.0)];
    // A subagent's misses count in its main session.
    let mut sub = entry("a-sub", "~/src/heron");
    sub.main = "a".into();
    sub.subagent = true;
    sub.misses = vec![(at(date(2026, 10, 4), 10, 0) / HOUR_MS, "idle > TTL".into(), 3.0)];
    let y = year(&[a, b, sub], 2026, nov12());
    let c = y.sums.cards.as_ref().expect("a year has its store cards");
    assert_eq!(c.agents, [("claude".to_string(), 100.0), ("codex".to_string(), 30.0)]);
    assert_eq!(c.models[0], ("claude".to_string(), "claude-opus-5-5".to_string(), 60.0));
    let idle = &c.causes[0];
    assert_eq!((idle.name.as_str(), idle.usd), ("idle > TTL", 10.0), "the night of 31 Dec 2025 counts in 2025");
    assert_eq!(idle.months, [(Ym { year: 2026, month: 10 }, 8.0), (Ym { year: 2026, month: 11 }, 2.0)]);
    assert_eq!(idle.sessions, [("claude".to_string(), "a".to_string(), 10.0)]);
    assert_eq!(y.sums.key.causes.get("idle > TTL"), Some(&10.0), "the same dollars as the key numbers");
    assert!(Year::of(&[], &Filters::default(), 2026, nov12(), &off).sums.cards.is_some_and(|c| c.causes.is_empty()));
    let q = PeriodSums::of(&[], &Filters::default(), Period::Quarter(crate::period::Yq { year: 2026, q: 4 }), nov12(), &off);
    assert!(q.cards.is_none(), "only a year has them");
}

#[test]
fn the_biggest_week_counts_its_window_as_week_does() {
    // Sunday night 11 Oct runs into Monday 12 Oct: the calendar's day keeps it on Sunday, and
    // the week's window from Monday 00:00 counts it in the week of 12 Oct, as Week's totals do.
    let mut e = entry("a", "~/src/heron");
    e.actions = vec![
        (at(date(2026, 10, 11), 10, 0), true),
        (at(date(2026, 10, 11), 22, 0), true),
        (at(date(2026, 10, 12), 1, 30), true),
        (at(date(2026, 10, 12), 11, 0), true),
    ];
    e.work = Vec::new();
    e.minutes = vec![
        spend(at(date(2026, 10, 11), 23, 0), 10.0),
        spend(at(date(2026, 10, 12), 1, 0), 100.0),
        spend(at(date(2026, 10, 14), 12, 0), 50.0),
        spend(at(date(2026, 10, 6), 12, 0), 120.0),
    ];
    let y = year(&[e], 2026, nov12());
    let w = y.biggest_week().unwrap();
    assert_eq!((w.monday, w.amounts.usd), (date(2026, 10, 12), 150.0));
    assert_eq!(y.sums.records.biggest_week, Some((date(2026, 10, 12), 150.0)));
}
