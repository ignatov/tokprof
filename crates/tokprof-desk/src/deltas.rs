//! Reports against the period before: each key number and each cause of `Could have been saved`
//! compared with the period of the same length just before the one shown, from the day store the
//! calendar's worker keeps (see `tokprof_core::compare`).

use rune::{Color, El, row, text};
use tokprof_browse::reports::Report;
use tokprof_core::analysis::cache::Cause;
use tokprof_core::compare::{self, Sense, Totals, Verdict};
use tokprof_core::overview::Overview;

use crate::Desk;
use crate::pal::{self, Pal};

/// What the period before is.
#[derive(Clone, Debug, PartialEq)]
pub enum Before {
    /// The store is not there yet.
    Waiting,
    /// The window runs with `--client`, which the store cannot filter by.
    Client,
    /// The store holds nothing of the window before.
    NoData((i64, i64)),
    Totals(Totals),
}

impl Before {
    /// The note of the key numbers' card.
    pub fn note(&self) -> Option<String> {
        match self {
            Before::Waiting => Some("comparing with the period before…".into()),
            Before::Client => Some("no comparison with --client".into()),
            Before::NoData(_) => Some("no earlier data".into()),
            Before::Totals(t) => {
                let mut note = format!("against {}", compare::dates((t.from_ms, t.to_ms), true));
                if t.incomplete() {
                    note += &format!(", incomplete: {}", t.gaps_note());
                }
                if !t.minor.is_empty() {
                    note += &format!("; {}", t.minor_note());
                }
                Some(note)
            }
        }
    }

    pub fn totals(&self) -> Option<&Totals> {
        match self {
            Before::Totals(t) => Some(t),
            _ => None,
        }
    }
}

/// The period before the report's, from the store the calendar's worker last read.
pub fn before(s: &Desk, report: &Report) -> Before {
    if s.calendar.client_ignored {
        return Before::Client;
    }
    let Some(store) = s.calendar.data.get().and_then(|c| c.store.clone()) else { return Before::Waiting };
    let t = Totals::before(&store.entries, &tokprof_browse::calendar::filters(&s.calendar.select), (report.since_ms, report.until_ms));
    if t.any { Before::Totals(t) } else { Before::NoData((t.from_ms, t.to_ms)) }
}

/// What identifies the store compared with, for the view's memo.
pub fn version(s: &Desk) -> Option<usize> {
    s.calendar.data.get().and_then(|c| c.store.as_ref()).map(|st| std::sync::Arc::as_ptr(st) as usize)
}

fn color(v: Verdict, p: &Pal) -> Color {
    match v {
        Verdict::Better => p.warm,
        Verdict::Worse => p.cold,
        Verdict::Neutral => p.ink3,
    }
}

/// A line of runs, each in its color.
fn runs(key: &'static str, parts: Vec<(String, Color)>) -> El {
    row().key(key).w_full().min_w(0.0).children(parts.into_iter().map(|(t, c)| text(t).font(pal::SMALL_NUM).color(c).single_line()))
}

/// The comparison lines of the key numbers, in the tiles' order: spend, avoidable, cold
/// returns, sessions, requests; none without a period before. The card's note names the
/// period before.
pub fn key_lines(o: &Overview, before: &Before, p: &Pal) -> Vec<Option<El>> {
    let Some(b) = before.totals() else { return vec![None, None, None, None, None] };
    // An incomplete period before judges nothing: every change is grey.
    let judged = |v: Verdict| if b.incomplete() { p.ink3 } else { color(v, p) };
    let neutral = |now: f64, was: f64, money: bool| (compare::delta(now, was, money), p.ink3);
    let sensed = |now: f64, was: f64, money: bool| (compare::delta(now, was, money), judged(compare::verdict(now, was, Sense::LowerIsBetter, money)));
    let (avoidable, share) = o.avoidable();
    let (returns, _) = o.cold_returns();
    let share_part =
        (compare::points(share, b.share()), judged(compare::verdict((share * 10.0).round(), (b.share() * 10.0).round(), Sense::LowerIsBetter, false)));
    vec![
        Some(runs("delta-spend", vec![neutral(o.spend_usd(), b.spend, true)])),
        Some(runs("delta-avoidable", vec![sensed(avoidable, b.avoidable, true), (" · ".into(), p.ink3), share_part])),
        Some(runs("delta-returns", vec![sensed(returns as f64, b.cold as f64, false)])),
        Some(runs("delta-sessions", vec![neutral(o.sessions() as f64, b.sessions as f64, false)])),
        Some(runs("delta-requests", vec![neutral(o.requests() as f64, b.requests as f64, false)])),
    ]
}

