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
use crate::pauses::Pauses;
use crate::pricing::Pricing;
use crate::query::Query;
use crate::rhythm::{Lane, Timeline};
use crate::scan::{OnUnreadable, scan_counted};
use crate::source::{AgentId, Locator, SessionRef};
use crate::tools::ToolShare;

/// The store's format, in its file name and its header.
pub const FORMAT: u32 = 2;
/// The store's file name in the cache folder.
pub const FILE_NAME: &str = "days-2.json";

const MINUTE_MS: i64 = 60_000;
const HOUR_MS: i64 = 3_600_000;

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
    /// The prompt tokens the requests read from the cache, and all their prompt tokens.
    pub read_tokens: u64,
    pub prompt_tokens: u64,
    /// What the cache reads saved against sending the same tokens uncached: the tokens read
    /// times the difference between the uncached input price and the cache read price.
    pub saved_usd: f64,
    /// The requests whose model has no price.
    pub unpriced: u32,
}

/// A minute as the file writes it: an array, so that the store stays small.
type MinuteRow = (i64, f64, f64, u32, u32, u32, u32, u64, u64, f64, u32);

impl From<MinuteRow> for Minute {
    fn from((minute, usd, avoidable_usd, requests, commits, pushes, pull_requests, read_tokens, prompt_tokens, saved_usd, unpriced): MinuteRow) -> Minute {
        Minute { minute, usd, avoidable_usd, requests, commits, pushes, pull_requests, read_tokens, prompt_tokens, saved_usd, unpriced }
    }
}

