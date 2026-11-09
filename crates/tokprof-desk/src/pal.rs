//! The window's colors and type: the colors of the chosen theme's roles, and the system font at
//! a few sizes, with tabular figures where numbers line up.
//!
//! A theme's canvas, surfaces, strokes and content give the window's background, panels, lines
//! and text; its accent the selection; its success, danger, warning and info the marks (green for
//! work and warm caches, red for cold ones, now and stops, yellow for waits and cache writes, blue
//! for reads and requests), and its terminal colors the rest (cyan for output and messages,
//! magenta for questions, and the agents).

use rune::{Color, TextStyle};

use crate::themes::Theme;

/// The window's colors in one appearance.
pub struct Pal {
    /// The theme is dark.
    pub dark: bool,
    pub bg: Color,
    /// The title bar and the separator rows of the list.
    pub bar: Color,
    /// Behind the lanes.
    pub track: Color,
    /// The lines between rows.
    pub row_line: Color,
    pub line: Color,
    pub hour: Color,
    pub ink: Color,
    pub ink2: Color,
    pub ink3: Color,
    pub panel: Color,
    pub hover: Color,
    pub accent: Color,
    pub on_accent: Color,
    /// The selected lane.
    pub sel: Color,
    pub now: Color,
    /// The time after now.
    pub later: Color,
    pub warm: Color,
    pub cold: Color,
    pub warm_dot: Color,
    pub cold_dot: Color,
    pub cold_bg: Color,
    /// The bars of the requests chart and of the chains.
    pub bars: Color,
    pub read: Color,
    pub write: Color,
    pub out: Color,
    /// The rewrite after a miss as a part of the spend; `cold` is its text.
    pub miss: Color,
    pub agents: [Color; 4],
    pub work: Color,
    pub wait: Color,
    /// The edge of a wait's bar, so that its shape shows where its color is close to work's.
    pub wait_edge: Color,
    pub message: Color,
    pub question: Color,
    pub stop: Color,
    pub you: Color,
    pub tail: Color,
    /// The context and other bars that are not money.
    pub neutral: Color,
}

/// The window's colors in a theme. The alphas of the translucent marks are lower on a light
/// theme, where the same share of a color reads stronger.
pub fn pal(t: &Theme) -> Pal {
    let c = |role: &str| t.color(role);
    let dark = t.dark();
    let (wait, you, tail) = if dark { (0.45, 0.35, 0.3) } else { (0.3, 0.22, 0.25) };
    let on_accent = c("accentOnSolid");
    // On a dark theme, the large filled marks (work, requests, the parts of the spend) sit 40%
    // of the way into the background, so they do not glare; their text colors stay. The mix is in
    // sRGB, as the eye sees it.
    let fill = |role: &str| if dark { mix_srgb(c("canvas"), c(role), DARK_FILL) } else { c(role) };
    Pal {
        dark,
        bg: c("canvas"),
        bar: c("surfaceSubtle"),
        track: c("terminalBackground"),
        row_line: c("strokeSubtle"),
        line: c("stroke"),
        hour: c("strokeSubtle"),
        ink: c("content"),
        ink2: c("contentSecondary"),
        ink3: c("contentTertiary"),
        panel: c("surfaceRaised"),
        hover: c("controlHover"),
        accent: c("accent"),
        on_accent,
        sel: c("accentDim"),
        now: c("danger"),
        later: c("content").alpha(0.025),
        warm: c("success"),
        cold: c("danger"),
        warm_dot: c("success"),
        cold_dot: c("danger"),
        cold_bg: c("dangerContainer"),
        bars: fill("info"),
        read: fill("info"),
        write: fill("warning"),
        out: fill("terminalAnsi6"),
        miss: fill("danger"),
        agents: [c("terminalAnsi5"), c("terminalAnsi4"), c("terminalAnsi6"), c("terminalAnsi3")],
        work: fill("success"),
        wait: c("warning").alpha(wait),
        wait_edge: c("warning").alpha(wait + 0.25),
        message: c("terminalAnsi6"),
        question: c("terminalAnsi5"),
        stop: c("danger"),
        you: c("info").alpha(you),
        tail: c("success").alpha(tail),
        neutral: c("contentTertiary"),
    }
}

/// How much of its color a large filled mark keeps over a dark theme's background, in sRGB.
pub const DARK_FILL: f32 = 0.6;

/// `t` of the way from `a` to `b` in sRGB, where a share of the way reads as that share to the
/// eye; Rune's own mix is in linear light.
pub fn mix_srgb(a: Color, b: Color, t: f32) -> Color {
    use rune::color::{srgb_decode, srgb_encode};
    let m = |x: f32, y: f32| srgb_decode(srgb_encode(x) + (srgb_encode(y) - srgb_encode(x)) * t);
    Color { r: m(a.r, b.r), g: m(a.g, b.g), b: m(a.b, b.b), a: a.a + (b.a - a.a) * t }
}

/// Tabular figures, so that numbers in a column line up.
const TNUM: &str = "'tnum' 1";

pub const BODY: TextStyle = TextStyle::new(12.5, 400.0);
pub const BODY_NUM: TextStyle = TextStyle::new(12.5, 400.0).features(TNUM);
pub const STRONG: TextStyle = TextStyle::new(12.5, 600.0);
pub const STRONG_NUM: TextStyle = TextStyle::new(12.5, 650.0).features(TNUM);
pub const SMALL: TextStyle = TextStyle::new(11.5, 400.0);
pub const SMALL_NUM: TextStyle = TextStyle::new(11.5, 400.0).features(TNUM);
pub const SMALL_STRONG: TextStyle = TextStyle::new(11.5, 650.0);
pub const HEAD: TextStyle = TextStyle::new(12.0, 650.0);
pub const TITLE: TextStyle = TextStyle::new(18.0, 700.0).tracking(-0.2);
pub const DAY: TextStyle = TextStyle::new(14.5, 700.0).tracking(-0.15);
pub const KPI: TextStyle = TextStyle::new(28.0, 700.0).tracking(-0.4);
pub const HOURS: TextStyle = TextStyle::new(10.5, 400.0).features(TNUM);

/// About how wide `t` is in `style`, in points: for a least width that keeps a short text whole
/// where a long one beside it gives way.
pub fn text_w(t: &str, style: TextStyle) -> f32 {
    t.chars().count() as f32 * style.size * 0.55
}
