//! The window's colors and type: those of the profiler sketch the author chose, in light and
//! dark, and the system font at a few sizes, with tabular figures where numbers line up.

use rune::{Color, TextStyle};

/// A color from its sRGB hex.
fn hx(s: &str) -> Color {
    rune::color::hex(s).unwrap_or(Color::BLACK)
}

/// The window's colors in one appearance.
pub struct Pal {
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
    pub on_accent2: Color,
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
    pub read: Color,
    pub write: Color,
    pub out: Color,
    pub agents: [Color; 4],
    pub work: Color,
    pub wait: Color,
    pub message: Color,
    pub question: Color,
    pub stop: Color,
    pub you: Color,
    pub tail: Color,
    /// The context and other bars that are not money.
    pub neutral: Color,
}

pub fn pal(dark: bool) -> Pal {
    let pick = |light: &str, dark_hex: &str| if dark { hx(dark_hex) } else { hx(light) };
    Pal {
        bg: pick("#f5f5f3", "#1b1b1e"),
        bar: pick("#ebebe7", "#232326"),
        track: pick("#fcfcfb", "#161619"),
        row_line: pick("#efefeb", "#222225"),
        line: pick("#dadad5", "#2f2f33"),
        hour: pick("#ececE8", "#222226"),
        ink: pick("#1a1a1c", "#ededef"),
        ink2: pick("#5e5e63", "#a4a4ab"),
        ink3: pick("#8f8f95", "#6e6e75"),
        panel: pick("#ffffff", "#212124"),
        hover: if dark { Color::WHITE.alpha(0.05) } else { Color::BLACK.alpha(0.04) },
        accent: pick("#0a6cf0", "#4d97ff"),
        on_accent: Color::WHITE,
        on_accent2: Color::WHITE.alpha(0.82),
        sel: if dark { hx("#5096ff").alpha(0.15) } else { hx("#0a6cf0").alpha(0.09) },
        now: pick("#d23a32", "#ff6b61"),
        later: if dark { Color::WHITE.alpha(0.025) } else { Color::BLACK.alpha(0.025) },
        warm: pick("#167a33", "#4cc46a"),
        cold: pick("#c12c2c", "#ff6f66"),
        warm_dot: hx("#0ca30c"),
        cold_dot: hx("#d03b3b"),
        cold_bg: pick("#fff5f3", "#2a1d1d"),
        read: pick("#2a78d6", "#3987e5"),
        write: pick("#eb6834", "#d95926"),
        out: pick("#1baf7a", "#199e70"),
        agents: [pick("#8748d1", "#b98cf5"), pick("#2f5fd0", "#86a5ff"), pick("#0f8f8f", "#4cc9c9"), pick("#b07a00", "#e0b040")],
        work: pick("#2e8b4a", "#4cc26f"),
        wait: if dark { hx("#e6b63c").alpha(0.5) } else { hx("#d6a014").alpha(0.55) },
        message: pick("#1a87a8", "#4dbad8"),
        question: pick("#b2409c", "#de80cb"),
        stop: pick("#cc3530", "#ff6b61"),
        you: if dark { hx("#82a0eb").alpha(0.42) } else { hx("#506ec8").alpha(0.42) },
        tail: if dark { hx("#4cc26f").alpha(0.3) } else { hx("#2e8b4a").alpha(0.3) },
        neutral: pick("#8592a8", "#66768f"),
    }
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
