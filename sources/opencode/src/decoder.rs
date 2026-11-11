//! The decoder of an OpenCode session: the records the store gives for it (see `store`), the
//! session first, then one per message, oldest first.

use std::path::PathBuf;

use serde_json::Value;
use tokprof_core::api::ApiId;
use tokprof_core::labels::display_path;
use tokprof_core::model::{Chain, ChainKind, Compaction, CompactionTrigger, Request, Session, Usage};
use tokprof_core::source::{AgentId, Decoder};
use tokprof_logio::str_at;

#[derive(Clone)]
pub(crate) struct Parser {
    session: Session,
    home: PathBuf,
    /// The id the session is reported under: its own, or for a subagent its main session's.
    report_as: String,
    subagent: bool,
    /// The OpenCode agent that ran the session, such as `explore`.
    agent_name: Option<String>,
    requests: Vec<Request>,
    /// The model of the latest request, for a compaction that names none.
    model: Option<(String, ApiId)>,
    /// A compaction came after the latest request.
    compacted: bool,
    /// The sum of `cost`, when a request carried one.
    cost: Option<f64>,
}

impl Parser {
    pub(crate) fn new(agent: AgentId, report_as: String, subagent: bool, home: PathBuf) -> Parser {
        Parser {
            session: Session::new(agent, report_as.clone()),
            home,
            report_as,
            subagent,
            agent_name: None,
            requests: Vec::new(),
            model: None,
            compacted: false,
            cost: None,
        }
    }

    fn record(&mut self, record: &str) {
        let Ok(v) = serde_json::from_str::<Value>(record) else { return };
        if let Some(s) = v.get("session") {
            self.session_row(s);
        } else if let Some(m) = v.get("message") {
            self.message(m);
        }
    }

    fn session_row(&mut self, s: &Value) {
        if let Some(dir) = str_at(s, "directory").filter(|d| !d.is_empty()) {
            self.session.project = display_path(dir, None, &self.home);
            self.session.cwd = Some(dir.to_string());
        }
        self.agent_name = str_at(s, "agent").map(str::to_string);
        let title = str_at(s, "title").map(str::to_string);
        if self.subagent {
            self.session.task = title.map(|t| task_of(&t));
        } else {
            self.session.title = title;
            if let Some(id) = str_at(s, "id") {
                self.report_as = id.to_string();
                self.session.id = id.to_string();
            }
        }
    }

    fn message(&mut self, m: &Value) {
        let data = m.get("data").unwrap_or(&Value::Null);
        let created = m.get("created").and_then(Value::as_i64).unwrap_or(0);
        let updated = m.get("updated").and_then(Value::as_i64).unwrap_or(created);
        let v1 = str_at(m, "layout") == Some("v1");
        match str_at(m, "type").unwrap_or_default() {
            "assistant" => {
                if self.agent_name.is_none() {
                    self.agent_name = str_at(data, "agent").map(str::to_string);
                }
                let model = if v1 {
                    (str_at(data, "modelID"), str_at(data, "providerID"))
                } else {
                    let model = data.get("model").unwrap_or(&Value::Null);
                    (str_at(model, "id"), str_at(model, "providerID"))
                };
                let model = model_of(model.0, model.1);
                let summary = v1 && data.get("summary").and_then(Value::as_bool) == Some(true);
                let time = data.get("time").unwrap_or(&Value::Null);
                let sent = time.get("created").and_then(Value::as_i64).unwrap_or(created);
                let ended = time.get("completed").and_then(Value::as_i64).unwrap_or(updated);
                let steps = m.get("steps").and_then(Value::as_array).filter(|s| !s.is_empty());
                match steps {
                    Some(steps) => {
                        for step in steps {
                            let end = step.get("end").and_then(Value::as_i64).unwrap_or(ended);
                            let start = step.get("start").and_then(Value::as_i64).unwrap_or(sent);
                            self.request(step.get("tokens"), step.get("cost"), start, end, model.clone(), summary);
                        }
                    }
                    None => self.request(data.get("tokens"), data.get("cost"), sent, ended, model.clone(), summary),
                }
                self.model = Some(model);
            }
            "compaction" => {
                // 2.x: the compaction and its own call in one message.
                let named = data
                    .get("model")
                    .and_then(|model| str_at(model, "id"))
                    .map(|id| model_of(Some(id), data.get("model").and_then(|m| str_at(m, "providerID"))));
                let model = named.or_else(|| self.model.clone()).unwrap_or_else(|| ("unknown".to_string(), ApiId::Unknown));
                let status = str_at(data, "status");
                if matches!(status, Some("completed" | "failed")) {
                    self.request(data.get("tokens"), data.get("cost"), created, updated, model, true);
                }
                if status == Some("completed") {
                    let kind = match str_at(data, "reason") {
                        Some("auto") => CompactionTrigger::Auto,
                        Some("manual") => CompactionTrigger::Manual,
                        _ => CompactionTrigger::Unknown,
                    };
                    self.mark(created, kind);
                }
            }
            "user" => {
                // 1.x: a user message whose `compaction` part asked for one; the summary follows.
                if let Some(auto) = m.get("compaction").and_then(|c| c.get("auto")).and_then(Value::as_bool) {
                    self.mark(created, if auto { CompactionTrigger::Auto } else { CompactionTrigger::Manual });
                }
            }
            _ => {}
        }
    }

