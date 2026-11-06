//! Logs that a reader which runs for long keeps after reading them, so that reading one again
//! reads only what was appended since: the session browser reads the same growing logs every few
//! seconds, for costs, X-rays and the day. A log is kept as its decoder, fed every record read so
//! far, and the session it gave; a log that did not change since gives that session again.
//!
//! Reading on gives what reading the whole log gives, since a decoder fed a log in parts gives
//! what it gives fed at once. A log that was written anew is read whole: when it is another
//! file at the same path, when it is shorter than what was read, or when the bytes before where
//! the last read stopped changed. Every kept log is also read whole every few minutes, so that a
//! change none of these shows is not kept for longer.

use std::collections::HashMap;
use std::fs::Metadata;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant, SystemTime};

use anyhow::Result;

use crate::model::Session;
use crate::source::{Decoder, LogSource, ReadOn, SessionRef};

/// Logs last written longer ago than this are read whole and not kept: they hardly change, and
/// they are read for reports over long periods, which would keep every log.
const RECENT: Duration = Duration::from_secs(3 * 24 * 3600);
/// A kept log that nothing read for this long is dropped.
const UNUSED: Duration = Duration::from_secs(30 * 60);
/// A kept log is read whole again after this long.
const WHOLE_AFTER: Duration = Duration::from_secs(10 * 60);
/// The most bytes of logs kept at once; a kept decoder takes about a quarter of its log's size.
const BUDGET: u64 = 1 << 30;

/// The kept logs, by source, path and whether the log is a subagent's, which the decoder depends
/// on. Each log has a lock of its own, so that logs are read in parallel and a log that two
/// readers ask for at once is read once.
#[derive(Default)]
pub(crate) struct Kept {
    logs: Mutex<HashMap<Key, Arc<Mutex<Option<Log>>>>>,
}

type Key = (&'static str, PathBuf, bool);

struct Log {
    decoder: Box<dyn Decoder>,
    /// The file as it was before the records fed were read.
    stamp: Stamp,
    /// Where the read stopped.
    on: ReadOn,
    /// The session the records fed give.
    session: Session,
    /// When the log was last read whole, and last asked for.
    whole_at: Instant,
    used: Instant,
}

/// What tells that a file changed: its length, its last write, and which file it is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Stamp {
    len: u64,
    modified: Option<SystemTime>,
    file: (u64, u64),
}

impl Stamp {
    fn of(meta: &Metadata) -> Stamp {
        #[cfg(unix)]
        let file = {
            use std::os::unix::fs::MetadataExt;
            (meta.dev(), meta.ino())
        };
        #[cfg(not(unix))]
        let file = (0, 0);
        Stamp { len: meta.len(), modified: meta.modified().ok(), file }
    }

    fn recent(&self) -> bool {
        self.modified.and_then(|m| SystemTime::now().duration_since(m).ok()).is_none_or(|age| age < RECENT)
    }
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// Reads a log whole, as a reader that keeps nothing does.
pub(crate) fn read_whole(source: &dyn LogSource, s: &SessionRef) -> Result<Session> {
    let mut decoder = source.decoder(s);
    source.open(s)?.for_each(&mut |record| decoder.feed(record))?;
    Ok(decoder.finish())
}

impl Kept {
    /// The session a log gives now: read on from where its last read stopped when it is kept,
    /// else read whole and kept when it is recent and its source can read it on.
    pub(crate) fn parse(&self, source: &dyn LogSource, s: &SessionRef) -> Result<Session> {
        let key: Key = (source.name(), s.path().to_path_buf(), s.subagent);
        let stamp = match std::fs::metadata(s.path()) {
            Ok(meta) if Stamp::of(&meta).recent() => Stamp::of(&meta),
            _ => {
                self.forget(&key);
                return read_whole(source, s);
            }
        };
        let slot = lock(&self.logs).entry(key.clone()).or_default().clone();
        let mut kept = lock(&slot);
        let now = Instant::now();
        let read = match kept.as_mut() {
            Some(log) if log.stamp == stamp => {
                log.used = now;
                Ok(Some(log.on.open))
            }
            Some(log) if log.stamp.file == stamp.file && now.duration_since(log.whole_at) < WHOLE_AFTER => read_on(source, s, log, stamp, now),
            _ => Ok(None),
        };
        let open = match read {
            Ok(Some(open)) => open,
            Ok(None) => match read_new(source, s, stamp, now) {
                Ok(Some(log)) => kept.insert(log).on.open,
                Ok(None) => {
                    *kept = None;
                    drop(kept);
                    self.forget(&key);
                    return read_whole(source, s);
                }
                Err(e) => {
                    *kept = None;
                    return Err(e);
                }
            },
            Err(e) => {
                *kept = None;
                return Err(e);
            }
        };
        let session = kept.as_ref().map(|log| log.session.clone());
        drop(kept);
        self.trim();
        match session {
            // A last record without its end, which a whole read hands over: read whole this time.
            Some(_) if open => read_whole(source, s),
            Some(session) => Ok(session),
            None => read_whole(source, s),
        }
    }

    fn forget(&self, key: &Key) {
        lock(&self.logs).remove(key);
    }

    /// Drops the logs nothing asked for lately, then the ones asked for longest ago until the
    /// rest fit the budget. A log being read stays.
    fn trim(&self) {
        let mut logs = lock(&self.logs);
        let now = Instant::now();
        let mut held: Vec<(Instant, u64, Key)> = Vec::new();
        logs.retain(|key, slot| {
            let Ok(log) = slot.try_lock() else { return true };
            match log.as_ref() {
                Some(log) if now.duration_since(log.used) < UNUSED => {
                    held.push((log.used, log.on.end, key.clone()));
                    true
                }
                _ => false,
            }
        });
        let mut total: u64 = held.iter().map(|h| h.1).sum();
        held.sort_by_key(|h| h.0);
        for (_, bytes, key) in held {
            if total <= BUDGET {
                break;
            }
            logs.remove(&key);
            total -= bytes;
        }
    }
}

/// Feeds a kept log what was appended to it; `None` when it has to be read whole.
fn read_on(source: &dyn LogSource, s: &SessionRef, log: &mut Log, stamp: Stamp, now: Instant) -> Result<Option<bool>> {
    if stamp.len < log.on.end {
        return Ok(None);
    }
    let decoder = &mut log.decoder;
    let Some(on) = source.open(s)?.for_each_from(log.on.end, &log.on.check, &mut |record| decoder.feed(record))? else { return Ok(None) };
    // A snapshot also when no record was added: a decoder may read more than the log, such as a
    // file beside it that was written since.
    log.session = log.decoder.snapshot();
    log.on = on;
    log.stamp = stamp;
    log.used = now;
    Ok(Some(log.on.open))
}

/// Reads a log whole to keep it; `None` when its source cannot read it on.
fn read_new(source: &dyn LogSource, s: &SessionRef, stamp: Stamp, now: Instant) -> Result<Option<Log>> {
    let mut decoder = source.decoder(s);
    let Some(on) = source.open(s)?.for_each_from(0, &[], &mut |record| decoder.feed(record))? else { return Ok(None) };
    let session = decoder.snapshot();
    Ok(Some(Log { decoder, stamp, on, session, whole_at: now, used: now }))
}
