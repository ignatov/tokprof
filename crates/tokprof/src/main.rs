use std::io::IsTerminal;
use std::path::{Path, PathBuf};
use std::time::Instant;

use anyhow::{Context, Result, bail};
use clap::{CommandFactory, FromArgMatches, Parser, Subcommand, ValueEnum};

use tokprof_browse::day::{self, keeps_client, temp_folders};
use tokprof_browse::filters::{normalize_project, parse_date, parse_since};
use tokprof_browse::reading::{sources, warnings};
use tokprof_browse::{calendar, clipboard, paths, render_calendar, render_month, render_requests, render_rhythm, render_sessions, render_xray, render_year};
use tokprof_core::analysis::{self, Component};
use tokprof_core::calendar::Metric;
use tokprof_core::export;
use tokprof_core::labels::display_path;
use tokprof_core::pauses::Pauses;
use tokprof_core::pool;
use tokprof_core::profile::Measure;
use tokprof_core::query::Query;
use tokprof_core::report::{self, Collector, Dim};
use tokprof_core::rhythm::{self, Timeline, View};
use tokprof_core::scan::{OnUnreadable, report_logs, scan};
use tokprof_core::share::{self, Share};
use tokprof_core::source::{AgentId, SessionRef};
use tokprof_core::{bench, fmt, handoff, xray};

/// `print!` to stdout through [`write_out`].
macro_rules! out {
    ($($arg:tt)*) => {
        $crate::write_out(format_args!($($arg)*))
    };
}

/// `println!` to stdout through [`write_out`].
macro_rules! outln {
    ($($arg:tt)*) => {
        $crate::write_out(format_args!("{}\n", format_args!($($arg)*)))
    };
}

mod render_bench;
mod render_pool;
mod render_report;
mod ui;

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
    /// Only sessions whose project, or the folder they ran in, contains this text.
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
    Scan {
        target: Option<String>,
        /// Print the requests of the main conversation instead: each with its tokens, cost and
        /// why, what entered its context since the previous request and what changed.
        #[arg(long)]
        requests: bool,
    },
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
        /// Add the waiting order to `s`: the sessions that wait for you, the longest wait first.
        #[arg(long)]
        waiting: bool,
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
    Summary {
        /// Compare the spend and the avoidable dollars with the window of the same length just
        /// before, from the day store; needs --since.
        #[arg(long)]
        compare: bool,
    },
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
    /// compare or pool with other users, and print what it says. Nothing is sent anywhere.
    Share {
        /// Output file; by default share-<date>.json in ~/tokprof (or $TOKPROF_DIR).
        #[arg(short, long)]
        output: Option<PathBuf>,
        /// A label for the survey the file is for, written into its header: 1 to 40 letters,
        /// digits, '.', '_' or '-'.
        #[arg(long)]
        survey: Option<String>,
        /// MCP servers whose tools and origins the file counts by name, comma separated. Every
        /// other MCP server counts as `mcp`.
        #[arg(long, value_delimiter = ',')]
        mcp_names: Vec<String>,
        /// Answers about you, comma separated: role=developer|qa|devops|manager|designer|
        /// researcher|other, agents=under-3m|3-12m|over-1y (time using coding agents), and
        /// bother=one or two of cost|waiting|quality|context|setup|limits|nothing, joined with '+'.
        #[arg(long, value_delimiter = ',')]
        about: Vec<String>,
        /// Write the spend and the tokens into the header, so that the file holds dollars. Without
        /// it, every dollar and token amount is a share of them, and they stay on this machine.
        #[arg(long)]
        dollars: bool,
    },
    /// Pool share files of several people into one report: where the money went, what could
    /// have been saved, the habits that cost the most, tools and commands, and agent versions.
    Pool {
        /// Share files, or folders whose .json files are read.
        #[arg(required = true)]
        paths: Vec<PathBuf>,
        /// Fold the rows that fewer than this many files have into `other`; 3 or more before the
        /// report is shown to others.
        #[arg(long, default_value_t = 1)]
        min_files: usize,
        /// Add a column per file, headed by its name.
        #[arg(long)]
        by_file: bool,
        /// Print the report's numbers as JSON instead of the tables.
        #[arg(long)]
        json: bool,
        /// Pool only the files with this answer about the person, as about.<key>=<value>, such
        /// as about.role=developer; may be given several times, and all must hold.
        #[arg(long = "where", value_name = "about.<key>=<value>")]
        where_: Vec<String>,
    },
    /// One day of your work with the agents: a strip with a lane per session that shows when the
    /// agent worked and when it waited for your answer, then your sittings, what the agents did
    /// after you left, their ends of turn while you were there, your answers, your interrupts, and
    /// the day's cold returns. The day starts in the middle of your night, as the logs show it.
    Day {
        /// The day that begins on this date (YYYY-MM-DD); by default the day that holds now.
        date: Option<String>,
    },
    /// A calendar of your days, a column per week and a row per weekday, as GitHub draws
    /// contributions, with the total, active days, streaks, the busiest day and weekday, and a
    /// punchcard by weekday and hour. Its numbers come from a day store in the cache folder that
    /// is read again only for the logs that changed, and that keeps the days of deleted logs.
    Calendar {
        /// What the marks show: spend, agent (minutes some agent worked), keyboard (your time at
        /// the keyboard), messages, commits (by the agents) or avoidable (lost to cache misses).
        #[arg(long, default_value = "spend", value_parser = parse_metric)]
        metric: Metric,
    },
    /// A month in review, as a card to post: spend, agent hours, your hours at the keyboard,
    /// sessions, messages and commits, the month's days, its busiest day and hour, the cache,
    /// the top model, the change from the month before, a title, and one thing to change. It
    /// names no project. From the day store, as `tokprof calendar`.
    Month {
        /// YYYY-MM, or `this` for this month so far; by default the last complete month.
        month: Option<String>,
    },
    /// A year in review: the numbers of `tokprof month` over a calendar year, a bar per month,
    /// the records (biggest day and week, longest streak, most commits in a day, most sessions
    /// at once), the rhythm, the cache, the change from the year before or the pace per day,
    /// whether its card is ready, and one thing to change. It names no project. From the day
    /// store, as `tokprof calendar`.
    Year {
        /// YYYY, or `this` for this year so far; by default this year so far when it holds a
        /// request or a message, else the last year that does.
        year: Option<String>,
    },
    /// Spend by tool: calls, sizes, what keeping them cost (rent) and what they made the agent
    /// do (trips), with a hint per row.
    Tools {
        /// Sort by rent, by trips, or by failed calls and then the spend after them.
        #[arg(long, value_enum, default_value = "rent")]
        by: By,
    },
    /// What coming back to a big session after its prompt cache expired cost, and what the
    /// same work costs after /compact, in a new session that starts from a handoff note, or
    /// with heartbeats that keep the cache warm.
    Bench {
        /// Follow each return for up to this many requests.
        #[arg(long, default_value_t = bench::DEFAULT_REQUESTS, value_parser = clap::builder::RangedU64ValueParser::<usize>::new().range(1..))]
        requests: usize,
        /// Only returns to contexts of at least this many tokens.
        #[arg(long, default_value_t = bench::DEFAULT_MIN_CONTEXT)]
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
    Failed,
}

