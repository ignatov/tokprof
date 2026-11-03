//! What a Codex tool call worked on, for the `target` profile dimension, for the tools that
//! `call_target` does not know itself.

use std::path::Path;

use serde_json::Value;
use tokprof_core::labels::{exit_line, first_line};
use tokprof_core::model::{CallEnd, Outcome};

pub(crate) fn tool_target(tool: &str, _input: &Value, _cwd: Option<&str>, _home: &Path) -> Option<String> {
    let rest = tool.strip_prefix("mcp__")?;
    rest.split("__").next().filter(|server| !server.is_empty()).map(|server| format!("mcp:{server}"))
}

/// How a call ended, from its output item: `None` for a call that worked. In code mode an `exec`
/// output starts with `Script failed` when the script threw, and each command it printed is a JSON
/// object with its `exit_code`; a direct command reports `Process exited with code <n>`. Seen in
/// 0.147–0.166.
pub(crate) fn call_end(output: &Value) -> Option<CallEnd> {
    let texts: Vec<&str> = match output {
        Value::String(s) => vec![s.as_str()],
        Value::Array(items) => items.iter().filter_map(|i| i.get("text").and_then(Value::as_str)).collect(),
        _ => Vec::new(),
    };
    let first = texts.first().map_or("", |t| t.trim_start());
    if first.starts_with("aborted by user") {
        return Some(CallEnd { outcome: Outcome::Refused, error: None });
    }
    let failed = |error: Option<String>| Some(CallEnd { outcome: Outcome::Failed, error });
    if first.starts_with("Script failed") {
        let all = texts.join("\n");
        let error = all.split_once("Script error:").and_then(|(_, rest)| first_line(rest)).or_else(|| first_line(&all));
        return failed(error);
    }
    for text in texts.iter().filter(|t| t.contains("\"exit_code\"")) {
        let Ok(result) = serde_json::from_str::<Value>(text.trim()) else { continue };
        if let Some(code) = result.get("exit_code").and_then(Value::as_i64).filter(|&c| c != 0) {
            return failed(Some(exit_line(code, result.get("output").and_then(Value::as_str).unwrap_or_default())));
        }
    }
    let exited = first.lines().find_map(|l| l.trim().strip_prefix("Process exited with code ")).and_then(|c| c.trim().parse::<i64>().ok());
    if let Some(code) = exited.filter(|&c| c != 0) {
        let output = first.split_once("Output:\n").map_or("", |(_, rest)| rest);
        return failed(Some(exit_line(code, output)));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn end(output: Value) -> (Outcome, Option<String>) {
        call_end(&output).map_or((Outcome::Worked, None), |e| (e.outcome, e.error))
    }

    #[test]
    fn a_script_that_threw_failed_with_its_error() {
        let output = json!([{"type": "input_text", "text": "Script failed\nWall time 0.0 seconds\nOutput:\n"}, {"type": "input_text", "text": "Script error:\nSyntaxError: Unexpected token ')'"}]);
        assert_eq!(end(output), (Outcome::Failed, Some("SyntaxError: Unexpected token ')'".to_string())));
    }

    #[test]
    fn a_command_with_an_exit_code_failed_with_its_last_line() {
        let result = json!({"chunk_id": "c26f8a", "exit_code": 1, "output": "Traceback (most recent call last):\n  File \"<stdin>\", line 2\nModuleNotFoundError: No module named 'bs4'\n"});
        let output = json!([{"type": "input_text", "text": "Script completed\nWall time 0.2 seconds\nOutput:\n"}, {"type": "input_text", "text": result.to_string()}]);
        assert_eq!(end(output), (Outcome::Failed, Some("exit code 1: ModuleNotFoundError: No module named 'bs4'".to_string())));
        let fine = json!({"exit_code": 0, "output": "ok"});
        assert_eq!(end(json!([{"type": "input_text", "text": "Script completed\n"}, {"type": "input_text", "text": fine.to_string()}])), (Outcome::Worked, None));
        let direct = "Chunk ID: 60d45d\nWall time: 0.4 seconds\nProcess exited with code 2\nOriginal token count: 9\nOutput:\nls: x: No such file or directory\n";
        assert_eq!(end(json!(direct)), (Outcome::Failed, Some("exit code 2: ls: x: No such file or directory".to_string())));
        assert_eq!(end(json!("Process exited with code 0\nOutput:\nfine")), (Outcome::Worked, None));
    }

    #[test]
    fn a_call_the_user_aborted_was_refused() {
        assert_eq!(end(json!("aborted by user after 0.5s")), (Outcome::Refused, None));
    }
}
