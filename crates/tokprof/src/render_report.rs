//! The text of `tokprof summary`, `misses`, `whatif` and `prices`.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::Path;

use tokprof_browse::render_tools::{failures_line, tool_rows};
use tokprof_core::analysis::cache::Cause;
use tokprof_core::analysis::{Component, Filter, MissEvent};
use tokprof_core::api::ApiId;
use tokprof_core::fmt::{self, clip_end, table, tokens, usd};
use tokprof_core::pauses::{Pauses, SHORT_FOLLOW};
use tokprof_core::pricing::{Price, Pricing};
use tokprof_core::profile::Values;
use tokprof_core::report::{Collector, Spend, TriggerSpend, Work};
use tokprof_core::setup::SetupSpend;
use tokprof_core::source::AgentId;
use tokprof_core::tools::{RowOrder, ToolTable};

/// `window` is the time filter the requests were selected by; it limits the reconciliation to
/// the sessions it holds whole.
/// `price_file` is the file the hint for models without a price names.
pub fn summary(c: &Collector, window: &Filter, price_file: &Path) -> String {
    let mut out = String::new();
    let mut rows = vec![
        ["agent", "model", "requests", "prompt known"]
            .iter()
            .map(|s| s.to_string())
            .chain(Component::ALL.iter().map(|c| c.name().to_string()))
            .chain(["total".to_string()])
            .collect::<Vec<_>>(),
    ];
    let mut unpriced: Vec<(String, f64)> = Vec::new();
    for ((agent, model), m) in c.spend.by_cost() {
        let cell = |v: &Values| if m.priced { usd(v.usd) } else { tokens(v.tokens) };
        let mut row =
            vec![agent.name().to_string(), model.clone(), m.requests.to_string(), format!("{:.0}%", 100.0 * m.with_prompt as f64 / m.requests.max(1) as f64)];
        row.extend(m.by_component.iter().map(cell));
        row.push(if m.priced { usd(m.usd()) } else { format!("{} tok", tokens(m.tokens())) });
        rows.push(row);
        if !m.priced {
            unpriced.push((model.clone(), m.tokens()));
        }
    }
    let _ = writeln!(out, "Spend by model (API-equivalent USD; token counts where no price is known)\n");
    out.push_str(&table(&rows, 2));
    let total = c.spend.priced_usd();
    // The premium of the misses the cause table lists, so expected rebuilds are left out of both.
    let avoidable = c.avoidable_usd();
    let _ = writeln!(out, "\nTotal priced: {}   avoidable on cache misses: {} ({:.1}%)", usd(total), usd(avoidable), 100.0 * avoidable / total.max(1e-9));
    if !unpriced.is_empty() {
        let list = unpriced.iter().map(|(m, t)| format!("{m} ({} tokens)", tokens(*t))).collect::<Vec<_>>().join(", ");
        let _ = writeln!(out, "No price for {list}. Add them to {}", price_file.display());
    }
    let windowed = window.since_ms.is_some() || window.until_ms.is_some();
    for r in c.spend.reconciliation(window) {
        let n = r.sessions;
        let what = format!("{n} session{}{}", if n == 1 { "" } else { "s" }, if windowed { " inside the window" } else { "" });
        let info = r.agent.info();
        let counter = info.cost_counter.clone().unwrap_or_else(|| format!("{}'s own cost counter", info.title));
        let _ = writeln!(out, "{counter} for {what}: {}; tokprof for the same sessions: {}", usd(r.reported_usd), usd(r.ours_usd));
    }
    out.push('\n');
    out.push_str(&clients(c));
    out.push('\n');
    out.push_str(&work(&c.work, 15, 15));
    out.push('\n');
    out.push_str(&triggers(&c.work));
    out.push('\n');
    out.push_str(&activities(&c.work));
    let setup = setup(&c.setup);
    if !setup.is_empty() {
        out.push('\n');
        out.push_str(&setup);
    }
    out.push('\n');
    out.push_str(&miss_causes(c, false));
    out
}

/// A cache lifetime as the pause report names it: `5-minute`, `1-hour`.
fn lifetime_name(ms: i64) -> String {
    const HOUR: i64 = 3_600_000;
    if ms % HOUR == 0 { format!("{}-hour", ms / HOUR) } else { format!("{}-minute", ms / 60_000) }
}

