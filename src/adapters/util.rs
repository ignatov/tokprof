//! Helpers shared by the adapters.

use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

use anyhow::{Context, Result};
use serde_json::Value;
use sha2::{Digest, Sha256};

/// A log file for reading, decompressed when it ends in `.zst` (archived Codex rollouts).
pub fn open(path: &Path) -> Result<Box<dyn BufRead>> {
    let file = File::open(path).with_context(|| format!("opening {}", path.display()))?;
    if path.extension().is_some_and(|e| e == "zst") {
        let decoder = ruzstd::decoding::StreamingDecoder::new(BufReader::new(file))
            .map_err(|e| anyhow::anyhow!("{}: {e}", path.display()))?;
        Ok(Box::new(BufReader::with_capacity(1 << 20, decoder)))
    } else {
        Ok(Box::new(BufReader::with_capacity(1 << 20, file)))
    }
}

/// Calls `f` for every line of a JSONL file, tolerating invalid UTF-8.
pub fn for_each_line(path: &Path, mut f: impl FnMut(&str)) -> Result<()> {
    let mut reader = open(path)?;
    let mut buf = Vec::new();
    loop {
        buf.clear();
        if reader.read_until(b'\n', &mut buf)? == 0 {
            return Ok(());
        }
        match std::str::from_utf8(&buf) {
            Ok(s) => f(s),
            Err(_) => f(&String::from_utf8_lossy(&buf)),
        }
    }
}

pub fn parse_ts_ms(v: Option<&Value>) -> Option<i64> {
    match v? {
        Value::String(s) => chrono::DateTime::parse_from_rfc3339(s).ok().map(|t| t.timestamp_millis()),
        Value::Number(n) => n.as_i64().map(|t| if t < 100_000_000_000 { t * 1000 } else { t }),
        _ => None,
    }
}

pub fn u64_at(v: &Value, key: &str) -> u64 {
    v.get(key).and_then(Value::as_u64).unwrap_or(0)
}

pub fn str_at<'a>(v: &'a Value, key: &str) -> Option<&'a str> {
    v.get(key).and_then(Value::as_str)
}

/// SHA-256 of the canonical JSON form (sorted keys, no whitespace, UTF-8). This matches the
/// content hashes Claude Code uses in its request records.
pub fn canonical_hash(v: &Value) -> String {
    let mut out = Vec::with_capacity(4096);
    write_canonical(v, &mut out);
    format!("{:x}", Sha256::digest(&out))
}

pub fn canonical_string(v: &Value) -> String {
    let mut out = Vec::new();
    write_canonical(v, &mut out);
    String::from_utf8(out).unwrap_or_default()
}

fn write_canonical(v: &Value, out: &mut Vec<u8>) {
    match v {
        Value::Object(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            out.push(b'{');
            for (i, k) in keys.iter().enumerate() {
                if i > 0 {
                    out.push(b',');
                }
                serde_json::to_writer(&mut *out, k).expect("writing to a Vec cannot fail");
                out.push(b':');
                write_canonical(&map[k.as_str()], out);
            }
            out.push(b'}');
        }
        Value::Array(items) => {
            out.push(b'[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push(b',');
                }
                write_canonical(item, out);
            }
            out.push(b']');
        }
        scalar => serde_json::to_writer(&mut *out, scalar).expect("writing to a Vec cannot fail"),
    }
}

/// JSON length of a value, used as a size estimate for structured content.
pub fn json_len(v: &Value) -> u64 {
    match v {
        Value::Null => 0,
        Value::String(s) => s.len() as u64,
        other => serde_json::to_string(other).map_or(0, |s| s.len() as u64),
    }
}

/// Shows a path relative to the project, or to the home directory.
pub fn display_path(path: &str, cwd: Option<&str>) -> String {
    if let Some(cwd) = cwd
        && let Some(rest) = path.strip_prefix(cwd) {
            if rest.is_empty() {
                return ".".to_string();
            }
            if let Some(rest) = rest.strip_prefix('/') {
                return rest.to_string();
            }
        }
    let home = super::home();
    if let Some(home) = home.to_str().filter(|h| !h.is_empty())
        && let Some(rest) = path.strip_prefix(home) {
            return format!("~{rest}");
        }
    path.to_string()
}

/// First words of a shell command: `cargo test`, `git status`, `python3 -I`.
pub fn command_head(cmd: &str) -> String {
    let mut cmd = cmd.trim();
    if cmd.starts_with("cd ")
        && let Some((_, rest)) = cmd.split_once("&&") {
            cmd = rest.trim();
        }
    let words: Vec<&str> = cmd
        .split_whitespace()
        .skip_while(|w| w.contains('=') && !w.starts_with('-'))
        .take(2)
        .collect();
    let mut head = words
        .iter()
        .enumerate()
        .map(|(i, w)| if i == 0 { w.rsplit('/').next().unwrap_or(w) } else { w })
        .collect::<Vec<_>>()
        .join(" ");
    if head.len() > 60 {
        head.truncate(head.char_indices().nth(60).map_or(head.len(), |(i, _)| i));
    }
    head
}

pub fn url_host(url: &str) -> Option<String> {
    let rest = url.split_once("://").map_or(url, |(_, r)| r);
    rest.split(['/', '?', '#']).next().filter(|h| !h.is_empty()).map(str::to_string)
}

/// What a tool call worked on, for the `target` profile dimension.
pub fn tool_target(tool: &str, input: &Value, cwd: Option<&str>) -> Option<String> {
    let s = |k: &str| input.get(k).and_then(Value::as_str);
    if let Some(rest) = tool.strip_prefix("mcp__") {
        return rest.split("__").next().map(|server| format!("mcp:{server}"));
    }
    let target = match tool {
        "Read" | "Edit" | "Write" | "MultiEdit" | "NotebookEdit" | "NotebookRead" => {
            s("file_path").or(s("notebook_path")).map(|p| display_path(p, cwd))
        }
        "Grep" | "Glob" | "LS" => s("path").map(|p| display_path(p, cwd)).or(Some("(project)".to_string())),
        "Bash" | "BashOutput" | "shell" | "exec_command" => s("command").or(s("cmd")).map(command_head),
        "WebFetch" => s("url").and_then(url_host),
        "Agent" | "Task" => s("subagent_type").or(s("agent_type")).map(str::to_string),
        "Skill" => s("skill").or(s("command")).map(str::to_string),
        _ => None,
    };
    target.filter(|t| !t.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn canonical_form_sorts_keys_and_keeps_unicode() {
        let v = json!({"role": "user", "content": [{"type": "text", "text": "привет \"x\"\n"}]});
        assert_eq!(
            canonical_string(&v),
            r#"{"content":[{"text":"привет \"x\"\n","type":"text"}],"role":"user"}"#
        );
    }

    #[test]
    fn command_heads() {
        assert_eq!(command_head("cd /tmp && cargo test --all"), "cargo test");
        assert_eq!(command_head("RUST_LOG=debug /usr/bin/python3 -I x.py"), "python3 -I");
    }

    #[test]
    fn paths_are_shown_relative_to_the_project() {
        assert_eq!(display_path("/w/p/src/a.rs", Some("/w/p")), "src/a.rs");
        assert_eq!(display_path("/w/p", Some("/w/p")), ".");
        assert_eq!(display_path("/etc/hosts", Some("/w/p")), "/etc/hosts");
    }
}
