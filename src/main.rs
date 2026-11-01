use std::io::IsTerminal;
use std::path::PathBuf;
use std::time::Instant;

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand, ValueEnum};
use rayon::prelude::*;

use tokprof::adapters::{self, SourceFile};
use tokprof::analysis::{self, Component, Filter};
use tokprof::export::{self, Measure};
use tokprof::model::Agent;
use tokprof::pricing::Pricing;
use tokprof::report::{self, Collector, Dim};
use tokprof::{bench, handoff, xray};

#[derive(Parser)]
#[command(name = "tokprof", version, about = "Token, cost and prompt-cache profiler for AI coding-agent session logs")]
struct Cli {
    /// Agents to read, comma separated.
    #[arg(long, value_delimiter = ',', default_value = "claude,codex", global = true)]
    agent: Vec<String>,
    /// Only requests on or after this date (YYYY-MM-DD) or this many days back (7d).
    #[arg(long, global = true)]
    since: Option<String>,
    /// Only requests before this date (YYYY-MM-DD).
    #[arg(long, global = true)]
    until: Option<String>,
    /// Only projects whose path contains this text.
    #[arg(long, global = true)]
    project: Option<String>,
    /// Only the session with this id prefix.
    #[arg(long, global = true)]
    session: Option<String>,
    /// TOML price file applied over the built-in prices and ~/.config/tokprof/prices.toml.
    #[arg(long, global = true)]
    prices: Option<PathBuf>,
    /// Without a command, X-ray the latest session on this machine.
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// X-ray one session: a session file, a session id prefix, or the latest session.
    Scan { target: Option<String> },
    /// The line to start a new session with so that it picks up where an earlier one stopped,
    /// instead of resuming it: the session's id and the path of its log. In a terminal, it is
    /// also copied to the clipboard.
    Handoff {
        /// A session file, a session id prefix, or the latest session.
        target: Option<String>,
        /// Add a note built from the log without a model: the task, the latest prompts, where
        /// the work stopped, the task list, and the files and commands it worked with.
        #[arg(long)]
        note: bool,
    },
    /// Browse all sessions interactively: the list on the left, the X-ray on the right.
    Ui {
        /// Render one frame as text, e.g. 160x45, instead of running in the terminal.
        #[arg(long, hide = true)]
        dump: Option<String>,
    },
    /// List session logs on this machine, newest first.
    Sessions {
        #[arg(long, default_value_t = 20)]
        limit: usize,
        /// Also compute each session's cost (parses the listed sessions).
        #[arg(long)]
        cost: bool,
    },
    /// Spend by model and cost component, and cache misses by cause.
    Summary,
    /// Cache misses by cause and the most expensive ones.
    Misses {
        #[arg(long, default_value_t = 20)]
        top: usize,
        /// Also list expected rebuilds (compaction, cleared tool results).
        #[arg(long)]
        expected: bool,
    },
    /// Write a profile; the extension picks the format: .jfr (IntelliJ profiler, JMC), .html
    /// (async-profiler flame graph), .collapsed (for async-profiler's jfrconv), .pb.gz (pprof),
    /// anything else folded stacks.
    Flame {
        /// Output file; by default a .jfr in ~/tokprof (or $TOKPROF_DIR) named after the filters.
        #[arg(short, long)]
        output: Option<PathBuf>,
        /// Stack dimensions, root first.
        #[arg(long, value_delimiter = ',', default_value = report::DEFAULT_STACK)]
        stack: Vec<String>,
        #[arg(long, value_enum, default_value = "cost")]
        value: Value,
        /// Only these components: read, write, rewrite, uncached, output.
        #[arg(long, value_delimiter = ',')]
        component: Vec<String>,
        /// Title of the HTML flame graph.
        #[arg(long)]
        title: Option<String>,
        /// Highlight frames matching this regex in the HTML flame graph.
        #[arg(long)]
        highlight: Option<String>,
        /// For .jfr: dimensions that make up a thread; the rest of the stack goes below it.
        #[arg(long, value_delimiter = ',', default_value = report::DEFAULT_THREADS)]
        threads: Vec<String>,
    },
    /// Replay Claude requests under 5m and 1h cache lifetimes.
    Whatif,
    /// What coming back to a big session after its prompt cache expired cost, and what the
    /// same work costs after /compact, in a new session that starts from a handoff note, or
    /// with heartbeats that keep the cache warm.
    Bench {
        /// Follow each return for up to this many requests.
        #[arg(long, default_value_t = 60)]
        requests: usize,
        /// Only returns to contexts of at least this many tokens.
        #[arg(long, default_value_t = 100_000)]
        min_context: u64,
        /// How long heartbeats keep the cache warm after the last request: 6h, 90m.
        #[arg(long, default_value = "6h", value_parser = parse_duration)]
        keep_warm: i64,
        /// Write the chart of the mean cost per return to this .svg file, and a dark-theme
        /// version next to it with -dark before the extension.
        #[arg(long)]
        svg: Option<PathBuf>,
    },
    /// Show the prices, their periods and which config each one came from.
    Prices {
        /// Only the prices in effect on this date (YYYY-MM-DD).
        #[arg(long)]
        at: Option<String>,
    },
}

