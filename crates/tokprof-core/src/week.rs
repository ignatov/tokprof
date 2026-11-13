//! A calendar week from the day store: its window from Monday 00:00, its totals and the week
//! before them, the seven days as the calendar's cells with what their strips draw on one clock
//! axis, the projects, and the lines the window prints of them (see desk-window, Week view).
//!
//! The totals count the week's window, from local midnight on its Monday; the rows are whole
//! calendar days, so a night across one of the Mondays lies in a row of the week next to it.

use std::collections::{BTreeMap, HashMap, HashSet};

use crate::analysis::cache::Cause;
use crate::calendar::{self, Calendar, DayRule, Filters, weekday};
use crate::daystore::Entry;
use crate::fmt;
use crate::month::NIGHT_HOURS;
use crate::rhythm::{self, Break, Offset, Summary};

const MINUTE_MS: i64 = 60_000;
const HOUR_MS: i64 = 60 * MINUTE_MS;
const DAY_MS: i64 = 24 * HOUR_MS;

/// The longest the clock axis of the week's strips runs, in hours.
pub const AXIS_MAX_HOURS: i64 = 30;
/// The least full height of the agents band, in sessions at work.
pub const BAND_MIN: u32 = 4;
/// The projects By project shows before its `<n> more` row.
pub const PROJECTS_SHOWN: usize = 6;

/// The Monday of the week that holds a local date, as days since 1970-01-01.
pub fn monday_of(date: i64) -> i64 {
    date - weekday(date) as i64
}

/// A week's window and the weeks before it, in epoch ms.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Window {
    /// The local date of its Monday.
    pub monday: i64,
    /// Monday 00:00 local, and the next Monday 00:00.
    pub from_ms: i64,
    pub end_ms: i64,
    /// The end of what it counts: the next Monday 00:00, or now for this week.
    pub to_ms: i64,
    /// What it compares with: the same point of last week for a partial week, from last week's
    /// Monday 00:00 to the same weekday and time; the whole week before for a full one.
    pub before: (i64, i64),
    /// The whole week before, from its Monday 00:00 to this Monday 00:00.
    pub last_week: (i64, i64),
}

impl Window {
    /// The window of the week whose Monday is `monday`, as of `now_ms`.
    pub fn of(monday: i64, now_ms: i64, offset: Offset) -> Window {
        let from_ms = rhythm::midnight(monday, offset);
        let end_ms = rhythm::midnight(monday + 7, offset);
        let to_ms = end_ms.min(now_ms).max(from_ms);
        let last_week = (rhythm::midnight(monday - 7, offset), from_ms);
        let before = if to_ms >= end_ms {
            last_week
        } else {
            // The same weekday and the same time of day, a week before.
            let today = rhythm::date_of(to_ms, offset);
            let time = to_ms - rhythm::midnight(today, offset);
            (last_week.0, (rhythm::midnight(today - 7, offset) + time).clamp(last_week.0, from_ms))
        };
        Window { monday, from_ms, end_ms, to_ms, before, last_week }
    }

    /// The week has ended.
    pub fn complete(&self) -> bool {
        self.to_ms >= self.end_ms
    }
}

/// The name of a week: `9–15 Nov 2026`, `26 Oct – 1 Nov 2026`, `28 Dec 2026 – 3 Jan 2027`.
pub fn name(monday: i64) -> String {
    dates(monday, monday + 6, true)
}

/// Two local dates as a span: `2–5 Nov`, `26 Oct – 1 Nov`, with the year after the last date
/// when `year`; across years each date has its year.
pub fn dates(first: i64, last: i64, year: bool) -> String {
    use chrono::Datelike;
    let (a, b) = (naive(first), naive(last));
    let tail = |d: chrono::NaiveDate| if year { d.format("%-d %b %Y").to_string() } else { d.format("%-d %b").to_string() };
    if a.year() != b.year() {
        return format!("{} – {}", tail(a), tail(b));
    }
    if a.month() == b.month() {
        return format!("{}–{}", a.day(), tail(b));
    }
    format!("{} – {}", a.format("%-d %b"), tail(b))
}

fn naive(date: i64) -> chrono::NaiveDate {
    chrono::DateTime::from_timestamp(date * 86_400, 0).map(|t| t.date_naive()).unwrap_or_default()
}

/// A moment in local time, as `format` writes it.
fn local(ts: i64, offset: Offset, format: &str) -> String {
    chrono::DateTime::from_timestamp_millis(ts + offset(ts)).map(|t| t.naive_utc().format(format).to_string()).unwrap_or_default()
}

/// Dollars as a sentence says them: whole dollars from $1, cents below.
pub fn dollars(v: f64) -> String {
    if v.abs() < 1.0 {
        return fmt::usd(v);
    }
    format!("{}${}", if v < 0.0 { "-" } else { "" }, fmt::int(v.abs().round() as u64))
}

