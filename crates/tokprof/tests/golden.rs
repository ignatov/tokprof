//! What the commands print over the log sources' fixture logs, compared with the expected text in
//! `tests/golden/<case>.txt`: the command line, its stdout, its stderr and, for `flame`, the file
//! it wrote. Each case runs the binary on a home of its own (see `common`), over the fixtures of
//! Claude Code, Codex, pi and OpenCode, and with `--prices tests/golden/prices.toml`, which prices every
//! model the fixtures name, so that a change to the built-in prices changes no golden.
//!
//! After a change of the printed text, write the files again and review their diff:
//!
//! ```sh
//! TOKPROF_BLESS=1 cargo test -p tokprof --test golden
//! ```
//!
//! Only output that depends neither on the time the test runs nor on the machine's time zone has
//! a golden: the runs give no window ending at "now", and the reports that print local times
//! (`scan`, `sessions`, `day`, `calendar`, `month`, the list of `misses`, the rhythm of `pauses`)
//! are left out or cut before them. A Windows binary ignores TZ, so these must hold in any zone.
//! `year` runs on a synthetic day store of a year that is over, with no log, no agent time and
//! no action, its requests at noon UTC: it prints dates but no time of day.
//! Printed paths start with `<home>` or `<repo>` and use `/`, and the read line's timing is
//! `<time>`, so the files are the same on every platform.

mod common;

use std::path::{Path, PathBuf};