#[derive(Clone, Copy, ValueEnum)]
enum Value {
    Tokens,
    Cost,
    Avoidable,
}

fn main() -> Result<()> {
    let catalog = paths::placing(tokprof_sources::catalog());
    // The default agents are the ones the catalog declares; clap keeps the text for `--help`.
    let names: &'static str = Box::leak(catalog.agents().iter().map(AgentId::name).collect::<Vec<_>>().join(",").into_boxed_str());
    let matches = Cli::command().mut_arg("agent", |a| a.default_value(names)).get_matches();
    let mut cli = Cli::from_arg_matches(&matches).unwrap_or_else(|e| e.exit());
    cli.project = cli.project.as_deref().map(normalize_project);
    let agents = cli.agent.iter().map(|a| catalog.agent(a)).collect::<Result<Vec<_>>>()?;
    let pricing = paths::pricing(cli.prices.as_deref())?;
    let style = fmt::Style::with_color(std::io::stdout().is_terminal() && std::env::var_os("NO_COLOR").is_none());
    let mut command = match cli.command.take() {
        None => Command::Scan { target: None, requests: false },
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
        out!("{}", render_report::prices(&pricing, at));
        return Ok(());
    }
    if let Command::Pool { paths, min_files, by_file, json, where_ } = &command {
        if cli.since.is_some() || cli.until.is_some() || cli.project.is_some() || cli.session.is_some() || cli.client.is_some() {
            bail!("pool takes no filters: each file was filtered when it was written");
        }
        let clauses = where_
            .iter()
            .map(|c| {
                c.strip_prefix("about.")
                    .ok_or_else(|| "give about.<key>=<value>, with the keys role, agents and bother".to_string())
                    .and_then(share::about_answer)
                    .map_err(|e| anyhow::anyhow!("--where {c}: {e}"))
            })
            .collect::<Result<Vec<_>>>()?;
        let mut files = read_share_files(paths, catalog.home())?;
        let mut filter = None;
        if !clauses.is_empty() {
            let text: Vec<String> = clauses.iter().map(|(k, a)| format!("about.{k}={}", a.join("+"))).collect();
            let kept = pool::keep(&files, &clauses);
            if kept.is_empty() {
                bail!("no share file has {}", text.join(" and "));
            }
            filter = Some(pool::Filter { clauses: text, of: files.len() });
            files = kept;
        }
        let mut p = pool::build(&files, *min_files);
        p.filter = filter;
        if *json {
            outln!("{}", serde_json::to_string_pretty(&p)?);
        } else {
            out!("{}", render_pool::text(&p, *by_file));
        }
        return Ok(());
    }
    if let Command::Share { survey: Some(label), .. } = &command
        && !share::valid_label(label)
    {
        bail!("--survey takes 1 to 40 letters, digits, '.', '_' or '-'");
    }
    // The answers about the person are checked before any log is read.
    let about = match &command {
        Command::Share { about, .. } => share::parse_about(about).map_err(anyhow::Error::msg)?,
        _ => Vec::new(),
    };
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
    // The comparison's checks come before any log is read.
    if let Command::Summary { compare: true } = &command {
        if select.since_ms.is_none() {
            bail!("--compare needs --since: all of history has nothing before it");
        }
        if cli.client.is_some() {
            bail!("--compare cannot filter by client; drop --client");
        }
        compare_window(select.since_ms, select.until_ms, now_ms())?;
    }
    let filtered = cli.project.is_some() || cli.session.is_some() || cli.since.is_some() || cli.until.is_some() || cli.client.is_some();
    let none_found = || if filtered { "no session logs found for these filters" } else { "no session logs found" };
    match &command {
        Command::Scan { target, requests } => {
            let file = match target {
                Some(spec) => xray::find(spec, &catalog, &select)?,
                None => xray::latest(&catalog, &select).context(none_found())?,
            };
            let x = xray::xray(&catalog, &file, &pricing, now_ms())?;
            if *requests {
                out!("{}", render_requests::text(&x, &style));
                return Ok(());
            }
            out!("{}", render_xray::text(&x, &style, &paths::profile_dir(), catalog.home()));
            return Ok(());
        }
        Command::Sessions { limit, cost } => {
            let rows = xray::rows(&catalog, &select, *limit);
            let now = now_ms();
            let glances: Vec<Option<xray::Glance>> =
                if *cost { xray::glances(&catalog, &rows, &pricing, now).into_iter().map(Some).collect() } else { vec![None; rows.len()] };
            out!("{}", render_sessions::sessions(&rows, &glances, *cost, &style, now));
            return Ok(());
        }
        Command::Handoff { target, note } => {
            let file = match target {
                Some(spec) => xray::find(spec, &catalog, &select)?,
                None => xray::latest(&catalog, &select).context(none_found())?,
            };
            let text = if *note { handoff::with_note(&catalog, &file)? } else { handoff::line(&catalog, &file) };
            outln!("{text}");
            if std::io::stdout().is_terminal() {
                let done = match clipboard::copy(&text) {
                    Some(_) => "copied to the clipboard: paste it into a new session".to_string(),
                    None => format!("no clipboard tool found ({}): copy the text above", clipboard::TOOLS),
                };
                eprintln!("{}", style.dim(&done));
            }
            return Ok(());
        }
        Command::Ui { dump, waiting } => {
            let size = match dump {
                Some(d) => {
                    let size = d.split_once('x').and_then(|(w, h)| Some((w.parse().ok()?, h.parse().ok()?)));
                    Some(size.context("--dump takes WIDTHxHEIGHT")?)
                }
                None => None,
            };
            return ui::run(std::sync::Arc::new(catalog), pricing, size, select, *waiting);
        }
        _ => {}
    }
    if let Command::Day { date } = &command {
        day_takes_no_time_filter(&cli)?;
        let date = date.as_deref().map(|d| parse_date(d).map(|ms| rhythm::date_of(ms + 12 * 3_600_000, &rhythm::local_offset))).transpose()?;
        let day = day::build(&catalog, &select, &pricing, date, now_ms(), rhythm::Layout::TERMINAL, None, &style, None)?;
        eprintln!("{}", day.read);
        for line in &day.warnings {
            eprintln!("{line}");
        }
        out!("{}", day.text);
        return Ok(());
    }
    if let Command::Calendar { metric } = &command {
        if cli.client.is_some() {
            bail!("calendar cannot filter by client; drop --client");
        }
        let started = Instant::now();
        let path = paths::day_store();
        let shown = std::io::stderr().is_terminal();
        let last = std::sync::Mutex::new(Instant::now());
        let progress = |done: usize, total: usize| {
            let mut at = last.lock().unwrap_or_else(|e| e.into_inner());
            if shown && (at.elapsed().as_millis() >= 250 || done == total) {
                *at = Instant::now();
                eprint!("\rreading the logs into the store… {done} of {total} logs");
                if done == total {
                    eprint!("\r\x1b[2K");
                }
            }
        };
        let (store, refreshed) = calendar::update(&catalog, &agents, &pricing, &path, progress)?;
        eprintln!("{}", calendar::read_line(&catalog, &refreshed, started.elapsed().as_secs_f64(), &path));
        warn(&refreshed.unreadable);
        out!("{}", render_calendar::text(&calendar::compute(&store, &select, now_ms()), *metric, &style));
        // What went wrong with the store comes after the calendar, which it did not stop.
        for w in &refreshed.warnings {
            eprintln!("{w}");
        }
        return Ok(());
    }
    if let Command::Month { month } = &command {
        if cli.client.is_some() {
            bail!("month cannot filter by client; drop --client");
        }
        if cli.since.is_some() || cli.until.is_some() {
            bail!("month picks its time by YYYY-MM; drop --since and --until");
        }
        let now = now_ms();
        let this = tokprof_core::month::Ym::of_date(rhythm::date_of(now, &rhythm::local_offset));
        let ym = pick_month(month.as_deref(), this)?;
        let started = Instant::now();
        let path = paths::day_store();
        let (store, refreshed) = calendar::update(&catalog, &agents, &pricing, &path, |_, _| {})?;
        eprintln!("{}", calendar::read_line(&catalog, &refreshed, started.elapsed().as_secs_f64(), &path));
        warn(&refreshed.unreadable);
        out!("{}", render_month::text(&calendar::month(&store, &select, ym, now), &style));
        for w in &refreshed.warnings {
            eprintln!("{w}");
        }
        return Ok(());
    }
    if let Command::Year { year } = &command {
        if cli.client.is_some() {
            bail!("year cannot filter by client; drop --client");
        }
        if cli.since.is_some() || cli.until.is_some() {
            bail!("year picks its time by YYYY; drop --since and --until");
        }
        let now = now_ms();
        let this = tokprof_core::month::Ym::of_date(rhythm::date_of(now, &rhythm::local_offset)).year;
        let asked = pick_year(year.as_deref(), this)?;
        let started = Instant::now();
        let path = paths::day_store();
        let (store, refreshed) = calendar::update(&catalog, &agents, &pricing, &path, |_, _| {})?;
        eprintln!("{}", calendar::read_line(&catalog, &refreshed, started.elapsed().as_secs_f64(), &path));
        warn(&refreshed.unreadable);
        let shown = asked.unwrap_or_else(|| calendar::default_year(&store, &select, now));
        out!("{}", render_year::text(&calendar::year(&store, &select, shown, now), &style));
        for w in &refreshed.warnings {
            eprintln!("{w}");
        }
        return Ok(());
    }
    if let Command::Bench { requests, min_context, keep_warm, svg } = &command {
        let started = Instant::now();
        // Returns happen in main conversations, so subagent logs are left out.
        let files: Vec<SessionRef> = catalog.discover(&agents).into_iter().filter(|f| !f.subagent && select.holds(f)).collect();
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
        out!("{}", render_bench::report(&b, &style));
        if let Some(path) = svg {
            for written in render_bench::write_svg(&b, path)? {
                outln!("wrote {}", display_path(&written.to_string_lossy(), None, catalog.home()));
            }
        }
        return Ok(());
    }
    if let Command::Pauses = &command {
        let started = Instant::now();
        // A session's subagent logs go with it.
        let files = report_logs(&catalog, &select);
        if files.is_empty() {
            bail!("no session logs found for {}", cli.agent.join(", "));
        }
        let temp = temp_folders();
        // Busy time needs every log of a session, which the scan reads one by one: each log gives
        // its parts, keyed by agent and session id, and the parts of a session are painted at the end.
        type Busy = std::collections::BTreeMap<(AgentId, String), tokprof_core::busy::Parts>;
        let busy_window = window_of(&filter);
        let ((mut p, timelines, busy), stats) = scan(
            &catalog,
            &files,
            &select,
            OnUnreadable::Skip,
            || (Pauses::new(filter.clone()), Vec::new(), Busy::new()),
            |(p, timelines, busy): &mut (Pauses, Vec<Timeline>, Busy), session, f| {
                analysis::analyze(session, &pricing, &filter, p);
                let kept = keeps_client(session, &filter);
                if !f.subagent && kept {
                    timelines.push(Timeline::of(session, &temp));
                }
                let parts = if f.subagent {
                    Some(tokprof_core::busy::Parts::of_subagent_in(session, busy_window))
                } else {
                    kept.then(|| tokprof_core::busy::Parts::of_main_in(session, busy_window))
                };
                if let Some(parts) = parts {
                    busy.entry((session.agent.clone(), session.id.clone())).or_default().add(parts);
                }
            },
            |mut a, b| {
                a.0.merge(b.0);
                a.1.extend(b.1);
                for (k, parts) in b.2 {
                    a.2.entry(k).or_default().add(parts);
                }
                a
            },
        )?;
        p.finish();
        for ((agent, _), parts) in busy {
            if let (true, Some(a)) = (parts.has_main, p.agents.get_mut(&agent)) {
                a.busy.merge(parts.finish());
            }
        }
        eprintln!("{}", sources(&catalog, &files, started.elapsed().as_secs_f64()));
        warn(&stats.unreadable);
        out!("{}", render_report::pauses(&p));
        let offset = &rhythm::local_offset;
        let view = View::new(timelines, filter.until_ms.unwrap_or_else(now_ms), offset);
        let window = (filter.since_ms.unwrap_or(i64::MIN), filter.until_ms.unwrap_or(i64::MAX));
        let returns = rhythm::Returns::within(&p, window);
        out!("{}", render_rhythm::section(&view, window, offset, &returns, &style));
        return Ok(());
    }
    if let Command::Share { output, survey, mcp_names, dollars, .. } = &command {
        let started = Instant::now();
        // A session's subagent logs go with it.
        let files = report_logs(&catalog, &select);
        if files.is_empty() {
            bail!("no session logs found for {}", cli.agent.join(", "));
        }
        // The busy time of each session is painted from all its logs at the end, as for
        // `tokprof pauses`, inside the same window.
        type Busy = std::collections::BTreeMap<(AgentId, String), tokprof_core::busy::Parts>;
        let busy_window = window_of(&filter);
        // The rhythm reads the main sessions as `tokprof pauses` does, each with the client of its
        // first main request, and the bench scans them as `tokprof bench` does.
        type Rhythm = (Vec<(String, Timeline)>, bench::Found);
        let temp = temp_folders();
        let ((sink, busy, (timelines, found)), stats) = scan(
            &catalog,
            &files,
            &select,
            OnUnreadable::Skip,
            || ((Collector::new(None, None), (Pauses::new(filter.clone()), Share::new(filter.clone()))), Busy::new(), Rhythm::default()),
            |(t, busy, (timelines, found)): &mut (_, Busy, Rhythm), session, f| {
                analysis::analyze(session, &pricing, &filter, t);
                let kept = keeps_client(session, &filter);
                if !f.subagent {
                    if kept {
                        timelines.push((session.first_client().to_string(), Timeline::of(session, &temp)));
                    }
                    *found = std::mem::take(found).merge(bench::scan(session, &filter, bench::DEFAULT_REQUESTS, bench::DEFAULT_MIN_CONTEXT));
                }
                let parts = if f.subagent {
                    Some(tokprof_core::busy::Parts::of_subagent_in(session, busy_window))
                } else {
                    kept.then(|| tokprof_core::busy::Parts::of_main_in(session, busy_window))
                };
                if let Some(parts) = parts {
                    busy.entry((session.agent.clone(), session.id.clone())).or_default().add(parts);
                }
            },
            |mut a, b| {
                a.0.0.merge(b.0.0);
                a.0.1.0.merge(b.0.1.0);
                a.0.1.1.merge(b.0.1.1);
                for (k, parts) in b.1 {
                    a.1.entry(k).or_default().add(parts);
                }
                a.2.0.extend(b.2.0);
                a.2.1 = std::mem::take(&mut a.2.1).merge(b.2.1);
                a
            },
        )?;
        let (collector, (mut p, s)) = sink;
        p.finish();
        for ((agent, _), parts) in busy {
            if let (true, Some(a)) = (parts.has_main, p.agents.get_mut(&agent)) {
                a.busy.merge(parts.finish());
            }
        }
        eprintln!("{}", sources(&catalog, &files, started.elapsed().as_secs_f64()));
        warn(&stats.unreadable);
        if s.span_ms.is_none() {
            bail!("no requests found for these filters; nothing written");
        }
        let platform = format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH);
        let header = share::Header { tokprof: env!("CARGO_PKG_VERSION"), platform: &platform, survey: survey.as_deref(), mcp_names, about: &about };
        // The rhythm of all the main sessions together and of each agent's clients, over the
        // window of the filters, and the bench with its defaults.
        let offset = &rhythm::local_offset;
        let now = filter.until_ms.unwrap_or_else(now_ms);
        let window = (filter.since_ms.unwrap_or(i64::MIN), filter.until_ms.unwrap_or(i64::MAX));
        let agent_rhythm = share::agent_rhythms(&timelines, now, offset, window);
        let left_out = share::left_out_sessions(&timelines);
        let (all, client_rhythm) = share::rhythms(timelines, now, offset, window);
        let extra = share::Extra {
            rhythm: Some(all),
            client_rhythm,
            agent_rhythm,
            left_out,
            bench: Some(bench::replay(found, &pricing, bench::DEFAULT_REQUESTS, bench::DEFAULT_MIN_CONTEXT, bench::DEFAULT_KEEP_WARM_MS)),
        };
        let value = share::to_json_with(&collector, &p, &s, &header, &extra, |ms| fmt::local_time(ms, "%Y-%m-%d"));
        // Before anything is written, the words that tokprof did not make are searched for this
        // machine's names.
        let words = share::words(&value, &share::own_words(&catalog.agents()));
        let home = catalog.home().to_string_lossy().to_string();
        let user = catalog.home().file_name().map(|n| n.to_string_lossy().to_string());
        let host = host_name();
        let secrets = share::Secrets { home: &home, user: user.as_deref(), host: host.as_deref(), ids: &s.ids, titles: &s.titles };
        if let Some((what, field)) = share::leak(&words, &secrets) {
            bail!("the file would hold {what} in {field}; nothing written");
        }
        let path = output.clone().unwrap_or_else(|| paths::profile_dir().join(format!("share-{}.json", chrono::Local::now().format("%Y-%m-%d"))));
        if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
            std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
        }
        std::fs::write(&path, share::pretty(&share::as_shares(&value, *dollars))).with_context(|| format!("writing {}", path.display()))?;
        outln!("wrote {}", display_path(&path.to_string_lossy(), None, catalog.home()));
        let name = path.file_stem().map_or_else(|| "share".to_string(), |n| n.to_string_lossy().to_string());
        let (usd, tokens) = share::totals(&value);
        out!("\n{}", render_pool::text(&pool::build(&[(name, value)], 1), false));
        outln!("{}", share::totals_line(usd, tokens, *dollars));
        outln!("Nothing was sent anywhere: review the file, then send it yourself.");
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
    // A session's subagent logs go with it.
    let files = report_logs(&catalog, &select);
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
        | Command::Calendar { .. }
        | Command::Month { .. }
        | Command::Year { .. }
        | Command::Pauses
        | Command::Share { .. }
        | Command::Pool { .. } => {
            unreachable!("handled before reading logs")
        }
        Command::Summary { compare } => {
            let mut text = render_report::summary(&collector, &filter, &paths::user_prices());
            if compare {
                let line = compare_line(&catalog, &select, &pricing, &collector)?;
                text = with_line_after(&text, "Total priced: ", &line);
            }
            out!("{text}");
        }
        Command::Misses { top, expected } => out!("{}", render_report::misses(&collector, top, expected)),
        Command::Whatif => out!("{}", render_report::whatif(&collector, &catalog.agents())),
        Command::Tools { by } => {
            let order = match by {
                By::Rent => tokprof_core::tools::RowOrder::Rent,
                By::Trips => tokprof_core::tools::RowOrder::Trips,
                By::Failed => tokprof_core::tools::RowOrder::Failed,
            };
            out!("{}", render_report::tools(&collector.tools, order, 25))
        }
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
                outln!(
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
            outln!("wrote {written} to {shown}");
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

/// Writes to stdout. Once its reader went away, as `head` does after its lines, nothing more is
/// written and the command goes on quietly to its end: what it does besides printing, such as
/// writing a file, still happens, and it ends as it would have. Any other failure panics, as
/// `print!` does. SIGPIPE stays ignored, as Rust leaves it: the terminal UI and the clipboard's
/// child share this process.
fn write_out(args: std::fmt::Arguments) {
    use std::io::Write;
    use std::sync::atomic::{AtomicBool, Ordering};
    static CLOSED: AtomicBool = AtomicBool::new(false);
    if CLOSED.load(Ordering::Relaxed) {
        return;
    }
    let mut stdout = std::io::stdout().lock();
    match stdout.write_fmt(args).and_then(|()| stdout.flush()) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::BrokenPipe => CLOSED.store(true, Ordering::Relaxed),
        Err(e) => panic!("failed printing to stdout: {e}"),
    }
}

