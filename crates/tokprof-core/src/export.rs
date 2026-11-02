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
use std::path::Path;

use anyhow::{Context, Result};

use crate::profile::{Measure, Stacks};

#[cfg(feature = "export")]
mod pprof;

const FLAME_TEMPLATE: &str = include_str!("../assets/async-profiler/flame.html");

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
        #[cfg(feature = "export")]
        return pprof::write(path, stacks, measure);
        #[cfg(not(feature = "export"))]
        anyhow::bail!("{}: this build writes no pprof profiles, as tokprof-core was built without its `export` feature", path.display());
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile::{Frame, Tint, Values};

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
