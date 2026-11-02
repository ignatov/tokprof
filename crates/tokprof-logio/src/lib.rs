//! Reading agent logs, without knowing any agent: files that may be zstd-compressed, their lines,
//! and the JSON helpers every log format needs.

use std::fs::{File, Metadata};
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::time::SystemTime;

use anyhow::{Context, Result};
use serde_json::Value;
use sha2::{Digest, Sha256};

/// A log file for reading, decompressed when it ends in `.zst`.
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

/// SHA-256 of the canonical JSON form (sorted keys, no whitespace, UTF-8), as agents that
/// address messages by content hash compute it.
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

/// Calls `f` for every file under `dir`, at any depth, with its metadata. A directory that cannot
/// be read adds nothing.
pub fn walk(dir: &Path, f: &mut dyn FnMut(&Path, &Metadata)) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let Ok(meta) = entry.metadata() else { continue };
        let path = entry.path();
        if meta.is_dir() {
            walk(&path, f);
        } else if meta.is_file() {
            f(&path, &meta);
        }
    }
}

/// When a file was last modified, in epoch nanoseconds so that files written within one
/// millisecond keep their order, and when it was created, in epoch ms. A file system that keeps no
/// modification time gives 0, and one that keeps no creation time gives `None`.
pub fn file_times(meta: &Metadata) -> (i128, Option<i64>) {
    let since_epoch = |t: SystemTime| t.duration_since(SystemTime::UNIX_EPOCH).unwrap_or_default();
    (since_epoch(meta.modified().unwrap_or(SystemTime::UNIX_EPOCH)).as_nanos() as i128, meta.created().ok().map(|t| since_epoch(t).as_millis() as i64))
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
}
