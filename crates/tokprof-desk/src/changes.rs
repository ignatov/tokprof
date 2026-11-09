//! What to change, in Reports: what could have been saved, by cause; the ways to come back after
//! a cold break; and the habits that cost the most, each with what tokprof's advice says to do.
//! The rows are those `tokprof share` prints for the period (see `tokprof_core::savings`); a row
//! split by session opens its top sessions under it, and a session opens in Day.

use rune::{Color, El, Tx, col, el, row, spacer, text};
use tokprof_browse::browser::now_ms;
use tokprof_core::fmt::{int, usd};
use tokprof_core::pool::Row;
use tokprof_core::rhythm::RETURNS_HINT;
use tokprof_core::savings::{Changes, SessionCost};

use crate::Desk;
use crate::overlay::{R_MARK, R_ROW};
use crate::pal::{self, Pal};
use crate::reports::{self, Mode, Part};

/// The sessions a row opens.
pub const SESSIONS: usize = 5;
/// The width of a row's name in the savings, and in the habits, whose tool hints are sentences.
const NAME_W: f32 = 230.0;
const HABIT_W: f32 = 400.0;

/// A line of a card the cursor moves through: a row, or a session of the row that is open.
#[derive(Clone, Debug, PartialEq)]
pub enum Line {
    Row(String),
    Session(String, usize),
}

/// The rows of a part of what to change.
fn rows_of(c: &Changes, part: Part) -> &[Row] {
    match part {
        Part::Saved => &c.saved,
        Part::Habits => &c.habits,
        _ => &[],
    }
}

/// The lines of a part, with the sessions of the row that is open under it.
pub fn lines(c: &Changes, part: Part, open: Option<&(Part, String)>) -> Vec<Line> {
    if part == Part::Ways {
        // Without cold returns the card shows no ways, and the cursor has no row in it.
        if c.returns == 0 {
            return Vec::new();
        }
        return c.ways.iter().map(|w| Line::Row(w.name.clone())).collect();
    }
    let mut out = Vec::new();
    for r in rows_of(c, part) {
        out.push(Line::Row(r.name.clone()));
        if open.is_some_and(|(p, name)| *p == part && *name == r.name) {
            let n = c.sessions_of(&r.name).map_or(0, |l| l.len().min(SESSIONS));
            out.extend((0..n).map(|i| Line::Session(r.name.clone(), i)));
        }
    }
    out
}

/// Opens or closes the sessions of a row; a row not split by session has none to open.
pub fn toggle(s: &mut Desk, part: Part, name: &str) {
    let r = &mut s.reports;
    let Some(c) = r.changes() else { return };
    if c.sessions_of(name).is_none_or(|l| l.is_empty()) {
        return;
    }
    let key = (part, name.to_string());
    r.open = if r.open.as_ref() == Some(&key) { None } else { Some(key) };
    // The cursor stays on the row.
    let at = lines(r.changes().expect("read above"), part, r.open.as_ref()).iter().position(|l| *l == Line::Row(name.to_string()));
    r.part = part;
    r.row = at.unwrap_or(0);
}

/// Opens a session behind a row in Day: selected in the list, with its day in the lanes. One the
/// list does not hold, as its filter left it out, says so in the bar.
pub fn open_session(s: &mut Desk, x: &SessionCost, tx: &mut Tx) {
    let Some(row) = s.browser.row_of_short(&x.agent, &x.session) else {
        s.status = (format!("session {} is not in the list", x.session), std::time::Instant::now());
        return;
    };
    let id = s.browser.session(row).to_string();
    reports::show(s, Mode::Day);
    s.open_lane(&id, tx);
    // Not the key `d`, which a filter being typed would take.
    s.browser.show_selected_day();
}

