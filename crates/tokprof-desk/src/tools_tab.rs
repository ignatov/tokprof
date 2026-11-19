//! The `Tools` tab: a card per group of tools, the tool table, and the call log of the main
//! conversation (desk-window, Tools tab). The cells come from tokprof-browse, as `tokprof scan`
//! and `tokprof scan --calls` print them; this lays them out, filters them by the chosen card, and
//! steps from a call to its request in `Requests`.

use rune::{Color, El, KeyEvent, Tx, col, el, row, spacer, text};
use tokprof_browse::render_calls::{CallFilter, CardCells, call_row_cells, calls_line, card_cells, kept_calls};
use tokprof_browse::render_tools::{ToolCells, failures_line, tool_cells_of};
use tokprof_core::calllog::CallStatus;
use tokprof_core::fmt::int;
use tokprof_core::tools::{Group, GroupFilter, RowOrder};
use tokprof_core::xray::XRay;

use crate::Desk;
use crate::inspector::{self, Column, Tab, boxed, note, plain};
use crate::overlay::{self, R_GROUP};
use crate::pal::{self, Pal};

/// A card's width and height, the room between cards, and the height of its bars.
pub const CARD_W: f32 = 156.0;
pub const CARD_H: f32 = 108.0;
pub const CARD_GAP: f32 = 8.0;
pub const BAR_H: f32 = 4.0;
/// The tool table's rows at most.
pub const TOOL_ROWS: usize = 24;
/// The line over the call log.
pub const CALLS_NOTE: &str = "calls of the main conversation; the cards and the tool table count the subagents' calls too";

/// The tab's state: for which session, the chosen card's group, and the cursor over the tool
/// table's rows and then the call log's, as one list.
#[derive(Clone, Debug, Default)]
pub struct State {
    pub session: String,
    pub group: Option<Group>,
    pub cursor: usize,
}

impl State {
    /// What the tab draws from, for the inspector's memo.
    pub fn version(&self) -> impl std::hash::Hash {
        (self.session.clone(), self.group.clone(), self.cursor)
    }
}

/// What the tab lays out for a state and an X-ray: the cards, the chosen group, the tool cells
/// and the calls it keeps, and the cursor in the joined list. A state of another session counts
/// as none: no card chosen and the cursor on the first row.
pub struct Layout {
    pub cards: Vec<CardCells>,
    pub group: Option<Group>,
    pub tools: ToolCells,
    /// The indices in the call log of the calls the chosen group keeps.
    pub calls: Vec<usize>,
    pub cursor: usize,
}

impl Layout {
    pub fn of(st: &State, x: &XRay) -> Layout {
        let cards = card_cells(&x.tools);
        let ours = st.session == x.id;
        let group = st.group.clone().filter(|g| ours && cards.iter().any(|c| !c.never_called && &c.group == g));
        let filter = group.clone().map(GroupFilter::One);
        let tools = tool_cells_of(&x.tools, RowOrder::Rent, TOOL_ROWS, x.tools.failed > 0, filter.as_ref());
        let calls = kept_calls(&x.call_log, &CallFilter { group: filter, ..CallFilter::default() });
        let mut l = Layout { cards, group, tools, calls, cursor: 0 };
        l.cursor = if ours { st.cursor.min(l.len().saturating_sub(1)) } else { 0 };
        l
    }

    /// The rows of the tool table.
    pub fn tool_rows(&self) -> usize {
        self.tools.rows.len().saturating_sub(1)
    }

    /// The rows of both tables.
    pub fn len(&self) -> usize {
        self.tool_rows() + self.calls.len()
    }

    /// The page of the call log drawn: the kept calls from `lo` to `hi`, the 300 that hold the
    /// cursor, or the first 300 while it is in the tool table.
    pub fn page(&self) -> (usize, usize) {
        crate::requests::page(self.calls.len(), self.cursor.saturating_sub(self.tool_rows()))
    }