    fn mark(&mut self, at: i64, kind: CompactionTrigger) {
        self.compacted = true;
        self.session.compactions.push(Compaction { ts_ms: at, kind });
    }

    /// One request, when its usage is not all zero. A compaction call keeps the mark of a
    /// compaction for the request after it.
    fn request(&mut self, tokens: Option<&Value>, cost: Option<&Value>, sent: i64, ended: i64, (model, api): (String, ApiId), compaction_call: bool) {
        let Some(t) = tokens else { return };
        let cache = t.get("cache").unwrap_or(&Value::Null);
        let (input, output, reasoning, read, written) = (num(t, "input"), num(t, "output"), num(t, "reasoning"), num(cache, "read"), num(cache, "write"));
        if input + output + reasoning + read + written == 0 {
            return;
        }
        if let Some(c) = cost.and_then(Value::as_f64) {
            *self.cost.get_or_insert(0.0) += c;
        }
        self.requests.push(Request {
            ts_ms: ended,
            api,
            model,
            usage: Usage {
                uncached: input,
                cache_read: read,
                // OpenCode asks for the provider's default lifetime.
                write_short: written,
                write_long: 0,
                // `reasoning` comes on top of `output` in OpenCode's counts.
                output: output + reasoning,
                reasoning,
            },
            compacted_before: !compaction_call && self.compacted,
            compaction_call,
            first_sent_ms: Some(sent.min(ended)),
            sent_ms: Some(sent.min(ended)),
            ended_ms: Some(ended),
            ..Request::default()
        });
        if !compaction_call {
            self.compacted = false;
        }
    }

    pub(crate) fn into_session(mut self) -> Session {
        self.requests.sort_by_key(|r| r.ts_ms);
        let (name, kind) = if self.subagent {
            let name = match (&self.session.task, &self.agent_name) {
                (Some(task), _) => format!("subagent: {task}"),
                (None, Some(agent)) => format!("subagent:{agent}"),
                (None, None) => "subagent".to_string(),
            };
            (name, ChainKind::Subagent)
        } else {
            ("main".to_string(), ChainKind::Main)
        };
        self.session.id = self.report_as;
        self.session.chains.push(Chain { name, kind, requests: self.requests });
        self.session.compactions.sort_by_key(|c| c.ts_ms);
        self.session.reported_usd = self.cost;
        self.session
    }
}

impl Decoder for Parser {
    fn feed(&mut self, record: &str) {
        self.record(record);
    }

    fn snapshot(&self) -> Session {
        self.clone().into_session()
    }

    fn finish(self: Box<Self>) -> Session {
        (*self).into_session()
    }
}

/// A count, which OpenCode writes as a JSON number.
fn num(v: &Value, key: &str) -> u64 {
    match v.get(key) {
        Some(n) => n.as_u64().or_else(|| n.as_f64().filter(|f| f.is_finite() && *f > 0.0).map(|f| f.round() as u64)).unwrap_or(0),
        None => 0,
    }
}

