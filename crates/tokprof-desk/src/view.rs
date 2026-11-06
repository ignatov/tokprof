//! What the window draws: the list of sessions on the left, the X-ray or the day on the right, and
//! the footer with the keys, the filter and the status.

use std::borrow::Cow;

use rune::tok::{self, Theme};
use rune::{Color, Cx, El, Family, GrowthSpec, ScrollSpec, Span, TextStyle, col, el, oklch, oklcha, rich, row, text};
use tokprof_browse::browser::{Pane, now_ms};

use crate::Desk;
use crate::ansi::Styled;

/// The height of a row of the list, and of a line of the X-ray and the day.
pub const ROW: f32 = 24.0;
pub const LINE: f32 = 16.0;
/// The width of the list.
pub const LIST_WIDTH: f32 = 560.0;
/// The keys of the list's and the right pane's scroll viewports.
pub const LIST: &str = "list";
pub const PANE: &str = "pane";
/// The size, in characters, the browser lays the day's text out for.
pub const DAY_PANE: (u16, u16) = (200, 400);
/// The widths of the cost and the next-request columns.
const COST_WIDTH: f32 = 72.0;
const NEXT_WIDTH: f32 = 92.0;

const MONO: TextStyle = TextStyle::new(12.0, 400.0).family(Family::SystemMono).lh(LINE);
const MONO_BOLD: TextStyle = TextStyle::new(12.0, 600.0).family(Family::SystemMono).lh(LINE);
const TITLE: TextStyle = TextStyle::new(12.0, 500.0);

/// The colors of the agent column, by the agent's place in the registry, as the terminal's: magenta,
/// green, cyan and yellow.
const AGENT_COLORS: [u8; 4] = [5, 2, 6, 3];

/// One of the terminal's eight colors in the window's theme.
pub fn ansi_color(c: u8, dark: bool, t: &Theme) -> Color {
    let l = if dark { 0.76 } else { 0.52 };
    match c % 8 {
        1 => oklch(l - 0.08, 0.19, 25.0),
        2 => oklch(l - 0.02, 0.17, 148.0),
        3 => oklch(l + 0.06, 0.15, 82.0),
        4 => oklch(l - 0.06, 0.15, 258.0),
        5 => oklch(l - 0.04, 0.17, 322.0),
        6 => oklch(l, 0.12, 205.0),
        _ => t.text,
    }
}

/// The styles of a text as spans: its colors, dim text in the secondary color, bold text bold.
pub fn spans(s: &Styled, dark: bool, t: &Theme) -> Vec<Span> {
    s.runs
        .iter()
        .map(|&(start, end, style)| {
            let mut span = Span::new(start, end);
            match style.fg {
                Some(c) => span = span.color(ansi_color(c, dark, t)),
                None if style.dim => span = span.color(t.text_secondary),
                None => {}
            }
            if style.bold {
                span = span.bold();
            }
            span
        })
        .collect()
}

pub fn view(s: &Desk, cx: &mut Cx<Desk>) -> El {
    let dark = cx.system_dark();
    s.dark.set(dark);
    let t = tok::theme(dark);
    let mut body = row().key("body").w_full().grow(1.0).min_h(0.0);
    if !s.browser.zoomed() {
        body = body.child(list_pane(s, &t)).child(el().w(1.0).h_full().fill(t.separator));
    }
    col().w_full().h_full().fill(t.bg).child(body.child(right_pane(s, &t, dark))).child(el().w_full().h(1.0).fill(t.separator)).child(footer(s, &t, dark))
}

fn title_bar(key: &'static str, title: String, t: &Theme) -> El {
    row()
        .key(key)
        .w_full()
        .h(30.0)
        .px(12.0)
        .center_y()
        .child(text(title.trim().to_string()).font(TITLE).color(t.text_secondary).single_line().truncate().grow(1.0).min_w(0.0))
}

fn list_pane(s: &Desk, t: &Theme) -> El {
    let offset = s.list_offset.clone();
    let rows = col().key("rows").w_full().growth(GrowthSpec::default()).virtual_rows(
        s.browser.len(),
        ROW,
        |s: &Desk, i| Cow::Borrowed(s.browser.session(i)),
        list_row,
    );
    // Where the rows on the screen go when sessions come and go is the browser's: the poll scrolls
    // the list to the first row it keeps there.
    let list = col()
        .key(LIST)
        .w_full()
        .grow(1.0)
        .min_h(0.0)
        .scroll(ScrollSpec { stick_bottom: false })
        .on_scroll_offset(move |at| offset.set(at.offset[1]))
        .on_layout(|s: &mut Desk, r| s.list_height = r[3])
        .child(rows);
    col().key("list-pane").w(LIST_WIDTH).h_full().min_h(0.0).child(title_bar("list-title", s.browser.list_title(now_ms()), t)).child(list)
}

