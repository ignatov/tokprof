//! The log sources tokprof reads, as one catalog: each compiled-in source in registry order, a
//! cargo feature each.
//!
//! A new source is a crate under `sources/`, a feature here, and one line in [`catalog`].
//!
//! Spend of the last week, and the X-ray of the latest session:
//!
//! ```no_run
//! use tokprof_core::report::Collector;
//! use tokprof_core::{OnUnreadable, Pricing, Query, analyze, scan, xray};
//!
//! # fn main() -> anyhow::Result<()> {
//! let catalog = tokprof_sources::catalog();
//! let now_ms = 1_793_620_800_000; // the caller's clock
//! let query = Query { agents: catalog.agents(), since_ms: Some(now_ms - 7 * 86_400_000), ..Query::default() };
//! let pricing = Pricing::defaults();
//! let files = catalog.discover(&query.agents);
//! let window = query.window();
//! let (spend, stats) = scan(
//!     &catalog,
//!     &files,
//!     &query,
//!     OnUnreadable::Skip,
//!     Collector::default,
//!     |c, session, _| analyze(session, &pricing, &window, c),
//!     |mut a, b| {
//!         a.merge(b);
//!         a
//!     },
//! )?;
//! println!("{} models, {} logs unreadable", spend.spend.models.len(), stats.unreadable.len());
//! if let Some(latest) = xray::latest(&catalog, &query) {
//!     let x = xray::xray(&catalog, &latest, &pricing, now_ms)?;
//!     println!("{} {}: {} requests", x.agent, x.short_id, x.requests);
//! }
//! # Ok(())
//! # }
//! ```

use std::path::Path;

use tokprof_core::catalog::Catalog;
#[allow(unused_imports)]
use tokprof_core::source::LogSource;

/// Every compiled-in source, reading the locations its environment variables name.
pub fn catalog() -> Catalog {
    let home = std::env::home_dir().unwrap_or_default();
    catalog_at(&home)
}

/// Every compiled-in source, with `home` as the home directory.
// One push per feature, so a source that is left out leaves no gap.
#[allow(clippy::vec_init_then_push)]
pub fn catalog_at(home: &Path) -> Catalog {
    #[allow(unused_mut)]
    let mut sources: Vec<Box<dyn LogSource>> = Vec::new();
    #[cfg(feature = "claude-code")]
    sources.push(Box::new(tokprof_source_claude_code::ClaudeCode::from_env(home)));
    #[cfg(feature = "codex")]
    sources.push(Box::new(tokprof_source_codex::Codex::from_env(home)));
    Catalog::new(sources, home.to_path_buf())
}