impl From<Minute> for MinuteRow {
    fn from(m: Minute) -> MinuteRow {
        (m.minute, m.usd, m.avoidable_usd, m.requests, m.commits, m.pushes, m.pull_requests, m.read_tokens, m.prompt_tokens, m.saved_usd, m.unpriced)
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
    /// The session's project (see session-logs, Session id and project).
    pub project: String,
    /// The folder the session ran in, only when it differs from the project; an entry of an
    /// earlier build has none.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub folder: String,
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
    /// Per hour since 1970-01-01 00:00 UTC, the avoidable premium of each miss cause that is not
    /// expected, by the cause's name.
    pub misses: Vec<(i64, String, f64)>,
    /// Per hour, the spend of each model.
    pub models: Vec<(i64, String, f64)>,
    /// When the cold returns of the log were sent (see pause-report).
    pub cold: Vec<i64>,
    /// The log was not found in this run; set by the refresh, not kept in the file.
    #[serde(skip)]
    pub gone: bool,
}

/// The build the numbers come from: the tokprof version and the git commit it was built from,
/// or the version alone where it was built without git.
pub fn build_id() -> String {
    match env!("TOKPROF_COMMIT") {
        "" => format!("tokprof {}", env!("CARGO_PKG_VERSION")),
        commit => format!("tokprof {} ({commit})", env!("CARGO_PKG_VERSION")),
    }
}

/// The fingerprint of the numbers an entry holds: the build and the prices in use.
pub fn fingerprint(pricing: &Pricing) -> String {
    let digest = Sha256::digest(format!("{:?}", pricing.entries()).as_bytes());
    let hex: String = digest.iter().take(8).map(|b| format!("{b:02x}")).collect();
    format!("{}; prices {hex}", build_id())
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
    let mut sink = (Minutes::default(), Pauses::new(Filter::default()));
    analysis::analyze(s, pricing, &Filter::default(), &mut sink);
    let (sink, mut pauses) = sink;
    pauses.finish();
    let mut cold: Vec<i64> = pauses.agents.values().flat_map(|a| a.returns.iter().map(|r| r.ts_ms)).collect();
    cold.sort_unstable();
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
        folder: if s.folder == s.project { String::new() } else { s.folder.clone() },
        minutes: sink.rows.into_values().collect(),
        misses: sink.misses.into_iter().map(|((h, c), usd)| (h, c.to_string(), usd)).collect(),
        models: sink.models.into_iter().map(|((h, m), usd)| (h, m, usd)).collect(),
        cold,
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
    misses: BTreeMap<(i64, &'static str), f64>,
    models: BTreeMap<(i64, String), f64>,
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
        let saved = r.price.map_or(0.0, |p| r.usage.cache_read as f64 * (p.input - p.read) / 1e6);
        let row = self.row(minute);
        row.requests += 1;
        row.usd += usd;
        row.read_tokens += r.usage.cache_read;
        row.prompt_tokens += r.usage.prompt();
        row.saved_usd += saved;
        row.unpriced += u32::from(r.price.is_none());
        if r.price.is_some() {
            *self.models.entry((r.ts_ms.div_euclid(HOUR_MS), r.model.to_string())).or_default() += usd;
        }
        self.current = Some(minute);
    }

    fn miss(&mut self, m: MissEvent) {
        if m.cause.expected() {
            return;
        }
        if let Some(p) = m.premium_usd {
            self.row(m.ts_ms.div_euclid(MINUTE_MS)).avoidable_usd += p;
            *self.misses.entry((m.ts_ms.div_euclid(HOUR_MS), m.cause.name())).or_default() += p;
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
    /// The file exists but could not be read, and why: this store is in memory only and must
    /// not be written over it.
    pub unreadable: Option<String>,
}

impl Store {
    /// The store at `path`; empty when the file does not exist. A file that exists but cannot
    /// be read gives an empty store marked `unreadable`, which is never written. A file that
    /// cannot be parsed is kept as `<name>.broken-<UTC time of now_ms>`, never over an earlier
    /// one, and the store starts empty.
    pub fn load(path: &Path, now_ms: i64) -> Store {
        let mut store = Store { path: path.to_path_buf(), ..Store::default() };
        let bytes = match std::fs::read(path) {
            Ok(b) => b,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return store,
            Err(e) => {
                store.unreadable = Some(e.to_string());
                return store;
            }
        };
        match serde_json::from_slice::<File>(&bytes) {
            Ok(f) if f.format == FORMAT => store.entries = f.logs,
            _ => {
                let name = path.file_name().map_or_else(|| FILE_NAME.into(), |n| n.to_string_lossy().to_string());
                let stamp = chrono::DateTime::from_timestamp_millis(now_ms).map_or_else(String::new, |t| t.format("%Y%m%dT%H%M%SZ").to_string());
                let first = path.with_file_name(format!("{name}.broken-{stamp}"));
                let aside = std::iter::once(first.clone())
                    .chain((2..).map(|n| path.with_file_name(format!("{name}.broken-{stamp}-{n}"))))
                    .find(|p| !p.exists())
                    .unwrap_or(first);
                match std::fs::rename(path, &aside) {
                    Ok(()) => store.broken = Some(aside),
                    // A file that cannot be moved aside is not written over either.
                    Err(e) => store.unreadable = Some(format!("it could not be parsed or moved aside: {e}")),
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
        // The new file is whole on disk before it takes the old one's name.
        let written = (|| -> std::io::Result<()> {
            use std::io::Write as _;
            let mut f = std::fs::File::create(&tmp)?;
            f.write_all(&bytes)?;
            f.sync_all()
        })();
        if let Err(e) = written {
            let _ = std::fs::remove_file(&tmp);
            return Err(e).with_context(|| format!("writing {}", tmp.display()));
        }
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
    /// What went wrong with the store itself: unreadable, reset, or not written.
    pub warnings: Vec<String>,
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
    for f in &logs {
        let key = key_of(f);
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
    // The entries of the logs not found: moved ones go, once the log at the new key has an entry
    // of its own; the others stay as the only record.
    let ids: HashSet<(String, String, String)> =
        store.entries.iter().filter(|e| live.contains(&e.key)).map(|e| (e.agent.clone(), e.session.clone(), e.main.clone())).collect();
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
    out.changed |= place_again(store, catalog);
    Ok(out)
}

/// Places again the project of each entry that names no folder, as the folder of a log of its
/// agent (activity-calendar, Projects of kept entries): an entry an earlier build made, or one
/// whose folder was its own project. When the project moves, the old one becomes the folder.
/// Each pair of agent and folder is placed once per process. Whether an entry changed.
fn place_again(store: &mut Store, catalog: &Catalog) -> bool {
    let mut sources: HashMap<String, &'static str> = HashMap::new();
    let mut changed = false;
    for e in store.entries.iter_mut().filter(|e| e.folder.is_empty()) {
        let source = *sources.entry(e.agent.clone()).or_insert_with(|| catalog.agent(&e.agent).ok().and_then(|a| catalog.source_of(&a)).unwrap_or(""));
        let placed = catalog.place(source, &e.project);
        if placed.project != e.project {
            e.folder = std::mem::replace(&mut e.project, placed.project);
            changed = true;
        }
    }
    changed
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
        let m = Minute {
            minute: 7,
            usd: 0.5,
            avoidable_usd: 0.25,
            requests: 2,
            commits: 1,
            read_tokens: 90,
            prompt_tokens: 100,
            saved_usd: 0.125,
            ..Minute::default()
        };
        let text = serde_json::to_string(&m).unwrap();
        assert_eq!(text, "[7,0.5,0.25,2,1,0,0,90,100,0.125,0]");
        assert_eq!(serde_json::from_str::<Minute>(&text).unwrap(), m);
    }

    #[test]
    fn the_fingerprint_follows_the_prices() {
        let a = fingerprint(&Pricing::defaults());
        let mut p = Pricing::defaults();
        p.add_layer("test", None, "[models.\"claude-opus-5-5\"]\ninput = 1.0\noutput = 2.0\n").unwrap();
        assert!(a.starts_with(&format!("{}; prices ", build_id())));
        assert!(build_id().starts_with(&format!("tokprof {}", env!("CARGO_PKG_VERSION"))));
        // Built in a git checkout, the commit is part of it, so another build reads the logs
        // again.
        if !env!("TOKPROF_COMMIT").is_empty() {
            assert!(build_id().contains(env!("TOKPROF_COMMIT")) && env!("TOKPROF_COMMIT").len() == 12);
        }
        assert_ne!(a, fingerprint(&p));
        assert_eq!(a, fingerprint(&Pricing::defaults()));
    }

    #[test]
    fn an_entry_names_the_folder_only_where_it_differs_from_the_project() {
        let f = SessionRef {
            source: "test",
            agent: crate::testing::claude(),
            locator: Locator::File("/l/s.jsonl".into()),
            session_id: "s".into(),
            main_id: "s".into(),
            subagent: true,
            size: 0,
            modified_ns: 0,
            created_ms: None,
        };
        let entry = |project: &str, folder: &str| {
            let mut s = Session::new(crate::testing::claude(), "s".into());
            (s.project, s.folder) = (project.into(), folder.into());
            serde_json::to_value(entry_of(&s, &f, &Pricing::defaults(), &[], "fp")).unwrap()
        };
        let worktree = entry("~/src/app", "~/w/pool/4/app");
        assert_eq!((&worktree["project"], &worktree["folder"]), (&"~/src/app".into(), &"~/w/pool/4/app".into()), "a worktree's session writes both");
        let root = entry("~/src/app", "~/src/app");
        assert!(root.get("folder").is_none(), "the root of a repository writes no folder: {root}");
        assert!(entry("~/src/app", "").get("folder").is_none());
    }

    #[test]
    fn entries_with_and_without_a_folder_and_with_unknown_fields_load() {
        let d = dir("folder");
        let path = d.join(FILE_NAME);
        // An entry of an earlier build, one of this build, and one of a later build.
        let text = r#"{"format":2,"logs":[
            {"key":"/a","len":1,"mtime_ns":0,"fingerprint":"f","agent":"claude","session":"a","main":"a","subagent":false,"project":"~/src/app","left_out":false,"minutes":[],"work":[],"actions":[],"misses":[],"models":[],"cold":[]},
            {"key":"/b","len":1,"mtime_ns":0,"fingerprint":"f","agent":"claude","session":"b","main":"b","subagent":false,"project":"~/src/app","folder":"~/w/pool/4/app","left_out":false,"minutes":[],"work":[],"actions":[],"misses":[],"models":[],"cold":[]},
            {"key":"/c","len":1,"mtime_ns":0,"fingerprint":"f","agent":"claude","session":"c","main":"c","subagent":false,"project":"~/src/app","branch":"main","left_out":false,"minutes":[],"work":[],"actions":[],"misses":[],"models":[],"cold":[]}
        ]}"#;
        std::fs::write(&path, text).unwrap();
        let store = Store::load(&path, 0);
        assert!(store.broken.is_none() && store.unreadable.is_none());
        let folders: Vec<&str> = store.entries.iter().map(|e| e.folder.as_str()).collect();
        assert_eq!(folders, ["", "~/w/pool/4/app", ""]);
        store.save().unwrap();
        let written = std::fs::read_to_string(&path).unwrap();
        assert_eq!(written.matches("\"folder\"").count(), 1, "{written}");
        assert!(written.starts_with("{\"format\":2,"));
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
        let store = Store { path: path.clone(), entries: vec![entry.clone()], ..Store::default() };
        store.save().unwrap();
        // Two writers at once: each renames a whole file, and the last one wins.
        let other = Store { entries: vec![Entry { key: "/b.jsonl".into(), ..Entry::default() }], ..store.clone() };
        std::thread::scope(|s| {
            s.spawn(|| store.save().unwrap());
            s.spawn(|| other.save().unwrap());
        });
        let back = Store::load(&path, 0);
        assert!(back.entries == vec![entry] || back.entries == other.entries, "one whole store");
        let left: Vec<_> = std::fs::read_dir(&d).unwrap().flatten().map(|e| e.file_name()).collect();
        assert_eq!(left.len(), 1, "no temporary file stays: {left:?}");
    }

    #[cfg(unix)]
    #[test]
    fn a_store_that_cannot_be_read_is_not_replaced() {
        use std::os::unix::fs::PermissionsExt;
        let d = dir("unreadable");
        let path = d.join(FILE_NAME);
        std::fs::write(&path, "{\"format\":2,\"logs\":[]}").unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o000)).unwrap();
        let store = Store::load(&path, 0);
        // Root reads anything; the rule only shows for a user the file shuts out.
        if std::fs::read(&path).is_err() {
            assert!(store.unreadable.is_some() && store.broken.is_none());
        }
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "{\"format\":2,\"logs\":[]}", "left as it was");
    }

    #[test]
    fn a_failed_write_leaves_no_temporary_file() {
        let d = dir("failed");
        // The path is a folder, so the rename over it fails.
        let path = d.join(FILE_NAME);
        std::fs::create_dir_all(path.join("x")).unwrap();
        let store = Store { path: path.clone(), ..Store::default() };
        assert!(store.save().is_err());
        let left: Vec<_> = std::fs::read_dir(&d).unwrap().flatten().map(|e| e.file_name()).collect();
        assert_eq!(left.len(), 1, "only the folder: {left:?}");
    }

    #[test]
    fn a_broken_store_is_moved_aside() {
        let d = dir("broken");
        let path = d.join(FILE_NAME);
        std::fs::write(&path, "{not json").unwrap();
        let store = Store::load(&path, 0);
        assert!(store.entries.is_empty());
        let first = d.join("days-2.json.broken-19700101T000000Z");
        assert_eq!(store.broken.as_ref(), Some(&first));
        assert!(!path.exists() && first.exists());
        // A store of another format is not read either, and its backup does not replace the
        // first one.
        std::fs::write(&path, "{\"format\":1,\"logs\":[]}").unwrap();
        assert_eq!(Store::load(&path, 0).broken, Some(d.join("days-2.json.broken-19700101T000000Z-2")));
        assert_eq!(std::fs::read_to_string(&first).unwrap(), "{not json");
        // No file: an empty store, nothing moved.
        let none = Store::load(&d.join("none.json"), 0);
        assert!(none.entries.is_empty() && none.broken.is_none());
    }
}
