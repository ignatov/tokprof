//! Prompt-cache miss detection.
//!
//! A request misses when it re-processed more than 5% and at least 2,000 tokens of the prefix
//! the previous request of its chain had cached; this is the rule behind Claude Code's
//! `/usage` "Prompt cache" line, so the counts are comparable. The cause is the first
//! explanation that fits, checked from the most to the least certain.

use crate::fmt::human_duration;
use crate::model::{BlockKind, CompactionTrigger, Params, Request, Session, Usage};

const MIN_MISS_TOKENS: u64 = 2_000;
const MIN_MISS_SHARE: f64 = 0.05;
/// The API looks back at most 20 blocks from a cache breakpoint for an earlier entry.

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[non_exhaustive]
pub enum Cause {
    ModelSwitch,
    Compaction,
    ToolResultsCleared,
    IdleTtl,
    ParamsChanged,
    AgentUpdated,
    ToolsChanged,
    SystemChanged,
    ImagesRemoved,
    ThinkingDropped,
    HistoryEdited,
    Lookback,
    AfterAssistantMessage,
    Unknown,
}

impl Cause {
    pub const ALL: [Cause; 14] = [
        Cause::ModelSwitch,
        Cause::Compaction,
        Cause::ToolResultsCleared,
        Cause::IdleTtl,
        Cause::ParamsChanged,
        Cause::AgentUpdated,
        Cause::ToolsChanged,
        Cause::SystemChanged,
        Cause::ImagesRemoved,
        Cause::ThinkingDropped,
        Cause::HistoryEdited,
        Cause::Lookback,
        Cause::AfterAssistantMessage,
        Cause::Unknown,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Cause::ModelSwitch => "model switch",
            Cause::Compaction => "compaction",
            Cause::ToolResultsCleared => "tool results cleared",
            Cause::IdleTtl => "idle > TTL",
            Cause::ParamsChanged => "params changed",
            Cause::AgentUpdated => "agent updated",
            Cause::ToolsChanged => "tools changed",
            Cause::SystemChanged => "system prompt changed",
            Cause::ImagesRemoved => "images removed",
            Cause::ThinkingDropped => "thinking dropped",
            Cause::HistoryEdited => "history edited",
            Cause::Lookback => "lookback > 20 blocks",
            Cause::AfterAssistantMessage => "after assistant message",
            Cause::Unknown => "unknown (server side?)",
        }
    }

    /// Rebuilds the agent does on purpose; Claude Code reports them as expected rebuilds.
    pub fn expected(self) -> bool {
        matches!(self, Cause::Compaction | Cause::ToolResultsCleared)
    }
}

#[derive(Clone, Debug)]
pub struct Verdict {
    /// Tokens of the previously cached prefix that this request processed again.
    pub rewritten: u64,
    pub cause: Cause,
    pub detail: String,
}

/// What the cache comparison needs to know about the previous request of a chain.
#[derive(Clone, Debug)]
pub struct Prev {
    pub prompt: u64,
    pub ts_ms: i64,
    pub model: String,
    pub long_ttl: bool,
    pub params: Option<Params>,
    pub version: Option<String>,
    pub shape: Option<usize>,
}

/// Messages replaced in the current prompt: the old ones from position `at`, and the new ones
/// now at the same positions.
pub struct Edit<'a> {
    pub at: usize,
    pub len_before: usize,
    pub removed: &'a [usize],
    pub added: &'a [usize],
}

pub fn judge(s: &Session, prev: &Prev, req: &Request, shape: Option<usize>, edit: Option<&Edit>, tail_blocks: usize) -> Option<Verdict> {
    let rewritten = rewritten(prev.prompt, &req.usage)?;
    let (cause, detail) = classify(s, prev, req, shape, edit, tail_blocks);
    Some(Verdict { rewritten, cause, detail })
}

/// Tokens of the previous prompt a request processed again, when that makes it a miss.
pub fn rewritten(prev_prompt: u64, usage: &Usage) -> Option<u64> {
    let expected = prev_prompt.min(usage.prompt());
    let rewritten = expected.saturating_sub(usage.cache_read);
    (rewritten >= MIN_MISS_TOKENS && rewritten as f64 > MIN_MISS_SHARE * expected as f64).then_some(rewritten)
}

