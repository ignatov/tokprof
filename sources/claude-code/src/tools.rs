//! What a Claude Code tool call worked on, for the `target` profile dimension.

use std::path::Path;

use serde_json::Value;
use tokprof_core::labels::{command_head, display_path, url_host};
use tokprof_core::trigger::{CallClass, command_class};

pub(crate) fn tool_target(tool: &str, input: &Value, cwd: Option<&str>, home: &Path) -> Option<String> {
    let s = |k: &str| input.get(k).and_then(Value::as_str);
    if let Some(rest) = tool.strip_prefix("mcp__") {
        return rest.split("__").next().map(|server| format!("mcp:{server}"));
    }
    let target = match tool {
        "Read" | "Edit" | "Write" | "MultiEdit" | "NotebookEdit" | "NotebookRead" => {
            s("file_path").or(s("notebook_path")).map(|p| display_path(p, cwd, home))
        }
        "Grep" | "Glob" | "LS" => s("path").map(|p| display_path(p, cwd, home)).or(Some("(project)".to_string())),
        "Bash" | "BashOutput" => s("command").or(s("cmd")).map(command_head),
        "WebFetch" => s("url").and_then(url_host),
        "Agent" | "Task" => s("subagent_type").or(s("agent_type")).map(str::to_string),
        "Skill" => s("skill").or(s("command")).map(str::to_string),
        _ => None,
    };
    target.filter(|t| !t.is_empty())
}

/// What a Claude Code tool call did, for the trigger and activity of the request after it.
pub(crate) fn call_class(tool: &str, input: &Value) -> CallClass {
    match tool {
        "Read" | "NotebookRead" | "Grep" | "Glob" | "LS" => CallClass::Reading,
        "WebFetch" | "WebSearch" => CallClass::Web,
        "Edit" | "MultiEdit" | "Write" | "NotebookEdit" => CallClass::Editing,
        "Agent" | "Task" => CallClass::Subagents,
        "Bash" => input.get("command").and_then(Value::as_str).map_or(CallClass::System, command_class),
        _ => CallClass::OtherTools,
    }
}
