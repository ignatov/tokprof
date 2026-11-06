//! The texts of the X-ray and the day as the window draws them: the text the terminal would show,
//! without its color codes, and the byte ranges each style covers.

/// The style of a run of text, as SGR sets it: bold, dim, and a foreground color, 0 to 7 for
/// black, red, green, yellow, blue, magenta, cyan and white, 8 to 15 for their bright forms.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Sgr {
    pub bold: bool,
    pub dim: bool,
    pub fg: Option<u8>,
}

impl Sgr {
    fn apply(&mut self, code: u16) {
        match code {
            0 => *self = Sgr::default(),
            1 => self.bold = true,
            2 => self.dim = true,
            22 => (self.bold, self.dim) = (false, false),
            30..=37 => self.fg = Some((code - 30) as u8),
            39 => self.fg = None,
            90..=97 => self.fg = Some((code - 90 + 8) as u8),
            _ => {}
        }
    }
}

/// A text without its color codes.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Styled {
    pub text: String,
    /// Byte ranges of `text` in order, each with the style it has when that is not the plain one.
    pub runs: Vec<(usize, usize, Sgr)>,
    pub lines: usize,
}

/// Takes the escape codes out of `ansi`. SGR codes set the style of the text after them; other
/// control sequences are dropped.
pub fn parse(ansi: &str) -> Styled {
    let bytes = ansi.as_bytes();
    let mut text = String::with_capacity(ansi.len());
    let mut runs = Vec::new();
    let (mut style, mut start) = (Sgr::default(), 0);
    let (mut i, mut copied) = (0, 0);
    while i < bytes.len() {
        if bytes[i] != 0x1b {
            i += 1;
            continue;
        }
        // ESC is a single byte that no UTF-8 sequence holds, so the text around it is whole.
        text.push_str(&ansi[copied..i]);
        let mut end = i + 1;
        if bytes.get(end) == Some(&b'[') {
            end += 1;
            while end < bytes.len() && !(0x40..=0x7e).contains(&bytes[end]) {
                end += 1;
            }
            if bytes.get(end) == Some(&b'm') {
                let mut next = style;
                for code in ansi[i + 2..end].split(';') {
                    next.apply(code.parse().unwrap_or(0));
                }
                if next != style {
                    if style != Sgr::default() && start < text.len() {
                        runs.push((start, text.len(), style));
                    }
                    (style, start) = (next, text.len());
                }
            }
            end += 1;
        }
        i = end.min(bytes.len());
        copied = i;
    }
    text.push_str(&ansi[copied..]);
    if style != Sgr::default() && start < text.len() {
        runs.push((start, text.len(), style));
    }
    let lines = text.lines().count();
    Styled { text, runs, lines }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BOLD: Sgr = Sgr { bold: true, dim: false, fg: None };
    const DIM: Sgr = Sgr { bold: false, dim: true, fg: None };

    fn fg(c: u8) -> Sgr {
        Sgr { fg: Some(c), ..Sgr::default() }
    }

    #[test]
    fn text_without_codes_stays_as_it_is() {
        let s = parse("context   10.8%\nnext      ≈ $0.54");
        assert_eq!((s.text.as_str(), s.runs.len(), s.lines), ("context   10.8%\nnext      ≈ $0.54", 0, 2));
    }

    #[test]
    fn each_code_the_renderers_write_sets_its_style() {
        // `Style::bold`, `Style::dim` and `Style::paint`, as the X-ray and the day write them.
        let s = parse("\x1b[1mtokprof\x1b[0m · \x1b[2mdim\x1b[0m \x1b[32mwarm\x1b[0m \x1b[31mcold\x1b[0m \x1b[33m░\x1b[0m");
        assert_eq!(s.text, "tokprof · dim warm cold ░");
        let shown: Vec<(&str, Sgr)> = s.runs.iter().map(|&(a, b, st)| (&s.text[a..b], st)).collect();
        assert_eq!(shown, vec![("tokprof", BOLD), ("dim", DIM), ("warm", fg(2)), ("cold", fg(1)), ("░", fg(3))]);
        for (code, color) in [("34", 4), ("35", 5), ("36", 6), ("37", 7), ("92", 10)] {
            let s = parse(&format!("\x1b[{code}mx\x1b[0m"));
            assert_eq!(s.runs, vec![(0, 1, fg(color))], "{code}");
        }
    }

    #[test]
    fn combined_codes_set_several_parts_and_resets_end_them() {
        // The day strip's marks: `1;36` for a message, `1;31` for an interrupt.
        let s = parse("\x1b[1;36m*\x1b[0m\x1b[32m█\x1b[0m\x1b[1;31m!\x1b[0m");
        assert_eq!(s.text, "*█!");
        let bold = |c| Sgr { bold: true, ..fg(c) };
        assert_eq!(s.runs, vec![(0, 1, bold(6)), (1, 4, fg(2)), (4, 5, bold(1))]);
        let s = parse("\x1b[1m\x1b[31mA\x1b[39mB\x1b[22mC\x1b[mD");
        assert_eq!(s.text, "ABCD");
        assert_eq!(s.runs, vec![(0, 1, bold(1)), (1, 2, BOLD)], "39 drops the color, 22 the weight, and an empty code resets");
    }

    #[test]
    fn ranges_are_bytes_of_the_text_without_codes() {
        let s = parse("≈ \x1b[1m$2.62\x1b[0m once cold");
        let (a, b, _) = s.runs[0];
        assert_eq!((a, b, &s.text[a..b]), ("≈ ".len(), "≈ $2.62".len(), "$2.62"));
    }

    #[test]
    fn other_sequences_are_dropped_and_a_style_left_open_runs_to_the_end() {
        let s = parse("a\x1b[2Kb\x1b[33mc");
        assert_eq!((s.text.as_str(), s.runs.clone()), ("abc", vec![(2, 3, fg(3))]));
        assert_eq!(parse("end\x1b[").text, "end", "a sequence cut short");
        assert_eq!(parse("\x1b[0m\x1b[0m").runs, vec![], "resets with nothing between them");
    }
}
