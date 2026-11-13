//! Month and Quarter against the same point of the period before: the lines under the key
//! numbers, a change or the pace per day, and the comparison of each cause of `Could have been
//! saved`, from the period's sums of the day store (see `tokprof_core::period`).

use rune::Color;
use tokprof_core::analysis::cache::Cause;
use tokprof_core::compare::{self, Sense, Verdict};
use tokprof_core::fmt::int;
use tokprof_core::period::{KeyNumbers, PeriodSums};

use crate::Desk;
use crate::pal::Pal;
use crate::reports::{big_hours, usd_whole};

fn color(v: Verdict, p: &Pal) -> Color {
    match v {
        Verdict::Better => p.warm,
        Verdict::Worse => p.cold,
        Verdict::Neutral => p.ink2,
    }
}

/// A change of hours: `↑ 5.7h (+19%)`, `= 0h`; whole hours from ten up.
fn hours_delta(now_ms: i64, was_ms: i64) -> String {
    let (now, was) = (now_ms as f64 / 3_600_000.0, was_ms as f64 / 3_600_000.0);
    let d = now - was;
    if d.abs() < 0.05 {
        return "= 0h".to_string();
    }
    let arrow = if d > 0.0 { "↑" } else { "↓" };
    let percent = if was.abs() > 1e-9 { format!(" ({}{:.0}%)", if d > 0.0 { "+" } else { "−" }, (100.0 * d / was).abs()) } else { String::new() };
    let amount = if d.abs() >= 10.0 { format!("{:.0}h", d.abs()) } else { format!("{:.1}h", d.abs()) };
    format!("{arrow} {amount}{percent}")
}

/// The lines under the seven key numbers, in their order: spend, agent hours, at the keyboard,
/// sessions, avoidable (its change and its share's in points, then its cold returns'), saved by
/// the cache and commits; each a line of runs in their colors. Without a comparison, each number's
/// pace per day.
pub fn key_lines(sums: &PeriodSums, p: &Pal) -> Vec<Vec<Vec<(String, Color)>>> {
    let k = &sums.key;
    if let Some(b) = sums.before.numbers() {
        let plain = |t: String| vec![vec![(t, p.ink2)]];
        let money = |now: f64, was: f64| (compare::delta(now, was, true), color(compare::verdict(now, was, Sense::LowerIsBetter, true), p));
        let share = |x: &KeyNumbers| 100.0 * x.avoidable_share().unwrap_or(0.0);
        let points = (
            compare::points(share(k), share(b)),
            color(compare::verdict((share(k) * 10.0).round(), (share(b) * 10.0).round(), Sense::LowerIsBetter, false), p),
        );
        let cold = (compare::delta(k.cold as f64, b.cold as f64, false), color(compare::verdict(k.cold as f64, b.cold as f64, Sense::LowerIsBetter, false), p));
        return vec![
            plain(compare::delta(k.usd, b.usd, true)),
            plain(hours_delta(k.agent_minutes as i64 * 60_000, b.agent_minutes as i64 * 60_000)),
            plain(hours_delta(k.keyboard_ms, b.keyboard_ms)),
            plain(compare::delta(k.sessions as f64, b.sessions as f64, false)),
            vec![vec![money(k.avoidable_usd, b.avoidable_usd), (" · ".into(), p.ink3), points], vec![cold, (" cold returns".into(), p.ink3)]],
            plain(compare::delta(k.saved_usd, b.saved_usd, true)),
            plain(compare::delta(k.commits as f64, b.commits as f64, false)),
        ];
    }
    let pace = |v: f64, f: &dyn Fn(f64) -> String| sums.pace(v).map(|x| vec![vec![(format!("{} a day", f(x)), p.ink2)]]).unwrap_or_default();
    let money = |x: f64| usd_whole(x);
    let hours = |x: f64| big_hours(x as i64);
    let n = |x: f64| if x >= 10.0 { int(x.round() as u64) } else { format!("{x:.1}") };
    vec![
        pace(k.usd, &money),
        pace(k.agent_minutes as f64 * 60_000.0, &hours),
        pace(k.keyboard_ms as f64, &hours),
        pace(k.sessions as f64, &n),
        pace(k.avoidable_usd, &money),
        sums.saved_multiple().map(|t| vec![vec![(t, p.ink2)]]).unwrap_or_default(),
        pace(k.commits as f64, &n),
    ]
}

/// The numbers the causes compare with: the period before's, when the key numbers have a
/// comparison and the window does not filter by client.
pub fn causes_before(s: &Desk) -> Option<&KeyNumbers> {
    if s.reports.client_ignored {
        return None;
    }
    s.reports.sums().and_then(|x| x.before.numbers())
}

/// The comparison of a `Could have been saved` row: `was $1,012 · ↓ 21%`, or `new`; nothing for a
/// row that is not a miss cause.
pub fn cause_cell(name: &str, usd: f64, before: &KeyNumbers, p: &Pal) -> Option<(String, Color)> {
    Cause::ALL.iter().any(|c| c.name() == name).then_some(())?;
    let was = before.causes.get(name).copied();
    let t = compare::cause_change(usd, was);
    let c = match was.filter(|w| *w >= 0.005) {
        None => p.cold,
        Some(w) => color(compare::verdict(usd, w, Sense::LowerIsBetter, true), p),
    };
    Some((t, c))
}

/// The causes of the period before with no avoidable dollars in the period shown, most first,
/// each with `was $367 · gone` in green; none without a comparison.
pub fn gone_rows(s: &Desk, p: &Pal) -> Vec<(String, String, Color)> {
    if causes_before(s).is_none() {
        return Vec::new();
    }
    let Some(sums) = s.reports.sums() else { return Vec::new() };
    sums.gone_causes().into_iter().map(|(name, was)| (name, compare::gone_change(was), p.warm)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_cause_row_reads_what_it_was_or_new() {
        let p = crate::pal::pal(crate::themes::default(true));
        let b = KeyNumbers { causes: [("idle > TTL".to_string(), 1_012.0)].into_iter().collect(), ..KeyNumbers::default() };
        assert_eq!(cause_cell("idle > TTL", 796.0, &b, &p), Some(("was $1,012 · ↓ 21%".to_string(), p.warm)));
        assert_eq!(cause_cell("tools changed", 12.0, &b, &p), Some(("new".to_string(), p.cold)));
        assert_eq!(cause_cell("cache lifetime: main", 1.0, &b, &p), None, "not a miss cause");
    }

    #[test]
    fn hours_change_in_whole_hours_from_ten() {
        assert_eq!(hours_delta(36 * 3_600_000, 30 * 3_600_000), "↑ 6.0h (+20%)");
        assert_eq!(hours_delta(221 * 3_600_000, 180 * 3_600_000), "↑ 41h (+23%)");
        assert_eq!(hours_delta(3_600_000, 3_600_000), "= 0h");
    }
}
