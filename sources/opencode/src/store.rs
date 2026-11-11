//! Reading an OpenCode store: a SQLite file that holds every session, in the tables of opencode
//! 1.x (`session`, `message`, `part`) and of 2.x (`session_v2`, `session_message`), often both.
//! A store is opened read-only and never written; SQLite reads the journal OpenCode writes first.

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use rusqlite::{Connection, OpenFlags, params};
use serde_json::{Map, Value, json};

/// One session as the store lists it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SessionRow {
    pub id: String,
    pub parent_id: Option<String>,
    pub directory: String,
    pub title: Option<String>,
    pub agent: Option<String>,
    pub created_ms: i64,
    pub updated_ms: i64,
    /// The bytes of its rows, its messages' and their parts'.
    pub bytes: u64,
    /// The latest `time_updated` of its messages.
    pub last_message_ms: Option<i64>,
}

impl SessionRow {
    /// When the session or one of its messages was last written.
    pub fn written_ms(&self) -> i64 {
        self.updated_ms.max(self.last_message_ms.unwrap_or(0))
    }
}

/// An open store and the tables it has.
pub(crate) struct Store {
    db: Connection,
    tables: HashSet<String>,
}

/// The columns of a session table that tokprof reads; one an older release lacks reads as null.
const SESSION_COLUMNS: [&str; 7] = ["id", "parent_id", "directory", "title", "agent", "time_created", "time_updated"];

impl Store {
    pub fn open(path: &Path) -> Result<Store> {
        let flags = OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX;
        let db = Connection::open_with_flags(path, flags).with_context(|| format!("opening {}", path.display()))?;
        db.busy_timeout(Duration::from_secs(2))?;
        let tables: HashSet<String> = {
            let mut q = db.prepare("SELECT name FROM sqlite_master WHERE type = 'table'")?;
            q.query_map([], |r| r.get::<_, String>(0))?.collect::<rusqlite::Result<_>>()?
        };
        if !tables.contains("session") && !tables.contains("session_v2") {
            bail!("not an OpenCode store: no session table");
        }
        Ok(Store { db, tables })
    }

    fn has(&self, table: &str) -> bool {
        self.tables.contains(table)
    }

    /// The query of a session table's rows, with null for a column it lacks.
    fn select_sessions(&self, table: &str, filter: &str) -> Result<String> {
        let mut q = self.db.prepare(&format!("SELECT name FROM pragma_table_info('{table}')"))?;
        let have: HashSet<String> = q.query_map([], |r| r.get::<_, String>(0))?.collect::<rusqlite::Result<_>>()?;
        let columns: Vec<&str> = SESSION_COLUMNS.iter().map(|c| if have.contains(*c) { *c } else { "NULL" }).collect();
        Ok(format!("SELECT {} FROM {table} {filter}", columns.join(", ")))
    }

    /// Every session, a session in both layouts once, as `session_v2` gives it.
    pub fn sessions(&self) -> Result<Vec<SessionRow>> {
        let mut out: Vec<SessionRow> = Vec::new();
        let mut seen = HashSet::new();
        for table in ["session_v2", "session"].into_iter().filter(|t| self.has(t)) {
            let mut q = self.db.prepare(&self.select_sessions(table, "")?)?;
            let rows = q.query_map([], session_row)?;
            for row in rows {
                let row = row?;
                if seen.insert(row.id.clone()) {
                    out.push(row);
                }
            }
        }
        // The size and last write of each session's messages, in each layout. The parts (1.x
        // `part`, 2.x `session_part`), which hold most of the text, are left out: summing them
        // reads the whole table, 0.8 s for 400 MB, on every write OpenCode makes while the ui
        // follows it.
        let mut sizes: HashMap<String, ([u64; 2], Option<i64>)> = HashMap::new();
        for (layout, table) in [(0, "message"), (1, "session_message")] {
            for (id, bytes, last) in self.message_sizes(table)? {
                let e = sizes.entry(id).or_default();
                e.0[layout] = bytes;
                e.1 = e.1.max(last);
            }
        }
        for s in &mut out {
            if let Some((bytes, last)) = sizes.get(&s.id) {
                // A session in both layouts holds most of its messages twice: the larger counts.
                s.bytes = bytes[0].max(bytes[1]);
                s.last_message_ms = *last;
            }
        }
        Ok(out)
    }

