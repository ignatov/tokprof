//! JDK Flight Recorder output, for IntelliJ's profiler, JDK Mission Control, `jfr` and
//! async-profiler's `jfrconv`. The layout follows async-profiler 4.6's writer
//! (src/flightRecorder.cpp, src/jfrMetadata.cpp): one chunk with the metadata after the header,
//! then the events, then one constant pool.
//!
//! The profile becomes CPU samples (`jdk.ExecutionSample`). Each frame is a Java method whose
//! class is the dimension and whose name is the value (`tool.Read`, `miss.idle > TTL`), each
//! group of the `--threads` dimensions is a thread, and a sample stands for a round amount of
//! the measure. Remainders below one sample are rounded stochastically, so totals hold. A
//! sample is timed when its content entered the context.

use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::Path;

use anyhow::{Context, Result};

use crate::export::{Frame, Measure, Tint};
use crate::report::Values;

/// (thread, stack, minute start in epoch ms).
pub type TimedKey = (String, Vec<Frame>, i64);
pub type TimedStacks = HashMap<TimedKey, Values>;

pub struct Written {
    pub samples: u64,
    /// Measure units per sample: tokens, or micro-dollars.
    pub quantum: i64,
    pub threads: usize,
    pub stack_traces: usize,
}

const T_METADATA: u64 = 0;
const T_CPOOL: u64 = 1;
const T_BOOLEAN: u64 = 4;
const T_INT: u64 = 10;
const T_LONG: u64 = 11;
const T_STRING: u64 = 20;
const T_CLASS: u64 = 21;
const T_THREAD: u64 = 22;
const T_CLASS_LOADER: u64 = 23;
const T_FRAME_TYPE: u64 = 24;
const T_THREAD_STATE: u64 = 25;
const T_STACK_TRACE: u64 = 26;
const T_STACK_FRAME: u64 = 27;
const T_METHOD: u64 = 28;
const T_PACKAGE: u64 = 30;
const T_SYMBOL: u64 = 31;
const T_EXECUTION_SAMPLE: u64 = 101;
const T_LABEL: u64 = 201;
const T_CATEGORY: u64 = 202;
const T_CONTENT_TYPE: u64 = 203;
const T_TIMESTAMP: u64 = 204;

/// async-profiler marks CPU samples with STATE_DEFAULT; RUNNABLE and SLEEPING mean wall-clock
/// samples, which `jfrconv --cpu` leaves out.
const STATE_CPU: u64 = 0;
const TICKS_PER_SECOND: u64 = 1_000_000_000;
const MINUTE_NS: u64 = 60 * TICKS_PER_SECOND;
const MAX_SAMPLES: i64 = 2_000_000;

pub fn write(path: &Path, stacks: &TimedStacks, measure: Measure) -> Result<Written> {
    let mut entries: Vec<(&TimedKey, i64)> =
        stacks.iter().map(|(k, v)| (k, measure.of(v))).filter(|(_, v)| *v > 0).collect();
    entries.sort_by(|a, b| (a.0.2, &a.0.0, &a.0.1).cmp(&(b.0.2, &b.0.0, &b.0.1)));
    let total: i64 = entries.iter().map(|(_, v)| v).sum();
    let mut quantum = if measure == Measure::Tokens { 100 } else { 1_000 };
    while total / quantum > MAX_SAMPLES {
        quantum *= 10;
    }
    let start_ms = entries.first().map_or(0, |(k, _)| k.2);
    let end_ms = entries.last().map_or(0, |(k, _)| k.2) + 60_000;

    let mut pools = Pools::default();
    let mut events = Buf::default();
    let mut samples = 0u64;
    for ((thread, stack, minute), value) in entries {
        let n = value / quantum + i64::from(unit_hash((thread, stack, minute)) < (value % quantum) as f64 / quantum as f64);
        if n == 0 || stack.is_empty() {
            continue;
        }
        let thread_id = pools.thread(thread);
        let trace_id = pools.stack_trace(stack);
        let base = (*minute - start_ms) as u64 * 1_000_000;
        for i in 0..n as u64 {
            let mut e = Buf::default();
            e.var(T_EXECUTION_SAMPLE);
            e.var(base + i * MINUTE_NS / n as u64);
            e.var(thread_id);
            e.var(trace_id);
            e.var(STATE_CPU);
            events.event(&e);
        }
        samples += n as u64;
    }

    let mut out = Buf::default();
    out.0.extend_from_slice(b"FLR\0");
    out.be(2, 2);
    out.be(0, 2);
    let chunk_size_at = out.0.len();
    out.be(0, 8); // chunk size
    out.be(0, 8); // constant pool offset
    out.be(68, 8); // metadata offset
    out.be(start_ms as u64 * 1_000_000, 8);
    out.be((end_ms - start_ms) as u64 * 1_000_000, 8);
    out.be(0, 8); // start ticks
    out.be(TICKS_PER_SECOND, 8);
    out.be(1, 4); // features: compressed integers
    write_metadata(&mut out);
    out.0.extend_from_slice(&events.0);
    let cpool_offset = out.0.len() as u64;
    let (threads, stack_traces) = (pools.threads.len(), pools.traces.len());
    pools.write(&mut out);
    let size = out.0.len() as u64;
    out.0[chunk_size_at..chunk_size_at + 8].copy_from_slice(&size.to_be_bytes());
    out.0[chunk_size_at + 8..chunk_size_at + 16].copy_from_slice(&cpool_offset.to_be_bytes());

    std::fs::write(path, &out.0).with_context(|| format!("writing {}", path.display()))?;
    Ok(Written { samples, quantum, threads, stack_traces })
}

