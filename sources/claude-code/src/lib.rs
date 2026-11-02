//! Claude Code transcripts: `$CLAUDE_CONFIG_DIR/projects/<project>/<session>.jsonl`, by default
//! under `~/.claude`, with the transcripts of the session's subagents in
//! `<session>/subagents/agent-<id>.jsonl` and what started each in `agent-<id>.meta.json`.

mod activity;
mod decoder;
mod tools;

use std::collections::HashMap;
use std::fs::Metadata;
use std::io::BufRead;
use std::path::{Path, PathBuf};

use anyhow::Result;
use serde_json::Value;
use tokprof_core::labels::display_path;
use tokprof_core::source::{
    Activity, AgentId, AgentInfo, Capabilities, Claim, Decoder, Detail, Harness, Head, Locator, LogSource, OwnCost, Pickup, Records, SessionRef,
    Touched,
};
use tokprof_logio::{file_times, str_at, walk};

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
            cost_counter: Some("Claude Code's own cost counter".to_string()),
            // Measured by the reference bench (docs/reference-bench.md).
            harness: Harness { pickup: Some(Pickup { taken_tokens: 8_000.0, requests: 4.0, output_tokens: 2_000.0 }) },
            capabilities: Capabilities {
                own_cost: OwnCost::Session,
                compaction_trigger: true,
                context_window: false,
                prompt_composition: true,
                titles: true,
                subagent_tasks: true,
                origins: true,
            },
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
        let (mut project, mut id) = (String::new(), String::new());
        if let Ok(reader) = tokprof_logio::open(s.path()) {
            for line in reader.lines().take(200).map_while(Result::ok) {
                if !line.contains("\"cwd\"") && !line.contains("essionId") {
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
                if !project.is_empty() && !id.is_empty() {
                    break;
                }
            }
        }
        if id.is_empty() {
            id = s.session_id.clone();
        }
        Head { project, session_id: id }
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
            p.agent = read_meta(&path.with_extension("meta.json"));
        } else {
            p.metas = read_metas(&path.with_extension("").join("subagents"));
        }
        Box::new(p)
    }

    fn activity(&self, s: &SessionRef) -> Result<Activity> {
        activity::read(s.path())
    }
}

/// The lines of a transcript.
struct Lines(PathBuf);

impl Records for Lines {
    fn for_each(&mut self, f: &mut dyn FnMut(&str)) -> Result<()> {
        tokprof_logio::for_each_line(&self.0, f)
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
