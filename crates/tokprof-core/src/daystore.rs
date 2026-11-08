//! The day store: what each log holds by hour of UTC, kept on disk so that the calendar does
//! not read every log again, and so that the days of logs the agents deleted stay.
//!
//! An entry per log keeps, per minute with a request, the spend, the avoidable premium, the
//! requests and the commits, pushes and pull requests; for a main log also the minutes its agent
//! worked, as a set of minutes per hour, and the exact times of the user's actions. It keeps no
//! text but the project path. The caller gives the file's path; the store reads no environment.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::analysis::{self, Fact, Filter, MissEvent, RequestRecord, Sink};
use crate::catalog::Catalog;
use crate::model::{Outcome, Session};
use crate::pricing::Pricing;
use crate::query::Query;
use crate::rhythm::{Lane, Timeline};
use crate::scan::{OnUnreadable, scan_counted};
use crate::source::{AgentId, Locator, SessionRef};
use crate::tools::ToolShare;

/// The store's format, in its file name and its header.
pub const FORMAT: u32 = 1;
/// The store's file name in the cache folder.
pub const FILE_NAME: &str = "days-1.json";

const MINUTE_MS: i64 = 60_000;

/// What the requests of one minute of a log hold.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(from = "MinuteRow", into = "MinuteRow")]
pub struct Minute {
    /// Minutes since 1970-01-01 00:00 UTC.
    pub minute: i64,
    pub usd: f64,
    pub avoidable_usd: f64,
    pub requests: u32,
    pub commits: u32,
    pub pushes: u32,
    pub pull_requests: u32,
}

/// A minute as the file writes it: an array, so that the store stays small.
type MinuteRow = (i64, f64, f64, u32, u32, u32, u32);

impl From<MinuteRow> for Minute {
    fn from((minute, usd, avoidable_usd, requests, commits, pushes, pull_requests): MinuteRow) -> Minute {
        Minute { minute, usd, avoidable_usd, requests, commits, pushes, pull_requests }
    }
}

impl From<Minute> for MinuteRow {
    fn from(m: Minute) -> MinuteRow {
        (m.minute, m.usd, m.avoidable_usd, m.requests, m.commits, m.pushes, m.pull_requests)
    }
}

/// What the store keeps of one log.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Entry {
    /// The log's path, or `<database>#<id>` for a log in a database.
    pub key: String,
    pub len: u64,
    pub mtime_ns: i64,
    /// The fingerprint it was made with (see [`fingerprint`]).
    pub fingerprint: String,
    pub agent: String,
    pub session: String,
    /// The id of the main session the log belongs to.
    pub main: String,
    pub subagent: bool,
    pub project: String,
    /// The rhythm leaves the session out: a program ran it, or it ran in a temporary folder.
    pub left_out: bool,
    /// The minutes with a request or a commit, in time order.
    pub minutes: Vec<Minute>,
    /// Per hour since 1970-01-01 00:00 UTC, the minutes the agent worked in it as bits, minute 0
    /// lowest; only for a main log.
    pub work: Vec<(i64, u64)>,
    /// The user's actions in time order, in epoch ms, and whether each is a message; only for a
    /// main log.
    pub actions: Vec<(i64, bool)>,
    /// The log was not found in this run; set by the refresh, not kept in the file.
    #[serde(skip)]
    pub gone: bool,
}

/// The fingerprint of the numbers an entry holds: the tokprof version and the prices in use.
pub fn fingerprint(pricing: &Pricing) -> String {
    let digest = Sha256::digest(format!("{:?}", pricing.entries()).as_bytes());
    let hex: String = digest.iter().take(8).map(|b| format!("{b:02x}")).collect();
    format!("tokprof {}; prices {hex}", env!("CARGO_PKG_VERSION"))
}

/// The key of a log in the store.
pub fn key_of(f: &SessionRef) -> String {
    match &f.locator {
        Locator::File(p) | Locator::Dir(p) => p.to_string_lossy().to_string(),
        Locator::Row { db, id } => format!("{}#{id}", db.to_string_lossy()),
    }
}

