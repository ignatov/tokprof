//! Follows the agents' log directories for `tokprof ui`: new sessions, sessions that are still
//! being written, and logs that are moved or deleted.

use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::time::{Duration, Instant, SystemTime};

use anyhow::{Result, bail};
use notify::event::ModifyKind;
use notify::{Event, EventKind, RecursiveMode, Watcher};
use rayon::prelude::*;

use std::sync::Arc;

use tokprof_core::catalog::Catalog;
use tokprof_core::source::{AgentId, Touched};
use tokprof_core::xray::{self, SessionRow};

/// How long changes are gathered after the first one. An agent writes its log many times a
/// second, and each session that changed is read once per batch.
const BATCH: Duration = Duration::from_millis(500);

/// How often the logs written lately are looked at for writes that raised no event. macOS reports
/// a write to a file once its writer closes it, and Codex keeps its rollout open for as long as the
/// session runs, so a Codex session would never be seen to grow.
const POLL: Duration = Duration::from_secs(2);

/// A log that was not written for this long is no longer looked at; an event brings it back.
const HOT: Duration = Duration::from_secs(3600);

pub enum Change {
    /// A session that is new or whose logs changed.
    Session(Box<SessionRow>),
    /// Nothing is left at this path: a session's main log, or a directory that held sessions.
    Gone(PathBuf),
    /// Every session, after the file system dropped changes.
    All(Vec<SessionRow>),
}

/// A log directory, and how change events name it.
struct Root {
    /// The source whose logs the directory holds.
    source: &'static str,
    path: PathBuf,
    /// The path with symlinks resolved, as events carry it: `/private/var/…` for a temporary
    /// directory under `/var` on macOS.
    real: PathBuf,
}

impl Root {
    /// `path` as a path in this directory, when it is in it.
    fn inside(&self, path: &Path) -> Option<PathBuf> {
        let rest = path.strip_prefix(&self.path).or_else(|_| path.strip_prefix(&self.real)).ok()?;
        Some(if rest.as_os_str().is_empty() { self.path.clone() } else { self.path.join(rest) })
    }
}

/// Watches the log directories from a thread of its own, which hands the changes to `send`
/// until it returns false.
pub fn watch(catalog: Arc<Catalog>, agents: Vec<AgentId>, send: impl FnMut(Change) -> bool + Send + 'static) -> Result<()> {
    let roots = catalog.watched(&agents);
    let (tx, rx) = mpsc::channel();
    let mut watcher = notify::recommended_watcher(tx)?;
    let mut watched = BTreeSet::new();
    for (_, path) in &roots {
        // A directory that is not there yet is watched for in its parent, as Codex creates
        // ~/.codex/archived_sessions when it archives its first thread.
        let target = match path.parent() {
            _ if path.is_dir() => path.as_path(),
            Some(parent) if parent.is_dir() => parent,
            _ => continue,
        };
        if !watched.contains(target) && watcher.watch(target, RecursiveMode::Recursive).is_ok() {
            watched.insert(target.to_path_buf());
        }
    }
    if watched.is_empty() {
        bail!("none of the log directories exists");
    }
    let roots: Vec<Root> = roots.into_iter().map(|(source, path)| Root { source, real: real_path(&path), path }).collect();
    std::thread::spawn(move || {
        // Events stop when the watcher is dropped.
        let _watcher = watcher;
        follow(&rx, &catalog, &agents, &roots, send);
    });
    Ok(())
}

fn follow(events: &Receiver<notify::Result<Event>>, catalog: &Catalog, agents: &[AgentId], roots: &[Root], mut send: impl FnMut(Change) -> bool) {
    let mut hot = Hot::of(catalog, agents);
    let mut poll_at = Instant::now() + POLL;
    loop {
        let mut batch = Vec::new();
        match events.recv_timeout(poll_at.saturating_duration_since(Instant::now())) {
            Ok(first) => batch.push(first),
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => return,
        }
        if !batch.is_empty() {
            let until = Instant::now() + BATCH;
            while let Some(left) = until.checked_duration_since(Instant::now())
                && let Ok(event) = events.recv_timeout(left)
            {
                batch.push(event);
            }
        }
        let batch: Vec<Event> = batch.into_iter().flatten().collect();
        let changes = if batch.iter().any(Event::need_rescan) {
            hot = Hot::of(catalog, agents);
            vec![Change::All(rescan(catalog, agents))]
        } else {
            let mut paths = event_paths(roots, &batch);
            hot.saw(&paths);
            if Instant::now() >= poll_at {
                paths.extend(hot.written());
                poll_at = Instant::now() + POLL;
            }
            changes_at(catalog, &paths)
        };
        for change in changes {
            if !send(change) {
                return;
            }
        }
    }
}

