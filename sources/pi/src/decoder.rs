//! The decoder of a pi session log: one line per entry, the `session` header first.

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;

use serde_json::Value;
use tokprof_core::api::ApiId;
use tokprof_core::labels::display_path;
use tokprof_core::model::{Chain, ChainKind, Compaction, CompactionTrigger, Request, Session, Usage};
use tokprof_core::source::{AgentId, Decoder};
use tokprof_logio::{parse_ts_ms, str_at, u64_at};

/// The entries a fork copied from the log it came from, by `entry_key`: they count in that log.
pub(crate) type Copied = Arc<HashSet<String>>;

/// What tells one entry from another across logs: its `id` and its `timestamp`, which `/fork`,
/// `/clone` and `--fork` keep when they copy it (pi 0.84.2).
pub(crate) fn entry_key(v: &Value) -> Option<String> {
    Some(format!("{}@{}", str_at(v, "id")?, str_at(v, "timestamp").unwrap_or_default()))
}

#[derive(Clone)]
pub(crate) struct Parser {
    session: Session,
    home: PathBuf,
    copied: Copied,
    requests: Vec<Request>,
    /// The model of the latest response, and of the latest `model_change` line.
    response_model: Option<String>,
    changed_model: Option<String>,
    /// A compaction came after the latest request.
    compacted: bool,
    /// The sum of `usage.cost.total`, when a request carried one.
    cost: Option<f64>,
}

impl Parser {
    pub(crate) fn new(agent: AgentId, id: String, home: PathBuf, copied: Copied) -> Parser {
        Parser { session: Session::new(agent, id), home, copied, requests: Vec::new(), response_model: None, changed_model: None, compacted: false, cost: None }
    }

    fn line(&mut self, line: &str) {
        let Ok(v) = serde_json::from_str::<Value>(line) else { return };
        let kind = str_at(&v, "type").unwrap_or_default();
        if kind == "session" {
            if let Some(id) = str_at(&v, "id").filter(|id| !id.is_empty()) {
                self.session.id = id.to_string();
            }
            if let Some(cwd) = str_at(&v, "cwd").filter(|_| self.session.cwd.is_none()) {
                self.session.project = display_path(cwd, None, &self.home);
                self.session.cwd = Some(cwd.to_string());
            }
            return;
        }
        // A fork's copy of an entry of the log it came from counts there; its model still
        // tells what a later compaction ran on.
        let copy = entry_key(&v).is_some_and(|k| self.copied.contains(&k));
        let ts = parse_ts_ms(v.get("timestamp"));
        match kind {
            "model_change" => self.changed_model = str_at(&v, "modelId").map(str::to_string),
            "message" => {
                let Some(m) = v.get("message").filter(|m| str_at(m, "role") == Some("assistant")) else { return };
                if let Some(model) = str_at(m, "model") {
                    self.response_model = Some(model.to_string());
                }
                if copy {
                    return;
                }
                let Some(usage) = m.get("usage") else { return };
                let model = self.response_model.clone().unwrap_or_else(|| "unknown".to_string());
                let ended = ts.unwrap_or(0);
                // The entry is written when the response ended; `durationMs` is how long it ran.
                let sent = m.get("durationMs").and_then(Value::as_i64).map_or(ended, |d| ended - d.max(0));
                let api = api_of(str_at(m, "api"), str_at(m, "provider"));
                self.request(usage, ended, sent, model, api, false);
            }
            "compaction" | "branch_summary" if !copy => {
                if let Some(usage) = v.get("usage") {
                    let model = self.response_model.clone().or_else(|| self.changed_model.clone()).unwrap_or_else(|| "unknown".to_string());
                    let at = ts.unwrap_or(0);
                    let api = self.requests.last().map_or(ApiId::Unknown, |r| r.api);
                    self.request(usage, at, at, model, api, true);
                }
                if kind == "compaction" {
                    self.compacted = true;
                    self.session.compactions.push(Compaction { ts_ms: ts.unwrap_or(0), kind: CompactionTrigger::Unknown });
                }
            }
            _ => {}
        }
    }