/// Deterministic value in [0, 1) for rounding a remainder up or down.
fn unit_hash(key: impl Hash) -> f64 {
    let mut h = DefaultHasher::new();
    key.hash(&mut h);
    (h.finish() >> 11) as f64 / (1u64 << 53) as f64
}

#[derive(Default)]
struct Buf(Vec<u8>);

impl Buf {
    /// LEB128, which is JFR's compressed integer for values below 2^56.
    fn var(&mut self, mut v: u64) {
        debug_assert!(v < 1 << 56);
        loop {
            let b = (v & 0x7f) as u8;
            v >>= 7;
            if v == 0 {
                self.0.push(b);
                return;
            }
            self.0.push(b | 0x80);
        }
    }

    fn utf8(&mut self, s: &str) {
        self.0.push(3);
        self.var(s.len() as u64);
        self.0.extend_from_slice(s.as_bytes());
    }

    fn be(&mut self, v: u64, bytes: usize) {
        self.0.extend_from_slice(&v.to_be_bytes()[8 - bytes..]);
    }

    /// Appends an event: its size, which counts itself, then the body.
    fn event(&mut self, body: &Buf) {
        let mut width = 1;
        while body.0.len() + width >= 1 << (7 * width) {
            width += 1;
        }
        self.var((body.0.len() + width) as u64);
        self.0.extend_from_slice(&body.0);
    }

    /// Reserves a 5-byte size field, for events whose size is known only at the end.
    fn reserve_size(&mut self) -> usize {
        self.0.extend_from_slice(&[0; 5]);
        self.0.len() - 5
    }

    fn patch_size(&mut self, at: usize) {
        let v = (self.0.len() - at) as u32;
        for i in 0..5 {
            let b = ((v >> (7 * i)) & 0x7f) as u8;
            self.0[at + i] = if i < 4 { b | 0x80 } else { b };
        }
    }
}

#[derive(Default)]
struct Pools<'a> {
    threads: HashMap<&'a str, u64>,
    traces: HashMap<&'a [Frame], u64>,
    methods: HashMap<(&'a str, &'a str, u8), u64>,
    symbols: HashMap<String, u64>,
}

impl<'a> Pools<'a> {
    fn thread(&mut self, name: &'a str) -> u64 {
        let next = self.threads.len() as u64 + 1;
        *self.threads.entry(name).or_insert(next)
    }

    fn stack_trace(&mut self, stack: &'a [Frame]) -> u64 {
        if let Some(&id) = self.traces.get(stack) {
            return id;
        }
        for f in stack {
            let key = (f.dim, method_name(f), frame_type(f.tint));
            let next = self.methods.len() as u64 + 1;
            self.methods.entry(key).or_insert(next);
        }
        let id = self.traces.len() as u64 + 1;
        self.traces.insert(stack, id);
        id
    }

    fn symbol(&mut self, s: &str) -> u64 {
        let next = self.symbols.len() as u64 + 1;
        *self.symbols.entry(s.to_string()).or_insert(next)
    }