/// What ⏎ does on the cursor's line of a part.
pub fn enter(s: &mut Desk, part: Part, tx: &mut Tx) {
    let r = &s.reports;
    let Some(c) = r.changes() else { return };
    match lines(c, part, r.open.as_ref()).get(r.row).cloned() {
        Some(Line::Row(name)) => toggle(s, part, &name),
        Some(Line::Session(name, i)) => {
            if let Some(x) = c.sessions_of(&name).and_then(|l| l.get(i)).cloned() {
                open_session(s, &x, tx);
            }
        }
        None => {}
    }
}

/// How many lines the cursor moves through in a part.
pub fn line_count(s: &Desk, part: Part) -> usize {
    s.reports.changes().map_or(0, |c| lines(c, part, s.reports.open.as_ref()).len())
}

/// The cards of what to change: the savings and the ways back side by side, the habits under
/// them.
pub fn cards(s: &Desk, c: &Changes, p: &Pal) -> El {
    let r = &s.reports;
    let total: f64 = c.saved.iter().map(|x| x.usd).sum();
    let share: f64 = c.saved.iter().map(|x| x.share).sum();
    let saved_aside = format!("{} could have been saved, {:.1}% of spend", usd(total), 100.0 * share);
    let top = row()
        .w_full()
        .gap(20.0)
        .items_start()
        .child(reports::card(Some(Part::Saved), "Could have been saved", Some(saved_aside), saved(s, c, p), r, p).grow(3.0))
        .child(reports::card(Some(Part::Ways), "Coming back after a cold break", Some("as tokprof bench replays it".into()), ways(s, c, p), r, p).grow(2.0));
    col().key("rep-changes").w_full().gap(20.0).child(top).child(reports::card(
        Some(Part::Habits),
        "Habits that cost the most",
        Some("where to look, not what could be saved; the rows overlap".into()),
        habits(s, c, p),
        r,
        p,
    ))
}

/// The head of a card's columns.
fn head(cols: &[(&str, f32)], p: &Pal) -> El {
    row().w_full().h(22.0).px(6.0).gap(10.0).center_y().children(cols.iter().map(|(t, w)| {
        let t = text(t.to_string()).font(pal::SMALL_STRONG).color(p.ink2).single_line();
        match *w {
            0.0 => row().grow(1.0).min_w(0.0).child(t),
            w => row().w(w).justify_end().child(t),
        }
    }))
}

fn amount_cell(t: String, w: f32, color: Color, font: rune::TextStyle) -> El {
    row().w(w).justify_end().child(text(t).font(font).color(color).single_line())
}

/// A row with its bar, its amounts and the advice under it, and its sessions under that while it
/// is open.
/// The savings carry their dollars per active hour; the habits do not.
/// The width of the column of the period before.
const BEFORE_W: f32 = 150.0;

