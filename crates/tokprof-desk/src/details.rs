//! The day summary of Day's details pane, while no session is selected (desk-window, Day
//! summary): `The day` with its date, six cards (Spend, Sittings, Agents, Answers, Queue and
//! Cache), `Top sessions of the day`, `Cold returns of the day` and, on the current day, `Waiting
//! for you now`. The spend comes from the day store, inside the bounds the day computation found;
//! the rhythm cards read the day's lines as `tokprof day` prints them.

use std::collections::HashMap;
use std::sync::Arc;

use rune::{Color, Cx, El, Key as RuneKey, KeyEvent, ScrollSpec, Span, Tx, col, el, rich, row, spacer, text};
use tokprof_browse::browser::now_ms;
use tokprof_core::calendar::{SessionKey, Spend, day_bounds, spend_between};
use tokprof_core::daystore::Entry;
use tokprof_core::fmt::{count, local_time, usd};
use tokprof_core::rhythm;

use crate::overlay::{self, R_GROUP, R_ROW, R_TILE};
use crate::pal::{self, Pal};
use crate::view::{self, dot};
use crate::{Desk, Focus};

const HOUR_MS: i64 = 3_600_000;
/// The sessions `Top sessions of the day` lists before `<n> more sessions`.
pub const TOP: usize = 5;
/// Under this window width the cards go three in a row.
pub const WIDE: f32 = 1200.0;
/// The padding of the pane, the gap between the cards, and the height of a row of a list.
const PAD: f32 = 18.0;
const GAP: f32 = 10.0;
const ROW_H: f32 = 26.0;

/// What the day store gives the day summary: the spend inside the day's bounds, and the day
/// before's.
#[derive(Clone, Debug, Default)]
pub struct DaySpend {
    pub day: Spend,
    /// What the day before spent, up to the same time of day on the current day (that time) or in
    /// full on a past day; none when the store holds no request on it.
    pub before: Option<(f64, Option<i64>)>,
    /// The project of each main session of the day, as the store names it.
    pub projects: HashMap<SessionKey, String>,
}

/// The store's address, the day's bounds, its date, and on the current day the minute.
type MemoKey = (usize, (i64, i64), i64, Option<i64>);

/// The day spend computed last, and what it was computed from: the store, the day's bounds, and
/// on the current day the minute.
#[derive(Default)]
pub struct Memo {
    key: Option<MemoKey>,
    spend: Option<Arc<DaySpend>>,
}

/// The day spend of the day shown, from the day store: `None` while the day is being computed,
/// an error text while the store is being filled (`reading the logs into the store… <n> of <m>
/// logs`) or could not be read.
pub fn day_spend(s: &Desk) -> Option<Result<Arc<DaySpend>, String>> {
    let parts = s.browser.day_parts()?;
    let computed = s.calendar.data.get();
    let store = match computed.and_then(|c| c.store.clone()) {
        Some(store) => store,
        None => {
            if let (Some(c), false) = (computed, s.calendar.data.reading())
                && let Err(e) = &c.calendar
            {
                return Some(Err(format!("the day store could not be read: {e}")));
            }
            return Some(Err(reading(s.calendar.data.progress())));
        }
    };
    let now = now_ms();
    let latest = s.browser.day_is_latest();
    let date = rhythm::date_of(s.browser.day_noon(), &rhythm::local_offset);
    let key = (Arc::as_ptr(&store) as usize, parts.bounds, date, latest.then_some(now / 60_000));
    let mut memo = s.day_spend.borrow_mut();
    if memo.key == Some(key)
        && let Some(d) = &memo.spend
    {
        return Some(Ok(d.clone()));
    }
    let filters = tokprof_browse::calendar::filters(&s.calendar.select);
    let spend = Arc::new(compute(&store.entries, |e| filters.keeps(e), parts.bounds, date, latest.then_some(now), now));
    *memo = Memo { key: Some(key), spend: Some(spend.clone()) };
    Some(Ok(spend))
}

/// The line while the store is being filled.
pub fn reading(progress: Option<(usize, usize)>) -> String {
    match progress {
        Some((done, total)) if total > 0 => format!("reading the logs into the store… {done} of {total} logs"),
        _ => "reading the logs into the store…".to_string(),
    }
}

