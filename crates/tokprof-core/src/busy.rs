//! Where an agent's time went while it was busy, and which requests were slow (see the agent-time
//! spec). The rules read one main session with the sessions of its subagents.
//!
//! Busy time is painted, not summed: the rules collect the intervals of each kind and of the
//! busy set, and one sweep over their ends gives each moment the first kind that covers it.

use std::collections::BTreeMap;

use crate::model::{ChainKind, HandoffKind, Request, Session, WakeSource};
use crate::rhythm::{GAP_MS, Lane, Start, Timeline};
use crate::trigger::Trigger;

/// A wait is at least this long.
pub const WAIT_MS: i64 = 60_000;
/// A request is slow when its answered attempt runs at least this long.
pub const SLOW_MS: i64 = 60_000;
/// How many of the longest waits the views name.
pub const LONGEST: usize = 3;

pub const MODEL: &str = "model";
pub const API_RETRIES: &str = "API retries";
pub const SUBAGENTS: &str = "subagents";
pub const COMPACTION: &str = "compaction";
pub const BACKGROUND: &str = "background commands";
pub const NOTHING: &str = "nothing";

/// What a moment of busy time goes to, in the order the rules try them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Kind {
    Model,
    Retries,
    Subagents,
    /// The activity of the next main request, or `compaction`.
    Between(&'static str),
    Background,
}

impl Kind {
    fn name(self) -> &'static str {
        match self {
            Kind::Model => MODEL,
            Kind::Retries => API_RETRIES,
            Kind::Subagents => SUBAGENTS,
            Kind::Between(name) => name,
            Kind::Background => BACKGROUND,
        }
    }
}

/// A stretch of busy time of one kind other than the model's.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Wait {
    pub kind: &'static str,
    pub start_ms: i64,
    pub ms: i64,
}

/// The busy time of one session or of many, and its requests' times.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Busy {
    /// Per kind: its time, and the part of it after ends of turn.
    pub kinds: BTreeMap<&'static str, (i64, i64)>,
    /// The longest waits, longest first.
    pub waits: Vec<Wait>,
    /// Requests of the main conversations that got a response, advisor calls left out.
    pub main_requests: u64,
    /// Requests of every chain that got a response, subagents included, advisor calls left out.
    pub requests: u64,
    pub slow: u64,
    pub slow_main: u64,
    /// The slowest request: how long its answered attempt ran, and its output tokens.
    pub longest: Option<(i64, u64)>,
    /// Main requests that waited for retries after API errors, and their last errors.
    pub retried: u64,
    pub retry_errors: BTreeMap<String, u64>,
}

/// What can be painted: the kinds, and the layers that decide whether a moment is busy.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Layer {
    Turn,
    Question,
    Kind(Kind),
}

/// What busy time is made of before all of a session's logs are read: the intervals to paint, and
/// the counts of the requests read so far. A report that reads logs one by one adds the parts of
/// a session's subagents to the part of its main log, then paints them once.
#[derive(Clone, Debug, Default)]
pub struct Parts {
    paint: Vec<(i64, i64, Layer)>,
    counts: Busy,
    /// The main log of the session is among the parts.
    pub has_main: bool,
    /// The window of a report, `[since, until)` in epoch ms: only the requests sent inside it
    /// count, and only the busy time inside it is painted. `None` for a whole session.
    window: Option<(i64, i64)>,
}

/// Whether a moment falls in a window, or there is none.
fn inside(window: Option<(i64, i64)>, ts_ms: i64) -> bool {
    window.is_none_or(|(since, until)| since <= ts_ms && ts_ms < until)
}

impl Parts {
    /// The parts of a main log: its turns, its requests, and the subagent chains it holds.
    pub fn of_main(main: &Session) -> Parts {
        Parts::of_main_in(main, None)
    }