/// What of a run the golden holds besides stdout and stderr.
enum View {
    /// Stdout whole.
    Out,
    /// Stdout up to the line that starts with this text: what follows prints local times.
    Before(&'static str),
    /// The price table's layers and the rows of the test's price file, with runs of spaces as one
    /// space: the built-in rows and the widths of the columns change with the built-in prices.
    Prices,
    /// Stdout, and the file of this name that the command wrote in the home.
    File(&'static str),
    /// Stdout whole, of a run on no log with the synthetic day store of `year_store`.
    Store,
}

macro_rules! goldens {
    ($($name:ident: [$($arg:literal),*] $view:expr;)*) => {
        $(
            #[test]
            fn $name() {
                check(stringify!($name), &[$($arg),*], $view);
            }
        )*
        const CASES: &[&str] = &[$(stringify!($name)),*];
    };
}

// `<home>` in an argument is the run's home.
goldens! {
    summary: ["summary"] View::Out;
    summary_window: ["--since", "2026-10-29", "--until", "2026-11-03", "--agent", "claude,pi", "summary"] View::Out;
    summary_project: ["--project", "app", "summary"] View::Out;
    misses: ["misses", "--top", "0", "--expected"] View::Out;
    misses_pi: ["--agent", "pi", "misses", "--top", "0"] View::Out;
    tools: ["tools"] View::Out;
    tools_trips: ["tools", "--by", "trips"] View::Out;
    tools_failed: ["tools", "--by", "failed"] View::Out;
    whatif: ["whatif"] View::Out;
    bench: ["bench"] View::Out;
    bench_small: ["bench", "--min-context", "1000", "--keep-warm", "90m", "--requests", "5"] View::Out;
    pauses: ["--since", "2026-10-29", "--until", "2026-11-03", "pauses"] View::Before("Your rhythm:");
    prices: ["prices"] View::Prices;
    handoff: ["handoff", "5e5510f0"] View::Out;
    flame: ["flame", "-o", "<home>/f.folded"] View::File("f.folded");
    flame_misses: ["flame", "--value", "avoidable", "--stack", "agent,model,cause", "-o", "<home>/m.collapsed"] View::File("m.collapsed");
    year: ["year", "2025"] View::Store;
}

const PRICES: &str = "tests/golden/prices.toml";

fn golden_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests").join("golden")
}

fn bless() -> bool {
    std::env::var_os("TOKPROF_BLESS").is_some_and(|v| !v.is_empty() && v != "0")
}

fn check(name: &str, args: &[&str], view: View) {
    let home = common::temp_home(&format!("golden-{name}"));
    let shown = home.to_string_lossy().to_string();
    let args: Vec<String> = args.iter().map(|a| a.replace("<home>", &shown)).collect();
    let prices = Path::new(env!("CARGO_MANIFEST_DIR")).join(PRICES);
    let mut command = if matches!(view, View::Store) {
        year_store(&home);
        common::tokprof(&home)
    } else {
        common::tokprof_fixtures(&home, true)
    };
    let out = command.arg("--prices").arg(&prices).args(&args).output().unwrap();
    let paths = |text: &str| normalize(text, &home);
    let (stdout, stderr) = (paths(&String::from_utf8_lossy(&out.stdout)), paths(&String::from_utf8_lossy(&out.stderr)));
    assert!(out.status.success(), "tokprof {} failed with {}:\n{stdout}{stderr}", args.join(" "), out.status);
    let stdout = match view {
        View::Out | View::File(_) | View::Store => stdout,
        View::Before(start) => {
            let at = stdout.find(&format!("\n{start}")).unwrap_or_else(|| panic!("no line starts with {start:?}:\n{stdout}"));
            stdout[..at + 1].to_string()
        }
        View::Prices => test_prices(&stdout),
    };
    let mut text = format!("$ tokprof --prices {PRICES} {}\n", paths(&args.join(" ")));
    text.push_str(&format!("--- stdout\n{stdout}"));
    text.push_str(&format!("--- stderr\n{}", timing(&stderr)));
    if let View::File(file) = view {
        let written = std::fs::read_to_string(home.join(file)).unwrap_or_else(|e| panic!("{file}: {e}"));
        text.push_str(&format!("--- {file}\n{}", paths(&written)));
    }
    std::fs::remove_dir_all(&home).unwrap();
    compare(name, &text);
}

/// The home as `<home>` and the repository as `<repo>`, and `/` in the paths that start with
/// them or with `~`, as a Windows binary prints them with `\`.
fn normalize(text: &str, home: &Path) -> String {
    let text = text.replace("\r\n", "\n").replace(&*home.to_string_lossy(), "<home>").replace(&*common::root().to_string_lossy(), "<repo>");
    let lines = text.split('\n').map(|line| {
        line.split(' ')
            .map(|w| if w.starts_with("<home>") || w.starts_with("<repo>") || w.starts_with('~') { w.replace('\\', "/") } else { w.to_string() })
            .collect::<Vec<_>>()
            .join(" ")
    });
    lines.collect::<Vec<_>>().join("\n")
}

/// The read line's ` in 0.1s` as ` in <time>`, at its end or before the store's `;`.
fn timing(stderr: &str) -> String {
    stderr
        .split_inclusive('\n')
        .map(|line| {
            let Some(at) = line.rfind(" in ").filter(|_| line.starts_with("read ")) else { return line.to_string() };
            let rest = &line[at + 4..];
            let digits = rest.find(|c: char| !(c.is_ascii_digit() || c == '.')).unwrap_or(rest.len());
            match rest[digits..].strip_prefix('s') {
                Some(tail) if digits > 0 => format!("{} in <time>{tail}", &line[..at]),
                _ => line.to_string(),
            }
        })
        .collect()
}

/// The day store of 2025 that `year` reads, in the home's cache folder: Claude Code from Mon 17
/// Mar, a request a day at noon UTC with a break in July, and Codex on weekdays from September;
/// with spend, avoidable dollars by cause, models, cache reads, commits and cold returns, and
/// nothing in 2024. Its logs are gone, so the refresh keeps its entries as they are.
fn year_store(home: &Path) {
    use tokprof_core::daystore::{Entry, Minute, Store};
    let day = |y: i32, m: u32, d: u32| chrono::NaiveDate::from_ymd_opt(y, m, d).unwrap();
    let noon = |d: chrono::NaiveDate| d.and_hms_opt(12, 0, 0).unwrap().and_utc().timestamp_millis();
    let mut entries: Vec<Entry> = Vec::new();
    let mut date = day(2025, 3, 17);
    while date <= day(2025, 12, 31) {
        use chrono::Datelike;
        let (month, dom, weekday) = (date.month(), date.day(), date.weekday().num_days_from_monday());
        let ts = noon(date);
        let hour = ts / 3_600_000;
        let week = date.iso_week().week();
        let mut log = |agent: &str, usd: f64, commits: u32| {
            let id = format!("{agent}-{week}");
            let project = if week % 3 == 0 { "~/src/zebrafish" } else { "~/src/heron" };
            let at = entries.iter().position(|e| e.session == id).unwrap_or_else(|| {
                let key = format!("/logs/{agent}/{id}.jsonl");
                entries.push(Entry { key, agent: agent.into(), session: id.clone(), main: id.clone(), project: project.into(), ..Entry::default() });
                entries.len() - 1
            });
            let e = &mut entries[at];
            let avoidable = (usd * 0.08 * 100.0).round() / 100.0;
            e.minutes.push(Minute {
                minute: ts / 60_000,
                usd,
                avoidable_usd: avoidable,
                requests: 40,
                commits,
                read_tokens: 3_800_000,
                prompt_tokens: 4_000_000,
                saved_usd: usd * 6.0,
                ..Minute::default()
            });
            let model = if agent == "codex" { "gpt-5.6-codex" } else { "claude-opus-5-5" };
            e.models.push((hour, model.into(), usd));
            e.misses.push((hour, "idle > TTL".into(), (avoidable * 0.7 * 100.0).round() / 100.0));
            e.misses.push((hour, "model switch".into(), (avoidable * 0.3 * 100.0).round() / 100.0));
            if dom % 3 == 0 {
                e.cold.push(ts);
            }
        };
        let paused = month == 7 && (14..=27).contains(&dom);
        if !paused {
            let base = [0.0, 0.0, 20.0, 30.0, 35.0, 25.0, 15.0, 40.0, 45.0, 60.0, 50.0, 30.0][month as usize - 1];
            log("claude", base + f64::from(dom % 7) * 3.0, dom % 4);
        }
        if month >= 9 && weekday < 5 {
            log("codex", 5.0 + f64::from(dom % 5), u32::from(dom % 2 == 0));
        }
        date = date.succ_opt().unwrap();
    }
    let cache = home.join("cache");
    std::fs::create_dir_all(&cache).unwrap();
    let store = Store { path: cache.join(tokprof_core::daystore::FILE_NAME), entries, ..Store::default() };
    store.save().unwrap();
}

/// The lines of `tokprof prices` that the test's price file decides: everything but the table's
/// rows, and the rows of the models it names, each with its continuation lines.
fn test_prices(stdout: &str) -> String {
    let mut out = Vec::new();
    let mut in_table = false;
    let mut keep = false;
    for line in stdout.lines() {
        if line.starts_with("model ") {
            in_table = true;
        } else if in_table && line.is_empty() {
            in_table = false;
        } else if in_table {
            if !line.starts_with(' ') {
                keep = line.trim_end().ends_with(" --prices");
            }
            if !keep {
                continue;
            }
        }
        out.push(line.split_whitespace().collect::<Vec<_>>().join(" "));
    }
    out.join("\n") + "\n"
}

fn compare(name: &str, actual: &str) {
    let path = golden_dir().join(format!("{name}.txt"));
    if bless() {
        std::fs::write(&path, actual).unwrap();
        return;
    }
    let expected = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}; write it with TOKPROF_BLESS=1", path.display())).replace("\r\n", "\n");
    if expected == actual {
        return;
    }
    let (want, got): (Vec<&str>, Vec<&str>) = (expected.lines().collect(), actual.lines().collect());
    let first = want.iter().zip(&got).position(|(w, g)| w != g).unwrap_or(want.len().min(got.len()));
    let around =
        |lines: &[&str]| lines.iter().enumerate().skip(first.saturating_sub(2)).take(6).map(|(i, l)| format!("{:>4} {l}\n", i + 1)).collect::<String>();
    panic!(
        "{name}: the output differs from {} from line {}\nexpected:\n{}actual:\n{}\nthe whole output:\n{actual}\nIf the change is meant, write the file again with TOKPROF_BLESS=1 cargo test -p tokprof --test golden",
        path.display(),
        first + 1,
        around(&want),
        around(&got),
    );
}