#[derive(Clone, Copy, ValueEnum)]
enum Value {
    Tokens,
    Cost,
    Avoidable,
}

fn main() -> Result<()> {
    let mut cli = Cli::parse();
    let agents = cli
        .agent
        .iter()
        .map(|a| Agent::parse(a).with_context(|| format!("unknown agent {a:?}")))
        .collect::<Result<Vec<_>>>()?;
    let pricing = Pricing::load(cli.prices.as_deref())?;
    let style = xray::Style::detect();
    let mut command = match cli.command.take() {
        None => Command::Scan { target: None },
        Some(c) => c,
    };
    if let Command::Flame { output: output @ None, value, .. } = &mut command {
        let path = export::default_dir().join(default_name(&cli, *value));
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
        }
        *output = Some(path);
    }
    match &command {
        Command::Prices { at } => {
            let at = match at.as_deref() {
                Some(d) => Some((parse_date(d)?, d)),
                None => None,
            };
            print!("{}", report::prices(&pricing, at));
            return Ok(());
        }
        Command::Scan { target } => {
            let file = match target {
                Some(spec) => xray::find(spec, &agents)?,
                None => xray::main_sessions(&agents).into_iter().next().context("no session logs found")?,
            };
            print!("{}", xray::xray(&file, &pricing, &style)?);
            return Ok(());
        }
        Command::Sessions { limit, cost } => {
            print!("{}", xray::sessions(&agents, *limit, *cost, &pricing, &style));
            return Ok(());
        }
        Command::Handoff { target, note } => {
            let file = match target {
                Some(spec) => xray::find(spec, &agents)?,
                None => xray::main_sessions(&agents).into_iter().next().context("no session logs found")?,
            };
            let text = if *note { handoff::with_note(&file)? } else { handoff::line(&file) };
            println!("{text}");
            if std::io::stdout().is_terminal() {
                let done = match handoff::copy(&text) {
                    Some(_) => "copied to the clipboard: paste it into a new session",
                    None => "no clipboard tool found (pbcopy, wl-copy, xclip): copy the text above",
                };
                eprintln!("{}", style.dim(done));
            }
            return Ok(());
        }
        Command::Ui { dump } => {
            let size = match dump {
                Some(d) => {
                    let (w, h) = d.split_once('x').context("--dump takes WIDTHxHEIGHT")?;
                    Some((w.parse()?, h.parse()?))
                }
                None => None,
            };
            return tokprof::ui::run(&agents, pricing, size);
        }
        _ => {}
    }
    let filter = Filter { since_ms: cli.since.as_deref().map(parse_since).transpose()?, until_ms: cli.until.as_deref().map(parse_date).transpose()? };
    if let Command::Bench { requests, min_context, keep_warm, svg } = &command {
        let started = Instant::now();
        // Returns happen in main conversations, so subagent logs are left out.
        let files: Vec<SourceFile> = adapters::discover(&agents)
            .into_iter()
            .filter(|f| !f.is_subagent() && cli.session.as_ref().is_none_or(|id| f.path.to_string_lossy().contains(id.as_str())))
            .collect();
        let found = files
            .par_iter()
            .filter_map(|f| adapters::parse(f).ok())
            .filter(|s| accepts(&cli, s))
            .map(|s| bench::scan(&s, &filter, *requests, *min_context))
            .reduce(bench::Found::default, bench::Found::merge);
        eprintln!("{}", sources(&files, started.elapsed().as_secs_f64()));
        let b = bench::replay(found, &pricing, *requests, *min_context, *keep_warm);
        print!("{}", bench::report(&b, &style));
        if let Some(path) = svg {
            for written in bench::write_svg(&b, path)? {
                println!("wrote {}", adapters::display_path(&written.to_string_lossy()));
            }
        }
        return Ok(());
    }

    let parse_dims = |names: &[String]| {
        names
            .iter()
            .filter(|d| !d.is_empty())
            .map(|d| Dim::parse(d).with_context(|| format!("unknown dimension {d:?}; use {}", Dim::NAMES)))
            .collect::<Result<Vec<_>>>()
    };
    let mut threads = None;
    let (stack, components) = match &command {
        Command::Flame { stack, component, output, threads: thread_dims, .. } => {
            let dims = parse_dims(stack)?;
            if output.as_ref().is_some_and(|o| o.extension().is_some_and(|e| e == "jfr")) {
                threads = Some(parse_dims(thread_dims)?);
            }
            let comps = component
                .iter()
                .map(|c| Component::parse(c).with_context(|| format!("unknown component {c:?}")))
                .collect::<Result<Vec<_>>>()?;
            (Some(dims), (!comps.is_empty()).then_some(comps))
        }
        _ => (None, None),
    };

    let started = Instant::now();
    let mut files = adapters::discover(&agents);
    if let Some(id) = &cli.session {
        // Session ids are part of the file paths, subagent transcripts included; `accepts`
        // still checks the parsed id.
        files.retain(|f| f.path.to_string_lossy().contains(id.as_str()));
    }
    if files.is_empty() {
        bail!("no session logs found for {}", cli.agent.join(", "));
    }
    let collector = files
        .par_iter()
        .map(|file| {
            let mut c = Collector::new(stack.clone(), components.clone());
            c.threads = threads.clone();
            c.files = 1;
            c.bytes = file.size;
            match adapters::parse(file) {
                Ok(session) if accepts(&cli, &session) => analysis::analyze(&session, &pricing, &filter, &mut c),
                Ok(_) => {}
                Err(e) => c.errors.push(format!("{}: {e:#}", file.path.display())),
            }
            c
        })
        .reduce(
            || {
                let mut c = Collector::new(stack.clone(), components.clone());
                c.threads = threads.clone();
                c
            },
            |mut a, b| {
                a.merge(b);
                a
            },
        );
    eprintln!("{}", sources(&files, started.elapsed().as_secs_f64()));
    for e in collector.errors.iter().take(5) {
        eprintln!("warning: {e}");
    }

    match command {
        Command::Scan { .. }
        | Command::Sessions { .. }
        | Command::Handoff { .. }
        | Command::Ui { .. }
        | Command::Prices { .. }
        | Command::Bench { .. } => {
            unreachable!("handled before reading logs")
        }
        Command::Summary => print!("{}", report::summary(&collector)),
        Command::Misses { top, expected } => print!("{}", report::misses(&collector, top, expected)),
        Command::Whatif => print!("{}", report::whatif(&collector)),
        Command::Flame { output, value, stack, title, highlight, .. } => {
            let output = output.expect("set above");
            let shown = adapters::display_path(&output.to_string_lossy());
            let (measure, what) = match value {
                Value::Tokens => (Measure::Tokens, "tokens"),
                Value::Cost => (Measure::Cost, "cost, µ$"),
                Value::Avoidable => (Measure::Avoidable, "avoidable cost of cache misses, µ$"),
            };
            if output.extension().is_some_and(|e| e == "jfr") {
                let w = tokprof::jfr::write(&output, &collector.timed, measure)?;
                let per_sample = match measure {
                    Measure::Tokens => format!("{} tokens", w.quantum),
                    _ => format!("${}", w.quantum as f64 / 1e6),
                };
                println!(
                    "wrote {} samples (1 sample = {per_sample}), {} threads, {} stack traces to {shown}",
                    w.samples, w.threads, w.stack_traces
                );
                return Ok(());
            }
            let title = title.unwrap_or_else(|| format!("tokprof: {what} by {}", stack.join(" > ")));
            let html = export::HtmlOptions { title: &title, highlight: highlight.as_deref() };
            let n = export::write(&output, &collector.stacks, measure, &html)?;
            println!("wrote {n} {} to {shown}", if output.extension().is_some_and(|e| e == "html") { "frames" } else { "stacks" });
        }
    }
    Ok(())
}

