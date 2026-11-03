//! The work rhythm: how the user and the agents hand work to each other. A main session becomes
//! a timeline of the user's actions, the agent's hand-offs and its requests; the rules turn the
//! timelines into turns, work, waits, sittings and breaks, and find where the user's day starts.
//!
//! Local time comes in as an offset function, so that the rules read no time zone themselves.

use std::collections::BTreeSet;

use crate::fmt;
use crate::model::{HandoffKind, Session};
use crate::source::AgentId;

const MINUTE: i64 = 60_000;
const HOUR: i64 = 60 * MINUTE;
const DAY: i64 = 24 * HOUR;
const SLOT: i64 = 30 * MINUTE;
const SLOTS: usize = 48;

/// Actions less than this apart are one sitting; a turn with no event for this long is silent.
pub const GAP_MS: i64 = 15 * MINUTE;
/// The start of the day is found from the actions of this many days before the end of what is
/// shown.
pub const NIGHT_DAYS: i64 = 30;
/// With actions on fewer days than this, the day starts at midnight.
pub const MIN_DAYS: usize = 7;
/// A sitting lasts at least this long.
const MIN_SITTING_MS: i64 = MINUTE;
/// The widest the day strip gets, in columns, and the column widths it picks from, in minutes.
pub const STRIP_COLUMNS: i64 = 72;
const COLUMN_MINUTES: [i64; 5] = [10, 15, 20, 30, 60];
/// Lanes the strip draws before it folds the rest into one line.
pub const STRIP_LANES: usize = 8;
const LABEL_WIDTH: usize = 20;

/// The offset of local time from UTC at an instant, both in ms.
pub type Offset<'a> = &'a dyn Fn(i64) -> i64;

/// The offset of the machine's time zone at an instant, in ms.
pub fn local_offset(ts_ms: i64) -> i64 {
    use chrono::Offset as _;
    chrono::DateTime::from_timestamp_millis(ts_ms)
        .map(|t| t.with_timezone(&chrono::Local).offset().fix().local_minus_utc() as i64 * 1000)
        .unwrap_or(0)
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

/// Where days start.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DayStart {
    /// In the middle of the night: `len` half hours from half hour `from` of the day, in which
    /// the user acted on `days` days.
    Night { from: usize, len: usize, days: usize },
    /// At midnight: the user acted on only this many days.
    Midnight { days: usize },
}

impl DayStart {
    /// Minutes after local midnight.
    pub fn minutes(self) -> i64 {
        match self {
            DayStart::Night { from, len, .. } => ((from + len / 2) % SLOTS) as i64 * 30,
            DayStart::Midnight { .. } => 0,
        }
    }

    /// The `night` line.
    pub fn line(self) -> String {
        match self {
            DayStart::Night { from, len, days } => {
                let at = |slot: usize| {
                    let m = (slot % SLOTS) as i64 * 30;
                    format!("{:02}:{:02}", m / 60, m % 60)
                };
                let then = match days {
                    0 => "with no message then".to_string(),
                    1 => "with messages then on 1 day".to_string(),
                    n => format!("with messages then on {n} days"),
                };
                format!("night     {}–{} in the last {NIGHT_DAYS} days, {then}; days start at {}", at(from), at(from + len), at(from + len / 2))
            }
            DayStart::Midnight { .. } => format!("night     messages on fewer than {MIN_DAYS} of the last {NIGHT_DAYS} days; days start at midnight"),
        }
    }

    /// The day an instant falls in, as a count of days since 1970-01-01 in local time.
    pub fn day_of(self, ts: i64, offset: Offset) -> i64 {
        (ts + offset(ts) - self.minutes() * MINUTE).div_euclid(DAY)
    }

    /// When a day starts and ends, in epoch ms.
    pub fn bounds(self, day: i64, offset: Offset) -> (i64, i64) {
        let utc = |local: i64| local - offset(local - offset(local));
        let start = |d: i64| utc(d * DAY + self.minutes() * MINUTE);
        (start(day), start(day + 1))
    }
}

