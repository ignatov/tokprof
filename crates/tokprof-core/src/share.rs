//! `tokprof share`: one JSON file of aggregates over the selected requests that holds no names,
//! so that several users can compare and pool their numbers without showing their work.
//!
//! The file is built from the summary's collector, the pause report's and [`Share`], with only
//! allowed keys: model, client, cause, trigger, activity, setup source and effort names, and the
//! fixed range names. Origins are cut to their kind. Nothing else of the collectors is copied, so a
//! new field in one cannot leak into the file without a change here.

use std::collections::{BTreeMap, HashSet};

use serde_json::{Map, Value, json};

use crate::analysis::{Component, Fact, MissEvent, RequestRecord, Sink};
use crate::model::{ChainKind, Session};
use crate::pauses::{AgentPauses, Pauses, Starter};
use crate::profile::Values;
use crate::report::{Collector, Spend, Work};
use crate::source::AgentId;

/// The format number of the file; a change that renames or removes a field raises it.
pub const FORMAT: u64 = 1;

/// The setup sources the file names, as setup-breakdown names them; any other is `other`.
const SETUP_SOURCES: [&str; 7] = ["tool schemas", "system prompt", "instructions", "hook context", "deferred tool lists", "skills", "other"];

/// What the file adds to the reports for one agent.
#[derive(Clone, Debug, Default)]
pub struct AgentShare {
    /// Tokens written with the short and the long lifetime: main conversations first, then
    /// subagents and side requests.
    pub cache_writes: [[u64; 2]; 2],
    /// Requests and output tokens per effort level as the log names it, `unknown` for none: main
    /// conversations first, then the rest.
    pub effort: [BTreeMap<String, (u64, u64)>; 2],
    /// Spend by origin, trigger and activity, of this agent only.
    pub work: Work,
    /// Logs with a selected request, subagent logs included.
    pub logs: u64,
    /// The main sessions among them, by short id: subagent logs carry their main session's.
    pub main_sessions: HashSet<String>,
}

/// Collects what [`to_json`] needs beyond the summary and the pause report.
#[derive(Clone, Debug, Default)]
pub struct Share {
    pub agents: BTreeMap<AgentId, AgentShare>,
    /// The first and the last selected request.
    pub span_ms: Option<(i64, i64)>,
    /// The log being read: its agent, its short id, and whether a request of it counted yet.
    current: Option<(AgentId, String, bool)>,
}

impl Share {
    pub fn merge(&mut self, o: Share) {
        for (agent, a) in o.agents {
            let mine = self.agents.entry(agent).or_default();
            for (k, w) in a.cache_writes.iter().enumerate() {
                mine.cache_writes[k][0] += w[0];
                mine.cache_writes[k][1] += w[1];
            }
            for (k, levels) in a.effort.into_iter().enumerate() {
                for (level, (requests, output)) in levels {
                    let e = mine.effort[k].entry(level).or_default();
                    e.0 += requests;
                    e.1 += output;
                }
            }
            mine.work.merge(a.work);
            mine.logs += a.logs;
            mine.main_sessions.extend(a.main_sessions);
        }
        self.span_ms = match (self.span_ms, o.span_ms) {
            (Some(a), Some(b)) => Some((a.0.min(b.0), a.1.max(b.1))),
            (a, b) => a.or(b),
        };
    }
}

impl Sink for Share {
    fn session(&mut self, s: &Session) {
        self.current = Some((s.agent.clone(), s.short_id().to_string(), false));
    }

    fn request(&mut self, r: &RequestRecord) {
        let Share { agents, span_ms, current } = self;
        let Some((agent, session, counted)) = current else { return };
        let a = agents.entry(agent.clone()).or_default();
        if !*counted {
            a.logs += 1;
            a.main_sessions.insert(session.clone());
            *counted = true;
        }
        let k = (r.chain_kind != ChainKind::Main) as usize;
        a.cache_writes[k][0] += r.usage.write_short;
        a.cache_writes[k][1] += r.usage.write_long;
        let e = a.effort[k].entry(r.effort.unwrap_or("unknown").to_string()).or_default();
        e.0 += 1;
        e.1 += r.usage.output;
        a.work.add(r);
        *span_ms = Some(span_ms.map_or((r.ts_ms, r.ts_ms), |(first, last)| (first.min(r.ts_ms), last.max(r.ts_ms))));
    }

