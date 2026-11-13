//! Claude Code transcripts: `$CLAUDE_CONFIG_DIR/projects/<project>/<session>.jsonl`, by default
//! under `~/.claude`, with the transcripts of the session's subagents in
//! `<session>/subagents/agent-<id>.jsonl` and what started each in `agent-<id>.meta.json`.

mod activity;
mod decoder;
mod tools;

use std::collections::HashMap;
use std::fs::Metadata;
use std::io::BufRead;
use std::ops::ControlFlow;
use std::path::{Path, PathBuf};

use anyhow::Result;
use serde_json::Value;
use tokprof_core::labels::display_path;
use tokprof_core::source::{
    Activity, AgentId, AgentInfo, Capabilities, Claim, Decoder, Detail, Harness, Head, Locator, LogSource, OwnCost, Pickup, PublicNames, ReadOn, Records,
    SessionRef, Touched,
};
use tokprof_logio::{file_times, parse_ts_ms, str_at, walk};

/// The names that ship with Claude Code, which a share file keeps (see README.md): read from the
/// bundled code of the 2.1.316 binary, with the tools 2.1.317 adds and the older names that
/// transcripts of 2.1.284 and later still carry.
const PUBLIC_NAMES: PublicNames =
    PublicNames { tools: CLAUDE_TOOLS, commands: CLAUDE_COMMANDS, skills: CLAUDE_SKILLS, subagent_types: CLAUDE_SUBAGENT_TYPES, mcp: Some(("mcp__", "__")) };

/// Built-in tools; the server tools Claude Code sends to the API (`advisor`, `web_search`) and its
/// deferred-tool stub; the aliases it still accepts; and the retired tools it still recognizes.
const CLAUDE_TOOLS: &[&str] = &[
    "Agent",
    "AppifactRepl",
    "Artifact",
    "ArtifactCheck",
    "ArtifactComments",
    "ArtifactData",
    "AskUserQuestion",
    "Bash",
    "ClaudeDesign",
    "CronCreate",
    "CronDelete",
    "CronList",
    "DesignSync",
    "Edit",
    "EndConversation",
    "EnterPlanMode",
    "EnterWorktree",
    "ExitPlanMode",
    "ExitWorktree",
    "FetchInboxMessage",
    "GetTask",
    "Glob",
    "Grep",
    "LSP",
    "ListAgents",
    "ListConnectors",
    "ListMcpResourcesTool",
    "ListPlugins",
    "ListSkills",
    "Monitor",
    "NotebookEdit",
    "ObserverReport",
    "OfferChromeSetup",
    "Poll",
    "PowerShell",
    "Projects",
    "ProposeGoal",
    "PublishPlugin",
    "PushNotification",
    "Read",
    "ReadMcpResourceDirTool",
    "ReadMcpResourceTool",
    "ReadNotifications",
    "RefreshMcpTools",
    "RemoteTrigger",
    "ReportFindings",
    "ScheduleWakeup",
    "SearchMcpRegistry",
    "SearchPlugins",
    "SearchSkills",
    "SendFeedback",
    "SendFile",
    "SendMessage",
    "SendToProject",
    "SendToProjectContext",
    "SendUserFile",
    "SendUserMessage",
    "ShareOnboardingGuide",
    "ShowOnboardingRolePicker",
    "Skill",
    "StructuredOutput",
    "SubagentHandback",
    "SuggestConnectors",
    "SuggestPluginInstall",
    "SuggestSkills",
    "TaskCreate",
    "TaskGet",
    "TaskList",
    "TaskStop",
    "TaskUpdate",
    "TestingPermission",
    "TodoWrite",
    "ToolSearch",
    "WaitForMcpServers",
    "WebFetch",
    "WebSearch",
    "Workflow",
    "Write",
    "memory_list",
    "memory_read",
    "memory_write",
    "propose_skills",
    "advisor",
    "web_search",
    "DeferredToolPlaceholder",
    "Task",
    "KillShell",
    "KillBash",
    "Brief",
    "ListPeers",
    "RunWorkflow",
    "ListMcpResources",
    "ReadMcpResource",
    "ReadMcpResourceDir",
    "BashOutput",
    "BashOutputTool",
    "AgentOutput",
    "AgentOutputTool",
    "TaskOutput",
    "Frame",
    "FrameRead",
    "TeamCreate",
    "TeamDelete",
    "SuggestBackgroundPR",
    "AutofixPr",
    "WebBrowser",
    "REPL",
    "MultiEdit",
    "LS",
    "NotebookRead",
    "ReadWidgetContext",
    "WrapUp",
];

