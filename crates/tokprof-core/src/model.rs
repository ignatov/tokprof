//! Normalized session data shared by all agent adapters.
//!
//! A session owns a table of prompt content blocks and the chains of API requests that were
//! sent. A chain is one cache lineage: the main conversation, a subagent, or a side stream such
//! as compaction or advisor calls. Each request stores its prompt as a delta against the
//! previous request of the same chain (`keep` messages kept, `tail` appended), which is how
//! prompts actually grow and what lets the analysis see where a prefix changed.

use std::ops::Range;

use crate::api::ApiId;
use crate::source::AgentId;
use crate::trigger::Trigger;

/// Token counts the provider reported for one API request.
#[derive(Clone, Copy, Debug, Default)]
pub struct Usage {
    /// Prompt tokens that were neither read from nor written to the cache.
    pub uncached: u64,
    pub cache_read: u64,
    /// Cache writes with the short lifetime (Anthropic 5m, OpenAI 30m).
    pub write_short: u64,
    /// Cache writes with the long lifetime (Anthropic 1h, OpenAI 12h).
    pub write_long: u64,
    /// All generated tokens, reasoning included.
    pub output: u64,
    /// The reasoning share of `output`, when the agent reports it.
    pub reasoning: u64,
}

impl Usage {
    pub fn cache_write(&self) -> u64 {
        self.write_short + self.write_long
    }

    /// Full prompt size: everything the model read for this request.
    pub fn prompt(&self) -> u64 {
        self.uncached + self.cache_read + self.cache_write()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[non_exhaustive]
pub enum BlockKind {
    Tools,
    System,
    Prompt,
    Reminder,
    ToolResult,
    Text,
    Thinking,
    ToolUse,
    Image,
    Unknown,
}

impl BlockKind {
    pub fn name(self) -> &'static str {
        match self {
            BlockKind::Tools => "tool schemas",
            BlockKind::System => "system prompt",
            BlockKind::Prompt => "user prompt",
            BlockKind::Reminder => "reminders",
            BlockKind::ToolResult => "tool results",
            BlockKind::Text => "assistant text",
            BlockKind::Thinking => "thinking",
            BlockKind::ToolUse => "tool calls",
            BlockKind::Image => "images",
            BlockKind::Unknown => "unknown",
        }
    }
}

/// What kind of tool a call used, after ACP's tool kinds, so that rules work without the names
/// of any agent's tools. Two classes refine an ACP kind where advice tells them apart.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ToolClass {
    Read,
    Edit,
    /// A whole-file write, a refinement of `edit`.
    Write,
    Delete,
    Move,
    Search,
    Execute,
    /// Reading the output of a command running in the background, a refinement of `execute`.
    ExecuteOutput,
    Think,
    Fetch,
    SwitchMode,
    #[default]
    Other,
}

impl ToolClass {
    /// The ACP tool kind the class is or refines.
    pub fn acp_kind(self) -> &'static str {
        match self {
            ToolClass::Read => "read",
            ToolClass::Edit | ToolClass::Write => "edit",
            ToolClass::Delete => "delete",
            ToolClass::Move => "move",
            ToolClass::Search => "search",
            ToolClass::Execute | ToolClass::ExecuteOutput => "execute",
            ToolClass::Think => "think",
            ToolClass::Fetch => "fetch",
            ToolClass::SwitchMode => "switch_mode",
            ToolClass::Other => "other",
        }
    }
}

/// What injected the text of a reminder.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ReminderKind {
    /// The instruction files of the project and the user.
    Instructions,
    BackgroundNotifications,
    HookContext,
    DeferredToolLists,
    Skills,
    #[default]
    Other,
}

impl ReminderKind {
    /// The kind of a reminder label, as the sources write labels.
    pub fn of_label(label: &str) -> ReminderKind {
        match label {
            "instructions" => ReminderKind::Instructions,
            "background notifications" => ReminderKind::BackgroundNotifications,
            "hook context" => ReminderKind::HookContext,
            "deferred tool lists" => ReminderKind::DeferredToolLists,
            "skills" => ReminderKind::Skills,
            _ => ReminderKind::Other,
        }
    }
}

/// The class of a block's tool or reminder, for the rules that look at it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Class {
    #[default]
    None,
    Tool(ToolClass),
    Reminder(ReminderKind),
}

/// A piece of prompt content and what put it there.
#[derive(Clone, Debug)]
pub struct Block {
    pub kind: BlockKind,
    /// The tool's class for tool calls and results, the reminder's kind for reminders.
    pub class: Class,
    /// Size in characters. The analysis converts it to tokens per request, scaled so that the
    /// blocks of a prompt add up to the prompt size the provider reported.
    pub chars: u64,
    pub tool: Option<String>,
    /// What the tool worked on: a file, a command, a host, a subagent type, an MCP server.
    pub target: Option<String>,
}