/// The entry of one parsed log. `temp` lists the temporary folders whose sessions the rhythm
/// leaves out.
pub fn entry_of(s: &Session, f: &SessionRef, pricing: &Pricing, temp: &[String], fingerprint: &str) -> Entry {
    let mut sink = Minutes::default();
    analysis::analyze(s, pricing, &Filter::default(), &mut sink);
    let mut entry = Entry {
        key: key_of(f),
        len: f.size,
        mtime_ns: f.modified_ns as i64,
        fingerprint: fingerprint.to_string(),
        agent: f.agent.name().to_string(),
        session: f.session_id.clone(),
        main: f.main_id.clone(),
        subagent: f.subagent,
        project: s.project.clone(),
        minutes: sink.rows.into_values().collect(),
        ..Entry::default()
    };
    if !f.subagent {
        let t = Timeline::of(s, temp);
        let lane = Lane::of(&t);
        entry.left_out = t.left_out;
        entry.work = work_bits(&lane.work);
        entry.actions = lane.actions.iter().map(|&(ts, e)| (ts, e.is_message())).collect();
    }
    entry
}

/// The minutes touched by work: by hour, the bits of the minutes that some interval overlaps by
/// more than nothing.
pub fn work_bits(work: &[(i64, i64)]) -> Vec<(i64, u64)> {
    let mut hours: BTreeMap<i64, u64> = BTreeMap::new();
    for &(a, b) in work.iter().filter(|(a, b)| b > a) {
        for m in a.div_euclid(MINUTE_MS)..=(b - 1).div_euclid(MINUTE_MS) {
            *hours.entry(m.div_euclid(60)).or_default() |= 1 << m.rem_euclid(60);
        }
    }
    hours.into_iter().collect()
}

/// The sink that sums a log's requests by minute.
#[derive(Default)]
struct Minutes {
    rows: BTreeMap<i64, Minute>,
    /// The minute of the request being analyzed, for the calls and the miss that follow it.
    current: Option<i64>,
}

impl Minutes {
    fn row(&mut self, minute: i64) -> &mut Minute {
        self.rows.entry(minute).or_insert(Minute { minute, ..Minute::default() })
    }
}

impl Sink for Minutes {
    fn session(&mut self, _: &Session) {}

    fn request(&mut self, r: &RequestRecord) {
        let minute = r.ts_ms.div_euclid(MINUTE_MS);
        let usd = r.price.map_or(0.0, |p| p.cost(&r.usage));
        let row = self.row(minute);
        row.requests += 1;
        row.usd += usd;
        self.current = Some(minute);
    }

    fn miss(&mut self, m: MissEvent) {
        if m.cause.expected() {
            return;
        }
        if let Some(p) = m.premium_usd {
            self.row(m.ts_ms.div_euclid(MINUTE_MS)).avoidable_usd += p;
        }
    }

    fn fact(&mut self, _: &Fact) {}

    /// A call that worked counts what its command did to version control, as the share counts
    /// outcomes, at the minute of the request whose response made it.
    fn tool_share(&mut self, t: &ToolShare) {
        if t.call.is_none() || t.outcome != Outcome::Worked || !t.vcs.any() {
            return;
        }
        let Some(minute) = self.current else { return };
        let row = self.row(minute);
        row.commits += u32::from(t.vcs.commit);
        row.pushes += u32::from(t.vcs.push);
        row.pull_requests += u32::from(t.vcs.pull_request);
    }
}

/// The store's file.
#[derive(Serialize, Deserialize)]
struct File {
    format: u32,
    logs: Vec<Entry>,
}

/// The day store, as loaded from its file.
#[derive(Clone, Debug, Default)]
pub struct Store {
    pub path: PathBuf,
    pub entries: Vec<Entry>,
    /// The file could not be parsed and was moved aside to this path.
    pub broken: Option<PathBuf>,
}

impl Store {
    /// The store at `path`; empty when the file does not exist. A file that cannot be parsed is
    /// renamed to `<name>.broken` and the store starts empty.
    pub fn load(path: &Path) -> Store {
        let mut store = Store { path: path.to_path_buf(), ..Store::default() };
        let Ok(bytes) = std::fs::read(path) else { return store };
        match serde_json::from_slice::<File>(&bytes) {
            Ok(f) if f.format == FORMAT => store.entries = f.logs,
            _ => {
                let aside = path.with_file_name(format!("{}.broken", path.file_name().map_or_else(|| FILE_NAME.into(), |n| n.to_string_lossy())));
                if std::fs::rename(path, &aside).is_ok() {
                    store.broken = Some(aside);
                }
            }
        }
        store
    }

