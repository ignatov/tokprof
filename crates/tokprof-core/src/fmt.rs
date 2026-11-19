//! How numbers, durations, times and text are written in every report: token counts, dollars,
//! counts with their word, wrapped and clipped text, tables and terminal styles.

use std::fmt::Write as _;

pub fn tokens(n: f64) -> String {
    let a = n.abs();
    if a >= 1e9 {
        format!("{:.2}B", n / 1e9)
    } else if a >= 1e6 {
        format!("{:.1}M", n / 1e6)
    } else if a >= 1e3 {
        format!("{:.1}k", n / 1e3)
    } else {
        format!("{n:.0}")
    }
}

/// A whole number with its thousands apart: `1,021`.
pub fn int(n: u64) -> String {
    let digits = n.to_string();
    let mut out = String::new();
    for (i, ch) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(ch);
    }
    out
}

/// `1 miss`, `2 misses`: a count with the word for it.
pub fn count(n: impl Into<u64>, one: &str, many: &str) -> String {
    let n = n.into();
    format!("{n} {}", if n == 1 { one } else { many })
}

pub fn usd(n: f64) -> String {
    let n = n + 0.0; // no "-0.00"
    if n.abs() >= 100.0 {
        let whole = format!("{:.0}", n.abs());
        let mut out = String::new();
        for (i, ch) in whole.chars().enumerate() {
            if i > 0 && (whole.len() - i) % 3 == 0 {
                out.push(',');
            }
            out.push(ch);
        }
        format!("{}${out}", if n < 0.0 { "-" } else { "" })
    } else {
        let cents = format!("{:.2}", n.abs());
        format!("{}${cents}", if n < 0.0 && cents != "0.00" { "-" } else { "" })
    }
}

pub fn human_duration(s: f64) -> String {
    if s >= 86_400.0 {
        format!("{:.1}d", s / 86_400.0)
    } else if s >= 3_600.0 {
        format!("{:.1}h", s / 3_600.0)
    } else if s >= 60.0 {
        format!("{:.0}m", s / 60.0)
    } else {
        format!("{s:.0}s")
    }
}

/// How long a tool call took (see tool-breakdown, Format of a duration): one decimal under
/// 10 s, whole seconds under a minute, minutes with one decimal up to 10 minutes, whole minutes up
/// to an hour, then hours and days with one decimal. Halves round up.
pub fn call_time(ms: f64) -> String {
    let s = ms.max(0.0) / 1000.0;
    let one = |x: f64| (x * 10.0).round() / 10.0;
    let (m, h) = (s / 60.0, s / 3_600.0);
    if one(s) < 10.0 {
        format!("{:.1}s", one(s))
    } else if s.round() < 60.0 {
        format!("{:.0}s", s.round())
    } else if one(m) < 10.0 {
        format!("{:.1}m", one(m))
    } else if m.round() < 60.0 {
        format!("{:.0}m", m.round())
    } else if one(h) < 24.0 {
        format!("{:.1}h", one(h))
    } else {
        format!("{:.1}d", one(s / 86_400.0))
    }
}

/// The text cut to `width` characters, with an ellipsis where it was cut.
pub fn clip_end(s: &str, width: usize) -> String {
    let s = s.split_whitespace().collect::<Vec<_>>().join(" ");
    if s.chars().count() <= width {
        return s;
    }
    let head: String = s.chars().take(width - 1).collect();
    format!("{}…", head.trim_end())
}

/// Splits text into lines of at most `width` characters at spaces. `≈` stays with the amount
/// after it.
pub fn wrap(text: &str, width: usize) -> Vec<String> {
    let mut words: Vec<String> = Vec::new();
    for word in text.split(' ') {
        match words.last_mut() {
            Some(last) if last.ends_with('≈') => {
                last.push(' ');
                last.push_str(word);
            }
            _ => words.push(word.to_string()),
        }
    }
    let mut lines = vec![String::new()];
    for word in &words {
        let line = lines.last_mut().expect("never empty");
        if !line.is_empty() && line.chars().count() + 1 + word.chars().count() > width {
            lines.push(word.to_string());
        } else {
            if !line.is_empty() {
                line.push(' ');
            }
            line.push_str(word);
        }
    }
    lines
}