/// Row `i` of the list. Its handlers find the session by its id: the row may hold another one
/// by the time it is pressed.
fn list_row(s: &Desk, i: usize) -> El {
    let dark = s.dark.get();
    let t = tok::theme(dark);
    let c = s.browser.cells(i, now_ms());
    let selected = s.browser.selected() == Some(i);
    let (press, open) = (s.browser.session(i).to_string(), s.browser.session(i).to_string());
    let share = bar_share(c.usd, s.browser.dearest());
    let next = match &c.next {
        Some((cell, cold)) => text(cell.clone()).key("next").font(MONO).color(ansi_color(if *cold { 1 } else { 2 }, dark, &t)).single_line(),
        None => text(String::new()).key("next").font(MONO),
    };
    let mut cost = col().key("cost").w(COST_WIDTH).items_end().gap(2.0).child(text(c.cost.clone()).key("usd").font(MONO_BOLD).color(t.text).single_line());
    if let Some(share) = share {
        // Under the amount, from its right edge, as long as the share of the dearest session.
        cost = cost.child(row().w_full().justify_end().child(el().key("bar").w(COST_WIDTH * share).h(2.0).radius(1.0).fill(tok::accent(0, dark))));
    }
    row()
        .w_full()
        .h(ROW)
        .px(10.0)
        .gap(10.0)
        .center_y()
        .radius(5.0)
        .fill(if selected { oklcha(0.62, 0.12, 256.0, if dark { 0.26 } else { 0.18 }) } else { Color::TRANSPARENT })
        .hover(t.hover)
        .role("option")
        .label(format!("{} {} {} {}", c.time, c.agent, c.project, c.cost))
        .checked(selected)
        .on_press_tx(move |s: &mut Desk, tx| {
            s.select_session(&press);
            tx.scroll_to_y(PANE, 0.0);
        })
        .on_double_press(move |s: &mut Desk| {
            s.select_session(&open);
            s.browser.zoom_in();
        })
        .child(text(c.time).key("time").font(MONO).color(t.text_secondary).single_line())
        .child(
            text(format!("{:<6}", c.agent)).key("agent").font(MONO).color(ansi_color(AGENT_COLORS[c.agent_place % AGENT_COLORS.len()], dark, &t)).single_line(),
        )
        .child(text(c.project.to_string()).key("project").font(MONO).color(t.text).single_line().truncate_head().grow(1.0).shrink().min_w(0.0))
        .child(cost)
        .child(row().w(NEXT_WIDTH).justify_end().child(next))
}

/// The length of a session's cost bar as a share of the column: its cost as a share of the dearest
/// listed session's. A session still being costed has no bar.
pub fn bar_share(usd: Option<f64>, dearest: Option<f64>) -> Option<f32> {
    match (usd, dearest) {
        (Some(usd), Some(dearest)) if dearest > 0.0 => Some((usd / dearest).clamp(0.0, 1.0) as f32),
        (Some(_), _) => Some(0.0),
        (None, _) => None,
    }
}

/// A pane's text: the X-ray or the day with their colors, or a note while it is computed.
fn pane_text(pane: Pane<'_, Styled>, dark: bool, t: &Theme) -> El {
    match pane {
        Pane::Text(s) => rich(s.text.clone(), spans(s, dark, t)).key("pane-text").font(MONO).color(t.text).selectable(true),
        Pane::Computing(note) => text(note).key("pane-text").font(MONO).color(t.text_secondary),
        Pane::Empty(note) => text(note).key("pane-text").font(MONO).color(t.text),
    }
}

fn right_pane(s: &Desk, t: &Theme, dark: bool) -> El {
    let (title, body) = match s.browser.showing_day() {
        true => (s.browser.day_title(), pane_text(s.browser.day_pane(), dark, t)),
        false => (s.browser.report_title(), pane_text(s.browser.report(), dark, t)),
    };
    let offset = s.pane_offset.clone();
    let scroll = col()
        .key(PANE)
        .w_full()
        .grow(1.0)
        .min_h(0.0)
        .min_w(0.0)
        .scroll_xy()
        .on_scroll_offset(move |at| offset.set(at.offset[1]))
        .on_layout(|s: &mut Desk, r| s.pane_height = r[3])
        .child(col().px(12.0).pb(12.0).child(body));
    col().key("right").grow(1.0).h_full().min_w(0.0).min_h(0.0).child(title_bar("pane-title", title, t)).child(scroll)
}

fn footer(s: &Desk, t: &Theme, dark: bool) -> El {
    let b = &s.browser;
    let line = row().key("footer").w_full().h(26.0).px(12.0).center_y();
    let dim = |s: &str| text(s.to_string()).font(MONO).color(t.text_secondary).single_line();
    let plain = |s: String| text(s).font(MONO).color(t.text).single_line();
    if b.editing() {
        return line.child(dim("filter: ")).child(plain(format!("{}▌", b.filter()))).child(dim("   ⏎ done · esc clear"));
    }
    let mut line = line.child(dim(b.footer_keys().trim_start()).key("keys").truncate().shrink().min_w(0.0));
    if !b.filter().is_empty() {
        line = line.child(plain(format!("   filter: {}", b.filter())).key("filter"));
    }
    if !b.status().is_empty() {
        line = line.child(text(format!("   {}", b.status())).key("status").font(MONO).color(ansi_color(3, dark, t)).single_line());
    }
    line
}
