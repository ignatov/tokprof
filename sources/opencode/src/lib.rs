//! OpenCode sessions: no log files, but SQLite stores in `$XDG_DATA_HOME/opencode`, by default
//! `~/.local/share/opencode`. `opencode.db` holds every session, `opencode-<channel>.db` those of
//! a channel of its own, and `$OPENCODE_DB` names another store. opencode 1.x writes the tables
//! `session`, `message` and `part`; 2.x writes `session_v2` and `session_message`, and copies the
//! 1.x sessions there (opencode 1.18.16, 1.19.3 and 2.2.4). Each session is a log of its own,
//! known by the store and its id.

mod decoder;
mod store;

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::Result;
use serde_json::Value;
use tokprof_core::labels::display_path;
use tokprof_core::source::{
    Activity, AgentId, AgentInfo, Capabilities, Claim, Decoder, Detail, Harness, Head, Locator, LogSource, OwnCost, PublicNames, Records, SessionRef, Touched,
};
use tokprof_logio::str_at;

use decoder::Parser;
use store::{SessionRow, Store};

/// A store 2.x imports into `opencode.db` and leaves where it was.
const IMPORTED: &str = "opencode-next.db";

/// A change to a store gives the sessions in it written within this long.
const RECENT_MS: i64 = 3600 * 1000;

pub struct OpenCode {
    /// The data directory, where the stores are.
    dir: PathBuf,
    /// The store `$OPENCODE_DB` names.
    named: Option<PathBuf>,
    /// The home directory, which paths are shown relative to.
    home: PathBuf,
    agent: AgentId,
    /// What each store held when it was last listed, while it is unchanged: a list asks for the
    /// subagents and the last activity of every session.
    listed: Mutex<HashMap<PathBuf, Arc<Listing>>>,
}

/// The sessions of a store, as it was when its files looked like `stamp`.
#[derive(Default)]
struct Listing {
    stamp: Stamp,
    logs: Vec<SessionRef>,
    /// Each session's row, by id.
    rows: HashMap<String, SessionRow>,
    /// The main sessions the listing before this one did not hold, such as ones 2.x imported
    /// with their old times.
    fresh: HashSet<String>,
}

/// The length and last write of a store and of its journal.
type Stamp = [Option<(u64, SystemTime)>; 2];

fn stamp_of(db: &Path) -> Stamp {
    let one = |p: &Path| std::fs::metadata(p).ok().and_then(|m| Some((m.len(), m.modified().ok()?)));
    [one(db), one(&journal_of(db))]
}

/// The journal SQLite writes before the store: `<store>-wal`.
fn journal_of(db: &Path) -> PathBuf {
    let mut wal = db.as_os_str().to_owned();
    wal.push("-wal");
    PathBuf::from(wal)
}

impl OpenCode {
    /// The stores in `$XDG_DATA_HOME/opencode`, by default `~/.local/share/opencode`, and the one
    /// `$OPENCODE_DB` names.
    pub fn from_env(home: &Path) -> OpenCode {
        let var = |name: &str| std::env::var_os(name).filter(|v| !v.is_empty()).map(PathBuf::from);
        let data = var("XDG_DATA_HOME").filter(|d| d.is_absolute()).unwrap_or_else(|| home.join(".local/share"));
        let mut source = OpenCode::at(data.join("opencode"), home);
        source.named = var("OPENCODE_DB").filter(|p| p.as_os_str() != ":memory:").map(|p| source.dir.join(p));
        source
    }

    /// The stores in a data directory.
    pub fn at(dir: PathBuf, home: &Path) -> OpenCode {
        OpenCode { dir, named: None, home: home.to_path_buf(), agent: OpenCode::agent(), listed: Mutex::default() }
    }

    pub fn agent() -> AgentId {
        AgentId::new(AgentInfo {
            name: "opencode".to_string(),
            title: "OpenCode".to_string(),
            // `ses_`, 12 hex digits of the time and 4 of the 14 random characters.
            short_id_len: 20,
            instruction_files: vec!["AGENTS.md".to_string(), "CLAUDE.md".to_string()],
            ttl_settings: None,
            setup_hints: Vec::new(),
            cost_counter: Some("OpenCode's own cost count".to_string()),
            // No pickup measured, and the compaction call is not known to read the cache.
            harness: Harness { pickup: None, compaction_reads_cache: false },
            capabilities: Capabilities {
                own_cost: OwnCost::Session,
                compaction_trigger: true,
                context_window: false,
                prompt_composition: false,
                titles: true,
                subagent_tasks: true,
                origins: false,
            },
            public_names: PublicNames::default(),
        })
    }

