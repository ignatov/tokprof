//! Reading agent logs, without knowing any agent: files that may be zstd-compressed, their lines,
//! and the JSON helpers every log format needs.

use std::fs::{File, Metadata};
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom};
use std::ops::ControlFlow;
use std::path::Path;
use std::sync::mpsc::{Receiver, Sender};
use std::thread::JoinHandle;
use std::time::SystemTime;

use anyhow::{Context, Result, bail};
use serde_json::Value;
use sha2::{Digest, Sha256};

/// A log file for reading, decompressed when it ends in `.zst`.
pub fn open(path: &Path) -> Result<Box<dyn BufRead>> {
    if is_compressed(path) {
        Ok(Box::new(BufReader::with_capacity(CHUNK, open_compressed(path)?)))
    } else {
        Ok(Box::new(BufReader::with_capacity(CHUNK, open_file(path)?)))
    }
}

/// How much is read from a log at once.
const CHUNK: usize = 1 << 20;

/// How many chunks of a compressed log are decompressed ahead of the lines handed over.
const AHEAD: usize = 4;

fn is_compressed(path: &Path) -> bool {
    path.extension().is_some_and(|e| e == "zst")
}

fn open_file(path: &Path) -> Result<File> {
    File::open(path).with_context(|| format!("opening {}", path.display()))
}

type Zstd = ruzstd::decoding::StreamingDecoder<BufReader<File>, ruzstd::decoding::FrameDecoder>;

fn open_compressed(path: &Path) -> Result<Zstd> {
    ruzstd::decoding::StreamingDecoder::new(BufReader::new(open_file(path)?)).map_err(|e| anyhow::anyhow!("{}: {e}", path.display()))
}

/// Calls `f` for every line of a JSONL file, tolerating invalid UTF-8. A compressed file is
/// decompressed on a thread of its own, a few chunks ahead, so that its lines are handled while
/// the next ones are decompressed; the lines and any error are the same as `open` gives.
pub fn for_each_line(path: &Path, f: impl FnMut(&str)) -> Result<()> {
    if is_compressed(path) { lines(Ahead::spawn(open_compressed(path)?), f) } else { lines(open(path)?, f) }
}

/// A reader's chunks, read on a thread of its own: the bytes it gave, then the error it stopped
/// with, if any. The thread reads as `open`'s buffer does, a chunk's worth at a time, so that the
/// reader gives the same bytes before an error. It stops when the reader ends or fails, or when
/// nothing reads from it any more: an `Ahead` dropped early waits for nothing, and the thread's
/// next send fails. The chunks' buffers go back to it to be filled again. A thread that panics
/// closes the channel too; that is an error, not the end of the log.
struct Ahead {
    chunks: Receiver<std::io::Result<(Vec<u8>, usize)>>,
    spare: Sender<Vec<u8>>,
    /// The thread, joined once the channel closes to tell an end from a panic.
    thread: Option<JoinHandle<()>>,
    /// The chunk being handed over, its length, and how much of it was.
    chunk: Vec<u8>,
    len: usize,
    at: usize,
}

impl Ahead {
    fn spawn(mut reader: impl Read + Send + 'static) -> Ahead {
        let (send, chunks) = std::sync::mpsc::sync_channel(AHEAD);
        let (spare, spares) = std::sync::mpsc::channel::<Vec<u8>>();
        let thread = std::thread::spawn(move || {
            loop {
                let mut chunk = spares.try_recv().unwrap_or_else(|_| vec![0u8; CHUNK]);
                let read = match reader.read(&mut chunk) {
                    Ok(0) => return,
                    Ok(n) => Ok((chunk, n)),
                    Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                    Err(e) => Err(e),
                };
                let failed = read.is_err();
                if send.send(read).is_err() || failed {
                    return;
                }
            }
        });
        Ahead { chunks, spare, thread: Some(thread), chunk: Vec::new(), len: 0, at: 0 }
    }
}

impl Read for Ahead {
    fn read(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
        let held = self.fill_buf()?;
        let n = held.len().min(out.len());
        out[..n].copy_from_slice(&held[..n]);
        self.consume(n);
        Ok(n)
    }
}

impl BufRead for Ahead {
    fn fill_buf(&mut self) -> std::io::Result<&[u8]> {
        if self.at == self.len {
            let done = std::mem::take(&mut self.chunk);
            (self.len, self.at) = (0, 0);
            if !done.is_empty() {
                let _ = self.spare.send(done);
            }
            match self.chunks.recv() {
                Ok(read) => (self.chunk, self.len) = read?,
                // A closed channel: the thread is done, and the reader ended unless it panicked.
                Err(_) => {
                    if let Some(Err(panic)) = self.thread.take().map(JoinHandle::join) {
                        return Err(stopped(panic.as_ref()));
                    }
                }
            }
        }
        Ok(&self.chunk[self.at..self.len])
    }