/// The logs written lately, with how each looked when it was last looked at, by path.
struct Hot(HashMap<PathBuf, (&'static str, Option<Stamp>)>);

/// A log's length and last write.
type Stamp = (u64, SystemTime);

fn stamp(path: &Path) -> Option<Stamp> {
    let meta = std::fs::metadata(path).ok().filter(|m| m.is_file())?;
    Some((meta.len(), meta.modified().ok()?))
}

fn is_recent(stamp: &Stamp) -> bool {
    SystemTime::now().duration_since(stamp.1).is_ok_and(|age| age < HOT) || stamp.1 > SystemTime::now()
}

impl Hot {
    /// The logs the agents wrote within the last hour.
    fn of(catalog: &Catalog, agents: &[AgentId]) -> Hot {
        let logs = catalog.discover(agents).into_iter().filter_map(|r| {
            let path = r.path().to_path_buf();
            let stamp = stamp(&path).filter(is_recent)?;
            Some((path, (r.source, Some(stamp))))
        });
        Hot(logs.collect())
    }

    /// Takes in the logs that events named, as they are now.
    fn saw(&mut self, paths: &[(&'static str, PathBuf, bool)]) {
        for (source, path, _) in paths {
            if let Some(now) = stamp(path) {
                self.0.insert(path.clone(), (source, Some(now)));
            }
        }
    }

    /// The logs written since they were last looked at, or gone; logs not written for an hour
    /// are no longer looked at.
    fn written(&mut self) -> Vec<(&'static str, PathBuf, bool)> {
        let mut out = Vec::new();
        self.0.retain(|path, (source, was)| {
            let now = stamp(path);
            if now != *was {
                out.push((*source, path.clone(), false));
                *was = now;
            }
            now.as_ref().is_some_and(is_recent)
        });
        out
    }
}

/// The paths a batch of events names in the log directories, with their source and whether they
/// arrived: a directory that is created or moved in brings its logs along.
fn event_paths(roots: &[Root], events: &[Event]) -> Vec<(&'static str, PathBuf, bool)> {
    let mut out = Vec::new();
    for event in events {
        let arrived = matches!(event.kind, EventKind::Create(_) | EventKind::Modify(ModifyKind::Name(_)));
        for path in &event.paths {
            if let Some((source, path)) = roots.iter().find_map(|r| Some((r.source, r.inside(path)?))) {
                out.push((source, path, arrived));
            }
        }
    }
    out
}

/// What a batch of events means for the sessions.
#[cfg(test)]
fn changes(catalog: &Catalog, roots: &[Root], events: &[Event]) -> Vec<Change> {
    changes_at(catalog, &event_paths(roots, events))
}

/// What changes at these paths mean for the sessions.
fn changes_at(catalog: &Catalog, paths: &[(&'static str, PathBuf, bool)]) -> Vec<Change> {
    let mut sessions = BTreeSet::new();
    let mut gone = BTreeSet::new();
    for (source, path, arrived) in paths {
        let source = *source;
        match catalog.touched(source, path, *arrived) {
            Touched::Log(main) => sessions.extend(main.map(|m| (source, m))),
            Touched::Logs(mains) => sessions.extend(mains.into_iter().map(|m| (source, m))),
            Touched::Other if !path.exists() => {
                gone.insert(path.clone());
            }
            Touched::Other => {}
        }
    }
    let mut out: Vec<Change> = sessions
        .into_iter()
        .map(|(source, path)| match catalog.session_at(source, &path) {
            Some(file) => Change::Session(Box::new(SessionRow::new(catalog, file))),
            None => Change::Gone(path),
        })
        .collect();
    out.extend(gone.into_iter().map(Change::Gone));
    out
}

/// Every session in the log directories.
fn rescan(catalog: &Catalog, agents: &[AgentId]) -> Vec<SessionRow> {
    xray::main_sessions(catalog, agents).into_par_iter().map(|f| SessionRow::new(catalog, f)).collect()
}

/// A path with its symlinks resolved, also when its last part does not exist yet.
fn real_path(path: &Path) -> PathBuf {
    if let Ok(real) = path.canonicalize() {
        return real;
    }
    match (path.parent().and_then(|p| p.canonicalize().ok()), path.file_name()) {
        (Some(parent), Some(name)) => parent.join(name),
        _ => path.to_path_buf(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use notify::event::{CreateKind, RemoveKind};
    use std::fs;

    fn dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("tokprof-watch-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    fn write(path: &Path, text: &str) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }

    /// The changes as `+path subagents` for a session and `-path` for a gone path, relative to `d`.
    fn catalog(d: &Path) -> Catalog {
        Catalog::new(vec![Box::new(tokprof_source_claude_code::ClaudeCode::at(d.to_path_buf(), Path::new("/h")))], PathBuf::from("/h"))
    }

    fn changed(d: &Path, events: &[(EventKind, PathBuf)]) -> Vec<String> {
        let roots = [Root { source: "claude", path: d.to_path_buf(), real: real_path(d) }];
        let events: Vec<Event> = events.iter().map(|(kind, path)| Event::new(*kind).add_path(path.clone())).collect();
        // With `/` on Windows too, where the paths of a listed directory come with `\`.
        let rel = |p: &Path| p.strip_prefix(d).unwrap().display().to_string().replace('\\', "/");
        changes(&catalog(d), &roots, &events)
            .iter()
            .map(|c| match c {
                Change::Session(r) => format!("+{} {}", rel(r.file.path()), r.subagents.len()),
                Change::Gone(p) => format!("-{}", rel(p)),
                Change::All(_) => "all".to_string(),
            })
            .collect()
    }

    #[test]
    fn events_name_the_sessions_they_change() {
        let d = dir("changes");
        let (main, sub) = (d.join("p/s1.jsonl"), d.join("p/s1/subagents/agent-a.jsonl"));
        write(&main, "{\"cwd\":\"/src/p\",\"sessionId\":\"s1\"}\n");
        write(&sub, "{}\n");
        let modify = EventKind::Modify(ModifyKind::Any);
        assert_eq!(
            changed(&d, &[(modify, sub.clone()), (modify, real_path(&main))]),
            ["+p/s1.jsonl 1"],
            "a subagent's log changes its session, and events with resolved paths count too"
        );

        let meta = d.join("p/s1/subagents/agent-a.meta.json");
        write(&meta, "{}");
        assert!(changed(&d, &[(modify, meta), (modify, d.join("p"))]).is_empty(), "other files and directories change nothing");

        let moved = d.join("q");
        write(&moved.join("s2.jsonl"), "{}\n");
        write(&moved.join("s3.jsonl"), "{}\n");
        assert_eq!(changed(&d, &[(EventKind::Create(CreateKind::Folder), moved.clone())]), ["+q/s2.jsonl 0", "+q/s3.jsonl 0"]);

        fs::remove_file(&sub).unwrap();
        fs::remove_file(&main).unwrap();
        fs::remove_dir_all(&moved).unwrap();
        let removed = EventKind::Remove(RemoveKind::Any);
        assert_eq!(changed(&d, &[(removed, sub), (removed, moved)]), ["-p/s1.jsonl", "-q"], "gone logs and directories");
        fs::remove_dir_all(&d).unwrap();
    }

    /// Watches a real directory: a session shows up, grows, and is gone.
    #[test]
    fn follows_a_session_as_it_is_written() {
        let d = dir("follow");
        let (tx, rx) = mpsc::channel();
        let catalog = Arc::new(catalog(&d));
        let agents = catalog.agents();
        watch(catalog, agents, move |c| tx.send(c).is_ok()).unwrap();
        let log = d.join("p/s1.jsonl");
        let next = || loop {
            match rx.recv_timeout(Duration::from_secs(10)).expect("a change within 10 s") {
                Change::Session(r) if r.file.path() == log => return Some(*r),
                Change::Gone(p) if p == log => return None,
                _ => {}
            }
        };
        write(&log, "{\"cwd\":\"/src/p\",\"sessionId\":\"s1\"}\n");
        let row = next().expect("the new session");
        assert_eq!((row.project.as_str(), row.id.as_str()), ("/src/p", "s1"));

        write(&log, "{\"cwd\":\"/src/p\",\"sessionId\":\"s1\"}\n{\"type\":\"user\"}\n");
        while next().expect("the grown session").file.size == row.file.size {}

        fs::remove_file(&log).unwrap();
        while next().is_some() {}
        fs::remove_dir_all(&d).unwrap();
    }

    /// A log its writer keeps open, as Codex keeps its rollout: macOS reports no write to it
    /// until it is closed, and the session still grows in the list.
    #[test]
    fn follows_a_log_its_writer_keeps_open() {
        use std::io::Write;
        let d = dir("open");
        let log = d.join("p/s1.jsonl");
        fs::create_dir_all(log.parent().unwrap()).unwrap();
        let mut f = fs::File::create(&log).unwrap();
        let (tx, rx) = mpsc::channel();
        let catalog = Arc::new(catalog(&d));
        let agents = catalog.agents();
        watch(catalog, agents, move |c| tx.send(c).is_ok()).unwrap();
        // Written after the watch started, without closing the file.
        std::thread::sleep(Duration::from_millis(300));
        f.write_all(b"{\"cwd\":\"/src/p\",\"sessionId\":\"s1\"}\n").unwrap();
        f.flush().unwrap();
        let grown = loop {
            match rx.recv_timeout(Duration::from_secs(10)).expect("a change within 10 s") {
                Change::Session(r) if r.file.path() == log && r.file.size > 0 => break r,
                _ => {}
            }
        };
        assert_eq!(grown.id, "s1");
        drop(f);
        fs::remove_dir_all(&d).unwrap();
    }
}