/// The first logs that could not be read, then how many more there were.
fn warn(errors: &[String]) {
    for line in warnings(errors) {
        eprintln!("{line}");
    }
}

/// `day` picks its time by its date, so the time filters would contradict it.
/// The share files among the paths: the files, and the `.json` files of the folders, not of
/// their subfolders. A file that is not a share file, one of a newer format, and one with the
/// same content as a file read before are left out, with a line on stderr each.
fn read_share_files(paths: &[PathBuf], home: &Path) -> Result<Vec<(String, serde_json::Value)>> {
    let shown = |p: &Path| display_path(&p.to_string_lossy(), None, home);
    let mut candidates = Vec::new();
    for path in paths {
        if path.is_dir() {
            let mut inside: Vec<PathBuf> = std::fs::read_dir(path)
                .with_context(|| format!("reading {}", shown(path)))?
                .filter_map(|e| e.ok().map(|e| e.path()))
                .filter(|p| p.is_file() && p.extension().is_some_and(|x| x == "json"))
                .collect();
            // By name without `.json`, so `a.json` comes before the copy `a (1).json`.
            inside.sort_by(|x, y| x.file_stem().cmp(&y.file_stem()).then_with(|| x.cmp(y)));
            candidates.extend(inside);
        } else {
            candidates.push(path.clone());
        }
    }
    let mut seen: std::collections::HashMap<String, PathBuf> = std::collections::HashMap::new();
    let mut files = Vec::new();
    for path in candidates {
        let text = std::fs::read_to_string(&path).with_context(|| format!("reading {}", shown(&path)))?;
        if let Some(first) = seen.get(&text) {
            eprintln!("skipped {}: the same as {}", shown(&path), shown(first));
            continue;
        }
        match pool::parse(&text) {
            Ok(v) => {
                seen.insert(text, path.clone());
                let name = path.file_stem().map_or_else(|| shown(&path), |n| n.to_string_lossy().to_string());
                files.push((name, v));
            }
            Err(pool::Skip::NotShare) => eprintln!("skipped {}: not a tokprof share file", shown(&path)),
            Err(pool::Skip::Newer(n)) => eprintln!("skipped {}: format {n} needs a newer tokprof", shown(&path)),
        }
    }
    if files.is_empty() {
        bail!("no share files in {}", paths.iter().map(|p| shown(p)).collect::<Vec<_>>().join(", "));
    }
    Ok(files)
}

