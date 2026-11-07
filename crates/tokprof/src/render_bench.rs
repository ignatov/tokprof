//! The text and the charts of `tokprof bench`.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use tokprof_core::bench::{Bench, PING_OUTPUT, Return, Strategy, break_even, mean_curve, median, ping_every, window_name};
use tokprof_core::fmt::{self, Style, human_duration, table, tokens, usd};
use tokprof_core::forecast::SUMMARY_TOKENS;
use tokprof_core::source::AgentId;

/// The request counts the table shows: the 1st, the 10th when there are more, and the Nth.
fn table_marks(n: usize) -> Vec<usize> {
    let mut marks = vec![1, 10, n];
    marks.retain(|&k| k <= n);
    marks.dedup();
    marks
}

pub fn report(b: &Bench, style: &Style) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "tokprof bench · coming back to a big session after its prompt cache expired\n");
    if b.returns.is_empty() {
        let _ = writeln!(out, "No cold returns to contexts of {}+ tokens in these logs.", round_tokens(b.min_context));
        return out;
    }
    let n = b.requests;
    for agent in b.agents() {
        let returns = b.of(&agent);
        if returns.is_empty() {
            continue;
        }
        let ctx = median(returns.iter().map(|r| r.context).collect()).unwrap_or(0);
        let idle = median(returns.iter().map(|r| r.idle_ms as u64).collect()).unwrap_or(0);
        let len = median(returns.iter().map(|r| r.requests() as u64).collect()).unwrap_or(0);
        let _ = writeln!(
            out,
            "{} {} returns to contexts of {}+ tokens (median {}) after a break of {} (median);",
            style.bold(&format!("{:<6}", agent.name())),
            returns.len(),
            round_tokens(b.min_context),
            tokens(ctx as f64),
            human_duration(idle as f64 / 1000.0),
        );
        let _ = writeln!(out, "       each followed for up to {n} requests, until the next break or compaction (median {len})\n");
        let marks = table_marks(n);
        let mut header = vec!["mean per return".to_string()];
        header.extend(marks.iter().map(|&k| if k == 1 { "1st request".to_string() } else { format!("{k} requests") }));
        header.push("all returns".to_string());
        let mut rows = vec![header];
        for s in Strategy::ALL {
            let mut row = vec![s.name().to_string()];
            row.extend(marks.iter().map(|&k| usd(returns.iter().map(|r| r.after(s, k)).sum::<f64>() / returns.len() as f64)));
            row.push(usd(returns.iter().map(|r| r.after(s, n)).sum()));
            rows.push(row);
        }
        for line in table(&rows, 1).lines() {
            let _ = writeln!(out, "  {line}");
        }
        let total = |s| returns.iter().map(|r| r.after(s, n)).sum::<f64>();
        let (resume, compact, handoff) = (total(Strategy::Resume), total(Strategy::Compact), total(Strategy::Handoff));
        let _ = writeln!(
            out,
            "\n  A handoff note before the break would have saved {} of {} ({:.0}%).",
            usd(resume - handoff),
            usd(resume),
            100.0 * (resume - handoff) / resume.max(1e-9)
        );
        let mut saved: Vec<(f64, f64)> = returns
            .iter()
            .map(|r| {
                (r.after(Strategy::Resume, n) - r.after(Strategy::Handoff, n), 1.0 - r.after(Strategy::Handoff, n) / r.after(Strategy::Resume, n).max(1e-9))
            })
            .collect();
        let cheaper = saved.iter().filter(|(usd, _)| *usd > 0.0).count();
        saved.sort_by(|a, b| a.0.total_cmp(&b.0));
        let (median_usd, _) = saved[saved.len() / 2];
        saved.sort_by(|a, b| a.1.total_cmp(&b.1));
        let (_, median_share) = saved[saved.len() / 2];
        let _ = writeln!(
            out,
            "  Per return it saves a median of {} ({:.0}%), and it is cheaper in {cheaper} of {} returns.",
            usd(median_usd),
            100.0 * median_share,
            returns.len()
        );
        match break_even(&returns, Strategy::Compact, Strategy::Resume, n) {
            Some(k) => {
                let _ = writeln!(
                    out,
                    "  /compact first costs more up front and is cheaper than resuming from request {k} on ({} over all).",
                    usd(resume - compact)
                );
            }
            None => {
                let _ = writeln!(out, "  /compact first does not pay off within {n} requests.");
            }
        }
        let heartbeats = total(Strategy::Heartbeats);
        let warm = returns.iter().filter(|r| r.kept_warm).count();
        let pinged: f64 = returns.iter().map(|r| r.after(Strategy::Heartbeats, 0)).sum();
        let change = if heartbeats <= resume {
            format!("saved {} of {} ({:.0}%)", usd(resume - heartbeats), usd(resume), 100.0 * (resume - heartbeats) / resume.max(1e-9))
        } else {
            format!("cost {} more than resuming", usd(heartbeats - resume))
        };
        let _ = writeln!(
            out,
            "  Heartbeats for up to {} would have kept {warm} of {} returns warm, for {} of pings, and {change}.",
            window_name(b.keep_warm_ms),
            returns.len(),
            usd(pinged)
        );
        let below_note = returns.iter().filter(|r| r.after(Strategy::Heartbeats, n) < r.after(Strategy::Handoff, n)).count();
        let _ = writeln!(out, "  They cost less than a handoff note in {below_note} of {} returns.", returns.len());
        // What the heartbeat ways also send during the breaks that were no cold return.
        let elsewhere = b.elsewhere.get(&agent).copied().unwrap_or_default();
        let net = |cost: f64, subject: &str| {
            if cost <= resume {
                format!(
                    "{subject} {} {} ({:.0}%)",
                    if subject == "it" { "saves" } else { "save" },
                    usd(resume - cost),
                    100.0 * (resume - cost) / resume.max(1e-9)
                )
            } else {
                format!("{subject} {} {} more than resuming", if subject == "it" { "costs" } else { "cost" }, usd(cost - resume))
            }
        };
        let _ = writeln!(
            out,
            "  Breaks that were not cold returns would have taken another {} for {}; with them, {}.",
            fmt::count(elsewhere.pings as u64, "ping", "pings"),
            usd(elsewhere.ping_usd),
            net(heartbeats + elsewhere.ping_usd, "heartbeats")
        );
        let then_compact = total(Strategy::WarmThenCompact);
        let compacted = returns.iter().filter(|r| r.compacted).count();
        let sent: f64 = returns.iter().map(|r| r.after(Strategy::WarmThenCompact, 0)).sum();
        let change = if then_compact <= resume {
            format!("saved {} of {} ({:.0}%)", usd(resume - then_compact), usd(resume), 100.0 * (resume - then_compact) / resume.max(1e-9))
        } else {
            format!("cost {} more than resuming", usd(then_compact - resume))
        };
        let _ = writeln!(
            out,
            "  Heartbeats for up to {}, then a compaction, would have kept {} of {} returns warm and compacted {compacted}, for {} of pings and compactions, and {change}.",
            window_name(b.keep_warm_ms),
            returns.len() - compacted,
            returns.len(),
            usd(sent)
        );
        let _ = writeln!(
            out,
            "  Breaks that were not cold returns would have taken another {} and {} for {}; with them, {}.",
            fmt::count(elsewhere.pings as u64, "ping", "pings"),
            fmt::count(elsewhere.compactions as u64, "compaction", "compactions"),
            usd(elsewhere.ping_usd + elsewhere.compaction_usd),
            net(then_compact + elsewhere.ping_usd + elsewhere.compaction_usd, "it")
        );
        let errors: Vec<f64> = returns.iter().map(|r| (r.forecast - r.after(Strategy::Resume, 1)).abs() / r.after(Strategy::Resume, 1).max(1e-9)).collect();
        let within = errors.iter().filter(|e| **e <= 0.1).count();
        let _ = writeln!(
            out,
            "  The X-ray's forecast of the first request was within 10% of its logged cost for {within} of {} returns ({:.0}%).\n",
            errors.len(),
            100.0 * within as f64 / errors.len() as f64
        );
    }
    let bases: Vec<String> = b
        .base
        .iter()
        .filter(|(a, _)| !b.of(a).is_empty())
        .map(|(a, base)| match base.compactions {
            0 => format!("{} {} (first prompt + note)", a.name(), tokens(base.tokens as f64)),
            c => format!("{} {} (median of {c} compactions)", a.name(), tokens(base.tokens as f64)),
        })
        .collect();
    let _ = writeln!(out, "{}", style.dim(&format!("The alternatives start from what a compaction leaves: {}.", bases.join(", "))));
    let _ = writeln!(
        out,
        "{}",
        style.dim(&format!(
            "A compaction re-writes the cold context and outputs {} tokens; a handoff note is {} tokens written while the cache was warm.",
            tokens(SUMMARY_TOKENS),
            tokens(SUMMARY_TOKENS)
        ))
    );
    let _ = writeln!(
        out,
        "{}",
        style.dim(&format!(
            "A heartbeat goes out at 5/6 of the cache lifetime, every {} on a 1h cache, reads the context and outputs {} tokens{}.",
            human_duration(ping_every(3_600_000) as f64 / 1000.0),
            PING_OUTPUT,
            assumption(b)
        ))
    );
    let summary = tokens(SUMMARY_TOKENS);
    let warm = match (compaction_titles(b, true), compaction_titles(b, false)) {
        (_, writes) if writes.is_empty() => format!("a compaction that reads the warm context and outputs {summary} tokens."),
        (reads, writes) if reads.is_empty() => format!(
            "a compaction while the cache is warm that outputs {summary} tokens: it writes the context again in {writes}, whose compaction calls read nothing from the cache."
        ),
        (reads, writes) => format!(
            "a compaction while the cache is warm that outputs {summary} tokens: it reads the context in {reads} and writes it again in {writes}, whose compaction calls read nothing from the cache."
        ),
    };
    let _ = writeln!(out, "{}", style.dim(&format!("Heartbeats, then compaction sends the same pings and, at the next ping time past the window, {warm}")));
    out
}