    fn consume(&mut self, n: usize) {
        self.at = (self.at + n).min(self.len);
    }
}

/// The error of a decompression whose thread panicked, with the panic's text when it has one.
fn stopped(panic: &(dyn std::any::Any + Send)) -> std::io::Error {
    let text = panic.downcast_ref::<&str>().copied().or_else(|| panic.downcast_ref::<String>().map(String::as_str));
    std::io::Error::other(match text {
        Some(text) => format!("the decompression stopped: {text}"),
        None => "the decompression stopped".to_string(),
    })
}

fn lines(mut reader: impl BufRead, mut f: impl FnMut(&str)) -> Result<()> {
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

/// What `for_each_line_from` read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReadOn {
    /// The byte after the newline of the last line handed over: where the next read goes on.
    pub end: u64,
    /// Up to `CHECK` bytes before `end`, which the next read checks the file still holds.
    pub check: Vec<u8>,
    /// The file goes on after `end` with a last line without a newline that is a whole JSON
    /// value: `for_each_line` would hand it over, and a reader that goes on from `end` misses it.
    pub open: bool,
}

/// How many bytes before where a read stopped the next read checks.
pub const CHECK: usize = 256;

/// Calls `f` for the lines of a plain JSONL file from byte `from` on that end with a newline,
/// as `for_each_line` hands them over, for a reader that follows a file as it grows. `from` is
/// where an earlier read ended, and `check` the bytes before it that the read returned; `None`
/// means the file no longer holds them there, as after it was written anew, or that it is
/// compressed and cannot be read from the middle. A last line without a newline is left for the
/// next read.
pub fn for_each_line_from(path: &Path, from: u64, check: &[u8], mut f: impl FnMut(&str)) -> Result<Option<ReadOn>> {
    if path.extension().is_some_and(|e| e == "zst") {
        return Ok(None);
    }
    let Some(start) = from.checked_sub(check.len() as u64) else { return Ok(None) };
    let mut file = File::open(path).with_context(|| format!("opening {}", path.display()))?;
    file.seek(SeekFrom::Start(start))?;
    let mut held = vec![0u8; check.len()];
    if file.read_exact(&mut held).is_err() || held != check {
        return Ok(None);
    }
    let mut reader = BufReader::with_capacity(1 << 20, file);
    let mut buf = Vec::new();
    let mut end = from;
    // The bytes before `end`, for the next check.
    let mut last = check.to_vec();
    loop {
        buf.clear();
        if reader.read_until(b'\n', &mut buf)? == 0 {
            return Ok(Some(ReadOn { end, check: last, open: false }));
        }
        if buf.last() != Some(&b'\n') {
            let open = serde_json::from_str::<Value>(&String::from_utf8_lossy(&buf)).is_ok();
            return Ok(Some(ReadOn { end, check: last, open }));
        }
        end += buf.len() as u64;
        last.extend_from_slice(&buf[buf.len().saturating_sub(CHECK)..]);
        last.drain(..last.len().saturating_sub(CHECK));
        match std::str::from_utf8(&buf) {
            Ok(s) => f(s),
            Err(_) => f(&String::from_utf8_lossy(&buf)),
        }
    }
}

/// Calls `f` for the lines of a plain JSONL file from the last to the first, without their
/// newlines, until `f` breaks. It reads blocks from the end of the file, so finding something
/// near the end costs no more than reading that far back. Empty lines are skipped, a last line
/// still being written is handed over like the others, and invalid UTF-8 is replaced. A
/// compressed file cannot be read this way.
pub fn for_each_line_back(path: &Path, f: impl FnMut(&str) -> ControlFlow<()>) -> Result<()> {
    lines_back(path, 64 * 1024, f)
}

fn lines_back(path: &Path, mut block: u64, mut f: impl FnMut(&str) -> ControlFlow<()>) -> Result<()> {
    if path.extension().is_some_and(|e| e == "zst") {
        bail!("{} is compressed and cannot be read from its end", path.display());
    }
    let mut file = File::open(path).with_context(|| format!("opening {}", path.display()))?;
    let mut pos = file.metadata()?.len();
    // The start of a line whose end is in the blocks already read.
    let mut carry: Vec<u8> = Vec::new();
    while pos > 0 {
        let start = pos.saturating_sub(block);
        let mut buf = vec![0u8; (pos - start) as usize];
        file.seek(SeekFrom::Start(start))?;
        file.read_exact(&mut buf).with_context(|| format!("reading {}", path.display()))?;
        buf.extend_from_slice(&carry);
        let mut end = buf.len();
        while let Some(nl) = buf[..end].iter().rposition(|&b| b == b'\n') {
            if hand_over(&buf[nl + 1..end], &mut f).is_break() {
                return Ok(());
            }
            end = nl;
        }
        if end == buf.len() {
            // A line longer than a block: read more at once, so that a long line costs no more
            // than twice its length.
            block = block.saturating_mul(2);
        }
        buf.truncate(end);
        carry = buf;
        pos = start;
    }
    let _ = hand_over(&carry, &mut f);
    Ok(())
}