/// The window of the global filters, for the busy time of a report; `None` without one.
fn window_of(filter: &analysis::Filter) -> Option<(i64, i64)> {
    (filter.since_ms.is_some() || filter.until_ms.is_some()).then(|| (filter.since_ms.unwrap_or(i64::MIN), filter.until_ms.unwrap_or(i64::MAX)))
}

/// This machine's host name, for the leak check of `tokprof share`.
fn host_name() -> Option<String> {
    if let Some(name) = std::env::var_os("COMPUTERNAME") {
        return Some(name.to_string_lossy().to_string());
    }
    let out = std::process::Command::new("hostname").output().ok()?;
    let name = String::from_utf8_lossy(&out.stdout).trim().to_string();
    (out.status.success() && !name.is_empty()).then_some(name)
}

/// The summary's `compared with` line: the store brought up to date, and the window of the same
/// length before the summary's totalled from it.
/// The window `--compare` compares, checked before any log is read: `--since` before `--until`
/// (or now), and a window before it that dates can hold.
fn compare_window(since: Option<i64>, until: Option<i64>, now: i64) -> Result<(i64, i64)> {
    let from = since.context("--compare needs --since: all of history has nothing before it")?;
    let to = until.map_or(now, |u| u.min(now));
    if to <= from {
        bail!("--compare needs --since before --until and before now");
    }
    if tokprof_core::compare::try_before((from, to)).is_none() {
        bail!("--compare cannot go back that far: the window before --since is out of range");
    }
    Ok((from, to))
}

