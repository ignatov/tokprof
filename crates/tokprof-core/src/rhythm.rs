//! The work rhythm: how the user and the agents hand work to each other. A main session becomes
//! a timeline of the user's actions, the agent's hand-offs and its requests; the rules turn the
//! timelines into turns, work, waits, sittings and breaks, and find where the user's day starts.
//!
//! Local time comes in as an offset function, so that the rules read no time zone themselves.

use std::collections::BTreeMap;

use crate::fmt;
use crate::model::{HandoffKind, Session};
use crate::pauses::{ColdReturn, Pauses};
use crate::source::AgentId;

const MINUTE: i64 = 60_000;
const HOUR: i64 = 60 * MINUTE;
const DAY: i64 = 24 * HOUR;

/// Actions less than this apart are one sitting; a turn with no event for this long is silent.
pub const GAP_MS: i64 = 15 * MINUTE;
/// A day's night is a pause whose middle falls from this long before its midnight to this long
/// after it: from 18:00 the day before to 12:00 the day.
const NIGHT_FROM: i64 = 6 * HOUR;
const NIGHT_TO: i64 = 12 * HOUR;
/// While a day's window is still open, only a pause this long counts as its night.
pub const OPEN_NIGHT_MS: i64 = 4 * HOUR;
/// A sitting lasts at least this long.
const MIN_SITTING_MS: i64 = MINUTE;
/// The widest the day strip gets in a terminal, in columns, and the column widths it picks
/// from, in minutes.
pub const STRIP_COLUMNS: i64 = 72;
const COLUMN_MINUTES: [i64; 5] = [10, 15, 20, 30, 60];
/// Lanes the strip draws in a terminal before it folds the rest into one line.
pub const STRIP_LANES: usize = 8;

/// Where a strip is shown: the width of its labels, its columns, the rows its lane block may take
/// (lanes, hour-mark rows and the `more sessions` line), and whether lanes may be spaced.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Layout {
    pub label: usize,
    pub columns: i64,
    pub rows: usize,
    pub space: bool,
}

impl Layout {
    /// `tokprof day`'s layout: 20-character labels, 72 columns, 8 lanes and the `more` line.
    pub const TERMINAL: Layout = Layout { label: 20, columns: STRIP_COLUMNS, rows: STRIP_LANES + 1, space: false };
}

/// The offset of local time from UTC at an instant, both in ms.
pub type Offset<'a> = &'a dyn Fn(i64) -> i64;

/// The offset of the machine's time zone at an instant, in ms. Offsets change on the hour, so
/// each thread asks the time zone once per hour of UTC: the rules ask it for every pause.
pub fn local_offset(ts_ms: i64) -> i64 {
    use chrono::Offset as _;
    thread_local! {
        static BY_HOUR: std::cell::RefCell<std::collections::HashMap<i64, i64>> = Default::default();
    }
    let hour = ts_ms.div_euclid(HOUR);
    BY_HOUR.with(|cache| {
        *cache.borrow_mut().entry(hour).or_insert_with(|| {
            chrono::DateTime::from_timestamp_millis(hour * HOUR)
                .map(|t| t.with_timezone(&chrono::Local).offset().fix().local_minus_utc() as i64 * 1000)
                .unwrap_or(0)
        })
    })
}

/// What happened at one instant of a timeline.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Event {
    /// A message the user typed.
    Typed,
    /// A background task or another session gave the agent work.
    Wake,
    /// A request of a main conversation.
    Request,
    TypedWhileWorking,
    Question,
    Answer,
    TurnEnd,
    Interrupt,
}

impl Event {
    /// At one instant, starts come first and ends last.
    fn rank(self) -> u8 {
        match self {
            Event::Typed | Event::Wake => 0,
            Event::Request | Event::TypedWhileWorking | Event::Question | Event::Answer => 1,
            Event::TurnEnd | Event::Interrupt => 2,
        }
    }

    /// Whether the user did it.
    pub fn is_action(self) -> bool {
        matches!(self, Event::Typed | Event::TypedWhileWorking | Event::Answer | Event::Interrupt)
    }

    /// Whether it is a message the user typed.
    pub fn is_message(self) -> bool {
        matches!(self, Event::Typed | Event::TypedWhileWorking)
    }
}

/// One main session as the rhythm reads it.
#[derive(Clone, Debug)]
pub struct Timeline {
    pub agent: AgentId,
    pub id: String,
    pub title: Option<String>,
    /// A program ran the session, or it ran in a temporary folder: nobody typed in it.
    pub left_out: bool,
    /// In time order; at one instant, starts first and ends last.
    pub events: Vec<(i64, Event)>,
}

impl Timeline {
    /// The timeline of a main session. `temp` lists the temporary folders, whose sessions are
    /// left out.
    pub fn of(s: &Session, temp: &[String]) -> Timeline {
        let mut events: Vec<(i64, Event)> = s.typed.iter().map(|t| (t.ts_ms, Event::Typed)).collect();
        events.extend(s.handoffs.iter().map(|h| {
            let e = match h.kind {
                HandoffKind::TurnEnd => Event::TurnEnd,
                HandoffKind::Interrupt => Event::Interrupt,
                HandoffKind::TypedWhileWorking => Event::TypedWhileWorking,
                HandoffKind::Question => Event::Question,
                HandoffKind::Answer => Event::Answer,
                HandoffKind::Wake => Event::Wake,
            };
            (h.ts_ms, e)
        }));
        for (_, requests) in s.main_conversations() {
            events.extend(requests.iter().map(|r| (r.ts_ms, Event::Request)));
        }
        events.retain(|(ts, _)| *ts > 0);
        events.sort_by_key(|&(ts, e)| (ts, e.rank()));
        let under = |cwd: &str, root: &str| {
            let root = root.trim_end_matches('/');
            !root.is_empty() && (cwd == root || cwd.strip_prefix(root).is_some_and(|rest| rest.starts_with('/')))
        };
        let in_temp = s.cwd.as_deref().is_some_and(|cwd| temp.iter().any(|t| under(cwd, t)));
        Timeline { agent: s.agent.clone(), id: s.id.clone(), title: s.title.clone(), left_out: s.scripted || in_temp, events }
    }

    /// The times of the user's actions.
    pub fn actions(&self) -> impl Iterator<Item = i64> + '_ {
        self.events.iter().filter(|(_, e)| e.is_action()).map(|(ts, _)| *ts)
    }
}

/// What started a turn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Start {
    Typed,
    /// The agent started it itself.
    Wake,
    /// A request with neither a message nor a wake before it.
    Other,
}

/// How a turn ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum End {
    Logged,
    Interrupted,
    /// The log does not say; the turn ends at its last event.
    Unlogged,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Turn {
    pub start: i64,
    pub end: i64,
    pub by: Start,
    pub how: End,
}

impl Turn {
    /// Whether the agent ended it, rather than the user.
    pub fn ended_by_agent(&self) -> bool {
        self.how != End::Interrupted
    }
}

/// What one session did, worked out from its timeline.
#[derive(Clone, Debug, Default)]
pub struct Lane {
    pub turns: Vec<Turn>,
    /// When the agent worked: its turns without their silences and question waits.
    pub work: Vec<(i64, i64)>,
    /// The user's actions.
    pub actions: Vec<(i64, Event)>,
    /// Each typed message after an end of the agent: (that end, the message).
    pub answers: Vec<(i64, i64)>,
    /// Each question and its answer, if one came.
    pub questions: Vec<(i64, Option<i64>)>,
}

/// A turn while it runs.
struct Open {
    start: i64,
    by: Start,
    last: i64,
    work_from: Option<i64>,
    asked: Option<usize>,
}

impl Lane {
    pub fn of(t: &Timeline) -> Lane {
        let mut lane = Lane::default();
        let mut open: Option<Open> = None;
        // The latest end of the agent that no message has answered yet.
        let mut last_end: Option<i64> = None;
        for &(ts, e) in &t.events {
            if e.is_action() {
                lane.actions.push((ts, e));
            }
            match e {
                Event::Typed => {
                    if let Some(o) = open.take() {
                        let end = o.last;
                        lane.close(o, end, End::Unlogged);
                        last_end = Some(end);
                    }
                    if let Some(end) = last_end.take() {
                        lane.answers.push((end, ts));
                    }
                    open = Some(Open { start: ts, by: Start::Typed, last: ts, work_from: Some(ts), asked: None });
                }
                Event::Wake => {
                    if open.is_none() {
                        open = Some(Open { start: ts, by: Start::Wake, last: ts, work_from: Some(ts), asked: None });
                    }
                }
                Event::Request | Event::TypedWhileWorking | Event::Question | Event::Answer => {
                    let o = open.get_or_insert(Open { start: ts, by: Start::Other, last: ts, work_from: Some(ts), asked: None });
                    if e != Event::TypedWhileWorking {
                        lane.step(o, ts);
                    }
                    match e {
                        Event::Question => {
                            if let Some(from) = o.work_from.take() {
                                lane.work.push((from, ts));
                            }
                            lane.questions.push((ts, None));
                            o.asked = Some(lane.questions.len() - 1);
                        }
                        Event::Answer => {
                            if let Some(q) = o.asked.take() {
                                lane.questions[q].1 = Some(ts);
                            }
                            o.work_from = Some(ts);
                        }
                        _ => {}
                    }
                }
                Event::TurnEnd | Event::Interrupt => {
                    if let Some(mut o) = open.take() {
                        lane.step(&mut o, ts);
                        let how = if e == Event::TurnEnd { End::Logged } else { End::Interrupted };
                        lane.close(o, ts, how);
                        last_end = (how == End::Logged).then_some(ts);
                    }
                }
            }
        }
        if let Some(o) = open.take() {
            let end = o.last;
            lane.close(o, end, End::Unlogged);
        }
        lane
    }