/// `tokprof pauses`: per agent, the pauses before typed messages, the share after each cache
/// lifetime, the cold returns by pause with who started them and what followed, and the
/// sessions by largest context.
pub fn pauses(p: &Pauses) -> String {
    let mut out = String::from("Pauses before typed messages, and what they cost through the prompt cache\n");
    for (agent, a) in &p.agents {
        let amount = |dollars: f64, count: f64| if a.priced { usd(dollars) } else { format!("{} tok", tokens(count)) };
        let _ = writeln!(out, "\n{} ({})\n", agent.info().title, agent.name());
        if a.typed.is_empty() {
            out.push_str("No typed messages after an earlier request.\n");
        } else {
            let _ = writeln!(
                out,
                "{} typed in main conversations after an earlier request, by the pause before them\n",
                fmt::count(a.typed.len() as u64, "message", "messages")
            );
            let mut rows = vec![["pause", "messages", "share", "median context"].map(str::to_string).to_vec()];
            for r in a.pause_rows() {
                let share = 100.0 * r.count as f64 / a.typed.len() as f64;
                rows.push(vec![
                    r.name.to_string(),
                    r.count.to_string(),
                    format!("{share:.1}%"),
                    r.median_context.map_or_else(String::new, |t| tokens(t as f64)),
                ]);
            }
            out.push_str(&table(&rows, 1));
            let shares: Vec<String> = a
                .lifetimes
                .iter()
                .map(|&ms| (lifetime_name(ms), a.share_after(ms).round()))
                .enumerate()
                .map(|(i, (name, share))| {
                    if i == 0 {
                        format!("On a {name} cache, {share:.0}% of these messages come after it expired")
                    } else {
                        format!("on a {name} cache, {share:.0}%")
                    }
                })
                .collect();
            if !shares.is_empty() {
                let _ = writeln!(out, "\n{}.", shares.join("; "));
            }
        }
        out.push_str("\nCold returns: requests of main conversations that missed the cache after a pause longer than its lifetime\n\n");
        let returns = a.return_rows();
        if returns.is_empty() {
            out.push_str("No cold returns.\n");
        } else {
            let mut rows = vec![["pause", "returns", "first request", "avoidable", "median context"].map(str::to_string).to_vec()];
            for r in &returns {
                rows.push(vec![
                    r.name.to_string(),
                    r.count.to_string(),
                    usd(r.usd),
                    usd(r.premium_usd),
                    r.median_context.map_or_else(String::new, |t| tokens(t as f64)),
                ]);
            }
            out.push_str(&table(&rows, 1));
            let (n, first, premium) = a.return_totals();
            let _ =
                writeln!(out, "\n{}: {} in first requests, {} of it avoidable.", fmt::count(n as u64, "cold return", "cold returns"), usd(first), usd(premium));
            let [typed, background, tool, other] = a.starters().map(|n| n as u64);
            let _ = writeln!(
                out,
                "Started by: {}, {}, {}, {} other.",
                fmt::count(typed, "typed message", "typed messages"),
                fmt::count(background, "background task", "background tasks"),
                fmt::count(tool, "tool result", "tool results"),
                other
            );
            if let Some((median, q1, q3, short)) = a.follow_stats() {
                let _ = writeln!(
                    out,
                    "After a cold return: a median of {} until the next cold pause or compaction (quarters {q1} and {q3}); {} within {SHORT_FOLLOW} requests.",
                    fmt::count(median, "request", "requests"),
                    fmt::count(short as u64, "return ended", "returns ended")
                );
            }
        }
        out.push_str("\nSessions by largest context: the largest prompt of each session's main conversations\n\n");
        let (spend_usd, spend_tokens) = a.spend();
        let total = if a.priced { spend_usd } else { spend_tokens };
        let mut rows = vec![["largest context", "sessions", "spend", "share", "cold returns"].map(str::to_string).to_vec()];
        for r in a.size_rows() {
            let part = if a.priced { r.usd } else { r.tokens };
            rows.push(vec![
                r.name.to_string(),
                r.count.to_string(),
                amount(r.usd, r.tokens),
                format!("{:.1}%", 100.0 * part / total.max(1e-9)),
                r.cold_returns.to_string(),
            ]);
        }
        out.push_str(&table(&rows, 1));
        busy_part(&mut out, &a.busy);
    }
    if p.agents.is_empty() {
        out.push_str("\nNo requests in these logs.\n");
    }
    out
}

/// The last part of an agent's pause report: its busy time by kind, and the slow requests (see
/// agent-time).
fn busy_part(out: &mut String, b: &tokprof_core::busy::Busy) {
    use tokprof_core::rhythm::duration;
    out.push_str("\nWhere the agent's busy time went: what it waited on in its turns, and while its own background work ran after them\n\n");
    let total = b.total_ms();
    if total == 0 {
        out.push_str("No busy time.\n");
        return;
    }
    // Over many days, hours read better than days.
    let time = |ms: i64| if ms >= 3_600_000 { format!("{:.1}h", ms as f64 / 3_600_000.0) } else { duration(ms) };
    let mut rows = vec![["kind", "time", "share", "after turns"].map(str::to_string).to_vec()];
    for (kind, ms, after) in b.sorted() {
        let after = if after > 0 { time(after) } else { "–".to_string() };
        rows.push(vec![kind.to_string(), time(ms), format!("{:.1}%", 100.0 * ms as f64 / total as f64), after]);
    }
    out.push_str(&table(&rows, 1));
    let slow =
        format!("\n{} of {} requests took a minute or more ({:.1}%)", fmt::int(b.slow), fmt::int(b.requests), 100.0 * b.slow as f64 / b.requests.max(1) as f64);
    match b.longest.filter(|_| b.slow > 0) {
        Some((ms, _)) => {
            let _ = writeln!(out, "{slow}; the longest took {}.", duration(ms));
        }
        None => {
            let _ = writeln!(out, "{slow}.");
        }
    }
}

/// Spend by client: one row per agent and client, most expensive first, with the main sessions
/// that had a request of the client and the premium of its misses that the cause table lists.
pub fn clients(c: &Collector) -> String {
    let rows = &c.clients.rows;
    let priced = rows.values().any(|r| r.spend.usd > 0.0);
    let key = |s: &Spend| if priced { s.usd } else { s.tokens };
    let total: f64 = rows.values().map(|r| key(&r.spend)).sum();
    let mut avoidable: BTreeMap<(&AgentId, &str), f64> = BTreeMap::new();
    for m in c.misses.iter().filter(|m| !m.cause.expected()) {
        *avoidable.entry((&m.agent, m.client.as_str())).or_default() += m.premium_usd.unwrap_or(0.0);
    }
    let mut sorted: Vec<_> = rows.iter().collect();
    sorted.sort_by(|a, b| key(&b.1.spend).total_cmp(&key(&a.1.spend)).then(a.0.cmp(b.0)));
    let mut table_rows = vec![["agent", "client", "sessions", "requests", "cost", "share", "avoidable"].map(str::to_string).to_vec()];
    for ((agent, client), r) in sorted {
        let amount = if priced { usd(r.spend.usd) } else { format!("{} tok", tokens(r.spend.tokens)) };
        let saved = avoidable.get(&(agent, client.as_str())).copied().unwrap_or(0.0);
        table_rows.push(vec![
            agent.name().to_string(),
            clip_end(client, 32),
            r.sessions.len().to_string(),
            r.spend.requests.to_string(),
            amount,
            format!("{:.1}%", 100.0 * key(&r.spend) / total.max(1e-9)),
            usd(saved),
        ]);
    }
    let mut out = String::from("Spend by client: the program that ran the agent, as its log names it\n\n");
    out.push_str(&table(&table_rows, 2));
    out
}