/// The spend of `date` inside `bounds` of the entries `keep` keeps, and of the day before: up to
/// the same time of day when `clock` is now on the current day, in full otherwise.
pub fn compute(entries: &[Entry], keep: impl Fn(&Entry) -> bool, bounds: (i64, i64), date: i64, clock: Option<i64>, now: i64) -> DaySpend {
    let offset = &rhythm::local_offset;
    let day = spend_between(entries, &keep, bounds, offset);
    let (from, _) = day_bounds(entries, date - 1, now, offset);
    // The same time of day on the day before, by the local clock.
    let by = clock.map(|now| rhythm::midnight(date - 1, offset) + (now - rhythm::midnight(date, offset)));
    let to = by.map_or(bounds.0, |b| b.min(bounds.0));
    let before = spend_between(entries, &keep, (from, to), offset);
    let before = (!before.hours.is_empty()).then_some((before.usd, by));
    let wanted: std::collections::HashSet<&SessionKey> = day.sessions.iter().map(|(k, _)| k).collect();
    let mut projects = HashMap::new();
    for e in entries.iter().filter(|e| keep(e) && !e.subagent) {
        let k = (e.agent.clone(), e.main.clone());
        if wanted.contains(&k) {
            projects.insert(k, e.project.clone());
        }
    }
    DaySpend { day, before, projects }
}

/// The spend of every local hour from the first with a request to the last, empty ones too.
pub fn hour_bars(hours: &[(i64, f64)]) -> Vec<(i64, f64)> {
    let (Some(first), Some(last)) = (hours.first(), hours.last()) else { return Vec::new() };
    let by: HashMap<i64, f64> = hours.iter().copied().collect();
    // Local hours start on the hour of epoch time in every time zone of whole hours.
    (0..=(last.0 - first.0) / HOUR_MS).map(|k| first.0 + k * HOUR_MS).map(|at| (at, by.get(&at).copied().unwrap_or(0.0))).collect()
}

/// A rhythm line cut into the card's large number and its text: the line without its label, as
/// `tokprof day` prints it, less the number where the line starts with it. A line that says there
/// is nothing gives `—`.
pub fn card_parts(label: &str, rest: &str) -> (String, String) {
    let rest = rest.trim();
    let nothing = || ("—".to_string(), rest.to_string());
    let lead = |rest: &str| -> Option<(String, String)> {
        let n: String = rest.chars().take_while(|c| c.is_ascii_digit() || *c == '.' || *c == ',').collect();
        let n = n.trim_end_matches([',', '.']).to_string();
        (!n.is_empty()).then(|| {
            let tail = rest[n.len()..].trim_start_matches(',').trim_start();
            (n, tail.to_string())
        })
    };
    match label {
        "sittings" | "agents" => lead(rest).unwrap_or_else(nothing),
        "answers" => match rest.split_once("you answered in a median ") {
            Some((_, v)) => (v.split([';', ',']).next().unwrap_or(v).trim().to_string(), rest.to_string()),
            None => nothing(),
        },
        "queue" => match rest.split_once("% of") {
            Some((head, _)) => {
                let n = head.rsplit(' ').next().unwrap_or(head);
                if n.is_empty() || !n.chars().all(|c| c.is_ascii_digit()) { nothing() } else { (format!("{n}%"), rest.to_string()) }
            }
            None => nothing(),
        },
        "cache" => match lead(rest) {
            Some((n, tail)) if tail.starts_with("cold return") => {
                let (words, after) = tail.split_once(", ").unwrap_or((&tail, ""));
                (format!("{n} {words}"), after.to_string())
            }
            _ => nothing(),
        },
        _ => nothing(),
    }
}