    /// An event inside a turn: after a silence, the work before it ends at the turn's previous
    /// event and starts again now.
    fn step(&mut self, o: &mut Open, ts: i64) {
        if o.asked.is_none() && ts - o.last >= GAP_MS {
            if let Some(from) = o.work_from {
                self.work.push((from, o.last));
            }
            o.work_from = Some(ts);
        }
        o.last = ts;
    }

    fn close(&mut self, o: Open, end: i64, how: End) {
        if let Some(from) = o.work_from {
            self.work.push((from, end.max(from)));
        }
        self.turns.push(Turn { start: o.start, end, by: o.by, how });
    }

    /// How long the agent worked in all.
    pub fn work_ms(&self) -> i64 {
        self.work.iter().map(|(a, b)| b - a).sum()
    }
}

/// Where one day starts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DayStart {
    /// The local date, as days since 1970-01-01.
    pub date: i64,
    /// When the day starts, in epoch ms.
    pub at: i64,
    /// The pause the day starts in the middle of, from the user's last action before it to the
    /// first after it; `None` for a day that starts at midnight.
    pub night: Option<(i64, i64)>,
}

/// Where each day starts: in the middle of the night before it, the longest pause between the
/// user's actions whose middle falls from 18:00 the day before to 12:00 the day.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Days {
    /// In time order. No day starts after the latest action, so the last one runs until now.
    pub starts: Vec<DayStart>,
}

/// Local midnight of a date, in epoch ms.
pub fn midnight(date: i64, offset: Offset) -> i64 {
    let local = date * DAY;
    local - offset(local - offset(local))
}

/// The local date of an instant, as days since 1970-01-01.
pub fn date_of(ts: i64, offset: Offset) -> i64 {
    (ts + offset(ts)).div_euclid(DAY)
}

impl Days {
    /// The days of these actions, as of `now_ms`.
    pub fn of(actions: impl Iterator<Item = i64>, now_ms: i64, offset: Offset) -> Days {
        let mut acts: Vec<i64> = actions.collect();
        acts.sort_unstable();
        acts.dedup();
        let (Some(&first), Some(&latest)) = (acts.first(), acts.last()) else { return Days::default() };
        // A date's window ends at noon; until then only a long pause can be its night.
        let open = |date: i64| now_ms < midnight(date, offset) + NIGHT_TO;
        let mut best: BTreeMap<i64, (i64, i64)> = BTreeMap::new();
        for pair in acts.windows(2) {
            let (a, b) = (pair[0], pair[1]);
            let middle = a + (b - a) / 2;
            let local = middle + offset(middle);
            if (NIGHT_TO..DAY - NIGHT_FROM).contains(&local.rem_euclid(DAY)) {
                continue;
            }
            let date = (local + NIGHT_FROM).div_euclid(DAY);
            if open(date) && b - a < OPEN_NIGHT_MS {
                continue;
            }
            let e = best.entry(date).or_insert((a, b));
            if b - a > e.1 - e.0 {
                *e = (a, b);
            }
        }
        let mut starts = Vec::new();
        for date in date_of(first, offset)..=date_of(latest, offset) + 1 {
            let start = match best.get(&date) {
                Some(&(a, b)) => DayStart { date, at: a + (b - a) / 2, night: Some((a, b)) },
                None if open(date) => continue,
                None => DayStart { date, at: midnight(date, offset), night: None },
            };
            if start.at <= latest {
                starts.push(start);
            }
        }
        Days { starts }
    }

    /// The date of the day an instant falls in; `None` before the first day.
    pub fn day_of(&self, ts: i64) -> Option<i64> {
        let i = self.starts.partition_point(|s| s.at <= ts);
        (i > 0).then(|| self.starts[i - 1].date)
    }

    /// The date of the day of the latest action.
    pub fn latest(&self) -> Option<i64> {
        self.starts.last().map(|s| s.date)
    }

    /// When the day of a date starts and ends. The current day ends at `i64::MAX`; a date with no
    /// day of its own runs from midnight to midnight.
    pub fn bounds(&self, date: i64, offset: Offset) -> (i64, i64) {
        match self.starts.iter().position(|s| s.date == date) {
            Some(i) => (self.starts[i].at, self.starts.get(i + 1).map_or(i64::MAX, |s| s.at)),
            None => (midnight(date, offset), midnight(date + 1, offset)),
        }
    }

    /// The `night` line of the day of a date.
    pub fn line(&self, date: i64, offset: Offset) -> String {
        let hm = |ts: i64| {
            let m = (ts + offset(ts)).rem_euclid(DAY) / MINUTE;
            format!("{:02}:{:02}", m / 60, m % 60)
        };
        let pause = |(a, b): (i64, i64)| format!("{}–{} ({})", hm(a), hm(b), fmt::human_duration((b - a) as f64 / 1000.0));
        let i = self.starts.iter().position(|s| s.date == date);
        let Some((i, before)) = i.and_then(|i| self.starts[i].night.map(|n| (i, n))) else {
            return "night     no pause before the day; it starts at midnight".to_string();
        };
        match self.starts.get(i + 1) {
            None => format!("night     before the day {}; the day runs until now", pause(before)),
            Some(DayStart { night: Some(after), .. }) => {
                format!("night     before the day {}, after it {}; a day runs from the middle of one night to the middle of the next", pause(before), pause(*after))
            }
            Some(_) => format!("night     before the day {}; the next day starts at midnight", pause(before)),
        }
    }

    /// The median start of the days with a night that start in `[from, to)`, in minutes after
    /// local midnight; a start in the evening before counts as negative.
    pub fn median_start(&self, (from, to): (i64, i64), offset: Offset) -> Option<i64> {
        let mut minutes: Vec<i64> = self
            .starts
            .iter()
            .filter(|s| s.night.is_some() && s.at >= from && s.at < to)
            .map(|s| {
                let m = (s.at + offset(s.at)).rem_euclid(DAY) / MINUTE;
                if m >= (DAY - NIGHT_FROM) / MINUTE { m - DAY / MINUTE } else { m }
            })
            .collect();
        minutes.sort_unstable();
        minutes.get(minutes.len() / 2).copied()
    }

    /// The pause report's `nights` line.
    pub fn nights_line(&self, window: (i64, i64), offset: Offset) -> String {
        match self.median_start(window, offset) {
            Some(m) => {
                let m = m.rem_euclid(DAY / MINUTE);
                format!("nights    a day runs from the middle of one night to the middle of the next; days started at a median {:02}:{:02}", m / 60, m % 60)
            }
            None => "nights    no night in this time; days start at midnight".to_string(),
        }
    }
}

/// A break between two sittings of one day: when it ran, how long after its start all agents
/// stood still, and how long some agent worked in it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Break {
    pub from: i64,
    pub to: i64,
    pub still_after: i64,
    pub worked: i64,
}

/// The rhythm of the sessions shown over a stretch of time.
#[derive(Clone, Debug, Default)]
pub struct Summary {
    /// From the first action to the last.
    pub sittings: Vec<(i64, i64)>,
    pub breaks: Vec<Break>,
    /// The times of the messages the user typed.
    pub messages: Vec<i64>,
    pub sessions: usize,
    /// The turns the agent ended: when, whether the user was there, whether it started them itself.
    pub ends: Vec<(i64, bool, bool)>,
    /// How long the turns the user started took.
    pub turns: Vec<i64>,
    /// Each answer: when the message came and how long after the end of the turn.
    pub answers: Vec<(i64, i64)>,
    pub interrupts: usize,
    pub typed_while_working: usize,
    pub questions: usize,
    /// How long after its question each answer came.
    pub question_answers: Vec<i64>,
}

impl Summary {
    /// The rhythm of these lanes in `[from, to)`, with days that start at `start`.
    pub fn of(lanes: &[&Lane], (from, to): (i64, i64), days: &Days) -> Summary {
        let inside = |ts: i64| ts >= from && ts < to;
        let mut s = Summary::default();
        let mut actions: Vec<i64> = Vec::new();
        for lane in lanes {
            let mut acted = false;
            for &(ts, e) in lane.actions.iter().filter(|(ts, _)| inside(*ts)) {
                acted = true;
                actions.push(ts);
                match e {
                    Event::Typed => s.messages.push(ts),
                    Event::TypedWhileWorking => {
                        s.messages.push(ts);
                        s.typed_while_working += 1;
                    }
                    Event::Interrupt => s.interrupts += 1,
                    _ => {}
                }
            }
            s.sessions += usize::from(acted);
            for t in &lane.turns {
                if t.by == Start::Typed && inside(t.start) {
                    s.turns.push(t.end - t.start);
                }
            }
            for &(end, msg) in &lane.answers {
                if inside(msg) && days.day_of(end) == days.day_of(msg) {
                    s.answers.push((msg, msg - end));
                }
            }
            for &(ask, answer) in lane.questions.iter().filter(|(ask, _)| inside(*ask)) {
                s.questions += 1;
                s.question_answers.extend(answer.map(|a| a - ask));
            }
        }
        actions.sort_unstable();
        s.messages.sort_unstable();
        for ts in actions {
            match s.sittings.last_mut() {
                Some(last) if ts - last.1 < GAP_MS => last.1 = ts,
                _ => s.sittings.push((ts, ts)),
            }
        }
        let present = |ts: i64| {
            let i = s.sittings.partition_point(|(a, _)| *a <= ts);
            i > 0 && ts <= s.sittings[i - 1].1
        };
        let mut ends: Vec<(i64, bool, bool)> = lanes
            .iter()
            .flat_map(|l| &l.turns)
            .filter(|t| t.ended_by_agent() && inside(t.end))
            .map(|t| (t.end, present(t.end), t.by == Start::Wake))
            .collect();
        ends.sort_unstable();
        s.ends = ends;
        let work = union(lanes.iter().flat_map(|l| l.work.iter().copied()));
        for pair in s.sittings.windows(2) {
            let (from, to) = (pair[0].1, pair[1].0);
            if days.day_of(from) != days.day_of(to) {
                continue;
            }
            let worked = work.iter().map(|&(a, b)| (b.min(to) - a.max(from)).max(0)).sum();
            let still_after = work.iter().find(|(a, b)| *a <= from && from < *b).map_or(0, |(_, b)| b.min(&to) - from);
            s.breaks.push(Break { from, to, still_after, worked });
        }
        s
    }