/// Setup: what every request carries before the work, one row per agent and source with the
/// agent's hint, and a line per agent. Empty when no request held setup with a cost.
pub fn setup(s: &SetupSpend) -> String {
    /// The sources of an agent shown as rows; the rest fold into one `<n> more` row.
    const ROWS_PER_AGENT: usize = 8;
    let rows = s.rows();
    if rows.is_empty() {
        return String::new();
    }
    let amount = |v: &Values| if s.priced { usd(v.usd) } else { format!("{} tok", tokens(v.tokens)) };
    let share = |agent: &AgentId, v: &Values| 100.0 * s.amount(v) / s.spend.get(agent).map_or(0.0, |w| s.amount(w)).max(1e-9);
    // Agents by their whole setup, the costliest first.
    let mut agents: Vec<&AgentId> = Vec::new();
    for (agent, _, _) in &rows {
        if !agents.contains(agent) {
            agents.push(agent);
        }
    }
    agents.sort_by(|a, b| s.amount(&s.total(b).all).total_cmp(&s.amount(&s.total(a).all)).then_with(|| a.cmp(b)));
    let mut out = String::from("Setup: what every request carries before the work\n\n");
    let mut table_rows = vec![["agent", "source", "cost", "subagents", "share"].map(str::to_string).to_vec()];
    let mut hints = vec!["hint".to_string()];
    for agent in &agents {
        let mine: Vec<_> = rows.iter().filter(|(a, _, _)| a == agent).collect();
        for (_, source, cost) in mine.iter().take(ROWS_PER_AGENT) {
            table_rows.push(vec![
                agent.name().to_string(),
                clip_end(source, 32),
                amount(&cost.all),
                amount(&cost.subagents),
                format!("{:.1}%", share(agent, &cost.all)),
            ]);
            hints.push(agent.info().setup_hints.iter().find(|(src, _)| src == source).map_or_else(String::new, |(_, h)| h.clone()));
        }
        if mine.len() > ROWS_PER_AGENT {
            let (mut all, mut subagents) = (Values::default(), Values::default());
            for (_, _, cost) in &mine[ROWS_PER_AGENT..] {
                all.add(cost.all);
                subagents.add(cost.subagents);
            }
            let more = format!("{} more", mine.len() - ROWS_PER_AGENT);
            table_rows.push(vec![agent.name().to_string(), more, amount(&all), amount(&subagents), format!("{:.1}%", share(agent, &all))]);
            hints.push(String::new());
        }
    }
    // The hint is text, so it goes after the aligned columns.
    for (line, h) in table(&table_rows, 2).lines().zip(&hints) {
        let _ = writeln!(out, "{}", format!("{line}  {h}").trim_end());
    }
    out.push('\n');
    for agent in agents {
        let total = s.total(agent);
        let _ = writeln!(out, "{} setup: {}, {:.1}% of its spend", agent.info().title, amount(&total.all), share(agent, &total.all));
    }
    out
}

/// Spend by origin, and the most expensive tasks.
pub fn work(w: &Work, top_origins: usize, top_tasks: usize) -> String {
    let origins = w.origins();
    let priced = origins.iter().any(|(_, s)| s.usd > 0.0);
    let key = |s: &Spend| if priced { s.usd } else { s.tokens };
    let total: f64 = origins.iter().map(|(_, s)| key(s)).sum();
    let share = |s: &Spend| format!("{:.1}%", 100.0 * key(s) / total.max(1e-9));
    let amount = |s: &Spend| if priced { usd(s.usd) } else { format!("{} tok", tokens(s.tokens)) };

    let mut out = String::from("Spend by origin: the kind of work each request was part of\n\n");
    let mut rows = vec![["origin", "requests", "tokens", "cost", "share"].map(str::to_string).to_vec()];
    let mut rest = (0, Spend::default());
    for (i, (origin, s)) in origins.iter().enumerate() {
        if i < top_origins {
            rows.push(vec![origin.to_string(), s.requests.to_string(), tokens(s.tokens), amount(s), share(s)]);
        } else {
            rest.0 += 1;
            rest.1.add(*s);
        }
    }
    if rest.0 > 0 {
        let s = rest.1;
        rows.push(vec![format!("{} more", rest.0), s.requests.to_string(), tokens(s.tokens), amount(&s), share(&s)]);
    }
    out.push_str(&table(&rows, 1));

    let _ = writeln!(out, "\nMost expensive tasks: session titles, and what subagents were asked to do\n");
    let mut rows = vec![["task", "origin", "requests", "cost", "share"].map(str::to_string).to_vec()];
    for (task, origin, s) in w.tasks().into_iter().take(top_tasks) {
        rows.push(vec![clip_end(task, 60), origin.to_string(), s.requests.to_string(), amount(&s), share(&s)]);
    }
    out.push_str(&table(&rows, 2));
    out
}

/// Spend by trigger: what made the agent send each request.
pub fn triggers(w: &Work) -> String {
    let rows: Vec<(&str, TriggerSpend)> = w.triggers().into_iter().map(|(t, s)| (t.name(), s)).collect();
    request_table("Spend by trigger: what made the agent send each request", "trigger", rows)
}

/// Spend by activity: what the agent was doing before each request.
pub fn activities(w: &Work) -> String {
    request_table("Spend by activity: what the agent was doing before each request", "activity", w.activities())
}