/// A row of the summary's lists, in the order the cursor moves through them: its session, by the
/// id the browser lists it by; none for a session the list does not hold.
#[derive(Clone, Debug, PartialEq, Hash)]
pub struct Line {
    pub kind: Kind,
    pub session: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Kind {
    Top,
    Return,
    Waiting,
}

/// The sessions that wait for the user now, the longest wait first, with since when.
pub fn waiting(s: &Desk, now: i64) -> Vec<(usize, i64)> {
    let b = &s.browser;
    let mut out: Vec<(usize, i64)> =
        (0..b.len()).filter_map(|r| b.glance(r)?.waiting_since.filter(|&since| rhythm::waits_at(since, now)).map(|since| (r, since))).collect();
    out.sort_by_key(|&(r, since)| (since, r));
    out
}

/// When a wait of `Waiting for you now` next passes the waiting limit, after `now`, on the
/// current day while the summary shows.
pub fn next_wait(s: &Desk, now: i64) -> Option<i64> {
    if !shows(s) || !s.browser.day_is_latest() {
        return None;
    }
    waiting(s, now).into_iter().map(|(_, since)| since + rhythm::WAITING_LIMIT_MS).filter(|&at| at > now).min()
}

/// The rows of the lists, in the cursor's order: the top sessions, the cold returns, and those
/// who wait now.
pub fn lines(s: &Desk) -> Vec<Line> {
    let b = &s.browser;
    let mut out = Vec::new();
    if let Some(Ok(spend)) = day_spend(s) {
        for ((_, main), _) in spend.day.sessions.iter().take(TOP) {
            out.push(Line { kind: Kind::Top, session: b.row_of(main).map(|r| b.session(r).to_string()) });
        }
    }
    if let Some(parts) = b.day_parts() {
        for c in &parts.returns {
            out.push(Line { kind: Kind::Return, session: b.row_of(&c.session).map(|r| b.session(r).to_string()) });
        }
    }
    if b.day_is_latest() {
        for (r, _) in waiting(s, now_ms()) {
            out.push(Line { kind: Kind::Waiting, session: Some(b.session(r).to_string()) });
        }
    }
    out
}

/// Whether Day shows the day summary.
pub fn shows(s: &Desk) -> bool {
    s.reports.mode == crate::reports::Mode::Day && s.current().is_none()
}

/// Whether the day summary has the keyboard.
pub fn has_keys(s: &Desk) -> bool {
    shows(s) && s.pane == Focus::Inspector && !s.browser.editing()
}

/// Whether a `Next` cell of the top sessions counts down at `now`.
pub fn counting(s: &Desk, now: i64) -> bool {
    if !shows(s) {
        return false;
    }
    lines(s)
        .iter()
        .filter(|l| l.kind == Kind::Top)
        .filter_map(|l| s.browser.row_of(l.session.as_deref()?))
        .any(|r| s.browser.cells(r, now).warm_until.is_some())
}

/// What the summary shows, for `memo`.
pub fn version(s: &Desk, cx: &Cx<Desk>, theme: &str) -> u64 {
    let b = &s.browser;
    let now = now_ms();
    let spend = day_spend(s).map(|d| d.map(|d| Arc::as_ptr(&d) as usize));
    let (known, pending) = b.known_total();
    view::hash_of((
        (theme, view::window_size(s, cx).0.to_bits(), b.day_parts().map(|p| p as *const _ as usize), b.day_noon(), b.day_is_latest()),
        (spend, view::minute(), counting(s, now).then_some(now / 1000), lines(s), s.day_cursor, has_keys(s)),
        (known.to_bits(), pending, b.relisted(), b.filter(), s.calendar.client_ignored),
    ))
}

/// The day summary: its head, and under it in a pane that scrolls the cards and the lists.
pub fn summary(s: &Desk, cx: &Cx<Desk>, p: &Pal) -> El {
    let b = &s.browser;
    let keys = has_keys(s);
    let width = view::window_size(s, cx).0;
    let head = row()
        .key("day-head")
        .w_full()
        .h(view::HEAD_H)
        .px(PAD)
        .gap(10.0)
        .center_y()
        .child(text("The day").key("day-title").font(pal::HEAD).color(if keys { p.accent } else { p.ink }).single_line())
        .child(text(local_time(b.day_noon(), "%A, %-d %B")).key("day-date").font(pal::BODY).color(p.ink2).single_line())
        .child(spacer().grow(1.0))
        .child(
            text("Click a lane, or press ↓ in the lanes, to see a session")
                .key("day-hint")
                .font(pal::SMALL)
                .color(p.ink3)
                .single_line()
                .truncate()
                .shrink()
                .min_w(0.0),
        );
    let body = match b.day_parts() {
        Some(parts) => {
            let spend = day_spend(s);
            let inner = width - 2.0 * PAD;
            let lines = lines(s);
            let tops = lines.iter().filter(|l| l.kind == Kind::Top).count();
            let returns = lines.iter().filter(|l| l.kind == Kind::Return).count();
            let ring = keys.then_some(s.day_cursor);
            // Where each list's rows start among the rows the cursor moves through.
            let (t0, r0, w0) = (0, tops, tops + returns);
            let left_w = ((inner - 12.0) * 0.575).floor();
            let right_w = inner - 12.0 - left_w;
            let mut right = col().key("day-right").w(right_w).gap(14.0).child(returns_box(s, parts, r0, ring, right_w, p));
            if b.day_is_latest() {
                right = right.child(waiting_box(s, w0, ring, p));
            }
            let lists = row()
                .key("day-lists")
                .w_full()
                .gap(12.0)
                .items_start()
                .child(col().key("day-left").w(left_w).child(top_box(s, spend.as_ref(), t0, ring, left_w, p)))
                .child(right);
            vec![cards(s, parts, spend.as_ref(), width, p), lists]
        }
        None => vec![text(format!("computing the day of {}…", local_time(b.day_noon(), "%Y-%m-%d"))).key("day-computing").font(pal::BODY).color(p.ink2)],
    };
    let offset = s.pane_offset.clone();
    let pane = col()
        .key(view::PANE)
        .w_full()
        .grow(1.0)
        .min_h(0.0)
        .scroll(ScrollSpec { stick_bottom: false })
        .on_scroll_offset(move |at| offset.set(at.offset[1]))
        .on_layout(|s: &mut Desk, r| s.pane_height = r[3])
        .child(col().key("day-body").w_full().px(PAD).pt(14.0).pb(26.0).gap(16.0).children(body));
    col()
        .key("day-summary")
        .w_full()
        .grow(1.0)
        .min_h(0.0)
        .on_press(|s: &mut Desk| s.pane = Focus::Inspector)
        .child(head)
        .child(el().w_full().h(1.0).fill(p.line))
        .child(pane)
}

/// A card: its label with a note at its right, the large number, and what follows it.
fn card(key: &'static str, label: &str, aside: Option<String>, big: El, body: Vec<El>, w: f32, p: &Pal) -> El {
    let mut head = row().w_full().gap(6.0).center_y().child(text(label.to_string()).key("card-label").font(pal::SMALL_STRONG).color(p.ink2).single_line());
    if let Some(a) = aside {
        head = head.child(spacer().grow(1.0)).child(text(a).key("card-aside").font(pal::SMALL).color(p.ink3).single_line().truncate().shrink().min_w(0.0));
    }
    col().key(key).w(w).min_w(0.0).px(12.0).py(10.0).gap(4.0).radius(R_TILE).fill(p.bar).child(head).child(big).children(body)
}

fn big(t: impl Into<String>, color: Color) -> El {
    text(t.into()).key("card-value").font(pal::CARD).color(color).single_line()
}

fn small(t: impl Into<String>, p: &Pal) -> El {
    text(t.into()).key("card-text").w_full().wrap().font(pal::SMALL).color(p.ink2).selectable(true)
}

/// The six cards, six in a row, or three in a row in a window narrower than 1,200 points.
fn cards(s: &Desk, parts: &tokprof_browse::render_rhythm::DayParts, spend: Option<&Result<Arc<DaySpend>, String>>, width: f32, p: &Pal) -> El {
    let per_row = if width < WIDE { 3 } else { 6 };
    let inner = width - 2.0 * PAD;
    let w = ((inner - GAP * (per_row as f32 - 1.0)) / per_row as f32).floor();
    let mut all = vec![spend_card(s, spend, w, p)];
    for (key, label, name) in [
        ("card-sittings", "Sittings", "sittings"),
        ("card-agents", "Agents", "agents"),
        ("card-answers", "Answers", "answers"),
        ("card-queue", "Queue", "queue"),
        ("card-cache", "Cache", "cache"),
    ] {
        let line = crate::day::line_of(&parts.below, name);
        let (value, rest) = line.as_deref().map_or(("—".to_string(), String::new()), |l| card_parts(name, l));
        let cold = name == "cache" && value != "—";
        let body = if cold {
            // The dollars in the theme's red, as the cache line marks them.
            let mut spans = Vec::new();
            let mut at = 0;
            for (run, hot) in crate::day::cold_runs(&rest) {
                if hot {
                    spans.push(Span::new(at, at + run.len()).color(p.cold).weight(650.0));
                }
                at += run.len();
            }
            rich(rest.clone(), spans).key("card-text").w_full().font(pal::SMALL).color(p.ink2).selectable(true)
        } else {
            small(rest.clone(), p)
        };
        let body = if rest.is_empty() { Vec::new() } else { vec![body] };
        all.push(card(key, label, None, big(value, if cold { p.cold } else { p.ink }), body, w, p));
    }
    let mut out = col().key("day-cards").w_full().gap(GAP);
    let mut it = all.into_iter().peekable();
    while it.peek().is_some() {
        out = out.child(row().w_full().gap(GAP).items_stretch().children(it.by_ref().take(per_row)));
    }
    out
}

/// The Spend card: the day's spend, the comparison with the day before, and a bar for each hour.
fn spend_card(s: &Desk, spend: Option<&Result<Arc<DaySpend>, String>>, w: f32, p: &Pal) -> El {
    let mut body = Vec::new();
    let value = match spend {
        Some(Ok(d)) => {
            if let Some((before, by)) = d.before {
                let line = match by {
                    Some(at) => format!("vs {} by {} yesterday", usd(before), local_time(at, "%H:%M")),
                    None => format!("vs {} the day before", usd(before)),
                };
                body.push(text(line).key("spend-before").font(pal::SMALL).color(p.ink2).single_line().truncate().min_w(0.0));
            }
            let bars = hour_bars(&d.day.hours);
            if !bars.is_empty() {
                body.push(hour_chart(&bars, w - 24.0, p));
            }
            usd(d.day.usd)
        }
        Some(Err(e)) => {
            body.push(small(e.clone(), p).key("spend-reading"));
            "…".to_string()
        }
        None => "…".to_string(),
    };
    if s.calendar.client_ignored {
        body.push(text("every client: the day store has no client").key("spend-client").font(pal::SMALL).color(p.ink3).single_line().truncate().min_w(0.0));
    }
    card("card-spend", "Spend", Some("by hour".into()), big(value, p.ink), body, w, p)
}

/// The hours as bars as high as their spend, each named under the pointer, with the hours under
/// them where there is room.
fn hour_chart(bars: &[(i64, f64)], w: f32, p: &Pal) -> El {
    const H: f32 = 26.0;
    let max = bars.iter().map(|b| b.1).fold(0.0, f64::max).max(1e-9);
    let every = ((bars.len() as f32 * 18.0 / w.max(1.0)).ceil() as usize).max(1);
    let mut cols = row().key("hour-bars").w_full().h(H).gap(2.0).items_end();
    let mut labels = row().key("hour-labels").w_full().gap(2.0);
    for (i, &(at, v)) in bars.iter().enumerate() {
        let h = if v > 0.0 { (v / max * H as f64).max(1.0) as f32 } else { 0.0 };
        let name = format!("{}–{} · {}", local_time(at, "%H:%M"), local_time(at + HOUR_MS, "%H:%M"), usd(v));
        let bar = el().key(format!("hour-{}", local_time(at, "%H"))).w_full().h(h).radius(2.0).fill(p.read);
        cols = cols.child(col().grow(1.0).min_w(0.0).h(H).justify_end().help(name).child(bar));
        let label = if i % every == 0 { local_time(at, "%-H") } else { String::new() };
        labels = labels.child(row().grow(1.0).min_w(0.0).center().child(text(label).font(pal::HOURS).color(p.ink3).single_line()));
    }
    col().w_full().pt(4.0).gap(2.0).child(cols).child(labels)
}

/// A list of the summary: its title with a note at its right, over a soft group of rows.
fn section(key: &'static str, title: &str, aside: Option<&str>, rows: Vec<El>, p: &Pal) -> El {
    let mut head = row().w_full().gap(8.0).center_y().child(text(title.to_string()).key("list-title").font(pal::HEAD).color(p.ink2).single_line());
    if let Some(a) = aside {
        head = head
            .child(spacer().grow(1.0))
            .child(text(a.to_string()).key("list-aside").font(pal::SMALL).color(p.ink3).single_line().truncate().shrink().min_w(0.0));
    }
    let group = col().w_full().min_w(0.0).px(6.0).py(6.0).gap(1.0).radius(R_GROUP).fill(p.bar).children(rows);
    col().key(key).w_full().gap(8.0).child(head).child(group)
}

/// A row of a list: `n` among the rows the cursor moves through, ringed under it while the summary
/// has the keyboard; a press selects its session.
fn line_row(key: String, n: usize, session: Option<String>, ring: Option<usize>, p: &Pal) -> El {
    let on = ring == Some(n);
    let mut r = row().key(key).w_full().h(ROW_H).px(8.0).gap(8.0).center_y().radius(R_ROW);
    r = if on { overlay::ring_at(r.fill(p.sel), true, 1.0, p) } else { r.hover(p.hover) };
    match session {
        Some(id) => r.role("button").on_press_tx(move |s: &mut Desk, tx| {
            s.day_cursor = n;
            s.pick(&id, tx);
        }),
        None => r,
    }
}

/// A session as a row names it: its agent's dot, its project's last part in the ink and its title,
/// or the store's project and the id when the list does not hold it.
fn who(s: &Desk, session: Option<&str>, project: Option<&str>, id: &str, p: &Pal) -> Vec<El> {
    let b = &s.browser;
    let (color, head, rest) = match session.and_then(|id| b.row_of(id)) {
        Some(r) => {
            let c = b.cells(r, now_ms());
            let head = c.project.rsplit(['/', '\\']).next().unwrap_or(c.project).to_string();
            (p.agents[c.agent_place % p.agents.len()], head, c.title.unwrap_or(c.project).to_string())
        }
        None => {
            let project = project.unwrap_or("");
            (p.ink3, project.rsplit(['/', '\\']).next().unwrap_or(project).to_string(), id.chars().take(8).collect())
        }
    };
    vec![
        dot(color, 7.0).key("agent"),
        text(head).key("project").font(pal::STRONG).color(p.ink).single_line().truncate().max_w(140.0),
        text(rest).key("title").font(pal::BODY).color(p.ink2).single_line().truncate().grow(1.0).shrink().min_w(0.0),
    ]
}

/// `Top sessions of the day`: the 5 with the most spend that day, a bar as long as each one's
/// share of the first, its spend and its `Next` cell; then the rest in one row.
fn top_box(s: &Desk, spend: Option<&Result<Arc<DaySpend>, String>>, first: usize, ring: Option<usize>, w: f32, p: &Pal) -> El {
    let b = &s.browser;
    let now = now_ms();
    let aside = Some("spend that day · next request");
    let d = match spend {
        Some(Ok(d)) => d,
        Some(Err(e)) => {
            return section(
                "day-top",
                "Top sessions of the day",
                aside,
                vec![text(e.clone()).key("top-reading").w_full().wrap().font(pal::BODY).color(p.ink2).padding(8.0)],
                p,
            );
        }
        None => return section("day-top", "Top sessions of the day", aside, Vec::new(), p),
    };
    let sessions = &d.day.sessions;
    if sessions.is_empty() {
        return section(
            "day-top",
            "Top sessions of the day",
            aside,
            vec![text("No spend this day.").key("top-none").font(pal::BODY).color(p.ink2).padding(8.0)],
            p,
        );
    }
    let max = sessions[0].1.max(1e-9);
    let bar_w = if w >= 640.0 { 140.0 } else { 60.0 };
    let mut rows = Vec::new();
    for (i, ((agent, main), v)) in sessions.iter().take(TOP).enumerate() {
        let row_of = b.row_of(main);
        let id = row_of.map(|r| b.session(r).to_string());
        let project = d.projects.get(&(agent.clone(), main.clone())).map(String::as_str);
        let next = match row_of {
            Some(r) => view::next_cell(&b.cells(r, now), now, p),
            None => el(),
        };
        rows.push(
            line_row(format!("top-{i}"), first + i, id.clone(), ring, p)
                .children(who(s, id.as_deref(), project, main, p))
                .child(overlay::meter(v / max, p.read, p, 6.0).key("top-bar").w(bar_w).min_w(bar_w).grow(0.0))
                .child(row().w(64.0).justify_end().child(text(usd(*v)).key("top-spend").font(pal::STRONG_NUM).color(p.ink).single_line()))
                .child(row().w(72.0).justify_end().child(next)),
        );
    }
    if sessions.len() > TOP {
        let rest: f64 = sessions[TOP..].iter().map(|x| x.1).sum();
        let n = sessions.len() - TOP;
        rows.push(
            row()
                .key("top-more")
                .w_full()
                .h(ROW_H)
                .px(8.0)
                .gap(8.0)
                .center_y()
                .child(text(count(n as u64, "more session", "more sessions")).key("top-more-text").font(pal::BODY).color(p.ink3).single_line())
                .child(spacer().grow(1.0))
                .child(row().w(64.0).justify_end().child(text(usd(rest)).key("top-more-spend").font(pal::BODY_NUM).color(p.ink3).single_line()))
                .child(el().w(72.0)),
        );
    }
    section("day-top", "Top sessions of the day", aside, rows, p)
}

/// `Cold returns of the day`, in time order: the time, the session, the pause, the first
/// request's cost and its avoidable dollars.
fn returns_box(s: &Desk, parts: &tokprof_browse::render_rhythm::DayParts, first: usize, ring: Option<usize>, w: f32, p: &Pal) -> El {
    let b = &s.browser;
    let aside = Some("first request · avoidable");
    if parts.returns.is_empty() {
        return section(
            "day-returns",
            "Cold returns of the day",
            aside,
            vec![text("No cold returns.").key("returns-none").font(pal::BODY).color(p.ink2).padding(8.0)],
            p,
        );
    }
    let after_w = if w >= 420.0 { 80.0 } else { 64.0 };
    let rows = parts
        .returns
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let id = b.row_of(&c.session).map(|r| b.session(r).to_string());
            line_row(format!("return-{i}"), first + i, id.clone(), ring, p)
                .child(text(local_time(c.ts_ms, "%H:%M")).key("return-time").font(pal::STRONG_NUM).color(p.ink).single_line().w(38.0))
                .children(who(s, id.as_deref(), None, &c.session, p))
                .child(text(format!("after {}", rhythm::duration(c.pause_ms))).key("return-pause").font(pal::SMALL).color(p.ink3).single_line().w(after_w))
                .child(row().w(52.0).justify_end().child(text(usd(c.usd)).key("return-cost").font(pal::BODY_NUM).color(p.ink).single_line()))
                .child(row().w(52.0).justify_end().child(text(usd(c.premium_usd)).key("return-avoidable").font(pal::STRONG_NUM).color(p.cold).single_line()))
        })
        .collect();
    section("day-returns", "Cold returns of the day", aside, rows, p)
}

