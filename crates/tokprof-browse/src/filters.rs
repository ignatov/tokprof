//! The filters of the command line as both programs read them: `--since`, `--until` and
//! `--project`.

use anyhow::{Context, Result};
use tokprof_core::labels::display_path;

/// The start of a local date, YYYY-MM-DD, in epoch ms.
pub fn parse_date(s: &str) -> Result<i64> {
    let d = chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").with_context(|| format!("bad date {s:?}, expected YYYY-MM-DD"))?;
    let t = d.and_hms_opt(0, 0, 0).context("bad date")?.and_local_timezone(chrono::Local).earliest().context("bad local date")?;
    Ok(t.timestamp_millis())
}

/// `--since`: a date, or a count of days back from now, as in `7d`.
pub fn parse_since(s: &str) -> Result<i64> {
    match s.strip_suffix('d').and_then(|n| n.parse::<i64>().ok()) {
        Some(days) => Ok(chrono::Utc::now().timestamp_millis() - days * 86_400_000),
        None => parse_date(s),
    }
}

/// `--project` as the projects are written: the home directory as `~`, no trailing `/`, so that a
/// path the shell expanded matches.
pub fn normalize_project(p: &str) -> String {
    let p = if p.len() > 1 { p.trim_end_matches('/') } else { p };
    display_path(p, None, &crate::paths::home())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_filters_read_like_the_projects() {
        let home = std::env::home_dir().unwrap().to_string_lossy().to_string();
        assert_eq!(normalize_project(&format!("{home}/src/app")), "~/src/app");
        assert_eq!(normalize_project("~/src/app/"), "~/src/app");
        assert_eq!(normalize_project("billing"), "billing");
        assert_eq!(normalize_project("/"), "/");
    }
}
