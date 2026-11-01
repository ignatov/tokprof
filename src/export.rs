//! Profile writers, chosen by the output extension:
//! - `.html`: async-profiler's interactive flame graph;
//! - `.collapsed`: collapsed stacks with async-profiler frame-type suffixes, so `jfrconv`
//!   (including `jfrconv --diff`) keeps the colors;
//! - `.pb.gz`, `.pprof`, `.pb`: gzipped pprof;
//! - anything else: plain folded stacks for speedscope or flamegraph.pl.
//!
//! The HTML writer fills `assets/async-profiler/flame.html` from async-profiler 4.6
//! (Apache License 2.0, see `assets/async-profiler/LICENSE`) the way its converter does.

use std::collections::HashMap;
use std::fmt::Write as _;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use flate2::Compression;
use flate2::write::GzEncoder;
use prost::Message;

use crate::report::Values;

const FLAME_TEMPLATE: &str = include_str!("../assets/async-profiler/flame.html");

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Measure {
    Tokens,
    Cost,
    Avoidable,
}

impl Measure {
    /// Integer sample value: tokens, or micro-dollars for money.
    pub fn of(self, v: &Values) -> i64 {
        match self {
            Measure::Tokens => v.tokens.round() as i64,
            Measure::Cost => (v.usd * 1e6).round() as i64,
            Measure::Avoidable => (v.avoidable * 1e6).round() as i64,
        }
    }

    pub fn units(self) -> &'static str {
        match self {
            Measure::Tokens => "tokens",
            Measure::Cost | Measure::Avoidable => "µ$",
        }
    }
}

/// What a frame stands for, which decides its color in the flame graph.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Tint {
    /// Agent, project, session, chain, model, turn.
    Structure,
    /// Content kind, tool, target.
    Content,
    Read,
    Write,
    Uncached,
    Output,
    /// Re-written prefix and miss causes.
    Miss,
}

impl Tint {
    /// async-profiler frame type, an index into its palette: 0 pale green, 1 green, 2 aqua,
    /// 3 red, 5 orange, 6 yellow-green.
    fn frame_type(self) -> u8 {
        match self {
            Tint::Structure => 0,
            Tint::Content => 1,
            Tint::Read => 2,
            Tint::Miss => 3,
            Tint::Uncached | Tint::Output => 5,
            Tint::Write => 6,
        }
    }

    /// Suffix from which async-profiler's converter reads the same frame type. Red frames have
    /// no suffix; their names must not look like Java or C++ symbols.
    fn suffix(self) -> &'static str {
        match self.frame_type() {
            0 => "_[0]",
            1 => "_[j]",
            2 => "_[i]",
            5 => "_[k]",
            6 => "_[1]",
            _ => "",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Frame {
    pub name: String,
    pub tint: Tint,
    /// The dimension the frame comes from; JFR uses it as the class name.
    pub dim: &'static str,
}

pub type Stacks = HashMap<Vec<Frame>, Values>;

/// Where profiles go when no output path is given: `$TOKPROF_DIR`, or `~/tokprof`.
pub fn default_dir() -> PathBuf {
    std::env::var_os("TOKPROF_DIR")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| crate::adapters::home().join("tokprof"))
}

pub struct HtmlOptions<'a> {
    pub title: &'a str,
    pub highlight: Option<&'a str>,
}

/// Whether a file name ends with `.<ext>`, in any case.
pub fn has_ext(path: &Path, ext: &str) -> bool {
    let name = path.to_string_lossy();
    name.len() > ext.len() && name.is_char_boundary(name.len() - ext.len() - 1) && {
        let (head, tail) = name.split_at(name.len() - ext.len());
        head.ends_with('.') && tail.eq_ignore_ascii_case(ext)
    }
}

pub fn write(path: &Path, stacks: &Stacks, measure: Measure, html: &HtmlOptions) -> Result<usize> {
    let text = if has_ext(path, "pb.gz") || has_ext(path, "pprof") || has_ext(path, "pb") {
        return write_pprof(path, stacks, measure);
    } else if has_ext(path, "html") {
        flame_html(stacks, measure, html)
    } else {
        folded(stacks, measure, has_ext(path, "collapsed"))
    };
    std::fs::write(path, &text.0).with_context(|| format!("writing {}", path.display()))?;
    Ok(text.1)
}