#[derive(Clone, Debug, Default)]
pub struct Message {
    pub blocks: Range<usize>,
    /// The message carries text the user typed, so it starts a new turn.
    pub starts_turn: bool,
    /// False when the log referenced the message but its content was not found.
    pub resolved: bool,
}

/// Tool definitions and system prompt of a request.
#[derive(Clone, Debug, Default)]
pub struct Shape {
    pub blocks: Range<usize>,
    /// Hash of the rendered tools and system prompt, ignoring cache markers.
    pub fingerprint: u64,
    pub tool_names: Vec<String>,
    pub system_chars: u64,
}

/// Request parameters that invalidate the cache when they change.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Params {
    /// The reasoning effort, which the thinking advice also reads.
    pub effort: Option<String>,
    /// The other settings, by name, in the order a change is reported in after the effort.
    pub settings: Vec<(&'static str, Option<String>)>,
}

impl Params {
    /// The value of a setting; `None` when it is not set or not logged.
    pub fn get(&self, name: &str) -> Option<&str> {
        self.settings.iter().find(|(n, _)| *n == name).and_then(|(_, v)| v.as_deref())
    }

    /// Name of the first parameter that differs from `other`.
    pub fn changed_from(&self, other: &Params) -> Option<&'static str> {
        if self.effort != other.effort {
            return Some("effort");
        }
        self.settings.iter().map(|(name, _)| *name).find(|name| self.get(name) != other.get(name))
    }
}

#[derive(Clone, Debug, Default)]
pub struct PromptDelta {
    /// Index into `Session::shapes`; `None` keeps the previous request's shape.
    pub shape: Option<usize>,
    pub keep: usize,
    /// Indices into `Session::messages`.
    pub tail: Vec<usize>,
}

#[derive(Clone, Debug, Default)]
pub struct Request {
    pub ts_ms: i64,
    /// The API the request was sent to, whose cache rules apply to it.
    pub api: ApiId,
    pub model: String,
    pub usage: Usage,
    pub params: Option<Params>,
    pub version: Option<String>,
    /// `None` when the log does not allow reconstructing the prompt.
    pub prompt: Option<PromptDelta>,
    /// Index into `Session::messages` of the message this response produced.
    pub produced: Option<usize>,
    /// A compaction happened between the previous request of the chain and this one.
    pub compacted_before: bool,
    /// The previous request ended with an assistant message (Codex miss signature).
    pub after_assistant_message: bool,
    /// An advisor call: it reads the chain's conversation with another model and is not part
    /// of the chain's cache lineage.
    pub advisor: bool,
    /// Context window size, when the agent logs it.
    pub context_window: Option<u64>,
    /// What kind of work the request was part of. `None` counts as a prompt.
    pub origin: Option<Origin>,
    /// What made the agent send the request, as its source saw it; see [`Request::trigger`].
    pub trigger: Option<Trigger>,
}

impl Request {
    /// What made the agent send the request: an advisor call and the first request after a
    /// compaction say so themselves, everything else is what the source saw, else `unknown`.
    pub fn trigger(&self) -> Trigger {
        if self.advisor {
            Trigger::Advisor
        } else if self.compacted_before {
            Trigger::Compaction
        } else {
            self.trigger.unwrap_or_default()
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[non_exhaustive]
pub enum ChainKind {
    Main,
    Subagent,
    Side,
    Advisor,
}

impl ChainKind {
    pub fn name(self) -> &'static str {
        match self {
            ChainKind::Main => "main",
            ChainKind::Subagent => "subagent",
            ChainKind::Side => "side",
            ChainKind::Advisor => "advisor",
        }
    }
}

#[derive(Clone, Debug)]
pub struct Chain {
    pub name: String,
    pub kind: ChainKind,
    pub requests: Vec<Request>,
}

/// The short form of a session id that lists and profiles show: as many characters as the
/// agent's ids need to tell sessions apart.
pub fn short_id<'a>(agent: &AgentId, id: &'a str) -> &'a str {
    let end = id.char_indices().nth(agent.info().short_id_len).map_or(id.len(), |(i, _)| i);
    &id[..end]
}

