//! The day view: the logs active around one day, read into the text `tokprof day` prints and
//! the browser shows.

use std::time::Instant;

use anyhow::{Result, bail};

use tokprof_core::analysis::{self, Filter};
use tokprof_core::catalog::Catalog;
use tokprof_core::fmt::Style;
use tokprof_core::pauses::Pauses;
use tokprof_core::pricing::Pricing;
use tokprof_core::query::Query;
use tokprof_core::rhythm::{self, Offset, Timeline, View};
use tokprof_core::scan::{OnUnreadable, scan};
use tokprof_core::source::SessionRef;
use tokprof_core::Session;

const HOUR_MS: i64 = 3_600_000;

/// A day view and what it read.
pub struct Day {
    /// The local date of the day shown, as days since 1970-01-01.
    pub date: i64,
    pub text: String,
    /// The line about the logs read.
    pub read: String,
    /// The warnings about logs that could not be read.
    pub warnings: Vec<String>,
    /// The local hour the strip starts at, so that other days can start theirs there too.
    pub axis_hour: i64,
}

/// The logs a day view reads: main logs last written at or after `from` and created before
/// `to`, whose last line with a time is at or after `from`.
pub fn logs_around(catalog: &Catalog, select: &Query, window: (i64, i64)) -> Vec<SessionRef> {
    catalog
        .discover(&select.agents)
        .into_iter()
        .filter(|f| !f.subagent && select.holds(f) && around(f.modified_ms(), f.created_ms, || catalog.last_active(f), window))
        .collect()
}

/// Whether a log is read for a window: last written at or after its start, created before its
/// end when the file system keeps a creation time, and with its last line with a time at or after
/// its start. The last line with a time is read from the end of the log, and only for the logs
/// that pass the other two.
fn around(modified_ms: i64, created_ms: Option<i64>, last_active: impl FnOnce() -> i64, (from, to): (i64, i64)) -> bool {
    modified_ms >= from && created_ms.is_none_or(|c| c < to) && last_active() >= from
}

/// The window of logs around a day: from 18:00 the day before to 12:00 the day after. Without a
/// date, from 18:00 two days before the date of `now_ms`, so that the day of the latest action is
/// in it even before the morning.
pub fn window(date: Option<i64>, now_ms: i64, offset: Offset) -> (i64, i64) {
    match date {
        Some(d) => (rhythm::midnight(d, offset) - 6 * HOUR_MS, rhythm::midnight(d + 1, offset) + 12 * HOUR_MS),
        None => (rhythm::midnight(rhythm::date_of(now_ms, offset) - 1, offset) - 6 * HOUR_MS, now_ms + 1),
    }
}

/// The day view of a date, or of the day of the latest action, for a strip at most `columns`
/// wide over the 24 hours from `axis_hour` on the day's date, by default from the hour the day
/// starts in.
#[allow(clippy::too_many_arguments)]
pub fn build(catalog: &Catalog, select: &Query, pricing: &Pricing, date: Option<i64>, now_ms: i64, columns: i64, axis_hour: Option<i64>, style: &Style) -> Result<Day> {
    let started = Instant::now();
    let offset: Offset = &rhythm::local_offset;
    let (from, to) = window(date, now_ms, offset);
    let files = logs_around(catalog, select, (from, to));
    let filter = Filter { since_ms: Some(from), until_ms: Some(to), client: select.client.clone() };
    let temp = temp_folders();
    let ((timelines, mut cold), stats) = scan(
        catalog,
        &files,
        select,
        OnUnreadable::Skip,
        || (Vec::new(), Pauses::new(filter.clone())),
        |(timelines, cold): &mut (Vec<Timeline>, Pauses), s, _| {
            if !keeps_client(s, &filter) {
                return;
            }
            timelines.push(Timeline::of(s, &temp));
            if s.chains.iter().flat_map(|c| &c.requests).any(|r| filter.keeps(r)) {
                analysis::analyze(s, pricing, &filter, cold);
            }
        },
        |mut a, b| {
            a.0.extend(b.0);
            a.1.merge(b.1);
            a
        },
    )?;
    cold.finish();
    let read = crate::sources(catalog, &files, started.elapsed().as_secs_f64());
    let view = View::new(timelines, now_ms, offset);
    let date = match date.or_else(|| view.days.latest()) {
        Some(d) => d,
        None if files.is_empty() => bail!("no session logs found around this day"),
        None => rhythm::date_of(now_ms, offset),
    };
    let bounds = view.days.bounds(date, offset);
    let returns: Vec<_> = cold.agents.values().flat_map(|a| a.returns.iter().copied()).filter(|r| r.ts_ms >= bounds.0 && r.ts_ms < bounds.1).collect();
    let priced = cold.agents.values().any(|a| a.priced);
    let hour = axis_hour.unwrap_or_else(|| (bounds.0 + offset(bounds.0)).rem_euclid(24 * HOUR_MS) / HOUR_MS);
    let axis = axis_at(bounds.0, hour, offset);
    let text = crate::render_rhythm::day(&view, date, bounds, offset, &returns, priced, columns, Some(axis), style);
    Ok(Day { date, text, read, warnings: crate::warnings(&stats.unreadable), axis_hour: hour })
}