/// What the store holds of a window, for the totals, the tiles and the rhythm lines.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Sums {
    pub from_ms: i64,
    pub to_ms: i64,
    pub usd: f64,
    pub avoidable_usd: f64,
    pub saved_usd: f64,
    pub requests: u64,
    pub unpriced: u64,
    /// Prompt tokens read from the cache, and all prompt tokens.
    pub read_tokens: u64,
    pub prompt_tokens: u64,
    /// The minutes some agent worked, each once (the calendar's `agent` metric), and the minutes
    /// each session worked, summed over sessions (the `agents` line's agent-hours).
    pub agent_minutes: u64,
    pub session_minutes: u64,
    /// The user's sittings over the window, and the breaks between two sittings of one day.
    pub sittings: Vec<(i64, i64)>,
    pub breaks: Vec<(i64, i64)>,
    /// When the user typed a message.
    pub messages: Vec<i64>,
    /// Main sessions the user acted in, and main sessions with a request.
    pub typed_sessions: usize,
    pub sessions: usize,
    pub cold: usize,
    /// The avoidable dollars of each unexpected miss cause, by name, by the hours whose start is
    /// in the window.
    pub causes: BTreeMap<String, f64>,
    /// The minutes some agent worked between 22:00 and 05:00, and by the date of the evening
    /// each night starts on.
    pub night_minutes: u64,
    pub nights: BTreeMap<i64, u64>,
    /// The time the user sat, cut to the window, and the part of it in minutes when two or more
    /// sessions' agents worked.
    pub present_ms: i64,
    pub multi_ms: i64,
    /// The logs with anything in the window, and those of them that are gone.
    pub logs: usize,
    pub gone: usize,
}

impl Sums {
    /// The time at the keyboard: the sittings, each at least a minute.
    pub fn keyboard_ms(&self) -> i64 {
        self.sittings.iter().map(|&s| rhythm::sitting_ms(s)).sum()
    }

    /// Anything happened: a request or a message.
    pub fn any(&self) -> bool {
        self.requests > 0 || !self.messages.is_empty()
    }

    /// The share of prompt tokens read from the cache.
    pub fn cache_hit(&self) -> Option<f64> {
        (self.prompt_tokens > 0).then(|| self.read_tokens as f64 / self.prompt_tokens as f64)
    }

    /// The avoidable share of the spend.
    pub fn avoidable_share(&self) -> Option<f64> {
        (self.usd > 0.0).then(|| self.avoidable_usd / self.usd)
    }

    /// The `sittings` line, as the rhythm section of `tokprof pauses` prints it.
    pub fn sittings_line(&self) -> String {
        let summary = Summary {
            sittings: self.sittings.clone(),
            breaks: self.breaks.iter().map(|&(from, to)| Break { from, to, still_after: 0, worked: 0 }).collect(),
            messages: self.messages.clone(),
            sessions: self.typed_sessions,
            ..Summary::default()
        };
        summary.lines().swap_remove(0)
    }

    /// The `agents` line of the week, as the day view's: the agent-hours summed over sessions,
    /// the time at the keyboard, the minutes some agent worked, the share of the sittings when
    /// two or more worked, and the peak the week's logs give, with its weekday.
    pub fn agents_line(&self, peak: Option<(u32, i64)>, offset: Offset) -> String {
        if self.session_minutes == 0 {
            return "agents    no agent worked".to_string();
        }
        let mut out = format!(
            "agents    {:.1} agent-hours over {} at the keyboard, some agent at work {}",
            self.session_minutes as f64 / 60.0,
            rhythm::duration(self.keyboard_ms()),
            rhythm::duration(self.agent_minutes as i64 * MINUTE_MS)
        );
        if self.present_ms > 0 {
            out += &format!("; 2 or more worked {:.0}% of the time you were there", 100.0 * self.multi_ms as f64 / self.present_ms as f64);
        }
        if let Some((n, ts)) = peak {
            out += &format!("; peak {n} on {}", local(ts, offset, "%a at %H:%M"));
        }
        out
    }

    /// The `night` line: the share of agent time between 22:00 and 05:00, its hours, and the
    /// night with the most of it, named by the evening it starts on.
    pub fn night_line(&self) -> String {
        if self.night_minutes == 0 {
            return "night     no agent worked between 22:00 and 05:00".to_string();
        }
        let share = 100.0 * self.night_minutes as f64 / self.agent_minutes.max(1) as f64;
        let mut line = format!("night     {share:.0}% of agent time between 22:00 and 05:00, {:.1}h", self.night_minutes as f64 / 60.0);
        // The most, and the earliest night of equal ones.
        let best = self.nights.iter().fold(None, |b: Option<(i64, u64)>, (&d, &m)| if b.is_none_or(|b| m > b.1) { Some((d, m)) } else { b });
        if let Some((date, minutes)) = best {
            line += &format!("; {:.1}h of it on {} night", minutes as f64 / 60.0, naive(date).format("%A"));
        }
        line
    }