/// Every golden file has its case, so that a case removed leaves no file behind.
#[test]
fn every_golden_file_has_a_case() {
    for entry in std::fs::read_dir(golden_dir()).unwrap().flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        let Some(case) = name.strip_suffix(".txt") else { continue };
        if CASES.contains(&case) {
            continue;
        }
        if bless() {
            std::fs::remove_file(entry.path()).unwrap();
        } else {
            panic!("{name} has no case in golden.rs; remove it, or run with TOKPROF_BLESS=1");
        }
    }
}

/// A reader that went away, as `head` does after its lines, ends tokprof quietly and with success:
/// stdout is a pipe whose read end is closed before tokprof starts.
#[test]
fn a_closed_stdout_ends_quietly() {
    let home = common::temp_home("closed-pipe");
    let cases: [&[&str]; 6] =
        [&["prices"], &["summary"], &["sessions", "--limit", "5000"], &["handoff", "5e5510f0"], &["sessions", "--cost"], &["ui", "--dump", "80x20"]];
    for args in cases {
        let (reader, writer) = std::io::pipe().unwrap();
        drop(reader);
        let out = common::tokprof_fixtures(&home, true).args(args).stdout(writer).stderr(std::process::Stdio::piped()).output().unwrap();
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert!(out.status.success() && !stderr.contains("panicked"), "tokprof {}: {}\n{stderr}", args.join(" "), out.status);
    }
    std::fs::remove_dir_all(&home).unwrap();
}

/// What a command does besides printing still happens when its reader went away.
#[test]
fn a_closed_stdout_still_writes_the_file_asked_for() {
    let home = common::temp_home("closed-pipe-file");
    let svg = home.join("bench.svg");
    let (reader, writer) = std::io::pipe().unwrap();
    drop(reader);
    let out = common::tokprof_fixtures(&home, true)
        .args(["bench", "--min-context", "1", "--svg"])
        .arg(&svg)
        .stdout(writer)
        .stderr(std::process::Stdio::piped())
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success() && !stderr.contains("panicked"), "{}\n{stderr}", out.status);
    assert!(svg.exists(), "the chart is written though nobody read the report\n{stderr}");
    std::fs::remove_dir_all(&home).unwrap();
}