    /// The time at the keyboard: the sittings, each at least a minute.
    pub fn keyboard_ms(&self) -> i64 {
        self.sittings.iter().map(|(a, b)| (b - a).max(MIN_SITTING_MS)).sum()
    }

    /// The five rhythm lines.
    pub fn lines(&self) -> Vec<String> {
        let mut out = Vec::new();
        let sittings: Vec<i64> = self.sittings.iter().map(|(a, b)| (b - a).max(MIN_SITTING_MS)).collect();
        let breaks: Vec<i64> = self.breaks.iter().map(|b| b.to - b.from).collect();
        out.push(format!(
            "sittings  {}, {} at the keyboard, {} to {}; median sitting {}, {}",
            self.sittings.len(),
            duration(self.keyboard_ms()),
            fmt::count(self.messages.len() as u64, "message", "messages"),
            fmt::count(self.sessions as u64, "session", "sessions"),
            median(&sittings).map_or("n/a".to_string(), duration),
            median(&breaks).map_or("no break".to_string(), |b| format!("median break {}", duration(b))),
        ));
        out.push(match (median(&self.breaks.iter().map(|b| b.still_after).collect::<Vec<_>>()), self.worked_share()) {
            (Some(still), Some(share)) => {
                format!("left      the agents stood still a median {} after you left, and worked through {share:.0}% of a median break", duration(still))
            }
            _ => "left      no break between sittings".to_string(),
        });
        let present = self.ends.iter().filter(|e| e.1).count();
        let first = if present == 0 {
            "no turn ended while you were there".to_string()
        } else {
            format!("an agent ended a turn every {} while you were there", duration(self.keyboard_ms() / present as i64))
        };
        let own = self.ends.iter().filter(|e| e.2).count();
        let share = 100.0 * own as f64 / self.ends.len().max(1) as f64;
        out.push(format!("present   {first}; it started {share:.0}% of its turns itself"));
        let turns = median(&self.turns).map_or("no turns you started".to_string(), |t| format!("the agent's turn took a median {}", duration(t)));
        let answers: Vec<i64> = self.answers.iter().map(|a| a.1).collect();
        let answered = median(&answers).map_or("no answers".to_string(), |a| format!("you answered in a median {}", duration(a)));
        out.push(format!("answers   {turns}; {answered}"));
        let questions = match (self.questions, median(&self.question_answers)) {
            (0, _) => "no questions from the agent".to_string(),
            (n, Some(a)) => format!("{} answered in a median {}", fmt::count(n as u64, "question from the agent", "questions from the agent"), duration(a)),
            (n, None) => fmt::count(n as u64, "question from the agent", "questions from the agent"),
        };
        out.push(format!(
            "stops     {}, {}, {questions}",
            fmt::count(self.interrupts as u64, "interrupt", "interrupts"),
            fmt::count(self.typed_while_working as u64, "message typed while the agent worked", "messages typed while the agent worked"),
        ));
        out
    }

    /// The median over breaks of the share of the break in which some agent worked, in percent.
    fn worked_share(&self) -> Option<f64> {
        let mut shares: Vec<f64> = self.breaks.iter().map(|b| 100.0 * b.worked as f64 / (b.to - b.from).max(1) as f64).collect();
        shares.sort_by(f64::total_cmp);
        shares.get(shares.len() / 2).copied()
    }

    /// The table by hour of the day: messages, turn ends and the median answer of each hour that
    /// has a message or an end, from `first` on.
    pub fn hours(&self, first: usize, offset: Offset) -> Vec<Vec<String>> {
        let hour = |ts: i64| ((ts + offset(ts)).rem_euclid(DAY) / HOUR) as usize;
        let mut messages = [0usize; 24];
        let mut ends = [0usize; 24];
        let mut answers: Vec<Vec<i64>> = vec![Vec::new(); 24];
        self.messages.iter().for_each(|&ts| messages[hour(ts)] += 1);
        self.ends.iter().for_each(|&(ts, _, _)| ends[hour(ts)] += 1);
        self.answers.iter().for_each(|&(ts, a)| answers[hour(ts)].push(a));
        let mut rows = vec![vec!["hour".to_string(), "messages".to_string(), "turn ends".to_string(), "median answer".to_string()]];
        for h in (0..24).map(|k| (first + k) % 24) {
            if messages[h] + ends[h] == 0 {
                continue;
            }
            rows.push(vec![format!("{h:02}"), messages[h].to_string(), ends[h].to_string(), median(&answers[h]).map_or("–".to_string(), duration)]);
        }
        rows
    }
}

/// The thresholds of the rhythm advice: what a warm cache would have saved on the cold returns,
/// the median wait of the agent's questions, the share of a median break the agents worked, and
/// both the median turn and the time between turn ends while the user was there.
pub const ADVICE_SAVED_USD: f64 = 10.0;
pub const ADVICE_QUESTION_MS: i64 = 10 * MINUTE;
pub const ADVICE_WORKED_PERCENT: f64 = 30.0;
pub const ADVICE_STEP_MS: i64 = 5 * MINUTE;

/// The cold returns of the time shown, as the pause report finds them: how many, what a warm
/// cache would have saved on them, the breaks before them, and whether they had prices.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Returns {
    pub count: usize,
    pub saved_usd: f64,
    pub pauses_ms: Vec<i64>,
    pub priced: bool,
}

impl Returns {
    pub fn of<'a>(returns: impl IntoIterator<Item = &'a ColdReturn>, priced: bool) -> Returns {
        let mut out = Returns { priced, ..Returns::default() };
        for r in returns {
            out.count += 1;
            out.saved_usd += r.premium_usd;
            out.pauses_ms.push(r.pause_ms);
        }
        out
    }

    /// The cold returns of all agents of a pause report in `[from, to)`.
    pub fn within(p: &Pauses, (from, to): (i64, i64)) -> Returns {
        let returns = p.agents.values().flat_map(|a| &a.returns).filter(|r| r.ts_ms >= from && r.ts_ms < to);
        Returns::of(returns, p.agents.values().any(|a| a.priced))
    }
}

impl Summary {
    /// The advice over this stretch of time, in the order its items come: cold returns,
    /// questions, out of work, step by step. `files` are the instruction files of the agents
    /// shown.
    pub fn advice(&self, returns: &Returns, files: &[String]) -> Vec<String> {
        let mut out = Vec::new();
        if returns.priced && returns.saved_usd >= ADVICE_SAVED_USD {
            let breaks = match (returns.count, median(&returns.pauses_ms)) {
                (1, Some(p)) => format!("; its break was {}", duration(p)),
                (_, Some(p)) => format!("; their breaks had a median of {}", duration(p)),
                _ => String::new(),
            };
            let n = fmt::count(returns.count as u64, "return", "returns");
            out.push(format!(
                "{n} after the cache expired cost {} that a warm cache would have saved{breaks}. Come back before the cache expires, or before a longer break run /compact or start the next session from a handoff (tokprof handoff).",
                fmt::usd(returns.saved_usd)
            ));
        }
        if let Some(wait) = median(&self.question_answers).filter(|w| *w > ADVICE_QUESTION_MS) {
            let file = if files.is_empty() { "the agent's instructions".to_string() } else { files.join(" or ") };
            out.push(format!(
                "The agent's questions waited a median {} for your answer ({}). Answer them as they come, or let the agent decide small things: add \"when a choice is small, pick one, say which, and go on\" to {file}.",
                duration(wait),
                fmt::count(self.questions as u64, "question", "questions")
            ));
        }
        let still = median(&self.breaks.iter().map(|b| b.still_after).collect::<Vec<_>>());
        let length = median(&self.breaks.iter().map(|b| b.to - b.from).collect::<Vec<_>>());
        if let (Some(still), Some(length), Some(share)) = (still, length, self.worked_share())
            && share < ADVICE_WORKED_PERCENT
        {
            out.push(format!(
                "After you left, the agents stood still a median {} into a median {} break and worked through {share:.0}% of it. Before a break, give each session its next steps too, or a plan it can work through without asking.",
                duration(still),
                duration(length)
            ));
        }
        let present = self.ends.iter().filter(|e| e.1).count() as i64;
        if let Some(turn) = median(&self.turns)
            && present > 0
            && turn < ADVICE_STEP_MS
            && self.keyboard_ms() / present < ADVICE_STEP_MS
        {
            out.push(format!(
                "While you were there, an agent ended a turn every {}, after turns of a median {}: you led it step by step. Give it a goal and a way to check it, not a step, and leave between its turns.",
                duration(self.keyboard_ms() / present),
                duration(turn)
            ));
        }
        out
    }
}