    /// The bytes and the latest `time_updated` of each session's rows in a message table, or
    /// none when the table is not there or lacks a column. A read that fails, as on a lock held
    /// too long, is an error. `octet_length` reads the size of a value, not the value.
    fn message_sizes(&self, table: &str) -> Result<Vec<(String, u64, Option<i64>)>> {
        if !self.has(table) {
            return Ok(Vec::new());
        }
        let Ok(mut q) = self.db.prepare(&format!("SELECT session_id, SUM(octet_length(data)), MAX(time_updated) FROM {table} GROUP BY session_id")) else {
            return Ok(Vec::new());
        };
        let rows = q.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, Option<i64>>(1)?.unwrap_or(0).max(0) as u64, r.get::<_, Option<i64>>(2)?)))?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// The session with this id, from `session_v2` else `session`.
    pub fn session(&self, id: &str) -> Result<Option<SessionRow>> {
        for table in ["session_v2", "session"].into_iter().filter(|t| self.has(t)) {
            let mut q = self.db.prepare(&self.select_sessions(table, "WHERE id = ?1")?)?;
            if let Some(row) = q.query_map(params![id], session_row)?.next() {
                return Ok(Some(row?));
            }
        }
        Ok(None)
    }

    /// The records a decoder reads for a session: the session first, then each of its own
    /// messages once, oldest first. A message in both layouts is read from the 1.x tables, whose
    /// copy keeps what 2.x's leaves out; a message older than the session is a fork's copy.
    /// `with_text` keeps the messages' text, which the handoff note reads.
    pub fn records(&self, id: &str, with_text: bool) -> Result<Vec<String>> {
        let Some(session) = self.session(id)? else { bail!("no session {id} in the store") };
        let mut messages: HashMap<String, Message> = HashMap::new();
        if self.has("message") {
            let parts = if self.has("part") { self.parts(id, with_text)? } else { HashMap::new() };
            let mut q = self.db.prepare("SELECT id, time_created, time_updated, data FROM message WHERE session_id = ?1")?;
            let rows = q.query_map(params![id], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?, r.get::<_, i64>(2)?, r.get::<_, String>(3)?)))?;
            for row in rows {
                let (mid, created, updated, data) = row?;
                let Ok(data) = serde_json::from_str::<Value>(&data) else { continue };
                let kind = data.get("role").and_then(Value::as_str).unwrap_or_default().to_string();
                let parts = parts.get(&mid);
                messages.insert(mid.clone(), Message { id: mid, layout: "v1", kind, created, updated, data, parts: parts.cloned() });
            }
        }
        if self.has("session_message") {
            let mut q = self.db.prepare("SELECT id, type, time_created, time_updated, data FROM session_message WHERE session_id = ?1")?;
            let rows = q.query_map(params![id], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, i64>(2)?, r.get::<_, i64>(3)?, r.get::<_, String>(4)?))
            })?;
            for row in rows {
                let (mid, kind, created, updated, data) = row?;
                if messages.contains_key(&mid) {
                    continue;
                }
                let Ok(mut data) = serde_json::from_str::<Value>(&data) else { continue };
                if !with_text && let Some(m) = data.as_object_mut() {
                    m.remove("content");
                }
                messages.insert(mid.clone(), Message { id: mid, layout: "v2", kind, created, updated, data, parts: None });
            }
        }
        let mut messages: Vec<Message> = messages.into_values().filter(|m| m.created >= session.created_ms).collect();
        messages.sort_by(|a, b| a.created.cmp(&b.created).then_with(|| a.id.cmp(&b.id)));
        let mut out = Vec::with_capacity(messages.len() + 1);
        out.push(
            json!({"session": {"id": session.id, "parent_id": session.parent_id, "directory": session.directory, "title": session.title,
                "agent": session.agent, "created": session.created_ms}})
            .to_string(),
        );
        out.extend(messages.into_iter().map(|m| m.record().to_string()));
        Ok(out)
    }

    /// For each 1.x message of a session, what its parts tell: the usage and times of its steps,
    /// whether it asked for a compaction, and with `with_text` its text.
    fn parts(&self, id: &str, with_text: bool) -> Result<HashMap<String, Parts>> {
        let mut out: HashMap<String, Parts> = HashMap::new();
        let mut q = self.db.prepare("SELECT message_id, time_created, data FROM part WHERE session_id = ?1 ORDER BY message_id, id")?;
        let rows = q.query_map(params![id], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?, r.get::<_, String>(2)?)))?;
        for row in rows {
            let (mid, created, data) = row?;
            let Ok(part) = serde_json::from_str::<Value>(&data) else { continue };
            let p = out.entry(mid).or_default();
            match part.get("type").and_then(Value::as_str) {
                Some("step-start") => p.started = Some(created),
                Some("step-finish") => {
                    let mut step = Map::new();
                    step.insert("tokens".into(), part.get("tokens").cloned().unwrap_or(Value::Null));
                    step.insert("cost".into(), part.get("cost").cloned().unwrap_or(Value::Null));
                    step.insert("start".into(), p.started.take().map_or(Value::Null, Value::from));
                    step.insert("end".into(), Value::from(created));
                    p.steps.push(Value::Object(step));
                }
                Some("compaction") => p.compaction = Some(part.get("auto").and_then(Value::as_bool).unwrap_or(false)),
                Some("text") if with_text && !part.get("synthetic").and_then(Value::as_bool).unwrap_or(false) => {
                    if let Some(text) = part.get("text").and_then(Value::as_str) {
                        p.text.push(text.to_string());
                    }
                }
                _ => {}
            }
        }
        Ok(out)
    }
}

/// A session as `select_sessions` reads it.
fn session_row(r: &rusqlite::Row) -> rusqlite::Result<SessionRow> {
    Ok(SessionRow {
        id: r.get(0)?,
        parent_id: r.get(1)?,
        directory: r.get::<_, Option<String>>(2)?.unwrap_or_default(),
        title: r.get::<_, Option<String>>(3)?.filter(|t| !t.is_empty()),
        agent: r.get(4)?,
        created_ms: r.get::<_, Option<i64>>(5)?.unwrap_or(0),
        updated_ms: r.get::<_, Option<i64>>(6)?.unwrap_or(0),
        bytes: 0,
        last_message_ms: None,
    })
}

#[derive(Clone, Debug, Default)]
struct Parts {
    started: Option<i64>,
    steps: Vec<Value>,
    compaction: Option<bool>,
    text: Vec<String>,
}

struct Message {
    id: String,
    layout: &'static str,
    kind: String,
    created: i64,
    updated: i64,
    data: Value,
    parts: Option<Parts>,
}

impl Message {
    fn record(self) -> Value {
        let mut m = json!({"id": self.id, "layout": self.layout, "type": self.kind, "created": self.created, "updated": self.updated, "data": self.data});
        if let Some(p) = self.parts {
            m["steps"] = Value::Array(p.steps);
            if let Some(auto) = p.compaction {
                m["compaction"] = json!({"auto": auto});
            }
            if !p.text.is_empty() {
                m["text"] = Value::from(p.text.join("\n\n"));
            }
        }
        json!({ "message": m })
    }
}