#[allow(clippy::too_many_arguments)]
fn money_row(s: &Desk, c: &Changes, part: Part, at: usize, r: &Row, first: f64, before: Option<&crate::deltas::Before>, p: &Pal) -> (El, usize) {
    let rate = part == Part::Saved;
    let st = &s.reports;
    let split = c.sessions_of(&r.name).is_some_and(|l| !l.is_empty());
    let open = st.open.as_ref().is_some_and(|(q, n)| *q == part && *n == r.name);
    let chevron = if !split {
        " "
    } else if open {
        "▾"
    } else {
        "▸"
    };
    let name = r.name.clone();
    let mut line = row()
        .w_full()
        .h(26.0)
        .px(6.0)
        .gap(10.0)
        .center_y()
        .child(row().w(12.0).child(text(chevron).font(pal::SMALL).color(p.ink3).single_line()))
        .child(text(r.name.clone()).key("label").font(pal::BODY).color(p.ink).single_line().truncate().w(if rate { NAME_W } else { HABIT_W } - 22.0))
        .child(crate::overlay::meter(r.usd / first.max(1e-9), p.accent.alpha(0.75), p, 6.0))
        .child(amount_cell(format!("{:.1}%", 100.0 * r.share), 52.0, p.ink2, pal::BODY_NUM))
        .child(amount_cell(usd(r.usd), 70.0, p.accent, pal::STRONG_NUM));
    if rate {
        line = line.child(amount_cell(r.per_hour.map_or("–".into(), usd), 104.0, p.ink2, pal::BODY_NUM));
    }
    if let Some(b) = before.filter(|b| b.totals().is_some()) {
        let (t, color) = crate::deltas::cause_cell(&r.name, r.usd, b, p).unwrap_or_else(|| ("–".into(), p.ink3));
        line = line.child(amount_cell(t, BEFORE_W, color, pal::BODY_NUM).key("before"));
    }
    let mut line = reports::cursor_row(line, part, at, s, p).role("button").label(r.name.clone());
    if split {
        line = line.on_press(move |s: &mut Desk| toggle(s, part, &name));
    } else {
        let name = r.name.clone();
        line = line.on_press(move |s: &mut Desk| {
            let at = line_count_until(s, part, &name);
            s.reports.part = part;
            s.reports.row = at;
        });
    }
    let mut out = col().w_full().gap(1.0).child(line);
    // A tool hint's name is its sentence already.
    if let Some(h) = c.hint(&r.name).filter(|_| !r.name.starts_with("tool hint: ")) {
        out = out.child(text(h.to_string()).key("hint").w_full().wrap().pl(28.0).pr(6.0).pb(4.0).font(pal::SMALL).color(p.ink3));
    }
    let mut used = 1;
    if open && let Some(list) = c.sessions_of(&r.name) {
        for (i, x) in list.iter().take(SESSIONS).enumerate() {
            out = out.child(session_line(s, part, at + 1 + i, x, p));
            used += 1;
        }
    }
    (out, used)
}

/// The place of a row among the lines of its part.
fn line_count_until(s: &Desk, part: Part, name: &str) -> usize {
    s.reports.changes().and_then(|c| lines(c, part, s.reports.open.as_ref()).iter().position(|l| *l == Line::Row(name.to_string()))).unwrap_or(0)
}

/// A session behind a row: its title, or its project and id while it has none, and its dollars;
/// a click opens it in Day.
fn session_line(s: &Desk, part: Part, at: usize, x: &SessionCost, p: &Pal) -> El {
    let b = &s.browser;
    let listed = b.row_of_short(&x.agent, &x.session);
    let (title, project) = match listed {
        Some(row) => {
            let cells = b.cells(row, now_ms());
            (cells.title.map_or_else(|| format!("{} {}", cells.agent, x.session), str::to_string), cells.project.to_string())
        }
        None => (format!("{} {}", x.agent.name(), x.session), "not in the list".to_string()),
    };
    let color = if listed.is_some() { p.ink } else { p.ink3 };
    let target = x.clone();
    let line = row()
        .w_full()
        .h(24.0)
        .pl(28.0)
        .pr(6.0)
        .gap(10.0)
        .center_y()
        .child(el().w(6.0).h(6.0).radius(R_MARK).fill(p.accent.alpha(0.6)))
        .child(text(title).key("session-title").font(pal::BODY).color(color).single_line().truncate().shrink().min_w(0.0))
        .child(text(project).font(pal::SMALL).color(p.ink3).single_line().truncate().shrink().min_w(0.0))
        .child(spacer().grow(1.0))
        .child(amount_cell(usd(x.usd), 70.0, p.ink, pal::BODY_NUM));
    reports::cursor_row(line, part, at, s, p).role("button").label(x.session.clone()).on_press_tx(move |s: &mut Desk, tx| open_session(s, &target, tx))
}

