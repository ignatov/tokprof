//! How paths, commands and hosts are written in reports and profile frames.

use std::path::Path;

/// Shows a path relative to the project, or to the home directory.
pub fn display_path(path: &str, cwd: Option<&str>, home: &Path) -> String {
    if let Some(cwd) = cwd
        && let Some(rest) = path.strip_prefix(cwd) {
            if rest.is_empty() {
                return ".".to_string();
            }
            if let Some(rest) = rest.strip_prefix('/') {
                return rest.to_string();
            }
        }
    if let Some(home) = home.to_str().filter(|h| !h.is_empty())
        && let Some(rest) = path.strip_prefix(home) {
            return format!("~{rest}");
        }
    path.to_string()
}

/// First words of a shell command: `cargo test`, `git status`, `python3 -I`.
pub fn command_head(cmd: &str) -> String {
    let mut cmd = cmd.trim();
    if cmd.starts_with("cd ")
        && let Some((_, rest)) = cmd.split_once("&&") {
            cmd = rest.trim();
        }
    let words: Vec<&str> = cmd
        .split_whitespace()
        .skip_while(|w| w.contains('=') && !w.starts_with('-'))
        .take(2)
        .collect();
    let mut head = words
        .iter()
        .enumerate()
        .map(|(i, w)| if i == 0 { w.rsplit('/').next().unwrap_or(w) } else { w })
        .collect::<Vec<_>>()
        .join(" ");
    if head.len() > 60 {
        head.truncate(head.char_indices().nth(60).map_or(head.len(), |(i, _)| i));
    }
    head
}

pub fn url_host(url: &str) -> Option<String> {
    let rest = url.split_once("://").map_or(url, |(_, r)| r);
    rest.split(['/', '?', '#']).next().filter(|h| !h.is_empty()).map(str::to_string)
}

/// How a typed message is shown: its last paragraph, since a long message usually starts with
/// pasted text and ends with the request, with its whitespace joined and cut to 200 characters.
pub fn message_label(text: &str) -> String {
    let paragraphs: Vec<&str> = text.split("\n\n").map(str::trim).filter(|p| !p.is_empty()).collect();
    let last = paragraphs.last().copied().unwrap_or_default();
    let joined = last.split_whitespace().collect::<Vec<_>>().join(" ");
    joined.chars().take(200).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_heads() {
        assert_eq!(command_head("cd /tmp && cargo test --all"), "cargo test");
        assert_eq!(command_head("RUST_LOG=debug /usr/bin/python3 -I x.py"), "python3 -I");
    }

    #[test]
    fn paths_are_shown_relative_to_the_project() {
        assert_eq!(display_path("/w/p/src/a.rs", Some("/w/p"), Path::new("/h")), "src/a.rs");
        assert_eq!(display_path("/w/p", Some("/w/p"), Path::new("/h")), ".");
        assert_eq!(display_path("/etc/hosts", Some("/w/p"), Path::new("/h")), "/etc/hosts");
    }

    #[test]
    fn a_message_is_labeled_by_its_last_paragraph() {
        assert_eq!(message_label("Tool results take 47%…\n  advice ▸ trim\n\nwhat else can we\n analyze?"), "what else can we analyze?");
        assert_eq!(message_label("  fix the parser  "), "fix the parser");
        assert_eq!(message_label(&"x".repeat(300)).len(), 200);
        assert_eq!(message_label(""), "");
    }
}
