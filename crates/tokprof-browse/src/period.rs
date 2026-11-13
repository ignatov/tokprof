//! A calendar month or quarter as the window shows it (Month and Quarter): its numbers from the
//! day store, summed on a worker the moment a period is asked for and kept for each period and
//! filter text, as Week sums a week. What only the period's logs give is read by the reports
//! worker (see `reports`), keyed the same way.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::{Duration, Instant};

use tokprof_core::calendar::Filters;
use tokprof_core::daystore::Store;
use tokprof_core::period::{Period, PeriodSums};
use tokprof_core::query::Query;
use tokprof_core::rhythm::{self, Offset};

use crate::browser::{Wake, now_ms};

/// A period and the window's filter text: what the store's sums and the log read are kept by.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Key {
    pub period: Period,
    /// The filter text, trimmed; `None` when there is none.
    pub text: Option<String>,
}

impl Key {
    /// The period with the filter `text`; blank text is no filter.
    pub fn new(period: Period, text: &str) -> Key {
        let t = text.trim();
        Key { period, text: (!t.is_empty()).then(|| t.to_string()) }
    }
}

/// The store's filters of a period: the command line's, with the filter text.
pub fn filters(select: &Query, key: &Key) -> Filters {
    Filters { text: key.text.clone(), ..crate::calendar::filters(select) }
}

/// The period of `key` in the store, as of `now_ms`, in the machine's time zone. The store has no
/// client, so `--client` does not cut it.
pub fn sums_of(store: &Store, select: &Query, key: &Key, now_ms: i64) -> PeriodSums {
    PeriodSums::of(&store.entries, &filters(select, key), key.period, now_ms, &rhythm::local_offset)
}

/// What the log read of a period counts: the period's window, cut by `--since` and `--until`.
/// The end of the current period is its end, which is still to come: a read counts to the moment
/// it starts.
pub fn query(select: &Query, period: Period, offset: Offset) -> Query {
    let (from, end) = period.bounds(offset);
    let since = select.since_ms.map_or(from, |s| s.max(from));
    let until = select.until_ms.map_or(end, |u| u.min(end)).max(since);
    Query { since_ms: Some(since), until_ms: Some(until), ..select.clone() }
}

/// The store's sums of a period, as the worker computed them from one store.
pub struct Summed {
    pub sums: Arc<PeriodSums>,
    /// The store they come from, by its address.
    store: usize,
    /// The local date they were summed on: the current period is summed again on a new day.
    date: i64,
}

/// Sums asked for: the period and filter, the store, and its address.
type SumJob = (Key, Arc<Store>, usize);

struct SumDone {
    key: Key,
    store: usize,
    date: i64,
    sums: PeriodSums,
}

/// The periods' store sums and the worker that computes them, the newest asked for first.
pub struct Periods {
    jobs: Option<Sender<SumJob>>,
    done: Receiver<SumDone>,
    summed: HashMap<Key, Summed>,
    pending: Option<(Key, usize)>,
}

impl Periods {
    /// Starts the worker over the store's entries that `select` selects. It computes nothing until
    /// a period is asked for.
    pub fn start(select: Query, wake: Wake) -> Periods {
        let (jobs, rx) = mpsc::channel::<SumJob>();
        let (tx, done) = mpsc::channel();
        std::thread::spawn(move || {
            while let Ok(mut job) = rx.recv() {
                while let Ok(newer) = rx.try_recv() {
                    job = newer;
                }
                let (key, store, at) = job;
                let now = now_ms();
                let sums = crate::contain::run(|| sums_of(&store, &select, &key, now));
                // A sum that panicked leaves the period without its numbers; the next store tries
                // again.
                let Ok(sums) = sums else { continue };
                if tx.send(SumDone { key, store: at, date: rhythm::date_of(now, &rhythm::local_offset), sums }).is_err() {
                    return;
                }
                wake();
            }
        });
        Periods { jobs: Some(jobs), done, summed: HashMap::new(), pending: None }
    }

    /// Periods that compute nothing, for tests: they hold what `put` gives them.
    pub fn detached() -> Periods {
        let (_, done) = mpsc::channel();
        Periods { jobs: None, done, summed: HashMap::new(), pending: None }
    }

    /// Keeps `sums` as the store's sums of `key`.
    pub fn put(&mut self, key: Key, sums: PeriodSums) {
        let date = rhythm::date_of(now_ms(), &rhythm::local_offset);
        self.summed.insert(key, Summed { sums: Arc::new(sums), store: 0, date });
    }

    /// Takes in what the worker sent, and asks for the sums of `key` when `store` is newer than
    /// the one they came from, or when the current period was summed on another day. A key asked
    /// for replaces the one asked for before it. Returns whether anything arrived.
    pub fn tick(&mut self, key: &Key, store: Option<&Arc<Store>>) -> bool {
        let arrived = self.absorb();
        if let (Some(jobs), Some(store)) = (&self.jobs, store) {
            let at = Arc::as_ptr(store) as usize;
            let today = rhythm::date_of(now_ms(), &rhythm::local_offset);
            let fresh = self.summed.get(key).is_some_and(|s| s.store == at && (s.sums.complete || s.date == today));
            if !fresh && !self.pending.as_ref().is_some_and(|(k, s)| k == key && *s == at) {
                self.pending = Some((key.clone(), at));
                if jobs.send((key.clone(), store.clone(), at)).is_err() {
                    self.pending = None;
                }
            }
        }
        arrived
    }