fn saved(s: &Desk, c: &Changes, p: &Pal) -> El {
    if c.saved.is_empty() {
        return text("No cache misses that could have been saved.").font(pal::BODY).color(p.ink2).single_line();
    }
    let first = c.saved.first().map_or(0.0, |r| r.usd);
    let before = s.reports.report().map_or(crate::deltas::Before::Waiting, |r| crate::deltas::before(s, r));
    let mut cols = vec![("cause", 0.0), ("share", 52.0), ("avoidable", 70.0), ("per active hour", 104.0)];
    if before.totals().is_some() {
        cols.push(("period before", BEFORE_W));
    }
    let mut out = col().w_full().gap(2.0).child(head(&cols, p));
    let mut at = 0;
    for r in &c.saved {
        let (e, used) = money_row(s, c, Part::Saved, at, r, first, Some(&before), p);
        out = out.child(e);
        at += used;
    }
    out.child(
        text(format!("over {} active hours; a row with ▸ opens its top sessions", int(c.active_hours.round() as u64)))
            .pl(6.0)
            .pt(4.0)
            .font(pal::SMALL)
            .color(p.ink3)
            .single_line(),
    )
}

fn habits(s: &Desk, c: &Changes, p: &Pal) -> El {
    if c.habits.is_empty() {
        return text("No habits to show.").font(pal::BODY).color(p.ink2).single_line();
    }
    let first = c.habits.first().map_or(0.0, |r| r.usd);
    let mut out = col().w_full().gap(2.0).child(head(&[("habit", 0.0), ("share", 52.0), ("cost", 70.0)], p));
    let mut at = 0;
    for r in &c.habits {
        let (e, used) = money_row(s, c, Part::Habits, at, r, first, None, p);
        out = out.child(e);
        at += used;
    }
    out
}

/// The ways to come back after a cold break, the cheapest marked, and the rhythm's advice.
fn ways(s: &Desk, c: &Changes, p: &Pal) -> El {
    if c.ways.is_empty() || c.returns == 0 {
        return text("No cold returns to replay.").font(pal::BODY).color(p.ink2).single_line();
    }
    let cheapest = c.cheapest().map(|w| w.name.clone());
    let mut out = col().w_full().gap(2.0).child(head(&[("way", 0.0), ("cost", 70.0), ("of resume", 70.0), ("cheapest in", 96.0)], p));
    for (i, w) in c.ways.iter().enumerate() {
        let best = cheapest.as_deref() == Some(w.name.as_str());
        let mut name = row().grow(1.0).min_w(0.0).gap(8.0).center_y().child(text(w.name.clone()).key("label").font(pal::BODY).color(p.ink).single_line());
        if best {
            name = name.child(
                row()
                    .key("cheapest")
                    .h(18.0)
                    .px(8.0)
                    .center_y()
                    .radius(crate::overlay::PILL)
                    .fill(p.accent.alpha(0.16))
                    .child(text("cheapest").font(pal::SMALL_STRONG).color(p.accent).single_line()),
            );
        }
        let line = row()
            .w_full()
            .h(26.0)
            .px(6.0)
            .gap(10.0)
            .center_y()
            .child(name)
            .child(amount_cell(usd(w.usd), 70.0, if best { p.accent } else { p.ink }, pal::STRONG_NUM))
            .child(amount_cell(format!("{:.1}%", 100.0 * w.of_resume), 70.0, p.ink2, pal::BODY_NUM))
            .child(amount_cell(format!("{} of {}", int(w.cheapest), int(c.returns)), 96.0, p.ink2, pal::BODY_NUM));
        out = out.child(reports::cursor_row(line, Part::Ways, i, s, p).radius(R_ROW));
    }
    let resume = &c.ways[0];
    let line = format!("{} cold returns cost {} as resumed.", int(c.returns), usd(resume.usd));
    out.child(text(line).key("ways-line").pl(6.0).pt(6.0).font(pal::SMALL_STRONG).color(p.ink2).single_line())
        .child(text(RETURNS_HINT).key("hint").w_full().wrap().px(6.0).font(pal::SMALL).color(p.ink3))
}