/// Folded stacks, one `frame;frame;frame value` line per stack.
fn folded(stacks: &Stacks, measure: Measure, suffixes: bool) -> (String, usize) {
    let mut lines: Vec<(String, i64)> = stacks
        .iter()
        .map(|(stack, v)| {
            let frames: Vec<String> = stack
                .iter()
                .map(|f| if suffixes { format!("{}{}", f.name, f.tint.suffix()) } else { f.name.clone() })
                .collect();
            (frames.join(";"), measure.of(v))
        })
        .filter(|(k, v)| *v > 0 && !k.is_empty())
        .collect();
    lines.sort();
    let mut out = String::new();
    for (stack, value) in &lines {
        let _ = writeln!(out, "{stack} {value}");
    }
    (out, lines.len())
}

#[derive(Default)]
struct Node {
    total: i64,
    self_value: i64,
    children: HashMap<(usize, u8), Node>,
}

struct Cursor {
    level: usize,
    x: i64,
    total: i64,
}

/// Fills async-profiler's flame graph template like its `FlameGraph.dump`: a constant pool of
/// sorted, prefix-compressed frame names, then one `f`/`u`/`n` call per frame in depth-first
/// order with positions relative to the previous frame.
fn flame_html(stacks: &Stacks, measure: Measure, opts: &HtmlOptions) -> (String, usize) {
    let mut names: Vec<String> = vec![String::new()];
    let mut ids: HashMap<String, usize> = HashMap::new();
    let mut root = Node::default();
    let mut depth = 0;
    for (stack, v) in stacks {
        let value = measure.of(v);
        if value <= 0 || stack.is_empty() {
            continue;
        }
        depth = depth.max(stack.len());
        root.total += value;
        let mut node = &mut root;
        for frame in stack {
            let name = if frame.name.is_empty() { "(empty)" } else { frame.name.as_str() };
            let id = *ids.entry(name.to_string()).or_insert_with(|| {
                names.push(name.to_string());
                names.len() - 1
            });
            node = node.children.entry((id, frame.tint.frame_type())).or_default();
            node.total += value;
        }
        node.self_value += value;
    }

    let mut sorted: Vec<usize> = (0..names.len()).collect();
    sorted.sort_by(|&a, &b| names[a].cmp(&names[b]));
    let mut order = vec![0; names.len()];
    for (pos, &id) in sorted.iter().enumerate() {
        order[id] = pos;
    }
    let mut cpool = String::from("'all'");
    let mut prev = "";
    for &id in &sorted[1..] {
        let s = names[id].as_str();
        let shared = common_ascii_prefix(prev, s).min(95);
        let _ = write!(cpool, ",\n'{}'", js_escape(&format!("{}{}", char::from(b' ' + shared as u8), &s[shared..])));
        prev = s;
    }

    let mut frames = String::new();
    let mut cursor = Cursor { level: 0, x: 0, total: 0 };
    let count = print_frame(&root, 0, 3, 0, 0, &order, &mut cursor, &mut frames);

    let levels = depth + 1;
    let highlight = opts.highlight.map(|h| format!("'{}'", js_escape(h))).unwrap_or_default();
    let mut out = String::with_capacity(FLAME_TEMPLATE.len() + cpool.len() + frames.len());
    let mut tail = FLAME_TEMPLATE;
    for (placeholder, value) in [
        ("/*height:*/300", (levels * 16).min(32767).to_string()),
        ("/*title:*/", html_escape(opts.title)),
        ("/*inverted:*/false", "false".to_string()),
        ("/*treeview:*/false", "false".to_string()),
        ("/*units:*/", measure.units().to_string()),
        ("/*maxdiff:*/-1", "-1".to_string()),
        ("/*depth:*/0", levels.to_string()),
        ("/*cpool:*/", cpool),
        ("/*frames:*/", frames),
        ("/*highlight:*/", highlight),
    ] {
        let at = tail.find(placeholder).expect("flame.html has every placeholder, in this order");
        out.push_str(&tail[..at]);
        out.push_str(&value);
        tail = &tail[at + placeholder.len()..];
    }
    out.push_str(tail);
    (out, count)
}

