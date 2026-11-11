//! Logs that a reader which runs for long keeps after reading them, so that reading one again
//! reads only what was appended since: the session browser reads the same growing logs every few
//! seconds, for costs, X-rays and the day. A log is kept as its decoder, fed every record read so
//! far, and the session it gave; a log that did not change since gives that session again.
//!
//! Reading on gives what reading the whole log gives, since a decoder fed a log in parts gives
//! what it gives fed at once. A log that was written anew is read whole: when it is another
//! file at the same path, when it is shorter than what was read, or when the bytes before where
//! the last read stopped changed. Every kept log is also read whole every few minutes, changed or
//! not, so that a change none of these shows, or one of a file beside the log that its decoder
//! reads, is not kept for longer.

use std::collections::HashMap;
use std::fs::Metadata;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant, SystemTime};

use anyhow::Result;

use crate::model::Session;
use crate::source::{Decoder, Locator, LogSource, ReadOn, SessionRef};

/// Logs last written longer ago than this are read whole and not kept: they hardly change, and
/// they are read for reports over long periods, which would keep every log.
const RECENT: Duration = Duration::from_secs(3 * 24 * 3600);
/// A kept log that nothing read for this long is dropped.
const UNUSED: Duration = Duration::from_secs(30 * 60);
/// A kept log is read whole again after this long, changed or not.
const WHOLE_AFTER: Duration = Duration::from_secs(10 * 60);
/// The most bytes of logs kept at once; a kept decoder takes about a quarter of its log's size,
/// and much less of a huge one's (3 % of a 2.1 GB log). A log counts at most a quarter of the
/// budget, so that one huge log is kept beside the others and never pushes them out.
const BUDGET: u64 = 1 << 30;

/// The kept logs, by source, log and whether the log is a subagent's, which the decoder depends
/// on. Each log has a lock of its own, so that logs are read in parallel and a log that two
/// readers ask for at once is read once.
pub(crate) struct Kept {
    logs: Mutex<HashMap<Key, Arc<Mutex<Option<Log>>>>>,
    budget: u64,
}

impl Default for Kept {
    fn default() -> Kept {
        Kept::within(BUDGET)
    }
}