/// A wait as the list reads it: seconds under a minute, whole minutes under an hour, then hours.
pub fn wait_text(ms: i64) -> String {
    match ms.max(0) {
        w if w < 60_000 => format!("{}s", w / 1000),
        w if w < HOUR_MS => format!("{}m", w / 60_000),
        w => rhythm::duration(w),
    }
}

/// `Waiting for you now`: who waits for the user, the longest first, since when and how long.
fn waiting_box(s: &Desk, first: usize, ring: Option<usize>, p: &Pal) -> El {
    let b = &s.browser;
    let now = now_ms();
    let waits = waiting(s, now);
    if waits.is_empty() {
        return section(
            "day-waiting",
            "Waiting for you now",
            None,
            vec![text("Nobody waits for you").key("waiting-none").font(pal::BODY).color(p.ink2).padding(8.0)],
            p,
        );
    }
    let rows = waits
        .into_iter()
        .enumerate()
        .map(|(i, (r, since))| {
            let id = b.session(r).to_string();
            line_row(format!("waiting-{i}"), first + i, Some(id.clone()), ring, p).children(who(s, Some(&id), None, &id, p)).child(
                text(format!("since {}, {}", local_time(since, "%H:%M"), wait_text(now - since)))
                    .key("waiting-since")
                    .font(pal::SMALL_NUM)
                    .color(p.ink3)
                    .single_line(),
            )
        })
        .collect();
    section("day-waiting", "Waiting for you now", None, rows, p)
}