fn hand_over(line: &[u8], f: &mut impl FnMut(&str) -> ControlFlow<()>) -> ControlFlow<()> {
    if line.is_empty() {
        return ControlFlow::Continue(());
    }
    match std::str::from_utf8(line) {
        Ok(s) => f(s),
        Err(_) => f(&String::from_utf8_lossy(line)),
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

    /// The lines `lines_back` hands over for a file with this content, read in blocks of `block`
    /// bytes, stopping after `take` lines.
    fn back(name: &str, content: &[u8], block: u64, take: usize) -> Vec<String> {
        let path = std::env::temp_dir().join(format!("tokprof-logio-{}-{name}.jsonl", std::process::id()));
        std::fs::write(&path, content).unwrap();
        let mut out = Vec::new();
        lines_back(&path, block, |line| {
            out.push(line.to_string());
            if out.len() == take { ControlFlow::Break(()) } else { ControlFlow::Continue(()) }
        })
        .unwrap();
        std::fs::remove_file(&path).ok();
        out
    }

    #[test]
    fn lines_from_the_end() {
        assert_eq!(back("cross", b"a\nbb\nccc\n", 4, 99), ["ccc", "bb", "a"], "lines that cross block boundaries");
        assert_eq!(back("open", b"a\nb", 64, 99), ["b", "a"], "a last line without a newline");
        assert!(back("empty", b"", 4, 99).is_empty(), "an empty file");
        let long = format!("x\n{}\nz\n", "y".repeat(100));
        assert_eq!(back("long", long.as_bytes(), 8, 99), ["z".to_string(), "y".repeat(100), "x".to_string()], "a line longer than a block");
        assert_eq!(back("gaps", b"a\n\n\nb\n", 3, 99), ["b", "a"], "empty lines are skipped");
        assert_eq!(back("stop", b"a\nb\nc\n", 2, 1), ["c"], "it stops when told to");
        assert_eq!(back("utf8", "é\nя\n".as_bytes(), 2, 99), ["я", "é"], "a block may split a character");
    }

    #[test]
    fn a_growing_file_is_read_on_from_where_a_read_stopped() {
        let path = std::env::temp_dir().join(format!("tokprof-logio-{}-on.jsonl", std::process::id()));
        let read = |from: u64, check: &[u8]| {
            let mut lines = Vec::new();
            let on = for_each_line_from(&path, from, check, |l| lines.push(l.to_string())).unwrap();
            (lines, on)
        };
        std::fs::write(&path, "{\"a\":1}\n{\"b\":").unwrap();
        let (lines, on) = read(0, b"");
        let on = on.unwrap();
        assert_eq!(
            (lines, on.end, on.check.as_slice(), on.open),
            (vec!["{\"a\":1}\n".to_string()], 8, b"{\"a\":1}\n".as_slice(), false),
            "a line being written is left"
        );

        std::fs::write(&path, "{\"a\":1}\n{\"b\":2}\n{\"c\":3}").unwrap();
        let (lines, next) = read(on.end, &on.check);
        let next = next.unwrap();
        assert_eq!((lines, next.end, next.open), (vec!["{\"b\":2}\n".to_string()], 16, true), "a whole value without a newline is named");

        std::fs::write(&path, "{\"x\":1}\n{\"b\":2}\n{\"c\":3}\n").unwrap();
        assert_eq!(read(on.end, &on.check).1, None, "a file written anew");
        assert_eq!(read(99, b"").1.map(|r| r.end), Some(99), "nothing past the end");
        assert_eq!(for_each_line_from(Path::new("/x/rollout-1.jsonl.zst"), 0, b"", |_| {}).unwrap(), None, "compressed");

        let long = format!("{{\"y\":\"{}\"}}\n", "y".repeat(1000));
        std::fs::write(&path, &long).unwrap();
        let on = read(0, b"").1.unwrap();
        assert_eq!((on.end as usize, on.check.len()), (long.len(), CHECK), "the check keeps the last bytes");
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn compressed_logs_are_not_read_from_the_end() {
        let error = for_each_line_back(Path::new("/x/rollout-1.jsonl.zst"), |_| ControlFlow::Continue(())).unwrap_err();
        assert!(error.to_string().contains("compressed"), "{error}");
    }

    /// The lines a read hands over, and the error it ends with.
    fn read_all(read: impl FnOnce(&mut dyn FnMut(&str)) -> Result<()>) -> (Vec<String>, Option<String>) {
        let mut lines = Vec::new();
        let end = read(&mut |l| lines.push(l.to_string()));
        (lines, end.err().map(|e| format!("{e:#}")))
    }

    #[test]
    fn a_compressed_log_decompressed_ahead_gives_the_lines_and_the_error_a_plain_read_does() {
        // Lines of many sizes over several chunks, the last one without a newline.
        let mut text = String::new();
        for i in 0..40_000 {
            text.push_str(&format!("{{\"n\":{i},\"pad\":\"{}\"}}\n", "x".repeat(i % 300)));
        }
        text.push_str("{\"last\":true}");
        assert!(text.len() > 3 * CHUNK);
        let packed = ruzstd::encoding::compress_to_vec(text.as_bytes(), ruzstd::encoding::CompressionLevel::Fastest);
        let dir = std::env::temp_dir().join(format!("tokprof-logio-ahead-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let whole = dir.join("rollout-whole.jsonl.zst");
        let cut = dir.join("rollout-cut.jsonl.zst");
        std::fs::write(&whole, &packed).unwrap();
        std::fs::write(&cut, &packed[..packed.len() * 3 / 5]).unwrap();

        let ahead = read_all(|f| for_each_line(&whole, f));
        assert_eq!(ahead, read_all(|f| lines(open(&whole)?, f)));
        assert_eq!(ahead.0.len(), 40_001);
        assert_eq!(ahead.0.concat(), text);

        // A log cut short: the same lines before the same error.
        let ahead = read_all(|f| for_each_line(&cut, f));
        assert!(ahead.1.is_some() && !ahead.0.is_empty(), "{:?}", ahead.1);
        assert_eq!(ahead, read_all(|f| lines(open(&cut)?, f)));

        // A file that is no zstd frame fails as it opens.
        let junk = dir.join("rollout-junk.jsonl.zst");
        std::fs::write(&junk, b"not zstd").unwrap();
        assert_eq!(read_all(|f| for_each_line(&junk, f)), read_all(|f| lines(open(&junk)?, f)));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// Gives its bytes a few at a time, then panics, or gives them without end; it says when it
    /// is dropped.
    struct Feed {
        bytes: Vec<u8>,
        at: usize,
        endless: bool,
        dropped: std::sync::Arc<std::sync::atomic::AtomicBool>,
    }

    impl Read for Feed {
        fn read(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
            if self.endless {
                out.fill(b'\n');
                return Ok(out.len());
            }
            if self.at == self.bytes.len() {
                panic!("damaged frame");
            }
            let n = 3.min(self.bytes.len() - self.at).min(out.len());
            out[..n].copy_from_slice(&self.bytes[self.at..self.at + n]);
            self.at += n;
            Ok(n)
        }
    }

    impl Drop for Feed {
        fn drop(&mut self) {
            self.dropped.store(true, std::sync::atomic::Ordering::SeqCst);
        }
    }

    fn feed(bytes: &[u8], endless: bool) -> (Feed, std::sync::Arc<std::sync::atomic::AtomicBool>) {
        let dropped = std::sync::Arc::default();
        (Feed { bytes: bytes.to_vec(), at: 0, endless, dropped: std::sync::Arc::clone(&dropped) }, dropped)
    }

    #[test]
    fn a_decompression_that_panics_is_an_error_after_the_lines_before_it() {
        let (reader, _) = feed(b"{\"a\":1}\n{\"b\":2}\n{\"c\"", false);
        let (lines_read, error) = read_all(|f| lines(Ahead::spawn(reader), f));
        assert_eq!(lines_read, ["{\"a\":1}\n", "{\"b\":2}\n"]);
        assert_eq!(error.as_deref(), Some("the decompression stopped: damaged frame"));
    }

    #[test]
    fn an_ahead_dropped_early_leaves_no_thread_behind() {
        let (reader, dropped) = feed(b"", true);
        let mut ahead = Ahead::spawn(reader);
        assert!(!ahead.fill_buf().unwrap().is_empty());
        // The thread is filling a full channel without end; dropping the reader must not wait.
        let started = std::time::Instant::now();
        drop(ahead);
        assert!(started.elapsed() < std::time::Duration::from_secs(1));
        while !dropped.load(std::sync::atomic::Ordering::SeqCst) {
            assert!(started.elapsed() < std::time::Duration::from_secs(10), "the thread is still running");
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    }

    #[test]
    fn canonical_form_sorts_keys_and_keeps_unicode() {
        let v = json!({"role": "user", "content": [{"type": "text", "text": "привет \"x\"\n"}]});
        assert_eq!(canonical_string(&v), r#"{"content":[{"text":"привет \"x\"\n","type":"text"}],"role":"user"}"#);
    }
}
