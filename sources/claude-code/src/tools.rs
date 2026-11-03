//! What a Claude Code tool call worked on, for the `target` profile dimension.

use std::path::Path;

use serde_json::Value;
use tokprof_core::labels::{command_head, display_path, exit_line, first_line, url_host};
use tokprof_core::model::{CallEnd, Outcome, WakeSource};
use tokprof_core::trigger::{CallClass, command_class};

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

/// Whether a Claude Code tool asks the user and waits for the answer: a question, or a plan to
/// approve. Seen in 2.1.310–2.1.316.
pub(crate) fn asks_the_user(tool: &str) -> bool {
    matches!(tool, "AskUserQuestion" | "ExitPlanMode")
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

#[cfg(test)]
mod tests {
    use super::*;

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
    fn a_search_that_found_nothing_worked() {
        // Claude Code sets `returnCodeInterpretation` and no `is_error` for grep's exit code 1.
        assert_eq!(outcome(false, "No matches found", false), (Outcome::Worked, None));
    }
}