/// The causes of the period before that have no avoidable dollars in the period shown, most
/// first, each with `was $367 · gone`, green, or grey when the comparison is incomplete.
pub fn gone_rows(c: &tokprof_core::savings::Changes, before: &Before, p: &Pal) -> Vec<(String, String, Color)> {
    let Some(b) = before.totals() else { return Vec::new() };
    let shown = |name: &str| c.saved.iter().filter(|r| r.name == name).map(|r| r.usd).sum::<f64>();
    let color = if b.incomplete() { p.ink3 } else { p.warm };
    compare::gone(b, shown).into_iter().map(|(name, was)| (name, compare::gone_change(was), color)).collect()
}

/// The comparison of a `Could have been saved` row: `was $1,012 · ↓ 21%`, or `new`; nothing for a
/// row that is not a miss cause, or without a period before.
pub fn cause_cell(name: &str, usd: f64, before: &Before, p: &Pal) -> Option<(String, Color)> {
    let b = before.totals()?;
    Cause::ALL.iter().any(|c| c.name() == name).then_some(())?;
    let was = b.causes.get(name).copied();
    let t = compare::cause_change(usd, was);
    let c = match was.filter(|w| *w >= 0.005) {
        _ if b.incomplete() => p.ink3,
        None => p.cold,
        Some(w) => color(compare::verdict(usd, w, Sense::LowerIsBetter, true), p),
    };
    Some((t, c))
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};
    use std::sync::Arc;

    use rune::{Env, Runtime};
    use tokprof_browse::browser::Browser;
    use tokprof_core::catalog::Catalog;
    use tokprof_core::daystore::{Entry, Minute, Store};
    use tokprof_core::pricing::Pricing;
    use tokprof_core::query::Query;

    use super::*;

    const DAY: i64 = 86_400_000;

    fn fixture() -> (Arc<Catalog>, Query) {
        let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../sources/claude-code/tests/fixtures/projects");
        let catalog = Arc::new(Catalog::new(vec![Box::new(tokprof_source_claude_code::ClaudeCode::at(fixtures, Path::new("/w")))], PathBuf::from("/w")));
        let select = Query { agents: catalog.agents(), ..Query::default() };
        (catalog, select)
    }

    /// The window in Reports on the fixture's report over `window`, with a store that holds
    /// `before` dollars, $1 of it avoidable by `idle > TTL`, a day into the window before, and a
    /// request in the window shown; with `whole`, also a request a day before the window before,
    /// so that the store's data of the agent starts before it.
    fn desk_of(window: (i64, i64), before: Option<f64>, client: bool, whole: bool) -> (Runtime<Desk>, Overview) {
        let (catalog, select) = fixture();
        let o = tokprof_core::overview::collect(&catalog, &select, &Pricing::defaults(), |_, _| {}).unwrap();
        let o2 = tokprof_core::overview::collect(&catalog, &select, &Pricing::defaults(), |_, _| {}).unwrap();
        let mut desk = Desk::new(Browser::detached(catalog, Vec::new(), Pricing::defaults(), crate::frontend()), false);
        desk.reports.data.put(30, o, window, 0);
        let start = compare::before(window).0;
        let entries = before
            .map(|usd| {
                vec![Entry {
                    agent: "claude".into(),
                    main: "m".into(),
                    session: "m".into(),
                    minutes: [(whole, start - DAY, 0.0, 0.0, 1), (true, start + DAY, usd, 1.0, 4), (true, window.0 + DAY, 0.0, 0.0, 1)]
                        .into_iter()
                        .filter(|m| m.0)
                        .map(|(_, ts, usd, avoidable_usd, requests)| Minute { minute: ts / 60_000, usd, avoidable_usd, requests, ..Minute::default() })
                        .collect(),
                    misses: vec![((start + DAY) / 3_600_000, "idle > TTL".into(), 1.0)],
                    cold: vec![start + DAY],
                    ..Entry::default()
                }]
            })
            .unwrap_or_default();
        desk.calendar.data.put_with(tokprof_core::calendar::Calendar::default(), Store { entries, ..Store::default() }, 0);
        desk.calendar.client_ignored = client;
        crate::reports::show(&mut desk, crate::reports::Mode::Reports);
        let mut rt = Runtime::new(desk, crate::view::view, (1400.0, 1100.0), Env::new(true, 1.0));
        for _ in 0..30 {
            rt.frame(1.0 / 60.0);
        }
        (rt, o2)
    }

    fn desk(window: (i64, i64), before: Option<f64>, client: bool) -> (Runtime<Desk>, Overview) {
        desk_of(window, before, client, true)
    }

    fn texts(rt: &Runtime<Desk>) -> Vec<String> {
        rt.elements().into_iter().filter_map(|e| e.text).collect()
    }

    fn window() -> (i64, i64) {
        let now = 1_794_200_000_000;
        (now - 30 * DAY, now)
    }

    #[test]
    fn each_key_number_compares_with_the_period_before() {
        let (rt, o) = desk(window(), Some(0.5), false);
        let shown = texts(&rt);
        let spend = compare::delta(o.spend_usd(), 0.5, true);
        assert!(shown.contains(&spend), "{spend} in {shown:?}");
        assert!(shown.contains(&compare::delta(o.requests() as f64, 4.0, false)));
        assert!(shown.contains(&compare::delta(o.spend_usd(), 0.5, true)));
        assert!(shown.contains(&compare::delta(o.sessions() as f64, 1.0, false)));
        assert!(shown.contains(&compare::delta(o.cold_returns().0 as f64, 1.0, false)));
        assert!(shown.iter().any(|t| t.starts_with("against ") && !t.contains("incomplete")), "the card names the period before");
    }

    #[test]
    fn a_period_before_that_starts_before_an_agent_s_first_data_is_incomplete_and_grey() {
        let (rt, o) = desk_of(window(), Some(0.5), false, false);
        let shown = texts(&rt);
        let first = tokprof_core::fmt::local_time(compare::before(window()).0 + DAY, "%-d %b");
        assert!(shown.iter().any(|t| t.ends_with(&format!(", incomplete: claude only from {first}"))), "{shown:?}");
        // The changes are there, but grey.
        let p = crate::pal::pal(crate::themes::default(true));
        let cold = compare::delta(o.cold_returns().0 as f64, 1.0, false);
        assert!(shown.contains(&cold), "the cold returns' change");
        let t =
            Totals { gaps: vec![("claude".into(), 0)], causes: [("idle > TTL".to_string(), 1_012.0)].into_iter().collect(), any: true, ..Totals::default() };
        assert_eq!(cause_cell("idle > TTL", 796.0, &Before::Totals(t), &p).map(|c| c.1), Some(p.ink3));
    }

    #[test]
    fn without_earlier_data_or_with_a_client_filter_there_is_no_comparison() {
        let (rt, o) = desk(window(), None, false);
        let shown = texts(&rt);
        assert!(shown.iter().any(|t| t == "no earlier data"));
        assert!(!shown.contains(&compare::delta(o.spend_usd(), 0.0, true)));
        let (rt, _) = desk(window(), Some(0.5), true);
        assert!(texts(&rt).iter().any(|t| t == "no comparison with --client"));
    }

    #[test]
    fn a_small_agent_is_named_on_the_card_but_judges_nothing() {
        let (from, to) = (1_794_200_000_000 - 7 * DAY, 1_794_200_000_000);
        let t = Totals { from_ms: from, to_ms: to, any: true, minor: vec![("codex".into(), to - DAY, 0.03)], ..Totals::default() };
        let note = Before::Totals(t.clone()).note().unwrap();
        assert!(!t.incomplete());
        assert!(note.starts_with("against ") && note.ends_with(", 3% of spend, not judged") && note.contains("; codex only from "), "{note}");
    }

    #[test]
    fn a_cause_row_reads_what_it_was_or_new() {
        let p = crate::pal::pal(crate::themes::default(true));
        let t = Totals { causes: [("idle > TTL".to_string(), 1_012.0)].into_iter().collect(), any: true, ..Totals::default() };
        let b = Before::Totals(t);
        assert_eq!(cause_cell("idle > TTL", 796.0, &b, &p), Some(("was $1,012 · ↓ 21%".to_string(), p.warm)));
        assert_eq!(cause_cell("tools changed", 12.0, &b, &p), Some(("new".to_string(), p.cold)));
        assert_eq!(cause_cell("cache lifetime: main", 1.0, &b, &p), None, "not a miss cause");
        assert_eq!(cause_cell("idle > TTL", 796.0, &Before::Waiting, &p), None);
        // A cause of the period before with nothing now is gone, at the end.
        let t = Totals { causes: [("compaction call".to_string(), 367.17)].into_iter().collect(), any: true, ..Totals::default() };
        let rows = gone_rows(&tokprof_core::savings::Changes::default(), &Before::Totals(t.clone()), &p);
        assert_eq!(rows, [("compaction call".to_string(), "was $367 · gone".to_string(), p.warm)]);
        let partial = Totals { gaps: vec![("claude".into(), 0)], ..t };
        assert_eq!(gone_rows(&tokprof_core::savings::Changes::default(), &Before::Totals(partial), &p)[0].2, p.ink3);
    }
}