    /// The stores there are now: those of the data directory and the named one.
    fn stores(&self) -> Vec<PathBuf> {
        let mut out: Vec<PathBuf> = std::fs::read_dir(&self.dir)
            .into_iter()
            .flatten()
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.file_name().and_then(|n| n.to_str()).is_some_and(is_store_name) && p.is_file())
            .collect();
        out.sort();
        if let Some(named) = self.named.as_ref().filter(|p| p.is_file() && !out.contains(p)) {
            out.push(named.clone());
        }
        out
    }

    /// The store as it is now, listed again only when one of its files changed: every session
    /// in it, a subagent's under its topmost session. A store that cannot be read gives an error
    /// and is not kept, so that the next look tries again.
    fn listing(&self, db: &Path) -> Result<Arc<Listing>> {
        let stamp = stamp_of(db);
        if let Some(l) = self.listed.lock().unwrap_or_else(|e| e.into_inner()).get(db).filter(|l| l.stamp == stamp) {
            return Ok(l.clone());
        }
        let sessions = Store::open(db).and_then(|s| s.sessions())?;
        let parents: HashMap<&str, Option<&str>> = sessions.iter().map(|s| (s.id.as_str(), s.parent_id.as_deref())).collect();
        let logs: Vec<SessionRef> = sessions.iter().map(|s| self.reference(db, s, root_of(&parents, &s.id))).collect();
        let rows = sessions.iter().map(|s| (s.id.clone(), s.clone())).collect();
        let mut listed = self.listed.lock().unwrap_or_else(|e| e.into_inner());
        let fresh = match listed.get(db) {
            Some(before) => logs.iter().filter(|r| !r.subagent && !before.rows.contains_key(&r.session_id)).map(|r| r.session_id.clone()).collect(),
            None => HashSet::new(),
        };
        let listing = Arc::new(Listing { stamp, logs, rows, fresh });
        listed.insert(db.to_path_buf(), listing.clone());
        Ok(listing)
    }

    /// The listing of a store as it is now, else as it was when it was last read, else none: a
    /// store that cannot be read for a moment keeps its sessions as they were.
    fn known(&self, db: &Path) -> Arc<Listing> {
        self.listing(db).unwrap_or_else(|_| self.listed.lock().unwrap_or_else(|e| e.into_inner()).get(db).cloned().unwrap_or_default())
    }

    fn reference(&self, db: &Path, s: &SessionRow, root: &str) -> SessionRef {
        SessionRef {
            source: "opencode",
            agent: self.agent.clone(),
            locator: Locator::Row { db: db.to_path_buf(), id: s.id.clone() },
            session_id: s.id.clone(),
            main_id: root.to_string(),
            subagent: root != s.id,
            size: s.bytes,
            modified_ns: s.written_ms() as i128 * 1_000_000,
            created_ms: Some(s.created_ms),
        }
    }

    /// The main sessions of a listing written since `since_ms`, in their own logs or their
    /// subagents', the one written last first.
    fn mains_of(listing: &Listing, since_ms: Option<i64>) -> Vec<SessionRef> {
        let mut written: HashMap<&str, i128> = HashMap::new();
        for r in listing.logs.iter().filter(|r| since_ms.is_none_or(|since| r.modified_ms() >= since)) {
            let at = written.entry(r.main_id.as_str()).or_default();
            *at = (*at).max(r.modified_ns);
        }
        let mut mains: Vec<(i128, SessionRef)> =
            listing.logs.iter().filter(|r| !r.subagent).filter_map(|r| Some((*written.get(r.session_id.as_str())?, r.clone()))).collect();
        mains.sort_by_key(|(at, _)| std::cmp::Reverse(*at));
        mains.into_iter().map(|(_, r)| r).collect()
    }

    /// The store at a path, and the session in it a locator names.
    fn row<'a>(&self, s: &'a SessionRef) -> Result<(&'a Path, &'a str)> {
        match &s.locator {
            Locator::Row { db, id } => Ok((db, id)),
            other => anyhow::bail!("not a session of an OpenCode store: {other:?}"),
        }
    }

    /// The store a changed path belongs to: the store itself or the journal beside it. The
    /// `-shm` index beside a store changes when it is read, by tokprof too, so it is none.
    fn store_of(&self, changed: &Path) -> Option<PathBuf> {
        let name = changed.file_name()?.to_str()?;
        if name.ends_with("-shm") {
            return None;
        }
        let store = ["-wal", "-journal"].iter().find_map(|s| name.strip_suffix(s)).unwrap_or(name);
        let path = changed.with_file_name(store);
        (is_store_name(store) || self.named.as_ref() == Some(&path)).then_some(path)
    }
}