    /// The drawn row of the joined list's row `j`.
    pub fn drawn(&self, j: usize) -> usize {
        let t = self.tool_rows();
        if j < t { j } else { t + j - t - self.page().0 }
    }

    /// The joined list's row of drawn row `n`.
    pub fn joined(&self, n: usize) -> Option<usize> {
        let t = self.tool_rows();
        let (lo, hi) = self.page();
        let j = if n < t { n } else { t + lo + (n - t) };
        (n < t || lo + (n - t) < hi).then_some(j)
    }

    /// The call of the joined list's row `j`, as an index in the call log.
    pub fn call_at(&self, j: usize) -> Option<usize> {
        j.checked_sub(self.tool_rows()).and_then(|k| self.calls.get(k)).copied()
    }

    /// The cards a click or ← → can choose.
    fn choosable(&self) -> Vec<&Group> {
        self.cards.iter().filter(|c| !c.never_called).map(|c| &c.group).collect()
    }
}

fn has_calls(x: &XRay) -> bool {
    x.tools.rows.values().any(|r| r.calls > 0)
}

/// The tab: the cards, the tool table and the call log, or a note without tool calls. `room`
/// is the width the cards have.
pub fn tab(s: &Desk, x: &XRay, room: f32, p: &Pal) -> Vec<El> {
    if !has_calls(x) {
        return vec![note("No tool calls.", p)];
    }
    let l = Layout::of(&s.tools, x);
    inspector::cursor_row(inspector::keyboard(s).then_some(l.drawn(l.cursor)));
    vec![cards(&l, room, p), tool_table(x, &l, p), call_log(x, &l, p)]
}

/// How many cards fit in a row `room` points wide.
pub fn per_row(room: f32) -> usize {
    (((room + CARD_GAP) / (CARD_W + CARD_GAP)).floor() as usize).max(1)
}

/// The key of a group's card.
pub fn card_key(name: &str) -> String {
    format!("card-{}", name.replace(' ', "-"))
}

fn cards(l: &Layout, room: f32, p: &Pal) -> El {
    let head = row().w_full().gap(8.0).center_y().child(text("Groups").font(pal::HEAD).color(p.ink2).single_line()).child(spacer().grow(1.0)).child(
        text("calls and their time by where the tool comes from").key("aside").font(pal::SMALL).color(p.ink3).single_line().truncate().shrink().min_w(0.0),
    );
    let rows = l
        .cards
        .chunks(per_row(room))
        .enumerate()
        .map(|(i, chunk)| row().key(format!("cards-{i}")).gap(CARD_GAP).children(chunk.iter().map(|c| card(c, l.group.as_ref() == Some(&c.group), p))));
    col().key("groups").w_full().gap(10.0).child(head).child(col().w_full().gap(CARD_GAP).children(rows))
}