/// Where the day starts, from the user's actions in the `NIGHT_DAYS` days before `end_ms`.
pub fn day_start(actions: impl Iterator<Item = i64>, end_ms: i64, offset: Offset) -> DayStart {
    let since = end_ms - NIGHT_DAYS * DAY;
    let mut slots: Vec<BTreeSet<i64>> = vec![BTreeSet::new(); SLOTS];
    let mut days = BTreeSet::new();
    for ts in actions.filter(|t| (since..end_ms).contains(t)) {
        let local = ts + offset(ts);
        let date = local.div_euclid(DAY);
        slots[(local.rem_euclid(DAY) / SLOT) as usize].insert(date);
        days.insert(date);
    }
    let counts: Vec<usize> = slots.iter().map(BTreeSet::len).collect();
    let low = counts.iter().copied().min().unwrap_or(0);
    let Some(first_high) = counts.iter().position(|&c| c > low).filter(|_| days.len() >= MIN_DAYS) else {
        return DayStart::Midnight { days: days.len() };
    };
    // Runs are read from just after a busier half hour, so that none is split at midnight.
    let (mut best, mut run) = ((0, 0), None::<(usize, usize)>);
    for k in 1..=SLOTS {
        let i = (first_high + k) % SLOTS;
        if counts[i] == low {
            let r = run.get_or_insert((i, 0));
            r.1 += 1;
            if r.1 > best.1 {
                best = *r;
            }
        } else {
            run = None;
        }
    }
    DayStart::Night { from: best.0, len: best.1, days: low }
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
    pub fn of(lanes: &[&Lane], (from, to): (i64, i64), start: DayStart, offset: Offset) -> Summary {
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
                if inside(msg) && start.day_of(end, offset) == start.day_of(msg, offset) {
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
            if start.day_of(from, offset) != start.day_of(to, offset) {
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
    /// has a message or an end, from the hour the day starts in.
    pub fn hours(&self, start: DayStart, offset: Offset) -> Vec<Vec<String>> {
        let hour = |ts: i64| ((ts + offset(ts)).rem_euclid(DAY) / HOUR) as usize;
        let mut messages = [0usize; 24];
        let mut ends = [0usize; 24];
        let mut answers: Vec<Vec<i64>> = vec![Vec::new(); 24];
        self.messages.iter().for_each(|&ts| messages[hour(ts)] += 1);
        self.ends.iter().for_each(|&(ts, _, _)| ends[hour(ts)] += 1);
        self.answers.iter().for_each(|&(ts, a)| answers[hour(ts)].push(a));
        let first = (start.minutes() / 60) as usize;
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
    pub start: DayStart,
    pub timelines: Vec<Timeline>,
    pub lanes: Vec<Lane>,
}

impl View {
    /// The view of these timelines, with the day found from the actions of the sessions it keeps
    /// in the `NIGHT_DAYS` days before `end_ms`.
    pub fn new(timelines: Vec<Timeline>, end_ms: i64, offset: Offset) -> View {
        let lanes = timelines.iter().map(Lane::of).collect();
        let mut view = View { start: DayStart::Midnight { days: 0 }, timelines, lanes };
        view.start = view.day_start(end_ms, offset);
        view
    }

    /// Where the day starts, found from the 30 days before `end_ms`.
    pub fn day_start(&self, end_ms: i64, offset: Offset) -> DayStart {
        day_start(self.kept().flat_map(|(t, _)| t.actions()), end_ms, offset)
    }

    fn kept(&self) -> impl Iterator<Item = (&Timeline, &Lane)> {
        self.timelines.iter().zip(&self.lanes).filter(|(t, _)| !t.left_out)
    }

    /// The sessions left out that have an action in `[from, to)`.
    pub fn left_out(&self, (from, to): (i64, i64)) -> usize {
        self.timelines.iter().filter(|t| t.left_out && t.actions().any(|ts| ts >= from && ts < to)).count()
    }

    pub fn summary(&self, window: (i64, i64), offset: Offset) -> Summary {
        let lanes: Vec<&Lane> = self.kept().map(|(_, l)| l).collect();
        Summary::of(&lanes, window, self.start, offset)
    }

    pub fn strip(&self, summary: &Summary, window: (i64, i64), offset: Offset) -> Option<Strip> {
        let pairs: Vec<(&Timeline, &Lane)> = self.kept().collect();
        Strip::of(&pairs, summary, window, self.start, offset)
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
    /// The rhythm of a main session, with the day found from its own actions; `None` when it has
    /// no turn or nobody typed in it.
    pub fn of(s: &Session, offset: Offset) -> Option<SessionRhythm> {
        let t = Timeline::of(s, &[]);
        if t.left_out {
            return None;
        }
        let lane = Lane::of(&t);
        if lane.turns.is_empty() {
            return None;
        }
        let end = t.events.last().map_or(0, |e| e.0) + 1;
        let start = day_start(t.actions(), end, offset);
        let all = Summary::of(&[&lane], (i64::MIN, i64::MAX), start, offset);
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

/// The day as a strip: an hours row, the user's sittings and a lane per session.
#[derive(Clone, Debug, PartialEq)]
pub struct Strip {
    pub lines: Vec<String>,
}

impl Strip {
    /// The strip of a day `[from, to)`, or `None` when the user did not act in it. `lanes` pairs
    /// each timeline with its lane.
    pub fn of(lanes: &[(&Timeline, &Lane)], summary: &Summary, (from, to): (i64, i64), start: DayStart, offset: Offset) -> Option<Strip> {
        let first = *summary.sittings.first().map(|(a, _)| a)?;
        let last = summary.sittings.last().map_or(first, |s| s.1);
        let local = |ts: i64| ts + offset(ts);
        let floor_hour = |ts: i64| ts - local(ts).rem_euclid(HOUR);
        let begin = floor_hour(first);
        let end = floor_hour(last) + HOUR;
        let minutes = COLUMN_MINUTES.iter().copied().find(|m| (end - begin) / (m * MINUTE) <= STRIP_COLUMNS).unwrap_or(60);
        let col = minutes * MINUTE;
        let n = ((end - begin) / col).max(1) as usize;
        let at = |ts: i64| ((ts - begin).div_euclid(col)).clamp(0, n as i64 - 1) as usize;
        let span = |row: &mut [char], a: i64, b: i64, c: char, rank: &dyn Fn(char) -> u8| {
            let (a, b) = (a.max(begin).max(from), b.min(end).min(to));
            if b < a {
                return;
            }
            for cell in &mut row[at(a)..=at(b)] {
                if rank(c) > rank(*cell) {
                    *cell = c;
                }
            }
        };
        let rank = |c: char| match c {
            '!' => 5,
            '*' => 4,
            '?' => 3,
            '=' => 2,
            '-' => 1,
            _ => 0,
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
        let mut lines = vec![format!("{:w$} {}", "", hours.iter().collect::<String>().trim_end(), w = LABEL_WIDTH)];
        let mut you = vec![' '; n];
        for &(a, b) in &summary.sittings {
            span(&mut you, a, b, '#', &|c| u8::from(c == '#'));
        }
        lines.push(format!("{:w$} {}", "you", you.iter().collect::<String>().trim_end(), w = LABEL_WIDTH));
        let mut shown: Vec<(&Timeline, &Lane, usize)> = lanes
            .iter()
            .map(|(t, l)| (*t, *l, l.actions.iter().filter(|(ts, _)| *ts >= from && *ts < to).count()))
            .filter(|(_, _, n)| *n > 0)
            .collect();
        shown.sort_by(|a, b| b.2.cmp(&a.2).then_with(|| a.0.id.cmp(&b.0.id)));
        for (t, l, _) in shown.iter().take(STRIP_LANES) {
            let mut row = vec![' '; n];
            for &(end, msg) in &l.answers {
                if start.day_of(end, offset) == start.day_of(msg, offset) {
                    span(&mut row, end, msg, '-', &rank);
                }
            }
            for &(a, b) in &l.work {
                span(&mut row, a, b, '=', &rank);
            }
            for &(ask, _) in &l.questions {
                span(&mut row, ask, ask, '?', &rank);
            }
            for &(ts, e) in &l.actions {
                let c = if e == Event::Interrupt { '!' } else if e.is_message() { '*' } else { continue };
                span(&mut row, ts, ts, c, &rank);
            }
            let id: String = t.id.chars().take(6).collect();
            let label = fmt::clip_end(&format!("{id} {}", t.title.as_deref().unwrap_or("")), LABEL_WIDTH);
            lines.push(format!("{label:LABEL_WIDTH$} {}", row.iter().collect::<String>().trim_end()));
        }
        if shown.len() > STRIP_LANES {
            lines.push(fmt::count((shown.len() - STRIP_LANES) as u64, "more session", "more sessions"));
        }
        lines.push("= agent working   - waiting for your answer   * your message   ? its question   ! you stopped it   # you were there".to_string());
        Some(Strip { lines })
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

    fn night() -> DayStart {
        DayStart::Night { from: 4, len: 12, days: 0 }
    }

    fn summary(lanes: &[Lane]) -> Summary {
        let refs: Vec<&Lane> = lanes.iter().collect();
        Summary::of(&refs, (i64::MIN, i64::MAX), night(), &off)
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

    /// Actions on `days` days at the given half hours.
    fn actions(days: i64, slots: &[usize]) -> Vec<i64> {
        (0..days).flat_map(|d| slots.iter().map(move |&s| MIDNIGHT - d * DAY + s as i64 * SLOT + 60_000)).collect()
    }

    #[test]
    fn the_authors_night() {
        let busy: Vec<usize> = (0..SLOTS).filter(|s| !(4..16).contains(s)).collect();
        let start = day_start(actions(30, &busy).into_iter(), MIDNIGHT + DAY, &off);
        assert_eq!(start, DayStart::Night { from: 4, len: 12, days: 0 });
        assert_eq!(start.minutes(), 5 * 60);
        assert_eq!(start.line(), "night     02:00–08:00 in the last 30 days, with no message then; days start at 05:00");
    }

    #[test]
    fn a_night_that_is_not_empty() {
        let mut all = actions(2, &(0..SLOTS).collect::<Vec<_>>());
        all.extend(actions(20, &(0..SLOTS).filter(|s| !(5..14).contains(s)).collect::<Vec<_>>()));
        let start = day_start(all.into_iter(), MIDNIGHT + DAY, &off);
        assert_eq!(start, DayStart::Night { from: 5, len: 9, days: 2 });
        assert_eq!(start.minutes(), 4 * 60 + 30);
        assert_eq!(start.line(), "night     02:30–07:00 in the last 30 days, with messages then on 2 days; days start at 04:30");
    }

    #[test]
    fn too_few_days_start_at_midnight() {
        let start = day_start(actions(5, &[20, 30]).into_iter(), MIDNIGHT + DAY, &off);
        assert_eq!(start, DayStart::Midnight { days: 5 });
        assert_eq!(start.line(), "night     messages on fewer than 7 of the last 30 days; days start at midnight");
        let old = day_start(actions(30, &[20]).into_iter(), MIDNIGHT + 100 * DAY, &off);
        assert_eq!(old, DayStart::Midnight { days: 0 }, "only the 30 days before the end count");
    }

    #[test]
    fn a_night_across_midnight_is_one_run() {
        // Busy 08:00–22:00: the night runs from 22:00 to 08:00 and its middle is 03:00.
        let start = day_start(actions(10, &(16..44).collect::<Vec<_>>()).into_iter(), MIDNIGHT + DAY, &off);
        assert_eq!(start, DayStart::Night { from: 44, len: 20, days: 0 });
        assert_eq!(start.minutes(), 3 * 60);
    }

    #[test]
    fn days_run_from_start_to_start() {
        let start = night();
        assert_eq!(start.day_of(t(4, 59), &off) + 1, start.day_of(t(5, 0), &off));
        assert_eq!(start.day_of(t(5, 0), &off), start.day_of(t(28, 59), &off), "02:10 the next night is the same day");
        let day = start.day_of(t(12, 0), &off);
        assert_eq!(start.bounds(day, &off), (t(5, 0), t(29, 0)));
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
        assert_eq!(SessionRhythm::of(&s, &off), None);
    }

    #[test]
    fn the_hours_table_runs_from_the_start_of_the_day() {
        let lane = Lane::of(&timeline("a", &[(t(9, 0), Typed), (t(9, 4), TurnEnd), (t(9, 10), Typed), (t(9, 12), TurnEnd), (t(26, 0), Typed)]));
        let s = summary(&[lane]);
        let rows = s.hours(night(), &off);
        assert_eq!(rows[0], ["hour", "messages", "turn ends", "median answer"]);
        assert_eq!(rows[1], ["09", "2", "2", "6.0m"]);
        assert_eq!(rows[2], ["02", "1", "1", "16.8h"], "02:00 the next night is the same day, after the evening");
        let none = summary(&[Lane::of(&timeline("a", &[(t(9, 0), Typed), (t(9, 4), TurnEnd)]))]);
        assert_eq!(none.hours(night(), &off)[1], ["09", "1", "1", "–"]);
    }

    fn strip(lanes: &[(Timeline, Lane)]) -> Vec<String> {
        let pairs: Vec<(&Timeline, &Lane)> = lanes.iter().map(|(t, l)| (t, l)).collect();
        let refs: Vec<&Lane> = lanes.iter().map(|(_, l)| l).collect();
        let day = night().bounds(night().day_of(t(12, 0), &off), &off);
        let s = Summary::of(&refs, day, night(), &off);
        Strip::of(&pairs, &s, day, night(), &off).unwrap().lines
    }

    fn lane(id: &str, title: &str, events: &[(i64, Event)]) -> (Timeline, Lane) {
        let mut tl = timeline(id, events);
        tl.title = Some(title.into());
        let l = Lane::of(&tl);
        (tl, l)
    }

    #[test]
    fn the_strip_columns_and_marks() {
        let lines = strip(&[lane(
            "ecad11b9-0000",
            "Инфографика прерываний",
            &[(t(8, 20), Typed), (t(8, 30), Request), (t(8, 40), Request), (t(8, 50), TurnEnd), (t(9, 30), Typed), (t(9, 31), Question), (t(10, 10), Answer), (t(10, 20), Interrupt), (t(22, 26), Typed)],
        )]);
        assert_eq!(lines[0], format!("{:20} 08  09  10  11  12  13  14  15  16  17  18  19  20  21  22", ""));
        // 08:00–23:00 in 15-minute columns: the message at 08:20 is in the second.
        let row = &lines[2];
        assert!(row.starts_with("ecad11 Инфографика…  "), "{row}");
        let cells: Vec<char> = row.chars().skip(21).collect();
        assert_eq!(cells[0], ' ');
        assert_eq!(cells[1], '*');
        assert_eq!(cells[2], '=', "08:30 works");
        assert_eq!(cells[3], '=', "08:45 ends at 08:50");
        assert_eq!(cells[4], '-', "09:00 waits for the answer");
        assert_eq!(cells[6], '*', "09:30 the message wins over the question");
        assert_eq!(cells[7], ' ', "09:45 waits for the answer to its question");
        assert_eq!(cells[8], '=', "10:00 works again after the answer at 10:10");
        assert_eq!(cells[9], '!', "10:15 the interrupt");
        assert_eq!(cells.len(), 58, "the row ends at the message at 22:26");
        assert_eq!(lines.last().unwrap(), "= agent working   - waiting for your answer   * your message   ? its question   ! you stopped it   # you were there");
        assert!(lines[1].starts_with(&format!("{:20}  #", "you")), "{}", lines[1]);
    }

    #[test]
    fn many_sessions_fold_into_one_line() {
        let lanes: Vec<(Timeline, Lane)> = (0..11).map(|k| lane(&format!("s{k:02}"), "t", &[(t(10, k), Typed)])).collect();
        let lines = strip(&lanes);
        assert_eq!(lines.iter().filter(|l| l.starts_with('s')).count(), 8);
        assert_eq!(lines[lines.len() - 2], "3 more sessions");
    }

    #[test]
    fn a_whole_day_takes_twenty_minute_columns() {
        let lines = strip(&[lane("a", "t", &[(t(5, 10), Typed), (t(28, 50), Typed)])]);
        // 05:00–05:00 is 24 hours: 20-minute columns fit them into 72, three to an hour.
        assert_eq!(lines[0], format!("{:20} 05 06 07 08 09 10 11 12 13 14 15 16 17 18 19 20 21 22 23 00 01 02 03 04", ""));
    }
}
