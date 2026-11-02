//! The system clipboard, through the first tool that works: pbcopy, wl-copy or xclip.

use std::io::Write;
use std::process::{Command, Stdio};

/// Copies text to the clipboard; returns the tool that did it, or `None` if none worked.
pub fn copy(text: &str) -> Option<&'static str> {
    let tools: [(&str, &[&str]); 3] = [("pbcopy", &[]), ("wl-copy", &[]), ("xclip", &["-selection", "clipboard"])];
    for (tool, args) in tools {
        let Ok(mut child) = Command::new(tool).args(args).stdin(Stdio::piped()).stdout(Stdio::null()).stderr(Stdio::null()).spawn() else {
            continue;
        };
        if let Some(mut stdin) = child.stdin.take() {
            let _ = stdin.write_all(text.as_bytes());
        }
        if child.wait().is_ok_and(|s| s.success()) {
            return Some(tool);
        }
    }
    None
}