/// Colors of one chart theme.
struct Theme {
    surface: &'static str,
    grid: &'static str,
    axis: &'static str,
    ink: &'static str,
    ink2: &'static str,
    muted: &'static str,
    series: [&'static str; 5],
}

// The categorical slots 1–5 of the dataviz palette, in its order: blue, orange, aqua, yellow,
// magenta. Both sets pass its validator, adjacent pairs included.
const LIGHT: Theme = Theme {
    surface: "#fcfcfb",
    grid: "#e1e0d9",
    axis: "#c3c2b7",
    ink: "#0b0b0b",
    ink2: "#52514e",
    muted: "#898781",
    series: ["#2a78d6", "#eb6834", "#1baf7a", "#eda100", "#e87ba4"],
};

const DARK: Theme = Theme {
    surface: "#1a1a19",
    grid: "#2c2c2a",
    axis: "#383835",
    ink: "#f0efec",
    ink2: "#c3c2b7",
    muted: "#898781",
    series: ["#3987e5", "#d95926", "#199e70", "#c98500", "#d55181"],
};

/// Writes the chart of the mean cost per return to `path`, and a dark-theme version next to it
/// with `-dark` before the extension; returns the files written.
pub fn write_svg(b: &Bench, path: &Path) -> Result<Vec<PathBuf>> {
    let stem = path.file_stem().and_then(|s| s.to_str()).context("the chart needs a file name")?;
    let dark = path.with_file_name(format!("{stem}-dark.svg"));
    if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
        std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
    }
    let mut written = Vec::new();
    for (theme, file) in [(&LIGHT, path.to_path_buf()), (&DARK, dark)] {
        std::fs::write(&file, svg(b, theme)).with_context(|| format!("writing {}", file.display()))?;
        written.push(file);
    }
    Ok(written)
}