    fn miss(&mut self, _: MissEvent) {}

    fn fact(&mut self, _: &Fact) {}
}

fn dollars(usd: f64) -> Value {
    json!((usd * 1e4).round() / 1e4)
}

fn whole(tokens: f64) -> Value {
    json!(tokens.round() as u64)
}

fn amount(v: &Values) -> Value {
    json!({"usd": dollars(v.usd), "tokens": whole(v.tokens)})
}

fn spend(s: &Spend) -> Value {
    json!({"requests": s.requests, "usd": dollars(s.usd), "tokens": whole(s.tokens)})
}

/// A cache lifetime as the file names it: `5m`, `1h`, `30m`.
fn lifetime_key(ms: i64) -> String {
    if ms % 3_600_000 == 0 { format!("{}h", ms / 3_600_000) } else { format!("{}m", ms / 60_000) }
}

fn pause_parts(p: &AgentPauses) -> Value {
    let typed: Map<String, Value> =
        p.pause_rows().into_iter().map(|r| (r.name.to_string(), json!({"messages": r.count, "median_context": r.median_context}))).collect();
    let after: Map<String, Value> =
        p.lifetimes.iter().map(|&ms| (lifetime_key(ms), json!(p.typed.iter().filter(|(pause, _)| *pause > ms).count()))).collect();
    let returns: Map<String, Value> = p
        .return_rows()
        .into_iter()
        .map(|r| (r.name.to_string(), json!({"returns": r.count, "usd": dollars(r.usd), "avoidable_usd": dollars(r.premium_usd), "median_context": r.median_context})))
        .collect();
    let starters = p.starters();
    let names = ["typed messages", "background tasks", "tool results", "other"];
    let started: Map<String, Value> = Starter::ALL.iter().zip(names).map(|(s, name)| (name.to_string(), json!(starters[*s as usize]))).collect();
    let follow = p.follow_stats().map(|(median, q1, q3, short)| json!({"median": median, "quarter_1": q1, "quarter_3": q3, "ended_within_3": short}));
    let sizes: Map<String, Value> = p
        .size_rows()
        .into_iter()
        .map(|r| (r.name.to_string(), json!({"sessions": r.count, "usd": dollars(r.usd), "tokens": whole(r.tokens), "cold_returns": r.cold_returns})))
        .collect();
    json!({
        "typed_messages": p.typed.len(),
        "typed_by_pause": typed,
        "typed_after_lifetime": after,
        "cold_returns_by_pause": returns,
        "cold_returns_started_by": started,
        "after_cold_return": follow,
        "sessions_by_largest_context": sizes,
    })
}

/// The file's text: the value as indented JSON, ending with a newline.
pub fn pretty(v: &Value) -> String {
    let mut text = serde_json::to_string_pretty(v).unwrap_or_default();
    text.push('\n');
    text
}

