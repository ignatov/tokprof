//! Profiles token spend and prompt-cache behavior from AI coding-agent session logs.
//!
//! The library reads logs only through [`LogSource`]s in a [`Catalog`]; the sources live in
//! their own crates, and `tokprof-sources` builds a catalog of all of them. A [`Query`] picks the
//! sessions, [`scan()`] reads them into any [`Sink`], and the views ([`xray::XRay`],
//! [`bench::Bench`], [`report::Collector`]) hold what the reports show. The library reads no
//! environment variable and no clock: paths and the time come from the caller.

pub mod advice;
pub mod agents;
pub mod analysis;
pub mod api;
pub mod bench;
pub mod busy;
pub mod calendar;
pub mod catalog;
pub mod compare;
pub mod daystore;
pub mod export;
pub mod fmt;
pub mod forecast;
pub mod handoff;
pub mod jfr;
mod kept;
pub mod labels;
pub mod model;
pub mod month;
pub mod overview;
pub mod pauses;
pub mod place;
pub mod pool;
pub mod pricing;
pub mod profile;
pub mod query;
pub mod report;
pub mod rhythm;
pub mod savings;
pub mod scan;
pub mod setup;
pub mod share;
pub mod source;
#[cfg(feature = "testkit")]
pub mod testkit;
pub mod timeline;
pub mod tools;
pub mod trigger;
pub mod week;
pub mod xray;

pub use analysis::{Sink, analyze};
pub use catalog::Catalog;
pub use model::Session;
pub use pricing::Pricing;
pub use query::Query;
pub use scan::{OnUnreadable, ScanStats, scan};
pub use source::{AgentId, AgentInfo, LogSource, SessionRef};

/// The time tests run at, 2026-11-02 12:00 UTC, so that no result depends on the day they run.
#[cfg(test)]
pub(crate) const TEST_NOW_MS: i64 = 1_793_620_800_000;

/// Agents for tests, with the facts the real sources give them.
#[cfg(test)]
pub(crate) mod testing {
    use crate::source::{AgentId, AgentInfo, Harness, Pickup, PublicNames};

    /// Some of the names that ship with each agent, as its source lists them.
    const CLAUDE_NAMES: PublicNames = PublicNames {
        tools: &["Agent", "Bash", "Edit", "Grep", "Read", "Write"],
        commands: &["goal", "model"],
        skills: &["simplify"],
        subagent_types: &["general-purpose", "Explore"],
        mcp: Some(("mcp__", "__")),
    };
    const CODEX_NAMES: PublicNames =
        PublicNames { tools: &["apply_patch", "exec_command"], commands: &[], skills: &[], subagent_types: &["default"], mcp: Some(("mcp__", "__")) };

    pub fn claude() -> AgentId {
        AgentId::new(AgentInfo {
            short_id_len: 8,
            title: "Claude Code".into(),
            instruction_files: vec!["CLAUDE.md".into()],
            ttl_settings: Some(("promptCacheTtl".into(), "subagentPromptCacheTtl".into())),
            setup_hints: vec![(
                "skills".into(),
                "lower skillListingBudgetFraction (default 0.01 of the context window), or turn unused skills off with skillOverrides in settings.json".into(),
            )],
            cost_counter: Some("Claude Code's own cost counter".into()),
            harness: Harness { pickup: Some(Pickup { taken_tokens: 8_000.0, requests: 4.0, output_tokens: 2_000.0 }), compaction_reads_cache: true },
            public_names: CLAUDE_NAMES,
            ..AgentInfo::named("claude")
        })
    }

    pub fn codex() -> AgentId {
        AgentId::new(AgentInfo {
            short_id_len: 18,
            title: "Codex".into(),
            instruction_files: vec!["AGENTS.md".into()],
            harness: Harness { pickup: Some(Pickup { taken_tokens: 30_000.0, requests: 6.0, output_tokens: 4_000.0 }), compaction_reads_cache: false },
            public_names: CODEX_NAMES,
            ..AgentInfo::named("codex")
        })
    }

    /// The instruction files of the agents the real registry declares.
    pub fn instruction_files() -> &'static [String] {
        static FILES: std::sync::LazyLock<Vec<String>> = std::sync::LazyLock::new(|| vec!["CLAUDE.md".into(), "AGENTS.md".into()]);
        &FILES
    }
}