/// `opencode.db` or `opencode-<channel>.db`, but not the store 2.x imports.
fn is_store_name(name: &str) -> bool {
    name != IMPORTED && name.starts_with("opencode") && name.ends_with(".db") && (name == "opencode.db" || name.starts_with("opencode-"))
}

/// The topmost session above `id` that is in the store.
fn root_of<'a>(parents: &HashMap<&'a str, Option<&'a str>>, id: &'a str) -> &'a str {
    let mut at = id;
    // A cycle, which a store should never hold, ends after as many steps as there are sessions.
    for _ in 0..parents.len() {
        match parents.get(at).copied().flatten() {
            Some(parent) if parents.contains_key(parent) => at = parent,
            _ => break,
        }
    }
    at
}

fn now_ms() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_millis() as i64)
}

impl LogSource for OpenCode {
    fn name(&self) -> &'static str {
        "opencode"
    }

    fn agents(&self) -> Vec<AgentId> {
        vec![self.agent.clone()]
    }

    fn detail(&self) -> Detail {
        Detail::Requests
    }

    fn discover(&self) -> Vec<SessionRef> {
        self.stores().iter().flat_map(|db| self.known(db).logs.clone()).collect()
    }

    /// A store given as a path means its main session that was written last.
    fn session_at(&self, path: &Path) -> Option<SessionRef> {
        OpenCode::mains_of(&*self.listing(path).ok()?, None).into_iter().next()
    }

    fn claim(&self, path: &Path) -> Claim {
        let named = self.named.as_deref() == Some(path);
        let name = path.file_name().and_then(|n| n.to_str()).is_some_and(is_store_name);
        if (named || name) && Store::open(path).is_ok() { Claim::Yes } else { Claim::No }
    }

    /// The data directory. A store `$OPENCODE_DB` names elsewhere is not watched, since the watch
    /// would take in all below its directory: the poll follows it while it was written in the
    /// last hour, and the ui shows it anew when it starts or scans again.
    fn watched_paths(&self) -> Vec<PathBuf> {
        vec![self.dir.clone()]
    }

    /// For a store or its journal: its main sessions written in the last hour, and all it holds.
    /// A store that is gone holds nothing; one that cannot be read now tells nothing.
    fn touched(&self, changed: &Path, _arrived: bool) -> Touched {
        let Some(db) = self.store_of(changed) else { return Touched::Other };
        let listing = match self.listing(&db) {
            Ok(l) => l,
            Err(_) if !db.exists() => Arc::default(),
            Err(_) => return Touched::Other,
        };
        let held = listing.logs.iter().filter(|r| !r.subagent).map(|r| r.locator.clone()).collect();
        let mut changed = OpenCode::mains_of(&listing, Some(now_ms() - RECENT_MS));
        let fresh: Vec<SessionRef> =
            listing.logs.iter().filter(|r| listing.fresh.contains(&r.session_id) && !changed.iter().any(|c| c.session_id == r.session_id)).cloned().collect();
        changed.extend(fresh);
        Touched::Store { changed, held, db }
    }

    /// The store and the journal SQLite writes first: the store itself changes at checkpoints.
    fn polled(&self, s: &SessionRef) -> Vec<PathBuf> {
        vec![s.path().to_path_buf(), journal_of(s.path())]
    }

    fn head(&self, s: &SessionRef) -> Head {
        let directory = self.row(s).ok().and_then(|(db, id)| Some(self.known(db).rows.get(id)?.directory.clone()));
        let project = directory.map(|d| display_path(&d, None, &self.home)).unwrap_or_default();
        Head { project, session_id: s.session_id.clone(), client: None, ..Head::default() }
    }

    fn last_active(&self, s: &SessionRef) -> Option<i64> {
        let (db, id) = self.row(s).ok()?;
        self.known(db).rows.get(id)?.last_message_ms
    }

    fn first_active(&self, s: &SessionRef) -> Option<i64> {
        s.created_ms
    }

    fn subagents(&self, s: &SessionRef) -> Vec<SessionRef> {
        let Ok((db, _)) = self.row(s) else { return Vec::new() };
        self.known(db).logs.iter().filter(|r| r.subagent && r.main_id == s.session_id).cloned().collect()
    }

    fn open(&self, s: &SessionRef) -> Result<Box<dyn Records>> {
        let (db, id) = self.row(s)?;
        Ok(Box::new(Rows(Store::open(db)?.records(id, false)?)))
    }

    fn decoder(&self, s: &SessionRef) -> Box<dyn Decoder> {
        Box::new(Parser::new(self.agent.clone(), s.main_id.clone(), s.subagent, self.home.clone()))
    }

    /// Where the session worked and when, what the user typed and what the agent said last.
    fn activity(&self, s: &SessionRef) -> Result<Activity> {
        let (db, id) = self.row(s)?;
        let mut a = Activity::default();
        for record in Store::open(db)?.records(id, true)? {
            let Ok(v) = serde_json::from_str::<Value>(&record) else { continue };
            if let Some(session) = v.get("session") {
                a.cwd = str_at(session, "directory").map(str::to_string);
                continue;
            }
            let Some(m) = v.get("message") else { continue };
            let at = m.get("created").and_then(Value::as_i64);
            a.seen(at);
            let data = m.get("data").unwrap_or(&Value::Null);
            let text = match str_at(m, "layout") {
                Some("v1") => str_at(m, "text").map(str::to_string),
                _ => match str_at(m, "type") {
                    Some("user") => str_at(data, "text").map(str::to_string),
                    _ => assistant_text(data),
                },
            };
            match (str_at(m, "type"), text.filter(|t| !t.trim().is_empty())) {
                (Some("user"), Some(text)) => a.prompts.push((at.unwrap_or(0), text)),
                (Some("assistant"), Some(text)) => a.last_text = Some(text),
                _ => {}
            }
        }
        Ok(a)
    }
}