    /// The card for next week: the costliest miss cause tokprof has advice for, its dollars and
    /// the advice, then the other avoidable causes; or `No avoidable misses this week.`
    pub fn next_week(&self) -> Vec<String> {
        let mut causes: Vec<(&str, f64)> = self.causes.iter().filter(|(_, v)| **v >= 0.005).map(|(c, v)| (c.as_str(), *v)).collect();
        causes.sort_by(|a, b| b.1.total_cmp(&a.1).then_with(|| a.0.cmp(b.0)));
        let known = |name: &str| Cause::ALL.iter().copied().find(|c| c.name() == name && *c != Cause::Unknown);
        let top = causes.iter().position(|(c, _)| known(c).is_some());
        let mut out = Vec::new();
        if let Some(i) = top {
            let (name, v) = causes.remove(i);
            let hint = crate::advice::miss_hint(known(name).unwrap_or(Cause::Unknown));
            let mut chars = hint.chars();
            let hint = chars.next().map_or(String::new(), |c| c.to_uppercase().collect::<String>() + chars.as_str());
            out.push(format!("{} went to {name}. {hint}.", fmt::usd(v)));
        }
        if !causes.is_empty() {
            let list = causes.iter().map(|(c, v)| format!("{c} {}", fmt::usd(*v))).collect::<Vec<_>>().join(", ");
            out.push(if out.is_empty() { format!("The avoidable causes: {list}.") } else { format!("The other avoidable causes: {list}.") });
        }
        if out.is_empty() {
            out.push("No avoidable misses this week.".to_string());
        }
        out
    }

    /// The line on the logs of the week that are gone: what the week's logs give does not count
    /// them, or there is nothing to read at all.
    pub fn gone_line(&self) -> Option<String> {
        match self.gone {
            0 => None,
            n if n >= self.logs => Some(GONE_ALL.to_string()),
            n => Some(format!(
                "{} of this week {} gone: the waits, answers and What to change do not count them",
                fmt::count(n as u64, "log", "logs"),
                if n == 1 { "is" } else { "are" }
            )),
        }
    }

    /// Every log of the week is gone, and the store keeps them.
    pub fn all_gone(&self) -> bool {
        self.gone > 0 && self.gone >= self.logs
    }

    /// The sums of `[from, to)` of the entries `filters` keeps (its own window is not used), with
    /// the breaks and the nights by `rule`.
    fn of(entries: &[&Entry], (from, to): (i64, i64), rule: &DayRule, offset: Offset) -> Sums {
        let inside = |ts: i64| ts > 0 && ts >= from && ts < to;
        let mut s = Sums { from_ms: from, to_ms: to, ..Sums::default() };
        let mut sessions: HashSet<(&str, &str)> = HashSet::new();
        let mut typed: HashSet<(&str, &str)> = HashSet::new();
        let mut actions: Vec<i64> = Vec::new();
        let mut minutes: HashMap<i64, HashSet<(&str, &str)>> = HashMap::new();
        for e in entries {
            let key = (e.agent.as_str(), e.main.as_str());
            let mut touched = false;
            for m in e.minutes.iter().filter(|m| m.minute > 0 && inside(m.minute * MINUTE_MS)) {
                calendar_add(&mut s, m);
                touched = true;
                if m.requests > 0 {
                    sessions.insert(key);
                }
            }
            for (_, cause, v) in e.misses.iter().filter(|x| inside(x.0 * HOUR_MS)) {
                *s.causes.entry(cause.clone()).or_default() += v;
                touched = true;
            }
            let cold = e.cold.iter().filter(|&&t| inside(t)).count();
            s.cold += cold;
            touched |= cold > 0;
            if !e.subagent && !e.left_out {
                for &(ts, message) in e.actions.iter().filter(|a| inside(a.0)) {
                    actions.push(ts);
                    typed.insert(key);
                    touched = true;
                    if message {
                        s.messages.push(ts);
                    }
                }
                for minute in work_minutes(e).filter(|m| inside(m * MINUTE_MS)) {
                    touched = true;
                    minutes.entry(minute).or_default().insert(key);
                }
            }
            if touched {
                s.logs += 1;
                s.gone += usize::from(e.gone);
            }
        }
        s.sessions = sessions.len();
        s.typed_sessions = typed.len();
        actions.sort_unstable();
        s.messages.sort_unstable();
        s.sittings = rhythm::sittings(&actions);
        s.breaks = s.sittings.windows(2).map(|p| (p[0].1, p[1].0)).filter(|&(a, b)| rule.date_of(a, offset) == rule.date_of(b, offset)).collect();
        s.agent_minutes = minutes.len() as u64;
        // Each session's minutes once, though a resumed session has two logs.
        s.session_minutes = minutes.values().map(|k| k.len() as u64).sum();
        for &minute in minutes.keys() {
            let ts = minute * MINUTE_MS;
            let local = ts + offset(ts);
            let hour = (local.rem_euclid(DAY_MS) / HOUR_MS) as usize;
            if NIGHT_HOURS.contains(&hour) {
                s.night_minutes += 1;
                let date = local.div_euclid(DAY_MS) - i64::from(hour < 12);
                *s.nights.entry(date).or_default() += 1;
            }
        }
        for &(a, b) in &s.sittings {
            let (a, b) = (a.max(from), b.min(to));
            if b <= a {
                continue;
            }
            s.present_ms += b - a;
            for m in a.div_euclid(MINUTE_MS)..=(b - 1).div_euclid(MINUTE_MS) {
                if minutes.get(&m).is_some_and(|k| k.len() >= 2) {
                    s.multi_ms += (b.min((m + 1) * MINUTE_MS) - a.max(m * MINUTE_MS)).max(0);
                }
            }
        }
        s
    }
}