fn compare_line(catalog: &tokprof_core::catalog::Catalog, select: &Query, pricing: &tokprof_core::pricing::Pricing, c: &Collector) -> Result<String> {
    let (from, to) = compare_window(select.since_ms, select.until_ms, now_ms())?;
    let started = Instant::now();
    let path = paths::day_store();
    let (store, refreshed) = tokprof_browse::calendar::update(catalog, &select.agents, pricing, &path, |_, _| {})?;
    // What the store read, and what went wrong with it, as the calendar says it.
    eprintln!("{}", calendar::read_line(catalog, &refreshed, started.elapsed().as_secs_f64(), &path));
    warn(&refreshed.unreadable);
    for w in &refreshed.warnings {
        eprintln!("{w}");
    }
    let before = tokprof_core::compare::Totals::before(&store.entries, &tokprof_browse::calendar::filters(select), (from, to));
    // The causes the summary's table lists, as their names and dollars.
    let mut shown: std::collections::HashMap<&str, f64> = std::collections::HashMap::new();
    for m in c.misses.iter().filter(|m| !m.cause.expected()) {
        *shown.entry(m.cause.name()).or_default() += m.premium_usd.unwrap_or(0.0);
    }
    let gone = tokprof_core::compare::gone(&before, |name| shown.get(name).copied().unwrap_or(0.0));
    Ok(tokprof_core::compare::summary_line(c.spend.priced_usd(), c.avoidable_usd(), &before, &gone))
}

