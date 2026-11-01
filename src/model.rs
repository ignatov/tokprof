//! Normalized session data shared by all agent adapters.
//!
//! A session owns a table of prompt content blocks and the chains of API requests that were
//! sent. A chain is one cache lineage: the main conversation, a subagent, or a side stream such
//! as compaction or advisor calls. Each request stores its prompt as a delta against the
//! previous request of the same chain (`keep` messages kept, `tail` appended), which is how
//! prompts actually grow and what lets the analysis see where a prefix changed.

use std::ops::Range;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Agent {
    Claude,
    Codex,
}

impl Agent {
    pub const ALL: [Agent; 2] = [Agent::Claude, Agent::Codex];

    pub fn name(self) -> &'static str {
        match self {
            Agent::Claude => "claude",
            Agent::Codex => "codex",
        }
    }

    pub fn parse(s: &str) -> Option<Agent> {
        Agent::ALL.into_iter().find(|a| a.name() == s)
    }
}

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

/// A piece of prompt content and what put it there.
#[derive(Clone, Debug)]
pub struct Block {
    pub kind: BlockKind,
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
    pub effort: Option<String>,
    pub thinking: Option<String>,
    pub betas: Option<String>,
    pub context_management: Option<String>,
    pub speed: Option<String>,
}

impl Params {
    /// Name of the first parameter that differs from `other`.
    pub fn changed_from(&self, other: &Params) -> Option<&'static str> {
        if self.effort != other.effort {
            Some("effort")
        } else if self.thinking != other.thinking {
            Some("thinking")
        } else if self.speed != other.speed {
            Some("speed")
        } else if self.betas != other.betas {
            Some("betas")
        } else if self.context_management != other.context_management {
            Some("context management")
        } else {
            None
        }
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
    /// What kind of work the request was part of: `skill:<name>`, `agent:<type>`,
    /// `command:/<name>`, `notification`, `mcp:<server>`, `plugin:<name>`, or `prompt` for a
    /// prompt the user typed. `None` counts as `prompt`.
    pub origin: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
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

/// The short form of a session id: 8 characters of a Claude Code id, which is random, and the
/// first three groups of a Codex id, 18 characters. A Codex id is a UUIDv7: its first 13
/// characters are the millisecond it was made, which threads launched together share, and the
/// third group adds 12 random bits.
pub fn short_id(agent: Agent, id: &str) -> &str {
    let n = match agent {
        Agent::Claude => 8,
        Agent::Codex => 18,
    };
    let end = id.char_indices().nth(n).map_or(id.len(), |(i, _)| i);
    &id[..end]
}

#[derive(Clone, Debug)]
pub struct Session {
    pub agent: Agent,
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
    /// `idle` when Claude Code compacted the conversation while it sat idle, else the trigger
    /// it logged (`auto`, `manual`); `None` when the agent logs no kind (Codex).
    pub kind: Option<String>,
}

impl Session {
    pub fn new(agent: Agent, id: String) -> Session {
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
        short_id(self.agent, &self.id)
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
