//! The text of `tokprof summary`, `misses`, `whatif` and `prices`.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::Path;

use tokprof_core::analysis::{Component, Filter, MissEvent};
use tokprof_core::analysis::cache::Cause;
use tokprof_core::api::ApiId;
use tokprof_core::fmt::{clip_end, table, tokens, usd};
use tokprof_core::pricing::{Price, Pricing};
use tokprof_core::profile::Values;
use tokprof_core::report::{Collector, ModelTotals, Spend, Work};
use tokprof_core::source::AgentId;

/// `window` is the time filter the requests were selected by; it limits the reconciliation to
/// the sessions it holds whole.
/// `price_file` is the file the hint for models without a price names.
pub fn summary(c: &Collector, window: &Filter, price_file: &Path) -> String {
    let mut out = String::new();
    let mut rows = vec![
        ["agent", "model", "requests", "prompt known"].iter().map(|s| s.to_string())
            .chain(Component::ALL.iter().map(|c| c.name().to_string()))
            .chain(["total".to_string()])
            .collect::<Vec<_>>(),
    ];
    let mut total = 0.0;
    let mut unpriced: Vec<(String, f64)> = Vec::new();
    let mut models: Vec<_> = c.spend.models.iter().collect();
    models.sort_by(|a, b| {
        let cost = |m: &ModelTotals| m.by_component.iter().map(|v| v.usd).sum::<f64>();
        cost(b.1).total_cmp(&cost(a.1)).then(a.0.cmp(b.0))
    });
    for ((agent, model), m) in models {
        let sum: f64 = m.by_component.iter().map(|v| v.usd).sum();
        let cell = |v: &Values| if m.priced { usd(v.usd) } else { tokens(v.tokens) };
        let mut row = vec![
            agent.name().to_string(),
            model.clone(),
            m.requests.to_string(),
            format!("{:.0}%", 100.0 * m.with_prompt as f64 / m.requests.max(1) as f64),
        ];
        row.extend(m.by_component.iter().map(cell));
        row.push(if m.priced { usd(sum) } else { format!("{} tok", tokens(m.by_component.iter().map(|v| v.tokens).sum())) });
        rows.push(row);
        if m.priced {
            total += sum;
        } else {
            unpriced.push((model.clone(), m.by_component.iter().map(|v| v.tokens).sum::<f64>()));
        }
    }
    let _ = writeln!(out, "Spend by model (API-equivalent USD; token counts where no price is known)\n");
    out.push_str(&table(&rows, 2));
    // The premium of the misses the cause table lists, so expected rebuilds are left out of both.
    let avoidable: f64 = c.misses.iter().filter(|m| !m.cause.expected()).filter_map(|m| m.premium_usd).sum();
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
    out.push_str(&work(&c.work, 15, 15));
    out.push('\n');
    out.push_str(&triggers(&c.work));
    out.push('\n');
    out.push_str(&miss_causes(c, false));
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
    let rows_in = w.triggers();
    let priced = rows_in.iter().any(|(_, t)| t.spend.usd > 0.0);
    let key = |s: &Spend| if priced { s.usd } else { s.tokens };
    let total: f64 = rows_in.iter().map(|(_, t)| key(&t.spend)).sum();
    let mut out = String::from("Spend by trigger: what made the agent send each request\n\n");
    let mut rows = vec![["trigger", "requests", "avg prompt", "cost", "per request", "share"].map(str::to_string).to_vec()];
    for (trigger, t) in rows_in {
        let s = t.spend;
        let n = s.requests.max(1) as f64;
        let (cost, per) = if priced { (usd(s.usd), usd(s.usd / n)) } else { (format!("{} tok", tokens(s.tokens)), format!("{} tok", tokens(s.tokens / n))) };
        rows.push(vec![trigger.name().to_string(), s.requests.to_string(), tokens(t.avg_prompt()), cost, per, format!("{:.1}%", 100.0 * key(&s) / total.max(1e-9))]);
    }
    out.push_str(&table(&rows, 1));
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
    events.sort_by(|a, b| {
        b.premium_usd.unwrap_or(0.0).total_cmp(&a.premium_usd.unwrap_or(0.0)).then(b.rewritten.cmp(&a.rewritten))
    });
    let mut rows = vec![["when", "agent", "project", "session", "chain", "context", "re-processed", "avoidable", "cause"]
        .map(str::to_string)
        .to_vec()];
    for m in events.into_iter().take(top) {
        let when = chrono::DateTime::from_timestamp_millis(m.ts_ms)
            .map(|t| t.with_timezone(&chrono::Local).format("%Y-%m-%d %H:%M").to_string())
            .unwrap_or_default();
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
        rows.push(vec![
            w.requests[i].to_string(),
            name.to_string(),
            usd(w.actual[i]),
            usd(w.simulated[i][0]),
            usd(w.simulated[i][1]),
        ]);
    }
    out.push_str(&table(&rows, 0));
    let best = |i: usize| if w.simulated[i][0] < w.simulated[i][1] { "5m" } else { "1h" };
    let saving = (0..2).map(|i| w.simulated[i][1].max(w.simulated[i][0]) - w.simulated[i][1].min(w.simulated[i][0])).collect::<Vec<_>>();
    let _ = writeln!(
        out,
        "\nCheapest: main conversation {} (saves {} over the other), subagents {} (saves {}).",
        best(0),
        usd(saving[0]),
        best(1),
        usd(saving[1])
    );
    for agent in agents {
        if let Some((main, sub)) = &agent.info().ttl_settings {
            let _ = writeln!(out, "{} settings: {main} and {sub} (\"5m\" or \"1h\").", agent.info().title);
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
    use tokprof_core::source::AgentInfo;

    fn claude() -> AgentId {
        AgentId::new(AgentInfo { cost_counter: Some("Claude Code's own cost counter".into()), ..AgentInfo::named("claude") })
    }


    #[test]
    fn a_trigger_row() {
        use tokprof_core::report::TriggerSpend;
        use tokprof_core::trigger::Trigger;
        let mut w = Work::default();
        let row = |requests: u64, avg: f64, usd: f64| TriggerSpend { spend: Spend { requests, tokens: avg * requests as f64, usd }, prompt_tokens: avg * requests as f64 };
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
    fn summary_totals_match_the_cause_table_and_the_window() {
        use tokprof_core::model::{Chain, Request};
        let session = |id: &str, times: &[i64], reported| {
            let mut s = Session::new(claude(), id.into());
            s.reported_usd = Some(reported);
            let requests = times
                .iter()
                .map(|&ts_ms| Request { ts_ms, model: "claude-opus-5-5".into(), usage: Usage { cache_read: 1_000, output: 10, ..Usage::default() }, ..Request::default() })
                .collect();
            s.chains.push(Chain { name: "main".into(), kind: ChainKind::Main, requests });
            s
        };
        let pricing = Pricing::defaults();
        let window = Filter { since_ms: Some(10), until_ms: None };
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
        };
        c.misses = vec![miss(Cause::IdleTtl, 0.5), miss(Cause::Compaction, 0.2)];

        let out = summary(&c, &window, Path::new("prices.toml"));
        assert!(out.contains("avoidable on cache misses: $0.50"), "{out}");
        assert!(out.contains("own cost counter for 1 session inside the window: $1.00;"), "{out}");
        let all = summary(&c, &Filter::default(), Path::new("prices.toml"));
        assert!(all.contains("own cost counter for 2 sessions: $3.00;"), "{all}");
        // No session lies wholly inside a window that ends before the last request.
        let early = Filter { since_ms: None, until_ms: Some(15) };
        assert!(!summary(&c, &early, Path::new("prices.toml")).contains("own cost counter"));
    }
}
