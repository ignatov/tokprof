//! How many agents worked at each moment of a day: the kept main sessions whose agent worked
//! then, as the rhythm's work defines it, a subagent being part of its session's work. With the
//! agent-hours, the minutes some agent worked as the calendar counts them, the share of the
//! sittings when two or more worked, and the peak.

use std::collections::BTreeSet;

use crate::fmt;
use crate::rhythm::{Lane, Timeline, duration};

const MINUTE_MS: i64 = 60_000;

/// A session that worked: its id, its label and its work spans.
pub type AtWork = (String, String, Vec<(i64, i64)>);

/// The agents at work over a day's window.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Agents {
    /// The number at work from each moment on, in time order; the last step is back to 0.
    pub steps: Vec<(i64, u32)>,
    /// Each session that worked in the window: its id, its label (the first 6 characters of the
    /// id and the title), and its work cut to the window, merged.
    pub sessions: Vec<AtWork>,
    /// The minutes the sessions' work touched, summed over sessions, and each counted once.
    pub session_minutes: u64,
    pub some_minutes: u64,
    /// The time at the keyboard as the sittings line gives it, the sittings' own time, and the
    /// part of it when two or more agents worked.
    pub keyboard_ms: i64,
    pub present_ms: i64,
    pub multi_ms: i64,
    /// The highest number at work and the first moment it is reached.
    pub peak: Option<(u32, i64)>,
}

impl Agents {
    /// The agents of these lanes at work in `[from, to)`, with the user's sittings.
    pub fn of(lanes: &[(&Timeline, &Lane)], sittings: &[(i64, i64)], keyboard_ms: i64, (from, to): (i64, i64)) -> Agents {
        let mut out = Agents { keyboard_ms, ..Agents::default() };
        let mut some: BTreeSet<i64> = BTreeSet::new();
        let mut edges: Vec<(i64, i32)> = Vec::new();
        for (t, l) in lanes {
            let merged = merge(l.work.iter().copied());
            // The calendar's rule: a minute counts when work overlaps it by more than nothing and
            // it starts in the day.
            let mut mine: BTreeSet<i64> = BTreeSet::new();
            for &(a, b) in &merged {
                for m in a.div_euclid(MINUTE_MS)..=(b - 1).div_euclid(MINUTE_MS) {
                    let start = m * MINUTE_MS;
                    if start >= from && start < to {
                        mine.insert(m);
                    }
                }
            }
            out.session_minutes += mine.len() as u64;
            some.extend(mine);
            let cut: Vec<(i64, i64)> = merged.iter().map(|&(a, b)| (a.max(from), b.min(to))).filter(|(a, b)| b > a).collect();
            if cut.is_empty() {
                continue;
            }
            for &(a, b) in &cut {
                edges.push((a, 1));
                edges.push((b, -1));
            }
            let id6: String = t.id.chars().take(6).collect();
            out.sessions.push((t.id.clone(), format!("{id6} {}", t.title.as_deref().unwrap_or("")).trim_end().to_string(), cut));
        }
        out.some_minutes = some.len() as u64;
        edges.sort_unstable();
        let mut n: i32 = 0;
        for (ts, d) in edges {
            n += d;
            match out.steps.last_mut() {
                Some(last) if last.0 == ts => last.1 = n as u32,
                _ => out.steps.push((ts, n as u32)),
            }
        }
        out.steps.dedup_by(|b, a| a.1 == b.1);
        out.peak = out.steps.iter().filter(|s| s.1 > 0).fold(None, |best: Option<(u32, i64)>, &(ts, c)| match best {
            Some((m, _)) if m >= c => best,
            _ => Some((c, ts)),
        });
        for &(a, b) in sittings {
            let (a, b) = (a.max(from), b.min(to));
            if b <= a {
                continue;
            }
            out.present_ms += b - a;
            out.multi_ms += out.time_with(2, a, b);
        }
        out
    }

    /// How many were at work at `ts`.
    pub fn count_at(&self, ts: i64) -> u32 {
        let i = self.steps.partition_point(|s| s.0 <= ts);
        if i == 0 { 0 } else { self.steps[i - 1].1 }
    }

    /// The labels of the sessions at work at `ts`, in the order given.
    pub fn working_at(&self, ts: i64) -> Vec<&str> {
        self.sessions.iter().filter(|(_, _, spans)| spans.iter().any(|&(a, b)| a <= ts && ts < b)).map(|(_, label, _)| label.as_str()).collect()
    }

    /// The time in `[a, b)` when at least `n` were at work.
    fn time_with(&self, n: u32, a: i64, b: i64) -> i64 {
        let mut total = 0;
        for (i, &(ts, c)) in self.steps.iter().enumerate() {
            let end = self.steps.get(i + 1).map_or(i64::MAX, |s| s.0);
            if c >= n {
                total += (end.min(b) - ts.max(a)).max(0);
            }
        }
        total
    }