#[derive(Clone, Debug)]
pub struct Session {
    pub agent: AgentId,
    pub id: String,
    pub project: String,
    pub cwd: Option<String>,
    pub blocks: Vec<Block>,
    pub messages: Vec<Message>,
    pub shapes: Vec<Shape>,
    pub chains: Vec<Chain>,
    /// Session cost as reported by the agent itself, for reconciliation.
    pub reported_usd: Option<f64>,
    /// What the session is about: the title the user gave it, or the one the agent generated.
    /// A subagent's session carries its parent's title when the log tells it.
    pub title: Option<String>,
    /// What a subagent was asked to do, in the words of the agent that started it; `None` for
    /// a main conversation.
    pub task: Option<String>,
    /// Compactions the agent logged, in time order. Claude Code's apply to every chain of the
    /// transcript.
    pub compactions: Vec<Compaction>,
}

/// A compaction the agent logged: when, and what started it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Compaction {
    pub ts_ms: i64,
    pub kind: CompactionTrigger,
}

/// What started a compaction.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum CompactionTrigger {
    /// The agent compacted the conversation while it sat idle, before its cache expired.
    Idle,
    /// The context was close to full.
    Auto,
    /// The user asked for it.
    Manual,
    /// Another trigger the log names.
    Logged(String),
    /// The log does not say.
    Unknown,
}

impl CompactionTrigger {
    /// The trigger a log names: `auto`, `manual` or another; `None` gives `Unknown`.
    pub fn logged(trigger: Option<&str>) -> CompactionTrigger {
        match trigger {
            Some("auto") => CompactionTrigger::Auto,
            Some("manual") => CompactionTrigger::Manual,
            Some(other) => CompactionTrigger::Logged(other.to_string()),
            None => CompactionTrigger::Unknown,
        }
    }

    /// The trigger's name: `idle`, `auto`, `manual` or the logged one; `None` when unknown.
    pub fn name(&self) -> Option<&str> {
        match self {
            CompactionTrigger::Idle => Some("idle"),
            CompactionTrigger::Auto => Some("auto"),
            CompactionTrigger::Manual => Some("manual"),
            CompactionTrigger::Logged(s) => Some(s),
            CompactionTrigger::Unknown => None,
        }
    }
}

/// What kind of work a request was part of. It is written as `prompt`, `skill:<name>`,
/// `agent:<type>`, `command:/<name>`, `notification`, `mcp:<server>` or `plugin:<name>`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Origin {
    /// A prompt the user typed.
    Prompt,
    Skill(String),
    /// A subagent of this type.
    Subagent(String),
    /// A slash command, without its slash.
    Command(String),
    /// The follow-up of a background task.
    Notification,
    Mcp(String),
    Plugin(String),
    /// Another origin, as written.
    Other(String),
}

impl Origin {
    /// The origin a written form gives; the inverse of its display.
    pub fn parse(s: &str) -> Origin {
        let tail = |prefix: &str| s.strip_prefix(prefix).map(str::to_string);
        match s {
            "prompt" => Origin::Prompt,
            "notification" => Origin::Notification,
            _ => tail("skill:")
                .map(Origin::Skill)
                .or_else(|| tail("agent:").map(Origin::Subagent))
                .or_else(|| tail("command:/").map(Origin::Command))
                .or_else(|| tail("mcp:").map(Origin::Mcp))
                .or_else(|| tail("plugin:").map(Origin::Plugin))
                .unwrap_or_else(|| Origin::Other(s.to_string())),
        }
    }
}

impl std::fmt::Display for Origin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Origin::Prompt => f.write_str("prompt"),
            Origin::Notification => f.write_str("notification"),
            Origin::Skill(n) => write!(f, "skill:{n}"),
            Origin::Subagent(n) => write!(f, "agent:{n}"),
            Origin::Command(n) => write!(f, "command:/{n}"),
            Origin::Mcp(n) => write!(f, "mcp:{n}"),
            Origin::Plugin(n) => write!(f, "plugin:{n}"),
            Origin::Other(s) => f.write_str(s),
        }
    }
}

impl Session {
    pub fn new(agent: AgentId, id: String) -> Session {
        Session {
            agent,
            id,
            project: String::new(),
            cwd: None,
            blocks: Vec::new(),
            messages: Vec::new(),
            shapes: Vec::new(),
            chains: Vec::new(),
            reported_usd: None,
            title: None,
            task: None,
            compactions: Vec::new(),
        }
    }