/// What the parts of the week read from its logs say when every log of the week is gone.
pub const GONE_ALL: &str = "the logs of this week are gone; the store keeps only its numbers";

fn calendar_add(s: &mut Sums, m: &crate::daystore::Minute) {
    s.usd += m.usd;
    s.avoidable_usd += m.avoidable_usd;
    s.saved_usd += m.saved_usd;
    s.requests += m.requests as u64;
    s.unpriced += m.unpriced as u64;
    s.read_tokens += m.read_tokens;
    s.prompt_tokens += m.prompt_tokens;
}

/// The minutes an entry's agent worked, as minutes since 1970-01-01 00:00 UTC.
fn work_minutes(e: &Entry) -> impl Iterator<Item = i64> + '_ {
    e.work.iter().flat_map(|&(hour, bits)| (0..60).filter(move |b| bits & (1 << b) != 0).map(move |b| hour * 60 + b))
}

/// What the week's numbers compare with.
#[derive(Clone, Debug, PartialEq)]
pub enum Before {
    /// The week before, or its same point for a partial week, with the agents that start inside
    /// it but are too small to judge by.
    Against { window: (i64, i64), partial: bool, sums: Box<Sums>, minor: Vec<(String, i64, f64)> },
    /// The week before holds no request and no message.
    NoLogs((i64, i64)),
    /// An agent with 10% or more of the week's spend has its first request in the store inside
    /// the week before.
    Incomplete { window: (i64, i64), gaps: Vec<(String, i64)> },
    /// `--since` or `--until` cuts the week or the week before; the note names it.
    Within(String),
}

impl Before {
    /// The numbers compared with, when there is a comparison.
    pub fn sums(&self) -> Option<&Sums> {
        match self {
            Before::Against { sums, .. } => Some(sums),
            _ => None,
        }
    }

    /// The notes' text: `against 2–5 Nov, to 14:20`, `against 26 Oct – 1 Nov; opencode only from
    /// 29 Oct, 3% of spend, not judged`, `no logs in 23 Feb – 1 Mar`, `no comparison with 21–27
    /// Sep: claude's logs start 29 Sep`, `within --since 2026-11-04`.
    pub fn note(&self, offset: Offset) -> String {
        let span = |(from, to): (i64, i64)| dates(rhythm::date_of(from, offset), rhythm::date_of((to - 1).max(from), offset), false);
        let day = |ts: i64| local(ts, offset, "%-d %b");
        match self {
            Before::Against { window, partial, minor, .. } => {
                let mut note = format!("against {}", span(*window));
                if *partial {
                    note += &format!(", to {}", local(window.1, offset, "%H:%M"));
                }
                for (agent, ts, share) in minor {
                    note += &format!("; {agent} only from {}, {:.0}% of spend, not judged", day(*ts), 100.0 * share);
                }
                note
            }
            Before::NoLogs(window) => format!("no logs in {}", span(*window)),
            Before::Incomplete { window, gaps } => {
                let starts = gaps.iter().map(|(a, ts)| format!("{a}'s logs start {}", day(*ts))).collect::<Vec<_>>().join(", ");
                format!("no comparison with {}: {starts}", span(*window))
            }
            Before::Within(cut) => format!("within {cut}"),
        }
    }
}

/// What a day's row of the week is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RowKind {
    /// Before the store's first day with data: `no logs`, no strip and no numbers.
    NoLogs,
    /// After now (or `--until`): empty, with no numbers, opening nothing.
    Future,
    /// A day with its strip and its numbers, which may all be zero.
    Day,
}

/// A cold return on a strip: when, and the main session of the log it is in.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tick {
    pub ts_ms: i64,
    pub agent: String,
    pub session: String,
}

/// One day of the week: its calendar cell, and what its strip draws over the cell's span.
#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    pub date: i64,
    pub kind: RowKind,
    /// The calendar's cell of the date: its numbers and its span (`from_ms`, `to_ms`), the day's
    /// start and end by Day's rule, until now for today.
    pub cell: calendar::Day,
    /// What the strip draws: the cell's span, widened to what the day holds before its start, as
    /// on the first day of the logs, where a day starts in its first pause.
    pub span: (i64, i64),
    pub cold: usize,
    /// Where some agent worked, by whole minutes, merged.
    pub work: Vec<(i64, i64)>,
    /// The user's sittings.
    pub sittings: Vec<(i64, i64)>,
    pub ticks: Vec<Tick>,
    /// The agents band: from each moment on, the kept main sessions whose agent worked in that
    /// minute, each session once; the last step is back to 0.
    pub band: Vec<(i64, u32)>,
    /// The span reaches past the axis: before its start, from this moment; after its end, until
    /// this one.
    pub from_mark: Option<i64>,
    pub until_mark: Option<i64>,
}