    fn absorb(&mut self) -> bool {
        let mut arrived = false;
        while let Ok(d) = self.done.try_recv() {
            arrived = true;
            if self.pending.as_ref().is_some_and(|(k, s)| *k == d.key && *s == d.store) {
                self.pending = None;
            }
            self.summed.insert(d.key, Summed { sums: Arc::new(d.sums), store: d.store, date: d.date });
        }
        arrived
    }

    /// The store's sums of a period, once computed; from an older store while a newer one is
    /// summed.
    pub fn get(&self, key: &Key) -> Option<&Arc<PeriodSums>> {
        self.summed.get(key).map(|s| &s.sums)
    }

    /// Whether the period's sums are being computed.
    pub fn summing(&self, key: &Key) -> bool {
        self.pending.as_ref().is_some_and(|(k, _)| k == key)
    }

    /// Asks for the period and waits up to `limit` for its sums, for a frame without a window.
    pub fn settle(&mut self, key: &Key, store: Option<&Arc<Store>>, limit: Duration) {
        let deadline = Instant::now() + limit;
        self.tick(key, store);
        while Instant::now() < deadline && store.is_some() && (self.get(key).is_none() || self.summing(key)) {
            std::thread::sleep(Duration::from_millis(20));
            self.absorb();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokprof_core::daystore::{Entry, Minute};
    use tokprof_core::month::Ym;
    use tokprof_core::period::Yq;

    const OCT: Period = Period::Month(Ym { year: 2026, month: 10 });

    fn store() -> Arc<Store> {
        let at = rhythm::midnight(Ym { year: 2026, month: 10 }.first_date() + 2, &rhythm::local_offset) + 36_000_000;
        let e = |project: &str, usd: f64| Entry {
            key: format!("/{project}"),
            agent: "claude".into(),
            session: project.into(),
            main: project.into(),
            project: project.into(),
            minutes: vec![Minute { minute: at / 60_000, usd, requests: 1, ..Minute::default() }],
            ..Entry::default()
        };
        Arc::new(Store { entries: vec![e("~/src/zebrafish", 7.0), e("~/src/heron", 5.0)], ..Store::default() })
    }

    #[test]
    fn a_period_s_query_is_its_window_cut_by_the_command_line() {
        let offset: Offset = &rhythm::local_offset;
        let (from, end) = OCT.bounds(offset);
        let q = query(&Query { project: Some("app".into()), ..Query::default() }, OCT, offset);
        assert_eq!((q.since_ms, q.until_ms, q.project.as_deref()), (Some(from), Some(end), Some("app")));
        let q = query(&Query { since_ms: Some(from + 8 * 86_400_000), until_ms: Some(end + 86_400_000), ..Query::default() }, OCT, offset);
        assert_eq!((q.since_ms, q.until_ms), (Some(from + 8 * 86_400_000), Some(end)), "a later --since and the period's end");
        let q = query(&Query { until_ms: Some(from - 1), ..Query::default() }, OCT, offset);
        assert_eq!(q.since_ms, q.until_ms, "an --until before the period leaves nothing");
    }

    #[test]
    fn sums_are_kept_by_period_and_text_and_summed_again_for_a_newer_store() {
        let mut p = Periods::start(Query::default(), Arc::new(|| {}));
        let (all, zebra) = (Key::new(OCT, " "), Key::new(OCT, "ZebraF"));
        assert_eq!(all.text, None, "blank text is no filter");
        let s = store();
        p.settle(&all, Some(&s), Duration::from_secs(20));
        p.settle(&zebra, Some(&s), Duration::from_secs(20));
        assert_eq!((p.get(&all).unwrap().key.usd, p.get(&zebra).unwrap().key.usd), (12.0, 7.0));
        // The same store: nothing is asked for again.
        p.tick(&all, Some(&s));
        assert!(!p.summing(&all));
        // A newer store: summed again, the old sums shown meanwhile.
        let newer = Arc::new((*s).clone());
        p.tick(&all, Some(&newer));
        assert!(p.summing(&all) && p.get(&all).is_some());
        p.settle(&all, Some(&newer), Duration::from_secs(20));
        assert!(!p.summing(&all));
        // Without a store there is nothing to sum.
        let q4 = Key::new(Period::Quarter(Yq { year: 2026, q: 4 }), "");
        p.tick(&q4, None);
        assert!(!p.summing(&q4) && p.get(&q4).is_none());
    }

    #[test]
    fn detached_periods_compute_nothing() {
        let mut p = Periods::detached();
        p.tick(&Key::new(OCT, ""), Some(&store()));
        assert!(p.get(&Key::new(OCT, "")).is_none() && !p.summing(&Key::new(OCT, "")));
    }
}