/// A table of whole requests by one label, with their mean prompt and cost per request.
fn request_table(title: &str, label: &str, rows_in: Vec<(&str, TriggerSpend)>) -> String {
    let priced = rows_in.iter().any(|(_, t)| t.spend.usd > 0.0);
    let key = |s: &Spend| if priced { s.usd } else { s.tokens };
    let total: f64 = rows_in.iter().map(|(_, t)| key(&t.spend)).sum();
    let mut out = format!("{title}\n\n");
    let mut rows = vec![[label, "requests", "avg prompt", "cost", "per request", "share"].map(str::to_string).to_vec()];
    for (name, t) in rows_in {
        let s = t.spend;
        let n = s.requests.max(1) as f64;
        let (cost, per) = if priced { (usd(s.usd), usd(s.usd / n)) } else { (format!("{} tok", tokens(s.tokens)), format!("{} tok", tokens(s.tokens / n))) };
        rows.push(vec![name.to_string(), s.requests.to_string(), tokens(t.avg_prompt()), cost, per, format!("{:.1}%", 100.0 * key(&s) / total.max(1e-9))]);
    }
    out.push_str(&table(&rows, 1));
    out
}

/// Spend by tool: the table of `tokprof tools`, its top `limit` rows in `order`, with the
/// failures columns and line.
pub fn tools(t: &ToolTable, order: RowOrder, limit: usize) -> String {
    let mut out = String::from("Spend by tool: calls, sizes, what keeping them cost (rent) and what they made the agent do (trips)\n\n");
    out.push_str(&tool_rows(t, order, limit, true));
    if let Some(line) = failures_line(t) {
        let _ = writeln!(out, "{line}");
    }
    out
}

pub fn miss_causes(c: &Collector, include_expected: bool) -> String {
    let mut by: BTreeMap<(AgentId, Cause), (u64, f64, f64, bool)> = BTreeMap::new();
    for m in &c.misses {
        let e = by.entry((m.agent.clone(), m.cause)).or_default();
        e.0 += 1;
        e.1 += m.rewritten as f64;
        e.2 += m.premium_usd.unwrap_or(0.0);
        e.3 |= m.premium_usd.is_some();
    }
    let mut rows = vec![["agent", "cause", "misses", "re-processed", "avoidable"].map(str::to_string).to_vec()];
    let mut entries: Vec<_> = by.into_iter().filter(|((_, cause), _)| include_expected || !cause.expected()).collect();
    entries.sort_by(|a, b| b.1.2.total_cmp(&a.1.2).then(b.1.1.total_cmp(&a.1.1)));
    for ((agent, cause), (n, tok, prem, priced)) in entries {
        rows.push(vec![
            agent.name().to_string(),
            format!("{}{}", cause.name(), if cause.expected() { " (expected)" } else { "" }),
            n.to_string(),
            tokens(tok),
            if priced { usd(prem) } else { "-".to_string() },
        ]);
    }
    let mut out = String::from("Cache misses by cause\n\n");
    out.push_str(&table(&rows, 2));
    out
}

pub fn misses(c: &Collector, top: usize, include_expected: bool) -> String {
    let mut out = miss_causes(c, include_expected);
    let mut events: Vec<&MissEvent> = c.misses.iter().filter(|m| include_expected || !m.cause.expected()).collect();
    events.sort_by(|a, b| b.premium_usd.unwrap_or(0.0).total_cmp(&a.premium_usd.unwrap_or(0.0)).then(b.rewritten.cmp(&a.rewritten)));
    let mut rows = vec![["when", "agent", "project", "session", "chain", "context", "re-processed", "avoidable", "cause"].map(str::to_string).to_vec()];
    for m in events.into_iter().take(top) {
        let when =
            chrono::DateTime::from_timestamp_millis(m.ts_ms).map(|t| t.with_timezone(&chrono::Local).format("%Y-%m-%d %H:%M").to_string()).unwrap_or_default();
        rows.push(vec![
            when,
            m.agent.name().to_string(),
            m.project.clone(),
            m.session.clone(),
            m.chain.clone(),
            tokens(m.context as f64),
            tokens(m.rewritten as f64),
            m.premium_usd.map_or("-".to_string(), usd),
            if m.detail.is_empty() { m.cause.name().to_string() } else { format!("{}: {}", m.cause.name(), m.detail) },
        ]);
    }
    let _ = writeln!(out, "\nMost expensive misses\n");
    out.push_str(&table(&rows, 5));
    out
}

/// `agents` are the agents whose settings choose the cache lifetime are named at the end.
pub fn whatif(c: &Collector, agents: &[AgentId]) -> String {
    let w = &c.whatif;
    // The replay covers the APIs that offer two cache lifetimes.
    let models: Vec<&str> = ApiId::KNOWN.iter().filter_map(|a| a.rules()).filter(|r| r.long.is_some()).map(|r| r.models).collect();
    let mut out = format!(
        "{} input cost under each cache TTL, replaying the same requests.\n\
         The model counts only TTL expiry, so compare the simulated columns with each other.\n\n",
        models.join(" and ")
    );
    let mut rows = vec![["requests", "bucket", "actual", "simulated 5m", "simulated 1h"].map(str::to_string).to_vec()];
    for (i, name) in ["main conversation", "subagents and side requests"].into_iter().enumerate() {
        rows.push(vec![w.requests[i].to_string(), name.to_string(), usd(w.actual[i]), usd(w.simulated[i][0]), usd(w.simulated[i][1])]);
    }
    out.push_str(&table(&rows, 0));
    let best = |i: usize| if w.simulated[i][0] < w.simulated[i][1] { "5m" } else { "1h" };
    let saving = (0..2).map(|i| w.simulated[i][1].max(w.simulated[i][0]) - w.simulated[i][1].min(w.simulated[i][0])).collect::<Vec<_>>();
    let _ =
        writeln!(out, "\nCheapest: main conversation {} (saves {} over the other), subagents {} (saves {}).", best(0), usd(saving[0]), best(1), usd(saving[1]));
    for agent in agents {
        if let Some(line) = agent.info().ttl_line() {
            let _ = writeln!(out, "{line}");
        }
    }
    out
}

