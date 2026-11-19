//! What a Claude Code tool call worked on, for the `target` profile dimension.

use std::path::Path;

use serde_json::Value;
use tokprof_core::labels::{command_head, display_path, exit_line, first_line, url_host};
use tokprof_core::model::{CallEnd, Outcome, WakeSource};
use tokprof_core::trigger::{CallClass, VcsActions, command_class, vcs_actions};

/// Openings of a result whose call the user said no to or stopped. Seen in 2.1.287–2.1.316.
const REFUSALS: &[&str] = &[
    "The user doesn't want to proceed with this tool use",
    "User rejected",
    "[Request interrupted by user for tool use]",
    "MCP tool call has been cancelled likely by a user interaction",
];

/// How a call ended, from its `tool_result`: `is_error`, the text, and whether the line that holds
/// it has `permissionDecision.decision` `reject` (seen in 2.1.300–2.1.316). `None` for a call
/// that worked.
pub(crate) fn call_end(is_error: bool, text: &str, rejected: bool) -> Option<CallEnd> {
    if !is_error && !rejected {
        return None;
    }
    let t = text.trim_start();
    let denied = rejected
        || (t.starts_with("Permission to use ") && t.contains(" has been denied"))
        || t.starts_with("Permission for this command was denied by a built-in");
    let (outcome, error) = if REFUSALS.iter().any(|r| t.starts_with(r)) {
        (Outcome::Refused, None)
    } else if denied {
        (Outcome::Denied, None)
    } else {
        (Outcome::Failed, error_line(t))
    };
    Some(CallEnd { outcome, error })
}

/// The line that tells why a call failed. A command's result starts with `Exit code <n>`, and
/// then its output tells why; other results say it in their first line, sometimes inside
/// `<tool_use_error>` tags.
fn error_line(text: &str) -> Option<String> {
    let text = text.trim();
    let text = text.strip_prefix("<tool_use_error>").map_or(text, |t| t.trim_end().trim_end_matches("</tool_use_error>"));
    let (first, rest) = text.split_once('\n').unwrap_or((text, ""));
    if let Some(code) = first.trim().strip_prefix("Exit code ").and_then(|c| c.trim().parse::<i64>().ok()) {
        return Some(exit_line(code, rest));
    }
    first_line(text)
}

/// What a `<task-notification>` reports, by its `<summary>`. Seen in 2.1.291–2.1.316.
pub(crate) fn wake_source(summary: &str) -> WakeSource {
    let s = summary.trim_start();
    if s.starts_with("Agent \"") {
        WakeSource::Subagent
    } else if s.starts_with("Background command \"") || s.starts_with("Monitor ") {
        WakeSource::Command
    } else {
        WakeSource::Other
    }
}

pub(crate) fn tool_target(tool: &str, input: &Value, cwd: Option<&str>, home: &Path) -> Option<String> {
    let s = |k: &str| input.get(k).and_then(Value::as_str);
    if let Some(rest) = tool.strip_prefix("mcp__") {
        return rest.split("__").next().map(|server| format!("mcp:{server}"));
    }
    let target = match tool {
        "Read" | "Edit" | "Write" | "MultiEdit" | "NotebookEdit" | "NotebookRead" => s("file_path").or(s("notebook_path")).map(|p| display_path(p, cwd, home)),
        "Grep" | "Glob" | "LS" => s("path").map(|p| display_path(p, cwd, home)).or(Some("(project)".to_string())),
        "Bash" | "BashOutput" => s("command").or(s("cmd")).map(command_head),
        "WebFetch" => s("url").and_then(url_host),
        "Agent" | "Task" => s("subagent_type").or(s("agent_type")).map(str::to_string),
        "Skill" => s("skill").or(s("command")).map(str::to_string),
        _ => None,
    };
    target.filter(|t| !t.is_empty())
}

/// The short head of a call's input (see session-logs, Input of a call): the first line of
/// `command`, the path as the target shows it, `pattern`, `url`, `query`, the `description` of a
/// subagent, the skill, or the first string value of an MCP tool's input.
pub(crate) fn input_head(tool: &str, input: &Value, target: Option<&str>) -> Option<String> {
    let s = |k: &str| input.get(k).and_then(Value::as_str);
    let text = if tool.starts_with("mcp__") {
        input.as_object().and_then(|m| m.values().find_map(Value::as_str))
    } else {
        match tool {
            "Bash" => s("command"),
            "Read" | "Edit" | "Write" | "MultiEdit" | "NotebookEdit" | "NotebookRead" | "LS" => target,
            "Grep" | "Glob" => s("pattern"),
            "WebFetch" => s("url"),
            "WebSearch" | "ToolSearch" => s("query"),
            "Agent" | "Task" => s("description"),
            "Skill" => target,
            _ => None,
        }
    };
    text.and_then(tokprof_core::model::input_head)
}

