//! The system clipboard, through the first tool that works: pbcopy, wl-copy, xclip or clip.exe.

use std::io::Write;
use std::process::{Command, Stdio};

/// The tools in the order they are tried, as the messages for a machine without one name them.
pub const TOOLS: &str = "pbcopy, wl-copy, xclip, clip.exe";

/// How a tool wants the text on its stdin.
#[derive(Clone, Copy)]
enum Input {
    Utf8,
    /// UTF-16LE: clip.exe keeps letters beyond ASCII only so. A byte order mark in front would
    /// stay in the clipboard as U+FEFF, so the text goes without one.
    Utf16,
}

const ORDER: [(&str, &[&str], Input); 4] = [
    ("pbcopy", &[], Input::Utf8),
    ("wl-copy", &[], Input::Utf8),
    ("xclip", &["-selection", "clipboard"], Input::Utf8),
    ("clip.exe", &[], Input::Utf16),
];

/// Copies text to the clipboard; returns the tool that did it, or `None` if none worked.
pub fn copy(text: &str) -> Option<&'static str> {
    for (tool, args, input) in ORDER {
        let Ok(mut child) = Command::new(tool).args(args).stdin(Stdio::piped()).stdout(Stdio::null()).stderr(Stdio::null()).spawn() else {
            continue;
        };
        if let Some(mut stdin) = child.stdin.take() {
            let _ = stdin.write_all(&bytes(text, input));
        }
        if child.wait().is_ok_and(|s| s.success()) {
            return Some(tool);
        }
    }
    None
}

fn bytes(text: &str, input: Input) -> Vec<u8> {
    match input {
        Input::Utf8 => text.as_bytes().to_vec(),
        Input::Utf16 => text.encode_utf16().flat_map(u16::to_le_bytes).collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clip_exe_gets_utf16_without_a_byte_order_mark() {
        let text = "tokprof · Привет";
        let b = bytes(text, Input::Utf16);
        assert_eq!(b[..4], [b't', 0, b'o', 0]);
        let units: Vec<u16> = b.chunks(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
        assert_eq!(String::from_utf16(&units).unwrap(), text);
        assert_eq!(bytes(text, Input::Utf8), text.as_bytes(), "the other tools get UTF-8");
    }

    #[test]
    fn the_messages_name_the_tools_in_their_order() {
        assert_eq!(ORDER.map(|(tool, _, _)| tool).join(", "), TOOLS);
    }

    /// The round trip through the Windows clipboard, clip.exe in and Get-Clipboard out. It runs
    /// only in CI, so that a developer's clipboard is left alone.
    #[cfg(windows)]
    #[test]
    fn the_windows_clipboard_keeps_cyrillic() {
        if std::env::var_os("CI").is_none() {
            return;
        }
        let text = "tokprof · Привет";
        assert_eq!(copy(text), Some("clip.exe"));
        let out = Command::new("powershell")
            .args(["-NoProfile", "-NonInteractive", "-Command", "[Console]::OutputEncoding = [Text.Encoding]::UTF8; Get-Clipboard -Raw"])
            .output()
            .unwrap();
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
        assert_eq!(String::from_utf8_lossy(&out.stdout).trim_end_matches(['\r', '\n']), text);
    }
}