    /// The parts of a main log inside a report's window (see `window`).
    pub fn of_main_in(main: &Session, window: Option<(i64, i64)>) -> Parts {
        let mut parts = Parts { has_main: true, window, ..Parts::default() };
        let lane = Lane::of(&Timeline::of(main, &[]));
        let paint = &mut parts.paint;
        for t in &lane.turns {
            paint.push((t.start, t.end, Layer::Turn));
        }
        for &(asked, answered) in &lane.questions {
            let turn_end = lane.turns.iter().find(|t| t.start <= asked && asked <= t.end).map_or(asked, |t| t.end);
            paint.push((asked, answered.unwrap_or(turn_end).max(asked), Layer::Question));
        }
        // After an end of turn, until a wake that reports a background command.
        for w in lane.turns.windows(2) {
            let (before, turn) = (&w[0], &w[1]);
            let command = main.handoffs.iter().any(|h| h.ts_ms == turn.start && h.kind == HandoffKind::Wake(WakeSource::Command));
            if turn.by == Start::Wake && command {
                paint.push((before.end, turn.start, Layer::Kind(Kind::Background)));
            }
        }
        parts.counts.paint_requests(main, &mut parts.paint, window);
        parts.add_chains(main, true);
        parts
    }

    /// The parts of a subagent's log: when it worked, and its requests.
    pub fn of_subagent(sub: &Session) -> Parts {
        Parts::of_subagent_in(sub, None)
    }

    /// The parts of a subagent's log inside a report's window (see `window`).
    pub fn of_subagent_in(sub: &Session, window: Option<(i64, i64)>) -> Parts {
        let mut parts = Parts { window, ..Parts::default() };
        parts.add_chains(sub, false);
        parts
    }

    /// The stretches of work of a log's subagent chains (all of a subagent log's chains), and the
    /// counts of its requests. A subagent works from a request to the next while it runs the
    /// calls of the first, or when the next comes within a silence's length; a subagent resumed
    /// later with a new message starts a new stretch.
    fn add_chains(&mut self, s: &Session, main: bool) {
        for chain in &s.chains {
            if !main || chain.kind == ChainKind::Subagent {
                let mut stretch: Option<(i64, i64)> = None;
                for r in &chain.requests {
                    let (Some(sent), Some(ended)) = (r.first_sent_ms.or(r.sent_ms), r.ended_ms) else { continue };
                    stretch = match stretch {
                        Some((a, b)) if r.trigger().from_calls() || r.trigger() == Trigger::Compaction || sent - b < GAP_MS => Some((a, b.max(ended))),
                        Some((a, b)) => {
                            self.paint.push((a, b, Layer::Kind(Kind::Subagents)));
                            Some((sent, ended))
                        }
                        None => Some((sent, ended)),
                    };
                }
                if let Some((a, b)) = stretch {
                    self.paint.push((a, b, Layer::Kind(Kind::Subagents)));
                }
            }
            let (c, window) = (&mut self.counts, self.window);
            for r in chain.requests.iter().filter(|r| !r.advisor && (r.usage.prompt() > 0 || r.usage.output > 0) && inside(window, r.ts_ms)) {
                c.requests += 1;
                let Some(run) = r.run_ms() else { continue };
                if run >= SLOW_MS {
                    c.slow += 1;
                    c.slow_main += u64::from(main && chain.kind == ChainKind::Main);
                }
                if c.longest.is_none_or(|(ms, _)| run > ms) {
                    c.longest = Some((run, r.usage.output));
                }
            }
        }
    }

    /// Adds the parts of another log of the same session.
    pub fn add(&mut self, o: Parts) {
        self.paint.extend(o.paint);
        self.counts.merge(o.counts);
        self.has_main |= o.has_main;
        self.window = self.window.or(o.window);
    }

    /// Paints the parts into the session's busy time, cut at the edges of the window.
    pub fn finish(self) -> Busy {
        let mut busy = self.counts;
        let paint = match self.window {
            Some((since, until)) => self.paint.into_iter().map(|(a, b, layer)| (a.max(since), b.min(until), layer)).collect(),
            None => self.paint,
        };
        busy.sweep(paint);
        busy
    }
}

impl Busy {
    /// The busy time of a main session, with the sessions of its subagents.
    pub fn of(main: &Session, subagents: &[Session]) -> Busy {
        let mut parts = Parts::of_main(main);
        for s in subagents {
            parts.add(Parts::of_subagent(s));
        }
        parts.finish()
    }