/// `text` with `line` after its first line that starts with `start`.
fn with_line_after(text: &str, start: &str, line: &str) -> String {
    let mut out = String::with_capacity(text.len() + line.len() + 1);
    let mut done = false;
    for l in text.split_inclusive('\n') {
        out.push_str(l);
        if !done && l.starts_with(start) {
            out.push_str(line);
            out.push('\n');
            done = true;
        }
    }
    out
}

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

/// The month `tokprof month` shows: the last complete one by default, `this` so far, or a
/// YYYY-MM that has begun.
fn pick_month(arg: Option<&str>, this: tokprof_core::month::Ym) -> Result<tokprof_core::month::Ym> {
    let ym = match arg {
        None => this.previous(),
        Some("this") => this,
        Some(m) => tokprof_core::month::Ym::parse(m).with_context(|| format!("bad month {m:?}, expected YYYY-MM or this"))?,
    };
    if ym > this {
        bail!("{} has not begun; tokprof month this shows {} so far", ym.name(), this.name());
    }
    Ok(ym)
}

/// The year `tokprof year` was asked for: `this`, or a YYYY that has begun; `None` for the
/// default, which the store decides (see `tokprof_core::year::default_year`).
fn pick_year(arg: Option<&str>, this: i32) -> Result<Option<i32>> {
    let year = match arg {
        None => return Ok(None),
        Some("this") => this,
        Some(y) if y.len() == 4 && y.bytes().all(|b| b.is_ascii_digit()) => y.parse::<i32>()?,
        Some(y) => bail!("bad year {y:?}, expected YYYY or this"),
    };
    if year > this {
        bail!("{year} has not begun; tokprof year this shows {this} so far");
    }
    Ok(Some(year))
}