impl Row {
    /// The most agents at work at once in the day.
    pub fn peak(&self) -> u32 {
        self.band.iter().map(|s| s.1).max().unwrap_or(0)
    }

    /// The mark at the left edge: `from Fri 19:29`.
    pub fn from_text(&self, offset: Offset) -> Option<String> {
        self.from_mark.map(|ts| format!("from {}", local(ts, offset, "%a %H:%M")))
    }

    /// The mark at the right edge: `until Sun 11:56`.
    pub fn until_text(&self, offset: Offset) -> Option<String> {
        self.until_mark.map(|ts| format!("until {}", local(ts, offset, "%a %H:%M")))
    }

    /// Whether a click on the row opens its day.
    pub fn opens(&self) -> bool {
        self.kind == RowKind::Day
    }
}

/// The clock axis of the strips: hours from each row's local midnight.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Axis {
    pub from_h: i64,
    pub to_h: i64,
}

impl Default for Axis {
    fn default() -> Axis {
        Axis { from_h: 0, to_h: 24 }
    }
}

impl Axis {
    pub fn hours(&self) -> i64 {
        self.to_h - self.from_h
    }

    /// Where a moment of the row of `date` lies on the axis, 0 at its start and 1 at its end,
    /// by local clock time from the row's midnight; outside the axis it is below 0 or above 1.
    pub fn x(&self, date: i64, ts: i64, offset: Offset) -> f64 {
        let hours = (ts + offset(ts) - date * DAY_MS) as f64 / HOUR_MS as f64;
        (hours - self.from_h as f64) / self.hours().max(1) as f64
    }

    /// The hour labels, `03` to `06` for an axis from 03:00 to 07:00 the next morning: each whole
    /// hour from the axis's start to the one before its end, with its hours from the row's
    /// midnight.
    pub fn labels(&self) -> Vec<(i64, String)> {
        (self.from_h..self.to_h).map(|h| (h, format!("{:02}", h.rem_euclid(24)))).collect()
    }

    /// The axis of rows whose spans run `(date, from, to)`: from the whole hour at or before the
    /// earliest start, not before 00:00, to the whole hour at or after the latest end, at most
    /// `AXIS_MAX_HOURS` long.
    fn of(spans: impl Iterator<Item = (i64, i64, i64)>, offset: Offset) -> Axis {
        let at = |date: i64, ts: i64| ts + offset(ts) - date * DAY_MS;
        let (mut first, mut last): (Option<i64>, Option<i64>) = (None, None);
        for (date, from, to) in spans {
            let (a, b) = (at(date, from).div_euclid(HOUR_MS), (at(date, to) + HOUR_MS - 1).div_euclid(HOUR_MS));
            first = Some(first.map_or(a, |f| f.min(a)));
            last = Some(last.map_or(b, |l| l.max(b)));
        }
        let (Some(first), Some(last)) = (first, last) else { return Axis::default() };
        let from_h = first.max(0);
        Axis { from_h, to_h: last.max(from_h + 1).min(from_h + AXIS_MAX_HOURS) }
    }

    fn start(&self, date: i64, offset: Offset) -> i64 {
        local_to_utc(date * DAY_MS + self.from_h * HOUR_MS, offset)
    }

    fn end(&self, date: i64, offset: Offset) -> i64 {
        local_to_utc(date * DAY_MS + self.to_h * HOUR_MS, offset)
    }
}

/// The instant of a local time, in epoch ms.
fn local_to_utc(local: i64, offset: Offset) -> i64 {
    local - offset(local - offset(local))
}

/// The difference between the seven rows and the week's window, in spend: what the week's
/// window holds that rows of other weeks have, and what its rows hold outside the window.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Gap {
    /// The spend of the rows, today's included.
    pub rows_usd: f64,
    /// In the window, in the day before Monday's (the night of the Sunday before).
    pub before_usd: f64,
    /// In Monday's row, before Monday 00:00.
    pub early_usd: f64,
    /// In Sunday's row, after the next Monday 00:00.
    pub after_usd: f64,
    /// In the window, in the next Monday's day.
    pub beyond_usd: f64,
}

impl Gap {
    /// The line under the totals when the rows add up to another sum: `The days add up to
    /// $1,836: $28 of the week fell in the night of Sun 1 Nov, and $21 of Sun 8 Nov came after
    /// the week.`
    pub fn line(&self, monday: i64) -> Option<String> {
        let day = |d: i64| naive(d).format("%a %-d %b").to_string();
        let mut parts = Vec::new();
        let some = |v: f64| v >= 0.005;
        if some(self.before_usd) {
            parts.push(format!("{} of the week fell in the night of {}", dollars(self.before_usd), day(monday - 1)));
        }
        if some(self.early_usd) {
            parts.push(format!("{} of {} came before the week", dollars(self.early_usd), day(monday)));
        }
        if some(self.after_usd) {
            parts.push(format!("{} of {} came after the week", dollars(self.after_usd), day(monday + 6)));
        }
        if some(self.beyond_usd) {
            parts.push(format!("{} of the week fell in the day of {}", dollars(self.beyond_usd), day(monday + 7)));
        }
        let joined = match parts.len() {
            0 => return None,
            1 => parts.remove(0),
            n => format!("{}, and {}", parts[..n - 1].join(", "), parts[n - 1]),
        };
        Some(format!("The days add up to {}: {joined}.", dollars(self.rows_usd)))
    }
}

