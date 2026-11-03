use std::io::IsTerminal;
use std::path::{Path, PathBuf};
use std::time::Instant;

use anyhow::{Context, Result, bail};
use clap::{CommandFactory, FromArgMatches, Parser, Subcommand, ValueEnum};

use tokprof_core::catalog::Catalog;
use tokprof_core::query::Query;
use tokprof_core::scan::{OnUnreadable, scan};
use tokprof_core::labels::display_path;
use tokprof_core::source::{AgentId, SessionRef};
use tokprof_core::analysis::{self, Component};
use tokprof_core::export;
use tokprof_core::profile::Measure;
use tokprof_core::report::{self, Collector, Dim};
use tokprof_core::pauses::Pauses;
use tokprof_core::rhythm::{self, Timeline, View};
use crate::day::{keeps_client, temp_folders};
use tokprof_core::share::{self, Share};
use tokprof_core::{bench, fmt, handoff, xray};

mod clipboard;
mod day;
mod paths;
mod render_bench;
mod render_report;
mod render_rhythm;
mod render_sessions;
mod render_xray;
mod ui;
mod watch;

#[derive(Parser)]
#[command(name = "tokprof", version, about = "Token, cost and prompt-cache profiler for AI coding-agent session logs")]
struct Cli {
    /// Agents to read, comma separated.
    // The default is every agent the catalog declares, set in `main`.
    #[arg(long, value_delimiter = ',', global = true)]
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
    /// Only requests whose client, the program that ran the agent as its log names it (such as
    /// sdk-ts or cli), contains this text, ignoring case.
    #[arg(long, global = true)]
    client: Option<String>,
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
    /// List session logs on this machine, newest first by last activity.
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
    /// How long you pause before typing again, and what that costs through the prompt cache:
    /// pauses before typed messages, the share after each cache lifetime, cold returns by pause,
    /// and sessions by their largest context.
    Pauses,
    /// Write one JSON file of aggregates over the selected requests that holds no names, to
    /// compare or pool with other users. Nothing is sent anywhere.
    Share {
        /// Output file; by default share-<date>.json in ~/tokprof (or $TOKPROF_DIR).
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
    /// One day of your work with the agents: a strip with a lane per session that shows when the
    /// agent worked and when it waited for your answer, then your sittings, what the agents did
    /// after you left, their ends of turn while you were there, your answers, your interrupts, and
    /// the day's cold returns. The day starts in the middle of your night, as the logs show it.
    Day {
        /// The day that begins on this date (YYYY-MM-DD); by default the day that holds now.
        date: Option<String>,
    },
    /// Spend by tool: calls, sizes, what keeping them cost (rent) and what they made the agent
    /// do (trips), with a hint per row.
    Tools {
        /// Sort by rent or by trips.
        #[arg(long, value_enum, default_value = "rent")]
        by: By,
    },
    /// What coming back to a big session after its prompt cache expired cost, and what the
    /// same work costs after /compact, in a new session that starts from a handoff note, or
    /// with heartbeats that keep the cache warm.
    Bench {
        /// Follow each return for up to this many requests.
        #[arg(long, default_value_t = 60, value_parser = clap::builder::RangedU64ValueParser::<usize>::new().range(1..))]
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
enum By {
    Rent,
    Trips,
}

#[derive(Clone, Copy, ValueEnum)]
enum Value {
    Tokens,
    Cost,
    Avoidable,
}

fn main() -> Result<()> {
    let catalog = tokprof_sources::catalog();
    // The default agents are the ones the catalog declares; clap keeps the text for `--help`.
    let names: &'static str = Box::leak(catalog.agents().iter().map(AgentId::name).collect::<Vec<_>>().join(",").into_boxed_str());
    let matches = Cli::command().mut_arg("agent", |a| a.default_value(names)).get_matches();
    let mut cli = Cli::from_arg_matches(&matches).unwrap_or_else(|e| e.exit());
    cli.project = cli.project.as_deref().map(normalize_project);
    let agents = cli.agent.iter().map(|a| catalog.agent(a)).collect::<Result<Vec<_>>>()?;
    let pricing = paths::pricing(cli.prices.as_deref())?;
    let style = fmt::Style::with_color(std::io::stdout().is_terminal() && std::env::var_os("NO_COLOR").is_none());
    let mut command = match cli.command.take() {
        None => Command::Scan { target: None },
        Some(c) => c,
    };
    // Without `-o`, the profile goes into the profile directory, which is made once the options
    // are known to be valid.
    let mut default_dir = None;
    if let Command::Flame { output: output @ None, value, .. } = &mut command {
        let path = paths::profile_dir().join(default_name(&cli, *value));
        default_dir = path.parent().map(Path::to_path_buf);
        *output = Some(path);
    }
    if let Command::Prices { at } = &command {
        let at = match at.as_deref() {
            Some(d) => Some((parse_date(d)?, d)),
            None => None,
        };
        print!("{}", render_report::prices(&pricing, at));
        return Ok(());
    }
    // The reports and the commands that pick or list sessions select by the same filters.
    let select = Query {
        agents: agents.clone(),
        project: cli.project.clone(),
        session: cli.session.clone(),
        since_ms: cli.since.as_deref().map(parse_since).transpose()?,
        until_ms: cli.until.as_deref().map(parse_date).transpose()?,
        client: cli.client.clone(),
    };
    let filter = select.window();
    let filtered = cli.project.is_some() || cli.session.is_some() || cli.since.is_some() || cli.until.is_some() || cli.client.is_some();
    let none_found = || if filtered { "no session logs found for these filters" } else { "no session logs found" };
    match &command {
        Command::Scan { target } => {
            let file = match target {
                Some(spec) => xray::find(spec, &catalog, &select)?,
                None => xray::latest(&catalog, &select).context(none_found())?,
            };
            print!("{}", render_xray::text(&xray::xray(&catalog, &file, &pricing, now_ms())?, &style, &paths::profile_dir(), catalog.home()));
            return Ok(());
        }
        Command::Sessions { limit, cost } => {
            let rows = xray::rows(&catalog, &select, *limit);
            let now = now_ms();
            let glances: Vec<Option<xray::Glance>> = if *cost { xray::glances(&catalog, &rows, &pricing, now).into_iter().map(Some).collect() } else { vec![None; rows.len()] };
            print!("{}", render_sessions::sessions(&rows, &glances, *cost, &style, now));
            return Ok(());
        }
        Command::Handoff { target, note } => {
            let file = match target {
                Some(spec) => xray::find(spec, &catalog, &select)?,
                None => xray::latest(&catalog, &select).context(none_found())?,
            };
            let text = if *note { handoff::with_note(&catalog, &file)? } else { handoff::line(&catalog, &file) };
            println!("{text}");
            if std::io::stdout().is_terminal() {
                let done = match clipboard::copy(&text) {
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
                    let size = d.split_once('x').and_then(|(w, h)| Some((w.parse().ok()?, h.parse().ok()?)));
                    Some(size.context("--dump takes WIDTHxHEIGHT")?)
                }
                None => None,
            };
            return ui::run(std::sync::Arc::new(catalog), pricing, size, select);
        }
        _ => {}
    }
    if let Command::Day { date } = &command {
        day_takes_no_time_filter(&cli)?;
        let date = date.as_deref().map(|d| parse_date(d).map(|ms| rhythm::date_of(ms + 12 * 3_600_000, &rhythm::local_offset))).transpose()?;
        let day = day::build(&catalog, &select, &pricing, date, now_ms(), rhythm::Layout::TERMINAL, None, &style)?;
        eprintln!("{}", day.read);
        for line in &day.warnings {
            eprintln!("{line}");
        }
        print!("{}", day.text);
        return Ok(());
    }
    if let Command::Bench { requests, min_context, keep_warm, svg } = &command {
        let started = Instant::now();
        // Returns happen in main conversations, so subagent logs are left out.
        let files: Vec<SessionRef> = catalog
            .discover(&agents)
            .into_iter()
            .filter(|f| !f.subagent && select.holds(f))
            .collect();
        if files.is_empty() {
            bail!("no session logs found for {}", cli.agent.join(", "));
        }
        let (found, stats) = scan(
            &catalog,
            &files,
            &select,
            OnUnreadable::Skip,
            bench::Found::default,
            |found, s, _| *found = std::mem::take(found).merge(bench::scan(s, &filter, *requests, *min_context)),
            bench::Found::merge,
        )?;
        eprintln!("{}", sources(&catalog, &files, started.elapsed().as_secs_f64()));
        warn(&stats.unreadable);
        let b = bench::replay(found, &pricing, *requests, *min_context, *keep_warm);
        print!("{}", render_bench::report(&b, &style));
        if let Some(path) = svg {
            for written in render_bench::write_svg(&b, path)? {
                println!("wrote {}", display_path(&written.to_string_lossy(), None, catalog.home()));
            }
        }
        return Ok(());
    }
    if let Command::Pauses = &command {
        let started = Instant::now();
        let mut files = catalog.discover(&agents);
        // A session's subagent logs go with it.
        files.retain(|f| select.holds(f));
        if files.is_empty() {
            bail!("no session logs found for {}", cli.agent.join(", "));
        }
        let temp = temp_folders();
        let ((mut p, timelines), stats) = scan(
            &catalog,
            &files,
            &select,
            OnUnreadable::Skip,
            || (Pauses::new(filter.clone()), Vec::new()),
            |(p, timelines): &mut (Pauses, Vec<Timeline>), session, f| {
                analysis::analyze(session, &pricing, &filter, p);
                if !f.subagent && keeps_client(session, &filter) {
                    timelines.push(Timeline::of(session, &temp));
                }
            },
            |mut a, b| {
                a.0.merge(b.0);
                a.1.extend(b.1);
                a
            },
        )?;
        p.finish();
        eprintln!("{}", sources(&catalog, &files, started.elapsed().as_secs_f64()));
        warn(&stats.unreadable);
        print!("{}", render_report::pauses(&p));
        let offset = &rhythm::local_offset;
        let view = View::new(timelines, filter.until_ms.unwrap_or_else(now_ms), offset);
        print!("{}", render_rhythm::section(&view, (filter.since_ms.unwrap_or(i64::MIN), filter.until_ms.unwrap_or(i64::MAX)), offset));
        return Ok(());
    }
    if let Command::Share { output } = &command {
        let started = Instant::now();
        let mut files = catalog.discover(&agents);
        // A session's subagent logs go with it.
        files.retain(|f| select.holds(f));
        if files.is_empty() {
            bail!("no session logs found for {}", cli.agent.join(", "));
        }
        let (sink, stats) = scan(
            &catalog,
            &files,
            &select,
            OnUnreadable::Skip,
            || (Collector::new(None, None), (Pauses::new(filter.clone()), Share::default())),
            |t, session, _| analysis::analyze(session, &pricing, &filter, t),
            |mut a, b| {
                a.0.merge(b.0);
                a.1.0.merge(b.1.0);
                a.1.1.merge(b.1.1);
                a
            },
        )?;
        let (collector, (mut p, s)) = sink;
        p.finish();
        eprintln!("{}", sources(&catalog, &files, started.elapsed().as_secs_f64()));
        warn(&stats.unreadable);
        if s.span_ms.is_none() {
            bail!("no requests found for these filters; nothing written");
        }
        let value = share::to_json(&collector, &p, &s, env!("CARGO_PKG_VERSION"), |ms| fmt::local_time(ms, "%Y-%m-%d"));
        let path = output.clone().unwrap_or_else(|| paths::profile_dir().join(format!("share-{}.json", chrono::Local::now().format("%Y-%m-%d"))));
        if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
            std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
        }
        std::fs::write(&path, share::pretty(&value)).with_context(|| format!("writing {}", path.display()))?;
        println!("wrote {}", display_path(&path.to_string_lossy(), None, catalog.home()));
        println!(
            "Per agent it holds: spend by model and component, clients, cache writes by lifetime, misses by cause, spend by kind of origin, trigger and activity, setup by source, effort levels, the pause report and session counts."
        );
        println!("It holds no project, path, session id, title, task, prompt, tool, skill or server name.");
        println!("Nothing was sent anywhere: review the file, then send it yourself.");
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
            if output.as_deref().is_some_and(|o| export::has_ext(o, "jfr")) {
                let thread_dims = parse_dims(thread_dims)?;
                if dims.iter().all(|d| thread_dims.contains(d)) {
                    bail!("--stack needs a dimension that --threads does not take");
                }
                threads = Some(thread_dims);
            }
            let comps = component
                .iter()
                .map(|c| Component::parse(c).with_context(|| format!("unknown component {c:?}; use read, write, rewrite, uncached, output")))
                .collect::<Result<Vec<_>>>()?;
            if let Some(dir) = &default_dir {
                std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
            }
            (Some(dims), (!comps.is_empty()).then_some(comps))
        }
        _ => (None, None),
    };

    let started = Instant::now();
    let mut files = catalog.discover(&agents);
    // A session's subagent logs go with it.
    files.retain(|f| select.holds(f));
    if files.is_empty() {
        bail!("no session logs found for {}", cli.agent.join(", "));
    }
    let (collector, stats) = scan(
        &catalog,
        &files,
        &select,
        OnUnreadable::Skip,
        || {
            let mut c = Collector::new(stack.clone(), components.clone());
            c.profile.threads = threads.clone();
            c
        },
        |c, session, _| analysis::analyze(session, &pricing, &filter, c),
        |mut a, b| {
            a.merge(b);
            a
        },
    )?;
    eprintln!("{}", sources(&catalog, &files, started.elapsed().as_secs_f64()));
    warn(&stats.unreadable);

    match command {
        Command::Scan { .. }
        | Command::Sessions { .. }
        | Command::Handoff { .. }
        | Command::Ui { .. }
        | Command::Prices { .. }
        | Command::Bench { .. }
        | Command::Day { .. }
        | Command::Pauses
        | Command::Share { .. } => {
            unreachable!("handled before reading logs")
        }
        Command::Summary => print!("{}", render_report::summary(&collector, &filter, &paths::user_prices())),
        Command::Misses { top, expected } => print!("{}", render_report::misses(&collector, top, expected)),
        Command::Whatif => print!("{}", render_report::whatif(&collector, &catalog.agents())),
        Command::Tools { by } => print!("{}", render_report::tools(&collector.tools, matches!(by, By::Trips), 25)),
        Command::Flame { output, value, stack, title, highlight, .. } => {
            let output = output.expect("set above");
            let shown = display_path(&output.to_string_lossy(), None, catalog.home());
            let (measure, what) = match value {
                Value::Tokens => (Measure::Tokens, "tokens"),
                Value::Cost => (Measure::Cost, "cost, µ$"),
                Value::Avoidable => (Measure::Avoidable, "avoidable cost of cache misses, µ$"),
            };
            if export::has_ext(&output, "jfr") {
                let w = tokprof_core::jfr::write(&output, &collector.profile.timed, measure)?;
                let per_sample = match measure {
                    Measure::Tokens => format!("{} tokens", w.quantum),
                    _ => format!("${}", w.quantum as f64 / 1e6),
                };
                println!(
                    "wrote {} (1 sample = {per_sample}), {}, {} to {shown}",
                    fmt::count(w.samples as u64, "sample", "samples"),
                    fmt::count(w.threads as u64, "thread", "threads"),
                    fmt::count(w.stack_traces as u64, "stack trace", "stack traces")
                );
                return Ok(());
            }
            let title = title.unwrap_or_else(|| format!("tokprof: {what} by {}", stack.join(" > ")));
            let html = export::HtmlOptions { title: &title, highlight: highlight.as_deref() };
            let n = export::write(&output, &collector.profile.stacks, measure, &html)?;
            let written = if export::has_ext(&output, "html") { fmt::count(n as u64, "frame", "frames") } else { fmt::count(n as u64, "stack", "stacks") };
            println!("wrote {written} to {shown}");
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

/// `--project` as the projects are written: the home directory as `~`, no trailing `/`, so that a
/// path the shell expanded matches.
fn normalize_project(p: &str) -> String {
    let p = if p.len() > 1 { p.trim_end_matches('/') } else { p };
    display_path(p, None, &paths::home())
}

/// The first logs that could not be read, then how many more there were.
fn warn(errors: &[String]) {
    for line in warnings(errors) {
        eprintln!("{line}");
    }
}

fn warnings(errors: &[String]) -> Vec<String> {
    const SHOWN: usize = 5;
    let mut lines: Vec<String> = errors.iter().take(SHOWN).map(|e| format!("warning: {e}")).collect();
    match errors.len().saturating_sub(SHOWN) {
        0 => {}
        1 => lines.push("warning: 1 more log could not be read".to_string()),
        n => lines.push(format!("warning: {n} more logs could not be read")),
    }
    lines
}

fn sources(catalog: &Catalog, files: &[SessionRef], secs: f64) -> String {
    let mut agents: Vec<&AgentId> = files.iter().map(|f| &f.agent).collect();
    agents.sort_by_key(|a| catalog.order(a));
    agents.dedup();
    let mut parts = Vec::new();
    for agent in agents {
        let (n, bytes) = files.iter().filter(|f| &f.agent == agent).fold((0, 0u64), |(n, b), f| (n + 1, b + f.size));
        if n > 0 {
            parts.push(format!("{} {n} files ({:.1} GB)", agent.name(), bytes as f64 / 1e9));
        }
    }
    format!("read {} in {secs:.1}s", parts.join(", "))
}

/// `day` picks its time by its date, so the time filters would contradict it.
fn day_takes_no_time_filter(cli: &Cli) -> Result<()> {
    if cli.since.is_some() || cli.until.is_some() {
        bail!("day picks its time by DATE; drop --since and --until");
    }
    Ok(())
}

/// The clock: the library reads none, so the X-ray and the list get the time from here.
fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
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
    fn warnings_stop_after_five_and_count_the_rest() {
        let errors: Vec<String> = (0..12).map(|i| format!("log{i}: broken")).collect();
        let lines = warnings(&errors);
        assert_eq!(lines.len(), 6);
        assert_eq!((lines[0].as_str(), lines[5].as_str()), ("warning: log0: broken", "warning: 7 more logs could not be read"));
        assert_eq!(warnings(&errors[..6]).last().unwrap(), "warning: 1 more log could not be read");
        assert_eq!(warnings(&errors[..5]).len(), 5);
    }

    #[test]
    fn default_profile_names_follow_the_filters() {
        assert_eq!(name(&["flame"], Value::Cost), "all.jfr");
        assert_eq!(name(&["--session", "3f951de0", "flame"], Value::Cost), "3f951de0.jfr");
        assert_eq!(name(&["--project", "~/src/billing", "--since", "7d", "flame"], Value::Cost), "billing-since-7d.jfr");
        assert_eq!(name(&["--agent", "codex", "flame"], Value::Avoidable), "codex-all-misses.jfr");
        let mut cli = Cli::parse_from(["tokprof", "--project", "~/src/billing/", "--since", "7d", "flame"]);
        cli.project = cli.project.as_deref().map(normalize_project);
        assert_eq!(default_name(&cli, Value::Cost), "billing-since-7d.jfr");
    }

    #[test]
    fn day_fails_on_time_filters() {
        let err = day_takes_no_time_filter(&Cli::parse_from(["tokprof", "--since", "7d", "day"])).unwrap_err();
        assert_eq!(err.to_string(), "day picks its time by DATE; drop --since and --until");
        assert!(day_takes_no_time_filter(&Cli::parse_from(["tokprof", "--until", "2026-11-01", "day", "2026-10-30"])).is_err());
        assert!(day_takes_no_time_filter(&Cli::parse_from(["tokprof", "--project", "billing", "day"])).is_ok());
    }

    #[test]
    fn project_filters_read_like_the_projects() {
        let home = std::env::home_dir().unwrap().to_string_lossy().to_string();
        assert_eq!(normalize_project(&format!("{home}/src/app")), "~/src/app");
        assert_eq!(normalize_project("~/src/app/"), "~/src/app");
        assert_eq!(normalize_project("billing"), "billing");
        assert_eq!(normalize_project("/"), "/");
    }
}
