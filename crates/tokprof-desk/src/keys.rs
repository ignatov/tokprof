//! The key help: every key of the window, grouped by where it works, over the window as a card.
//! The groups are written from the code that handles the keys, `on_key` and the keys of the
//! panes, the settings, the palette and the browser, and the tests hold them to it.

use rune::{El, Key as RuneKey, KeyEvent, ScrollSpec, Tx, col, el, row, spacer, text};

use crate::Desk;
use crate::overlay::{self, R_CARD, R_GROUP};
use crate::pal::{self, Pal};

/// A row of the key help: the keys, any of which does it, and what it does.
pub struct Keys {
    pub keys: &'static [&'static str],
    pub what: &'static str,
}

const fn k(keys: &'static [&'static str], what: &'static str) -> Keys {
    Keys { keys, what }
}

/// The groups of the key help, in the order the card draws them in its two columns.
pub const GROUPS: &[(&str, &[Keys])] = &[
    (
        "Anywhere",
        &[
            k(&["⌘K"], "command palette"),
            k(&["⌘,"], "settings"),
            k(&["⌘1", "⌘2"], "Day, Reports"),
            k(&["?"], "this key help"),
            k(&["Tab", "⇧Tab"], "keyboard to the next, previous pane"),
            k(&["⏎"], "inspector full size; in Flame, zoom in"),
            k(&["esc", "⌫"], "back from full size"),
            k(&["[", "]"], "day before, day after"),
            k(&["d"], "the selected session's day"),
            k(&["/"], "filter by project, agent or id"),
            k(&["s"], "sort by time, cost, next request"),
            k(&["r"], "sort again"),
            k(&["c"], "copy the handoff line"),
            k(&["f"], "write the JFR for IntelliJ"),
            k(&["o"], "write the HTML flame graph, open it"),
            k(&["⌘W"], "close the window"),
            k(&["q", "⌘Q", "Ctrl+C"], "quit"),
        ],
    ),
    (
        "Lanes",
        &[
            k(&["↑", "↓", "k", "j"], "the lane above, below"),
            k(&["Home", "End"], "the first, the last lane"),
            k(&["←", "→", "h", "l"], "day before, day after"),
            k(&["esc"], "keyboard back to the list"),
        ],
    ),
    (
        "Palette",
        &[
            k(&["↑", "↓"], "highlight before, after"),
            k(&["Ctrl+P", "Ctrl+N"], "highlight before, after"),
            k(&["⏎"], "run it"),
            k(&["⌫"], "delete a letter"),
            k(&["esc", "⌘K"], "close"),
        ],
    ),
    (
        "Settings",
        &[
            k(&["Tab", "⇧Tab"], "next, previous section"),
            k(&["←", "→", "h", "l"], "appearance or theme before, after"),
            k(&["↑", "↓", "k", "j"], "theme above, below"),
            k(&["Home", "End"], "the first, the last theme"),
            k(&["esc", "⏎", "⌘,"], "close"),
        ],
    ),
    (
        "List",
        &[
            k(&["↑", "↓", "k", "j"], "session above, below"),
            k(&["PgUp", "PgDn"], "a page up, down"),
            k(&["Home", "End", "g", "G"], "the first, the last session"),
            k(&["←", "→", "h", "l"], "day before, day after"),
            k(&["esc"], "clear the filter, or quit"),
            k(&["⏎"], "while typing a filter: keep it"),
        ],
    ),
    (
        "Inspector",
        &[
            k(&["←", "→", "h", "l"], "tab before, after; in Flame, frame"),
            k(&["⇧←", "⇧→"], "tab before, after, in Flame too"),
            k(&["1–9"], "the tab of that place"),
            k(&["0"], "Flame"),
            k(&["↑", "↓", "k", "j"], "row above, below, or scroll"),
            k(&["PgUp", "PgDn"], "ten rows up, down"),
            k(&["Home", "End", "g", "G"], "the first, the last row"),
            k(&["Space"], "a page down"),
            k(&["esc"], "keyboard back to the list"),
        ],
    ),
    (
        "Flame",
        &[
            k(&["←", "→", "h", "l"], "frame before, after"),
            k(&["↓", "j"], "its first child"),
            k(&["↑", "k"], "its parent"),
            k(&["⏎"], "zoom into the frame"),
            k(&["esc", "⌫"], "zoom out; at the top, as elsewhere"),
            k(&["⇧←", "⇧→"], "tab before, after"),
        ],
    ),
    (
        "Reports",
        &[
            k(&["Tab", "⇧Tab"], "next, previous card"),
            k(&["←", "→", "h", "l"], "period, or day before, after"),
            k(&["↑", "↓", "k", "j"], "row above, below"),
            k(&["PgUp", "PgDn"], "ten rows up, down"),
            k(&["Home", "End", "g", "G"], "the first, the last"),
            k(&["⏎"], "open the day or the project"),
            k(&["esc", "⌘1"], "back to Day"),
        ],
    ),
];

/// The groups in the left column; the rest go in the right one.
const LEFT: usize = 4;