/// Built-in slash commands, their aliases, and the bundled skills, which can be typed as commands.
const CLAUDE_COMMANDS: &[&str] = &[
    "add-dir",
    "advisor",
    "agents",
    "allowed-tools",
    "android",
    "app",
    "artifact-capabilities",
    "artifact-design",
    "artifact-diagramming",
    "artifacts",
    "auto-mode-setup",
    "autocompact",
    "autofix-pr",
    "background",
    "bashes",
    "batch",
    "bg",
    "branch",
    "break-reminder",
    "breaks",
    "brief",
    "btw",
    "bug",
    "cd",
    "checkpoint",
    "checkup",
    "chrome",
    "claim-credit",
    "claude-api",
    "claude-code-docs",
    "claude-in-chrome",
    "claude-test",
    "claude-test-draft",
    "claude-test-execute",
    "clear",
    "cloud-plugins",
    "code-review",
    "color",
    "commit",
    "commit-push-pr",
    "compact",
    "config",
    "context",
    "continue",
    "copy",
    "cost",
    "cowork-plugin",
    "daemon",
    "dataviz",
    "debug",
    "deep-research",
    "design",
    "design-consent",
    "design-login",
    "design-revoke",
    "design-sync",
    "desktop",
    "diff",
    "doctor",
    "downtime",
    "effort",
    "exit",
    "explain-usage",
    "export",
    "extra-usage",
    "fast",
    "feedback",
    "fewer-permission-prompts",
    "focus",
    "fork",
    "goal",
    "heapdump",
    "help",
    "hooks",
    "ide",
    "import",
    "init",
    "insights",
    "install",
    "install-github-app",
    "install-slack-app",
    "ios",
    "keybindings",
    "keybindings-help",
    "limit-reset",
    "list-agents",
    "login",
    "logout",
    "loop",
    "loops",
    "low-priority",
    "marketplace",
    "mcp",
    "memory",
    "memory-pause",
    "memory-types",
    "mobile",
    "model",
    "name",
    "new",
    "output-style",
    "passes",
    "pause-memory",
    "peers",
    "permissions",
    "plan",
    "plugin",
    "plugin-authoring",
    "plugins",
    "powerup",
    "pr",
    "privacy-settings",
    "pro-trial-expired",
    "proactive",
    "quit",
    "radio",
    "rate-limit-options",
    "rc",
    "recap",
    "release-notes",
    "reload-plugins",
    "reload-skills",
    "remote",
    "remote-control",
    "remote-env",
    "rename",
    "reset",
    "restart",
    "resume",
    "review",
    "rewind",
    "routines",
    "run",
    "run-skill-generator",
    "sandbox",
    "schedule",
    "scroll-speed",
    "security-review",
    "send-to-project",
    "session",
    "settings",
    "setup-bedrock",
    "setup-claude",
    "setup-cowork",
    "setup-vertex",
    "share",
    "simplify",
    "skill-doctor",
    "skills",
    "slides",
    "stats",
    "status",
    "statusline",
    "stickers",
    "stop",
    "subtask",
    "tasks",
    "team-onboarding",
    "teleport",
    "terminal-setup",
    "theme",
    "toggle-memory",
    "tp",
    "tui",
    "ultraplan",
    "ultrareview",
    "undo",
    "update",
    "update-config",
    "upgrade",
    "usage",
    "usage-credits",
    "verify",
    "version",
    "vim",
    "voice",
    "web-setup",
    "wellbeing",
    "workflow-authoring",
    "workflows",
];