/// File name for a profile without `-o`: the filters that shaped it, e.g. `3f951de0.jfr`,
/// `billing-since-7d.jfr` or `all-misses.jfr`.
fn default_name(cli: &Cli, value: Value) -> String {
    let slug = |s: &str| {
        let s: String = s.chars().map(|c| if c.is_ascii_alphanumeric() || c == '.' { c } else { '-' }).collect();
        s.trim_matches('-').to_string()
    };
    let mut parts = Vec::new();
    if cli.agent.len() == 1 {
        parts.push(slug(&cli.agent[0]));
    }
    parts.extend(cli.session.iter().map(|s| slug(s)));
    parts.extend(cli.project.iter().map(|p| slug(p.rsplit('/').next().unwrap_or(p))));
    if cli.session.is_none() && cli.project.is_none() {
        parts.push("all".to_string());
    }
    parts.extend(cli.since.iter().map(|s| format!("since-{}", slug(s))));
    parts.extend(cli.until.iter().map(|u| format!("until-{}", slug(u))));
    match value {
        Value::Cost => {}
        Value::Tokens => parts.push("tokens".to_string()),
        Value::Avoidable => parts.push("misses".to_string()),
    }
    format!("{}.jfr", parts.join("-"))
}

fn accepts(cli: &Cli, s: &tokprof::model::Session) -> bool {
    cli.project.as_ref().is_none_or(|p| s.project.contains(p.as_str()))
        && cli.session.as_ref().is_none_or(|id| s.id.starts_with(id.as_str()))
}