/// The spans merged where they overlap, in time order.
fn union(spans: impl Iterator<Item = (i64, i64)>) -> Vec<(i64, i64)> {
    let mut v: Vec<(i64, i64)> = spans.filter(|(a, b)| b > a).collect();
    v.sort_unstable();
    let mut out: Vec<(i64, i64)> = Vec::new();
    for (a, b) in v {
        match out.last_mut() {
            Some(last) if a <= last.1 => last.1 = last.1.max(b),
            _ => out.push((a, b)),
        }
    }
    out
}

/// The upper of the two middle values.
pub fn median(v: &[i64]) -> Option<i64> {
    let mut v = v.to_vec();
    v.sort_unstable();
    v.get(v.len() / 2).copied()
}

/// A duration as the X-ray's span shows it, with one decimal from one to ten minutes.
pub fn duration(ms: i64) -> String {
    let s = ms as f64 / 1000.0;
    if (60.0..600.0).contains(&s) {
        format!("{:.1}m", s / 60.0)
    } else {
        fmt::human_duration(s)
    }
}

/// The line on the sessions left out, if any were.
pub fn left_out_line(n: usize) -> Option<String> {
    (n > 0).then(|| format!("left out  {} that nobody typed in: a program ran them, or they ran in a temporary folder", fmt::count(n as u64, "session", "sessions")))
}

/// The sessions a view read, with where their days start.
#[derive(Clone, Debug)]
pub struct View {
    pub days: Days,
    pub timelines: Vec<Timeline>,
    pub lanes: Vec<Lane>,
}

impl View {
    /// The view of these timelines, with the days found from the actions of the sessions it
    /// keeps, as of `now_ms`.
    pub fn new(timelines: Vec<Timeline>, now_ms: i64, offset: Offset) -> View {
        let lanes = timelines.iter().map(Lane::of).collect();
        let mut view = View { days: Days::default(), timelines, lanes };
        view.days = Days::of(view.kept().flat_map(|(t, _)| t.actions()), now_ms, offset);
        view
    }

    fn kept(&self) -> impl Iterator<Item = (&Timeline, &Lane)> {
        self.timelines.iter().zip(&self.lanes).filter(|(t, _)| !t.left_out)
    }

    /// The sessions left out that have an action in `[from, to)`.
    pub fn left_out(&self, (from, to): (i64, i64)) -> usize {
        self.timelines.iter().filter(|t| t.left_out && t.actions().any(|ts| ts >= from && ts < to)).count()
    }

    pub fn summary(&self, window: (i64, i64)) -> Summary {
        let lanes: Vec<&Lane> = self.kept().map(|(_, l)| l).collect();
        Summary::of(&lanes, window, &self.days)
    }

    /// The instruction files of the agents of the sessions shown: those kept with an action of
    /// the user in `[from, to)`. Each file comes once, in the order of the agents' names.
    pub fn instruction_files(&self, (from, to): (i64, i64)) -> Vec<String> {
        let mut agents: Vec<&AgentId> = self.kept().filter(|(t, _)| t.actions().any(|ts| ts >= from && ts < to)).map(|(t, _)| &t.agent).collect();
        agents.sort();
        agents.dedup();
        let mut files: Vec<String> = Vec::new();
        for f in agents.iter().flat_map(|a| &a.info().instruction_files) {
            if !files.contains(f) {
                files.push(f.clone());
            }
        }
        files
    }

    /// The strip of a day in a layout, over the 24 hours from `axis`, by default the whole hour
    /// at or before the day's start.
    pub fn strip(&self, summary: &Summary, window: (i64, i64), layout: Layout, axis: Option<i64>, style: &fmt::Style, offset: Offset) -> Option<Strip> {
        let pairs: Vec<(&Timeline, &Lane)> = self.kept().collect();
        Strip::of(&pairs, summary, window, &self.days, layout, axis, style, offset)
    }
}

/// The rhythm of one session, for the X-ray's `rhythm` line.
#[derive(Clone, Debug, PartialEq)]
pub struct SessionRhythm {
    pub turns: usize,
    pub work_ms: i64,
    pub turn_median: Option<i64>,
    pub answer_median: Option<i64>,
    pub self_started: usize,
    pub interrupts: usize,
    pub questions: usize,
}

impl SessionRhythm {
    /// The rhythm of a main session, with the days found from its own actions as of `now_ms`;
    /// `None` when it has no turn or nobody typed in it.
    pub fn of(s: &Session, now_ms: i64, offset: Offset) -> Option<SessionRhythm> {
        let t = Timeline::of(s, &[]);
        if t.left_out {
            return None;
        }
        let lane = Lane::of(&t);
        if lane.turns.is_empty() {
            return None;
        }
        let days = Days::of(t.actions(), now_ms, offset);
        let all = Summary::of(&[&lane], (i64::MIN, i64::MAX), &days);
        Some(SessionRhythm {
            turns: lane.turns.len(),
            work_ms: lane.work_ms(),
            turn_median: median(&all.turns),
            answer_median: median(&all.answers.iter().map(|a| a.1).collect::<Vec<_>>()),
            self_started: lane.turns.iter().filter(|t| t.by == Start::Wake).count(),
            interrupts: all.interrupts,
            questions: all.questions,
        })
    }

    /// The X-ray's line.
    pub fn line(&self) -> String {
        let mut parts = vec![format!("{}, {} of work", fmt::count(self.turns as u64, "turn", "turns"), duration(self.work_ms))];
        parts.extend(self.turn_median.map(|m| format!("the agent's turn median {}", duration(m))));
        parts.extend(self.answer_median.map(|m| format!("your answer median {}", duration(m))));
        if self.self_started > 0 {
            parts.push(format!("{} it started itself", self.self_started));
        }
        if self.interrupts > 0 {
            parts.push(fmt::count(self.interrupts as u64, "interrupt", "interrupts"));
        }
        if self.questions > 0 {
            parts.push(fmt::count(self.questions as u64, "question", "questions"));
        }
        format!("rhythm    {}", parts.join(" · "))
    }
}

/// What a column of a lane shows, from the least to the most telling.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Cell {
    Empty,
    Wait,
    Work,
    Question,
    Message,
    Interrupt,
    /// The user sat at the agents, in the `you` lane.
    Present,
}

impl Cell {
    const LEGEND: [(Cell, &'static str); 6] = [
        (Cell::Work, "agent working"),
        (Cell::Wait, "waiting for your answer"),
        (Cell::Message, "your message"),
        (Cell::Question, "its question"),
        (Cell::Interrupt, "you stopped it"),
        (Cell::Present, "you were there"),
    ];

    fn mark(self) -> char {
        match self {
            Cell::Empty => ' ',
            Cell::Wait => '░',
            Cell::Work => '█',
            Cell::Question => '?',
            Cell::Message => '*',
            Cell::Interrupt => '!',
            Cell::Present => '▓',
        }
    }

    /// The ANSI color of the mark: the 16 standard colors, which read on light and dark themes.
    fn color(self) -> &'static str {
        match self {
            Cell::Empty => "0",
            Cell::Wait => "33",
            Cell::Work => "32",
            Cell::Question => "1;35",
            Cell::Message => "1;36",
            Cell::Interrupt => "1;31",
            Cell::Present => "34",
        }
    }
}

/// A row of cells as text: runs of one kind take one color code, and trailing spaces go.
fn paint(cells: &[Cell], style: &fmt::Style) -> String {
    let end = cells.iter().rposition(|&c| c != Cell::Empty).map_or(0, |i| i + 1);
    let mut out = String::new();
    let mut i = 0;
    while i < end {
        let kind = cells[i];
        let run = cells[i..end].iter().take_while(|&&c| c == kind).count();
        let marks: String = std::iter::repeat_n(kind.mark(), run).collect();
        if kind == Cell::Empty {
            out.push_str(&marks);
        } else {
            out.push_str(&style.paint(kind.color(), &marks));
        }
        i += run;
    }
    out
}

/// The day as a strip: an hours row, the user's sittings and a lane per session.
#[derive(Clone, Debug, PartialEq)]
pub struct Strip {
    pub lines: Vec<String>,
}