    /// One request, when its usage is not all zero: a response stopped before its first token
    /// is none.
    fn request(&mut self, usage: &Value, ended: i64, sent: i64, model: String, api: ApiId, compaction_call: bool) {
        let (input, output, read, written) = (u64_at(usage, "input"), u64_at(usage, "output"), u64_at(usage, "cacheRead"), u64_at(usage, "cacheWrite"));
        if input + output + read + written == 0 {
            return;
        }
        if let Some(total) = usage.get("cost").and_then(|c| c.get("total")).and_then(Value::as_f64) {
            *self.cost.get_or_insert(0.0) += total;
        }
        let long = u64_at(usage, "cacheWrite1h").min(written);
        self.requests.push(Request {
            ts_ms: ended,
            api,
            model,
            usage: Usage {
                uncached: input,
                cache_read: read,
                write_short: written - long,
                write_long: long,
                // `reasoning` is part of `output` (pi-ai's `Usage`).
                output,
                reasoning: u64_at(usage, "reasoning").min(output),
            },
            compacted_before: self.compacted,
            compaction_call,
            first_sent_ms: Some(sent.min(ended)),
            sent_ms: Some(sent.min(ended)),
            ended_ms: Some(ended),
            ..Request::default()
        });
        self.compacted = false;
    }

    pub(crate) fn into_session(mut self) -> Session {
        // Every branch of the tree is in the log in the order it was written.
        self.requests.sort_by_key(|r| r.ts_ms);
        self.session.chains.push(Chain { name: "main".to_string(), kind: ChainKind::Main, requests: self.requests });
        self.session.compactions.sort_by_key(|c| c.ts_ms);
        self.session.reported_usd = self.cost;
        self.session
    }
}

/// The API whose cache rules apply: pi's `api` and `provider` of the response.
fn api_of(api: Option<&str>, provider: Option<&str>) -> ApiId {
    match (api, provider) {
        (Some("anthropic-messages"), _) => ApiId::Anthropic,
        (_, Some(p)) if p.starts_with("openai") => ApiId::OpenAi,
        _ => ApiId::Unknown,
    }
}

impl Decoder for Parser {
    fn feed(&mut self, record: &str) {
        self.line(record);
    }

    fn snapshot(&self) -> Session {
        self.clone().into_session()
    }