/// The keys of the day summary while it has the keyboard: ↑ ↓ (`k`, `j`), Home and End move the
/// cursor over the rows of its lists, ⏎ selects the cursor's session, PgUp, PgDn and Space
/// scroll; the rest go on.
pub fn on_key(s: &mut Desk, e: &KeyEvent, tx: &mut Tx) -> bool {
    let n = lines(s).len();
    let text = match &e.key {
        RuneKey::Text(t) if !(e.mods.cmd || e.mods.ctrl || e.mods.alt) => Some(t.as_str()),
        _ => None,
    };
    let step: Option<isize> = match (&e.key, text) {
        (RuneKey::Up, _) | (_, Some("k")) => Some(-1),
        (RuneKey::Down, _) | (_, Some("j")) => Some(1),
        (RuneKey::Home, _) | (_, Some("g")) => Some(isize::MIN / 2),
        (RuneKey::End, _) | (_, Some("G")) => Some(isize::MAX / 2),
        _ => None,
    };
    if let Some(step) = step {
        if n > 0 {
            s.day_cursor = (s.day_cursor.min(n - 1) as isize).saturating_add(step).clamp(0, n as isize - 1) as usize;
            if let Some(key) = row_key(s, s.day_cursor) {
                tx.scroll_into_view(key);
            }
        }
        return true;
    }
    match (&e.key, text) {
        (RuneKey::Enter, _) => {
            if let Some(id) = lines(s).get(s.day_cursor).and_then(|l| l.session.clone()) {
                s.pick(&id, tx);
            }
            true
        }
        (RuneKey::PageUp, _) => {
            tx.scroll_page(view::PANE, -1);
            true
        }
        (RuneKey::PageDown, _) | (_, Some(" ")) => {
            tx.scroll_page(view::PANE, 1);
            true
        }
        _ => false,
    }
}