impl Strip {
    /// The strip of a day `[from, to)`, or `None` when the user did not act in it. `lanes` pairs
    /// each timeline with its lane. It covers 24 hours from `axis`, by default the whole hour at
    /// or before `from`, in the shortest column that fits `columns`.
    #[allow(clippy::too_many_arguments)]
    pub fn of(lanes: &[(&Timeline, &Lane)], summary: &Summary, (from, to): (i64, i64), days: &Days, layout: Layout, axis: Option<i64>, style: &fmt::Style, offset: Offset) -> Option<Strip> {
        let (label_width, columns) = (layout.label, layout.columns);
        summary.sittings.first()?;
        let local = |ts: i64| ts + offset(ts);
        let begin = axis.unwrap_or_else(|| from - local(from).rem_euclid(HOUR));
        let end = begin + DAY;
        let minutes = COLUMN_MINUTES.iter().copied().find(|m| DAY / (m * MINUTE) <= columns).unwrap_or(60);
        let col = minutes * MINUTE;
        let n = (DAY / col) as usize;
        let at = |ts: i64| ((ts - begin).div_euclid(col)).clamp(0, n as i64 - 1) as usize;
        let span = |row: &mut [Cell], a: i64, b: i64, c: Cell| {
            let (a, b) = (a.max(from), b.min(to));
            if b < a {
                return;
            }
            for cell in &mut row[at(a)..=at(b)] {
                if c > *cell {
                    *cell = c;
                }
            }
        };
        let mut hours = vec![' '; n];
        let mut last_label: Option<usize> = None;
        let mut h = begin;
        while h < end {
            let c = at(h);
            if last_label.is_none_or(|l| c >= l + 3) && c + 2 <= n {
                let label = format!("{:02}", local(h).rem_euclid(DAY) / HOUR);
                for (k, ch) in label.chars().enumerate() {
                    hours[c + k] = ch;
                }
                last_label = Some(c);
            }
            h += HOUR;
        }
        let mut lines = vec![format!("{:w$} {}", "", hours.iter().collect::<String>().trim_end(), w = label_width)];
        // A row of dim marks in the first column of each hour, between spaced lanes.
        let mut ticks = vec![' '; n];
        let mut h = begin;
        while h < end {
            ticks[at(h)] = '·';
            h += HOUR;
        }
        let ticks = format!("{:w$} {}", "", style.dim(ticks.iter().collect::<String>().trim_end()), w = label_width);
        let mut you = vec![Cell::Empty; n];
        for &(a, b) in &summary.sittings {
            span(&mut you, a, b, Cell::Present);
        }
        lines.push(format!("{:w$} {}", "you", paint(&you, style), w = label_width));
        let mut shown: Vec<(&Timeline, &Lane, usize)> = lanes
            .iter()
            .map(|(t, l)| (*t, *l, l.actions.iter().filter(|(ts, _)| *ts >= from && *ts < to).count()))
            .filter(|(_, _, n)| *n > 0)
            .collect();
        shown.sort_by(|a, b| b.2.cmp(&a.2).then_with(|| a.0.id.cmp(&b.0.id)));
        // Lanes fill the rows, the most active first; the last row counts the rest.
        let fit = if shown.len() <= layout.rows { shown.len() } else { layout.rows.saturating_sub(1).max(1) };
        let spaced = layout.space && fit == shown.len() && 2 * fit <= layout.rows;
        for (t, l, _) in shown.iter().take(fit) {
            let mut row = vec![Cell::Empty; n];
            for &(end, msg) in &l.answers {
                if days.day_of(end) == days.day_of(msg) {
                    span(&mut row, end, msg, Cell::Wait);
                }
            }
            for &(a, b) in &l.work {
                span(&mut row, a, b, Cell::Work);
            }
            for &(ask, _) in &l.questions {
                span(&mut row, ask, ask, Cell::Question);
            }
            for &(ts, e) in &l.actions {
                let c = if e == Event::Interrupt { Cell::Interrupt } else if e.is_message() { Cell::Message } else { continue };
                span(&mut row, ts, ts, c);
            }
            let id: String = t.id.chars().take(6).collect();
            let label = fmt::clip_end(&format!("{id} {}", t.title.as_deref().unwrap_or("")), label_width);
            lines.push(format!("{label:label_width$} {}", paint(&row, style)));
            if spaced {
                lines.push(ticks.clone());
            }
        }
        if shown.len() > fit {
            lines.push(fmt::count((shown.len() - fit) as u64, "more session", "more sessions"));
        }
        lines.push(Strip::legend(style));
        Some(Strip { lines })
    }

