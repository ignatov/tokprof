//! Reading many logs at once, in parallel, into whatever a command collects.

use anyhow::{Result, bail};
use rayon::prelude::*;

use crate::catalog::Catalog;
use crate::model::Session;
use crate::query::Query;
use crate::source::SessionRef;

/// What to do with a log that cannot be opened, read or decompressed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OnUnreadable {
    /// Leave it out and report it in `ScanStats`.
    Skip,
    /// Fail the scan with its error.
    Fail,
}

/// What a scan read.
#[derive(Clone, Debug, Default)]
pub struct ScanStats {
    /// The logs that could not be read, as `<path>: <error>`, in no particular order.
    pub unreadable: Vec<String>,
}

/// Parses each log in parallel, keeps the sessions the query accepts, and folds them into a `T`
/// per thread, which `merge` then joins. Logs are taken largest first, as `Catalog::discover`
/// gives them, so that the parse does not end on one huge file.
pub fn scan<T: Send>(
    catalog: &Catalog,
    files: &[SessionRef],
    query: &Query,
    on_unreadable: OnUnreadable,
    init: impl Fn() -> T + Sync + Send,
    fold: impl Fn(&mut T, &Session, &SessionRef) + Sync + Send,
    merge: impl Fn(T, T) -> T + Sync + Send,
) -> Result<(T, ScanStats)> {
    let (value, unreadable) = files
        .par_iter()
        .map(|f| {
            let mut t = init();
            let mut unreadable = Vec::new();
            match catalog.parse(f) {
                Ok(s) if query.accepts(&s, f) => fold(&mut t, &s, f),
                Ok(_) => {}
                Err(e) => unreadable.push(format!("{}: {e:#}", f.path().display())),
            }
            (t, unreadable)
        })
        .reduce(
            || (init(), Vec::new()),
            |(a, mut ua), (b, ub)| {
                ua.extend(ub);
                (merge(a, b), ua)
            },
        );
    if on_unreadable == OnUnreadable::Fail
        && let Some(first) = unreadable.first()
    {
        bail!("{first}");
    }
    Ok((value, ScanStats { unreadable }))
}