fn sources(files: &[SourceFile], secs: f64) -> String {
    let mut parts = Vec::new();
    for agent in Agent::ALL {
        let (n, bytes) = files.iter().filter(|f| f.agent == agent).fold((0, 0u64), |(n, b), f| (n + 1, b + f.size));
        if n > 0 {
            parts.push(format!("{} {n} files ({:.1} GB)", agent.name(), bytes as f64 / 1e9));
        }
    }
    format!("read {} in {secs:.1}s", parts.join(", "))
}

fn parse_date(s: &str) -> Result<i64> {
    let d = chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").with_context(|| format!("bad date {s:?}, expected YYYY-MM-DD"))?;
    let t = d.and_hms_opt(0, 0, 0).context("bad date")?.and_local_timezone(chrono::Local).earliest().context("bad local date")?;
    Ok(t.timestamp_millis())
}

fn parse_since(s: &str) -> Result<i64> {
    match s.strip_suffix('d').and_then(|n| n.parse::<i64>().ok()) {
        Some(days) => Ok(chrono::Utc::now().timestamp_millis() - days * 86_400_000),
        None => parse_date(s),
    }
}

/// A duration in hours or minutes, 6h or 90m, in ms.
fn parse_duration(s: &str) -> Result<i64, String> {
    let (n, unit) = match s.strip_suffix('m') {
        Some(n) => (n, 60_000.0),
        None => (s.strip_suffix('h').unwrap_or(s), 3_600_000.0),
    };
    n.parse::<f64>().ok().filter(|v| *v >= 0.0).map(|v| (v * unit).round() as i64).ok_or_else(|| format!("bad duration {s:?}, expected 6h or 90m"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn name(args: &[&str], value: Value) -> String {
        default_name(&Cli::parse_from(std::iter::once("tokprof").chain(args.iter().copied())), value)
    }

    #[test]
    fn default_profile_names_follow_the_filters() {
        assert_eq!(name(&["flame"], Value::Cost), "all.jfr");
        assert_eq!(name(&["--session", "3f951de0", "flame"], Value::Cost), "3f951de0.jfr");
        assert_eq!(name(&["--project", "~/src/billing", "--since", "7d", "flame"], Value::Cost), "billing-since-7d.jfr");
        assert_eq!(name(&["--agent", "codex", "flame"], Value::Avoidable), "codex-all-misses.jfr");
    }
}