/// The card's width and the width of a column of groups in it.
const CARD_W: f32 = 860.0;
const COLUMN_W: f32 = (CARD_W - 2.0 * 24.0 - 18.0) / 2.0;
/// The room between two groups of a column.
const GROUP_GAP: f32 = 14.0;

/// The key of the card's scrolling body.
pub const BODY: &str = "keys-body";

impl Desk {
    /// Opens the key help unless another card is open.
    pub fn open_help(&mut self) {
        if self.panel.is_none() && self.palette.is_none() {
            self.help = true;
        }
    }

    /// The keys of the open key help: esc and `?` close it, the page keys scroll it, and no key
    /// goes on.
    pub fn help_key(&mut self, e: &KeyEvent, tx: &mut Tx) -> bool {
        match &e.key {
            RuneKey::Escape => self.help = false,
            RuneKey::Text(t) if t == "?" && !e.mods.cmd && !e.mods.ctrl => self.help = false,
            RuneKey::PageUp | RuneKey::Up => tx.scroll_page(BODY, -1),
            RuneKey::PageDown | RuneKey::Down => tx.scroll_page(BODY, 1),
            RuneKey::Text(t) if t == " " && crate::plain(e) => tx.scroll_page(BODY, 1),
            _ => {}
        }
        true
    }
}

/// The key help over the window: a card of the groups in two columns.
pub fn view(p: &Pal, height: f32) -> El {
    let column = |groups: &[(&'static str, &'static [Keys])]| col().w(COLUMN_W).gap(GROUP_GAP).children(groups.iter().map(|(name, keys)| group(name, keys, p)));
    let head = row().w_full().px(24.0).pt(20.0).pb(16.0).center_y().child(text("Keys").key("keys-title").font(pal::TITLE).color(p.ink).single_line());
    let foot = row()
        .key("keys-foot")
        .w_full()
        .px(24.0)
        .py(14.0)
        .gap(14.0)
        .center_y()
        .child(overlay::hint(&["↑", "↓"], "scroll", p))
        .child(spacer().grow(1.0))
        .child(overlay::hint(&["?", "esc"], "close", p));
    let body = col()
        .key(BODY)
        .w_full()
        .grow(1.0)
        .min_h(0.0)
        .scroll(ScrollSpec { stick_bottom: false })
        .child(row().w_full().px(24.0).py(14.0).gap(18.0).child(column(&GROUPS[..LEFT])).child(column(&GROUPS[LEFT..])));
    let card = overlay::card("keys", "Keys", CARD_W, R_CARD, p)
        .h(card_height().min(height - 80.0).max(320.0))
        .child(head)
        .child(el().w_full().h(1.0).fill(p.row_line))
        .child(if card_height() > height - 80.0 { overlay::faded(body, p.panel).grow(1.0) } else { body })
        .child(el().w_full().h(1.0).fill(p.row_line))
        .child(foot);
    overlay::scrim("keys-scrim", p, |s| s.help = false).center().child(card)
}

/// The height the card needs to show every group without scrolling: its head and line, and the
/// taller column.
fn card_height() -> f32 {
    let group_h = |keys: &[Keys]| 16.0 + 10.0 + 20.0 + keys.len() as f32 * ROW_H;
    let column_h = |groups: &[(&str, &[Keys])]| groups.iter().map(|(_, keys)| group_h(keys)).sum::<f32>() + GROUP_GAP * (groups.len() as f32 - 1.0);
    let body = column_h(&GROUPS[..LEFT]).max(column_h(&GROUPS[LEFT..])) + 28.0;
    // The head, its line, the body, the line and the foot.
    20.0 + 16.0 + 26.0 + 1.0 + body + 1.0 + 48.0
}

/// The height of a row of keys.
const ROW_H: f32 = 22.0;

/// A group of keys: its name over a soft group of rows, the keycaps left and what they do right.
fn group(name: &'static str, keys: &'static [Keys], p: &Pal) -> El {
    // The keys' column as wide as the group's widest keys, so that what they do starts near them.
    let caps_w = keys.iter().map(|k| k.keys.iter().map(|c| keycap_w(c) + 3.0).sum::<f32>()).fold(40.0, f32::max) + 8.0;
    let rows = keys.iter().map(|k| {
        row()
            .w_full()
            .h(ROW_H)
            .gap(12.0)
            .center_y()
            .child(row().w(caps_w).gap(3.0).center_y().children(k.keys.iter().map(|c| overlay::keycap_on(c, p.panel, p))))
            .child(text(k.what).font(pal::BODY).color(p.ink).single_line().truncate().grow(1.0).shrink().min_w(0.0))
    });
    col()
        .key(format!("keys-{}", name.to_lowercase()))
        .w_full()
        .gap(10.0)
        .child(text(name).font(pal::HEAD).color(p.ink2).single_line())
        .child(col().w_full().px(14.0).py(10.0).radius(R_GROUP).fill(p.bar).children(rows))
}

/// About how wide a keycap of `k` is: its text and its padding, at least square.
fn keycap_w(k: &str) -> f32 {
    (pal::text_w(k, pal::SMALL_STRONG) + 12.0).max(20.0)
}