/// The price table; with `at`, only the prices in effect at that moment.
pub fn prices(p: &Pricing, at: Option<(i64, &str)>) -> String {
    let mut out = String::from("Prices in USD per million tokens. Later layers replace earlier ones:\n");
    for layer in p.layers() {
        let state = match (&layer.path, layer.loaded) {
            (None, _) => "prices.toml in the binary".to_string(),
            (Some(path), true) => path.display().to_string(),
            (Some(path), false) => format!("{} (not found)", path.display()),
        };
        let _ = writeln!(out, "  {:<12} {state}", layer.name);
    }
    if let Some((_, shown)) = at {
        let _ = writeln!(out, "Showing the prices in effect on {shown}.");
    }
    let at = at.map(|(t, _)| t);
    out.push('\n');
    let num = |x: f64| format!("{x:.3}").trim_end_matches('0').trim_end_matches('.').to_string();
    let cells = |p: &Price| [p.input, p.write_short, p.write_long, p.read, p.output].map(num).to_vec();
    let mut rows = vec![["model", "input", "write short", "write long", "read", "output", "in effect", "source"].map(str::to_string).to_vec()];
    let mut entries: Vec<_> = p.entries().iter().collect();
    entries.sort_by(|a, b| a.model.cmp(&b.model));
    for e in entries {
        let periods: Vec<_> = e.periods.iter().filter(|period| at.is_none_or(|t| period.covers(t))).collect();
        for (i, period) in periods.iter().enumerate() {
            let when = match (period.since_ms, period.until_ms) {
                (None, None) => "always".to_string(),
                (Some(s), None) => format!("since {}", day(s)),
                (None, Some(u)) => format!("until {}", day(u)),
                (Some(s), Some(u)) => format!("{} to {}", day(s), day(u)),
            };
            let mut row = vec![if i == 0 { e.model.clone() } else { String::new() }];
            row.extend(cells(&period.price));
            row.push(when);
            row.push(if i == 0 { e.source.to_string() } else { String::new() });
            rows.push(row);
            if let Some((above, long)) = &period.long_context {
                let threshold = if above % 1000 == 0 { format!("{}k", above / 1000) } else { above.to_string() };
                let mut row = vec![format!("  over {threshold} tokens")];
                row.extend(cells(long));
                row.extend([String::new(), String::new()]);
                rows.push(row);
            }
        }
    }
    out.push_str(&table(&rows, 1));
    let lifetimes: Vec<String> = ApiId::KNOWN
        .iter()
        .filter_map(|a| a.rules())
        .enumerate()
        .map(|(i, r)| format!("{} and {}{} for {}", r.write_lifetimes.0, r.write_lifetimes.1, if i == 0 { " cache lifetimes" } else { "" }, r.vendor))
        .collect();
    let _ = writeln!(out, "\nShort and long writes are the {}.", lifetimes.join(", "));
    let _ = writeln!(out, "Each request is priced by the period in effect when it was sent; `until` dates are excluded.");
    out
}