/// Skills bundled with Claude Code, those of its built-in plugins, the built-in prompt commands the
/// Skill tool can call, the bundled workflow, and the skill aliases.
const CLAUDE_SKILLS: &[&str] = &[
    "artifact-capabilities",
    "artifact-design",
    "artifact-diagramming",
    "batch",
    "claude-api",
    "claude-code-docs",
    "claude-in-chrome",
    "code-review",
    "commit",
    "cowork-plugin",
    "dataviz",
    "debug",
    "design",
    "design-sync",
    "doctor",
    "explain-usage",
    "fewer-permission-prompts",
    "keybindings-help",
    "loop",
    "memory-types",
    "pr",
    "run",
    "run-skill-generator",
    "schedule",
    "send-to-project",
    "setup-claude",
    "simplify",
    "slides",
    "update-config",
    "verify",
    "workflow-authoring",
    "plugin-authoring",
    "claude-test",
    "claude-test-execute",
    "claude-test-draft",
    "init",
    "security-review",
    "commit-push-pr",
    "deep-research",
    "review",
    "checkup",
    "proactive",
    "routines",
    "setup-cowork",
];

/// Built-in subagent types, with those of the built-in claude-test plugin.
const CLAUDE_SUBAGENT_TYPES: &[&str] = &[
    "general-purpose",
    "Explore",
    "Plan",
    "statusline-setup",
    "claude-code-guide",
    "fork",
    "web-fetch",
    "claude",
    "worker",
    "workflow-subagent",
    "comment-thread-analyst",
    "claude-test:author",
    "claude-test:conductor",
    "claude-test:explorer",
    "claude-test:runner",
];

use decoder::{AgentMeta, Parser};

pub struct ClaudeCode {
    /// The `projects` directory.
    root: PathBuf,
    /// The home directory, which paths are shown relative to.
    home: PathBuf,
    agent: AgentId,
}

impl ClaudeCode {
    /// The transcripts under `$CLAUDE_CONFIG_DIR/projects`, by default `~/.claude/projects`.
    pub fn from_env(home: &Path) -> ClaudeCode {
        let base = std::env::var_os("CLAUDE_CONFIG_DIR").map(PathBuf::from).unwrap_or_else(|| home.join(".claude"));
        ClaudeCode::at(base.join("projects"), home)
    }

    /// The transcripts under a `projects` directory.
    pub fn at(root: PathBuf, home: &Path) -> ClaudeCode {
        ClaudeCode { root, home: home.to_path_buf(), agent: ClaudeCode::agent() }
    }

    pub fn agent() -> AgentId {
        AgentId::new(AgentInfo {
            name: "claude".to_string(),
            title: "Claude Code".to_string(),
            // Claude Code's ids are random, so 8 characters tell sessions apart.
            short_id_len: 8,
            instruction_files: vec!["CLAUDE.md".to_string()],
            ttl_settings: Some(("promptCacheTtl".to_string(), "subagentPromptCacheTtl".to_string())),
            // Settings seen in the settings schema of Claude Code 2.1.316 (see README.md).
            setup_hints: [
                (
                    "skills",
                    "lower skillListingBudgetFraction (default 0.01 of the context window), or turn unused skills off with skillOverrides in settings.json",
                ),
                ("instructions", "trim CLAUDE.md and AGENTS.md, and keep the auto-memory MEMORY.md to short index lines"),
                ("deferred tool lists", "turn off the MCP servers this project does not use, with /mcp"),
                ("hook context", "make the hooks print less"),
            ]
            .into_iter()
            .map(|(source, hint)| (source.to_string(), hint.to_string()))
            .collect(),
            cost_counter: Some("Claude Code's own cost counter".to_string()),
            harness: Harness {
                // Measured by the reference bench (docs/reference-bench.md).
                pickup: Some(Pickup { taken_tokens: 8_000.0, requests: 4.0, output_tokens: 2_000.0 }),
                // The compaction call runs as a fork of the conversation, on its cache (see README.md).
                compaction_reads_cache: true,
            },
            capabilities: Capabilities {
                own_cost: OwnCost::Session,
                compaction_trigger: true,
                context_window: false,
                prompt_composition: true,
                titles: true,
                subagent_tasks: true,
                origins: true,
            },
            public_names: PUBLIC_NAMES,
        })
    }

    fn reference(&self, path: &Path, meta: &Metadata) -> SessionRef {
        let session_id = stem(path);
        let main_id = parent_of(path).map_or_else(|| session_id.clone(), |main| stem(&main));
        let (modified_ns, created_ms) = file_times(meta);
        SessionRef {
            source: "claude",
            agent: self.agent.clone(),
            locator: Locator::File(path.to_path_buf()),
            subagent: is_subagent(path),
            session_id,
            main_id,
            size: meta.len(),
            modified_ns,
            created_ms,
        }
    }