/// Tick step and plot top for values up to `max`: a round step with at most five intervals.
fn nice_axis(max: f64) -> (f64, f64) {
    let rough = (max / 5.0).max(1e-9);
    let magnitude = 10f64.powf(rough.log10().floor());
    let step = [1.0, 2.0, 5.0, 10.0].into_iter().map(|m| m * magnitude).find(|s| *s >= rough).unwrap_or(10.0 * magnitude);
    (step, (max * 1.08).max(step))
}

fn money(v: f64, step: f64) -> String {
    if step >= 1.0 { format!("${v:.0}") } else { format!("${v:.2}") }
}

/// Token counts as the chart shows them: 100k, 491k, 1.2M.
fn round_tokens(n: u64) -> String {
    match n {
        n if n >= 1_000_000 => format!("{:.1}M", n as f64 / 1e6),
        n if n >= 1_000 => format!("{:.0}k", n as f64 / 1e3),
        n => n.to_string(),
    }
}

/// Approximate width of a label in the chart's sans at `size` px.
fn text_width(text: &str, size: f64) -> f64 {
    0.5 * size * text.chars().count() as f64
}

/// Which agents' heartbeats rest on an assumption: those on an API that does not document that a
/// read resets the cache lifetime, against those on one that does.
fn assumption(b: &Bench) -> String {
    let titles = |documented: bool| {
        let mut t: Vec<&str> =
            b.returns.iter().filter(|r| r.api.rules().is_some_and(|c| c.read_resets_ttl_documented == documented)).map(|r| agent_title(&r.agent)).collect();
        t.sort();
        t.dedup();
        t.join(" and ")
    };
    let (assumed, documented) = (titles(false), titles(true));
    match (assumed.is_empty(), documented.is_empty()) {
        (true, _) => String::new(),
        (false, true) => format!("; for {assumed} this assumes it resets the cache timer"),
        (false, false) => format!("; for {assumed} this assumes it resets the cache timer as it does in {documented}"),
    }
}