/// The instant at a local hour that is nearest to `start`: the axis a strip starts at.
fn axis_at(start: i64, hour: i64, offset: Offset) -> i64 {
    const DAY_MS: i64 = 24 * HOUR_MS;
    let axis = start - (start + offset(start)).rem_euclid(DAY_MS) + hour * HOUR_MS;
    if axis > start + DAY_MS / 2 {
        axis - DAY_MS
    } else if axis <= start - DAY_MS / 2 {
        axis + DAY_MS
    } else {
        axis
    }
}

/// Whether a session has a request of the client the filter asks for; any session without one.
pub fn keeps_client(s: &Session, filter: &Filter) -> bool {
    filter.client.is_none() || s.chains.iter().flat_map(|c| &c.requests).any(|r| filter.keeps_client(r.client_name()))
}

/// The temporary folders, whose sessions the rhythm leaves out: the system's, as it is written
/// and as it resolves, and on Unix `/tmp` in both forms.
pub fn temp_folders() -> Vec<String> {
    let dir = std::env::temp_dir();
    let mut out = vec![dir.to_string_lossy().to_string()];
    if let Ok(real) = dir.canonicalize() {
        out.push(real.to_string_lossy().to_string());
    }
    if cfg!(unix) {
        out.extend(["/tmp".to_string(), "/private/tmp".to_string()]);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const CET: i64 = HOUR_MS;

    fn off(_: i64) -> i64 {
        CET
    }

    /// 2026-11-02 00:00 in a zone one hour east of UTC.
    const MIDNIGHT: i64 = 1_793_574_000_000;

    #[test]
    fn the_window_of_a_day_runs_from_the_evening_before_to_the_noon_after() {
        let nov2 = rhythm::date_of(MIDNIGHT + 12 * HOUR_MS, &off);
        assert_eq!(window(Some(nov2), 0, &off), (MIDNIGHT - 6 * HOUR_MS, MIDNIGHT + 36 * HOUR_MS));
        // Without a date, at 02:10 on 11-03: from 18:00 on 11-01, so that 11-02 is in it.
        let now = MIDNIGHT + 26 * HOUR_MS + 10 * 60_000;
        assert_eq!(window(None, now, &off), (MIDNIGHT - 6 * HOUR_MS, now + 1));
    }

    #[test]
    fn the_axis_hour_is_the_one_nearest_the_days_start() {
        let at = |h: i64, m: i64| MIDNIGHT + h * HOUR_MS + m * 60_000;
        assert_eq!(axis_at(at(4, 21), 4, &off), at(4, 0), "the hour the day starts in");
        assert_eq!(axis_at(at(5, 14), 4, &off), at(4, 0), "an hour kept from another day");
        assert_eq!(axis_at(at(-1, 30), 23, &off), at(-1, 0), "a day that starts the evening before");
        assert_eq!(axis_at(at(23, 30), 4, &off), at(28, 0), "the nearest 04:00 is the next morning");
    }

    #[test]
    fn a_log_counts_by_its_last_line_with_a_time() {
        let at = |h: i64| MIDNIGHT + h * HOUR_MS;
        let nov2 = window(Some(rhythm::date_of(at(12), &off)), 0, &off);
        let nov3 = window(Some(rhythm::date_of(at(36), &off)), 0, &off);
        // Created on 10-20, last line with a time at 11-02 09:00, untimed lines on 11-03 10:00.
        let (modified, created, last) = (at(34), Some(at(-13 * 24)), at(9));
        assert!(around(modified, created, || last, nov2));
        assert!(!around(modified, created, || last, nov3), "the IDE's untimed lines do not make it active");
        // Created on 11-03 at 13:00: not read for 11-02.
        assert!(!around(at(37), Some(at(37)), || at(37), nov2));
        // The end of a log is read only when its write time and creation pass.
        assert!(!around(at(-48), created, || unreachable!(), nov2));
    }
}