/// A project of the week: its spend and share of the week's, and the minutes some agent of it
/// worked, each once.
#[derive(Clone, Debug, PartialEq)]
pub struct Project {
    pub name: String,
    pub usd: f64,
    pub share: f64,
    pub agent_minutes: u64,
}

/// A calendar week of the day store.
#[derive(Clone, Debug, PartialEq)]
pub struct Week {
    pub window: Window,
    /// What the numbers count: the week's window, cut by `--since` and `--until`.
    pub from_ms: i64,
    pub to_ms: i64,
    pub totals: Sums,
    pub before: Before,
    /// For a week in progress, the totals of the last full week.
    pub last_week: Option<Sums>,
    /// Monday to Sunday.
    pub rows: Vec<Row>,
    pub axis: Axis,
    /// The agents band's full height: the week's highest number at work, or `BAND_MIN`.
    pub band_top: u32,
    pub gap: Gap,
    /// Every project with a request in the window, the most spend first.
    pub projects: Vec<Project>,
    /// The days the pace per day counts over, when there is no comparison.
    pub pace_days: Option<i64>,
    /// The day the logs begin (see `period::logs_begin`): the rows before it read `no logs`.
    pub first_data: Option<i64>,
}

impl Week {
    /// The week whose Monday is `monday`, of the entries `filters` keeps, as of `now_ms`.
    /// `--since` and `--until` in the filters cut it as they cut the calendar.
    pub fn of(entries: &[Entry], filters: &Filters, monday: i64, now_ms: i64, offset: Offset) -> Week {
        let window = Window::of(monday, now_ms, offset);
        let cut = |(a, b): (i64, i64)| (filters.since_ms.map_or(a, |s| s.max(a)), filters.until_ms.map_or(b, |u| u.min(b)).max(a));
        let (from_ms, to_ms) = cut((window.from_ms, window.to_ms));
        let kept: Vec<&Entry> = entries.iter().filter(|e| filters.keeps(e)).collect();
        let end = filters.until_ms.map_or(now_ms, |u| u.min(now_ms));
        // The breaks of a window are found by its days as of its end, as `tokprof pauses
        // --until` finds them; the rows by the days as of now, as the calendar does.
        let rule_at = |at: i64| calendar::rule_of(kept.iter().copied(), at, offset);
        let totals = Sums::of(&kept, (from_ms, to_ms), &rule_at(to_ms.min(end)), offset);
        let last_week = (!window.complete()).then(|| Sums::of(&kept, cut(window.last_week), &rule_at(window.last_week.1.min(end)), offset));

        let before = before(entries, filters, &kept, &window, (from_ms, to_ms), &rule_at(window.before.1.min(end)), offset);
        let first_request = kept.iter().flat_map(|e| e.minutes.iter().filter(|m| m.requests > 0 && m.minute > 0).map(|m| m.minute * MINUTE_MS)).min();
        let pace_days = match before {
            Before::Against { .. } => None,
            _ => {
                let first = [Some(monday), first_request.map(|t| rhythm::date_of(t, offset)), Some(rhythm::date_of(from_ms, offset))]
                    .into_iter()
                    .flatten()
                    .max()
                    .unwrap_or(monday);
                let last = (monday + 6).min(rhythm::date_of((to_ms - 1).max(from_ms), offset));
                (last >= first && to_ms > from_ms).then_some(last - first + 1)
            }
        };

        // The rows: the calendar's cells, and their layers by the same days.
        let rule = rule_at(end);
        let cal = Calendar::span(entries, filters, (monday, monday + 6), now_ms, offset);
        // The day the logs begin, as Month, Quarter and Year have it.
        let first_data = crate::period::logs_begin(entries, filters, offset);
        let mut rows: Vec<Row> = (monday..monday + 7)
            .map(|date| {
                let kind = match (cal.day(date), first_data) {
                    (_, Some(f)) if date < f => RowKind::NoLogs,
                    (_, None) => RowKind::NoLogs,
                    (None, _) => RowKind::Future,
                    (Some(_), _) => RowKind::Day,
                };
                let cell = cal.day(date).cloned().unwrap_or(calendar::Day { date, ..calendar::Day::default() });
                let span = (cell.from_ms, cell.to_ms);
                Row {
                    date,
                    kind,
                    cell,
                    span,
                    cold: 0,
                    work: Vec::new(),
                    sittings: Vec::new(),
                    ticks: Vec::new(),
                    band: Vec::new(),
                    from_mark: None,
                    until_mark: None,
                }
            })
            .collect();
        layers(&mut rows, &kept, &rule, filters, offset);
        let axis = Axis::of(rows.iter().filter(|r| r.kind == RowKind::Day && r.cell.any()).map(|r| (r.date, r.span.0, r.span.1)), offset);
        for r in rows.iter_mut().filter(|r| r.kind == RowKind::Day && r.cell.any()) {
            r.from_mark = (r.span.0 < axis.start(r.date, offset)).then_some(r.span.0);
            r.until_mark = (r.span.1 > axis.end(r.date, offset)).then_some(r.span.1);
        }
        let band_top = rows.iter().map(Row::peak).max().unwrap_or(0).max(BAND_MIN);
        let gap = gap(&kept, &rows, &rule, monday, (from_ms, to_ms), filters, offset);
        let projects = projects(&kept, (from_ms, to_ms), totals.usd);
        Week { window, from_ms, to_ms, totals, before, last_week, rows, axis, band_top, gap, projects, pace_days, first_data }
    }