    /// The main conversations' requests: their own time, their retries, and what the agent
    /// waited on between a response and the next request.
    fn paint_requests(&mut self, main: &Session, paint: &mut Vec<(i64, i64, Layer)>, window: Option<(i64, i64)>) {
        let mut marks: Vec<i64> = main.handoffs.iter().map(|h| h.ts_ms).collect();
        for (_, requests) in main.main_conversations() {
            marks.extend(requests.iter().filter_map(|r| r.first_sent_ms.or(r.sent_ms)));
        }
        marks.sort_unstable();
        let next_mark = |after: i64| marks.get(marks.partition_point(|&m| m <= after)).copied();
        for (_, requests) in main.main_conversations() {
            let timed: Vec<&Request> = requests.into_iter().filter(|r| !r.advisor && r.sent_ms.is_some()).collect();
            for (i, r) in timed.iter().enumerate() {
                let (first, sent) = (r.first_sent_ms.unwrap_or(r.sent_ms.unwrap_or_default()), r.sent_ms.unwrap_or_default());
                let answered = r.usage.prompt() > 0 || r.usage.output > 0;
                let counted = inside(window, r.ts_ms);
                if answered && counted {
                    self.main_requests += 1;
                }
                if counted && (r.retries > 0 || (!answered && r.retry_error.is_some())) {
                    self.retried += 1;
                    if let Some(e) = &r.retry_error {
                        *self.retry_errors.entry(e.to_string()).or_default() += 1;
                    }
                }
                match r.ended_ms {
                    Some(ended) => {
                        paint.push((first, sent, Layer::Kind(Kind::Retries)));
                        paint.push((sent, ended, Layer::Kind(Kind::Model)));
                    }
                    // A request that got no response runs until the next event of the conversation.
                    None => {
                        let until = next_mark(sent).unwrap_or(sent);
                        let kind = if r.retry_error.is_some() { Kind::Retries } else { Kind::Model };
                        paint.push((first, until.max(first), Layer::Kind(kind)));
                    }
                }
                if let (Some(ended), Some(next)) = (r.ended_ms, timed.get(i + 1)) {
                    let start = next.first_sent_ms.or(next.sent_ms).unwrap_or_default();
                    if let Some(kind) = between(next) {
                        paint.push((ended, start.max(ended), Layer::Kind(kind)));
                    }
                }
            }
        }
    }

    /// Gives each moment of busy time its kind, and finds the waits. A run of `nothing` as long
    /// as work-rhythm's silence is no busy time: nothing of the agent's ran, as when a local
    /// command opened a turn that sent no request.
    fn sweep(&mut self, paint: Vec<(i64, i64, Layer)>) {
        let mut edges: Vec<(i64, i32, Layer)> = Vec::with_capacity(paint.len() * 2);
        for (a, b, layer) in paint.into_iter().filter(|(a, b, _)| b > a) {
            edges.push((a, 1, layer));
            edges.push((b, -1, layer));
        }
        edges.sort_by_key(|e| e.0);
        let mut open: BTreeMap<Layer, i32> = BTreeMap::new();
        // The current run of one kind: the run, and its part after ends of turn.
        let mut run: Option<(Wait, i64)> = None;
        let mut waits: Vec<Wait> = Vec::new();
        let mut kinds: BTreeMap<&'static str, (i64, i64)> = BTreeMap::new();
        let mut close = |run: &mut Option<(Wait, i64)>| {
            let Some((w, after)) = run.take() else { return };
            if w.kind == NOTHING && w.ms >= GAP_MS {
                return;
            }
            let slot = kinds.entry(w.kind).or_default();
            slot.0 += w.ms;
            slot.1 += after;
            if w.ms >= WAIT_MS && w.kind != MODEL && w.kind != API_RETRIES {
                waits.push(w);
            }
        };
        let mut i = 0;
        while i < edges.len() {
            let t = edges[i].0;
            while i < edges.len() && edges[i].0 == t {
                *open.entry(edges[i].2).or_default() += edges[i].1;
                i += 1;
            }
            open.retain(|_, n| *n > 0);
            let Some(&(next, _, _)) = edges.get(i) else { break };
            let has = |l: Layer| open.contains_key(&l);
            let in_turn = has(Layer::Turn);
            // A main request that runs keeps the agent busy, even past the end of a turn that the
            // log does not tell.
            let running = has(Layer::Kind(Kind::Model)) || has(Layer::Kind(Kind::Retries));
            let busy = running
                || if in_turn { !has(Layer::Question) } else { has(Layer::Kind(Kind::Subagents)) || has(Layer::Kind(Kind::Background)) };
            if !busy {
                close(&mut run);
                continue;
            }
            let kind = open.keys().find_map(|l| match l {
                Layer::Kind(k) => Some(k.name()),
                _ => None,
            });
            let kind = kind.unwrap_or(NOTHING);
            let ms = next - t;
            let after = if in_turn { 0 } else { ms };
            match &mut run {
                Some((w, a)) if w.kind == kind && w.start_ms + w.ms == t => {
                    w.ms += ms;
                    *a += after;
                }
                _ => {
                    close(&mut run);
                    run = Some((Wait { kind, start_ms: t, ms }, after));
                }
            }
        }
        close(&mut run);
        self.kinds = kinds;
        self.waits = waits;
        self.keep_longest();
    }