/// The MCP server a tool name names: `mcp__<server>__<tool>`.
pub(crate) fn mcp_server(tool: &str) -> Option<&str> {
    tool.strip_prefix("mcp__")?.split("__").next().filter(|s| !s.is_empty())
}

/// Whether a Claude Code tool asks the user and waits for the answer: a question, or a plan to
/// approve. Seen in 2.1.310–2.1.326.
pub(crate) fn asks_the_user(tool: &str) -> bool {
    matches!(tool, "AskUserQuestion" | "ExitPlanMode")
}

/// What a Claude Code tool call did, for the trigger and activity of the request after it.
/// What a call's shell command did to version control: only `Bash` runs commands.
pub(crate) fn call_vcs(tool: &str, input: &Value) -> VcsActions {
    match tool {
        "Bash" => input.get("command").and_then(Value::as_str).map(vcs_actions).unwrap_or_default(),
        _ => VcsActions::default(),
    }
}

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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn outcome(is_error: bool, text: &str, rejected: bool) -> (Outcome, Option<String>) {
        call_end(is_error, text, rejected).map_or((Outcome::Worked, None), |e| (e.outcome, e.error))
    }

    #[test]
    fn a_result_marked_as_an_error_is_a_failure_with_its_line() {
        let missing = "File does not exist. Note: your current working directory is /Users/me/src/app.";
        assert_eq!(outcome(true, missing, false), (Outcome::Failed, Some(missing.to_string())));
        let tests = "Exit code 101\nrunning 3 tests\n\nerror: test failed, to rerun pass --lib\n";
        assert_eq!(outcome(true, tests, false), (Outcome::Failed, Some("exit code 101: error: test failed, to rerun pass --lib".to_string())));
        let tagged = "<tool_use_error>String to replace not found in file.\nString: foo</tool_use_error>";
        assert_eq!(outcome(true, tagged, false).1.as_deref(), Some("String to replace not found in file."));
        assert_eq!(outcome(false, "1\tfn main() {}", false), (Outcome::Worked, None));
    }

    #[test]
    fn the_user_refuses_and_rules_deny() {
        for text in [
            "The user doesn't want to proceed with this tool use. The tool use was rejected (eg. if it was a file edit, the new_string was NOT written to the file).",
            "User rejected command execution",
            "[Request interrupted by user for tool use]",
            "MCP tool call has been cancelled likely by a user interaction: null",
        ] {
            assert_eq!(outcome(true, text, false), (Outcome::Refused, None), "{text}");
        }
        assert_eq!(outcome(true, "Permission to use Bash with command rm -rf build has been denied.", false), (Outcome::Denied, None));
        assert_eq!(
            outcome(true, "Permission for this command was denied by a built-in Claude Code safety check, not by the user.", false),
            (Outcome::Denied, None)
        );
        assert_eq!(outcome(true, "This session is isolated in the worktree /w, but this command is too complex", true), (Outcome::Denied, None));
    }

    #[test]
    fn input_heads_take_the_field_of_each_tool() {
        let input = json!({"command": "cargo test -p tokprof --test golden\necho done", "pattern": "fn main", "url": "https://example.com/a"});
        assert_eq!(input_head("Bash", &input, Some("cargo test")).as_deref(), Some("cargo test -p tokprof --test golden"));
        assert_eq!(input_head("Read", &json!({"file_path": "/w/src/a.rs"}), Some("src/a.rs")).as_deref(), Some("src/a.rs"));
        assert_eq!(input_head("Grep", &input, Some("(project)")).as_deref(), Some("fn main"));
        assert_eq!(input_head("WebFetch", &input, Some("example.com")).as_deref(), Some("https://example.com/a"));
        assert_eq!(input_head("Agent", &json!({"description": "Find the parser", "prompt": "long"}), None).as_deref(), Some("Find the parser"));
        assert_eq!(input_head("mcp__idea__search_symbol", &json!({"limit": 5, "query": "Parser"}), Some("mcp:idea")).as_deref(), Some("Parser"));
        assert_eq!(input_head("TodoWrite", &json!({"todos": []}), None), None);
        let long = "x".repeat(100);
        assert_eq!(input_head("Bash", &json!({"command": long}), None).map(|h| h.chars().count()), Some(60));
        assert_eq!((mcp_server("mcp__plugin_slack_slack__send"), mcp_server("Read")), (Some("plugin_slack_slack"), None));
    }

    #[test]
    fn a_search_that_found_nothing_worked() {
        // Claude Code sets `returnCodeInterpretation` and no `is_error` for grep's exit code 1.
        assert_eq!(outcome(false, "No matches found", false), (Outcome::Worked, None));
    }
}
