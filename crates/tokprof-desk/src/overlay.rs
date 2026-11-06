//! What the cards over the window draw alike, the settings panel, the command palette and the key
//! help: the scrim that dims and blurs the window, the card, and keys drawn as keycaps.

use rune::{Color, El, col, el, row, text};

use crate::Desk;
use crate::pal::{self, Pal};

/// A pill, as Rex rounds its buttons and keys.
pub const PILL: f32 = 999.0;
/// A bar `share` of the way along its track, its filled part keyed `fill`.
pub fn meter(share: f64, color: Color, p: &Pal, h: f32) -> El {
    let share = share.clamp(0.0, 1.0) as f32;
    row()
        .grow(1.0)
        .min_w(0.0)
        .h(h)
        .radius(h / 2.0)
        .fill(p.hover)
        .child(el().key("fill").h(h).radius(h / 2.0).fill(color).grow(share.max(1e-4)).min_w(if share > 0.0 { 2.0 } else { 0.0 }))
        .child(el().h(h).grow((1.0 - share).max(1e-4)).min_w(0.0))
}

/// The window dimmed and blurred under a card, covering it: a press on it runs `close`, so
/// nothing behind it is pressed. The card is laid out by `place`.
pub fn scrim(key: &'static str, p: &Pal, close: fn(&mut Desk)) -> El {
    el().key(key).absolute(0.0, 0.0).w_full().h_full().fill(p.bg.alpha(0.35)).backdrop_blur(10.0).on_press(close)
}

/// A card over the window: the raised surface with a thin line and a soft shadow, its corners
/// clipped. A press on it stays on it, so a press on the scrim around it closes it.
pub fn card(key: &'static str, label: &'static str, w: f32, radius: f32, p: &Pal) -> El {
    col()
        .key(key)
        .w(w)
        .radius(radius)
        .fill(p.panel)
        .border(1.0, p.row_line)
        .shadow(24.0, 64.0, Color::BLACK.alpha(0.28))
        .clip_rounded(radius)
        .role("dialog")
        .label(label)
        .on_press(|_: &mut Desk| {})
}

/// A key drawn as a keycap.
pub fn keycap(k: &str, p: &Pal) -> El {
    keycap_on(k, p.bar, p)
}

/// A keycap filled with `fill`, for one on the bar's color.
pub fn keycap_on(k: &str, fill: Color, p: &Pal) -> El {
    row()
        .h(20.0)
        .min_w(20.0)
        .px(6.0)
        .center()
        .radius(5.0)
        .fill(fill)
        .border(1.0, p.row_line)
        .child(text(k.to_string()).font(pal::SMALL_STRONG).color(p.ink2).single_line())
}

/// Keys and what they do, as the foot of a card names them.
pub fn hint(keys: &[&str], what: &str, p: &Pal) -> El {
    let caps = keys.iter().map(|k| keycap(k, p));
    row().gap(3.0).center_y().children(caps).child(text(what.to_string()).font(pal::SMALL).color(p.ink3).single_line().pl(4.0))
}