    fn logs_in(&self, dir: &Path) -> Vec<SessionRef> {
        let mut out = Vec::new();
        walk(dir, &mut |path, meta| {
            if is_log(path) {
                out.push(self.reference(path, meta));
            }
        });
        out
    }
}

impl LogSource for ClaudeCode {
    fn name(&self) -> &'static str {
        "claude"
    }

    fn agents(&self) -> Vec<AgentId> {
        vec![self.agent.clone()]
    }

    fn detail(&self) -> Detail {
        Detail::Requests
    }

    fn discover(&self) -> Vec<SessionRef> {
        self.logs_in(&self.root)
    }

    fn session_at(&self, path: &Path) -> Option<SessionRef> {
        let meta = std::fs::metadata(path).ok().filter(|m| m.is_file())?;
        Some(self.reference(path, &meta))
    }

    /// Any file that no other source claims is read as a transcript.
    fn claim(&self, _: &Path) -> Claim {
        Claim::Fallback
    }

    fn watched_paths(&self) -> Vec<PathBuf> {
        vec![self.root.clone()]
    }

    fn touched(&self, changed: &Path, arrived: bool) -> Touched {
        let main = |path: &Path| parent_of(path).unwrap_or_else(|| path.to_path_buf());
        if is_log(changed) {
            Touched::Log(Some(main(changed)))
        } else if arrived && changed.is_dir() {
            Touched::Logs(self.logs_in(changed).iter().map(|log| main(log.path())).collect())
        } else {
            Touched::Other
        }
    }

    fn head(&self, s: &SessionRef) -> Head {
        let (mut project, mut id, mut client) = (String::new(), String::new(), None);
        if let Ok(reader) = tokprof_logio::open(s.path()) {
            for line in reader.lines().take(200).map_while(Result::ok) {
                if !line.contains("\"cwd\"") && !line.contains("essionId") && !line.contains("\"entrypoint\"") {
                    continue;
                }
                let Ok(v) = serde_json::from_str::<Value>(&line) else { continue };
                if project.is_empty()
                    && let Some(cwd) = v.get("cwd").and_then(Value::as_str)
                {
                    project = display_path(cwd, None, &self.home);
                }
                if id.is_empty()
                    && let Some(i) = v.get("sessionId").or_else(|| v.get("id")).and_then(Value::as_str)
                {
                    id = i.to_string();
                }
                if client.is_none() {
                    client = v.get("entrypoint").and_then(Value::as_str).map(str::to_string);
                }
                if !project.is_empty() && !id.is_empty() && client.is_some() {
                    break;
                }
            }
        }
        if id.is_empty() {
            id = s.session_id.clone();
        }
        Head { project, session_id: id, client, ..Head::default() }
    }

    fn subagents(&self, s: &SessionRef) -> Vec<SessionRef> {
        if s.subagent {
            return Vec::new();
        }
        let mut out = self.logs_in(&s.path().with_extension("").join("subagents"));
        out.retain(|r| r.subagent);
        out
    }

    fn open(&self, s: &SessionRef) -> Result<Box<dyn Records>> {
        Ok(Box::new(Lines(s.path().to_path_buf())))
    }

    fn decoder(&self, s: &SessionRef) -> Box<dyn Decoder> {
        let path = s.path();
        let mut p = Parser::new(self.agent.clone(), stem(path), s.subagent, self.home.clone());
        if s.subagent {
            let file = path.with_extension("meta.json");
            p.agent = read_meta(&file);
            p.meta_file = Some(file);
        } else {
            let dir = path.with_extension("").join("subagents");
            p.metas = read_metas(&dir);
            p.metas_dir = Some(dir);
        }
        Box::new(p)
    }

    fn activity(&self, s: &SessionRef) -> Result<Activity> {
        activity::read(s.path())
    }

    fn last_active(&self, s: &SessionRef) -> Option<i64> {
        last_timestamp(s.path())
    }

    fn first_active(&self, s: &SessionRef) -> Option<i64> {
        first_timestamp(s.path())
    }
}