fn agent_title(agent: &AgentId) -> &str {
    &agent.info().title
}

/// The titles of the agents with returns whose compaction call reads the warm cache (`reads`), or
/// writes the context again (not `reads`), joined with "and".
fn compaction_titles(b: &Bench, reads: bool) -> String {
    let mut t: Vec<&str> = b.returns.iter().filter(|r| r.agent.info().harness.compaction_reads_cache == reads).map(|r| agent_title(&r.agent)).collect();
    t.sort();
    t.dedup();
    t.join(" and ")
}

/// The chart's notes, top down: the prices and the compaction model, heartbeats, and heartbeats
/// then compaction. When an agent's compaction call writes the warm context again, a fourth note
/// says in which agents a warm compaction reads the context and in which it writes it.
fn chart_notes(b: &Bench) -> Vec<String> {
    let window = window_name(b.keep_warm_ms);
    let mut notes = vec![
        format!(
            "tokprof bench · requests priced at their time · a compaction call is modeled as writing the cold context again plus a {}-token summary",
            round_tokens(SUMMARY_TOKENS as u64)
        ),
        format!(
            "heartbeats: a ping at 5/6 of the cache lifetime, every {} on a 1h cache, for up to {window} after the last request; a return within the last ping's lifetime reads the cache",
            human_duration(ping_every(3_600_000) as f64 / 1000.0)
        ),
    ];
    let (reads, writes) = (compaction_titles(b, true), compaction_titles(b, false));
    if writes.is_empty() {
        notes.push(format!(
            "heartbeats, then compaction: the same pings, then at the next ping time past {window} a compaction that reads the warm context; a later return starts from what it leaves"
        ));
        return notes;
    }
    notes.push(format!(
        "heartbeats, then compaction: the same pings, then at the next ping time past {window} a compaction while the cache is warm; a later return starts from what it leaves"
    ));
    notes.push(if reads.is_empty() {
        format!("a compaction while the cache is warm writes the context again in {writes}, whose compaction calls read nothing from the cache")
    } else {
        format!("a compaction while the cache is warm reads the context in {reads} and writes it again in {writes}, whose compaction calls read nothing from the cache")
    });
    notes
}

/// One agent's part of the chart: its returns and the mean cost curve of each strategy.
struct Panel<'a> {
    agent: AgentId,
    returns: Vec<&'a Return>,
    curves: [Vec<f64>; 5],
}