#[allow(clippy::too_many_arguments)]
fn print_frame(
    node: &Node,
    name: usize,
    ty: u8,
    level: usize,
    x: i64,
    order: &[usize],
    cursor: &mut Cursor,
    out: &mut String,
) -> usize {
    let key = order[name] << 3 | ty as usize;
    if level == cursor.level + 1 && x == cursor.x {
        let _ = write!(out, "u({key}");
    } else if level == cursor.level && x == cursor.x + cursor.total {
        let _ = write!(out, "n({key}");
    } else {
        let _ = write!(out, "f({key},{level},{}", x - cursor.x);
    }
    if node.total != cursor.total {
        let _ = write!(out, ",{}", node.total);
    }
    out.push_str(")\n");
    *cursor = Cursor { level, x, total: node.total };

    let mut children: Vec<_> = node.children.iter().collect();
    children.sort_by_key(|((id, ty), _)| (order[*id], *ty));
    let mut count = 1;
    let mut x = x + node.self_value;
    for (&(id, ty), child) in children {
        count += print_frame(child, id, ty, level + 1, x, order, cursor, out);
        x += child.total;
    }
    count
}

/// Shared prefix length, stopping at the first non-ASCII character as async-profiler does.
fn common_ascii_prefix(a: &str, b: &str) -> usize {
    a.bytes().zip(b.bytes()).take_while(|(x, y)| x == y && x.is_ascii()).count()
}

fn js_escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('\'', "\\'").replace('\n', "\\n").replace('\r', "\\r")
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

// The subset of github.com/google/pprof/proto/profile.proto this writer needs.
#[derive(Clone, PartialEq, Message)]
struct Profile {
    #[prost(message, repeated, tag = "1")]
    sample_type: Vec<ValueType>,
    #[prost(message, repeated, tag = "2")]
    sample: Vec<Sample>,
    #[prost(message, repeated, tag = "4")]
    location: Vec<Location>,
    #[prost(message, repeated, tag = "5")]
    function: Vec<Function>,
    #[prost(string, repeated, tag = "6")]
    string_table: Vec<String>,
    #[prost(int64, tag = "14")]
    default_sample_type: i64,
}

#[derive(Clone, PartialEq, Message)]
struct ValueType {
    #[prost(int64, tag = "1")]
    r#type: i64,
    #[prost(int64, tag = "2")]
    unit: i64,
}

#[derive(Clone, PartialEq, Message)]
struct Sample {
    #[prost(uint64, repeated, tag = "1")]
    location_id: Vec<u64>,
    #[prost(int64, repeated, tag = "2")]
    value: Vec<i64>,
}

#[derive(Clone, PartialEq, Message)]
struct Location {
    #[prost(uint64, tag = "1")]
    id: u64,
    #[prost(message, repeated, tag = "4")]
    line: Vec<Line>,
}

#[derive(Clone, PartialEq, Message)]
struct Line {
    #[prost(uint64, tag = "1")]
    function_id: u64,
}

#[derive(Clone, PartialEq, Message)]
struct Function {
    #[prost(uint64, tag = "1")]
    id: u64,
    #[prost(int64, tag = "2")]
    name: i64,
    #[prost(int64, tag = "3")]
    system_name: i64,
}