    /// One constant pool event with every pool the samples refer to.
    fn write(mut self, out: &mut Buf) {
        let at = out.reserve_size();
        out.var(T_CPOOL);
        out.var(0); // start ticks
        out.var(0); // duration
        out.var(0); // delta to the previous constant pool
        out.var(1); // flush
        out.var(7); // pools

        out.var(T_FRAME_TYPE);
        out.var(7);
        for (i, name) in ["Interpreted", "JIT compiled", "Inlined", "Native", "C++", "Kernel", "C1 compiled"].iter().enumerate() {
            out.var(i as u64);
            out.utf8(name);
        }
        out.var(T_THREAD_STATE);
        out.var(3);
        for (i, name) in ["STATE_DEFAULT", "STATE_RUNNABLE", "STATE_SLEEPING"].iter().enumerate() {
            out.var(i as u64);
            out.utf8(name);
        }

        let mut threads: Vec<_> = self.threads.iter().map(|(n, id)| (*id, *n)).collect();
        threads.sort_unstable();
        out.var(T_THREAD);
        out.var(threads.len() as u64);
        for (id, name) in threads {
            out.var(id);
            out.utf8(name); // osName
            out.var(id); // osThreadId
            out.utf8(name); // javaName
            out.var(id); // javaThreadId
        }

        let mut traces: Vec<_> = self.traces.iter().map(|(s, id)| (*id, *s)).collect();
        traces.sort_unstable_by_key(|(id, _)| *id);
        out.var(T_STACK_TRACE);
        out.var(traces.len() as u64);
        for (id, stack) in traces {
            out.var(id);
            out.var(0); // truncated
            out.var(stack.len() as u64);
            // JFR lists frames from the top of the stack.
            for f in stack.iter().rev() {
                let ty = frame_type(f.tint);
                out.var(self.methods[&(f.dim, method_name(f), ty)]);
                out.var(0); // line number
                out.var(0); // bytecode index
                out.var(u64::from(ty));
            }
        }

        // One class per dimension, so a frame reads `dimension.value`.
        let mut methods: Vec<_> = self.methods.iter().map(|(k, id)| (*id, *k)).collect();
        methods.sort_unstable_by_key(|(id, _)| *id);
        let mut classes: Vec<&str> = methods.iter().map(|(_, (dim, _, _))| *dim).collect();
        classes.sort_unstable();
        classes.dedup();
        let class_id = |dim: &str| classes.binary_search(&dim).map_or(0, |i| i as u64 + 1);
        let descriptor = self.symbol("()V");
        let mut method_rows = Vec::with_capacity(methods.len());
        for (id, (dim, name, _)) in &methods {
            method_rows.push((*id, class_id(dim), self.symbol(name)));
        }
        out.var(T_METHOD);
        out.var(method_rows.len() as u64);
        for (id, class, name) in method_rows {
            out.var(id);
            out.var(class);
            out.var(name);
            out.var(descriptor);
            out.var(1); // public
            out.var(0); // hidden
        }
        let class_rows: Vec<(u64, u64)> = classes.iter().enumerate().map(|(i, c)| (i as u64 + 1, self.symbol(c))).collect();
        out.var(T_CLASS);
        out.var(class_rows.len() as u64);
        for (id, name) in class_rows {
            out.var(id);
            out.var(0); // class loader
            out.var(name);
            out.var(0); // package
            out.var(1); // public
        }

        let mut symbols: Vec<_> = self.symbols.iter().map(|(s, id)| (*id, s.as_str())).collect();
        symbols.sort_unstable();
        out.var(T_SYMBOL);
        out.var(symbols.len() as u64);
        for (id, s) in symbols {
            out.var(id);
            out.utf8(s);
        }
        out.patch_size(at);
    }
}

/// The value part of a frame: `turn 12` reads `turn.12`, `miss: idle > TTL` reads
/// `miss.idle > TTL`.
fn method_name(f: &Frame) -> &str {
    f.name
        .strip_prefix(f.dim)
        .and_then(|rest| rest.strip_prefix(": ").or_else(|| rest.strip_prefix(' ')))
        .filter(|rest| !rest.is_empty())
        .unwrap_or(&f.name)
}

/// Java frame types only, so every viewer shows the frames; the type picks the color in
/// async-profiler's flame graph after `jfrconv`.
fn frame_type(tint: Tint) -> u8 {
    match tint {
        Tint::Structure => 0,
        Tint::Read => 2,
        Tint::Write => 6,
        _ => 1,
    }
}

struct Element {
    name: u64,
    attributes: Vec<(u64, u64)>,
    children: Vec<Element>,
}

#[derive(Default)]
struct Strings {
    list: Vec<String>,
    index: HashMap<String, u64>,
}

impl Strings {
    fn id(&mut self, s: &str) -> u64 {
        if let Some(&i) = self.index.get(s) {
            return i;
        }
        self.list.push(s.to_string());
        let i = self.list.len() as u64 - 1;
        self.index.insert(s.to_string(), i);
        i
    }