    /// A number over the days the week has run, when there is no comparison.
    pub fn pace(&self, v: f64) -> Option<f64> {
        self.pace_days.map(|d| v / d as f64)
    }

    /// The pace of dollars: `$140 a day`.
    pub fn pace_usd(&self, usd: f64) -> Option<String> {
        self.pace(usd).map(|v| format!("{} a day", dollars(v)))
    }

    /// The week's name, `9–15 Nov 2026`.
    pub fn name(&self) -> String {
        name(self.window.monday)
    }

    /// The projects By project shows, and how many more there are.
    pub fn shown_projects(&self) -> (&[Project], usize) {
        let n = self.projects.len().min(PROJECTS_SHOWN);
        (&self.projects[..n], self.projects.len() - n)
    }
}

/// The comparison of the week, by the rules of An incomplete period before and A small agent in
/// the period before.
fn before(entries: &[Entry], filters: &Filters, kept: &[&Entry], window: &Window, shown: (i64, i64), rule: &DayRule, offset: Offset) -> Before {
    let day = |ts: i64| local(ts, offset, "%Y-%m-%d");
    let since = filters.since_ms.filter(|&s| s > window.before.0).map(|s| format!("--since {}", day(s)));
    let until = filters.until_ms.filter(|&u| u < window.to_ms).map(|u| format!("--until {}", day(u)));
    match (since, until) {
        (Some(s), Some(u)) => return Before::Within(format!("{s} {u}")),
        (Some(c), None) | (None, Some(c)) => return Before::Within(c),
        (None, None) => {}
    }
    // No logs: the store's history of the agents kept holds nothing then. The project, session
    // and text filters do not make logs missing: a project new this week compares with nothing.
    let sums = Sums::of(kept, window.before, rule, offset);
    if !crate::period::stored_any(entries, filters, window.before) {
        return Before::NoLogs(window.before);
    }
    let gaps = crate::compare::gaps(entries, filters, shown, window.before.0);
    if !gaps.is_empty() {
        return Before::Incomplete { window: window.before, gaps };
    }
    let minor = crate::compare::minor_gaps(entries, filters, shown, window.before.0);
    Before::Against { window: window.before, partial: !window.complete(), sums: Box::new(sums), minor }
}