pub fn table(rows: &[Vec<String>], right_from: usize) -> String {
    let cols = rows.iter().map(Vec::len).max().unwrap_or(0);
    let widths: Vec<usize> = (0..cols).map(|c| rows.iter().filter_map(|r| r.get(c)).map(|s| s.chars().count()).max().unwrap_or(0)).collect();
    let mut out = String::new();
    for row in rows {
        let mut line = String::new();
        for (c, cell) in row.iter().enumerate() {
            let pad = widths[c] - cell.chars().count();
            if c >= right_from {
                let _ = write!(line, "{}{cell}  ", " ".repeat(pad));
            } else {
                let _ = write!(line, "{cell}{}  ", " ".repeat(pad));
            }
        }
        out.push_str(line.trim_end());
        out.push('\n');
    }
    out
}

pub fn local_time(ts_ms: i64, format: &str) -> String {
    chrono::DateTime::from_timestamp_millis(ts_ms).map(|t| t.with_timezone(&chrono::Local).format(format).to_string()).unwrap_or_default()
}

pub struct Style {
    color: bool,
}

impl Style {
    pub fn with_color(color: bool) -> Style {
        Style { color }
    }

    pub fn paint(&self, code: &str, s: &str) -> String {
        if self.color { format!("\x1b[{code}m{s}\x1b[0m") } else { s.to_string() }
    }

    pub fn dim(&self, s: &str) -> String {
        self.paint("2", s)
    }

    pub fn bold(&self, s: &str) -> String {
        self.paint("1", s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn call_times_show_one_decimal_under_ten_seconds() {
        let shown: Vec<String> = [80.0, 400.0, 23_000.0, 210_000.0, 1_680_000.0, 13_320_000.0].into_iter().map(call_time).collect();
        assert_eq!(shown, ["0.1s", "0.4s", "23s", "3.5m", "28m", "3.7h"]);
        assert_eq!((call_time(9_960.0), call_time(151_000.0)), ("10s".into(), "2.5m".into()));
        let edges: Vec<String> = [59_700.0, 75_000.0, 599_000.0, 3_590_000.0, 30_960_000.0].into_iter().map(call_time).collect();
        assert_eq!(edges, ["1.0m", "1.3m", "10m", "1.0h", "8.6h"]);
    }

    #[test]
    fn a_count_of_one_takes_the_singular() {
        assert_eq!(
            (count(1u64, "miss", "misses"), count(0u64, "miss", "misses"), count(2u64, "miss", "misses")),
            ("1 miss".into(), "0 misses".into(), "2 misses".into())
        );
    }

    #[test]
    fn negative_amounts_put_the_sign_first() {
        assert_eq!(
            (usd(-1.83), usd(-1234.0), usd(1.83), usd(-0.001), usd(0.0)),
            ("-$1.83".into(), "-$1,234".into(), "$1.83".into(), "$0.00".into(), "$0.00".into())
        );
    }

    #[test]
    fn long_text_is_cut_at_the_end() {
        assert_eq!(clip_end("Квартальные отчёты отдела", 12), "Квартальные…");
        assert_eq!(clip_end("a  b\nc", 10), "a b c");
    }

    #[test]
    fn the_wrap_keeps_an_estimate_with_its_amount() {
        let line = "The context fills 27% of the window. At 4 requests per prompt, each prompt costs ≈ $0.22 in reads alone.";
        assert_eq!(wrap(line, 82), ["The context fills 27% of the window. At 4 requests per prompt, each prompt costs", "≈ $0.22 in reads alone."]);
    }
}