    /// What each mark means, each in its color.
    pub fn legend(style: &fmt::Style) -> String {
        Cell::LEGEND.iter().map(|(c, what)| format!("{} {what}", style.paint(c.color(), &c.mark().to_string()))).collect::<Vec<_>>().join("   ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 2026-11-02 00:00 in a zone one hour east of UTC, as the tests read local time.
    const MIDNIGHT: i64 = 1_793_574_000_000;
    const CET: i64 = HOUR;

    fn off(_: i64) -> i64 {
        CET
    }

    fn t(h: i64, m: i64) -> i64 {
        MIDNIGHT + h * HOUR + m * MINUTE
    }

    fn timeline(id: &str, events: &[(i64, Event)]) -> Timeline {
        let mut events = events.to_vec();
        events.sort_by_key(|&(ts, e)| (ts, e.rank()));
        Timeline { agent: crate::testing::claude(), id: id.into(), title: None, left_out: false, events }
    }

    /// Days that start at 05:00 from 10-30 to 11-05.
    fn night() -> Days {
        Days {
            starts: (-3..=3)
                .map(|k| {
                    let at = t(5, 0) + k * DAY;
                    DayStart { date: nov2() + k, at, night: Some((at - 3 * HOUR, at + 3 * HOUR)) }
                })
                .collect(),
        }
    }

    fn summary(lanes: &[Lane]) -> Summary {
        let refs: Vec<&Lane> = lanes.iter().collect();
        Summary::of(&refs, (i64::MIN, i64::MAX), &night())
    }

    use Event::*;

    #[test]
    fn a_turn_the_user_started() {
        let lane = Lane::of(&timeline("a", &[(t(10, 0), Typed), (t(10, 1), Request), (t(10, 3), Request), (t(10, 4), TurnEnd)]));
        assert_eq!(lane.turns, [Turn { start: t(10, 0), end: t(10, 4), by: Start::Typed, how: End::Logged }]);
        assert_eq!(summary(&[lane]).turns, [4 * MINUTE]);
    }

    #[test]
    fn a_turn_the_agent_started_itself_and_a_wake_during_a_turn() {
        let lane = Lane::of(&timeline(
            "a",
            &[(t(10, 0), Typed), (t(10, 1), Request), (t(10, 2), Wake), (t(10, 3), Request), (t(10, 4), TurnEnd), (t(10, 30), Wake), (t(10, 31), Request), (t(10, 32), TurnEnd)],
        ));
        let by: Vec<Start> = lane.turns.iter().map(|t| t.by).collect();
        assert_eq!(by, [Start::Typed, Start::Wake], "the wake at 10:02 starts no turn");
        let s = summary(&[lane]);
        assert_eq!(s.ends.iter().filter(|e| e.2).count(), 1);
    }

    #[test]
    fn an_end_that_is_not_logged_ends_at_the_last_request() {
        let lane = Lane::of(&timeline("a", &[(t(10, 0), Typed), (t(10, 20), Request), (t(10, 30), Typed), (t(10, 31), TurnEnd)]));
        assert_eq!(lane.turns[0], Turn { start: t(10, 0), end: t(10, 20), by: Start::Typed, how: End::Unlogged });
        assert_eq!(lane.turns[1].start, t(10, 30));
        assert_eq!(lane.answers, [(t(10, 20), t(10, 30))]);
    }

    #[test]
    fn silences_and_questions_are_not_work() {
        let silent = Lane::of(&timeline("a", &[(t(10, 0), Typed), (t(10, 0), Request), (t(10, 2), Request), (t(10, 40), Request), (t(10, 45), TurnEnd)]));
        assert_eq!(silent.work, [(t(10, 0), t(10, 2)), (t(10, 40), t(10, 45))]);
        assert_eq!(silent.work_ms(), 7 * MINUTE);
        let asked = Lane::of(&timeline(
            "a",
            &[(t(10, 0), Typed), (t(10, 1), Request), (t(10, 5), Question), (t(10, 12), Answer), (t(10, 13), Request), (t(10, 15), TurnEnd)],
        ));
        assert_eq!(asked.work, [(t(10, 0), t(10, 5)), (t(10, 12), t(10, 15))]);
        assert_eq!(asked.questions, [(t(10, 5), Some(t(10, 12)))]);
        // A long wait for the answer is no silence either: the work after it starts at the answer.
        let slow = Lane::of(&timeline("a", &[(t(10, 0), Typed), (t(10, 1), Question), (t(10, 40), Answer), (t(10, 41), TurnEnd)]));
        assert_eq!(slow.work, [(t(10, 0), t(10, 1)), (t(10, 40), t(10, 41))]);
    }

    #[test]
    fn answers_count_within_a_day() {
        let lane = Lane::of(&timeline(
            "a",
            &[(t(10, 0), Typed), (t(10, 4), TurnEnd), (t(10, 7), Typed), (t(23, 30), TurnEnd), (t(34, 0), Typed), (t(34, 5), TurnEnd)],
        ));
        let s = summary(&[lane]);
        assert_eq!(s.answers, [(t(10, 7), 3 * MINUTE)], "the answer the next morning is no answer");
        assert_eq!(duration(3 * MINUTE), "3.0m");
    }

    #[test]
    fn a_wake_does_not_move_the_start_of_the_answer_but_its_end_does() {
        let lane = Lane::of(&timeline("a", &[(t(10, 0), Typed), (t(10, 4), TurnEnd), (t(10, 10), Wake), (t(10, 11), TurnEnd), (t(10, 15), Typed)]));
        assert_eq!(lane.answers, [(t(10, 11), t(10, 15))]);
    }

    #[test]
    fn an_interrupt_is_answered_by_no_message() {
        let lane = Lane::of(&timeline("a", &[(t(10, 0), Typed), (t(10, 2), Interrupt), (t(10, 3), Typed)]));
        assert_eq!(lane.turns[0].how, End::Interrupted);
        assert!(lane.answers.is_empty());
        // Claude Code writes `No response requested.` as an end right after the interrupt.
        let lane = Lane::of(&timeline("a", &[(t(10, 0), Typed), (t(10, 2), Interrupt), (t(10, 2), TurnEnd), (t(10, 3), Typed)]));
        assert_eq!(lane.turns.len(), 2);
        assert!(lane.answers.is_empty(), "the end after an interrupt is answered by no message");
    }

    #[test]
    fn sittings_and_breaks() {
        let a = Lane::of(&timeline("a", &[(t(10, 0), Typed), (t(10, 5), Typed), (t(10, 14), Typed)]));
        let b = Lane::of(&timeline("b", &[(t(10, 40), Typed)]));
        let s = summary(&[a, b]);
        assert_eq!(s.sittings, [(t(10, 0), t(10, 14)), (t(10, 40), t(10, 40))]);
        assert_eq!(s.breaks.len(), 1);
        assert_eq!(s.breaks[0].to - s.breaks[0].from, 26 * MINUTE);
        assert_eq!(s.keyboard_ms(), 15 * MINUTE, "the one-message sitting lasts a minute");
        let edge = Lane::of(&timeline("a", &[(t(10, 0), Typed), (t(10, 15), Typed)]));
        assert_eq!(summary(&[edge]).sittings.len(), 2, "15 minutes apart is two sittings");
    }

    #[test]
    fn agents_that_ran_out_of_work() {
        let you = Lane::of(&timeline("a", &[(t(9, 50), Typed), (t(9, 52), TurnEnd), (t(10, 0), Typed), (t(10, 6), TurnEnd), (t(10, 40), Typed)]));
        let other = Lane::of(&timeline("b", &[(t(10, 20), Wake), (t(10, 24), TurnEnd)]));
        let s = summary(&[you, other]);
        assert_eq!(s.breaks, [Break { from: t(10, 0), to: t(10, 40), still_after: 6 * MINUTE, worked: 10 * MINUTE }]);
        assert!(s.lines()[1].contains("stood still a median 6.0m after you left, and worked through 25% of a median break"), "{}", s.lines()[1]);
    }

    /// Days from `actions`, as of `now`.
    fn days(actions: &[i64], now: i64) -> Days {
        Days::of(actions.iter().copied(), now, &off)
    }

    fn start_of(d: &Days, date: i64) -> Option<DayStart> {
        d.starts.iter().copied().find(|s| s.date == date)
    }

    /// The local date of 2026-11-02, days since 1970-01-01.
    fn nov2() -> i64 {
        date_of(t(12, 0), &off)
    }

    #[test]
    fn the_authors_night() {
        // The evening of 11-01, the night to 08:20, the day, and the next morning.
        let d = days(&[t(-3, 0), t(0, 22), t(8, 20), t(12, 0), t(22, 26), t(34, 3)], t(40, 0));
        let s = start_of(&d, nov2()).unwrap();
        assert_eq!((s.at, s.night), (t(4, 21), Some((t(0, 22), t(8, 20)))));
        assert_eq!(d.bounds(nov2(), &off), (t(4, 21), t(28, 14) + 30_000), "it ends in the middle of the next night");
        assert_eq!(d.line(nov2(), &off), "night     before the day 00:22–08:20 (8.0h), after it 22:26–10:03 (11.6h); a day runs from the middle of one night to the middle of the next");
    }

    #[test]
    fn a_pause_in_the_afternoon_is_no_night() {
        let d = days(&[t(-4, 0), t(1, 0), t(8, 0), t(10, 0), t(18, 0), t(20, 0)], t(40, 0));
        assert_eq!(start_of(&d, nov2()).unwrap().at, t(4, 30), "the pause from 10:00 to 18:00 has its middle at 14:00");
    }

    #[test]
    fn a_night_that_is_not_empty() {
        // Breaks of 30 minutes through the night, and one of 40 from 03:10 to 03:50.
        let mut acts: Vec<i64> = (0..=18).map(|k| t(-6, 0) + k * 30 * MINUTE).filter(|&ts| ts <= t(3, 10)).collect();
        acts.push(t(3, 10));
        acts.extend((0..17).map(|k| t(3, 50) + k * 30 * MINUTE));
        let d = days(&acts, t(40, 0));
        assert_eq!(start_of(&d, nov2()).unwrap().at, t(3, 30));
    }

    #[test]
    fn too_few_days_start_at_midnight() {
        // An action on 10-31 at 10:00 and the next on 11-03 at 10:00: the pause's middle, 11-01
        // 22:00, is the night of 11-02, and 11-01 has none.
        let d = days(&[t(-38, 0), t(34, 0)], t(60, 0));
        let nov1 = nov2() - 1;
        assert_eq!(start_of(&d, nov1), Some(DayStart { date: nov1, at: t(-24, 0), night: None }));
        assert_eq!(d.line(nov1, &off), "night     no pause before the day; it starts at midnight");
        assert_eq!(start_of(&d, nov2()).unwrap().at, t(-2, 0));
        assert_eq!(d.line(nov2(), &off), "night     before the day 10:00–10:00 (3.0d); the next day starts at midnight");
    }

    #[test]
    fn a_day_with_one_action() {
        // 10-30 17:00, then 10-31 22:00 alone, then 11-01 09:00.
        let d = days(&[t(-55, 0), t(-26, 0), t(-15, 0)], t(40, 0));
        let oct31 = nov2() - 2;
        assert_eq!(start_of(&d, oct31).unwrap().at, t(-55, 0) + (t(-26, 0) - t(-55, 0)) / 2, "07:30, the middle of the pause that ends at 22:00");
    }

    #[test]
    fn the_current_day_runs_until_now() {
        let d = days(&[t(0, 22), t(8, 20), t(22, 26)], t(23, 0));
        assert_eq!(d.latest(), Some(nov2()));
        assert_eq!(d.bounds(nov2(), &off), (t(4, 21), i64::MAX));
        assert_eq!(d.line(nov2(), &off), "night     before the day 00:22–08:20 (8.0h); the day runs until now");
    }

    #[test]
    fn before_the_morning_the_latest_action_is_still_yesterdays() {
        // At 02:10 on 11-03, after a break from 19:00 to 20:30 and an action at 01:50: no pause of
        // 4 hours yet, so no day of 11-03.
        let d = days(&[t(0, 22), t(8, 20), t(19, 0), t(20, 30), t(23, 0), t(25, 50)], t(26, 10));
        assert_eq!(d.latest(), Some(nov2()));
        assert_eq!(d.day_of(t(25, 50)), Some(nov2()));
        // Once 4 hours have gone by, a long pause counts though the window is still open.
        let d = days(&[t(0, 22), t(8, 20), t(19, 0), t(25, 50), t(33, 55)], t(34, 0));
        assert_eq!(d.latest(), Some(nov2() + 1));
    }

    #[test]
    fn days_run_from_start_to_start() {
        let d = night();
        assert_eq!(d.day_of(t(4, 59)).unwrap() + 1, d.day_of(t(5, 0)).unwrap());
        assert_eq!(d.day_of(t(5, 0)), d.day_of(t(28, 59)), "02:10 the next night is the same day");
        assert_eq!(d.bounds(nov2(), &off), (t(5, 0), t(29, 0)));
        assert_eq!(d.bounds(nov2() + 30, &off), (t(30 * 24, 0), t(31 * 24, 0)), "a date with no day runs from midnight to midnight");
    }

    #[test]
    fn the_nights_line_gives_the_median_start() {
        assert_eq!(night().nights_line((i64::MIN, i64::MAX), &off), "nights    a day runs from the middle of one night to the middle of the next; days started at a median 05:00");
        assert_eq!(Days::default().nights_line((i64::MIN, i64::MAX), &off), "nights    no night in this time; days start at midnight");
        // A start in the evening before counts before midnight, not after.
        let late = Days { starts: [t(-1, 30), t(24, 30), t(49, 30)].iter().enumerate().map(|(k, &at)| DayStart { date: nov2() + k as i64, at, night: Some((at - HOUR, at + HOUR)) }).collect() };
        assert_eq!(late.median_start((i64::MIN, i64::MAX), &off), Some(30));
    }

    #[test]
    fn sessions_left_out() {
        let mut s = Session::new(crate::testing::claude(), "x".into());
        s.cwd = Some("/private/var/folders/19/abc/T/cmsg-acp-Hr88x0".into());
        assert!(Timeline::of(&s, &["/private/var/folders/19/abc/T/".to_string()]).left_out);
        s.cwd = Some("/Users/me/src/app".into());
        assert!(!Timeline::of(&s, &["/private/var/folders/19/abc/T/".to_string()]).left_out);
        s.scripted = true;
        assert!(Timeline::of(&s, &[]).left_out, "print mode");
        assert_eq!(left_out_line(92).unwrap(), "left out  92 sessions that nobody typed in: a program ran them, or they ran in a temporary folder");
        assert_eq!(left_out_line(0), None);
    }

    fn spans(n: usize, minutes: i64, gap: i64) -> Vec<(i64, i64)> {
        (0..n as i64).map(|k| (t(8, 0) + k * (minutes + gap) * MINUTE, t(8, 0) + (k * (minutes + gap) + minutes) * MINUTE)).collect()
    }

    #[test]
    fn the_sittings_line() {
        let mut sittings = spans(15, 6, 28);
        sittings.push((t(20, 0), t(22, 12)));
        let s = Summary {
            breaks: (0..15).map(|_| Break { from: 0, to: 28 * MINUTE, still_after: 0, worked: 0 }).collect(),
            sittings,
            messages: vec![0; 58],
            sessions: 11,
            ..Summary::default()
        };
        assert_eq!(s.lines()[0], "sittings  16, 3.7h at the keyboard, 58 messages to 11 sessions; median sitting 6.0m, median break 28m");
        let one = Summary { sittings: spans(1, 6, 0), messages: vec![0; 2], sessions: 1, ..Summary::default() };
        assert_eq!(one.lines()[0], "sittings  1, 6.0m at the keyboard, 2 messages to 1 session; median sitting 6.0m, no break");
        assert_eq!(one.lines()[1], "left      no break between sittings");
    }

    #[test]
    fn the_left_present_answers_and_stops_lines() {
        let s = Summary {
            sittings: vec![(0, 75 * HOUR)],
            breaks: vec![Break { from: 0, to: 100 * MINUTE, still_after: 8 * MINUTE, worked: 28 * MINUTE }],
            ends: (0..1936).map(|k| (k, k < 999, k < 717)).collect(),
            turns: vec![3 * MINUTE + 30_000],
            answers: vec![(0, 3 * MINUTE + 42_000)],
            interrupts: 39,
            typed_while_working: 46,
            questions: 74,
            question_answers: vec![2 * MINUTE + 18_000],
            ..Summary::default()
        };
        let lines = s.lines();
        assert_eq!(lines[1], "left      the agents stood still a median 8.0m after you left, and worked through 28% of a median break");
        assert_eq!(lines[2], "present   an agent ended a turn every 4.5m while you were there; it started 37% of its turns itself");
        assert_eq!(lines[3], "answers   the agent's turn took a median 3.5m; you answered in a median 3.7m");
        assert_eq!(lines[4], "stops     39 interrupts, 46 messages typed while the agent worked, 74 questions from the agent answered in a median 2.3m");
        let quiet = Summary { sittings: vec![(0, MINUTE)], interrupts: 1, typed_while_working: 1, ..Summary::default() };
        let lines = quiet.lines();
        assert_eq!(lines[2], "present   no turn ended while you were there; it started 0% of its turns itself");
        assert_eq!(lines[3], "answers   no turns you started; no answers");
        assert_eq!(lines[4], "stops     1 interrupt, 1 message typed while the agent worked, no questions from the agent");
    }

    #[test]
    fn durations() {
        assert_eq!(duration(23_000), "23s");
        assert_eq!(duration(3 * MINUTE + 30_000), "3.5m");
        assert_eq!(duration(28 * MINUTE), "28m");
        assert_eq!(duration(222 * MINUTE), "3.7h");
        assert_eq!(median(&[1, 2, 3, 4]), Some(3), "the upper of two middle values");
    }

    #[test]
    fn the_session_line() {
        let r = SessionRhythm { turns: 34, work_ms: 126 * MINUTE, turn_median: Some(210_000), answer_median: Some(4 * MINUTE), self_started: 12, interrupts: 2, questions: 0 };
        assert_eq!(r.line(), "rhythm    34 turns, 2.1h of work · the agent's turn median 3.5m · your answer median 4.0m · 12 it started itself · 2 interrupts");
        let bare = SessionRhythm { turns: 1, work_ms: 60_000, turn_median: None, answer_median: None, self_started: 0, interrupts: 0, questions: 0 };
        assert_eq!(bare.line(), "rhythm    1 turn, 1.0m of work");
    }

    #[test]
    fn a_session_without_turns_has_no_line() {
        let s = Session::new(crate::testing::claude(), "x".into());
        assert_eq!(SessionRhythm::of(&s, t(12, 0), &off), None);
    }

    #[test]
    fn the_hours_table_runs_from_the_start_of_the_day() {
        let lane = Lane::of(&timeline("a", &[(t(9, 0), Typed), (t(9, 4), TurnEnd), (t(9, 10), Typed), (t(9, 12), TurnEnd), (t(26, 0), Typed)]));
        let s = summary(&[lane]);
        let rows = s.hours(5, &off);
        assert_eq!(rows[0], ["hour", "messages", "turn ends", "median answer"]);
        assert_eq!(rows[1], ["09", "2", "2", "6.0m"]);
        assert_eq!(rows[2], ["02", "1", "1", "16.8h"], "02:00 the next night is the same day, after the evening");
        let none = summary(&[Lane::of(&timeline("a", &[(t(9, 0), Typed), (t(9, 4), TurnEnd)]))]);
        assert_eq!(none.hours(5, &off)[1], ["09", "1", "1", "–"]);
    }

    fn strip(lanes: &[(Timeline, Lane)]) -> Vec<String> {
        strip_in(lanes, 72, false)
    }

    fn strip_in(lanes: &[(Timeline, Lane)], columns: i64, color: bool) -> Vec<String> {
        let pairs: Vec<(&Timeline, &Lane)> = lanes.iter().map(|(t, l)| (t, l)).collect();
        let refs: Vec<&Lane> = lanes.iter().map(|(_, l)| l).collect();
        let day = night().bounds(nov2(), &off);
        let s = Summary::of(&refs, day, &night());
        let layout = Layout { columns, ..Layout::TERMINAL };
        Strip::of(&pairs, &s, day, &night(), layout, None, &fmt::Style::with_color(color), &off).unwrap().lines
    }

    fn lane(id: &str, title: &str, events: &[(i64, Event)]) -> (Timeline, Lane) {
        let mut tl = timeline(id, events);
        tl.title = Some(title.into());
        let l = Lane::of(&tl);
        (tl, l)
    }

    fn marked() -> (Timeline, Lane) {
        lane(
            "ecad11b9-0000",
            "Инфографика прерываний",
            &[
                (t(8, 20), Typed),
                (t(8, 30), Request),
                (t(8, 40), Request),
                (t(8, 50), TurnEnd),
                (t(9, 30), Typed),
                (t(9, 31), Question),
                (t(10, 10), Answer),
                (t(10, 20), Interrupt),
                (t(22, 26), Typed),
            ],
        )
    }

    #[test]
    fn the_strip_covers_24_hours_from_the_days_start() {
        let lines = strip(&[marked()]);
        // The day starts at 05:00: 24 hours in 72 columns of 20 minutes, three to an hour.
        assert_eq!(lines[0], format!("{:20} 05 06 07 08 09 10 11 12 13 14 15 16 17 18 19 20 21 22 23 00 01 02 03 04", ""));
        let row = &lines[2];
        assert!(row.starts_with("ecad11 Инфографика…  "), "{row}");
        let cells: Vec<char> = row.chars().skip(21).collect();
        assert_eq!(&cells[..10], &[' '; 10]);
        assert_eq!(cells[10], '*', "the message at 08:20");
        assert_eq!(cells[11], '█', "work until 08:50");
        assert_eq!(cells[12], '░', "waiting for the answer");
        assert_eq!(cells[13], '*', "09:20–09:40: the message wins over the question");
        assert_eq!(cells[14], ' ', "09:40–10:00 waits for the answer to its question");
        assert_eq!(cells[15], '█', "10:00–10:20 works again after the answer at 10:10");
        assert_eq!(cells[16], '!', "the interrupt at 10:20");
        assert_eq!(cells.len(), 53, "the row ends at the message at 22:26");
        assert!(lines[1].starts_with(&format!("{:20} {}", "you", " ".repeat(10))) && lines[1].contains('▓'), "{}", lines[1]);
        assert_eq!(lines.last().unwrap(), "█ agent working   ░ waiting for your answer   * your message   ? its question   ! you stopped it   ▓ you were there");
    }

    #[test]
    fn a_narrow_pane_takes_wider_columns() {
        let lines = strip_in(&[lane("a", "t", &[(t(8, 20), Typed), (t(22, 26), Typed)])], 50, false);
        // 48 columns of 30 minutes, an hour label every other hour.
        assert_eq!(lines[0], format!("{:20} 05  07  09  11  13  15  17  19  21  23  01  03", ""));
        let you: Vec<char> = lines[1].chars().skip(21).collect();
        assert_eq!(you.len(), 35, "the last sitting, at 22:26, is in column 35 of 48");
    }

    #[test]
    fn a_wide_screen_takes_ten_minute_columns() {
        let lines = strip_in(&[lane("a", "t", &[(t(8, 20), Typed)])], 179, false);
        assert!(lines[0].starts_with(&format!("{:20} 05    06    07", "")), "{}", lines[0]);
        assert_eq!(lines[0].trim_end().chars().count(), 21 + 23 * 6 + 2, "144 columns, the last label at 04");
    }

    #[test]
    fn the_axis_can_be_given() {
        let pairs_lanes = [lane("a", "t", &[(t(8, 20), Typed)])];
        let pairs: Vec<(&Timeline, &Lane)> = pairs_lanes.iter().map(|(t, l)| (t, l)).collect();
        let refs: Vec<&Lane> = pairs_lanes.iter().map(|(_, l)| l).collect();
        let day = night().bounds(nov2(), &off);
        let s = Summary::of(&refs, day, &night());
        let lines = Strip::of(&pairs, &s, day, &night(), Layout::TERMINAL, Some(t(4, 0)), &fmt::Style::with_color(false), &off).unwrap().lines;
        assert!(lines[0].starts_with(&format!("{:20} 04 05 06", "")), "{}", lines[0]);
    }

    #[test]
    fn marks_take_their_colors() {
        let lines = strip_in(&[marked()], 72, true);
        assert!(lines[2].contains("\x1b[1;36m*\x1b[0m\x1b[32m█\x1b[0m\x1b[33m░\x1b[0m"), "{:?}", lines[2]);
        assert!(lines[2].contains("\x1b[1;31m!\x1b[0m"), "{:?}", lines[2]);
        assert!(lines[1].contains("\x1b[34m▓"), "{:?}", lines[1]);
        assert!(lines.last().unwrap().starts_with("\x1b[32m█\x1b[0m agent working   \x1b[33m░\x1b[0m waiting"), "{:?}", lines.last());
        assert!(strip(&[marked()]).iter().all(|l| !l.contains('\x1b')), "no escape code without color");
    }

    fn strip_laid(lanes: &[(Timeline, Lane)], layout: Layout) -> Vec<String> {
        let pairs: Vec<(&Timeline, &Lane)> = lanes.iter().map(|(t, l)| (t, l)).collect();
        let refs: Vec<&Lane> = lanes.iter().map(|(_, l)| l).collect();
        let day = night().bounds(nov2(), &off);
        let s = Summary::of(&refs, day, &night());
        Strip::of(&pairs, &s, day, &night(), layout, None, &fmt::Style::with_color(false), &off).unwrap().lines
    }

    fn sessions(n: i64) -> Vec<(Timeline, Lane)> {
        (0..n).map(|k| lane(&format!("s{k:02}"), "a title that runs on for a while", &[(t(10, k), Typed)])).collect()
    }

    #[test]
    fn many_sessions_fold_into_one_line() {
        let lines = strip(&sessions(11));
        assert_eq!(lines.iter().filter(|l| l.starts_with('s')).count(), 8);
        assert_eq!(lines[lines.len() - 2], "3 more sessions");
        // With rows for 11 lanes, all of them show.
        let lines = strip_laid(&sessions(11), Layout { rows: 11, ..Layout::TERMINAL });
        assert_eq!(lines.iter().filter(|l| l.starts_with('s')).count(), 11);
        assert!(!lines.iter().any(|l| l.contains("more session")));
    }

    #[test]
    fn a_terminal_never_spaces_its_lanes() {
        let lines = strip(&sessions(3));
        assert!(!lines.iter().any(|l| l.contains('·')), "{lines:?}");
        assert_eq!(lines[2], format!("{:20} {}*", "s00 a title that ru…", " ".repeat(15)), "labels of 20 characters; 10:00 is column 15 of 20 minutes from 05:00");
    }

    #[test]
    fn spare_rows_space_the_lanes_with_hour_marks() {
        let layout = Layout { label: 38, columns: 149, rows: 32, space: true };
        let lines = strip_laid(&sessions(16), layout);
        let lanes: Vec<usize> = (0..lines.len()).filter(|&i| lines[i].starts_with("s")).collect();
        assert_eq!(lanes.len(), 16, "all 16 lanes in 32 rows");
        let ticks = &lines[lanes[0] + 1];
        assert!(ticks.starts_with(&format!("{:38} ·     ·", "")), "a mark in the first column of each hour: {ticks:?}");
        assert_eq!(ticks.matches('·').count(), 24);
        assert!(lines[lanes[0]].starts_with("s00 a title that runs on for a while   "), "labels of 38 characters: {}", lines[lanes[0]]);
        // Rows for fewer than twice the lanes: no marks.
        let tight = strip_laid(&sessions(16), Layout { rows: 20, ..layout });
        assert!(!tight.iter().any(|l| l.contains('·')));
    }

    const CLAUDE: &[&str] = &["CLAUDE.md"];

    fn files(names: &[&str]) -> Vec<String> {
        names.iter().map(|n| n.to_string()).collect()
    }

    fn cold(count: usize, saved_usd: f64) -> Returns {
        Returns { count, saved_usd, pauses_ms: vec![HOUR, 84 * MINUTE, 2 * HOUR], priced: true }
    }

    #[test]
    fn advice_on_cold_returns() {
        let none = Summary::default();
        assert_eq!(
            none.advice(&cold(23, 107.2), &files(CLAUDE)),
            ["23 returns after the cache expired cost $107 that a warm cache would have saved; their breaks had a median of 1.4h. Come back before the cache expires, or before a longer break run /compact or start the next session from a handoff (tokprof handoff)."]
        );
        let one = Returns { count: 1, saved_usd: 12.5, pauses_ms: vec![84 * MINUTE], priced: true };
        assert!(none.advice(&one, &[])[0].starts_with("1 return after the cache expired cost $12.50 that a warm cache would have saved; its break was 1.4h. "));
        assert!(none.advice(&cold(23, 9.99), &[]).is_empty(), "below $10");
        assert!(none.advice(&Returns { priced: false, ..cold(23, 107.2) }, &[]).is_empty(), "without prices");
    }

    #[test]
    fn advice_on_questions_that_waited() {
        let s = Summary { questions: 1, question_answers: vec![78 * MINUTE], ..Summary::default() };
        assert_eq!(
            s.advice(&Returns::default(), &files(CLAUDE)),
            ["The agent's questions waited a median 1.3h for your answer (1 question). Answer them as they come, or let the agent decide small things: add \"when a choice is small, pick one, say which, and go on\" to CLAUDE.md."]
        );
        assert!(s.advice(&Returns::default(), &files(&["CLAUDE.md", "AGENTS.md"]))[0].ends_with(" to CLAUDE.md or AGENTS.md."));
        let quick = Summary { questions: 3, question_answers: vec![10 * MINUTE; 3], ..Summary::default() };
        assert!(quick.advice(&Returns::default(), &files(CLAUDE)).is_empty(), "a median of 10 minutes");
    }

    #[test]
    fn advice_on_agents_out_of_work() {
        let worked = |share: i64| Summary {
            breaks: vec![Break { from: 0, to: 38 * MINUTE, still_after: 4 * MINUTE, worked: 38 * MINUTE * share / 100 }],
            ..Summary::default()
        };
        assert_eq!(
            worked(19).advice(&Returns::default(), &[]),
            ["After you left, the agents stood still a median 4.0m into a median 38m break and worked through 19% of it. Before a break, give each session its next steps too, or a plan it can work through without asking."]
        );
        assert!(worked(30).advice(&Returns::default(), &[]).is_empty(), "30%");
        assert!(Summary::default().advice(&Returns::default(), &[]).is_empty(), "no break");
    }

    #[test]
    fn advice_on_step_by_step() {
        // Ten turns end in 36 minutes at the keyboard: one every 3.6m.
        let steps = |turn: i64, sitting: i64| Summary {
            sittings: vec![(0, sitting)],
            ends: (0..10).map(|k| (k, true, false)).chain([(HOUR, false, false)]).collect(),
            turns: vec![turn; 10],
            ..Summary::default()
        };
        assert_eq!(
            steps(210_000, 36 * MINUTE).advice(&Returns::default(), &[]),
            ["While you were there, an agent ended a turn every 3.6m, after turns of a median 3.5m: you led it step by step. Give it a goal and a way to check it, not a step, and leave between its turns."]
        );
        assert!(steps(5 * MINUTE, 36 * MINUTE).advice(&Returns::default(), &[]).is_empty(), "a median turn of 5 minutes");
        assert!(steps(210_000, 50 * MINUTE).advice(&Returns::default(), &[]).is_empty(), "a turn ending every 5 minutes");
        assert!(Summary { ends: Vec::new(), ..steps(210_000, 36 * MINUTE) }.advice(&Returns::default(), &[]).is_empty(), "no turn ended while the user was there");
    }

    #[test]
    fn the_advice_comes_in_order() {
        let s = Summary {
            sittings: vec![(0, 36 * MINUTE), (74 * MINUTE, 75 * MINUTE)],
            breaks: vec![Break { from: 36 * MINUTE, to: 74 * MINUTE, still_after: 4 * MINUTE, worked: 4 * MINUTE }],
            ends: (0..10).map(|k| (k, true, false)).collect(),
            turns: vec![210_000],
            questions: 1,
            question_answers: vec![78 * MINUTE],
            ..Summary::default()
        };
        let starts: Vec<String> = s.advice(&cold(23, 107.2), &files(CLAUDE)).iter().map(|a| a.split(' ').take(3).collect::<Vec<_>>().join(" ")).collect();
        assert_eq!(starts, ["23 returns after", "The agent's questions", "After you left,", "While you were"]);
    }

    #[test]
    fn the_cold_returns_of_all_agents() {
        use crate::pauses::{AgentPauses, Starter};
        let ret = |ts_ms: i64, premium_usd: f64| ColdReturn { ts_ms, pause_ms: 84 * MINUTE, usd: premium_usd + 0.2, premium_usd, context: 0, starter: Starter::Typed };
        let mut p = Pauses::default();
        p.agents.insert(crate::testing::claude(), AgentPauses { returns: vec![ret(t(9, 0), 6.0), ret(t(23, 0), 50.0)], priced: true, ..AgentPauses::default() });
        p.agents.insert(crate::testing::codex(), AgentPauses { returns: vec![ret(t(10, 0), 5.0)], ..AgentPauses::default() });
        let r = Returns::within(&p, (t(0, 0), t(12, 0)));
        assert_eq!(r, Returns { count: 2, saved_usd: 11.0, pauses_ms: vec![84 * MINUTE; 2], priced: true });
    }

    #[test]
    fn the_instruction_files_of_the_agents_shown() {
        let codex = Timeline { agent: crate::testing::codex(), ..timeline("c", &[(t(10, 0), Typed), (t(10, 1), TurnEnd)]) };
        let claude = timeline("a", &[(t(11, 0), Typed), (t(11, 1), TurnEnd)]);
        let view = View::new(vec![codex, claude], t(12, 0), &off);
        assert_eq!(view.instruction_files((t(0, 0), t(12, 0))), ["CLAUDE.md", "AGENTS.md"], "in the order of the agents' names");
        assert_eq!(view.instruction_files((t(10, 30), t(12, 0))), ["CLAUDE.md"], "only the sessions with an action in the time");
        assert!(view.instruction_files((t(10, 0) + 1, t(10, 30))).is_empty(), "an end of turn is no action of the user");
    }
}