/// The layers of each day's row over its span: work, sittings, cold ticks and the agents band.
fn layers(rows: &mut [Row], kept: &[&Entry], rule: &DayRule, filters: &Filters, offset: Offset) {
    let inside = |ts: i64| filters.since_ms.is_none_or(|s| ts >= s) && filters.until_ms.is_none_or(|u| ts < u);
    let index = |ts: i64| {
        let date = rule.date_of(ts, offset);
        rows.iter().position(|r| r.date == date && r.kind == RowKind::Day)
    };
    let mut minutes: Vec<HashMap<i64, HashSet<(&str, &str)>>> = vec![HashMap::new(); rows.len()];
    let mut actions: Vec<Vec<i64>> = vec![Vec::new(); rows.len()];
    let mut ticks: Vec<Vec<Tick>> = vec![Vec::new(); rows.len()];
    // A day before its own start holds what came before the first start of the logs.
    let (first, last) = match (rows.iter().find(|r| r.kind == RowKind::Day), rows.iter().rev().find(|r| r.kind == RowKind::Day)) {
        (Some(a), Some(b)) => (a.cell.from_ms - DAY_MS, b.cell.to_ms),
        _ => return,
    };
    let near = |ts: i64| ts >= first && ts < last && inside(ts);
    for e in kept {
        for &ts in e.cold.iter().filter(|&&t| near(t)) {
            if let Some(i) = index(ts) {
                ticks[i].push(Tick { ts_ms: ts, agent: e.agent.clone(), session: e.main.clone() });
            }
        }
        if e.subagent || e.left_out {
            continue;
        }
        for &(ts, _) in e.actions.iter().filter(|a| near(a.0)) {
            if let Some(i) = index(ts) {
                actions[i].push(ts);
            }
        }
        for minute in work_minutes(e).filter(|m| near(m * MINUTE_MS)) {
            if let Some(i) = index(minute * MINUTE_MS) {
                minutes[i].entry(minute).or_default().insert((e.agent.as_str(), e.main.as_str()));
            }
        }
    }
    for (i, r) in rows.iter_mut().enumerate() {
        let mut ts: Vec<Tick> = std::mem::take(&mut ticks[i]);
        ts.sort_by(|a, b| a.ts_ms.cmp(&b.ts_ms).then_with(|| a.session.cmp(&b.session)));
        r.cold = ts.len();
        r.ticks = ts;
        let mut acts = std::mem::take(&mut actions[i]);
        acts.sort_unstable();
        r.sittings = rhythm::sittings(&acts);
        let mut counted: Vec<(i64, u32)> = minutes[i].iter().map(|(m, k)| (*m, k.len() as u32)).collect();
        counted.sort_unstable();
        let earliest = [r.ticks.first().map(|t| t.ts_ms), acts.first().copied(), counted.first().map(|c| c.0 * MINUTE_MS)].into_iter().flatten().min();
        r.span.0 = earliest.map_or(r.span.0, |e| e.min(r.span.0));
        let end = r.cell.to_ms;
        let mut work: Vec<(i64, i64)> = Vec::new();
        let mut band: Vec<(i64, u32)> = Vec::new();
        for (k, &(m, n)) in counted.iter().enumerate() {
            let (a, b) = (m * MINUTE_MS, ((m + 1) * MINUTE_MS).min(end));
            match work.last_mut() {
                Some(last) if last.1 >= a => last.1 = b,
                _ => work.push((a, b)),
            }
            if band.last().is_none_or(|s: &(i64, u32)| s.1 != n) {
                band.push((a, n));
            }
            // Back to 0 where the next minute does not follow.
            if counted.get(k + 1).is_none_or(|next| next.0 != m + 1) {
                band.push((b, 0));
            }
        }
        r.work = work;
        r.band = band;
    }
}

/// What the rows and the week's window hold of each other's spend.
fn gap(kept: &[&Entry], rows: &[Row], rule: &DayRule, monday: i64, (from, to): (i64, i64), filters: &Filters, offset: Offset) -> Gap {
    let mut g = Gap { rows_usd: rows.iter().filter(|r| r.kind == RowKind::Day).map(|r| r.cell.amounts.usd).sum(), ..Gap::default() };
    let inside = |ts: i64| filters.since_ms.is_none_or(|s| ts >= s) && filters.until_ms.is_none_or(|u| ts < u);
    let in_rows = |ts: i64, date: i64| rows.iter().any(|r| r.date == date && r.kind == RowKind::Day && ts >= r.cell.from_ms && ts < r.cell.to_ms);
    for e in kept {
        for m in e.minutes.iter().filter(|m| m.minute > 0) {
            let ts = m.minute * MINUTE_MS;
            if !inside(ts) || ts < from - DAY_MS || ts >= to + DAY_MS {
                continue;
            }
            let date = rule.date_of(ts, offset);
            let in_window = ts >= from && ts < to;
            match (in_window, date) {
                (true, d) if d < monday => g.before_usd += m.usd,
                (true, d) if d > monday + 6 => g.beyond_usd += m.usd,
                (false, d) if d == monday && ts < from && in_rows(ts, d) => g.early_usd += m.usd,
                (false, d) if d == monday + 6 && ts >= to && in_rows(ts, d) => g.after_usd += m.usd,
                _ => {}
            }
        }
    }
    g
}

/// The projects with a request in the window: spend, share of `total`, and the minutes some agent
/// of the project worked, each once, the most spend first.
fn projects(kept: &[&Entry], (from, to): (i64, i64), total: f64) -> Vec<Project> {
    let inside = |ts: i64| ts > 0 && ts >= from && ts < to;
    let mut spend: HashMap<&str, (f64, bool)> = HashMap::new();
    let mut minutes: HashMap<&str, HashSet<i64>> = HashMap::new();
    for e in kept {
        for m in e.minutes.iter().filter(|m| m.minute > 0 && inside(m.minute * MINUTE_MS)) {
            let s = spend.entry(e.project.as_str()).or_default();
            s.0 += m.usd;
            s.1 |= m.requests > 0;
        }
        if !e.subagent && !e.left_out {
            minutes.entry(e.project.as_str()).or_default().extend(work_minutes(e).filter(|m| inside(m * MINUTE_MS)));
        }
    }
    let mut out: Vec<Project> = spend
        .into_iter()
        .filter(|(_, (_, requested))| *requested)
        .map(|(name, (usd, _))| Project {
            name: name.to_string(),
            usd,
            share: if total > 0.0 { usd / total } else { 0.0 },
            agent_minutes: minutes.get(name).map_or(0, |m| m.len() as u64),
        })
        .collect();
    out.sort_by(|a, b| b.usd.total_cmp(&a.usd).then_with(|| b.agent_minutes.cmp(&a.agent_minutes)).then_with(|| a.name.cmp(&b.name)));
    out
}

#[cfg(test)]
#[path = "week_tests.rs"]
mod tests;