/// All three measures go in as sample types, so pprof can switch between them; the chosen one
/// is the default.
fn write_pprof(path: &Path, stacks: &Stacks, measure: Measure) -> Result<usize> {
    let mut strings: Vec<String> = vec![String::new()];
    let mut string_ids: HashMap<String, i64> = HashMap::new();
    let mut intern = |s: &str, strings: &mut Vec<String>| -> i64 {
        if let Some(&id) = string_ids.get(s) {
            return id;
        }
        strings.push(s.to_string());
        let id = strings.len() as i64 - 1;
        string_ids.insert(s.to_string(), id);
        id
    };
    let measures = [Measure::Tokens, Measure::Cost, Measure::Avoidable];
    let sample_type = [("tokens", "count"), ("cost", "microdollars"), ("avoidable", "microdollars")]
        .iter()
        .map(|(t, u)| ValueType { r#type: intern(t, &mut strings), unit: intern(u, &mut strings) })
        .collect();

    let mut functions: Vec<Function> = Vec::new();
    let mut function_ids: HashMap<String, u64> = HashMap::new();
    let mut samples = Vec::new();
    for (stack, v) in stacks {
        let values: Vec<i64> = measures.iter().map(|m| m.of(v)).collect();
        if values.iter().all(|&x| x <= 0) || stack.is_empty() {
            continue;
        }
        let mut location_id = Vec::with_capacity(stack.len());
        for frame in stack.iter().rev() {
            let id = match function_ids.get(&frame.name) {
                Some(&id) => id,
                None => {
                    let id = functions.len() as u64 + 1;
                    let name = intern(&frame.name, &mut strings);
                    functions.push(Function { id, name, system_name: name });
                    function_ids.insert(frame.name.clone(), id);
                    id
                }
            };
            location_id.push(id);
        }
        samples.push(Sample { location_id, value: values });
    }
    let location = functions.iter().map(|f| Location { id: f.id, line: vec![Line { function_id: f.id }] }).collect();
    let default_sample_type = intern(
        match measure {
            Measure::Tokens => "tokens",
            Measure::Cost => "cost",
            Measure::Avoidable => "avoidable",
        },
        &mut strings,
    );
    let n = samples.len();
    let profile = Profile { sample_type, sample: samples, location, function: functions, string_table: strings, default_sample_type };

    let file = File::create(path).with_context(|| format!("creating {}", path.display()))?;
    let mut gz = GzEncoder::new(BufWriter::new(file), Compression::default());
    gz.write_all(&profile.encode_to_vec())?;
    gz.finish()?.flush()?;
    Ok(n)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stacks() -> Stacks {
        let f = |name: &str, tint| Frame { name: name.to_string(), tint, dim: "test" };
        let mut s = Stacks::new();
        let v = |usd: f64| Values { tokens: usd * 1000.0, usd, avoidable: 0.0 };
        s.insert(vec![f("claude", Tint::Structure), f("Read", Tint::Content), f("cache read", Tint::Read)], v(2.0));
        s.insert(vec![f("claude", Tint::Structure), f("Read", Tint::Content), f("rewrite after miss", Tint::Miss)], v(1.0));
        s.insert(vec![f("claude", Tint::Structure), f("Bash 'x'", Tint::Content)], v(0.5));
        s
    }

    #[test]
    fn extensions_match_in_any_case() {
        assert!(has_ext(Path::new("x.HTML"), "html") && has_ext(Path::new("x.JFR"), "jfr") && has_ext(Path::new("/t/x.PB.GZ"), "pb.gz"));
        assert!(!has_ext(Path::new("xhtml"), "html") && !has_ext(Path::new("html"), "html") && !has_ext(Path::new("x.pb.gz.txt"), "pb.gz"));
    }

    #[test]
    fn collapsed_carries_frame_types() {
        let (text, n) = folded(&stacks(), Measure::Cost, true);
        assert_eq!(n, 3);
        assert!(text.contains("claude_[0];Read_[j];cache read_[i] 2000000\n"), "{text}");
        assert!(text.contains("claude_[0];Read_[j];rewrite after miss 1000000\n"), "{text}");
    }

    #[test]
    fn html_lists_every_frame_once() {
        let opts = HtmlOptions { title: "t <1>", highlight: Some("miss") };
        let (html, frames) = flame_html(&stacks(), Measure::Cost, &opts);
        // root, claude, Read, cache read, rewrite after miss, Bash 'x'
        assert_eq!(frames, 6);
        // Sorted, each name stored as (shared prefix length + ' ') and the rest.
        assert!(html.contains("const cpool = [\n'all',\n' Bash \\'x\\'',\n' Read',\n' cache read',\n'!laude',\n' rewrite after miss'\n];"), "{html}");
        assert!(html.contains("<h1>t &lt;1&gt;</h1>"));
        assert!(html.contains("const units = 'µ$';"));
        assert!(html.contains("search('miss');"));
        // root (n), then claude as its first child (u), pale green: key = order << 3 | 0
        assert!(html.contains("n(3,3500000)\nu(32)\n"), "{html}");
    }
}
