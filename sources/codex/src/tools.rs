//! What a Codex tool call worked on, for the `target` profile dimension, for the tools that
//! `call_target` does not know itself.

use std::path::Path;

use serde_json::Value;

pub(crate) fn tool_target(tool: &str, _input: &Value, _cwd: Option<&str>, _home: &Path) -> Option<String> {
    let rest = tool.strip_prefix("mcp__")?;
    rest.split("__").next().filter(|server| !server.is_empty()).map(|server| format!("mcp:{server}"))
}