/// The model's name, after the last `/` of a router's id, and the API whose cache rules apply.
fn model_of(id: Option<&str>, provider: Option<&str>) -> (String, ApiId) {
    let id = id.filter(|i| !i.is_empty()).unwrap_or("unknown");
    let name = id.rsplit('/').next().unwrap_or(id).to_string();
    let api = match provider {
        Some("anthropic") => ApiId::Anthropic,
        Some("openai") => ApiId::OpenAi,
        _ if name.starts_with("claude") => ApiId::Anthropic,
        _ if name.starts_with("gpt-") => ApiId::OpenAi,
        _ => ApiId::Unknown,
    };
    (name, api)
}

/// What a subagent was asked to do: its session's title without ` (@<agent> subagent)`.
fn task_of(title: &str) -> String {
    match title.rfind(" (@") {
        Some(i) if title.ends_with(" subagent)") => title[..i].to_string(),
        _ => title.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn decode(subagent: bool, records: &[Value]) -> Session {
        let mut p = Parser::new(crate::OpenCode::agent(), "ses_main".into(), subagent, PathBuf::from("/h"));
        p.record(
            &json!({"session": {"id": "ses_own", "directory": "/h/src/app", "title": "Fix the build (@general subagent)", "agent": null, "created": 0}})
                .to_string(),
        );
        records.iter().for_each(|r| p.record(&r.to_string()));
        p.into_session()
    }

    fn tokens(input: u64, output: u64, reasoning: u64, read: u64, write: u64) -> Value {
        json!({"input": input, "output": output, "reasoning": reasoning, "cache": {"read": read, "write": write}})
    }

    fn v2(id: &str, at: i64, model: &str, provider: &str, t: Value, cost: f64) -> Value {
        json!({"message": {"id": id, "layout": "v2", "type": "assistant", "created": at, "updated": at + 900,
            "data": {"agent": "build", "model": {"id": model, "providerID": provider}, "tokens": t, "cost": cost, "time": {"created": at, "completed": at + 800}}}})
    }

    fn v1(id: &str, at: i64, steps: Vec<Value>, summary: bool) -> Value {
        json!({"message": {"id": id, "layout": "v1", "type": "assistant", "created": at, "updated": at + 900, "steps": steps,
            "data": {"role": "assistant", "agent": "build", "modelID": "claude-sonnet-4-5", "providerID": "anthropic", "summary": summary,
                "tokens": tokens(1, 1, 0, 0, 0), "cost": 0.5, "time": {"created": at, "completed": at + 800}}}})
    }

    fn step(start: i64, end: i64, t: Value, cost: f64) -> Value {
        json!({"start": start, "end": end, "tokens": t, "cost": cost})
    }

    #[test]
    fn usage_fields_and_reasoning() {
        let s = decode(false, &[v2("msg_a", 1000, "claude-sonnet-4-5", "anthropic", tokens(6762, 49, 5, 1792, 10), 0.02)]);
        let r = &s.chains[0].requests[0];
        let u = r.usage;
        assert_eq!((u.uncached, u.cache_read, u.write_short, u.write_long, u.output, u.reasoning), (6762, 1792, 10, 0, 54, 5));
        assert_eq!((r.api, r.sent_ms, r.ended_ms, r.ts_ms), (ApiId::Anthropic, Some(1000), Some(1800), 1800));
        assert_eq!((s.id.as_str(), s.project.as_str(), s.title.as_deref()), ("ses_own", "~/src/app", Some("Fix the build (@general subagent)")));
        assert_eq!(s.reported_usd, Some(0.02));
    }

    #[test]
    fn a_step_without_usage_is_no_request() {
        let s = decode(false, &[v2("msg_a", 1000, "big-pickle", "opencode", tokens(0, 0, 0, 0, 0), 0.0)]);
        assert!(s.chains[0].requests.is_empty());
        assert_eq!(s.reported_usd, None, "no request, no cost");
    }

    #[test]
    fn each_step_of_a_1x_message_is_a_request() {
        let steps = vec![step(1000, 2000, tokens(10, 1, 0, 0, 0), 0.1), step(2100, 3000, tokens(20, 2, 0, 5, 0), 0.2)];
        let s = decode(false, &[v1("msg_a", 1000, steps, false), v1("msg_b", 4000, vec![], false)]);
        let r = &s.chains[0].requests;
        let got: Vec<_> = r.iter().map(|r| (r.usage.uncached, r.sent_ms.unwrap(), r.ended_ms.unwrap())).collect();
        assert_eq!(got, [(10, 1000, 2000), (20, 2100, 3000), (1, 4000, 4800)], "a message without steps counts its own tokens");
        assert!((s.reported_usd.unwrap() - 0.8).abs() < 1e-9);
    }

    #[test]
    fn a_model_behind_a_router() {
        assert_eq!(model_of(Some("anthropic/claude-sonnet-4.5"), Some("openrouter")), ("claude-sonnet-4.5".to_string(), ApiId::Anthropic));
        assert_eq!(model_of(Some("gpt-5.1-codex"), Some("github-copilot")).1, ApiId::OpenAi);
        assert_eq!(model_of(Some("big-pickle"), Some("opencode")), ("big-pickle".to_string(), ApiId::Unknown));
        assert_eq!(model_of(None, None).0, "unknown");
    }

    #[test]
    fn a_2x_compaction_is_a_call_and_a_marker() {
        let compaction = json!({"message": {"id": "msg_c", "layout": "v2", "type": "compaction", "created": 5000, "updated": 6000,
            "data": {"status": "completed", "reason": "auto", "summary": "s", "tokens": tokens(9000, 800, 0, 0, 0), "cost": 0.1}}});
        let failed = json!({"message": {"id": "msg_f", "layout": "v2", "type": "compaction", "created": 8000, "updated": 8500,
            "data": {"status": "failed", "reason": "manual", "tokens": tokens(100, 0, 0, 0, 0), "error": {}}}});
        let s = decode(
            false,
            &[
                v2("msg_a", 1000, "claude-opus-5-5", "anthropic", tokens(1, 1, 0, 0, 0), 0.0),
                compaction,
                v2("msg_b", 7000, "claude-opus-5-5", "anthropic", tokens(1, 1, 0, 0, 0), 0.0),
                failed,
                v2("msg_d", 9000, "claude-opus-5-5", "anthropic", tokens(1, 1, 0, 0, 0), 0.0),
            ],
        );
        let marks: Vec<_> = s.chains[0].requests.iter().map(|r| (r.model.as_str(), r.compaction_call, r.compacted_before, r.usage.uncached)).collect();
        assert_eq!(
            marks,
            [
                ("claude-opus-5-5", false, false, 1),
                ("claude-opus-5-5", true, false, 9000),
                ("claude-opus-5-5", false, true, 1),
                ("claude-opus-5-5", true, false, 100),
                ("claude-opus-5-5", false, false, 1)
            ],
            "a failed compaction is a call but no marker"
        );
        assert_eq!(s.compactions.iter().map(|c| c.kind.clone()).collect::<Vec<_>>(), [CompactionTrigger::Auto]);
    }

    #[test]
    fn a_1x_compaction_marks_the_request_after_the_summary() {
        let ask = json!({"message": {"id": "msg_u", "layout": "v1", "type": "user", "created": 5000, "updated": 5000, "steps": [], "compaction": {"auto": false}, "data": {"role": "user"}}});
        let s = decode(
            false,
            &[
                v1("msg_a", 1000, vec![step(1000, 2000, tokens(5, 1, 0, 0, 0), 0.0)], false),
                ask,
                v1("msg_s", 5100, vec![step(5100, 6000, tokens(7000, 600, 0, 0, 0), 0.0)], true),
                v1("msg_b", 7000, vec![step(7000, 7500, tokens(6, 1, 0, 0, 0), 0.0)], false),
            ],
        );
        let marks: Vec<_> = s.chains[0].requests.iter().map(|r| (r.compaction_call, r.compacted_before)).collect();
        assert_eq!(marks, [(false, false), (true, false), (false, true)]);
        assert_eq!(s.compactions[0].kind, CompactionTrigger::Manual);
    }

    #[test]
    fn a_subagent_reports_under_its_main_session_with_its_task() {
        let s = decode(true, &[v2("msg_a", 1000, "claude-haiku-4-5", "anthropic", tokens(1, 1, 0, 0, 0), 0.0)]);
        assert_eq!((s.id.as_str(), s.chains[0].name.as_str(), s.chains[0].kind), ("ses_main", "subagent: Fix the build", ChainKind::Subagent));
        assert_eq!((s.task.as_deref(), s.title.as_deref()), (Some("Fix the build"), None));
        assert_eq!(task_of("List files"), "List files");
    }
}