/// The text of a 2.x assistant message.
fn assistant_text(data: &Value) -> Option<String> {
    let parts: Vec<&str> = data.get("content")?.as_array()?.iter().filter(|c| str_at(c, "type") == Some("text")).filter_map(|c| str_at(c, "text")).collect();
    (!parts.is_empty()).then(|| parts.join("\n\n"))
}

/// The records of one session, read from its store when it was opened.
struct Rows(Vec<String>);

impl Records for Rows {
    fn for_each(&mut self, f: &mut dyn FnMut(&str)) -> Result<()> {
        self.0.iter().for_each(|r| f(r));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn store_names() {
        assert!(is_store_name("opencode.db") && is_store_name("opencode-beta-2.db"));
        assert!(!is_store_name("opencode-next.db") && !is_store_name("opencode.db-wal") && !is_store_name("drafts.sqlite") && !is_store_name("opencodex.db"));
        let oc = OpenCode::at(PathBuf::from("/d/opencode"), Path::new("/h"));
        assert_eq!(oc.store_of(Path::new("/d/opencode/opencode.db-wal")), Some(PathBuf::from("/d/opencode/opencode.db")));
        assert_eq!(oc.store_of(Path::new("/d/opencode/snapshot/x")), None);
    }

    /// A listing of main `a` (written at 10:00, its subagent `s` at 12:00) and main `b` (11:00).
    fn listing() -> Listing {
        let oc = OpenCode::at(PathBuf::from("/d"), Path::new("/h"));
        let row = |id: &str, parent: Option<&str>, at: i64| SessionRow {
            id: id.into(),
            parent_id: parent.map(str::to_string),
            directory: "/h/w".into(),
            title: None,
            agent: None,
            created_ms: 0,
            updated_ms: at,
            bytes: 0,
            last_message_ms: Some(at),
        };
        let rows = [row("a", None, 10), row("s", Some("a"), 12), row("b", None, 11)];
        let logs = rows.iter().map(|r| oc.reference(Path::new("/d/opencode.db"), r, r.parent_id.as_deref().unwrap_or(&r.id))).collect();
        Listing { stamp: Stamp::default(), logs, rows: rows.into_iter().map(|r| (r.id.clone(), r)).collect(), fresh: HashSet::new() }
    }

    #[test]
    fn main_sessions_by_the_last_write_of_their_tree() {
        let ids = |since| OpenCode::mains_of(&listing(), since).into_iter().map(|r| r.session_id).collect::<Vec<_>>();
        assert_eq!(ids(None), ["a", "b"], "a's subagent wrote last");
        assert_eq!(ids(Some(12)), ["a"], "only a's tree was written since");
    }

    #[test]
    fn the_topmost_session_above() {
        let parents: HashMap<&str, Option<&str>> =
            [("m", None), ("a", Some("m")), ("b", Some("a")), ("o", Some("gone")), ("x", Some("y")), ("y", Some("x"))].into();
        assert_eq!([root_of(&parents, "b"), root_of(&parents, "m"), root_of(&parents, "o")], ["m", "m", "o"]);
        assert!(["x", "y"].contains(&root_of(&parents, "x")), "a cycle ends");
    }
}