    fn element(&mut self, name: &str, attributes: &[(&str, &str)], children: Vec<Element>) -> Element {
        let name = self.id(name);
        let attributes = attributes.iter().map(|(k, v)| (self.id(k), self.id(v))).collect();
        Element { name, attributes, children }
    }

    fn annotation(&mut self, class: u64, values: &[(&str, &str)]) -> Element {
        let class = class.to_string();
        let mut attributes = vec![("class", class.as_str())];
        attributes.extend_from_slice(values);
        self.element("annotation", &attributes, Vec::new())
    }

    fn class(&mut self, name: &str, id: u64, label: Option<&str>, simple: bool, mut members: Vec<Element>) -> Element {
        let id_text = id.to_string();
        let mut attributes = vec![("name", name), ("id", id_text.as_str())];
        if simple {
            attributes.push(("simpleType", "true"));
        } else if id > 200 {
            attributes.push(("superType", "java.lang.annotation.Annotation"));
        } else if id > 100 {
            attributes.push(("superType", "jdk.jfr.Event"));
        }
        let mut children = Vec::new();
        if let Some(label) = label {
            children.push(self.annotation(T_LABEL, &[("value", label)]));
        }
        if id == T_TIMESTAMP {
            children.push(self.annotation(T_CONTENT_TYPE, &[]));
        }
        children.append(&mut members);
        self.element("class", &attributes, children)
    }

    fn field(&mut self, name: &str, class: u64, label: Option<&str>, constant_pool: bool, array: bool) -> Element {
        let class = class.to_string();
        let mut attributes = vec![("name", name), ("class", class.as_str())];
        if constant_pool {
            attributes.push(("constantPool", "true"));
        }
        if array {
            attributes.push(("dimension", "1"));
        }
        let children = label.map(|l| vec![self.annotation(T_LABEL, &[("value", l)])]).unwrap_or_default();
        self.element("field", &attributes, children)
    }
}