    /// Writes the store to a new file next to its path and renames it over the old one, so that
    /// a reader sees one whole store or the other.
    pub fn save(&self) -> Result<()> {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let dir = self.path.parent().filter(|d| !d.as_os_str().is_empty()).unwrap_or(Path::new("."));
        std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
        let name = self.path.file_name().map_or_else(|| FILE_NAME.into(), |n| n.to_string_lossy());
        let tmp = dir.join(format!("{name}.{}.{}.tmp", std::process::id(), COUNTER.fetch_add(1, Ordering::Relaxed)));
        let file = File { format: FORMAT, logs: self.entries.clone() };
        let bytes = serde_json::to_vec(&file)?;
        std::fs::write(&tmp, bytes).with_context(|| format!("writing {}", tmp.display()))?;
        if let Err(e) = std::fs::rename(&tmp, &self.path) {
            let _ = std::fs::remove_file(&tmp);
            return Err(e).with_context(|| format!("writing {}", self.path.display()));
        }
        Ok(())
    }
}

/// What a refresh did.
#[derive(Clone, Debug, Default)]
pub struct Refreshed {
    /// The logs read again.
    pub read: Vec<SessionRef>,
    /// The logs whose entries were used as they were.
    pub from_store: usize,
    /// The entries whose logs were not found.
    pub gone: usize,
    /// The logs that could not be read, as `<path>: <error>`.
    pub unreadable: Vec<String>,
    /// Whether any entry changed, so that the store must be written.
    pub changed: bool,
}