fn day(ts_ms: i64) -> String {
    chrono::DateTime::from_timestamp_millis(ts_ms).map(|t| t.format("%Y-%m-%d").to_string()).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokprof_core::model::{ChainKind, Session, Usage};
    use tokprof_core::profile::Values;
    use tokprof_core::source::AgentInfo;

    fn claude() -> AgentId {
        AgentId::new(AgentInfo { cost_counter: Some("Claude Code's own cost counter".into()), ..AgentInfo::named("claude") })
    }

    #[test]
    fn the_pause_report_prints_its_parts_per_agent() {
        use tokprof_core::pauses::{AgentPauses, ColdReturn, SessionSize, Starter};
        let minute = 60_000;
        let mut p = Pauses::default();
        let claude_pauses = AgentPauses {
            typed: vec![(30_000, 100_000), (7 * minute, 300_000), (90 * minute, 420_000), (100 * minute, 600_000)],
            lifetimes: [5 * minute, 60 * minute].into_iter().collect(),
            returns: vec![
                ColdReturn { ts_ms: 0, pause_ms: 90 * minute, usd: 3.5, premium_usd: 3.3, context: 420_000, starter: Starter::Typed },
                ColdReturn { ts_ms: 0, pause_ms: 570 * minute, usd: 7.0, premium_usd: 6.8, context: 878_000, starter: Starter::Background },
            ],
            follows: vec![20, 2],
            cold_returns_by_client: Default::default(),
            sessions: [
                ("a".to_string(), SessionSize { largest: 966_000, usd: 412.0, tokens: 1.0, cold_returns: 2, span: None, typed: 0 }),
                ("b".to_string(), SessionSize { largest: 30_000, usd: 8.0, tokens: 1.0, cold_returns: 0, span: None, typed: 0 }),
            ]
            .into_iter()
            .collect(),
            priced: true,
            busy: {
                let mut b = tokprof_core::busy::Busy::default();
                b.kinds.insert(tokprof_core::busy::SUBAGENTS, (60 * 60 * minute, 43 * 60 * minute));
                b.kinds.insert(tokprof_core::busy::MODEL, (84 * 60 * minute, 0));
                (b.requests, b.slow, b.longest) = (81_324, 2_652, Some((1_398_000, 9_000)));
                b
            },
        };
        p.agents.insert(claude(), claude_pauses);
        let codex = AgentPauses {
            lifetimes: [30 * minute].into_iter().collect(),
            sessions: [("c".to_string(), SessionSize { largest: 50_000, usd: 1.0, tokens: 1.0, cold_returns: 0, span: None, typed: 0 })].into_iter().collect(),
            priced: true,
            ..AgentPauses::default()
        };
        p.agents.insert(AgentId::new(AgentInfo { title: "Codex".into(), ..AgentInfo::named("codex") }), codex);
        let out = pauses(&p);
        // Claude Code first, then Codex.
        assert!(out.find("(claude)").unwrap() < out.find("Codex (codex)").unwrap(), "{out}");
        let row = |name: &str| out.lines().find(|l| l.starts_with(name)).map(|l| l.split_whitespace().collect::<Vec<_>>());
        assert_eq!(row("5–15m").unwrap(), ["5–15m", "1", "25.0%", "300.0k"]);
        assert_eq!(row("1–2h").unwrap(), ["1–2h", "2", "50.0%", "600.0k"], "the pause table comes first, with the upper of the two middle contexts");
        assert!(out.contains("On a 5-minute cache, 75% of these messages come after it expired; on a 1-hour cache, 50%."), "{out}");
        assert!(out.contains("2 cold returns: $10.50 in first requests, $10.10 of it avoidable."), "{out}");
        assert!(out.contains("Started by: 1 typed message, 1 background task, 0 tool results, 0 other."), "{out}");
        assert!(
            out.contains(
                "After a cold return: a median of 20 requests until the next cold pause or compaction (quarters 2 and 20); 1 return ended within 3 requests."
            ),
            "{out}"
        );
        assert_eq!(row("800k+").unwrap(), ["800k+", "1", "$412", "98.1%", "2"]);
        // The busy time is the last part, most time first.
        let claude_part = &out[..out.find("Codex (codex)").unwrap()];
        assert!(claude_part.find("Sessions by largest context").unwrap() < claude_part.find("Where the agent's busy time went").unwrap(), "{claude_part}");
        assert_eq!(row("model").unwrap(), ["model", "84.0h", "58.3%", "–"], "no time after turns shows a dash");
        assert_eq!(row("subagents").unwrap(), ["subagents", "60.0h", "41.7%", "43.0h"]);
        assert!(claude_part.contains("2,652 of 81,324 requests took a minute or more (3.3%); the longest took 23m."), "{claude_part}");
        // Codex: no typed message, no cold return.
        let codex_part = &out[out.find("Codex (codex)").unwrap()..];
        assert!(codex_part.contains("No typed messages after an earlier request.") && codex_part.contains("No cold returns."), "{codex_part}");
        assert!(!codex_part.contains("Started by"), "{codex_part}");
        assert!(codex_part.contains("Where the agent's busy time went") && codex_part.contains("No busy time."), "{codex_part}");
    }

    #[test]
    fn the_client_table_lists_the_costliest_client_first() {
        use tokprof_core::report::ClientTotals;
        let mut c = Collector::new(None, None);
        let row = |requests: u64, usd: f64, sessions: &[&str]| ClientTotals {
            spend: Spend { requests, tokens: 1.0, usd },
            sessions: sessions.iter().map(|s| s.to_string()).collect(),
        };
        c.clients.rows.insert((claude(), "cli".into()), row(2_292, 424.0, &["a", "b"]));
        c.clients.rows.insert((claude(), "sdk-ts".into()), row(67_097, 8_534.0, &["b", "c", "d"]));
        c.misses = vec![MissEvent {
            agent: claude(),
            project: String::new(),
            session: "c".into(),
            chain: "main".into(),
            ts_ms: 0,
            model: "claude-opus-5-5".into(),
            cause: Cause::IdleTtl,
            detail: String::new(),
            context: 0,
            rewritten: 0,
            premium_usd: Some(7.41),
            client: "sdk-ts".into(),
        }];
        let text = clients(&c);
        assert!(text.starts_with("Spend by client: the program that ran the agent, as its log names it\n\n"), "{text}");
        let lines: Vec<Vec<&str>> = text.lines().skip(2).map(|l| l.split_whitespace().collect()).collect();
        assert_eq!(lines[0], ["agent", "client", "sessions", "requests", "cost", "share", "avoidable"]);
        assert_eq!(lines[1], ["claude", "sdk-ts", "3", "67097", "$8,534", "95.3%", "$7.41"]);
        assert_eq!(lines[2], ["claude", "cli", "2", "2292", "$424", "4.7%", "$0.00"]);
        // In the summary, after the totals and before the spend by origin.
        let out = summary(&c, &Filter::default(), Path::new("prices.toml"));
        let at = |s: &str| out.find(s).unwrap_or_else(|| panic!("{s} in {out}"));
        assert!(at("Total priced") < at("Spend by client") && at("Spend by client") < at("Spend by origin"));
    }

    #[test]
    fn a_trigger_row() {
        use tokprof_core::trigger::Trigger;
        let mut w = Work::default();
        let row = |requests: u64, avg: f64, usd: f64| TriggerSpend {
            spend: Spend { requests, tokens: avg * requests as f64, usd },
            prompt_tokens: avg * requests as f64,
        };
        w.by_trigger.insert(Trigger::Lookup, row(31_833, 263_000.0, 3_393.0));
        w.by_trigger.insert(Trigger::Prompt, row(10_000, 100_000.0, 5_703.0));
        let text = triggers(&w);
        assert!(text.starts_with("Spend by trigger: what made the agent send each request\n\n"));
        let line = text.lines().find(|l| l.starts_with("lookup")).unwrap();
        assert_eq!(line.split_whitespace().collect::<Vec<_>>(), ["lookup", "31833", "263.0k", "$3,393", "$0.11", "37.3%"]);
        let first = text.lines().nth(3).unwrap();
        assert!(first.starts_with("prompt"), "most expensive first: {first}");
    }

    #[test]
    fn the_tools_table_lists_rows_hints_and_the_total() {
        use tokprof_core::model::{Class, ToolClass};
        use tokprof_core::tools::ToolRow;
        let v = |usd| Values { tokens: 1.0, usd, avoidable: 0.0 };
        let mut t = ToolTable::default();
        t.rows.insert(
            ("Bash".into(), Some("python3 -I".into())),
            ToolRow {
                class: Class::Tool(ToolClass::Execute),
                calls: 60,
                single: 50,
                results: 60,
                result_tokens: 60.0 * 120.0,
                inputs: 60,
                input_tokens: 60.0 * 702.0,
                rent: v(4.32),
                trips: v(7.04),
                ..ToolRow::default()
            },
        );
        t.rows.insert(
            ("Read".into(), None),
            ToolRow {
                class: Class::Tool(ToolClass::Read),
                calls: 1,
                single: 1,
                results: 1,
                result_tokens: 16_900.0,
                inputs: 1,
                input_tokens: 20.0,
                rent: v(1.16),
                trips: v(0.2),
                ..ToolRow::default()
            },
        );
        t.spend = v(27.37);
        let text = tools(&t, RowOrder::Rent, 25);
        assert!(text.starts_with("Spend by tool: calls, sizes, what keeping them cost (rent) and what they made the agent do (trips)\n\n"));
        let py = text.lines().find(|l| l.contains("python3")).unwrap();
        assert!(py.contains("$4.32  $7.04     83%") && py.ends_with("  save long scripts to a file; edit files with the edit tool"), "{py}");
        let read = text.lines().find(|l| l.starts_with("Read")).unwrap();
        assert!(read.ends_with("$1.16  $0.20    100%"), "a large read once has no hint, and calls that all worked leave the failures cells empty: {read}");
        let by_trips = tools(&t, RowOrder::Trips, 1);
        assert!(by_trips.contains("1 more") && by_trips.lines().nth(3).unwrap().contains("python3"), "{by_trips}");
        assert!(
            text.contains("spend $27.37 · rent of these tools $5.48 · trips of these tools $7.24 (two views of the same money; do not add them)"),
            "{text}"
        );
        assert!(!text.contains("until the tool worked again"), "no failure, no failures line: {text}");
        let hint_at = |l: &str| l.find("save long").or_else(|| l.find("hint"));
        assert_eq!(hint_at(text.lines().nth(2).unwrap()), hint_at(py), "hints stay in their column");
    }

    #[test]
    fn the_tools_table_counts_failures_and_sorts_by_them() {
        use tokprof_core::model::{Class, ToolClass};
        use tokprof_core::tools::ToolRow;
        let v = |usd| Values { tokens: 1.0, usd, avoidable: 0.0 };
        let mut t = ToolTable::default();
        t.rows.insert(
            ("Bash".into(), Some("cargo test".into())),
            ToolRow { class: Class::Tool(ToolClass::Execute), calls: 9, single: 9, rent: v(3.0), trips: v(2.0), ..ToolRow::default() },
        );
        t.rows.insert(
            ("Edit".into(), None),
            ToolRow { class: Class::Tool(ToolClass::Edit), calls: 40, single: 30, rent: v(1.0), trips: v(1.0), failed: 2, after: v(1.5), ..ToolRow::default() },
        );
        t.spend = v(27.37);
        (t.failed, t.refused, t.after) = (2, 1, v(1.5));
        let text = tools(&t, RowOrder::Failed, 25);
        let header = text.lines().nth(2).unwrap();
        assert!(header.contains("single  failed  after"), "{header}");
        let first = text.lines().nth(3).unwrap();
        assert!(first.starts_with("Edit") && first.ends_with("2  $1.50"), "the row with failures comes first: {first}");
        assert!(text.ends_with("failed 2 calls, after them $1.50 until the tool worked again · you refused 1\n"), "{text}");
        (t.failed, t.refused, t.denied) = (0, 2, 0);
        assert_eq!(failures_line(&t).as_deref(), Some("you refused 2"));
        t.refused = 0;
        assert_eq!(failures_line(&t), None);
        t.failed = 1_021;
        assert!(failures_line(&t).unwrap().starts_with("failed 1,021 calls"));
    }

    fn claude_with_hints() -> AgentId {
        AgentId::new(AgentInfo {
            title: "Claude Code".into(),
            setup_hints: vec![("skills".into(), "lower skillListingBudgetFraction".into())],
            ..AgentInfo::named("claude")
        })
    }

    #[test]
    fn the_setup_table_lists_sources_hints_and_agent_lines() {
        use tokprof_core::setup::SetupCost;
        let claude = claude_with_hints();
        let v = |usd: f64| Values { tokens: usd * 5_000.0, usd, avoidable: 0.0 };
        let mut s = SetupSpend { priced: true, ..SetupSpend::default() };
        s.by_source.insert((claude.clone(), "skills".into()), SetupCost { all: v(197.0), subagents: v(150.0) });
        s.by_source.insert((claude.clone(), "instructions".into()), SetupCost { all: v(249.0), subagents: v(180.0) });
        s.spend.insert(claude.clone(), v(9_174.0));
        let text = setup(&s);
        assert!(text.starts_with("Setup: what every request carries before the work\n\n"), "{text}");
        let first = text.lines().nth(3).unwrap();
        assert!(first.starts_with("claude") && first.contains("instructions"), "most expensive first: {text}");
        let skills = text.lines().find(|l| l.contains("skills")).unwrap();
        assert_eq!(skills.split_whitespace().collect::<Vec<_>>(), ["claude", "skills", "$197", "$150", "2.1%", "lower", "skillListingBudgetFraction"]);
        assert!(text.contains("\nClaude Code setup: $446, 4.9% of its spend\n"), "{text}");
    }

    #[test]
    fn the_setup_table_groups_agents_and_folds_small_sources() {
        use tokprof_core::setup::SetupCost;
        let claude = claude_with_hints();
        let codex = AgentId::new(AgentInfo { title: "Codex".into(), ..AgentInfo::named("codex") });
        let v = |usd: f64| Values { tokens: usd * 5_000.0, usd, avoidable: 0.0 };
        let mut s = SetupSpend { priced: true, ..SetupSpend::default() };
        for (i, usd) in [40.0, 15.0, 14.0, 6.0, 5.0, 4.0, 3.0, 2.5, 2.0, 1.0].into_iter().enumerate() {
            s.by_source.insert((codex.clone(), format!("source {i}")), SetupCost { all: v(usd), subagents: Values::default() });
        }
        s.by_source.insert((claude.clone(), "instructions".into()), SetupCost { all: v(249.0), subagents: v(150.0) });
        s.by_source.insert((claude.clone(), "system prompt".into()), SetupCost { all: v(36.0), subagents: v(11.0) });
        s.spend.insert(claude.clone(), v(9_174.0));
        s.spend.insert(codex.clone(), v(1_930.0));
        let text = setup(&s);
        let rows: Vec<&str> = text.lines().skip(3).take_while(|l| !l.is_empty()).collect();
        assert_eq!(rows.len(), 2 + 8 + 1, "{text}");
        assert!(
            rows[0].starts_with("claude") && rows[0].contains("instructions") && rows[1].contains("system prompt"),
            "Claude Code first, though Codex has a $40 source: {text}"
        );
        assert!(rows[2].starts_with("codex") && rows[2].contains("source 0"), "{text}");
        assert_eq!(rows[10].split_whitespace().collect::<Vec<_>>(), ["codex", "2", "more", "$3.00", "$0.00", "0.2%"]);
        assert!(text.find("Claude Code setup:").unwrap() < text.find("Codex setup:").unwrap(), "{text}");
    }

    #[test]
    fn no_setup_table_without_setup() {
        assert_eq!(setup(&SetupSpend::default()), "");
    }

    #[test]
    fn an_activity_row() {
        let mut w = Work::default();
        let row = |requests: u64, avg: f64, usd: f64| TriggerSpend {
            spend: Spend { requests, tokens: avg * requests as f64, usd },
            prompt_tokens: avg * requests as f64,
        };
        w.by_activity.insert("reading", row(100, 50_000.0, 6.0));
        w.by_activity.insert("building and testing", row(10, 80_000.0, 4.0));
        let text = activities(&w);
        assert!(text.starts_with("Spend by activity: what the agent was doing before each request\n\n"));
        let line = text.lines().find(|l| l.starts_with("building")).unwrap();
        assert_eq!(line.split_whitespace().collect::<Vec<_>>(), ["building", "and", "testing", "10", "80.0k", "$4.00", "$0.40", "40.0%"]);
    }

    #[test]
    fn summary_totals_match_the_cause_table_and_the_window() {
        use tokprof_core::model::{Chain, Request};
        let session = |id: &str, times: &[i64], reported| {
            let mut s = Session::new(claude(), id.into());
            s.reported_usd = Some(reported);
            let requests = times
                .iter()
                .map(|&ts_ms| Request {
                    ts_ms,
                    model: "claude-opus-5-5".into(),
                    usage: Usage { cache_read: 1_000, output: 10, ..Usage::default() },
                    ..Request::default()
                })
                .collect();
            s.chains.push(Chain { name: "main".into(), kind: ChainKind::Main, requests });
            s
        };
        let pricing = Pricing::defaults();
        let window = Filter { since_ms: Some(10), ..Filter::default() };
        let mut c = Collector::new(None, None);
        for s in [session("inside", &[10, 20], 1.0), session("across", &[5, 20], 2.0)] {
            tokprof_core::analysis::analyze(&s, &pricing, &window, &mut c);
        }
        let miss = |cause, premium| MissEvent {
            agent: claude(),
            project: String::new(),
            session: "inside".into(),
            chain: "main".into(),
            ts_ms: 20,
            model: "claude-opus-5-5".into(),
            cause,
            detail: String::new(),
            context: 0,
            rewritten: 0,
            premium_usd: Some(premium),
            client: "unknown".into(),
        };
        c.misses = vec![miss(Cause::IdleTtl, 0.5), miss(Cause::Compaction, 0.2)];

        let out = summary(&c, &window, Path::new("prices.toml"));
        assert!(out.contains("avoidable on cache misses: $0.50"), "{out}");
        assert!(out.contains("own cost counter for 1 session inside the window: $1.00;"), "{out}");
        let all = summary(&c, &Filter::default(), Path::new("prices.toml"));
        assert!(all.contains("own cost counter for 2 sessions: $3.00;"), "{all}");
        // No session lies wholly inside a window that ends before the last request.
        let early = Filter { until_ms: Some(15), ..Filter::default() };
        assert!(!summary(&c, &early, Path::new("prices.toml")).contains("own cost counter"));
    }

    #[test]
    fn the_setup_table_comes_after_the_activities_and_before_the_misses() {
        use tokprof_core::setup::SetupCost;
        let mut c = Collector::new(None, None);
        assert!(!summary(&c, &Filter::default(), Path::new("prices.toml")).contains("Setup:"), "no setup, no section");
        let v = Values { tokens: 5_000.0, usd: 1.0, avoidable: 0.0 };
        c.setup.priced = true;
        c.setup.by_source.insert((claude(), "instructions".into()), SetupCost { all: v, subagents: Values::default() });
        c.setup.spend.insert(claude(), Values { tokens: 50_000.0, usd: 10.0, avoidable: 0.0 });
        let out = summary(&c, &Filter::default(), Path::new("prices.toml"));
        let at = |title: &str| out.find(title).unwrap_or_else(|| panic!("{title} in {out}"));
        assert!(at("Spend by activity") < at("Setup: what every request carries before the work"));
        assert!(at("Setup: what every request carries before the work") < at("Cache misses by cause"));
    }
}