/// A metric of `tokprof calendar`.
fn parse_metric(s: &str) -> Result<Metric, String> {
    Metric::parse(s).ok_or_else(|| format!("unknown metric {s:?}; use {}", Metric::NAMES))
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

    #[test]
    fn the_comparison_follows_the_total_priced_line_and_nothing_else_moves() {
        let text = "Spend by model\n\nTotal priced: $5   avoidable on cache misses: $1 (20.0%)\nClaude Code's own cost counter\n";
        assert_eq!(
            with_line_after(text, "Total priced: ", "compared with x: no earlier data"),
            "Spend by model\n\nTotal priced: $5   avoidable on cache misses: $1 (20.0%)\ncompared with x: no earlier data\nClaude Code's own cost counter\n"
        );
        assert_eq!(with_line_after(text, "Nothing: ", "x"), text);
    }

    #[test]
    fn compare_needs_a_window_with_a_start_before_its_end() {
        let day = 86_400_000;
        assert!(compare_window(None, None, 10 * day).unwrap_err().to_string().starts_with("--compare needs --since"));
        assert_eq!(compare_window(Some(5 * day), Some(day), 10 * day).unwrap_err().to_string(), "--compare needs --since before --until and before now");
        assert!(compare_window(Some(11 * day), None, 10 * day).is_err(), "a start after now");
        assert!(compare_window(Some(i64::MIN + 5), Some(i64::MAX - 5), i64::MAX).unwrap_err().to_string().contains("out of range"));
        assert_eq!(compare_window(Some(day), Some(5 * day), 10 * day).unwrap(), (day, 5 * day));
    }

    #[test]
    fn compare_needs_a_start_and_no_client() {
        let cli = |args: &[&str]| Cli::try_parse_from(args).unwrap();
        assert!(matches!(cli(&["tokprof", "summary", "--compare"]).command, Some(Command::Summary { compare: true })));
        assert!(matches!(cli(&["tokprof", "summary"]).command, Some(Command::Summary { compare: false })));
    }

    fn name(args: &[&str], value: Value) -> String {
        default_name(&Cli::parse_from(std::iter::once("tokprof").chain(args.iter().copied())), value)
    }

    #[test]
    fn the_bench_defaults_are_the_bench_constants() {
        let cli = Cli::parse_from(["tokprof", "bench"]);
        let Some(Command::Bench { requests, min_context, keep_warm, .. }) = cli.command else { panic!("not the bench") };
        assert_eq!((requests, min_context, keep_warm), (bench::DEFAULT_REQUESTS, bench::DEFAULT_MIN_CONTEXT, bench::DEFAULT_KEEP_WARM_MS));
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
    fn a_month_that_has_not_begun_is_refused() {
        use tokprof_core::month::Ym;
        let nov = Ym { year: 2026, month: 11 };
        assert_eq!(pick_month(None, nov).unwrap(), Ym { year: 2026, month: 10 }, "the last complete month");
        assert_eq!(pick_month(Some("this"), nov).unwrap(), nov);
        let err = pick_month(Some("2027-05"), nov).unwrap_err().to_string();
        assert_eq!(err, "May 2027 has not begun; tokprof month this shows November 2026 so far");
        assert!(pick_month(Some("october"), nov).is_err());
    }

    #[test]
    fn a_year_that_has_not_begun_is_refused() {
        assert_eq!(pick_year(None, 2026).unwrap(), None, "the store decides the default");
        assert_eq!(pick_year(Some("this"), 2026).unwrap(), Some(2026));
        assert_eq!(pick_year(Some("2025"), 2026).unwrap(), Some(2025));
        assert_eq!(pick_year(Some("2027"), 2026).unwrap_err().to_string(), "2027 has not begun; tokprof year this shows 2026 so far");
        for bad in ["26", "2026-11", "+202", "twenty"] {
            assert_eq!(pick_year(Some(bad), 2026).unwrap_err().to_string(), format!("bad year {bad:?}, expected YYYY or this"));
        }
    }

    #[test]
    fn calendar_metrics_by_name() {
        let Some(Command::Calendar { metric }) = Cli::parse_from(["tokprof", "calendar"]).command else { panic!("not the calendar") };
        assert_eq!(metric, Metric::Spend);
        let Some(Command::Calendar { metric }) = Cli::parse_from(["tokprof", "calendar", "--metric", "keyboard"]).command else { panic!() };
        assert_eq!(metric, Metric::Keyboard);
        let err = Cli::try_parse_from(["tokprof", "calendar", "--metric", "tokens"]).err().unwrap().to_string();
        assert!(err.contains("spend, agent, keyboard, messages, commits, avoidable"), "{err}");
    }

    #[test]
    fn day_fails_on_time_filters() {
        let err = day_takes_no_time_filter(&Cli::parse_from(["tokprof", "--since", "7d", "day"])).unwrap_err();
        assert_eq!(err.to_string(), "day picks its time by DATE; drop --since and --until");
        assert!(day_takes_no_time_filter(&Cli::parse_from(["tokprof", "--until", "2026-11-01", "day", "2026-10-30"])).is_err());
        assert!(day_takes_no_time_filter(&Cli::parse_from(["tokprof", "--project", "billing", "day"])).is_ok());
    }
}