type Key = (&'static str, Locator, bool);

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
    /// Keeps at most `budget` bytes of logs, each counted up to a quarter of it.
    pub(crate) fn within(budget: u64) -> Kept {
        Kept { logs: Mutex::default(), budget }
    }

    /// What a log counts against the budget.
    fn weight(&self, log: &Log) -> u64 {
        log.on.end.min(self.budget / 4)
    }

    /// The session a log gives now: read on from where its last read stopped when it is kept,
    /// else read whole and kept when it is recent and its source can read it on.
    pub(crate) fn parse(&self, source: &dyn LogSource, s: &SessionRef) -> Result<Session> {
        // A session in a store that holds many is rewritten in place, so it cannot be read on,
        // and the store's file says nothing about one session: it is read whole, without the
        // read `read_new` would make only to find that out.
        if matches!(s.locator, Locator::Row { .. }) {
            return read_whole(source, s);
        }
        let key: Key = (source.name(), s.locator.clone(), s.subagent);
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
        // Also a log that did not change is read whole now and then: its decoder may read files
        // beside it, such as a subagent's meta file written later.
        let read = match kept.as_mut() {
            Some(log) if now.duration_since(log.whole_at) >= WHOLE_AFTER => Ok(None),
            Some(log) if log.stamp == stamp => {
                log.used = now;
                Ok(Some(log.on.open))
            }
            Some(log) if log.stamp.file == stamp.file => read_on(source, s, log, stamp, now),
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
        self.trim(&key);
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
    /// rest fit the budget. A log being read stays, and so does `just`, the log just read.
    fn trim(&self, just: &Key) {
        let mut logs = lock(&self.logs);
        let now = Instant::now();
        let mut held: Vec<(Instant, u64, Key)> = Vec::new();
        logs.retain(|key, slot| {
            let Ok(log) = slot.try_lock() else { return true };
            match log.as_ref() {
                Some(log) if now.duration_since(log.used) < UNUSED => {
                    if key != just {
                        held.push((log.used, self.weight(log), key.clone()));
                    }
                    true
                }
                _ => false,
            }
        });
        // The log just read counts too, but is not dropped.
        let just_weight = logs.get(just).and_then(|slot| slot.try_lock().ok()?.as_ref().map(|log| self.weight(log))).unwrap_or(0);
        let mut total: u64 = just_weight + held.iter().map(|h| h.1).sum::<u64>();
        held.sort_by_key(|h| h.0);
        for (_, bytes, key) in held {
            if total <= self.budget {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source::{Activity, AgentId, Claim, Detail, Head, Locator, Records, Touched};
    use std::path::{Path, PathBuf};

    /// A source of plain line logs whose session is named by the count of its lines.
    struct Lines;

    struct File(PathBuf);

    impl Records for File {
        fn for_each(&mut self, f: &mut dyn FnMut(&str)) -> Result<()> {
            std::fs::read_to_string(&self.0)?.split_inclusive('\n').for_each(f);
            Ok(())
        }

        fn for_each_from(&mut self, from: u64, check: &[u8], f: &mut dyn FnMut(&str)) -> Result<Option<ReadOn>> {
            let bytes = std::fs::read(&self.0)?;
            let from = from as usize;
            if bytes.len() < from || &bytes[from - check.len()..from] != check {
                return Ok(None);
            }
            let mut end = from;
            for line in std::str::from_utf8(&bytes[from..])?.split_inclusive('\n').filter(|l| l.ends_with('\n')) {
                f(line);
                end += line.len();
            }
            Ok(Some(ReadOn { end: end as u64, check: bytes[end.saturating_sub(8)..end].to_vec(), open: false }))
        }
    }

    /// Counts the lines; the session's title is what the file `<log>.meta` beside it says when
    /// the decoder is made, as a decoder that reads a meta file does.
    #[derive(Clone)]
    struct Count(usize, Option<String>);

    impl Decoder for Count {
        fn feed(&mut self, _: &str) {
            self.0 += 1;
        }
        fn snapshot(&self) -> Session {
            let mut s = Session::new(AgentId::unknown("lines"), self.0.to_string());
            s.title = self.1.clone();
            s
        }
        fn finish(self: Box<Self>) -> Session {
            self.snapshot()
        }
    }

    impl LogSource for Lines {
        fn name(&self) -> &'static str {
            "lines"
        }
        fn agents(&self) -> Vec<AgentId> {
            vec![AgentId::unknown("lines")]
        }
        fn detail(&self) -> Detail {
            Detail::Turns
        }
        fn discover(&self) -> Vec<SessionRef> {
            Vec::new()
        }
        fn session_at(&self, _: &Path) -> Option<SessionRef> {
            None
        }
        fn claim(&self, _: &Path) -> Claim {
            Claim::No
        }
        fn watched_paths(&self) -> Vec<PathBuf> {
            Vec::new()
        }
        fn touched(&self, _: &Path, _: bool) -> Touched {
            Touched::Other
        }
        fn head(&self, _: &SessionRef) -> Head {
            Head::default()
        }
        fn subagents(&self, _: &SessionRef) -> Vec<SessionRef> {
            Vec::new()
        }
        fn open(&self, s: &SessionRef) -> Result<Box<dyn Records>> {
            Ok(Box::new(File(s.path().to_path_buf())))
        }
        fn decoder(&self, s: &SessionRef) -> Box<dyn Decoder> {
            Box::new(Count(0, std::fs::read_to_string(s.path().with_extension("meta")).ok()))
        }
        fn activity(&self, _: &SessionRef) -> Result<Activity> {
            Ok(Activity::default())
        }
    }

    fn log(path: &Path) -> SessionRef {
        SessionRef {
            source: "lines",
            agent: AgentId::unknown("lines"),
            locator: Locator::File(path.to_path_buf()),
            session_id: "s".into(),
            main_id: "s".into(),
            subagent: false,
            size: 0,
            modified_ns: 0,
            created_ms: None,
        }
    }

    fn kept_names(k: &Kept) -> Vec<String> {
        let mut names: Vec<String> = lock(&k.logs).keys().map(|key| key.1.path().file_name().unwrap().to_string_lossy().into_owned()).collect();
        names.sort();
        names
    }

    #[test]
    fn a_log_over_the_budget_is_kept_beside_the_others_and_the_oldest_go_first() {
        let dir = std::env::temp_dir().join(format!("tokprof-kept-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let write = |name: &str, lines: usize| {
            let path = dir.join(name);
            let text: String = (0..lines).map(|_| "x".repeat(99) + "\n").collect();
            std::fs::write(&path, text).unwrap();
            path
        };
        let k = Kept::within(4_000);
        let parse = |path: &Path| k.parse(&Lines, &log(path)).unwrap().id;
        let (a, b, big) = (write("a", 5), write("b", 5), write("big", 1_000));
        assert_eq!((parse(&a), parse(&b), parse(&big)), ("5".into(), "5".into(), "1000".into()));
        assert_eq!(kept_names(&k), ["a", "b", "big"], "a log of 100 kB counts 1 kB of the 4 kB");
        std::fs::write(&big, "x".repeat(99) + "\n" + &"x".repeat(99_999) + "\n").unwrap();
        let mut more = std::fs::OpenOptions::new().append(true).open(&big).unwrap();
        std::io::Write::write_all(&mut more, b"y\n").unwrap();
        assert_eq!(parse(&big), "3", "written anew, then read whole");
        assert_eq!(kept_names(&k), ["a", "b", "big"], "and the others stay");

        // Four more large logs: the ones read longest ago go, the one just read stays.
        for name in ["c", "d", "e", "f"] {
            parse(&write(name, 1_000));
        }
        assert_eq!(kept_names(&k), ["c", "d", "e", "f"]);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn an_unchanged_log_is_read_whole_again_after_a_while() {
        let dir = std::env::temp_dir().join(format!("tokprof-kept-idle-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("a.log");
        std::fs::write(&path, "x\n").unwrap();
        let k = Kept::default();
        let title = || k.parse(&Lines, &log(&path)).unwrap().title;
        assert_eq!(title(), None);
        std::fs::write(dir.join("a.meta"), "named").unwrap();
        assert_eq!(title(), None, "unchanged and read whole lately: the kept session");
        let slot = lock(&k.logs).values().next().unwrap().clone();
        let mut held = lock(&slot);
        let log = held.as_mut().unwrap();
        // A machine started less than 10 minutes ago has no earlier instant.
        let Some(earlier) = log.whole_at.checked_sub(WHOLE_AFTER) else { return };
        log.whole_at = earlier;
        drop(held);
        assert_eq!(title().as_deref(), Some("named"), "read whole again: the meta file written since shows");
        std::fs::remove_dir_all(&dir).ok();
    }
}