/// The time of the first of the log's first 500 lines with a top-level `timestamp`.
fn first_timestamp(path: &Path) -> Option<i64> {
    let reader = tokprof_logio::open(path).ok()?;
    reader.lines().take(500).map_while(Result::ok).filter(|l| l.contains("\"timestamp\"")).find_map(|l| {
        let v: Value = serde_json::from_str(&l).ok()?;
        tokprof_logio::parse_ts_ms(v.get("timestamp"))
    })
}

/// The time of the last line of a transcript with a top-level `timestamp`. Claude Code writes
/// `cost-state`, `last-prompt`, `ai-title`, `mode`, `tools-used` and other bookkeeping lines
/// without one, also when the IDE resumes a session that then does nothing, so they are passed
/// over, and so is a line that does not parse, such as one still being written.
fn last_timestamp(path: &Path) -> Option<i64> {
    let mut found = None;
    tokprof_logio::for_each_line_back(path, |line| {
        if !line.contains("\"timestamp\"") {
            return ControlFlow::Continue(());
        }
        let Ok(v) = serde_json::from_str::<Value>(line) else { return ControlFlow::Continue(()) };
        found = parse_ts_ms(v.get("timestamp"));
        if found.is_some() { ControlFlow::Break(()) } else { ControlFlow::Continue(()) }
    })
    .ok()?;
    found
}

/// The lines of a transcript.
struct Lines(PathBuf);

impl Records for Lines {
    fn for_each(&mut self, f: &mut dyn FnMut(&str)) -> Result<()> {
        tokprof_logio::for_each_line(&self.0, f)
    }

    fn for_each_from(&mut self, from: u64, check: &[u8], f: &mut dyn FnMut(&str)) -> Result<Option<ReadOn>> {
        let read = tokprof_logio::for_each_line_from(&self.0, from, check, f)?;
        Ok(read.map(|r| ReadOn { end: r.end, check: r.check, open: r.open }))
    }
}

fn is_log(path: &Path) -> bool {
    path.extension().is_some_and(|e| e == "jsonl")
}

fn is_subagent(path: &Path) -> bool {
    path.components().any(|c| c.as_os_str() == "subagents")
}

fn stem(path: &Path) -> String {
    path.file_stem().and_then(|s| s.to_str()).unwrap_or_default().to_string()
}

/// The main transcript of the session a subagent transcript belongs to:
/// `<session>/subagents/agent-<id>.jsonl` → `<session>.jsonl`.
fn parent_of(path: &Path) -> Option<PathBuf> {
    let session = path.ancestors().find(|a| a.file_name().is_some_and(|n| n == "subagents"))?.parent()?;
    let mut main = session.as_os_str().to_owned();
    main.push(".jsonl");
    Some(main.into())
}

fn read_meta(path: &Path) -> Option<AgentMeta> {
    let v: Value = serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()?;
    let text = |key: &str| str_at(&v, key).map(str::trim).filter(|s| !s.is_empty()).map(str::to_string);
    Some(AgentMeta { agent_type: text("agentType"), description: text("description"), tool_use_id: text("toolUseId") })
}