fn svg(b: &Bench, t: &Theme) -> String {
    const W: f64 = 880.0;
    const PAD: f64 = 24.0;
    const GAP: f64 = 40.0;
    const ROW: f64 = 20.0;
    const FONT: &str = "system-ui, -apple-system, 'Segoe UI', Helvetica, Arial, sans-serif";
    let n = b.requests;
    let panels: Vec<Panel> = b
        .agents()
        .into_iter()
        .map(|agent| {
            let returns = b.of(&agent);
            (agent, returns)
        })
        .filter(|(_, returns)| !returns.is_empty())
        .map(|(agent, returns)| {
            let curves = Strategy::ALL.map(|s| mean_curve(&returns, s, n));
            Panel { agent, returns, curves }
        })
        .collect();
    let top = panels.iter().flat_map(|p| p.curves.iter().flatten()).fold(0.0f64, |m, v| m.max(*v));
    let (step, ymax) = nice_axis(top);
    // Heartbeats sit next to /compact in hue, so their line is dotted, not dashed; heartbeats
    // then compaction mixes the two, dash-dotted.
    let dash = |s: Strategy| match s {
        Strategy::Resume => "",
        Strategy::Heartbeats => " stroke-dasharray=\"1 4\"",
        Strategy::WarmThenCompact => " stroke-dasharray=\"7 3 1 3\"",
        _ => " stroke-dasharray=\"6 4\"",
    };
    // The legend: one key per strategy, wrapped to the chart's width; the rows below it push
    // the panels down.
    let keys: Vec<(Strategy, String, f64, f64)> = {
        let (mut lx, mut ly) = (PAD, 80.0);
        let mut keys = Vec::new();
        for s in Strategy::ALL {
            let label = format!("{} ({})", s.name(), if s == Strategy::Resume { "as logged" } else { "replayed" });
            let width = 30.0 + text_width(&label, 12.5);
            if lx > PAD && lx + width > W - PAD {
                (lx, ly) = (PAD, ly + ROW);
            }
            keys.push((s, label, lx, ly));
            lx += width + 24.0;
        }
        keys
    };
    let below = keys.last().map_or(0.0, |k| k.3 - 80.0);
    let notes = chart_notes(b);
    // Three notes fit under the panels; each more makes the chart a line taller.
    let h = 456.0 + below + 16.0 * notes.len().saturating_sub(3) as f64;

    let mut desc = String::new();
    for Panel { agent, returns, curves } in &panels {
        let ends: Vec<String> = Strategy::ALL.iter().map(|s| format!("{} {}", s.name(), usd(curves[*s as usize][n]))).collect();
        let _ = write!(desc, "{}: {} returns, after {n} requests {}. ", agent_title(agent), returns.len(), ends.join(", "));
    }

    let mut o = String::new();
    let _ = writeln!(
        o,
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{W}" height="{h}" viewBox="0 0 {W} {h}" role="img" aria-labelledby="title desc" font-family="{FONT}">"#
    );
    let _ = writeln!(o, r#"<title id="title">Coming back to a big session after its prompt cache expired</title>"#);
    let _ = writeln!(o, r#"<desc id="desc">Mean cost per return. {}</desc>"#, desc.trim_end());
    let _ = writeln!(o, "<style>.num{{font-variant-numeric:tabular-nums}} .cross{{opacity:0}} .col:hover .cross{{opacity:1}}</style>");
    let _ = writeln!(o, r#"<rect width="{W}" height="{h}" rx="10" fill="{}"/>"#, t.surface);
    let _ = writeln!(
        o,
        r#"<text x="{PAD}" y="34" font-size="17" font-weight="600" fill="{}">Coming back to a big session after its prompt cache expired</text>"#,
        t.ink
    );
    let _ = writeln!(
        o,
        r#"<text x="{PAD}" y="56" font-size="12.5" fill="{}">Mean cost per return over the requests that follow, for cold returns to sessions of {}+ tokens in local logs</text>"#,
        t.ink2,
        round_tokens(b.min_context)
    );

    // The legend keys, each stroke drawn like its line.
    for (s, label, lx, ly) in &keys {
        let _ = writeln!(
            o,
            r#"<line x1="{lx}" y1="{ly}" x2="{}" y2="{ly}" stroke="{}" stroke-width="2" stroke-linecap="round"{}/>"#,
            lx + 24.0,
            t.series[*s as usize],
            dash(*s)
        );
        let _ = writeln!(o, r#"<text x="{}" y="{}" font-size="12.5" fill="{}">{label}</text>"#, lx + 30.0, ly + 4.0, t.ink2);
    }

    let panel_w = (W - 2.0 * PAD - GAP * (panels.len().max(1) - 1) as f64) / panels.len().max(1) as f64;
    let (y0, y1) = (136.0 + below, 344.0 + below);
    let xstep = (n as f64 / 6.0).ceil().max(1.0) as usize;
    for (i, Panel { agent, returns, curves }) in panels.iter().enumerate() {
        let left = PAD + i as f64 * (panel_w + GAP);
        let (x0, x1) = (left + 34.0, left + panel_w - 50.0);
        let x = |k: f64| x0 + (x1 - x0) * k / n as f64;
        let y = |v: f64| y1 - (y1 - y0) * v / ymax;
        let ctx = median(returns.iter().map(|r| r.context).collect()).unwrap_or(0);
        let _ = writeln!(
            o,
            r#"<text x="{left}" y="{}" font-size="13" font-weight="600" fill="{}">{}<tspan font-weight="400" fill="{}"> · {} returns, median context {} tokens</tspan></text>"#,
            116.0 + below,
            t.ink,
            agent_title(agent),
            t.ink2,
            returns.len(),
            round_tokens(ctx)
        );
        let mut v = 0.0;
        while v <= ymax + 1e-9 {
            let yy = y(v);
            let stroke = if v == 0.0 { t.axis } else { t.grid };
            let _ = writeln!(o, r#"<line x1="{x0}" y1="{yy:.1}" x2="{x1}" y2="{yy:.1}" stroke="{stroke}" stroke-width="1"/>"#);
            let _ = writeln!(
                o,
                r#"<text class="num" x="{:.1}" y="{:.1}" font-size="11.5" text-anchor="end" fill="{}">{}</text>"#,
                x0 - 8.0,
                yy + 4.0,
                t.ink2,
                money(v, step)
            );
            v += step;
        }
        for k in (0..=n).step_by(xstep) {
            let _ =
                writeln!(o, r#"<text class="num" x="{:.1}" y="{}" font-size="11.5" text-anchor="middle" fill="{}">{k}</text>"#, x(k as f64), y1 + 18.0, t.ink2);
        }
        let _ = writeln!(
            o,
            r#"<text x="{:.1}" y="{}" font-size="12" text-anchor="middle" fill="{}">requests after coming back</text>"#,
            (x0 + x1) / 2.0,
            y1 + 38.0,
            t.ink2
        );
        for s in Strategy::ALL {
            let points: Vec<String> = curves[s as usize].iter().enumerate().map(|(k, v)| format!("{:.1},{:.1}", x(k as f64), y(*v))).collect();
            let _ = writeln!(
                o,
                r#"<polyline points="{}" fill="none" stroke="{}" stroke-width="2" stroke-linejoin="round" stroke-linecap="round"{}/>"#,
                points.join(" "),
                t.series[s as usize],
                dash(s)
            );
        }
        // The first request after the break, where resuming writes the whole context again:
        // labeled above every line it would cross, with a leader down to the point.
        if n >= 1 {
            let first = curves[Strategy::Resume as usize][1];
            let label = format!("1st request {}", usd(first));
            let (px, py) = (x(1.0), y(first));
            let reach = ((text_width(&label, 11.5) + 6.0) / (x1 - x0) * n as f64).ceil() as usize + 1;
            let highest = (1..=reach.min(n)).flat_map(|k| curves.iter().map(move |c| c[k])).fold(first, f64::max);
            let ly = (y(highest) - 12.0).max(y0 + 10.0);
            let _ = writeln!(o, r#"<line x1="{px:.1}" y1="{:.1}" x2="{px:.1}" y2="{:.1}" stroke="{}" stroke-width="1"/>"#, py - 6.0, ly + 4.0, t.muted);
            let _ = writeln!(
                o,
                r#"<circle cx="{px:.1}" cy="{py:.1}" r="4" fill="{}" stroke="{}" stroke-width="2"/>"#,
                t.series[Strategy::Resume as usize],
                t.surface
            );
            let _ = writeln!(
                o,
                r#"<text class="num" x="{:.1}" y="{ly:.1}" font-size="11.5" fill="{}" stroke="{}" stroke-width="3" paint-order="stroke">{label}</text>"#,
                px + 5.0,
                t.ink2,
                t.surface
            );
        }
        // End values, kept at least a line apart; a label pushed off its line gets a leader.
        let mut ends: Vec<(Strategy, f64, f64)> = Strategy::ALL.iter().map(|s| (*s, curves[*s as usize][n], y(curves[*s as usize][n]))).collect();
        ends.sort_by(|a, b| a.2.total_cmp(&b.2));
        let mut placed: Vec<f64> = Vec::new();
        for (j, e) in ends.iter().enumerate() {
            let ly = if j == 0 { e.2 } else { e.2.max(placed[j - 1] + 15.0) };
            placed.push(ly);
        }
        // Bottom up, so where end points crowd, the one beside its own label is drawn on top.
        for (e, ly) in ends.iter().zip(&placed).rev() {
            let (s, _, ey) = *e;
            if (ly - ey).abs() > 3.0 {
                let _ = writeln!(o, r#"<line x1="{:.1}" y1="{ey:.1}" x2="{:.1}" y2="{ly:.1}" stroke="{}" stroke-width="1"/>"#, x1 + 5.0, x1 + 9.0, t.muted);
            }
            let _ = writeln!(o, r#"<circle cx="{x1:.1}" cy="{ey:.1}" r="4" fill="{}" stroke="{}" stroke-width="2"/>"#, t.series[s as usize], t.surface);
        }
        for (e, ly) in ends.iter().zip(&placed) {
            let value = e.1;
            let _ = writeln!(
                o,
                r#"<text class="num" x="{:.1}" y="{:.1}" font-size="12" font-weight="600" fill="{}">{}</text>"#,
                x1 + 10.0,
                ly + 4.0,
                t.ink,
                usd(value)
            );
        }
        // Hovering a column shows a crosshair and every strategy's cost at that request.
        let half = (x1 - x0) / n as f64 / 2.0;
        for (k, at) in (0..=n).map(|k| Strategy::ALL.map(|s| curves[s as usize][k])).enumerate() {
            let cx = x(k as f64);
            let when = match k {
                0 => "before the first request".to_string(),
                1 => "after 1 request".to_string(),
                _ => format!("after {k} requests"),
            };
            let values: Vec<String> = Strategy::ALL.iter().zip(at).map(|(s, v)| format!("{} {}", s.name(), usd(v))).collect();
            let _ = writeln!(
                o,
                r#"<g class="col"><title>{}, {when}: {}</title><rect x="{:.1}" y="{y0}" width="{:.1}" height="{}" fill="{}" fill-opacity="0"/><line class="cross" x1="{cx:.1}" y1="{y0}" x2="{cx:.1}" y2="{y1}" stroke="{}" stroke-width="1"/></g>"#,
                agent_title(agent),
                values.join(" · "),
                (cx - half).max(x0 - half),
                2.0 * half,
                y1 - y0,
                t.surface,
                t.axis
            );
        }
    }
    for (i, note) in notes.iter().enumerate() {
        let _ = writeln!(o, r#"<text x="{PAD}" y="{}" font-size="11" fill="{}">{note}</text>"#, h - 14.0 - 16.0 * (notes.len() - 1 - i) as f64, t.muted);
    }
    o.push_str("</svg>\n");
    o
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn axis_steps_are_round() {
        assert_eq!(nice_axis(8.93).0, 2.0);
        assert_eq!(nice_axis(3.98).0, 1.0);
        assert_eq!(nice_axis(0.4).0, 0.1);
        assert_eq!(round_tokens(490_700), "491k");
    }

    /// An agent with this title whose compaction call reads the warm cache or writes the context
    /// again, as its source says.
    fn agent(name: &str, title: &str, compaction_reads_cache: bool) -> AgentId {
        use tokprof_core::source::{AgentInfo, Harness};
        AgentId::new(AgentInfo { title: title.into(), harness: Harness { compaction_reads_cache, ..Harness::default() }, ..AgentInfo::named(name) })
    }

    /// The bench of `three_returns` with a Codex return too, and the bases of the reference run.
    fn two_agents() -> Bench {
        use tokprof_core::bench::Base;
        let mut b = three_returns();
        let codex = agent("codex", "Codex", false);
        b.returns.push(Return {
            agent: codex.clone(),
            api: tokprof_core::api::ApiId::OpenAi,
            context: 180_000,
            idle_ms: 7 * 3_600_000,
            long_ttl: false,
            cumulative: [(0.0, 1.43), (1.88, 2.1), (0.2, 0.61), (0.9, 2.05), (1.0, 1.52)].map(|(before, after)| vec![before, after]),
            forecast: 1.4,
            pings: 14,
            kept_warm: false,
            compacted: true,
        });
        b.base.insert(codex, Base { tokens: 20_800, compactions: 791 });
        b
    }

    /// Three returns followed for one request each, with what every way spends before the first
    /// request and after it.
    fn three_returns() -> Bench {
        use tokprof_core::bench::{Base, Elsewhere};
        let claude = agent("claude", "Claude Code", true);
        // Per return: resume, /compact first, handoff, heartbeats, heartbeats then compaction,
        // each as (before the first request, after it); then kept warm and compacted.
        let ways = [
            ([(0.0, 4.00), (4.1, 4.5), (0.3, 1.0), (0.30, 0.70), (0.30, 0.70)], true, false),
            ([(0.0, 4.00), (4.1, 4.5), (0.3, 1.0), (0.31, 0.71), (0.31, 0.71)], true, false),
            ([(0.0, 3.73), (4.0, 4.4), (0.3, 1.0), (0.00, 3.12), (0.30, 1.22)], false, true),
        ];
        let returns = ways
            .iter()
            .map(|(costs, kept_warm, compacted)| Return {
                agent: claude.clone(),
                api: tokprof_core::api::ApiId::Anthropic,
                context: 500_000,
                idle_ms: 3 * 3_600_000,
                long_ttl: true,
                cumulative: costs.map(|(before, after)| vec![before, after]),
                forecast: 4.0,
                pings: 2,
                kept_warm: *kept_warm,
                compacted: *compacted,
            })
            .collect();
        let base = [(claude.clone(), Base { tokens: 52_500, compactions: 31 })].into_iter().collect();
        let elsewhere = [(claude, Elsewhere { pings: 12, ping_usd: 1.20, compactions: 2, compaction_usd: 0.60 })].into_iter().collect();
        Bench { requests: 1, min_context: 100_000, keep_warm_ms: 2 * 3_600_000, returns, base, elsewhere }
    }

    #[test]
    fn the_report_prices_the_breaks_that_were_no_return() {
        let out = report(&three_returns(), &Style::with_color(false));
        assert!(out.contains("  heartbeats, then compaction "), "the fifth row: {out}");
        assert!(out.contains("Heartbeats for up to 2h would have kept 2 of 3 returns warm, for $0.61 of pings, and saved $7.20 of $11.73 (61%)."), "{out}");
        assert!(out.contains("They cost less than a handoff note in 2 of 3 returns.\n  Breaks that were not cold returns would have taken another 12 pings for $1.20; with them, heartbeats save $6.00 (51%)."), "{out}");
        assert!(out.contains("Heartbeats for up to 2h, then a compaction, would have kept 2 of 3 returns warm and compacted 1, for $0.91 of pings and compactions, and saved $9.10 of $11.73 (78%)."), "{out}");
        assert!(
            out.contains("Breaks that were not cold returns would have taken another 12 pings and 2 compactions for $1.80; with them, it saves $7.30 (62%)."),
            "{out}"
        );
        assert!(
            out.ends_with("Heartbeats, then compaction sends the same pings and, at the next ping time past the window, a compaction that reads the warm context and outputs 10.0k tokens.\n"),
            "Claude Code alone: {out}"
        );
    }

    #[test]
    fn the_notes_say_where_a_warm_compaction_writes_the_context() {
        let out = report(&two_agents(), &Style::with_color(false));
        let notes: Vec<&str> = out.lines().rev().take(4).collect::<Vec<_>>().into_iter().rev().collect();
        assert_eq!(
            notes,
            [
                "The alternatives start from what a compaction leaves: claude 52.5k (median of 31 compactions), codex 20.8k (median of 791 compactions).",
                "A compaction re-writes the cold context and outputs 10.0k tokens; a handoff note is 10.0k tokens written while the cache was warm.",
                "A heartbeat goes out at 5/6 of the cache lifetime, every 50m on a 1h cache, reads the context and outputs 100 tokens; for Codex this assumes it resets the cache timer as it does in Claude Code.",
                "Heartbeats, then compaction sends the same pings and, at the next ping time past the window, a compaction while the cache is warm that outputs 10.0k tokens: it reads the context in Claude Code and writes it again in Codex, whose compaction calls read nothing from the cache.",
            ]
        );
        let mut codex_alone = two_agents();
        codex_alone.returns.retain(|r| r.agent.name() == "codex");
        assert!(
            report(&codex_alone, &Style::with_color(false)).ends_with(
                "a compaction while the cache is warm that outputs 10.0k tokens: it writes the context again in Codex, whose compaction calls read nothing from the cache.\n"
            ),
            "Codex alone"
        );
    }

    #[test]
    fn the_chart_notes_name_the_agents_whose_warm_compaction_writes_the_context() {
        let notes = |b: &Bench| {
            let chart = svg(b, &LIGHT);
            chart
                .lines()
                .filter(|l| l.contains(r#"font-size="11""#))
                .map(|l| l[l.find('>').unwrap() + 1..l.rfind("</").unwrap()].to_string())
                .collect::<Vec<_>>()
        };
        let first = "tokprof bench · requests priced at their time · a compaction call is modeled as writing the cold context again plus a 10k-token summary";
        let heartbeats = "heartbeats: a ping at 5/6 of the cache lifetime, every 50m on a 1h cache, for up to 2h after the last request; a return within the last ping's lifetime reads the cache";
        assert_eq!(
            notes(&three_returns()),
            [
                first,
                heartbeats,
                "heartbeats, then compaction: the same pings, then at the next ping time past 2h a compaction that reads the warm context; a later return starts from what it leaves",
            ],
            "Claude Code alone"
        );
        assert_eq!(
            notes(&two_agents()),
            [
                first,
                heartbeats,
                "heartbeats, then compaction: the same pings, then at the next ping time past 2h a compaction while the cache is warm; a later return starts from what it leaves",
                "a compaction while the cache is warm reads the context in Claude Code and writes it again in Codex, whose compaction calls read nothing from the cache",
            ]
        );
        // The fourth note makes the chart a line taller.
        let height = |b: &Bench| svg(b, &LIGHT).split("height=\"").nth(1).and_then(|h| h.split('"').next()).and_then(|h| h.parse::<f64>().ok()).unwrap();
        assert_eq!(height(&two_agents()) - height(&three_returns()), 16.0);
    }

    #[test]
    fn the_chart_draws_the_fifth_way_dash_dotted() {
        let b = three_returns();
        let light = svg(&b, &LIGHT);
        let legend: Vec<&str> = Strategy::ALL.iter().map(|s| s.name()).collect();
        assert_eq!(legend.len(), 5);
        for name in legend {
            assert!(light.contains(&format!(">{name} (")), "{name} in the legend");
        }
        assert!(light.contains("heartbeats, then compaction (replayed)"));
        assert!(
            light.contains(r##"stroke="#e87ba4" stroke-width="2" stroke-linecap="round" stroke-dasharray="7 3 1 3""##),
            "the legend key is dash-dotted in slot 5"
        );
        assert!(svg(&b, &DARK).contains(r##"stroke="#d55181""##));
        assert!(light.contains("heartbeats, then compaction: the same pings"), "the note");
    }

    #[test]
    fn the_table_shows_each_request_count_once() {
        assert_eq!(table_marks(60), vec![1, 10, 60]);
        assert_eq!(table_marks(10), vec![1, 10]);
        assert_eq!(table_marks(5), vec![1, 5]);
        assert_eq!(table_marks(1), vec![1]);
    }
}
