//! `tokprof handoff`: what a new session needs to carry on from an earlier one.
//!
//! By default that is one line with the earlier session's id and the path of its log. In the
//! reference bench (docs/reference-bench.md), Claude Code and Codex each picked up a cold
//! session from that line. They pulled what they needed out of the log with a short script, for
//! a fraction of what resuming costs. With `--note`, the line comes with a note built from the
//! log without a model, for an agent that cannot read the log itself: the task, the latest
//! prompts, where the work stopped, the task list, the files changed and read, and the latest
//! commands.

use std::fmt::Write as _;

use anyhow::Result;

use crate::catalog::Catalog;
use crate::fmt::local_time;
use crate::labels::display_path;
use crate::model::short_id;
use crate::source::SessionRef;

/// The line to start a new session with.
pub fn line(catalog: &Catalog, s: &SessionRef) -> String {
    format!("Continue from the earlier session {}. Its log: {}.", s.session_id, display_path(&s.path().to_string_lossy(), None, catalog.home()))
}

/// The line and a note about the session.
pub fn with_note(catalog: &Catalog, s: &SessionRef) -> Result<String> {
    let log = display_path(&s.path().to_string_lossy(), None, catalog.home());
    Ok(format!("Continue from the earlier session {}. A note about it follows; its log is {log}.\n\n{}", s.session_id, note(catalog, s)?))
}

/// Text on one line, cut to `n` characters.
fn clip(text: &str, n: usize) -> String {
    let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if text.chars().count() <= n {
        return text;
    }
    let cut: String = text.chars().take(n - 1).collect();
    format!("{cut}…")
}

/// Text cut to `n` characters with its line breaks kept.
fn cut(text: &str, n: usize) -> String {
    if text.chars().count() <= n {
        return text.to_string();
    }
    let cut: String = text.chars().take(n - 1).collect();
    format!("{}…", cut.trim_end())
}

/// A note about the session, built from its log without a model.
pub fn note(catalog: &Catalog, s: &SessionRef) -> Result<String> {
    let e = catalog.activity(s)?;
    let cwd = e.cwd.as_deref();
    let home = catalog.home();
    let shown = |path: &str| display_path(path, cwd, home);
    let when = |ts: Option<i64>| ts.map_or("?".to_string(), |t| local_time(t, "%Y-%m-%d %H:%M"));
    let agent = &s.agent.info().title;
    let mut out = String::new();
    let _ = writeln!(out, "# Handoff from {agent} session {}\n", short_id(&s.agent, &s.session_id));
    let branch = e.branch.as_deref().map_or(String::new(), |b| format!(", branch {b}"));
    let _ = writeln!(out, "Project {}{branch}; {} → {}.", cwd.map_or("?".to_string(), |c| display_path(c, None, home)), when(e.first_ms), when(e.last_ms));
    let _ = writeln!(out, "The full log is {:.1} MB: search it only for details this note lacks.", s.size as f64 / 1e6);
    if let Some((_, task)) = e.prompts.first() {
        let _ = writeln!(out, "\n## Task\n{}", clip(task, 900));
    }
    let latest: Vec<&(i64, String)> = e.prompts.iter().skip(1).rev().take(6).collect();
    if !latest.is_empty() {
        let _ = writeln!(out, "\n## Latest prompts");
        for (ts, p) in latest.into_iter().rev() {
            let _ = writeln!(out, "- {} {}", local_time(*ts, "%m-%d %H:%M"), clip(p, 300));
        }
    }
    if let Some(text) = &e.last_text {
        let _ = writeln!(out, "\n## Where it stopped\n{}", cut(text, 2500));
    }
    if !e.todos.is_empty() {
        let _ = writeln!(out, "\n## Task list");
        for (status, t) in &e.todos {
            let mark = match status.as_str() {
                "completed" => "x",
                "in_progress" => "~",
                _ => " ",
            };
            let _ = writeln!(out, "- [{mark}] {}", clip(t, 160));
        }
    }
    let mut changed: Vec<&String> = Vec::new();
    for f in e.changed.iter().rev() {
        if !changed.contains(&f) && changed.len() < 15 {
            changed.push(f);
        }
    }
    if !changed.is_empty() {
        let _ = writeln!(out, "\n## Files changed, latest first");
        for f in changed {
            let _ = writeln!(out, "- {}", shown(f));
        }
    }
    if !e.read.is_empty() {
        let mut read: Vec<(&String, &(usize, usize))> = e.read.iter().collect();
        read.sort_by_key(|(_, (n, order))| (std::cmp::Reverse(*n), *order));
        let _ = writeln!(out, "\n## Files read most");
        for (f, (n, _)) in read.into_iter().take(8) {
            let _ = writeln!(out, "- {} ×{n}", shown(f));
        }
    }
    let mut commands: Vec<String> = Vec::new();
    for c in e.commands.iter().rev() {
        let c = clip(c.lines().find(|l| !l.trim().is_empty()).unwrap_or_default(), 140);
        if !c.is_empty() && !commands.contains(&c) && commands.len() < 8 {
            commands.push(c);
        }
    }
    if !commands.is_empty() {
        let _ = writeln!(out, "\n## Latest commands");
        for c in commands {
            let _ = writeln!(out, "- `{c}`");
        }
    }
    Ok(out)
}