/// The file's content, from one scan into the three collectors with the same filters.
/// `tokprof` is the version that writes it; `day` turns epoch ms into `YYYY-MM-DD`.
pub fn to_json(c: &Collector, p: &Pauses, s: &Share, tokprof: &str, day: impl Fn(i64) -> String) -> Value {
    let mut agents = Map::new();
    for (agent, a) in &s.agents {
        // Spend by model and component, as the summary's model table counts it.
        let mut models = Map::new();
        for ((who, model), m) in &c.spend.models {
            if who != agent {
                continue;
            }
            let components: Map<String, Value> = Component::ALL.iter().zip(&m.by_component).map(|(comp, v)| (comp.name().to_string(), amount(v))).collect();
            let mut total = Values::default();
            m.by_component.iter().for_each(|v| total.add(*v));
            models.insert(model.clone(), json!({"requests": m.requests, "priced": m.priced, "components": components, "total": amount(&total)}));
        }
        // The misses the cause table lists, and their premium per client.
        let mut causes: BTreeMap<&str, (u64, u64, f64)> = BTreeMap::new();
        let mut avoidable: BTreeMap<&str, f64> = BTreeMap::new();
        for m in c.misses.iter().filter(|m| &m.agent == agent && !m.cause.expected()) {
            let e = causes.entry(m.cause.name()).or_default();
            e.0 += 1;
            e.1 += m.rewritten;
            e.2 += m.premium_usd.unwrap_or(0.0);
            *avoidable.entry(m.client.as_str()).or_default() += m.premium_usd.unwrap_or(0.0);
        }
        let misses: Map<String, Value> =
            causes.into_iter().map(|(cause, (n, tokens, usd))| (cause.to_string(), json!({"misses": n, "tokens": tokens, "avoidable_usd": dollars(usd)}))).collect();
        let clients: Map<String, Value> = c
            .clients
            .rows
            .iter()
            .filter(|((who, _), _)| who == agent)
            .map(|((_, client), r)| (client.clone(), json!({"requests": r.spend.requests, "usd": dollars(r.spend.usd), "avoidable_usd": dollars(avoidable.get(client.as_str()).copied().unwrap_or(0.0))})))
            .collect();
        // Origins by kind: the part before `:`, so no skill, MCP server, plugin, command or
        // subagent-type name goes into the file.
        let mut origins: BTreeMap<String, Spend> = BTreeMap::new();
        for (origin, sp) in &a.work.by_origin {
            origins.entry(origin.split(':').next().unwrap_or("other").to_string()).or_default().add(*sp);
        }
        let origins: Map<String, Value> = origins.iter().map(|(kind, sp)| (kind.clone(), spend(sp))).collect();
        let triggers: Map<String, Value> = a.work.triggers().iter().map(|(t, sp)| (t.name().to_string(), spend(&sp.spend))).collect();
        let activities: Map<String, Value> = a.work.activities().iter().map(|(name, sp)| (name.to_string(), spend(&sp.spend))).collect();
        // Setup by source, from the fixed list.
        let mut setup: BTreeMap<&str, (Values, Values)> = BTreeMap::new();
        for ((who, source), cost) in &c.setup.by_source {
            if who != agent {
                continue;
            }
            let name = SETUP_SOURCES.iter().copied().find(|s| s == source).unwrap_or("other");
            let e = setup.entry(name).or_default();
            e.0.add(cost.all);
            e.1.add(cost.subagents);
        }
        let setup: Map<String, Value> = setup
            .into_iter()
            .map(|(name, (all, subagents))| (name.to_string(), json!({"usd": dollars(all.usd), "tokens": whole(all.tokens), "subagents_usd": dollars(subagents.usd)})))
            .collect();
        let effort = |levels: &BTreeMap<String, (u64, u64)>| -> Map<String, Value> {
            levels.iter().map(|(level, (requests, output))| (level.clone(), json!({"requests": requests, "output_tokens": output}))).collect()
        };
        let writes = |w: [u64; 2]| json!({"short": w[0], "long": w[1]});
        let mut entry = json!({
            "sessions": a.logs,
            "main_sessions": a.main_sessions.len(),
            "spend": models,
            "clients": clients,
            "cache_writes": {"main": writes(a.cache_writes[0]), "subagents": writes(a.cache_writes[1])},
            "misses": misses,
            "origins": origins,
            "triggers": triggers,
            "activities": activities,
            "setup": setup,
            "effort": {"main": effort(&a.effort[0]), "subagents": effort(&a.effort[1])},
        });
        if let (Some(pauses), Value::Object(e)) = (p.agents.get(agent), &mut entry) {
            e.insert("pauses".into(), pause_parts(pauses));
        }
        agents.insert(agent.name().to_string(), entry);
    }
    json!({
        "format": FORMAT,
        "tokprof": tokprof,
        "window": s.span_ms.map(|(first, last)| json!({"first": day(first), "last": day(last)})),
        "agents": agents,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::Filter;
    use crate::model::{Block, BlockKind, Chain, Class, Origin, Params, PromptDelta, Request, Usage};
    use crate::pricing::Pricing;

    fn request(min: i64, chain_effort: Option<&str>, usage: Usage) -> Request {
        Request {
            ts_ms: 1_793_620_800_000 + min * 60_000,
            api: crate::api::ApiId::Anthropic,
            model: "claude-opus-5-5".into(),
            usage,
            params: chain_effort.map(|e| Params { effort: Some(e.into()), ..Params::default() }),
            ..Request::default()
        }
    }

    fn session(agent: AgentId, id: &str, chains: Vec<(&str, ChainKind, Vec<Request>)>) -> Session {
        let mut s = Session::new(agent, id.into());
        s.chains = chains.into_iter().map(|(name, kind, requests)| Chain { name: name.into(), kind, requests }).collect();
        s
    }

    /// The three collectors of one scan, over the given sessions.
    fn collect(sessions: &[Session]) -> (Collector, Pauses, Share) {
        let pricing = Pricing::defaults();
        let filter = Filter::default();
        let mut sink = (Collector::new(None, None), (Pauses::new(filter.clone()), Share::default()));
        for s in sessions {
            crate::analysis::analyze(s, &pricing, &filter, &mut sink);
        }
        let (c, (mut p, s)) = sink;
        p.finish();
        (c, p, s)
    }

    fn file(sessions: &[Session]) -> Value {
        let (c, p, s) = collect(sessions);
        to_json(&c, &p, &s, "0.1.0", |_| "2026-11-02".into())
    }

    #[test]
    fn cache_writes_by_lifetime_and_effort_by_level() {
        let main = vec![
            request(0, Some("max"), Usage { write_long: 190_000, output: 10, ..Usage::default() }),
            request(1, Some("max"), Usage { cache_read: 190_000, write_long: 700, output: 20, ..Usage::default() }),
            request(2, None, Usage { cache_read: 190_700, output: 5, ..Usage::default() }),
        ];
        let sub = vec![request(3, Some("high"), Usage { write_long: 154_000, write_short: 300, output: 7, ..Usage::default() })];
        let s = session(crate::testing::claude(), "s", vec![("main", ChainKind::Main, main), ("subagent", ChainKind::Subagent, sub)]);
        let (_, _, share) = collect(&[s]);
        let a = &share.agents[&crate::testing::claude()];
        assert_eq!(a.cache_writes, [[0, 190_700], [300, 154_000]]);
        assert_eq!(a.effort[0]["max"], (2, 30));
        assert_eq!(a.effort[0]["unknown"], (1, 5));
        assert_eq!(a.effort[1]["high"], (1, 7));
    }

    #[test]
    fn a_session_with_subagents_counts_its_logs_and_one_main_session() {
        let one = || vec![request(0, None, Usage { cache_read: 1_000, output: 1, ..Usage::default() })];
        let mut logs = vec![session(crate::testing::claude(), "s", vec![("main", ChainKind::Main, one())])];
        for _ in 0..9 {
            logs.push(session(crate::testing::claude(), "s", vec![("subagent", ChainKind::Subagent, one())]));
        }
        // A log with no selected request counts in neither.
        logs.push(session(crate::testing::claude(), "t", vec![("main", ChainKind::Main, vec![request(0, None, Usage::default())])]));
        let v = file(&logs);
        assert_eq!((v["agents"]["claude"]["sessions"].as_u64(), v["agents"]["claude"]["main_sessions"].as_u64()), (Some(10), Some(1)));
    }

    #[test]
    fn the_file_has_its_header_and_one_object_per_agent() {
        let one = || vec![request(0, None, Usage { cache_read: 1_000, output: 1, ..Usage::default() })];
        let v = file(&[session(crate::testing::claude(), "s", vec![("main", ChainKind::Main, one())]), session(crate::testing::codex(), "c", vec![("main", ChainKind::Main, one())])]);
        assert_eq!(v["format"], 1);
        assert_eq!(v["tokprof"], "0.1.0");
        assert_eq!(v["window"], json!({"first": "2026-11-02", "last": "2026-11-02"}));
        let keys: Vec<&String> = v["agents"].as_object().unwrap().keys().collect();
        assert_eq!(keys, ["claude", "codex"]);
    }

    #[test]
    fn origins_are_cut_to_their_kind_and_clients_keep_their_names() {
        let with = |origin: &str, client: &str| Request {
            origin: Some(Origin::parse(origin)),
            client: Some(client.into()),
            ..request(0, None, Usage { cache_read: 1_000_000, output: 10, ..Usage::default() })
        };
        let s = session(crate::testing::claude(), "s", vec![("main", ChainKind::Main, vec![with("skill:code-review", "sdk-ts"), with("skill:release-notes", "sdk-ts"), with("agent:general-purpose", "cli")])]);
        let v = file(&[s]);
        let a = &v["agents"]["claude"];
        let origins: Vec<&String> = a["origins"].as_object().unwrap().keys().collect();
        assert_eq!(origins, ["agent", "skill"]);
        assert_eq!(a["origins"]["skill"]["requests"], 2);
        let clients = a["clients"].as_object().unwrap();
        assert_eq!((clients["sdk-ts"]["requests"].as_u64(), clients["cli"]["requests"].as_u64()), (Some(2), Some(1)));
        let text = v.to_string();
        assert!(!text.contains("code-review") && !text.contains("release-notes") && !text.contains("general-purpose"), "{text}");
    }

    #[test]
    fn the_numbers_are_the_summarys_and_the_pause_reports() {
        // A cold return after two hours, so misses and pauses have something to say.
        let main = vec![
            request(0, None, Usage { write_long: 500_000, output: 100, ..Usage::default() }),
            request(120, None, Usage { cache_read: 10_000, write_long: 492_000, output: 100, ..Usage::default() }),
        ];
        let sessions = [session(crate::testing::claude(), "s", vec![("main", ChainKind::Main, main)])];
        let (c, p, s) = collect(&sessions);
        let v = to_json(&c, &p, &s, "0.1.0", |_| String::new());
        let a = &v["agents"]["claude"];
        let model = &c.spend.models[&(crate::testing::claude(), "claude-opus-5-5".to_string())];
        let total: f64 = model.by_component.iter().map(|x| x.usd).sum();
        assert_eq!(a["spend"]["claude-opus-5-5"]["total"]["usd"], dollars(total));
        let miss = &c.misses[0];
        assert_eq!(a["misses"]["idle > TTL"], json!({"misses": 1, "tokens": miss.rewritten, "avoidable_usd": dollars(miss.premium_usd.unwrap())}));
        let pauses = &p.agents[&crate::testing::claude()];
        // Two hours is the lower bound of `2–4h`; empty ranges are left out, as in the report.
        assert_eq!(a["pauses"]["cold_returns_by_pause"]["1–2h"], Value::Null, "{a}");
        assert_eq!(a["pauses"]["cold_returns_by_pause"]["2–4h"]["returns"], pauses.return_rows()[0].count);
        assert_eq!(a["pauses"]["typed_after_lifetime"], json!({"5m": 0, "1h": 0}), "no typed message");
    }

    #[test]
    fn the_file_holds_no_names() {
        let mut s = session(crate::testing::claude(), "5e5510f0-zebra", vec![]);
        s.project = "~/src/zebra-billing".into();
        s.cwd = Some("/Users/me/src/zebra-billing".into());
        s.title = Some("Fix quokka export".into());
        s.task = Some("ask the walrus".into());
        let block = |kind, tool: Option<&str>, target: Option<&str>| Block { class: Class::None, kind, chars: 4_000, tool: tool.map(str::to_string), target: target.map(str::to_string) };
        let prompt = s.push_message(vec![block(BlockKind::Prompt, None, Some("ping the yak"))], true);
        let read = s.push_message(vec![block(BlockKind::ToolResult, Some("Read"), Some("/tmp/heron.txt"))], false);
        let reminder = s.push_message(vec![block(BlockKind::Reminder, None, Some("narwhal server notes"))], false);
        let r = |min, keep, tail: Vec<usize>, origin: &str| Request {
            origin: Some(Origin::parse(origin)),
            prompt: Some(PromptDelta { shape: None, keep, tail }),
            ..request(min, Some("max"), Usage { cache_read: 10_000, write_long: 2_000, output: 10, ..Usage::default() })
        };
        s.chains = vec![Chain {
            name: "main".into(),
            kind: ChainKind::Main,
            requests: vec![r(0, 0, vec![prompt], "skill:walrus-review"), r(1, 1, vec![read], "mcp:narwhal"), r(2, 2, vec![reminder], "agent:ocelot")],
        }];
        let text = serde_json::to_string_pretty(&file(&[s])).unwrap();
        for word in ["zebra", "quokka", "walrus", "narwhal", "ocelot", "heron", "yak", "5e5510f0", "/Users/", "~/"] {
            assert!(!text.contains(word), "{word} leaked into {text}");
        }
    }
}