    fn keep_longest(&mut self) {
        self.waits.sort_by(|a, b| b.ms.cmp(&a.ms).then(a.start_ms.cmp(&b.start_ms)));
        self.waits.truncate(LONGEST);
    }

    /// All busy time.
    pub fn total_ms(&self) -> i64 {
        self.kinds.values().map(|v| v.0).sum()
    }

    /// The kinds with their time and the part after ends of turn, most time first.
    pub fn sorted(&self) -> Vec<(&'static str, i64, i64)> {
        let mut rows: Vec<(&'static str, i64, i64)> = self.kinds.iter().filter(|(_, v)| v.0 > 0).map(|(k, v)| (*k, v.0, v.1)).collect();
        rows.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));
        rows
    }

    /// The last error that most retried requests had.
    pub fn top_error(&self) -> Option<&str> {
        self.retry_errors.iter().max_by(|a, b| a.1.cmp(b.1).then(b.0.cmp(a.0))).map(|(e, _)| e.as_str())
    }

    /// Adds another session's busy time.
    pub fn merge(&mut self, o: Busy) {
        for (k, v) in o.kinds {
            let slot = self.kinds.entry(k).or_default();
            slot.0 += v.0;
            slot.1 += v.1;
        }
        self.waits.extend(o.waits);
        self.keep_longest();
        self.main_requests += o.main_requests;
        self.requests += o.requests;
        self.slow += o.slow;
        self.slow_main += o.slow_main;
        if let Some(l) = o.longest.filter(|l| self.longest.is_none_or(|m| l.0 > m.0)) {
            self.longest = Some(l);
        }
        self.retried += o.retried;
        for (e, n) in o.retry_errors {
            *self.retry_errors.entry(e).or_default() += n;
        }
    }
}