/// The type system: primitives, the stack trace types and `jdk.ExecutionSample`, as declared by
/// async-profiler.
fn write_metadata(out: &mut Buf) {
    let mut s = Strings::default();
    let mut classes = Vec::new();
    for (name, id) in [
        ("boolean", T_BOOLEAN),
        ("char", 5),
        ("float", 6),
        ("double", 7),
        ("byte", 8),
        ("short", 9),
        ("int", T_INT),
        ("long", T_LONG),
        ("java.lang.String", T_STRING),
    ] {
        classes.push(s.class(name, id, None, false, Vec::new()));
    }
    let fields = vec![
        s.field("classLoader", T_CLASS_LOADER, Some("Class Loader"), true, false),
        s.field("name", T_SYMBOL, Some("Name"), true, false),
        s.field("package", T_PACKAGE, Some("Package"), true, false),
        s.field("modifiers", T_INT, Some("Access Modifiers"), false, false),
    ];
    classes.push(s.class("java.lang.Class", T_CLASS, Some("Java Class"), false, fields));
    let fields = vec![
        s.field("osName", T_STRING, Some("OS Thread Name"), false, false),
        s.field("osThreadId", T_LONG, Some("OS Thread Id"), false, false),
        s.field("javaName", T_STRING, Some("Java Thread Name"), false, false),
        s.field("javaThreadId", T_LONG, Some("Java Thread Id"), false, false),
    ];
    classes.push(s.class("java.lang.Thread", T_THREAD, Some("Thread"), false, fields));
    let fields = vec![s.field("type", T_CLASS, Some("Type"), true, false), s.field("name", T_SYMBOL, Some("Name"), true, false)];
    classes.push(s.class("jdk.types.ClassLoader", T_CLASS_LOADER, Some("Java Class Loader"), false, fields));
    let fields = vec![s.field("description", T_STRING, Some("Description"), false, false)];
    classes.push(s.class("jdk.types.FrameType", T_FRAME_TYPE, Some("Frame type"), true, fields));
    let fields = vec![s.field("name", T_STRING, Some("Name"), false, false)];
    classes.push(s.class("jdk.types.ThreadState", T_THREAD_STATE, Some("Java Thread State"), true, fields));
    let fields = vec![
        s.field("truncated", T_BOOLEAN, Some("Truncated"), false, false),
        s.field("frames", T_STACK_FRAME, Some("Stack Frames"), false, true),
    ];
    classes.push(s.class("jdk.types.StackTrace", T_STACK_TRACE, Some("Stacktrace"), false, fields));
    let fields = vec![
        s.field("method", T_METHOD, Some("Java Method"), true, false),
        s.field("lineNumber", T_INT, Some("Line Number"), false, false),
        s.field("bytecodeIndex", T_INT, Some("Bytecode Index"), false, false),
        s.field("type", T_FRAME_TYPE, Some("Frame Type"), true, false),
    ];
    classes.push(s.class("jdk.types.StackFrame", T_STACK_FRAME, None, false, fields));
    let fields = vec![
        s.field("type", T_CLASS, Some("Type"), true, false),
        s.field("name", T_SYMBOL, Some("Name"), true, false),
        s.field("descriptor", T_SYMBOL, Some("Descriptor"), true, false),
        s.field("modifiers", T_INT, Some("Access Modifiers"), false, false),
        s.field("hidden", T_BOOLEAN, Some("Hidden"), false, false),
    ];
    classes.push(s.class("jdk.types.Method", T_METHOD, Some("Java Method"), false, fields));
    let fields = vec![s.field("name", T_SYMBOL, Some("Name"), true, false)];
    classes.push(s.class("jdk.types.Package", T_PACKAGE, Some("Package"), false, fields));
    let fields = vec![s.field("string", T_STRING, Some("String"), false, false)];
    classes.push(s.class("jdk.types.Symbol", T_SYMBOL, Some("Symbol"), true, fields));

    let category = s.annotation(T_CATEGORY, &[("value-0", "Java Virtual Machine"), ("value-1", "Profiling")]);
    let mut start_time = s.field("startTime", T_LONG, Some("Start Time"), false, false);
    start_time.children.push(s.annotation(T_TIMESTAMP, &[("value", "TICKS")]));
    let fields = vec![
        category,
        start_time,
        s.field("sampledThread", T_THREAD, Some("Thread"), true, false),
        s.field("stackTrace", T_STACK_TRACE, Some("Stack Trace"), true, false),
        s.field("state", T_THREAD_STATE, Some("Thread State"), true, false),
    ];
    classes.push(s.class("jdk.ExecutionSample", T_EXECUTION_SAMPLE, Some("Method Profiling Sample"), false, fields));

    let fields = vec![s.field("value", T_STRING, None, false, false)];
    classes.push(s.class("jdk.jfr.Label", T_LABEL, None, false, fields));
    let fields = vec![s.field("value", T_STRING, None, false, true)];
    classes.push(s.class("jdk.jfr.Category", T_CATEGORY, None, false, fields));
    classes.push(s.class("jdk.jfr.ContentType", T_CONTENT_TYPE, Some("Content Type"), false, Vec::new()));
    let fields = vec![s.field("value", T_STRING, None, false, false)];
    classes.push(s.class("jdk.jfr.Timestamp", T_TIMESTAMP, Some("Timestamp"), false, fields));

    let metadata = s.element("metadata", &[], classes);
    let region = s.element("region", &[("locale", "en_US"), ("gmtOffset", "0")], Vec::new());
    let root = s.element("root", &[], vec![metadata, region]);

    let at = out.reserve_size();
    out.var(T_METADATA);
    out.var(0); // start ticks
    out.var(0); // duration
    out.var(0x7fff_ffff); // metadata id; must not clash with the JDK's, or `jfr print` breaks
    out.var(s.list.len() as u64);
    for string in &s.list {
        out.utf8(string);
    }
    write_element(out, &root);
    out.patch_size(at);
}

fn write_element(out: &mut Buf, e: &Element) {
    out.var(e.name);
    out.var(e.attributes.len() as u64);
    for &(k, v) in &e.attributes {
        out.var(k);
        out.var(v);
    }
    out.var(e.children.len() as u64);
    for child in &e.children {
        write_element(out, child);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_size_counts_itself() {
        let mut out = Buf::default();
        out.event(&Buf(vec![7; 126]));
        // 126 bytes and a one-byte size make 127, which still fits in one byte.
        assert_eq!((out.0[0], out.0.len()), (127, 127));
        let mut out = Buf::default();
        out.event(&Buf(vec![7; 127]));
        // 127 + 1 = 128 does not fit; with a two-byte size the event is 129 bytes.
        assert_eq!((&out.0[..2], out.0.len()), (&[0x81, 0x01][..], 129));
    }

    #[test]
    fn method_names_drop_the_dimension() {
        let f = |name: &str, dim| Frame { name: name.to_string(), tint: Tint::Structure, dim };
        assert_eq!(method_name(&f("turn 12", "turn")), "12");
        assert_eq!(method_name(&f("miss: idle > TTL", "miss")), "idle > TTL");
        assert_eq!(method_name(&f("Read", "tool")), "Read");
    }
}
