//! The lines tokprof writes about the logs it read: which ones and how long it took, and the logs
//! it could not read.

use tokprof_core::catalog::Catalog;
use tokprof_core::source::{AgentId, SessionRef};

/// The first logs that could not be read, as warnings, then how many more there were.
pub fn warnings(errors: &[String]) -> Vec<String> {
    const SHOWN: usize = 5;
    let mut lines: Vec<String> = errors.iter().take(SHOWN).map(|e| format!("warning: {e}")).collect();
    match errors.len().saturating_sub(SHOWN) {
        0 => {}
        1 => lines.push("warning: 1 more log could not be read".to_string()),
        n => lines.push(format!("warning: {n} more logs could not be read")),
    }
    lines
}

/// What was read, per agent in the catalog's order, and in how many seconds.
pub fn sources(catalog: &Catalog, files: &[SessionRef], secs: f64) -> String {
    let mut agents: Vec<&AgentId> = files.iter().map(|f| &f.agent).collect();
    agents.sort_by_key(|a| catalog.order(a));
    agents.dedup();
    let mut parts = Vec::new();
    for agent in agents {
        let (n, bytes) = files.iter().filter(|f| &f.agent == agent).fold((0, 0u64), |(n, b), f| (n + 1, b + f.size));
        if n > 0 {
            parts.push(format!("{} {n} files ({:.1} GB)", agent.name(), bytes as f64 / 1e9));
        }
    }
    format!("read {} in {secs:.1}s", parts.join(", "))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn warnings_stop_after_five_and_count_the_rest() {
        let errors: Vec<String> = (0..12).map(|i| format!("log{i}: broken")).collect();
        let lines = warnings(&errors);
        assert_eq!(lines.len(), 6);
        assert_eq!((lines[0].as_str(), lines[5].as_str()), ("warning: log0: broken", "warning: 7 more logs could not be read"));
        assert_eq!(warnings(&errors[..6]).last().unwrap(), "warning: 1 more log could not be read");
        assert_eq!(warnings(&errors[..5]).len(), 5);
    }
}