    fn finish(self: Box<Self>) -> Session {
        (*self).into_session()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const HEADER: &str =
        r#"{"type":"session","version":3,"id":"01a1a000-0000-7000-8000-000000000001","timestamp":"2026-11-01T10:00:00.000Z","cwd":"/h/src/app"}"#;

    fn decode(lines: &[Value], copied: &[&str]) -> Session {
        let copied: HashSet<String> = copied.iter().map(|s| s.to_string()).collect();
        let mut p = Parser::new(crate::Pi::agent(), "from-name".into(), PathBuf::from("/h"), Arc::new(copied));
        p.line(HEADER);
        lines.iter().for_each(|l| p.line(&l.to_string()));
        p.into_session()
    }

    fn reply(id: &str, parent: &str, at: &str, model: &str, usage: Value) -> Value {
        json!({"type": "message", "id": id, "parentId": parent, "timestamp": format!("2026-11-01T{at}Z"),
            "message": {"role": "assistant", "provider": "anthropic", "api": "anthropic-messages", "model": model, "stopReason": "stop", "usage": usage}})
    }

    fn usage(input: u64, output: u64, read: u64, write: u64) -> Value {
        json!({"input": input, "output": output, "cacheRead": read, "cacheWrite": write, "totalTokens": input + output + read + write, "cost": {"total": 0.01}})
    }

    #[test]
    fn the_header_names_the_session_and_project() {
        let s = decode(&[], &[]);
        assert_eq!((s.id.as_str(), s.project.as_str()), ("01a1a000-0000-7000-8000-000000000001", "~/src/app"));
    }

    #[test]
    fn a_log_without_a_header_id_keeps_the_id_of_its_name() {
        let mut p = Parser::new(crate::Pi::agent(), "from-name".into(), PathBuf::from("/h"), Copied::default());
        p.line(r#"{"type":"session","version":3,"cwd":"/h/w"}"#);
        assert_eq!(p.into_session().id, "from-name");
    }

    #[test]
    fn usage_fields_and_an_aborted_reply() {
        let aborted = reply("b", "a", "10:00:02", "claude-sonnet-4-5", json!({"input": 0, "output": 0, "cacheRead": 0, "cacheWrite": 0, "cost": {"total": 0}}));
        let s = decode(&[reply("a", "h", "10:00:01", "claude-sonnet-4-5", usage(13, 77, 1544, 0)), aborted], &[]);
        let r = &s.chains[0].requests;
        assert_eq!(r.len(), 1, "a reply stopped before its first token is no request");
        let u = r[0].usage;
        assert_eq!((u.uncached, u.cache_read, u.cache_write(), u.output), (13, 1544, 0, 77));
        assert_eq!(r[0].api, ApiId::Anthropic);
        assert_eq!(s.reported_usd, Some(0.01));
    }

    #[test]
    fn long_lifetime_writes_come_out_of_cache_write() {
        let mut u = usage(1, 2, 0, 1342);
        u["cacheWrite1h"] = json!(1000);
        u["reasoning"] = json!(1);
        let s = decode(&[reply("a", "h", "10:00:01", "claude-opus-5-5", u)], &[]);
        let u = s.chains[0].requests[0].usage;
        assert_eq!((u.write_short, u.write_long, u.output, u.reasoning), (342, 1000, 2, 1), "reasoning is part of output");
    }

    #[test]
    fn a_compaction_call_runs_on_the_latest_model_and_marks_the_next_request() {
        let compaction = json!({"type": "compaction", "id": "c", "parentId": "a", "timestamp": "2026-11-01T10:05:00Z", "summary": "s", "firstKeptEntryId": "a", "tokensBefore": 9,
            "usage": {"input": 1735, "output": 897, "cacheRead": 0, "cacheWrite": 0, "cost": {"total": 0}}});
        let s = decode(
            &[
                json!({"type": "model_change", "id": "m", "parentId": "h", "timestamp": "2026-11-01T10:00:00Z", "provider": "anthropic", "modelId": "claude-sonnet-4-5"}),
                reply("a", "m", "10:00:01", "claude-opus-5-5", usage(1, 1, 0, 0)),
                compaction,
                reply("b", "c", "10:06:00", "claude-opus-5-5", usage(1, 1, 0, 0)),
            ],
            &[],
        );
        let r = &s.chains[0].requests;
        let marks: Vec<_> = r.iter().map(|r| (r.model.as_str(), r.compaction_call, r.compacted_before, r.usage.uncached)).collect();
        assert_eq!(marks, [("claude-opus-5-5", false, false, 1), ("claude-opus-5-5", true, false, 1735), ("claude-opus-5-5", false, true, 1)]);
        assert_eq!(s.compactions.len(), 1);
    }

    #[test]
    fn a_compaction_before_any_reply_uses_the_model_change() {
        let compaction = json!({"type": "compaction", "id": "c", "parentId": "m", "timestamp": "2026-11-01T10:05:00Z", "usage": usage(5, 5, 0, 0)});
        let change = json!({"type": "model_change", "id": "m", "parentId": "h", "timestamp": "2026-11-01T10:00:00Z", "modelId": "qwen3-coder"});
        assert_eq!(decode(&[change, compaction.clone()], &[]).chains[0].requests[0].model, "qwen3-coder");
        assert_eq!(decode(&[compaction], &[]).chains[0].requests[0].model, "unknown");
    }

    #[test]
    fn every_branch_of_the_tree_counts() {
        let s = decode(
            &[reply("a", "u", "10:00:01", "claude-sonnet-4-5", usage(1, 1, 0, 0)), reply("b", "u", "10:01:00", "claude-sonnet-4-5", usage(2, 2, 0, 0))],
            &[],
        );
        assert_eq!(s.chains[0].requests.len(), 2);
    }

    #[test]
    fn a_fork_leaves_out_what_it_copied() {
        let copied = reply("a", "u", "10:00:01", "claude-sonnet-4-5", usage(1, 1, 0, 0));
        let key = entry_key(&copied).unwrap();
        let compaction = json!({"type": "compaction", "id": "c", "parentId": "a", "timestamp": "2026-11-01T10:05:00Z", "usage": usage(5, 5, 0, 0)});
        let s = decode(&[copied, compaction], &[&key]);
        let r = &s.chains[0].requests;
        assert_eq!((r.len(), r[0].model.as_str(), r[0].compaction_call), (1, "claude-sonnet-4-5", true), "the copy still names the model");
    }
}