fn classify(s: &Session, prev: &Prev, req: &Request, shape: Option<usize>, edit: Option<&Edit>, tail_blocks: usize) -> (Cause, String) {
    let gap_s = (req.ts_ms - prev.ts_ms) as f64 / 1000.0;
    if req.model != prev.model {
        return (Cause::ModelSwitch, format!("{} -> {}", prev.model, req.model));
    }
    if req.compacted_before {
        // The kind of the latest compaction since the previous answered request.
        let kind = s.compactions.iter().rev().find(|c| c.ts_ms > prev.ts_ms && c.ts_ms <= req.ts_ms).map(|c| &c.kind);
        let detail = match kind {
            Some(CompactionTrigger::Idle) => "while idle".to_string(),
            Some(k) => k.name().unwrap_or_default().to_string(),
            None => String::new(),
        };
        return (Cause::Compaction, detail);
    }
    if let Some((ttl_ms, ttl_name)) = req.api.ttl(prev.long_ttl)
        && gap_s > ttl_ms as f64 / 1000.0
    {
        return (Cause::IdleTtl, format!("{} idle, {ttl_name} TTL", human_duration(gap_s)));
    }
    if let (Some(p), Some(c)) = (&prev.params, &req.params)
        && let Some(name) = c.changed_from(p)
    {
        return (Cause::ParamsChanged, name.to_string());
    }
    if let (Some(a), Some(b)) = (&prev.version, &req.version)
        && a != b
    {
        return (Cause::AgentUpdated, format!("{a} -> {b}"));
    }
    if let (Some(a), Some(b)) = (prev.shape, shape) {
        let (a, b) = (&s.shapes[a], &s.shapes[b]);
        if a.fingerprint != b.fingerprint {
            if a.tool_names != b.tool_names {
                let added = b.tool_names.iter().filter(|t| !a.tool_names.contains(t)).count();
                let removed = a.tool_names.iter().filter(|t| !b.tool_names.contains(t)).count();
                return (Cause::ToolsChanged, format!("+{added} -{removed} tools"));
            }
            let delta = b.system_chars as i64 - a.system_chars as i64;
            return (Cause::SystemChanged, format!("{delta:+} chars"));
        }
    }
    if let Some(edit) = edit {
        return classify_edit(s, edit);
    }
    if req.api.rules().and_then(|r| r.lookback_blocks).is_some_and(|n| tail_blocks > n) {
        return (Cause::Lookback, format!("{tail_blocks} blocks appended"));
    }
    if req.after_assistant_message {
        return (Cause::AfterAssistantMessage, String::new());
    }
    (Cause::Unknown, String::new())
}

fn classify_edit(s: &Session, edit: &Edit) -> (Cause, String) {
    let sum = |msgs: &[usize], kind: BlockKind| -> u64 {
        msgs.iter()
            .flat_map(|&m| s.blocks[s.messages[m].blocks.clone()].iter())
            .filter(|b| b.kind == kind)
            .map(|b| if kind == BlockKind::Image { 1 } else { b.chars })
            .sum()
    };
    let at = format!("at message {} of {}", edit.at, edit.len_before);
    let (old_results, new_results) = (sum(edit.removed, BlockKind::ToolResult), sum(edit.added, BlockKind::ToolResult));
    if old_results > new_results * 2 && old_results - new_results > 1_000 {
        return (Cause::ToolResultsCleared, at);
    }
    if sum(edit.removed, BlockKind::Image) > sum(edit.added, BlockKind::Image) {
        return (Cause::ImagesRemoved, at);
    }
    let (old_thinking, new_thinking) = (sum(edit.removed, BlockKind::Thinking), sum(edit.added, BlockKind::Thinking));
    if old_thinking > new_thinking + 1_000 {
        return (Cause::ThinkingDropped, at);
    }
    (Cause::HistoryEdited, at)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Compaction;

    fn prev() -> Prev {
        Prev { prompt: 400_000, ts_ms: 0, model: "claude-opus-5-5".into(), long_ttl: true, params: None, version: None, shape: None }
    }

    /// The request after a compaction: 50k sent, only the 20k of tools and system prompt read.
    fn after_compaction() -> Request {
        Request {
            ts_ms: 3_600_000,
            model: "claude-opus-5-5".into(),
            compacted_before: true,
            usage: Usage { cache_read: 20_000, write_long: 30_000, ..Usage::default() },
            ..Request::default()
        }
    }

    fn detail(agent: crate::source::AgentId, kind: Option<&str>) -> (Cause, String) {
        let mut s = Session::new(agent, "s".into());
        s.compactions.push(Compaction { ts_ms: 3_000_000, kind: if kind == Some("idle") { CompactionTrigger::Idle } else { CompactionTrigger::logged(kind) } });
        let v = judge(&s, &prev(), &after_compaction(), None, None, 0).expect("a miss");
        assert_eq!(v.rewritten, 30_000);
        (v.cause, v.detail)
    }

    #[test]
    fn a_compaction_cause_names_its_kind() {
        assert_eq!(detail(crate::testing::claude(), Some("idle")), (Cause::Compaction, "while idle".to_string()));
        assert_eq!(detail(crate::testing::claude(), Some("auto")), (Cause::Compaction, "auto".to_string()));
        assert_eq!(detail(crate::testing::codex(), None), (Cause::Compaction, String::new()));
    }

    #[test]
    fn an_older_compaction_does_not_name_the_cause() {
        let mut s = Session::new(crate::testing::claude(), "s".into());
        s.compactions.push(Compaction { ts_ms: -1, kind: CompactionTrigger::Idle });
        let v = judge(&s, &prev(), &after_compaction(), None, None, 0).expect("a miss");
        assert_eq!((v.cause, v.detail.as_str()), (Cause::Compaction, ""));
    }
}