/// The key of the `n`th row the cursor moves through.
fn row_key(s: &Desk, n: usize) -> Option<String> {
    let lines = lines(s);
    let kind = lines.get(n)?.kind;
    let i = lines[..n].iter().filter(|l| l.kind == kind).count();
    Some(match kind {
        Kind::Top => format!("top-{i}"),
        Kind::Return => format!("return-{i}"),
        Kind::Waiting => format!("waiting-{i}"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_line_splits_into_its_number_and_its_text() {
        let sittings = card_parts("sittings", "7, 1.8h at the keyboard, 34 messages to 16 sessions; median sitting 1.1m, median break 29m");
        assert_eq!(sittings, ("7".into(), "1.8h at the keyboard, 34 messages to 16 sessions; median sitting 1.1m, median break 29m".into()));
        let agents = card_parts("agents", "8.8 agent-hours over 3.4h at the keyboard, some agent at work 5.9h; peak 4 at 11:54");
        assert_eq!(agents, ("8.8".into(), "agent-hours over 3.4h at the keyboard, some agent at work 5.9h; peak 4 at 11:54".into()));
        assert_eq!(card_parts("agents", "no agent worked"), ("—".into(), "no agent worked".into()));
        let answers = "the agent's turn took a median 44s; you answered in a median 3.4m";
        assert_eq!(card_parts("answers", answers), ("3.4m".into(), answers.into()), "the median answer, and the whole line");
        assert_eq!(card_parts("answers", "no turns you started; no answers").0, "—");
        let queue = "when several sessions waited, you answered the one that waited least first in 71% of 17 answers (45% by chance); the oldest had waited a median 1.4h";
        assert_eq!(card_parts("queue", queue), ("71%".into(), queue.into()));
        assert_eq!(card_parts("queue", "no answer while another session waited"), ("—".into(), "no answer while another session waited".into()));
        let cache = card_parts("cache", "3 cold returns, $4.60 in their first requests, $4.14 of it avoidable");
        assert_eq!(cache, ("3 cold returns".into(), "$4.60 in their first requests, $4.14 of it avoidable".into()));
        assert_eq!(card_parts("cache", "1 cold return, $3.00 in its first request, $2.90 of it avoidable").0, "1 cold return");
        assert_eq!(card_parts("cache", "4 cold returns"), ("4 cold returns".into(), String::new()), "unpriced: no dollars");
        assert_eq!(card_parts("cache", "no cold returns"), ("—".into(), "no cold returns".into()));
        assert_eq!(card_parts("sittings", "12, 3.4h at the keyboard").0, "12");
    }

    #[test]
    fn hours_without_a_request_still_have_their_bar() {
        let h = |k: i64| 1_000 * HOUR_MS + k * HOUR_MS;
        assert_eq!(hour_bars(&[(h(0), 1.0), (h(3), 2.0)]), vec![(h(0), 1.0), (h(1), 0.0), (h(2), 0.0), (h(3), 2.0)]);
        assert!(hour_bars(&[]).is_empty());
    }

    #[test]
    fn a_wait_reads_in_whole_minutes_under_an_hour() {
        assert_eq!(wait_text(4 * 60_000 + 30_000), "4m");
        assert_eq!(wait_text(42_000), "42s");
        assert_eq!(wait_text(84 * 60_000), "1.4h");
    }
}