    /// The requests of each main conversation, in time order, with the conversation's name.
    ///
    /// One conversation can be in several chains of one name. A Claude Code session that spans
    /// the release which added request-audit lines keeps the requests from before it apart from
    /// the audited ones. Such chains follow one another in time and are joined. Chains of one name
    /// that overlap in time are separate conversations and stay apart.
    pub fn main_conversations(&self) -> Vec<(&str, Vec<&Request>)> {
        let span = |c: &Chain| {
            let ts = c.requests.iter().map(|r| r.ts_ms);
            (ts.clone().min().unwrap_or(0), ts.max().unwrap_or(0))
        };
        let mut chains: Vec<&Chain> = self.chains.iter().filter(|c| c.kind == ChainKind::Main && !c.requests.is_empty()).collect();
        chains.sort_by_key(|c| span(c).0);
        let mut out: Vec<(&str, Vec<&Request>, i64)> = Vec::new();
        for c in chains {
            let (first, last) = span(c);
            match out.iter_mut().filter(|(name, _, end)| *name == c.name && *end < first).max_by_key(|(_, _, end)| *end) {
                Some(conversation) => {
                    conversation.1.extend(&c.requests);
                    conversation.2 = last;
                }
                None => out.push((&c.name, c.requests.iter().collect(), last)),
            }
        }
        out.into_iter()
            .map(|(name, mut requests, _)| {
                requests.sort_by_key(|r| r.ts_ms);
                (name, requests)
            })
            .collect()
    }

    /// What the session's requests work on: what a subagent was asked to do, the title, or
    /// `untitled <short id>` when the log names neither.
    pub fn task_label(&self) -> String {
        match self.task.as_deref().or(self.title.as_deref()) {
            Some(label) => label.to_string(),
            None => format!("untitled {}", self.short_id()),
        }
    }

    pub fn short_id(&self) -> &str {
        short_id(&self.agent, &self.id)
    }

    pub fn push_message(&mut self, blocks: Vec<Block>, starts_turn: bool) -> usize {
        let start = self.blocks.len();
        self.blocks.extend(blocks);
        self.messages.push(Message {
            blocks: start..self.blocks.len(),
            starts_turn,
            resolved: true,
        });
        self.messages.len() - 1
    }

    pub fn push_unresolved_message(&mut self) -> usize {
        let start = self.blocks.len();
        self.blocks.push(Block {
            class: Class::None,
            kind: BlockKind::Unknown,
            chars: 0,
            tool: None,
            target: None,
        });
        self.messages.push(Message {
            blocks: start..self.blocks.len(),
            starts_turn: false,
            resolved: false,
        });
        self.messages.len() - 1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_first_changed_setting_is_named() {
        let p = |effort: &str, values: [&str; 4]| Params {
            effort: Some(effort.into()),
            settings: ["thinking", "speed", "betas", "context management"].into_iter().zip(values).map(|(n, v)| (n, Some(v.to_string()))).collect(),
        };
        let base = p("high", ["t", "s", "b", "c"]);
        assert_eq!(p("low", ["x", "x", "x", "x"]).changed_from(&base), Some("effort"));
        assert_eq!(p("high", ["x", "x", "x", "x"]).changed_from(&base), Some("thinking"));
        assert_eq!(p("high", ["t", "x", "x", "x"]).changed_from(&base), Some("speed"));
        assert_eq!(p("high", ["t", "s", "x", "x"]).changed_from(&base), Some("betas"));
        assert_eq!(p("high", ["t", "s", "b", "x"]).changed_from(&base), Some("context management"));
        assert_eq!(base.changed_from(&base), None);
    }

    #[test]
    fn origins_and_triggers_are_written_as_before() {
        for s in ["prompt", "skill:code-review", "agent:Explore", "command:/goal", "notification", "mcp:github", "plugin:x", "work"] {
            assert_eq!(Origin::parse(s).to_string(), s);
        }
        assert_eq!(Origin::parse("command:/goal"), Origin::Command("goal".into()));
        assert_eq!(CompactionTrigger::logged(Some("auto")).name(), Some("auto"));
        assert_eq!(CompactionTrigger::logged(Some("other")).name(), Some("other"));
        assert_eq!((CompactionTrigger::Idle.name(), CompactionTrigger::logged(None).name()), (Some("idle"), None));
    }

    #[test]
    fn every_tool_class_names_its_acp_kind() {
        let acp = ["read", "edit", "delete", "move", "search", "execute", "think", "fetch", "switch_mode", "other"];
        for class in [
            ToolClass::Read,
            ToolClass::Edit,
            ToolClass::Write,
            ToolClass::Delete,
            ToolClass::Move,
            ToolClass::Search,
            ToolClass::Execute,
            ToolClass::ExecuteOutput,
            ToolClass::Think,
            ToolClass::Fetch,
            ToolClass::SwitchMode,
            ToolClass::Other,
        ] {
            assert!(acp.contains(&class.acp_kind()), "{class:?}");
        }
        assert_eq!((ToolClass::Write.acp_kind(), ToolClass::ExecuteOutput.acp_kind()), ("edit", "execute"));
    }
}