fn card(c: &CardCells, on: bool, p: &Pal) -> El {
    let base = col().key(card_key(&c.name)).w(CARD_W).h(CARD_H).padding(12.0).gap(4.0).radius(overlay::R_TILE).role("button").label(c.name.clone());
    // A long server's name wraps to a second line rather than lose its end.
    let name = |color| text(c.name.clone()).key("card-name").font(pal::STRONG).color(color).wrap().max_lines(2).min_w(0.0);
    if c.never_called {
        // Dimmed: the secondary text and line colors, no fill and no bars, and nothing to choose.
        return base
            .border(1.0, p.line)
            .help(format!("{}: connected, never called", c.name))
            .child(name(p.ink2))
            .child(text("never called").key("card-never").font(pal::SMALL).color(p.ink3).single_line())
            .child(spacer().grow(1.0));
    }
    let group = c.group.clone();
    let mut head = row().w_full().gap(6.0).items_start().child(name(p.ink).grow(1.0));
    if let Some(f) = &c.failed {
        head = head.child(text(f.clone()).key("card-failed").font(pal::SMALL_STRONG_NUM).color(p.cold).single_line());
    }
    let counts = row()
        .w_full()
        .gap(6.0)
        .center_y()
        .child(text(c.calls.clone()).key("card-calls").font(pal::STRONG_NUM).color(p.ink).single_line())
        .child(spacer().grow(1.0))
        .child(text(c.tokens.clone()).key("card-tokens").font(pal::SMALL_NUM).color(p.ink2).single_line());
    let time_bar = match c.time_share {
        Some(share) => overlay::meter(share, p.work, p, BAR_H).key("card-time-bar").w_full(),
        None => el().w_full().h(BAR_H),
    };
    let body = base
        .fill(if on { p.sel } else { p.bar })
        .hover(if on { p.sel } else { p.hover })
        .border(1.0, p.row_line)
        .checked(on)
        .help(format!(
            "{}: {} of the calls, {}",
            c.name,
            pct(c.calls_share),
            c.time_share.map_or("no time to count".to_string(), |t| format!("{} of the time in calls", pct(t)))
        ))
        .on_press(move |s: &mut Desk| choose(s, Some(group.clone()), true))
        .child(head)
        .child(counts)
        .child(text(c.time.clone()).key("card-time").font(pal::SMALL_NUM).color(p.ink2).single_line())
        .child(spacer().grow(1.0))
        .child(col().w_full().gap(3.0).child(overlay::meter(c.calls_share, p.bars, p, BAR_H).key("card-calls-bar").w_full()).child(time_bar));
    overlay::ring(body, on, p)
}

fn pct(share: f64) -> String {
    format!("{:.0}%", 100.0 * share)
}

/// The tool table of the chosen group, or of all: the rows of `tokprof scan`'s tools block.
fn tool_table(x: &XRay, l: &Layout, p: &Pal) -> El {
    let cells = &l.tools;
    let widths = [0.0, 50.0, 56.0, 64.0, 80.0, 80.0, 70.0, 70.0, 56.0, 50.0, 70.0];
    let names: Vec<&'static str> = vec!["tool", "calls", "time", "med time", "avg result", "avg input", "rent", "trips", "single", "failed", "after"];
    let n = cells.rows.first().map_or(0, Vec::len);
    let mut cols: Vec<Column> = (0..n).map(|i| Column(names[i], widths[i], i > 0)).collect();
    cols.push(Column("hint", 230.0, false));
    let rows = cells
        .rows
        .iter()
        .skip(1)
        .zip(cells.hints.iter().skip(1))
        .map(|(r, h)| r.iter().cloned().map(plain).chain([(h.clone(), Some(p.ink2))]).collect())
        .collect();
    let mut body = col().w_full().gap(8.0).child(inspector::table_dropping(
        &cols,
        rows,
        &["hint", "after", "failed", "single", "avg input", "trips", "med time", "time"],
        p,
    ));
    if cells.more > 0 {
        body = body.child(note(format!("{} more", cells.more), p));
    }
    // The spend and the failures are the session's, so they show without a chosen card.
    if l.group.is_none() {
        body = body.child(note(cells.spend.clone(), p));
        if let Some(f) = failures_line(&x.tools) {
            body = body.child(text(f).w_full().wrap().font(pal::BODY).color(p.cold).selectable(true));
        }
    }
    let aside = match &l.group {
        Some(g) => format!("{} only", g.name()),
        None => "calls, sizes, rent and trips by tool".to_string(),
    };
    boxed("tools", "Tools", Some(aside), body, p)
}

/// The color of a call's status (desk-window, Call log table).
fn status_color(status: CallStatus, p: &Pal) -> Option<Color> {
    match status {
        CallStatus::Failed => Some(p.cold),
        CallStatus::Running => Some(p.warm),
        CallStatus::AskedYou => Some(p.question),
        CallStatus::Refused | CallStatus::Denied | CallStatus::Background => Some(p.ink2),
        CallStatus::Done | CallStatus::NoResult => None,
    }
}