/// What the meta files of a session's subagents say, by agent id: `agent-<id>.meta.json`.
fn read_metas(dir: &Path) -> HashMap<String, AgentMeta> {
    let mut out = HashMap::new();
    let Ok(entries) = std::fs::read_dir(dir) else { return out };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(id) = name.to_str().and_then(|n| n.strip_prefix("agent-")?.strip_suffix(".meta.json")) else { continue };
        if let Some(meta) = read_meta(&entry.path()) {
            out.insert(id.to_string(), meta);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_names_that_ship_with_claude_code_are_listed() {
        let n = &PUBLIC_NAMES;
        assert!(n.tools.contains(&"Bash") && n.tools.contains(&"Read") && n.tools.contains(&"Task"), "built-in tools and an alias");
        assert!(n.origin_name("agent", "general-purpose") && n.origin_name("command", "/goal") && n.origin_name("skill", "code-review"));
        assert!(n.origin_name("command", "/simplify"), "a bundled skill typed as a command");
        assert!(!n.origin_name("skill", "walrus-review") && !n.origin_name("command", "/walrus-sync") && !n.origin_name("agent", "ocelot"));
        assert_eq!(n.mcp_server("mcp__plugin_slack_slack__send"), Some("plugin_slack_slack"));
    }

    /// The last activity of a transcript with these lines.
    fn last(name: &str, lines: &[&str]) -> Option<i64> {
        let path = std::env::temp_dir().join(format!("tokprof-claude-last-{}-{name}.jsonl", std::process::id()));
        std::fs::write(&path, lines.join("\n")).unwrap();
        let t = last_timestamp(&path);
        std::fs::remove_file(&path).ok();
        t
    }

    fn ms(ts: &str) -> Option<i64> {
        parse_ts_ms(Some(&Value::String(ts.to_string())))
    }

    #[test]
    fn the_head_names_the_first_client() {
        let path = std::env::temp_dir().join(format!("tokprof-claude-head-{}.jsonl", std::process::id()));
        let lines = [
            r#"{"type":"file-history-snapshot","messageId":"x"}"#,
            r#"{"type":"user","sessionId":"s1","cwd":"/h/src/app","entrypoint":"cli","message":{"role":"user","content":"go"}}"#,
            r#"{"type":"user","sessionId":"s1","cwd":"/h/src/app","entrypoint":"sdk-ts","message":{"role":"user","content":"on"}}"#,
        ];
        std::fs::write(&path, lines.join("\n")).unwrap();
        let source = ClaudeCode::at(PathBuf::from("/h/.claude/projects"), Path::new("/h"));
        let file = SessionRef {
            source: "claude-code",
            agent: ClaudeCode::agent(),
            locator: Locator::File(path.clone()),
            session_id: "s1".into(),
            main_id: "s1".into(),
            subagent: false,
            size: 0,
            modified_ns: 0,
            created_ms: None,
        };
        let head = source.head(&file);
        std::fs::remove_file(&path).ok();
        assert_eq!((head.project.as_str(), head.session_id.as_str(), head.client.as_deref()), ("~/src/app", "s1", Some("cli")));
    }

    #[test]
    fn records_without_a_time_are_no_activity() {
        let resumed = [
            r#"{"type":"user","sessionId":"s","timestamp":"2026-11-02T08:27:39.981Z","message":{"role":"user","content":"go"}}"#,
            r#"{"type":"cost-state","sessionId":"s","totalCostUSD":1.98,"totalDuration":24366969,"startTime":1793607245463}"#,
            r#"{"type":"last-prompt","sessionId":"s","lastPrompt":"go"}"#,
            r#"{"type":"ai-title","sessionId":"s","aiTitle":"a title with a \"timestamp\" in it"}"#,
            "",
        ];
        assert_eq!(last("resumed", &resumed), ms("2026-11-02T08:27:39.981Z"), "an idle-resume tail gives the timed line before it");

        let cut = [
            r#"{"type":"assistant","sessionId":"s","timestamp":"2026-11-02T15:13:15.635Z","message":{"id":"m1"}}"#,
            r#"{"type":"assistant","sessionId":"s","timestamp":"2026-11-02T15:13:19.922Z","message":{"id"#,
        ];
        assert_eq!(last("cut", &cut), ms("2026-11-02T15:13:15.635Z"), "a cut-off last line is passed over");

        let untimed = [r#"{"type":"cost-state","sessionId":"s","totalCostUSD":0.5}"#, r#"{"type":"mode","sessionId":"s","mode":"default"}"#];
        assert_eq!(last("untimed", &untimed), None, "no timed line gives nothing");
    }

    #[test]
    fn setup_hints_name_claude_code_settings() {
        let info = ClaudeCode::agent();
        let hint = |source: &str| info.info().setup_hints.iter().find(|(s, _)| s == source).map(|(_, h)| h.clone());
        assert!(hint("skills").is_some_and(|h| h.contains("skillListingBudgetFraction") && h.contains("skillOverrides")));
        assert!(hint("instructions").is_some_and(|h| h.contains("MEMORY.md")));
        assert!(hint("deferred tool lists").is_some_and(|h| h.contains("/mcp")));
        assert_eq!(hint("hook context").as_deref(), Some("make the hooks print less"));
        assert_eq!(hint("tool schemas"), None);
    }
}
