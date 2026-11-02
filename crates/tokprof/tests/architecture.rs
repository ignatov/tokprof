//! The layout rules of the workspace: no agent knowledge outside `sources/`, and the dependency
//! edges between the packages.

use std::path::{Path, PathBuf};
use std::process::Command;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// What only a source may know: agent names, the names of agents' tools, and log fields.
const AGENT_WORDS: &[&str] = &[
    "Claude Code",
    "Codex",
    "codex",
    "BashOutput",
    "exec_command",
    "MultiEdit",
    "WebFetch",
    "TodoWrite",
    "update_plan",
    "apply_patch",
    "sessionId",
    "session_meta",
    "cache_creation",
    "parent_thread_id",
    "token_count",
    "response_item",
];

/// The one file of the framework that may name vendors: the cache rules of their APIs.
const ALLOWED: &[&str] = &["crates/tokprof-core/src/api.rs"];

fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).unwrap().flatten() {
        let path = entry.path();
        if path.is_dir() {
            rust_files(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

/// Lines of code and string literals outside test modules and comments.
fn code_lines(path: &Path) -> Vec<(usize, String)> {
    let text = std::fs::read_to_string(path).unwrap();
    let mut out = Vec::new();
    for (i, line) in text.lines().enumerate() {
        if line.trim_start() == "#[cfg(test)]" {
            break;
        }
        let code = line.trim_start();
        if code.starts_with("//") {
            continue;
        }
        let code = code.split(" // ").next().unwrap_or(code);
        out.push((i + 1, code.to_string()));
    }
    out
}

#[test]
fn the_framework_names_no_agent() {
    let root = root();
    let mut files = Vec::new();
    for dir in ["crates/tokprof-core/src", "crates/tokprof-logio/src", "crates/tokprof/src"] {
        rust_files(&root.join(dir), &mut files);
    }
    let mut found = Vec::new();
    for file in &files {
        let shown = file.strip_prefix(&root).unwrap().display().to_string();
        if ALLOWED.contains(&shown.as_str()) {
            continue;
        }
        for (n, line) in code_lines(file) {
            for word in AGENT_WORDS.iter().filter(|w| line.contains(**w)) {
                found.push(format!("{shown}:{n}: {word}: {line}"));
            }
        }
    }
    assert!(found.is_empty(), "agent knowledge outside sources/:\n{}", found.join("\n"));
}

#[test]
fn the_packages_depend_as_the_layout_says() {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    let out = Command::new(cargo).args(["metadata", "--format-version", "1", "--no-deps"]).current_dir(root()).output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let meta: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let mut broken = Vec::new();
    for package in meta["packages"].as_array().unwrap() {
        let name = package["name"].as_str().unwrap();
        // Normal dependencies on other packages of the workspace; dev-dependencies are free.
        let deps: Vec<&str> = package["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|d| d["kind"].is_null() && d["path"].is_string())
            .map(|d| d["name"].as_str().unwrap())
            .collect();
        let allowed: &[&str] = match name {
            "tokprof-core" | "tokprof-logio" => &[],
            n if n.starts_with("tokprof-source-") => &["tokprof-core", "tokprof-logio"],
            "tokprof-sources" => &["tokprof-core", "tokprof-source-*"],
            "tokprof" => &["tokprof-core", "tokprof-sources"],
            other => panic!("a package the layout does not know: {other}"),
        };
        for dep in deps {
            let ok = allowed.iter().any(|a| *a == dep || (a.ends_with('*') && dep.starts_with(a.trim_end_matches('*'))));
            if !ok {
                broken.push(format!("{name} depends on {dep}"));
            }
        }
    }
    assert!(broken.is_empty(), "{}", broken.join("\n"));
}