/// The call log: the line over it, the `calls` line, and the page of calls that holds the cursor.
fn call_log(x: &XRay, l: &Layout, p: &Pal) -> El {
    let small = |t: String, key: &'static str, color| text(t).key(key).w_full().wrap().font(pal::SMALL).color(color).selectable(true);
    let mut body = col()
        .w_full()
        .gap(8.0)
        .child(small(CALLS_NOTE.to_string(), "calls-note", p.ink3))
        .child(text(calls_line(x)).key("calls-line").w_full().wrap().font(pal::BODY).color(p.ink2).selectable(true));
    if l.group.is_some() {
        body = body.child(small(format!("showing {} of {} calls", int(l.calls.len() as u64), int(x.call_log.len() as u64)), "calls-showing", p.ink2));
    }
    if !l.calls.is_empty() {
        let (lo, hi) = l.page();
        if lo > 0 {
            body = body.child(text(format!("and {lo} earlier calls")).key("calls-earlier").font(pal::BODY).color(p.ink2));
        }
        let rows = l.calls[lo..hi]
            .iter()
            .map(|&i| {
                let r = &x.call_log[i];
                let color = status_color(r.status, p);
                call_row_cells(i, r).into_iter().enumerate().map(|(c, t)| (t, if c == 6 { color } else { None })).collect()
            })
            .collect();
        let cols = [
            Column("#", 40.0, true),
            Column("started", 108.0, false),
            Column("req", 36.0, true),
            Column("group", 84.0, false),
            Column("tool", 132.0, false),
            Column("input", 0.0, false),
            Column("status", 72.0, false),
            Column("took", 48.0, true),
            Column("tokens", 52.0, true),
        ];
        body = body.child(inspector::table_open(&cols, rows, &["group", "started", "tokens", "req"], Some(open_row), p));
        if hi < l.calls.len() {
            body = body.child(text(format!("and {} later calls", l.calls.len() - hi)).key("calls-later").font(pal::BODY).color(p.ink2));
        }
    }
    let head = row().w_full().gap(8.0).center_y().child(text("Calls").font(pal::HEAD).color(p.ink2).single_line());
    col().key("calls").w_full().gap(10.0).child(head).child(col().w_full().padding(14.0).radius(R_GROUP).fill(p.bar).child(body))
}

/// Makes the state about session `id`, with no card chosen, when it is about another.
fn adopt(s: &mut Desk, id: &str) {
    if s.tools.session != id {
        s.tools = State { session: id.to_string(), ..State::default() };
    }
}

/// Chooses the card of `group`, or none; with `toggle`, choosing the chosen card clears it. A
/// card that cannot be chosen changes nothing. The cursor goes to the first row.
fn choose(s: &mut Desk, group: Option<Group>, toggle: bool) {
    let Some(x) = s.browser.xray_shared() else { return };
    adopt(s, &x.id);
    s.pane = crate::Focus::Inspector;
    let l = Layout::of(&s.tools, &x);
    if group.as_ref().is_some_and(|g| !l.choosable().contains(&g)) {
        return;
    }
    s.tools.group = if toggle && group == l.group { None } else { group };
    s.tools.cursor = 0;
}

/// A click on a drawn row puts the cursor on it.
pub fn row_pressed(s: &mut Desk, n: usize) {
    let Some(x) = s.browser.xray_shared() else { return };
    adopt(s, &x.id);
    if let Some(j) = Layout::of(&s.tools, &x).joined(n) {
        s.tools.cursor = j;
    }
}

/// A double click on a drawn row of the call log opens its request.
fn open_row(s: &mut Desk, n: usize, tx: &mut Tx) {
    row_pressed(s, n);
    open(s, tx);
}

/// Shows `Requests` with the request of the call under the cursor selected; nothing for a row of
/// the tool table or a call without a request.
fn open(s: &mut Desk, tx: &mut Tx) {
    let Some(x) = s.browser.xray_shared() else { return };
    let l = Layout::of(&s.tools, &x);
    let Some(req) = l.call_at(l.cursor).and_then(|i| x.call_log[i].request) else { return };
    if let Some(i) = (req as usize).checked_sub(1) {
        crate::requests::show_request(s, i, tx);
    }
}