/// What the agent waited on before a main request, from the end of the response before it: the
/// activity of the calls that triggered it, or the compaction before it; `None` for the rest.
fn between(next: &Request) -> Option<Kind> {
    let trigger = next.trigger();
    if trigger == Trigger::Compaction {
        Some(Kind::Between(COMPACTION))
    } else if trigger.from_calls() {
        let activity = next.activity();
        Some(if activity == SUBAGENTS { Kind::Subagents } else { Kind::Between(activity) })
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Chain, TypedMessage, Usage};
    use crate::trigger::CallClass;

    const MIN: i64 = 60_000;

    /// 2026-10-01 at `h:m:s` UTC, in ms.
    fn at(h: i64, m: i64, s: i64) -> i64 {
        1_790_812_800_000 + ((h * 60 + m) * 60 + s) * 1000
    }

    fn t(h: i64, m: i64) -> i64 {
        at(h, m, 0)
    }

    /// A main request sent at `sent` whose response ended at `ended`, triggered by a prompt.
    fn req(sent: i64, ended: i64) -> Request {
        Request {
            ts_ms: sent,
            usage: Usage { cache_read: 1_000, output: 10, ..Usage::default() },
            trigger: Some(Trigger::Prompt),
            first_sent_ms: Some(sent),
            sent_ms: Some(sent),
            ended_ms: Some(ended),
            ..Request::default()
        }
    }

    /// A request that the calls of the response before it triggered.
    fn after(sent: i64, ended: i64, trigger: Trigger, class: CallClass) -> Request {
        Request { trigger: Some(trigger), activity: Some(class), ..req(sent, ended) }
    }

    fn session(typed: &[i64], handoffs: &[(i64, HandoffKind)], requests: Vec<Request>) -> Session {
        let mut s = Session::new(crate::testing::claude(), "s".into());
        s.typed = typed.iter().map(|&ts_ms| TypedMessage { ts_ms, label: "go".into() }).collect();
        for &(ts, kind) in handoffs {
            s.push_handoff(ts, kind);
        }
        s.chains.push(Chain { name: "main".into(), kind: ChainKind::Main, requests });
        s
    }

    /// A subagent's session that works from `start` to `end`: a request, a long call, and the
    /// request its result triggered.
    fn subagent(start: i64, end: i64) -> Session {
        let mut s = Session::new(crate::testing::claude(), "s".into());
        let requests = vec![req(start, start + 1_000), after(end - 1_000, end, Trigger::Run, CallClass::Building)];
        s.chains.push(Chain { name: "subagent".into(), kind: ChainKind::Subagent, requests });
        s
    }

    #[test]
    fn a_subagent_resumed_later_works_in_two_stretches() {
        let s = session(&[t(10, 0)], &[(t(10, 4), TurnEnd)], vec![req(t(10, 0), t(10, 4))]);
        let mut sub = Session::new(crate::testing::claude(), "s".into());
        // It works 10:04–10:10, then a new message resumes it at 12:00 until 12:05.
        let requests = vec![req(t(10, 4), t(10, 5)), after(t(10, 9), t(10, 10), Trigger::Run, CallClass::Building), req(t(12, 0), t(12, 5))];
        sub.chains.push(Chain { name: "subagent".into(), kind: ChainKind::Subagent, requests });
        let b = Busy::of(&s, &[sub]);
        assert_eq!(time(&b, SUBAGENTS), (11 * MIN, 11 * MIN), "10:04–10:10 and 12:00–12:05, not the time between");
    }

    fn time(b: &Busy, kind: &str) -> (i64, i64) {
        b.kinds.get(kind).copied().unwrap_or_default()
    }

    use HandoffKind::*;

    #[test]
    fn a_turn_and_the_answer_after_it() {
        let s = session(&[t(10, 0), t(10, 10)], &[(t(10, 4), TurnEnd), (t(10, 11), TurnEnd)], vec![req(t(10, 0), t(10, 4)), req(t(10, 10), t(10, 11))]);
        let b = Busy::of(&s, &[]);
        assert_eq!(b.total_ms(), 5 * MIN, "busy 10:00–10:04 and 10:10–10:11, not 10:04–10:10");
        assert_eq!(time(&b, MODEL), (5 * MIN, 0));
    }

    #[test]
    fn a_question_is_no_busy_time() {
        let s = session(&[t(10, 0)], &[(t(10, 2), Question), (t(10, 7), Answer), (t(10, 9), TurnEnd)], vec![req(t(10, 0), t(10, 2)), after(t(10, 7), t(10, 9), Trigger::Run, CallClass::OtherTools)]);
        let b = Busy::of(&s, &[]);
        assert_eq!(b.total_ms(), 4 * MIN, "busy 10:00–10:02 and 10:07–10:09");
    }

    #[test]
    fn a_command_in_the_background_keeps_the_agent_busy_until_its_wake() {
        let s = session(
            &[t(10, 0)],
            &[(t(10, 4), TurnEnd), (t(10, 19), Wake(WakeSource::Command)), (t(10, 20), TurnEnd)],
            vec![req(t(10, 0), t(10, 4)), Request { trigger: Some(Trigger::Notification), ..req(t(10, 19), t(10, 20)) }],
        );
        let b = Busy::of(&s, &[]);
        assert_eq!(time(&b, BACKGROUND), (15 * MIN, 15 * MIN), "all of it after the end of the turn");
        assert_eq!(b.total_ms(), 20 * MIN);
    }

    #[test]
    fn subagents_keep_the_agent_busy_after_its_turn() {
        let s = session(&[t(10, 0), t(11, 0)], &[(t(10, 4), TurnEnd)], vec![req(t(10, 0), t(10, 4)), req(t(11, 0), t(11, 1))]);
        let b = Busy::of(&s, &[subagent(t(10, 2), t(10, 30))]);
        assert_eq!(time(&b, SUBAGENTS), (26 * MIN, 26 * MIN), "busy until 10:30, not until 11:00");
    }

    #[test]
    fn a_session_that_began_before_the_window() {
        // Busy from 08:00 to 09:00 and from 11:00 to 11:30, each with one slow request; the
        // window starts at 10:00.
        let s = session(&[t(8, 0), t(11, 0)], &[(t(9, 0), TurnEnd), (t(11, 30), TurnEnd)], vec![req(t(8, 0), t(9, 0)), req(t(11, 0), t(11, 30))]);
        let b = Parts::of_main_in(&s, Some((t(10, 0), i64::MAX))).finish();
        assert_eq!(b.total_ms(), 30 * MIN, "only the half hour inside the window");
        assert_eq!((b.requests, b.slow, b.main_requests), (1, 1, 1), "only the request sent inside the window");
        let whole = Busy::of(&s, &[]);
        assert_eq!((whole.total_ms(), whole.requests, whole.slow), (90 * MIN, 2, 2), "without a window, the whole session");
    }

    #[test]
    fn a_message_from_another_session_or_the_user_first_is_no_busy_time() {
        let other = session(&[t(10, 0)], &[(t(10, 4), TurnEnd), (t(12, 0), Wake(WakeSource::Other)), (t(12, 1), TurnEnd)], vec![req(t(10, 0), t(10, 4)), req(t(12, 0), t(12, 1))]);
        assert_eq!(Busy::of(&other, &[]).total_ms(), 5 * MIN);
        // The user types at 10:10, before the wake of the background command at 10:19.
        let first = session(&[t(10, 0), t(10, 10)], &[(t(10, 4), TurnEnd), (t(10, 19), Wake(WakeSource::Command)), (t(10, 20), TurnEnd)], vec![req(t(10, 0), t(10, 4)), req(t(10, 10), t(10, 20))]);
        let b = Busy::of(&first, &[]);
        assert_eq!(b.total_ms(), 14 * MIN, "busy 10:00–10:04 and 10:10–10:20");
        assert_eq!(time(&b, BACKGROUND), (0, 0));
    }

    #[test]
    fn requests_retries_and_requests_without_a_response() {
        let retried = Request { first_sent_ms: Some(at(10, 0, 0)), retries: 2, retry_error: Some("502 local proxy: upstream request failed".into()), ..req(at(10, 0, 20), at(10, 0, 30)) };
        let s = session(&[t(10, 0)], &[(at(10, 0, 30), TurnEnd)], vec![retried]);
        let b = Busy::of(&s, &[]);
        assert_eq!((time(&b, API_RETRIES).0, time(&b, MODEL).0), (20_000, 10_000));
        assert_eq!((b.retried, b.top_error()), (1, Some("502 local proxy: upstream request failed")));
        // Sent at 10:00 and never answered; the user interrupts at 10:03.
        let cancelled = Request { usage: Usage::default(), ended_ms: None, ..req(t(10, 0), 0) };
        let s = session(&[t(10, 0)], &[(t(10, 3), Interrupt)], vec![cancelled.clone()]);
        assert_eq!(time(&Busy::of(&s, &[]), MODEL).0, 3 * MIN);
        let failed = Request { retry_error: Some("502".into()), ..cancelled };
        let s = session(&[t(10, 0)], &[(t(10, 3), TurnEnd)], vec![failed]);
        assert_eq!(time(&Busy::of(&s, &[]), API_RETRIES).0, 3 * MIN);
    }

    #[test]
    fn subagents_while_the_main_conversation_waits_and_while_it_runs() {
        let s = session(
            &[t(10, 0)],
            &[(t(10, 22), TurnEnd)],
            vec![req(t(10, 0), t(10, 1)), after(t(10, 21), t(10, 22), Trigger::Subagent, CallClass::Subagents)],
        );
        let b = Busy::of(&s, &[subagent(t(10, 1), t(10, 20))]);
        assert_eq!(time(&b, SUBAGENTS).0, 20 * MIN, "10:01–10:20 the subagent worked, and the minute after it went to its call");
        let b = Busy::of(&s, &[subagent(t(10, 0), t(10, 1))]);
        assert_eq!(time(&b, MODEL).0, 2 * MIN, "while a main request runs, the time is the model's");
    }

    #[test]
    fn between_requests_the_next_request_decides() {
        let s = session(
            &[t(10, 0)],
            &[(t(10, 10), TurnEnd)],
            vec![
                req(at(10, 0, 5), t(10, 1)),
                after(t(10, 4), t(10, 5), Trigger::Run, CallClass::Building),
                Request { compacted_before: true, ..req(t(10, 7), t(10, 8)) },
                after(at(10, 8, 40), t(10, 10), Trigger::Lookup, CallClass::Reading),
            ],
        );
        let b = Busy::of(&s, &[]);
        assert_eq!(time(&b, "building and testing").0, 3 * MIN);
        assert_eq!(time(&b, COMPACTION).0, 2 * MIN);
        assert_eq!(time(&b, "reading").0, 40_000);
        assert_eq!(time(&b, NOTHING).0, 5_000, "from the message to the first send");
        let waits: Vec<(&str, i64, i64)> = b.waits.iter().map(|w| (w.kind, w.start_ms, w.ms)).collect();
        assert_eq!(waits, [("building and testing", t(10, 1), 3 * MIN), (COMPACTION, t(10, 5), 2 * MIN)], "the 40 seconds of reading are no wait");
    }

    #[test]
    fn a_long_stretch_of_nothing_is_no_busy_time() {
        // A local command opens a turn that sends no request; another session wakes the agent
        // 52 minutes later.
        let s = session(&[t(10, 0)], &[(t(10, 52), Wake(WakeSource::Other)), (t(10, 53), TurnEnd)], vec![req(at(10, 52, 10), t(10, 53))]);
        let b = Busy::of(&s, &[]);
        assert_eq!((b.total_ms(), time(&b, NOTHING)), (50_000, (0, 0)));
        assert!(b.waits.is_empty());
    }

    #[test]
    fn a_wait_cut_by_a_request() {
        let s = session(&[t(10, 0), t(10, 20)], &[(t(10, 4), TurnEnd), (t(10, 21), TurnEnd)], vec![req(t(10, 0), t(10, 4)), req(t(10, 20), t(10, 21))]);
        let b = Busy::of(&s, &[subagent(t(10, 2), t(10, 30))]);
        let waits: Vec<(&str, i64, i64)> = b.waits.iter().map(|w| (w.kind, w.start_ms, w.ms)).collect();
        assert_eq!(waits, [(SUBAGENTS, t(10, 4), 16 * MIN), (SUBAGENTS, t(10, 21), 9 * MIN)]);
    }

    #[test]
    fn slow_requests_leave_out_their_retries_and_advisor_calls() {
        let long = Request { usage: Usage { output: 21_700, ..Usage::default() }, ..req(at(10, 0, 0), at(10, 3, 29)) };
        let retried = Request { first_sent_ms: Some(at(10, 5, 0)), retries: 3, ..req(at(10, 5, 50), at(10, 6, 10)) };
        let advisor = Request { advisor: true, ..req(at(10, 0, 0), at(10, 3, 29)) };
        let s = session(&[t(10, 0)], &[(at(10, 6, 10), TurnEnd)], vec![long, advisor, retried]);
        let b = Busy::of(&s, &[]);
        assert_eq!((b.slow, b.slow_main, b.requests, b.main_requests), (1, 1, 2, 2));
        assert_eq!(b.longest, Some((209_000, 21_700)));
        assert_eq!(time(&b, API_RETRIES).0, 50_000);
    }

    #[test]
    fn merged_sessions_add_up() {
        let s = session(&[t(10, 0)], &[(t(10, 4), TurnEnd)], vec![req(t(10, 0), t(10, 4))]);
        let mut all = Busy::of(&s, &[]);
        all.merge(Busy::of(&s, &[]));
        assert_eq!((time(&all, MODEL).0, all.main_requests), (8 * MIN, 2));
        assert_eq!(all.sorted(), [(MODEL, 8 * MIN, 0)]);
    }
}