    /// The peak as the lane labels it: `peak 7 at 14:20`, or `none`.
    pub fn peak_text(&self) -> String {
        self.peak.map_or("none".to_string(), |(n, ts)| format!("peak {n} at {}", fmt::local_time(ts, "%H:%M")))
    }

    /// The `agents` line of the day view.
    pub fn line(&self) -> String {
        if self.session_minutes == 0 {
            return "agents    no agent worked".to_string();
        }
        let mut out = format!(
            "agents    {:.1} agent-hours over {} at the keyboard, some agent at work {}",
            self.session_minutes as f64 / 60.0,
            duration(self.keyboard_ms),
            duration(self.some_minutes as i64 * MINUTE_MS)
        );
        if self.present_ms > 0 {
            out += &format!("; 2 or more worked {:.0}% of the time you were there", 100.0 * self.multi_ms as f64 / self.present_ms as f64);
        }
        if self.peak.is_some() {
            out += &format!("; {}", self.peak_text());
        }
        out
    }
}

fn merge(spans: impl Iterator<Item = (i64, i64)>) -> Vec<(i64, i64)> {
    let mut v: Vec<(i64, i64)> = spans.filter(|(a, b)| b > a).collect();
    v.sort_unstable();
    let mut out: Vec<(i64, i64)> = Vec::new();
    for (a, b) in v {
        match out.last_mut() {
            Some(last) if a <= last.1 => last.1 = last.1.max(b),
            _ => out.push((a, b)),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rhythm::Lane;

    const M: i64 = MINUTE_MS;

    fn timeline(id: &str) -> Timeline {
        Timeline { agent: crate::testing::claude(), id: id.into(), title: Some(format!("title {id}")), left_out: false, events: Vec::new() }
    }

    fn lane(work: &[(i64, i64)]) -> Lane {
        Lane { work: work.to_vec(), ..Lane::default() }
    }

    #[test]
    fn overlapping_sessions_count_two_and_the_peak_is_its_first_moment() {
        let (ta, tb) = (timeline("aaaaaaaa"), timeline("bbbbbbbb"));
        let (la, lb) = (lane(&[(600 * M, 630 * M)]), lane(&[(620 * M, 650 * M), (700 * M, 710 * M)]));
        let a = Agents::of(&[(&ta, &la), (&tb, &lb)], &[(610 * M, 640 * M)], 30 * M, (0, 1440 * M));
        assert_eq!(a.steps, [(600 * M, 1), (620 * M, 2), (630 * M, 1), (650 * M, 0), (700 * M, 1), (710 * M, 0)]);
        assert_eq!(a.peak, Some((2, 620 * M)));
        assert_eq!(a.count_at(625 * M), 2);
        assert_eq!(a.working_at(625 * M), ["aaaaaa title aaaaaaaa", "bbbbbb title bbbbbbbb"]);
        // The peak is a direct count of the sessions at work then.
        assert_eq!(a.working_at(a.peak.unwrap().1).len() as u32, a.peak.unwrap().0);
        // 30 + 40 minutes, 60 of them by some agent; 10 of the 30 at the keyboard had two.
        assert_eq!((a.session_minutes, a.some_minutes), (70, 60));
        assert_eq!((a.present_ms, a.multi_ms), (30 * M, 10 * M));
        assert_eq!(
            a.line(),
            format!(
                "agents    1.2 agent-hours over 30m at the keyboard, some agent at work 1.0h; 2 or more worked 33% of the time you were there; peak 2 at {}",
                fmt::local_time(620 * M, "%H:%M")
            )
        );
    }

    #[test]
    fn a_minute_counts_when_work_touches_it_and_it_starts_in_the_day() {
        let t = timeline("cccccccc");
        // From 09:59:30 to 10:00:30 touches two minutes; the window starts at 10:00.
        let l = lane(&[(599 * M + 30_000, 600 * M + 30_000)]);
        let a = Agents::of(&[(&t, &l)], &[], 0, (600 * M, 1440 * M));
        assert_eq!((a.session_minutes, a.some_minutes), (1, 1));
        assert_eq!(a.steps, [(600 * M, 1), (600 * M + 30_000, 0)]);
    }

    #[test]
    fn a_day_without_work_says_so() {
        let t = timeline("dddddddd");
        let l = lane(&[]);
        let a = Agents::of(&[(&t, &l)], &[(0, M)], M, (0, 1440 * M));
        assert_eq!((a.line().as_str(), a.peak_text().as_str(), a.peak), ("agents    no agent worked", "none", None));
        assert!(a.sessions.is_empty());
    }
}