/// The keys of the tab while the inspector has the keyboard: ← → choose a card, ↑ ↓ PgUp PgDn
/// Home End move the cursor through both tables, ⏎ opens a call's request. Shift+← → go on, to
/// change the tab.
pub fn key(s: &mut Desk, e: &KeyEvent, tx: &mut Tx) -> bool {
    use rune::Key as K;
    if e.mods.cmd || e.mods.ctrl || e.mods.alt {
        return false;
    }
    let Some(x) = s.browser.xray_shared().filter(|x| has_calls(x)) else { return false };
    let text = match &e.key {
        K::Text(t) => Some(t.as_str()),
        _ => None,
    };
    if e.mods.shift && matches!(e.key, K::Left | K::Right) {
        return false;
    }
    adopt(s, &x.id);
    let l = Layout::of(&s.tools, &x);
    let step: isize = match (&e.key, text) {
        (K::Left, _) | (_, Some("h")) => {
            step_card(s, &l, -1, tx);
            return true;
        }
        (K::Right, _) | (_, Some("l")) => {
            step_card(s, &l, 1, tx);
            return true;
        }
        (K::Enter, _) => {
            open(s, tx);
            return true;
        }
        (K::Up, _) | (_, Some("k")) => -1,
        (K::Down, _) | (_, Some("j")) => 1,
        (K::PageUp, _) => -10,
        (K::PageDown, _) => 10,
        (K::Home, _) | (_, Some("g")) => isize::MIN / 2,
        (K::End, _) | (_, Some("G")) => isize::MAX / 2,
        _ => return false,
    };
    if l.len() == 0 {
        return true;
    }
    s.tools.cursor = (l.cursor as isize).saturating_add(step).clamp(0, l.len() as isize - 1) as usize;
    let l = Layout::of(&s.tools, &x);
    tx.scroll_into_view(format!("row-{}", l.drawn(l.cursor)));
    true
}

/// Chooses the card before or after the chosen one: with none chosen, ← the last and → the first.
fn step_card(s: &mut Desk, l: &Layout, dir: isize, tx: &mut Tx) {
    let cards = l.choosable();
    if cards.is_empty() {
        return;
    }
    let to = match l.group.as_ref().and_then(|g| cards.iter().position(|c| *c == g)) {
        Some(at) => (at as isize + dir).clamp(0, cards.len() as isize - 1) as usize,
        None if dir < 0 => cards.len() - 1,
        None => 0,
    };
    let group = cards[to].clone();
    tx.scroll_into_view(card_key(&group.name()));
    choose(s, Some(group), false);
}

/// esc with a chosen card clears it; true when there was one.
pub fn clear_group(s: &mut Desk) -> bool {
    let ours = s.browser.xray().is_some_and(|x| x.id == s.tools.session);
    if s.tab != Tab::Tools || !ours || s.tools.group.is_none() {
        return false;
    }
    s.tools.group = None;
    s.tools.cursor = 0;
    true
}

/// Puts the cursor on row `j` of both tables, as for the inspector's keyboard scene.
pub fn scene_cursor(s: &mut Desk, j: usize) {
    if let Some(id) = s.browser.xray().map(|x| x.id.clone()) {
        adopt(s, &id);
        s.tools.cursor = j;
    }
}

/// For `--scene tools-group`: `Tools` with the keyboard, the first card chosen and the cursor on
/// the first row of the call log.
pub fn scene(s: &mut Desk) {
    s.pane = crate::Focus::Inspector;
    s.tab = Tab::Tools;
    let Some(x) = s.browser.xray_shared() else { return };
    adopt(s, &x.id);
    let l = Layout::of(&s.tools, &x);
    s.tools.group = l.choosable().first().map(|g| (*g).clone());
    let l = Layout::of(&s.tools, &x);
    s.tools.cursor = if l.calls.is_empty() { 0 } else { l.tool_rows() };
}