/// Brings the entries of `agents` up to date with their logs: an entry is kept while its log has
/// the same key, length and modification time and the current fingerprint; the other logs are
/// read, in parallel. An entry whose log is gone stays, flagged `gone`, unless a log of the same
/// agent with the same session and main ids exists at another key. `progress(done, total)` is
/// called after each log read.
pub fn refresh(
    store: &mut Store,
    catalog: &Catalog,
    agents: &[AgentId],
    pricing: &Pricing,
    temp: &[String],
    progress: impl Fn(usize, usize) + Sync + Send,
) -> Result<Refreshed> {
    let fp = fingerprint(pricing);
    let logs = catalog.discover(agents);
    let names: HashSet<&str> = agents.iter().map(AgentId::name).collect();
    let mut by_key: HashMap<String, usize> = HashMap::new();
    for (i, e) in store.entries.iter().enumerate() {
        by_key.insert(e.key.clone(), i);
    }
    let mut stale = Vec::new();
    let mut out = Refreshed::default();
    let mut live: HashSet<String> = HashSet::new();
    let mut ids: HashSet<(String, String, String)> = HashSet::new();
    for f in &logs {
        let key = key_of(f);
        ids.insert((f.agent.name().to_string(), f.session_id.clone(), f.main_id.clone()));
        let fresh = by_key.get(&key).map(|&i| &store.entries[i]).is_some_and(|e| e.len == f.size && e.mtime_ns == f.modified_ns as i64 && e.fingerprint == fp);
        if fresh {
            out.from_store += 1;
        } else {
            stale.push(f.clone());
        }
        live.insert(key);
    }
    let total = stale.len();
    let done = AtomicUsize::new(0);
    let query = Query { agents: agents.to_vec(), ..Query::default() };
    let (read, stats) = scan_counted(
        catalog,
        &stale,
        &query,
        OnUnreadable::Skip,
        Vec::new,
        |v: &mut Vec<Entry>, s, f| v.push(entry_of(s, f, pricing, temp, &fp)),
        |mut a, b| {
            a.extend(b);
            a
        },
        || progress(done.fetch_add(1, Ordering::Relaxed) + 1, total),
    )?;
    out.unreadable = stats.unreadable;
    out.changed = !read.is_empty();
    for e in read {
        match by_key.get(&e.key) {
            Some(&i) => store.entries[i] = e,
            None => {
                by_key.insert(e.key.clone(), store.entries.len());
                store.entries.push(e);
            }
        }
    }
    out.read = stale;
    // The entries of the logs not found: moved ones go, the others stay as the only record.
    let before = store.entries.len();
    store.entries.retain(|e| {
        let refreshed = names.contains(e.agent.as_str());
        !(refreshed && !live.contains(&e.key) && ids.contains(&(e.agent.clone(), e.session.clone(), e.main.clone())))
    });
    out.changed |= store.entries.len() != before;
    for e in &mut store.entries {
        e.gone = names.contains(e.agent.as_str()) && !live.contains(&e.key);
        out.gone += usize::from(e.gone);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn work_marks_every_minute_it_touches() {
        let at = |m: i64, s: i64| 600 * MINUTE_MS + m * MINUTE_MS + s * 1000;
        // 10:00:40 to 10:02:10 touches minutes 0, 1 and 2 of the hour; an empty interval none.
        let bits = work_bits(&[(at(0, 40), at(2, 10)), (at(30, 0), at(30, 0))]);
        assert_eq!(bits, vec![(10, 0b111)]);
        // Work that ends on a minute's start does not touch that minute.
        assert_eq!(work_bits(&[(at(59, 0), at(60, 0))]), vec![(10, 1 << 59)]);
        assert_eq!(work_bits(&[(at(59, 30), at(60, 30))]), vec![(10, 1 << 59), (11, 1)]);
    }

    #[test]
    fn minutes_are_written_as_arrays() {
        let m = Minute { minute: 7, usd: 0.5, avoidable_usd: 0.25, requests: 2, commits: 1, pushes: 0, pull_requests: 0 };
        let text = serde_json::to_string(&m).unwrap();
        assert_eq!(text, "[7,0.5,0.25,2,1,0,0]");
        assert_eq!(serde_json::from_str::<Minute>(&text).unwrap(), m);
    }

    #[test]
    fn the_fingerprint_follows_the_prices() {
        let a = fingerprint(&Pricing::defaults());
        let mut p = Pricing::defaults();
        p.add_layer("test", None, "[models.\"claude-opus-5-5\"]\ninput = 1.0\noutput = 2.0\n").unwrap();
        assert!(a.starts_with(&format!("tokprof {}; prices ", env!("CARGO_PKG_VERSION"))));
        assert_ne!(a, fingerprint(&p));
        assert_eq!(a, fingerprint(&Pricing::defaults()));
    }

    fn dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("tokprof-daystore-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn a_store_is_written_whole_and_read_back() {
        let d = dir("save");
        let path = d.join(FILE_NAME);
        let entry = Entry { key: "/a.jsonl".into(), len: 3, actions: vec![(5, true)], ..Entry::default() };
        let store = Store { path: path.clone(), entries: vec![entry.clone()], broken: None };
        store.save().unwrap();
        // Two writers at once: each renames a whole file, and the last one wins.
        let other = Store { entries: vec![Entry { key: "/b.jsonl".into(), ..Entry::default() }], ..store.clone() };
        std::thread::scope(|s| {
            s.spawn(|| store.save().unwrap());
            s.spawn(|| other.save().unwrap());
        });
        let back = Store::load(&path);
        assert!(back.entries == vec![entry] || back.entries == other.entries, "one whole store");
        let left: Vec<_> = std::fs::read_dir(&d).unwrap().flatten().map(|e| e.file_name()).collect();
        assert_eq!(left.len(), 1, "no temporary file stays: {left:?}");
    }

    #[test]
    fn a_broken_store_is_moved_aside() {
        let d = dir("broken");
        let path = d.join(FILE_NAME);
        std::fs::write(&path, "{not json").unwrap();
        let store = Store::load(&path);
        assert!(store.entries.is_empty());
        assert_eq!(store.broken, Some(d.join("days-1.json.broken")));
        assert!(!path.exists() && d.join("days-1.json.broken").exists());
        // A store of another format is not read either.
        std::fs::write(&path, "{\"format\":2,\"logs\":[]}").unwrap();
        assert!(Store::load(&path).broken.is_some());
        // No file: an empty store, nothing moved.
        let none = Store::load(&d.join("none.json"));
        assert!(none.entries.is_empty() && none.broken.is_none());
    }
}
